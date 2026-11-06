//! Whether Hairline shows a Dock icon: while the main window is open, or
//! always when the user turns "hide Dock icon" off.

use tauri::AppHandle;

/// A Tauri-free stand-in for `tauri::ActivationPolicy`, so the rule is testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationPolicyKind {
    /// Dock icon and menu bar, like an ordinary app.
    Regular,
    /// Menu-bar only.
    Accessory,
}

#[cfg(target_os = "macos")]
impl From<ActivationPolicyKind> for tauri::ActivationPolicy {
    fn from(kind: ActivationPolicyKind) -> Self {
        match kind {
            ActivationPolicyKind::Regular => tauri::ActivationPolicy::Regular,
            ActivationPolicyKind::Accessory => tauri::ActivationPolicy::Accessory,
        }
    }
}

pub fn policy(main_window_open: bool, hide_dock_icon: bool) -> ActivationPolicyKind {
    if main_window_open || !hide_dock_icon {
        ActivationPolicyKind::Regular
    } else {
        ActivationPolicyKind::Accessory
    }
}

/// Applies `policy(main_window_open, hide_dock_icon)` on the main thread (macOS only).
pub fn apply_for(app: &AppHandle, main_window_open: bool, hide_dock_icon: bool) {
    apply(app, policy(main_window_open, hide_dock_icon));
}

/// Sets the activation policy on the main thread (macOS only).
pub fn apply(app: &AppHandle, kind: ActivationPolicyKind) {
    #[cfg(target_os = "macos")]
    {
        let handle = app.clone();
        let result = app.run_on_main_thread(move || {
            if let Err(e) = handle.set_activation_policy(kind.into()) {
                eprintln!("hairline: activation policy not set: {e}");
            }
        });
        if let Err(e) = result {
            eprintln!("hairline: activation policy not scheduled: {e}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, kind);
}
