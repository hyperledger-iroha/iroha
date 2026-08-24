use halo2_proofs::halo2curves::{
    ff::PrimeField,
    group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
    pasta::{EpAffine, EqAffine, Fp, Fq},
};

use super::*;

fn accumulator_bytes(
    parity: StateRecursiveFoldParityV2,
) -> [u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2] {
    let mut bytes = [0_u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2];
    match parity {
        StateRecursiveFoldParityV2::Eq => {
            let scalar = Fp::from(7).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..]
                .copy_from_slice(EqAffine::generator().to_bytes().as_ref());
        }
        StateRecursiveFoldParityV2::Ep => {
            let scalar = Fq::from(11).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..]
                .copy_from_slice(EpAffine::generator().to_bytes().as_ref());
        }
    }
    bytes
}

fn accumulator(parity: StateRecursiveFoldParityV2) -> CanonicalStateAccumulatorV2 {
    CanonicalStateAccumulatorV2::decode(parity, &accumulator_bytes(parity))
        .expect("fixture accumulator is canonical")
}

fn ordered_inputs(
    parity: StateRecursiveFoldParityV2,
) -> [StateRecursiveFoldInputV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
    STATE_RECURSIVE_FOLD_INPUT_ORDER_V2
        .map(|role| StateRecursiveFoldInputV2::new(role, accumulator(parity)))
}

fn proof() -> OpaqueStateBgh19ProofV2 {
    OpaqueStateBgh19ProofV2::decode(&[0x5a; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2])
        .expect("opaque fixture has exact nonzero length")
}

fn state_words(
    parity: StateRecursiveFoldParityV2,
) -> [u32; STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2] {
    let mut words = [0_u32; STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2];
    words[..8].copy_from_slice(&[
        2,
        2,
        STATE_RECURSIVE_FOLD_K_V2,
        parity as u32,
        1,
        2,
        8,
        STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2 as u32,
    ]);
    for (digest_index, start) in [8, 16, 24, 32, 40, 48, 56, 64, 72, 80]
        .into_iter()
        .enumerate()
    {
        words[start..start + 8].fill(u32::try_from(digest_index + 1).expect("small digest tag"));
    }
    words[STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2] = 1;
    words[STATE_RECURSIVE_FOLD_SCALE_WORD_V2] = 28;
    for (target, chunk) in words[STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2..]
        .iter_mut()
        .zip(accumulator_bytes(parity).chunks_exact(4))
    {
        *target = u32::from_le_bytes(chunk.try_into().expect("four-byte accumulator word"));
    }
    words
}

#[test]
fn k17_six_input_bgh19_ledger_is_exact_and_non_authorizing() {
    assert_eq!(STATE_RECURSIVE_FOLD_K_V2, 17);
    assert_eq!(STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2, 576);
    assert_eq!(STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2, 6);
    assert_eq!(
        STATE_RECURSIVE_FOLD_INPUT_ACCUMULATOR_BYTES_PER_PARITY_V2,
        3_456
    );
    assert_eq!(STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2, 2 * 17 + 8);
    assert_eq!(STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2, 1_344);
    assert_eq!(STATE_RECURSIVE_FOLD_OUTPUT_ACCUMULATOR_BYTES_V2, 576);
    assert_eq!(
        STATE_RECURSIVE_FOLD_LEDGER_V2,
        StateRecursiveFoldLedgerV2 {
            k: 17,
            parity_count: 2,
            inputs_per_parity: 6,
            accumulator_bytes: 576,
            input_accumulator_bytes_per_parity: 3_456,
            bgh19_elements: 42,
            bgh19_proof_bytes_per_parity: 1_344,
            output_accumulator_bytes_per_parity: 576,
            query_instance: false,
        }
    );
    assert!(
        STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2
            <= STATE_RECURSIVE_FOLD_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2
    );
    assert_eq!(STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2, 6_272);
    assert_eq!(
        STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2,
        6_528
    );
    assert!(
        STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2
            < STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2
    );
}

#[test]
fn exact_parity_local_input_order_and_sources_are_frozen() {
    assert_eq!(
        STATE_RECURSIVE_FOLD_INPUT_ORDER_V2,
        [
            StateRecursiveFoldInputRoleV2::Parent0Current,
            StateRecursiveFoldInputRoleV2::Parent0Prior,
            StateRecursiveFoldInputRoleV2::Parent1Current,
            StateRecursiveFoldInputRoleV2::Parent1Prior,
            StateRecursiveFoldInputRoleV2::GuardCurrent,
            StateRecursiveFoldInputRoleV2::GuardPrior,
        ]
    );
    assert_eq!(
        STATE_RECURSIVE_FOLD_INPUT_ORDER_V2.map(StateRecursiveFoldInputRoleV2::source),
        [
            StateRecursiveFoldInputSourceV2::CurrentProofAccumulator,
            StateRecursiveFoldInputSourceV2::PriorLineageAtWord93,
            StateRecursiveFoldInputSourceV2::CurrentProofAccumulator,
            StateRecursiveFoldInputSourceV2::PriorLineageAtWord93,
            StateRecursiveFoldInputSourceV2::CurrentProofAccumulator,
            StateRecursiveFoldInputSourceV2::GuardPriorLineageAtWord192,
        ]
    );

    for parity in [
        StateRecursiveFoldParityV2::Eq,
        StateRecursiveFoldParityV2::Ep,
    ] {
        let envelope = UnverifiedStateRecursiveFoldEnvelopeV2::from_structural_parts(
            parity,
            ordered_inputs(parity),
            proof(),
            accumulator(parity),
        )
        .expect("canonical structural envelope");
        assert_eq!(envelope.parity(), parity);
        let roles: [StateRecursiveFoldInputRoleV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] =
            std::array::from_fn(|index| envelope.inputs()[index].role());
        assert_eq!(roles, STATE_RECURSIVE_FOLD_INPUT_ORDER_V2);
        assert_eq!(
            envelope.proof().as_bytes().len(),
            STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2
        );
        assert_eq!(envelope.claimed_output().parity(), parity);
        assert_eq!(
            envelope.inputs()[0].accumulator().as_bytes().len(),
            STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2
        );
    }
}

#[test]
fn accumulator_codec_is_parity_typed_canonical_and_bootstrap_rejecting() {
    for parity in [
        StateRecursiveFoldParityV2::Eq,
        StateRecursiveFoldParityV2::Ep,
    ] {
        let bytes = accumulator_bytes(parity);
        let decoded = CanonicalStateAccumulatorV2::decode(parity, &bytes)
            .expect("parity-local canonical accumulator");
        assert_eq!(decoded.parity(), parity);
        assert_eq!(decoded.as_bytes(), &bytes);
        assert_eq!(
            CanonicalStateAccumulatorV2::decode(parity, &bytes[..575]),
            Err(StateRecursiveFoldCodecErrorV2::InvalidAccumulatorLength { actual: 575 })
        );
        assert_eq!(
            CanonicalStateAccumulatorV2::decode(parity, &[0; 576]),
            Err(StateRecursiveFoldCodecErrorV2::BootstrapLineageForbidden)
        );
    }

    let mut bad_scalar = accumulator_bytes(StateRecursiveFoldParityV2::Eq);
    bad_scalar[..32].fill(0xff);
    assert_eq!(
        CanonicalStateAccumulatorV2::decode(StateRecursiveFoldParityV2::Eq, &bad_scalar),
        Err(StateRecursiveFoldCodecErrorV2::NonCanonicalRoundChallenge { index: 0 })
    );

    let mut bad_point = accumulator_bytes(StateRecursiveFoldParityV2::Ep);
    bad_point[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..].fill(0xff);
    assert_eq!(
        CanonicalStateAccumulatorV2::decode(StateRecursiveFoldParityV2::Ep, &bad_point),
        Err(StateRecursiveFoldCodecErrorV2::NonCanonicalFoldedGenerator)
    );

    let mut identity = accumulator_bytes(StateRecursiveFoldParityV2::Eq);
    identity[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..]
        .copy_from_slice(EqAffine::identity().to_bytes().as_ref());
    assert_eq!(
        CanonicalStateAccumulatorV2::decode(StateRecursiveFoldParityV2::Eq, &identity),
        Err(StateRecursiveFoldCodecErrorV2::IdentityFoldedGenerator)
    );
}

#[test]
fn order_parity_and_bgh19_framing_mutations_fail_closed() {
    let mut reordered = ordered_inputs(StateRecursiveFoldParityV2::Eq);
    reordered.swap(0, 1);
    assert_eq!(
        UnverifiedStateRecursiveFoldEnvelopeV2::from_structural_parts(
            StateRecursiveFoldParityV2::Eq,
            reordered,
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateRecursiveFoldCodecErrorV2::InputOrderMismatch { index: 0 })
    );

    let mut cross_parity = ordered_inputs(StateRecursiveFoldParityV2::Eq);
    cross_parity[4] = StateRecursiveFoldInputV2::new(
        StateRecursiveFoldInputRoleV2::GuardCurrent,
        accumulator(StateRecursiveFoldParityV2::Ep),
    );
    assert_eq!(
        UnverifiedStateRecursiveFoldEnvelopeV2::from_structural_parts(
            StateRecursiveFoldParityV2::Eq,
            cross_parity,
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateRecursiveFoldCodecErrorV2::ParityMismatch { index: 4 })
    );

    assert_eq!(
        UnverifiedStateRecursiveFoldEnvelopeV2::from_structural_parts(
            StateRecursiveFoldParityV2::Eq,
            ordered_inputs(StateRecursiveFoldParityV2::Eq),
            proof(),
            accumulator(StateRecursiveFoldParityV2::Ep),
        ),
        Err(StateRecursiveFoldCodecErrorV2::ParityMismatch { index: 6 })
    );
    assert_eq!(
        OpaqueStateBgh19ProofV2::decode(&vec![1; 1_343]),
        Err(StateRecursiveFoldCodecErrorV2::InvalidBgh19ProofLength { actual: 1_343 })
    );
    assert_eq!(
        OpaqueStateBgh19ProofV2::decode(&vec![1; 1_345]),
        Err(StateRecursiveFoldCodecErrorV2::InvalidBgh19ProofLength { actual: 1_345 })
    );
    assert_eq!(
        OpaqueStateBgh19ProofV2::decode(&[0; 1_344]),
        Err(StateRecursiveFoldCodecErrorV2::ZeroBgh19Proof)
    );
}

#[test]
fn paired_fold_result_owner_retains_exact_eq_then_ep_claims_without_verifying_them() {
    let eq_proof_bytes = [0x31; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2];
    let ep_proof_bytes = [0x52; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2];
    let pair = UnverifiedStateRecursiveFoldResultPairV2::from_eq_then_ep(
        OpaqueStateBgh19ProofV2::decode(&eq_proof_bytes).expect("canonical Eq framing"),
        accumulator(StateRecursiveFoldParityV2::Eq),
        OpaqueStateBgh19ProofV2::decode(&ep_proof_bytes).expect("canonical Ep framing"),
        accumulator(StateRecursiveFoldParityV2::Ep),
    )
    .expect("canonical Eq-then-Ep result ownership");

    assert_eq!(pair.eq_proof().as_bytes(), &eq_proof_bytes);
    assert_eq!(pair.ep_proof().as_bytes(), &ep_proof_bytes);
    assert_eq!(
        pair.eq_claimed_output().parity(),
        StateRecursiveFoldParityV2::Eq
    );
    assert_eq!(
        pair.ep_claimed_output().parity(),
        StateRecursiveFoldParityV2::Ep
    );
}

#[test]
fn paired_fold_result_owner_rejects_claimed_output_parity_substitution() {
    assert_eq!(
        UnverifiedStateRecursiveFoldResultPairV2::from_eq_then_ep(
            proof(),
            accumulator(StateRecursiveFoldParityV2::Ep),
            proof(),
            accumulator(StateRecursiveFoldParityV2::Ep),
        ),
        Err(StateRecursiveFoldResultOwnershipErrorV2::EqClaimedOutputParityMismatch)
    );
    assert_eq!(
        UnverifiedStateRecursiveFoldResultPairV2::from_eq_then_ep(
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateRecursiveFoldResultOwnershipErrorV2::EpClaimedOutputParityMismatch)
    );
}

#[test]
fn direct_state_abi_word93_lineage_and_final_padding_are_exact() {
    assert_eq!(STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2, 237);
    assert_eq!(STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2, 93);
    assert_eq!(STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2, 144);
    assert_eq!(STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2, 88);
    assert_eq!(STATE_RECURSIVE_FOLD_SCALE_WORD_V2, 92);
    assert_eq!(STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2, 34);
    assert_eq!(STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2, 7);
    assert_eq!(STATE_RECURSIVE_FOLD_FINAL_CELL_ZERO_PADDING_WORDS_V2, 1);
    assert_eq!(34 * 7, 237 + 1);
    assert!(!STATE_RECURSIVE_FOLD_QUERY_INSTANCE_V2);
    assert!(!STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_IN_CURRENT_INSTANCES_V2);
    assert_eq!(
        STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_INSTANCE_WORDS_V2,
        0
    );
    assert!(!STATE_RECURSIVE_FOLD_ZERO_BOOTSTRAP_ACCEPTED_V2);

    for parity in [
        StateRecursiveFoldParityV2::Eq,
        StateRecursiveFoldParityV2::Ep,
    ] {
        let words = state_words(parity);
        let lineage = decode_prior_lineage_from_state_words_v2(parity, &words)
            .expect("word-93 lineage decodes");
        assert_eq!(lineage.parity(), parity);
        assert_eq!(lineage.as_bytes(), &accumulator_bytes(parity));

        let cells = pack_state_words_v2(parity, &words).expect("canonical packing");
        assert_eq!(cells.len(), 34);
        assert!(cells[33][24..].iter().all(|byte| *byte == 0));
        assert_eq!(
            unpack_state_cells_v2(parity, &cells).expect("canonical unpacking"),
            words
        );

        assert_eq!(
            unpack_state_cells_v2(parity, &cells[..33]),
            Err(StateRecursiveFoldCodecErrorV2::InvalidStateLength { actual: 33 })
        );
        let mut bad_padding = cells;
        bad_padding[33][27] = 1;
        assert_eq!(
            unpack_state_cells_v2(parity, &bad_padding),
            Err(StateRecursiveFoldCodecErrorV2::NonCanonicalFinalCellPadding)
        );
    }
}

#[test]
fn state_header_bootstrap_and_parity_substitutions_fail_closed() {
    let mut wrong_header = state_words(StateRecursiveFoldParityV2::Eq);
    wrong_header[2] = 16;
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Eq, &wrong_header),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader)
    );

    let mut zero_digest = state_words(StateRecursiveFoldParityV2::Eq);
    zero_digest[40..48].fill(0);
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Eq, &zero_digest),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader)
    );

    let mut zero_amount = state_words(StateRecursiveFoldParityV2::Eq);
    zero_amount
        [STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2..STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2 + 4]
        .fill(0);
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Eq, &zero_amount),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader)
    );

    let mut excessive_scale = state_words(StateRecursiveFoldParityV2::Eq);
    excessive_scale[STATE_RECURSIVE_FOLD_SCALE_WORD_V2] = 29;
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Eq, &excessive_scale),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader)
    );

    let mut bootstrap = state_words(StateRecursiveFoldParityV2::Ep);
    bootstrap[STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2..].fill(0);
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Ep, &bootstrap),
        Err(StateRecursiveFoldCodecErrorV2::BootstrapLineageForbidden)
    );

    let eq_words = state_words(StateRecursiveFoldParityV2::Eq);
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Ep, &eq_words),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader)
    );
    assert_eq!(
        decode_prior_lineage_from_state_words_v2(StateRecursiveFoldParityV2::Eq, &eq_words[..236],),
        Err(StateRecursiveFoldCodecErrorV2::InvalidStateLength { actual: 236 })
    );
}

#[test]
fn blocker_inventory_and_every_live_gate_remain_false() {
    assert_eq!(
        STATE_RECURSIVE_FOLD_BLOCKERS_V2,
        [
            StateRecursiveFoldBlockerV2::EccStrategyUnresolved,
            StateRecursiveFoldBlockerV2::GuardBundleUnavailable,
            StateRecursiveFoldBlockerV2::FinalStatePairTargetUnresolved {
                qualification_target_bytes: 6_272,
                absolute_maximum_bytes: 6_528,
            },
            StateRecursiveFoldBlockerV2::ArtifactInventoryUnavailable,
            StateRecursiveFoldBlockerV2::MeasuredProcessRssUnavailable {
                qualification_bytes: 268_435_456,
            },
        ]
    );
    assert!(STATE_RECURSIVE_FOLD_DECLARED_V2);
    assert!(!STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2);
}

#[test]
fn source_guards_keep_the_candidate_privately_declared_sealed_and_non_live() {
    let source = include_str!("state_recursive_fold.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod state_recursive_fold;")
            .count(),
        1
    );
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim_end().ends_with("mod state_recursive_fold;"))
            .count(),
        1
    );
    assert!(parent.contains("#[path = \"offline_cash_v2/state_recursive_fold.rs\"]"));
    assert!(!parent.contains("pub mod state_recursive_fold"));
    assert!(source.contains("Privately declared, non-authorizing"));
    assert!(source.contains("BGH19 transcript bytes remain opaque"));
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim() == "mod sealed {")
            .count(),
        1
    );
    assert_eq!(
        source
            .lines()
            .filter(|line| line.trim_end().ends_with("mod sealed {"))
            .count(),
        1
    );
    assert!(source.contains("mod sealed {\n    pub trait Sealed {}\n}"));
    assert!(!source.contains("pub(super) mod sealed"));
    assert!(
        !source
            .lines()
            .any(|line| line.trim() == "pub(super) trait Sealed {}")
    );
    assert!(
        !source
            .lines()
            .any(|line| { line.trim() == "pub(in crate::zk::offline_cash_v2) trait Sealed {}" })
    );
    assert!(source.contains("enum StateRecursiveCompilerAdapterV2 {}"));
    assert!(source.contains("enum StateRecursiveCircuitAdapterV2 {}"));
    assert!(source.contains("enum StateRecursiveArtifactAdapterV2 {}"));
    assert!(source.contains("enum StateRecursiveProductionAdapterV2 {}"));
    assert!(source.contains("GuardBundle ABI word 192"));
    assert!(source.contains("fn new(\n        role: StateRecursiveFoldInputRoleV2"));
    assert!(
        !source.contains("pub(super) const fn new(\n        role: StateRecursiveFoldInputRoleV2")
    );
    assert!(!source.contains("impl Circuit for"));
    assert!(!source.contains("verify_proof("));
    assert!(!source.contains("VerifierIPA"));
    assert!(!source.contains("STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2: bool = true"));
    assert!(!source.contains("STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2: bool = true"));
}

#[test]
fn provenance_bound_input_view_is_borrowed_and_position_tagged() {
    let eq = accumulator(StateRecursiveFoldParityV2::Eq);
    let view =
        StateRecursiveFoldInputRefV2::new(StateRecursiveFoldInputRoleV2::Parent0Current, &eq);
    assert_eq!(view.role(), StateRecursiveFoldInputRoleV2::Parent0Current);
    assert_eq!(view.accumulator(), &eq);
    assert_eq!(view.accumulator().parity(), StateRecursiveFoldParityV2::Eq);
}

#[test]
fn six_input_owner_consumes_exact_p0_p1_and_guard_types() {
    let _assembly_signature: fn(
        ProvenanceBoundStateParent0InputsV2,
        ProvenanceBoundStateParent1InputsV2,
        ProvenanceBoundStateGuardInputsV2,
    ) -> ProvenanceBoundStateSixInputSetV2 = assemble_provenance_bound_state_six_input_set_v2;

    let source = include_str!("state_recursive_fold.rs");
    assert!(source.contains("pub(super) struct ProvenanceBoundStateSixInputSetV2"));
    assert!(source.contains("parent_0: ProvenanceBoundStateParent0InputsV2"));
    assert!(source.contains("parent_1: ProvenanceBoundStateParent1InputsV2"));
    assert!(source.contains("guard: ProvenanceBoundStateGuardInputsV2"));
    assert!(source.contains("remains a cloneable codec-only value"));
    assert!(source.contains("cloning\n/// it conveys no proof authority"));
    assert!(source.contains("whose fields and constructors remain private"));
    assert!(source.contains(
        ") -> [StateRecursiveFoldInputRefV2<'_>; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2]"
    ));
    assert!(!source.contains("impl Clone for ProvenanceBoundStateSixInputSetV2"));
    assert!(!source.contains("into_six_input_parts"));
}

#[test]
fn fold_result_carrier_consumes_the_six_input_owner_and_exposes_only_borrowed_views() {
    let _assembly_signature: fn(
        ProvenanceBoundStateSixInputSetV2,
        UnverifiedStateRecursiveFoldResultPairV2,
    ) -> ProvenanceBoundStateRecursiveFoldResultV2 =
        assemble_provenance_bound_state_recursive_fold_result_v2;
    let _fail_closed_signature: fn(
        ProvenanceBoundStateRecursiveFoldResultV2,
    ) -> Result<
        core::convert::Infallible,
        StateRecursiveFoldResultOwnershipErrorV2,
    > = fail_closed_provenance_bound_state_recursive_fold_result_v2;

    let source = include_str!("state_recursive_fold.rs");
    let carrier = source
        .split_once("pub(super) struct ProvenanceBoundStateRecursiveFoldResultV2 {")
        .and_then(|(_, tail)| tail.split_once("/// Structurally valid six-input envelope."))
        .map(|(carrier, _)| carrier)
        .expect("fold-result carrier source section exists");

    assert!(carrier.contains("inputs: ProvenanceBoundStateSixInputSetV2"));
    assert!(carrier.contains("result: UnverifiedStateRecursiveFoldResultPairV2"));
    assert!(carrier.contains("self.inputs.eq_inputs()"));
    assert!(carrier.contains("self.inputs.ep_inputs()"));
    assert!(carrier.contains("ProvenanceBoundStateRecursiveFoldResultV2 { inputs, result }"));
    assert!(carrier.contains("StateRecursiveFoldResultOwnershipErrorV2::VerificationUnavailable"));
    assert!(!carrier.contains("[StateRecursiveFoldInputV2;"));
    assert!(!carrier.contains("into_parts"));
    assert!(!carrier.contains("impl Clone for ProvenanceBoundStateRecursiveFoldResultV2"));
    assert!(!carrier.contains("impl Copy for ProvenanceBoundStateRecursiveFoldResultV2"));
    assert!(!source.contains("VerifiedStateRecursiveFoldResult"));
    assert!(!source.contains("verify_provenance_bound_state_recursive_fold_result_v2"));
}

#[test]
fn fold_result_carrier_keeps_every_authority_gate_false_and_adds_no_crypto_surface() {
    assert!(!STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2);

    let source = include_str!("state_recursive_fold.rs");
    assert!(!source.contains("impl Circuit for"));
    assert!(!source.contains("verify_proof("));
    assert!(!source.contains("create_proof("));
    assert!(!source.contains("VerifierIPA"));
    assert!(!source.contains("semantic_hash"));
    assert!(!source.contains("semantic_domain"));
}

#[test]
fn provenance_bound_eq_and_ep_views_freeze_the_same_exact_six_role_order() {
    fn assert_role_order(source: &str) {
        let mut cursor = 0;
        for role in [
            "StateRecursiveFoldInputRoleV2::Parent0Current",
            "StateRecursiveFoldInputRoleV2::Parent0Prior",
            "StateRecursiveFoldInputRoleV2::Parent1Current",
            "StateRecursiveFoldInputRoleV2::Parent1Prior",
            "StateRecursiveFoldInputRoleV2::GuardCurrent",
            "StateRecursiveFoldInputRoleV2::GuardPrior",
        ] {
            let relative = source[cursor..]
                .find(role)
                .unwrap_or_else(|| panic!("missing ordered role {role}"));
            cursor += relative + role.len();
        }
    }

    let source = include_str!("state_recursive_fold.rs");
    let eq_start = source
        .find("pub(super) fn eq_inputs(\n        &self,\n    ) -> [StateRecursiveFoldInputRefV2")
        .expect("borrowed Eq view exists");
    let ep_start = source[eq_start..]
        .find("pub(super) fn ep_inputs(\n        &self,\n    ) -> [StateRecursiveFoldInputRefV2")
        .map(|offset| eq_start + offset)
        .expect("borrowed Ep view exists");
    let assembly_start = source[ep_start..]
        .find("pub(super) fn assemble_provenance_bound_state_six_input_set_v2")
        .map(|offset| ep_start + offset)
        .expect("consuming assembly function exists");
    let eq_body = &source[eq_start..ep_start];
    let ep_body = &source[ep_start..assembly_start];

    assert_role_order(eq_body);
    assert_role_order(ep_body);
    assert_eq!(
        eq_body
            .matches("StateRecursiveFoldInputRefV2::new(")
            .count(),
        6
    );
    assert_eq!(
        ep_body
            .matches("StateRecursiveFoldInputRefV2::new(")
            .count(),
        6
    );
    assert!(eq_body.contains("self.parent_0.eq_current()"));
    assert!(eq_body.contains("self.parent_0.eq_prior()"));
    assert!(eq_body.contains("self.parent_1.eq_current()"));
    assert!(eq_body.contains("self.parent_1.eq_prior()"));
    assert!(eq_body.contains("self.guard.eq_inputs()[0].accumulator()"));
    assert!(eq_body.contains("self.guard.eq_inputs()[1].accumulator()"));
    assert!(ep_body.contains("self.parent_0.ep_current()"));
    assert!(ep_body.contains("self.parent_0.ep_prior()"));
    assert!(ep_body.contains("self.parent_1.ep_current()"));
    assert!(ep_body.contains("self.parent_1.ep_prior()"));
    assert!(ep_body.contains("self.guard.ep_inputs()[0].accumulator()"));
    assert!(ep_body.contains("self.guard.ep_inputs()[1].accumulator()"));
}
