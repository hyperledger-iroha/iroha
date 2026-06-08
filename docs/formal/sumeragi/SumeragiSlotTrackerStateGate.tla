---- MODULE SumeragiSlotTrackerStateGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `SlotTrackerState` helper semantics.

This slice models the map/set transformations in
`main_loop/slot_tracker.rs` and the direct proposal-seen helpers that mutate
the same state: authoritative slot-owner recording, authoritative frontier
metadata replacement, proposal-seen insert/read/pruning, retained-branch
refresh/seed priority, and height pruning. Concrete hashes, frontier payloads,
and instants are collapsed into representative cases while preserving the
observable contracts that keep same-height branch ownership deterministic.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ClearAllMaps == 1
ReadExistingOwner == 2
ReadAbsentOwner == 3
NoteOwnerInserts == 4
NoteOwnerReplacesOwner == 5
NoteOwnerKeepsMatchingFrontierInfo == 6
NoteOwnerDropsConflictingFrontierInfo == 7
NoteOwnerDropsMatchingRetainedBranch == 8
NoteOwnerKeepsConflictingRetainedBranch == 9
NoteFrontierInfoInserts == 10
NoteFrontierInfoReplacesSameHash == 11
NoteFrontierInfoDropsOtherHashSameSlot == 12
NoteFrontierInfoKeepsOtherSlot == 13
NoteRetainedNewBranch == 14
NoteRetainedExistingPreservesInfoWhenNone == 15
NoteRetainedExistingReplacesInfoWhenSome == 16
NoteRetainedPayloadOrs == 17
NoteRetainedRefreshesTimestamp == 18
RetainedSeedIgnoresMissingPayload == 19
RetainedSeedChoosesHighestView == 20
RetainedSeedChoosesLatestRefreshTie == 21
RetainedSeedAbsentHeightNone == 22
RemoveHeightRemovesExact == 23
RemoveHeightKeepsOther == 24
PruneAboveKeepsBelowAndEqual == 25
PruneAboveDropsAbove == 26
PruneCommittedDropsBelowAndEqual == 27
PruneCommittedKeepsAbove == 28
ProposalSeenInsertNew == 29
ProposalSeenDuplicatePreserves == 30
ProposalSeenReadExisting == 31
ProposalSeenReadAbsent == 32
ViewChangePruneDropsOldSameHeight == 33
ViewChangePruneKeepsNextSameHeight == 34
ViewChangePruneKeepsOtherHeight == 35
ProposalHorizonDropsCommitted == 36
ProposalHorizonDropsTooLowHeight == 37
ProposalHorizonDropsTooHighHeight == 38
ProposalHorizonDropsActiveOldView == 39
ProposalHorizonKeepsActiveCurrentView == 40
ProposalHorizonDropsActiveAboveCap == 41
ProposalHorizonKeepsOtherHeightHighView == 42

Candidates == 1..42

ClearOwners == 1
ClearFrontiers == 2
ClearProposals == 3
ClearRetained == 4
ReturnOwner == 5
ReturnNone == 6
InsertOwner == 7
ReplaceOwner == 8
KeepMatchingFrontierInfo == 9
DropConflictingFrontierInfo == 10
KeepOtherSlotFrontierInfo == 11
DropMatchingRetained == 12
KeepConflictingRetained == 13
KeepOtherSlotRetained == 14
InsertFrontierInfo == 15
ReplaceFrontierInfo == 16
DropOtherHashFrontierInfo == 17
InsertRetained == 18
PreserveExistingInfo == 19
ReplaceExistingInfo == 20
PayloadOr == 21
RefreshTimestamp == 22
SeedNone == 23
SeedHighestView == 24
SeedLatestRefresh == 25
IgnoreMissingPayload == 26
FilterSeedHeight == 27
RemoveExactHeight == 28
KeepOtherHeight == 29
KeepEqualHeight == 30
KeepBelowHeight == 31
DropAboveHeight == 32
DropEqualHeight == 33
DropBelowHeight == 34
KeepAboveHeight == 35
InsertProposalSeen == 36
PreserveProposalSeen == 37
ReturnProposalSeen == 38
ReturnProposalAbsent == 39
DropProposalSeen == 40
KeepProposalSeen == 41
DropOldSameHeightView == 42
KeepNextSameHeightView == 43
KeepOtherHeightProposalSeen == 44
DropCommittedProposalSeen == 45
DropTooLowProposalHeight == 46
DropTooHighProposalHeight == 47
DropActiveOldView == 48
KeepActiveCurrentView == 49
DropActiveAboveViewCap == 50
KeepOtherHeightHighView == 51

Actions == 1..51

SpecActions(candidate) ==
  CASE candidate = ClearAllMaps ->
      {ClearOwners, ClearFrontiers, ClearProposals, ClearRetained}
    [] candidate = ReadExistingOwner -> {ReturnOwner}
    [] candidate = ReadAbsentOwner -> {ReturnNone}
    [] candidate = NoteOwnerInserts ->
      {InsertOwner, KeepOtherSlotFrontierInfo, KeepOtherSlotRetained}
    [] candidate = NoteOwnerReplacesOwner ->
      {ReplaceOwner, KeepOtherSlotFrontierInfo, KeepOtherSlotRetained}
    [] candidate = NoteOwnerKeepsMatchingFrontierInfo ->
      {InsertOwner, KeepMatchingFrontierInfo}
    [] candidate = NoteOwnerDropsConflictingFrontierInfo ->
      {InsertOwner, DropConflictingFrontierInfo}
    [] candidate = NoteOwnerDropsMatchingRetainedBranch ->
      {InsertOwner, DropMatchingRetained}
    [] candidate = NoteOwnerKeepsConflictingRetainedBranch ->
      {InsertOwner, KeepConflictingRetained}
    [] candidate = NoteFrontierInfoInserts ->
      {InsertFrontierInfo, KeepOtherSlotFrontierInfo}
    [] candidate = NoteFrontierInfoReplacesSameHash ->
      {ReplaceFrontierInfo}
    [] candidate = NoteFrontierInfoDropsOtherHashSameSlot ->
      {InsertFrontierInfo, DropOtherHashFrontierInfo}
    [] candidate = NoteFrontierInfoKeepsOtherSlot ->
      {InsertFrontierInfo, KeepOtherSlotFrontierInfo}
    [] candidate = NoteRetainedNewBranch ->
      {InsertRetained, PreserveExistingInfo, PayloadOr, RefreshTimestamp}
    [] candidate = NoteRetainedExistingPreservesInfoWhenNone ->
      {PreserveExistingInfo, PayloadOr, RefreshTimestamp}
    [] candidate = NoteRetainedExistingReplacesInfoWhenSome ->
      {ReplaceExistingInfo, PayloadOr, RefreshTimestamp}
    [] candidate = NoteRetainedPayloadOrs ->
      {PreserveExistingInfo, PayloadOr, RefreshTimestamp}
    [] candidate = NoteRetainedRefreshesTimestamp ->
      {PreserveExistingInfo, PayloadOr, RefreshTimestamp}
    [] candidate = RetainedSeedIgnoresMissingPayload ->
      {IgnoreMissingPayload, FilterSeedHeight, SeedNone}
    [] candidate = RetainedSeedChoosesHighestView ->
      {IgnoreMissingPayload, FilterSeedHeight, SeedHighestView}
    [] candidate = RetainedSeedChoosesLatestRefreshTie ->
      {IgnoreMissingPayload, FilterSeedHeight, SeedLatestRefresh}
    [] candidate = RetainedSeedAbsentHeightNone ->
      {IgnoreMissingPayload, FilterSeedHeight, SeedNone}
    [] candidate = RemoveHeightRemovesExact ->
      {RemoveExactHeight, KeepOtherHeight}
    [] candidate = RemoveHeightKeepsOther ->
      {RemoveExactHeight, KeepOtherHeight}
    [] candidate = PruneAboveKeepsBelowAndEqual ->
      {KeepBelowHeight, KeepEqualHeight}
    [] candidate = PruneAboveDropsAbove ->
      {DropAboveHeight}
    [] candidate = PruneCommittedDropsBelowAndEqual ->
      {DropBelowHeight, DropEqualHeight}
    [] candidate = PruneCommittedKeepsAbove ->
      {KeepAboveHeight}
    [] candidate = ProposalSeenInsertNew -> {InsertProposalSeen}
    [] candidate = ProposalSeenDuplicatePreserves -> {PreserveProposalSeen}
    [] candidate = ProposalSeenReadExisting -> {ReturnProposalSeen}
    [] candidate = ProposalSeenReadAbsent -> {ReturnProposalAbsent}
    [] candidate = ViewChangePruneDropsOldSameHeight ->
      {DropOldSameHeightView}
    [] candidate = ViewChangePruneKeepsNextSameHeight ->
      {KeepNextSameHeightView}
    [] candidate = ViewChangePruneKeepsOtherHeight ->
      {KeepOtherHeightProposalSeen}
    [] candidate = ProposalHorizonDropsCommitted ->
      {DropCommittedProposalSeen}
    [] candidate = ProposalHorizonDropsTooLowHeight ->
      {DropTooLowProposalHeight}
    [] candidate = ProposalHorizonDropsTooHighHeight ->
      {DropTooHighProposalHeight}
    [] candidate = ProposalHorizonDropsActiveOldView ->
      {DropActiveOldView}
    [] candidate = ProposalHorizonKeepsActiveCurrentView ->
      {KeepActiveCurrentView}
    [] candidate = ProposalHorizonDropsActiveAboveCap ->
      {DropActiveAboveViewCap}
    [] candidate = ProposalHorizonKeepsOtherHeightHighView ->
      {KeepOtherHeightHighView}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ClearAllMaps /\ Bug = "clear_leaves_retained" ->
      spec \ {ClearRetained}
    [] candidate = ReadAbsentOwner /\ Bug = "owner_read_missing_returns_owner" ->
      (spec \ {ReturnNone}) \cup {ReturnOwner}
    [] candidate = NoteOwnerInserts /\ Bug = "owner_insert_missing" ->
      spec \ {InsertOwner}
    [] candidate = NoteOwnerReplacesOwner /\ Bug = "owner_replace_keeps_old" ->
      (spec \ {ReplaceOwner}) \cup {InsertOwner}
    [] candidate = NoteOwnerKeepsMatchingFrontierInfo /\
          Bug = "owner_drops_matching_frontier_info" ->
      (spec \ {KeepMatchingFrontierInfo}) \cup {DropConflictingFrontierInfo}
    [] candidate = NoteOwnerDropsConflictingFrontierInfo /\
          Bug = "owner_keeps_conflicting_frontier_info" ->
      (spec \ {DropConflictingFrontierInfo}) \cup {KeepMatchingFrontierInfo}
    [] candidate = NoteOwnerDropsMatchingRetainedBranch /\
          Bug = "owner_keeps_matching_retained" ->
      (spec \ {DropMatchingRetained}) \cup {KeepConflictingRetained}
    [] candidate = NoteOwnerKeepsConflictingRetainedBranch /\
          Bug = "owner_drops_conflicting_retained" ->
      (spec \ {KeepConflictingRetained}) \cup {DropMatchingRetained}
    [] candidate = NoteFrontierInfoDropsOtherHashSameSlot /\
          Bug = "frontier_info_keeps_other_hash" ->
      spec \ {DropOtherHashFrontierInfo}
    [] candidate = NoteFrontierInfoReplacesSameHash /\
          Bug = "frontier_info_drops_same_hash" ->
      (spec \ {ReplaceFrontierInfo}) \cup {DropOtherHashFrontierInfo}
    [] candidate = NoteFrontierInfoKeepsOtherSlot /\
          Bug = "frontier_info_drops_other_slot" ->
      (spec \ {KeepOtherSlotFrontierInfo}) \cup {DropOtherHashFrontierInfo}
    [] candidate = NoteRetainedNewBranch /\ Bug = "retained_new_missing_insert" ->
      spec \ {InsertRetained}
    [] candidate = NoteRetainedExistingPreservesInfoWhenNone /\
          Bug = "retained_none_drops_existing_info" ->
      (spec \ {PreserveExistingInfo}) \cup {ReplaceExistingInfo}
    [] candidate = NoteRetainedExistingReplacesInfoWhenSome /\
          Bug = "retained_some_preserves_old_info" ->
      (spec \ {ReplaceExistingInfo}) \cup {PreserveExistingInfo}
    [] candidate = NoteRetainedPayloadOrs /\ Bug = "retained_payload_not_or" ->
      spec \ {PayloadOr}
    [] candidate = NoteRetainedRefreshesTimestamp /\
          Bug = "retained_timestamp_not_refreshed" ->
      spec \ {RefreshTimestamp}
    [] candidate = RetainedSeedIgnoresMissingPayload /\
          Bug = "seed_uses_missing_payload" ->
      (spec \ {IgnoreMissingPayload, SeedNone}) \cup {SeedHighestView}
    [] candidate = RetainedSeedChoosesHighestView /\
          Bug = "seed_picks_lower_view" ->
      (spec \ {SeedHighestView}) \cup {SeedLatestRefresh}
    [] candidate = RetainedSeedChoosesLatestRefreshTie /\
          Bug = "seed_tie_picks_older" ->
      (spec \ {SeedLatestRefresh}) \cup {SeedHighestView}
    [] candidate = RetainedSeedAbsentHeightNone /\ Bug = "seed_ignores_height" ->
      (spec \ {FilterSeedHeight, SeedNone}) \cup {SeedHighestView}
    [] candidate = RemoveHeightRemovesExact /\ Bug = "remove_height_keeps_exact" ->
      (spec \ {RemoveExactHeight}) \cup {KeepEqualHeight}
    [] candidate = RemoveHeightKeepsOther /\ Bug = "remove_height_drops_other" ->
      (spec \ {KeepOtherHeight}) \cup {RemoveExactHeight}
    [] candidate = PruneAboveKeepsBelowAndEqual /\
          Bug = "prune_above_drops_equal" ->
      spec \ {KeepEqualHeight}
    [] candidate = PruneAboveDropsAbove /\ Bug = "prune_above_keeps_above" ->
      (spec \ {DropAboveHeight}) \cup {KeepAboveHeight}
    [] candidate = PruneCommittedDropsBelowAndEqual /\
          Bug = "prune_committed_keeps_equal" ->
      spec \ {DropEqualHeight}
    [] candidate = PruneCommittedKeepsAbove /\
          Bug = "prune_committed_drops_above" ->
      (spec \ {KeepAboveHeight}) \cup {DropAboveHeight}
    [] candidate = ProposalSeenInsertNew /\ Bug = "proposal_insert_missing" ->
      spec \ {InsertProposalSeen}
    [] candidate = ProposalSeenDuplicatePreserves /\
          Bug = "proposal_duplicate_drops_seen" ->
      (spec \ {PreserveProposalSeen}) \cup {DropProposalSeen}
    [] candidate = ProposalSeenReadAbsent /\
          Bug = "proposal_read_absent_returns_seen" ->
      (spec \ {ReturnProposalAbsent}) \cup {ReturnProposalSeen}
    [] candidate = ViewChangePruneDropsOldSameHeight /\
          Bug = "view_prune_keeps_old_view" ->
      (spec \ {DropOldSameHeightView}) \cup {KeepProposalSeen}
    [] candidate = ViewChangePruneKeepsNextSameHeight /\
          Bug = "view_prune_drops_next_view" ->
      (spec \ {KeepNextSameHeightView}) \cup {DropProposalSeen}
    [] candidate = ProposalHorizonDropsCommitted /\
          Bug = "horizon_keeps_committed_seen" ->
      (spec \ {DropCommittedProposalSeen}) \cup {KeepProposalSeen}
    [] candidate = ProposalHorizonDropsTooHighHeight /\
          Bug = "horizon_keeps_far_future_seen" ->
      (spec \ {DropTooHighProposalHeight}) \cup {KeepProposalSeen}
    [] candidate = ProposalHorizonDropsActiveOldView /\
          Bug = "horizon_keeps_active_old_view" ->
      (spec \ {DropActiveOldView}) \cup {KeepActiveCurrentView}
    [] candidate = ProposalHorizonKeepsActiveCurrentView /\
          Bug = "horizon_drops_active_current_view" ->
      (spec \ {KeepActiveCurrentView}) \cup {DropActiveOldView}
    [] candidate = ProposalHorizonKeepsOtherHeightHighView /\
          Bug = "horizon_applies_view_cap_to_other_height" ->
      (spec \ {KeepOtherHeightHighView}) \cup {DropActiveAboveViewCap}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

Bugs == {
  "none",
  "clear_leaves_retained",
  "owner_read_missing_returns_owner",
  "owner_insert_missing",
  "owner_replace_keeps_old",
  "owner_drops_matching_frontier_info",
  "owner_keeps_conflicting_frontier_info",
  "owner_keeps_matching_retained",
  "owner_drops_conflicting_retained",
  "frontier_info_keeps_other_hash",
  "frontier_info_drops_same_hash",
  "frontier_info_drops_other_slot",
  "retained_new_missing_insert",
  "retained_none_drops_existing_info",
  "retained_some_preserves_old_info",
  "retained_payload_not_or",
  "retained_timestamp_not_refreshed",
  "seed_uses_missing_payload",
  "seed_picks_lower_view",
  "seed_tie_picks_older",
  "seed_ignores_height",
  "remove_height_keeps_exact",
  "remove_height_drops_other",
  "prune_above_drops_equal",
  "prune_above_keeps_above",
  "prune_committed_keeps_equal",
  "prune_committed_drops_above",
  "proposal_insert_missing",
  "proposal_duplicate_drops_seen",
  "proposal_read_absent_returns_seen",
  "view_prune_keeps_old_view",
  "view_prune_drops_next_view",
  "horizon_keeps_committed_seen",
  "horizon_keeps_far_future_seen",
  "horizon_keeps_active_old_view",
  "horizon_drops_active_current_view",
  "horizon_applies_view_cap_to_other_height"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A candidate \in Candidates:
       ImplementationActions(candidate) \subseteq Actions

SlotTrackerStateMatchesSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ClearMatchesSpec ==
  ImplementationActions(ClearAllMaps) = SpecActions(ClearAllMaps)

OwnerReadMatchesSpec ==
  \A candidate \in {
    ReadExistingOwner,
    ReadAbsentOwner
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

OwnerRecordingMatchesSpec ==
  \A candidate \in {
    NoteOwnerInserts,
    NoteOwnerReplacesOwner,
    NoteOwnerKeepsMatchingFrontierInfo,
    NoteOwnerDropsConflictingFrontierInfo,
    NoteOwnerDropsMatchingRetainedBranch,
    NoteOwnerKeepsConflictingRetainedBranch
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

FrontierInfoMatchesSpec ==
  \A candidate \in {
    NoteFrontierInfoInserts,
    NoteFrontierInfoReplacesSameHash,
    NoteFrontierInfoDropsOtherHashSameSlot,
    NoteFrontierInfoKeepsOtherSlot
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

RetainedBranchMatchesSpec ==
  \A candidate \in {
    NoteRetainedNewBranch,
    NoteRetainedExistingPreservesInfoWhenNone,
    NoteRetainedExistingReplacesInfoWhenSome,
    NoteRetainedPayloadOrs,
    NoteRetainedRefreshesTimestamp
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

RetainedSeedMatchesSpec ==
  \A candidate \in {
    RetainedSeedIgnoresMissingPayload,
    RetainedSeedChoosesHighestView,
    RetainedSeedChoosesLatestRefreshTie,
    RetainedSeedAbsentHeightNone
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

HeightRemovalMatchesSpec ==
  \A candidate \in {
    RemoveHeightRemovesExact,
    RemoveHeightKeepsOther
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

HeightPruneMatchesSpec ==
  \A candidate \in {
    PruneAboveKeepsBelowAndEqual,
    PruneAboveDropsAbove,
    PruneCommittedDropsBelowAndEqual,
    PruneCommittedKeepsAbove
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

ProposalSeenMatchesSpec ==
  \A candidate \in {
    ProposalSeenInsertNew,
    ProposalSeenDuplicatePreserves,
    ProposalSeenReadExisting,
    ProposalSeenReadAbsent
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

ViewChangeProposalPruneMatchesSpec ==
  \A candidate \in {
    ViewChangePruneDropsOldSameHeight,
    ViewChangePruneKeepsNextSameHeight,
    ViewChangePruneKeepsOtherHeight
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

ProposalHorizonMatchesSpec ==
  \A candidate \in {
    ProposalHorizonDropsCommitted,
    ProposalHorizonDropsTooLowHeight,
    ProposalHorizonDropsTooHighHeight,
    ProposalHorizonDropsActiveOldView,
    ProposalHorizonKeepsActiveCurrentView,
    ProposalHorizonDropsActiveAboveCap,
    ProposalHorizonKeepsOtherHeightHighView
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

SeedRequiresPayloadAndHeightMatch ==
  \A candidate \in Candidates:
    (SeedHighestView \in ImplementationActions(candidate) \/
     SeedLatestRefresh \in ImplementationActions(candidate)) =>
      /\ IgnoreMissingPayload \in ImplementationActions(candidate)
      /\ FilterSeedHeight \in ImplementationActions(candidate)

CommittedPruneKeepsOnlyAbove ==
  \A candidate \in Candidates:
    DropEqualHeight \in ImplementationActions(candidate) =>
      DropBelowHeight \in ImplementationActions(candidate)

ProposalSeenReadIsExact ==
  \A candidate \in Candidates:
    ReturnProposalSeen \in ImplementationActions(candidate) =>
      candidate = ProposalSeenReadExisting

ViewPruneNeverDropsOtherHeights ==
  \A candidate \in Candidates:
    KeepOtherHeightProposalSeen \in ImplementationActions(candidate) =>
      DropOldSameHeightView \notin ImplementationActions(candidate)

HorizonViewCapOnlyAppliesToActiveHeight ==
  \A candidate \in Candidates:
    DropActiveAboveViewCap \in ImplementationActions(candidate) =>
      candidate = ProposalHorizonDropsActiveAboveCap

ClearAnchors ==
  SpecActions(ClearAllMaps) =
    {ClearOwners, ClearFrontiers, ClearProposals, ClearRetained}

OwnerReadAnchors ==
  /\ SpecActions(ReadExistingOwner) = {ReturnOwner}
  /\ SpecActions(ReadAbsentOwner) = {ReturnNone}

OwnerRecordingAnchors ==
  /\ SpecActions(NoteOwnerInserts) =
       {InsertOwner, KeepOtherSlotFrontierInfo, KeepOtherSlotRetained}
  /\ SpecActions(NoteOwnerReplacesOwner) =
       {ReplaceOwner, KeepOtherSlotFrontierInfo, KeepOtherSlotRetained}
  /\ SpecActions(NoteOwnerKeepsMatchingFrontierInfo) =
       {InsertOwner, KeepMatchingFrontierInfo}
  /\ SpecActions(NoteOwnerDropsConflictingFrontierInfo) =
       {InsertOwner, DropConflictingFrontierInfo}
  /\ SpecActions(NoteOwnerDropsMatchingRetainedBranch) =
       {InsertOwner, DropMatchingRetained}
  /\ SpecActions(NoteOwnerKeepsConflictingRetainedBranch) =
       {InsertOwner, KeepConflictingRetained}

FrontierInfoAnchors ==
  /\ SpecActions(NoteFrontierInfoInserts) =
       {InsertFrontierInfo, KeepOtherSlotFrontierInfo}
  /\ SpecActions(NoteFrontierInfoReplacesSameHash) =
       {ReplaceFrontierInfo}
  /\ SpecActions(NoteFrontierInfoDropsOtherHashSameSlot) =
       {InsertFrontierInfo, DropOtherHashFrontierInfo}
  /\ SpecActions(NoteFrontierInfoKeepsOtherSlot) =
       {InsertFrontierInfo, KeepOtherSlotFrontierInfo}

RetainedBranchAnchors ==
  /\ SpecActions(NoteRetainedNewBranch) =
       {InsertRetained, PreserveExistingInfo, PayloadOr, RefreshTimestamp}
  /\ SpecActions(NoteRetainedExistingPreservesInfoWhenNone) =
       {PreserveExistingInfo, PayloadOr, RefreshTimestamp}
  /\ SpecActions(NoteRetainedExistingReplacesInfoWhenSome) =
       {ReplaceExistingInfo, PayloadOr, RefreshTimestamp}
  /\ SpecActions(NoteRetainedPayloadOrs) =
       {PreserveExistingInfo, PayloadOr, RefreshTimestamp}
  /\ SpecActions(NoteRetainedRefreshesTimestamp) =
       {PreserveExistingInfo, PayloadOr, RefreshTimestamp}

RetainedSeedAnchors ==
  /\ SpecActions(RetainedSeedIgnoresMissingPayload) =
       {IgnoreMissingPayload, FilterSeedHeight, SeedNone}
  /\ SpecActions(RetainedSeedChoosesHighestView) =
       {IgnoreMissingPayload, FilterSeedHeight, SeedHighestView}
  /\ SpecActions(RetainedSeedChoosesLatestRefreshTie) =
       {IgnoreMissingPayload, FilterSeedHeight, SeedLatestRefresh}
  /\ SpecActions(RetainedSeedAbsentHeightNone) =
       {IgnoreMissingPayload, FilterSeedHeight, SeedNone}

HeightRemovalAnchors ==
  /\ SpecActions(RemoveHeightRemovesExact) =
       {RemoveExactHeight, KeepOtherHeight}
  /\ SpecActions(RemoveHeightKeepsOther) =
       {RemoveExactHeight, KeepOtherHeight}

HeightPruneAnchors ==
  /\ SpecActions(PruneAboveKeepsBelowAndEqual) =
       {KeepBelowHeight, KeepEqualHeight}
  /\ SpecActions(PruneAboveDropsAbove) = {DropAboveHeight}
  /\ SpecActions(PruneCommittedDropsBelowAndEqual) =
       {DropBelowHeight, DropEqualHeight}
  /\ SpecActions(PruneCommittedKeepsAbove) = {KeepAboveHeight}

ProposalSeenAnchors ==
  /\ SpecActions(ProposalSeenInsertNew) = {InsertProposalSeen}
  /\ SpecActions(ProposalSeenDuplicatePreserves) = {PreserveProposalSeen}
  /\ SpecActions(ProposalSeenReadExisting) = {ReturnProposalSeen}
  /\ SpecActions(ProposalSeenReadAbsent) = {ReturnProposalAbsent}

ViewChangeProposalPruneAnchors ==
  /\ SpecActions(ViewChangePruneDropsOldSameHeight) =
       {DropOldSameHeightView}
  /\ SpecActions(ViewChangePruneKeepsNextSameHeight) =
       {KeepNextSameHeightView}
  /\ SpecActions(ViewChangePruneKeepsOtherHeight) =
       {KeepOtherHeightProposalSeen}

ProposalHorizonAnchors ==
  /\ SpecActions(ProposalHorizonDropsCommitted) =
       {DropCommittedProposalSeen}
  /\ SpecActions(ProposalHorizonDropsTooLowHeight) =
       {DropTooLowProposalHeight}
  /\ SpecActions(ProposalHorizonDropsTooHighHeight) =
       {DropTooHighProposalHeight}
  /\ SpecActions(ProposalHorizonDropsActiveOldView) =
       {DropActiveOldView}
  /\ SpecActions(ProposalHorizonKeepsActiveCurrentView) =
       {KeepActiveCurrentView}
  /\ SpecActions(ProposalHorizonDropsActiveAboveCap) =
       {DropActiveAboveViewCap}
  /\ SpecActions(ProposalHorizonKeepsOtherHeightHighView) =
       {KeepOtherHeightHighView}

SlotTrackerClearExact ==
  /\ ClearMatchesSpec
  /\ ClearAnchors

SlotTrackerAuthoritativeExact ==
  /\ OwnerReadMatchesSpec
  /\ OwnerRecordingMatchesSpec
  /\ FrontierInfoMatchesSpec
  /\ OwnerReadAnchors
  /\ OwnerRecordingAnchors
  /\ FrontierInfoAnchors

SlotTrackerRetainedBranchExact ==
  /\ RetainedBranchMatchesSpec
  /\ RetainedSeedMatchesSpec
  /\ SeedRequiresPayloadAndHeightMatch
  /\ RetainedBranchAnchors
  /\ RetainedSeedAnchors

SlotTrackerHeightLifecycleExact ==
  /\ HeightRemovalMatchesSpec
  /\ HeightPruneMatchesSpec
  /\ CommittedPruneKeepsOnlyAbove
  /\ HeightRemovalAnchors
  /\ HeightPruneAnchors

SlotTrackerProposalSeenExact ==
  /\ ProposalSeenMatchesSpec
  /\ ViewChangeProposalPruneMatchesSpec
  /\ ProposalSeenReadIsExact
  /\ ViewPruneNeverDropsOtherHeights
  /\ ProposalSeenAnchors
  /\ ViewChangeProposalPruneAnchors

SlotTrackerProposalHorizonExact ==
  /\ ProposalHorizonMatchesSpec
  /\ HorizonViewCapOnlyAppliesToActiveHeight
  /\ ProposalHorizonAnchors

SlotTrackerStateExactness ==
  /\ SlotTrackerStateMatchesSpec
  /\ SlotTrackerClearExact
  /\ SlotTrackerAuthoritativeExact
  /\ SlotTrackerRetainedBranchExact
  /\ SlotTrackerHeightLifecycleExact
  /\ SlotTrackerProposalSeenExact
  /\ SlotTrackerProposalHorizonExact

Safety ==
  SlotTrackerStateExactness

=============================================================================
====
