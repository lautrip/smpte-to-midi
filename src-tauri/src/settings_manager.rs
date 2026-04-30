use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use crate::trigger_manager::Trigger;
use rfd::FileDialog;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppSettings {
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub channel_index: usize,
    #[serde(default = "default_fps")]
    pub fps: f32,
    #[serde(default = "default_osc_target")]
    pub osc_target: String,
    #[serde(default = "default_midi_device")]
    pub midi_device_name: String,
    #[serde(default)]
    pub triggers: Vec<Trigger>,
}

fn default_fps() -> f32 { 25.0 }
fn default_osc_target() -> String { "127.0.0.1:8000".to_string() }
fn default_midi_device() -> String { "Virtual: SMPTE-to-MIDI".to_string() }

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            channel_index: 0,
            fps: 25.0,
            osc_target: "127.0.0.1:8000".to_string(),
            midi_device_name: "Virtual: SMPTE-to-MIDI".to_string(),
            triggers: Vec::new(),
        }
    }
}

pub fn get_settings_path(app: &AppHandle) -> PathBuf {
    let mut path = app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."));
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.push("settings.json");
    path
}

pub fn load_settings(app: &AppHandle) -> AppSettings {
    let path = get_settings_path(app);
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str::<AppSettings>(&data) {
            return settings;
        }
    }
    AppSettings::default()
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) {
    let path = get_settings_path(app);
    if let Ok(data) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(&path, data);
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    load_settings(&app)
}

// ── CSV Export / Import (Hotcues only) ────────────────────────────────────────

fn triggers_to_csv(triggers: &[Trigger]) -> String {
    let mut lines = vec![
        "id,name,timestamp,osc_address,osc_args,midi_type,midi_note,midi_velocity,midi_channel"
            .to_string(),
    ];
    for t in triggers {
        let osc_addr = t.osc.as_ref().map(|o| o.address.clone()).unwrap_or_default();
        let osc_args = t.osc.as_ref().map(|o| o.args.join(" ")).unwrap_or_default();
        let midi_type = t.midi.as_ref().map(|m| m.msg_type.clone()).unwrap_or_default();
        let midi_note = t.midi.as_ref().map(|m| m.note.to_string()).unwrap_or_default();
        let midi_vel  = t.midi.as_ref().map(|m| m.velocity.to_string()).unwrap_or_default();
        let midi_ch   = t.midi.as_ref().map(|m| m.channel.to_string()).unwrap_or_default();
        lines.push(format!(
            "{},{},{},{},{},{},{},{},{}",
            csv_escape(&t.id),
            csv_escape(&t.name),
            csv_escape(&t.timestamp),
            csv_escape(&osc_addr),
            csv_escape(&osc_args),
            csv_escape(&midi_type),
            csv_escape(&midi_note),
            csv_escape(&midi_vel),
            csv_escape(&midi_ch),
        ));
    }
    lines.join("\n")
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_to_triggers(data: &str) -> Result<Vec<Trigger>, String> {
    use crate::trigger_manager::{OscAction, MidiAction};
    let mut lines = data.lines();
    // Skip header
    lines.next();
    let mut triggers = Vec::new();
    for line in lines {
        if line.trim().is_empty() { continue; }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() < 9 { return Err(format!("Invalid CSV row: {}", line)); }
        let id         = cols[0].trim_matches('"').to_string();
        let name       = cols[1].trim_matches('"').to_string();
        let timestamp  = cols[2].trim_matches('"').to_string();
        let osc_addr   = cols[3].trim_matches('"').to_string();
        let osc_args   = cols[4].trim_matches('"').to_string();
        let midi_type  = cols[5].trim_matches('"').to_string();
        let midi_note  = cols[6].trim_matches('"').to_string();
        let midi_vel   = cols[7].trim_matches('"').to_string();
        let midi_ch    = cols[8].trim_matches('"').to_string();

        let osc = if osc_addr.is_empty() { None } else {
            Some(OscAction {
                address: osc_addr,
                args: if osc_args.is_empty() { vec![] } else { osc_args.split(' ').map(|s| s.to_string()).collect() },
            })
        };
        let midi = if midi_type.is_empty() { None } else {
            Some(MidiAction {
                msg_type: midi_type,
                note:     midi_note.parse().unwrap_or(0),
                velocity: midi_vel.parse().unwrap_or(100),
                channel:  midi_ch.parse().unwrap_or(0),
            })
        };
        triggers.push(Trigger { id, name, timestamp, osc, midi });
    }
    Ok(triggers)
}

#[tauri::command]
pub fn export_hotcues(app: AppHandle) -> Result<(), String> {
    let settings = load_settings(&app);
    let csv = triggers_to_csv(&settings.triggers);
    let path = FileDialog::new()
        .set_file_name("hotcues.csv")
        .add_filter("CSV", &["csv"])
        .save_file();
    if let Some(path) = path {
        fs::write(path, csv).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("Export cancelled".into())
    }
}

#[tauri::command]
pub fn import_hotcues(app: AppHandle) -> Result<Vec<Trigger>, String> {
    let path = FileDialog::new()
        .add_filter("CSV", &["csv"])
        .pick_file();
    if let Some(path) = path {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let triggers = csv_to_triggers(&data)?;
        // Persist imported hotcues into settings
        let mut settings = load_settings(&app);
        settings.triggers = triggers.clone();
        save_settings(&app, &settings);
        Ok(triggers)
    } else {
        Err("Import cancelled".into())
    }
}
