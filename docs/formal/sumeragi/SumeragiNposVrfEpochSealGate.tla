---- MODULE SumeragiNposVrfEpochSealGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for NPoS VRF epoch-seal staging.

This slice models the consensus-side contracts in
`stage_vrf_epoch_record`, `reconcile_pending_vrf_record_with_committed`,
`merge_vrf_epoch_records`, `committed_vrf_record_covers_pending`,
`note_committed_npos_effects`, `activation_plan_from_vrf_record`, and the
block-level NPoS VRF effect admission guard. The model abstracts record fields
to representative cases while preserving the key safety obligations: epoch
identity fields are immutable, observations only extend, finalized and penalty
markers are sticky, unfinalized records cannot carry offender lists, conflicting
pending records are removed rather than proposed, and elected rosters activate
only after their finality margin.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

TlcSingletonOrEmpty == Cardinality(tried) \in {0, 1}

MergeCompatible == 1
MergeHeaderMismatch == 2
MergeCommitmentRewrite == 3
MergeRevealRewrite == 4
MergeLateRevealRewrite == 5
MergePenaltyHeightRewrite == 6
MergeOffenderOverlap == 7
MergeUnfinalizedOffenders == 8
MergeFinalizedSticky == 9
MergeHeightMax == 10
MergePreserveExistingObservation == 11
MergeAddIncomingObservation == 12
MergePenaltyMarkerSticky == 13
MergeElectionRewrite == 14
MergeElectionSticky == 15
StageEqualCommittedNoPending == 16
StageCommittedExtension == 17
StageBadSnapshotKeepsCompatiblePending == 18
StageBadPendingReplacedByGoodSnapshot == 19
StageBadPendingDroppedWhenSnapshotBad == 20
StageNoCommittedConflictDropsStaged == 21
CommittedEffectCoveredPending == 22
CommittedEffectCompatibleProgress == 23
CommittedEffectConflictDropsPending == 24
ActivationEmptyElection == 25
ActivationBeforeMargin == 26
ActivationAtMargin == 27
EffectPenaltyHeightWithoutMarker == 28
EffectDuplicateParticipants == 29
EffectDuplicateOffenders == 30
EffectOffenderOutOfRoster == 31

Candidates == 1..31

NoBug == 0
AcceptHeaderMismatchBug == 1
AcceptCommitmentRewriteBug == 2
AcceptRevealRewriteBug == 3
AcceptLateRevealRewriteBug == 4
AcceptPenaltyHeightRewriteBug == 5
AllowOffenderOverlapBug == 6
KeepUnfinalizedOffendersBug == 7
LoseFinalizedStateBug == 8
LowerUpdateHeightBug == 9
DropExistingObservationBug == 10
SkipIncomingObservationBug == 11
DropPenaltyMarkerBug == 12
AllowElectionRewriteBug == 13
DropElectionBug == 14
KeepEqualCommittedPendingBug == 15
DropCommittedExtensionBug == 16
DamageCompatiblePendingBug == 17
KeepBadPendingBug == 18
ReplaceWithBadSnapshotBug == 19
InsertConflictWithoutCommittedBug == 20
CommittedEffectKeepsCoveredBug == 21
CommittedEffectDropsProgressBug == 22
CommittedEffectKeepsConflictBug == 23
ActivationEmptyInstallsBug == 24
ActivationBeforeMarginAppliesBug == 25
ActivationAtMarginDefersBug == 26
AcceptPenaltyHeightWithoutMarkerBug == 27
AcceptDuplicateParticipantsBug == 28
AcceptDuplicateOffendersBug == 29
AcceptOffenderOutOfRosterBug == 30

Bugs == 0..30

BugAcceptHeaderMismatch == Bug = AcceptHeaderMismatchBug
BugAcceptCommitmentRewrite == Bug = AcceptCommitmentRewriteBug
BugAcceptRevealRewrite == Bug = AcceptRevealRewriteBug
BugAcceptLateRevealRewrite == Bug = AcceptLateRevealRewriteBug
BugAcceptPenaltyHeightRewrite == Bug = AcceptPenaltyHeightRewriteBug
BugAllowOffenderOverlap == Bug = AllowOffenderOverlapBug
BugKeepUnfinalizedOffenders == Bug = KeepUnfinalizedOffendersBug
BugLoseFinalizedState == Bug = LoseFinalizedStateBug
BugLowerUpdateHeight == Bug = LowerUpdateHeightBug
BugDropExistingObservation == Bug = DropExistingObservationBug
BugSkipIncomingObservation == Bug = SkipIncomingObservationBug
BugDropPenaltyMarker == Bug = DropPenaltyMarkerBug
BugAllowElectionRewrite == Bug = AllowElectionRewriteBug
BugDropElection == Bug = DropElectionBug
BugKeepEqualCommittedPending == Bug = KeepEqualCommittedPendingBug
BugDropCommittedExtension == Bug = DropCommittedExtensionBug
BugDamageCompatiblePending == Bug = DamageCompatiblePendingBug
BugKeepBadPending == Bug = KeepBadPendingBug
BugReplaceWithBadSnapshot == Bug = ReplaceWithBadSnapshotBug
BugInsertConflictWithoutCommitted == Bug = InsertConflictWithoutCommittedBug
BugCommittedEffectKeepsCovered == Bug = CommittedEffectKeepsCoveredBug
BugCommittedEffectDropsProgress == Bug = CommittedEffectDropsProgressBug
BugCommittedEffectKeepsConflict == Bug = CommittedEffectKeepsConflictBug
BugActivationEmptyInstalls == Bug = ActivationEmptyInstallsBug
BugActivationBeforeMarginApplies == Bug = ActivationBeforeMarginAppliesBug
BugActivationAtMarginDefers == Bug = ActivationAtMarginDefersBug
BugAcceptPenaltyHeightWithoutMarker == Bug = AcceptPenaltyHeightWithoutMarkerBug
BugAcceptDuplicateParticipants == Bug = AcceptDuplicateParticipantsBug
BugAcceptDuplicateOffenders == Bug = AcceptDuplicateOffendersBug
BugAcceptOffenderOutOfRoster == Bug = AcceptOffenderOutOfRosterBug

MergeAcceptCases == {
  MergeCompatible,
  MergeUnfinalizedOffenders,
  MergeFinalizedSticky,
  MergeHeightMax,
  MergePreserveExistingObservation,
  MergeAddIncomingObservation,
  MergePenaltyMarkerSticky,
  MergeElectionSticky
}

MergeRejectCases == {
  MergeHeaderMismatch,
  MergeCommitmentRewrite,
  MergeRevealRewrite,
  MergeLateRevealRewrite,
  MergePenaltyHeightRewrite,
  MergeOffenderOverlap,
  MergeElectionRewrite
}

MergeCases == MergeAcceptCases \cup MergeRejectCases

SpecMergeAccepts(candidate) ==
  candidate \in MergeAcceptCases

ImplementationMergeAccepts(candidate) ==
  CASE candidate = MergeHeaderMismatch -> BugAcceptHeaderMismatch
    [] candidate = MergeCommitmentRewrite -> BugAcceptCommitmentRewrite
    [] candidate = MergeRevealRewrite -> BugAcceptRevealRewrite
    [] candidate = MergeLateRevealRewrite -> BugAcceptLateRevealRewrite
    [] candidate = MergePenaltyHeightRewrite -> BugAcceptPenaltyHeightRewrite
    [] candidate = MergeOffenderOverlap -> BugAllowOffenderOverlap
    [] candidate = MergeElectionRewrite -> BugAllowElectionRewrite
    [] OTHER -> SpecMergeAccepts(candidate)

ImplementationKeepsUnfinalizedOffenders(candidate) ==
  /\ candidate = MergeUnfinalizedOffenders
  /\ BugKeepUnfinalizedOffenders

ImplementationFinalized(candidate) ==
  CASE candidate = MergeFinalizedSticky -> ~BugLoseFinalizedState
    [] OTHER -> FALSE

ImplementationUsesMaxUpdatedHeight(candidate) ==
  /\ candidate = MergeHeightMax
  /\ ~BugLowerUpdateHeight

ImplementationPreservesExistingObservation(candidate) ==
  /\ candidate = MergePreserveExistingObservation
  /\ ~BugDropExistingObservation

ImplementationAddsIncomingObservation(candidate) ==
  /\ candidate = MergeAddIncomingObservation
  /\ ~BugSkipIncomingObservation

ImplementationPenaltyMarkerSticky(candidate) ==
  /\ candidate = MergePenaltyMarkerSticky
  /\ ~BugDropPenaltyMarker

ImplementationElectionSticky(candidate) ==
  /\ candidate = MergeElectionSticky
  /\ ~BugDropElection

NoPending == 0
StageMerged == 1
KeepPending == 2
ReplacePending == 3
DropPending == 4

StageCases == {
  StageEqualCommittedNoPending,
  StageCommittedExtension,
  StageBadSnapshotKeepsCompatiblePending,
  StageBadPendingReplacedByGoodSnapshot,
  StageBadPendingDroppedWhenSnapshotBad,
  StageNoCommittedConflictDropsStaged
}

SpecStageAction(candidate) ==
  CASE candidate = StageEqualCommittedNoPending -> NoPending
    [] candidate = StageCommittedExtension -> StageMerged
    [] candidate = StageBadSnapshotKeepsCompatiblePending -> KeepPending
    [] candidate = StageBadPendingReplacedByGoodSnapshot -> ReplacePending
    [] candidate = StageBadPendingDroppedWhenSnapshotBad -> DropPending
    [] candidate = StageNoCommittedConflictDropsStaged -> KeepPending
    [] OTHER -> NoPending

ImplementationStageAction(candidate) ==
  CASE candidate = StageEqualCommittedNoPending /\ BugKeepEqualCommittedPending ->
      StageMerged
    [] candidate = StageCommittedExtension /\ BugDropCommittedExtension ->
      NoPending
    [] candidate = StageBadSnapshotKeepsCompatiblePending /\ BugDamageCompatiblePending ->
      ReplacePending
    [] candidate = StageBadPendingReplacedByGoodSnapshot /\ BugKeepBadPending ->
      KeepPending
    [] candidate = StageBadPendingDroppedWhenSnapshotBad /\ BugReplaceWithBadSnapshot ->
      ReplacePending
    [] candidate = StageNoCommittedConflictDropsStaged /\
          BugInsertConflictWithoutCommitted -> ReplacePending
    [] OTHER -> SpecStageAction(candidate)

RemovePending == 0
RetainMerged == 1

CommittedEffectCases == {
  CommittedEffectCoveredPending,
  CommittedEffectCompatibleProgress,
  CommittedEffectConflictDropsPending
}

SpecCommittedEffectAction(candidate) ==
  CASE candidate = CommittedEffectCompatibleProgress -> RetainMerged
    [] candidate \in {CommittedEffectCoveredPending,
                      CommittedEffectConflictDropsPending} -> RemovePending
    [] OTHER -> RemovePending

ImplementationCommittedEffectAction(candidate) ==
  CASE candidate = CommittedEffectCoveredPending /\ BugCommittedEffectKeepsCovered ->
      RetainMerged
    [] candidate = CommittedEffectCompatibleProgress /\
          BugCommittedEffectDropsProgress -> RemovePending
    [] candidate = CommittedEffectConflictDropsPending /\
          BugCommittedEffectKeepsConflict -> RetainMerged
    [] OTHER -> SpecCommittedEffectAction(candidate)

NoPlan == 0
DeferActivation == 1
ApplyActivation == 2

ActivationCases == {
  ActivationEmptyElection,
  ActivationBeforeMargin,
  ActivationAtMargin
}

SpecActivationAction(candidate) ==
  CASE candidate = ActivationEmptyElection -> NoPlan
    [] candidate = ActivationBeforeMargin -> DeferActivation
    [] candidate = ActivationAtMargin -> ApplyActivation
    [] OTHER -> NoPlan

ImplementationActivationAction(candidate) ==
  CASE candidate = ActivationEmptyElection /\ BugActivationEmptyInstalls ->
      ApplyActivation
    [] candidate = ActivationBeforeMargin /\ BugActivationBeforeMarginApplies ->
      ApplyActivation
    [] candidate = ActivationAtMargin /\ BugActivationAtMarginDefers ->
      DeferActivation
    [] OTHER -> SpecActivationAction(candidate)

EffectValidationCases == {
  EffectPenaltyHeightWithoutMarker,
  EffectDuplicateParticipants,
  EffectDuplicateOffenders,
  EffectOffenderOutOfRoster
}

SpecEffectAccepts(candidate) == FALSE

ImplementationEffectAccepts(candidate) ==
  CASE candidate = EffectPenaltyHeightWithoutMarker ->
      BugAcceptPenaltyHeightWithoutMarker
    [] candidate = EffectDuplicateParticipants -> BugAcceptDuplicateParticipants
    [] candidate = EffectDuplicateOffenders -> BugAcceptDuplicateOffenders
    [] candidate = EffectOffenderOutOfRoster -> BugAcceptOffenderOutOfRoster
    [] OTHER -> SpecEffectAccepts(candidate)

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Candidates \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Candidates: TryCandidate(candidate)
  \/ Stable

MergeAdmissionMatchesSpec ==
  \A candidate \in tried:
    candidate \in MergeCases =>
      ImplementationMergeAccepts(candidate) <=> SpecMergeAccepts(candidate)

MergePreservesMonotonicFields ==
  /\ MergeUnfinalizedOffenders \in tried =>
       ~ImplementationKeepsUnfinalizedOffenders(MergeUnfinalizedOffenders)
  /\ MergeFinalizedSticky \in tried =>
       ImplementationFinalized(MergeFinalizedSticky)
  /\ MergeHeightMax \in tried =>
       ImplementationUsesMaxUpdatedHeight(MergeHeightMax)
  /\ MergePreserveExistingObservation \in tried =>
       ImplementationPreservesExistingObservation(MergePreserveExistingObservation)
  /\ MergeAddIncomingObservation \in tried =>
       ImplementationAddsIncomingObservation(MergeAddIncomingObservation)
  /\ MergePenaltyMarkerSticky \in tried =>
       ImplementationPenaltyMarkerSticky(MergePenaltyMarkerSticky)
  /\ MergeElectionSticky \in tried =>
       ImplementationElectionSticky(MergeElectionSticky)

StageMatchesSpec ==
  \A candidate \in tried:
    candidate \in StageCases =>
      ImplementationStageAction(candidate) = SpecStageAction(candidate)

CommittedEffectsMatchSpec ==
  \A candidate \in tried:
    candidate \in CommittedEffectCases =>
      ImplementationCommittedEffectAction(candidate) =
        SpecCommittedEffectAction(candidate)

ActivationMatchesSpec ==
  \A candidate \in tried:
    candidate \in ActivationCases =>
      ImplementationActivationAction(candidate) = SpecActivationAction(candidate)

EffectValidationFailsClosed ==
  \A candidate \in tried:
    candidate \in EffectValidationCases =>
      ImplementationEffectAccepts(candidate) = SpecEffectAccepts(candidate)

NposVrfEpochSealGroupedCases ==
  MergeCases \cup
  StageCases \cup
  CommittedEffectCases \cup
  ActivationCases \cup
  EffectValidationCases

NposVrfEpochSealCaseGroupsComplete ==
  NposVrfEpochSealGroupedCases = Candidates

NposVrfEpochSealMergeExactness ==
  /\ MergeAdmissionMatchesSpec
  /\ MergePreservesMonotonicFields

NposVrfEpochSealStageExactness ==
  /\ StageMatchesSpec

NposVrfEpochSealCommittedEffectExactness ==
  /\ CommittedEffectsMatchSpec

NposVrfEpochSealActivationExactness ==
  /\ ActivationMatchesSpec

NposVrfEpochSealEffectValidationExactness ==
  /\ EffectValidationFailsClosed

NposVrfEpochSealExactness ==
  /\ NposVrfEpochSealCaseGroupsComplete
  /\ MergeAdmissionMatchesSpec
  /\ MergePreservesMonotonicFields
  /\ StageMatchesSpec
  /\ CommittedEffectsMatchSpec
  /\ ActivationMatchesSpec
  /\ EffectValidationFailsClosed

NposVrfEpochSealCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ NposVrfEpochSealExactness

Safety ==
  NposVrfEpochSealExactness

====
