use crate::trigger_manager::TriggerState;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ltc::LTCDecoder;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub channels: u16,
}

pub struct AudioState {
    stream: Mutex<Option<cpal::Stream>>,
    device_name: Mutex<String>,
    channel_index: Mutex<usize>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            stream: Mutex::new(None),
            device_name: Mutex::new(String::new()),
            channel_index: Mutex::new(0),
        }
    }
}

// ── Flywheel / Intelligent Processor ──────────────────────────────────────────

struct LtcProcessor {
    last_frame_count: Option<u32>,
    fps: f32,
    last_received_at: Instant,
}

impl LtcProcessor {
    fn new(fps: f32) -> Self {
        Self {
            last_frame_count: None,
            fps,
            last_received_at: Instant::now() - Duration::from_secs(10),
        }
    }

    fn process_frame(&mut self, h: u8, m: u8, s: u8, f: u8) -> Option<String> {
        let fps_int = self.fps.round() as u32;
        let current_frames = (h as u32 * 3600 * fps_int) + (m as u32 * 60 * fps_int) + (s as u32 * fps_int) + (f as u32);
        
        self.last_received_at = Instant::now();

        // INSTANT JAM-SYNC: To eliminate any 'glitch' or wait period, 
        // we accept every valid LTC frame immediately. 
        // LTC is robust enough that noise rarely produces a valid sync word.
        self.last_frame_count = Some(current_frames);
        
        Some(format_tc(current_frames, fps_int))
    }

    fn get_status(&self) -> &'static str {
        let elapsed = self.last_received_at.elapsed();
        if elapsed > Duration::from_millis(500) {
            "NO_SIGNAL"
        } else if elapsed > Duration::from_millis(100) {
            "FREEWHEELING"
        } else {
            "LOCKED"
        }
    }
}

fn format_tc(frames: u32, fps: u32) -> String {
    let f = frames % fps;
    let s = (frames / fps) % 60;
    let m = (frames / (fps * 60)) % 60;
    let h = (frames / (fps * 3600)) % 24;
    format!("{:02}:{:02}:{:02}:{:02}", h, m, s, f)
}

// ── Audio Core ────────────────────────────────────────────────────────────────

fn find_device(host: &cpal::Host, name: &str) -> Result<cpal::Device, String> {
    if name.is_empty() {
        return host.default_input_device().ok_or_else(|| "No default device found".into());
    }
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    for device in devices {
        if let Ok(dname) = device.name() {
            if dname == name { return Ok(device); }
        }
    }
    Err(format!("Device '{}' not found", name))
}

fn build_stream(
    device_name: &str,
    channel_index: usize,
    app: AppHandle,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = find_device(&host, device_name)?;
    let config = device.default_input_config().map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    let ch_idx = if channel_index >= channels { 0 } else { channel_index };
    let sample_rate = u32::from(config.sample_rate()) as f32;
    let fps = *app.state::<TriggerState>().fps.lock().unwrap();
    let apv = sample_rate / fps;

    let (tx, rx) = std::sync::mpsc::channel::<[u8; 4]>();
    let app_clone = app.clone();

    // ── Dispatcher Thread ──────────────────────────────────────────────────────
    std::thread::Builder::new()
        .name("ltc-dispatch".into())
        .spawn(move || {
            let mut processor = LtcProcessor::new(fps);
            let mut last_ui_emit = Instant::now();
            let mut last_status_emit = Instant::now();
            let mut last_status = "NO_SIGNAL";

            loop {
                // High-performance blocking receive
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(frame) => {
                        let mut smooth_tc = processor.process_frame(frame[0], frame[1], frame[2], frame[3]);
                        while let Ok(f) = rx.try_recv() {
                            if let Some(stc) = processor.process_frame(f[0], f[1], f[2], f[3]) {
                                smooth_tc = Some(stc);
                            }
                        }
                        if let Some(tc) = smooth_tc {
                            // Check triggers on every valid frame
                            crate::trigger_manager::check_triggers(&tc, &app_clone.state::<crate::trigger_manager::TriggerState>(), &app_clone);

                            // UI update at ~60fps
                            if last_ui_emit.elapsed() >= Duration::from_millis(16) {
                                let _ = app_clone.emit("timecode", &tc);
                                last_ui_emit = Instant::now();
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }

                if last_status_emit.elapsed() >= Duration::from_millis(100) {
                    let status = processor.get_status();
                    if status != last_status {
                        let _ = app_clone.emit("status", status);
                        last_status = status;
                    }
                    last_status_emit = Instant::now();
                }
            }
        })
        .ok();

    // ── Audio Thread with AGGRESSIVE Watchdog ─────────────────────────────────
    let samples_since_frame = Arc::new(AtomicUsize::new(0));
    let watchdog_limit = (sample_rate * 0.25) as usize; // Reset if 250ms of audio but no frames
    
    let err_fn = |err| eprintln!("[audio] error: {}", err);
    
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut decoder = LTCDecoder::new(apv, VecDeque::with_capacity(32));
            let mut buf = Vec::with_capacity(4096);
            let ssf = Arc::clone(&samples_since_frame);
            device.build_input_stream(&config.into(), move |data: &[f32], _: &_| {
                buf.clear();
                for chunk in data.chunks(channels) {
                    if let Some(&s) = chunk.get(ch_idx) { buf.push(s); }
                }
                
                let total = ssf.fetch_add(buf.len(), Ordering::Relaxed);
                if total > watchdog_limit {
                    decoder = LTCDecoder::new(apv, VecDeque::with_capacity(32));
                    ssf.store(0, Ordering::Relaxed);
                }

                decoder.write_samples(&buf);
                for frame in &mut decoder {
                    ssf.store(0, Ordering::Relaxed);
                    let _ = tx.send([frame.hour, frame.minute, frame.second, frame.frame]);
                }
            }, err_fn, None)
        }
        cpal::SampleFormat::I16 => {
            let mut decoder = LTCDecoder::new(apv, VecDeque::with_capacity(32));
            let mut buf = Vec::with_capacity(4096);
            let ssf = Arc::clone(&samples_since_frame);
            device.build_input_stream(&config.into(), move |data: &[i16], _: &_| {
                buf.clear();
                for chunk in data.chunks(channels) {
                    if let Some(&s) = chunk.get(ch_idx) { 
                        buf.push(s as f32 / i16::MAX as f32); 
                    }
                }

                let total = ssf.fetch_add(buf.len(), Ordering::Relaxed);
                if total > watchdog_limit {
                    decoder = LTCDecoder::new(apv, VecDeque::with_capacity(32));
                    ssf.store(0, Ordering::Relaxed);
                }

                decoder.write_samples(&buf);
                for frame in &mut decoder {
                    ssf.store(0, Ordering::Relaxed);
                    let _ = tx.send([frame.hour, frame.minute, frame.second, frame.frame]);
                }
            }, err_fn, None)
        }
        _ => return Err("Unsupported format".into()),
    }.map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

// ── API ──────────────────────────────────────────────────────────────────────

pub fn auto_start(app: &AppHandle) {
    let audio = app.state::<AudioState>();
    let settings = crate::settings_manager::load_settings(app);
    
    // Set FPS and MIDI from settings
    {
        let trigger_state = app.state::<TriggerState>();
        *trigger_state.fps.lock().unwrap() = settings.fps;
        *trigger_state.global_osc_target.lock().unwrap() = settings.osc_target.clone();
        let _ = crate::midi_sender::switch_midi_output(settings.midi_device_name);
    }

    if let Ok(stream) = build_stream(&settings.device_name, settings.channel_index, app.clone()) {
        *audio.stream.lock().unwrap() = Some(stream);
        *audio.device_name.lock().unwrap() = settings.device_name;
        *audio.channel_index.lock().unwrap() = settings.channel_index;
    }
}

#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for device in devices {
        if let Ok(name) = device.name() {
            if let Ok(config) = device.default_input_config() {
                result.push(AudioDevice { name, channels: config.channels() });
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn switch_device(device_name: String, channel_index: usize, app: AppHandle) -> Result<(), String> {
    let audio = app.state::<AudioState>();
    {
        let mut stream = audio.stream.lock().unwrap();
        *stream = None; 
    }
    let new_stream = build_stream(&device_name, channel_index, app.clone())?;
    
    // Save to memory state
    *audio.stream.lock().unwrap() = Some(new_stream);
    *audio.device_name.lock().unwrap() = device_name.clone();
    *audio.channel_index.lock().unwrap() = channel_index;

    // Persist to disk
    let mut settings = crate::settings_manager::load_settings(&app);
    settings.device_name = device_name;
    settings.channel_index = channel_index;
    crate::settings_manager::save_settings(&app, &settings);
    
    Ok(())
}
