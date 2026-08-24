//! Privately declared, non-authorizing recursive STATE-fold contract for Offline Cash V2.
//!
//! This private framing child freezes the field-neutral boundary an authorized
//! recursive implementation must satisfy; it does not itself implement a Halo2
//! circuit, artifact loading, or a production backend. Its ownership carrier is
//! consumed only by the private native BGH19 relation sibling. For each Pasta
//! parity, the one permitted predecessor fold order is
//! `[P0.current, P0.prior, P1.current, P1.prior, Guard.current, Guard.prior]`.
//! A `current` accumulator is derived only after reading that child's proof and
//! is never copied from the child's current public instances. A `prior` lineage
//! is the canonical 576-byte tail beginning at STATE ABI word 93 for STATE
//! parents and GuardBundle ABI word 192 for the GuardBundle parent.
//!
//! BGH19 transcript bytes remain opaque to this framing module. Exact length and
//! canonical input accumulator codecs are useful fail-closed framing checks,
//! but they are not proof verification. In particular, this module grants no
//! recursion, STATE, GuardBundle, artifact, persistence, readiness, or release
//! authority.

use core::{convert::Infallible, fmt};

use halo2_proofs::halo2curves::{
    CurveAffine,
    ff::PrimeField,
    pasta::{EpAffine, EqAffine},
};
use iroha_data_model::offline::KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2;

use super::{
    guard_bundle_provenance::{
        OfflineCashGuardBundleStateProvenanceSealV2, VerifiedOfflineCashGuardBundleStateHandoffV2,
    },
    state_semantic_parent_provenance::{
        ProvenanceBoundStateParent0InputsV2, ProvenanceBoundStateParent1InputsV2,
    },
};

/// Fixed recursive domain exponent under review.
pub(super) const STATE_RECURSIVE_FOLD_K_V2: u32 = 17;
/// Canonical scalar and compressed-point element width.
pub(super) const STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2: usize = 32;
/// Round challenges in one k=17 IPA accumulator.
pub(super) const STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2: usize =
    STATE_RECURSIVE_FOLD_K_V2 as usize;
/// Exact canonical bytes in one k=17 accumulator: 17 scalars plus one point.
pub(super) const STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2: usize =
    (STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 + 1) * STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2;
/// Exact predecessor inputs folded independently in each parity.
pub(super) const STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2: usize = 6;
/// Exact bytes across the six canonical accumulator inputs.
pub(super) const STATE_RECURSIVE_FOLD_INPUT_ACCUMULATOR_BYTES_PER_PARITY_V2: usize =
    STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2 * STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2;
/// BGH19 k=17 transcript elements: `2 * k + 8`.
pub(super) const STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2: usize =
    2 * STATE_RECURSIVE_FOLD_K_V2 as usize + 8;
/// Exact opaque BGH19 fold transcript bytes for k=17.
pub(super) const STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2: usize =
    STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2 * STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2;
/// One fold produces one canonical k=17 accumulator.
pub(super) const STATE_RECURSIVE_FOLD_OUTPUT_ACCUMULATOR_BYTES_V2: usize =
    STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2;

/// Exact field-neutral STATE ABI words consumed by each child verifier.
pub(super) const STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2: usize = 237;
/// Canonical little-endian words per direct public-instance cell.
pub(super) const STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2: usize = 7;
/// Exact direct public-instance cells.
pub(super) const STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2: usize = 34;
/// Bytes in one field-neutral 224-bit packed cell.
pub(super) const STATE_RECURSIVE_FOLD_PACKED_CELL_BYTES_V2: usize =
    STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2 * 4;
/// First word of the aggregate predecessor-lineage tail.
pub(super) const STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2: usize = 93;
/// Exact words in one 576-byte aggregate predecessor lineage.
pub(super) const STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2: usize =
    STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2 / 4;
/// First of four little-endian `u32` amount words.
pub(super) const STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2: usize = 88;
/// Fixed-point scale word immediately before the lineage.
pub(super) const STATE_RECURSIVE_FOLD_SCALE_WORD_V2: usize = 92;
/// Mandatory zero words after word 236 in the last packed cell.
pub(super) const STATE_RECURSIVE_FOLD_FINAL_CELL_ZERO_PADDING_WORDS_V2: usize = 1;
/// Direct public-instance policy: the prover and verifier set `QUERY_INSTANCE = false`.
pub(super) const STATE_RECURSIVE_FOLD_QUERY_INSTANCE_V2: bool = false;
/// The current proof accumulator is post-transcript state, not a current instance.
pub(super) const STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_IN_CURRENT_INSTANCES_V2: bool = false;
/// No current public ABI words are allocated to the current accumulator.
pub(super) const STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_INSTANCE_WORDS_V2: usize = 0;
/// Reserved all-zero bootstrap lineages are rejected at this live fold boundary.
pub(super) const STATE_RECURSIVE_FOLD_ZERO_BOOTSTRAP_ACCEPTED_V2: bool = false;

/// Existing absolute augmented-child transcript cap. A fold transcript fitting
/// this byte count is only a framing fact, never proof-size or release evidence.
pub(super) const STATE_RECURSIVE_FOLD_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2: usize = 3_264;
/// Unresolved paired final-STATE qualification target.
pub(super) const STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2: usize = 6_272;
/// Absolute paired final-STATE transport reservation.
pub(super) const STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2: usize = 6_528;
/// Whole-process RSS qualification ceiling recorded by the V2 scaffold.
pub(super) const STATE_RECURSIVE_FOLD_PROCESS_RSS_QUALIFICATION_BYTES_V2: u64 = 268_435_456;

/// This source-only contract is privately declared but remains non-live.
pub(super) const STATE_RECURSIVE_FOLD_DECLARED_V2: bool = true;
/// No reviewed recursive compiler exists for this candidate.
pub(super) const STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2: bool = false;
/// No k=17 recursive fold circuit is implemented by this candidate.
pub(super) const STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2: bool = false;
/// The in-circuit Pasta ECC strategy remains unresolved.
pub(super) const STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2: bool = false;
/// The recursive GuardBundle child is not available.
pub(super) const STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2: bool = false;
/// The paired final-STATE qualification target has not been governed.
pub(super) const STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2: bool = false;
/// No complete authenticated recursive artifact inventory exists.
pub(super) const STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2: bool = false;
/// No production recursive verifier/backend exists.
pub(super) const STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2: bool = false;
/// No measured whole-process RSS evidence exists for this candidate.
pub(super) const STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2: bool = false;
/// Structural source cannot authorize readiness.
pub(super) const STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2: bool = false;
/// Structural source cannot authorize a release.
pub(super) const STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2: bool = false;
/// No production recursive fold path exists.
pub(super) const STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2: bool = false;

const _: () = assert!(STATE_RECURSIVE_FOLD_K_V2 == 17);
const _: () = assert!(STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2 == 576);
const _: () = assert!(STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2 == 6);
const _: () = assert!(STATE_RECURSIVE_FOLD_INPUT_ACCUMULATOR_BYTES_PER_PARITY_V2 == 3_456);
const _: () = assert!(STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2 == 42);
const _: () = assert!(STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2 == 1_344);
const _: () = assert!(STATE_RECURSIVE_FOLD_OUTPUT_ACCUMULATOR_BYTES_V2 == 576);
const _: () = assert!(
    STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2 + STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2
        == STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2
);
const _: () = assert!(STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2 + 4 == 92);
const _: () = assert!(STATE_RECURSIVE_FOLD_SCALE_WORD_V2 + 1 == 93);
const _: () = assert!(
    STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2
        == STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2
            .div_ceil(STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2)
);
const _: () = assert!(
    STATE_RECURSIVE_FOLD_FINAL_CELL_ZERO_PADDING_WORDS_V2
        == STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2
            * STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2
            - STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2
);
const _: () = assert!(!STATE_RECURSIVE_FOLD_QUERY_INSTANCE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_IN_CURRENT_INSTANCES_V2);
const _: () = assert!(STATE_RECURSIVE_FOLD_CURRENT_ACCUMULATOR_INSTANCE_WORDS_V2 == 0);
const _: () = assert!(!STATE_RECURSIVE_FOLD_ZERO_BOOTSTRAP_ACCEPTED_V2);
const _: () = assert!(
    STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2
        <= STATE_RECURSIVE_FOLD_CHILD_PROOF_ABSOLUTE_MAX_BYTES_V2
);
const _: () = assert!(
    STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2
        < STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2
);
const _: () = assert!(STATE_RECURSIVE_FOLD_DECLARED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_COMPILER_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_CIRCUIT_IMPLEMENTED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_ECC_STRATEGY_GOVERNED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_GUARD_BUNDLE_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_FINAL_STATE_TARGET_GOVERNED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_BACKEND_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_MEASURED_RSS_EVIDENCE_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_READINESS_AVAILABLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_RELEASE_ELIGIBLE_V2);
const _: () = assert!(!STATE_RECURSIVE_FOLD_PRODUCTION_AVAILABLE_V2);

/// Pasta parity. Values match the frozen STATE ABI header word three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum StateRecursiveFoldParityV2 {
    /// Eq/Vesta accumulator with Fp challenges.
    Eq = 1,
    /// Ep/Pallas accumulator with Fq challenges.
    Ep = 2,
}

/// Exact semantic role of an accumulator in the per-parity six-input fold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum StateRecursiveFoldInputRoleV2 {
    /// First STATE parent accumulator derived after its current proof.
    Parent0Current = 1,
    /// First STATE parent's predecessor lineage from ABI word 93.
    Parent0Prior = 2,
    /// Second STATE parent accumulator derived after its current proof.
    Parent1Current = 3,
    /// Second STATE parent's predecessor lineage from ABI word 93.
    Parent1Prior = 4,
    /// GuardBundle accumulator derived after its current proof.
    GuardCurrent = 5,
    /// GuardBundle predecessor lineage from GuardBundle ABI word 192.
    GuardPrior = 6,
}

/// Exact per-parity BGH19 input order. There is no cross-parity fold.
pub(super) const STATE_RECURSIVE_FOLD_INPUT_ORDER_V2: [StateRecursiveFoldInputRoleV2;
    STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] = [
    StateRecursiveFoldInputRoleV2::Parent0Current,
    StateRecursiveFoldInputRoleV2::Parent0Prior,
    StateRecursiveFoldInputRoleV2::Parent1Current,
    StateRecursiveFoldInputRoleV2::Parent1Prior,
    StateRecursiveFoldInputRoleV2::GuardCurrent,
    StateRecursiveFoldInputRoleV2::GuardPrior,
];

/// Provenance class required for each ordered input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateRecursiveFoldInputSourceV2 {
    /// Derived after exact ordinary-proof parsing and succinct verification.
    CurrentProofAccumulator,
    /// Decoded from the already-public 576-byte tail beginning at word 93.
    PriorLineageAtWord93,
    /// Decoded from the GuardBundle 576-byte tail beginning at word 192.
    GuardPriorLineageAtWord192,
}

impl StateRecursiveFoldInputRoleV2 {
    /// Fixed source class. This documents dependency direction; it does not
    /// authenticate a caller-supplied value.
    pub(super) const fn source(self) -> StateRecursiveFoldInputSourceV2 {
        match self {
            Self::Parent0Current | Self::Parent1Current | Self::GuardCurrent => {
                StateRecursiveFoldInputSourceV2::CurrentProofAccumulator
            }
            Self::Parent0Prior | Self::Parent1Prior => {
                StateRecursiveFoldInputSourceV2::PriorLineageAtWord93
            }
            Self::GuardPrior => StateRecursiveFoldInputSourceV2::GuardPriorLineageAtWord192,
        }
    }
}

/// Static fold-size ledger. It is not a compiled-circuit measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateRecursiveFoldLedgerV2 {
    pub(super) k: u32,
    pub(super) parity_count: usize,
    pub(super) inputs_per_parity: usize,
    pub(super) accumulator_bytes: usize,
    pub(super) input_accumulator_bytes_per_parity: usize,
    pub(super) bgh19_elements: usize,
    pub(super) bgh19_proof_bytes_per_parity: usize,
    pub(super) output_accumulator_bytes_per_parity: usize,
    pub(super) query_instance: bool,
}

/// Exact reviewed arithmetic ledger, with no compiler or artifact authority.
pub(super) const STATE_RECURSIVE_FOLD_LEDGER_V2: StateRecursiveFoldLedgerV2 =
    StateRecursiveFoldLedgerV2 {
        k: STATE_RECURSIVE_FOLD_K_V2,
        parity_count: 2,
        inputs_per_parity: STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2,
        accumulator_bytes: STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2,
        input_accumulator_bytes_per_parity:
            STATE_RECURSIVE_FOLD_INPUT_ACCUMULATOR_BYTES_PER_PARITY_V2,
        bgh19_elements: STATE_RECURSIVE_FOLD_BGH19_ELEMENTS_V2,
        bgh19_proof_bytes_per_parity: STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2,
        output_accumulator_bytes_per_parity: STATE_RECURSIVE_FOLD_OUTPUT_ACCUMULATOR_BYTES_V2,
        query_instance: STATE_RECURSIVE_FOLD_QUERY_INSTANCE_V2,
    };

/// Open blockers which prevent the structural ledger becoming live recursion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateRecursiveFoldBlockerV2 {
    /// No governed constraint strategy for non-native Pasta ECC operations.
    EccStrategyUnresolved,
    /// The exact recursive GuardBundle child/protocol is unavailable.
    GuardBundleUnavailable,
    /// The 6,272-byte paired target remains telemetry, not a decision.
    FinalStatePairTargetUnresolved {
        qualification_target_bytes: usize,
        absolute_maximum_bytes: usize,
    },
    /// Complete authenticated Params/PK/VK/protocol/release inventory is absent.
    ArtifactInventoryUnavailable,
    /// Whole-process RSS has not been measured under the release workload.
    MeasuredProcessRssUnavailable { qualification_bytes: u64 },
}

/// The irreducible reviewed blocker inventory.
pub(super) const STATE_RECURSIVE_FOLD_BLOCKERS_V2: [StateRecursiveFoldBlockerV2; 5] = [
    StateRecursiveFoldBlockerV2::EccStrategyUnresolved,
    StateRecursiveFoldBlockerV2::GuardBundleUnavailable,
    StateRecursiveFoldBlockerV2::FinalStatePairTargetUnresolved {
        qualification_target_bytes: STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_TARGET_BYTES_V2,
        absolute_maximum_bytes: STATE_RECURSIVE_FOLD_FINAL_STATE_PAIR_ABSOLUTE_MAX_BYTES_V2,
    },
    StateRecursiveFoldBlockerV2::ArtifactInventoryUnavailable,
    StateRecursiveFoldBlockerV2::MeasuredProcessRssUnavailable {
        qualification_bytes: STATE_RECURSIVE_FOLD_PROCESS_RSS_QUALIFICATION_BYTES_V2,
    },
];

const STATE_RECURSIVE_FOLD_DIGEST_WORD_STARTS_V2: [usize; 10] =
    [8, 16, 24, 32, 40, 48, 56, 64, 72, 80];

/// Strict structural-codec failure. None of these checks verifies BGH19.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateRecursiveFoldCodecErrorV2 {
    /// An accumulator was not exactly 576 bytes.
    InvalidAccumulatorLength { actual: usize },
    /// Live recursion may not use the reserved all-zero bootstrap sentinel.
    BootstrapLineageForbidden,
    /// A k=17 round challenge is not canonical in its parity's scalar field.
    NonCanonicalRoundChallenge { index: usize },
    /// The compressed folded generator is not a canonical parity-local point.
    NonCanonicalFoldedGenerator,
    /// A live accumulator may not carry the identity point.
    IdentityFoldedGenerator,
    /// A STATE word vector or packed-cell vector had the wrong length.
    InvalidStateLength { actual: usize },
    /// STATE header version, k, parity, operation, or lineage geometry differed.
    InvalidStateHeader,
    /// The final 224-bit cell contained nonzero padding.
    NonCanonicalFinalCellPadding,
    /// One of the six inputs appeared in the wrong semantic slot.
    InputOrderMismatch { index: usize },
    /// An input or output accumulator used the other Pasta parity.
    ParityMismatch { index: usize },
    /// An opaque BGH19 transcript was not exactly 1,344 bytes.
    InvalidBgh19ProofLength { actual: usize },
    /// The all-zero transcript is structurally impossible for a live fold.
    ZeroBgh19Proof,
}

impl fmt::Display for StateRecursiveFoldCodecErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAccumulatorLength { actual } => write!(
                formatter,
                "offline-cash V2 recursive accumulator has {actual} bytes instead of {STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2}"
            ),
            Self::BootstrapLineageForbidden => formatter
                .write_str("offline-cash V2 recursive fold rejects the all-zero bootstrap lineage"),
            Self::NonCanonicalRoundChallenge { index } => write!(
                formatter,
                "offline-cash V2 recursive round challenge {index} is non-canonical"
            ),
            Self::NonCanonicalFoldedGenerator => {
                formatter.write_str("offline-cash V2 recursive folded generator is non-canonical")
            }
            Self::IdentityFoldedGenerator => {
                formatter.write_str("offline-cash V2 recursive folded generator is the identity")
            }
            Self::InvalidStateLength { actual } => write!(
                formatter,
                "offline-cash V2 recursive STATE codec received invalid length {actual}"
            ),
            Self::InvalidStateHeader => {
                formatter.write_str("offline-cash V2 recursive STATE header is invalid")
            }
            Self::NonCanonicalFinalCellPadding => {
                formatter.write_str("offline-cash V2 recursive STATE final-cell padding is nonzero")
            }
            Self::InputOrderMismatch { index } => write!(
                formatter,
                "offline-cash V2 recursive fold input {index} has the wrong role"
            ),
            Self::ParityMismatch { index } => write!(
                formatter,
                "offline-cash V2 recursive fold value {index} has the wrong parity"
            ),
            Self::InvalidBgh19ProofLength { actual } => write!(
                formatter,
                "offline-cash V2 BGH19 transcript has {actual} bytes instead of {STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2}"
            ),
            Self::ZeroBgh19Proof => {
                formatter.write_str("offline-cash V2 BGH19 transcript is all zero")
            }
        }
    }
}

impl std::error::Error for StateRecursiveFoldCodecErrorV2 {}

/// Canonical, parity-tagged k=17 accumulator bytes.
///
/// Construction checks scalar and point codecs only. It does not decide the
/// accumulator against ParamsIPA and conveys no proof-verification authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalStateAccumulatorV2 {
    parity: StateRecursiveFoldParityV2,
    bytes: [u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2],
}

impl CanonicalStateAccumulatorV2 {
    /// Decode one exact live accumulator without reducing malformed scalars.
    pub(super) fn decode(
        parity: StateRecursiveFoldParityV2,
        bytes: &[u8],
    ) -> Result<Self, StateRecursiveFoldCodecErrorV2> {
        let bytes: [u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2] =
            bytes.try_into().map_err(|_| {
                StateRecursiveFoldCodecErrorV2::InvalidAccumulatorLength {
                    actual: bytes.len(),
                }
            })?;
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(StateRecursiveFoldCodecErrorV2::BootstrapLineageForbidden);
        }
        match parity {
            StateRecursiveFoldParityV2::Eq => validate_accumulator::<EqAffine>(&bytes)?,
            StateRecursiveFoldParityV2::Ep => validate_accumulator::<EpAffine>(&bytes)?,
        }
        Ok(Self { parity, bytes })
    }

    /// Tagged Pasta parity.
    pub(super) const fn parity(&self) -> StateRecursiveFoldParityV2 {
        self.parity
    }

    /// Exact canonical bytes.
    pub(super) const fn as_bytes(&self) -> &[u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2] {
        &self.bytes
    }
}

fn validate_accumulator<C>(
    bytes: &[u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2],
) -> Result<(), StateRecursiveFoldCodecErrorV2>
where
    C: CurveAffine,
    C::Scalar: PrimeField,
{
    for (index, scalar_bytes) in bytes
        [..STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2]
        .chunks_exact(STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2)
        .enumerate()
    {
        let mut repr = <C::Scalar as PrimeField>::Repr::default();
        if repr.as_ref().len() != STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2 {
            return Err(StateRecursiveFoldCodecErrorV2::NonCanonicalRoundChallenge { index });
        }
        repr.as_mut().copy_from_slice(scalar_bytes);
        if Option::<C::Scalar>::from(C::Scalar::from_repr(repr)).is_none() {
            return Err(StateRecursiveFoldCodecErrorV2::NonCanonicalRoundChallenge { index });
        }
    }

    let point_bytes = &bytes
        [STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2..];
    let mut repr = C::Repr::default();
    if repr.as_ref().len() != STATE_RECURSIVE_FOLD_ELEMENT_BYTES_V2 {
        return Err(StateRecursiveFoldCodecErrorV2::NonCanonicalFoldedGenerator);
    }
    repr.as_mut().copy_from_slice(point_bytes);
    let point = Option::<C>::from(C::from_bytes(&repr))
        .ok_or(StateRecursiveFoldCodecErrorV2::NonCanonicalFoldedGenerator)?;
    if point.to_bytes().as_ref() != point_bytes {
        return Err(StateRecursiveFoldCodecErrorV2::NonCanonicalFoldedGenerator);
    }
    if bool::from(point.is_identity()) {
        return Err(StateRecursiveFoldCodecErrorV2::IdentityFoldedGenerator);
    }
    Ok(())
}

/// Opaque exact-length BGH19 bytes.
///
/// This codec grants no verification authority. The private native relation
/// verifier is the sole current interpreter, and no recursive or production
/// backend consumes these bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OpaqueStateBgh19ProofV2([u8; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2]);

impl OpaqueStateBgh19ProofV2 {
    /// Enforce exact framing and reject the impossible all-zero live transcript.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, StateRecursiveFoldCodecErrorV2> {
        let bytes: [u8; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2] =
            bytes.try_into().map_err(|_| {
                StateRecursiveFoldCodecErrorV2::InvalidBgh19ProofLength {
                    actual: bytes.len(),
                }
            })?;
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(StateRecursiveFoldCodecErrorV2::ZeroBgh19Proof);
        }
        Ok(Self(bytes))
    }

    /// Exact opaque transcript bytes.
    pub(super) const fn as_bytes(&self) -> &[u8; STATE_RECURSIVE_FOLD_BGH19_PROOF_BYTES_V2] {
        &self.0
    }
}

/// Ownership-only failure for the paired recursive-fold result carrier.
///
/// No variant represents BGH19 verification or acceptance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateRecursiveFoldResultOwnershipErrorV2 {
    /// The first claimed output was not the Eq accumulator.
    EqClaimedOutputParityMismatch,
    /// The second claimed output was not the Ep accumulator.
    EpClaimedOutputParityMismatch,
    /// The structural/production boundary has no authorized verifier.
    VerificationUnavailable,
}

impl fmt::Display for StateRecursiveFoldResultOwnershipErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EqClaimedOutputParityMismatch => {
                "offline-cash V2 Eq recursive-fold claimed output has wrong parity"
            }
            Self::EpClaimedOutputParityMismatch => {
                "offline-cash V2 Ep recursive-fold claimed output has wrong parity"
            }
            Self::VerificationUnavailable => {
                "offline-cash V2 production recursive-fold result verification is unavailable"
            }
        })
    }
}

impl std::error::Error for StateRecursiveFoldResultOwnershipErrorV2 {}

/// Exact Eq-then-Ep opaque fold transcripts and their unverified claimed outputs.
///
/// This owner is deliberately neither `Clone` nor `Copy`. Canonical codecs and
/// parity order are enforced, but no relation between a transcript, its inputs,
/// and its claimed output is verified here.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnverifiedStateRecursiveFoldResultPairV2 {
    eq_proof: OpaqueStateBgh19ProofV2,
    eq_claimed_output: CanonicalStateAccumulatorV2,
    ep_proof: OpaqueStateBgh19ProofV2,
    ep_claimed_output: CanonicalStateAccumulatorV2,
}

impl UnverifiedStateRecursiveFoldResultPairV2 {
    /// Own one exact Eq-then-Ep pair without interpreting either BGH19 transcript.
    pub(super) fn from_eq_then_ep(
        eq_proof: OpaqueStateBgh19ProofV2,
        eq_claimed_output: CanonicalStateAccumulatorV2,
        ep_proof: OpaqueStateBgh19ProofV2,
        ep_claimed_output: CanonicalStateAccumulatorV2,
    ) -> Result<Self, StateRecursiveFoldResultOwnershipErrorV2> {
        if eq_claimed_output.parity() != StateRecursiveFoldParityV2::Eq {
            return Err(StateRecursiveFoldResultOwnershipErrorV2::EqClaimedOutputParityMismatch);
        }
        if ep_claimed_output.parity() != StateRecursiveFoldParityV2::Ep {
            return Err(StateRecursiveFoldResultOwnershipErrorV2::EpClaimedOutputParityMismatch);
        }
        Ok(Self {
            eq_proof,
            eq_claimed_output,
            ep_proof,
            ep_claimed_output,
        })
    }

    pub(super) const fn eq_proof(&self) -> &OpaqueStateBgh19ProofV2 {
        &self.eq_proof
    }

    pub(super) const fn eq_claimed_output(&self) -> &CanonicalStateAccumulatorV2 {
        &self.eq_claimed_output
    }

    pub(super) const fn ep_proof(&self) -> &OpaqueStateBgh19ProofV2 {
        &self.ep_proof
    }

    pub(super) const fn ep_claimed_output(&self) -> &CanonicalStateAccumulatorV2 {
        &self.ep_claimed_output
    }
}

/// One structurally decoded fold input. Its origin is not authenticated here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StateRecursiveFoldInputV2 {
    role: StateRecursiveFoldInputRoleV2,
    accumulator: CanonicalStateAccumulatorV2,
}

impl StateRecursiveFoldInputV2 {
    const fn new(
        role: StateRecursiveFoldInputRoleV2,
        accumulator: CanonicalStateAccumulatorV2,
    ) -> Self {
        Self { role, accumulator }
    }

    pub(super) const fn role(&self) -> StateRecursiveFoldInputRoleV2 {
        self.role
    }

    pub(super) const fn accumulator(&self) -> &CanonicalStateAccumulatorV2 {
        &self.accumulator
    }
}

/// Borrowed view of one provenance-bound accumulator in exact fold order.
///
/// This view cannot outlive or detach an accumulator from the owner retaining
/// its semantic-parent or GuardBundle provenance seal. The referenced
/// `CanonicalStateAccumulatorV2` remains a cloneable codec-only value: cloning
/// it conveys no proof authority and cannot recreate a provenance-bound input,
/// whose fields and constructors remain private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateRecursiveFoldInputRefV2<'a> {
    role: StateRecursiveFoldInputRoleV2,
    accumulator: &'a CanonicalStateAccumulatorV2,
}

impl<'a> StateRecursiveFoldInputRefV2<'a> {
    const fn new(
        role: StateRecursiveFoldInputRoleV2,
        accumulator: &'a CanonicalStateAccumulatorV2,
    ) -> Self {
        Self { role, accumulator }
    }

    pub(super) const fn role(self) -> StateRecursiveFoldInputRoleV2 {
        self.role
    }

    pub(super) const fn accumulator(self) -> &'a CanonicalStateAccumulatorV2 {
        self.accumulator
    }
}

/// The only production-shaped GuardBundle input pair accepted by future STATE assembly.
///
/// The opaque seal keeps the complete current-helper/role-6 provenance alive
/// beside the exact Eq and Ep `[Guard.current, Guard.prior]` inputs. Its sole
/// constructor consumes a verified GuardBundle handoff whose production
/// constructor is uninhabited while the recursive verifier is unavailable.
pub(super) struct ProvenanceBoundStateGuardInputsV2 {
    provenance_seal: OfflineCashGuardBundleStateProvenanceSealV2,
    eq_inputs: [StateRecursiveFoldInputV2; 2],
    ep_inputs: [StateRecursiveFoldInputV2; 2],
}

impl ProvenanceBoundStateGuardInputsV2 {
    pub(super) const fn eq_inputs(&self) -> &[StateRecursiveFoldInputV2; 2] {
        &self.eq_inputs
    }

    pub(super) const fn ep_inputs(&self) -> &[StateRecursiveFoldInputV2; 2] {
        &self.ep_inputs
    }

    pub(super) const fn provenance_seal(&self) -> &OfflineCashGuardBundleStateProvenanceSealV2 {
        &self.provenance_seal
    }
}

/// Consume the one verified GuardBundle handoff into exact parity-local STATE inputs.
pub(super) fn state_guard_inputs_from_verified_guard_bundle_v2(
    handoff: VerifiedOfflineCashGuardBundleStateHandoffV2,
) -> ProvenanceBoundStateGuardInputsV2 {
    let (provenance_seal, eq_current, eq_prior, ep_current, ep_prior) =
        handoff.into_state_accumulator_parts_v2().into_parts_v2();
    ProvenanceBoundStateGuardInputsV2 {
        provenance_seal,
        eq_inputs: [
            StateRecursiveFoldInputV2::new(StateRecursiveFoldInputRoleV2::GuardCurrent, eq_current),
            StateRecursiveFoldInputV2::new(StateRecursiveFoldInputRoleV2::GuardPrior, eq_prior),
        ],
        ep_inputs: [
            StateRecursiveFoldInputV2::new(StateRecursiveFoldInputRoleV2::GuardCurrent, ep_current),
            StateRecursiveFoldInputV2::new(StateRecursiveFoldInputRoleV2::GuardPrior, ep_prior),
        ],
    }
}

/// Move-only owner of the exact six recursive inputs for both Pasta parities.
///
/// P0, P1, and GuardBundle remain position-bound and retain their opaque
/// provenance seals. Only ordered borrowed accumulator views are exposed.
pub(super) struct ProvenanceBoundStateSixInputSetV2 {
    parent_0: ProvenanceBoundStateParent0InputsV2,
    parent_1: ProvenanceBoundStateParent1InputsV2,
    guard: ProvenanceBoundStateGuardInputsV2,
}

impl ProvenanceBoundStateSixInputSetV2 {
    /// Borrow the position-bound P0 owner, including its opaque provenance seal.
    pub(super) const fn parent_0(&self) -> &ProvenanceBoundStateParent0InputsV2 {
        &self.parent_0
    }

    /// Borrow the position-bound P1 owner, including its opaque provenance seal.
    pub(super) const fn parent_1(&self) -> &ProvenanceBoundStateParent1InputsV2 {
        &self.parent_1
    }

    /// Borrow the GuardBundle owner, including its opaque provenance seal.
    pub(super) const fn guard(&self) -> &ProvenanceBoundStateGuardInputsV2 {
        &self.guard
    }

    /// Exact Eq order: P0 current/prior, P1 current/prior, Guard current/prior.
    pub(super) fn eq_inputs(
        &self,
    ) -> [StateRecursiveFoldInputRefV2<'_>; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
        [
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent0Current,
                self.parent_0.eq_current(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent0Prior,
                self.parent_0.eq_prior(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent1Current,
                self.parent_1.eq_current(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent1Prior,
                self.parent_1.eq_prior(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::GuardCurrent,
                self.guard.eq_inputs()[0].accumulator(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::GuardPrior,
                self.guard.eq_inputs()[1].accumulator(),
            ),
        ]
    }

    /// Exact Ep order: P0 current/prior, P1 current/prior, Guard current/prior.
    pub(super) fn ep_inputs(
        &self,
    ) -> [StateRecursiveFoldInputRefV2<'_>; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
        [
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent0Current,
                self.parent_0.ep_current(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent0Prior,
                self.parent_0.ep_prior(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent1Current,
                self.parent_1.ep_current(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::Parent1Prior,
                self.parent_1.ep_prior(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::GuardCurrent,
                self.guard.ep_inputs()[0].accumulator(),
            ),
            StateRecursiveFoldInputRefV2::new(
                StateRecursiveFoldInputRoleV2::GuardPrior,
                self.guard.ep_inputs()[1].accumulator(),
            ),
        ]
    }
}

/// Consume all three provenance-bound owners into one immutable six-input set.
pub(super) fn assemble_provenance_bound_state_six_input_set_v2(
    parent_0: ProvenanceBoundStateParent0InputsV2,
    parent_1: ProvenanceBoundStateParent1InputsV2,
    guard: ProvenanceBoundStateGuardInputsV2,
) -> ProvenanceBoundStateSixInputSetV2 {
    ProvenanceBoundStateSixInputSetV2 {
        parent_0,
        parent_1,
        guard,
    }
}

/// Move-only ownership link from exact semantic-parent/GuardBundle provenance
/// to the paired opaque fold transcripts and their unverified claimed outputs.
///
/// The six inputs remain inside their provenance-bound owner and are exposed
/// only as ordered borrowed views. Co-ownership records the dependency without
/// claiming that either BGH19 transcript produces its claimed output.
pub(super) struct ProvenanceBoundStateRecursiveFoldResultV2 {
    inputs: ProvenanceBoundStateSixInputSetV2,
    result: UnverifiedStateRecursiveFoldResultPairV2,
}

impl ProvenanceBoundStateRecursiveFoldResultV2 {
    pub(super) const fn inputs(&self) -> &ProvenanceBoundStateSixInputSetV2 {
        &self.inputs
    }

    pub(super) const fn result(&self) -> &UnverifiedStateRecursiveFoldResultPairV2 {
        &self.result
    }

    /// Exact borrowed Eq order retained by the provenance-bound input owner.
    pub(super) fn eq_inputs(
        &self,
    ) -> [StateRecursiveFoldInputRefV2<'_>; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
        self.inputs.eq_inputs()
    }

    /// Exact borrowed Ep order retained by the provenance-bound input owner.
    pub(super) fn ep_inputs(
        &self,
    ) -> [StateRecursiveFoldInputRefV2<'_>; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
        self.inputs.ep_inputs()
    }
}

/// Consume the exact provenance-bound six-input owner and unverified fold result
/// pair into one non-authorizing carrier without duplicating accumulator arrays.
pub(super) fn assemble_provenance_bound_state_recursive_fold_result_v2(
    inputs: ProvenanceBoundStateSixInputSetV2,
    result: UnverifiedStateRecursiveFoldResultPairV2,
) -> ProvenanceBoundStateRecursiveFoldResultV2 {
    ProvenanceBoundStateRecursiveFoldResultV2 { inputs, result }
}

/// Fail closed before a candidate can cross the structural production boundary.
///
/// The private native relation verifier is deliberately separate and grants no
/// recursive-backend, readiness, release, or production authority.
pub(super) fn fail_closed_provenance_bound_state_recursive_fold_result_v2(
    _candidate: ProvenanceBoundStateRecursiveFoldResultV2,
) -> Result<Infallible, StateRecursiveFoldResultOwnershipErrorV2> {
    Err(StateRecursiveFoldResultOwnershipErrorV2::VerificationUnavailable)
}

/// Structurally valid six-input envelope. It is deliberately named unverified.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnverifiedStateRecursiveFoldEnvelopeV2 {
    parity: StateRecursiveFoldParityV2,
    inputs: [StateRecursiveFoldInputV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
    proof: OpaqueStateBgh19ProofV2,
    claimed_output: CanonicalStateAccumulatorV2,
}

impl UnverifiedStateRecursiveFoldEnvelopeV2 {
    /// Check parity, role order, codecs, and byte shape without verifying BGH19.
    pub(super) fn from_structural_parts(
        parity: StateRecursiveFoldParityV2,
        inputs: [StateRecursiveFoldInputV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2],
        proof: OpaqueStateBgh19ProofV2,
        claimed_output: CanonicalStateAccumulatorV2,
    ) -> Result<Self, StateRecursiveFoldCodecErrorV2> {
        for (index, (input, expected_role)) in inputs
            .iter()
            .zip(STATE_RECURSIVE_FOLD_INPUT_ORDER_V2)
            .enumerate()
        {
            if input.role != expected_role {
                return Err(StateRecursiveFoldCodecErrorV2::InputOrderMismatch { index });
            }
            if input.accumulator.parity != parity {
                return Err(StateRecursiveFoldCodecErrorV2::ParityMismatch { index });
            }
        }
        if claimed_output.parity != parity {
            return Err(StateRecursiveFoldCodecErrorV2::ParityMismatch {
                index: STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2,
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
    ) -> &[StateRecursiveFoldInputV2; STATE_RECURSIVE_FOLD_INPUTS_PER_PARITY_V2] {
        &self.inputs
    }

    pub(super) const fn proof(&self) -> &OpaqueStateBgh19ProofV2 {
        &self.proof
    }

    pub(super) const fn claimed_output(&self) -> &CanonicalStateAccumulatorV2 {
        &self.claimed_output
    }
}

/// Decode the live prior lineage from the exact field-neutral STATE word ABI.
///
/// This validates layout and accumulator codecs only. It does not authenticate
/// the semantic statement or compiled protocol.
pub(super) fn decode_prior_lineage_from_state_words_v2(
    parity: StateRecursiveFoldParityV2,
    words: &[u32],
) -> Result<CanonicalStateAccumulatorV2, StateRecursiveFoldCodecErrorV2> {
    if words.len() != STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2 {
        return Err(StateRecursiveFoldCodecErrorV2::InvalidStateLength {
            actual: words.len(),
        });
    }
    if words[..8]
        != [
            2,
            2,
            STATE_RECURSIVE_FOLD_K_V2,
            parity as u32,
            words[4],
            2,
            8,
            STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2 as u32,
        ]
        || !matches!(words[4], 1 | 2)
        || words[STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2
            ..STATE_RECURSIVE_FOLD_AMOUNT_WORD_START_V2 + 4]
            .iter()
            .all(|word| *word == 0)
        || words[STATE_RECURSIVE_FOLD_SCALE_WORD_V2] > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
    {
        return Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader);
    }
    if STATE_RECURSIVE_FOLD_DIGEST_WORD_STARTS_V2
        .iter()
        .any(|start| words[*start..*start + 8].iter().all(|word| *word == 0))
    {
        return Err(StateRecursiveFoldCodecErrorV2::InvalidStateHeader);
    }

    let mut bytes = [0_u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2];
    for (target, word) in bytes.chunks_exact_mut(4).zip(
        &words[STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2
            ..STATE_RECURSIVE_FOLD_LINEAGE_WORD_START_V2 + STATE_RECURSIVE_FOLD_LINEAGE_WORDS_V2],
    ) {
        target.copy_from_slice(&word.to_le_bytes());
    }
    CanonicalStateAccumulatorV2::decode(parity, &bytes)
}

/// Pack exact STATE words into 34 field-neutral 28-byte cells.
pub(super) fn pack_state_words_v2(
    parity: StateRecursiveFoldParityV2,
    words: &[u32],
) -> Result<
    [[u8; STATE_RECURSIVE_FOLD_PACKED_CELL_BYTES_V2]; STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2],
    StateRecursiveFoldCodecErrorV2,
> {
    decode_prior_lineage_from_state_words_v2(parity, words)?;
    let mut cells = [[0_u8; STATE_RECURSIVE_FOLD_PACKED_CELL_BYTES_V2];
        STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2];
    for (index, word) in words.iter().enumerate() {
        let cell = index / STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2;
        let offset = index % STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2 * 4;
        cells[cell][offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    Ok(cells)
}

/// Strictly unpack 34 cells, reject nonzero final padding, and revalidate the
/// parity-local lineage beginning at word 93.
pub(super) fn unpack_state_cells_v2(
    parity: StateRecursiveFoldParityV2,
    cells: &[[u8; STATE_RECURSIVE_FOLD_PACKED_CELL_BYTES_V2]],
) -> Result<[u32; STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2], StateRecursiveFoldCodecErrorV2> {
    if cells.len() != STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2 {
        return Err(StateRecursiveFoldCodecErrorV2::InvalidStateLength {
            actual: cells.len(),
        });
    }
    let padding_bytes = STATE_RECURSIVE_FOLD_FINAL_CELL_ZERO_PADDING_WORDS_V2 * 4;
    if cells[STATE_RECURSIVE_FOLD_STATE_INSTANCE_CELLS_V2 - 1]
        [STATE_RECURSIVE_FOLD_PACKED_CELL_BYTES_V2 - padding_bytes..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(StateRecursiveFoldCodecErrorV2::NonCanonicalFinalCellPadding);
    }

    let mut words = [0_u32; STATE_RECURSIVE_FOLD_STATE_ABI_WORDS_V2];
    for (index, word) in words.iter_mut().enumerate() {
        let cell = &cells[index / STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2];
        let offset = index % STATE_RECURSIVE_FOLD_WORDS_PER_INSTANCE_V2 * 4;
        *word = u32::from_le_bytes(
            cell[offset..offset + 4]
                .try_into()
                .expect("one packed STATE word is four bytes"),
        );
    }
    decode_prior_lineage_from_state_words_v2(parity, &words)?;
    Ok(words)
}

mod sealed {
    pub trait Sealed {}
}

/// Marker implemented only by uninhabited adapters in this source module.
pub(super) trait SealedStateRecursiveAdapterV2: sealed::Sealed {}

/// Uninhabited move-only compiler adapter. No circuit compiler is wired.
pub(super) enum StateRecursiveCompilerAdapterV2 {}
impl sealed::Sealed for StateRecursiveCompilerAdapterV2 {}
impl SealedStateRecursiveAdapterV2 for StateRecursiveCompilerAdapterV2 {}

/// Uninhabited move-only circuit adapter. No recursive circuit exists.
pub(super) enum StateRecursiveCircuitAdapterV2 {}
impl sealed::Sealed for StateRecursiveCircuitAdapterV2 {}
impl SealedStateRecursiveAdapterV2 for StateRecursiveCircuitAdapterV2 {}

/// Uninhabited move-only authenticated artifact adapter.
pub(super) enum StateRecursiveArtifactAdapterV2 {}
impl sealed::Sealed for StateRecursiveArtifactAdapterV2 {}
impl SealedStateRecursiveAdapterV2 for StateRecursiveArtifactAdapterV2 {}

/// Uninhabited move-only production backend adapter.
pub(super) enum StateRecursiveProductionAdapterV2 {}
impl sealed::Sealed for StateRecursiveProductionAdapterV2 {}
impl SealedStateRecursiveAdapterV2 for StateRecursiveProductionAdapterV2 {}

#[cfg(test)]
#[path = "state_recursive_fold_tests.rs"]
mod tests;
