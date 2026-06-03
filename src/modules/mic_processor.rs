//! Microphone Audio Processor - MANUAL MODE
//!
//! Recording is controlled by a hotkey (Alt+Space toggle).
//! When recording: accumulate audio.
//! When stopped: single Whisper call → immediately add to context.
//! No continuous VAD, no resource contention with interviewer stream.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::collections::VecDeque;
use std::time::Duration;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use tokio::sync::{broadcast, RwLock};
use hound::{WavSpec, WavWriter};
use std::io::Cursor;
use tracing::{info, error};
use crate::modules::groq::GroqClient;

type MessageBuffer = Arc<RwLock<VecDeque<String>>>;
type CandidateContext = Arc<RwLock<VecDeque<String>>>;

const SAMPLE_RATE: u32 = 16000;
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(500);
const MAX_CONTEXT_CHARS: usize = 24000;

pub struct MicProcessor {
    audio_receiver: Receiver<Vec<f32>>,
    groq_client: Arc<GroqClient>,
    tx: broadcast::Sender<String>,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    recording_flag: Arc<AtomicBool>,
    
    accumulated_audio: Vec<f32>,
    was_recording: bool,
}

impl MicProcessor {
    pub fn new(
        audio_receiver: Receiver<Vec<f32>>,
        groq_client: Arc<GroqClient>,
        tx: broadcast::Sender<String>,
        candidate_context: CandidateContext,
        buffer: MessageBuffer,
        connected: Arc<AtomicUsize>,
        recording_flag: Arc<AtomicBool>,
    ) -> Self {
        info!("[Candidate] MicProcessor initializing (MANUAL mode - Alt+Space to record)...");
        Self {
            audio_receiver,
            groq_client,
            tx,
            candidate_context,
            buffer,
            connected,
            recording_flag,
            accumulated_audio: Vec::with_capacity(SAMPLE_RATE as usize * 300),
            was_recording: false,
        }
    }

    pub async fn run(&mut self) {
        info!("[Candidate] MicProcessor started in manual mode.");
        
        loop {
            let recv_result = self.audio_receiver.recv_timeout(Duration::from_millis(50));
            let is_recording = self.recording_flag.load(Ordering::SeqCst);
            
            match recv_result {
                Ok(chunk) => {
                    if is_recording {
                        // Recording active - accumulate
                        if !self.was_recording {
                            info!("[Candidate] 🎤 Recording STARTED");
                            self.accumulated_audio.clear();
                            self.was_recording = true;
                        }
                        self.accumulated_audio.extend_from_slice(&chunk);
                    } else if self.was_recording {
                        // Just stopped - transcribe what we have
                        self.was_recording = false;
                        info!("[Candidate] 🎤 Recording STOPPED, transcribing...");
                        self.transcribe_and_store().await;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Check if recording stopped while no audio arriving
                    if self.was_recording && !is_recording {
                        self.was_recording = false;
                        info!("[Candidate] 🎤 Recording STOPPED (idle), transcribing...");
                        self.transcribe_and_store().await;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("[Candidate] Mic channel disconnected.");
                    break;
                }
            }
        }
    }

    async fn transcribe_and_store(&mut self) {
        let duration = self.accumulated_audio.len() as f32 / SAMPLE_RATE as f32;
        if Duration::from_secs_f32(duration) < MIN_AUDIO_DURATION {
            info!("[Candidate] Recording too short ({:.2}s), skipping", duration);
            self.accumulated_audio.clear();
            return;
        }
        
        info!("[Candidate] Transcribing {:.2}s of audio...", duration);
        let wav_data = self.samples_to_wav(&self.accumulated_audio);
        self.accumulated_audio.clear();
        
        // 15-second timeout
        let result = tokio::time::timeout(
            Duration::from_secs(15),
            self.groq_client.transcribe_with_options(&wav_data, "whisper-large-v3", Some(""))
        ).await;
        
        let text = match result {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => { error!("[Candidate] Transcription failed: {}", e); return; }
            Err(_) => { error!("[Candidate] Transcription timed out"); return; }
        };
        
        let text = text.trim().to_string();
        if text.is_empty() || text.len() < 3 {
            info!("[Candidate] Empty transcription, skipping");
            return;
        }
        
        info!("[Candidate] Transcribed: '{}'", text.chars().take(80).collect::<String>());
        
        // Add to context
        let mut context = self.candidate_context.write().await;
        context.push_back(text.clone());
        
        // Summarize overflow
        let mut total: usize = context.iter().map(|s| s.len()).sum();
        while total > MAX_CONTEXT_CHARS && context.len() > 2 {
            if let Some(old) = context.pop_front() {
                if old.len() > 100 {
                    let summary: String = old.chars().take(100).collect();
                    context.push_front(format!("[Summary] {}...", summary));
                    total = context.iter().map(|s| s.len()).sum();
                    if total > MAX_CONTEXT_CHARS { context.pop_front(); total = context.iter().map(|s| s.len()).sum(); }
                } else {
                    total -= old.len();
                }
            }
        }
        
        let size = context.len();
        drop(context);
        
        info!("[Candidate] Context updated: {} entries", size);
        
        self.send_or_buffer(serde_json::json!({
            "type": "candidate_transcription",
            "text": text,
            "context_size": size
        }).to_string()).await;
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
