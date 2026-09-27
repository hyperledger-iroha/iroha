//! TON liteserver access over ADNL-TCP (spec §7.2, §8).
//!
//! `adnl` owns the ADNL handshake and framing; `liteclient` issues the
//! `liteServer.*` queries used for evidence and submissions.

pub mod adnl;
pub mod liteclient;

// TODO(ws25): pick liteservers from the configured global-config peers with failover.
