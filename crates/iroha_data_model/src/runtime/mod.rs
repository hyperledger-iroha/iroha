//! Live runtime upgrades (`specs/live_runtime_upgrades.md`).
//!
//! [`crate::runtime::definition`] holds the immutable definitions and the root selections built from them;
//! [`crate::runtime::lifecycle`] holds change sets, validator support attestations, obligations, emergency
//! containment, adoption anchors and the stable rejections of the mechanism.
pub mod definition;
pub mod lifecycle;
