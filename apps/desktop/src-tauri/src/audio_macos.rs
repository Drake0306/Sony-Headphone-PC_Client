//! Name of the host's default audio output device (CoreAudio on macOS).

/// The default output device's name, or `None` when unknown or not on macOS.
#[cfg(target_os = "macos")]
pub fn current_output_device_name() -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use std::ffi::c_void;
    use std::mem::size_of;

    #[repr(C)]
    struct PropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            data_size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    const fn fourcc(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }
    const SYSTEM_OBJECT: u32 = 1;
    const DEFAULT_OUTPUT_DEVICE: u32 = fourcc(b"dOut");
    const OBJECT_NAME: u32 = fourcc(b"lnam");
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const ELEMENT_MAIN: u32 = 0;

    let mut device: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    let address = PropertyAddress { selector: DEFAULT_OUTPUT_DEVICE, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    // SAFETY: `device` is a u32 and `size` says so; CoreAudio writes at most `size` bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut device as *mut u32).cast(),
        )
    };
    if status != 0 || device == 0 {
        return None;
    }

    let mut name: CFStringRef = std::ptr::null();
    let mut size = size_of::<CFStringRef>() as u32;
    let address = PropertyAddress { selector: OBJECT_NAME, scope: SCOPE_GLOBAL, element: ELEMENT_MAIN };
    // SAFETY: the property is a CFStringRef the caller owns (create rule); `size` matches.
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            std::ptr::null(),
            &mut size,
            (&mut name as *mut CFStringRef).cast(),
        )
    };
    if status != 0 || name.is_null() {
        return None;
    }
    // SAFETY: non-null CFString returned under the create rule; wrapping releases it once.
    Some(unsafe { CFString::wrap_under_create_rule(name) }.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn current_output_device_name() -> Option<String> {
    None
}
