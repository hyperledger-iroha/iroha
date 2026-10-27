//! Native zk-X509 engine boundary.
//!
//! The consensus verifier and complete native prover are present in ordinary builds. Holder-side
//! ownership and clearing witness buffers are exposed through [`super::holder`]; release fixtures
//! and process diagnostics remain separate from that product API.
//!
//! The sole credential path constructs and independently verifies the bound `X5S1` MAIN/compact-CA
//! envelope. A native reference check, projection-only proof, or collection of unbound subproofs is
//! never accepted as a credential proof.
#[cfg(test)]
use super::prover_observation::{PhaseTimerV1, PhaseV1};
use super::{
    accumulator_stark::{ZkX509CaAccumulatorProofErrorV1, stages::commit_ca_through_auxiliary_v1},
    codec::{ZkX509WitnessCodecErrorV1, ZkX509WitnessV1},
    credential_pre_aux::ZkX509CredentialPreAuxErrorV1,
    credential_stark::encode_zk_x509_credential_envelope_v1,
    errors::ZkX509RelationErrorV1,
    main_assembly::{ZkX509MainAssemblyErrorV1, build_zk_x509_main_trace_assembly_v1},
    relation::{ZkX509GovernanceV1, ZkX509RelationOutputV1, validate_reference_relation_v1},
    stark::{
        ZkX509StarkErrorV1, commit_zk_x509_main_base_phase_v1_with_rng, finish_joint_main_ca_v1,
    },
};
use super::{
    accumulator_stark::{
        ca_accumulator_base_root_from_proof_v1, ca_profile_digest_v1, ca_public_digest_v1,
        joint_verifier::CaJointVerifierOraclesV1,
    },
    air::{ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1, ZK_X509_COMPACT_CA_SUBPROOF_DESCRIPTOR_SHA256_V1},
    credential_joint::JointOriginalOpeningsV1,
    credential_pre_aux::{
        ZK_X509_CREDENTIAL_PRE_AUX_DESCRIPTOR_V1, derive_zk_x509_credential_pre_aux_binding_v1,
    },
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
    main_io::ZK_X509_MAIN_IO_DECLARATIONS_DESCRIPTOR_V1,
    merkle::hash_frame_v1,
    profile::{
        ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1, ZK_X509_CRL_PROFILE_V1,
        ZK_X509_CRL_REVISION_SCHEMA_V1, ZK_X509_CRL_SCOPE_PROFILE_V1, ZK_X509_ECDSA_RULES_V1,
        ZK_X509_RFC5280_PROFILE_V1, ZK_X509_SOURCE_PROFILE_V1, ZK_X509_STARK_PROFILE_DESCRIPTOR_V1,
        ZK_X509_SUITE_V1, ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1,
    },
    proof_instance::ZkX509ProofInstanceV1,
    sha_call_bus_stark::{
        ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1, ZkX509ShaCallPublicShapeV1,
        ZkX509ShaCallScheduleV1,
    },
    sha256_word_air::ZK_X509_SHA256_WORD_AIR_DESCRIPTOR_V1,
    stark::{
        MainJointVerifierOraclesV1, main_joint_auxiliary_from_proofs_v1,
        zk_x509_main_pre_aux_from_proof_v1,
    },
    verifier_profile::{
        ZK_X509_MAIN_ASSEMBLY_DESCRIPTOR_V1, ZK_X509_SHA256_LOCAL_AIR_DESCRIPTOR_V1,
        compile_zk_x509_rfc_statement_from_authoritative_state_v1,
    },
};
use crate::privacy_engines::prover_randomness::{
    HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1,
};
use crate::privacy_state::PrivacyZkX509AuthoritativeStateV1;
use crate::privacy_state::validate_privacy_zk_x509_statement_state_v1;
use iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1;
use iroha_data_model::privacy::PrivacyConsensusLimitsV1;
use rand::TryCryptoRng;
use thiserror::Error;
const COMPILED_PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.compiled-profile.v1";
const REFERENCE_PREPARATION_SCHEMA_V1: &[u8] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-norito-zk-x509-witness-v1-flags0+strict-reference-relation";
const COMPILED_PROFILE_FIELD_COUNT_V1: usize = 29;
const SHA_DISCLOSURE_SHAPE_COUNT_V1: usize = 5;
// Independently encoded and SHA-256 checked from the exact ordered 29-field
// manifest, including the compact-CA descriptor and all six SHA3-384 algebraic
// schedule digests in their opaque byte order. The manifest itself uses SHA-256.
// This identifies the sole compiled AIR and geometry; activation additionally requires
// the proof cap and the complete soundness and resource certificates.
// Native derivation and independent framing bind the joint MAIN/CA private links,
// quotient blinding, selected P256 inputs and RFC output metadata in this candidate.
// Native diagnostics and independent framing bind the required proof-instance nonce.
// Every fixed child and composite digest also binds the canonical 32-byte native
// BLAKE3 catalog; a catalog change requires coherent rederivation of this pin.
// TODO: complete native credential, hiding and resource qualification before activating this profile.
const ZK_X509_COMPILED_PROFILE_DIGEST_V1: Option<[u8; 32]> = Some([
    0xf5, 0xd2, 0xb9, 0x94, 0xa2, 0x5e, 0x90, 0xd6, 0xf3, 0xa5, 0x7e, 0x80, 0x0c, 0xe8, 0x04, 0xf8,
    0xa0, 0xc6, 0xd8, 0x07, 0x3f, 0xaf, 0xfc, 0xa5, 0xc5, 0xc2, 0xa6, 0x70, 0x4b, 0xef, 0x42, 0x7c,
]);
/// Exact algebraic-schedule-bearing profile required by MAIN.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub struct ZkX509CompiledProfileV1 {
    digest: [u8; 32],
}
impl ZkX509CompiledProfileV1 {
    /// Consensus transcript digest of the complete release manifest.
    ///
    /// This fingerprint identifies the compiled profile; it grants no source-pin or activation
    /// authority. Acceptance still requires the complete proof, soundness and resource checks.
    pub const fn digest(self) -> [u8; 32] {
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
    /// Digest and timestamp endpoints selected from the retained authoritative CRL revision.
    crl_public_inputs: super::verifier_profile::ZkX509CrlPublicInputsV1,
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
        crl_public_inputs:
            super::verifier_profile::ZkX509CrlPublicInputsV1::from_authoritative_state_v1(
                authoritative_state,
            ),
    })
}
/// Native prover preparation or credential-proof failure.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum ZkX509EngineErrorV1 {
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
    /// Canonical challenge-independent MAIN material could not be assembled.
    #[error(transparent)]
    MainAssembly(#[from] ZkX509MainAssemblyErrorV1),
    /// Joint MAIN/compact-CA challenge derivation failed.
    #[error(transparent)]
    CredentialPreAux(#[from] ZkX509CredentialPreAuxErrorV1),
    /// Complete MAIN proof construction failed.
    #[error(transparent)]
    MainProofConstruction(#[from] ZkX509StarkErrorV1),
    /// Dedicated compact-CA proof construction failed.
    #[error(transparent)]
    CaProofConstruction(#[from] ZkX509CaAccumulatorProofErrorV1),
    /// Prover entropy was unavailable or failed the first-release health policy.
    #[error(transparent)]
    ProverRandomness(#[from] TryCryptoProverRandomnessErrorV1),
    /// Canonical credential envelope or consensus-context binding failure.
    #[error(transparent)]
    CredentialProof(#[from] ZkX509CredentialProofErrorV1),
    /// Producer-generated X5S1 bytes failed the independent consensus verifier.
    #[error("zk-X509 credential prover self-check failed")]
    ProverSelfCheckFailed,
    /// Canonical MAIN assembly did not reproduce prover preparation exactly.
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
    proof_instance: ZkX509ProofInstanceV1,
    statement: &IrohaZkX509StarkP256StatementV1,
    consensus_public: &ZkX509ConsensusPublicInputsV1,
    main_aggregate: &[u8],
    ca_subproof: &[u8],
    joint_openings: &JointOriginalOpeningsV1,
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
    let main_pre_aux = zk_x509_main_pre_aux_from_proof_v1(
        proof_instance,
        consensus_public.credential_binding,
        main_aggregate,
    )
    .map_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1("credential-main", &error);
        #[cfg(not(test))]
        let _ = error;
        ZkX509CredentialProofErrorV1::MainProof
    })?;
    let ca_base_root = ca_accumulator_base_root_from_proof_v1(ca_subproof).map_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1("credential-ca", &error);
        #[cfg(not(test))]
        let _ = error;
        ZkX509CredentialProofErrorV1::CaProof
    })?;
    let credential_binding = derive_zk_x509_credential_pre_aux_binding_v1(
        main_pre_aux,
        ca_profile_digest_v1().map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?,
        ca_public_digest_v1(
            proof_instance,
            &consensus_public.credential_binding.ca_public_v1(),
            &sha_schedule,
        )
        .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?,
        ca_base_root,
    )
    .map_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1("credential-cross-binding", &error);
        #[cfg(not(test))]
        let _ = error;
        ZkX509CredentialProofErrorV1::CrossSubproofMismatch
    })?;
    let auxiliary =
        main_joint_auxiliary_from_proofs_v1(&credential_binding, main_aggregate, ca_subproof)
            .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?;
    let main = MainJointVerifierOraclesV1::new_v1(
        statement,
        &consensus_public.rfc_statement,
        consensus_public.crl_public_inputs,
        consensus_public.credential_binding,
        &credential_binding,
        auxiliary,
        &sha_schedule,
        main_aggregate,
    )
    .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    let ca = CaJointVerifierOraclesV1::new_v1(
        &consensus_public.credential_binding.ca_public_v1(),
        &sha_schedule,
        main_pre_aux,
        auxiliary,
        ca_subproof,
    )
    .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    let point = main
        .derive_joint_point_v1(&ca)
        .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?;
    let mut main = main
        .open_joint_point_v1(point)
        .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    let mut ca = ca
        .open_joint_point_v1(point)
        .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    let binding = main
        .bind_ca_openings_v1(&mut ca, joint_openings)
        .map_err(|_| ZkX509CredentialProofErrorV1::CrossSubproofMismatch)?;
    main.finish_v1(&binding)
        .map_err(|_| ZkX509CredentialProofErrorV1::MainProof)?;
    ca.finish_v1(&binding)
        .map_err(|_| ZkX509CredentialProofErrorV1::CaProof)?;
    Ok(())
}
/// Verify one canonical credential proof against verifier-owned consensus data.
///
/// This is the sole consensus entry point for `X5S1`. It already performs strict envelope decoding
/// and binds the complete typed statement to the committed genesis hash before inspecting any
/// aggregate. The caller must supply the same authoritative snapshot it already validated against
/// trusted block time and consensus limits; the engine compiles the RFC public input from that
/// snapshot rather than from proof metadata.
#[doc(hidden)]
pub fn verify_zk_x509_credential_proof_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    genesis_hash: [u8; 32],
    encoded_proof: &[u8],
) -> Result<(), ZkX509EngineErrorV1> {
    let consensus_public =
        compile_zk_x509_consensus_public_inputs_v1(statement, authoritative_state, genesis_hash)?;
    let envelope = decode_zk_x509_credential_envelope_v1(encoded_proof).inspect_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1("credential-envelope-decode", error);
        #[cfg(not(test))]
        let _ = error;
    })?;
    if envelope.public != consensus_public.credential_binding {
        return Err(ZkX509CredentialProofErrorV1::PublicBindingMismatch.into());
    }
    verify_zk_x509_credential_subproofs_v1(
        envelope.proof_instance,
        statement,
        &consensus_public,
        envelope.main_aggregate,
        envelope.ca_subproof,
        &envelope.joint_openings,
    )
}
fn validate_witness_round_trip_v1(
    witness: &ZkX509WitnessV1,
    encoded_witness: &[u8],
) -> Result<(), ZkX509EngineErrorV1> {
    let canonical = super::private_table::PrivateTableV1::new(
        witness.encode_v1()?,
        super::private_table::zeroize_words_v1,
    );
    if canonical.as_slice() != encoded_witness {
        return Err(ZkX509EngineErrorV1::WitnessRoundTripMismatch);
    }
    Ok(())
}

/// Decode and validate the exact prover input against trusted ledger state.
///
/// This function performs no proof construction. Its output is the only
/// admitted input to the credential-proof constructor.
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
    validate_witness_round_trip_v1(&witness, encoded_witness)?;
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
/// 1. joins the exact six MAIN base groups under one authenticated root;
/// 2. commits the CA base root, derives the joint `X5B1` capability, and commits
///    the CA auxiliary columns using those shared challenges;
/// 3. commits the MAIN auxiliary columns and binds both original auxiliary roots;
/// 4. commits both compositions and FRI masks before the shared DEEP point,
///    binds all joint openings, and completes both local FRI proofs;
/// 5. wraps the ordered pair and 132 joint openings in `X5S1` and independently
///    invokes the consensus verifier on the exact final bytes.
///
/// There is no independently accepted subproof path and no host-side
/// reference-relation substitute for the final self-check.
///
/// Successful construction consumes the canonical entropy sequence. Construction
/// is fail-fast after preflight: an error may consume only a prefix of that
/// sequence, and the injected RNG is neither rolled back nor advanced to a fixed
/// failure position. An uncertain accelerator completion returns no proof and
/// stops further source construction and entropy use under the existing process
/// quarantine. Callers must not depend on identical RNG state across failures.
#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
pub fn prove_zk_x509_credential_proof_v1_with_rng<R: TryCryptoRng>(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
    genesis_hash: [u8; 32],
    encoded_witness: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509EngineErrorV1> {
    if fastpq_prover::goldilocks_transform::goldilocks_transform_completion_uncertain_v1() {
        return Err(ZkX509StarkErrorV1::AcceleratorCompletionUncertain.into());
    }
    // Every witness-dependent preflight deliberately precedes the first
    // entropy read.
    #[cfg(test)]
    let preparation_timer = PhaseTimerV1::start_v1(PhaseV1::Preparation);
    construct_zk_x509_compiled_profile_v1()?;
    let consensus_public =
        compile_zk_x509_consensus_public_inputs_v1(statement, authoritative_state, genesis_hash)?;
    super::stark::validate_zk_x509_main_proof_budget_v1()?;
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
    #[cfg(test)]
    preparation_timer.complete_v1();
    #[cfg(test)]
    let assembly_timer = PhaseTimerV1::start_v1(PhaseV1::Assembly);
    let assembly = build_zk_x509_main_trace_assembly_v1(statement, governance, prepared.witness())?;
    if assembly.relation_output != prepared.projection() {
        return Err(ZkX509EngineErrorV1::ProverProjectionMismatch);
    }
    #[cfg(test)]
    assembly_timer.complete_v1();
    let mut checked_rng = HealthCheckedTryCryptoRngV1::new(rng)?;
    let proof_instance = ZkX509ProofInstanceV1::sample_v1(&mut checked_rng)?;
    let (main_phase, main_pre_aux) = commit_zk_x509_main_base_phase_v1_with_rng(
        proof_instance,
        statement,
        &assembly,
        consensus_public.credential_binding,
        &mut checked_rng,
    )?;
    #[cfg(test)]
    let ca_timer = PhaseTimerV1::start_v1(PhaseV1::CompactCa);
    let ca_phase = commit_ca_through_auxiliary_v1(
        &assembly.ca_accumulator_trace,
        &assembly.sha_schedule,
        main_pre_aux,
        &mut checked_rng,
    )?;
    #[cfg(test)]
    ca_timer.complete_v1();
    let main_phase =
        main_phase.bind_joint_credential_pre_aux_v1_with_rng(&ca_phase, &mut checked_rng)?;
    let (main_aggregate, ca_subproof, joint_openings) =
        finish_joint_main_ca_v1(main_phase, ca_phase, &mut checked_rng)?;
    #[cfg(test)]
    let envelope_timer = PhaseTimerV1::start_v1(PhaseV1::EnvelopeAndSelfCheck);
    let encoded = encode_zk_x509_credential_envelope_v1(
        proof_instance,
        consensus_public.credential_binding,
        &main_aggregate,
        &ca_subproof,
        &joint_openings
            .encode_v1()
            .map_err(|_| ZkX509CredentialProofErrorV1::MalformedEnvelope)?,
    )?;
    #[cfg(test)]
    prover_diagnostic::capture_public_unverified_candidate_v1(&encoded);
    let envelope = decode_zk_x509_credential_envelope_v1(&encoded).map_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1("credential-envelope-decode", &error);
        #[cfg(not(test))]
        let _ = error;
        ZkX509EngineErrorV1::ProverSelfCheckFailed
    })?;
    if envelope.proof_instance != proof_instance
        || envelope.public != consensus_public.credential_binding
    {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1(
            "credential-public-binding",
            &ZkX509CredentialProofErrorV1::PublicBindingMismatch,
        );
        return Err(ZkX509EngineErrorV1::ProverSelfCheckFailed);
    }
    verify_zk_x509_credential_subproofs_v1(
        envelope.proof_instance,
        statement,
        &consensus_public,
        envelope.main_aggregate,
        envelope.ca_subproof,
        &envelope.joint_openings,
    )
    .map_err(|error| {
        #[cfg(test)]
        prover_diagnostic::record_public_verifier_error_v1(
            "credential-subproof-self-check",
            &error,
        );
        #[cfg(not(test))]
        let _ = error;
        ZkX509EngineErrorV1::ProverSelfCheckFailed
    })?;
    #[cfg(test)]
    envelope_timer.complete_v1();
    Ok(encoded)
}
fn compiled_profile_fields_v1<'a>(
    sha_schedule_digests: &'a [[u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1],
    p256_schedule_digest: &'a [u8; 48],
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
-> Result<([[u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1], [u8; 48]), ZkX509EngineErrorV1> {
    let mut sha = [[0_u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1];
    for (disclosed_attributes, digest) in sha.iter_mut().enumerate() {
        *digest = zk_x509_sha_fixed_algebraic_schedule_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })?
        .descriptor_digest_v1()
        .to_bytes();
    }
    let p256 = zk_x509_p256_fixed_algebraic_schedule_v1()?
        .descriptor_digest_v1()
        .to_bytes();
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
#[doc(hidden)]
pub fn construct_zk_x509_compiled_profile_v1()
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
    use iroha_data_model::privacy::PrivacyProtocolIdV1;
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
        let sha_digests: [[u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x31 + shape).expect("five shapes"); 48]);
        let p256_digest = [0x41; 48];
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
                .windows(b"main-common-lde-log22".len())
                .any(|window| window == b"main-common-lde-log22")
        );
        assert!(
            !fields[17]
                .windows(b"common-lde-log25".len())
                .any(|window| window == b"common-lde-log25")
        );
    }
    #[test]
    fn compiled_profile_digest_exactly_binds_compact_ca_subproof_descriptor_pin() {
        let sha_digests: [[u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x71 + shape).expect("five shapes"); 48]);
        let p256_digest = [0x81; 48];
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
        let sha_digests: [[u8; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1] =
            core::array::from_fn(|shape| [u8::try_from(0x51 + shape).expect("five shapes"); 48]);
        let p256_digest = [0x61; 48];
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
    #[ignore = "explicit native profile capture after a reviewed relation/compiler change; does not accept or update a profile pin"]
    fn capture_compiled_profile_inputs_from_actual_schedules() {
        let (sha, p256) =
            compiled_profile_schedule_digests_v1().expect("all actual schedule compilers");
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let digest = independent_compiled_profile_digest_v1(&fields);
        assert_eq!(
            digest,
            recompute_zk_x509_compiled_profile_digest_v1().unwrap()
        );
        for (index, field) in fields.iter().enumerate() {
            eprintln!(
                "zk-x509-compiled-profile-field-{index}={}",
                hex::encode(field)
            );
        }
        eprintln!(
            "zk-x509-independent-compiled-profile-sha256={}",
            hex::encode(digest)
        );
        for disclosed_attributes in 0..5 {
            let schedule = zk_x509_sha_fixed_algebraic_schedule_v1(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes,
            })
            .unwrap();
            for (segment, child) in schedule.children_v1().iter().enumerate() {
                eprintln!(
                    "zk-x509-sha-schedule-shape-{disclosed_attributes}-segment-{segment}-width={}-atoms={}",
                    child.width_v1(),
                    child.atoms_v1().len()
                );
            }
        }
        match ZK_X509_COMPILED_PROFILE_DIGEST_V1 {
            Some(pin) if pin == digest => assert_eq!(
                construct_zk_x509_compiled_profile_v1().unwrap().digest(),
                digest
            ),
            Some(_) => assert_eq!(
                construct_zk_x509_compiled_profile_v1().err(),
                Some(ZkX509EngineErrorV1::CompiledProfileMismatch)
            ),
            None => assert_eq!(
                construct_zk_x509_compiled_profile_v1().err(),
                Some(ZkX509EngineErrorV1::CompiledProfileUnpinned)
            ),
        }
    }
    #[test]
    fn compiled_profile_rejects_unbound_sha_frame_prefix_relation() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let current = independent_compiled_profile_digest_v1(&fields);
        let mut predecessor = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_pre_frame_profile_fields_v1(&mut predecessor);
        let borrowed = predecessor.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old = independent_compiled_profile_digest_v1(&borrowed);
        assert_eq!(
            hex::encode(old),
            "ec254e72636e97e6440ba1bc27604ae15b018a0f3f3790454bbe0202ef5b2993"
        );
        assert_ne!(old, current);
        assert_eq!(Some(current), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
        let mut supplied =
            super::super::stark::construct_zk_x509_main_verifier_profile_v1().unwrap();
        supplied.compiled_profile_digest = old;
        assert!(super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err());
    }
    #[test]
    fn compiled_profile_rejects_missing_projection_and_authoritative_crl_bindings() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let current = independent_compiled_profile_digest_v1(&fields);
        let mut predecessor = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_pre_endpoint_binding_profile_fields_v1(&mut predecessor);
        let borrowed = predecessor.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old = independent_compiled_profile_digest_v1(&borrowed);
        assert_eq!(
            hex::encode(old),
            "9abb3d3f21cbfa362e3589b3b01a6f2a414a5423fae87074aa8344f5aa13c336"
        );
        assert_ne!(old, current);
        assert_eq!(Some(current), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
        let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
            .expect("current verifier profile");
        supplied.compiled_profile_digest = old;
        assert!(super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err());
    }

    #[test]
    fn historical_profile_restoration_is_independent_of_every_current_field() {
        let base = vec![Vec::new(); COMPILED_PROFILE_FIELD_COUNT_V1];
        let restorers: [fn(&mut [Vec<u8>]); 3] = [
            restore_pre_endpoint_binding_profile_fields_v1,
            restore_pre_frame_profile_fields_v1,
            restore_retired_public_terminal_profile_descriptors_v1,
        ];
        let expected = [
            "9abb3d3f21cbfa362e3589b3b01a6f2a414a5423fae87074aa8344f5aa13c336",
            "ec254e72636e97e6440ba1bc27604ae15b018a0f3f3790454bbe0202ef5b2993",
            "0312a22aad46561f42f28831baff280a072f34899284e354938c09a82e7adcdf",
        ];
        for (restore, expected) in restorers.into_iter().zip(expected) {
            let mut historical = base.clone();
            restore(&mut historical);
            let borrowed = historical.iter().map(Vec::as_slice).collect::<Vec<_>>();
            assert_eq!(
                hex::encode(independent_compiled_profile_digest_v1(&borrowed)),
                expected
            );
            for index in 0..COMPILED_PROFILE_FIELD_COUNT_V1 {
                let mut substituted = historical.clone();
                substituted[index] = vec![0xa5; index + 1];
                restore(&mut substituted);
                assert_eq!(
                    substituted, historical,
                    "historical field {index} inherited current bytes"
                );
            }
        }
        let mut pre_selector = base;
        restore_pre_frame_profile_fields_v1(&mut pre_selector);
        let marker =
            "-certificate-slot-output-producer-and-consumer-fixed-selector-equals-global-activity";
        let descriptor = core::str::from_utf8(&pre_selector[9]).unwrap();
        assert_eq!(descriptor.matches(marker).count(), 1);
        pre_selector[9] = descriptor.replace(marker, "").into_bytes();
        let borrowed = pre_selector.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_eq!(
            hex::encode(independent_compiled_profile_digest_v1(&borrowed)),
            "c6a43af732ff56b3036e0d90064da643cfa046496c3c11ddb085ebc8f1834ab4"
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
        if cfg!(feature = "privacy-release-evidence")
            || ZK_X509_COMPILED_PROFILE_DIGEST_V1 != Some(independent)
        {
            // These are public profile descriptors, never witness material.
            // Release evidence retains the genuine native framing inputs even
            // when the current stored pin already matches the constructor.
            eprintln!(
                "zk-x509-independent-compiled-profile-sha256={}",
                hex::encode(independent)
            );
            for (index, field) in fields.iter().enumerate() {
                eprintln!(
                    "zk-x509-compiled-profile-field-{index}={}",
                    hex::encode(field)
                );
            }
        }
        assert_eq!(ZK_X509_COMPILED_PROFILE_DIGEST_V1, Some(independent));
        assert_eq!(
            construct_zk_x509_compiled_profile_v1()
                .expect("release-pinned profile")
                .digest(),
            independent
        );
    }
    // Exact public manifest emitted by the native 29-field capture for 9abb3d3f.
    // Test-only rejection material: no production reader or compatibility path.
    // Restore every field before older overrides so later relation changes cannot
    // silently manufacture a hybrid historical profile.
    fn restore_pre_endpoint_binding_profile_fields_v1(fields: &mut [Vec<u8>]) {
        assert_eq!(fields.len(), COMPILED_PROFILE_FIELD_COUNT_V1);
        fields[0] = b"iroha-zk-x509-stark-p256-v1".to_vec();
        fields[1] = b"iroha-native-rust:strict-der:rfc5280-p256-sha256:private-chain-crl-ownership:goldilocks-stark:v1".to_vec();
        fields[2] = b"rfc5280-closed-v1:der-only:v3:serial-positive-nonzero-max20:exact-name-der:name-oids-only-c-o-ou-cn:no-duplicate-name-attributes:c-printablestring-two-uppercase-ascii:o-ou-cn-utf8string-or-printablestring:max-name-value256:disclosed-attribute-hash-content-octets-only:utf8-well-formed-no-u0000-u001f-u007f-u009f:utc-time-1970-2049:generalized-time-2050-9999:seconds-z-no-fraction:certificate-validity-inclusive:no-unique-id:ecdsa-sha256-absent-params:spki-id-ec-public-key-prime256v1-uncompressed:extensions-exact-order-aki-ski-keyusage-basicconstraints-and-optional-leaf-eku:no-duplicates-no-unknown:aki-ski-linked:bc-ku-eku-critical:ca-keycertsign-and-crlsign-only:ca-pathlen-required-explicit:direct-leaf-issuer-crlsign:root-self-name-self-signature:leaf-revocation-only:no-nameconstraints:no-policy-mapping:no-certificate-policies:no-policy-constraints:no-inhibit-any-policy:no-alt-names:no-distribution-points:no-freshest-crl".to_vec();
        fields[3] = b"rfc5280-crl-closed-v1:der-only:v2:ecdsa-sha256-absent-params:aki-required-noncritical:crl-number-required-noncritical-u64-strictly-increasing:no-delta-crl-indicator:no-issuing-distribution-point:no-freshest-crl:no-indirect:no-partition:no-entry-extensions:complete-base-crl:max64-revoked-entries".to_vec();
        fields[4] = b"leaf-only:one-issuer-per-certificate-policy:complete-base-crl:no-delta:no-indirect:no-partition:no-distribution-point".to_vec();
        fields[5] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|record_epoch:u64-be|trust_store_digest:bytes32|ca_membership_root:bytes32-compact-depth12|ca_membership_root_epoch:u64-be|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256".to_vec();
        fields[6] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|policy_id:bytes32|record_epoch:u64-be|policy_digest:bytes32|required_key_usage:u8-mask-bits0through3-upper-zero|required_extended_key_usages:u8-count-plus-strictly-sorted-u8-codes-max3|required_disclosed_attribute_indices:u8-count-plus-strictly-sorted-u8-indices-max4|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256".to_vec();
        fields[7] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|certificate_policy_id:bytes32|record_epoch:u64-be|crl_number:u64-be-strictly-increasing|crl_der_digest:domain-framed-sha256-of-complete-exact-signed-der|issuer_spki_digest:domain-framed-sha256|this_update_unix_seconds:u64-be|next_update_unix_seconds:u64-be|complete-crl-entry-nonrevocation:proved-in-rfc5280-stark-no-secondary-root|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256".to_vec();
        fields[8] = b"cert-and-crl:ecdsa-with-sha256-over-exact-tbs:minimal-der-rs-valid-range-high-or-low-s|wallet:ecdsa-p256-prehash-over-exact-32-byte-ownership-digest:minimal-der-rs-valid-range-low-s".to_vec();
        fields[9] = b"proof-instance=required-public32-after-version-count-before-consensus-context:checked-disjoint-entropy-before-first-commitment:all-zero-canonical|dynamic-hash-profile-suffix=00-X5I1-family-u8-nonce32:joint0-main1-ca2:all-commitments-and-transcript-operations:fixed-compiler-metadata-unscoped|field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-main24-ca108-joint-original-openings-and-exactly-one-X5M1-and-one-X5C1-v1|x5m1=version1+31-original-key-DEEP-values+length-delimited-aggregate-no-fixed-sidecar|main-public-terminal-records=0|ca-public-terminal-records=0|main-frame-bytes=1002-includes992-key-DEEP-values|ca-frame-bytes=10|joint-openings=132-canonical-Fp4-values4224|ca-private-endpoint-links=108-original-polynomial-quotients-MAIN-composition|private-der-rfc-source-and-p256-terminal-scalars=364-committed-air-only|main-private-endpoint-links=192-linear-quotients-original-masks-shared-native-points|main-key-byte-joins=12-blocks647-equalities-rfc-to-io-and-real-p256-root-powers2,8-max-quotient-degree538744|main-sha-digest-joins=5-blocks40-u32-equalities-unreduced-be-four-byte-real-p256-root-power32-max-quotient-degree2155224|main-sha-rfc-private-union=16-constant-native-bridges+16-segment-quartic-links+4-role-quartic-links-original-masks-existing-aux-DEEP-no-extra-openings-degree2104411|main-link-alpha-order=local-component-history+both-original-aux-roots+fresh-local-alphas+192-ordered-private-endpoint-link-descriptors+endpoint-link-alphas+17-key-and-digest-join-descriptors+key-and-digest-alphas+20-private-sha-union-descriptors+sha-union-alphas+108-ordered-CA-private-link-descriptors+CA-link-alphas|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-roots=one-joined-base-and-one-joined-aux|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log12-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|p256-binding-sink-degree=3-including-fixed-selectors|sha-capacity-and-call-degree=6-including-fixed-selectors|sha-digest-address=polynomial-select|sha-canonical-frame-prefix-and-static-field-headers-with-carried-private-crl-length|sha-fixed-algebraic-width=536-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-polynomials=verifier-derived-at-deep-and-native-translates|shared-x5b1-challenges=single-joined-main-base-root+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=1816|ca-trace-hiding-coefficients=2100|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=9over64|main-fri-blowup=8|ca-lde-log2=16|fri-queries=136-distinct-without-replacement|composition-fp4-lanes=1|quotient-chunk-stride=fri-degree-cap-minus137|quotient-chunk-masks=137-independent-fp4-coefficients-adjacent-cancellation|main-quotient-mask-order=all-local-and-private-link-contributions-then-blind-before-root|fri-batching-m=3|binary-fold-arity=2|fri-folding=2|fri-leaves=ordered-low-high-pairs|main-fri-terminal-length=1024-degree143|ca-fri-terminal-length=1024-degree143|deep-points=one-joint-point-after-both-original-aux-composition-and-FRI-mask-roots:both-local-checkpoints-bound:current+next+MAIN31-key-and-digest:5-power8+6-power2+20-power32+MAIN24-translated-aux+CA108-translated-aux:combined-domain-admission:both-local-DEEP-and-MAIN31-then-ordered132-values-bound-before-independent-local-mixes|ca-deep-constraints=all1363-fp4-verifier-fixed-polynomials-current-only-query-rows|main-deep-constraints=all49-fp4-native-vanishing-six-chunk-recomposition-verifier-fixed-polynomials-current-only-query-rows|grinding-bits=20|target-soundness-bits=128|rfc5280-temporal-air=base285-aux280-fixed147-constraints2145-degree4-basic-constraints-fixed3x12-signed-parent-complete-canonical-u32-and-32bit-nonwrapping-slack-source-node-root-kind-active-gated-helper-fixed-eight-equality-and-fifteen-embedded-endpoint-slots-complete-nonempty-original-node-spans-live-prefix-count-typed-copy-domains103-104-affine-one-plus-tuple-for-shared-domains90-98-99-102-103-104-affine-one-plus-tuple-for-shared-profile-domains96-97-optional-slot-canonical-zero-exact-extension-oid-ordinal-ku-document-and-critical-cardinality-eku-canonical-profile-uuid-oids-five-key-and-five-complete-spki-outputs-der-node-byte-lookups-key-length66-offset1-spki-content89-total91-offset0-nine-variable-tbs-crl-signature-byte-and-canonical-u16-in-eight-byte-big-endian-length-pairs-live-prefix-count-node-continuity-source-start-helper-projection-leaf-serial-magnitude-sign-octet-nonzero-first-byte-big-endian-length-selected-original-subject-oid-value-provenance-with-complete-name-oid-census-three-original-classes-disjoint-document-stride12-keys1through48-six-bit-positive-gaps-uniqueness-nonempty-bounded-rdns-and-complete-original-value-census-country-printable-utf8-dfa-output-metadata-six-verifier-fixed-equations-certificate-slot-output-producer-and-consumer-fixed-selector-equals-global-activity-authenticated72-times-73-relations-38bit-slack-affine-loglookup-39-relations|rbr-budget-bits=157|joint-configuration-bound=8-per-domain,64-paired|joint-point-exclusion-bound=3277643777|joint-cleared-degree-bounds=19398656,40960|joint-algebraic-error-bits=219|base-collision-configuration-factor=64|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=9412944|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|shared-stark-v1=q136-blowup8-digest384-fp4-blocked-pending-independent-qualification|activation=unavailable".to_vec();
        fields[10] = b"zk-x509-main-assembly-v1-incompatible:strict-reference-prover-invariant:exact-der-rfc-projection-ca-sources:29-verifier-positioned-sha-witnesses:five-p256-equations:selected-input-byte-pairs40960to41119-bound-to-writers47-48-52-53-and-digest-word:small-multiplicity-polynomial012:optional-slot2-rfc-zero-source-and-public-valid-dummy-selector:statement-compiled-deduplicated-sequential-byte-io:exact-witness-declaration-replay:logical-active-row-census:exact49-registrations:no-host-verification-substitute:public-terminal-replay-and-private-committed-endpoint-air-links=complete:no-unmasked-der-rfc-source-or-p256-terminal-scalars:activation=governance-gated".to_vec();
        fields[11] = b"zk-x509-main-io-declarations-v1-incompatible:statement-only:three-spki-prefix:serial-length+padded:per-disclosure-attribute-length+padded:three-certificate-tbs-pairs:optional-certificate-selector:three-certificate-signature-triples:crl-tbs+complete-crl+signature:issuer+leaf-keys:issuer+root-spki:active-projection-sha-triples:public-digests-verifier-fixed:declarations=40+5d:logical-rows=55922+4736d:max74866:fixed-capacity262144:first-release".to_vec();
        fields[12] = b"zk-x509-credential-pre-aux-v1:X5B1:version1:profile=main-profile-digest+ca-profile-digest:public=consensus-context-digest+ca-public-digest:base-roots=exact-main1-joined-ordered-native-log5,8,15,16,18,19-then-ca1-log12:post-base-challenges=exact272-goldilocks-fields:01-sha-call=4lanes*(beta,call,role,slot,kind,word,value)=28:02-rfc=4lanes*tuple12=48:03-projection=4lanes*(copy-beta,copy-gamma,compaction-active,compaction-invocation,compaction-position,compaction-value,compaction-gamma)=28:04-io=4lanes*(beta,channel,offset,value,is-write)=20:05-der=4lanes*(tuple12-then-byte-lookup)=52:06-sha-word-memory=4lanes*(beta,address,value,is-write)=16:07-sha-word-base-fold=4:08-p256-value=4lanes*7=28:09-p256-cross=4lanes*4=16:10-p256-scalar=4lanes*5=20:11-p256-arithmetic-copy=4lanes*3=12:lane-major-within-each-family:one-private-opaque-main-post-base-capability:no-raw-constructor:bind-post-challenge-state-and-all272-canonical-challenges-into-each-local-transcript-before-aux-roots:no-caller-selected-binding".to_vec();
        fields[13] = b"byte-memory-permutation=complete|strict-der-segment=complete|projection-segment=complete|shared-current-next-deep-ali=complete|rfc5280-base-row-provider=complete|rfc5280-aggregate-and-eighteen-independent-output-role-products=complete|rfc5280-private-source-and-CA-original-polynomial-air-links=complete|sha-call-witness-assembly-and-terminal-binding=complete|p256-witness-assembly-and-private-committed-terminal-air-links=complete|compact-ca-subproof=complete|full-49-registration-prover-and-verifier=complete|combined-main-ca-envelope=complete|consensus-verifier-integration=complete|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|activation=unavailable-qualification".to_vec();
        fields[14] =
            hex::decode("1dab24a7dd4c6061bb38e15865dfdf5be701671697a7ba669455c778ddc14605")
                .expect("authenticated native profile field 14");
        fields[15] = b"sha256-local-air-v1:canonical-padding:private-message-length:word-input-bits-le:sha256-bytes-be:boolean-and-xor-full-adder:gates-per-block=55552:fixed-canonical-topology:acyclic-single-assignment-wire-addresses:mod2^32-carry-discard:output-digest-reconstruction:global-wire-copy-and-cross-segment-binding=complete-via-sha256-word-air+sha-call-bus-stark".to_vec();
        fields[16] = b"sha256-word-air-v1-incompatible:u32-range-row=packed-plus32bits:sigma-degree3:choose-degree2:majority-degree3:add-up-to5-plus-u32-constant:carry-3bits:local-rows-per-block=1728:local-initial-rows=8:word-copy=four-independent-transcript-challenged-address-value-write-grand-products:sorted-address-step-0-or1:exactly-one-write-per-address:read-value-equals-write:memory-rows-per-block=2136:memory-fixed-rows=16:fixed-canonical-topology:physical-segment-offset-and-copy-product-continuations:shared-sha-call-bus-binding-required".to_vec();
        fields[17] = b"zk-x509-sha-call-bus-stark-v1-incompatible:29-fixed-capacity-calls=cert-tbs[3]+crl-tbs+framed-complete-signed-crl+projection[7]+issuer-spki+trust-record+policy-record+crl-record+compact-ca-leaf+compact-ca-node[12]:max-blocks616:word-rows1972128=compression655424+local-init232+local-digest232+memory1316240:four-log19-segments-whole-call-packed-active-rows480288,521952,521696,448192-no-cross-segment-call-transition:base89=word-capacity76+proof-bound-rfc-raw-length-bits13:aux78=word-capacity54+input-products4+digest-products4+rfc-consumer-products16:fixed134=word72+call-segment-length-control9+thirteen-verifier-one-hot-compact-ca-call-selectors+four-field-native-rfc-event-descriptors-of-width6+canonical-frame-byte-mask-constant-and-private-length-high-low16:constraints585=local-private-running-products:MAIN108-cross-proof-original-polynomial-quotients:degree6-including-fixed-selectors:polynomial-digest-address=digest*dynamic+(1-digest)*fixed:base-two-chunks-aux-two-chunks-per-segment:same-log-bucket-base356-aux312-base-chunks8-aux-chunks8:private-exact-length-unique-padding-transition-across-blocks-and-active-block-prefix:fine-grained-message-cap-and-fixed-role-length-enforcement:frozen-canonical-inactive-computation-memory-and-mask-suffix:selected-digest-from-unique-final-active-block:inactive-chain-and-projection-slots-canonical-sha-empty-dummy:address=(call,role,slot,input-or-digest,word):four-independent-domain-separated-goldilocks-lanes:separate-word-memory-and-call-challenge-families:segment-continuous-private-source-digest-and-rfc-products:no-public-segment-products:main-16-quartic-native-endpoint-links-to-private-rfc-bridges:cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-aux=zero:word-capacity-recurrence=local-compute+digest+memory-call-last:compact-ca-calls16through28-bind-original-source-and-digest-endpoints-through-MAIN108-private-polynomial-quotients-without-product-division:rfc-consumer-products-derived-algebraically-from-committed-message-bits-masks-and-verifier-fixed-event-descriptors:four-byte-streams-total-degree5-recurrences-including-fixed-selectors:all-framed-roles-verifier-compiled-domain-and-static-field-headers-and-protocol-constants:frame-bytes-must-be-message-bytes:private-variable-payloads-remain-unfixed:crl-frame-u64-length-equals-carried13bit-raw-length:proof-bound-u64-raw-length-consumers:certificate-tbs-crl-tbs-framed-complete-crl-and-framed-issuer-spki-channels:three-governance-self-digests-explicit-sha-field-frames:no-host-branch-on-opened-fixed-columns:main-common-lde-log22:protocol2-independent-per-lane-fri-mask-oracles:max-encoded-sha-proof2836064:stream-one-call-at-a-time:on-demand-full-row-widening-without-duplicated-aux-or-fixed-vectors".to_vec();
        fields[18] = b"zk-x509-cross-segment-io-v1:fixed-sequential-channel-ids:fixed-producer-and-consumer-endpoints:byte-offsets-contiguous:one-write-per-channel-byte:all-reads-equal-write:public-input-reads-verifier-fixed:four-independent-lane-and-coordinate-labelled-transcript-challenged-channel-offset-value-write-grand-products:max-accesses=byte-memory-segment-capacity:first-release".to_vec();
        fields[19] = b"trusted-authoritative-state+trusted-block-time+taira-consensus-limits+exact-norito-zk-x509-witness-v1-flags0+strict-reference-relation".to_vec();
        fields[20] = b"zk-x509-fixed-algebraic-v1-incompatible:verifier-derived-only:no-proof-fixed-material:no-artifact:no-merkle:additive-canonical-atoms=affine-range+repeated-affine-stride+sparse:overlap=goldilocks-field-addition:exact-duplicate-atoms-rejected:semantically-equivalent-alternate-decompositions-have-distinct-descriptor-digests:goldilocks-modulus=0xffffffff00000001:native-root-domain:generator-shifted-lde-coset:coset-disjoint-from-lde-subgroup:query-index-derived-point:residue-grouped-barycentric-lagrange:batch-inverted-native-denominators:cyclic-prefix-affine-sums:repeated-sums=generic-gcd-cycles+reduced-stride-modular-inverse+cyclic-weight-and-ordinal-prefixes+per-stride-min-direct-occurrence-work-vs-native-prefix-work:one-column-native-streaming:no-native-times-width-or-lde-table:bounded-native20-lde25-blowup8-width536-atoms65536-queries272-output-fields145792-work2pow28:digest=sha3-384-opaque48:wire=X5K1+u16be-version1+u16be-header24+native-log2-u8+lde-log2-u8+width-u16be+atom-count-u32be+coset-shift-u64be+canonical-variable-atoms:first-release-no-legacy".to_vec();
        fields[21] = b"zk-x509-sha-fixed-algebraic-compiler-v1-incompatible:public-shapes=disclosed-attributes0through4:four-independent-physical-log19-generic-children:child-widths=134,134,134,134:combined-width536:segment-major-column-order:each-child-generic-cap65536:typed-composite-digest=sha3-384-opaque48-binds-disclosure-shape+profile+ordered-widths+ordered-child-digests:row-major-child-opening-concatenation:typed-zero-capacity-sha-word-circuit-topology:word-operation-and-execution-sorted-memory-walk:authoritative-local-row-event-order-replay:definition-writes-immediately-before-consuming-operation:operation-input-reads-then-output-write:eight-digest-reads-last:derived-execution-sort-must-equal-circuit-canonical-sorted-memory:execution-write-axis-key=typed-word-phase(initial|input|expansion3|round8|final):compute+execution+sorted-memory-call-axis-transpose-on-exact-maximal-contiguous-same-segment-same-geometry-runs-iff-calls-strictly-greater-than-blocks:block-row-axis-on-ties:boolean-topology-three-way-exact-atom-planner(block-or-call=2048*min(calls,blocks)|round=32*blocks*calls|block-gap=416*calls):strict-lower-only:old-then-round-wins-ties:block-gap-axis-one-stride14-hull+12-negative-stride1064-gap-residues-per-lane:operation-read-typed-block+phase(expansion6|round18|final2)+read-slot-axis-with-exact-per-call-cost44*blocks-2:operation-read-axis-transpose-iff-exact-cost-strictly-less-than-existing-block-or-call-axis:sorted-memory-typed-initial-or-block+word-phase(initial|input|expansion3|round8|final)+access-occurrence-axis:sorted-memory-phase-axis-on-every-exact-geometry-run-iff-phase-cost=298*blocks*calls-is-strictly-less-than-existing-axis-cost(call=4952*blocks+32|block=(4952+32)*calls):old-axis-wins-ties:call-axis-key=local-column+family+block(initial-or-sha-index)+word-position+occurrence:remaining-sorted-memory-nontransposed-series-maximal-across-ordered-calls:no-native-row-matrix:no-lde-matrix:no-artifact:no-merkle-root:no-proof-supplied-fixed-values:affine+repeated+sparse-atoms:generator-coset-log19-to-log22:call-role-slot-boundaries+compact-ca-selectors+field-native-rfc-events+physical-padding:exact-shape-derived-rfc-channel-offsets:canonical-domain-and-static-field-frame-byte-mask-constant-and-raw-length-high-low-columns:all-six-formerly-reconstructed-word-columns-native:first-release".to_vec();
        fields[22] = hex::decode("865b5d46d69c73eb3ac278691b9d987520f1d8c05c00e0a1202fb771901e3c84aeba08b3ac60b2f5799f3b715532645d").expect("authenticated native profile field 22");
        fields[23] = hex::decode("513a3d19dc70af7fd5c99bf722e949efe03369c219b109bbc5d6a72744510d1b1a19d8650208c899686e40aab0a87be5").expect("authenticated native profile field 23");
        fields[24] = hex::decode("f3c6539fe9bda0bcacdc793b39155f09214b90e222c63be9d0c54688f468402e5bf9787c549ef23af8f9c5cabb729d29").expect("authenticated native profile field 24");
        fields[25] = hex::decode("a8d38d93fc6e7e2cce6eab914fbc1608dc258cc16f1be4c952721500ce13a81e86d1f2d230950f334e8a8327602ec88f").expect("authenticated native profile field 25");
        fields[26] = hex::decode("5d6ef8dc891a6ffa0f81f26e6efa7f94206e3b48512e6678823a97d9154959925f8dc08e260ac549182917066b08dc99").expect("authenticated native profile field 26");
        fields[27] = b"zk-x509-p256-fixed-algebraic-v1-incompatible:native-log19:generator-coset-lde-log22:width404:six-schedules=certificate-arithmetic134+wallet-arithmetic134+certificate-execution46+wallet-execution46+certificate-sorted22+wallet-sorted22:typed-composite-children=134,134,46,46,22,22:each-child-generic-cap65536:composite-digest=sha3-384-opaque48-binds-profile+ordered-widths+ordered-child-digests:row-major-child-opening-concatenation:aliases-exactly15=signatures0through4-times-arithmetic0+value-execution0+value-sorted1:signatures0through3-certificate-role:signature4-wallet-role:closed-value-free-topology-only:writer-source=external-sink-plus-selected-Qx-Qy-r-s64-limb-uses:multiplicity-small0through2+64+65+129:additive-affine+repeated-affine+sparse:operation-metadata-plan=min-exact-row-axis-vs-canonical-call-axis:row-axis-on-tie:call-segments=14x43+64x222+row-tail18:sorted-active-factors=725504-distinct-from-execution-logical-factors949312:sorted-equal-read-runs=min-exact-relative-factor-axis-vs-per-value-axis:relative-factor-axis-on-tie:sorted-whole-plan=min-exact-global-local-vs-phase-hybrid:global-local-on-tie:phase-hybrid=prefix893-local+min-local-vs13x43-phase+scalar-boundary222-local+min-local-vs63x222-phase+tail18-local:pinned-boundary-extents=1712,9984:pinned-repeated-extents=1888,10176:local-on-phase-tie:no-native-matrix:no-lde-table:no-artifact:no-merkle:no-proof-fixed-bytes:first-release".to_vec();
        fields[28] = hex::decode("cdc1a3f8c743e838794750963671c8e3c5e7c5e56849ec4356da660d57533377252c367780787451582b60bcf5773755").expect("authenticated native profile field 28");
    }

    // Historical profile tests must use the exact old public descriptors,
    // rather than accidentally mixing a new closure with an old SHA-padding pin.
    // Fixed rejection-fixture bytes from the authenticated pre-catalog-change
    // native 29-field manifest. These are never accepted by production code.
    // Reusing current schedule digests would synthesize a different old profile.
    fn restore_historical_catalog_schedule_fields_v1(fields: &mut [Vec<u8>]) {
        restore_pre_frame_profile_fields_v1(fields);
        assert_eq!(fields.len(), COMPILED_PROFILE_FIELD_COUNT_V1);
        fields[9] = b"proof-instance=required-public32-after-version-count-before-consensus-context:checked-disjoint-entropy-before-first-commitment:all-zero-canonical|dynamic-hash-profile-suffix=00-X5I1-family-u8-nonce32:joint0-main1-ca2:all-commitments-and-transcript-operations:fixed-compiler-metadata-unscoped|field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-main24-ca108-joint-original-openings-and-exactly-one-X5M1-and-one-X5C1-v1|x5m1=version1+31-original-key-DEEP-values+length-delimited-aggregate-no-fixed-sidecar|main-public-terminal-records=0|ca-public-terminal-records=0|main-frame-bytes=1002-includes992-key-DEEP-values|ca-frame-bytes=10|joint-openings=132-canonical-Fp4-values4224|ca-private-endpoint-links=108-original-polynomial-quotients-MAIN-composition|private-der-rfc-source-and-p256-terminal-scalars=364-committed-air-only|main-private-endpoint-links=192-linear-quotients-original-masks-shared-native-points|main-key-byte-joins=12-blocks647-equalities-rfc-to-io-and-real-p256-root-powers2,8-max-quotient-degree538744|main-sha-digest-joins=5-blocks40-u32-equalities-unreduced-be-four-byte-real-p256-root-power32-max-quotient-degree2155224|main-sha-rfc-private-union=16-constant-native-bridges+16-segment-quartic-links+4-role-quartic-links-original-masks-existing-aux-DEEP-no-extra-openings-degree2104411|main-link-alpha-order=local-component-history+both-original-aux-roots+fresh-local-alphas+192-ordered-private-endpoint-link-descriptors+endpoint-link-alphas+17-key-and-digest-join-descriptors+key-and-digest-alphas+20-private-sha-union-descriptors+sha-union-alphas+108-ordered-CA-private-link-descriptors+CA-link-alphas|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-roots=one-joined-base-and-one-joined-aux|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log12-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|p256-binding-sink-degree=3-including-fixed-selectors|sha-capacity-and-call-degree=6-including-fixed-selectors|sha-digest-address=polynomial-select|sha-fixed-algebraic-width=472-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-polynomials=verifier-derived-at-deep-and-native-translates|shared-x5b1-challenges=single-joined-main-base-root+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=1816|ca-trace-hiding-coefficients=2100|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=9over64|main-fri-blowup=8|ca-lde-log2=16|fri-queries=136-distinct-without-replacement|composition-fp4-lanes=1|quotient-chunk-stride=fri-degree-cap-minus137|quotient-chunk-masks=137-independent-fp4-coefficients-adjacent-cancellation|main-quotient-mask-order=all-local-and-private-link-contributions-then-blind-before-root|fri-batching-m=3|binary-fold-arity=2|fri-folding=2|fri-leaves=ordered-low-high-pairs|main-fri-terminal-length=1024-degree143|ca-fri-terminal-length=1024-degree143|deep-points=one-joint-point-after-both-original-aux-composition-and-FRI-mask-roots:both-local-checkpoints-bound:current+next+MAIN31-key-and-digest:5-power8+6-power2+20-power32+MAIN24-translated-aux+CA108-translated-aux:combined-domain-admission:both-local-DEEP-and-MAIN31-then-ordered132-values-bound-before-independent-local-mixes|ca-deep-constraints=all1363-fp4-verifier-fixed-polynomials-current-only-query-rows|main-deep-constraints=all49-fp4-native-vanishing-six-chunk-recomposition-verifier-fixed-polynomials-current-only-query-rows|grinding-bits=20|target-soundness-bits=128|rfc5280-temporal-air=base285-aux280-fixed147-constraints2145-degree4-basic-constraints-fixed3x12-signed-parent-complete-canonical-u32-and-32bit-nonwrapping-slack-source-node-root-kind-active-gated-helper-fixed-eight-equality-and-fifteen-embedded-endpoint-slots-complete-nonempty-original-node-spans-live-prefix-count-typed-copy-domains103-104-optional-slot-canonical-zero-exact-extension-oid-ordinal-ku-document-and-critical-cardinality-eku-canonical-profile-uuid-oids-five-key-and-five-complete-spki-outputs-der-node-byte-lookups-key-length66-offset1-spki-content89-total91-offset0-nine-variable-tbs-crl-signature-byte-and-canonical-u16-in-eight-byte-big-endian-length-pairs-live-prefix-count-node-continuity-source-start-helper-projection-leaf-serial-magnitude-sign-octet-nonzero-first-byte-big-endian-length-selected-original-subject-oid-value-provenance-with-complete-name-oid-census-three-original-classes-disjoint-document-stride12-keys1through48-six-bit-positive-gaps-uniqueness-nonempty-bounded-rdns-and-complete-original-value-census-country-printable-utf8-dfa-output-metadata-six-verifier-fixed-equations-authenticated72-times-73-relations-38bit-slack-affine-loglookup-39-relations|rbr-budget-bits=157|joint-configuration-bound=8-per-domain,64-paired|joint-point-exclusion-bound=3277643777|joint-cleared-degree-bounds=19398656,40960|joint-algebraic-error-bits=219|base-collision-configuration-factor=64|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=9412944|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|shared-stark-v1=q136-blowup8-digest384-fp4-blocked-pending-independent-qualification|activation=unavailable".to_vec();
        fields[17] = b"zk-x509-sha-call-bus-stark-v1-incompatible:29-fixed-capacity-calls=cert-tbs[3]+crl-tbs+framed-complete-signed-crl+projection[7]+issuer-spki+trust-record+policy-record+crl-record+compact-ca-leaf+compact-ca-node[12]:max-blocks616:word-rows1972128=compression655424+local-init232+local-digest232+memory1316240:four-log19-segments-whole-call-packed-active-rows480288,521952,521696,448192-no-cross-segment-call-transition:base89=word-capacity76+proof-bound-rfc-raw-length-bits13:aux78=word-capacity54+input-products4+digest-products4+rfc-consumer-products16:fixed118=word72+call-segment-length-control9+thirteen-verifier-one-hot-compact-ca-call-selectors+four-field-native-rfc-event-descriptors-of-width6:constraints564=local-private-running-products:MAIN108-cross-proof-original-polynomial-quotients:degree6-including-fixed-selectors:polynomial-digest-address=digest*dynamic+(1-digest)*fixed:base-two-chunks-aux-two-chunks-per-segment:same-log-bucket-base356-aux312-base-chunks8-aux-chunks8:private-exact-length-unique-padding-transition-across-blocks-and-active-block-prefix:fine-grained-message-cap-and-fixed-role-length-enforcement:frozen-canonical-inactive-computation-memory-and-mask-suffix:selected-digest-from-unique-final-active-block:inactive-chain-and-projection-slots-canonical-sha-empty-dummy:address=(call,role,slot,input-or-digest,word):four-independent-domain-separated-goldilocks-lanes:separate-word-memory-and-call-challenge-families:segment-continuous-private-source-digest-and-rfc-products:no-public-segment-products:main-16-quartic-native-endpoint-links-to-private-rfc-bridges:cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-aux=zero:word-capacity-recurrence=local-compute+digest+memory-call-last:compact-ca-calls16through28-bind-original-source-and-digest-endpoints-through-MAIN108-private-polynomial-quotients-without-product-division:rfc-consumer-products-derived-algebraically-from-committed-message-bits-masks-and-verifier-fixed-event-descriptors:four-byte-streams-total-degree5-recurrences-including-fixed-selectors:proof-bound-u64-raw-length-consumers:certificate-tbs-crl-tbs-framed-complete-crl-and-framed-issuer-spki-channels:three-governance-self-digests-explicit-sha-field-frames:no-host-branch-on-opened-fixed-columns:main-common-lde-log22:protocol2-independent-per-lane-fri-mask-oracles:max-encoded-sha-proof2836064:stream-one-call-at-a-time:on-demand-full-row-widening-without-duplicated-aux-or-fixed-vectors".to_vec();
        // Freeze the predecessor fixed evaluator/compiler semantics as well.
        fields[20] = b"zk-x509-fixed-algebraic-v1-incompatible:verifier-derived-only:no-proof-fixed-material:no-artifact:no-merkle:additive-canonical-atoms=affine-range+repeated-affine-stride+sparse:overlap=goldilocks-field-addition:exact-duplicate-atoms-rejected:semantically-equivalent-alternate-decompositions-have-distinct-descriptor-digests:goldilocks-modulus=0xffffffff00000001:native-root-domain:generator-shifted-lde-coset:coset-disjoint-from-lde-subgroup:query-index-derived-point:residue-grouped-barycentric-lagrange:batch-inverted-native-denominators:cyclic-prefix-affine-sums:repeated-sums=generic-gcd-cycles+reduced-stride-modular-inverse+cyclic-weight-and-ordinal-prefixes+per-stride-min-direct-occurrence-work-vs-native-prefix-work:one-column-native-streaming:no-native-times-width-or-lde-table:bounded-native20-lde25-blowup8-width472-atoms65536-queries272-output-fields128384-work2pow28:digest=sha3-384-opaque48:wire=X5K1+u16be-version1+u16be-header24+native-log2-u8+lde-log2-u8+width-u16be+atom-count-u32be+coset-shift-u64be+canonical-variable-atoms:first-release-no-legacy".to_vec();
        fields[21] = b"zk-x509-sha-fixed-algebraic-compiler-v1-incompatible:public-shapes=disclosed-attributes0through4:four-independent-physical-log19-generic-children:child-widths=118,118,118,118:combined-width472:segment-major-column-order:each-child-generic-cap65536:typed-composite-digest=sha3-384-opaque48-binds-disclosure-shape+profile+ordered-widths+ordered-child-digests:row-major-child-opening-concatenation:typed-zero-capacity-sha-word-circuit-topology:word-operation-and-execution-sorted-memory-walk:authoritative-local-row-event-order-replay:definition-writes-immediately-before-consuming-operation:operation-input-reads-then-output-write:eight-digest-reads-last:derived-execution-sort-must-equal-circuit-canonical-sorted-memory:execution-write-axis-key=typed-word-phase(initial|input|expansion3|round8|final):compute+execution+sorted-memory-call-axis-transpose-on-exact-maximal-contiguous-same-segment-same-geometry-runs-iff-calls-strictly-greater-than-blocks:block-row-axis-on-ties:boolean-topology-three-way-exact-atom-planner(block-or-call=2048*min(calls,blocks)|round=32*blocks*calls|block-gap=416*calls):strict-lower-only:old-then-round-wins-ties:block-gap-axis-one-stride14-hull+12-negative-stride1064-gap-residues-per-lane:operation-read-typed-block+phase(expansion6|round18|final2)+read-slot-axis-with-exact-per-call-cost44*blocks-2:operation-read-axis-transpose-iff-exact-cost-strictly-less-than-existing-block-or-call-axis:sorted-memory-typed-initial-or-block+word-phase(initial|input|expansion3|round8|final)+access-occurrence-axis:sorted-memory-phase-axis-on-every-exact-geometry-run-iff-phase-cost=298*blocks*calls-is-strictly-less-than-existing-axis-cost(call=4952*blocks+32|block=(4952+32)*calls):old-axis-wins-ties:call-axis-key=local-column+family+block(initial-or-sha-index)+word-position+occurrence:remaining-sorted-memory-nontransposed-series-maximal-across-ordered-calls:no-native-row-matrix:no-lde-matrix:no-artifact:no-merkle-root:no-proof-supplied-fixed-values:affine+repeated+sparse-atoms:generator-coset-log19-to-log22:call-role-slot-boundaries+compact-ca-selectors+field-native-rfc-events+physical-padding:exact-shape-derived-rfc-channel-offsets:all-six-formerly-reconstructed-word-columns-native:first-release".to_vec();
        for (field, encoded) in [
            (
                22,
                "d2f034439a77e29c78931c0994aaf83cfd9b10e2b44504b9c7ced6142459ad5abf60ad6edaf5b90d79c7682f0aa81a01",
            ),
            (
                23,
                "f25dd4fdd2260511779086bba2419aeab283927874a2398f5857578740e0432db5e48ce48fdc50bb6af33ab5a8179433",
            ),
            (
                24,
                "6b3585b3ae53e81a01e46815ffecc2076e7c9c008b1ff2a5d1ae2581973501830c80c6cdb7961063bca7e9a069769b49",
            ),
            (
                25,
                "f5b536183b0918b59b9b772f8b836f2f7a3672172aae2fe68ee387f7ec776ed7c23a5b6e1a8b73c17c7bffc56715ad2a",
            ),
            (
                26,
                "c7ecda6b0db3b644c0b94e51ca830462e360f16b43ed47773f980e81a704e60440528812e0412a3159c92336008cc0b3",
            ),
            (
                28,
                "fb201720f6dada59a0ee686e12c0a0c418c5376ce44ec1b3f21ace0159499f3597d0a75b0539cf4924d727a8f3485f19",
            ),
        ] {
            fields[field] = hex::decode(encoded).expect("fixed historical schedule digest");
        }
    }

    fn restore_retired_public_terminal_profile_descriptors_v1(fields: &mut [Vec<u8>]) {
        restore_historical_catalog_schedule_fields_v1(fields);
        assert_eq!(fields.len(), COMPILED_PROFILE_FIELD_COUNT_V1);
        fields[9] = b"field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-exactly-one-X5M1-and-one-X5C1-v1|x5m1=claims-plus-length-delimited-aggregate-only-no-fixed-sidecar|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-roots=one-joined-base-and-one-joined-aux|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log13-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|p256-binding-sink-degree=3-including-fixed-selectors|sha-capacity-and-call-degree=6-including-fixed-selectors|sha-digest-address=polynomial-select|sha-fixed-algebraic-width=472-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-polynomials=verifier-derived-at-deep-and-native-translates|shared-x5b1-challenges=single-joined-main-base-root+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=1816|ca-trace-hiding-coefficients=696|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=9over64|main-fri-blowup=8|ca-lde-log2=16|fri-queries=136-distinct-without-replacement|composition-fp4-lanes=1|fri-batching-m=3|affine-arities=2,2,2|fri-folding=2|fri-leaves=ordered-low-high-pairs|main-fri-terminal-length=1024-degree143|ca-fri-terminal-length=1024-degree143|deep-points=1-per-subproof-current+next-openings|ca-deep-constraints=all1379-fp4-verifier-fixed-polynomials-current-only-query-rows|main-deep-constraints=all49-fp4-native-vanishing-six-chunk-recomposition-verifier-fixed-polynomials-current-only-query-rows|grinding-bits=20|target-soundness-bits=128|rfc5280-temporal-air=base285-aux280-fixed102-constraints1681-degree4-authenticated72-times-73-relations-38bit-slack-affine-loglookup-30-relations|rbr-budget-bits=157|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=9420938|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|shared-stark-v1=q136-blowup8-digest384-fp4-blocked-pending-independent-qualification|activation=unavailable".to_vec();
        fields[10] = b"zk-x509-main-assembly-v1-incompatible:strict-reference-prover-invariant:exact-der-rfc-projection-ca-sources:29-verifier-positioned-sha-witnesses:five-p256-equations:optional-slot2-rfc-zero-source-and-public-valid-dummy-selector:statement-compiled-deduplicated-sequential-byte-io:exact-witness-declaration-replay:logical-active-row-census:exact49-registrations:no-host-verification-substitute:verifier-terminal-replay=complete:activation=governance-gated".to_vec();
        // Historical profiles predate the corrected CA domain descriptor.
        // Restore its exact bytes alongside the retired relation descriptors.
        fields[12] = b"zk-x509-credential-pre-aux-v1:X5B1:version1:profile=main-profile-digest+ca-profile-digest:public=consensus-context-digest+ca-public-digest:base-roots=exact-main1-joined-ordered-native-log5,8,15,16,18,19-then-ca1-log13:post-base-challenges=exact272-goldilocks-fields:01-sha-call=4lanes*(beta,call,role,slot,kind,word,value)=28:02-rfc=4lanes*tuple12=48:03-projection=4lanes*(copy-beta,copy-gamma,compaction-active,compaction-invocation,compaction-position,compaction-value,compaction-gamma)=28:04-io=4lanes*(beta,channel,offset,value,is-write)=20:05-der=4lanes*(tuple12-then-byte-lookup)=52:06-sha-word-memory=4lanes*(beta,address,value,is-write)=16:07-sha-word-base-fold=4:08-p256-value=4lanes*7=28:09-p256-cross=4lanes*4=16:10-p256-scalar=4lanes*5=20:11-p256-arithmetic-copy=4lanes*3=12:lane-major-within-each-family:one-private-opaque-main-post-base-capability:no-raw-constructor:bind-post-challenge-state-and-all272-canonical-challenges-into-each-local-transcript-before-aux-roots:no-caller-selected-binding".to_vec();
        fields[13] = b"byte-memory-permutation=complete|strict-der-segment=complete|projection-segment=complete|shared-current-next-deep-ali=complete|rfc5280-base-row-provider=complete|rfc5280-aggregate-and-eighteen-independent-output-role-products=complete|rfc5280-x5r1-and-der-terminal-validator=complete|sha-call-witness-assembly-and-terminal-binding=complete|p256-witness-assembly-and-terminal-binding=complete|compact-ca-subproof=complete|full-49-registration-prover-and-verifier=complete|combined-main-ca-envelope=complete|consensus-verifier-integration=complete|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|activation=unavailable-qualification".to_vec();
        fields[14] =
            hex::decode("307915059aa0f173351facf134c2c08df720e365cfdb96c239eca44c07b654bb")
                .unwrap();
        // These historical pins included the original 32 whole-segment totals
        // and 64 public RFC-stream products; restore their exact old descriptor.
        fields[17] = b"zk-x509-sha-call-bus-stark-v1-incompatible:29-fixed-capacity-calls=cert-tbs[3]+crl-tbs+framed-complete-signed-crl+projection[7]+issuer-spki+trust-record+policy-record+crl-record+compact-ca-leaf+compact-ca-node[12]:max-blocks616:word-rows1972128=compression655424+local-init232+local-digest232+memory1316240:four-log19-segments-whole-call-packed-active-rows480288,521952,521696,448192-no-cross-segment-call-transition:base89=word-capacity76+proof-bound-rfc-raw-length-bits13:aux78=word-capacity54+input-products4+digest-products4+rfc-consumer-products16:fixed118=word72+call-segment-length-control9+thirteen-verifier-one-hot-compact-ca-call-selectors+four-field-native-rfc-event-descriptors-of-width6:constraints796=prior588+thirteen-call-times-four-lanes-times-four-start-terminal-equalities208:degree6-including-fixed-selectors:polynomial-digest-address=digest*dynamic+(1-digest)*fixed:base-two-chunks-aux-two-chunks-per-segment:same-log-bucket-base356-aux312-base-chunks8-aux-chunks8:private-exact-length-unique-padding-transition-across-blocks-and-active-block-prefix:fine-grained-message-cap-and-fixed-role-length-enforcement:frozen-canonical-inactive-computation-memory-and-mask-suffix:selected-digest-from-unique-final-active-block:inactive-chain-and-projection-slots-canonical-sha-empty-dummy:address=(call,role,slot,input-or-digest,word):four-independent-domain-separated-goldilocks-lanes:separate-word-memory-and-call-challenge-families:segment-continuous-source-digest-and-rfc-products-with-registration-owned-terminals:cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-aux=zero:word-capacity-recurrence=local-compute+digest+memory-call-last:compact-ca-calls16through28-each-bind-proof-carried-source-and-digest-start-and-terminal-products-by-verifier-fixed-one-hot-selectors-without-division:rfc-consumer-products-derived-algebraically-from-committed-message-bits-masks-and-verifier-fixed-event-descriptors:four-byte-streams-total-degree5-recurrences-including-fixed-selectors:proof-bound-u64-raw-length-consumers:certificate-tbs-crl-tbs-framed-complete-crl-and-framed-issuer-spki-channels:three-governance-self-digests-explicit-sha-field-frames:no-host-branch-on-opened-fixed-columns:main-common-lde-log22:protocol2-independent-per-lane-fri-mask-oracles:max-encoded-sha-proof2836064:stream-one-call-at-a-time:on-demand-full-row-widening-without-duplicated-aux-or-fixed-vectors".to_vec();
        // These rejection fixtures predate the selected-input writer multiplicities.
        // Freeze both the descriptor and its exact original schedule digest;
        // inheriting either current field would manufacture a mixed profile.
        fields[27] = b"zk-x509-p256-fixed-algebraic-v1-incompatible:native-log19:generator-coset-lde-log22:width404:six-schedules=certificate-arithmetic134+wallet-arithmetic134+certificate-execution46+wallet-execution46+certificate-sorted22+wallet-sorted22:typed-composite-children=134,134,46,46,22,22:each-child-generic-cap65536:composite-digest=sha3-384-opaque48-binds-profile+ordered-widths+ordered-child-digests:row-major-child-opening-concatenation:aliases-exactly15=signatures0through4-times-arithmetic0+value-execution0+value-sorted1:signatures0through3-certificate-role:signature4-wallet-role:closed-value-free-topology-only:additive-affine+repeated-affine+sparse:operation-metadata-plan=min-exact-row-axis-vs-canonical-call-axis:row-axis-on-tie:call-segments=14x43+64x222+row-tail18:sorted-active-factors=725504-distinct-from-execution-logical-factors949312:sorted-equal-read-runs=min-exact-relative-factor-axis-vs-per-value-axis:relative-factor-axis-on-tie:sorted-whole-plan=min-exact-global-local-vs-phase-hybrid:global-local-on-tie:phase-hybrid=prefix893-local+min-local-vs13x43-phase+scalar-boundary222-local+min-local-vs63x222-phase+tail18-local:pinned-boundary-extents=1712,9984:pinned-repeated-extents=1888,10176:local-on-phase-tie:no-native-matrix:no-lde-table:no-artifact:no-merkle:no-proof-fixed-bytes:first-release".to_vec();
        fields[28] = hex::decode(
            "27920427fcfec454c4454b1af2035f3b0af4dcadfeafb6c6824f93478217fc9a3107137c5eac8eacb141fe8e72d1384d",
        )
        .unwrap();
    }
    #[test]
    fn retired_profile_restoration_freezes_pre_aux_sha_and_p256_manifest_fields() {
        let sha_digests = [[0x51; 48]; SHA_DISCLOSURE_SHAPE_COUNT_V1];
        let p256_digest = [0x61; 48];
        let fields = compiled_profile_fields_v1(&sha_digests, &p256_digest);
        let mut canonical = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        let mut substituted = canonical.clone();
        for (index, field) in substituted.iter_mut().enumerate() {
            *field = vec![u8::try_from(index).unwrap(); index + 1];
        }
        restore_retired_public_terminal_profile_descriptors_v1(&mut canonical);
        restore_retired_public_terminal_profile_descriptors_v1(&mut substituted);
        assert_eq!(substituted, canonical);
        assert_eq!(canonical[28].len(), 48);
        assert!(canonical[22..27].iter().all(|digest| digest.len() == 48));
        let borrowed = canonical.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert_eq!(
            hex::encode(independent_compiled_profile_digest_v1(&borrowed)),
            "0312a22aad46561f42f28831baff280a072f34899284e354938c09a82e7adcdf"
        );
    }
    #[test]
    fn compiled_profile_binds_private_terminal_closure() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let profile = core::str::from_utf8(fields[9]).unwrap();
        assert!(profile.contains("main-public-terminal-records=0|ca-public-terminal-records=0"));
        assert!(
            profile.contains("main-frame-bytes=1002-includes992-key-DEEP-values|ca-frame-bytes=10")
        );
        assert!(profile.contains("main-key-byte-joins=12-blocks647-equalities-rfc-to-io-and-real-p256-root-powers2,8-max-quotient-degree538744"));
        assert!(profile.contains("main-sha-digest-joins=5-blocks40-u32-equalities-unreduced-be-four-byte-real-p256-root-power32-max-quotient-degree2155224"));
        assert!(profile.contains("main-public-crl-digest-joins=calls4,12-authoritative-retained-record-two-eight-root-quotients16-u32-no-extra-openings-degree526095"));
        assert!(profile.contains("main-public-crl-time-joins=required-rfc-calendar-slots6,7-phase6-authoritative-this-and-next-update-two-single-root-quotients-no-extra-openings-degree526102"));
        assert!(profile.contains("21-key-and-digest-join-descriptors+key-and-digest-alphas+24-private-sha-union-descriptors"));
        assert!(profile.contains(
            "MAIN31-key-and-digest:5-power8+6-power2+20-power32+MAIN24-translated-aux+CA108-translated-aux"
        ));
        assert!(profile.contains("max-encoded-combined-bound=9435984"));
        assert!(profile.contains("main-sha-rfc-private-union=16-constant-native-bridges+16-segment-quartic-links+4-role-quartic-links+4-projection-to-sha-point1-links-original-masks-existing-aux-DEEP-no-extra-openings-degree2104411"));
        assert!(profile.contains("max-proof-bytes=9437184"));
        assert!(profile.contains("quotient-chunk-stride=fri-degree-cap-minus137"));
        assert!(profile.contains(
            "quotient-chunk-masks=137-independent-fp4-coefficients-adjacent-cancellation"
        ));
        assert!(profile.contains("main-quotient-mask-order=all-local-and-private-link-contributions-then-blind-before-root"));
        assert!(
            profile.contains(
                "private-der-rfc-source-and-p256-terminal-scalars=364-committed-air-only"
            )
        );
        assert!(
            core::str::from_utf8(fields[10])
                .unwrap()
                .contains("no-unmasked-der-rfc-source-or-p256-terminal-scalars")
        );
        let components = core::str::from_utf8(fields[13]).unwrap();
        assert!(
            components
                .contains("rfc5280-private-source-and-CA-original-polynomial-air-links=complete")
        );
        assert!(components.contains("private-committed-terminal-air-links=complete"));
    }
    #[test]
    fn compiled_profile_rejects_retired_public_terminal_exposure() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let mut retired = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_retired_public_terminal_profile_descriptors_v1(&mut retired);
        let retired_fields = retired.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old_digest = independent_compiled_profile_digest_v1(&retired_fields);
        assert_eq!(
            hex::encode(old_digest),
            "0312a22aad46561f42f28831baff280a072f34899284e354938c09a82e7adcdf"
        );
        assert_ne!(
            old_digest,
            recompute_zk_x509_compiled_profile_digest_v1().unwrap()
        );
        assert_ne!(Some(old_digest), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
        let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
            .expect("current verifier profile");
        supplied.compiled_profile_digest = old_digest;
        assert!(super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err());
    }
    #[test]
    fn compiled_profile_rejects_the_superseded_sha_padding_descriptor() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let mut superseded = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_retired_public_terminal_profile_descriptors_v1(&mut superseded);
        let current = core::str::from_utf8(&superseded[17]).unwrap().to_owned();
        let word_boundary = "word-capacity-recurrence=local-compute+digest+memory-call-last:";
        let bus_boundary =
            "cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-aux=zero:";
        assert_eq!(current.matches(word_boundary).count(), 1);
        assert_eq!(current.matches(bus_boundary).count(), 1);
        // Historical SHA-padding profiles also used the retired compact-CA
        // descriptor. Preserve their exact original digests and rejection tests.
        superseded[14] =
            hex::decode("9a34a72f020551e65442c24b58ee075f6c485e36e0df7c579b246b19d0195b9a")
                .unwrap();
        for (descriptor, expected_digest) in [
            (
                current.clone(),
                "19aa35927ebc6e0ff800a0c80b6b315c2615350f35c82afdb9eae609d4c9ca9c",
            ),
            (
                current.replace(word_boundary, ""),
                "9d2d34512de90d13a0f68d352bbcc887ba9ac2f2a89e5deb845ff5c4c64d45ff",
            ),
            (
                current.replace(word_boundary, "").replace(bus_boundary, ""),
                "f82e78a995ce1b9ca1e91628901e9acd9b01a6841c13e062e30ad6dfdf028795",
            ),
        ] {
            superseded[17] = descriptor.into_bytes();
            let old_fields = superseded.iter().map(Vec::as_slice).collect::<Vec<_>>();
            let old_digest = independent_compiled_profile_digest_v1(&old_fields);
            assert_eq!(hex::encode(old_digest), expected_digest);
            assert_ne!(Some(old_digest), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
            assert_ne!(
                construct_zk_x509_compiled_profile_v1().unwrap().digest(),
                old_digest
            );
            let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
                .expect("current verifier profile");
            supplied.compiled_profile_digest = old_digest;
            assert!(
                super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err()
            );
        }
    }
    #[test]
    fn compiled_profile_rejects_retired_enterprise_eku_identifiers() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let mut predecessor = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_historical_catalog_schedule_fields_v1(&mut predecessor);
        let descriptor = core::str::from_utf8(&predecessor[9]).unwrap();
        let marker = "-eku-canonical-profile-uuid-oids";
        assert_eq!(descriptor.matches(marker).count(), 1);
        predecessor[9] = descriptor.replace(marker, "").into_bytes();
        let borrowed = predecessor.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old_digest = independent_compiled_profile_digest_v1(&borrowed);
        // All 29 predecessor fields were emitted by the frozen fd0a8 native
        // constructor; this is its authentic profile, not a fabricated pin.
        assert_eq!(
            hex::encode(old_digest),
            "c095101f36ac92550ae15df711d2a1fff126b7540e65922ab18a34d5691ae688"
        );
        assert_ne!(
            old_digest,
            recompute_zk_x509_compiled_profile_digest_v1().unwrap()
        );
        assert_ne!(Some(old_digest), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
        let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
            .expect("current verifier profile");
        supplied.compiled_profile_digest = old_digest;
        assert!(super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err());
    }
    #[test]
    fn compiled_profile_rejects_ungated_rfc_root_helper_descriptor() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let mut predecessor = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_historical_catalog_schedule_fields_v1(&mut predecessor);
        let descriptor = core::str::from_utf8(&predecessor[9]).unwrap();
        let marker = "source-node-root-kind-active-gated-helper-";
        assert_eq!(descriptor.matches(marker).count(), 1);
        let current_descriptor = core::str::from_utf8(fields[9]).unwrap();
        assert_eq!(current_descriptor.matches(marker).count(), 1);
        let current_without_gate = current_descriptor.replace(marker, "").into_bytes();
        // Freeze the exact native predecessor descriptor. Stripping only this
        // marker from later RFC closures would manufacture a different profile.
        predecessor[9] = b"proof-instance=required-public32-after-version-count-before-consensus-context:checked-disjoint-entropy-before-first-commitment:all-zero-canonical|dynamic-hash-profile-suffix=00-X5I1-family-u8-nonce32:joint0-main1-ca2:all-commitments-and-transcript-operations:fixed-compiler-metadata-unscoped|field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-main24-ca108-joint-original-openings-and-exactly-one-X5M1-and-one-X5C1-v1|x5m1=version1+31-original-key-DEEP-values+length-delimited-aggregate-no-fixed-sidecar|main-public-terminal-records=0|ca-public-terminal-records=0|main-frame-bytes=1002-includes992-key-DEEP-values|ca-frame-bytes=10|joint-openings=132-canonical-Fp4-values4224|ca-private-endpoint-links=108-original-polynomial-quotients-MAIN-composition|private-der-rfc-source-and-p256-terminal-scalars=364-committed-air-only|main-private-endpoint-links=192-linear-quotients-original-masks-shared-native-points|main-key-byte-joins=12-blocks647-equalities-rfc-to-io-and-real-p256-root-powers2,8-max-quotient-degree538744|main-sha-digest-joins=5-blocks40-u32-equalities-unreduced-be-four-byte-real-p256-root-power32-max-quotient-degree2155224|main-sha-rfc-private-union=16-constant-native-bridges+16-segment-quartic-links+4-role-quartic-links-original-masks-existing-aux-DEEP-no-extra-openings-degree2104411|main-link-alpha-order=local-component-history+both-original-aux-roots+fresh-local-alphas+192-ordered-private-endpoint-link-descriptors+endpoint-link-alphas+17-key-and-digest-join-descriptors+key-and-digest-alphas+20-private-sha-union-descriptors+sha-union-alphas+108-ordered-CA-private-link-descriptors+CA-link-alphas|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-roots=one-joined-base-and-one-joined-aux|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log12-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|p256-binding-sink-degree=3-including-fixed-selectors|sha-capacity-and-call-degree=6-including-fixed-selectors|sha-digest-address=polynomial-select|sha-fixed-algebraic-width=472-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-polynomials=verifier-derived-at-deep-and-native-translates|shared-x5b1-challenges=single-joined-main-base-root+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=1816|ca-trace-hiding-coefficients=2100|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=9over64|main-fri-blowup=8|ca-lde-log2=16|fri-queries=136-distinct-without-replacement|composition-fp4-lanes=1|quotient-chunk-stride=fri-degree-cap-minus137|quotient-chunk-masks=137-independent-fp4-coefficients-adjacent-cancellation|main-quotient-mask-order=all-local-and-private-link-contributions-then-blind-before-root|fri-batching-m=3|binary-fold-arity=2|fri-folding=2|fri-leaves=ordered-low-high-pairs|main-fri-terminal-length=1024-degree143|ca-fri-terminal-length=1024-degree143|deep-points=one-joint-point-after-both-original-aux-composition-and-FRI-mask-roots:both-local-checkpoints-bound:current+next+MAIN31-key-and-digest:5-power8+6-power2+20-power32+MAIN24-translated-aux+CA108-translated-aux:combined-domain-admission:both-local-DEEP-and-MAIN31-then-ordered132-values-bound-before-independent-local-mixes|ca-deep-constraints=all1363-fp4-verifier-fixed-polynomials-current-only-query-rows|main-deep-constraints=all49-fp4-native-vanishing-six-chunk-recomposition-verifier-fixed-polynomials-current-only-query-rows|grinding-bits=20|target-soundness-bits=128|rfc5280-temporal-air=base285-aux280-fixed147-constraints1941-degree4-five-key-and-five-complete-spki-outputs-der-node-byte-lookups-key-length66-offset1-spki-content89-total91-offset0-nine-variable-tbs-crl-signature-byte-and-canonical-u16-in-eight-byte-big-endian-length-pairs-live-prefix-count-node-continuity-source-start-helper-projection-leaf-serial-magnitude-sign-octet-nonzero-first-byte-big-endian-length-selected-original-subject-oid-value-provenance-with-complete-name-oid-census-three-original-classes-disjoint-document-stride12-keys1through48-six-bit-positive-gaps-uniqueness-nonempty-bounded-rdns-and-complete-original-value-census-country-printable-utf8-dfa-output-metadata-six-verifier-fixed-equations-authenticated72-times-73-relations-38bit-slack-affine-loglookup-39-relations|rbr-budget-bits=157|joint-configuration-bound=8-per-domain,64-paired|joint-point-exclusion-bound=3277643777|joint-cleared-degree-bounds=19398656,40960|joint-algebraic-error-bits=219|base-collision-configuration-factor=64|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=9412944|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|shared-stark-v1=q136-blowup8-digest384-fp4-blocked-pending-independent-qualification|activation=unavailable".to_vec();
        let borrowed = predecessor.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old_digest = independent_compiled_profile_digest_v1(&borrowed);
        assert_eq!(
            hex::encode(old_digest),
            "294649842da2b565e94b022ce59c7a7c662aa2bf70502d8a554abb510d487852"
        );
        assert_ne!(
            old_digest,
            recompute_zk_x509_compiled_profile_digest_v1().unwrap()
        );
        // Also reject the current relation with just its activity-gate marker
        // removed; this distinct mutation must not replace the historical pin.
        let mut ungated_current = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        ungated_current[9] = current_without_gate;
        let current_fields = ungated_current
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let current_mutation_digest = independent_compiled_profile_digest_v1(&current_fields);
        assert_ne!(
            current_mutation_digest,
            recompute_zk_x509_compiled_profile_digest_v1().unwrap()
        );
        for rejected_digest in [old_digest, current_mutation_digest] {
            let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
                .expect("current verifier profile");
            supplied.compiled_profile_digest = rejected_digest;
            assert!(
                super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err()
            );
        }
    }

    // Exact predecessor fields authenticated by the ec254e72 source derivation
    // and the retained native schedule capture. Rejection fixtures only.
    fn restore_pre_frame_profile_fields_v1(fields: &mut [Vec<u8>]) {
        restore_pre_endpoint_binding_profile_fields_v1(fields);
        assert_eq!(fields.len(), COMPILED_PROFILE_FIELD_COUNT_V1);
        fields[9] = b"proof-instance=required-public32-after-version-count-before-consensus-context:checked-disjoint-entropy-before-first-commitment:all-zero-canonical|dynamic-hash-profile-suffix=00-X5I1-family-u8-nonce32:joint0-main1-ca2:all-commitments-and-transcript-operations:fixed-compiler-metadata-unscoped|field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-main24-ca108-joint-original-openings-and-exactly-one-X5M1-and-one-X5C1-v1|x5m1=version1+31-original-key-DEEP-values+length-delimited-aggregate-no-fixed-sidecar|main-public-terminal-records=0|ca-public-terminal-records=0|main-frame-bytes=1002-includes992-key-DEEP-values|ca-frame-bytes=10|joint-openings=132-canonical-Fp4-values4224|ca-private-endpoint-links=108-original-polynomial-quotients-MAIN-composition|private-der-rfc-source-and-p256-terminal-scalars=364-committed-air-only|main-private-endpoint-links=192-linear-quotients-original-masks-shared-native-points|main-key-byte-joins=12-blocks647-equalities-rfc-to-io-and-real-p256-root-powers2,8-max-quotient-degree538744|main-sha-digest-joins=5-blocks40-u32-equalities-unreduced-be-four-byte-real-p256-root-power32-max-quotient-degree2155224|main-sha-rfc-private-union=16-constant-native-bridges+16-segment-quartic-links+4-role-quartic-links-original-masks-existing-aux-DEEP-no-extra-openings-degree2104411|main-link-alpha-order=local-component-history+both-original-aux-roots+fresh-local-alphas+192-ordered-private-endpoint-link-descriptors+endpoint-link-alphas+17-key-and-digest-join-descriptors+key-and-digest-alphas+20-private-sha-union-descriptors+sha-union-alphas+108-ordered-CA-private-link-descriptors+CA-link-alphas|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-roots=one-joined-base-and-one-joined-aux|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log12-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|p256-binding-sink-degree=3-including-fixed-selectors|sha-capacity-and-call-degree=6-including-fixed-selectors|sha-digest-address=polynomial-select|sha-fixed-algebraic-width=472-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-polynomials=verifier-derived-at-deep-and-native-translates|shared-x5b1-challenges=single-joined-main-base-root+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=1816|ca-trace-hiding-coefficients=2100|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=9over64|main-fri-blowup=8|ca-lde-log2=16|fri-queries=136-distinct-without-replacement|composition-fp4-lanes=1|quotient-chunk-stride=fri-degree-cap-minus137|quotient-chunk-masks=137-independent-fp4-coefficients-adjacent-cancellation|main-quotient-mask-order=all-local-and-private-link-contributions-then-blind-before-root|fri-batching-m=3|binary-fold-arity=2|fri-folding=2|fri-leaves=ordered-low-high-pairs|main-fri-terminal-length=1024-degree143|ca-fri-terminal-length=1024-degree143|deep-points=one-joint-point-after-both-original-aux-composition-and-FRI-mask-roots:both-local-checkpoints-bound:current+next+MAIN31-key-and-digest:5-power8+6-power2+20-power32+MAIN24-translated-aux+CA108-translated-aux:combined-domain-admission:both-local-DEEP-and-MAIN31-then-ordered132-values-bound-before-independent-local-mixes|ca-deep-constraints=all1363-fp4-verifier-fixed-polynomials-current-only-query-rows|main-deep-constraints=all49-fp4-native-vanishing-six-chunk-recomposition-verifier-fixed-polynomials-current-only-query-rows|grinding-bits=20|target-soundness-bits=128|rfc5280-temporal-air=base285-aux280-fixed147-constraints2145-degree4-basic-constraints-fixed3x12-signed-parent-complete-canonical-u32-and-32bit-nonwrapping-slack-source-node-root-kind-active-gated-helper-fixed-eight-equality-and-fifteen-embedded-endpoint-slots-complete-nonempty-original-node-spans-live-prefix-count-typed-copy-domains103-104-affine-one-plus-tuple-for-shared-domains90-98-99-102-103-104-affine-one-plus-tuple-for-shared-profile-domains96-97-optional-slot-canonical-zero-exact-extension-oid-ordinal-ku-document-and-critical-cardinality-eku-canonical-profile-uuid-oids-five-key-and-five-complete-spki-outputs-der-node-byte-lookups-key-length66-offset1-spki-content89-total91-offset0-nine-variable-tbs-crl-signature-byte-and-canonical-u16-in-eight-byte-big-endian-length-pairs-live-prefix-count-node-continuity-source-start-helper-projection-leaf-serial-magnitude-sign-octet-nonzero-first-byte-big-endian-length-selected-original-subject-oid-value-provenance-with-complete-name-oid-census-three-original-classes-disjoint-document-stride12-keys1through48-six-bit-positive-gaps-uniqueness-nonempty-bounded-rdns-and-complete-original-value-census-country-printable-utf8-dfa-output-metadata-six-verifier-fixed-equations-certificate-slot-output-producer-and-consumer-fixed-selector-equals-global-activity-authenticated72-times-73-relations-38bit-slack-affine-loglookup-39-relations|rbr-budget-bits=157|joint-configuration-bound=8-per-domain,64-paired|joint-point-exclusion-bound=3277643777|joint-cleared-degree-bounds=19398656,40960|joint-algebraic-error-bits=219|base-collision-configuration-factor=64|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=9412944|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|shared-stark-v1=q136-blowup8-digest384-fp4-blocked-pending-independent-qualification|activation=unavailable".to_vec();
        fields[17] = b"zk-x509-sha-call-bus-stark-v1-incompatible:29-fixed-capacity-calls=cert-tbs[3]+crl-tbs+framed-complete-signed-crl+projection[7]+issuer-spki+trust-record+policy-record+crl-record+compact-ca-leaf+compact-ca-node[12]:max-blocks616:word-rows1972128=compression655424+local-init232+local-digest232+memory1316240:four-log19-segments-whole-call-packed-active-rows480288,521952,521696,448192-no-cross-segment-call-transition:base89=word-capacity76+proof-bound-rfc-raw-length-bits13:aux78=word-capacity54+input-products4+digest-products4+rfc-consumer-products16:fixed118=word72+call-segment-length-control9+thirteen-verifier-one-hot-compact-ca-call-selectors+four-field-native-rfc-event-descriptors-of-width6:constraints564=local-private-running-products:MAIN108-cross-proof-original-polynomial-quotients:degree6-including-fixed-selectors:polynomial-digest-address=digest*dynamic+(1-digest)*fixed:base-two-chunks-aux-two-chunks-per-segment:same-log-bucket-base356-aux312-base-chunks8-aux-chunks8:private-exact-length-unique-padding-transition-across-blocks-and-active-block-prefix:fine-grained-message-cap-and-fixed-role-length-enforcement:frozen-canonical-inactive-computation-memory-and-mask-suffix:selected-digest-from-unique-final-active-block:inactive-chain-and-projection-slots-canonical-sha-empty-dummy:address=(call,role,slot,input-or-digest,word):four-independent-domain-separated-goldilocks-lanes:separate-word-memory-and-call-challenge-families:segment-continuous-private-source-digest-and-rfc-products:no-public-segment-products:main-16-quartic-native-endpoint-links-to-private-rfc-bridges:cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-aux=zero:word-capacity-recurrence=local-compute+digest+memory-call-last:compact-ca-calls16through28-bind-original-source-and-digest-endpoints-through-MAIN108-private-polynomial-quotients-without-product-division:rfc-consumer-products-derived-algebraically-from-committed-message-bits-masks-and-verifier-fixed-event-descriptors:four-byte-streams-total-degree5-recurrences-including-fixed-selectors:proof-bound-u64-raw-length-consumers:certificate-tbs-crl-tbs-framed-complete-crl-and-framed-issuer-spki-channels:three-governance-self-digests-explicit-sha-field-frames:no-host-branch-on-opened-fixed-columns:main-common-lde-log22:protocol2-independent-per-lane-fri-mask-oracles:max-encoded-sha-proof2836064:stream-one-call-at-a-time:on-demand-full-row-widening-without-duplicated-aux-or-fixed-vectors".to_vec();
        fields[20] = b"zk-x509-fixed-algebraic-v1-incompatible:verifier-derived-only:no-proof-fixed-material:no-artifact:no-merkle:additive-canonical-atoms=affine-range+repeated-affine-stride+sparse:overlap=goldilocks-field-addition:exact-duplicate-atoms-rejected:semantically-equivalent-alternate-decompositions-have-distinct-descriptor-digests:goldilocks-modulus=0xffffffff00000001:native-root-domain:generator-shifted-lde-coset:coset-disjoint-from-lde-subgroup:query-index-derived-point:residue-grouped-barycentric-lagrange:batch-inverted-native-denominators:cyclic-prefix-affine-sums:repeated-sums=generic-gcd-cycles+reduced-stride-modular-inverse+cyclic-weight-and-ordinal-prefixes+per-stride-min-direct-occurrence-work-vs-native-prefix-work:one-column-native-streaming:no-native-times-width-or-lde-table:bounded-native20-lde25-blowup8-width472-atoms65536-queries272-output-fields128384-work2pow28:digest=sha3-384-opaque48:wire=X5K1+u16be-version1+u16be-header24+native-log2-u8+lde-log2-u8+width-u16be+atom-count-u32be+coset-shift-u64be+canonical-variable-atoms:first-release-no-legacy".to_vec();
        fields[21] = b"zk-x509-sha-fixed-algebraic-compiler-v1-incompatible:public-shapes=disclosed-attributes0through4:four-independent-physical-log19-generic-children:child-widths=118,118,118,118:combined-width472:segment-major-column-order:each-child-generic-cap65536:typed-composite-digest=sha3-384-opaque48-binds-disclosure-shape+profile+ordered-widths+ordered-child-digests:row-major-child-opening-concatenation:typed-zero-capacity-sha-word-circuit-topology:word-operation-and-execution-sorted-memory-walk:authoritative-local-row-event-order-replay:definition-writes-immediately-before-consuming-operation:operation-input-reads-then-output-write:eight-digest-reads-last:derived-execution-sort-must-equal-circuit-canonical-sorted-memory:execution-write-axis-key=typed-word-phase(initial|input|expansion3|round8|final):compute+execution+sorted-memory-call-axis-transpose-on-exact-maximal-contiguous-same-segment-same-geometry-runs-iff-calls-strictly-greater-than-blocks:block-row-axis-on-ties:boolean-topology-three-way-exact-atom-planner(block-or-call=2048*min(calls,blocks)|round=32*blocks*calls|block-gap=416*calls):strict-lower-only:old-then-round-wins-ties:block-gap-axis-one-stride14-hull+12-negative-stride1064-gap-residues-per-lane:operation-read-typed-block+phase(expansion6|round18|final2)+read-slot-axis-with-exact-per-call-cost44*blocks-2:operation-read-axis-transpose-iff-exact-cost-strictly-less-than-existing-block-or-call-axis:sorted-memory-typed-initial-or-block+word-phase(initial|input|expansion3|round8|final)+access-occurrence-axis:sorted-memory-phase-axis-on-every-exact-geometry-run-iff-phase-cost=298*blocks*calls-is-strictly-less-than-existing-axis-cost(call=4952*blocks+32|block=(4952+32)*calls):old-axis-wins-ties:call-axis-key=local-column+family+block(initial-or-sha-index)+word-position+occurrence:remaining-sorted-memory-nontransposed-series-maximal-across-ordered-calls:no-native-row-matrix:no-lde-matrix:no-artifact:no-merkle-root:no-proof-supplied-fixed-values:affine+repeated+sparse-atoms:generator-coset-log19-to-log22:call-role-slot-boundaries+compact-ca-selectors+field-native-rfc-events+physical-padding:exact-shape-derived-rfc-channel-offsets:all-six-formerly-reconstructed-word-columns-native:first-release".to_vec();
        fields[22] = hex::decode("19d545d3401cdcdbfa1bf2be01947ac5ad97e675ff923d44ca11a3dba798515f227673a5baedcb22882b27b97b5cfc1e").expect("authenticated pre-frame schedule");
        fields[23] = hex::decode("ffa119cd841089678f55413b3ab3f886517d35db56977a9bc8417a272581b0559d21cd8b9dcb0da54e37306fa5a99943").expect("authenticated pre-frame schedule");
        fields[24] = hex::decode("cd23c06d51565f93f16ef70095a60f461e50ca886a5509d444b26aee166101336e25517354ea3574309b46afb09062f6").expect("authenticated pre-frame schedule");
        fields[25] = hex::decode("23f9a09f8e8d1c114e5530cbe7d0775ff028b2682fa3f8c6e84cde4d754eb71f26be3aad78ea325e458ecff36e9f3bab").expect("authenticated pre-frame schedule");
        fields[26] = hex::decode("8755e85893dc6fe29f7a396fce4cba07955b51f986b4c64e4e397fef979ada2ec7f2b762eda5f1e333624e9505920c06").expect("authenticated pre-frame schedule");
        fields[28] = hex::decode("ceee30c43a1f13bdcddb9ef69da529b5733f4714a9b87b0e3d792ea5d6639f399899cbea68f607c2f580c0f8e30e23cd").expect("authenticated pre-frame schedule");
    }
    #[test]
    fn compiled_profile_rejects_unbound_certificate_slot_output_relation() {
        let (sha, p256) = compiled_profile_schedule_digests_v1().unwrap();
        let fields = compiled_profile_fields_v1(&sha, &p256);
        let current = independent_compiled_profile_digest_v1(&fields);
        let marker =
            "-certificate-slot-output-producer-and-consumer-fixed-selector-equals-global-activity";
        let descriptor = core::str::from_utf8(fields[9]).unwrap();
        assert_eq!(descriptor.matches(marker).count(), 1);
        let mut predecessor = fields
            .iter()
            .map(|field| field.to_vec())
            .collect::<Vec<_>>();
        restore_pre_frame_profile_fields_v1(&mut predecessor);
        predecessor[9] = core::str::from_utf8(&predecessor[9])
            .unwrap()
            .replace(marker, "")
            .into_bytes();
        let borrowed = predecessor.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let old = independent_compiled_profile_digest_v1(&borrowed);
        assert_eq!(
            hex::encode(old),
            "c6a43af732ff56b3036e0d90064da643cfa046496c3c11ddb085ebc8f1834ab4"
        );
        assert_ne!(old, current);
        assert_eq!(Some(current), ZK_X509_COMPILED_PROFILE_DIGEST_V1);
        let mut supplied = super::super::stark::construct_zk_x509_main_verifier_profile_v1()
            .expect("current verifier profile");
        supplied.compiled_profile_digest = old;
        assert!(super::super::stark::validate_zk_x509_main_verifier_profile_v1(supplied).is_err());
    }

    /// Record forbidden entropy reads while rejecting every request immediately.
    #[derive(Default)]
    struct PreflightEntropyV1 {
        requests: usize,
    }
    impl rand::TryRngCore for PreflightEntropyV1 {
        type Error = &'static str;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }
        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }
    }
    impl rand::TryCryptoRng for PreflightEntropyV1 {}

    #[test]
    fn credential_prover_rejects_profile_or_genesis_before_entropy() {
        let fixture = super::super::relation::release_fixture::build_zk_x509_reference_fixture_v1()
            .expect("canonical reference fixture");
        let mut rng = PreflightEntropyV1::default();
        let error = prove_zk_x509_credential_proof_v1_with_rng(
            &fixture.statement,
            &fixture.authoritative_state,
            fixture.statement.presentation_not_before_unix_seconds * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            [0; 32],
            &[],
            &mut rng,
        )
        .expect_err("invalid genesis must not reach credential proof construction");
        assert_eq!(
            error,
            ZkX509EngineErrorV1::CredentialProof(ZkX509CredentialProofErrorV1::InvalidStatement)
        );
        assert_eq!(rng.requests, 0, "credential preflight consumed entropy");
    }

    #[test]
    fn credential_prover_accepts_geometry_but_rejects_invalid_witness_before_entropy() {
        super::super::stark::validate_zk_x509_main_proof_budget_v1()
            .expect("complete MAIN and CA wire fits the unchanged credential budget");
        let fixture = super::super::relation::release_fixture::build_zk_x509_reference_fixture_v1()
            .expect("canonical reference fixture");
        let mut rng = PreflightEntropyV1::default();
        let error = prove_zk_x509_credential_proof_v1_with_rng(
            &fixture.statement,
            &fixture.authoritative_state,
            fixture.statement.presentation_not_before_unix_seconds * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            *fixture.statement.context.network_id.as_bytes(),
            &[],
            &mut rng,
        )
        .expect_err("invalid witness must reject before proof construction or entropy");
        assert_eq!(
            error,
            ZkX509EngineErrorV1::WitnessCodec(ZkX509WitnessCodecErrorV1::Truncated)
        );
        assert_eq!(rng.requests, 0);
    }

    #[test]
    fn sole_profile_is_pinned_while_release_activation_stays_unavailable() {
        assert!(ZK_X509_COMPILED_PROFILE_DIGEST_V1.is_some());
        assert!(
            String::from_utf8_lossy(ZK_X509_AIR_COMPONENT_DESCRIPTOR_V1)
                .ends_with("activation=unavailable-qualification")
        );
    }
    #[test]
    fn consensus_public_sha_endpoints_follow_the_exact_retained_crl_record() {
        use super::super::relation::release_fixture::{
            authoritative_state_v1, build_zk_x509_reference_fixture_v1,
        };
        use iroha_data_model::privacy::{
            PrivacyX509CrlDerDigestV1, PrivacyX509CrlIssuerSpkiDigestV1, PrivacyZkX509CrlRecordV1,
        };
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let genesis = *fixture.statement.context.network_id.as_bytes();
        let original = compile_zk_x509_consensus_public_inputs_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            genesis,
        )
        .unwrap();
        let crl = fixture.authoritative_state.crl_record();
        assert_eq!(
            original.crl_public_inputs.crl_der,
            *crl.crl_der_digest.as_bytes()
        );
        assert_eq!(
            original.crl_public_inputs.issuer_spki,
            *crl.issuer_spki_digest.as_bytes()
        );
        for issuer in [false, true] {
            let mut der = *crl.crl_der_digest.as_bytes();
            let mut spki = *crl.issuer_spki_digest.as_bytes();
            if issuer {
                spki[0] ^= 1;
            } else {
                der[0] ^= 1;
            }
            let record = PrivacyZkX509CrlRecordV1::new(
                crl.trust_anchor_id,
                crl.certificate_policy_id,
                crl.record_epoch,
                crl.crl_number,
                PrivacyX509CrlDerDigestV1::new(der),
                PrivacyX509CrlIssuerSpkiDigestV1::new(spki),
                crl.this_update_unix_seconds,
                crl.next_update_unix_seconds,
                crl.previous_record_digest,
                crl.lifecycle,
            )
            .unwrap();
            let state = authoritative_state_v1(
                fixture.authoritative_state.trust_anchor(),
                fixture.authoritative_state.certificate_policy().clone(),
                record,
            )
            .unwrap();
            let mut statement = fixture.statement.clone();
            statement.crl_record_digest = record.record_digest;
            let changed =
                compile_zk_x509_consensus_public_inputs_v1(&statement, &state, genesis).unwrap();
            assert_eq!(changed.rfc_statement, original.rfc_statement);
            assert_eq!(changed.crl_public_inputs.crl_der, der);
            assert_eq!(changed.crl_public_inputs.issuer_spki, spki);
            assert_ne!(changed.crl_public_inputs, original.crl_public_inputs);
        }
    }
    #[test]
    fn consensus_public_crl_timestamps_follow_the_exact_retained_record() {
        use super::super::relation::release_fixture::{
            authoritative_state_v1, build_zk_x509_reference_fixture_v1,
        };
        use iroha_data_model::privacy::PrivacyZkX509CrlRecordV1;
        let fixture = build_zk_x509_reference_fixture_v1().unwrap();
        let genesis = *fixture.statement.context.network_id.as_bytes();
        let original = compile_zk_x509_consensus_public_inputs_v1(
            &fixture.statement,
            &fixture.authoritative_state,
            genesis,
        )
        .unwrap();
        let crl = fixture.authoritative_state.crl_record();
        assert_eq!(
            original.crl_public_inputs.this_update,
            crl.this_update_unix_seconds
        );
        assert_eq!(
            original.crl_public_inputs.next_update,
            crl.next_update_unix_seconds
        );
        for index in 0..2 {
            let record = PrivacyZkX509CrlRecordV1::new(
                crl.trust_anchor_id,
                crl.certificate_policy_id,
                crl.record_epoch,
                crl.crl_number,
                crl.crl_der_digest,
                crl.issuer_spki_digest,
                crl.this_update_unix_seconds + u64::from(index == 0),
                crl.next_update_unix_seconds + u64::from(index == 1),
                crl.previous_record_digest,
                crl.lifecycle,
            )
            .unwrap();
            let state = authoritative_state_v1(
                fixture.authoritative_state.trust_anchor(),
                fixture.authoritative_state.certificate_policy().clone(),
                record,
            )
            .unwrap();
            let mut statement = fixture.statement.clone();
            statement.crl_record_digest = record.record_digest;
            crate::privacy_state::validate_privacy_zk_x509_statement_state_v1(
                &statement,
                &state,
                statement.presentation_not_before_unix_seconds * 1_000,
                &PrivacyConsensusLimitsV1::taira_default(),
            )
            .unwrap();
            let changed =
                compile_zk_x509_consensus_public_inputs_v1(&statement, &state, genesis).unwrap();
            assert_eq!(changed.rfc_statement, original.rfc_statement);
            assert_eq!(
                changed.crl_public_inputs.crl_der,
                original.crl_public_inputs.crl_der
            );
            assert_eq!(
                changed.crl_public_inputs.issuer_spki,
                original.crl_public_inputs.issuer_spki
            );
            assert_eq!(
                changed.crl_public_inputs.this_update,
                record.this_update_unix_seconds
            );
            assert_eq!(
                changed.crl_public_inputs.next_update,
                record.next_update_unix_seconds
            );
            assert_ne!(changed.crl_public_inputs, original.crl_public_inputs);
        }
    }
    #[test]
    fn consensus_entry_point_reports_the_missing_joint_security_reduction() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
        assert_eq!(
            crate::privacy_profiles::compiled_privacy_profile_v1(protocol_id),
            Err(
                crate::privacy_profiles::CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete {
                    protocol_id,
                }
            )
        );
    }
    #[test]
    fn credential_prover_has_one_preflighted_joint_root_path_and_no_subproof_escape() {
        let source = include_str!("engine.rs");
        // Search only production text: an obsolete signature must not match
        // this test's own search literal and inspect the assertions themselves.
        let production_source = &source[..source
            .find("\n#[cfg(test)]\nmod tests {")
            .expect("test module")];
        let prover_start = production_source
            .find("pub fn prove_zk_x509_credential_proof_v1_with_rng")
            .expect("sole credential prover");
        let prover_end = production_source[prover_start..]
            .find("fn compiled_profile_fields_v1")
            .map(|offset| prover_start + offset)
            .expect("sole credential prover end");
        let prover = &production_source[prover_start..prover_end];
        let quarantine = prover
            .find("goldilocks_transform_completion_uncertain_v1()")
            .expect("quarantine before construction or entropy");
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
        let nonce = prover
            .find("ZkX509ProofInstanceV1::sample_v1(&mut checked_rng)")
            .expect("disjoint public nonce before private masks");
        let main_base = prover
            .find("commit_zk_x509_main_base_phase_v1_with_rng(")
            .expect("joined MAIN base root");
        let ca = prover
            .find("commit_ca_through_auxiliary_v1(")
            .expect("original compact-CA auxiliary phase");
        let joint_binding = prover
            .find(".bind_joint_credential_pre_aux_v1_with_rng(")
            .expect("joint MAIN and CA root X5B1 with retained owner admission");
        let main_aux = prover
            .find("finish_joint_main_ca_v1(")
            .expect("paired original MAIN and CA composition/DEEP/FRI");
        let envelope = prover
            .find("encode_zk_x509_credential_envelope_v1(")
            .expect("X5S1 envelope");
        let self_check = prover
            .find("verify_zk_x509_credential_subproofs_v1(")
            .expect("independent final self-check");
        assert!(quarantine < profile_gate);
        assert!(
            profile_gate < preparation
                && preparation < assembly
                && assembly < entropy
                && entropy < nonce
                && nonce < main_base
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
        let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
        assert_eq!(
            crate::privacy_profiles::compiled_privacy_profile_v1(protocol_id),
            Err(
                crate::privacy_profiles::CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete {
                    protocol_id,
                }
            )
        );
    }
}

#[cfg(test)]
#[path = "engine_prover_diagnostic.rs"]
pub(super) mod prover_diagnostic;
