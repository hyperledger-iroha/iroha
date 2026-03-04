//! Shared retry helpers for Connect transports.
//!
//! This module exposes deterministic back-off policies so that every Connect
//! client (Rust, Swift, Android, JavaScript) can follow the same exponential
//! strategy with full jitter. The algorithm intentionally avoids RNG state so
//! callers can derive retry delays solely from the session identifier and the
//! attempt counter, guaranteeing parity across SDKs.

/// Deterministic exponential back-off policy with full jitter sampling.
pub mod policy;
