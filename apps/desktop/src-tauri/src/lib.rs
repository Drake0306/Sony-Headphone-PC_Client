//! Hairline desktop app: wires the broker, the tray, the popover and the commands.

mod audio_macos;
pub mod broker;
mod commands;
pub mod dock;
pub mod frame_log;
pub mod hotkeys;
mod popover;
pub mod scenes;
mod sensors_macos;
pub mod settings;
mod tray;

use std::sync::Mutex;

use mdr_session::DeviceState;
use tauri::Manager;

/// `--fake-device` or `HAIRLINE_FAKE=1` selects the scripted fake headphones.
fn wants_fake_device() -> bool {
    std::env::args().any(|a| a == "--fake-device")
        || std::env::var("HAIRLINE_FAKE").is_ok_and(|v| v == "1")
}

/// `HAIRLINE_DEBUG=1` registers the `debug_raw` command.
fn wants_debug() -> bool {
    std::env::var("HAIRLINE_DEBUG").is_ok_and(|v| v == "1")
}

type InvokeHandler = Box<dyn Fn(tauri::ipc::Invoke) -> bool + Send + Sync>;

/// The handler for every command, plus any `extra` ones.
macro_rules! handler {
    ($($extra:path),*) => {
        Box::new(tauri::generate_handler![
            commands::runtime_info,
            commands::get_state,
            commands::set_nc,
            commands::set_ult,
            commands::set_multipoint,
            commands::set_eq_preset,
            commands::set_eq_bands,
            commands::set_volume,
            commands::set_dsee_auto_off,
            commands::set_connection_mode,
            commands::set_auto_power_off,
            commands::set_wear_pause,
            commands::set_voice_assistant,
            commands::set_quick_access,
            commands::set_voice_guide_volume,
            commands::set_touch_panel,
            commands::set_sidetone,
            commands::get_scenes,
            commands::set_scenes,
            commands::apply_scene,
            commands::connect,
            commands::open_main_window,
            settings::get_settings,
            settings::set_settings,
            $($extra),*
        ])
    };
}

fn invoke_handler(debug: bool) -> InvokeHandler {
    if debug {
        handler!(commands::debug_raw)
    } else {
        handler!()
    }
}

pub fn run() {
    let fake = wants_fake_device();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build());
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .manage(Mutex::new(DeviceState::default()))
        .manage(commands::Runtime { fake })
        .manage(hotkeys::HotkeyErrors::default())
        .invoke_handler(invoke_handler(wants_debug()))
        .setup(move |app| {
            let dir = settings::dir(app.handle());
            let app_settings = match &dir {
                Ok(dir) => settings::load(dir),
                Err(e) => {
                    eprintln!("hairline: no settings directory ({e}); using defaults");
                    settings::AppSettings::default()
                }
            };
            let app_settings = settings::reconcile_launch_at_login(
                app.handle(),
                dir.as_deref().ok(),
                app_settings,
            );
            #[cfg(target_os = "macos")]
            app.set_activation_policy(dock::policy(false, app_settings.hide_dock_icon).into());
            app.manage(Mutex::new(app_settings));
            let scenes = match &dir {
                Ok(dir) => scenes::store::load(dir),
                Err(_) => scenes::store::seed_scenes(),
            };
            // Read by the broker thread when it starts.
            hotkeys::apply(app.handle(), &scenes);
            app.manage(Mutex::new(scenes));
            tray::create(app.handle())?;
            popover::create(app.handle())?;
            let tx = broker::spawn_broker(app.handle().clone(), fake);
            app.manage(commands::Broker(tx));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Hairline")
        .run(|app, event| {
            // Clicking the Dock icon reopens the main window.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                let _ = popover::open_main_window(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
