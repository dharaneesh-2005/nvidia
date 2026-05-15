//! Microphone Audio Processor
//!
//! Captures candidate's speech using the SAME proven logic as audio_processor.rs.
//! Key difference: supports long answers with NON-BLOCKING 20-second streaming chunks.
//! Transcription happens in background - audio processing never blocks.
//! When candidate stops speaking, waits for all pending transcriptions then finalizes.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use tokio::sync::{broadcast, RwLock, Mutex};
use tokio::task::JoinHandle;
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use tracing::{info, error};
use crate::modules::groq::GroqClient;

type MessageBuffer = Arc<RwLock<VecDeque<String>>>;
type CandidateContext = Arc<RwLock<VecDeque<String>>>;

// Configuration constants
const SAMPLE_RATE: u32 = 16000;
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(1500);
const CHUNK_DURATION: Duration = Duration::from_secs(20);

// Adaptive silence detection
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_millis(1500);
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_millis(2500);
const NOISE_CALIBRATION_FRAMES: usize = 50;
const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(10);

// Voice activity detection thresholds
const NOISE_FLOOR_MULTIPLIER: f32 = 2.2;
const MIN_SPEECH_FRAMES: usize = 6;
const MIN_SPEECH_ENERGY: f32 = 0.008;
const MAX_NOISE_FLOOR: f32 = 0.020;

// Zero-crossing rate
const MIN_ZCR: f32 = 0.015;
const MAX_ZCR: f32 = 0.40;

// Context management
const MAX_CONTEXT_CHARS: usize = 8000;

/// Shared queue for transcription results from background tasks
type TranscriptionQueue = Arc<Mutex<Vec<String>>>;

pub struct MicProcessor {
    audio_receiver: Receiver<Vec<f32>>,
    groq_client: Arc<GroqClient>,
    tx: broadcast::Sender<String>,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    
    // State
    accumulated_audio: Vec<f32>,
    speech_start: Option<Instant>,
    silence_start: Option<Instant>,
    consecutive_speech_frames: usize,
    consecutive_silence_frames: usize,
    
    // Adaptive noise floor
    noise_floor: f32,
    noise_calibration_buffer: Vec<f32>,
    is_calibrated: bool,
    energy_history: VecDeque<f32>,
    last_recalibration: Option<Instant>,
    
    // Speech quality metrics
    peak_energy: f32,
    avg_speech_energy: f32,
    speech_frame_count: usize,
    
    // NON-BLOCKING transcription: results collected here
    transcription_queue: TranscriptionQueue,
    pending_tasks: Vec<JoinHandle<()>>,
    
    // Logging
    chunk_count: usize,
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
        info!("[Candidate] MicProcessor initializing (non-blocking transcription)...");
        Self {
            audio_receiver,
            groq_client,
            tx,
            candidate_context,
            buffer,
            connected,
            accumulated_audio: Vec::with_capacity(SAMPLE_RATE as usize * 30),
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
            transcription_queue: Arc::new(Mutex::new(Vec::new())),
            pending_tasks: Vec::new(),
            chunk_count: 0,
        }
    }

    /// Main run loop - NEVER blocks on transcription
    pub async fn run(&mut self) {
        info!("[Candidate] MicProcessor started. Calibrating noise floor...");
        
        loop {
            let recv_result = self.audio_receiver.recv_timeout(Duration::from_millis(20));
            let now = Instant::now();

            match recv_result {
                Ok(audio_chunk) => {
                    self.chunk_count += 1;
                    if self.chunk_count % 500 == 0 {
                        info!("[Candidate] {} audio chunks processed", self.chunk_count);
                    }
                    
                    if !self.is_calibrated {
                        self.calibrate_noise_floor(&audio_chunk);
                    } else {
                        self.process_chunk(audio_chunk, now).await;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if self.is_calibrated {
                        self.check_silence_timeout(now).await;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("[Candidate] Microphone channel disconnected.");
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

    fn should_recalibrate(&self, now: Instant) -> bool {
        if let Some(last) = self.last_recalibration {
            now.duration_since(last) >= RECALIBRATION_INTERVAL
        } else {
            false
        }
    }

    async fn process_chunk(&mut self, chunk: Vec<f32>, now: Instant) {
        // Continuous recalibration when not speaking
        if self.speech_start.is_none() && self.should_recalibrate(now) {
            self.recalibrate_noise_floor(&chunk, now);
        }

        let energy = self.calculate_energy(&chunk);
        let zcr = self.calculate_zero_crossing_rate(&chunk);
        
        self.energy_history.push_back(energy);
        if self.energy_history.len() > 100 {
            self.energy_history.pop_front();
        }
        
        let dynamic_threshold = (self.noise_floor * NOISE_FLOOR_MULTIPLIER).max(MIN_SPEECH_ENERGY);
        let snr = if self.noise_floor > 0.0 { energy / self.noise_floor } else { 0.0 };
        
        let is_voice_like = energy > dynamic_threshold && 
                           zcr >= MIN_ZCR && zcr <= MAX_ZCR &&
                           snr >= 1.6;
        
        if is_voice_like {
            self.consecutive_speech_frames += 1;
            self.consecutive_silence_frames = 0;
            if energy > self.peak_energy {
                self.peak_energy = energy;
            }
        } else {
            self.consecutive_speech_frames = 0;
            self.consecutive_silence_frames += 1;
        }

        let is_speech_confirmed = self.consecutive_speech_frames >= MIN_SPEECH_FRAMES;

        if self.speech_start.is_none() {
            if is_speech_confirmed {
                info!("[Candidate] Speech START | Energy: {:.4} | SNR: {:.2} | ZCR: {:.3}", 
                      energy, snr, zcr);
                self.speech_start = Some(now);
                self.silence_start = None;
                self.peak_energy = energy;
                self.avg_speech_energy = energy;
                self.speech_frame_count = 1;
                self.accumulated_audio.extend_from_slice(&chunk);
            }
        } else {
            self.accumulated_audio.extend_from_slice(&chunk);
            
            if is_voice_like {
                self.avg_speech_energy = (self.avg_speech_energy * self.speech_frame_count as f32 + energy) / (self.speech_frame_count + 1) as f32;
                self.speech_frame_count += 1;
            }

            if is_speech_confirmed {
                self.silence_start = None;
            } else if !is_voice_like {
                if self.silence_start.is_none() {
                    self.silence_start = Some(now);
                }
                
                let silence_timeout = self.get_adaptive_silence_timeout();
                
                if let Some(silence_start) = self.silence_start {
                    if now.duration_since(silence_start) >= silence_timeout {
                        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
                        info!("[Candidate] Speech END | Duration: {:.2}s | Peak: {:.4}", 
                              duration, self.peak_energy);
                        // Spawn transcription for remaining audio (non-blocking)
                        self.spawn_transcription_task();
                        // Wait for ALL pending transcriptions and finalize
                        self.finalize_answer().await;
                        self.reset_state();
                    }
                }
            }

            // 20-second streaming: spawn transcription in background, keep accumulating
            if let Some(start) = self.speech_start {
                if now.duration_since(start) >= CHUNK_DURATION {
                    info!("[Candidate] 20s chunk - spawning background transcription...");
                    self.spawn_transcription_task();
                    // Clear audio but DON'T block - keep listening
                    self.accumulated_audio.clear();
                    self.speech_start = Some(now);
                }
            }
        }
    }

    fn get_adaptive_silence_timeout(&self) -> Duration {
        let quality_ratio = if self.noise_floor > 0.0 {
            (self.avg_speech_energy / self.noise_floor).min(10.0)
        } else {
            5.0
        };
        
        if quality_ratio > 5.0 {
            EXTENDED_SILENCE_TIMEOUT
        } else {
            BASE_SILENCE_TIMEOUT
        }
    }

    fn recalibrate_noise_floor(&mut self, chunk: &[f32], now: Instant) {
        let energy = self.calculate_energy(chunk);
        let alpha = 0.1;
        self.noise_floor = (alpha * energy + (1.0 - alpha) * self.noise_floor).min(MAX_NOISE_FLOOR);
        self.last_recalibration = Some(now);
    }

    async fn check_silence_timeout(&mut self, now: Instant) {
        if let Some(silence_start) = self.silence_start {
            let silence_timeout = self.get_adaptive_silence_timeout();
            if now.duration_since(silence_start) >= silence_timeout {
                if !self.accumulated_audio.is_empty() {
                    self.spawn_transcription_task();
                }
                // Check if we have any pending transcriptions to finalize
                if !self.pending_tasks.is_empty() || !self.transcription_queue.lock().await.is_empty() {
                    self.finalize_answer().await;
                }
                self.reset_state();
            }
        }
    }

    /// Spawn a background transcription task - DOES NOT BLOCK the audio loop
    fn spawn_transcription_task(&mut self) {
        let audio = std::mem::take(&mut self.accumulated_audio);
        self.accumulated_audio = Vec::with_capacity(SAMPLE_RATE as usize * 30);
        
        let duration = audio.len() as f32 / SAMPLE_RATE as f32;
        if Duration::from_secs_f32(duration) < MIN_AUDIO_DURATION {
            return;
        }
        
        let rms = self.calculate_rms(&audio);
        let min_valid_energy = (self.noise_floor * 1.2).max(0.004);
        if rms < min_valid_energy {
            return;
        }
        
        info!("[Candidate] Spawning transcription task: {:.2}s, RMS: {:.4}", duration, rms);
        
        let groq = self.groq_client.clone();
        let queue = self.transcription_queue.clone();
        let wav_data = self.samples_to_wav(&audio);
        
        let handle = tokio::spawn(async move {
            match groq.transcribe_with_options(&wav_data, "whisper-large-v3", Some("")).await {
                Ok(text) => {
                    let text = text.trim().to_string();
                    let lower = text.to_lowercase();
                    
                    if !is_hallucination(&lower) && !text.is_empty() && text.len() >= 5 {
                        info!("[Candidate] Background transcription done: '{}'", 
                              text.chars().take(60).collect::<String>());
                        queue.lock().await.push(text);
                    } else if is_hallucination(&lower) {
                        info!("[Candidate] Hallucination filtered: '{}'", text);
                    }
                }
                Err(e) => {
                    error!("[Candidate] Background transcription error: {}", e);
                }
            }
        });
        
        self.pending_tasks.push(handle);
    }

    /// Wait for ALL pending transcriptions, combine, and store in context
    async fn finalize_answer(&mut self) {
        if self.pending_tasks.is_empty() {
            return;
        }
        
        let task_count = self.pending_tasks.len();
        info!("[Candidate] Waiting for {} pending transcription(s)...", task_count);
        
        // Wait for all background tasks to complete
        let tasks = std::mem::take(&mut self.pending_tasks);
        for task in tasks {
            let _ = task.await;
        }
        
        // Collect all results from queue
        let mut queue = self.transcription_queue.lock().await;
        let chunks: Vec<String> = queue.drain(..).collect();
        drop(queue);
        
        if chunks.is_empty() {
            info!("[Candidate] No valid transcriptions after filtering");
            return;
        }
        
        let complete_answer = chunks.join(" ");
        
        if complete_answer.len() < 5 {
            return;
        }
        
        info!("[Candidate] Answer finalized ({} chunks): '{}' ({} chars)", 
              chunks.len(),
              complete_answer.chars().take(80).collect::<String>(), 
              complete_answer.len());
        
        // Store in context with token-based management
        let mut context = self.candidate_context.write().await;
        context.push_back(complete_answer.clone());
        
        // Token-based trimming
        let mut total_chars: usize = context.iter().map(|s| s.len()).sum();
        while total_chars > MAX_CONTEXT_CHARS && context.len() > 1 {
            if let Some(removed) = context.pop_front() {
                total_chars -= removed.len();
            }
        }
        
        let context_size = context.len();
        let token_estimate = total_chars / 4;
        info!("[Candidate] Context: {} entries, ~{} tokens", context_size, token_estimate);
        
        // Send to UI
        self.send_or_buffer(serde_json::json!({
            "type": "candidate_transcription",
            "text": complete_answer,
            "context_size": context_size,
            "token_estimate": token_estimate
        }).to_string()).await;
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

    fn calculate_zero_crossing_rate(&self, samples: &[f32]) -> f32 {
        if samples.len() < 2 { return 0.0; }
        let mut crossings = 0;
        for i in 1..samples.len() {
            if (samples[i] >= 0.0 && samples[i-1] < 0.0) || (samples[i] < 0.0 && samples[i-1] >= 0.0) {
                crossings += 1;
            }
        }
        crossings as f32 / samples.len() as f32
    }

    fn calculate_energy(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() { return 0.0; }
        let sum_squares: f32 = samples.iter().map(|&x| x * x).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    fn calculate_rms(&self, samples: &[f32]) -> f32 {
        self.calculate_energy(samples)
    }

    fn samples_to_wav(&self, samples: &[f32]) -> Vec<u8> {
        let spec = WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut cursor, spec).unwrap();
            for &sample in samples {
                let amplitude = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                writer.write_sample(amplitude).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }

    async fn send_or_buffer(&self, msg: String) {
        if self.connected.load(Ordering::SeqCst) > 0 {
            let _ = self.tx.send(msg);
        } else {
            let mut buf = self.buffer.write().await;
            buf.push_back(msg);
            if buf.len() > 100 {
                buf.pop_front();
            }
        }
    }
}

/// Hallucination check (standalone function for use in spawned tasks)
fn is_hallucination(text: &str) -> bool {
    let hallucinations = [
        "thanks for watching", "thank you for watching", "thanks for listening",
        "thank you", "thanks", "bye", "hmm", "hmmm", "okay", "ok",
        "uh", "uhh", "um", "umm", "uh-huh", "yeah", "yep", "nope",
        "huh", "mhmm", "mm-hmm", "amara.org", "subtitles by",
        "copyright", "all rights reserved",
    ];
    
    if text.len() < 20 && hallucinations.iter().any(|&h| text.contains(h)) {
        return true;
    }
    
    // Filter Whisper default prompt hallucination
    if text.contains("technical interview question") || 
       text.contains("programming, databases, algorithms") ||
       text.contains("computer science") {
        return true;
    }
    
    false
}
