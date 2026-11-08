//! The popover window (an NSPanel on macOS, a plain borderless window beside
//! the tray elsewhere) and the main window.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::tray::TrayIconEvent;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::dock::{self, ActivationPolicyKind};
use crate::settings::AppSettings;

const LABEL: &str = "popover";
/// The main window's label.
pub(crate) const MAIN: &str = "main";
/// A tray click right after a focus-loss hide is the same click; don't reopen.
const REOPEN_GUARD: Duration = Duration::from_millis(300);

static HIDDEN_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// The tray icon's rectangle from the last tray event, physical pixels
/// (x, y, width, height). Windows places the popover from it.
static TRAY_RECT: Mutex<Option<(i32, i32, u32, u32)>> = Mutex::new(None);

/// The popover's logical size, also set on the window builder.
pub const POPOVER_SIZE: (f64, f64) = (340.0, 420.0);
/// Logical gap between the popover and the taskbar or screen edge.
const GAP: f64 = 8.0;

/// One monitor in physical pixels: its bounds, its work area (the bounds
/// minus the taskbar) and its scale factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// x, y, width, height.
    pub bounds: (i32, i32, u32, u32),
    /// x, y, width, height.
    pub work_area: (i32, i32, u32, u32),
    pub scale: f64,
}

/// Where the popover goes on Windows, in physical pixels: inside the work
/// area, against the taskbar's edge with a small gap, and centred on the tray
/// icon along that edge when its rectangle is known (otherwise at the end of
/// the taskbar, where the notification area is). The taskbar's edge is read
/// from where the work area falls short of the monitor, so a taskbar at the
/// top or at a side is handled too. The vertical position never depends on
/// the tray rectangle, which keeps the popover by the taskbar whatever the
/// shell reports for the icon.
pub fn windows_position(screen: Screen, tray: Option<(i32, i32, u32, u32)>) -> (i32, i32) {
    let w = (POPOVER_SIZE.0 * screen.scale).round() as i32;
    let h = (POPOVER_SIZE.1 * screen.scale).round() as i32;
    let gap = (GAP * screen.scale).round() as i32;
    let (mx, my, mw, mh) = screen.bounds;
    let (wx, wy, ww, wh) = screen.work_area;
    let (m_right, m_bottom) = (mx + mw as i32, my + mh as i32);
    let (w_right, w_bottom) = (wx + ww as i32, wy + wh as i32);
    let centre = tray.map(|(x, y, tw, th)| (x + tw as i32 / 2, y + th as i32 / 2));
    let (x_min, x_max) = (wx + gap, w_right - w - gap);
    let (y_min, y_max) = (wy + gap, w_bottom - h - gap);
    let (x, y) = if w_bottom < m_bottom || (wy == my && wx == mx && w_right == m_right) {
        // Taskbar at the bottom (or hidden): sit on the work area's bottom edge.
        (centre.map_or(x_max, |(cx, _)| cx - w / 2), y_max)
    } else if wy > my {
        // Taskbar at the top.
        (centre.map_or(x_max, |(cx, _)| cx - w / 2), y_min)
    } else if wx > mx {
        // Taskbar on the left.
        (x_min, centre.map_or(y_max, |(_, cy)| cy - h / 2))
    } else {
        // Taskbar on the right.
        (x_max, centre.map_or(y_max, |(_, cy)| cy - h / 2))
    };
    // Clamp, keeping the top-left on screen when the work area is small.
    (x.min(x_max).max(x_min), y.min(y_max).max(y_min))
}

/// Remembers the tray icon's rectangle from a tray event (every platform; only
/// Windows reads it).
pub fn note_tray_event(event: &TrayIconEvent) {
    let rect = match event {
        TrayIconEvent::Click { rect, .. }
        | TrayIconEvent::DoubleClick { rect, .. }
        | TrayIconEvent::Enter { rect, .. }
        | TrayIconEvent::Move { rect, .. }
        | TrayIconEvent::Leave { rect, .. } => rect,
        _ => return,
    };
    // tray-icon reports physical pixels, so a scale of 1 changes nothing.
    let pos = rect.position.to_physical::<i32>(1.0);
    let size = rect.size.to_physical::<u32>(1.0);
    if let Ok(mut slot) = TRAY_RECT.lock() {
        *slot = Some((pos.x, pos.y, size.width, size.height));
    }
}

/// What a left click on the tray icon does to the popover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// The popover is showing: hide it.
    Hide,
    /// The popover is hidden: show it.
    Show,
    /// The popover was hidden by focus loss within the reopen guard: this is
    /// the same click that took the focus away, so leave it hidden.
    Ignore,
}

/// The tray-click decision on every platform: `hidden_at` is when focus loss
/// last hid the popover, `visible` whether it shows now.
pub fn tray_click_action(hidden_at: Option<Instant>, now: Instant, visible: bool) -> TrayAction {
    if visible {
        return TrayAction::Hide;
    }
    let just_hidden = hidden_at.is_some_and(|t| now.saturating_duration_since(t) < REOPEN_GUARD);
    if just_hidden {
        TrayAction::Ignore
    } else {
        TrayAction::Show
    }
}

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
        .title("SHDC")
        .inner_size(POPOVER_SIZE.0, POPOVER_SIZE.1)
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
            eprintln!("shdc: popover stays activating: {e:?}");
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
    let hidden_at = HIDDEN_AT.lock().ok().and_then(|at| *at);
    let visible = window.is_visible().unwrap_or(false);
    match tray_click_action(hidden_at, Instant::now(), visible) {
        TrayAction::Hide => hide(app),
        TrayAction::Show => show(app, &window),
        TrayAction::Ignore => {}
    }
}

/// Places the popover by the tray icon: centred under the menu-bar icon on
/// macOS (the positioner plugin). On Windows the position is computed here
/// from the monitor's work area and the tray icon's rectangle, see
/// `windows_position`; the positioner is the fallback when no monitor can be
/// found. Linux trays report no icon position, so there it goes to the
/// top-right corner of the screen, where most panels keep the tray.
fn place(window: &WebviewWindow) {
    #[cfg(target_os = "windows")]
    if let Err(e) = place_windows(window) {
        eprintln!("shdc: popover not placed by the work area ({e}); using the positioner");
        let _ = window.move_window_constrained(Position::TrayCenter);
    }
    #[cfg(target_os = "macos")]
    let _ = window.move_window(Position::TrayCenter);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = window.move_window(Position::TopRight);
}

#[cfg(target_os = "windows")]
fn place_windows(window: &WebviewWindow) -> tauri::Result<()> {
    let tray = TRAY_RECT.lock().ok().and_then(|t| *t);
    let by_tray = match tray {
        Some((x, y, w, h)) => {
            window.monitor_from_point((x + w as i32 / 2) as f64, (y + h as i32 / 2) as f64)?
        }
        None => None,
    };
    let monitor = match by_tray {
        Some(m) => Some(m),
        None => match window.primary_monitor()? {
            Some(m) => Some(m),
            None => window.current_monitor()?,
        },
    };
    let Some(monitor) = monitor else {
        return Err(tauri::Error::Io(std::io::Error::other("no monitor")));
    };
    let wa = monitor.work_area();
    let screen = Screen {
        bounds: (
            monitor.position().x,
            monitor.position().y,
            monitor.size().width,
            monitor.size().height,
        ),
        work_area: (wa.position.x, wa.position.y, wa.size.width, wa.size.height),
        scale: monitor.scale_factor(),
    };
    let (x, y) = windows_position(screen, tray);
    eprintln!(
        "shdc: popover at ({x}, {y}); tray {tray:?}; monitor {:?}; work area {:?}; scale {}",
        screen.bounds, screen.work_area, screen.scale
    );
    window.set_position(tauri::PhysicalPosition::new(x, y))
}

/// Shows the popover (the Linux tray menu's "Show SHDC").
pub fn show_popover(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        show(app, &window);
    }
}

fn show(app: &AppHandle, window: &WebviewWindow) {
    place(window);
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
    // Windows may apply a hidden window's position only once it is shown.
    #[cfg(target_os = "windows")]
    place(window);
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
    if let Some(window) = app.get_webview_window(MAIN) {
        bring_forward(app)?;
        window.show()?;
        window.unminimize()?;
        return window.set_focus();
    }
    // Hidden until its page has loaded (no white flash, and on Windows a
    // webview shown before it has painted could stay white); a timer shows
    // it anyway after a moment so a page that never loads is still a window
    // the user can see and report.
    let window =
        WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html?view=main".into()))
            .title("SHDC · Sony Headphone Desktop Client")
            .inner_size(880.0, 560.0)
            .min_inner_size(880.0, 560.0)
            .resizable(true)
            .visible(false)
            .on_page_load(|window, payload| {
                crate::diag::log(
                    window.app_handle(),
                    &format!("main window page {:?}: {}", payload.event(), payload.url()),
                );
                if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                    reveal(&window, "page loaded");
                }
            })
            .build()?;
    crate::diag::log(
        app,
        &format!(
            "main window created at {}",
            window.url().map(|u| u.to_string()).unwrap_or_default()
        ),
    );
    let fallback = window.clone();
    std::thread::Builder::new()
        .name("shdc-main-window-reveal".into())
        .spawn(move || {
            std::thread::sleep(REVEAL_FALLBACK);
            if !fallback.is_visible().unwrap_or(true) {
                reveal(&fallback, "no page load after the fallback wait");
            }
        })
        .ok();
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
    // Only once the window exists, so a failed build leaves no Dock icon behind.
    bring_forward(app)
}

/// How long a new main window may stay hidden waiting for its page.
const REVEAL_FALLBACK: Duration = Duration::from_millis(1500);

/// Shows and focuses the main window, once; logs why.
fn reveal(window: &WebviewWindow, why: &str) {
    if window.is_visible().unwrap_or(false) {
        return;
    }
    crate::diag::log(window.app_handle(), &format!("main window shown ({why})"));
    let _ = window.show();
    let _ = window.set_focus();
}

/// Switches to the Regular policy (Dock icon) and unhides the app, so the main
/// window can come to the front.
fn bring_forward(app: &AppHandle) -> tauri::Result<()> {
    dock::apply(app, ActivationPolicyKind::Regular);
    #[cfg(target_os = "macos")]
    app.show()?;
    Ok(())
}

/// Opens the main window's web inspector (creating the window first), from
/// the tray menu's "Web inspector".
pub fn open_inspector(app: &AppHandle) {
    if app.get_webview_window(MAIN).is_none() {
        if let Err(e) = open_main_window(app) {
            crate::diag::log(
                app,
                &format!("main window not created for the inspector: {e}"),
            );
            return;
        }
    }
    if let Some(window) = app.get_webview_window(MAIN) {
        window.open_devtools();
    }
}
