//! State-free Parliament timed-OVN evidence, casting archives and public TLE verification.
//!
//! Core retains committed state readers, authenticated authorization capabilities
//! and release-share custody. Decoding public evidence never grants authority.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

pub mod casting;
pub mod evidence;
#[cfg(test)]
mod frame_test_support;
pub mod tle;
