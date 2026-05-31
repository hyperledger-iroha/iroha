---- MODULE SumeragiMissingBlockHardCapCleanupGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for missing-block hard-cap cleanup preservation.

This slice models the cleanup boundary formed by
`apply_frontier_recovery_cleanup(...)`,
`should_preserve_hard_cap_live_contiguous_frontier_cleanup(...)`,
`has_quorum_backed_missing_payload_recovery_for_height(...)`,
`collect_live_frontier_rbc_keys_for_cleanup(...)`, and the same-height/future
prune helpers. Hard-cap cleanup must preserve live contiguous frontier material
and quorum-backed missing-payload repair while still pruning descendant state,
invalid same-height RBC sessions, and stale same-height state when no live
frontier evidence remains.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Set(Int);
  tried

\* @type: <<Set(Int)>>;
vars == <<tried>>

PreserveRequiresFrontier == 1
PreserveRequiresMaterial == 2
OwnerMaterialPreserves == 3
PendingTipMaterialPreserves == 4
InflightTipMaterialPreserves == 5
RbcSessionMaterialPreserves == 6
RbcPendingMaterialPreserves == 7
InvalidPendingNotMaterial == 8
NonTipPendingNotMaterial == 9
LiveCleanupSkipsSameHeightPrune == 10
LiveCleanupKeepsMetadata == 11
LiveCleanupKeepsRecoveryState == 12
LiveCleanupKeepsValidRbc == 13
LiveCleanupKeepsRbcPacing == 14
LiveCleanupPrunesInvalidRbc == 15
LiveCleanupPrunesFuturePending == 16
LiveCleanupPrunesFutureMissing == 17
LiveCleanupPrunesFutureRbc == 18
QuorumBackedPreservesMissingRequest == 19
QuorumBackedPreservesSameHeightRbc == 20
NoLiveCleanupPrunesSameHeightPending == 21
NoLiveCleanupClearsRecoveryState == 22
HardCapKeepsFrontierNewViewEvidence == 23
HardCapKeepsSameViewOwner == 24
QuorumTimeoutDropsStaleSameViewOwner == 25

Candidates == 1..25

NoBug == 0
PreserveNonFrontierBug == 1
PreserveWithoutMaterialBug == 2
IgnoreOwnerMaterialBug == 3
IgnorePendingMaterialBug == 4
IgnoreInflightMaterialBug == 5
IgnoreRbcSessionMaterialBug == 6
IgnoreRbcPendingMaterialBug == 7
TreatInvalidPendingLiveBug == 8
TreatNonTipPendingLiveBug == 9
PruneLiveSameHeightBug == 10
DropLiveMetadataBug == 11
ClearLiveRecoveryStateBug == 12
DropValidRbcBug == 13
DropRbcPacingBug == 14
KeepInvalidRbcBug == 15
KeepFuturePendingBug == 16
KeepFutureMissingBug == 17
KeepFutureRbcBug == 18
DropQuorumMissingRequestBug == 19
PurgeQuorumSameHeightRbcBug == 20
KeepNoLivePendingBug == 21
KeepNoLiveRecoveryStateBug == 22
DropFrontierNewViewEvidenceBug == 23
DropSameViewOwnerBug == 24
KeepQuorumTimeoutStaleOwnerBug == 25

Bugs == 0..25

BugPreserveNonFrontier == Bug = PreserveNonFrontierBug
BugPreserveWithoutMaterial == Bug = PreserveWithoutMaterialBug
BugIgnoreOwnerMaterial == Bug = IgnoreOwnerMaterialBug
BugIgnorePendingMaterial == Bug = IgnorePendingMaterialBug
BugIgnoreInflightMaterial == Bug = IgnoreInflightMaterialBug
BugIgnoreRbcSessionMaterial == Bug = IgnoreRbcSessionMaterialBug
BugIgnoreRbcPendingMaterial == Bug = IgnoreRbcPendingMaterialBug
BugTreatInvalidPendingLive == Bug = TreatInvalidPendingLiveBug
BugTreatNonTipPendingLive == Bug = TreatNonTipPendingLiveBug
BugPruneLiveSameHeight == Bug = PruneLiveSameHeightBug
BugDropLiveMetadata == Bug = DropLiveMetadataBug
BugClearLiveRecoveryState == Bug = ClearLiveRecoveryStateBug
BugDropValidRbc == Bug = DropValidRbcBug
BugDropRbcPacing == Bug = DropRbcPacingBug
BugKeepInvalidRbc == Bug = KeepInvalidRbcBug
BugKeepFuturePending == Bug = KeepFuturePendingBug
BugKeepFutureMissing == Bug = KeepFutureMissingBug
BugKeepFutureRbc == Bug = KeepFutureRbcBug
BugDropQuorumMissingRequest == Bug = DropQuorumMissingRequestBug
BugPurgeQuorumSameHeightRbc == Bug = PurgeQuorumSameHeightRbcBug
BugKeepNoLivePending == Bug = KeepNoLivePendingBug
BugKeepNoLiveRecoveryState == Bug = KeepNoLiveRecoveryStateBug
BugDropFrontierNewViewEvidence == Bug = DropFrontierNewViewEvidenceBug
BugDropSameViewOwner == Bug = DropSameViewOwnerBug
BugKeepQuorumTimeoutStaleOwner == Bug = KeepQuorumTimeoutStaleOwnerBug

PreserveDecisionCandidates == {
  PreserveRequiresFrontier,
  PreserveRequiresMaterial,
  OwnerMaterialPreserves,
  PendingTipMaterialPreserves,
  InflightTipMaterialPreserves,
  RbcSessionMaterialPreserves,
  RbcPendingMaterialPreserves,
  InvalidPendingNotMaterial,
  NonTipPendingNotMaterial
}

SpecPreservesLiveFrontier(candidate) ==
  candidate \in {
    OwnerMaterialPreserves,
    PendingTipMaterialPreserves,
    InflightTipMaterialPreserves,
    RbcSessionMaterialPreserves,
    RbcPendingMaterialPreserves
  }

ImplementationPreservesLiveFrontier(candidate) ==
  CASE candidate = PreserveRequiresFrontier -> BugPreserveNonFrontier
    [] candidate = PreserveRequiresMaterial -> BugPreserveWithoutMaterial
    [] candidate = OwnerMaterialPreserves -> ~BugIgnoreOwnerMaterial
    [] candidate = PendingTipMaterialPreserves -> ~BugIgnorePendingMaterial
    [] candidate = InflightTipMaterialPreserves -> ~BugIgnoreInflightMaterial
    [] candidate = RbcSessionMaterialPreserves -> ~BugIgnoreRbcSessionMaterial
    [] candidate = RbcPendingMaterialPreserves -> ~BugIgnoreRbcPendingMaterial
    [] candidate = InvalidPendingNotMaterial -> BugTreatInvalidPendingLive
    [] candidate = NonTipPendingNotMaterial -> BugTreatNonTipPendingLive
    [] OTHER -> FALSE

ImplementationSameHeightPruned(candidate) ==
  CASE candidate = LiveCleanupSkipsSameHeightPrune -> BugPruneLiveSameHeight
    [] candidate = NoLiveCleanupPrunesSameHeightPending -> ~BugKeepNoLivePending
    [] OTHER -> FALSE

ImplementationMetadataKept(candidate) ==
  /\ candidate = LiveCleanupKeepsMetadata
  /\ ~BugDropLiveMetadata

ImplementationRecoveryStateKept(candidate) ==
  /\ candidate = LiveCleanupKeepsRecoveryState
  /\ ~BugClearLiveRecoveryState

ImplementationValidRbcKept(candidate) ==
  /\ candidate = LiveCleanupKeepsValidRbc
  /\ ~BugDropValidRbc

ImplementationRbcPacingKept(candidate) ==
  /\ candidate = LiveCleanupKeepsRbcPacing
  /\ ~BugDropRbcPacing

ImplementationInvalidRbcPruned(candidate) ==
  /\ candidate = LiveCleanupPrunesInvalidRbc
  /\ ~BugKeepInvalidRbc

ImplementationFuturePendingPruned(candidate) ==
  /\ candidate = LiveCleanupPrunesFuturePending
  /\ ~BugKeepFuturePending

ImplementationFutureMissingPruned(candidate) ==
  /\ candidate = LiveCleanupPrunesFutureMissing
  /\ ~BugKeepFutureMissing

ImplementationFutureRbcPruned(candidate) ==
  /\ candidate = LiveCleanupPrunesFutureRbc
  /\ ~BugKeepFutureRbc

ImplementationQuorumMissingRequestKept(candidate) ==
  /\ candidate = QuorumBackedPreservesMissingRequest
  /\ ~BugDropQuorumMissingRequest

ImplementationQuorumSameHeightRbcKept(candidate) ==
  /\ candidate = QuorumBackedPreservesSameHeightRbc
  /\ ~BugPurgeQuorumSameHeightRbc

ImplementationNoLiveRecoveryCleared(candidate) ==
  /\ candidate = NoLiveCleanupClearsRecoveryState
  /\ ~BugKeepNoLiveRecoveryState

ImplementationFrontierNewViewEvidenceKept(candidate) ==
  /\ candidate = HardCapKeepsFrontierNewViewEvidence
  /\ ~BugDropFrontierNewViewEvidence

ImplementationSameViewOwnerKept(candidate) ==
  /\ candidate = HardCapKeepsSameViewOwner
  /\ ~BugDropSameViewOwner

ImplementationQuorumTimeoutStaleOwnerDropped(candidate) ==
  /\ candidate = QuorumTimeoutDropsStaleSameViewOwner
  /\ ~BugKeepQuorumTimeoutStaleOwner

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

HardCapPreservationDecisionMatchesSpec ==
  \A candidate \in tried:
    candidate \in PreserveDecisionCandidates =>
      ImplementationPreservesLiveFrontier(candidate) <=> SpecPreservesLiveFrontier(candidate)

LiveCleanupPreservesSameHeightState ==
  /\ LiveCleanupSkipsSameHeightPrune \in tried =>
       ~ImplementationSameHeightPruned(LiveCleanupSkipsSameHeightPrune)
  /\ LiveCleanupKeepsMetadata \in tried =>
       ImplementationMetadataKept(LiveCleanupKeepsMetadata)
  /\ LiveCleanupKeepsRecoveryState \in tried =>
       ImplementationRecoveryStateKept(LiveCleanupKeepsRecoveryState)
  /\ LiveCleanupKeepsValidRbc \in tried =>
       ImplementationValidRbcKept(LiveCleanupKeepsValidRbc)
  /\ LiveCleanupKeepsRbcPacing \in tried =>
       ImplementationRbcPacingKept(LiveCleanupKeepsRbcPacing)

CleanupStillPrunesDeadOrFutureState ==
  /\ LiveCleanupPrunesInvalidRbc \in tried =>
       ImplementationInvalidRbcPruned(LiveCleanupPrunesInvalidRbc)
  /\ LiveCleanupPrunesFuturePending \in tried =>
       ImplementationFuturePendingPruned(LiveCleanupPrunesFuturePending)
  /\ LiveCleanupPrunesFutureMissing \in tried =>
       ImplementationFutureMissingPruned(LiveCleanupPrunesFutureMissing)
  /\ LiveCleanupPrunesFutureRbc \in tried =>
       ImplementationFutureRbcPruned(LiveCleanupPrunesFutureRbc)
  /\ NoLiveCleanupPrunesSameHeightPending \in tried =>
       ImplementationSameHeightPruned(NoLiveCleanupPrunesSameHeightPending)
  /\ NoLiveCleanupClearsRecoveryState \in tried =>
       ImplementationNoLiveRecoveryCleared(NoLiveCleanupClearsRecoveryState)

QuorumBackedRepairSurvivesCleanup ==
  /\ QuorumBackedPreservesMissingRequest \in tried =>
       ImplementationQuorumMissingRequestKept(QuorumBackedPreservesMissingRequest)
  /\ QuorumBackedPreservesSameHeightRbc \in tried =>
       ImplementationQuorumSameHeightRbcKept(QuorumBackedPreservesSameHeightRbc)

FrontierEvidenceOwnershipRules ==
  /\ HardCapKeepsFrontierNewViewEvidence \in tried =>
       ImplementationFrontierNewViewEvidenceKept(HardCapKeepsFrontierNewViewEvidence)
  /\ HardCapKeepsSameViewOwner \in tried =>
       ImplementationSameViewOwnerKept(HardCapKeepsSameViewOwner)
  /\ QuorumTimeoutDropsStaleSameViewOwner \in tried =>
       ImplementationQuorumTimeoutStaleOwnerDropped(QuorumTimeoutDropsStaleSameViewOwner)

Safety ==
  /\ HardCapPreservationDecisionMatchesSpec
  /\ LiveCleanupPreservesSameHeightState
  /\ CleanupStillPrunesDeadOrFutureState
  /\ QuorumBackedRepairSurvivesCleanup
  /\ FrontierEvidenceOwnershipRules

====
