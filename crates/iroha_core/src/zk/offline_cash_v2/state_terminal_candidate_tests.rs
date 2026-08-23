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
            let scalar = Fp::from(13).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_K_V2 as usize {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_K_V2 as usize * 32..]
                .copy_from_slice(EqAffine::generator().to_bytes().as_ref());
        }
        StateRecursiveFoldParityV2::Ep => {
            let scalar = Fq::from(17).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_K_V2 as usize {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_K_V2 as usize * 32..]
                .copy_from_slice(EpAffine::generator().to_bytes().as_ref());
        }
    }
    bytes
}

fn accumulator(parity: StateRecursiveFoldParityV2) -> CanonicalStateAccumulatorV2 {
    CanonicalStateAccumulatorV2::decode(parity, &accumulator_bytes(parity))
        .expect("fixture accumulator is canonical")
}

fn proof() -> OpaqueStateBgh19ProofV2 {
    OpaqueStateBgh19ProofV2::decode(&[0xa5; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2])
        .expect("exact nonzero opaque proof")
}

fn successor_fold(parity: StateRecursiveFoldParityV2) -> UnverifiedStateTerminalSuccessorFoldV2 {
    UnverifiedStateTerminalSuccessorFoldV2::from_structural_parts(
        parity,
        [
            StateTerminalSuccessorFoldInputV2::new(
                StateTerminalSuccessorFoldInputRoleV2::Current,
                accumulator(parity),
            ),
            StateTerminalSuccessorFoldInputV2::new(
                StateTerminalSuccessorFoldInputRoleV2::Prior,
                accumulator(parity),
            ),
        ],
        proof(),
        accumulator(parity),
    )
    .expect("canonical structural successor fold")
}

#[test]
fn public_terminal_order_remains_the_exact_twelve_stage_contract() {
    assert_eq!(
        STATE_TERMINAL_CANDIDATE_ORDER_V2,
        [
            StateTerminalCandidateStageV2::CanonicalWireDecode,
            StateTerminalCandidateStageV2::StatementAndLiveness,
            StateTerminalCandidateStageV2::ArtifactAndProtocolAuthentication,
            StateTerminalCandidateStageV2::ReconstructPublicInstances,
            StateTerminalCandidateStageV2::EqCurrentProof,
            StateTerminalCandidateStageV2::EpCurrentProof,
            StateTerminalCandidateStageV2::EqCurrentDecision,
            StateTerminalCandidateStageV2::EpCurrentDecision,
            StateTerminalCandidateStageV2::EqParentLineageDecision,
            StateTerminalCandidateStageV2::EpParentLineageDecision,
            StateTerminalCandidateStageV2::PersistSuccessorLineages,
            StateTerminalCandidateStageV2::IssueReceipt,
        ]
    );
    assert_eq!(STATE_TERMINAL_CANDIDATE_ORDER_V2.len(), 12);
    assert!(
        STATE_TERMINAL_CANDIDATE_ORDER_V2
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        STATE_TERMINAL_CANDIDATE_ORDER_V2[10],
        StateTerminalCandidateStageV2::PersistSuccessorLineages
    );
    assert_eq!(
        STATE_TERMINAL_CANDIDATE_ORDER_V2.last(),
        Some(&StateTerminalCandidateStageV2::IssueReceipt)
    );
}

#[test]
fn successor_folds_are_ordered_internal_persistence_substeps_only() {
    assert_eq!(
        STATE_TERMINAL_PERSISTENCE_SUBORDER_V2,
        [
            StateTerminalPersistenceSubstepV2::EqSuccessorFold,
            StateTerminalPersistenceSubstepV2::EpSuccessorFold,
            StateTerminalPersistenceSubstepV2::AtomicPersist,
        ]
    );
    assert!(
        STATE_TERMINAL_PERSISTENCE_SUBORDER_V2
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_ORDER_V2,
        [
            StateTerminalSuccessorFoldInputRoleV2::Current,
            StateTerminalSuccessorFoldInputRoleV2::Prior,
        ]
    );
}

#[test]
fn two_input_successor_fold_ledger_is_exact_for_both_parities() {
    assert_eq!(STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2, 2);
    assert_eq!(
        STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_BYTES_PER_PARITY_V2,
        1_152
    );
    assert_eq!(
        STATE_TERMINAL_SUCCESSOR_FOLD_PROOF_BYTES_PER_PARITY_V2,
        1_344
    );
    assert_eq!(
        STATE_TERMINAL_SUCCESSOR_FOLD_OUTPUT_BYTES_PER_PARITY_V2,
        576
    );
    assert_eq!(STATE_TERMINAL_SUCCESSOR_FOLD_PAIRED_PROOF_BYTES_V2, 2_688);
    assert_eq!(
        STATE_TERMINAL_SUCCESSOR_FOLD_LEDGER_V2,
        StateTerminalSuccessorFoldLedgerV2 {
            k: 17,
            parity_count: 2,
            inputs_per_parity: 2,
            input_accumulator_bytes_per_parity: 1_152,
            bgh19_elements_per_parity: 42,
            bgh19_proof_bytes_per_parity: 1_344,
            output_accumulator_bytes_per_parity: 576,
            paired_bgh19_proof_bytes: 2_688,
        }
    );
}

#[test]
fn eq_then_ep_structural_pair_preserves_parity_and_current_prior_order() {
    let pair = UnverifiedStateTerminalSuccessorPairV2::from_eq_then_ep(
        successor_fold(StateRecursiveFoldParityV2::Eq),
        successor_fold(StateRecursiveFoldParityV2::Ep),
    )
    .expect("Eq then Ep structural pair");
    assert_eq!(pair.eq().parity(), StateRecursiveFoldParityV2::Eq);
    assert_eq!(pair.ep().parity(), StateRecursiveFoldParityV2::Ep);
    assert_eq!(
        [pair.eq().inputs()[0].role(), pair.eq().inputs()[1].role()],
        STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_ORDER_V2
    );
    assert_eq!(
        [pair.ep().inputs()[0].role(), pair.ep().inputs()[1].role()],
        STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_ORDER_V2
    );
    assert_eq!(
        pair.eq().inputs()[0].accumulator().parity(),
        StateRecursiveFoldParityV2::Eq
    );
    assert_eq!(
        pair.ep().inputs()[1].accumulator().parity(),
        StateRecursiveFoldParityV2::Ep
    );
    assert_eq!(pair.eq().proof().as_bytes().len(), 1_344);
    assert_eq!(pair.ep().claimed_output().as_bytes().len(), 576);
}

#[test]
fn successor_order_parity_and_pair_substitutions_fail_closed() {
    assert_eq!(
        UnverifiedStateTerminalSuccessorFoldV2::from_structural_parts(
            StateRecursiveFoldParityV2::Eq,
            [
                StateTerminalSuccessorFoldInputV2::new(
                    StateTerminalSuccessorFoldInputRoleV2::Prior,
                    accumulator(StateRecursiveFoldParityV2::Eq),
                ),
                StateTerminalSuccessorFoldInputV2::new(
                    StateTerminalSuccessorFoldInputRoleV2::Current,
                    accumulator(StateRecursiveFoldParityV2::Eq),
                ),
            ],
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateTerminalCandidateErrorV2::InputOrderMismatch { index: 0 })
    );

    assert_eq!(
        UnverifiedStateTerminalSuccessorFoldV2::from_structural_parts(
            StateRecursiveFoldParityV2::Eq,
            [
                StateTerminalSuccessorFoldInputV2::new(
                    StateTerminalSuccessorFoldInputRoleV2::Current,
                    accumulator(StateRecursiveFoldParityV2::Eq),
                ),
                StateTerminalSuccessorFoldInputV2::new(
                    StateTerminalSuccessorFoldInputRoleV2::Prior,
                    accumulator(StateRecursiveFoldParityV2::Ep),
                ),
            ],
            proof(),
            accumulator(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateTerminalCandidateErrorV2::ParityMismatch { index: 1 })
    );

    assert_eq!(
        UnverifiedStateTerminalSuccessorPairV2::from_eq_then_ep(
            successor_fold(StateRecursiveFoldParityV2::Ep),
            successor_fold(StateRecursiveFoldParityV2::Ep),
        ),
        Err(StateTerminalCandidateErrorV2::EqFoldRequiredFirst)
    );
    assert_eq!(
        UnverifiedStateTerminalSuccessorPairV2::from_eq_then_ep(
            successor_fold(StateRecursiveFoldParityV2::Eq),
            successor_fold(StateRecursiveFoldParityV2::Eq),
        ),
        Err(StateTerminalCandidateErrorV2::EpFoldRequiredSecond)
    );
}

#[test]
fn unresolved_ecc_guard_target_artifact_rss_and_persistence_blockers_are_explicit() {
    assert_eq!(
        STATE_TERMINAL_CANDIDATE_BLOCKERS_V2,
        [
            StateTerminalCandidateBlockerV2::EccStrategyUnresolved,
            StateTerminalCandidateBlockerV2::GuardBundleUnavailable,
            StateTerminalCandidateBlockerV2::FinalStatePairTargetUnresolved {
                qualification_target_bytes: 6_272,
                absolute_maximum_bytes: 6_528,
            },
            StateTerminalCandidateBlockerV2::AuthenticatedArtifactInventoryUnavailable,
            StateTerminalCandidateBlockerV2::MeasuredProcessRssUnavailable {
                qualification_bytes: 268_435_456,
            },
            StateTerminalCandidateBlockerV2::AtomicPersistenceUnavailable,
            StateTerminalCandidateBlockerV2::VerifiedReceiptUnavailable,
        ]
    );
    assert!(STATE_TERMINAL_CANDIDATE_DECLARED_V2);
    assert!(!STATE_TERMINAL_COMPILER_AVAILABLE_V2);
    assert!(!STATE_TERMINAL_CIRCUIT_IMPLEMENTED_V2);
    assert!(!STATE_TERMINAL_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_TERMINAL_BACKEND_AVAILABLE_V2);
    assert!(!STATE_TERMINAL_PERSISTENCE_AVAILABLE_V2);
    assert!(!STATE_TERMINAL_RECEIPT_AVAILABLE_V2);
    assert!(!STATE_TERMINAL_READINESS_AVAILABLE_V2);
    assert!(!STATE_TERMINAL_RELEASE_ELIGIBLE_V2);
    assert!(!STATE_TERMINAL_PRODUCTION_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2);
    assert!(!STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2);
    assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2);
}

#[test]
fn receipt_boundary_always_fails_closed() {
    assert!(matches!(
        fail_closed_state_terminal_candidate_v2(),
        Err(StateTerminalCandidateErrorV2::RecursiveVerificationUnavailable)
    ));
    assert_eq!(
        StateTerminalCandidateErrorV2::PersistenceUnavailable.to_string(),
        "offline-cash V2 atomic successor persistence is unavailable"
    );
    assert_eq!(
        StateTerminalCandidateErrorV2::ReceiptUnavailable.to_string(),
        "offline-cash V2 terminal receipt is unavailable"
    );
}

#[test]
fn source_guards_keep_terminal_adapters_move_only_uninhabited_and_private() {
    let source = include_str!("state_terminal_candidate.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod state_terminal_candidate;")
            .count(),
        1
    );
    assert!(parent.contains("#[path = \"offline_cash_v2/state_terminal_candidate.rs\"]"));
    assert!(!parent.contains("pub mod state_terminal_candidate"));
    assert!(source.contains("public terminal order remains the frozen twelve-stage contract"));
    assert!(source.contains("enum StateTerminalProductionAdapterV2 {}"));
    assert!(source.contains("enum StateTerminalArtifactAdapterV2 {}"));
    assert!(source.contains("enum StateTerminalEqSuccessorFoldCapabilityV2 {}"));
    assert!(source.contains("enum StateTerminalEpSuccessorFoldCapabilityV2 {}"));
    assert!(source.contains("enum StateTerminalStoreAdapterV2 {}"));
    assert!(source.contains("enum StateTerminalVerifiedReceiptV2 {}"));
    assert!(source.contains("struct StateTerminalAtomicPersistenceInputsV2"));
    assert!(
        source.contains("Err(StateTerminalCandidateErrorV2::RecursiveVerificationUnavailable)")
    );
    assert!(!source.contains("impl Clone for StateTerminal"));
    assert!(!source.contains("impl Copy for StateTerminal"));
    assert!(!source.contains("verify_proof("));
    assert!(!source.contains("impl Circuit for"));
    assert!(!source.contains("Ok(StateTerminalVerifiedReceiptV2"));
    assert!(!source.contains("STATE_TERMINAL_PERSISTENCE_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("STATE_TERMINAL_RELEASE_ELIGIBLE_V2: bool = true"));
}
