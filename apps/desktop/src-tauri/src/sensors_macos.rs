//! Desktop trigger inputs for scenes: the frontmost app and whether any app is
//! using the microphone. Other platforms report nothing. A failing sensor is
//! logged once and then reads as `None`/`false`.

/// The frontmost app's localized name, e.g. `zoom.us`.
#[cfg(target_os = "macos")]
pub fn frontmost_app_name() -> Option<String> {
    use objc2_app_kit::NSWorkspace;
    use std::sync::Once;

    static FAILED: Once = Once::new();
    let workspace = NSWorkspace::sharedWorkspace();
    let name = workspace
        .frontmostApplication()
        .and_then(|app| app.localizedName())
        .map(|name| name.to_string());
    if name.is_none() {
        FAILED.call_once(|| eprintln!("hairline: frontmost app unknown; app triggers stay off"));
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
            eprintln!("hairline: {what} (CoreAudio {status}); microphone triggers stay off")
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

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app_name() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn mic_in_use() -> bool {
    false
}
