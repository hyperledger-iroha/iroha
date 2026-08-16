//! Native zk-X509 engine boundary.
//!
//! The consensus verifier is present in every node build. Prover preparation and proof construction
//! are compiled only for tests or the explicitly non-shipping `privacy-release-evidence` workflow.
//!
//! The sole credential path constructs and independently verifies the bound `X5S1` MAIN/compact-CA
//! envelope. A native reference check, projection-only proof, or collection of unbound subproofs is
//! never accepted as a credential proof.
#[cfg(any(test, feature = "privacy-release-evidence"))]
use super::{
    accumulator_stark::{
        ZkX509CaAccumulatorProofErrorV1, prove_zk_x509_ca_accumulator_stark_v1_with_rng,
    },
    codec::{ZkX509WitnessCodecErrorV1, ZkX509WitnessV1},
    credential_pre_aux::ZkX509CredentialPreAuxErrorV1,
    credential_stark::encode_zk_x509_credential_envelope_v1,
    main_assembly::{ZkX509MainAssemblyErrorV1, build_zk_x509_main_trace_assembly_v1},
    relation::{
        ZkX509GovernanceV1, ZkX509RelationErrorV1, ZkX509RelationOutputV1,
        validate_reference_relation_v1,
    },
    stark::{ZkX509StarkErrorV1, commit_zk_x509_main_base_phase_v1_with_rng},
};
use super::{
    accumulator_stark::{
        ca_accumulator_base_root_from_proof_v1, ca_accumulator_subproof_binding_from_proof_v1,
        ca_profile_digest_v1, ca_public_digest_v1,
    },
    air::{ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1, ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1},
    credential_pre_aux::{
        ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1, derive_zk_x509_credential_pre_aux_binding_v1,
    },
    credential_stark::{
        ZkX509CredentialProofErrorV1, ZkX509CredentialPublicBindingV1,
        decode_zk_x509_credential_envelope_v1, validate_cross_subproof_binding_v1,
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
    main_io::ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
    merkle::hash_frame_v1,
    profile::{
        ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1, ZK_X509_CRL_PROFILE_V1,
        ZK_X509_CRL_REVISION_SCHEMA_V1, ZK_X509_CRL_SCOPE_PROFILE_V1, ZK_X509_ECDSA_RULES_V1,
        ZK_X509_RFC5280_PROFILE_V1, ZK_X509_SOURCE_PROFILE_V1, ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
        ZK_X509_SUITE_V1, ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1, ZkX509ShaCallPublicShapeV1,
        ZkX509ShaCallScheduleV1,
    },
    sha256_word_air::ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
    stark::{verify_zk_x509_main_aggregate_stark_v1, zk_x509_main_pre_aux_from_proof_v1},
    verifier_profile::{
        ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1, ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
        compile_zk_x509_rfc_statement_from_authoritative_state_v1,
    },
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use crate::privacy_engines::prover_randomness::{
    HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1,
};
use crate::privacy_state::PrivacyZkX509AuthoritativeStateV1;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use crate::privacy_state::validate_privacy_zk_x509_statement_state_v1;
use iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use iroha_data_model::privacy::PrivacyConsensusLimitsV1;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use rand::TryCryptoRng;
use thiserror::Error;
const COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.compiled-profile.v1";
const REFERENCE_PREPARATION_SCHEMA_V1: &[u8] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-IRX509W1-private-witness+strict-reference-relation";
const COMPILED_PROFILE_FIELD_COUNT_V1: usize = 29;
const SHA_DISCLOSURE_SHAPE_COUNT_V1: usize = 5;
// Independently encoded and SHA-256 checked from the exact ordered 29-field
// release manifest after the compact-CA descriptor and all six algebraic
// schedules passed their release KATs. There is no provisional, root-bearing,
// or certificate-bearing profile in the first-release protocol.
const ZK_X509_COMPILED_PROFILE_DIGEST_V1: Option<[u8; 32]> = Some([
    0x88, 0x1e, 0x56, 0x0d, 0xb8, 0x72, 0xf4, 0x7f, 0xdc, 0xd7, 0xdc, 0x29, 0x0e, 0xb6, 0xe2, 0x80,
    0x8a, 0x09, 0x82, 0x2b, 0x41, 0xe6, 0x87, 0x94, 0x92, 0x41, 0x4b, 0x7e, 0x99, 0x30, 0x3b, 0x67,
]);
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
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedZkX509ProverInputV1 {
    witness: ZkX509WitnessV1,
    projection: ZkX509RelationOutputV1,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
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
/// Native prover preparation or credential-proof failure.
#[derive(Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509EngineErrorV1 {
    /// Persisted state, roots, epochs, policy, or trusted block time mismatch.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 authoritative state validation failed: {0}")]
    InvalidAuthoritativeState(String),
    /// Private witness bytes are not the sole exact bounded grammar.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    WitnessCodec(#[from] ZkX509WitnessCodecErrorV1),
    /// Decode followed by encode did not reproduce the exact input bytes.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 witness codec failed its exact round-trip invariant")]
    WitnessRoundTripMismatch,
    /// Strict RFC 5280/reference relation failure.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    ReferenceRelation(#[from] ZkX509RelationErrorV1),
    /// Canonical challenge-independent MAIN material could not be assembled.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    MainAssembly(#[from] ZkX509MainAssemblyErrorV1),
    /// Joint MAIN/compact-CA challenge derivation failed.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    CredentialPreAux(#[from] ZkX509CredentialPreAuxErrorV1),
    /// Complete MAIN proof construction failed.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    MainProofConstruction(#[from] ZkX509StarkErrorV1),
    /// Dedicated compact-CA proof construction failed.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    CaProofConstruction(#[from] ZkX509CaAccumulatorProofErrorV1),
    /// Prover entropy was unavailable or failed the first-release health policy.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error(transparent)]
    ProverRandomness(#[from] TryCryptoProverRandomnessErrorV1),
    /// Canonical credential envelope or consensus-context binding failure.
    #[error(transparent)]
    CredentialProof(#[from] ZkX509CredentialProofErrorV1),
    /// Producer-generated X5S1 bytes failed the independent consensus verifier.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 credential prover self-check failed")]
    ProverSelfCheckFailed,
    /// Canonical MAIN assembly did not reproduce prover preparation exactly.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 canonical prover projection mismatch")]
    ProverProjectionMismatch,
    /// The canonical SHA algebraic compiler or schedule rejected its profile.
    #[error(transparent)]
    ShaFixedAlgebraic(#[from] ZkX509ShaFixedAlgebraicErrorV1),
    /// The canonical P-256 algebraic compiler or schedule rejected its profile.
    #[error(transparent)]
    P256FixedAlgebraic(#[from] ZkX509P256FixedAlgebraicErrorV1),
    /// The complete 29-field release manifest has not been pinned.
    #[error("zk-X509 compiled profile is not release-pinned")]
    CompiledProfileUnpinned,
    /// Recomputed 29-field manifest digest differs from the consensus pin.
    #[error("zk-X509 compiled profile digest mismatch")]
    CompiledProfileMismatch,
}
/// Replay the complete cryptographic verifier for the bound subproof pair.
fn verify_zk_x509_credential_subproofs_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    consensus_public: &ZkX509ConsensusPublicInputsV1,
    main_aggregate: &[u8],
    ca_subproof: &[u8],
) -> Result<(), ZkX509EngineErrorV1> {
    construct_zk_x509_compiled_profile_v1()?;
    let sha_shape = ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: consensus_public
            .rfc_statement
            .disclosed_attribute_indices
            .len(),
    };
    let sha_schedule = ZkX509ShaCallScheduleV1::new(sha_shape)
        .map_err(|_| ZkX509CredentialProofErrorV1::InvalidStatement)?;
    let main_pre_aux =
        zk_x509_main_pre_aux_from_proof_v1(consensus_public.credential_binding, main_aggregate)
            .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    let ca_base_root = ca_accumulator_base_root_from_proof_v1(ca_subproof)
        .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    let credential_binding = derive_zk_x509_credential_pre_aux_binding_v1(
        main_pre_aux,
        ca_profile_digest_v1().map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?,
        ca_public_digest_v1(
            consensus_public.credential_binding.ca_public_v1(),
            &sha_schedule,
        )
        .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?,
        ca_base_root,
    )
    .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?;
    let main_binding = verify_zk_x509_main_aggregate_stark_v1(
        statement,
        &consensus_public.rfc_statement,
        consensus_public.credential_binding,
        credential_binding,
        main_aggregate,
    )
    .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    let ca_binding = ca_accumulator_subproof_binding_from_proof_v1(
        consensus_public.credential_binding.ca_public_v1(),
        &sha_schedule,
        main_pre_aux,
        ca_subproof,
    )
    .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    validate_cross_subproof_binding_v1(
        consensus_public.credential_binding,
        main_binding,
        ca_binding,
    )?;
    Ok(())
}
/// Verify one canonical credential proof against verifier-owned consensus data.
///
/// This is the sole consensus entry point for `X5S1`. It already performs strict envelope decoding
/// and binds the complete typed statement to the committed genesis hash before inspecting any
/// aggregate. The caller must supply the same authoritative snapshot it already validated against
/// trusted block time and consensus limits; the engine compiles the RFC public input from that
/// snapshot rather than from proof metadata.
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
    verify_zk_x509_credential_subproofs_v1(
        statement,
        &consensus_public,
        envelope.main_aggregate,
        envelope.ca_subproof,
    )
}
/// Decode and validate the exact prover input against trusted ledger state.
///
/// This function performs no proof construction. Its output is the only
/// admitted input to the credential-proof constructor.
#[cfg(any(test, feature = "privacy-release-evidence"))]
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
/// Construct one canonical `X5S1` credential proof with injected entropy.
///
/// Every state, witness, release-profile, topology, and native-relation check
/// completes before the entropy source is touched. The constructor then:
///
/// 1. commits the exact six MAIN base groups;
/// 2. constructs and self-verifies the compact-CA proof against those roots;
/// 3. derives the sole joint `X5B1` capability from the seven committed roots;
/// 4. commits MAIN auxiliary groups and completes `X5M1`;
/// 5. wraps the ordered pair in `X5S1` and independently invokes the consensus
///    verifier on the exact final bytes.
///
/// There is no independently accepted subproof path and no host-side
/// reference-relation substitute for the final self-check.
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn prove_zk_x509_credential_proof_v1_with_rng<R: TryCryptoRng>(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
    genesis_hash: [u8; 32],
    encoded_witness: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509EngineErrorV1> {
    // Every witness-dependent preflight deliberately precedes the first
    // entropy read.
    construct_zk_x509_compiled_profile_v1()?;
    let consensus_public =
        compile_zk_x509_consensus_public_inputs_v1(statement, authoritative_state, genesis_hash)?;
    let prepared = prepare_zk_x509_prover_input_v1(
        statement,
        authoritative_state,
        trusted_block_timestamp_ms,
        consensus_limits,
        encoded_witness,
    )?;
    let trust_anchor = authoritative_state.trust_anchor();
    let crl = authoritative_state.crl_record();
    let governance = ZkX509GovernanceV1 {
        trust_anchor: &trust_anchor,
        certificate_policy: authoritative_state.certificate_policy(),
        crl: &crl,
    };
    let assembly = build_zk_x509_main_trace_assembly_v1(statement, governance, prepared.witness())?;
    if assembly.relation_output != prepared.projection() {
        return Err(ZkX509EngineErrorV1::ProverProjectionMismatch);
    }
    let mut checked_rng = HealthCheckedTryCryptoRngV1::new(rng)?;
    let (main_phase, main_pre_aux) = commit_zk_x509_main_base_phase_v1_with_rng(
        statement,
        &assembly,
        consensus_public.credential_binding,
        &mut checked_rng,
    )?;
    let ca_subproof = prove_zk_x509_ca_accumulator_stark_v1_with_rng(
        &assembly.ca_accumulator_trace,
        &assembly.sha_schedule,
        main_pre_aux,
        &mut checked_rng,
    )?;
    let ca_base_root = ca_accumulator_base_root_from_proof_v1(&ca_subproof)?;
    let credential_binding = derive_zk_x509_credential_pre_aux_binding_v1(
        main_pre_aux,
        ca_profile_digest_v1()?,
        ca_public_digest_v1(
            consensus_public.credential_binding.ca_public_v1(),
            &assembly.sha_schedule,
        )?,
        ca_base_root,
    )?;
    let main_aggregate = main_phase
        .bind_credential_pre_aux_v1_with_rng(credential_binding, &mut checked_rng)?
        .finish_v1_with_rng(&mut checked_rng)?;
    let encoded = encode_zk_x509_credential_envelope_v1(
        consensus_public.credential_binding,
        &main_aggregate,
        &ca_subproof,
    )?;
    let envelope = decode_zk_x509_credential_envelope_v1(&encoded)
        .map_err(|_| ZkX509EngineErrorV1::ProverSelfCheckFailed)?;
    if envelope.public != consensus_public.credential_binding {
        return Err(ZkX509EngineErrorV1::ProverSelfCheckFailed);
    }
    verify_zk_x509_credential_subproofs_v1(
        statement,
        &consensus_public,
        envelope.main_aggregate,
        envelope.ca_subproof,
    )
    .map_err(|_| ZkX509EngineErrorV1::ProverSelfCheckFailed)?;
    Ok(encoded)
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
        &ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1,
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
/// Recompute the sole exact 29-field compiled-profile digest.
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
    use super::super::profile::ZK_X509_HASH_FRAME_DOMAIN_V1;
    use super::*;
    use crate::privacy_engines::zk_x509::credential_stark::encode_zk_x509_credential_envelope_v1;
    use sha2::{Digest, Sha256};
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
    fn compiled_profile_manifest_has_the_exact_29_field_order() {
        let sha_digests: [[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x31 + shape).expect("five shapes"); 32]);
        let p256_digest = [0x41; 32];
        let fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        let original_fields: [&[u8]; 20] = [
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
            &ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1,
            ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
            ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
            ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1,
            ZK_X509_IO_AIR_DESCRIPTOR_V1,
            REFERENCE_PREPARATION_SCHEMA_V1,
        ];
        assert_eq!(fields.len(), 29);
        assert_eq!(&fields[..20], &original_fields);
        assert_eq!(fields[20], ZK_X509_FIXED_ALGEBRAIC_DESCRIPTOR_V1);
        assert_eq!(
            fields[21],
            ZK_X509_SHA_FIXED_ALGEBRAIC_COMPILER_DESCRIPTOR_V1
        );
        for (shape, digest) in sha_digests.iter().enumerate() {
            assert_eq!(fields[22 + shape], digest);
        }
        assert_eq!(fields[27], ZK_X509_P256_FIXED_ALGEBRAIC_DESCRIPTOR_V1);
        assert_eq!(fields[28], p256_digest);
        assert!(
            fields[12]
                .windows(b"post-base-challenges=exact272-goldilocks-fields".len())
                .any(|window| window == b"post-base-challenges=exact272-goldilocks-fields")
        );
        assert!(
            fields[17]
                .windows(b"main-common-lde-log25".len())
                .any(|window| window == b"main-common-lde-log25")
        );
        assert!(
            !fields[17]
                .windows(b"common-lde-log22".len())
                .any(|window| window == b"common-lde-log22")
        );
    }
    #[test]
    fn compiled_profile_digest_exactly_binds_compact_ca_subproof_descriptor_pin() {
        let sha_digests: [[u8; 32]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x71 + shape).expect("five shapes"); 32]);
        let p256_digest = [0x81; 32];
        let canonical_fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        assert_eq!(
            canonical_fields[14],
            ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1.as_slice()
        );
        let canonical = independent_compiled_profile_digest_v1(&canonical_fields);
        let mut changed = canonical_fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        changed[14][0] ^= 1;
        let changed_fields = changed.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_ne!(
            independent_compiled_profile_digest_v1(&changed_fields),
            canonical,
            "the compact-CA prover/verifier descriptor pin must rotate the compiled profile"
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
        for field in 20..COMPILED_PROFILE_FIELD_COUNT_V1 {
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
        reordered.swap(22, 23);
        let reordered_fields = reordered.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_ne!(
            independent_compiled_profile_digest_v1(&reordered_fields),
            canonical,
            "SHA disclosure-shape digest order must be bound"
        );
    }
    #[test]
    fn compiled_profile_constructor_matches_the_independent_release_pin() {
        let (sha_digests, p256_digest) =
            compiled_profile_schedule_digests_v1().expect("all six frozen schedules");
        let fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        let independent = independent_compiled_profile_digest_v1(&fields);
        let recomputed =
            recompute_zk_x509_compiled_profile_digest_v1().expect("canonical manifest digest");
        assert_eq!(recomputed, independent);
        assert_eq!(ZK_X509_COMPILED_PROFILE_DIGEST_V1, Some(independent));
        assert_eq!(
            construct_zk_x509_compiled_profile_v1()
                .expect("release-pinned profile")
                .digest(),
            independent
        );
    }
    #[test]
    fn sole_profile_is_pinned_while_release_activation_stays_governance_gated() {
        assert!(ZK_X509_COMPILED_PROFILE_DIGEST_V1.is_some());
        assert!(
            String::from_utf8_lossy(ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1)
                .ends_with("activation=governance-gated")
        );
    }
    #[test]
    fn consensus_entry_point_decodes_and_binds_context_before_verification() {
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
            Err(ZkX509EngineErrorV1::CredentialProof(
                ZkX509CredentialProofErrorV1::MainProof
            ))
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
    #[test]
    fn credential_prover_has_one_preflighted_joint_root_path_and_no_subproof_escape() {
        let source = include_str!("engine.rs");
        let prover_start = source
            .find("pub(crate) fn prove_zk_x509_credential_proof_v1_with_rng")
            .expect("sole credential prover");
        let prover_end = source[prover_start..]
            .find("fn compiled_profile_fields_v1")
            .map(|offset| prover_start + offset)
            .expect("sole credential prover end");
        let prover = &source[prover_start..prover_end];
        let production_source = &source[..source.find("#[cfg(test)]").expect("test module")];
        let profile_gate = prover
            .find("construct_zk_x509_compiled_profile_v1()")
            .expect("pinned profile validation");
        let preparation = prover
            .find("prepare_zk_x509_prover_input_v1(")
            .expect("canonical witness preparation");
        let assembly = prover
            .find("build_zk_x509_main_trace_assembly_v1(")
            .expect("canonical MAIN assembly");
        let entropy = prover
            .find("HealthCheckedTryCryptoRngV1::new(rng)")
            .expect("health-checked entropy");
        let main_base = prover
            .find("commit_zk_x509_main_base_phase_v1_with_rng(")
            .expect("six MAIN base roots");
        let ca = prover
            .find("prove_zk_x509_ca_accumulator_stark_v1_with_rng(")
            .expect("compact-CA proof");
        let joint_binding = prover
            .find("derive_zk_x509_credential_pre_aux_binding_v1(")
            .expect("joint seven-root X5B1");
        let main_aux = prover
            .find(".bind_credential_pre_aux_v1_with_rng(")
            .expect("bound MAIN auxiliary phase");
        let envelope = prover
            .find("encode_zk_x509_credential_envelope_v1(")
            .expect("X5S1 envelope");
        let self_check = prover
            .find("verify_zk_x509_credential_subproofs_v1(")
            .expect("independent final self-check");
        assert!(
            profile_gate < preparation
                && preparation < assembly
                && assembly < entropy
                && entropy < main_base
                && main_base < ca
                && ca < joint_binding
                && joint_binding < main_aux
                && main_aux < envelope
                && envelope < self_check
        );
        for forbidden in [
            "verify_reference",
            "accept_main_subproof",
            "accept_ca_subproof",
            "ConsensusVerifierUnavailable",
            "require_complete_zk_x509_air_v1",
            "after_release_gate",
            "zk_x509_air_gaps_v1",
            "OsRng",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "credential prover must not contain {forbidden}"
            );
        }
        #[derive(Debug)]
        struct EntropyMustNotBeRead;
        impl core::fmt::Display for EntropyMustNotBeRead {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("credential preflight reached entropy")
            }
        }
        struct PanicEntropy;
        impl rand::TryRngCore for PanicEntropy {
            type Error = EntropyMustNotBeRead;
            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                panic!("invalid credential preflight reached entropy")
            }
            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                panic!("invalid credential preflight reached entropy")
            }
            fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
                panic!("invalid credential preflight reached entropy")
            }
        }
        impl rand::TryCryptoRng for PanicEntropy {}
        let (statement, authoritative_state) =
            crate::privacy_verifier::zk_x509_dispatch_fixture_for_test();
        let trusted_block_timestamp_ms = statement
            .presentation_not_before_unix_seconds
            .checked_mul(1_000)
            .expect("fixture timestamp");
        assert!(matches!(
            prove_zk_x509_credential_proof_v1_with_rng(
                &statement,
                &authoritative_state,
                trusted_block_timestamp_ms,
                &PrivacyConsensusLimitsV1::taira_default(),
                [0x91; 32],
                &[],
                &mut PanicEntropy,
            ),
            Err(ZkX509EngineErrorV1::WitnessCodec(_))
        ));
    }
}
