//! Microphone Audio Processor
//!
//! Simple architecture: accumulate audio until candidate stops speaking,
//! then single Whisper API call, immediately add to context.
//! No chunking, no background tasks, no queuing.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use tokio::sync::{broadcast, RwLock};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use tracing::{info, error};
use crate::modules::groq::GroqClient;

type MessageBuffer = Arc<RwLock<VecDeque<String>>>;
type CandidateContext = Arc<RwLock<VecDeque<String>>>;

// Configuration
const SAMPLE_RATE: u32 = 16000;
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(1500);

// Silence detection
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_millis(1500);
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_millis(2500);
const NOISE_CALIBRATION_FRAMES: usize = 50;
const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(10);

// VAD thresholds
const NOISE_FLOOR_MULTIPLIER: f32 = 2.2;
const MIN_SPEECH_FRAMES: usize = 6;
const MIN_SPEECH_ENERGY: f32 = 0.008;
const MAX_NOISE_FLOOR: f32 = 0.020;
const MIN_ZCR: f32 = 0.015;
const MAX_ZCR: f32 = 0.40;

// Context
const MAX_CONTEXT_CHARS: usize = 24000;

pub struct MicProcessor {
    audio_receiver: Receiver<Vec<f32>>,
    groq_client: Arc<GroqClient>,
    tx: broadcast::Sender<String>,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    
    // Audio state
    accumulated_audio: Vec<f32>,
    speech_start: Option<Instant>,
    silence_start: Option<Instant>,
    consecutive_speech_frames: usize,
    consecutive_silence_frames: usize,
    
    // Noise floor
    noise_floor: f32,
    noise_calibration_buffer: Vec<f32>,
    is_calibrated: bool,
    energy_history: VecDeque<f32>,
    last_recalibration: Option<Instant>,
    
    // Speech metrics
    peak_energy: f32,
    avg_speech_energy: f32,
    speech_frame_count: usize,
    
    // Processing flag
    is_processing: bool,
}

impl MicProcessor {
    pub fn new(
        audio_receiver: Receiver<Vec<f32>>,
        groq_client: Arc<GroqClient>,
        tx: broadcast::Sender<String>,
        candidate_context: CandidateContext,
        buffer: MessageBuffer,
        connected: Arc<AtomicUsize>,
    ) -> Self {
        info!("[Candidate] MicProcessor initializing (single-call mode)...");
        Self {
            audio_receiver,
            groq_client,
            tx,
            candidate_context,
            buffer,
            connected,
            accumulated_audio: Vec::with_capacity(SAMPLE_RATE as usize * 300), // 5 min max
            speech_start: None,
            silence_start: None,
            consecutive_speech_frames: 0,
            consecutive_silence_frames: 0,
            noise_floor: 0.003,
            noise_calibration_buffer: Vec::new(),
            is_calibrated: false,
            energy_history: VecDeque::with_capacity(100),
            last_recalibration: None,
            peak_energy: 0.0,
            avg_speech_energy: 0.0,
            speech_frame_count: 0,
            is_processing: false,
        }
    }

    pub async fn run(&mut self) {
        info!("[Candidate] MicProcessor started. Calibrating...");
        
        loop {
            let recv_result = self.audio_receiver.recv_timeout(Duration::from_millis(20));
            let now = Instant::now();

            match recv_result {
                Ok(chunk) => {
                    if !self.is_calibrated {
                        self.calibrate_noise_floor(&chunk);
                    } else if !self.is_processing {
                        self.process_chunk(chunk, now).await;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.is_calibrated && !self.is_processing {
                        self.check_silence_timeout(now).await;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("[Candidate] Mic channel disconnected.");
                    break;
                }
            }
        }
    }

    fn calibrate_noise_floor(&mut self, chunk: &[f32]) {
        self.noise_calibration_buffer.extend_from_slice(chunk);
        if self.noise_calibration_buffer.len() >= NOISE_CALIBRATION_FRAMES * chunk.len() {
            let energy = self.calculate_energy(&self.noise_calibration_buffer);
            self.noise_floor = energy.min(MAX_NOISE_FLOOR);
            self.is_calibrated = true;
            self.last_recalibration = Some(Instant::now());
            info!("[Candidate] Noise floor calibrated: {:.5}", self.noise_floor);
            self.noise_calibration_buffer.clear();
        }
    }

    async fn process_chunk(&mut self, chunk: Vec<f32>, now: Instant) {
        // Recalibrate when silent
        if self.speech_start.is_none() {
            if let Some(last) = self.last_recalibration {
                if now.duration_since(last) >= RECALIBRATION_INTERVAL {
                    let energy = self.calculate_energy(&chunk);
                    self.noise_floor = (0.1 * energy + 0.9 * self.noise_floor).min(MAX_NOISE_FLOOR);
                    self.last_recalibration = Some(now);
                }
            }
        }

        let energy = self.calculate_energy(&chunk);
        let zcr = self.calculate_zcr(&chunk);
        
        self.energy_history.push_back(energy);
        if self.energy_history.len() > 100 { self.energy_history.pop_front(); }
        
        let threshold = (self.noise_floor * NOISE_FLOOR_MULTIPLIER).max(MIN_SPEECH_ENERGY);
        let snr = if self.noise_floor > 0.0 { energy / self.noise_floor } else { 0.0 };
        let is_voice = energy > threshold && zcr >= MIN_ZCR && zcr <= MAX_ZCR && snr >= 1.6;
        
        if is_voice {
            self.consecutive_speech_frames += 1;
            self.consecutive_silence_frames = 0;
            if energy > self.peak_energy { self.peak_energy = energy; }
        } else {
            self.consecutive_speech_frames = 0;
            self.consecutive_silence_frames += 1;
        }

        let speech_confirmed = self.consecutive_speech_frames >= MIN_SPEECH_FRAMES;

        if self.speech_start.is_none() {
            if speech_confirmed {
                info!("[Candidate] Speech START | Energy: {:.4} | SNR: {:.2}", energy, snr);
                self.speech_start = Some(now);
                self.silence_start = None;
                self.peak_energy = energy;
                self.avg_speech_energy = energy;
                self.speech_frame_count = 1;
                self.accumulated_audio.extend_from_slice(&chunk);
            }
        } else {
            // Accumulate audio (no limit - handles up to 5 min)
            self.accumulated_audio.extend_from_slice(&chunk);
            
            if is_voice {
                self.avg_speech_energy = (self.avg_speech_energy * self.speech_frame_count as f32 + energy) / (self.speech_frame_count + 1) as f32;
                self.speech_frame_count += 1;
                self.silence_start = None;
            } else {
                if self.silence_start.is_none() {
                    self.silence_start = Some(now);
                }
                
                let timeout = self.get_silence_timeout();
                if let Some(ss) = self.silence_start {
                    if now.duration_since(ss) >= timeout {
                        let dur = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
                        info!("[Candidate] Speech END | Duration: {:.2}s | Peak: {:.4}", dur, self.peak_energy);
                        self.transcribe_and_store().await;
                        self.reset_state();
                    }
                }
            }
        }
    }

    fn get_silence_timeout(&self) -> Duration {
        let ratio = if self.noise_floor > 0.0 {
            (self.avg_speech_energy / self.noise_floor).min(10.0)
        } else { 5.0 };
        if ratio > 5.0 { EXTENDED_SILENCE_TIMEOUT } else { BASE_SILENCE_TIMEOUT }
    }

    async fn check_silence_timeout(&mut self, now: Instant) {
        if let Some(ss) = self.silence_start {
            let timeout = self.get_silence_timeout();
            if now.duration_since(ss) >= timeout && !self.accumulated_audio.is_empty() {
                let dur = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
                info!("[Candidate] Speech END (timeout) | Duration: {:.2}s", dur);
                self.transcribe_and_store().await;
                self.reset_state();
            }
        }
    }

    /// Single API call → immediately store in context
    async fn transcribe_and_store(&mut self) {
        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
        if Duration::from_secs_f32(duration) < MIN_AUDIO_DURATION {
            return;
        }
        
        let rms = self.calculate_energy(&self.accumulated_audio);
        if rms < (self.noise_floor * 1.2).max(0.004) {
            return;
        }
        
        self.is_processing = true;
        info!("[Candidate] Transcribing {:.2}s of audio...", duration);
        
        let wav_data = self.samples_to_wav(&self.accumulated_audio);
        
        // 10-second hard timeout - if Groq doesn't respond, skip this chunk
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            self.groq_client.transcribe_with_options(&wav_data, "whisper-large-v3", Some(""))
        ).await;
        
        let text = match result {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => {
                error!("[Candidate] Transcription failed: {}", e);
                self.is_processing = false;
                return;
            }
            Err(_) => {
                error!("[Candidate] Transcription timed out after 10s, skipping");
                self.is_processing = false;
                return;
            }
        };
        
        let text = text.trim().to_string();
        let lower = text.to_lowercase();
        
        // Filter hallucinations
        if is_hallucination(&lower) || text.is_empty() || text.len() < 5 {
            if !text.is_empty() {
                info!("[Candidate] Filtered: '{}'", text);
            }
            self.is_processing = false;
            return;
        }
        
        info!("[Candidate] Transcribed: '{}'", text.chars().take(80).collect::<String>());
        
        // IMMEDIATELY add to context
        let mut context = self.candidate_context.write().await;
        context.push_back(text.clone());
        
        // Summarize overflow
        let mut total: usize = context.iter().map(|s| s.len()).sum();
        while total > MAX_CONTEXT_CHARS && context.len() > 2 {
            if let Some(old) = context.pop_front() {
                if old.len() > 100 {
                    let summary: String = old.chars().take(100).collect();
                    let compressed = format!("[Summary] {}...", summary);
                    context.push_front(compressed.clone());
                    total = context.iter().map(|s| s.len()).sum();
                    if total > MAX_CONTEXT_CHARS { context.pop_front(); total = context.iter().map(|s| s.len()).sum(); }
                } else {
                    total -= old.len();
                }
            }
        }
        
        let size = context.len();
        let tokens = total / 4;
        drop(context);
        
        info!("[Candidate] Context updated: {} entries, ~{} tokens", size, tokens);
        
        // Notify UI
        self.send_or_buffer(serde_json::json!({
            "type": "candidate_transcription",
            "text": text,
            "context_size": size,
            "token_estimate": tokens
        }).to_string()).await;
        
        self.is_processing = false;
    }

    fn reset_state(&mut self) {
        self.accumulated_audio.clear();
        self.speech_start = None;
        self.silence_start = None;
        self.consecutive_speech_frames = 0;
        self.consecutive_silence_frames = 0;
        self.peak_energy = 0.0;
        self.avg_speech_energy = 0.0;
        self.speech_frame_count = 0;
    }

    fn calculate_energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() { return 0.0; }
        (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn calculate_zcr(&self, samples: &[f32]) -> f32 {
        if samples.len() < 2 { return 0.0; }
        let mut c = 0;
        for i in 1..samples.len() {
            if (samples[i] >= 0.0) != (samples[i-1] >= 0.0) { c += 1; }
        }
        c as f32 / samples.len() as f32
    }

    fn samples_to_wav(&self, samples: &[f32]) -> Vec<u8> {
        let spec = WavSpec { channels: 1, sample_rate: SAMPLE_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut w = WavWriter::new(&mut cursor, spec).unwrap();
            for &s in samples {
                w.write_sample((s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        cursor.into_inner()
    }

    async fn send_or_buffer(&self, msg: String) {
        if self.connected.load(Ordering::SeqCst) > 0 {
            let _ = self.tx.send(msg);
        } else {
            let mut buf = self.buffer.write().await;
            buf.push_back(msg);
            if buf.len() > 100 { buf.pop_front(); }
        }
    }
}

fn is_hallucination(text: &str) -> bool {
    let h = ["thanks for watching", "thank you for watching", "thanks for listening",
        "thank you", "thanks", "bye", "hmm", "hmmm", "okay", "ok",
        "uh", "uhh", "um", "umm", "uh-huh", "yeah", "yep", "nope",
        "huh", "mhmm", "mm-hmm", "amara.org", "subtitles by",
        "copyright", "all rights reserved"];
    
    if text.len() < 20 && h.iter().any(|&x| text.contains(x)) { return true; }
    if text.contains("technical interview question") || 
       text.contains("programming, databases, algorithms") ||
       text.contains("computer science") { return true; }
    false
}
