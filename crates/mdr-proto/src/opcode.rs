//! Opcode and sub-type byte constants (plain `u8`), per `docs/protocol-notes.md`.

/// Table 1 (COMMAND_1) opcodes.
pub mod t1 {
    pub const CONNECT_GET_PROTOCOL_INFO: u8 = 0x00;
    pub const CONNECT_RET_PROTOCOL_INFO: u8 = 0x01;
    pub const CONNECT_GET_CAPABILITY_INFO: u8 = 0x02;
    pub const CONNECT_RET_CAPABILITY_INFO: u8 = 0x03;
    pub const CONNECT_GET_DEVICE_INFO: u8 = 0x04;
    pub const CONNECT_RET_DEVICE_INFO: u8 = 0x05;
    pub const CONNECT_GET_SUPPORT_FUNCTION: u8 = 0x06;
    pub const CONNECT_RET_SUPPORT_FUNCTION: u8 = 0x07;

    pub const COMMON_GET_CAPABILITY: u8 = 0x10;
    pub const COMMON_RET_CAPABILITY: u8 = 0x11;
    pub const COMMON_GET_STATUS: u8 = 0x12;
    pub const COMMON_RET_STATUS: u8 = 0x13;
    pub const COMMON_NTFY_STATUS: u8 = 0x15;
    pub const COMMON_SET_PARAM: u8 = 0x18;
    pub const COMMON_NTFY_PARAM: u8 = 0x19;

    pub const POWER_GET_CAPABILITY: u8 = 0x20;
    pub const POWER_RET_CAPABILITY: u8 = 0x21;
    pub const POWER_GET_STATUS: u8 = 0x22;
    pub const POWER_RET_STATUS: u8 = 0x23;
    pub const POWER_SET_STATUS: u8 = 0x24;
    pub const POWER_NTFY_STATUS: u8 = 0x25;
    pub const POWER_GET_PARAM: u8 = 0x26;
    pub const POWER_RET_PARAM: u8 = 0x27;
    pub const POWER_SET_PARAM: u8 = 0x28;
    pub const POWER_NTFY_PARAM: u8 = 0x29;

    /// Firmware update (UPDT) family: `UPDT_FIRST..=UPDT_LAST`, plus `UPDT_EXTRA`.
    pub const UPDT_EXTRA: u8 = 0x2D;
    pub const UPDT_FIRST: u8 = 0x30;
    pub const UPDT_LAST: u8 = 0x3F;

    pub const EQEBB_GET_CAPABILITY: u8 = 0x50;
    pub const EQEBB_RET_CAPABILITY: u8 = 0x51;
    pub const EQEBB_GET_PARAM: u8 = 0x56;
    pub const EQEBB_RET_PARAM: u8 = 0x57;
    pub const EQEBB_SET_PARAM: u8 = 0x58;
    pub const EQEBB_NTFY_PARAM: u8 = 0x59;
    pub const EQEBB_GET_EXTENDED_INFO: u8 = 0x5A;
    pub const EQEBB_RET_EXTENDED_INFO: u8 = 0x5B;

    pub const NCASM_GET_CAPABILITY: u8 = 0x60;
    pub const NCASM_RET_CAPABILITY: u8 = 0x61;
    pub const NCASM_GET_PARAM: u8 = 0x66;
    pub const NCASM_RET_PARAM: u8 = 0x67;
    pub const NCASM_SET_PARAM: u8 = 0x68;
    pub const NCASM_NTFY_PARAM: u8 = 0x69;

    /// ALERT_SET_PARAM family (reboot/commit sequences). Deny-listed.
    pub const ALERT_SET_PARAM: u8 = 0x98;

    pub const PLAY_GET_CAPABILITY: u8 = 0xA0;
    pub const PLAY_RET_CAPABILITY: u8 = 0xA1;
    pub const PLAY_GET_STATUS: u8 = 0xA2;
    pub const PLAY_RET_STATUS: u8 = 0xA3;
    pub const PLAY_SET_STATUS: u8 = 0xA4;
    pub const PLAY_NTFY_STATUS: u8 = 0xA5;
    pub const PLAY_GET_PARAM: u8 = 0xA6;
    pub const PLAY_RET_PARAM: u8 = 0xA7;
    pub const PLAY_SET_PARAM: u8 = 0xA8;
    pub const PLAY_NTFY_PARAM: u8 = 0xA9;

    pub const LOG_SET_STATUS: u8 = 0xC4;
    pub const LOG_NTFY_STATUS: u8 = 0xC9;

    pub const GS_GET_CAPABILITY: u8 = 0xD0;
    pub const GS_RET_CAPABILITY: u8 = 0xD1;
    pub const GS_GET_STATUS: u8 = 0xD2;
    pub const GS_RET_STATUS: u8 = 0xD3;
    pub const GS_NTFY_STATUS: u8 = 0xD5;
    pub const GS_GET_PARAM: u8 = 0xD6;
    pub const GS_RET_PARAM: u8 = 0xD7;
    pub const GS_SET_PARAM: u8 = 0xD8;
    pub const GS_NTFY_PARAM: u8 = 0xD9;

    pub const AUDIO_GET_CAPABILITY: u8 = 0xE0;
    pub const AUDIO_RET_CAPABILITY: u8 = 0xE1;
    pub const AUDIO_GET_STATUS: u8 = 0xE2;
    pub const AUDIO_RET_STATUS: u8 = 0xE3;
    pub const AUDIO_NTFY_STATUS: u8 = 0xE5;
    pub const AUDIO_GET_PARAM: u8 = 0xE6;
    pub const AUDIO_RET_PARAM: u8 = 0xE7;
    pub const AUDIO_SET_PARAM: u8 = 0xE8;
    pub const AUDIO_NTFY_PARAM: u8 = 0xE9;

    pub const SYSTEM_GET_CAPABILITY: u8 = 0xF0;
    pub const SYSTEM_RET_CAPABILITY: u8 = 0xF1;
    pub const SYSTEM_GET_STATUS: u8 = 0xF2;
    pub const SYSTEM_RET_STATUS: u8 = 0xF3;
    pub const SYSTEM_NTFY_STATUS: u8 = 0xF5;
    pub const SYSTEM_GET_PARAM: u8 = 0xF6;
    pub const SYSTEM_RET_PARAM: u8 = 0xF7;
    pub const SYSTEM_SET_PARAM: u8 = 0xF8;
    pub const SYSTEM_NTFY_PARAM: u8 = 0xF9;
    pub const SYSTEM_GET_EXT_PARAM: u8 = 0xFA;
    pub const SYSTEM_RET_EXT_PARAM: u8 = 0xFB;
    pub const SYSTEM_SET_EXT_PARAM: u8 = 0xFC;
    pub const SYSTEM_NTFY_EXT_PARAM: u8 = 0xFD;
}

/// Table 2 (COMMAND_2) opcodes.
pub mod t2 {
    pub const CONNECT_GET_SUPPORT_FUNCTION: u8 = 0x06;
    pub const CONNECT_RET_SUPPORT_FUNCTION: u8 = 0x07;
    pub const VOICE_GUIDANCE_GET_PARAM: u8 = 0x46;
    pub const VOICE_GUIDANCE_RET_PARAM: u8 = 0x47;
    pub const VOICE_GUIDANCE_SET_PARAM: u8 = 0x48;
    pub const VOICE_GUIDANCE_NTFY_PARAM: u8 = 0x49;
    /// Inquired type: voice guidance volume.
    pub const VOICE_GUIDANCE_VOLUME: u8 = 0x20;
}

/// Second-byte "inquired type" values used by the requests we send.
pub mod inquired {
    pub const BATTERY_SINGLE: u8 = 0x00;
    pub const CODEC: u8 = 0x02;
    pub const PRESET_EQ_AND_ULT_MODE: u8 = 0x03;
    pub const AUTO_POWER_OFF: u8 = 0x05;
    pub const NC_ASM_DUAL_NC_SWITCH_AND_ASM_SEAMLESS: u8 = 0x17;
    pub const VOLUME: u8 = 0x20;
    /// COMMON status: DSEE indicator.
    pub const DSEE_INDICATOR: u8 = 0x03;
    /// AUDIO param: upscaling (DSEE) auto/off.
    pub const UPSCALING: u8 = 0x01;
    /// AUDIO param: LDAC connection mode.
    pub const CONNECTION_MODE_LDAC: u8 = 0x02;
    /// SYSTEM param: pause playback when headphones are removed.
    pub const PLAYBACK_CONTROL_BY_WEARING: u8 = 0x01;
    /// SYSTEM param: voice assistant selection.
    pub const VOICE_ASSISTANT: u8 = 0x04;
    /// SYSTEM ext param: quick access functions.
    pub const QUICK_ACCESS: u8 = 0x0D;
    /// PLAY status: playback status.
    pub const PLAYBACK_STATUS: u8 = 0x01;
    /// ALERT set param: multipoint commit sequence.
    pub const MULTIPOINT_COMMIT: [u8; 3] = [0x00, 0x06, 0x01];
    pub const POWER_OFF: u8 = 0x03;
    pub const POWER_OFF_USER: u8 = 0x01;
    /// General-setting value type: boolean.
    pub const GS_BOOLEAN: u8 = 0x00;
    /// General-setting value type: list.
    pub const GS_LIST: u8 = 0x01;
}

/// Support-function ids (T1 FunctionType), used as request capabilities.
pub mod function {
    pub const CODEC_INDICATOR: u8 = 0x12;
    pub const BATTERY_LEVEL_INDICATOR: u8 = 0x20;
    pub const POWER_OFF: u8 = 0x23;
    pub const AUTO_POWER_OFF: u8 = 0x25;
    pub const PRESET_EQ_AND_ULT_MODE: u8 = 0x53;
    pub const NC_ASM_DUAL_LEVEL_ADJUSTMENT: u8 = 0x6B;
    pub const PLAYBACK_CONTROLLER: u8 = 0xA1;
    pub const DSEE_INDICATOR: u8 = 0x13;
    pub const UPSCALING: u8 = 0xE2;
    pub const CONNECTION_MODE: u8 = 0xE3;
    pub const PLAYBACK_CONTROL_BY_WEARING: u8 = 0xF1;
    pub const VOICE_ASSISTANT: u8 = 0xF4;
    pub const QUICK_ACCESS: u8 = 0xFD;
    pub const MULTIPOINT: u8 = 0xD1;
    /// T2 FunctionType: voice guidance volume.
    pub const VOICE_GUIDANCE_VOLUME: u8 = 0x42;
}
