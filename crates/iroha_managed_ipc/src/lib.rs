//! Owner-authenticated local control transport and bound listener handoff.
//!
//! A managed localnet keeps its loopback ports bound from reservation to validator startup: the
//! frontend hands its original sockets to its native worker, and the worker hands two of them to
//! each validator it starts. Validators link this crate to adopt their sockets, so it owns only
//! that handoff, the transport under it and the control envelope both carry. Managed lifecycle,
//! generated configuration and stop budgets belong to `iroha_deploy`.

use norito::json::{JsonDeserialize, JsonSerialize};
use std::time::Duration;

mod listeners;
pub mod transport;

pub use listeners::{
    FrontendListeners, PEER_COUNT, RuntimeListeners, WorkerListeners, adopt_runtime_listeners,
    validate_addresses,
};

/// Result of one managed transport or listener handoff operation.
pub type Result<T> = std::result::Result<T, Error>;

/// A managed transport or listener handoff operation that could not safely complete.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A filesystem, socket or process operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Stored metadata, a peer's frame or caller input is invalid.
    #[error("{0}")]
    Invalid(String),
    /// The original native I/O budget elapsed; retained custody remains unresolved.
    #[error("native operation I/O deadline elapsed; retain original journals")]
    NativeDeadline,
}

/// One authenticated request on a managed control or listener handoff endpoint.
#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ControlRequest {
    /// Secret session token the endpoint's owner published privately.
    pub token: String,
    /// The requested operation.
    pub action: ControlAction,
}

/// Closed worker control actions. An unknown kind fails decoding and receives no reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ControlAction {
    /// Adopt bound sockets through the separate managed startup endpoint.
    ListenerAdoption,
    /// Report the live worker status.
    Status,
    /// Stop every owned validator and end the worker.
    Down,
    /// Close a startup whose foreground deadline expired, retaining its last safe phase.
    StartupExpired,
    /// Start parent attachment work for a ready private root.
    AttachmentStart,
    /// Report parent attachment progress together with the local status.
    AttachmentStatus,
}

const MAX_METADATA: usize = 1024 * 1024;
const POLL: Duration = Duration::from_millis(50);

fn encode(value: &impl JsonSerialize) -> Result<Vec<u8>> {
    norito::json::to_vec(value).map_err(|_| Error::Invalid("cannot encode managed metadata".into()))
}

fn decode<T: JsonDeserialize>(bytes: &[u8]) -> Result<T> {
    norito::json::from_slice(bytes)
        .map_err(|_| Error::Invalid("managed control or context metadata is invalid".into()))
}

/// Create one fresh secret session token: 32 random bytes in lowercase hexadecimal.
#[must_use]
pub fn random_token() -> String {
    // Erase this owned random-byte buffer after constructing the returned token.
    let bytes = zeroize::Zeroizing::new(rand::random::<[u8; 32]>());
    encode_token(&bytes)
}

// The control protocol uses exactly 32 bytes; reserve the final 64-byte string once.
fn encode_token(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut token = String::with_capacity(64);
    for &byte in bytes {
        token.push(char::from(DIGITS[usize::from(byte >> 4)]));
        token.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    token
}

/// Compare two session tokens without stopping at the first difference.
///
/// Anything other than two exact 64-byte tokens never matches, so an empty record cannot.
#[must_use]
pub fn same_token(left: &str, right: &str) -> bool {
    left.len() == 64
        && right.len() == 64
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// Read what remains of a same-boot native continuous deadline, or `None` once it has passed.
///
/// # Errors
/// The native clock is unavailable, or the remainder does not fit a [`Duration`].
pub fn continuous_remaining(deadline: u128) -> std::io::Result<Option<Duration>> {
    let now = iroha_primitives::time::native_continuous_clock_nanos()?;
    let Some(nanos) = deadline.checked_sub(now).filter(|value| *value > 0) else {
        return Ok(None);
    };
    let seconds = u64::try_from(nanos / 1_000_000_000)
        .map_err(|_| std::io::Error::other("startup continuous remainder overflow"))?;
    Ok(Some(Duration::new(seconds, (nanos % 1_000_000_000) as u32)))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn native_test_guard() -> std::sync::MutexGuard<'static, ()> {
    // Serialize resource tests to keep parallel socket and private-directory ancestor custody
    // within macOS's default 256-FD budget.
    static NATIVE_RESOURCES: std::sync::Mutex<()> = std::sync::Mutex::new(());
    NATIVE_RESOURCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
