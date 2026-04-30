use midir::{MidiOutput, MidiOutputConnection};
use midir::os::unix::VirtualOutput; // Required for virtual ports on macOS
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    static ref MIDI_OUT: Mutex<Option<MidiOutputConnection>> = Mutex::new(None);
}

#[tauri::command]
pub fn get_midi_outputs() -> Vec<String> {
    let midi_out = match MidiOutput::new("SMPTE-to-MIDI Enumerator") {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    let mut names = vec!["Virtual: SMPTE-to-MIDI".to_string()];
    for port in midi_out.ports() {
        if let Ok(name) = midi_out.port_name(&port) {
            names.push(name);
        }
    }
    names
}

#[tauri::command]
pub fn switch_midi_output(name: String) -> Result<(), String> {
    let midi_out = MidiOutput::new("SMPTE-to-MIDI Output").map_err(|e| e.to_string())?;
    
    let mut global_conn = MIDI_OUT.lock().unwrap();
    *global_conn = None; // Close previous

    if name.starts_with("Virtual:") {
        let conn = midi_out.create_virtual("SMPTE-to-MIDI").map_err(|e| e.to_string())?;
        *global_conn = Some(conn);
    } else {
        let ports = midi_out.ports();
        for port in ports {
            if let Ok(pname) = midi_out.port_name(&port) {
                if pname == name {
                    let conn = midi_out.connect(&port, "SMPTE-to-MIDI-Conn").map_err(|e| e.to_string())?;
                    *global_conn = Some(conn);
                    return Ok(());
                }
            }
        }
        return Err("MIDI port not found".into());
    }
    Ok(())
}

pub fn send_midi_note(channel: u8, note: u8, velocity: u8) -> Result<(), String> {
    let mut global_conn = MIDI_OUT.lock().unwrap();
    if let Some(conn) = global_conn.as_mut() {
        // Note On: 0x90 + channel
        let msg = [0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F];
        let _ = conn.send(&msg);
        Ok(())
    } else {
        Err("MIDI not initialized".into())
    }
}

pub fn send_midi_cc(channel: u8, controller: u8, value: u8) -> Result<(), String> {
    let mut global_conn = MIDI_OUT.lock().unwrap();
    if let Some(conn) = global_conn.as_mut() {
        // Control Change: 0xB0 + channel
        let msg = [0xB0 | (channel & 0x0F), controller & 0x7F, value & 0x7F];
        let _ = conn.send(&msg);
        Ok(())
    } else {
        Err("MIDI not initialized".into())
    }
}
