//! Live runtime upgrades (`specs/live_runtime_upgrades.md`).
//!
//! [`definition`] holds the immutable definitions and the root selections built from them;
//! [`lifecycle`] holds change sets, validator support attestations, obligations, emergency
//! containment, adoption anchors and the stable rejections of the mechanism.
pub mod definition;
pub mod lifecycle;
