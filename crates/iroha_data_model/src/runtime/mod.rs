//! Live runtime upgrades (`specs/live_runtime_upgrades.md`).
//!
//! [`definition`] holds the immutable definitions and the root selections built from them;
//! [`lifecycle`] holds change sets, validator support attestations, obligations, emergency
//! containment, adoption anchors and the stable rejections of the mechanism.
pub mod adoption;
pub mod definition;
pub mod lifecycle;
/// Explicit one-time SoraFS successor authorization and certificate.
pub mod sorafs_repair;
pub mod taira_activation;

/// Native implementation identity for complete compact trigger lifecycle capture.
pub const TRIGGER_LIFECYCLE_MODULE: &str = "iroha.trigger-lifecycle.v1";
/// Immutable service reference closure for compact evidence and reserved failure policy.
pub const TRIGGER_LIFECYCLE_SERVICE_CLOSURE: &[u8] =
    b"iroha:runtime:trigger-lifecycle:v1:cached-executables-block-work-and-native-progress";
