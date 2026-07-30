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
    io_air::ZK_X509_IO_AIR_DESCRIPTOR_V1,
    main_assembly::ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1,
    main_io::ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
    merkle::hash_frame_v1,
    preprocessed_fixed::{
        ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1,
        ZK_X509_P256_LOG19_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1,
        ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1,
        ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        ZkX509P256Log19PreprocessedFixedCertificateV1, ZkX509PreprocessedFixedErrorV1,
        ZkX509ShaPreprocessedFixedCertificateV1,
        pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1,
        pinned_zk_x509_sha_preprocessed_fixed_certificate_v1,
    },
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
    sha_call_bus_stark::ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1,
    sha256_air::ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
    sha256_word_air::ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
};
use crate::privacy_state::{
    PrivacyZkX509AuthoritativeStateV1, validate_privacy_zk_x509_statement_state_v1,
};

const COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.provisional-compiled-profile.v1";
const RELEASE_COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-x509.release-compiled-profile.v1";
const REFERENCE_PREPARATION_SCHEMA_V1: &[u8] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-IRX509W1-private-witness+strict-reference-relation";
const COMPILED_PROFILE_FIELD_COUNT_V1: usize = 19;
const RELEASE_COMPILED_PROFILE_FIELD_COUNT_V1: usize = 24;

/// Frozen digest of the exact fail-closed native profile.
///
/// This digest binds the implemented subproof parameters and the explicit gap
/// inventory. It is not an activation digest.
pub(crate) const ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1: [u8; 32] = [
    0x0f, 0x1b, 0x79, 0x24, 0xf2, 0xfc, 0xda, 0x03, 0x84, 0xe0, 0xfc, 0x20, 0xad, 0x68, 0x1d, 0xd0,
    0x32, 0x17, 0xc9, 0xe0, 0x24, 0xa4, 0x5a, 0xd6, 0xf7, 0x40, 0x71, 0x29, 0x3d, 0xd3, 0xc1, 0x7f,
];

// This second pin is intentionally absent until the fixed-oracle root and the
// resulting 24-field release frame have both been independently reproduced.
// The 19-field provisional digest above remains usable only by the isolated
// fail-closed laboratory subproofs.
const ZK_X509_RELEASE_COMPILED_PROFILE_DIGEST_V1: Option<[u8; 32]> = None;

/// Exact certificate-bearing profile required by MAIN prover and verifier
/// construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CompiledProfileV1 {
    digest: [u8; 32],
    sha_preprocessed_fixed: ZkX509ShaPreprocessedFixedCertificateV1,
    encoded_sha_preprocessed_fixed: [u8; ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
    p256_log19_preprocessed_fixed: ZkX509P256Log19PreprocessedFixedCertificateV1,
    encoded_p256_log19_preprocessed_fixed:
        [u8; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
}

impl ZkX509CompiledProfileV1 {
    /// Consensus transcript digest of the complete release manifest.
    pub(crate) const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Verifier-owned fixed-oracle certificate bound by the manifest.
    pub(crate) const fn sha_preprocessed_fixed(self) -> ZkX509ShaPreprocessedFixedCertificateV1 {
        self.sha_preprocessed_fixed
    }

    /// Exact bytes committed as the final manifest field.
    pub(crate) const fn encoded_sha_preprocessed_fixed(
        self,
    ) -> [u8; ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1] {
        self.encoded_sha_preprocessed_fixed
    }

    /// Verifier-owned P-256 log19 fixed-oracle certificate.
    pub(crate) const fn p256_log19_preprocessed_fixed(
        self,
    ) -> ZkX509P256Log19PreprocessedFixedCertificateV1 {
        self.p256_log19_preprocessed_fixed
    }

    /// Exact P-256 log19 certificate bytes committed by the release manifest.
    pub(crate) const fn encoded_p256_log19_preprocessed_fixed(
        self,
    ) -> [u8; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1] {
        self.encoded_p256_log19_preprocessed_fixed
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
    /// Fixed preprocessing has no pinned root or its certificate is invalid.
    #[error(transparent)]
    FixedPreprocessing(#[from] ZkX509PreprocessedFixedErrorV1),
    /// The complete certificate-bearing release manifest has not been pinned.
    #[error("zk-X509 certificate-bearing compiled profile is not release-pinned")]
    CompiledProfileUnpinned,
    /// Recomputed release-manifest digest differs from the consensus pin.
    #[error("zk-X509 certificate-bearing compiled profile digest mismatch")]
    CompiledProfileMismatch,
}

/// Verify one canonical credential proof against verifier-owned consensus data.
///
/// This is the sole consensus entry point for `X5S1`. It already performs
/// strict envelope decoding and binds the complete typed statement to the
/// committed genesis hash before inspecting any aggregate. It deliberately
/// cannot succeed while either the explicit gap inventory is non-empty or the
/// complete main-plus-compact-CA verifier is unavailable.
pub(crate) fn verify_zk_x509_credential_proof_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    genesis_hash: [u8; 32],
    encoded_proof: &[u8],
) -> Result<(), ZkX509EngineErrorV1> {
    let expected_public =
        ZkX509CredentialPublicBindingV1::from_consensus_context_v1(statement, genesis_hash)?;
    let envelope = decode_zk_x509_credential_envelope_v1(encoded_proof)?;
    if envelope.public != expected_public {
        return Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch.into());
    }

    require_complete_zk_x509_air_v1()?;

    // TODO(zk-x509-release): invoke the completed MAIN aggregate verifier and
    // the real compact-CA verifier here under
    // `construct_zk_x509_compiled_profile_v1`, then compare every SHA-call
    // terminal and the root-SPKI I/O products through
    // `verify_zk_x509_credential_envelope_with_v1`. Never replace this with a
    // reference-relation check or an independently accepted CA proof.
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

fn compiled_profile_fields_v1() -> [&'static [u8]; COMPILED_PROFILE_FIELD_COUNT_V1] {
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
    ]
}

fn release_compiled_profile_fields_v1<'a>(
    encoded_sha_certificate: &'a [u8; ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
    encoded_p256_certificate: &'a [u8; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
) -> [&'a [u8]; RELEASE_COMPILED_PROFILE_FIELD_COUNT_V1] {
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
        ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1,
        ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        encoded_sha_certificate,
        ZK_X509_P256_LOG19_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        encoded_p256_certificate,
    ]
}

/// Recompute the exact provisional compiled-profile digest.
pub(crate) fn recompute_zk_x509_provisional_compiled_profile_digest_v1() -> [u8; 32] {
    hash_frame_v1(
        COMPILED_PROFILE_DIGEST_DOMAIN_V1,
        &compiled_profile_fields_v1(),
    )
    .expect("fixed compiled-profile fields are representable")
}

/// Construct the sole complete certificate-bearing release profile.
///
/// Root pinning and manifest-digest pinning are independent release checks.
/// A MAIN constructor cannot succeed between those two ceremony steps.
pub(crate) fn construct_zk_x509_compiled_profile_v1()
-> Result<ZkX509CompiledProfileV1, ZkX509EngineErrorV1> {
    let sha_preprocessed_fixed = pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()?;
    let encoded_sha_preprocessed_fixed = sha_preprocessed_fixed.encode_v1()?;
    let p256_log19_preprocessed_fixed =
        pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()?;
    let encoded_p256_log19_preprocessed_fixed = p256_log19_preprocessed_fixed.encode_v1()?;
    let digest = hash_frame_v1(
        RELEASE_COMPILED_PROFILE_DIGEST_DOMAIN_V1,
        &release_compiled_profile_fields_v1(
            &encoded_sha_preprocessed_fixed,
            &encoded_p256_log19_preprocessed_fixed,
        ),
    )
    .expect("fixed release compiled-profile fields are representable");
    let expected = ZK_X509_RELEASE_COMPILED_PROFILE_DIGEST_V1
        .ok_or(ZkX509EngineErrorV1::CompiledProfileUnpinned)?;
    if digest != expected {
        return Err(ZkX509EngineErrorV1::CompiledProfileMismatch);
    }
    Ok(ZkX509CompiledProfileV1 {
        digest,
        sha_preprocessed_fixed,
        encoded_sha_preprocessed_fixed,
        p256_log19_preprocessed_fixed,
        encoded_p256_log19_preprocessed_fixed,
    })
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
    fn compiled_profile_manifest_and_frame_are_exact_and_independently_pinned() {
        let expected_fields: [&[u8]; COMPILED_PROFILE_FIELD_COUNT_V1] = [
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
        assert_eq!(expected_fields.len(), 19);
        assert_eq!(compiled_profile_fields_v1(), expected_fields);
        assert!(
            expected_fields[12]
                .windows(b"post-base-challenges=exact272-goldilocks-fields".len())
                .any(|window| window == b"post-base-challenges=exact272-goldilocks-fields")
        );
        assert!(
            expected_fields[16]
                .windows(b"main-common-lde-log25".len())
                .any(|window| window == b"main-common-lde-log25")
        );
        assert!(
            !expected_fields[16]
                .windows(b"common-lde-log22".len())
                .any(|window| window == b"common-lde-log22")
        );

        let frame = independently_encode_compiled_profile_frame_v1(&expected_fields);
        assert_eq!(frame.len(), 10_631);
        let digest: [u8; 32] = Sha256::digest(frame).into();
        assert_eq!(digest, ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1);
        assert_eq!(
            digest,
            recompute_zk_x509_provisional_compiled_profile_digest_v1()
        );
    }

    #[test]
    fn omitted_sha_call_descriptor_and_stale_digest_fail_closed() {
        const STALE_PINNED_DIGEST_V1: [u8; 32] = [
            0x21, 0xb9, 0x75, 0xe1, 0xec, 0x97, 0x8c, 0x6c, 0x67, 0xca, 0x95, 0xd6, 0xb9, 0x0d,
            0x73, 0x67, 0x75, 0x2c, 0x3f, 0x7e, 0x87, 0x10, 0x25, 0x1f, 0x37, 0xc2, 0xcd, 0x31,
            0xd6, 0x42, 0x98, 0x0b,
        ];
        const OMITTED_SHA_CALL_DIGEST_V1: [u8; 32] = [
            0xa5, 0x6b, 0xc0, 0x86, 0xbc, 0xfb, 0x56, 0x35, 0x29, 0xdd, 0x51, 0x25, 0xb3, 0xdb,
            0xa2, 0xf5, 0x17, 0xc0, 0xb7, 0xd8, 0x4a, 0x1c, 0x63, 0xf9, 0x37, 0xeb, 0x25, 0x91,
            0xb0, 0x14, 0x3e, 0x2c,
        ];

        let canonical = compiled_profile_fields_v1();
        let mut omitted = Vec::with_capacity(COMPILED_PROFILE_FIELD_COUNT_V1 - 1);
        omitted.extend_from_slice(&canonical[..16]);
        omitted.extend_from_slice(&canonical[17..]);
        assert_eq!(omitted.len(), 18);
        let omitted_digest = independent_compiled_profile_digest_v1(&omitted);
        assert_eq!(omitted_digest, OMITTED_SHA_CALL_DIGEST_V1);
        assert_ne!(
            omitted_digest,
            ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1
        );
        assert_ne!(
            STALE_PINNED_DIGEST_V1,
            ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1
        );
        assert_ne!(
            STALE_PINNED_DIGEST_V1,
            recompute_zk_x509_provisional_compiled_profile_digest_v1()
        );
    }

    #[test]
    fn certificate_bearing_release_manifest_binds_root_geometry_order_and_descriptors() {
        let sha_certificate =
            ZkX509ShaPreprocessedFixedCertificateV1::from_derived_root_v1([1; 32])
                .expect("nonzero candidate root");
        let p256_certificate =
            ZkX509P256Log19PreprocessedFixedCertificateV1::from_derived_root_v1([2; 32])
                .expect("nonzero P-256 candidate root");
        let encoded_sha = sha_certificate
            .encode_v1()
            .expect("SHA candidate certificate");
        let encoded_p256 = p256_certificate
            .encode_v1()
            .expect("P-256 candidate certificate");
        let fields = release_compiled_profile_fields_v1(&encoded_sha, &encoded_p256);
        assert_eq!(fields.len(), 24);
        assert_eq!(fields[19], ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1);
        assert_eq!(
            fields[20],
            ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1
        );
        assert_eq!(fields[21], encoded_sha);
        assert_eq!(
            fields[22],
            ZK_X509_P256_LOG19_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1
        );
        assert_eq!(fields[23], encoded_p256);
        let canonical = hash_frame_v1(RELEASE_COMPILED_PROFILE_DIGEST_DOMAIN_V1, &fields)
            .expect("candidate release frame");

        for byte in [9, 13, 17, 49] {
            let mut changed = encoded_sha;
            changed[byte] ^= 1;
            let digest = hash_frame_v1(
                RELEASE_COMPILED_PROFILE_DIGEST_DOMAIN_V1,
                &release_compiled_profile_fields_v1(&changed, &encoded_p256),
            )
            .expect("mutated release frame");
            assert_ne!(
                digest, canonical,
                "certificate byte {byte} must be transcript-bound"
            );
        }
        for byte in [9, 13, 19, 51] {
            let mut changed = encoded_p256;
            changed[byte] ^= 1;
            let digest = hash_frame_v1(
                RELEASE_COMPILED_PROFILE_DIGEST_DOMAIN_V1,
                &release_compiled_profile_fields_v1(&encoded_sha, &changed),
            )
            .expect("mutated release frame");
            assert_ne!(
                digest, canonical,
                "P-256 certificate byte {byte} must be transcript-bound"
            );
        }
    }

    #[test]
    fn certificate_bearing_release_constructor_is_closed_until_both_pins_exist() {
        assert_eq!(
            construct_zk_x509_compiled_profile_v1(),
            Err(ZkX509EngineErrorV1::FixedPreprocessing(
                ZkX509PreprocessedFixedErrorV1::Unpinned
            ))
        );
    }

    #[test]
    fn provisional_profile_and_gap_manifest_are_pinned_fail_closed() {
        assert_eq!(
            recompute_zk_x509_provisional_compiled_profile_digest_v1(),
            ZK_X509_PROVISIONAL_COMPILED_PROFILE_DIGEST_V1
        );
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
        let (statement, _) = crate::privacy_engines::zk_x509::projection_air::tests::fixture();
        let genesis_hash = [0x91; 32];
        let public =
            ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis_hash)
                .expect("canonical public binding");
        let encoded = encode_zk_x509_credential_envelope_v1(public, b"X5M1main", b"X5C1ca")
            .expect("canonical credential envelope");

        assert_eq!(
            verify_zk_x509_credential_proof_v1(&statement, genesis_hash, &encoded),
            Err(ZkX509EngineErrorV1::AirIncomplete)
        );

        let mut malformed = encoded.clone();
        malformed.push(0);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(&statement, genesis_hash, &malformed),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::MalformedEnvelope
            ))
        );

        let mut wrong_intent = statement.clone();
        wrong_intent.context.transaction_intent_digest =
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0xA1; 32]);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(&wrong_intent, genesis_hash, &encoded),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );

        let mut wrong_profile = statement.clone();
        wrong_profile.context.verifier_digest =
            iroha_data_model::privacy::PrivacyVerifierDigestV1::new([0xA2; 32]);
        assert_eq!(
            verify_zk_x509_credential_proof_v1(&wrong_profile, genesis_hash, &encoded),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );

        assert_eq!(
            verify_zk_x509_credential_proof_v1(&statement, [0x92; 32], &encoded),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::PublicBindingMismatch
            ))
        );
        assert_eq!(
            verify_zk_x509_credential_proof_v1(&statement, [0; 32], &encoded),
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::InvalidStatement
            ))
        );
    }
}
