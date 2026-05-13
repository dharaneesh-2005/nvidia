//! Microphone Capture Module
//!
//! This module captures audio from the default microphone input device.
//! It mirrors the audio.rs module but captures from INPUT device instead of OUTPUT (loopback).
//!
//! Key features:
//! - Captures from default INPUT device (microphone)
//! - Downsamples to 16kHz for Whisper
//! - Converts to mono
//! - Sends processed audio chunks via crossbeam channel

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_channel::{Sender, unbounded};

pub struct MicCapture {
    _stream: Option<cpal::Stream>,
}

impl MicCapture {
    pub fn new() -> (Self, crossbeam_channel::Receiver<Vec<f32>>) {
        Self::new_with_device(None)
    }
    
    pub fn new_with_device(device_name: Option<String>) -> (Self, crossbeam_channel::Receiver<Vec<f32>>) {
        let (audio_sender, audio_receiver) = unbounded();
        
        std::thread::spawn(move || {
            if let Err(e) = Self::start_capture(audio_sender, device_name) {
                eprintln!("Microphone capture error: {}", e);
            }
        });
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        (MicCapture { _stream: None }, audio_receiver)
    }
    
    pub fn list_devices() -> Vec<(String, String)> {
        let host = cpal::default_host();
        let mut devices = Vec::new();
        
        if let Ok(input_devices) = host.input_devices() {
            for device in input_devices {
                if let Ok(name) = device.name() {
                    // Use name as both ID and display name for now
                    devices.push((name.clone(), name));
                }
            }
        }
        
        devices
    }
    
    fn start_capture(audio_sender: Sender<Vec<f32>>, device_name: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        
        println!("=== MICROPHONE CAPTURE MODE ===");
        println!("[MIC] Attempting to find input device...");
        
        // Use specific device if provided, otherwise use default
        let device = if let Some(ref name) = device_name {
            println!("[MIC] Looking for device: {}", name);
            host.input_devices()?
                .find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                .ok_or_else(|| format!("Microphone '{}' not found", name))?
        } else {
            println!("[MIC] Using default input device");
            host.default_input_device()
                .ok_or("No input device (microphone) found")?
        };
        
        println!("✅ [MIC] Using microphone: {}", device.name().unwrap_or("Unknown".to_string()));
        println!("   [MIC] This will capture candidate's voice continuously");
        
        // Get the device's native input config
        let mut supported_configs = device.supported_input_configs()?;
        let supported_config = supported_configs
            .next()
            .ok_or("No supported audio config found")?
            .with_max_sample_rate();
        
        let native_sample_rate = supported_config.sample_rate().0;
        let native_channels = supported_config.channels();
        
        println!("   [MIC] Native format: {}Hz, {} channels", native_sample_rate, native_channels);
        println!("   [MIC] Will downsample to: 16000Hz, 1 channel (mono) for Whisper");
        
        let config = StreamConfig {
            channels: native_channels,
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        
        let channels = config.channels;
        let sample_rate = config.sample_rate.0;
        
        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => {
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let processed = Self::process_audio_f32(data, channels, sample_rate);
                        let _ = audio_sender.try_send(processed);
                    },
                    |err| eprintln!("Microphone stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let processed = Self::process_audio_i16(data, channels, sample_rate);
                        let _ = audio_sender.try_send(processed);
                    },
                    |err| eprintln!("Microphone stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::U16 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let processed = Self::process_audio_u16(data, channels, sample_rate);
                        let _ = audio_sender.try_send(processed);
                    },
                    |err| eprintln!("Microphone stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::U8 => {
                device.build_input_stream(
                    &config,
                    move |data: &[u8], _: &cpal::InputCallbackInfo| {
                        let processed = Self::process_audio_u8(data, channels, sample_rate);
                        let _ = audio_sender.try_send(processed);
                    },
                    |err| eprintln!("Microphone stream error: {}", err),
                    None,
                )?
            }
            _ => return Err("Unsupported sample format".into()),
        };
        
        stream.play()?;
        println!("✅ [MIC] Microphone active - capturing candidate's voice");
        println!("   [MIC] Speak into your microphone to test...");
        
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    
    fn process_audio_f32(data: &[f32], channels: u16, sample_rate: u32) -> Vec<f32> {
        let mut processed = Self::convert_to_mono(data, channels);
        if sample_rate != 16000 {
            processed = Self::resample(&processed, sample_rate, 16000);
        }
        processed
    }
    
    fn process_audio_i16(data: &[i16], channels: u16, sample_rate: u32) -> Vec<f32> {
        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
        Self::process_audio_f32(&f32_data, channels, sample_rate)
    }
    
    fn process_audio_u16(data: &[u16], channels: u16, sample_rate: u32) -> Vec<f32> {
        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).collect();
        Self::process_audio_f32(&f32_data, channels, sample_rate)
    }
    
    fn process_audio_u8(data: &[u8], channels: u16, sample_rate: u32) -> Vec<f32> {
        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect();
        Self::process_audio_f32(&f32_data, channels, sample_rate)
    }
    
    fn convert_to_mono(data: &[f32], channels: u16) -> Vec<f32> {
        if channels == 1 {
            data.to_vec()
        } else {
            data.chunks(channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect()
        }
    }
    
    fn resample(data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return data.to_vec();
        }
        let ratio = from_rate as f64 / to_rate as f64;
        let output_len = (data.len() as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);
        for i in 0..output_len {
            let src_index = (i as f64 * ratio) as usize;
            if src_index < data.len() {
                output.push(data[src_index]);
            }
        }
        output
    }
}
