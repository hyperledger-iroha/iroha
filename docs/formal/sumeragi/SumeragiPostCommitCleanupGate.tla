---- MODULE SumeragiPostCommitCleanupGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for Sumeragi post-commit cleanup and stale-evidence
pruning.

This slice models the cleanup contracts that run after the durable committed
head advances through the Sumeragi commit path. The model abstracts concrete
hashes, heights, transaction batches, QC bodies, and RBC transcripts into
representative boundary cases while preserving the key safety obligations:
undelivered DA-backed RBC material for the committed block is retained, settled
or invalid RBC runtime state is drained without purging retained summaries,
only descendants that extend the committed tip stay pending, divergent or
unknown descendants are requeued, already committed duplicates are dropped
without requeue, stale validation/RBC/proposal/QC/missing/vote state is pruned,
payload-available missing-block clears require local payload knowledge except
for obsolete clears, active vote windows are preserved, and canonical frontier
evidence is not discarded when committed-edge conflict cleanup realigns state.
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

CommittedRbcUndeliveredDaRetained == 1
CommittedRbcSettledDrained == 2
CommittedRbcInvalidDrained == 3
CommittedRbcNoDaDrained == 4
DescendantExtendsTipKept == 5
DescendantDivergesRequeued == 6
DescendantUnknownParentRequeued == 7
CommittedDuplicateDroppedNoRequeue == 8
CommittedKuraDuplicateDroppedNoRequeue == 9
StalePendingAtOrBelowDropped == 10
StalePendingValidationCleared == 11
StalePendingRbcCleaned == 12
QcCacheKeepsCommittedHash == 13
QcCacheDropsStaleConflict == 14
ProposalHintsDropButSeenKept == 15
ProposalCachePruneCommitted == 16
MissingCommittedPayloadCleared == 17
MissingStaleObsoleteCleared == 18
MissingUncommittedPayloadClearDenied == 19
MissingObsoleteClearAllowedWithoutPayload == 20
VoteCacheDropsCommittedHeight == 21
VoteCachePreservesLocalActive == 22
VoteCachePreservesActivePending == 23
VoteCachePreservesNewViewWindow == 24
VoteCacheDropsAncientNewView == 25
SlotTrackerPrunesCommitted == 26
ForcedViewPrunedAtCommitted == 27
OnCommitClearsRecoveryForHeight == 28
CommittedEdgeKeepsCanonicalFrontierEvidence == 29
CommittedEdgeClearsNoEvidenceFrontier == 30
ValidationWithoutPendingPruned == 31

Candidates == 1..31

NoBug == 0
DropUndeliveredDaRbcBug == 1
PurgeSettledRbcSummaryBug == 2
RetainInvalidRbcBug == 3
RetainWithoutDaBug == 4
DropExtendingDescendantBug == 5
SkipDivergentRequeueBug == 6
KeepUnknownParentBug == 7
RequeueCommittedDuplicateBug == 8
RequeueKuraDuplicateBug == 9
KeepStalePendingBug == 10
KeepStaleValidationBug == 11
KeepStaleRbcBug == 12
DropCommittedQcBug == 13
KeepConflictingQcBug == 14
DropProposalsSeenBug == 15
KeepCommittedProposalCacheBug == 16
SkipCommittedMissingClearBug == 17
SkipObsoleteMissingClearBug == 18
ClearUnavailableNonobsoleteBug == 19
BlockObsoleteWithoutPayloadBug == 20
KeepCommittedVoteBug == 21
DropLocalActiveVoteBug == 22
DropActivePendingVoteBug == 23
DropNewViewWindowBug == 24
KeepAncientNewViewBug == 25
KeepCommittedSlotBug == 26
KeepForcedViewBug == 27
SkipCommitRecoveryClearBug == 28
DropCanonicalFrontierEvidenceBug == 29
KeepNoEvidenceFrontierBug == 30
KeepValidationWithoutPendingBug == 31

Bugs == 0..31

RetainRbcSession == 1
DrainRbcRuntime == 2
PreserveRbcSummary == 3
DropPending == 4
KeepPending == 5
RequeueTx == 6
NoRequeueTx == 7
ClearValidation == 8
CleanRbc == 9
KeepCommittedQc == 10
KeepQc == 11
DropQc == 12
DropHint == 13
DropProposal == 14
KeepProposalSeen == 15
PruneProposalCache == 16
ClearMissingPayloadAvailable == 17
ClearMissingObsolete == 18
DenyMissingClear == 19
RemovePendingFetch == 20
RecordRecoverySuccess == 21
DropVote == 22
KeepLocalVote == 23
KeepPendingVote == 24
KeepNewViewVote == 25
DropNewViewVote == 26
PruneSlotTracker == 27
PruneForcedView == 28
ClearRecovery == 29
ClearSidecarMismatch == 30
PreserveFrontierEvidence == 31
PruneFrontierState == 32
ClearCooldown == 33
PruneValidationInflight == 34
KeepVote == 35

Actions == 1..35

SpecActions(candidate) ==
  CASE candidate = CommittedRbcUndeliveredDaRetained ->
      {RetainRbcSession, PreserveRbcSummary}
    [] candidate = CommittedRbcSettledDrained ->
      {DrainRbcRuntime, PreserveRbcSummary}
    [] candidate = CommittedRbcInvalidDrained -> {DrainRbcRuntime}
    [] candidate = CommittedRbcNoDaDrained -> {DrainRbcRuntime}
    [] candidate = DescendantExtendsTipKept -> {KeepPending}
    [] candidate = DescendantDivergesRequeued ->
      {DropPending, RequeueTx, CleanRbc, ClearValidation}
    [] candidate = DescendantUnknownParentRequeued ->
      {DropPending, RequeueTx, CleanRbc, ClearValidation}
    [] candidate = CommittedDuplicateDroppedNoRequeue ->
      {DropPending, NoRequeueTx, CleanRbc, ClearValidation}
    [] candidate = CommittedKuraDuplicateDroppedNoRequeue ->
      {DropPending, NoRequeueTx, CleanRbc, ClearValidation}
    [] candidate = StalePendingAtOrBelowDropped -> {DropPending}
    [] candidate = StalePendingValidationCleared -> {ClearValidation}
    [] candidate = StalePendingRbcCleaned -> {CleanRbc}
    [] candidate = QcCacheKeepsCommittedHash -> {KeepCommittedQc}
    [] candidate = QcCacheDropsStaleConflict -> {DropQc}
    [] candidate = ProposalHintsDropButSeenKept ->
      {DropHint, DropProposal, KeepProposalSeen}
    [] candidate = ProposalCachePruneCommitted -> {PruneProposalCache}
    [] candidate = MissingCommittedPayloadCleared ->
      {ClearMissingPayloadAvailable, RemovePendingFetch,
       RecordRecoverySuccess, ClearSidecarMismatch}
    [] candidate = MissingStaleObsoleteCleared ->
      {ClearMissingObsolete, RemovePendingFetch}
    [] candidate = MissingUncommittedPayloadClearDenied -> {DenyMissingClear}
    [] candidate = MissingObsoleteClearAllowedWithoutPayload ->
      {ClearMissingObsolete, RemovePendingFetch}
    [] candidate = VoteCacheDropsCommittedHeight -> {DropVote}
    [] candidate = VoteCachePreservesLocalActive -> {KeepLocalVote}
    [] candidate = VoteCachePreservesActivePending -> {KeepPendingVote}
    [] candidate = VoteCachePreservesNewViewWindow -> {KeepNewViewVote}
    [] candidate = VoteCacheDropsAncientNewView -> {DropNewViewVote}
    [] candidate = SlotTrackerPrunesCommitted -> {PruneSlotTracker}
    [] candidate = ForcedViewPrunedAtCommitted -> {PruneForcedView}
    [] candidate = OnCommitClearsRecoveryForHeight ->
      {ClearRecovery, ClearSidecarMismatch}
    [] candidate = CommittedEdgeKeepsCanonicalFrontierEvidence ->
      {PreserveFrontierEvidence}
    [] candidate = CommittedEdgeClearsNoEvidenceFrontier ->
      {PruneFrontierState, ClearRecovery, ClearCooldown}
    [] candidate = ValidationWithoutPendingPruned -> {PruneValidationInflight}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = CommittedRbcUndeliveredDaRetained /\
          Bug = DropUndeliveredDaRbcBug ->
      (spec \ {RetainRbcSession}) \cup {DrainRbcRuntime}
    [] candidate = CommittedRbcSettledDrained /\
          Bug = PurgeSettledRbcSummaryBug ->
      spec \ {PreserveRbcSummary}
    [] candidate = CommittedRbcInvalidDrained /\ Bug = RetainInvalidRbcBug ->
      spec \cup {RetainRbcSession, PreserveRbcSummary}
    [] candidate = CommittedRbcNoDaDrained /\ Bug = RetainWithoutDaBug ->
      spec \cup {RetainRbcSession}
    [] candidate = DescendantExtendsTipKept /\
          Bug = DropExtendingDescendantBug ->
      (spec \ {KeepPending}) \cup {DropPending, RequeueTx}
    [] candidate = DescendantDivergesRequeued /\
          Bug = SkipDivergentRequeueBug ->
      spec \ {RequeueTx}
    [] candidate = DescendantUnknownParentRequeued /\
          Bug = KeepUnknownParentBug ->
      (spec \ {DropPending, RequeueTx}) \cup {KeepPending}
    [] candidate = CommittedDuplicateDroppedNoRequeue /\
          Bug = RequeueCommittedDuplicateBug ->
      (spec \ {NoRequeueTx}) \cup {RequeueTx}
    [] candidate = CommittedKuraDuplicateDroppedNoRequeue /\
          Bug = RequeueKuraDuplicateBug ->
      (spec \ {NoRequeueTx}) \cup {RequeueTx}
    [] candidate = StalePendingAtOrBelowDropped /\
          Bug = KeepStalePendingBug ->
      (spec \ {DropPending}) \cup {KeepPending}
    [] candidate = StalePendingValidationCleared /\
          Bug = KeepStaleValidationBug ->
      spec \ {ClearValidation}
    [] candidate = StalePendingRbcCleaned /\ Bug = KeepStaleRbcBug ->
      spec \ {CleanRbc}
    [] candidate = QcCacheKeepsCommittedHash /\ Bug = DropCommittedQcBug ->
      (spec \ {KeepCommittedQc}) \cup {DropQc}
    [] candidate = QcCacheDropsStaleConflict /\ Bug = KeepConflictingQcBug ->
      (spec \ {DropQc}) \cup {KeepQc}
    [] candidate = ProposalHintsDropButSeenKept /\
          Bug = DropProposalsSeenBug ->
      spec \ {KeepProposalSeen}
    [] candidate = ProposalCachePruneCommitted /\
          Bug = KeepCommittedProposalCacheBug ->
      spec \ {PruneProposalCache}
    [] candidate = MissingCommittedPayloadCleared /\
          Bug = SkipCommittedMissingClearBug ->
      spec \ {ClearMissingPayloadAvailable}
    [] candidate = MissingStaleObsoleteCleared /\
          Bug = SkipObsoleteMissingClearBug ->
      spec \ {ClearMissingObsolete}
    [] candidate = MissingUncommittedPayloadClearDenied /\
          Bug = ClearUnavailableNonobsoleteBug ->
      (spec \ {DenyMissingClear}) \cup {ClearMissingPayloadAvailable}
    [] candidate = MissingObsoleteClearAllowedWithoutPayload /\
          Bug = BlockObsoleteWithoutPayloadBug ->
      (spec \ {ClearMissingObsolete, RemovePendingFetch}) \cup
        {DenyMissingClear}
    [] candidate = VoteCacheDropsCommittedHeight /\
          Bug = KeepCommittedVoteBug ->
      (spec \ {DropVote}) \cup {KeepVote}
    [] candidate = VoteCachePreservesLocalActive /\
          Bug = DropLocalActiveVoteBug ->
      spec \ {KeepLocalVote}
    [] candidate = VoteCachePreservesActivePending /\
          Bug = DropActivePendingVoteBug ->
      spec \ {KeepPendingVote}
    [] candidate = VoteCachePreservesNewViewWindow /\
          Bug = DropNewViewWindowBug ->
      spec \ {KeepNewViewVote}
    [] candidate = VoteCacheDropsAncientNewView /\
          Bug = KeepAncientNewViewBug ->
      (spec \ {DropNewViewVote}) \cup {KeepNewViewVote}
    [] candidate = SlotTrackerPrunesCommitted /\ Bug = KeepCommittedSlotBug ->
      spec \ {PruneSlotTracker}
    [] candidate = ForcedViewPrunedAtCommitted /\ Bug = KeepForcedViewBug ->
      spec \ {PruneForcedView}
    [] candidate = OnCommitClearsRecoveryForHeight /\
          Bug = SkipCommitRecoveryClearBug ->
      spec \ {ClearRecovery, ClearSidecarMismatch}
    [] candidate = CommittedEdgeKeepsCanonicalFrontierEvidence /\
          Bug = DropCanonicalFrontierEvidenceBug ->
      spec \ {PreserveFrontierEvidence}
    [] candidate = CommittedEdgeClearsNoEvidenceFrontier /\
          Bug = KeepNoEvidenceFrontierBug ->
      (spec \ {PruneFrontierState, ClearRecovery, ClearCooldown}) \cup
        {PreserveFrontierEvidence}
    [] candidate = ValidationWithoutPendingPruned /\
          Bug = KeepValidationWithoutPendingBug ->
      spec \ {PruneValidationInflight}
    [] OTHER -> spec

Init ==
  tried = {}

Next ==
  \E candidate \in Candidates \ tried:
    tried' = tried \cup {candidate}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ tried \subseteq Candidates
  /\ \A candidate \in tried: ImplementationActions(candidate) \subseteq Actions

Safety ==
  \A candidate \in tried:
    ImplementationActions(candidate) = SpecActions(candidate)

====
