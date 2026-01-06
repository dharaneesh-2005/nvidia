use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn, error};
use crate::modules::groq::{GroqClient, ConversationMessage};

// Configuration constants
const SPEECH_ENERGY_THRESHOLD: f32 = 0.005; 
const MIN_AVERAGE_ENERGY: f32 = 0.002; // Minimum average energy to confirm valid speech
const SILENCE_TIMEOUT: Duration = Duration::from_millis(800);
const MAX_AUDIO_DURATION: Duration = Duration::from_secs(120);
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(500);
const SAMPLE_RATE: u32 = 16000;
const MIN_SPEECH_FRAMES: usize = 3; // Consecutive frames required to trigger speech

// Type aliases matching main.rs
type ConversationHistory = Arc<RwLock<Vec<ConversationMessage>>>;
type MessageBuffer = Arc<RwLock<VecDeque<String>>>;

pub struct AudioProcessor {
    audio_receiver: Receiver<Vec<f32>>,
    groq_client: Arc<GroqClient>,
    tx: broadcast::Sender<String>,
    conversation: ConversationHistory,
    buffer: MessageBuffer,
    connected: Arc<RwLock<bool>>,
    
    // State
    accumulated_audio: Vec<f32>,
    speech_start: Option<Instant>,
    silence_start: Option<Instant>,
    is_processing: bool,
    consecutive_speech_frames: usize,
}

impl AudioProcessor {
    pub fn new(
        audio_receiver: Receiver<Vec<f32>>,
        groq_client: Arc<GroqClient>,
        tx: broadcast::Sender<String>,
        conversation: ConversationHistory,
        buffer: MessageBuffer,
        connected: Arc<RwLock<bool>>,
    ) -> Self {
        Self {
            audio_receiver,
            groq_client,
            tx,
            conversation,
            buffer,
            connected,
            accumulated_audio: Vec::with_capacity(16000 * 10),
            speech_start: None,
            silence_start: None,
            is_processing: false,
            consecutive_speech_frames: 0,
        }
    }

    pub async fn run(&mut self) {
        info!("AudioProcessor started. Listening for speech...");
        
        loop {
            let recv_result = self.audio_receiver.recv_timeout(Duration::from_millis(20));
            let now = Instant::now();

            match recv_result {
                Ok(audio_chunk) => {
                    self.process_chunk(audio_chunk, now).await;
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.check_silence_timeout(now).await;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("Audio channel disconnected. Stopping processor.");
                    break;
                }
            }
        }
    }

    async fn process_chunk(&mut self, chunk: Vec<f32>, now: Instant) {
        if self.is_processing {
            // Drop audio while processing to ensure single API call mechanism per question
            return; 
        }

        let energy = self.calculate_energy(&chunk);
        let is_potential_speech = energy > SPEECH_ENERGY_THRESHOLD;

        if is_potential_speech {
            self.consecutive_speech_frames += 1;
        } else {
            self.consecutive_speech_frames = 0;
        }

        let is_speaking_confirmed = self.consecutive_speech_frames >= MIN_SPEECH_FRAMES;

        if self.speech_start.is_none() {
            // Not recording yet
            if is_speaking_confirmed {
                info!("Speech detected (Energy: {:.4})", energy);
                self.speech_start = Some(now);
                self.silence_start = None;
                self.accumulated_audio.extend_from_slice(&chunk);
            }
        } else {
            // Already recording
            self.accumulated_audio.extend_from_slice(&chunk);

            if is_speaking_confirmed {
                // User is actively speaking (confirmed by consecutive frames)
                self.silence_start = None;
            } else if is_potential_speech {
                // Potential speech (spike or start of phrase)
                // Do not reset silence timer yet, but also do not trigger timeout
                // This protects against cutting off new speech that starts during silence logic
            } else {
                // Silence
                if self.silence_start.is_none() {
                    self.silence_start = Some(now);
                }
                self.check_silence_timeout(now).await;
            }

            // Check max duration
            if let Some(start) = self.speech_start {
                if now.duration_since(start) >= MAX_AUDIO_DURATION {
                    info!("Max audio duration reached. Forcing processing.");
                    self.process_audio(now).await;
                    self.reset_state();
                }
            }
        }
    }

    async fn check_silence_timeout(&mut self, now: Instant) {
        if let Some(silence_start) = self.silence_start {
            if now.duration_since(silence_start) >= SILENCE_TIMEOUT {
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
        if rms < MIN_AVERAGE_ENERGY {
            info!("Discarding audio: Low average energy ({:.4} < {:.4})", rms, MIN_AVERAGE_ENERGY);
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
                
                // Hallucination filter
                let lower_text = text.to_lowercase();
                if self.is_hallucination(&lower_text) {
                     info!("Discarding hallucination: '{}'", text);
                     self.is_processing = false;
                     return;
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
        let connected = *self.connected.read().await;
        if connected {
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
