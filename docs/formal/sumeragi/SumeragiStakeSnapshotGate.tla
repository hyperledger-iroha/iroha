---- MODULE SumeragiStakeSnapshotGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi stake snapshot helpers.

This slice captures `CommitStakeSnapshot::from_roster(...)`,
`CommitStakeSnapshot::matches_roster(...)`,
`stake_quorum_reached_for_world(...)`,
`stake_coverage_bps_for_world(...)`,
`stake_quorum_reached_for_snapshot(...)`, `stake_map_from_world(...)`,
`fallback_stake_for_world(...)`, and `commit_stake_snapshot_from_map(...)`.
It abstracts numeric stake values to finite cases while preserving the
observable contracts: empty rosters have no snapshot, snapshot entries follow
roster order and use fallback stake for missing peers, active duplicate stake
records keep the maximum stake per peer, inactive validators are ignored,
fallback stake is at least one, roster-hash matching is exact and order
sensitive, stake quorum is strict greater-than two thirds, unknown signers and
bad snapshots fail closed, duplicate snapshot entries use their maximum stake,
and coverage basis points are integer-divided and clamped at 10,000.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

SnapshotEmptyRosterNone == "snapshot_empty_roster_none"
SnapshotEmptyStakeMapFallbackAll == "snapshot_empty_stake_map_fallback_all"
SnapshotPartialStakeMapFallbackMissing ==
  "snapshot_partial_stake_map_fallback_missing"
SnapshotPreservesRosterOrder == "snapshot_preserves_roster_order"
SnapshotHashMatchesExactRoster == "snapshot_hash_matches_exact_roster"
SnapshotHashRejectsReorderedRoster == "snapshot_hash_rejects_reordered_roster"
StakeMapIgnoresInactive == "stake_map_ignores_inactive"
StakeMapKeepsMaxPerPeer == "stake_map_keeps_max_per_peer"
FallbackMinSelfBondAtLeastOne == "fallback_min_self_bond_at_least_one"
WorldQuorumFallbackStrictBoundary == "world_quorum_fallback_strict_boundary"
WorldQuorumFallbackAboveThreshold == "world_quorum_fallback_above_threshold"
WorldQuorumPartialMapFallbackMatchesSnapshot ==
  "world_quorum_partial_map_fallback_matches_snapshot"
WorldQuorumSignerOutOfRoster == "world_quorum_signer_out_of_roster"
SnapshotQuorumMismatchRejects == "snapshot_quorum_mismatch_rejects"
SnapshotQuorumMissingStakeRejects == "snapshot_quorum_missing_stake_rejects"
SnapshotQuorumZeroTotalRejects == "snapshot_quorum_zero_total_rejects"
SnapshotQuorumDuplicateEntriesUseMax ==
  "snapshot_quorum_duplicate_entries_use_max"
SnapshotQuorumStrictBoundaryRejects ==
  "snapshot_quorum_strict_boundary_rejects"
SnapshotQuorumAboveThresholdAccepts ==
  "snapshot_quorum_above_threshold_accepts"
CoverageReportsFloorBps == "coverage_reports_floor_bps"
CoverageClampsAt10000 == "coverage_clamps_at_10000"

Cases == {
  SnapshotEmptyRosterNone,
  SnapshotEmptyStakeMapFallbackAll,
  SnapshotPartialStakeMapFallbackMissing,
  SnapshotPreservesRosterOrder,
  SnapshotHashMatchesExactRoster,
  SnapshotHashRejectsReorderedRoster,
  StakeMapIgnoresInactive,
  StakeMapKeepsMaxPerPeer,
  FallbackMinSelfBondAtLeastOne,
  WorldQuorumFallbackStrictBoundary,
  WorldQuorumFallbackAboveThreshold,
  WorldQuorumPartialMapFallbackMatchesSnapshot,
  WorldQuorumSignerOutOfRoster,
  SnapshotQuorumMismatchRejects,
  SnapshotQuorumMissingStakeRejects,
  SnapshotQuorumZeroTotalRejects,
  SnapshotQuorumDuplicateEntriesUseMax,
  SnapshotQuorumStrictBoundaryRejects,
  SnapshotQuorumAboveThresholdAccepts,
  CoverageReportsFloorBps,
  CoverageClampsAt10000
}

ReturnNone == 1
ReturnSnapshot == 2
FallbackUsed == 3
StakeMapValueUsed == 4
OrderPreserved == 5
HashMatches == 6
HashMismatch == 7
InactiveIgnored == 8
MaxStakeKept == 9
FallbackAtLeastOne == 10
QuorumTrue == 11
QuorumFalse == 12
StrictGreaterThan == 13
BoundaryRejected == 14
SignerOutOfRosterError == 15
SnapshotMismatchError == 16
MissingStakeError == 17
ZeroTotalError == 18
DuplicateMaxUsed == 19
CoverageBps7500 == 20
CoverageBps10000 == 21
CoverageClamped == 22
SnapshotMatchesDirectWorld == 23

Actions == 1..23

SpecActions(c) ==
  CASE c = SnapshotEmptyRosterNone ->
      {ReturnNone}
    [] c = SnapshotEmptyStakeMapFallbackAll ->
      {ReturnSnapshot, FallbackUsed, OrderPreserved, HashMatches}
    [] c = SnapshotPartialStakeMapFallbackMissing ->
      {ReturnSnapshot, StakeMapValueUsed, FallbackUsed, OrderPreserved}
    [] c = SnapshotPreservesRosterOrder ->
      {ReturnSnapshot, OrderPreserved}
    [] c = SnapshotHashMatchesExactRoster ->
      {HashMatches}
    [] c = SnapshotHashRejectsReorderedRoster ->
      {HashMismatch}
    [] c = StakeMapIgnoresInactive ->
      {InactiveIgnored}
    [] c = StakeMapKeepsMaxPerPeer ->
      {MaxStakeKept}
    [] c = FallbackMinSelfBondAtLeastOne ->
      {FallbackAtLeastOne}
    [] c = WorldQuorumFallbackStrictBoundary ->
      {FallbackUsed, StrictGreaterThan, BoundaryRejected, QuorumFalse}
    [] c = WorldQuorumFallbackAboveThreshold ->
      {FallbackUsed, StrictGreaterThan, QuorumTrue}
    [] c = WorldQuorumPartialMapFallbackMatchesSnapshot ->
      {StakeMapValueUsed, FallbackUsed, SnapshotMatchesDirectWorld}
    [] c = WorldQuorumSignerOutOfRoster ->
      {SignerOutOfRosterError}
    [] c = SnapshotQuorumMismatchRejects ->
      {SnapshotMismatchError}
    [] c = SnapshotQuorumMissingStakeRejects ->
      {MissingStakeError}
    [] c = SnapshotQuorumZeroTotalRejects ->
      {ZeroTotalError}
    [] c = SnapshotQuorumDuplicateEntriesUseMax ->
      {DuplicateMaxUsed, StrictGreaterThan, QuorumTrue}
    [] c = SnapshotQuorumStrictBoundaryRejects ->
      {StrictGreaterThan, BoundaryRejected, QuorumFalse}
    [] c = SnapshotQuorumAboveThresholdAccepts ->
      {StrictGreaterThan, QuorumTrue}
    [] c = CoverageReportsFloorBps ->
      {CoverageBps7500}
    [] c = CoverageClampsAt10000 ->
      {CoverageBps10000, CoverageClamped}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "snapshot_empty_roster_returns_snapshot"
       /\ c = SnapshotEmptyRosterNone ->
      (spec \ {ReturnNone}) \cup {ReturnSnapshot}
    [] Bug = "snapshot_empty_map_missing_fallback"
       /\ c = SnapshotEmptyStakeMapFallbackAll ->
      (spec \ {FallbackUsed, ReturnSnapshot}) \cup {ReturnNone}
    [] Bug = "snapshot_partial_map_skips_missing"
       /\ c = SnapshotPartialStakeMapFallbackMissing ->
      (spec \ {FallbackUsed}) \cup {MissingStakeError}
    [] Bug = "snapshot_reorders_roster"
       /\ c = SnapshotPreservesRosterOrder ->
      spec \ {OrderPreserved}
    [] Bug = "matches_roster_ignores_order"
       /\ c = SnapshotHashRejectsReorderedRoster ->
      (spec \ {HashMismatch}) \cup {HashMatches}
    [] Bug = "stake_map_keeps_inactive"
       /\ c = StakeMapIgnoresInactive ->
      spec \ {InactiveIgnored}
    [] Bug = "stake_map_uses_min_stake"
       /\ c = StakeMapKeepsMaxPerPeer ->
      spec \ {MaxStakeKept}
    [] Bug = "fallback_allows_zero"
       /\ c = FallbackMinSelfBondAtLeastOne ->
      spec \ {FallbackAtLeastOne}
    [] Bug = "world_quorum_boundary_accepts"
       /\ c = WorldQuorumFallbackStrictBoundary ->
      (spec \ {BoundaryRejected, QuorumFalse}) \cup {QuorumTrue}
    [] Bug = "world_quorum_missing_fallback"
       /\ c = WorldQuorumFallbackAboveThreshold ->
      (spec \ {FallbackUsed, QuorumTrue}) \cup {MissingStakeError}
    [] Bug = "world_quorum_accepts_unknown_signer"
       /\ c = WorldQuorumSignerOutOfRoster ->
      (spec \ {SignerOutOfRosterError}) \cup {QuorumTrue}
    [] Bug = "snapshot_mismatch_accepted"
       /\ c = SnapshotQuorumMismatchRejects ->
      (spec \ {SnapshotMismatchError}) \cup {QuorumTrue}
    [] Bug = "snapshot_missing_stake_fallback"
       /\ c = SnapshotQuorumMissingStakeRejects ->
      (spec \ {MissingStakeError}) \cup {FallbackUsed, QuorumFalse}
    [] Bug = "snapshot_zero_total_false"
       /\ c = SnapshotQuorumZeroTotalRejects ->
      (spec \ {ZeroTotalError}) \cup {QuorumFalse}
    [] Bug = "snapshot_duplicate_uses_first"
       /\ c = SnapshotQuorumDuplicateEntriesUseMax ->
      (spec \ {DuplicateMaxUsed, QuorumTrue}) \cup {QuorumFalse}
    [] Bug = "snapshot_boundary_accepts"
       /\ c = SnapshotQuorumStrictBoundaryRejects ->
      (spec \ {BoundaryRejected, QuorumFalse}) \cup {QuorumTrue}
    [] Bug = "coverage_rounds_up"
       /\ c = CoverageReportsFloorBps ->
      (spec \ {CoverageBps7500}) \cup {CoverageBps10000}
    [] Bug = "coverage_not_clamped"
       /\ c = CoverageClampsAt10000 ->
      spec \ {CoverageClamped}
    [] Bug = "world_snapshot_diverge_on_missing"
       /\ c = WorldQuorumPartialMapFallbackMatchesSnapshot ->
      spec \ {SnapshotMatchesDirectWorld}
    [] OTHER -> spec

Bugs == {
  "none",
  "snapshot_empty_roster_returns_snapshot",
  "snapshot_empty_map_missing_fallback",
  "snapshot_partial_map_skips_missing",
  "snapshot_reorders_roster",
  "matches_roster_ignores_order",
  "stake_map_keeps_inactive",
  "stake_map_uses_min_stake",
  "fallback_allows_zero",
  "world_quorum_boundary_accepts",
  "world_quorum_missing_fallback",
  "world_quorum_accepts_unknown_signer",
  "snapshot_mismatch_accepted",
  "snapshot_missing_stake_fallback",
  "snapshot_zero_total_false",
  "snapshot_duplicate_uses_first",
  "snapshot_boundary_accepts",
  "coverage_rounds_up",
  "coverage_not_clamped",
  "world_snapshot_diverge_on_missing"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

StakeSnapshotCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == StakeSnapshotCoreSafety

SafetyFast == StakeSnapshotCoreSafety

BugSnapshotEmptyRosterReturnsSnapshot == NoBugInvariant
BugSnapshotEmptyMapMissingFallback == NoBugInvariant
BugSnapshotPartialMapSkipsMissing == NoBugInvariant
BugSnapshotReordersRoster == NoBugInvariant
BugMatchesRosterIgnoresOrder == NoBugInvariant
BugStakeMapKeepsInactive == NoBugInvariant
BugStakeMapUsesMinStake == NoBugInvariant
BugFallbackAllowsZero == NoBugInvariant
BugWorldQuorumBoundaryAccepts == NoBugInvariant
BugWorldQuorumMissingFallback == NoBugInvariant
BugWorldQuorumAcceptsUnknownSigner == NoBugInvariant
BugSnapshotMismatchAccepted == NoBugInvariant
BugSnapshotMissingStakeFallback == NoBugInvariant
BugSnapshotZeroTotalFalse == NoBugInvariant
BugSnapshotDuplicateUsesFirst == NoBugInvariant
BugSnapshotBoundaryAccepts == NoBugInvariant
BugCoverageRoundsUp == NoBugInvariant
BugCoverageNotClamped == NoBugInvariant
BugWorldSnapshotDivergeOnMissing == NoBugInvariant

====
