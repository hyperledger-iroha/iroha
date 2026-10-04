//! Private genuine producer for the separate ordinary Terminal and compact Wrapper circuits.
//!
//! The Native production caller constructs witnesses only while borrowing the held financial
//! secret and captured original selection, then re-admits this output with the actual verifier.
//! Loaded common-role keys are genuine release artifacts; the physical circuits below are the
//! distinct ordinary relations. Old circuit witness constructors and OEM wire types are unused.
use super::super::{
    DigestV1, KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1,
    ordinary_cash_commit_wrapper::{
        OrdinaryCashCommitWrapperWitnessV1, build_ordinary_cash_commit_wrapper_ep_v1,
        build_ordinary_cash_commit_wrapper_eq_v1, collect_ordinary_cash_commit_wrapper_audits_v1,
    },
    ordinary_cash_terminal_circuit::{
        OrdinaryCashTerminalCircuitWitnessV1, build_ordinary_cash_terminal_ep_v1,
        build_ordinary_cash_terminal_eq_v1, collect_ordinary_cash_terminal_audits_v1,
    },
    ordinary_cash_terminal_verifier::{
        ORDINARY_INNER_TERMINAL_MAX_BYTES_V1, ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1,
        OrdinaryCashProofPairWireV1, OrdinaryCashTerminalPublicV1,
    },
};
use super::*;

/// Generated proof data. Only separate Native or stateless admission can supply authority.
pub(in super::super) struct GeneratedOrdinaryCashPairV1 {
    pub(in super::super) original: Vec<u8>,
    pub(in super::super) eq_protocol: PlonkProtocol<EqAffine>,
    pub(in super::super) ep_protocol: PlonkProtocol<EpAffine>,
    pub(in super::super) eq_instances: Vec<Fp>,
    pub(in super::super) ep_instances: Vec<Fq>,
    pub(in super::super) eq_current: KagemushaEqAccumulatorV1,
    pub(in super::super) ep_current: KagemushaEpAccumulatorV1,
    pub(in super::super) eq_history: KagemushaEqAccumulatorV1,
    pub(in super::super) ep_history: KagemushaEpAccumulatorV1,
    /// Actual producer-created Eq/Ep complete-inner BGH19 folds, retained only for Wrapper.
    pub(in super::super) wrapper_history_fold_originals:
        Option<[[u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]; 2]>,
}
fn fail(message: impl core::fmt::Display) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::CircuitBuild(message.to_string())
}
fn require_public_keys(
    p: &OrdinaryCashTerminalPublicV1,
    eq: [DigestV1; 6],
    ep: [DigestV1; 6],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    p.validate().map_err(fail)?;
    if eq[..4] != ep[..4]
        || [p.release_id, p.suite_id, p.vk_set_digest] != [eq[0], eq[1], eq[2]]
        || eq[3] == [0; 32]
        || eq[4] != p.eq_protocol_digest
        || ep[4] != p.ep_protocol_digest
        || eq[4] == ep[4]
        || eq[5] != ep[5]
    {
        return Err(fail(
            "ordinary proof parities do not share exact released keys/profile",
        ));
    }
    Ok(())
}
/// Prove the sole full ordinary semantic SHA queue with genuine released shard/claim keys.
/// Caller-selected messages are never accepted: the exact same Terminal semantic constructor
/// generates both ordered queues, and the later Terminal consumes every padded job/history.
pub(super) fn prove_ordinary_cash_terminal_sha_claim_v1(
    eq: &KagemushaLoadedEqMintHashArtifactsV1,
    ep: &KagemushaLoadedEpMintHashArtifactsV1,
    public: &OrdinaryCashTerminalPublicV1,
    semantic:&super::super::composite::ordinary_cash_terminal_math::OrdinaryCashTerminalSemanticWitnessV1<'_>,
    eq_candidate: &[Vec<Fp>],
    ep_candidate: &[Vec<Fq>],
    seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
    validate_loaded_typed_sha_pair_v1(eq, ep)?;
    if public.release_id != eq.release_id {
        return Err(fail(
            "ordinary Terminal SHA queue belongs to another release",
        ));
    }
    let (eq_messages, ep_messages) =
        super::super::ordinary_cash_terminal_circuit::plan_ordinary_cash_terminal_sha_v1(
            public,
            semantic,
            eq_candidate,
            ep_candidate,
        )
        .map_err(fail)?;
    prove_kagemusha_typed_sha_claim_v1(
        eq,
        ep,
        KagemushaPairedShaMessagesV1::try_new(eq_messages, ep_messages)?,
        seed,
    )
}

/// Actual first-release full Send Terminal proof under already authenticated distinct common roles.
/// Both discovery graphs are dropped before reciprocal construction; proof graphs are consumed
/// one parity at a time by the maintained prover. All native current/history decisions follow.
pub(super) fn prove_ordinary_cash_terminal_pair_v1(
    eq: &KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
    ep: &KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
    mut witness: OrdinaryCashTerminalCircuitWitnessV1<'_>,
    seed: &KagemushaRecoverySeedV1,
) -> Result<GeneratedOrdinaryCashPairV1, KagemushaArtifactGenerationErrorV1> {
    require_public_keys(
        &witness.public,
        [
            eq.release_id,
            eq.suite_id,
            eq.vk_digest,
            eq.artifact_manifest_digest,
            eq.protocol_digest,
            eq.profile_digest,
        ],
        [
            ep.release_id,
            ep.suite_id,
            ep.vk_digest,
            ep.artifact_manifest_digest,
            ep.protocol_digest,
            ep.profile_digest,
        ],
    )?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    let pins = [
        eq.eq_claim_protocol_digest,
        eq.ep_claim_protocol_digest,
        eq.eq_shard_protocol_digest,
        eq.ep_shard_protocol_digest,
    ];
    for other in [
        [
            ep.eq_claim_protocol_digest,
            ep.ep_claim_protocol_digest,
            ep.eq_shard_protocol_digest,
            ep.ep_shard_protocol_digest,
        ],
        witness.eq.hash_claim.protocol_digests,
        witness.ep.hash_claim.protocol_digests,
    ] {
        validate_terminal_hash_protocol_pins_v1(pins, other)?;
    }
    if eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || !eq
            .enabled_hardware_profiles
            .contains(&witness.public.app_credential_profile_id)
    {
        return Err(fail(
            "ordinary Terminal profile not in actual released catalog",
        ));
    }
    let audits = collect_ordinary_cash_terminal_audits_v1(&eq.parameters, &ep.parameters, &witness)
        .map_err(fail)?;
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, eq_instances) =
        build_ordinary_cash_terminal_eq_v1(&eq.parameters, &witness, &audits).map_err(fail)?;
    if eq_instances
        != witness
            .public
            .public_column::<Fp>(witness.eq.successor_history)
            .map_err(fail)?
        || !same_base_params(&circuit.params(), &eq.circuit_params)
    {
        return Err(fail(
            "ordinary Eq Terminal exact graph/public layout differs from released key",
        ));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        seed,
    )?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Eq,
        "ordinary inner Terminal",
        &eq_protocol,
        &eq_proof,
    )?;
    if native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq).map_err(fail)?
        != eq.protocol_digest
    {
        return Err(fail("ordinary Eq Terminal protocol changed after key load"));
    }
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(fail)?,
    )
    .map_err(|e| fail(e.to_string()))?;
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(witness.eq.successor_history)
        .map_err(|e| fail(e.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_current).map_err(fail)?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_history).map_err(fail)?;
    halo2_proofs::release_allocator_slack();
    let (circuit, ep_instances) =
        build_ordinary_cash_terminal_ep_v1(&ep.parameters, &witness, &audits).map_err(fail)?;
    if ep_instances
        != witness
            .public
            .public_column::<Fq>(witness.ep.successor_history)
            .map_err(fail)?
        || !same_base_params(&circuit.params(), &ep.circuit_params)
    {
        return Err(fail(
            "ordinary Ep Terminal exact graph/public layout differs from released key",
        ));
    }
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        seed,
    )?;
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Ep,
        "ordinary inner Terminal",
        &ep_protocol,
        &ep_proof,
    )?;
    if native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep).map_err(fail)?
        != ep.protocol_digest
    {
        return Err(fail("ordinary Ep Terminal protocol changed after key load"));
    }
    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(fail)?,
    )
    .map_err(|e| fail(e.to_string()))?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(witness.ep.successor_history)
        .map_err(|e| fail(e.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_current).map_err(fail)?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_history).map_err(fail)?;
    finish_pair(
        1,
        witness.public,
        eq.artifact_manifest_digest,
        eq_protocol,
        ep_protocol,
        eq_instances,
        ep_instances,
        eq_proof,
        ep_proof,
        eq_current,
        ep_current,
        eq_history,
        ep_history,
        None,
    )
}

/// Actual compact physical Wrapper proof over the complete generated inner Terminal and history.
/// The returned raw original is data; publication still requires whole selection/CAS admission.
pub(super) fn prove_ordinary_cash_wrapper_pair_v1(
    eq: &KagemushaLoadedEqCommitWrapperArtifactsV1,
    ep: &KagemushaLoadedEpCommitWrapperArtifactsV1,
    mut witness: OrdinaryCashCommitWrapperWitnessV1<'_>,
    seed: &KagemushaRecoverySeedV1,
) -> Result<GeneratedOrdinaryCashPairV1, KagemushaArtifactGenerationErrorV1> {
    let wrapper_history_fold_originals = [
        witness
            .eq
            .history_fold_proof
            .try_into()
            .map_err(|_| fail("ordinary Eq Wrapper original fold length differs"))?,
        witness
            .ep
            .history_fold_proof
            .try_into()
            .map_err(|_| fail("ordinary Ep Wrapper original fold length differs"))?,
    ];
    require_public_keys(
        &witness.public,
        [
            eq.release_id,
            eq.suite_id,
            eq.vk_digest,
            eq.artifact_manifest_digest,
            eq.protocol_digest,
            eq.profile_digest,
        ],
        [
            ep.release_id,
            ep.suite_id,
            ep.vk_digest,
            ep.artifact_manifest_digest,
            ep.protocol_digest,
            ep.profile_digest,
        ],
    )?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Eq, &eq.circuit_params)?;
    validate_terminal_authorization_profile(KagemushaPastaParityV1::Ep, &ep.circuit_params)?;
    if native_parent_protocol_digest_v1(witness.eq.protocol, KagemushaPastaParityV1::Eq)
        .map_err(fail)?
        != eq.terminal_authorization_protocol_digest
        || native_parent_protocol_digest_v1(witness.ep.protocol, KagemushaPastaParityV1::Ep)
            .map_err(fail)?
            != ep.terminal_authorization_protocol_digest
        || eq.enabled_hardware_profiles != ep.enabled_hardware_profiles
        || !eq
            .enabled_hardware_profiles
            .contains(&witness.public.app_credential_profile_id)
    {
        return Err(fail(
            "ordinary Wrapper exact released inner protocols/profile differ",
        ));
    }
    let audits =
        collect_ordinary_cash_commit_wrapper_audits_v1(&eq.parameters, &ep.parameters, &witness)
            .map_err(fail)?;
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, eq_instances) =
        build_ordinary_cash_commit_wrapper_eq_v1(&eq.parameters, &witness, &audits)
            .map_err(fail)?;
    if eq_instances
        != witness
            .public
            .public_column::<Fp>(witness.eq.successor_history)
            .map_err(fail)?
        || !same_base_params(&circuit.params(), &eq.circuit_params)
    {
        return Err(fail("ordinary Eq Wrapper exact released layout differs"));
    }
    let eq_proof = create_eq_proof_with_key_v1(
        &eq.parameters,
        &eq.proving_key,
        circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        seed,
    )?;
    validate_paired_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "ordinary Wrapper",
        &eq_protocol,
    )?;
    if native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq).map_err(fail)?
        != eq.protocol_digest
    {
        return Err(fail("ordinary Eq Wrapper actual protocol changed"));
    }
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq.parameters, &eq_protocol, &eq_proof, &eq_instances)
            .map_err(fail)?,
    )
    .map_err(|e| fail(e.to_string()))?;
    let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(witness.eq.successor_history)
        .map_err(|e| fail(e.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_current).map_err(fail)?;
    decide_kagemusha_eq_accumulator_v1(&eq.parameters, &eq_history).map_err(fail)?;
    halo2_proofs::release_allocator_slack();
    let (circuit, ep_instances) =
        build_ordinary_cash_commit_wrapper_ep_v1(&ep.parameters, &witness, &audits)
            .map_err(fail)?;
    if ep_instances
        != witness
            .public
            .public_column::<Fq>(witness.ep.successor_history)
            .map_err(fail)?
        || !same_base_params(&circuit.params(), &ep.circuit_params)
    {
        return Err(fail("ordinary Ep Wrapper exact released layout differs"));
    }
    let ep_proof = create_ep_proof_with_key_v1(
        &ep.parameters,
        &ep.proving_key,
        circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        seed,
    )?;
    validate_paired_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "ordinary Wrapper",
        &ep_protocol,
    )?;
    if native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep).map_err(fail)?
        != ep.protocol_digest
    {
        return Err(fail("ordinary Ep Wrapper actual protocol changed"));
    }
    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep.parameters, &ep_protocol, &ep_proof, &ep_instances)
            .map_err(fail)?,
    )
    .map_err(|e| fail(e.to_string()))?;
    let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(witness.ep.successor_history)
        .map_err(|e| fail(e.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_current).map_err(fail)?;
    decide_kagemusha_ep_accumulator_v1(&ep.parameters, &ep_history).map_err(fail)?;
    finish_pair(
        2,
        witness.public,
        eq.artifact_manifest_digest,
        eq_protocol,
        ep_protocol,
        eq_instances,
        ep_instances,
        eq_proof,
        ep_proof,
        eq_current,
        ep_current,
        eq_history,
        ep_history,
        Some(wrapper_history_fold_originals),
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_pair(
    relation: u8,
    public: OrdinaryCashTerminalPublicV1,
    manifest: DigestV1,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_instances: Vec<Fp>,
    ep_instances: Vec<Fq>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_current: KagemushaEqAccumulatorV1,
    ep_current: KagemushaEpAccumulatorV1,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    wrapper_history_fold_originals: Option<[[u8; KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]; 2]>,
) -> Result<GeneratedOrdinaryCashPairV1, KagemushaArtifactGenerationErrorV1> {
    if (relation == 2) != wrapper_history_fold_originals.is_some() {
        return Err(fail(
            "ordinary exact Wrapper fold originals differ from role",
        ));
    }
    let wire = OrdinaryCashProofPairWireV1 {
        version: 1,
        relation,
        release_id: public.release_id,
        artifact_manifest_digest: manifest,
        eq_protocol_digest: public.eq_protocol_digest,
        ep_protocol_digest: public.ep_protocol_digest,
        eq_deferred_audit: public.eq_deferred_audit,
        ep_deferred_audit: public.ep_deferred_audit,
        eq_proof,
        ep_proof,
        eq_history: *eq_history.as_bytes(),
        ep_history: *ep_history.as_bytes(),
    };
    let original = norito::encode_canonical(&wire).map_err(|e| fail(e.to_string()))?;
    let maximum = if relation == 1 {
        ORDINARY_INNER_TERMINAL_MAX_BYTES_V1
    } else {
        KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    };
    if original.len() > maximum {
        return Err(fail(
            "ordinary actual complete proof original exceeds maintained bound",
        ));
    }
    Ok(GeneratedOrdinaryCashPairV1 {
        original,
        eq_protocol,
        ep_protocol,
        eq_instances,
        ep_instances,
        eq_current,
        ep_current,
        eq_history,
        ep_history,
        wrapper_history_fold_originals,
    })
}
