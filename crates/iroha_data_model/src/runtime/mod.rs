//! Live runtime upgrades (`specs/live_runtime_upgrades.md`).
//!
//! [`definition`] holds the immutable definitions and the root selections built from them;
//! [`lifecycle`] holds change sets, validator support attestations, emergency containment,
//! adoption anchors and the stable rejections of the mechanism; [`obligation`] holds the
//! obligations registered through the kernel, their completion capabilities and migration
//! cursors; [`checkpoint`] holds the startup floor and the policy records a selection carries.
pub mod adoption;
pub mod checkpoint;
pub mod deferred;
pub mod definition;
pub mod keyed_state;
pub mod lifecycle;
/// Kernel-owned captures of native AMX decisions and settlement.
pub mod native_amx;
/// Kernel-owned captures of native escrow custody.
pub mod native_escrow;
/// Destination-reserved native game transfers and original completion custody.
pub mod native_game;
pub mod obligation;
/// Certified retirement statements and bounded ordinary-write inclusion checks.
pub mod retirement;
/// Explicit one-time SoraFS successor authorization and certificate.
pub mod sorafs_repair;
pub mod taira_activation;

/// Native implementation identity for complete compact trigger lifecycle capture.
pub const TRIGGER_LIFECYCLE_MODULE: &str = "iroha.trigger-lifecycle.v1";
/// Immutable service reference closure for compact evidence and reserved failure policy.
pub const TRIGGER_LIFECYCLE_SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:trigger-lifecycle:v1:cached-executables-block-work-and-native-progress";
