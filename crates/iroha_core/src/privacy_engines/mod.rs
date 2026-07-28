//! Native, transparent privacy protocol engines.
//!
//! These modules contain closed first-release cryptographic profiles.  They do
//! not perform ledger admission or governance; callers must bind the exact
//! governed statement and parameter digests through [`p256::TranscriptBindingV1`].

pub mod anonymous_pgc;
pub mod bootle_lantern;
pub mod jindo;
pub(crate) mod orchard;
pub mod p256;
pub(crate) mod transparent_stark;
pub mod vega;
pub mod verange;
#[cfg(feature = "zk-stark")]
pub mod zk_ace;
#[cfg(feature = "zk-stark")]
pub(crate) mod zk_ace_stark;
pub mod zk_ams;
pub(crate) mod zk_x509;

use iroha_data_model::privacy::{
    PrivacyProofManagedPoolBootstrapV1, PrivacyProtocolIdV1, PrivacyRootV1,
};
use thiserror::Error;

/// Failure to derive a canonical root for one proof-managed pool bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ProofManagedPoolRootErrorV1 {
    /// The protocol-specific native accumulator has not passed its release gate.
    #[error("native accumulator for {protocol_id:?} is not compiled")]
    EngineUnavailable {
        /// Exact unavailable protocol.
        protocol_id: PrivacyProtocolIdV1,
    },
}

/// Derive the sole canonical epoch-one root for a typed pool bootstrap.
///
/// Each match arm remains fail-closed until its protocol-specific accumulator
/// implementation and adversarial suite are complete. This central boundary is
/// shared by governance admission and snapshot restore, so neither path can
/// accept a caller-selected or unrecomputable origin.
pub(crate) fn proof_managed_pool_initial_root_v1(
    bootstrap: &PrivacyProofManagedPoolBootstrapV1,
) -> Result<PrivacyRootV1, ProofManagedPoolRootErrorV1> {
    Err(ProofManagedPoolRootErrorV1::EngineUnavailable {
        protocol_id: bootstrap.protocol_id(),
    })
}
