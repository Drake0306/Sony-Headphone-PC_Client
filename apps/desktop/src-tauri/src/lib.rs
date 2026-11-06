//! Hairline desktop app: wires the broker, the tray, the popover and the commands.

mod audio_macos;
pub mod broker;
mod commands;
mod popover;
mod tray;

use std::sync::Mutex;

use mdr_session::DeviceState;
use tauri::Manager;

/// `--fake-device` or `HAIRLINE_FAKE=1` selects the scripted fake headphones.
fn wants_fake_device() -> bool {
    std::env::args().any(|a| a == "--fake-device")
        || std::env::var("HAIRLINE_FAKE").is_ok_and(|v| v == "1")
}

pub fn run() {
    let fake = wants_fake_device();
    let builder = tauri::Builder::default().plugin(tauri_plugin_positioner::init());
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    builder
        .manage(Mutex::new(DeviceState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_nc,
            commands::set_ult,
            commands::set_multipoint,
            commands::connect,
            commands::open_main_window,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::create(app.handle())?;
            popover::create(app.handle())?;
            let tx = broker::spawn_broker(app.handle().clone(), fake);
            app.manage(commands::Broker(tx));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Hairline");
}
