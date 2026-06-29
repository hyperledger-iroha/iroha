---- MODULE SumeragiEffectiveTimingStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for effective timing status storage and projection.

This slice captures `set_effective_timing(...)`,
`effective_npos_timeouts_snapshot()`, and the `snapshot().effective_*` status
fields. `SumeragiEffectiveTimingGate` covers the deterministic derivation of
effective timing from consensus configuration; this gate pins the
operator-visible storage/projection contract after that derivation has produced
concrete values.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialZero == 1
StoreScalarTiming == 2
StoreSchedulingTiming == 3
StoreFanout == 4
StoreNposCore == 5
StoreNposPipeline == 6
StatusProjectsScalar == 7
StatusProjectsNpos == 8
OverwriteScalar == 9
OverwriteFanout == 10
OverwriteNpos == 11
ClearNpos == 12

Candidates == 1..12

InitialScalarsZero == 1
InitialNposAbsent == 2
MinFinalityStored == 3
BlockTimeStored == 4
CommitTimeStored == 5
PacingStored == 6
CommitQuorumStored == 7
AvailabilityStored == 8
PacemakerStored == 9
CollectorsStored == 10
RedundantStored == 11
NposPresent == 12
NposProposeStored == 13
NposPrevoteStored == 14
NposPrecommitStored == 15
NposCommitStored == 16
NposDaStored == 17
NposAggregatorStored == 18
NposExecStored == 19
NposWitnessStored == 20
StatusMinFinalityMatches == 21
StatusBlockTimeMatches == 22
StatusCommitTimeMatches == 23
StatusPacingMatches == 24
StatusCommitQuorumMatches == 25
StatusAvailabilityMatches == 26
StatusPacemakerMatches == 27
StatusCollectorsMatches == 28
StatusRedundantMatches == 29
StatusNposPresent == 30
StatusNposCoreMatches == 31
StatusNposPipelineMatches == 32
ScalarOverwriteLatest == 33
FanoutOverwriteLatest == 34
NposOverwriteLatest == 35
ClearNposAbsent == 36
ClearNposZeroed == 37

Actions == 1..37

ScalarStoreActions ==
  {MinFinalityStored, BlockTimeStored, CommitTimeStored, PacingStored}

SchedulingStoreActions ==
  {CommitQuorumStored, AvailabilityStored, PacemakerStored}

FanoutStoreActions == {CollectorsStored, RedundantStored}

NposCoreActions ==
  {NposPresent, NposProposeStored, NposPrevoteStored,
   NposPrecommitStored, NposCommitStored}

NposPipelineActions ==
  {NposDaStored, NposAggregatorStored, NposExecStored, NposWitnessStored}

StatusScalarActions ==
  {StatusMinFinalityMatches, StatusBlockTimeMatches, StatusCommitTimeMatches,
   StatusPacingMatches}

StatusSchedulingActions ==
  {StatusCommitQuorumMatches, StatusAvailabilityMatches,
   StatusPacemakerMatches}

StatusFanoutActions == {StatusCollectorsMatches, StatusRedundantMatches}

StatusNposActions ==
  {StatusNposPresent, StatusNposCoreMatches, StatusNposPipelineMatches}

SpecActions(candidate) ==
  CASE candidate = InitialZero ->
      {InitialScalarsZero, InitialNposAbsent}
    [] candidate = StoreScalarTiming ->
      ScalarStoreActions
    [] candidate = StoreSchedulingTiming ->
      SchedulingStoreActions
    [] candidate = StoreFanout ->
      FanoutStoreActions
    [] candidate = StoreNposCore ->
      NposCoreActions
    [] candidate = StoreNposPipeline ->
      NposPipelineActions
    [] candidate = StatusProjectsScalar ->
      StatusScalarActions \cup StatusSchedulingActions \cup StatusFanoutActions
    [] candidate = StatusProjectsNpos ->
      StatusNposActions
    [] candidate = OverwriteScalar ->
      ScalarStoreActions \cup SchedulingStoreActions \cup {ScalarOverwriteLatest}
    [] candidate = OverwriteFanout ->
      FanoutStoreActions \cup {FanoutOverwriteLatest}
    [] candidate = OverwriteNpos ->
      NposCoreActions \cup NposPipelineActions \cup {NposOverwriteLatest}
    [] candidate = ClearNpos ->
      {ClearNposAbsent, ClearNposZeroed}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialZero /\ Bug = "initial_scalar_nonzero" ->
      spec \ {InitialScalarsZero}
    [] candidate = InitialZero /\ Bug = "initial_npos_present" ->
      spec \ {InitialNposAbsent}
    [] candidate \in {StoreScalarTiming, OverwriteScalar} /\
          Bug = "min_finality_not_stored" ->
      spec \ {MinFinalityStored}
    [] candidate \in {StoreScalarTiming, OverwriteScalar} /\
          Bug = "block_time_not_stored" ->
      spec \ {BlockTimeStored}
    [] candidate \in {StoreScalarTiming, OverwriteScalar} /\
          Bug = "commit_time_not_stored" ->
      spec \ {CommitTimeStored}
    [] candidate \in {StoreScalarTiming, OverwriteScalar} /\
          Bug = "pacing_not_stored" ->
      spec \ {PacingStored}
    [] candidate \in {StoreSchedulingTiming, OverwriteScalar} /\
          Bug = "commit_quorum_not_stored" ->
      spec \ {CommitQuorumStored}
    [] candidate \in {StoreSchedulingTiming, OverwriteScalar} /\
          Bug = "availability_not_stored" ->
      spec \ {AvailabilityStored}
    [] candidate \in {StoreSchedulingTiming, OverwriteScalar} /\
          Bug = "pacemaker_not_stored" ->
      spec \ {PacemakerStored}
    [] candidate \in {StoreFanout, OverwriteFanout} /\
          Bug = "collectors_not_stored" ->
      spec \ {CollectorsStored}
    [] candidate \in {StoreFanout, OverwriteFanout} /\
          Bug = "redundant_not_stored" ->
      spec \ {RedundantStored}
    [] candidate \in {StoreNposCore, OverwriteNpos} /\
          Bug = "npos_missing" ->
      spec \ {NposPresent}
    [] candidate \in {StoreNposCore, OverwriteNpos} /\
          Bug = "npos_propose_not_stored" ->
      spec \ {NposProposeStored}
    [] candidate \in {StoreNposCore, OverwriteNpos} /\
          Bug = "npos_prevote_not_stored" ->
      spec \ {NposPrevoteStored}
    [] candidate \in {StoreNposCore, OverwriteNpos} /\
          Bug = "npos_precommit_not_stored" ->
      spec \ {NposPrecommitStored}
    [] candidate \in {StoreNposCore, OverwriteNpos} /\
          Bug = "npos_commit_not_stored" ->
      spec \ {NposCommitStored}
    [] candidate \in {StoreNposPipeline, OverwriteNpos} /\
          Bug = "npos_da_not_stored" ->
      spec \ {NposDaStored}
    [] candidate \in {StoreNposPipeline, OverwriteNpos} /\
          Bug = "npos_aggregator_not_stored" ->
      spec \ {NposAggregatorStored}
    [] candidate \in {StoreNposPipeline, OverwriteNpos} /\
          Bug = "npos_exec_not_stored" ->
      spec \ {NposExecStored}
    [] candidate \in {StoreNposPipeline, OverwriteNpos} /\
          Bug = "npos_witness_not_stored" ->
      spec \ {NposWitnessStored}
    [] candidate = StatusProjectsScalar /\
          Bug = "status_scalar_mismatch" ->
      spec \ StatusScalarActions
    [] candidate = StatusProjectsScalar /\
          Bug = "status_scheduling_mismatch" ->
      spec \ StatusSchedulingActions
    [] candidate = StatusProjectsScalar /\
          Bug = "status_fanout_mismatch" ->
      spec \ StatusFanoutActions
    [] candidate = StatusProjectsNpos /\ Bug = "status_npos_missing" ->
      spec \ {StatusNposPresent}
    [] candidate = StatusProjectsNpos /\ Bug = "status_npos_mismatch" ->
      spec \ (StatusNposActions \ {StatusNposPresent})
    [] candidate = OverwriteScalar /\ Bug = "overwrite_scalar_ignored" ->
      spec \ {ScalarOverwriteLatest}
    [] candidate = OverwriteFanout /\ Bug = "overwrite_fanout_ignored" ->
      spec \ {FanoutOverwriteLatest}
    [] candidate = OverwriteNpos /\ Bug = "overwrite_npos_ignored" ->
      spec \ {NposOverwriteLatest}
    [] candidate = ClearNpos /\ Bug = "clear_npos_keeps_active" ->
      spec \ {ClearNposAbsent}
    [] candidate = ClearNpos /\ Bug = "clear_npos_keeps_values" ->
      spec \ {ClearNposZeroed}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 12
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_scalar_nonzero",
       "initial_npos_present",
       "min_finality_not_stored",
       "block_time_not_stored",
       "commit_time_not_stored",
       "pacing_not_stored",
       "commit_quorum_not_stored",
       "availability_not_stored",
       "pacemaker_not_stored",
       "collectors_not_stored",
       "redundant_not_stored",
       "npos_missing",
       "npos_propose_not_stored",
       "npos_prevote_not_stored",
       "npos_precommit_not_stored",
       "npos_commit_not_stored",
       "npos_da_not_stored",
       "npos_aggregator_not_stored",
       "npos_exec_not_stored",
       "npos_witness_not_stored",
       "status_scalar_mismatch",
       "status_scheduling_mismatch",
       "status_fanout_mismatch",
       "status_npos_missing",
       "status_npos_mismatch",
       "overwrite_scalar_ignored",
       "overwrite_fanout_ignored",
       "overwrite_npos_ignored",
       "clear_npos_keeps_active",
       "clear_npos_keeps_values"
     }
  /\ checked \in 0..12
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

EffectiveTimingStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

EffectiveTimingStatusExactness ==
  /\ EffectiveTimingStatusActionsMatchSpec

EffectiveTimingStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EffectiveTimingStatusExactness

BugInitialScalarNonzero ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugInitialNposPresent ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugMinFinalityNotStored ==
  ImplementationActions(StoreScalarTiming) = SpecActions(StoreScalarTiming)

BugBlockTimeNotStored ==
  ImplementationActions(StoreScalarTiming) = SpecActions(StoreScalarTiming)

BugCommitTimeNotStored ==
  ImplementationActions(StoreScalarTiming) = SpecActions(StoreScalarTiming)

BugPacingNotStored ==
  ImplementationActions(StoreScalarTiming) = SpecActions(StoreScalarTiming)

BugCommitQuorumNotStored ==
  ImplementationActions(StoreSchedulingTiming) =
    SpecActions(StoreSchedulingTiming)

BugAvailabilityNotStored ==
  ImplementationActions(StoreSchedulingTiming) =
    SpecActions(StoreSchedulingTiming)

BugPacemakerNotStored ==
  ImplementationActions(StoreSchedulingTiming) =
    SpecActions(StoreSchedulingTiming)

BugCollectorsNotStored ==
  ImplementationActions(StoreFanout) = SpecActions(StoreFanout)

BugRedundantNotStored ==
  ImplementationActions(StoreFanout) = SpecActions(StoreFanout)

BugNposMissing ==
  ImplementationActions(StoreNposCore) = SpecActions(StoreNposCore)

BugNposProposeNotStored ==
  ImplementationActions(StoreNposCore) = SpecActions(StoreNposCore)

BugNposPrevoteNotStored ==
  ImplementationActions(StoreNposCore) = SpecActions(StoreNposCore)

BugNposPrecommitNotStored ==
  ImplementationActions(StoreNposCore) = SpecActions(StoreNposCore)

BugNposCommitNotStored ==
  ImplementationActions(StoreNposCore) = SpecActions(StoreNposCore)

BugNposDaNotStored ==
  ImplementationActions(StoreNposPipeline) = SpecActions(StoreNposPipeline)

BugNposAggregatorNotStored ==
  ImplementationActions(StoreNposPipeline) = SpecActions(StoreNposPipeline)

BugNposExecNotStored ==
  ImplementationActions(StoreNposPipeline) = SpecActions(StoreNposPipeline)

BugNposWitnessNotStored ==
  ImplementationActions(StoreNposPipeline) = SpecActions(StoreNposPipeline)

BugStatusScalarMismatch ==
  ImplementationActions(StatusProjectsScalar) =
    SpecActions(StatusProjectsScalar)

BugStatusSchedulingMismatch ==
  ImplementationActions(StatusProjectsScalar) =
    SpecActions(StatusProjectsScalar)

BugStatusFanoutMismatch ==
  ImplementationActions(StatusProjectsScalar) =
    SpecActions(StatusProjectsScalar)

BugStatusNposMissing ==
  ImplementationActions(StatusProjectsNpos) = SpecActions(StatusProjectsNpos)

BugStatusNposMismatch ==
  ImplementationActions(StatusProjectsNpos) = SpecActions(StatusProjectsNpos)

BugOverwriteScalarIgnored ==
  ImplementationActions(OverwriteScalar) = SpecActions(OverwriteScalar)

BugOverwriteFanoutIgnored ==
  ImplementationActions(OverwriteFanout) = SpecActions(OverwriteFanout)

BugOverwriteNposIgnored ==
  ImplementationActions(OverwriteNpos) = SpecActions(OverwriteNpos)

BugClearNposKeepsActive ==
  ImplementationActions(ClearNpos) = SpecActions(ClearNpos)

BugClearNposKeepsValues ==
  ImplementationActions(ClearNpos) = SpecActions(ClearNpos)

====
