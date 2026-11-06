use crate::opcode::t1;
use crate::types::Table;

/// Sub-id values that make an otherwise-allowed opcode dangerous.
const SYSTEM_RESET_SETTINGS: u8 = 0x09;
const ALERT_REBOOT: u8 = 0x16;

/// Returns the reason a payload must never be sent, or `None` if it is allowed.
/// `0x24` (power off) is listed here; only `Session::request_confirmed` lifts it.
pub fn denied(table: Table, bytes: &[u8]) -> Option<&'static str> {
    if table != Table::T1 {
        return None;
    }
    let op = *bytes.first()?;
    match op {
        t1::SYSTEM_SET_PARAM if bytes.get(1) == Some(&SYSTEM_RESET_SETTINGS) => Some("factory reset"),
        t1::ALERT_SET_PARAM if bytes.get(2) == Some(&ALERT_REBOOT) => Some("reboot"),
        t1::POWER_SET_STATUS => Some("power off (needs confirmation)"),
        t1::LOG_SET_STATUS => Some("log dump"),
        t1::UPDT_EXTRA | t1::UPDT_FIRST..=t1::UPDT_LAST => Some("firmware update"),
        _ => None,
    }
}
