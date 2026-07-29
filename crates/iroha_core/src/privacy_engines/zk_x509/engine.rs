//! Fail-closed native zk-X509 engine boundary.
//!
//! The prover preparation path is complete and authoritative: it validates
//! persisted governance/root state against trusted block time, decodes the
//! sole canonical private-witness grammar, and executes the strict reference
//! relation.  This makes the codec and relation reachable production code
//! rather than disconnected laboratory helpers.
//!
//! Proof construction and verification are intentionally not exposed here
//! while [`ZK_X509_AIR_GAPS_V1`] is non-empty.  Returning a witness digest,
//! embedding private DER in a proof, or accepting a native reference check as
//! a proof would destroy zero knowledge or soundness.  Consensus therefore
//! remains unavailable until the segmented AIR and exact Merkle/FRI wire close
//! every enumerated gap.

use iroha_data_model::privacy::{IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1};
use thiserror::Error;

use super::{
    air::{ZK_X509_AIR_GAP_DESCRIPTOR_V1, ZK_X509_AIR_GAPS_V1, ZkX509AirGapV1},
    codec::{ZkX509WitnessCodecErrorV1, ZkX509WitnessV1},
    merkle::hash_frame_v1,
    profile::{
        ZK_X509_CRL_PROFILE_V1, ZK_X509_CRL_REVISION_SCHEMA_V1, ZK_X509_CRL_SCOPE_PROFILE_V1,
        ZK_X509_ECDSA_RULES_V1, ZK_X509_PROVISIONAL_STARK_PROFILE_DESCRIPTOR_V1,
        ZK_X509_RFC5280_PROFILE_V1, ZK_X509_SOURCE_PROFILE_V1, ZK_X509_SUITE_V1,
    },
    relation::{
        ZkX509GovernanceV1, ZkX509RelationErrorV1, ZkX509RelationOutputV1,
        validate_reference_relation_v1,
    },
};
use crate::privacy_state::{
    PrivacyZkX509AuthoritativeStateV1, validate_privacy_zk_x509_statement_state_v1,
};

const COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.provisional-compiled-profile.v1";
const REFERENCE_PREPARATION_SCHEMA_V1: &[u8] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-IRX509W1-private-witness+strict-reference-relation";

/// Frozen digest of the exact currently compiled, fail-closed native profile.
///
/// This is provisional and cannot be registered for activation.  It pins the
/// reference implementation and gap inventory so subsequent AIR work cannot
/// silently change an already-reviewed predicate.
pub(crate) const ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x40, 0xe4, 0x36, 0xc5, 0x0f, 0x07, 0xd3, 0x50, 0xee, 0x68, 0x66, 0xc2, 0x79, 0x8c, 0x1d, 0x8c,
    0xd6, 0x92, 0xb8, 0x9f, 0x88, 0xfa, 0x06, 0x77, 0x93, 0xdd, 0x9e, 0xbb, 0x98, 0xd4, 0x25, 0x1e,
];

/// Canonically decoded and reference-validated private prover input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedZkX509ProverInputV1 {
    witness: ZkX509WitnessV1,
    projection: ZkX509RelationOutputV1,
}

impl PreparedZkX509ProverInputV1 {
    /// Borrow the sole canonical witness representation.
    pub(crate) const fn witness(&self) -> &ZkX509WitnessV1 {
        &self.witness
    }

    /// Deterministic public projection recomputed from the private relation.
    pub(crate) const fn projection(&self) -> ZkX509RelationOutputV1 {
        self.projection
    }
}

/// Native preparation or release-gate failure.
#[derive(Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509EngineErrorV1 {
    /// Persisted state, roots, epochs, policy, or trusted block time mismatch.
    #[error("zk-X509 authoritative state validation failed: {0}")]
    InvalidAuthoritativeState(String),
    /// Private witness bytes are not the sole exact bounded grammar.
    #[error(transparent)]
    WitnessCodec(#[from] ZkX509WitnessCodecErrorV1),
    /// Decode followed by encode did not reproduce the exact input bytes.
    #[error("zk-X509 witness codec failed its exact round-trip invariant")]
    WitnessRoundTripMismatch,
    /// Strict RFC 5280/reference relation failure.
    #[error(transparent)]
    ReferenceRelation(#[from] ZkX509RelationErrorV1),
    /// The constrained AIR/proof implementation is not complete.
    #[error("zk-X509 segmented AIR remains incomplete")]
    AirIncomplete,
}

/// Decode and validate the exact prover input against trusted ledger state.
///
/// This function performs no proof construction and does not weaken the
/// activation gate.  Its output is the only admitted input to the future
/// segmented witness generator.
pub(crate) fn prepare_zk_x509_prover_input_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
    encoded_witness: &[u8],
) -> Result<PreparedZkX509ProverInputV1, ZkX509EngineErrorV1> {
    validate_privacy_zk_x509_statement_state_v1(
        statement,
        authoritative_state,
        trusted_block_timestamp_ms,
        consensus_limits,
    )
    .map_err(ZkX509EngineErrorV1::InvalidAuthoritativeState)?;

    let witness = ZkX509WitnessV1::decode_exact_v1(encoded_witness)?;
    // Decode is already exact.  Re-encoding here is a deliberate differential
    // invariant for the eventual external prover boundary.
    if witness.encode_v1()? != encoded_witness {
        return Err(ZkX509EngineErrorV1::WitnessRoundTripMismatch);
    }

    let trust_anchor = authoritative_state.trust_anchor();
    let crl = authoritative_state.crl_record();
    let governance = ZkX509GovernanceV1 {
        trust_anchor: &trust_anchor,
        certificate_policy: authoritative_state.certificate_policy(),
        crl: &crl,
    };
    let projection = validate_reference_relation_v1(statement, governance, &witness)?;
    Ok(PreparedZkX509ProverInputV1 {
        witness,
        projection,
    })
}

/// Exact remaining constrained AIR components.
pub(crate) const fn zk_x509_air_gaps_v1() -> &'static [ZkX509AirGapV1] {
    &ZK_X509_AIR_GAPS_V1
}

/// Enforce that no caller can enter proof construction while an AIR gap
/// remains.
pub(crate) fn require_complete_zk_x509_air_v1() -> Result<(), ZkX509EngineErrorV1> {
    if ZK_X509_AIR_GAPS_V1.is_empty() {
        Ok(())
    } else {
        Err(ZkX509EngineErrorV1::AirIncomplete)
    }
}

/// Recompute the exact provisional compiled-profile digest.
pub(crate) fn recompute_zk_x509_provisional_compiled_profile_digest_v1() -> [u8; 32] {
    hash_frame_v1(
        COMPILED_PROFILE_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            ZK_X509_RFC5280_PROFILE_V1,
            ZK_X509_CRL_PROFILE_V1,
            ZK_X509_CRL_SCOPE_PROFILE_V1,
            ZK_X509_CRL_REVISION_SCHEMA_V1,
            ZK_X509_ECDSA_RULES_V1,
            ZK_X509_PROVISIONAL_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_AIR_GAP_DESCRIPTOR_V1,
            REFERENCE_PREPARATION_SCHEMA_V1,
        ],
    )
    .expect("fixed compiled-profile fields are representable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisional_profile_and_gap_inventory_are_pinned_fail_closed() {
        assert_eq!(
            recompute_zk_x509_provisional_compiled_profile_digest_v1(),
            ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1
        );
        assert_eq!(zk_x509_air_gaps_v1(), ZK_X509_AIR_GAPS_V1);
        assert_eq!(
            require_complete_zk_x509_air_v1(),
            Err(ZkX509EngineErrorV1::AirIncomplete)
        );
    }
}
