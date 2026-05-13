//! Microphone Audio Processor
//!
//! This module processes microphone audio chunks and transcribes them.
//! It maintains a rolling context window of the candidate's recent speech.
//! Uses the same sophisticated VAD logic as audio_processor.rs for consistency.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use tracing::{info, error};
use crate::modules::groq::GroqClient;

type MessageBuffer = Arc<RwLock<VecDeque<String>>>;
type CandidateSummary = Arc<RwLock<String>>;

// Configuration constants (matching audio_processor.rs)
const SAMPLE_RATE: u32 = 16000;
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(600);
const MAX_AUDIO_DURATION: Duration = Duration::from_secs(20); // Process every 20 seconds for long answers
const ABSOLUTE_MAX_DURATION: Duration = Duration::from_secs(300); // 5 minutes absolute max

// Adaptive silence detection
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_millis(700);
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_millis(1200);
const NOISE_CALIBRATION_FRAMES: usize = 50;
const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(10);

// Voice activity detection thresholds
const NOISE_FLOOR_MULTIPLIER: f32 = 2.2;
const MIN_SPEECH_FRAMES: usize = 6;
const MIN_SPEECH_ENERGY: f32 = 0.008;
const MAX_NOISE_FLOOR: f32 = 0.020;

// Zero-crossing rate for voice detection
const MIN_ZCR: f32 = 0.015;
const MAX_ZCR: f32 = 0.40;

// Context window optimization
const MAX_CANDIDATE_CONTEXT: usize = 15; // Last 15 complete answers (up from 5)
const MAX_ANSWER_TOKENS: usize = 500; // ~375 words per answer
const MAX_CONTEXT_TOKENS: usize = 6000; // Total token budget for recent context
const CHARS_PER_TOKEN: usize = 4; // Rough estimate: 1 token ≈ 4 characters

pub struct MicProcessor {
    groq_client: Arc<GroqClient>,
    candidate_context: Arc<RwLock<VecDeque<String>>>,
    candidate_summary: CandidateSummary,
    tx: broadcast::Sender<String>,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    
    // State (matching audio_processor.rs)
    accumulated_audio: Vec<f32>,
    speech_start: Option<Instant>,
    silence_start: Option<Instant>,
    is_processing: bool,
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
    
    // Streaming transcription for long answers
    current_answer_parts: Vec<String>,
    total_speech_duration: Duration,
    
    // Chunk counter for logging
    chunk_count: usize,
}

impl MicProcessor {
    pub fn new(
        groq_client: Arc<GroqClient>,
        candidate_context: Arc<RwLock<VecDeque<String>>>,
        candidate_summary: CandidateSummary,
        tx: broadcast::Sender<String>,
        buffer: MessageBuffer,
        connected: Arc<AtomicUsize>,
    ) -> Self {
        info!("[Candidate] MicProcessor initializing with VAD...");
        Self {
            groq_client,
            candidate_context,
            candidate_summary,
            tx,
            buffer,
            connected,
            accumulated_audio: Vec::with_capacity(16000 * 30),
            speech_start: None,
            silence_start: None,
            is_processing: false,
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
            current_answer_parts: Vec::new(),
            total_speech_duration: Duration::from_secs(0),
            chunk_count: 0,
        }
    }
    
    pub async fn process_audio_chunk(&mut self, audio_data: Vec<f32>) {
        self.chunk_count += 1;
        
        // Log every 100 chunks
        if self.chunk_count % 100 == 0 {
            info!("[Candidate] Received {} audio chunks so far...", self.chunk_count);
        }
        
        let now = Instant::now();
        
        // Calibrate noise floor first
        if !self.is_calibrated {
            self.calibrate_noise_floor(&audio_data);
            return;
        }
        
        // Don't process new chunks while transcribing
        if self.is_processing {
            return;
        }
        
        // Continuous recalibration
        if self.speech_start.is_none() && self.should_recalibrate(now) {
            self.recalibrate_noise_floor(&audio_data, now);
        }
        
        let energy = self.calculate_energy(&audio_data);
        let zcr = self.calculate_zero_crossing_rate(&audio_data);
        
        self.energy_history.push_back(energy);
        if self.energy_history.len() > 100 {
            self.energy_history.pop_front();
        }
        
        // Dynamic threshold with SNR
        let dynamic_threshold = (self.noise_floor * NOISE_FLOOR_MULTIPLIER).max(MIN_SPEECH_ENERGY);
        let snr = if self.noise_floor > 0.0 { energy / self.noise_floor } else { 0.0 };
        
        // Multi-factor voice detection
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
        
        // Speech start detection
        if self.speech_start.is_none() {
            if is_speech_confirmed {
                info!("[Candidate] Speech START | Energy: {:.4} | Threshold: {:.4} | SNR: {:.2} | ZCR: {:.3}", 
                      energy, dynamic_threshold, snr, zcr);
                self.speech_start = Some(now);
                self.silence_start = None;
                self.peak_energy = energy;
                self.avg_speech_energy = energy;
                self.speech_frame_count = 1;
                self.accumulated_audio.extend_from_slice(&audio_data);
            }
        } else {
            // Continue accumulating audio
            self.accumulated_audio.extend_from_slice(&audio_data);
            
            if is_voice_like {
                self.avg_speech_energy = (self.avg_speech_energy * self.speech_frame_count as f32 + energy) / (self.speech_frame_count + 1) as f32;
                self.speech_frame_count += 1;
            }
            
            // Silence detection
            if is_speech_confirmed {
                self.silence_start = None;
            } else if !is_voice_like {
                if self.silence_start.is_none() {
                    self.silence_start = Some(now);
                }
                
                let silence_timeout = self.get_adaptive_silence_timeout();
                
                if let Some(silence_start) = self.silence_start {
                    if now.duration_since(silence_start) >= silence_timeout {
                        info!("[Candidate] Speech END | Duration: {:.2}s | Peak: {:.4} | Avg: {:.4} | Silence: {}ms", 
                              self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32,
                              self.peak_energy, self.avg_speech_energy, silence_timeout.as_millis());
                        
                        // Process final chunk if any
                        if !self.accumulated_audio.is_empty() {
                            self.process_audio_chunk_streaming(now).await;
                        }
                        
                        // Finalize and store complete answer
                        self.finalize_answer().await;
                        self.reset_state();
                    }
                }
            }
            
            // Max duration check - process in chunks for long answers
            if let Some(start) = self.speech_start {
                let current_duration = now.duration_since(start);
                
                if current_duration >= MAX_AUDIO_DURATION {
                    info!("[Candidate] 20-second chunk reached. Processing partial answer...");
                    self.process_audio_chunk_streaming(now).await;
                    // Don't reset state - continue accumulating
                    self.accumulated_audio.clear();
                    self.speech_start = Some(now); // Reset timer for next chunk
                }
                
                // Absolute max duration safety check
                if self.total_speech_duration + current_duration >= ABSOLUTE_MAX_DURATION {
                    info!("[Candidate] Absolute max duration (5 min) reached. Finalizing...");
                    self.process_audio_chunk_streaming(now).await;
                    self.finalize_answer().await;
                    self.reset_state();
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
        info!("[Candidate] Noise floor recalibrated: {:.5}", self.noise_floor);
    }
    
    /// Process a chunk of audio (for streaming long answers)
    async fn process_audio_chunk_streaming(&mut self, now: Instant) {
        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
        let duration_duration = Duration::from_secs_f32(duration);
        
        // Update total speech duration
        if let Some(start) = self.speech_start {
            self.total_speech_duration += now.duration_since(start);
        }
        
        // Minimum duration check
        if duration_duration < MIN_AUDIO_DURATION {
            info!("[Candidate] Chunk too short {:.2}s, skipping", duration);
            return;
        }
        
        let rms = self.calculate_rms(&self.accumulated_audio);
        let min_valid_energy = (self.noise_floor * 1.2).max(0.004);
        
        if rms < min_valid_energy {
            info!("[Candidate] Chunk low energy {:.4}, skipping", rms);
            return;
        }
        
        info!("[Candidate] Processing chunk {}: {:.2}s, RMS: {:.4}", 
              self.current_answer_parts.len() + 1, duration, rms);
        self.is_processing = true;
        
        let wav_data = self.samples_to_wav(&self.accumulated_audio);
        let start_time = Instant::now();
        
        // Transcribe this chunk
        match self.groq_client.transcribe_with_options(&wav_data, "whisper-large-v3", None).await {
            Ok(text) => {
                let transcription_time = start_time.elapsed();
                let text = text.trim();
                
                let lower_text = text.to_lowercase();
                if self.is_hallucination(&lower_text) {
                    info!("[Candidate] Chunk hallucination, skipping: '{}'", text);
                    self.is_processing = false;
                    return;
                }
                
                if !text.is_empty() && text.len() >= 5 {
                    info!("[Candidate] Chunk transcribed ({:?}): {}", transcription_time, 
                          text.chars().take(50).collect::<String>());
                    
                    // Add to current answer parts
                    self.current_answer_parts.push(text.to_string());
                    
                    // Send partial update to UI
                    let partial_answer = self.current_answer_parts.join(" ");
                    self.send_or_buffer(serde_json::json!({
                        "type": "candidate_partial",
                        "text": partial_answer,
                        "chunk_count": self.current_answer_parts.len(),
                        "total_duration_secs": self.total_speech_duration.as_secs()
                    }).to_string()).await;
                }
            }
            Err(e) => {
                error!("[Candidate] Chunk transcription error: {}", e);
            }
        }
        
        self.is_processing = false;
    }
    
    /// Finalize the complete answer and store in context
    async fn finalize_answer(&mut self) {
        if self.current_answer_parts.is_empty() {
            info!("[Candidate] No answer parts to finalize");
            return;
        }
        
        // Combine all parts into complete answer
        let complete_answer = self.current_answer_parts.join(" ");
        
        // Truncate if too long (approximate token limit)
        let truncated_answer = if complete_answer.len() > MAX_ANSWER_TOKENS * 4 {
            // Rough estimate: 1 token ≈ 4 characters
            let truncate_at = MAX_ANSWER_TOKENS * 4;
            format!("{}... [truncated]", &complete_answer[..truncate_at])
        } else {
            complete_answer.clone()
        };
        
        info!("[Candidate] Complete answer finalized: {} parts, {} chars, {:.1}s total", 
              self.current_answer_parts.len(), 
              truncated_answer.len(),
              self.total_speech_duration.as_secs_f32());
        
        // Store in candidate context window (last 5 answers)
        let mut context = self.candidate_context.write().await;
        context.push_back(truncated_answer.clone());
        
        while context.len() > MAX_CANDIDATE_CONTEXT {
            context.pop_front();
        }
        
        let context_size = context.len();
        info!("[Candidate] Context updated: {}/{} answers", context_size, MAX_CANDIDATE_CONTEXT);
        
        // Send final answer to UI
        self.send_or_buffer(serde_json::json!({
            "type": "candidate_complete",
            "text": truncated_answer,
            "context_size": context_size,
            "chunk_count": self.current_answer_parts.len(),
            "total_duration_secs": self.total_speech_duration.as_secs()
        }).to_string()).await;
        
        // Clear answer parts for next answer
        self.current_answer_parts.clear();
        self.total_speech_duration = Duration::from_secs(0);
    }
    
    /// Legacy method - now unused, kept for reference
    async fn process_audio(&mut self, _now: Instant) {
        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
        let duration_duration = Duration::from_secs_f32(duration);
        
        // Minimum duration check
        if duration_duration < MIN_AUDIO_DURATION {
            info!("[Candidate] Discarding: Too short {:.2}s < {:.2}s", duration, MIN_AUDIO_DURATION.as_secs_f32());
            return;
        }
        
        let rms = self.calculate_rms(&self.accumulated_audio);
        let min_valid_energy = (self.noise_floor * 1.2).max(0.004);
        
        if rms < min_valid_energy {
            info!("[Candidate] Discarding: Low energy {:.4} < {:.4}", rms, min_valid_energy);
            return;
        }
        
        info!("[Candidate] Processing audio: {:.2}s, RMS: {:.4}", duration, rms);
        self.is_processing = true;
        
        let wav_data = self.samples_to_wav(&self.accumulated_audio);
        let start_time = Instant::now();
        
        // Transcribe using Whisper (no prompt to avoid hallucinations)
        match self.groq_client.transcribe_with_options(&wav_data, "whisper-large-v3", None).await {
            Ok(text) => {
                let transcription_time = start_time.elapsed();
                let text = text.trim();
                
                let lower_text = text.to_lowercase();
                if self.is_hallucination(&lower_text) {
                    info!("[Candidate] Discarding hallucination: '{}'", text);
                    self.is_processing = false;
                    return;
                }
                
                // Recalibrate noise floor after each transcription
                if let Some(recent_avg) = self.get_recent_avg_energy() {
                    if recent_avg < self.noise_floor * 0.5 || recent_avg > self.noise_floor * 3.0 {
                        self.noise_floor = recent_avg.clamp(0.001, MAX_NOISE_FLOOR);
                        info!("[Candidate] Noise floor adjusted: {:.5}", self.noise_floor);
                    }
                }
                
                if !text.is_empty() && text.len() >= 5 {
                    info!("[Candidate] Transcription ({:?}): {}", transcription_time, text);
                    
                    // Store in candidate context window (last 10)
                    let mut context = self.candidate_context.write().await;
                    context.push_back(text.to_string());
                    
                    while context.len() > 10 {
                        context.pop_front();
                    }
                    
                    let context_size = context.len();
                    info!("[Candidate] Context size: {}/10", context_size);
                    
                    // Send to UI via WebSocket
                    self.send_or_buffer(serde_json::json!({
                        "type": "candidate_transcription",
                        "text": text,
                        "context_size": context_size,
                        "metrics": {
                            "processing_time_ms": transcription_time.as_millis()
                        }
                    }).to_string()).await;
                }
            }
            Err(e) => {
                error!("[Candidate] Transcription error: {}", e);
            }
        }
        
        self.is_processing = false;
    }
    
    fn is_hallucination(&self, text: &str) -> bool {
        let hallucinations = [
            "thanks for watching",
            "thank you for watching",
            "thanks for listening",
            "thank you",
            "thanks",
            "bye",
            "hmm",
            "hmmm",
            "okay",
            "okayy",
            "okayyy",
            "ok",
            "uh",
            "uhh",
            "um",
            "umm",
            "uh-huh",
            "yeah",
            "yep",
            "nope",
            "huh",
            "mhmm",
            "mm-hmm",
            "amara.org",
            "subtitles by",
            "copyright",
            "all rights reserved",
            "technical interview question about programming",
            "databases, algorithms, or computer science",
        ];
        
        // Exact matches or very short common phrases
        if text.len() < 20 && hallucinations.iter().any(|&h| text.contains(h)) {
            return true;
        }
        
        // Filter out the default hallucination
        if text.contains("technical interview question") || 
           text.contains("databases, algorithms") {
            return true;
        }
        
        false
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
        self.current_answer_parts.clear();
        self.total_speech_duration = Duration::from_secs(0);
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
    
    fn get_recent_avg_energy(&self) -> Option<f32> {
        if self.energy_history.is_empty() { return None; }
        let sum: f32 = self.energy_history.iter().sum();
        Some(sum / self.energy_history.len() as f32)
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
