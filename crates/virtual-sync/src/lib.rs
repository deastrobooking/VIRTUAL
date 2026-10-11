//! VIRTUAL Sync: show synchronization over the local network.
//!
//! A leader running VIRTUAL publishes its tempo and beat phase, parameter
//! changes, audio analysis and light zones sampled from the program image.
//! Followers are other VIRTUAL machines (a second room, a TV wall) or small
//! devices such as an ESP32, Arduino or Raspberry Pi driving lighting.
//!
//! [`protocol`] is the wire format and is pure; [`transport`] is a
//! non-blocking UDP implementation for the application. `clients/` has a
//! single-header C client and a Python client, and `docs/NETWORK_SYNC.md`
//! documents the format for anyone writing another.

pub mod clock;
pub mod interfaces;
pub mod mac;
pub mod protocol;
pub mod transport;
#[doc(hidden)]
pub mod vectors;
pub mod zones;

pub use mac::{Key, derive_key};
pub use protocol::{ControlValue, DEFAULT_PORT, Message, NodeKind, control_id, streams};
pub use transport::{Follower, FollowerInfo, Leader, LeaderInfo, Received};
