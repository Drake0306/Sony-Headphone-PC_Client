//! Desktop trigger inputs for scenes: the frontmost app and whether any app is
//! using the microphone. A failing sensor is logged once and then reads as
//! `None`/`false`.
//!
//! Other non-Windows systems (Linux) report nothing yet. The routes for later
//! on Linux: the frontmost app has no portable answer under Wayland (X11 has
//! `_NET_ACTIVE_WINDOW`; GNOME and KDE need their own D-Bus interfaces), and
//! the microphone is in use when PipeWire has an active capture stream (a
//! `Stream/Input/Audio` node in the running state).

/// The frontmost app's localized name, e.g. `zoom.us`.
#[cfg(target_os = "macos")]
pub fn frontmost_app_name() -> Option<String> {
    use objc2_app_kit::NSWorkspace;
    use std::sync::Once;

    static FAILED: Once = Once::new();
    // The broker thread never ends, so its implicit pool would never drain.
    let name = objc2::rc::autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .and_then(|app| app.localizedName())
            .map(|name| name.to_string())
    });
    if name.is_none() {
        FAILED.call_once(|| eprintln!("shdc: frontmost app unknown; app triggers stay off"));
    }
    name
}

/// Whether the default input device is running in any process.
#[cfg(target_os = "macos")]
pub fn mic_in_use() -> bool {
    use coreaudio_sys::{
        kAudioDevicePropertyDeviceIsRunningSomewhere, kAudioHardwarePropertyDefaultInputDevice,
        kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
        kAudioObjectUnknown, AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress,
    };
    use std::mem::size_of;
    use std::sync::Once;

    static FAILED: Once = Once::new();
    let fail = |what: &str, status: i32| {
        FAILED.call_once(|| {
            eprintln!("shdc: {what} (CoreAudio {status}); microphone triggers stay off")
        });
        false
    };

    let mut device: AudioObjectID = kAudioObjectUnknown;
    let mut size = size_of::<AudioObjectID>() as u32;
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    // SAFETY: `device` is an AudioObjectID and `size` says so; CoreAudio writes at most `size` bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut device as *mut AudioObjectID).cast(),
        )
    };
    if status != 0 {
        return fail("no default input device", status);
    }
    if device == kAudioObjectUnknown {
        // No input device at all: nothing can be using one.
        return false;
    }

    let mut running: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDeviceIsRunningSomewhere,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };
    // SAFETY: the property is a UInt32 and `size` matches.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut running as *mut u32).cast(),
        )
    };
    if status != 0 {
        return fail("input device state unknown", status);
    }
    running != 0
}

/// Unknown off macOS until a Linux module exists.
#[cfg(not(target_os = "macos"))]
pub fn frontmost_app_name() -> Option<String> {
    None
}

/// Unknown off macOS until a Linux module (PipeWire) exists.
#[cfg(not(target_os = "macos"))]
pub fn mic_in_use() -> bool {
    false
}
