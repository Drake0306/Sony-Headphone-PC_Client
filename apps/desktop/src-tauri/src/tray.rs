//! The menu-bar icon: left click toggles the popover, right click offers Quit.
//! Linux trays (StatusNotifierItem through libayatana-appindicator) report no
//! clicks, only the menu, so there the menu also offers "Show SHDC".

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::popover;

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit SHDC", true, None::<&str>)?;
    // With the Advanced panel on at launch, the menu also opens the web
    // inspector of the main window: the way to read a page's console when
    // the page itself shows nothing.
    let advanced = app
        .try_state::<std::sync::Mutex<crate::settings::AppSettings>>()
        .and_then(|s| s.lock().ok().map(|s| s.show_advanced))
        .unwrap_or(false);
    let inspect = advanced
        .then(|| MenuItem::with_id(app, "inspect", "Web inspector", true, None::<&str>))
        .transpose()?;
    #[cfg(target_os = "linux")]
    let show = MenuItem::with_id(app, "show", "Show SHDC", true, None::<&str>)?;
    let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = Vec::new();
    #[cfg(target_os = "linux")]
    items.push(&show);
    if let Some(item) = &inspect {
        items.push(item);
    }
    items.push(&quit);
    let menu = Menu::with_items(app, &items)?;
    // macOS tints the monochrome template icon for the menu bar. Windows and
    // Linux have no template icons, and the black glyph vanishes on a dark
    // taskbar or panel, so they get the full-colour app icon (a dark tile,
    // readable on light and dark).
    #[cfg(target_os = "macos")]
    let builder = TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true);
    #[cfg(not(target_os = "macos"))]
    let builder = TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?);
    builder
        .tooltip("SHDC · Sony Headphone Desktop Client")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "show" => popover::show_popover(app),
            "inspect" => popover::open_inspector(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            popover::note_tray_event(&event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                popover::toggle(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}
