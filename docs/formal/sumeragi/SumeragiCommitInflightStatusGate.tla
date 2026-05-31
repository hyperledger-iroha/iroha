---- MODULE SumeragiCommitInflightStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-inflight status projection.

This slice captures `set_commit_inflight_timeout(...)`,
`record_commit_inflight_start(...)`, `record_commit_inflight_finish(...)`,
`record_commit_inflight_timeout(...)`, `reset_commit_inflight_for_tests()`,
`commit_inflight_snapshot()`, and the JSON/typed Torii status projections for
`commit_inflight`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsActiveIdentity == 1
ResetClearsHashes == 2
ResetClearsTimingAndConfig == 3
ResetClearsCountersAndQueues == 4
SetTimeoutStoresMs == 5
StartStoresActiveIdentity == 6
StartStoresHashAndStarted == 7
StartIncrementsPause == 8
StartCapturesPauseDepth == 9
StartOverwritesPrevious == 10
FinishNoActiveNoop == 11
FinishWrongIdNoop == 12
FinishMatchingClearsIdentity == 13
FinishMatchingIncrementsResume == 14
FinishCapturesResumeDepth == 15
FinishPreservesTimeoutHistory == 16
SnapshotElapsedActive == 17
SnapshotElapsedInactiveZero == 18
TimeoutRecordsCounterAndElapsed == 19
TimeoutRecordsRoundHashTimestamp == 20
TopLevelSnapshotIncludesInflight == 21
JsonProjectsCoreFields == 22
JsonProjectsTimeoutFields == 23
JsonProjectsQueueFields == 24
TypedProjectsAllFields == 25

Candidates == 1..25

ResetActive == 1
ResetId == 2
ResetHeight == 3
ResetView == 4
ResetBlockHash == 5
ResetLastTimeoutHash == 6
ResetStarted == 7
ResetTimeoutConfig == 8
ResetTimeoutCounters == 9
ResetLastTimeoutRound == 10
ResetLastTimeoutTimestamp == 11
ResetPauseResumeCounters == 12
ResetPausedSince == 13
ResetQueueDepths == 14
SetTimeoutMs == 15
StartActive == 16
StartId == 17
StartHeight == 18
StartView == 19
StartHash == 20
StartStarted == 21
PauseTotalIncrement == 22
PausedSinceSet == 23
PauseDepthSnapshot == 24
OverwritePrevious == 25
FinishChecksActive == 26
FinishChecksId == 27
FinishClearsActive == 28
FinishClearsIdentity == 29
FinishClearsStarted == 30
FinishClearsHash == 31
FinishClearsPausedSince == 32
ResumeTotalIncrement == 33
ResumeDepthSnapshot == 34
PreserveTimeoutHistory == 35
ElapsedActiveSaturating == 36
ElapsedInactiveZero == 37
TimeoutTotalIncrement == 38
TimeoutElapsedStored == 39
TimeoutRoundStored == 40
TimeoutHashStored == 41
TimeoutTimestampStored == 42
SnapshotIncludesInflight == 43
JsonCoreMatches == 44
JsonTimeoutMatches == 45
JsonQueuesMatch == 46
TypedCoreMatches == 47
TypedTimeoutMatches == 48
TypedQueuesMatch == 49

Actions == 1..49

ResetIdentityActions == {ResetActive, ResetId, ResetHeight, ResetView}
ResetHashActions == {ResetBlockHash, ResetLastTimeoutHash}
ResetTimingActions ==
  {ResetStarted, ResetTimeoutConfig, ResetLastTimeoutTimestamp}
ResetCounterQueueActions ==
  {ResetTimeoutCounters, ResetLastTimeoutRound, ResetPauseResumeCounters,
   ResetPausedSince, ResetQueueDepths}
StartIdentityActions == {StartActive, StartId, StartHeight, StartView}
StartHashStartedActions == {StartHash, StartStarted}
FinishClearIdentityActions ==
  {FinishClearsActive, FinishClearsIdentity, FinishClearsStarted,
   FinishClearsHash, FinishClearsPausedSince}
TimeoutProjectionActions ==
  {TimeoutTotalIncrement, TimeoutElapsedStored, TimeoutRoundStored,
   TimeoutHashStored, TimeoutTimestampStored}
JsonAllActions == {JsonCoreMatches, JsonTimeoutMatches, JsonQueuesMatch}
TypedAllActions == {TypedCoreMatches, TypedTimeoutMatches, TypedQueuesMatch}

SpecActions(candidate) ==
  CASE candidate = ResetClearsActiveIdentity ->
      ResetIdentityActions
    [] candidate = ResetClearsHashes ->
      ResetHashActions
    [] candidate = ResetClearsTimingAndConfig ->
      ResetTimingActions
    [] candidate = ResetClearsCountersAndQueues ->
      ResetCounterQueueActions
    [] candidate = SetTimeoutStoresMs ->
      {SetTimeoutMs}
    [] candidate = StartStoresActiveIdentity ->
      StartIdentityActions
    [] candidate = StartStoresHashAndStarted ->
      StartHashStartedActions
    [] candidate = StartIncrementsPause ->
      {PauseTotalIncrement, PausedSinceSet}
    [] candidate = StartCapturesPauseDepth ->
      {PauseDepthSnapshot}
    [] candidate = StartOverwritesPrevious ->
      StartIdentityActions \cup StartHashStartedActions \cup {OverwritePrevious}
    [] candidate = FinishNoActiveNoop ->
      {FinishChecksActive}
    [] candidate = FinishWrongIdNoop ->
      {FinishChecksActive, FinishChecksId}
    [] candidate = FinishMatchingClearsIdentity ->
      {FinishChecksActive, FinishChecksId} \cup FinishClearIdentityActions
    [] candidate = FinishMatchingIncrementsResume ->
      {FinishChecksActive, FinishChecksId, ResumeTotalIncrement}
    [] candidate = FinishCapturesResumeDepth ->
      {FinishChecksActive, FinishChecksId, ResumeDepthSnapshot}
    [] candidate = FinishPreservesTimeoutHistory ->
      {FinishChecksActive, FinishChecksId, PreserveTimeoutHistory}
    [] candidate = SnapshotElapsedActive ->
      {StartActive, StartStarted, ElapsedActiveSaturating}
    [] candidate = SnapshotElapsedInactiveZero ->
      {ElapsedInactiveZero}
    [] candidate = TimeoutRecordsCounterAndElapsed ->
      {TimeoutTotalIncrement, TimeoutElapsedStored}
    [] candidate = TimeoutRecordsRoundHashTimestamp ->
      {TimeoutRoundStored, TimeoutHashStored, TimeoutTimestampStored}
    [] candidate = TopLevelSnapshotIncludesInflight ->
      {SnapshotIncludesInflight}
    [] candidate = JsonProjectsCoreFields ->
      {SnapshotIncludesInflight, JsonCoreMatches}
    [] candidate = JsonProjectsTimeoutFields ->
      {SnapshotIncludesInflight, JsonTimeoutMatches}
    [] candidate = JsonProjectsQueueFields ->
      {SnapshotIncludesInflight, JsonQueuesMatch}
    [] candidate = TypedProjectsAllFields ->
      {SnapshotIncludesInflight} \cup TypedAllActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsActiveIdentity /\
          Bug = "reset_keeps_active" ->
      spec \ ResetIdentityActions
    [] candidate = ResetClearsHashes /\
          Bug = "reset_keeps_hashes" ->
      spec \ ResetHashActions
    [] candidate = ResetClearsTimingAndConfig /\
          Bug = "reset_keeps_timeout_config" ->
      spec \ ResetTimingActions
    [] candidate = ResetClearsCountersAndQueues /\
          Bug = "reset_keeps_counters" ->
      spec \ ResetCounterQueueActions
    [] candidate = SetTimeoutStoresMs /\
          Bug = "timeout_config_not_stored" ->
      spec \ {SetTimeoutMs}
    [] candidate = StartStoresActiveIdentity /\
          Bug = "start_identity_dropped" ->
      spec \ StartIdentityActions
    [] candidate = StartStoresHashAndStarted /\
          Bug = "start_hash_started_dropped" ->
      spec \ StartHashStartedActions
    [] candidate = StartIncrementsPause /\
          Bug = "start_pause_not_incremented" ->
      spec \ {PauseTotalIncrement, PausedSinceSet}
    [] candidate = StartCapturesPauseDepth /\
          Bug = "start_pause_depth_missing" ->
      spec \ {PauseDepthSnapshot}
    [] candidate = StartOverwritesPrevious /\
          Bug = "start_does_not_overwrite" ->
      spec \ {OverwritePrevious}
    [] candidate = FinishNoActiveNoop /\
          Bug = "finish_no_active_mutates" ->
      spec \ {FinishChecksActive}
    [] candidate = FinishWrongIdNoop /\
          Bug = "finish_wrong_id_mutates" ->
      spec \ {FinishChecksId}
    [] candidate = FinishMatchingClearsIdentity /\
          Bug = "finish_keeps_identity" ->
      spec \ FinishClearIdentityActions
    [] candidate = FinishMatchingIncrementsResume /\
          Bug = "finish_resume_not_incremented" ->
      spec \ {ResumeTotalIncrement}
    [] candidate = FinishCapturesResumeDepth /\
          Bug = "finish_resume_depth_missing" ->
      spec \ {ResumeDepthSnapshot}
    [] candidate = FinishPreservesTimeoutHistory /\
          Bug = "finish_clears_timeout_history" ->
      spec \ {PreserveTimeoutHistory}
    [] candidate = SnapshotElapsedActive /\
          Bug = "elapsed_active_zero" ->
      spec \ {ElapsedActiveSaturating}
    [] candidate = SnapshotElapsedInactiveZero /\
          Bug = "elapsed_inactive_nonzero" ->
      spec \ {ElapsedInactiveZero}
    [] candidate = TimeoutRecordsCounterAndElapsed /\
          Bug = "timeout_counter_missing" ->
      spec \ {TimeoutTotalIncrement}
    [] candidate = TimeoutRecordsCounterAndElapsed /\
          Bug = "timeout_elapsed_missing" ->
      spec \ {TimeoutElapsedStored}
    [] candidate = TimeoutRecordsRoundHashTimestamp /\
          Bug = "timeout_round_missing" ->
      spec \ {TimeoutRoundStored}
    [] candidate = TimeoutRecordsRoundHashTimestamp /\
          Bug = "timeout_hash_dropped" ->
      spec \ {TimeoutHashStored}
    [] candidate = TimeoutRecordsRoundHashTimestamp /\
          Bug = "timeout_timestamp_zero" ->
      spec \ {TimeoutTimestampStored}
    [] candidate = TopLevelSnapshotIncludesInflight /\
          Bug = "snapshot_drops_inflight" ->
      spec \ {SnapshotIncludesInflight}
    [] candidate = JsonProjectsCoreFields /\
          Bug = "json_core_mismatch" ->
      spec \ {JsonCoreMatches}
    [] candidate = JsonProjectsTimeoutFields /\
          Bug = "json_timeout_mismatch" ->
      spec \ {JsonTimeoutMatches}
    [] candidate = JsonProjectsQueueFields /\
          Bug = "json_queue_depth_mismatch" ->
      spec \ {JsonQueuesMatch}
    [] candidate = TypedProjectsAllFields /\
          Bug = "typed_core_mismatch" ->
      spec \ {TypedCoreMatches}
    [] candidate = TypedProjectsAllFields /\
          Bug = "typed_timeout_mismatch" ->
      spec \ {TypedTimeoutMatches}
    [] candidate = TypedProjectsAllFields /\
          Bug = "typed_queue_depth_mismatch" ->
      spec \ {TypedQueuesMatch}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..25

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetKeepsActive ==
  ImplementationActions(ResetClearsActiveIdentity) =
    SpecActions(ResetClearsActiveIdentity)

BugResetKeepsHashes ==
  ImplementationActions(ResetClearsHashes) =
    SpecActions(ResetClearsHashes)

BugResetKeepsTimeoutConfig ==
  ImplementationActions(ResetClearsTimingAndConfig) =
    SpecActions(ResetClearsTimingAndConfig)

BugResetKeepsCounters ==
  ImplementationActions(ResetClearsCountersAndQueues) =
    SpecActions(ResetClearsCountersAndQueues)

BugTimeoutConfigNotStored ==
  ImplementationActions(SetTimeoutStoresMs) =
    SpecActions(SetTimeoutStoresMs)

BugStartIdentityDropped ==
  ImplementationActions(StartStoresActiveIdentity) =
    SpecActions(StartStoresActiveIdentity)

BugStartHashStartedDropped ==
  ImplementationActions(StartStoresHashAndStarted) =
    SpecActions(StartStoresHashAndStarted)

BugStartPauseNotIncremented ==
  ImplementationActions(StartIncrementsPause) =
    SpecActions(StartIncrementsPause)

BugStartPauseDepthMissing ==
  ImplementationActions(StartCapturesPauseDepth) =
    SpecActions(StartCapturesPauseDepth)

BugStartDoesNotOverwrite ==
  ImplementationActions(StartOverwritesPrevious) =
    SpecActions(StartOverwritesPrevious)

BugFinishNoActiveMutates ==
  ImplementationActions(FinishNoActiveNoop) =
    SpecActions(FinishNoActiveNoop)

BugFinishWrongIdMutates ==
  ImplementationActions(FinishWrongIdNoop) =
    SpecActions(FinishWrongIdNoop)

BugFinishKeepsIdentity ==
  ImplementationActions(FinishMatchingClearsIdentity) =
    SpecActions(FinishMatchingClearsIdentity)

BugFinishResumeNotIncremented ==
  ImplementationActions(FinishMatchingIncrementsResume) =
    SpecActions(FinishMatchingIncrementsResume)

BugFinishResumeDepthMissing ==
  ImplementationActions(FinishCapturesResumeDepth) =
    SpecActions(FinishCapturesResumeDepth)

BugFinishClearsTimeoutHistory ==
  ImplementationActions(FinishPreservesTimeoutHistory) =
    SpecActions(FinishPreservesTimeoutHistory)

BugElapsedActiveZero ==
  ImplementationActions(SnapshotElapsedActive) =
    SpecActions(SnapshotElapsedActive)

BugElapsedInactiveNonzero ==
  ImplementationActions(SnapshotElapsedInactiveZero) =
    SpecActions(SnapshotElapsedInactiveZero)

BugTimeoutCounterMissing ==
  ImplementationActions(TimeoutRecordsCounterAndElapsed) =
    SpecActions(TimeoutRecordsCounterAndElapsed)

BugTimeoutElapsedMissing ==
  ImplementationActions(TimeoutRecordsCounterAndElapsed) =
    SpecActions(TimeoutRecordsCounterAndElapsed)

BugTimeoutRoundMissing ==
  ImplementationActions(TimeoutRecordsRoundHashTimestamp) =
    SpecActions(TimeoutRecordsRoundHashTimestamp)

BugTimeoutHashDropped ==
  ImplementationActions(TimeoutRecordsRoundHashTimestamp) =
    SpecActions(TimeoutRecordsRoundHashTimestamp)

BugTimeoutTimestampZero ==
  ImplementationActions(TimeoutRecordsRoundHashTimestamp) =
    SpecActions(TimeoutRecordsRoundHashTimestamp)

BugSnapshotDropsInflight ==
  ImplementationActions(TopLevelSnapshotIncludesInflight) =
    SpecActions(TopLevelSnapshotIncludesInflight)

BugJsonCoreMismatch ==
  ImplementationActions(JsonProjectsCoreFields) =
    SpecActions(JsonProjectsCoreFields)

BugJsonTimeoutMismatch ==
  ImplementationActions(JsonProjectsTimeoutFields) =
    SpecActions(JsonProjectsTimeoutFields)

BugJsonQueueDepthMismatch ==
  ImplementationActions(JsonProjectsQueueFields) =
    SpecActions(JsonProjectsQueueFields)

BugTypedCoreMismatch ==
  ImplementationActions(TypedProjectsAllFields) =
    SpecActions(TypedProjectsAllFields)

BugTypedTimeoutMismatch ==
  ImplementationActions(TypedProjectsAllFields) =
    SpecActions(TypedProjectsAllFields)

BugTypedQueueDepthMismatch ==
  ImplementationActions(TypedProjectsAllFields) =
    SpecActions(TypedProjectsAllFields)

AllCommitInflightStatusCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetAnchors ==
  /\ ResetIdentityActions \subseteq
       ImplementationActions(ResetClearsActiveIdentity)
  /\ ResetHashActions \subseteq ImplementationActions(ResetClearsHashes)
  /\ ResetTimingActions \subseteq
       ImplementationActions(ResetClearsTimingAndConfig)
  /\ ResetCounterQueueActions \subseteq
       ImplementationActions(ResetClearsCountersAndQueues)

TimeoutConfigAnchors ==
  SetTimeoutMs \in ImplementationActions(SetTimeoutStoresMs)

StartLifecycleAnchors ==
  /\ StartIdentityActions \subseteq
       ImplementationActions(StartStoresActiveIdentity)
  /\ StartHashStartedActions \subseteq
       ImplementationActions(StartStoresHashAndStarted)
  /\ PauseTotalIncrement \in ImplementationActions(StartIncrementsPause)
  /\ PausedSinceSet \in ImplementationActions(StartIncrementsPause)
  /\ PauseDepthSnapshot \in ImplementationActions(StartCapturesPauseDepth)
  /\ StartIdentityActions \subseteq ImplementationActions(StartOverwritesPrevious)
  /\ StartHashStartedActions \subseteq
       ImplementationActions(StartOverwritesPrevious)
  /\ OverwritePrevious \in ImplementationActions(StartOverwritesPrevious)

FinishNoopAnchors ==
  /\ FinishChecksActive \in ImplementationActions(FinishNoActiveNoop)
  /\ FinishChecksActive \in ImplementationActions(FinishWrongIdNoop)
  /\ FinishChecksId \in ImplementationActions(FinishWrongIdNoop)
  /\ ~(FinishClearsActive \in ImplementationActions(FinishNoActiveNoop))
  /\ ~(FinishClearsIdentity \in ImplementationActions(FinishWrongIdNoop))

FinishMatchingAnchors ==
  /\ FinishChecksActive \in ImplementationActions(FinishMatchingClearsIdentity)
  /\ FinishChecksId \in ImplementationActions(FinishMatchingClearsIdentity)
  /\ FinishClearIdentityActions \subseteq
       ImplementationActions(FinishMatchingClearsIdentity)
  /\ ResumeTotalIncrement \in
       ImplementationActions(FinishMatchingIncrementsResume)
  /\ ResumeDepthSnapshot \in
       ImplementationActions(FinishCapturesResumeDepth)
  /\ PreserveTimeoutHistory \in
       ImplementationActions(FinishPreservesTimeoutHistory)

ElapsedSnapshotAnchors ==
  /\ StartActive \in ImplementationActions(SnapshotElapsedActive)
  /\ StartStarted \in ImplementationActions(SnapshotElapsedActive)
  /\ ElapsedActiveSaturating \in
       ImplementationActions(SnapshotElapsedActive)
  /\ ElapsedInactiveZero \in
       ImplementationActions(SnapshotElapsedInactiveZero)

TimeoutRecordAnchors ==
  /\ TimeoutTotalIncrement \in
       ImplementationActions(TimeoutRecordsCounterAndElapsed)
  /\ TimeoutElapsedStored \in
       ImplementationActions(TimeoutRecordsCounterAndElapsed)
  /\ TimeoutRoundStored \in
       ImplementationActions(TimeoutRecordsRoundHashTimestamp)
  /\ TimeoutHashStored \in
       ImplementationActions(TimeoutRecordsRoundHashTimestamp)
  /\ TimeoutTimestampStored \in
       ImplementationActions(TimeoutRecordsRoundHashTimestamp)

ProjectionAnchors ==
  /\ SnapshotIncludesInflight \in
       ImplementationActions(TopLevelSnapshotIncludesInflight)
  /\ SnapshotIncludesInflight \in ImplementationActions(JsonProjectsCoreFields)
  /\ JsonAllActions \subseteq
       (ImplementationActions(JsonProjectsCoreFields) \cup
        ImplementationActions(JsonProjectsTimeoutFields) \cup
        ImplementationActions(JsonProjectsQueueFields))
  /\ SnapshotIncludesInflight \in ImplementationActions(TypedProjectsAllFields)
  /\ TypedAllActions \subseteq ImplementationActions(TypedProjectsAllFields)

SafetyAnchors ==
  /\ AllCommitInflightStatusCandidatesMatchSpec
  /\ ResetAnchors
  /\ TimeoutConfigAnchors
  /\ StartLifecycleAnchors
  /\ FinishNoopAnchors
  /\ FinishMatchingAnchors
  /\ ElapsedSnapshotAnchors
  /\ TimeoutRecordAnchors
  /\ ProjectionAnchors

=============================================================================
