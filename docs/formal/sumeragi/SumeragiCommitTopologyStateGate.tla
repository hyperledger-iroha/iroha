---- MODULE SumeragiCommitTopologyStateGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for commit-topology state refresh and roster-change
reset handling.

This slice captures `refresh_commit_topology_state(...)` and
`reset_consensus_state_for_roster_change(...)`, plus the `on_block_commit(...)`
branch that invokes the reset only for membership changes. Commit topology
refresh must return `None` without mutating state for identical ordered
topologies, classify order-only changes by sorted membership hash, update the
stored order/membership hashes before returning `OrderOnly` or `Membership`,
clear NEW_VIEW/forced-view state only on membership changes, and preserve it on
leader-order rotations. Roster-change resets must clear runtime consensus,
validation, vote, recovery, QC, roster, block-sync, RBC, DA, rebroadcast, and
timing state, while preserving `proposals_seen` only when explicitly requested.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

RefreshSameOrderNoop == "refresh_same_order_noop"
RefreshSameOrderMembershipMissingNoop ==
  "refresh_same_order_membership_missing_noop"
RefreshFirstInstallMembership == "refresh_first_install_membership"
RefreshOrderOnlyUpdatesHashes == "refresh_order_only_updates_hashes"
RefreshOrderOnlyPreservesViewState ==
  "refresh_order_only_preserves_view_state"
RefreshMembershipUpdatesHashes == "refresh_membership_updates_hashes"
RefreshMembershipClearsViewState == "refresh_membership_clears_view_state"
ResetClearsPendingState == "reset_clears_pending_state"
ResetClearsValidationState == "reset_clears_validation_state"
ResetClearsVoteState == "reset_clears_vote_state"
ResetClearsDeferredRecoveryState == "reset_clears_deferred_recovery_state"
ResetClearsQcAndRosterCaches == "reset_clears_qc_and_roster_caches"
ResetClearsSignerAndVotingState == "reset_clears_signer_and_voting_state"
ResetPreservesProposalsSeenWhenRequested ==
  "reset_preserves_proposals_seen_when_requested"
ResetClearsProposalsSeenWhenNotRequested ==
  "reset_clears_proposals_seen_when_not_requested"
ResetClearsSlotAuthoritativeState == "reset_clears_slot_authoritative_state"
ResetRebuildsProposalAndCollectorState ==
  "reset_rebuilds_proposal_and_collector_state"
ResetClearsRbcRuntimeState == "reset_clears_rbc_runtime_state"
ResetClearsDaRuntimeState == "reset_clears_da_runtime_state"
ResetClearsRebroadcastAndWarningLogs ==
  "reset_clears_rebroadcast_and_warning_logs"
ResetRefreshesTickLagAndHotspotState ==
  "reset_refreshes_tick_lag_and_hotspot_state"
OnCommitMembershipResetsPreservingProposals ==
  "on_commit_membership_resets_preserving_proposals"
OnCommitOrderOnlyPreservesCaches == "on_commit_order_only_preserves_caches"
OnCommitNoneNoops == "on_commit_none_noops"

Cases == {
  RefreshSameOrderNoop,
  RefreshSameOrderMembershipMissingNoop,
  RefreshFirstInstallMembership,
  RefreshOrderOnlyUpdatesHashes,
  RefreshOrderOnlyPreservesViewState,
  RefreshMembershipUpdatesHashes,
  RefreshMembershipClearsViewState,
  ResetClearsPendingState,
  ResetClearsValidationState,
  ResetClearsVoteState,
  ResetClearsDeferredRecoveryState,
  ResetClearsQcAndRosterCaches,
  ResetClearsSignerAndVotingState,
  ResetPreservesProposalsSeenWhenRequested,
  ResetClearsProposalsSeenWhenNotRequested,
  ResetClearsSlotAuthoritativeState,
  ResetRebuildsProposalAndCollectorState,
  ResetClearsRbcRuntimeState,
  ResetClearsDaRuntimeState,
  ResetClearsRebroadcastAndWarningLogs,
  ResetRefreshesTickLagAndHotspotState,
  OnCommitMembershipResetsPreservingProposals,
  OnCommitOrderOnlyPreservesCaches,
  OnCommitNoneNoops
}

OrderHashComputed == 1
MembershipSorted == 2
MembershipHashComputed == 3
SameOrderGuard == 4
ReturnNone == 5
MembershipChangedCheck == 6
StoreOrderHash == 7
StoreMembershipHash == 8
ReturnOrderOnly == 9
ReturnMembership == 10
ClearNewViewTracker == 11
ClearForcedView == 12
PreserveNewViewTracker == 13
PreserveForcedView == 14
PendingStateCleared == 15
ValidationStateCleared == 16
VoteStateCleared == 17
DeferredRecoveryCleared == 18
QcCacheCleared == 19
VoteRosterCacheCleared == 20
BlockSignerCacheCleared == 21
VotingBlockCleared == 22
ProposalsSeenPreserved == 23
ProposalsSeenCleared == 24
SlotAuthoritativeCleared == 25
ProposalCacheRebuilt == 26
CollectorStateReset == 27
RbcRuntimeCleared == 28
DaRuntimeCleared == 29
RebroadcastLogsCleared == 30
WarningLogsCleared == 31
RecoveryWindowCleared == 32
TickLagProgressRefreshed == 33
TickLagWarningsCleared == 34
HotspotSummaryReset == 35
OnCommitRefresh == 36
ResetConsensusPreserveSeen == 37
NoReset == 38

SpecActions(c) ==
  CASE c = RefreshSameOrderNoop ->
      {OrderHashComputed, SameOrderGuard, ReturnNone}
    [] c = RefreshSameOrderMembershipMissingNoop ->
      {OrderHashComputed, SameOrderGuard, ReturnNone}
    [] c = RefreshFirstInstallMembership ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ClearNewViewTracker, ClearForcedView, ReturnMembership}
    [] c = RefreshOrderOnlyUpdatesHashes ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ReturnOrderOnly}
    [] c = RefreshOrderOnlyPreservesViewState ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       PreserveNewViewTracker, PreserveForcedView, ReturnOrderOnly}
    [] c = RefreshMembershipUpdatesHashes ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ClearNewViewTracker, ClearForcedView, ReturnMembership}
    [] c = RefreshMembershipClearsViewState ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ClearNewViewTracker, ClearForcedView, ReturnMembership}
    [] c = ResetClearsPendingState ->
      {PendingStateCleared}
    [] c = ResetClearsValidationState ->
      {ValidationStateCleared}
    [] c = ResetClearsVoteState ->
      {VoteStateCleared}
    [] c = ResetClearsDeferredRecoveryState ->
      {DeferredRecoveryCleared}
    [] c = ResetClearsQcAndRosterCaches ->
      {QcCacheCleared, VoteRosterCacheCleared}
    [] c = ResetClearsSignerAndVotingState ->
      {BlockSignerCacheCleared, VotingBlockCleared}
    [] c = ResetPreservesProposalsSeenWhenRequested ->
      {ProposalsSeenPreserved}
    [] c = ResetClearsProposalsSeenWhenNotRequested ->
      {ProposalsSeenCleared}
    [] c = ResetClearsSlotAuthoritativeState ->
      {SlotAuthoritativeCleared}
    [] c = ResetRebuildsProposalAndCollectorState ->
      {ProposalCacheRebuilt, CollectorStateReset}
    [] c = ResetClearsRbcRuntimeState ->
      {RbcRuntimeCleared}
    [] c = ResetClearsDaRuntimeState ->
      {DaRuntimeCleared}
    [] c = ResetClearsRebroadcastAndWarningLogs ->
      {RebroadcastLogsCleared, WarningLogsCleared, RecoveryWindowCleared}
    [] c = ResetRefreshesTickLagAndHotspotState ->
      {TickLagProgressRefreshed, TickLagWarningsCleared, HotspotSummaryReset}
    [] c = OnCommitMembershipResetsPreservingProposals ->
      {OnCommitRefresh, ReturnMembership, ResetConsensusPreserveSeen,
       PendingStateCleared, ValidationStateCleared, VoteStateCleared,
       DeferredRecoveryCleared, QcCacheCleared, VoteRosterCacheCleared,
       ProposalsSeenPreserved}
    [] c = OnCommitOrderOnlyPreservesCaches ->
      {OnCommitRefresh, ReturnOrderOnly, NoReset}
    [] c = OnCommitNoneNoops ->
      {OnCommitRefresh, ReturnNone, NoReset}
    [] OTHER -> {}

ActualActions(c) ==
  CASE Bug = "refresh_same_order_mutates"
       /\ c = RefreshSameOrderNoop ->
      {OrderHashComputed, SameOrderGuard, StoreOrderHash,
       StoreMembershipHash, ReturnOrderOnly}
    [] Bug = "refresh_same_order_backfills_membership"
       /\ c = RefreshSameOrderMembershipMissingNoop ->
      {OrderHashComputed, SameOrderGuard, MembershipSorted,
       MembershipHashComputed, StoreMembershipHash, ReturnNone}
    [] Bug = "refresh_first_install_order_only"
       /\ c = RefreshFirstInstallMembership ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ReturnOrderOnly}
    [] Bug = "refresh_order_only_membership"
       /\ c = RefreshOrderOnlyUpdatesHashes ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ClearNewViewTracker, ClearForcedView, ReturnMembership}
    [] Bug = "refresh_order_only_clears_view_state"
       /\ c = RefreshOrderOnlyPreservesViewState ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       ClearNewViewTracker, ClearForcedView, ReturnOrderOnly}
    [] Bug = "refresh_order_only_keeps_old_order_hash"
       /\ c = RefreshOrderOnlyUpdatesHashes ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreMembershipHash, ReturnOrderOnly}
    [] Bug = "refresh_membership_order_only"
       /\ c = RefreshMembershipClearsViewState ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       PreserveNewViewTracker, PreserveForcedView, ReturnOrderOnly}
    [] Bug = "refresh_membership_keeps_view_state"
       /\ c = RefreshMembershipClearsViewState ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, StoreOrderHash, StoreMembershipHash,
       PreserveNewViewTracker, PreserveForcedView, ReturnMembership}
    [] Bug = "refresh_membership_skips_hash_update"
       /\ c = RefreshMembershipUpdatesHashes ->
      {OrderHashComputed, MembershipSorted, MembershipHashComputed,
       MembershipChangedCheck, ClearNewViewTracker, ClearForcedView,
       ReturnMembership}
    [] Bug = "reset_keeps_pending_state"
       /\ c = ResetClearsPendingState ->
      {}
    [] Bug = "reset_keeps_validation_state"
       /\ c = ResetClearsValidationState ->
      {}
    [] Bug = "reset_keeps_vote_state"
       /\ c = ResetClearsVoteState ->
      {}
    [] Bug = "reset_keeps_deferred_recovery"
       /\ c = ResetClearsDeferredRecoveryState ->
      {}
    [] Bug = "reset_keeps_qc_cache"
       /\ c = ResetClearsQcAndRosterCaches ->
      {VoteRosterCacheCleared}
    [] Bug = "reset_keeps_vote_roster_cache"
       /\ c = ResetClearsQcAndRosterCaches ->
      {QcCacheCleared}
    [] Bug = "reset_keeps_block_signer_cache"
       /\ c = ResetClearsSignerAndVotingState ->
      {VotingBlockCleared}
    [] Bug = "reset_keeps_voting_block"
       /\ c = ResetClearsSignerAndVotingState ->
      {BlockSignerCacheCleared}
    [] Bug = "reset_drops_proposals_seen_when_preserved"
       /\ c = ResetPreservesProposalsSeenWhenRequested ->
      {ProposalsSeenCleared}
    [] Bug = "reset_preserves_proposals_seen_when_not_requested"
       /\ c = ResetClearsProposalsSeenWhenNotRequested ->
      {ProposalsSeenPreserved}
    [] Bug = "reset_keeps_slot_authoritative"
       /\ c = ResetClearsSlotAuthoritativeState ->
      {}
    [] Bug = "reset_keeps_proposal_cache"
       /\ c = ResetRebuildsProposalAndCollectorState ->
      {CollectorStateReset}
    [] Bug = "reset_keeps_collector_state"
       /\ c = ResetRebuildsProposalAndCollectorState ->
      {ProposalCacheRebuilt}
    [] Bug = "reset_keeps_rbc_runtime"
       /\ c = ResetClearsRbcRuntimeState ->
      {}
    [] Bug = "reset_keeps_da_runtime"
       /\ c = ResetClearsDaRuntimeState ->
      {}
    [] Bug = "reset_keeps_rebroadcast_logs"
       /\ c = ResetClearsRebroadcastAndWarningLogs ->
      {WarningLogsCleared, RecoveryWindowCleared}
    [] Bug = "reset_keeps_warning_logs"
       /\ c = ResetClearsRebroadcastAndWarningLogs ->
      {RebroadcastLogsCleared, RecoveryWindowCleared}
    [] Bug = "reset_stale_tick_progress"
       /\ c = ResetRefreshesTickLagAndHotspotState ->
      {TickLagWarningsCleared, HotspotSummaryReset}
    [] Bug = "on_commit_membership_no_reset"
       /\ c = OnCommitMembershipResetsPreservingProposals ->
      {OnCommitRefresh, ReturnMembership, NoReset}
    [] Bug = "on_commit_order_only_resets"
       /\ c = OnCommitOrderOnlyPreservesCaches ->
      {OnCommitRefresh, ReturnOrderOnly, ResetConsensusPreserveSeen,
       PendingStateCleared, VoteStateCleared}
    [] OTHER -> SpecActions(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

SafetyFast ==
  \A c \in Cases: ActualActions(c) = SpecActions(c)

BugRefreshSameOrderMutates ==
  ActualActions(RefreshSameOrderNoop) = SpecActions(RefreshSameOrderNoop)

BugRefreshSameOrderBackfillsMembership ==
  ActualActions(RefreshSameOrderMembershipMissingNoop) =
    SpecActions(RefreshSameOrderMembershipMissingNoop)

BugRefreshFirstInstallOrderOnly ==
  ActualActions(RefreshFirstInstallMembership) =
    SpecActions(RefreshFirstInstallMembership)

BugRefreshOrderOnlyMembership ==
  ActualActions(RefreshOrderOnlyUpdatesHashes) =
    SpecActions(RefreshOrderOnlyUpdatesHashes)

BugRefreshOrderOnlyClearsViewState ==
  ActualActions(RefreshOrderOnlyPreservesViewState) =
    SpecActions(RefreshOrderOnlyPreservesViewState)

BugRefreshOrderOnlyKeepsOldOrderHash ==
  ActualActions(RefreshOrderOnlyUpdatesHashes) =
    SpecActions(RefreshOrderOnlyUpdatesHashes)

BugRefreshMembershipOrderOnly ==
  ActualActions(RefreshMembershipClearsViewState) =
    SpecActions(RefreshMembershipClearsViewState)

BugRefreshMembershipKeepsViewState ==
  ActualActions(RefreshMembershipClearsViewState) =
    SpecActions(RefreshMembershipClearsViewState)

BugRefreshMembershipSkipsHashUpdate ==
  ActualActions(RefreshMembershipUpdatesHashes) =
    SpecActions(RefreshMembershipUpdatesHashes)

BugResetKeepsPendingState ==
  ActualActions(ResetClearsPendingState) = SpecActions(ResetClearsPendingState)

BugResetKeepsValidationState ==
  ActualActions(ResetClearsValidationState) =
    SpecActions(ResetClearsValidationState)

BugResetKeepsVoteState ==
  ActualActions(ResetClearsVoteState) = SpecActions(ResetClearsVoteState)

BugResetKeepsDeferredRecovery ==
  ActualActions(ResetClearsDeferredRecoveryState) =
    SpecActions(ResetClearsDeferredRecoveryState)

BugResetKeepsQcCache ==
  ActualActions(ResetClearsQcAndRosterCaches) =
    SpecActions(ResetClearsQcAndRosterCaches)

BugResetKeepsVoteRosterCache ==
  ActualActions(ResetClearsQcAndRosterCaches) =
    SpecActions(ResetClearsQcAndRosterCaches)

BugResetKeepsBlockSignerCache ==
  ActualActions(ResetClearsSignerAndVotingState) =
    SpecActions(ResetClearsSignerAndVotingState)

BugResetKeepsVotingBlock ==
  ActualActions(ResetClearsSignerAndVotingState) =
    SpecActions(ResetClearsSignerAndVotingState)

BugResetDropsProposalsSeenWhenPreserved ==
  ActualActions(ResetPreservesProposalsSeenWhenRequested) =
    SpecActions(ResetPreservesProposalsSeenWhenRequested)

BugResetPreservesProposalsSeenWhenNotRequested ==
  ActualActions(ResetClearsProposalsSeenWhenNotRequested) =
    SpecActions(ResetClearsProposalsSeenWhenNotRequested)

BugResetKeepsSlotAuthoritative ==
  ActualActions(ResetClearsSlotAuthoritativeState) =
    SpecActions(ResetClearsSlotAuthoritativeState)

BugResetKeepsProposalCache ==
  ActualActions(ResetRebuildsProposalAndCollectorState) =
    SpecActions(ResetRebuildsProposalAndCollectorState)

BugResetKeepsCollectorState ==
  ActualActions(ResetRebuildsProposalAndCollectorState) =
    SpecActions(ResetRebuildsProposalAndCollectorState)

BugResetKeepsRbcRuntime ==
  ActualActions(ResetClearsRbcRuntimeState) =
    SpecActions(ResetClearsRbcRuntimeState)

BugResetKeepsDaRuntime ==
  ActualActions(ResetClearsDaRuntimeState) =
    SpecActions(ResetClearsDaRuntimeState)

BugResetKeepsRebroadcastLogs ==
  ActualActions(ResetClearsRebroadcastAndWarningLogs) =
    SpecActions(ResetClearsRebroadcastAndWarningLogs)

BugResetKeepsWarningLogs ==
  ActualActions(ResetClearsRebroadcastAndWarningLogs) =
    SpecActions(ResetClearsRebroadcastAndWarningLogs)

BugResetStaleTickProgress ==
  ActualActions(ResetRefreshesTickLagAndHotspotState) =
    SpecActions(ResetRefreshesTickLagAndHotspotState)

BugOnCommitMembershipNoReset ==
  ActualActions(OnCommitMembershipResetsPreservingProposals) =
    SpecActions(OnCommitMembershipResetsPreservingProposals)

BugOnCommitOrderOnlyResets ==
  ActualActions(OnCommitOrderOnlyPreservesCaches) =
    SpecActions(OnCommitOrderOnlyPreservesCaches)

====
