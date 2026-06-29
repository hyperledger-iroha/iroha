---- MODULE SumeragiLiveVoteRosterGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for live local-vote roster selection.

This slice captures `roster_for_live_vote_with_mode(...)` and the active
topology fallback it delegates to. Local NEW_VIEW and precommit votes must use
an empty roster for heights beyond the live frontier, prefer a pending
activation roster for the requested height, otherwise derive the active roster
from committed topology, genesis/trusted fallback, or NPoS world state, and
canonicalize the result before signing.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FutureNoPending == "future_no_pending"
FutureWithPendingIgnored == "future_with_pending_ignored"
PendingCurrentWins == "pending_current_wins"
PendingNextWins == "pending_next_wins"
PendingEmptySuppressesFallback == "pending_empty_suppresses_fallback"
PermissionedCommitWins == "permissioned_commit_wins"
PermissionedCommitFilteredEmptyUsesGenesis ==
  "permissioned_commit_filtered_empty_uses_genesis"
PermissionedEmptyCommitUsesGenesis == "permissioned_empty_commit_uses_genesis"
PermissionedEmptyGenesisUsesTrusted == "permissioned_empty_genesis_uses_trusted"
PermissionedAllEmpty == "permissioned_all_empty"
NposWorldWins == "npos_world_wins"
NposWorldEmptyUsesFallback == "npos_world_empty_uses_fallback"
PendingCanonicalized == "pending_canonicalized"
ActiveCanonicalized == "active_canonicalized"
CommittedHeightUsesActive == "committed_height_uses_active"

Cases == {
  FutureNoPending,
  FutureWithPendingIgnored,
  PendingCurrentWins,
  PendingNextWins,
  PendingEmptySuppressesFallback,
  PermissionedCommitWins,
  PermissionedCommitFilteredEmptyUsesGenesis,
  PermissionedEmptyCommitUsesGenesis,
  PermissionedEmptyGenesisUsesTrusted,
  PermissionedAllEmpty,
  NposWorldWins,
  NposWorldEmptyUsesFallback,
  PendingCanonicalized,
  ActiveCanonicalized,
  CommittedHeightUsesActive
}

HeightGuard == 1
ReturnEmpty == 2
PendingLookup == 3
PendingSource == 4
NoFallbackAfterPending == 5
ActiveFallback == 6
PermissionedMode == 7
NposMode == 8
CommitTopologyChecked == 9
CommitTopologyEmpty == 10
FilterLiveKeys == 11
CommitFilteredEmpty == 12
CommitSource == 13
GenesisSource == 14
TrustedSource == 15
NposWorldSource == 16
NposLegacyFallbackSource == 17
GenesisEmpty == 18
TrustedEmpty == 19
Canonicalize == 20
Dedup == 21
Sort == 22
PrevTopologySource == 23

Actions == 1..23

SpecActions(c) ==
  CASE c = FutureNoPending ->
      {HeightGuard, ReturnEmpty}
    [] c = FutureWithPendingIgnored ->
      {HeightGuard, ReturnEmpty}
    [] c = PendingCurrentWins ->
      {PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] c = PendingNextWins ->
      {PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] c = PendingEmptySuppressesFallback ->
      {PendingLookup, PendingSource, NoFallbackAfterPending, Canonicalize,
       ReturnEmpty}
    [] c = PermissionedCommitWins ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitSource, Canonicalize, Dedup, Sort}
    [] c = PermissionedCommitFilteredEmptyUsesGenesis ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitFilteredEmpty, GenesisSource, Canonicalize,
       Dedup, Sort}
    [] c = PermissionedEmptyCommitUsesGenesis ->
      {ActiveFallback, PermissionedMode, CommitTopologyEmpty, GenesisSource,
       Canonicalize, Dedup, Sort}
    [] c = PermissionedEmptyGenesisUsesTrusted ->
      {ActiveFallback, PermissionedMode, CommitTopologyEmpty, GenesisEmpty,
       TrustedSource, Canonicalize, Dedup, Sort}
    [] c = PermissionedAllEmpty ->
      {ActiveFallback, PermissionedMode, CommitTopologyEmpty, GenesisEmpty,
       TrustedEmpty, ReturnEmpty}
    [] c = NposWorldWins ->
      {ActiveFallback, NposMode, NposWorldSource, Canonicalize, Dedup, Sort}
    [] c = NposWorldEmptyUsesFallback ->
      {ActiveFallback, NposMode, NposLegacyFallbackSource, Canonicalize,
       Dedup, Sort}
    [] c = PendingCanonicalized ->
      {PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] c = ActiveCanonicalized ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitSource, Canonicalize, Dedup, Sort}
    [] c = CommittedHeightUsesActive ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitSource, Canonicalize, Dedup, Sort}
    [] OTHER -> {}

ActualActions(c) ==
  CASE Bug = "allow_future_pending"
       /\ c = FutureWithPendingIgnored ->
      {PendingLookup, PendingSource, Canonicalize, Dedup, Sort}
    [] Bug = "allow_future_active"
       /\ c = FutureNoPending ->
      {ActiveFallback, PermissionedMode, CommitSource, Canonicalize,
       Dedup, Sort}
    [] Bug = "ignore_pending_activation"
       /\ c = PendingCurrentWins ->
      {ActiveFallback, PermissionedMode, CommitSource, Canonicalize,
       Dedup, Sort}
    [] Bug = "pending_empty_falls_back"
       /\ c = PendingEmptySuppressesFallback ->
      {PendingLookup, ActiveFallback, PermissionedMode, CommitSource,
       Canonicalize, Dedup, Sort}
    [] Bug = "skip_pending_canonicalize"
       /\ c = PendingCanonicalized ->
      {PendingLookup, PendingSource, Dedup, Sort}
    [] Bug = "skip_active_canonicalize"
       /\ c = ActiveCanonicalized ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitSource, Dedup, Sort}
    [] Bug = "preserve_pending_duplicates"
       /\ c = PendingCanonicalized ->
      {PendingLookup, PendingSource, Canonicalize, Sort}
    [] Bug = "preserve_pending_order"
       /\ c = PendingCanonicalized ->
      {PendingLookup, PendingSource, Canonicalize, Dedup}
    [] Bug = "use_prev_topology_at_committed_height"
       /\ c = CommittedHeightUsesActive ->
      {PrevTopologySource, Canonicalize, Dedup, Sort}
    [] Bug = "commit_filtered_empty_returns_empty"
       /\ c = PermissionedCommitFilteredEmptyUsesGenesis ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitFilteredEmpty, ReturnEmpty}
    [] Bug = "ignore_genesis_fallback"
       /\ c = PermissionedEmptyCommitUsesGenesis ->
      {ActiveFallback, PermissionedMode, CommitTopologyEmpty, ReturnEmpty}
    [] Bug = "ignore_trusted_fallback"
       /\ c = PermissionedEmptyGenesisUsesTrusted ->
      {ActiveFallback, PermissionedMode, CommitTopologyEmpty, GenesisEmpty,
       ReturnEmpty}
    [] Bug = "permissioned_uses_npos_world"
       /\ c = PermissionedCommitWins ->
      {ActiveFallback, NposMode, NposWorldSource, Canonicalize, Dedup, Sort}
    [] Bug = "npos_uses_permissioned_fallback"
       /\ c = NposWorldWins ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       FilterLiveKeys, CommitSource, Canonicalize, Dedup, Sort}
    [] Bug = "skip_live_key_filter"
       /\ c = PermissionedCommitWins ->
      {ActiveFallback, PermissionedMode, CommitTopologyChecked,
       CommitSource, Canonicalize, Dedup, Sort}
    [] Bug = "skip_npos_legacy_fallback"
       /\ c = NposWorldEmptyUsesFallback ->
      {ActiveFallback, NposMode, ReturnEmpty}
    [] OTHER -> SpecActions(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

LiveVoteRosterMatchesSpec ==
  \A c \in Cases: ActualActions(c) = SpecActions(c)

SafetyFast ==
  LiveVoteRosterMatchesSpec

LiveVoteRosterExactness ==
  /\ LiveVoteRosterMatchesSpec

LiveVoteRosterCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LiveVoteRosterExactness

BugAllowFuturePending ==
  ActualActions(FutureWithPendingIgnored) =
    SpecActions(FutureWithPendingIgnored)

BugAllowFutureActive ==
  ActualActions(FutureNoPending) = SpecActions(FutureNoPending)

BugIgnorePendingActivation ==
  ActualActions(PendingCurrentWins) = SpecActions(PendingCurrentWins)

BugPendingEmptyFallsBack ==
  ActualActions(PendingEmptySuppressesFallback) =
    SpecActions(PendingEmptySuppressesFallback)

BugSkipPendingCanonicalize ==
  ActualActions(PendingCanonicalized) = SpecActions(PendingCanonicalized)

BugSkipActiveCanonicalize ==
  ActualActions(ActiveCanonicalized) = SpecActions(ActiveCanonicalized)

BugPreservePendingDuplicates ==
  ActualActions(PendingCanonicalized) = SpecActions(PendingCanonicalized)

BugPreservePendingOrder ==
  ActualActions(PendingCanonicalized) = SpecActions(PendingCanonicalized)

BugUsePrevTopologyAtCommittedHeight ==
  ActualActions(CommittedHeightUsesActive) =
    SpecActions(CommittedHeightUsesActive)

BugCommitFilteredEmptyReturnsEmpty ==
  ActualActions(PermissionedCommitFilteredEmptyUsesGenesis) =
    SpecActions(PermissionedCommitFilteredEmptyUsesGenesis)

BugIgnoreGenesisFallback ==
  ActualActions(PermissionedEmptyCommitUsesGenesis) =
    SpecActions(PermissionedEmptyCommitUsesGenesis)

BugIgnoreTrustedFallback ==
  ActualActions(PermissionedEmptyGenesisUsesTrusted) =
    SpecActions(PermissionedEmptyGenesisUsesTrusted)

BugPermissionedUsesNposWorld ==
  ActualActions(PermissionedCommitWins) = SpecActions(PermissionedCommitWins)

BugNposUsesPermissionedFallback ==
  ActualActions(NposWorldWins) = SpecActions(NposWorldWins)

BugSkipLiveKeyFilter ==
  ActualActions(PermissionedCommitWins) = SpecActions(PermissionedCommitWins)

BugSkipNposLegacyFallback ==
  ActualActions(NposWorldEmptyUsesFallback) =
    SpecActions(NposWorldEmptyUsesFallback)

====
