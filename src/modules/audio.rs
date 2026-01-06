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
        
        println!("=== LISTING ALL AUDIO DEVICES ===");
        if let Ok(input_devices) = host.input_devices() {
            println!("INPUT DEVICES:");
            for device in input_devices {
                if let Ok(name) = device.name() {
                    println!("  Input: {}", name);
                }
            }
        }
        println!("=== END DEVICE LIST ===");
        
        let has_stereo_mix = if let Ok(mut input_devices) = host.input_devices() {
            input_devices.any(|device| {
                if let Ok(name) = device.name() {
                    let name_lower = name.to_lowercase();
                    name_lower.contains("stereo mix") || name_lower.contains("what u hear")
                } else { false }
            })
        } else { false };
        
        if !has_stereo_mix {
            println!("WARNING: No 'Stereo Mix' device found!");
            println!("To capture system audio (speakers), you need to:");
            println!("1. Right-click speaker icon in system tray");
            println!("2. Select 'Open Sound settings'");
            println!("3. Click 'Sound Control Panel' (on the right)");
            println!("4. Go to 'Recording' tab");
            println!("5. Right-click empty area and select 'Show Disabled Devices'");
            println!("6. Find 'Stereo Mix' and right-click -> Enable");
            println!("7. Set it as default recording device");
            println!("8. Restart this application");
        }
        
        let device = Self::find_loopback_device(&host)
            .or_else(|| host.default_input_device())
            .ok_or("No suitable audio device found")?;
        
        println!("Using audio device: {}", device.name().unwrap_or("Unknown".to_string()));
        
        let mut supported_configs = device.supported_input_configs()?;
        let supported_config = supported_configs
            .next()
            .ok_or("No supported audio config found")?
            .with_max_sample_rate();
        
        println!("Audio config: {:?}", supported_config);
        
        let config = StreamConfig {
            channels: supported_config.channels(),
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
        println!("Audio capture started - listening to Stereo Mix");
        
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    
    fn find_loopback_device(host: &Host) -> Option<Device> {
        if let Ok(devices) = host.input_devices() {
            let mut all_devices = Vec::new();
            for device in devices {
                if let Ok(name) = device.name() {
                    all_devices.push((device, name));
                }
            }
            
            // Priority 1: Stereo Mix or What U Hear
            for (device, name) in &all_devices {
                let name_lower = name.to_lowercase();
                if name_lower.contains("stereo mix") || name_lower.contains("what u hear") {
                    println!("✅ Using SYSTEM AUDIO loopback device: {}", name);
                    return Some(device.clone());
                }
            }
            
            // Priority 2: Realtek stereo mix
            for (device, name) in &all_devices {
                let name_lower = name.to_lowercase();
                if name_lower.contains("realtek") && name_lower.contains("stereo mix") {
                    println!("✅ Using Realtek stereo mix device: {}", name);
                    return Some(device.clone());
                }
            }
            
            // Priority 3: Any loopback device
            for (device, name) in &all_devices {
                let name_lower = name.to_lowercase();
                if name_lower.contains("loopback") {
                    println!("Using loopback device: {}", name);
                    return Some(device.clone());
                }
            }
        }
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
