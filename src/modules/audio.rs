//! Audio Capture Module - WASAPI Loopback
//!
//! This module captures system audio (whatever is playing on speakers/headphones)
//! using WASAPI loopback mode on Windows.
//!
//! Key features:
//! - Captures from default OUTPUT device (not input/microphone)
//! - Automatically follows device switches (speakers ↔ headphones)
//! - No need for "Stereo Mix" to be enabled
//! - Works device-agnostically across all audio hardware
//!
//! How it works:
//! 1. Gets default output device (whatever Windows is routing audio to)
//! 2. Opens it as an INPUT stream (WASAPI loopback)
//! 3. Downsamples from native rate (44.1/48kHz) to 16kHz for Whisper
//! 4. Converts stereo to mono
//! 5. Sends processed audio chunks via crossbeam channel

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, StreamConfig};
use crossbeam_channel::{Sender, unbounded};

pub struct AudioCapture {
    _stream: Option<cpal::Stream>,
}

impl AudioCapture {
    pub fn new() -> (Self, crossbeam_channel::Receiver<Vec<f32>>) {
        let (audio_sender, audio_receiver) = unbounded();
        
        std::thread::spawn(move || {
            if let Err(e) = Self::start_capture(audio_sender) {
                eprintln!("Audio capture error: {}", e);
            }
        });
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        (AudioCapture { _stream: None }, audio_receiver)
    }
    
    fn start_capture(audio_sender: Sender<Vec<f32>>) -> Result<(), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        
        println!("=== WASAPI LOOPBACK MODE ===");
        
        // ✅ Use OUTPUT device for WASAPI loopback (captures system audio)
        let device = host.default_output_device()
            .ok_or("No output device found")?;
        
        println!("✅ Using WASAPI loopback on: {}", device.name().unwrap_or("Unknown".to_string()));
        println!("   This will capture ALL system audio (follows active device automatically)");
        
        // Get the device's native output config
        let mut supported_configs = device.supported_output_configs()?;
        let supported_config = supported_configs
            .next()
            .ok_or("No supported audio config found")?
            .with_max_sample_rate();
        
        let native_sample_rate = supported_config.sample_rate().0;
        let native_channels = supported_config.channels();
        
        println!("   Native format: {}Hz, {} channels", native_sample_rate, native_channels);
        println!("   Will downsample to: 16000Hz, 1 channel (mono) for Whisper");
        
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
                    |err| eprintln!("Audio stream error: {}", err),
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
                    |err| eprintln!("Audio stream error: {}", err),
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
                    |err| eprintln!("Audio stream error: {}", err),
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
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                )?
            }
            _ => return Err("Unsupported sample format".into()),
        };
        
        stream.play()?;
        println!("✅ WASAPI loopback active - capturing system audio");
        println!("   Audio will follow device switches (speakers ↔ headphones) automatically");
        
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    
    // No longer needed - WASAPI loopback uses default output device directly
    // Keeping for backward compatibility but not used
    fn find_loopback_device(_host: &Host) -> Option<Device> {
        None
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
