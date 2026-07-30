//! Fail-closed native zk-X509 engine boundary.
//!
//! The prover preparation path is complete and authoritative: it validates
//! persisted governance/root state against trusted block time, decodes the
//! sole canonical private-witness grammar, and executes the strict reference
//! relation.  This makes the codec and relation reachable production code
//! rather than disconnected laboratory helpers.
//!
//! The constrained subproof machinery is not exposed as a credential proof
//! while the explicit end-to-end gap inventory is non-empty. A native
//! reference check, projection-only proof, or collection of unbound subproofs
//! is never accepted as a credential proof.

use iroha_data_model::privacy::{IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1};
use thiserror::Error;

use super::{
    air::{ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1, ZK_X509_AIR_GAPS_V1, ZkX509AirGapV1},
    codec::{ZkX509WitnessCodecErrorV1, ZkX509WitnessV1},
    credential_pre_aux::ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1,
    credential_stark::{
        ZkX509CredentialProofErrorV1, ZkX509CredentialPublicBindingV1,
        decode_zk_x509_credential_envelope_v1,
    },
    der_air::ZkX509Rfc5280StatementV1,
    fixed_algebraic::ZK_X509_FIXED_ALGEBRAIC_DESCRIPTOR_V1,
    fixed_algebraic_p256::{
        ZK_X509_P256_FIXED_ALGEBRAIC_DESCRIPTOR_V1, ZkX509P256FixedAlgebraicErrorV1,
        zk_x509_p256_fixed_algebraic_schedule_v1,
    },
    fixed_algebraic_sha::{
        ZK_X509_SHA_FIXED_ALGEBRAIC_COMPILER_DESCRIPTOR_V1, ZkX509ShaFixedAlgebraicErrorV1,
        zk_x509_sha_fixed_algebraic_schedule_v1,
    },
    io_air::ZK_X509_IO_AIR_DESCRIPTOR_V1,
    main_assembly::{
        ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1,
        compile_zk_x509_rfc_statement_from_authoritative_state_v1,
    },
    main_io::ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
    merkle::hash_frame_v1,
    profile::{
        ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1, ZK_X509_CRL_PROFILE_V1,
        ZK_X509_CRL_REVISION_SCHEMA_V1, ZK_X509_CRL_SCOPE_PROFILE_V1, ZK_X509_ECDSA_RULES_V1,
        ZK_X509_RFC5280_PROFILE_V1, ZK_X509_SOURCE_PROFILE_V1, ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
        ZK_X509_SUITE_V1, ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1,
    },
    relation::{
        ZkX509GovernanceV1, ZkX509RelationErrorV1, ZkX509RelationOutputV1,
        validate_reference_relation_v1,
    },
    sha_call_bus_stark::{ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1, ZkX509ShaCallPublicShapeV1},
    sha256_air::ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
    sha256_word_air::ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
};
use crate::privacy_state::{
    PrivacyZkX509AuthoritativeStateV1, validate_privacy_zk_x509_statement_state_v1,
};

const COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.compiled-profile.v1";
const REFERENCE_PREPARATION_SCHEMA_V1: &[u8] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-IRX509W1-private-witness+strict-reference-relation";
const COMPILED_PROFILE_FIELD_COUNT_V1: usize = 28;
const SHA_DISCLOSURE_SHAPE_COUNT_V1: usize = 5;

// Filled only after all six algebraic schedules have independent release KATs.
// There is deliberately no provisional, root-bearing, or certificate-bearing
// profile in the first-release protocol.
const ZK_X509_COMPILED_PROFILE_DIGEST_V1: Option<[u8; 32]> = None;

/// Exact algebraic-schedule-bearing profile required by MAIN.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CompiledProfileV1 {
    digest: [u8; 32],
}

impl ZkX509CompiledProfileV1 {
    /// Consensus transcript digest of the complete release manifest.
    pub(crate) const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

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

/// Complete verifier-owned public input compiled before aggregate verification.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkX509ConsensusPublicInputsV1 {
    /// Canonical `X5S1` header binding derived from statement plus genesis.
    credential_binding: ZkX509CredentialPublicBindingV1,
    /// RFC predicates with the CRL number selected only from trusted state.
    rfc_statement: ZkX509Rfc5280StatementV1,
}

fn compile_zk_x509_consensus_public_inputs_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    genesis_hash: [u8; 32],
) -> Result<ZkX509ConsensusPublicInputsV1, ZkX509EngineErrorV1> {
    let credential_binding =
        ZkX509CredentialPublicBindingV1::from_consensus_context_v1(statement, genesis_hash)?;
    let rfc_statement =
        compile_zk_x509_rfc_statement_from_authoritative_state_v1(statement, authoritative_state);
    Ok(ZkX509ConsensusPublicInputsV1 {
        credential_binding,
        rfc_statement,
    })
}

/// Native prover preparation or release-gate failure.
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
    /// Canonical credential envelope or consensus-context binding failure.
    #[error(transparent)]
    CredentialProof(#[from] ZkX509CredentialProofErrorV1),
    /// One or more end-to-end constrained proof gaps remain.
    #[error("zk-X509 canonical credential proof remains incomplete")]
    AirIncomplete,
    /// The complete main-plus-CA aggregate verifier has not been assembled.
    #[error("zk-X509 consensus credential verifier is not assembled")]
    ConsensusVerifierUnavailable,
    /// The canonical SHA algebraic compiler or schedule rejected its profile.
    #[error(transparent)]
    ShaFixedAlgebraic(#[from] ZkX509ShaFixedAlgebraicErrorV1),
    /// The canonical P-256 algebraic compiler or schedule rejected its profile.
    #[error(transparent)]
    P256FixedAlgebraic(#[from] ZkX509P256FixedAlgebraicErrorV1),
    /// The complete 28-field release manifest has not been pinned.
    #[error("zk-X509 compiled profile is not release-pinned")]
    CompiledProfileUnpinned,
    /// Recomputed 28-field manifest digest differs from the consensus pin.
    #[error("zk-X509 compiled profile digest mismatch")]
    CompiledProfileMismatch,
}

/// Verify one canonical credential proof against verifier-owned consensus data.
///
/// This is the sole consensus entry point for `X5S1`. It already performs
/// strict envelope decoding and binds the complete typed statement to the
/// committed genesis hash before inspecting any aggregate. The caller must
/// supply the same authoritative snapshot it already validated against trusted
/// block time and consensus limits; the engine compiles the RFC public input
/// from that snapshot rather than from proof metadata. It deliberately cannot
/// succeed while either the explicit gap inventory is non-empty or the complete
/// main-plus-compact-CA verifier is unavailable.
pub(crate) fn verify_zk_x509_credential_proof_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    genesis_hash: [u8; 32],
    encoded_proof: &[u8],
) -> Result<(), ZkX509EngineErrorV1> {
    let consensus_public =
        compile_zk_x509_consensus_public_inputs_v1(statement, authoritative_state, genesis_hash)?;
    let envelope = decode_zk_x509_credential_envelope_v1(encoded_proof)?;
    if envelope.public != consensus_public.credential_binding {
        return Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch.into());
    }

    require_complete_zk_x509_air_v1()?;

    // TODO(zk-x509-release): invoke the completed MAIN aggregate verifier and
    // the real compact-CA verifier here under
    // `construct_zk_x509_compiled_profile_v1`, then compare every SHA-call
    // terminal and the root-SPKI I/O products through
    // `verify_zk_x509_credential_envelope_with_v1`. Never replace this with a
    // reference-relation check or an independently accepted CA proof.
    let _rfc_statement = consensus_public.rfc_statement;
    let _main_aggregate = envelope.main_aggregate;
    let _ca_subproof = envelope.ca_subproof;
    Err(ZkX509EngineErrorV1::ConsensusVerifierUnavailable)
}

/// Decode and validate the exact prover input against trusted ledger state.
///
/// This function performs no proof construction and does not weaken the
/// activation gate. Its output is the only admitted input to a future complete
/// credential-proof constructor.
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

/// Exact remaining constrained proof gaps.
pub(crate) const fn zk_x509_air_gaps_v1() -> &'static [ZkX509AirGapV1] {
    &ZK_X509_AIR_GAPS_V1
}

/// Reject proof construction until the canonical end-to-end path exists.
pub(crate) fn require_complete_zk_x509_air_v1() -> Result<(), ZkX509EngineErrorV1> {
    if ZK_X509_AIR_GAPS_V1.is_empty() {
        Ok(())
    } else {
        Err(ZkX509EngineErrorV1::AirIncomplete)
    }
}

fn compiled_profile_fields_v1<'a>(
    sha_schedule_digests: &'a [[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1],
    p256_schedule_digest: &'a [u8; 32],
) -> [&'a [u8]; COMPILED_PROFILE_FIELD_COUNT_V1] {
    [
        ZK_X509_SUITE_V1,
        ZK_X509_SOURCE_PROFILE_V1,
        ZK_X509_RFC5280_PROFILE_V1,
        ZK_X509_CRL_PROFILE_V1,
        ZK_X509_CRL_SCOPE_PROFILE_V1,
        ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1,
        ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1,
        ZK_X509_CRL_REVISION_SCHEMA_V1,
        ZK_X509_ECDSA_RULES_V1,
        ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
        ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1,
        ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
        ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1,
        ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1,
        ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
        ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
        ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1,
        ZK_X509_IO_AIR_DESCRIPTOR_V1,
        REFERENCE_PREPARATION_SCHEMA_V1,
        ZK_X509_FIXED_ALGEBRAIC_DESCRIPTOR_V1,
        ZK_X509_SHA_FIXED_ALGEBRAIC_COMPILER_DESCRIPTOR_V1,
        &sha_schedule_digests[0],
        &sha_schedule_digests[1],
        &sha_schedule_digests[2],
        &sha_schedule_digests[3],
        &sha_schedule_digests[4],
        ZK_X509_P256_FIXED_ALGEBRAIC_DESCRIPTOR_V1,
        p256_schedule_digest,
    ]
}

fn compiled_profile_schedule_digests_v1()
-> Result<([[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1], [u8; 32]), ZkX509EngineErrorV1> {
    let mut sha = [[0_u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1];
    for (disclosed_attributes, digest) in sha.iter_mut().enumerate() {
        *digest = zk_x509_sha_fixed_algebraic_schedule_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })?
        .descriptor_digest_v1();
    }
    let p256 = zk_x509_p256_fixed_algebraic_schedule_v1()?.descriptor_digest_v1();
    Ok((sha, p256))
}

/// Recompute the sole exact 28-field compiled-profile digest.
pub(crate) fn recompute_zk_x509_compiled_profile_digest_v1() -> Result<[u8; 32], ZkX509EngineErrorV1>
{
    let (sha, p256) = compiled_profile_schedule_digests_v1()?;
    hash_frame_v1(
        COMPILED_PROFILE_DIGEST_DOMAIN_V1,
        &compiled_profile_fields_v1(&sha, &p256),
    )
    .map_err(|_| ZkX509EngineErrorV1::CompiledProfileMismatch)
}

/// Construct the sole complete algebraic-schedule-bearing release profile.
///
/// All six success-only verifier schedule caches must compile before the
/// manifest digest is compared with its independent release pin.
pub(crate) fn construct_zk_x509_compiled_profile_v1()
-> Result<ZkX509CompiledProfileV1, ZkX509EngineErrorV1> {
    let digest = recompute_zk_x509_compiled_profile_digest_v1()?;
    let expected =
        ZK_X509_COMPILED_PROFILE_DIGEST_V1.ok_or(ZkX509EngineErrorV1::CompiledProfileUnpinned)?;
    if digest != expected {
        return Err(ZkX509EngineErrorV1::CompiledProfileMismatch);
    }
    Ok(ZkX509CompiledProfileV1 { digest })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::super::profile::ZK_X509_HASH_FRAME_DOMAIN_V1;
    use super::*;
    use crate::privacy_engines::zk_x509::credential_stark::encode_zk_x509_credential_envelope_v1;

    fn independently_encode_compiled_profile_frame_v1(fields: &[&[u8]]) -> Vec<u8> {
        let domain_len =
            u16::try_from(COMPILED_PROFILE_DIGEST_DOMAIN_V1.len()).expect("small domain");
        let field_count = u16::try_from(fields.len()).expect("small field manifest");
        let mut frame = Vec::new();
        frame.extend_from_slice(ZK_X509_HASH_FRAME_DOMAIN_V1);
        frame.extend_from_slice(&domain_len.to_be_bytes());
        frame.extend_from_slice(COMPILED_PROFILE_DIGEST_DOMAIN_V1);
        frame.extend_from_slice(&field_count.to_be_bytes());
        for field in fields {
            let field_len = u64::try_from(field.len()).expect("profile field fits u64");
            frame.extend_from_slice(&field_len.to_be_bytes());
            frame.extend_from_slice(field);
        }
        frame
    }

    fn independent_compiled_profile_digest_v1(fields: &[&[u8]]) -> [u8; 32] {
        Sha256::digest(independently_encode_compiled_profile_frame_v1(fields)).into()
    }

    #[test]
    fn compiled_profile_manifest_has_the_exact_28_field_order() {
        let sha_digests: [[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x31 + shape).expect("five shapes"); 32]);
        let p256_digest = [0x41; 32];
        let fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        let original_fields: [&[u8]; 19] = [
            ZK_X509_SUITE_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            ZK_X509_RFC5280_PROFILE_V1,
            ZK_X509_CRL_PROFILE_V1,
            ZK_X509_CRL_SCOPE_PROFILE_V1,
            ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1,
            ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1,
            ZK_X509_CRL_REVISION_SCHEMA_V1,
            ZK_X509_ECDSA_RULES_V1,
            ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
            ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1,
            ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
            ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1,
            ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1,
            ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
            ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
            ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1,
            ZK_X509_IO_AIR_DESCRIPTOR_V1,
            REFERENCE_PREPARATION_SCHEMA_V1,
        ];
        assert_eq!(fields.len(), 28);
        assert_eq!(&fields[..19], &original_fields);
        assert_eq!(fields[19], ZK_X509_FIXED_ALGEBRAIC_DESCRIPTOR_V1);
        assert_eq!(
            fields[20],
            ZK_X509_SHA_FIXED_ALGEBRAIC_COMPILER_DESCRIPTOR_V1
        );
        for (shape, digest) in sha_digests.iter().enumerate() {
            assert_eq!(fields[21 + shape], digest);
        }
        assert_eq!(fields[26], ZK_X509_P256_FIXED_ALGEBRAIC_DESCRIPTOR_V1);
        assert_eq!(fields[27], p256_digest);
        assert!(
            fields[12]
                .windows(b"post-base-challenges=exact272-goldilocks-fields".len())
                .any(|window| window == b"post-base-challenges=exact272-goldilocks-fields")
        );
        assert!(
            fields[16]
                .windows(b"main-common-lde-log25".len())
                .any(|window| window == b"main-common-lde-log25")
        );
        assert!(
            !fields[16]
                .windows(b"common-lde-log22".len())
                .any(|window| window == b"common-lde-log22")
        );
    }

    #[test]
    fn compiled_profile_binds_every_algebraic_field_and_its_order() {
        let sha_digests: [[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x51 + shape).expect("five shapes"); 32]);
        let p256_digest = [0x61; 32];
        let canonical_fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        let canonical = independent_compiled_profile_digest_v1(&canonical_fields);
        let owned = canonical_fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();

        for field in 19..COMPILED_PROFILE_FIELD_COUNT_V1 {
            let mut changed = owned.clone();
            changed[field][0] ^= 1;
            let changed_fields = changed.iter().map(Vec::as_slice).collect::<Vec<_>>();
            assert_ne!(
                independent_compiled_profile_digest_v1(&changed_fields),
                canonical,
                "manifest field {field} must be bound"
            );
        }

        let mut reordered = owned;
        reordered.swap(21, 22);
        let reordered_fields = reordered.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_ne!(
            independent_compiled_profile_digest_v1(&reordered_fields),
            canonical,
            "SHA disclosure-shape digest order must be bound"
        );
    }

    #[test]
    fn compiled_profile_constructor_is_closed_until_the_manifest_pin_exists() {
        assert_eq!(
            construct_zk_x509_compiled_profile_v1(),
            Err(ZkX509EngineErrorV1::CompiledProfileUnpinned)
        );
    }

    #[test]
    fn sole_profile_and_gap_manifest_are_fail_closed() {
        assert!(ZK_X509_COMPILED_PROFILE_DIGEST_V1.is_none());
        assert!(!zk_x509_air_gaps_v1().is_empty());
        assert_eq!(
            require_complete_zk_x509_air_v1(),
            Err(ZkX509EngineErrorV1::AirIncomplete)
        );
        assert!(
            String::from_utf8_lossy(ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1)
                .ends_with("activation=false")
        );
    }

    #[test]
    fn consensus_entry_point_decodes_and_binds_context_before_the_air_gate() {
        let (statement, authoritative_state) =
            crate::privacy_verifier::zk_x509_dispatch_fixture_for_test();
        let genesis_hash = [0x91; 32];
        let public =
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis_hash)
                .expect("canonical public binding");
        let consensus_public = compile_zk_x509_consensus_public_inputs_v1(
            &statement,
            &authoritative_state,
            genesis_hash,
        )
        .expect("verifier-owned consensus public input");
        assert_eq!(consensus_public.credential_binding, public);
        assert_eq!(
            consensus_public.rfc_statement.crl_number,
            authoritative_state.crl_record().crl_number
        );
        assert_eq!(
            consensus_public
                .rfc_statement
                .presentation_not_before_unix_seconds,
            statement.presentation_not_before_unix_seconds
        );
        assert_eq!(
            consensus_public
                .rfc_statement
                .presentation_not_after_unix_seconds,
            statement.presentation_not_after_unix_seconds
        );
        let encoded = encode_zk_x509_credential_envelope_v1(public, b"X5M1main", b"X5C1ca")
            .expect("canonical credential envelope");

        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &statement,
                &authoritative_state,
                genesis_hash,
                &encoded,
            ),
            Err(ZkX509EngineErrorV1::AirIncomplete)
        );

        let mut malformed = encoded.clone();
        malformed.push(0);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &statement,
                &authoritative_state,
                genesis_hash,
                &malformed,
            ),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::MalformedEnvelope
            ))
        );

        let mut wrong_intent = statement.clone();
        wrong_intent.context.transaction_intent_digest =
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0xA1; 32]);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &wrong_intent,
                &authoritative_state,
                genesis_hash,
                &encoded,
            ),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );

        let mut wrong_profile = statement.clone();
        wrong_profile.context.verifier_digest =
            iroha_data_model::privacy::PrivacyVerifierDigestV1::new([0xA2; 32]);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &wrong_profile,
                &authoritative_state,
                genesis_hash,
                &encoded,
            ),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );

        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &statement,
                &authoritative_state,
                [0x92; 32],
                &encoded,
            ),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );
        assert_eq!(
            verify_zk_x509_credential_proof_v1(&statement, &authoritative_state, [0; 32], &encoded,),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::InvalidStatement
            ))
        );
    }
}
