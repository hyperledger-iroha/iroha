//! Inert structural owner for a future sole-global-lookup proof session.
//!
//! This child freezes only inventories, typestates, entropy coordinates,
//! one-shot replay permits, and arithmetic ledgers. Production entropy,
//! source ownership, materialization, and state transitions are uninhabited.
//! It adds no proof, transcript, wire format, receipt, or release authority.

#![allow(
    dead_code,
    reason = "the structural proof-session materializer is intentionally uninhabited"
)]

use core::{convert::Infallible, marker::PhantomData};

const PHYSICAL_INVENTORY_V3: u32 = 71_109;
const SOURCE_COMMITMENTS_V3: u32 = 344;
const SUFFIX_AFTER_SOURCE_V3: u32 = 70_765;
const PRE_Z_COMMITMENTS_V3: u32 = 39_338;
const GLOBAL_INVERSES_V3: u32 = 31_768;
const POST_DELTA_RESIDUALS_V3: u32 = 3;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalPhaseV3 {
    ChallengeIndependent = 1,
    JointPostZ = 2,
    PostDeltaResidual = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalPurposeV3 {
    Source = 1,
    ExistingDifferenceLow = 2,
    ExistingSumLow = 3,
    ComparatorDifferenceTop = 4,
    ComparatorSumTop = 5,
    ComparatorDifferenceDigit = 6,
    ComparatorBorrow = 7,
    ComparatorMixedTop = 8,
    SmallSigned = 9,
    SmallNegativeMagnitude = 10,
    QMaskDigit = 11,
    QMaskComplementDigit = 12,
    Multiplicity = 13,
    SumcheckMask = 14,
    SharedDifferenceInverse = 15,
    SharedSumInverse = 16,
    ComparatorDifferenceInverse = 17,
    SmallSignedInverse = 18,
    SmallNegativeInverse = 19,
    QMaskDigitInverse = 20,
    QMaskComplementInverse = 21,
    ResidualQ3 = 22,
    ResidualQ5 = 23,
    ResidualQ8 = 24,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PhysicalRoleRangeV3 {
    phase: PhysicalPhaseV3,
    purpose: PhysicalPurposeV3,
    first: u32,
    count: u32,
}

const fn physical_range_v3(
    phase: PhysicalPhaseV3,
    purpose: PhysicalPurposeV3,
    first: u32,
    count: u32,
) -> PhysicalRoleRangeV3 {
    PhysicalRoleRangeV3 {
        phase,
        purpose,
        first,
        count,
    }
}

#[rustfmt::skip]
const PHYSICAL_ROLE_RANGES_V3: [PhysicalRoleRangeV3; 24] = [
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::Source, 0, 344),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ExistingDifferenceLow, 344, 5_848),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ExistingSumLow, 6_192, 5_848),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ComparatorDifferenceTop, 12_040, 344),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ComparatorSumTop, 12_384, 344),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ComparatorDifferenceDigit, 12_728, 5_848),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ComparatorBorrow, 18_576, 6_192),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::ComparatorMixedTop, 24_768, 344),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::SmallSigned, 25_112, 1_032),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::SmallNegativeMagnitude, 26_144, 1_032),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::QMaskDigit, 27_176, 6_080),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::QMaskComplementDigit, 33_256, 6_080),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::Multiplicity, 39_336, 1),
    physical_range_v3(PhysicalPhaseV3::ChallengeIndependent, PhysicalPurposeV3::SumcheckMask, 39_337, 1),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::SharedDifferenceInverse, 39_338, 5_848),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::SharedSumInverse, 45_186, 5_848),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::ComparatorDifferenceInverse, 51_034, 5_848),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::SmallSignedInverse, 56_882, 1_032),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::SmallNegativeInverse, 57_914, 1_032),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::QMaskDigitInverse, 58_946, 6_080),
    physical_range_v3(PhysicalPhaseV3::JointPostZ, PhysicalPurposeV3::QMaskComplementInverse, 65_026, 6_080),
    physical_range_v3(PhysicalPhaseV3::PostDeltaResidual, PhysicalPurposeV3::ResidualQ3, 71_106, 1),
    physical_range_v3(PhysicalPhaseV3::PostDeltaResidual, PhysicalPurposeV3::ResidualQ5, 71_107, 1),
    physical_range_v3(PhysicalPhaseV3::PostDeltaResidual, PhysicalPurposeV3::ResidualQ8, 71_108, 1),
];

const VECTOR_ALIAS_COUNT_V3: u32 = 9_291;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AliasPurposeV3 {
    BooleanD = 1,
    BooleanS = 2,
    ComparatorBorrow = 3,
    MixedTop = 4,
    SmallSigned = 5,
    SmallNegativeMagnitude = 6,
    ResidualQ3 = 7,
    ResidualQ5 = 8,
    ResidualQ8 = 9,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AliasRangeV3 {
    purpose: AliasPurposeV3,
    logical_first: u32,
    physical_first: u32,
    count: u32,
}

const fn alias_range_v3(
    purpose: AliasPurposeV3,
    logical_first: u32,
    physical_first: u32,
    count: u32,
) -> AliasRangeV3 {
    AliasRangeV3 {
        purpose,
        logical_first,
        physical_first,
        count,
    }
}

#[rustfmt::skip]
const ALIAS_RANGES_V3: [AliasRangeV3; 9] = [
    alias_range_v3(AliasPurposeV3::BooleanD, 0, 12_040, 344),
    alias_range_v3(AliasPurposeV3::BooleanS, 344, 12_384, 344),
    alias_range_v3(AliasPurposeV3::ComparatorBorrow, 688, 18_576, 6_192),
    alias_range_v3(AliasPurposeV3::MixedTop, 6_880, 24_768, 344),
    alias_range_v3(AliasPurposeV3::SmallSigned, 7_224, 25_112, 1_032),
    alias_range_v3(AliasPurposeV3::SmallNegativeMagnitude, 8_256, 26_144, 1_032),
    alias_range_v3(AliasPurposeV3::ResidualQ3, 9_288, 71_106, 1),
    alias_range_v3(AliasPurposeV3::ResidualQ5, 9_289, 71_107, 1),
    alias_range_v3(AliasPurposeV3::ResidualQ8, 9_290, 71_108, 1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AliasCoordinateV3 {
    purpose: AliasPurposeV3,
    logical_ordinal: u32,
    purpose_ordinal: u32,
    physical_ordinal: u32,
}

fn alias_coordinate_v3(
    logical_ordinal: u32,
) -> Result<AliasCoordinateV3, ProofSessionStructuralErrorV3> {
    for range in ALIAS_RANGES_V3 {
        let end = range
            .logical_first
            .checked_add(range.count)
            .ok_or(ProofSessionStructuralErrorV3::Manifest)?;
        if logical_ordinal >= range.logical_first && logical_ordinal < end {
            let purpose_ordinal = logical_ordinal - range.logical_first;
            return Ok(AliasCoordinateV3 {
                purpose: range.purpose,
                logical_ordinal,
                purpose_ordinal,
                physical_ordinal: range.physical_first + purpose_ordinal,
            });
        }
    }
    Err(ProofSessionStructuralErrorV3::Alias)
}

const SUFFIX_SEMANTIC_RECORD_BYTES_V3: u64 = 65;
const AUTHENTICATION_TAG_BYTES_V3: u64 = 16;
const SUFFIX_PLAINTEXT_BYTES_V3: u64 = 4_599_725;
const SUFFIX_TAG_BYTES_V3: u64 = 1_132_240;
const SUFFIX_FILE_BYTES_V3: u64 = 5_731_965;
const SUFFIX_WRITE_AND_SEAL_BYTES_V3: u64 = 11_463_930;

const COMPOSITE_BLINDING_BYTES_V3: u64 = 2_275_488;
const COMPOSITE_POINT_BYTES_V3: u64 = 2_346_597;
const COMPOSITE_SEMANTIC_BYTES_V3: u64 = 4_622_085;
const COMPOSITE_TAG_BYTES_V3: u64 = 1_137_744;
const COMPOSITE_FILE_BYTES_V3: u64 = 5_759_829;

const PLANE_SLOTS_V3: u64 = 306_603;
const PLANE_FILE_BYTES_V3: u64 = 5_028_289_200;
const AUXILIARY_READ_A_BYTES_V3: u64 = 33_849_600;
const AUXILIARY_READ_B_BYTES_V3: u64 = 33_849_600;
const AUXILIARY_READ_C_BYTES_V3: u64 = 752_571;
const AUXILIARY_READ_BYTES_V3: u64 = 68_451_771;

const TENSOR_TERM_REPLAY_BYTES_V3: u64 = 68_262_835_200;
const TERMINAL_AGGREGATE_REPLAY_BYTES_V3: u64 = 4_875_916_800;
const COEFFICIENT_IPA_REPLAY_BYTES_V3: u64 = 152_372_400;
const ENDPOINT_OPENING_REPLAY_BYTES_V3: u64 = 1_623_600;
const REPLAY_READ_BYTES_V3: u64 = 73_292_748_000;
const PLANE_WRITE_AND_SEAL_BYTES_V3: u64 = 10_056_578_400;
const STRUCTURAL_IO_BYTES_V3: u64 = 83_349_326_400;

const HEAP_LANGUAGE_V3: &[u8] = b"inventory-asymptotic-O(1);fixed-role-array;fixed-alias-array;fixed-permit-array;not-an-RSS-claim";
const OWNER_REHOME_LANGUAGE_V3: &[u8] = b"future-integration-consumes-and-rehomes-existing-source,retained-openings,existing-sumcheck,and-tensor-sumcheck-owners-as-siblings;no-recursive-radix-nesting";

const PRODUCTION_ENTROPY_INHABITED_V3: bool = false;
const PRODUCTION_SOURCE_INHABITED_V3: bool = false;
const PRODUCTION_MATERIALIZER_INHABITED_V3: bool = false;
const PRODUCTION_TRANSITIONS_INHABITED_V3: bool = false;
const PROOF_SESSION_WIRED_V3: bool = false;
const PROOF_VERIFIED_V3: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V3: bool = false;
const COMPLETE_ACCOUNTING_QUALIFIED_V3: bool = false;
const RSS_QUALIFIED_V3: bool = false;
const READINESS_QUALIFIED_V3: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V3: bool = false;
const AUTHORITY_MINTED_V3: bool = false;
const RELEASE_READY_V3: bool = false;
const RELEASE_COMPLETE_V3: bool = false;

const _: () = {
    assert!(PHYSICAL_INVENTORY_V3 == 71_109);
    assert!(SUFFIX_AFTER_SOURCE_V3 == PHYSICAL_INVENTORY_V3 - SOURCE_COMMITMENTS_V3);
    assert!(PRE_Z_COMMITMENTS_V3 + GLOBAL_INVERSES_V3 + POST_DELTA_RESIDUALS_V3 == 71_109);
    assert!(VECTOR_ALIAS_COUNT_V3 == 9_291);
    assert!(SUFFIX_PLAINTEXT_BYTES_V3 == SUFFIX_AFTER_SOURCE_V3 as u64 * 65);
    assert!(SUFFIX_TAG_BYTES_V3 == SUFFIX_AFTER_SOURCE_V3 as u64 * 16);
    assert!(SUFFIX_FILE_BYTES_V3 == 5_731_965);
    assert!(SUFFIX_WRITE_AND_SEAL_BYTES_V3 == 11_463_930);
    assert!(COMPOSITE_BLINDING_BYTES_V3 == PHYSICAL_INVENTORY_V3 as u64 * 32);
    assert!(COMPOSITE_POINT_BYTES_V3 == PHYSICAL_INVENTORY_V3 as u64 * 33);
    assert!(COMPOSITE_SEMANTIC_BYTES_V3 == 4_622_085);
    assert!(COMPOSITE_TAG_BYTES_V3 == PHYSICAL_INVENTORY_V3 as u64 * 16);
    assert!(COMPOSITE_FILE_BYTES_V3 == 5_759_829);
    assert!(PLANE_SLOTS_V3 == 306_603 && PLANE_FILE_BYTES_V3 == 5_028_289_200);
    assert!(AUXILIARY_READ_BYTES_V3 == 68_451_771);
    assert!(REPLAY_READ_BYTES_V3 == 73_292_748_000);
    assert!(PLANE_WRITE_AND_SEAL_BYTES_V3 == 2 * PLANE_FILE_BYTES_V3);
    assert!(STRUCTURAL_IO_BYTES_V3 == 83_349_326_400);
    assert!(!PRODUCTION_ENTROPY_INHABITED_V3);
    assert!(!PRODUCTION_SOURCE_INHABITED_V3);
    assert!(!PRODUCTION_MATERIALIZER_INHABITED_V3);
    assert!(!PRODUCTION_TRANSITIONS_INHABITED_V3);
    assert!(!PROOF_SESSION_WIRED_V3 && !PROOF_VERIFIED_V3 && !ZERO_KNOWLEDGE_ACCEPTED_V3);
    assert!(!COMPLETE_ACCOUNTING_QUALIFIED_V3 && !RSS_QUALIFIED_V3);
    assert!(!READINESS_QUALIFIED_V3 && !OPERATIONAL_RECEIPT_ACCEPTED_V3);
    assert!(!AUTHORITY_MINTED_V3 && !RELEASE_READY_V3 && !RELEASE_COMPLETE_V3);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofSessionStructuralErrorV3 {
    Manifest,
    Alias,
    Entropy,
    Poisoned,
    PermitConsumed,
    Validation,
    Io,
    Incomplete,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EntropyPhaseV3 {
    PreZ = 0,
    SoleZ = 1,
    PostZ = 2,
    DeltaResiduals = 3,
    ExistingSumcheck = 4,
    TensorChallenges = 5,
    TensorSumcheck = 6,
    Endpoints = 7,
    Openings = 8,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EntropyPurposeV3 {
    CommitmentBlinding = 0,
    ChallengeRejection = 1,
    SumcheckMask = 2,
    IpaMask = 3,
}

const MAX_ENTROPY_RETRY_V3: u8 = 127;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntropyCoordinateV3 {
    phase: EntropyPhaseV3,
    purpose: EntropyPurposeV3,
    ordinal: u32,
    retry: u8,
}

impl EntropyCoordinateV3 {
    fn new_v3(
        phase: EntropyPhaseV3,
        purpose: EntropyPurposeV3,
        ordinal: u32,
        retry: u8,
    ) -> Result<Self, ProofSessionStructuralErrorV3> {
        if retry > MAX_ENTROPY_RETRY_V3 {
            return Err(ProofSessionStructuralErrorV3::Entropy);
        }
        Ok(Self {
            phase,
            purpose,
            ordinal,
            retry,
        })
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProofReplayPurposeV3 {
    TensorTermRound0 = 0,
    TensorTermRound1 = 1,
    TensorTermRound2 = 2,
    TensorTermRound3 = 3,
    TensorTermRound4 = 4,
    TensorTermRound5 = 5,
    TensorTermRound6 = 6,
    TensorTermRound7 = 7,
    TensorTermRound8 = 8,
    TensorTermRound9 = 9,
    TensorTermRound10 = 10,
    TensorTermRound11 = 11,
    TensorTermRound12 = 12,
    TensorTermRound13 = 13,
    TerminalAggregate = 14,
    CoefficientIpa3 = 15,
    CoefficientIpa5 = 16,
    CoefficientIpa8 = 17,
}

#[rustfmt::skip]
const REPLAY_PURPOSES_V3: [ProofReplayPurposeV3; 18] = [
    ProofReplayPurposeV3::TensorTermRound0, ProofReplayPurposeV3::TensorTermRound1,
    ProofReplayPurposeV3::TensorTermRound2, ProofReplayPurposeV3::TensorTermRound3,
    ProofReplayPurposeV3::TensorTermRound4, ProofReplayPurposeV3::TensorTermRound5,
    ProofReplayPurposeV3::TensorTermRound6, ProofReplayPurposeV3::TensorTermRound7,
    ProofReplayPurposeV3::TensorTermRound8, ProofReplayPurposeV3::TensorTermRound9,
    ProofReplayPurposeV3::TensorTermRound10, ProofReplayPurposeV3::TensorTermRound11,
    ProofReplayPurposeV3::TensorTermRound12, ProofReplayPurposeV3::TensorTermRound13,
    ProofReplayPurposeV3::TerminalAggregate, ProofReplayPurposeV3::CoefficientIpa3,
    ProofReplayPurposeV3::CoefficientIpa5, ProofReplayPurposeV3::CoefficientIpa8,
];
const REPLAY_COMPLETION_MASK_V3: u32 = 0x3ffff;

impl ProofReplayPurposeV3 {
    const fn index_v3(self) -> usize {
        self as usize
    }
}

struct ProofReplayPermitV3 {
    purpose: ProofReplayPurposeV3,
}

struct ProofReplayPermitsV3 {
    slots: [Option<ProofReplayPermitV3>; 18],
    completion_mask: u32,
}

impl ProofReplayPermitsV3 {
    fn new_v3() -> Self {
        Self {
            slots: core::array::from_fn(|index| {
                Some(ProofReplayPermitV3 {
                    purpose: REPLAY_PURPOSES_V3[index],
                })
            }),
            completion_mask: 0,
        }
    }

    fn remove_v3(
        &mut self,
        purpose: ProofReplayPurposeV3,
    ) -> Result<ProofReplayPermitV3, ProofSessionStructuralErrorV3> {
        self.slots[purpose.index_v3()]
            .take()
            .ok_or(ProofSessionStructuralErrorV3::PermitConsumed)
    }

    fn complete_v3(
        &mut self,
        permit: ProofReplayPermitV3,
    ) -> Result<(), ProofSessionStructuralErrorV3> {
        let bit = 1_u32 << permit.purpose.index_v3();
        if self.completion_mask & bit != 0 {
            return Err(ProofSessionStructuralErrorV3::PermitConsumed);
        }
        self.completion_mask |= bit;
        Ok(())
    }

    fn finish_v3(self) -> Result<(), ProofSessionStructuralErrorV3> {
        if self.slots.iter().any(Option::is_some)
            || self.completion_mask != REPLAY_COMPLETION_MASK_V3
        {
            return Err(ProofSessionStructuralErrorV3::Incomplete);
        }
        Ok(())
    }

    #[cfg(test)]
    fn take_for_test_v3(
        &mut self,
        purpose: ProofReplayPurposeV3,
        outcome: TestReplayOutcomeV3,
    ) -> Result<ProofReplayPermitV3, ProofSessionStructuralErrorV3> {
        let permit = self.remove_v3(purpose)?;
        match outcome {
            TestReplayOutcomeV3::Success => Ok(permit),
            TestReplayOutcomeV3::ValidationError => Err(ProofSessionStructuralErrorV3::Validation),
            TestReplayOutcomeV3::IoError => Err(ProofSessionStructuralErrorV3::Io),
            TestReplayOutcomeV3::Unwind => panic!("injected structural replay unwind"),
        }
    }
}

struct PreZOpenings;
struct SoleZLive;
struct PostZBound;
struct PostDeltaResiduals;
struct RetainedOpenings;
struct ExistingSumcheckComplete;
struct TensorChallenges;
struct TensorSumcheckComplete;
struct EndpointsBound;
struct OpeningsBound;
struct Verified;

struct ProductionEntropySourceV3 {
    entropy: Infallible,
}

struct ProductionSourceOwnerV3 {
    source: Infallible,
}

struct ProductionComponentSealV3 {
    component: Infallible,
}

enum OwnedSiblingComponentsV3<R> {
    Production {
        entropy: ProductionEntropySourceV3,
        source: ProductionSourceOwnerV3,
        retained_openings: ProductionComponentSealV3,
        existing_sumcheck: ProductionComponentSealV3,
        tensor_sumcheck: ProductionComponentSealV3,
        relation: PhantomData<R>,
    },
    #[cfg(test)]
    TestOnly(TestOwnedSiblingComponentsV3),
}

struct ProductionSessionMaterializerV3<R> {
    siblings: OwnedSiblingComponentsV3<R>,
    materializer: Infallible,
}

struct FutureTransitionSealV3<Next> {
    rehome_existing_owners: Infallible,
    next: PhantomData<Next>,
}

struct GlobalLookupProofSessionLiveV3<R> {
    siblings: OwnedSiblingComponentsV3<R>,
    permits: ProofReplayPermitsV3,
}

#[must_use = "dropping the session destroys its sole live structural owner"]
struct GlobalLookupProofSessionV3<R, State> {
    live: Option<GlobalLookupProofSessionLiveV3<R>>,
    poisoned: bool,
    state: PhantomData<State>,
}

impl<R> GlobalLookupProofSessionV3<R, PreZOpenings> {
    fn materialize_v3(materializer: ProductionSessionMaterializerV3<R>) -> Self {
        let ProductionSessionMaterializerV3 { materializer, .. } = materializer;
        match materializer {}
    }

    #[cfg(test)]
    fn test_only_v3(secret_probe: [u8; 32]) -> Self {
        Self {
            live: Some(GlobalLookupProofSessionLiveV3 {
                siblings: OwnedSiblingComponentsV3::TestOnly(TestOwnedSiblingComponentsV3 {
                    secret_probe,
                }),
                permits: ProofReplayPermitsV3::new_v3(),
            }),
            poisoned: false,
            state: PhantomData,
        }
    }
}

impl<R, State> GlobalLookupProofSessionV3<R, State> {
    fn advance_v3<Next>(
        mut self,
        transition: FutureTransitionSealV3<Next>,
    ) -> Result<GlobalLookupProofSessionV3<R, Next>, ProofSessionStructuralErrorV3> {
        if self.poisoned {
            return Err(ProofSessionStructuralErrorV3::Poisoned);
        }
        self.poisoned = true;
        let _live = self
            .live
            .take()
            .ok_or(ProofSessionStructuralErrorV3::Poisoned)?;
        match transition.rehome_existing_owners {}
    }

    #[cfg(test)]
    fn advance_for_test_v3<Next>(
        mut self,
        outcome: TestReplayOutcomeV3,
    ) -> Result<GlobalLookupProofSessionV3<R, Next>, ProofSessionStructuralErrorV3> {
        if self.poisoned {
            return Err(ProofSessionStructuralErrorV3::Poisoned);
        }
        self.poisoned = true;
        let live = self
            .live
            .take()
            .ok_or(ProofSessionStructuralErrorV3::Poisoned)?;
        match outcome {
            TestReplayOutcomeV3::Success => Ok(GlobalLookupProofSessionV3 {
                live: Some(live),
                poisoned: false,
                state: PhantomData,
            }),
            TestReplayOutcomeV3::ValidationError => Err(ProofSessionStructuralErrorV3::Validation),
            TestReplayOutcomeV3::IoError => Err(ProofSessionStructuralErrorV3::Io),
            TestReplayOutcomeV3::Unwind => panic!("injected structural transition unwind"),
        }
    }

    #[cfg(test)]
    fn replay_once_for_test_v3(
        mut self,
        purpose: ProofReplayPurposeV3,
        coordinate: EntropyCoordinateV3,
        outcome: TestReplayOutcomeV3,
    ) -> Result<Self, ProofSessionStructuralErrorV3> {
        if self.poisoned {
            return Err(ProofSessionStructuralErrorV3::Poisoned);
        }
        self.poisoned = true;
        let mut live = self
            .live
            .take()
            .ok_or(ProofSessionStructuralErrorV3::Poisoned)?;
        let permit = live.permits.remove_v3(purpose)?;
        if coordinate.retry > MAX_ENTROPY_RETRY_V3 {
            return Err(ProofSessionStructuralErrorV3::Entropy);
        }
        match outcome {
            TestReplayOutcomeV3::Success => {}
            TestReplayOutcomeV3::ValidationError => {
                return Err(ProofSessionStructuralErrorV3::Validation);
            }
            TestReplayOutcomeV3::IoError => return Err(ProofSessionStructuralErrorV3::Io),
            TestReplayOutcomeV3::Unwind => panic!("injected structural session unwind"),
        }
        live.permits.complete_v3(permit)?;
        self.live = Some(live);
        self.poisoned = false;
        Ok(self)
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum TestReplayOutcomeV3 {
    Success,
    ValidationError,
    IoError,
    Unwind,
}

#[cfg(test)]
static TEST_OWNED_SIBLING_DROPS_V3: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
struct TestOwnedSiblingComponentsV3 {
    secret_probe: [u8; 32],
}

#[cfg(test)]
impl Drop for TestOwnedSiblingComponentsV3 {
    fn drop(&mut self) {
        self.secret_probe.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        debug_assert!(self.secret_probe.iter().all(|byte| *byte == 0));
        TEST_OWNED_SIBLING_DROPS_V3.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
#[path = "global_lookup_proof_session_v3_tests.rs"]
mod tests;
