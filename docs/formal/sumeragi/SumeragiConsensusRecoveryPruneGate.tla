---- MODULE SumeragiConsensusRecoveryPruneGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for consensus-recovery entry cleanup.

This slice pins the entry lifecycle parts of:
- `clear_consensus_recovery_for_round(height, view)`, which removes every
  `consensus_recovery` entry for the cleared height, independent of the
  latest-committed-hash component of the key, and resets the published roster
  recovery status/dwell snapshot; and
- `prune_stale_consensus_recovery(now)`, which keeps only entries at or above
  `committed_height.saturating_sub(1)` whose last attempt is no older than
  `max(commit_quorum_timeout, propose_interval, 1ms) * 8`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ClearWrongHeight == "clear_wrong_height"
ClearSameHeight == "clear_same_height"
ClearSameHeightOtherHash == "clear_same_height_other_hash"
ClearNoMatching == "clear_no_matching"

PruneBelowFloor == "prune_below_floor"
PruneAtFloorFresh == "prune_at_floor_fresh"
PruneAtFloorAgeEqual == "prune_at_floor_age_equal"
PruneAtFloorAgeOver == "prune_at_floor_age_over"
PruneFutureFresh == "prune_future_fresh"
PruneCommittedZeroHeightZero == "prune_committed_zero_height_zero"
PruneCommitTimeoutMax == "prune_commit_timeout_max"
PruneProposeTimeoutMax == "prune_propose_timeout_max"
PruneFloorTimeout == "prune_floor_timeout"

ClearCases == {
  ClearWrongHeight,
  ClearSameHeight,
  ClearSameHeightOtherHash,
  ClearNoMatching
}

PruneCases == {
  PruneBelowFloor,
  PruneAtFloorFresh,
  PruneAtFloorAgeEqual,
  PruneAtFloorAgeOver,
  PruneFutureFresh,
  PruneCommittedZeroHeightZero,
  PruneCommitTimeoutMax,
  PruneProposeTimeoutMax,
  PruneFloorTimeout
}

Cases == ClearCases \cup PruneCases

SpecRemove(c) ==
  c \in {
    ClearSameHeight,
    ClearSameHeightOtherHash,
    PruneBelowFloor,
    PruneAtFloorAgeOver
  }

ClearCaseHasMatchingEntry(c) ==
  c # ClearNoMatching

ClearCaseSameHeight(c) ==
  c \in {ClearSameHeight, ClearSameHeightOtherHash}

PruneHeightPassesFloor(c) ==
  c # PruneBelowFloor

PruneAgeWithinRetention(c) ==
  c # PruneAtFloorAgeOver

PruneUsesCommitMax(c) ==
  c = PruneCommitTimeoutMax

PruneUsesProposeMax(c) ==
  c = PruneProposeTimeoutMax

PruneUsesFloorMax(c) ==
  c = PruneFloorTimeout

EntryPresent == 1
EntryAbsent == 2
ClearHeightChecked == 3
LatestCommittedHashIgnored == 4
StatusSteadySet == 5
DwellCleared == 6
CommittedFloorChecked == 7
AgeChecked == 8
CommitTimeoutChecked == 9
ProposeIntervalChecked == 10
FloorRetentionChecked == 11
RetentionMultipliedByEight == 12

ActionUniverse == 1..12

KeepActions == {EntryPresent}
RemoveActions == {EntryAbsent}

ClearMetadataActions ==
  {ClearHeightChecked, StatusSteadySet, DwellCleared}

RetentionActions(c) ==
  {CommittedFloorChecked}
    \cup (IF PruneHeightPassesFloor(c)
        THEN {AgeChecked, CommitTimeoutChecked, ProposeIntervalChecked,
              FloorRetentionChecked, RetentionMultipliedByEight}
        ELSE {})

SpecActions(c) ==
  (IF SpecRemove(c) THEN RemoveActions ELSE KeepActions)
    \cup (IF c \in ClearCases THEN ClearMetadataActions ELSE {})
    \cup (IF c = ClearSameHeightOtherHash
        THEN {LatestCommittedHashIgnored}
        ELSE {})
    \cup (IF c \in PruneCases THEN RetentionActions(c) ELSE {})

ClearKeepSameHeightActions ==
  KeepActions \cup ClearMetadataActions

ClearRemoveWrongHeightActions ==
  RemoveActions \cup ClearMetadataActions

ClearRequireHashMatchActions ==
  KeepActions \cup ClearMetadataActions

ClearSkipStatusActions ==
  (RemoveActions \cup ClearMetadataActions) \ {StatusSteadySet}

ClearSkipDwellActions ==
  (RemoveActions \cup ClearMetadataActions) \ {DwellCleared}

ClearNoMatchingSkipStatusActions ==
  (KeepActions \cup ClearMetadataActions) \ {StatusSteadySet}

PruneKeepBelowFloorActions ==
  KeepActions \cup {CommittedFloorChecked}

PruneDropAtFloorActions(c) ==
  RemoveActions \cup RetentionActions(c)

PruneKeepAgeOverActions ==
  KeepActions

PruneZeroCommittedUnderflowActions ==
  RemoveActions \cup RetentionActions(PruneCommittedZeroHeightZero)

PruneSkipAgeCheckActions ==
  KeepActions \cup {CommittedFloorChecked}

PruneAgeCheckBelowFloorActions ==
  RemoveActions \cup {CommittedFloorChecked, AgeChecked}

ImplementationActions(c) ==
  CASE Bug = "clear_keep_same_height"
       /\ c = ClearSameHeight ->
      ClearKeepSameHeightActions
    [] Bug = "clear_prune_wrong_height"
       /\ c = ClearWrongHeight ->
      ClearRemoveWrongHeightActions
    [] Bug = "clear_require_hash_match"
       /\ c = ClearSameHeightOtherHash ->
      ClearRequireHashMatchActions
    [] Bug = "clear_skip_status_reset"
       /\ c = ClearSameHeight ->
      ClearSkipStatusActions
    [] Bug = "clear_skip_dwell_clear"
       /\ c = ClearSameHeight ->
      ClearSkipDwellActions
    [] Bug = "clear_no_match_skip_status_reset"
       /\ c = ClearNoMatching ->
      ClearNoMatchingSkipStatusActions
    [] Bug = "prune_keep_below_floor"
       /\ c = PruneBelowFloor ->
      PruneKeepBelowFloorActions
    [] Bug = "prune_drop_at_floor"
       /\ c = PruneAtFloorFresh ->
      PruneDropAtFloorActions(c)
    [] Bug = "prune_drop_age_equal"
       /\ c = PruneAtFloorAgeEqual ->
      PruneDropAtFloorActions(c)
    [] Bug = "prune_keep_age_over"
       /\ c = PruneAtFloorAgeOver ->
      PruneKeepAgeOverActions
    [] Bug = "prune_underflow_zero_committed"
       /\ c = PruneCommittedZeroHeightZero ->
      PruneZeroCommittedUnderflowActions
    [] Bug = "prune_use_commit_timeout_only"
       /\ c = PruneProposeTimeoutMax ->
      PruneDropAtFloorActions(c)
    [] Bug = "prune_use_propose_timeout_only"
       /\ c = PruneCommitTimeoutMax ->
      PruneDropAtFloorActions(c)
    [] Bug = "prune_omit_floor_timeout"
       /\ c = PruneFloorTimeout ->
      PruneDropAtFloorActions(c)
    [] Bug = "prune_skip_age_check"
       /\ c = PruneAtFloorAgeOver ->
      PruneSkipAgeCheckActions
    [] Bug = "prune_age_check_below_floor"
       /\ c = PruneBelowFloor ->
      PruneAgeCheckBelowFloorActions
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "clear_keep_same_height",
  "clear_prune_wrong_height",
  "clear_require_hash_match",
  "clear_skip_status_reset",
  "clear_skip_dwell_clear",
  "clear_no_match_skip_status_reset",
  "prune_keep_below_floor",
  "prune_drop_at_floor",
  "prune_drop_age_equal",
  "prune_keep_age_over",
  "prune_underflow_zero_committed",
  "prune_use_commit_timeout_only",
  "prune_use_propose_timeout_only",
  "prune_omit_floor_timeout",
  "prune_skip_age_check",
  "prune_age_check_below_floor"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecRemove(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ClearRemovesOnlyMatchingHeightEntries ==
  /\ EntryPresent \in ImplementationActions(ClearWrongHeight)
  /\ EntryAbsent \in ImplementationActions(ClearSameHeight)
  /\ EntryAbsent \in ImplementationActions(ClearSameHeightOtherHash)
  /\ LatestCommittedHashIgnored \in
       ImplementationActions(ClearSameHeightOtherHash)

ClearAlwaysResetsPublishedStatus ==
  \A c \in ClearCases:
    /\ StatusSteadySet \in ImplementationActions(c)
    /\ DwellCleared \in ImplementationActions(c)

PruneUsesCommittedHeightFloor ==
  /\ EntryAbsent \in ImplementationActions(PruneBelowFloor)
  /\ EntryPresent \in ImplementationActions(PruneAtFloorFresh)
  /\ EntryPresent \in ImplementationActions(PruneCommittedZeroHeightZero)
  /\ ~(AgeChecked \in ImplementationActions(PruneBelowFloor))

PruneRetainsThroughRetentionBoundary ==
  /\ EntryPresent \in ImplementationActions(PruneAtFloorAgeEqual)
  /\ EntryAbsent \in ImplementationActions(PruneAtFloorAgeOver)

PruneRetentionUsesMaxTimeoutFloorTimesEight ==
  /\ EntryPresent \in ImplementationActions(PruneCommitTimeoutMax)
  /\ EntryPresent \in ImplementationActions(PruneProposeTimeoutMax)
  /\ EntryPresent \in ImplementationActions(PruneFloorTimeout)
  /\ RetentionMultipliedByEight \in ImplementationActions(PruneFloorTimeout)
  /\ CommitTimeoutChecked \in ImplementationActions(PruneCommitTimeoutMax)
  /\ ProposeIntervalChecked \in ImplementationActions(PruneProposeTimeoutMax)
  /\ FloorRetentionChecked \in ImplementationActions(PruneFloorTimeout)

NoEntryBothPresentAndAbsent ==
  \A c \in Cases:
    ~(/\ EntryPresent \in ImplementationActions(c)
      /\ EntryAbsent \in ImplementationActions(c))

EntryPresenceMatchesSpec ==
  \A c \in Cases:
    /\ (EntryAbsent \in ImplementationActions(c)) = SpecRemove(c)
    /\ (EntryPresent \in ImplementationActions(c)) = ~SpecRemove(c)

ClearMetadataResetAnchors ==
  /\ \A c \in ClearCases:
       /\ ClearHeightChecked \in ImplementationActions(c)
       /\ StatusSteadySet \in ImplementationActions(c)
       /\ DwellCleared \in ImplementationActions(c)
  /\ LatestCommittedHashIgnored \in
       ImplementationActions(ClearSameHeightOtherHash)
  /\ \A c \in ClearCases \ {ClearSameHeightOtherHash}:
       LatestCommittedHashIgnored \notin ImplementationActions(c)

PruneRetentionCheckGating ==
  /\ \A c \in PruneCases:
       CommittedFloorChecked \in ImplementationActions(c)
  /\ AgeChecked \notin ImplementationActions(PruneBelowFloor)
  /\ CommitTimeoutChecked \notin ImplementationActions(PruneBelowFloor)
  /\ ProposeIntervalChecked \notin ImplementationActions(PruneBelowFloor)
  /\ FloorRetentionChecked \notin ImplementationActions(PruneBelowFloor)
  /\ RetentionMultipliedByEight \notin ImplementationActions(PruneBelowFloor)
  /\ \A c \in PruneCases \ {PruneBelowFloor}:
       /\ AgeChecked \in ImplementationActions(c)
       /\ CommitTimeoutChecked \in ImplementationActions(c)
       /\ ProposeIntervalChecked \in ImplementationActions(c)
       /\ FloorRetentionChecked \in ImplementationActions(c)
       /\ RetentionMultipliedByEight \in ImplementationActions(c)

PruneBoundaryAnchors ==
  /\ EntryAbsent \in ImplementationActions(PruneBelowFloor)
  /\ EntryPresent \in ImplementationActions(PruneAtFloorFresh)
  /\ EntryPresent \in ImplementationActions(PruneAtFloorAgeEqual)
  /\ EntryAbsent \in ImplementationActions(PruneAtFloorAgeOver)
  /\ EntryPresent \in ImplementationActions(PruneFutureFresh)
  /\ EntryPresent \in ImplementationActions(PruneCommittedZeroHeightZero)

ConsensusRecoveryPruneCoreSafety ==
  /\ ActionsMatchSpec
  /\ ClearRemovesOnlyMatchingHeightEntries
  /\ ClearAlwaysResetsPublishedStatus
  /\ PruneUsesCommittedHeightFloor
  /\ PruneRetainsThroughRetentionBoundary
  /\ PruneRetentionUsesMaxTimeoutFloorTimesEight
  /\ NoEntryBothPresentAndAbsent
  /\ EntryPresenceMatchesSpec
  /\ ClearMetadataResetAnchors
  /\ PruneRetentionCheckGating
  /\ PruneBoundaryAnchors

NoBugInvariant == ConsensusRecoveryPruneCoreSafety

SafetyFast == ConsensusRecoveryPruneCoreSafety

ConsensusRecoveryPruneCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ConsensusRecoveryPruneCoreSafety

====
