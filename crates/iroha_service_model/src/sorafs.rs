//! Shared SoraFS protocol policy defaults used by gateways and clients.

/// Alias cache positive TTL (seconds) applied by Torii gateways and SDK helpers.
pub const DEFAULT_ALIAS_POSITIVE_TTL_SECS: u64 = 10 * 60;
/// Alias cache refresh window (seconds) before positive TTL elapses.
pub const DEFAULT_ALIAS_REFRESH_WINDOW_SECS: u64 = 2 * 60;
/// Hard expiry (seconds) after which stale alias proofs are rejected even if refresh failed.
pub const DEFAULT_ALIAS_HARD_EXPIRY_SECS: u64 = 15 * 60;
/// Alias cache negative TTL (seconds) for missing aliases.
pub const DEFAULT_ALIAS_NEGATIVE_TTL_SECS: u64 = 60;
/// Alias cache TTL (seconds) for revoked aliases (`410 Gone` responses).
pub const DEFAULT_ALIAS_REVOCATION_TTL_SECS: u64 = 5 * 60;
/// Maximum tolerated age (seconds) for alias proof bundles before rotation is required.
pub const DEFAULT_ALIAS_ROTATION_MAX_AGE_SECS: u64 = 6 * 60 * 60;
/// Grace period (seconds) applied after an approved successor before refusing predecessor proofs.
pub const DEFAULT_ALIAS_SUCCESSOR_GRACE_SECS: u64 = 5 * 60;
/// Grace period (seconds) applied to governance rotation events.
pub const DEFAULT_ALIAS_GOVERNANCE_GRACE_SECS: u64 = 0;
