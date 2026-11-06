//! The popover window (an NSPanel on macOS) and the main window.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::dock::{self, ActivationPolicyKind};
use crate::settings::AppSettings;

const LABEL: &str = "popover";
const MAIN: &str = "main";
/// A tray click right after a focus-loss hide is the same click; don't reopen.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

static HIDDEN_AT: Mutex<Option<Instant>> = Mutex::new(None);

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(PopoverPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Hairline")
        .inner_size(340.0, 420.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .resizable(false)
        .shadow(false)
        .build()?;

    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::{PanelLevel, StyleMask, WebviewWindowExt};
        let panel = window.to_panel::<PopoverPanel>()?;
        panel.set_level(PanelLevel::Floating.value());
        if let Err(e) = panel.add_style_mask(StyleMask::empty().nonactivating_panel().value()) {
            eprintln!("hairline: popover stays activating: {e:?}");
        }
    }

    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide(&handle);
            if let Ok(mut at) = HIDDEN_AT.lock() {
                *at = Some(Instant::now());
            }
        }
    });
    Ok(())
}

pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
        return;
    }
    let just_hidden = HIDDEN_AT
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|t| t.elapsed() < REOPEN_GUARD);
    if !just_hidden {
        show(app, &window);
    }
}

fn show(app: &AppHandle, window: &WebviewWindow) {
    let _ = window.move_window(Position::TrayCenter);
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel(LABEL) {
            panel.show_and_make_key();
            return;
        }
    }
    let _ = app;
    let _ = window.show();
    let _ = window.set_focus();
}

fn hide(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt;
        if let Ok(panel) = app.get_webview_panel(LABEL) {
            panel.hide();
            return;
        }
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

/// Focuses the main window, creating it on first use, and brings the app
/// forward. The Dock icon shows while the window exists.
pub fn open_main_window(app: &AppHandle) -> tauri::Result<()> {
    hide(app);
    // Regular before showing, so the window can come to the front.
    dock::apply(app, ActivationPolicyKind::Regular);
    #[cfg(target_os = "macos")]
    app.show()?;
    if let Some(window) = app.get_webview_window(MAIN) {
        window.show()?;
        window.unminimize()?;
        return window.set_focus();
    }
    let window =
        WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html?view=main".into()))
            .title("Hairline")
            .inner_size(880.0, 560.0)
            .min_inner_size(880.0, 560.0)
            .resizable(true)
            .visible(true)
            .build()?;
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            let hide_dock_icon = handle
                .state::<Mutex<AppSettings>>()
                .lock()
                .map(|s| s.hide_dock_icon)
                .unwrap_or(true);
            dock::apply_for(&handle, false, hide_dock_icon);
        }
    });
    window.set_focus()
}
