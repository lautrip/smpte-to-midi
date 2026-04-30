use crate::osc_sender;
use crate::midi_sender;
use rosc::OscType;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscAction {
    pub address: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiAction {
    pub msg_type: String, // "Note" or "CC"
    pub note: u8,         // Note number or CC number
    pub velocity: u8,     // Velocity or CC value
    pub channel: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: String,
    pub name: String,
    pub timestamp: String, // "HH:MM:SS:FF"
    pub osc: Option<OscAction>,
    pub midi: Option<MidiAction>,
}

pub struct TriggerState {
    pub triggers: Mutex<Vec<Trigger>>,
    pub global_osc_target: Mutex<String>,
    pub fired_triggers: Mutex<HashSet<String>>,
    pub fps: Mutex<f32>,
}

impl Default for TriggerState {
    fn default() -> Self {
        Self {
            triggers: Mutex::new(Vec::new()),
            global_osc_target: Mutex::new("127.0.0.1:8000".to_string()),
            fired_triggers: Mutex::new(HashSet::new()),
            fps: Mutex::new(25.0),
        }
    }
}

fn tc_to_frames(tc: &str, fps: f32) -> Option<u32> {
    let parts: Vec<&str> = tc.split(':').collect();
    if parts.len() != 4 { return None; }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    let s: u32 = parts[2].parse().ok()?;
    let f: u32 = parts[3].parse().ok()?;
    let fps_int = fps.round() as u32;
    Some(f + s * fps_int + m * 60 * fps_int + h * 3600 * fps_int)
}

pub fn check_triggers(current_time: &str, state: &tauri::State<TriggerState>, app: &tauri::AppHandle) {
    let fps = *state.fps.lock().unwrap();
    let current_frames = match tc_to_frames(current_time, fps) {
        Some(f) => f,
        None => return,
    };

    let triggers = state.triggers.lock().unwrap().clone();
    let mut fired = state.fired_triggers.lock().unwrap();
    let osc_target = state.global_osc_target.lock().unwrap().clone();

    for trigger in triggers {
        let trigger_frames = tc_to_frames(&trigger.timestamp, fps).unwrap_or(0);
        let is_after = current_frames >= trigger_frames;
        let is_too_far_after = current_frames > trigger_frames + (fps as u32);
        let was_fired = fired.contains(&trigger.id);

        if is_after && !is_too_far_after && !was_fired {
            fired.insert(trigger.id.clone());
            // Notify UI
            let _ = app.emit("trigger_fired", &trigger.id);

            if let Some(osc) = &trigger.osc {
                let osc_args = osc.args.iter().map(|a| {
                    if let Ok(i) = a.parse::<i32>() { OscType::Int(i) }
                    else if let Ok(f) = a.parse::<f32>() { OscType::Float(f) }
                    else { OscType::String(a.clone()) }
                }).collect();
                let _ = osc_sender::send_osc(&osc_target, &osc.address, osc_args);
            }
            if let Some(midi) = &trigger.midi {
                if midi.msg_type == "CC" {
                    let _ = midi_sender::send_midi_cc(midi.channel, midi.note, midi.velocity);
                } else {
                    let _ = midi_sender::send_midi_note(midi.channel, midi.note, midi.velocity);
                }
            }
        } else if current_frames < trigger_frames || is_too_far_after {
            fired.remove(&trigger.id);
        }
    }
}

fn save_triggers_to_settings(app: &AppHandle, triggers: Vec<Trigger>) {
    let mut settings = crate::settings_manager::load_settings(app);
    settings.triggers = triggers;
    crate::settings_manager::save_settings(app, &settings);
}

#[tauri::command]
pub fn add_trigger(trigger: Trigger, state: tauri::State<TriggerState>, app: AppHandle) {
    let mut triggers = state.triggers.lock().unwrap();
    triggers.push(trigger);
    save_triggers_to_settings(&app, triggers.clone());
}

#[tauri::command]
pub fn remove_trigger(id: String, state: tauri::State<TriggerState>, app: AppHandle) {
    let mut triggers = state.triggers.lock().unwrap();
    triggers.retain(|t| t.id != id);
    save_triggers_to_settings(&app, triggers.clone());
}

#[tauri::command]
pub fn update_trigger(trigger: Trigger, state: tauri::State<TriggerState>, app: AppHandle) {
    let mut triggers = state.triggers.lock().unwrap();
    if let Some(pos) = triggers.iter().position(|t| t.id == trigger.id) {
        triggers[pos] = trigger;
        save_triggers_to_settings(&app, triggers.clone());
    }
}

#[tauri::command]
pub fn get_triggers(state: tauri::State<TriggerState>) -> Vec<Trigger> {
    state.triggers.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_fps(fps: f32, state: tauri::State<TriggerState>, app: AppHandle) {
    *state.fps.lock().unwrap() = fps;
    let mut settings = crate::settings_manager::load_settings(&app);
    settings.fps = fps;
    crate::settings_manager::save_settings(&app, &settings);
}

#[tauri::command]
pub fn set_osc_target(target: String, state: tauri::State<TriggerState>, app: AppHandle) {
    *state.global_osc_target.lock().unwrap() = target.clone();
    let mut settings = crate::settings_manager::load_settings(&app);
    settings.osc_target = target;
    crate::settings_manager::save_settings(&app, &settings);
}

#[tauri::command]
pub fn set_midi_output(name: String, app: AppHandle) -> Result<(), String> {
    crate::midi_sender::switch_midi_output(name.clone())?;
    let mut settings = crate::settings_manager::load_settings(&app);
    settings.midi_device_name = name;
    crate::settings_manager::save_settings(&app, &settings);
    Ok(())
}
