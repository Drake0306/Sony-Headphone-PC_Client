//! MDR v2 message layer: typed decode of frame payloads and request builders.
//! Knows nothing about framing; see `mdr-frame` for that.

mod deny;
pub mod function;
mod message;
pub mod opcode;
pub mod request;
mod types;

pub use deny::denied;
pub use message::{decode, Kind, Message};
pub use request::{Group, Request};
pub use types::*;
