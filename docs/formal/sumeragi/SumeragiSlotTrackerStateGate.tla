---- MODULE SumeragiSlotTrackerStateGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `SlotTrackerState` helper semantics.

This slice models the map/set transformations in
`main_loop/slot_tracker.rs`: authoritative slot-owner recording, authoritative
frontier metadata replacement, retained-branch refresh/seed priority, and
height pruning. Concrete hashes, frontier payloads, and instants are collapsed
into representative cases while preserving the observable contracts that keep
same-height branch ownership deterministic.
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

Candidates == 1..28

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

Actions == 1..35

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
  "prune_committed_drops_above"
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

Safety ==
  /\ SlotTrackerStateMatchesSpec
  /\ ClearMatchesSpec
  /\ OwnerReadMatchesSpec
  /\ OwnerRecordingMatchesSpec
  /\ FrontierInfoMatchesSpec
  /\ RetainedBranchMatchesSpec
  /\ RetainedSeedMatchesSpec
  /\ HeightRemovalMatchesSpec
  /\ HeightPruneMatchesSpec
  /\ SeedRequiresPayloadAndHeightMatch
  /\ CommittedPruneKeepsOnlyAbove
  /\ ClearAnchors
  /\ OwnerReadAnchors
  /\ OwnerRecordingAnchors
  /\ FrontierInfoAnchors
  /\ RetainedBranchAnchors
  /\ RetainedSeedAnchors
  /\ HeightRemovalAnchors
  /\ HeightPruneAnchors

=============================================================================
====
