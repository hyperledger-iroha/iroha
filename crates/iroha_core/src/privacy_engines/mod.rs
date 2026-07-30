//! Native, transparent privacy protocol engines.
//!
//! These modules contain closed first-release cryptographic profiles.  They do
//! not perform ledger admission or governance; callers must bind the exact
//! governed statement and parameter digests through [`p256::TranscriptBindingV1`].

pub(crate) mod aggregate_stark;
pub mod anonymous_pgc;
pub mod bootle_lantern;
pub mod fcmp_plus_plus;
pub mod ivm_private_note;
pub mod jindo;
pub mod orchard;
pub mod p256;
pub mod pq_masp;
pub(crate) mod proof_managed_accumulator;
pub(crate) mod proof_managed_note_stark;
pub(crate) mod prover_randomness;
pub(crate) mod transparent_stark;
pub mod vega;
pub mod verange;
pub(crate) mod x25519_wallet;
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

use self::fcmp_plus_plus::{FcmpNativeErrorV1, FcmpOutputTupleV1};
use self::proof_managed_accumulator::{
    ProofManagedAccumulatorErrorV1, build_proof_managed_frontier_v1,
};

/// Failure to derive a canonical root for one proof-managed pool bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ProofManagedPoolRootErrorV1 {
    /// A native note accumulator rejected the typed genesis set.
    #[error("native accumulator for {protocol_id:?} rejected its genesis set: {source}")]
    InvalidGenesis {
        /// Exact note protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Structural accumulator failure.
        source: ProofManagedAccumulatorErrorV1,
    },
    /// The native FCMP++ curve tree rejected the complete genesis output set.
    #[error("native FCMP++ accumulator rejected its genesis set: {source}")]
    InvalidFcmpGenesis {
        /// Structural or curve-tree failure.
        source: FcmpNativeErrorV1,
    },
}

/// Derive the sole canonical epoch-one root for a typed pool bootstrap.
///
/// This central boundary is shared by governance admission and snapshot
/// restore. Every match arm invokes its protocol-native accumulator, so neither
/// path can accept a caller-selected or unrecomputable origin.
pub(crate) fn proof_managed_pool_initial_root_v1(
    bootstrap: &PrivacyProofManagedPoolBootstrapV1,
) -> Result<PrivacyRootV1, ProofManagedPoolRootErrorV1> {
    let protocol_id = bootstrap.protocol_id();
    match bootstrap {
        PrivacyProofManagedPoolBootstrapV1::IrohaIvmPrivateNoteStarkV1(note_bootstrap) => {
            build_proof_managed_frontier_v1(
                bootstrap.namespace(),
                &note_bootstrap.initial_note_commitments,
            )
            .map(|frontier| frontier.root)
            .map_err(|source| ProofManagedPoolRootErrorV1::InvalidGenesis {
                protocol_id,
                source,
            })
        }
        PrivacyProofManagedPoolBootstrapV1::PqMaspStarkV0(note_bootstrap) => {
            build_proof_managed_frontier_v1(
                bootstrap.namespace(),
                &note_bootstrap.initial_note_commitments,
            )
            .map(|frontier| frontier.root)
            .map_err(|source| ProofManagedPoolRootErrorV1::InvalidGenesis {
                protocol_id,
                source,
            })
        }
        PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(bootstrap) => {
            let outputs = bootstrap
                .initial_outputs
                .iter()
                .copied()
                .map(|output| {
                    FcmpOutputTupleV1::new(
                        output.output_key,
                        output.linking_tag_generator,
                        output.amount_commitment,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| ProofManagedPoolRootErrorV1::InvalidFcmpGenesis { source })?;
            let frontier = fcmp_plus_plus::build_fcmp_frontier_v1(&outputs)
                .map_err(|source| ProofManagedPoolRootErrorV1::InvalidFcmpGenesis { source })?;
            Ok(iroha_data_model::privacy::PrivacyFcmpTreeRootV1 {
                layers: frontier.root.layers(),
                point: frontier.root.point(),
            }
            .history_commitment())
        }
    }
}
