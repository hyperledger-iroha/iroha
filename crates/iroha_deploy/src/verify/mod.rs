//! Deployment verification gates over authenticated native protocol evidence.
//!
//! Gates check what the protocol already proves (certificates, attestations
//! and genesis identity); they never re-sign or re-implement it. Gate G5
//! (`specs/network_deployment.md` §9) is [`finality`]; the other gates come in
//! P2.

/// Bounded light finality verification and challenge-bound committee observations.
pub mod finality;

/// Deadline-bound native SDK transport for independently anchored finality observations.
pub mod http;
