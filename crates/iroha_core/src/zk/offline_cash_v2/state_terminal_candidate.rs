//! Privately declared, fail-closed terminal candidate for Offline Cash V2 STATE recursion.
//!
//! The public terminal order remains the frozen twelve-stage contract. Successor
//! folding does not add public acceptance stages: inside
//! `PersistSuccessorLineages`, a future implementation must consume an Eq
//! two-input BGH19 fold, then an Ep two-input BGH19 fold, then atomically persist
//! both outputs. Each fold order is `[current, prior]`, where `current` is derived
//! after the final STATE proof transcript and `prior` is the live 576-byte
//! lineage decoded from that proof's word-93 public tail.
//!
//! This private child provides structural ledgers and codecs, not verification.
//! Compiler, circuit, ECC, GuardBundle,
//! artifact, backend, persistence, receipt, readiness, and release authority all
//! remain unavailable and are represented by false gates or uninhabited types.

use core::fmt;

use super::state_recursive_fold::{
    CanonicalStateAccumulatorV2, OpaqueStateBgh19ProofV2,
    STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2, STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2,
    STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2, STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2,
    STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2, STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2,
    STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2, STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2,
    STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2,
    STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2,
    STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2,
    STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2, STATE_RECURSIVE_FOLD_K_V2,
    STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2,
    STATE_RECURSIVE_FOLD_PROCESS_RSS_QUALIFICATION_BYTES_V2, StateRecursiveFoldParityV2,
};

/// Exact successor-fold inputs per parity.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2: usize = 2;
/// Exact canonical input accumulator bytes per parity.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_BYTES_PER_PARITY_V2: usize =
    STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2 * STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2;
/// Exact opaque BGH19 transcript bytes per parity.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_PROOF_BYTES_PER_PARITY_V2: usize =
    STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2;
/// Exact successor accumulator bytes per parity.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_OUTPUT_BYTES_PER_PARITY_V2: usize =
    STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2;
/// Exact Eq plus Ep successor-fold transcript bytes.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_PAIRED_PROOF_BYTES_V2: usize =
    2 * STATE_TERMINAL_SUCCESSOR_FOLD_PROOF_BYTES_PER_PARITY_V2;

/// This source candidate is privately declared but remains non-live.
pub(super) const STATE_TERMINAL_CANDIDATE_DECLARED_V2: bool = true;
/// No compiler has produced the candidate's exact recursive protocols.
pub(super) const STATE_TERMINAL_COMPILER_AVAILABLE_V2: bool = false;
/// No production recursive STATE or successor-fold circuit is implemented.
pub(super) const STATE_TERMINAL_CIRCUIT_IMPLEMENTED_V2: bool = false;
/// No complete authenticated terminal artifact inventory exists.
pub(super) const STATE_TERMINAL_ARTIFACTS_AUTHENTICATED_V2: bool = false;
/// No production recursive verification backend exists.
pub(super) const STATE_TERMINAL_BACKEND_AVAILABLE_V2: bool = false;
/// No atomic paired-successor store adapter exists.
pub(super) const STATE_TERMINAL_PERSISTENCE_AVAILABLE_V2: bool = false;
/// No terminal can construct a verified receipt.
pub(super) const STATE_TERMINAL_RECEIPT_AVAILABLE_V2: bool = false;
/// No qualified terminal readiness authority exists.
pub(super) const STATE_TERMINAL_READINESS_AVAILABLE_V2: bool = false;
/// The candidate is not eligible for release.
pub(super) const STATE_TERMINAL_RELEASE_ELIGIBLE_V2: bool = false;
/// There is no production terminal path.
pub(super) const STATE_TERMINAL_PRODUCTION_AVAILABLE_V2: bool = false;

const _: () = assert!(STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2 == 2);
const _: () = assert!(STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_BYTES_PER_PARITY_V2 == 1_152);
const _: () = assert!(STATE_TERMINAL_SUCCESSOR_FOLD_PROOF_BYTES_PER_PARITY_V2 == 1_344);
const _: () = assert!(STATE_TERMINAL_SUCCESSOR_FOLD_OUTPUT_BYTES_PER_PARITY_V2 == 576);
const _: () = assert!(STATE_TERMINAL_SUCCESSOR_FOLD_PAIRED_PROOF_BYTES_V2 == 2_688);
const _: () = assert!(STATE_TERMINAL_CANDIDATE_DECLARED_V2);
const _: () = assert!(!STATE_TERMINAL_COMPILER_AVAILABLE_V2);
const _: () = assert!(!STATE_TERMINAL_CIRCUIT_IMPLEMENTED_V2);
const _: () = assert!(!STATE_TERMINAL_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!STATE_TERMINAL_BACKEND_AVAILABLE_V2);
const _: () = assert!(!STATE_TERMINAL_PERSISTENCE_AVAILABLE_V2);
const _: () = assert!(!STATE_TERMINAL_RECEIPT_AVAILABLE_V2);
const _: () = assert!(!STATE_TERMINAL_READINESS_AVAILABLE_V2);
const _: () = assert!(!STATE_TERMINAL_RELEASE_ELIGIBLE_V2);
const _: () = assert!(!STATE_TERMINAL_PRODUCTION_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2);

/// Frozen public terminal stages. Do not add successor folds to this list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum StateTerminalCandidateStageV2 {
    CanonicalWireDecode = 1,
    StatementAndLiveness = 2,
    ArtifactAndProtocolAuthentication = 3,
    ReconstructPublicInstances = 4,
    EqCurrentProof = 5,
    EpCurrentProof = 6,
    EqCurrentDecision = 7,
    EpCurrentDecision = 8,
    EqParentLineageDecision = 9,
    EpParentLineageDecision = 10,
    PersistSuccessorLineages = 11,
    IssueReceipt = 12,
}

/// Exact public fail-closed terminal sequence.
pub(super) const STATE_TERMINAL_CANDIDATE_ORDER_V2: [StateTerminalCandidateStageV2; 12] = [
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
];

/// Internal substeps consumed by `PersistSuccessorLineages` only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum StateTerminalPersistenceSubstepV2 {
    /// Verify/create Eq successor `[current, prior]` fold capability.
    EqSuccessorFold = 1,
    /// Verify/create Ep successor `[current, prior]` fold capability.
    EpSuccessorFold = 2,
    /// Atomically persist both parity outputs with no partial commit.
    AtomicPersist = 3,
}

/// Exact internal order; it does not replace or extend the public 12 stages.
pub(super) const STATE_TERMINAL_PERSISTENCE_SUBORDER_V2: [StateTerminalPersistenceSubstepV2; 3] = [
    StateTerminalPersistenceSubstepV2::EqSuccessorFold,
    StateTerminalPersistenceSubstepV2::EpSuccessorFold,
    StateTerminalPersistenceSubstepV2::AtomicPersist,
];

/// Exact input role for each parity's successor fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum StateTerminalSuccessorFoldInputRoleV2 {
    /// Accumulator derived from the completed current STATE transcript.
    Current = 1,
    /// Live predecessor lineage decoded from current STATE ABI word 93.
    Prior = 2,
}

/// Exact two-input successor fold order.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_ORDER_V2:
    [StateTerminalSuccessorFoldInputRoleV2; STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2] = [
    StateTerminalSuccessorFoldInputRoleV2::Current,
    StateTerminalSuccessorFoldInputRoleV2::Prior,
];

/// Static successor-fold accounting. It is not a verified transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateTerminalSuccessorFoldLedgerV2 {
    pub(super) k: u32,
    pub(super) parity_count: usize,
    pub(super) inputs_per_parity: usize,
    pub(super) input_accumulator_bytes_per_parity: usize,
    pub(super) bgh19_elements_per_parity: usize,
    pub(super) bgh19_proof_bytes_per_parity: usize,
    pub(super) output_accumulator_bytes_per_parity: usize,
    pub(super) paired_bgh19_proof_bytes: usize,
}

/// Exact reviewed arithmetic ledger for the internal persistence folds.
pub(super) const STATE_TERMINAL_SUCCESSOR_FOLD_LEDGER_V2: StateTerminalSuccessorFoldLedgerV2 =
    StateTerminalSuccessorFoldLedgerV2 {
        k: STATE_RECURSIVE_FOLD_K_V2,
        parity_count: 2,
        inputs_per_parity: STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2,
        input_accumulator_bytes_per_parity: STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_BYTES_PER_PARITY_V2,
        bgh19_elements_per_parity: STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2,
        bgh19_proof_bytes_per_parity: STATE_TERMINAL_SUCCESSOR_FOLD_PROOF_BYTES_PER_PARITY_V2,
        output_accumulator_bytes_per_parity:
            STATE_TERMINAL_SUCCESSOR_FOLD_OUTPUT_BYTES_PER_PARITY_V2,
        paired_bgh19_proof_bytes: STATE_TERMINAL_SUCCESSOR_FOLD_PAIRED_PROOF_BYTES_V2,
    };

/// Structural successor-fold input; origin is not authenticated by this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StateTerminalSuccessorFoldInputV2 {
    role: StateTerminalSuccessorFoldInputRoleV2,
    accumulator: CanonicalStateAccumulatorV2,
}

impl StateTerminalSuccessorFoldInputV2 {
    pub(super) const fn new(
        role: StateTerminalSuccessorFoldInputRoleV2,
        accumulator: CanonicalStateAccumulatorV2,
    ) -> Self {
        Self { role, accumulator }
    }

    pub(super) const fn role(&self) -> StateTerminalSuccessorFoldInputRoleV2 {
        self.role
    }

    pub(super) const fn accumulator(&self) -> &CanonicalStateAccumulatorV2 {
        &self.accumulator
    }
}

/// Structural successor-fold failure. No variant represents proof acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateTerminalCandidateErrorV2 {
    InputOrderMismatch { index: usize },
    ParityMismatch { index: usize },
    EqFoldRequiredFirst,
    EpFoldRequiredSecond,
    RecursiveVerificationUnavailable,
    PersistenceUnavailable,
    ReceiptUnavailable,
}

impl fmt::Display for StateTerminalCandidateErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputOrderMismatch { index } => write!(
                formatter,
                "offline-cash V2 successor-fold input {index} has the wrong role"
            ),
            Self::ParityMismatch { index } => write!(
                formatter,
                "offline-cash V2 successor-fold value {index} has the wrong parity"
            ),
            Self::EqFoldRequiredFirst => {
                formatter.write_str("offline-cash V2 terminal requires Eq successor fold first")
            }
            Self::EpFoldRequiredSecond => {
                formatter.write_str("offline-cash V2 terminal requires Ep successor fold second")
            }
            Self::RecursiveVerificationUnavailable => formatter
                .write_str("offline-cash V2 recursive terminal verification is unavailable"),
            Self::PersistenceUnavailable => {
                formatter.write_str("offline-cash V2 atomic successor persistence is unavailable")
            }
            Self::ReceiptUnavailable => {
                formatter.write_str("offline-cash V2 terminal receipt is unavailable")
            }
        }
    }
}

impl std::error::Error for StateTerminalCandidateErrorV2 {}

/// Unverified exact-shape successor fold for one parity.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnverifiedStateTerminalSuccessorFoldV2 {
    parity: StateRecursiveFoldParityV2,
    inputs: [StateTerminalSuccessorFoldInputV2; STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2],
    proof: OpaqueStateBgh19ProofV2,
    claimed_output: CanonicalStateAccumulatorV2,
}

impl UnverifiedStateTerminalSuccessorFoldV2 {
    /// Check only exact role/parity/codec shape. This never verifies BGH19.
    pub(super) fn from_structural_parts(
        parity: StateRecursiveFoldParityV2,
        inputs: [StateTerminalSuccessorFoldInputV2;
            STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2],
        proof: OpaqueStateBgh19ProofV2,
        claimed_output: CanonicalStateAccumulatorV2,
    ) -> Result<Self, StateTerminalCandidateErrorV2> {
        for (index, (input, expected_role)) in inputs
            .iter()
            .zip(STATE_TERMINAL_SUCCESSOR_FOLD_INPUT_ORDER_V2)
            .enumerate()
        {
            if input.role != expected_role {
                return Err(StateTerminalCandidateErrorV2::InputOrderMismatch { index });
            }
            if input.accumulator.parity() != parity {
                return Err(StateTerminalCandidateErrorV2::ParityMismatch { index });
            }
        }
        if claimed_output.parity() != parity {
            return Err(StateTerminalCandidateErrorV2::ParityMismatch {
                index: STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2,
            });
        }
        Ok(Self {
            parity,
            inputs,
            proof,
            claimed_output,
        })
    }

    pub(super) const fn parity(&self) -> StateRecursiveFoldParityV2 {
        self.parity
    }

    pub(super) const fn inputs(
        &self,
    ) -> &[StateTerminalSuccessorFoldInputV2; STATE_TERMINAL_SUCCESSOR_FOLD_INPUTS_PER_PARITY_V2]
    {
        &self.inputs
    }

    pub(super) const fn proof(&self) -> &OpaqueStateBgh19ProofV2 {
        &self.proof
    }

    pub(super) const fn claimed_output(&self) -> &CanonicalStateAccumulatorV2 {
        &self.claimed_output
    }
}

/// Ordered Eq/Ep structural pair. Still unverified and not persistable.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnverifiedStateTerminalSuccessorPairV2 {
    eq: UnverifiedStateTerminalSuccessorFoldV2,
    ep: UnverifiedStateTerminalSuccessorFoldV2,
}

impl UnverifiedStateTerminalSuccessorPairV2 {
    pub(super) fn from_eq_then_ep(
        eq: UnverifiedStateTerminalSuccessorFoldV2,
        ep: UnverifiedStateTerminalSuccessorFoldV2,
    ) -> Result<Self, StateTerminalCandidateErrorV2> {
        if eq.parity != StateRecursiveFoldParityV2::Eq {
            return Err(StateTerminalCandidateErrorV2::EqFoldRequiredFirst);
        }
        if ep.parity != StateRecursiveFoldParityV2::Ep {
            return Err(StateTerminalCandidateErrorV2::EpFoldRequiredSecond);
        }
        Ok(Self { eq, ep })
    }

    pub(super) const fn eq(&self) -> &UnverifiedStateTerminalSuccessorFoldV2 {
        &self.eq
    }

    pub(super) const fn ep(&self) -> &UnverifiedStateTerminalSuccessorFoldV2 {
        &self.ep
    }
}

/// Explicit blockers that keep terminal acceptance unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateTerminalCandidateBlockerV2 {
    EccStrategyUnresolved,
    GuardBundleUnavailable,
    FinalStatePairTargetUnresolved {
        qualification_target_bytes: usize,
        absolute_maximum_bytes: usize,
    },
    AuthenticatedArtifactInventoryUnavailable,
    MeasuredProcessRssUnavailable {
        qualification_bytes: u64,
    },
    AtomicPersistenceUnavailable,
    VerifiedReceiptUnavailable,
}

pub(super) const STATE_TERMINAL_CANDIDATE_BLOCKERS_V2: [StateTerminalCandidateBlockerV2; 7] = [
    StateTerminalCandidateBlockerV2::EccStrategyUnresolved,
    StateTerminalCandidateBlockerV2::GuardBundleUnavailable,
    StateTerminalCandidateBlockerV2::FinalStatePairTargetUnresolved {
        qualification_target_bytes: STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2,
        absolute_maximum_bytes: STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2,
    },
    StateTerminalCandidateBlockerV2::AuthenticatedArtifactInventoryUnavailable,
    StateTerminalCandidateBlockerV2::MeasuredProcessRssUnavailable {
        qualification_bytes: STATE_RECURSIVE_FOLD_PROCESS_RSS_QUALIFICATION_BYTES_V2,
    },
    StateTerminalCandidateBlockerV2::AtomicPersistenceUnavailable,
    StateTerminalCandidateBlockerV2::VerifiedReceiptUnavailable,
];

pub(super) mod sealed {
    pub(in crate::zk::offline_cash_v2) trait Sealed {}
}

/// Marker implemented only by uninhabited terminal adapters in this module.
pub(super) trait SealedStateTerminalAdapterV2: sealed::Sealed {}

/// Uninhabited move-only production terminal adapter.
pub(super) enum StateTerminalProductionAdapterV2 {}
impl sealed::Sealed for StateTerminalProductionAdapterV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalProductionAdapterV2 {}

/// Uninhabited move-only authenticated artifact adapter.
pub(super) enum StateTerminalArtifactAdapterV2 {}
impl sealed::Sealed for StateTerminalArtifactAdapterV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalArtifactAdapterV2 {}

/// Uninhabited move-only Eq fold-verification capability.
pub(super) enum StateTerminalEqSuccessorFoldCapabilityV2 {}
impl sealed::Sealed for StateTerminalEqSuccessorFoldCapabilityV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalEqSuccessorFoldCapabilityV2 {}

/// Uninhabited move-only Ep fold-verification capability.
pub(super) enum StateTerminalEpSuccessorFoldCapabilityV2 {}
impl sealed::Sealed for StateTerminalEpSuccessorFoldCapabilityV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalEpSuccessorFoldCapabilityV2 {}

/// Uninhabited move-only atomic paired-successor store adapter.
pub(super) enum StateTerminalStoreAdapterV2 {}
impl sealed::Sealed for StateTerminalStoreAdapterV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalStoreAdapterV2 {}

/// Impossible persistence bundle: both ordered fold capabilities and the store
/// must be consumed together before a receipt could exist.
pub(super) struct StateTerminalAtomicPersistenceInputsV2 {
    _eq: StateTerminalEqSuccessorFoldCapabilityV2,
    _ep: StateTerminalEpSuccessorFoldCapabilityV2,
    _store: StateTerminalStoreAdapterV2,
}

/// Uninhabited move-only verified receipt.
pub(super) enum StateTerminalVerifiedReceiptV2 {}
impl sealed::Sealed for StateTerminalVerifiedReceiptV2 {}
impl SealedStateTerminalAdapterV2 for StateTerminalVerifiedReceiptV2 {}

/// Fail closed at the only receipt-shaped boundary.
pub(super) const fn fail_closed_state_terminal_candidate_v2()
-> Result<StateTerminalVerifiedReceiptV2, StateTerminalCandidateErrorV2> {
    Err(StateTerminalCandidateErrorV2::RecursiveVerificationUnavailable)
}

#[cfg(test)]
#[path = "state_terminal_candidate_tests.rs"]
mod tests;
