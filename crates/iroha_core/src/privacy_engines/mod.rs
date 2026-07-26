//! Native, transparent privacy protocol engines.
//!
//! These modules contain closed first-release cryptographic profiles.  They do
//! not perform ledger admission or governance; callers must bind the exact
//! governed statement and parameter digests through [`p256::TranscriptBindingV1`].

pub mod anonymous_pgc;
pub mod bootle_lantern;
pub mod jindo;
pub mod p256;
pub mod vega;
pub mod verange;
pub mod zk_ams;
