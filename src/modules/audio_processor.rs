use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn, error};
use crate::modules::groq::{GroqClient, ConversationMessage};

// Configuration constants
const SAMPLE_RATE: u32 = 16000;
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(800);
const MAX_AUDIO_DURATION: Duration = Duration::from_secs(30);

// Adaptive silence detection (like Parakeet/LockedIn)
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_millis(1200);
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_millis(2000);
const NOISE_CALIBRATION_FRAMES: usize = 50;
const RECALIBRATION_INTERVAL: Duration = Duration::from_secs(10);

// Voice activity detection thresholds
const NOISE_FLOOR_MULTIPLIER: f32 = 3.0; // More aggressive than before
const MIN_SPEECH_FRAMES: usize = 10; // 200ms
const MIN_SPEECH_ENERGY: f32 = 0.010;
const MAX_NOISE_FLOOR: f32 = 0.020;

// Zero-crossing rate for voice detection
const MIN_ZCR: f32 = 0.02;
const MAX_ZCR: f32 = 0.35;

// Spectral features (frequency analysis)
const VOICE_FREQ_MIN: f32 = 80.0;  // Hz
const VOICE_FREQ_MAX: f32 = 300.0; // Hz

// Type aliases matching main.rs
type ConversationHistory = Arc<RwLock<Vec<ConversationMessage>>>;
type MessageBuffer = Arc<RwLock<VecDeque<String>>>;

pub struct AudioProcessor {
    audio_receiver: Receiver<Vec<f32>>,
    groq_client: Arc<GroqClient>,
    tx: broadcast::Sender<String>,
    conversation: ConversationHistory,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    
    // State
    accumulated_audio: Vec<f32>,
    speech_start: Option<Instant>,
    silence_start: Option<Instant>,
    is_processing: bool,
    consecutive_speech_frames: usize,
    consecutive_silence_frames: usize,
    
    // Adaptive noise floor (like Parakeet/LockedIn)
    noise_floor: f32,
    noise_calibration_buffer: Vec<f32>,
    is_calibrated: bool,
    energy_history: VecDeque<f32>,
    last_recalibration: Option<Instant>,
    
    // Speech quality metrics
    peak_energy: f32,
    avg_speech_energy: f32,
    speech_frame_count: usize,
}

impl AudioProcessor {
    pub fn new(
        audio_receiver: Receiver<Vec<f32>>,
        groq_client: Arc<GroqClient>,
        tx: broadcast::Sender<String>,
        conversation: ConversationHistory,
        buffer: MessageBuffer,
        connected: Arc<AtomicUsize>,
    ) -> Self {
        info!("AudioProcessor initializing with streaming-like VAD...");
        Self {
            audio_receiver,
            groq_client,
            tx,
            conversation,
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
        }
    }

    pub async fn run(&mut self) {
        info!("AudioProcessor started. Calibrating noise floor...");
        
        loop {
            let recv_result = self.audio_receiver.recv_timeout(Duration::from_millis(20));
            let now = Instant::now();

            match recv_result {
                Ok(audio_chunk) => {
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
                    error!("Audio channel disconnected. Stopping processor.");
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
            info!("Noise floor calibrated: {:.5}", self.noise_floor);
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
        if self.is_processing {
            return;
        }
        
        // Continuous recalibration (like Parakeet/LockedIn)
        if self.speech_start.is_none() && self.should_recalibrate(now) {
            self.recalibrate_noise_floor(&chunk, now);
        }

        let energy = self.calculate_energy(&chunk);
        let zcr = self.calculate_zero_crossing_rate(&chunk);
        
        self.energy_history.push_back(energy);
        if self.energy_history.len() > 100 {
            self.energy_history.pop_front();
        }
        
        // Dynamic threshold with SNR (Signal-to-Noise Ratio)
        let dynamic_threshold = (self.noise_floor * NOISE_FLOOR_MULTIPLIER).max(MIN_SPEECH_ENERGY);
        let snr = if self.noise_floor > 0.0 { energy / self.noise_floor } else { 0.0 };
        
        // Multi-factor voice detection (energy + ZCR + SNR)
        let is_voice_like = energy > dynamic_threshold && 
                           zcr >= MIN_ZCR && zcr <= MAX_ZCR &&
                           snr >= 2.0; // SNR must be at least 2:1
        
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
                info!("Speech START | Energy: {:.4} | Threshold: {:.4} | SNR: {:.2} | ZCR: {:.3}", 
                      energy, dynamic_threshold, snr, zcr);
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
                
                // Adaptive silence timeout based on speech quality
                let silence_timeout = self.get_adaptive_silence_timeout();
                
                if let Some(silence_start) = self.silence_start {
                    if now.duration_since(silence_start) >= silence_timeout {
                        info!("Speech END | Duration: {:.2}s | Peak: {:.4} | Avg: {:.4} | Silence: {}ms", 
                              self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32,
                              self.peak_energy, self.avg_speech_energy, silence_timeout.as_millis());
                        self.process_audio(now).await;
                        self.reset_state();
                    }
                }
            }

            if let Some(start) = self.speech_start {
                if now.duration_since(start) >= MAX_AUDIO_DURATION {
                    info!("Max duration reached. Processing...");
                    self.process_audio(now).await;
                    self.reset_state();
                }
            }
        }
    }
    
    fn get_adaptive_silence_timeout(&self) -> Duration {
        // Longer timeout for high-quality speech (clear questions)
        // Shorter timeout for low-quality speech (background noise)
        let quality_ratio = if self.noise_floor > 0.0 {
            (self.avg_speech_energy / self.noise_floor).min(10.0)
        } else {
            5.0
        };
        
        if quality_ratio > 5.0 {
            EXTENDED_SILENCE_TIMEOUT // High quality: allow longer pauses
        } else {
            BASE_SILENCE_TIMEOUT // Low quality: end quickly
        }
    }
    
    fn recalibrate_noise_floor(&mut self, chunk: &[f32], now: Instant) {
        let energy = self.calculate_energy(chunk);
        
        // Exponential moving average for smooth recalibration
        let alpha = 0.1; // Smoothing factor
        self.noise_floor = (alpha * energy + (1.0 - alpha) * self.noise_floor).min(MAX_NOISE_FLOOR);
        self.last_recalibration = Some(now);
        
        info!("Noise floor recalibrated: {:.5}", self.noise_floor);
    }

    async fn check_silence_timeout(&mut self, now: Instant) {
        if let Some(silence_start) = self.silence_start {
            let silence_timeout = self.get_adaptive_silence_timeout();
            if now.duration_since(silence_start) >= silence_timeout {
                if !self.accumulated_audio.is_empty() {
                    self.process_audio(now).await;
                }
                self.reset_state();
            }
        }
    }

    async fn process_audio(&mut self, _now: Instant) {
        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
        let duration_duration = Duration::from_secs_f32(duration);

        if duration_duration < MIN_AUDIO_DURATION {
            return;
        }

        let rms = self.calculate_rms(&self.accumulated_audio);
        let min_valid_energy = (self.noise_floor * 1.5).max(0.005);
        
        if rms < min_valid_energy {
            info!("Discarding: Low energy {:.4} < {:.4}", rms, min_valid_energy);
            return;
        }

        info!("Processing audio question: {:.2}s, RMS: {:.4}", duration, rms);
        self.is_processing = true;

        let wav_data = self.samples_to_wav(&self.accumulated_audio);
        let start_time = Instant::now();

        // Single API call to Whisper
        match self.groq_client.transcribe_with_options(&wav_data, "whisper-large-v3", None).await {
            Ok(text) => {
                let transcription_time = start_time.elapsed();
                let text = text.trim();
                
                let lower_text = text.to_lowercase();
                if self.is_hallucination(&lower_text) {
                     info!("Discarding hallucination: '{}'", text);
                     self.is_processing = false;
                     return;
                }
                
                // Recalibrate noise floor after each question (adapt to changing environment)
                if let Some(recent_avg) = self.get_recent_avg_energy() {
                    if recent_avg < self.noise_floor * 0.5 || recent_avg > self.noise_floor * 3.0 {
                        self.noise_floor = recent_avg.clamp(0.001, MAX_NOISE_FLOOR);
                        info!("Noise floor adjusted: {:.5}", self.noise_floor);
                    }
                }

                if !text.is_empty() && text.len() >= 5 {
                    info!("Transcription ({:?}): {}", transcription_time, text);
                    
                    // 1. Notify UI of transcription
                    self.send_or_buffer(serde_json::json!({
                        "type": "transcription",
                        "text": text,
                        "metrics": {
                            "processing_time_ms": transcription_time.as_millis()
                        }
                    }).to_string()).await;

                    // 2. Add to history
                    self.conversation.write().await.push(ConversationMessage {
                        role: "user".to_string(),
                        content: text.to_string(),
                    });

                    // 3. Get Answer (Question Sending)
                    info!("Sending to AI for answer...");
                    let history = self.conversation.read().await.clone();
                    
                    match self.groq_client.chat_with_history(text, &history).await {
                        Ok(answer) => {
                            let total_time = start_time.elapsed();
                            info!("Answer received ({:?} total): {}", total_time, answer.chars().take(50).collect::<String>());
                            
                            self.conversation.write().await.push(ConversationMessage {
                                role: "assistant".to_string(),
                                content: answer.clone(),
                            });
                            
                            self.send_or_buffer(serde_json::json!({
                                "type": "answer",
                                "text": answer,
                                "metrics": {
                                    "total_time_ms": total_time.as_millis()
                                }
                            }).to_string()).await;
                        }
                        Err(e) => {
                            error!("Chat error: {}", e);
                            self.send_or_buffer(serde_json::json!({
                                "type": "error",
                                "message": format!("AI Error: {}", e)
                            }).to_string()).await;
                        }
                    }
                }
            }
            Err(e) => {
                error!("Transcription failed: {}", e);
                self.send_or_buffer(serde_json::json!({
                    "type": "error",
                    "message": format!("Transcription failed: {}", e)
                }).to_string()).await;
            }
        }

        self.is_processing = false;
    }

    async fn send_or_buffer(&self, message: String) {
        let connected = self.connected.load(Ordering::SeqCst);
        if connected > 0 {
            let _ = self.tx.send(message);
        } else {
            self.buffer.write().await.push_back(message);
        }
    }

    fn is_hallucination(&self, text: &str) -> bool {
        let hallucinations = [
            "thanks for watching",
            "thank you for watching",
            "thanks for listening",
            "thank you",
            "bye",
            "amara.org",
            "subtitles by",
            "copyright",
            "all rights reserved"
        ];
        
        // Exact matches or very short common phrases
        if text.len() < 20 && hallucinations.iter().any(|&h| text.contains(h)) {
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
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
            for &sample in samples {
                let amplitude = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                writer.write_sample(amplitude).unwrap();
            }
            writer.finalize().unwrap();
        }
        cursor.into_inner()
    }
}
