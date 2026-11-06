//! Sans-IO MDR session: ARQ, handshake, capability gating and device state.
//! No I/O, threads or timers; the caller feeds bytes and the clock.

mod arq;
mod handshake;
mod session;
mod state;

pub use arq::{ACK_RETRIES, ACK_TIMEOUT_MS};
pub use session::{Event, Session, SessionError, REPLY_TIMEOUT_MS, VOLUME_MAX};
pub use state::{
    DeviceState, GsSetting, GsValue, PairedDevice, Status, Tracked, GS_MULTIPOINT, GS_SIDETONE,
    GS_TOUCH_PANEL,
};
