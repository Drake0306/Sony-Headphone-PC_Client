//! Host audio on Windows: not read yet, so the Link list stays empty rather
//! than showing a guessed "playing".
//!
//! The route for later is WASAPI: `IMMDeviceEnumerator::GetDefaultAudioEndpoint`
//! (`eRender`, `eConsole`) for the default output, then its property store's
//! `PKEY_Device_FriendlyName` to compare with the headphones' name, the way
//! `audio_macos` does with CoreAudio. It builds on every OS so its tests run
//! everywhere; only Windows calls it.

use mdr_session::PairedDevice;

/// The host's entry in the Link list: none until the WASAPI module exists.
pub fn paired_devices(_device_name: Option<&str>) -> Vec<PairedDevice> {
    Vec::new()
}

/// The default output device's name: unknown until the WASAPI module exists.
pub fn output_device_name() -> Option<String> {
    None
}
