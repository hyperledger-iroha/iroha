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
// The native prover and its RFC 5280 reference machinery are consumed by the
// internal test harness and the opt-in release-evidence runner. Normal node
// builds retain the verifier but cannot observe those crate-private roots.
#[cfg_attr(not(any(test, feature = "privacy-release-evidence")), allow(dead_code))]
pub(crate) mod zk_x509;

use iroha_data_model::privacy::{
    IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1, PrivacyProofManagedPoolBootstrapV1,
    PrivacyProtocolIdV1, PrivacyRootV1, PrivacyStatementV1,
};
use thiserror::Error;

use self::fcmp_plus_plus::{FcmpNativeErrorV1, FcmpOutputTupleV1};
use self::proof_managed_accumulator::{
    ProofManagedAccumulatorErrorV1, build_proof_managed_frontier_v1,
};
use self::zk_x509::credential_stark::{
    ZkX509CredentialProofErrorV1, ZkX509CredentialPublicBindingV1,
    decode_zk_x509_credential_envelope_v1,
};

/// Exact maximum byte length of one canonical first-release `X5S1` proof.
pub const ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1: usize =
    self::zk_x509::profile::ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 as usize;

/// Structural or public-binding failure for one externally produced `X5S1` proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ZkX509CredentialProofContainerErrorV1 {
    /// The typed public statement or committed genesis hash is invalid.
    #[error("zk-X509 credential statement or genesis binding is invalid")]
    InvalidStatement,
    /// The proof exceeds the exact first-release `X5S1` ceiling.
    #[error("zk-X509 credential proof exceeds its exact byte ceiling")]
    ProofTooLarge,
    /// The proof is not the sole canonical two-record `X5S1` container.
    #[error("zk-X509 credential proof container is malformed")]
    MalformedContainer,
    /// The `X5S1` header does not bind the exact statement and genesis hash.
    #[error("zk-X509 credential proof public binding does not match")]
    PublicBindingMismatch,
}

/// Validate the fixed-capacity `X5S1` container and its verifier-owned header.
///
/// This boundary is intended for resource-isolated prover workers. It performs
/// no ledger-state lookup and does not replace full proof verification; it
/// proves that the returned bytes are the sole first-release container and
/// that they bind the exact typed statement and committed genesis hash before
/// an SDK signs a submission transaction.
pub fn validate_zk_x509_credential_proof_container_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    canonical_genesis_hash: [u8; 32],
    encoded_proof: &[u8],
) -> Result<(), ZkX509CredentialProofContainerErrorV1> {
    PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone())
        .validate(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| ZkX509CredentialProofContainerErrorV1::InvalidStatement)?;
    let expected = ZkX509CredentialPublicBindingV1::from_consensus_context_v1(
        statement,
        canonical_genesis_hash,
    )
    .map_err(|_| ZkX509CredentialProofContainerErrorV1::InvalidStatement)?;
    let decoded =
        decode_zk_x509_credential_envelope_v1(encoded_proof).map_err(|error| match error {
            ZkX509CredentialProofErrorV1::ProofTooLarge => {
                ZkX509CredentialProofContainerErrorV1::ProofTooLarge
            }
            ZkX509CredentialProofErrorV1::MalformedEnvelope => {
                ZkX509CredentialProofContainerErrorV1::MalformedContainer
            }
            ZkX509CredentialProofErrorV1::InvalidStatement => {
                ZkX509CredentialProofContainerErrorV1::InvalidStatement
            }
            ZkX509CredentialProofErrorV1::PublicBindingMismatch => {
                ZkX509CredentialProofContainerErrorV1::PublicBindingMismatch
            }
            ZkX509CredentialProofErrorV1::MainProof
            | ZkX509CredentialProofErrorV1::CaProof
            | ZkX509CredentialProofErrorV1::CrossSubproofMismatch => {
                ZkX509CredentialProofContainerErrorV1::MalformedContainer
            }
        })?;
    if decoded.public != expected {
        return Err(ZkX509CredentialProofContainerErrorV1::PublicBindingMismatch);
    }
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::credential_stark::{
        ZkX509CredentialPublicBindingV1, encode_zk_x509_credential_envelope_v1,
    };

    fn fixture() -> (IrohaZkX509StarkP256StatementV1, [u8; 32], Vec<u8>) {
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let genesis = [0xA5; 32];
        let public =
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis)
                .expect("fixture public binding");
        let proof = encode_zk_x509_credential_envelope_v1(public, b"X5M1", b"X5C1")
            .expect("minimum canonical X5S1");
        (statement, genesis, proof)
    }

    #[test]
    fn x509_worker_container_boundary_accepts_only_exact_bound_x5s1() {
        let (statement, genesis, proof) = fixture();
        validate_zk_x509_credential_proof_container_v1(&statement, genesis, &proof)
            .expect("canonical fixed-capacity container");

        assert_eq!(
            validate_zk_x509_credential_proof_container_v1(&statement, [0; 32], &proof),
            Err(ZkX509CredentialProofContainerErrorV1::InvalidStatement)
        );
        let mut invalid_statement = statement.clone();
        invalid_statement.context.transaction_intent_digest =
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0; 32]);
        assert_eq!(
            validate_zk_x509_credential_proof_container_v1(&invalid_statement, genesis, &proof,),
            Err(ZkX509CredentialProofContainerErrorV1::InvalidStatement)
        );
        assert_eq!(
            validate_zk_x509_credential_proof_container_v1(&statement, [0xA6; 32], &proof),
            Err(ZkX509CredentialProofContainerErrorV1::PublicBindingMismatch)
        );
        for length in 0..proof.len() {
            assert_eq!(
                validate_zk_x509_credential_proof_container_v1(
                    &statement,
                    genesis,
                    &proof[..length],
                ),
                Err(ZkX509CredentialProofContainerErrorV1::MalformedContainer),
                "truncation at byte {length} was accepted"
            );
        }
        let mut trailing = proof.clone();
        trailing.push(0);
        assert_eq!(
            validate_zk_x509_credential_proof_container_v1(&statement, genesis, &trailing),
            Err(ZkX509CredentialProofContainerErrorV1::MalformedContainer)
        );
        for offset in [8, 40, 75] {
            let mut substituted_public_binding = proof.clone();
            substituted_public_binding[offset] ^= 1;
            assert_eq!(
                validate_zk_x509_credential_proof_container_v1(
                    &statement,
                    genesis,
                    &substituted_public_binding,
                ),
                Err(ZkX509CredentialProofContainerErrorV1::PublicBindingMismatch),
                "public header substitution at byte {offset} was accepted"
            );
        }
        for (offset, value) in [
            (5, 2),  // version
            (7, 0),  // record count
            (77, 2), // MAIN kind
            (79, 1), // MAIN instance
            (83, 0), // MAIN length
            (87, 2), // MAIN magic
            (89, 1), // CA kind
            (91, 1), // CA instance
            (95, 0), // CA length
            (99, 2), // CA magic
        ] {
            let mut malformed_field = proof.clone();
            malformed_field[offset] = value;
            assert_eq!(
                validate_zk_x509_credential_proof_container_v1(
                    &statement,
                    genesis,
                    &malformed_field,
                ),
                Err(ZkX509CredentialProofContainerErrorV1::MalformedContainer),
                "framing substitution at byte {offset} was accepted"
            );
        }
        for hostile in [Vec::new(), vec![0; 4]] {
            assert_eq!(
                validate_zk_x509_credential_proof_container_v1(&statement, genesis, &hostile,),
                Err(ZkX509CredentialProofContainerErrorV1::MalformedContainer)
            );
        }
        let oversized = vec![0xA5; ZK_X509_CREDENTIAL_PROOF_MAX_BYTES_V1 + 1];
        assert_eq!(
            validate_zk_x509_credential_proof_container_v1(&statement, genesis, &oversized,),
            Err(ZkX509CredentialProofContainerErrorV1::ProofTooLarge)
        );
    }
}
