mod audio_engine;
mod osc_sender;
mod midi_sender;
mod trigger_manager;
mod settings_manager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(audio_engine::AudioState::default())
        .manage(trigger_manager::TriggerState::default())
        .invoke_handler(tauri::generate_handler![
            audio_engine::get_audio_devices,
            audio_engine::switch_device,
            trigger_manager::add_trigger,
            trigger_manager::remove_trigger,
            trigger_manager::update_trigger,
            trigger_manager::get_triggers,
            trigger_manager::set_fps,
            trigger_manager::set_osc_target,
            settings_manager::get_settings,
            settings_manager::export_hotcues,
            settings_manager::import_hotcues,
            midi_sender::get_midi_outputs,
            trigger_manager::set_midi_output,
        ])
        .setup(|app| {
            let settings = settings_manager::load_settings(app.handle());
            
            // Sync states with settings
            {
                let trigger_state = app.state::<trigger_manager::TriggerState>();
                *trigger_state.fps.lock().unwrap() = settings.fps;
                *trigger_state.global_osc_target.lock().unwrap() = settings.osc_target.clone();
                *trigger_state.triggers.lock().unwrap() = settings.triggers;
            }

            // Initialize MIDI with saved device
            let _ = midi_sender::switch_midi_output(settings.midi_device_name);

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                audio_engine::auto_start(&handle);
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
