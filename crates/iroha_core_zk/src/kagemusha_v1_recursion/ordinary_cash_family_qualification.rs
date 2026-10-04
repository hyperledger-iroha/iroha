//! Actual ordinary Terminal/Wrapper key and proof factories for the full mathematical corridor.
//!
//! Uses the production physical constructors, consuming resource preflight, structured key
//! serialization, actual prover and native current/history decisions. Outputs are fixture data;
//! they cannot install a release, Native owner, FI clock or durable DATA effect.
use super::super::{
    ordinary_cash_commit_wrapper::{
        OrdinaryCashCommitWrapperWitnessV1, build_ordinary_cash_commit_wrapper_ep_v1,
        build_ordinary_cash_commit_wrapper_eq_v1, collect_ordinary_cash_commit_wrapper_audits_v1,
    },
    ordinary_cash_terminal_circuit::{
        OrdinaryCashTerminalCircuitWitnessV1, build_ordinary_cash_terminal_ep_v1,
        build_ordinary_cash_terminal_eq_v1, collect_ordinary_cash_terminal_audits_v1,
    },
    ordinary_cash_terminal_verifier::ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1,
};
use super::ordinary_cash_terminal_generation::{GeneratedOrdinaryCashPairV1, finish_pair};
use super::*;
use crate::kagemusha_v1_state::DigestV1;

/// Actual role material is retained only as canonical structured/processed originals.
/// The full financial corridor may inspect it but has no artifact-installation authority.
pub(super) struct OrdinaryCashRoleKeysForTestingV1 {
    pub(super) eq_parameters: Arc<[u8]>,
    pub(super) eq_proving_key: Arc<[u8]>,
    pub(super) eq_verifying_key: Arc<[u8]>,
    pub(super) ep_parameters: Arc<[u8]>,
    pub(super) ep_proving_key: Arc<[u8]>,
    pub(super) ep_verifying_key: Arc<[u8]>,
    pub(super) eq_base: BaseCircuitParams,
    pub(super) ep_base: BaseCircuitParams,
}
/// Data from real proof generation. Independent Native/service admission is still mandatory.
pub(super) struct OrdinaryCashProofForTestingV1 {
    pub(super) keys: OrdinaryCashRoleKeysForTestingV1,
    pub(super) generated: GeneratedOrdinaryCashPairV1,
}

/// Discover only the genuine Wrapper physical descriptor over selected83-cell Terminal parsers.
/// This emits no PK/current proof/history grant. The final full-cycle caller must compare its
/// final actually proved Wrapper descriptor to this exact candidate before accepting the graph.
pub(super) fn discover_ordinary_wrapper_layout_for_testing_v1(
    source: &OrdinaryCashCommitWrapperWitnessV1<'_>,
) -> (PlonkProtocol<EqAffine>, PlonkProtocol<EpAffine>) {
    use super::super::ordinary_cash_commit_wrapper::OrdinaryCashCommitWrapperHalfWitnessV1;
    let mut witness = OrdinaryCashCommitWrapperWitnessV1 {
        public: source.public.clone(),
        eq: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: source.eq.protocol,
            instances: source.eq.instances,
            proof: source.eq.proof,
            history: source.eq.history,
            history_fold_proof: source.eq.history_fold_proof,
            successor_history: source.eq.successor_history,
        },
        ep: OrdinaryCashCommitWrapperHalfWitnessV1 {
            protocol: source.ep.protocol,
            instances: source.ep.instances,
            proof: source.ep.proof,
            history: source.ep.history,
            history_fold_proof: source.ep.history_fold_proof,
            successor_history: source.ep.successor_history,
        },
    };
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let audits = collect_ordinary_cash_commit_wrapper_audits_v1(&eq, &ep, &witness).unwrap();
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, _) = build_ordinary_cash_commit_wrapper_eq_v1(&eq, &witness, &audits).unwrap();
    let vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &eq,
        circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary Wrapper layout",
        "value-free graph discovery",
    )
    .unwrap();
    let eq_protocol = compile(
        &eq,
        &vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(vk);
    halo2_proofs::release_allocator_slack();
    let (circuit, _) = build_ordinary_cash_commit_wrapper_ep_v1(&ep, &witness, &audits).unwrap();
    let vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &ep,
        circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary Wrapper layout",
        "value-free graph discovery",
    )
    .unwrap();
    let ep_protocol = compile(
        &ep,
        &vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(vk);
    (eq_protocol, ep_protocol)
}

/// Generate and prove the same full ordinary Terminal body used by the shipping producer.
/// State and both purpose-specific Guards are real inputs; parser padding never reaches a
/// positive decision. Final public self-protocol values are witnesses, not key-hash iteration.
pub(super) fn generate_ordinary_terminal_for_testing_v1(
    mut witness: OrdinaryCashTerminalCircuitWitnessV1<'_>,
    manifest: DigestV1,
    seed: &KagemushaRecoverySeedV1,
) -> OrdinaryCashProofForTestingV1 {
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let audits = collect_ordinary_cash_terminal_audits_v1(&eq, &ep, &witness).unwrap();
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, _) = build_ordinary_cash_terminal_eq_v1(&eq, &witness, &audits).unwrap();
    let eq_base = circuit.params();
    let eq_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &eq,
        circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary Terminal",
        "actual Terminal verifier layout",
    )
    .unwrap();
    let eq_vk_original = eq_vk.to_bytes(SerdeFormat::Processed);
    let eq_protocol = compile(
        &eq,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(eq_vk);
    halo2_proofs::release_allocator_slack();
    let (circuit, _) = build_ordinary_cash_terminal_ep_v1(&ep, &witness, &audits).unwrap();
    let ep_base = circuit.params();
    let ep_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &ep,
        circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary Terminal",
        "actual Terminal verifier layout",
    )
    .unwrap();
    let ep_vk_original = ep_vk.to_bytes(SerdeFormat::Processed);
    let ep_protocol = compile(
        &ep,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(ep_vk);
    drop(audits);
    witness.public.eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq).unwrap();
    witness.public.ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep).unwrap();
    let audits = collect_ordinary_cash_terminal_audits_v1(&eq, &ep, &witness).unwrap();
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, eq_instances) =
        build_ordinary_cash_terminal_eq_v1(&eq, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &eq_base));
    let eq_pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &eq,
        circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary Terminal",
        "actual ordinary Terminal proving key",
    )
    .unwrap();
    assert_eq!(
        eq_pk.get_vk().to_bytes(SerdeFormat::Processed),
        eq_vk_original,
        "public self-protocol identity rebinding must preserve the exact frozen key"
    );
    let (circuit, column) = build_ordinary_cash_terminal_eq_v1(&eq, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &eq_base));
    assert_eq!(column, eq_instances);
    let eq_proof = create_eq_proof_with_key_v1(
        &eq,
        &eq_pk,
        circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        seed,
    )
    .unwrap();
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Eq,
        "ordinary inner Terminal",
        &eq_protocol,
        &eq_proof,
    )
    .unwrap();
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq, &eq_protocol, &eq_proof, &eq_instances).unwrap(),
    )
    .unwrap();
    let eq_history =
        KagemushaEqAccumulatorV1::try_from_bytes(witness.eq.successor_history).unwrap();
    decide_kagemusha_eq_accumulator_v1(&eq, &eq_current).unwrap();
    decide_kagemusha_eq_accumulator_v1(&eq, &eq_history).unwrap();
    let (eq_parameters, eq_proving_key, eq_verifying_key) =
        build_generated_helper_parity(KagemushaPastaParityV1::Eq, "ordinary Terminal", &eq, eq_pk)
            .unwrap();
    halo2_proofs::release_allocator_slack();
    let (circuit, ep_instances) =
        build_ordinary_cash_terminal_ep_v1(&ep, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &ep_base));
    let ep_pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &ep,
        circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary Terminal",
        "actual ordinary Terminal proving key",
    )
    .unwrap();
    assert_eq!(
        ep_pk.get_vk().to_bytes(SerdeFormat::Processed),
        ep_vk_original,
        "opposite self-protocol identity rebinding must preserve the exact frozen key"
    );
    let (circuit, column) = build_ordinary_cash_terminal_ep_v1(&ep, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &ep_base));
    assert_eq!(column, ep_instances);
    let ep_proof = create_ep_proof_with_key_v1(
        &ep,
        &ep_pk,
        circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::TerminalAuthorization,
        seed,
    )
    .unwrap();
    validate_internal_recursive_proof_length(
        KagemushaPastaParityV1::Ep,
        "ordinary inner Terminal",
        &ep_protocol,
        &ep_proof,
    )
    .unwrap();
    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep, &ep_protocol, &ep_proof, &ep_instances).unwrap(),
    )
    .unwrap();
    let ep_history =
        KagemushaEpAccumulatorV1::try_from_bytes(witness.ep.successor_history).unwrap();
    decide_kagemusha_ep_accumulator_v1(&ep, &ep_current).unwrap();
    decide_kagemusha_ep_accumulator_v1(&ep, &ep_history).unwrap();
    let (ep_parameters, ep_proving_key, ep_verifying_key) =
        build_generated_helper_parity(KagemushaPastaParityV1::Ep, "ordinary Terminal", &ep, ep_pk)
            .unwrap();
    let generated = finish_pair(
        1,
        witness.public,
        manifest,
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
    .unwrap();
    let result = OrdinaryCashProofForTestingV1 {
        keys: OrdinaryCashRoleKeysForTestingV1 {
            eq_parameters,
            eq_proving_key,
            eq_verifying_key,
            ep_parameters,
            ep_proving_key,
            ep_verifying_key,
            eq_base,
            ep_base,
        },
        generated,
    };
    verify_retained_pair::<
        super::super::ordinary_cash_terminal_circuit::OrdinaryCashTerminalEqCircuitV1,
        super::super::ordinary_cash_terminal_circuit::OrdinaryCashTerminalEpCircuitV1,
    >(
        &result,
        [
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEq,
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEp,
            KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
        ],
        manifest,
    );
    result
}

/// Generate/prove the genuine compact Wrapper after the actual Terminal has been frozen.
/// The exact native producer folds of current Terminal+whole inner history are retained in
/// the sole frame. No inner-original/semantic-equality substitution is possible in this source.
pub(super) fn generate_ordinary_wrapper_for_testing_v1(
    mut witness: OrdinaryCashCommitWrapperWitnessV1<'_>,
    manifest: DigestV1,
    seed: &KagemushaRecoverySeedV1,
) -> OrdinaryCashProofForTestingV1 {
    let discovered = discover_ordinary_wrapper_layout_for_testing_v1(&witness);
    let folds = [
        witness
            .eq
            .history_fold_proof
            .try_into()
            .expect("complete Eq1280 fold original"),
        witness
            .ep
            .history_fold_proof
            .try_into()
            .expect("complete Ep1280 fold original"),
    ];
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let audits = collect_ordinary_cash_commit_wrapper_audits_v1(&eq, &ep, &witness).unwrap();
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, _) = build_ordinary_cash_commit_wrapper_eq_v1(&eq, &witness, &audits).unwrap();
    let eq_base = circuit.params();
    let vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &eq,
        circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary Wrapper",
        "actual compact Wrapper layout",
    )
    .unwrap();
    let eq_vk_original = vk.to_bytes(SerdeFormat::Processed);
    let eq_protocol = compile(
        &eq,
        &vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(vk);
    halo2_proofs::release_allocator_slack();
    let (circuit, _) = build_ordinary_cash_commit_wrapper_ep_v1(&ep, &witness, &audits).unwrap();
    let ep_base = circuit.params();
    let vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        &ep,
        circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary Wrapper",
        "actual compact Wrapper layout",
    )
    .unwrap();
    let ep_vk_original = vk.to_bytes(SerdeFormat::Processed);
    let ep_protocol = compile(
        &ep,
        &vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]),
    );
    drop(vk);
    drop(audits);
    witness.public.eq_protocol_digest =
        native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq).unwrap();
    witness.public.ep_protocol_digest =
        native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep).unwrap();
    let audits = collect_ordinary_cash_commit_wrapper_audits_v1(&eq, &ep, &witness).unwrap();
    witness.public.eq_deferred_audit = audits.eq_digest;
    witness.public.ep_deferred_audit = audits.ep_digest;
    let (circuit, eq_instances) =
        build_ordinary_cash_commit_wrapper_eq_v1(&eq, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &eq_base));
    let pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &eq,
        circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary Wrapper",
        "actual compact Wrapper proving key",
    )
    .unwrap();
    assert_eq!(pk.get_vk().to_bytes(SerdeFormat::Processed), eq_vk_original);
    let (circuit, column) =
        build_ordinary_cash_commit_wrapper_eq_v1(&eq, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &eq_base));
    assert_eq!(column, eq_instances);
    let eq_proof = create_eq_proof_with_key_v1(
        &eq,
        &pk,
        circuit,
        &eq_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        seed,
    )
    .unwrap();
    validate_paired_proof_length(KagemushaPastaParityV1::Eq, &eq_proof).unwrap();
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Eq,
        "ordinary Wrapper",
        &eq_protocol,
    )
    .unwrap();
    let eq_current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(&eq, &eq_protocol, &eq_proof, &eq_instances).unwrap(),
    )
    .unwrap();
    let eq_history =
        KagemushaEqAccumulatorV1::try_from_bytes(witness.eq.successor_history).unwrap();
    decide_kagemusha_eq_accumulator_v1(&eq, &eq_current).unwrap();
    decide_kagemusha_eq_accumulator_v1(&eq, &eq_history).unwrap();
    let (eq_parameters, eq_proving_key, eq_verifying_key) =
        build_generated_helper_parity(KagemushaPastaParityV1::Eq, "ordinary Wrapper", &eq, pk)
            .unwrap();
    halo2_proofs::release_allocator_slack();
    let (circuit, ep_instances) =
        build_ordinary_cash_commit_wrapper_ep_v1(&ep, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &ep_base));
    let pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &ep,
        circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary Wrapper",
        "actual compact Wrapper proving key",
    )
    .unwrap();
    assert_eq!(pk.get_vk().to_bytes(SerdeFormat::Processed), ep_vk_original);
    let (circuit, column) =
        build_ordinary_cash_commit_wrapper_ep_v1(&ep, &witness, &audits).unwrap();
    assert!(same_base_params(&circuit.params(), &ep_base));
    assert_eq!(column, ep_instances);
    let ep_proof = create_ep_proof_with_key_v1(
        &ep,
        &pk,
        circuit,
        &ep_instances,
        KagemushaProofRecoveryPhaseV1::CommitWrapper,
        seed,
    )
    .unwrap();
    validate_paired_proof_length(KagemushaPastaParityV1::Ep, &ep_proof).unwrap();
    validate_transport_protocol_profile(
        KagemushaPastaParityV1::Ep,
        "ordinary Wrapper",
        &ep_protocol,
    )
    .unwrap();
    let ep_current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(&ep, &ep_protocol, &ep_proof, &ep_instances).unwrap(),
    )
    .unwrap();
    let ep_history =
        KagemushaEpAccumulatorV1::try_from_bytes(witness.ep.successor_history).unwrap();
    decide_kagemusha_ep_accumulator_v1(&ep, &ep_current).unwrap();
    decide_kagemusha_ep_accumulator_v1(&ep, &ep_history).unwrap();
    let (ep_parameters, ep_proving_key, ep_verifying_key) =
        build_generated_helper_parity(KagemushaPastaParityV1::Ep, "ordinary Wrapper", &ep, pk)
            .unwrap();
    let generated = finish_pair(
        2,
        witness.public,
        manifest,
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
        Some(folds),
    )
    .unwrap();
    let result = OrdinaryCashProofForTestingV1 {
        keys: OrdinaryCashRoleKeysForTestingV1 {
            eq_parameters,
            eq_proving_key,
            eq_verifying_key,
            ep_parameters,
            ep_proving_key,
            ep_verifying_key,
            eq_base,
            ep_base,
        },
        generated,
    };
    verify_retained_pair::<
        super::super::ordinary_cash_commit_wrapper::OrdinaryCashCommitWrapperEqCircuitV1,
        super::super::ordinary_cash_commit_wrapper::OrdinaryCashCommitWrapperEpCircuitV1,
    >(
        &result,
        [
            KagemushaArtifactRoleV1::CommitWrapperPkEq,
            KagemushaArtifactRoleV1::CommitWrapperVkEq,
            KagemushaArtifactRoleV1::CommitWrapperPkEp,
            KagemushaArtifactRoleV1::CommitWrapperVkEp,
        ],
        manifest,
    );
    for (actual, found) in [
        (
            native_parent_protocol_digest_v1(
                &result.generated.eq_protocol,
                KagemushaPastaParityV1::Eq,
            )
            .unwrap(),
            native_parent_protocol_digest_v1(&discovered.0, KagemushaPastaParityV1::Eq).unwrap(),
        ),
        (
            native_parent_protocol_digest_v1(
                &result.generated.ep_protocol,
                KagemushaPastaParityV1::Ep,
            )
            .unwrap(),
            native_parent_protocol_digest_v1(&discovered.1, KagemushaPastaParityV1::Ep).unwrap(),
        ),
    ] {
        assert_eq!(
            actual, found,
            "proved Wrapper must use the discovered descriptor and exact key"
        );
    }
    result
}

/// Decode both actual role artifacts and verify the exact framed proof with the decoded
/// standalone keys. The existing full-cycle test drives Terminal and Wrapper through this.
fn verify_retained_pair<EqCircuit, EpCircuit>(
    value: &OrdinaryCashProofForTestingV1,
    roles: [KagemushaArtifactRoleV1; 4],
    manifest: DigestV1,
) where
    EqCircuit: halo2_proofs::plonk::Circuit<Fp, Params = BaseCircuitParams>,
    EpCircuit: halo2_proofs::plonk::Circuit<Fq, Params = BaseCircuitParams>,
{
    use super::super::ordinary_cash_terminal_verifier::OrdinaryCashProofPairWireV1;
    use super::ordinary_qualification_artifacts::{Originals, roundtrip};
    let relation = match roles[0] {
        KagemushaArtifactRoleV1::TerminalAuthorizationPkEq => 1,
        KagemushaArtifactRoleV1::CommitWrapperPkEq => 2,
        _ => panic!("qualification pair requires an exact Terminal or Wrapper role"),
    };
    let original: OrdinaryCashProofPairWireV1 =
        super::super::ordinary_cash_terminal_verifier::decode_profile_exact(
            &value.generated.original,
            relation,
            value.generated.public.release_id,
            // The factory supplies this separate original release-manifest context.
            manifest,
            [
                native_parent_protocol_digest_v1(
                    &value.generated.eq_protocol,
                    KagemushaPastaParityV1::Eq,
                )
                .unwrap(),
                native_parent_protocol_digest_v1(
                    &value.generated.ep_protocol,
                    KagemushaPastaParityV1::Ep,
                )
                .unwrap(),
            ],
            [
                ordinary_ipa_proof_profile_v1(&value.generated.eq_protocol)
                    .unwrap()
                    .byte_len,
                ordinary_ipa_proof_profile_v1(&value.generated.ep_protocol)
                    .unwrap()
                    .byte_len,
            ],
        )
        .unwrap();
    assert_eq!(
        norito::encode_canonical(&original).unwrap(),
        value.generated.original
    );
    assert_eq!(original.eq_history, *value.generated.eq_history.as_bytes());
    assert_eq!(original.ep_history, *value.generated.ep_history.as_bytes());
    assert_eq!(
        original.eq_deferred_audit,
        value.generated.public.eq_deferred_audit
    );
    assert_eq!(
        original.ep_deferred_audit,
        value.generated.public.ep_deferred_audit
    );
    assert_eq!(
        original.eq_protocol_digest,
        value.generated.public.eq_protocol_digest
    );
    assert_eq!(
        original.ep_protocol_digest,
        value.generated.public.ep_protocol_digest
    );
    let (eq, protocol) = roundtrip::<EqAffine, EqCircuit>(
        Originals {
            parameters: &value.keys.eq_parameters,
            proving_key: &value.keys.eq_proving_key,
            verifying_key: &value.keys.eq_verifying_key,
            proving_role: roles[0],
            verifying_role: roles[1],
            protocol: &value.generated.eq_protocol,
        },
        &canonical_kagemusha_eq_parameters_v1(),
        value.keys.eq_base.clone(),
        ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1,
    )
    .unwrap();
    assert_eq!(
        native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Eq).unwrap(),
        original.eq_protocol_digest
    );
    assert_eq!(
        value
            .generated
            .public
            .public_column::<Fp>(&original.eq_history)
            .unwrap(),
        value.generated.eq_instances
    );
    let current = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(
            &eq,
            &protocol,
            &original.eq_proof,
            &value.generated.eq_instances,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(current.as_bytes(), value.generated.eq_current.as_bytes());
    decide_kagemusha_eq_accumulator_v1(&eq, &current).unwrap();
    decide_kagemusha_eq_accumulator_v1(&eq, &value.generated.eq_history).unwrap();
    drop((eq, protocol));
    halo2_proofs::release_allocator_slack();
    let (ep, protocol) = roundtrip::<EpAffine, EpCircuit>(
        Originals {
            parameters: &value.keys.ep_parameters,
            proving_key: &value.keys.ep_proving_key,
            verifying_key: &value.keys.ep_verifying_key,
            proving_role: roles[2],
            verifying_role: roles[3],
            protocol: &value.generated.ep_protocol,
        },
        &canonical_kagemusha_ep_parameters_v1(),
        value.keys.ep_base.clone(),
        ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1,
    )
    .unwrap();
    assert_eq!(
        native_parent_protocol_digest_v1(&protocol, KagemushaPastaParityV1::Ep).unwrap(),
        original.ep_protocol_digest
    );
    assert_eq!(
        value
            .generated
            .public
            .public_column::<Fq>(&original.ep_history)
            .unwrap(),
        value.generated.ep_instances
    );
    let current = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(
            &ep,
            &protocol,
            &original.ep_proof,
            &value.generated.ep_instances,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(current.as_bytes(), value.generated.ep_current.as_bytes());
    decide_kagemusha_ep_accumulator_v1(&ep, &current).unwrap();
    decide_kagemusha_ep_accumulator_v1(&ep, &value.generated.ep_history).unwrap();
}
