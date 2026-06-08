---- MODULE SumeragiBlockSyncUpdateRosterHydrationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for BlockSyncUpdate roster hydration.

This slice captures the non-cryptographic control policy in
`block_sync_update_with_roster_inner(...)`. It preserves observable behavior:
the outbound update is built from the block first; consensus mode is resolved
for the block height; persisted roster evidence is tried before block-sync
history evidence; the persisted lookup receives the block hash, height, view,
roster cache, and sidecar permission; the history lookup receives the same
block identity plus chain id; selected evidence short-circuits later sources;
uncertified fallback is used only when allowed; fallback roster material comes
from commit-topology state or world peers when commit topology is empty; fallback
rosters are filtered at `block_height.saturating_add(1)`, canonicalized by
consensus mode, labeled `CommitTopologySnapshot`, and ignored when empty; any
selection is applied to the update; and NPoS missing stake snapshots are filled
from the selected roster without replacing existing snapshots or running in
Permissioned/no-selection cases.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

BuildUpdateFromBlock == 1
ResolveConsensusMode == 2
PersistedFirst == 3
PersistedIdentityArgs == 4
PersistedAllowsSidecar == 5
HistorySecond == 6
HistoryIdentityArgs == 7
PersistedShortCircuitsHistory == 8
PersistedShortCircuitsFallback == 9
HistoryShortCircuitsFallback == 10
CertifiedDisablesFallback == 11
FallbackUsesCommitTopology == 12
FallbackUsesWorldPeersWhenCommitEmpty == 13
FallbackFiltersNextHeight == 14
FallbackUsesSaturatingNextHeight == 15
FallbackCanonicalizesByMode == 16
FallbackSourceCommitTopologySnapshot == 17
FallbackEmptyNoSelection == 18
SelectionAppliedToUpdate == 19
NoSelectionLeavesUpdateUnrostered == 20
NposMissingStakeFilled == 21
NposExistingStakePreserved == 22
NposNoSelectionNoStakeFill == 23
PermissionedNoStakeFill == 24

Candidates == 1..24

ConstructionCases == {
  BuildUpdateFromBlock,
  ResolveConsensusMode
}

PersistedLookupCases == {
  PersistedFirst,
  PersistedIdentityArgs,
  PersistedAllowsSidecar
}

HistoryLookupCases == {
  HistorySecond,
  HistoryIdentityArgs
}

ShortCircuitCases == {
  PersistedShortCircuitsHistory,
  PersistedShortCircuitsFallback,
  HistoryShortCircuitsFallback
}

FallbackGateCases == {
  CertifiedDisablesFallback
}

FallbackSelectionCases == {
  FallbackUsesCommitTopology,
  FallbackUsesWorldPeersWhenCommitEmpty,
  FallbackFiltersNextHeight,
  FallbackUsesSaturatingNextHeight,
  FallbackCanonicalizesByMode,
  FallbackSourceCommitTopologySnapshot,
  FallbackEmptyNoSelection
}

UpdateApplicationCases == {
  SelectionAppliedToUpdate,
  NoSelectionLeavesUpdateUnrostered
}

StakeFillCases == {
  NposMissingStakeFilled,
  NposExistingStakePreserved,
  NposNoSelectionNoStakeFill,
  PermissionedNoStakeFill
}

BuildUpdateAction == 1
ResolveModeByHeight == 2
LookupPersisted == 3
LookupHistory == 4
LookupFallback == 5
UseBlockHash == 6
UseBlockHeight == 7
UseBlockView == 8
UseChainId == 9
UseRosterCache == 10
AllowSidecarTrue == 11
ShortCircuitHistory == 12
ShortCircuitFallback == 13
AllowUncertifiedGate == 14
CommitTopologyRead == 15
WorldPeersRead == 16
CommitTopologyPreferred == 17
WorldPeersWhenCommitEmpty == 18
FilterLiveConsensusKeys == 19
UseSaturatingNextHeight == 20
CanonicalizeRosterByMode == 21
SourceCommitTopologySnapshot == 22
DropEmptyFallbackRoster == 23
ApplySelection == 24
PreserveUnrosteredUpdate == 25
NposStakeSnapshotLookup == 26
RequireMissingStake == 27
PreserveExistingStake == 28
RequireSelectionForStakeFill == 29
PermissionedSkipsStakeFill == 30

Actions == 1..30

SpecActions(candidate) ==
  CASE candidate = BuildUpdateFromBlock ->
      {BuildUpdateAction}
    [] candidate = ResolveConsensusMode ->
      {ResolveModeByHeight}
    [] candidate = PersistedFirst ->
      {LookupPersisted}
    [] candidate = PersistedIdentityArgs ->
      {LookupPersisted, UseBlockHash, UseBlockHeight, UseBlockView,
       UseRosterCache}
    [] candidate = PersistedAllowsSidecar ->
      {LookupPersisted, AllowSidecarTrue}
    [] candidate = HistorySecond ->
      {LookupPersisted, LookupHistory}
    [] candidate = HistoryIdentityArgs ->
      {LookupHistory, UseBlockHash, UseBlockHeight, UseBlockView, UseChainId,
       UseRosterCache}
    [] candidate = PersistedShortCircuitsHistory ->
      {LookupPersisted, ShortCircuitHistory}
    [] candidate = PersistedShortCircuitsFallback ->
      {LookupPersisted, ShortCircuitFallback}
    [] candidate = HistoryShortCircuitsFallback ->
      {LookupPersisted, LookupHistory, ShortCircuitFallback}
    [] candidate = CertifiedDisablesFallback ->
      {AllowUncertifiedGate}
    [] candidate = FallbackUsesCommitTopology ->
      {LookupFallback, CommitTopologyRead, CommitTopologyPreferred}
    [] candidate = FallbackUsesWorldPeersWhenCommitEmpty ->
      {LookupFallback, CommitTopologyRead, WorldPeersRead,
       WorldPeersWhenCommitEmpty}
    [] candidate = FallbackFiltersNextHeight ->
      {FilterLiveConsensusKeys, UseBlockHeight}
    [] candidate = FallbackUsesSaturatingNextHeight ->
      {FilterLiveConsensusKeys, UseSaturatingNextHeight}
    [] candidate = FallbackCanonicalizesByMode ->
      {CanonicalizeRosterByMode, ResolveModeByHeight}
    [] candidate = FallbackSourceCommitTopologySnapshot ->
      {SourceCommitTopologySnapshot}
    [] candidate = FallbackEmptyNoSelection ->
      {DropEmptyFallbackRoster}
    [] candidate = SelectionAppliedToUpdate ->
      {ApplySelection}
    [] candidate = NoSelectionLeavesUpdateUnrostered ->
      {PreserveUnrosteredUpdate}
    [] candidate = NposMissingStakeFilled ->
      {RequireMissingStake, RequireSelectionForStakeFill,
       NposStakeSnapshotLookup}
    [] candidate = NposExistingStakePreserved ->
      {PreserveExistingStake}
    [] candidate = NposNoSelectionNoStakeFill ->
      {RequireSelectionForStakeFill}
    [] candidate = PermissionedNoStakeFill ->
      {PermissionedSkipsStakeFill}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = BuildUpdateFromBlock /\ Bug = "builds_empty_update" ->
      spec \ {BuildUpdateAction}
    [] candidate = ResolveConsensusMode /\ Bug = "uses_fallback_mode_directly" ->
      spec \ {ResolveModeByHeight}
    [] candidate = PersistedFirst /\ Bug = "history_before_persisted" ->
      (spec \ {LookupPersisted}) \cup {LookupHistory}
    [] candidate = PersistedIdentityArgs /\ Bug = "persisted_drops_block_view" ->
      spec \ {UseBlockView}
    [] candidate = PersistedAllowsSidecar /\ Bug = "persisted_disallows_sidecar" ->
      spec \ {AllowSidecarTrue}
    [] candidate = HistorySecond /\ Bug = "skips_history_lookup" ->
      spec \ {LookupHistory}
    [] candidate = HistoryIdentityArgs /\ Bug = "history_drops_chain_id" ->
      spec \ {UseChainId}
    [] candidate = PersistedShortCircuitsHistory /\
          Bug = "persisted_does_not_short_circuit_history" ->
      spec \ {ShortCircuitHistory}
    [] candidate = PersistedShortCircuitsFallback /\
          Bug = "persisted_does_not_short_circuit_fallback" ->
      spec \ {ShortCircuitFallback}
    [] candidate = HistoryShortCircuitsFallback /\
          Bug = "history_does_not_short_circuit_fallback" ->
      spec \ {ShortCircuitFallback}
    [] candidate = CertifiedDisablesFallback /\
          Bug = "certified_uses_uncertified_fallback" ->
      (spec \ {AllowUncertifiedGate}) \cup {LookupFallback}
    [] candidate = FallbackUsesCommitTopology /\
          Bug = "fallback_ignores_commit_topology" ->
      (spec \ {CommitTopologyRead, CommitTopologyPreferred}) \cup
        {WorldPeersRead}
    [] candidate = FallbackUsesWorldPeersWhenCommitEmpty /\
          Bug = "fallback_empty_commit_returns_none" ->
      (spec \ {WorldPeersRead, WorldPeersWhenCommitEmpty}) \cup
        {DropEmptyFallbackRoster}
    [] candidate = FallbackFiltersNextHeight /\
          Bug = "fallback_skips_live_key_filter" ->
      spec \ {FilterLiveConsensusKeys}
    [] candidate = FallbackUsesSaturatingNextHeight /\
          Bug = "fallback_uses_block_height_for_filter" ->
      spec \ {UseSaturatingNextHeight}
    [] candidate = FallbackCanonicalizesByMode /\
          Bug = "fallback_skips_canonicalize" ->
      spec \ {CanonicalizeRosterByMode}
    [] candidate = FallbackSourceCommitTopologySnapshot /\
          Bug = "fallback_source_history" ->
      (spec \ {SourceCommitTopologySnapshot}) \cup {LookupHistory}
    [] candidate = FallbackEmptyNoSelection /\
          Bug = "fallback_empty_selection" ->
      spec \ {DropEmptyFallbackRoster}
    [] candidate = SelectionAppliedToUpdate /\ Bug = "selection_not_applied" ->
      spec \ {ApplySelection}
    [] candidate = NoSelectionLeavesUpdateUnrostered /\
          Bug = "no_selection_adds_roster" ->
      (spec \ {PreserveUnrosteredUpdate}) \cup {ApplySelection}
    [] candidate = NposMissingStakeFilled /\ Bug = "npos_missing_stake_not_filled" ->
      spec \ {NposStakeSnapshotLookup}
    [] candidate = NposExistingStakePreserved /\
          Bug = "npos_existing_stake_replaced" ->
      (spec \ {PreserveExistingStake}) \cup {NposStakeSnapshotLookup}
    [] candidate = NposNoSelectionNoStakeFill /\
          Bug = "npos_no_selection_fills_stake" ->
      (spec \ {RequireSelectionForStakeFill}) \cup {NposStakeSnapshotLookup}
    [] candidate = PermissionedNoStakeFill /\
          Bug = "permissioned_fills_stake" ->
      (spec \ {PermissionedSkipsStakeFill}) \cup {NposStakeSnapshotLookup}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "builds_empty_update",
       "uses_fallback_mode_directly",
       "history_before_persisted",
       "persisted_drops_block_view",
       "persisted_disallows_sidecar",
       "skips_history_lookup",
       "history_drops_chain_id",
       "persisted_does_not_short_circuit_history",
       "persisted_does_not_short_circuit_fallback",
       "history_does_not_short_circuit_fallback",
       "certified_uses_uncertified_fallback",
       "fallback_ignores_commit_topology",
       "fallback_empty_commit_returns_none",
       "fallback_skips_live_key_filter",
       "fallback_uses_block_height_for_filter",
       "fallback_skips_canonicalize",
       "fallback_source_history",
       "fallback_empty_selection",
       "selection_not_applied",
       "no_selection_adds_roster",
       "npos_missing_stake_not_filled",
       "npos_existing_stake_replaced",
       "npos_no_selection_fills_stake",
       "permissioned_fills_stake"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterConstructionExact ==
  \A c \in ConstructionCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterPersistedLookupExact ==
  \A c \in PersistedLookupCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterHistoryLookupExact ==
  \A c \in HistoryLookupCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterShortCircuitExact ==
  \A c \in ShortCircuitCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterFallbackGateExact ==
  \A c \in FallbackGateCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterFallbackSelectionExact ==
  \A c \in FallbackSelectionCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterApplicationExact ==
  \A c \in UpdateApplicationCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterStakeFillExact ==
  \A c \in StakeFillCases:
    ImplementationActions(c) = SpecActions(c)

BlockSyncUpdateRosterHydrationExactness ==
  /\ BlockSyncUpdateRosterConstructionExact
  /\ BlockSyncUpdateRosterPersistedLookupExact
  /\ BlockSyncUpdateRosterHistoryLookupExact
  /\ BlockSyncUpdateRosterShortCircuitExact
  /\ BlockSyncUpdateRosterFallbackGateExact
  /\ BlockSyncUpdateRosterFallbackSelectionExact
  /\ BlockSyncUpdateRosterApplicationExact
  /\ BlockSyncUpdateRosterStakeFillExact

BugBuildsEmptyUpdate ==
  ImplementationActions(BuildUpdateFromBlock) = SpecActions(BuildUpdateFromBlock)

BugUsesFallbackModeDirectly ==
  ImplementationActions(ResolveConsensusMode) = SpecActions(ResolveConsensusMode)

BugHistoryBeforePersisted ==
  ImplementationActions(PersistedFirst) = SpecActions(PersistedFirst)

BugPersistedDropsBlockView ==
  ImplementationActions(PersistedIdentityArgs) =
    SpecActions(PersistedIdentityArgs)

BugPersistedDisallowsSidecar ==
  ImplementationActions(PersistedAllowsSidecar) =
    SpecActions(PersistedAllowsSidecar)

BugSkipsHistoryLookup ==
  ImplementationActions(HistorySecond) = SpecActions(HistorySecond)

BugHistoryDropsChainId ==
  ImplementationActions(HistoryIdentityArgs) = SpecActions(HistoryIdentityArgs)

BugPersistedDoesNotShortCircuitHistory ==
  ImplementationActions(PersistedShortCircuitsHistory) =
    SpecActions(PersistedShortCircuitsHistory)

BugPersistedDoesNotShortCircuitFallback ==
  ImplementationActions(PersistedShortCircuitsFallback) =
    SpecActions(PersistedShortCircuitsFallback)

BugHistoryDoesNotShortCircuitFallback ==
  ImplementationActions(HistoryShortCircuitsFallback) =
    SpecActions(HistoryShortCircuitsFallback)

BugCertifiedUsesUncertifiedFallback ==
  ImplementationActions(CertifiedDisablesFallback) =
    SpecActions(CertifiedDisablesFallback)

BugFallbackIgnoresCommitTopology ==
  ImplementationActions(FallbackUsesCommitTopology) =
    SpecActions(FallbackUsesCommitTopology)

BugFallbackEmptyCommitReturnsNone ==
  ImplementationActions(FallbackUsesWorldPeersWhenCommitEmpty) =
    SpecActions(FallbackUsesWorldPeersWhenCommitEmpty)

BugFallbackSkipsLiveKeyFilter ==
  ImplementationActions(FallbackFiltersNextHeight) =
    SpecActions(FallbackFiltersNextHeight)

BugFallbackUsesBlockHeightForFilter ==
  ImplementationActions(FallbackUsesSaturatingNextHeight) =
    SpecActions(FallbackUsesSaturatingNextHeight)

BugFallbackSkipsCanonicalize ==
  ImplementationActions(FallbackCanonicalizesByMode) =
    SpecActions(FallbackCanonicalizesByMode)

BugFallbackSourceHistory ==
  ImplementationActions(FallbackSourceCommitTopologySnapshot) =
    SpecActions(FallbackSourceCommitTopologySnapshot)

BugFallbackEmptySelection ==
  ImplementationActions(FallbackEmptyNoSelection) =
    SpecActions(FallbackEmptyNoSelection)

BugSelectionNotApplied ==
  ImplementationActions(SelectionAppliedToUpdate) =
    SpecActions(SelectionAppliedToUpdate)

BugNoSelectionAddsRoster ==
  ImplementationActions(NoSelectionLeavesUpdateUnrostered) =
    SpecActions(NoSelectionLeavesUpdateUnrostered)

BugNposMissingStakeNotFilled ==
  ImplementationActions(NposMissingStakeFilled) =
    SpecActions(NposMissingStakeFilled)

BugNposExistingStakeReplaced ==
  ImplementationActions(NposExistingStakePreserved) =
    SpecActions(NposExistingStakePreserved)

BugNposNoSelectionFillsStake ==
  ImplementationActions(NposNoSelectionNoStakeFill) =
    SpecActions(NposNoSelectionNoStakeFill)

BugPermissionedFillsStake ==
  ImplementationActions(PermissionedNoStakeFill) =
    SpecActions(PermissionedNoStakeFill)

====
