//! Desktop trigger inputs for scenes on Windows: the frontmost app's
//! executable name, and (not yet) whether any app is using the microphone.
//! A failing sensor is logged once and then reads as `None`/`false`.
//!
//! The name mapping builds on every OS so its tests run everywhere; the Win32
//! calls build on Windows only.

/// The executable's file stem from a full image path, e.g.
/// `C:\Program Files\Zoom\bin\Zoom.exe` -> `Zoom`. `None` when there is no name.
pub fn app_name_from_path(path: &str) -> Option<String> {
    let file = path.rsplit(['\\', '/']).next()?.trim();
    let stem = match file.len().checked_sub(4) {
        Some(cut) if file.is_char_boundary(cut) && file[cut..].eq_ignore_ascii_case(".exe") => {
            &file[..cut]
        }
        _ => file,
    };
    (!stem.is_empty()).then(|| stem.to_string())
}

/// The frontmost window's executable name, e.g. `Zoom`.
#[cfg(windows)]
pub fn frontmost_app_name() -> Option<String> {
    use std::sync::Once;

    static FAILED: Once = Once::new();
    match foreground_image_path() {
        Ok(path) => path.and_then(|path| app_name_from_path(&path)),
        Err(what) => {
            FAILED.call_once(|| eprintln!("hairline: frontmost app unknown ({what})"));
            None
        }
    }
}

/// The full image path of the process that owns the foreground window:
/// `Ok(None)` when no window has the foreground (nothing to log), `Err` when
/// the process could not be read.
#[cfg(windows)]
fn foreground_image_path() -> Result<Option<String>, &'static str> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    // SAFETY: no arguments; returns null when no window has the foreground.
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Ok(None);
    }
    let mut pid: u32 = 0;
    // SAFETY: `window` is a window handle and `pid` a valid u32 to write to.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if pid == 0 {
        return Ok(None);
    }
    // SAFETY: plain call; returns null on failure (e.g. access denied).
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err("OpenProcess failed");
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` u16s; on success `len` is the count written.
    let ok = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len)
    };
    // SAFETY: `process` is the open handle from `OpenProcess`, closed once.
    unsafe { CloseHandle(process) };
    if ok == 0 {
        return Err("QueryFullProcessImageNameW failed");
    }
    Ok(buf
        .get(..len as usize)
        .and_then(|name| String::from_utf16(name).ok()))
}

/// Whether any app is using the microphone: always `false` on Windows for
/// now, so microphone triggers never fire on a guess. The route for later is
/// WASAPI's `IAudioSessionManager2` on the default capture endpoint,
/// enumerating its sessions and checking for `AudioSessionStateActive`
/// (`IAudioSessionControl::GetState`).
#[cfg(windows)]
pub fn mic_in_use() -> bool {
    false
}
