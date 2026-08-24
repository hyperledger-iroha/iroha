use halo2_proofs::{
    halo2curves::{
        CurveExt as _,
        group::Curve as _,
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
};
use rand_core_06::{CryptoRng, Error as RngError, RngCore};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::{
        AccumulationSchemeProver,
        ipa::{Bgh19, IpaAccumulator, IpaAs, IpaProvingKey},
    },
    util::arithmetic::{Domain, root_of_unity},
};

use super::super::{
    OfflineCashHalo2CircuitRoleV2, OfflineCashHalo2ParityV2,
    guard_bundle_provenance::{
        guard_bundle_state_handoff_for_native_relation_test_v2,
        offline_cash_halo2_protocol_source_identity_v2,
    },
    state_lineage::{
        OfflineCashEpParentLineageV2, OfflineCashEqParentLineageV2, OfflineCashStateAbiFieldsV2,
        OfflineCashStateOperationV2, OfflineCashStatePublicInstancesV2,
    },
    state_recursive_fold::{
        UnverifiedStateRecursiveFoldResultPairV2,
        assemble_provenance_bound_state_recursive_fold_result_v2,
        assemble_provenance_bound_state_six_input_set_v2,
        state_guard_inputs_from_verified_guard_bundle_v2,
    },
    state_semantic_parent_provenance::{
        UnverifiedOfflineCashStateSemanticParentPairV2,
        VerifiedOfflineCashStateSemanticParentHandoffV2,
    },
};
use super::*;

struct DeterministicTestRng(u64);

impl DeterministicTestRng {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }
}

impl RngCore for DeterministicTestRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.0 = value;
        value
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        for chunk in destination.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
        self.fill_bytes(destination);
        Ok(())
    }
}

impl CryptoRng for DeterministicTestRng {}

fn canonical_eq(
    accumulator: &IpaAccumulator<EqAffine, NativeLoader>,
) -> CanonicalStateAccumulatorV2 {
    CanonicalStateAccumulatorV2::decode(
        StateRecursiveFoldParityV2::Eq,
        &encode_accumulator_v2(accumulator).expect("k=17 Eq output encodes"),
    )
    .expect("generated Eq accumulator is canonical")
}

fn canonical_ep(
    accumulator: &IpaAccumulator<EpAffine, NativeLoader>,
) -> CanonicalStateAccumulatorV2 {
    CanonicalStateAccumulatorV2::decode(
        StateRecursiveFoldParityV2::Ep,
        &encode_accumulator_v2(accumulator).expect("k=17 Ep output encodes"),
    )
    .expect("generated Ep accumulator is canonical")
}

fn state_fields(parity: OfflineCashHalo2ParityV2) -> OfflineCashStateAbiFieldsV2 {
    OfflineCashStateAbiFieldsV2 {
        operation: OfflineCashStateOperationV2::SendSplit,
        release_digest: [0x11; 32],
        protocol_digest: offline_cash_halo2_protocol_source_identity_v2(
            parity,
            OfflineCashHalo2CircuitRoleV2::State,
        )
        .digest(),
        semantic_digest: [0x22; 32],
        context_digest: [0x33; 32],
        request_digest: [0x44; 32],
        parent_0: [0x55; 32],
        parent_1: [0x66; 32],
        result: [0x77; 32],
        link: [0x88; 32],
        transition_digest: [0x99; 32],
        amount: 123,
        scale: 2,
    }
}

fn semantic_parent_handoff(
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: &CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: &CanonicalStateAccumulatorV2,
) -> VerifiedOfflineCashStateSemanticParentHandoffV2 {
    assert_eq!(eq_prior.parity(), StateRecursiveFoldParityV2::Eq);
    assert_eq!(ep_prior.parity(), StateRecursiveFoldParityV2::Ep);
    let eq_prior = OfflineCashEqParentLineageV2::decode(eq_prior.as_bytes())
        .expect("native relation Eq parent prior is canonical");
    let ep_prior = OfflineCashEpParentLineageV2::decode(ep_prior.as_bytes())
        .expect("native relation Ep parent prior is canonical");
    let eq_instances = OfflineCashStatePublicInstancesV2::eq(
        state_fields(OfflineCashHalo2ParityV2::Eq),
        &eq_prior,
    )
    .expect("native relation Eq parent instances are canonical");
    let ep_instances = OfflineCashStatePublicInstancesV2::ep(
        state_fields(OfflineCashHalo2ParityV2::Ep),
        &ep_prior,
    )
    .expect("native relation Ep parent instances are canonical");
    let provenance =
        UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(eq_instances, ep_instances)
            .expect("native relation parent provenance is canonical");
    VerifiedOfflineCashStateSemanticParentHandoffV2::from_test_verified_parts_v2(
        provenance, eq_current, ep_current,
    )
    .expect("native relation test parent handoff is parity-canonical")
}

fn provenance_bound_candidate(
    eq_inputs: &[CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    eq_proof: OpaqueStateBgh19ProofV2,
    eq_output: CanonicalStateAccumulatorV2,
    ep_inputs: &[CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    ep_proof: OpaqueStateBgh19ProofV2,
    ep_output: CanonicalStateAccumulatorV2,
) -> ProvenanceBoundStateRecursiveFoldResultV2 {
    let parent_0 = semantic_parent_handoff(
        eq_inputs[0].clone(),
        &eq_inputs[1],
        ep_inputs[0].clone(),
        &ep_inputs[1],
    )
    .into_fold_accumulator_parts_v2()
    .bind_parent_0_v2();
    let parent_1 = semantic_parent_handoff(
        eq_inputs[2].clone(),
        &eq_inputs[3],
        ep_inputs[2].clone(),
        &ep_inputs[3],
    )
    .into_fold_accumulator_parts_v2()
    .bind_parent_1_v2();
    let guard = guard_bundle_state_handoff_for_native_relation_test_v2(
        eq_inputs[4].clone(),
        &eq_inputs[5],
        ep_inputs[4].clone(),
        &ep_inputs[5],
    )
    .expect("native relation GuardBundle handoff is canonical");
    let inputs = assemble_provenance_bound_state_six_input_set_v2(
        parent_0,
        parent_1,
        state_guard_inputs_from_verified_guard_bundle_v2(guard),
    );
    let result = UnverifiedStateRecursiveFoldResultPairV2::from_eq_then_ep(
        eq_proof, eq_output, ep_proof, ep_output,
    )
    .expect("native relation Eq-then-Ep result pair is canonical");
    assemble_provenance_bound_state_recursive_fold_result_v2(inputs, result)
}

fn eq_fixture_v2() -> (
    [CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    OpaqueStateBgh19ProofV2,
    CanonicalStateAccumulatorV2,
) {
    let params = ParamsIPA::<EqAffine>::new(STATE_RECURSIVE_FOLD_K_V2);
    let inputs: [IpaAccumulator<EqAffine, NativeLoader>;
        STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] = std::array::from_fn(|input_index| {
        IpaAccumulator::new(
            (0..STATE_RECURSIVE_FOLD_K_V2)
                .map(|round| Fp::from(100 + input_index as u64 * 32 + u64::from(round)))
                .collect(),
            params.get_g()[input_index + 3],
        )
    });
    let canonical_inputs = std::array::from_fn(|index| canonical_eq(&inputs[index]));
    let hash_to_curve = Eq::hash_to_curve(HALO2_PARAMETERS_DOMAIN_V2);
    let proving_key = IpaProvingKey::new(
        Domain::new(
            STATE_RECURSIVE_FOLD_K_V2 as usize,
            root_of_unity(STATE_RECURSIVE_FOLD_K_V2 as usize),
        ),
        params.get_g().to_vec(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    drop(params);
    let proving_svk = proving_key.svk();
    let verifier_svk = eq_succinct_verifying_key_v2();
    assert_eq!(proving_svk.domain.k, verifier_svk.domain.k);
    assert_eq!(proving_svk.domain.r#gen, verifier_svk.domain.r#gen);
    assert_eq!(proving_svk.g, verifier_svk.g);
    assert_eq!(proving_svk.h, verifier_svk.h);
    assert_eq!(proving_svk.s, verifier_svk.s);
    let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<EqAffine>>::init(Vec::new());
    let output = <IpaAs<EqAffine, Bgh19> as AccumulationSchemeProver<EqAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        DeterministicTestRng::new(0x4551_1717_1717_1717),
    )
    .expect("deterministic Eq relation proof");
    let proof = transcript.finalize();
    assert_eq!(
        proof.len(),
        super::super::state_recursive_fold::STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2
    );
    (
        canonical_inputs,
        OpaqueStateBgh19ProofV2::decode(&proof).expect("exact Eq fold proof"),
        canonical_eq(&output),
    )
}

fn ep_fixture_v2() -> (
    [CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    OpaqueStateBgh19ProofV2,
    CanonicalStateAccumulatorV2,
) {
    let params = ParamsIPA::<EpAffine>::new(STATE_RECURSIVE_FOLD_K_V2);
    let inputs: [IpaAccumulator<EpAffine, NativeLoader>;
        STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] = std::array::from_fn(|input_index| {
        IpaAccumulator::new(
            (0..STATE_RECURSIVE_FOLD_K_V2)
                .map(|round| Fq::from(700 + input_index as u64 * 32 + u64::from(round)))
                .collect(),
            params.get_g()[input_index + 11],
        )
    });
    let canonical_inputs = std::array::from_fn(|index| canonical_ep(&inputs[index]));
    let hash_to_curve = Ep::hash_to_curve(HALO2_PARAMETERS_DOMAIN_V2);
    let proving_key = IpaProvingKey::new(
        Domain::new(
            STATE_RECURSIVE_FOLD_K_V2 as usize,
            root_of_unity(STATE_RECURSIVE_FOLD_K_V2 as usize),
        ),
        params.get_g().to_vec(),
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    drop(params);
    let proving_svk = proving_key.svk();
    let verifier_svk = ep_succinct_verifying_key_v2();
    assert_eq!(proving_svk.domain.k, verifier_svk.domain.k);
    assert_eq!(proving_svk.domain.r#gen, verifier_svk.domain.r#gen);
    assert_eq!(proving_svk.g, verifier_svk.g);
    assert_eq!(proving_svk.h, verifier_svk.h);
    assert_eq!(proving_svk.s, verifier_svk.s);
    let mut transcript = Blake2bWrite::<_, EpAffine, Challenge255<EpAffine>>::init(Vec::new());
    let output = <IpaAs<EpAffine, Bgh19> as AccumulationSchemeProver<EpAffine>>::create_proof(
        &proving_key,
        &inputs,
        &mut transcript,
        DeterministicTestRng::new(0x4550_1717_1717_1717),
    )
    .expect("deterministic Ep relation proof");
    let proof = transcript.finalize();
    assert_eq!(
        proof.len(),
        super::super::state_recursive_fold::STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2
    );
    (
        canonical_inputs,
        OpaqueStateBgh19ProofV2::decode(&proof).expect("exact Ep fold proof"),
        canonical_ep(&output),
    )
}

fn input_refs(
    inputs: &[CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
) -> [&CanonicalStateAccumulatorV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
    std::array::from_fn(|index| &inputs[index])
}

#[test]
fn full_k17_native_pair_accepts_and_every_substitution_fails_closed() {
    let (eq_inputs, eq_proof, eq_output) = eq_fixture_v2();
    let (ep_inputs, ep_proof, ep_output) = ep_fixture_v2();

    verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &eq_output,
        input_refs(&ep_inputs),
        &ep_proof,
        &ep_output,
    )
    .expect("canonical Eq-then-Ep relation pair verifies");

    let seal = verify_provenance_bound_state_recursive_fold_native_relation_v2(
        provenance_bound_candidate(
            &eq_inputs,
            eq_proof.clone(),
            eq_output.clone(),
            &ep_inputs,
            ep_proof.clone(),
            ep_output.clone(),
        ),
    )
    .expect("ownership-consuming Eq-then-Ep relation verifies atomically");
    assert_eq!(seal.eq_output(), &eq_output);
    assert_eq!(seal.ep_output(), &ep_output);

    let mut mutated_eq_bytes = *eq_proof.as_bytes();
    mutated_eq_bytes[0] ^= 1;
    let mutated_eq = OpaqueStateBgh19ProofV2::decode(&mutated_eq_bytes)
        .expect("mutated Eq proof remains structurally framed");
    let eq_mutation_error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &mutated_eq,
        &eq_output,
        input_refs(&ep_inputs),
        &ep_proof,
        &ep_output,
    )
    .expect_err("Eq transcript mutation must fail");
    assert_eq!(eq_mutation_error.parity(), StateRecursiveFoldParityV2::Eq);

    for index in 0..STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2 {
        let mut substituted_eq_inputs = input_refs(&eq_inputs);
        substituted_eq_inputs[index] =
            &eq_inputs[(index + 1) % STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2];
        let error = verify_relation_pair_v2(
            substituted_eq_inputs,
            &eq_proof,
            &eq_output,
            input_refs(&ep_inputs),
            &ep_proof,
            &ep_output,
        )
        .expect_err("every ordered Eq input substitution must fail");
        assert_eq!(error.parity(), StateRecursiveFoldParityV2::Eq);

        let mut substituted_ep_inputs = input_refs(&ep_inputs);
        substituted_ep_inputs[index] =
            &ep_inputs[(index + 1) % STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2];
        let error = verify_relation_pair_v2(
            input_refs(&eq_inputs),
            &eq_proof,
            &eq_output,
            substituted_ep_inputs,
            &ep_proof,
            &ep_output,
        )
        .expect_err("every ordered Ep input substitution must fail");
        assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);

        let mut cross_parity_eq_inputs = input_refs(&eq_inputs);
        cross_parity_eq_inputs[index] = &ep_inputs[index];
        let error = verify_relation_pair_v2(
            cross_parity_eq_inputs,
            &eq_proof,
            &eq_output,
            input_refs(&ep_inputs),
            &ep_proof,
            &ep_output,
        )
        .expect_err("every cross-parity Eq input must fail before transcript parsing");
        assert_eq!(error.parity(), StateRecursiveFoldParityV2::Eq);
        assert_eq!(
            error.stage(),
            StateRecursiveFoldNativeRelationStageV2::InputDecode
        );
        assert_eq!(error.input_index(), Some(index));
        assert!(!error.panic_was_contained());

        let mut cross_parity_ep_inputs = input_refs(&ep_inputs);
        cross_parity_ep_inputs[index] = &eq_inputs[index];
        let error = verify_relation_pair_v2(
            input_refs(&eq_inputs),
            &eq_proof,
            &eq_output,
            cross_parity_ep_inputs,
            &ep_proof,
            &ep_output,
        )
        .expect_err("every cross-parity Ep input must fail before transcript parsing");
        assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);
        assert_eq!(
            error.stage(),
            StateRecursiveFoldNativeRelationStageV2::InputDecode
        );
        assert_eq!(error.input_index(), Some(index));
        assert!(!error.panic_was_contained());
    }

    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &eq_inputs[0],
        input_refs(&ep_inputs),
        &ep_proof,
        &ep_output,
    )
    .expect_err("canonical but wrong Eq claimed output must fail");
    assert_eq!(
        error.stage(),
        StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch
    );

    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &eq_output,
        input_refs(&ep_inputs),
        &ep_proof,
        &ep_inputs[0],
    )
    .expect_err("canonical but wrong Ep claimed output must fail");
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);
    assert_eq!(
        error.stage(),
        StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch
    );

    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &ep_output,
        input_refs(&ep_inputs),
        &ep_proof,
        &ep_output,
    )
    .expect_err("cross-parity Eq claimed output must fail before transcript parsing");
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Eq);
    assert_eq!(
        error.stage(),
        StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch
    );

    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &eq_output,
        input_refs(&ep_inputs),
        &ep_proof,
        &eq_output,
    )
    .expect_err("cross-parity Ep claimed output must fail before transcript parsing");
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);
    assert_eq!(
        error.stage(),
        StateRecursiveFoldNativeRelationStageV2::ClaimedOutputMatch
    );

    let mut mutated_ep_bytes = *ep_proof.as_bytes();
    mutated_ep_bytes[17] ^= 1;
    let mutated_ep = OpaqueStateBgh19ProofV2::decode(&mutated_ep_bytes)
        .expect("mutated Ep proof remains structurally framed");
    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &eq_proof,
        &eq_output,
        input_refs(&ep_inputs),
        &mutated_ep,
        &ep_output,
    )
    .expect_err("Ep failure must reject the atomic pair after Eq");
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);

    let error = verify_relation_pair_v2(
        input_refs(&eq_inputs),
        &mutated_eq,
        &eq_output,
        input_refs(&ep_inputs),
        &mutated_ep,
        &ep_output,
    )
    .expect_err("when both proofs fail, Eq must be rejected before Ep is attempted");
    assert_eq!(error, eq_mutation_error);

    let error = match verify_provenance_bound_state_recursive_fold_native_relation_v2(
        provenance_bound_candidate(
            &eq_inputs,
            eq_proof.clone(),
            eq_output.clone(),
            &ep_inputs,
            mutated_ep.clone(),
            ep_output.clone(),
        ),
    ) {
        Ok(_) => panic!("Ep failure must not construct an ownership-consuming relation seal"),
        Err(error) => error,
    };
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Ep);

    let hostile = OpaqueStateBgh19ProofV2::decode(
        &[0xff; super::super::state_recursive_fold::STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2],
    )
    .expect("hostile proof has exact nonzero framing");
    let contained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        verify_relation_pair_v2(
            input_refs(&eq_inputs),
            &hostile,
            &eq_output,
            input_refs(&ep_inputs),
            &ep_proof,
            &ep_output,
        )
    }));
    assert!(
        contained.is_ok(),
        "hostile transcript rejection must not unwind"
    );
    let error = contained
        .expect("outer catch completed")
        .expect_err("hostile transcript must fail closed");
    assert_eq!(error.parity(), StateRecursiveFoldParityV2::Eq);
    assert_eq!(
        error.stage(),
        StateRecursiveFoldNativeRelationStageV2::TranscriptParse
    );
    assert_eq!(error.input_index(), None);
    assert!(!error.panic_was_contained());
}

#[test]
fn native_relation_seal_is_private_move_only_atomic_and_non_authorizing() {
    assert!(STATE_RECURSIVE_FOLD_NATIVE_RELATION_KERNEL_IMPLEMENTED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2);

    let source = include_str!("state_recursive_fold_native.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    let guard_source = include_str!("guard_bundle_provenance.rs");
    assert_eq!(
        parent
            .matches("#[path = \"offline_cash_v2/state_recursive_fold_native.rs\"]")
            .count(),
        1
    );
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod state_recursive_fold_native;")
            .count(),
        1
    );
    assert_eq!(
        parent
            .lines()
            .filter(|line| {
                line.trim_end()
                    .ends_with("mod state_recursive_fold_native;")
            })
            .count(),
        1
    );

    let seal_boundary = "impl std::error::Error for StateRecursiveFoldNativeRelationErrorV2 {}\n";
    let seal_start = source
        .find(seal_boundary)
        .map(|offset| offset + seal_boundary.len())
        .expect("native relation error boundary remains present");
    let seal_end = source
        .find("fn eq_succinct_verifying_key_v2()")
        .expect("Eq succinct verifying-key derivation remains present");
    assert!(seal_start < seal_end);
    let seal_source = &source[seal_start..seal_end];
    assert!(seal_source.contains(
        "pub(super) struct StateRecursiveFoldNativeRelationSealV2 {\n    candidate: ProvenanceBoundStateRecursiveFoldResultV2,\n}"
    ));
    assert!(!seal_source.contains("#[derive"));
    assert_eq!(
        seal_source
            .matches("ProvenanceBoundStateRecursiveFoldResultV2")
            .count(),
        1
    );
    assert_eq!(
        seal_source
            .matches("pub(super) const fn eq_output(&self) -> &CanonicalStateAccumulatorV2 {")
            .count(),
        1
    );
    assert_eq!(
        seal_source
            .matches("pub(super) const fn ep_output(&self) -> &CanonicalStateAccumulatorV2 {")
            .count(),
        1
    );
    assert_eq!(seal_source.matches("pub(super)").count(), 3);
    assert_eq!(seal_source.matches("fn ").count(), 2);
    assert!(!seal_source.contains("pub(crate)"));
    assert!(!seal_source.contains("pub(in "));
    assert_eq!(
        source
            .matches(".zip(STATE_RECURSIVE_FOLD_INPUT_ORDER_V2)")
            .count(),
        2
    );
    assert!(source.contains("verify_relation_pair_v2("));
    assert!(source.contains("Ok(StateRecursiveFoldNativeRelationSealV2 { candidate })"));
    assert!(!seal_source.contains("Clone for StateRecursiveFoldNativeRelationSealV2"));
    assert!(!seal_source.contains("Copy for StateRecursiveFoldNativeRelationSealV2"));
    assert!(!seal_source.contains("fn candidate("));
    assert!(!seal_source.contains("fn into_parts"));
    assert_eq!(
        source
            .matches("let mut cursor = Cursor::new(proof.as_bytes().as_slice());")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("Blake2bRead::<_, C, Challenge255<C>>::init(&mut cursor)")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("if cursor.position() != proof.as_bytes().len() as u64 {")
            .count(),
        1
    );
    assert!(source.contains("StateRecursiveFoldNativeRelationStageV2::TranscriptConsumption"));
    assert_eq!(
        guard_source
            .matches("fn guard_bundle_state_handoff_for_native_relation_test_v2(")
            .count(),
        1
    );
    assert!(guard_source.contains(
        "#[cfg(test)]\npub(super) fn guard_bundle_state_handoff_for_native_relation_test_v2("
    ));
    assert!(!source.contains("AccumulationDecider"));
    assert!(!source.contains("kagemusha_recursion_adapter"));
    assert!(!source.contains("state_terminal_candidate"));
    assert!(!source.contains("STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2: bool = true"));
}
