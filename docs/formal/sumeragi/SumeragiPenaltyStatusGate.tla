---- MODULE SumeragiPenaltyStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi penalty status projection.

This slice captures the status helpers in `status.rs`: `set_vrf_penalties(...)`,
`set_vrf_late_reveals_total(...)`, `set_epoch_parameters(...)`,
`inc_consensus_penalties_applied(...)`, `inc_vrf_penalties_applied(...)`,
`set_penalties_pending(...)`, `vrf_penalty_snapshot()`, `penalty_counters()`,
and the corresponding `snapshot()` projection fields. Penalty attribution and
action execution are covered by the penalty selection/action gates; this gate
pins the operator-visible counters and scheduling parameters.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitialZeros == 1
VrfSnapshotRecord == 2
LateRevealUpdateOnly == 3
EpochParametersRecord == 4
ConsensusAppliedZeroIgnored == 5
ConsensusAppliedIncrements == 6
VrfAppliedZeroIgnored == 7
VrfAppliedIncrements == 8
PendingCountsRecord == 9
RepeatedConsensusAppliedAccumulates == 10
RepeatedVrfAppliedAccumulates == 11
PendingOverwriteLatest == 12
GetterProjectsPenaltyCounters == 13
StatusProjectsVrfSnapshot == 14
StatusProjectsEpochParameters == 15
StatusProjectsPenaltyCounters == 16

Candidates == 1..16

InitialVrfZero == 1
InitialEpochParamsZero == 2
InitialPenaltyCountersZero == 3
VrfEpochStored == 4
VrfNonRevealStored == 5
VrfNoParticipationStored == 6
VrfLateRevealsStored == 7
LateRevealUpdated == 8
LateRevealPreservesVrfContext == 9
EpochLengthStored == 10
EpochCommitOffsetStored == 11
EpochRevealOffsetStored == 12
ConsensusAppliedZeroIgnoredAction == 13
ConsensusAppliedIncremented == 14
VrfAppliedZeroIgnoredAction == 15
VrfAppliedIncremented == 16
ConsensusPendingStored == 17
VrfPendingStored == 18
ConsensusAppliedAccumulates == 19
VrfAppliedAccumulates == 20
PendingOverwriteApplies == 21
GetterConsensusAppliedMatches == 22
GetterConsensusPendingMatches == 23
GetterVrfAppliedMatches == 24
GetterVrfPendingMatches == 25
StatusVrfEpochMatches == 26
StatusVrfNonRevealMatches == 27
StatusVrfNoParticipationMatches == 28
StatusVrfLateRevealsMatches == 29
StatusEpochLengthMatches == 30
StatusEpochCommitOffsetMatches == 31
StatusEpochRevealOffsetMatches == 32
StatusConsensusAppliedMatches == 33
StatusConsensusPendingMatches == 34
StatusVrfAppliedMatches == 35
StatusVrfPendingMatches == 36

InitialActions ==
  {InitialVrfZero, InitialEpochParamsZero, InitialPenaltyCountersZero}
VrfStoreActions ==
  {VrfEpochStored, VrfNonRevealStored, VrfNoParticipationStored,
   VrfLateRevealsStored}
EpochStoreActions ==
  {EpochLengthStored, EpochCommitOffsetStored, EpochRevealOffsetStored}
PendingStoreActions ==
  {ConsensusPendingStored, VrfPendingStored}
GetterCounterActions ==
  {GetterConsensusAppliedMatches, GetterConsensusPendingMatches,
   GetterVrfAppliedMatches, GetterVrfPendingMatches}
StatusVrfActions ==
  {StatusVrfEpochMatches, StatusVrfNonRevealMatches,
   StatusVrfNoParticipationMatches, StatusVrfLateRevealsMatches}
StatusEpochActions ==
  {StatusEpochLengthMatches, StatusEpochCommitOffsetMatches,
   StatusEpochRevealOffsetMatches}
StatusPenaltyCounterActions ==
  {StatusConsensusAppliedMatches, StatusConsensusPendingMatches,
   StatusVrfAppliedMatches, StatusVrfPendingMatches}

SpecActions(candidate) ==
  CASE candidate = InitialZeros ->
      InitialActions
    [] candidate = VrfSnapshotRecord ->
      VrfStoreActions
    [] candidate = LateRevealUpdateOnly ->
      {LateRevealUpdated, LateRevealPreservesVrfContext}
    [] candidate = EpochParametersRecord ->
      EpochStoreActions
    [] candidate = ConsensusAppliedZeroIgnored ->
      {ConsensusAppliedZeroIgnoredAction}
    [] candidate = ConsensusAppliedIncrements ->
      {ConsensusAppliedIncremented}
    [] candidate = VrfAppliedZeroIgnored ->
      {VrfAppliedZeroIgnoredAction}
    [] candidate = VrfAppliedIncrements ->
      {VrfAppliedIncremented}
    [] candidate = PendingCountsRecord ->
      PendingStoreActions
    [] candidate = RepeatedConsensusAppliedAccumulates ->
      {ConsensusAppliedIncremented, ConsensusAppliedAccumulates}
    [] candidate = RepeatedVrfAppliedAccumulates ->
      {VrfAppliedIncremented, VrfAppliedAccumulates}
    [] candidate = PendingOverwriteLatest ->
      {PendingOverwriteApplies}
    [] candidate = GetterProjectsPenaltyCounters ->
      GetterCounterActions
    [] candidate = StatusProjectsVrfSnapshot ->
      StatusVrfActions
    [] candidate = StatusProjectsEpochParameters ->
      StatusEpochActions
    [] candidate = StatusProjectsPenaltyCounters ->
      StatusPenaltyCounterActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialZeros /\ Bug = "initial_counters_nonzero" ->
      spec \ {InitialVrfZero, InitialPenaltyCountersZero}
    [] candidate = VrfSnapshotRecord /\ Bug = "vrf_epoch_not_stored" ->
      spec \ {VrfEpochStored}
    [] candidate = VrfSnapshotRecord /\ Bug = "vrf_non_reveal_not_stored" ->
      spec \ {VrfNonRevealStored}
    [] candidate = VrfSnapshotRecord /\
          Bug = "vrf_no_participation_not_stored" ->
      spec \ {VrfNoParticipationStored}
    [] candidate = VrfSnapshotRecord /\
          Bug = "vrf_late_reveals_not_stored" ->
      spec \ {VrfLateRevealsStored}
    [] candidate = LateRevealUpdateOnly /\ Bug = "late_reveal_not_updated" ->
      spec \ {LateRevealUpdated}
    [] candidate = LateRevealUpdateOnly /\
          Bug = "late_reveal_update_drops_context" ->
      spec \ {LateRevealPreservesVrfContext}
    [] candidate = EpochParametersRecord /\
          Bug = "epoch_length_not_stored" ->
      spec \ {EpochLengthStored}
    [] candidate = EpochParametersRecord /\
          Bug = "epoch_commit_offset_not_stored" ->
      spec \ {EpochCommitOffsetStored}
    [] candidate = EpochParametersRecord /\
          Bug = "epoch_reveal_offset_not_stored" ->
      spec \ {EpochRevealOffsetStored}
    [] candidate = ConsensusAppliedZeroIgnored /\
          Bug = "consensus_applied_zero_counted" ->
      spec \ {ConsensusAppliedZeroIgnoredAction}
    [] candidate = ConsensusAppliedIncrements /\
          Bug = "consensus_applied_not_incremented" ->
      spec \ {ConsensusAppliedIncremented}
    [] candidate = VrfAppliedZeroIgnored /\
          Bug = "vrf_applied_zero_counted" ->
      spec \ {VrfAppliedZeroIgnoredAction}
    [] candidate = VrfAppliedIncrements /\
          Bug = "vrf_applied_not_incremented" ->
      spec \ {VrfAppliedIncremented}
    [] candidate = PendingCountsRecord /\
          Bug = "pending_consensus_not_stored" ->
      spec \ {ConsensusPendingStored}
    [] candidate = PendingCountsRecord /\ Bug = "pending_vrf_not_stored" ->
      spec \ {VrfPendingStored}
    [] candidate = RepeatedConsensusAppliedAccumulates /\
          Bug = "repeated_consensus_applied_overwrites" ->
      spec \ {ConsensusAppliedAccumulates}
    [] candidate = RepeatedVrfAppliedAccumulates /\
          Bug = "repeated_vrf_applied_overwrites" ->
      spec \ {VrfAppliedAccumulates}
    [] candidate = PendingOverwriteLatest /\
          Bug = "pending_overwrite_ignored" ->
      spec \ {PendingOverwriteApplies}
    [] candidate = GetterProjectsPenaltyCounters /\
          Bug = "getter_counters_mismatch" ->
      spec \ GetterCounterActions
    [] candidate = StatusProjectsVrfSnapshot /\ Bug = "status_vrf_mismatch" ->
      spec \ StatusVrfActions
    [] candidate = StatusProjectsEpochParameters /\
          Bug = "status_epoch_mismatch" ->
      spec \ StatusEpochActions
    [] candidate = StatusProjectsPenaltyCounters /\
          Bug = "status_penalty_counters_mismatch" ->
      spec \ StatusPenaltyCounterActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 16
     /\ checked' = checked + 1
  \/ /\ checked = 16
     /\ UNCHANGED vars

TypeInvariant ==
  checked \in 0..16

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

StatusStorageActionsMatchSpec ==
  \A candidate \in {
    InitialZeros,
    VrfSnapshotRecord,
    LateRevealUpdateOnly,
    EpochParametersRecord
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

PenaltyCounterActionsMatchSpec ==
  \A candidate \in {
    ConsensusAppliedZeroIgnored,
    ConsensusAppliedIncrements,
    VrfAppliedZeroIgnored,
    VrfAppliedIncrements,
    PendingCountsRecord,
    RepeatedConsensusAppliedAccumulates,
    RepeatedVrfAppliedAccumulates,
    PendingOverwriteLatest
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

PenaltyProjectionActionsMatchSpec ==
  \A candidate \in {
    GetterProjectsPenaltyCounters,
    StatusProjectsVrfSnapshot,
    StatusProjectsEpochParameters,
    StatusProjectsPenaltyCounters
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

InitialStatusAnchors ==
  /\ InitialVrfZero \in ImplementationActions(InitialZeros)
  /\ InitialEpochParamsZero \in ImplementationActions(InitialZeros)
  /\ InitialPenaltyCountersZero \in ImplementationActions(InitialZeros)

VrfSnapshotAnchors ==
  /\ VrfEpochStored \in ImplementationActions(VrfSnapshotRecord)
  /\ VrfNonRevealStored \in ImplementationActions(VrfSnapshotRecord)
  /\ VrfNoParticipationStored \in ImplementationActions(VrfSnapshotRecord)
  /\ VrfLateRevealsStored \in ImplementationActions(VrfSnapshotRecord)
  /\ LateRevealUpdated \in ImplementationActions(LateRevealUpdateOnly)
  /\ LateRevealPreservesVrfContext \in
       ImplementationActions(LateRevealUpdateOnly)

EpochParameterAnchors ==
  /\ EpochLengthStored \in ImplementationActions(EpochParametersRecord)
  /\ EpochCommitOffsetStored \in ImplementationActions(EpochParametersRecord)
  /\ EpochRevealOffsetStored \in ImplementationActions(EpochParametersRecord)

PenaltyCounterMutationAnchors ==
  /\ ConsensusAppliedZeroIgnoredAction \in
       ImplementationActions(ConsensusAppliedZeroIgnored)
  /\ ConsensusAppliedIncremented \in
       ImplementationActions(ConsensusAppliedIncrements)
  /\ VrfAppliedZeroIgnoredAction \in
       ImplementationActions(VrfAppliedZeroIgnored)
  /\ VrfAppliedIncremented \in ImplementationActions(VrfAppliedIncrements)
  /\ ConsensusPendingStored \in ImplementationActions(PendingCountsRecord)
  /\ VrfPendingStored \in ImplementationActions(PendingCountsRecord)
  /\ ConsensusAppliedAccumulates \in
       ImplementationActions(RepeatedConsensusAppliedAccumulates)
  /\ VrfAppliedAccumulates \in
       ImplementationActions(RepeatedVrfAppliedAccumulates)
  /\ PendingOverwriteApplies \in ImplementationActions(PendingOverwriteLatest)

PenaltyProjectionAnchors ==
  /\ GetterCounterActions \subseteq
       ImplementationActions(GetterProjectsPenaltyCounters)
  /\ StatusVrfActions \subseteq ImplementationActions(StatusProjectsVrfSnapshot)
  /\ StatusEpochActions \subseteq
       ImplementationActions(StatusProjectsEpochParameters)
  /\ StatusPenaltyCounterActions \subseteq
       ImplementationActions(StatusProjectsPenaltyCounters)

PenaltyStatusSafetyAnchors ==
  /\ StatusStorageActionsMatchSpec
  /\ PenaltyCounterActionsMatchSpec
  /\ PenaltyProjectionActionsMatchSpec
  /\ InitialStatusAnchors
  /\ VrfSnapshotAnchors
  /\ EpochParameterAnchors
  /\ PenaltyCounterMutationAnchors
  /\ PenaltyProjectionAnchors

BugInitialCountersNonzero ==
  ImplementationActions(InitialZeros) = SpecActions(InitialZeros)

BugVrfEpochNotStored ==
  ImplementationActions(VrfSnapshotRecord) = SpecActions(VrfSnapshotRecord)

BugVrfNonRevealNotStored ==
  ImplementationActions(VrfSnapshotRecord) = SpecActions(VrfSnapshotRecord)

BugVrfNoParticipationNotStored ==
  ImplementationActions(VrfSnapshotRecord) = SpecActions(VrfSnapshotRecord)

BugVrfLateRevealsNotStored ==
  ImplementationActions(VrfSnapshotRecord) = SpecActions(VrfSnapshotRecord)

BugLateRevealNotUpdated ==
  ImplementationActions(LateRevealUpdateOnly) =
    SpecActions(LateRevealUpdateOnly)

BugLateRevealUpdateDropsContext ==
  ImplementationActions(LateRevealUpdateOnly) =
    SpecActions(LateRevealUpdateOnly)

BugEpochLengthNotStored ==
  ImplementationActions(EpochParametersRecord) =
    SpecActions(EpochParametersRecord)

BugEpochCommitOffsetNotStored ==
  ImplementationActions(EpochParametersRecord) =
    SpecActions(EpochParametersRecord)

BugEpochRevealOffsetNotStored ==
  ImplementationActions(EpochParametersRecord) =
    SpecActions(EpochParametersRecord)

BugConsensusAppliedZeroCounted ==
  ImplementationActions(ConsensusAppliedZeroIgnored) =
    SpecActions(ConsensusAppliedZeroIgnored)

BugConsensusAppliedNotIncremented ==
  ImplementationActions(ConsensusAppliedIncrements) =
    SpecActions(ConsensusAppliedIncrements)

BugVrfAppliedZeroCounted ==
  ImplementationActions(VrfAppliedZeroIgnored) =
    SpecActions(VrfAppliedZeroIgnored)

BugVrfAppliedNotIncremented ==
  ImplementationActions(VrfAppliedIncrements) =
    SpecActions(VrfAppliedIncrements)

BugPendingConsensusNotStored ==
  ImplementationActions(PendingCountsRecord) =
    SpecActions(PendingCountsRecord)

BugPendingVrfNotStored ==
  ImplementationActions(PendingCountsRecord) =
    SpecActions(PendingCountsRecord)

BugRepeatedConsensusAppliedOverwrites ==
  ImplementationActions(RepeatedConsensusAppliedAccumulates) =
    SpecActions(RepeatedConsensusAppliedAccumulates)

BugRepeatedVrfAppliedOverwrites ==
  ImplementationActions(RepeatedVrfAppliedAccumulates) =
    SpecActions(RepeatedVrfAppliedAccumulates)

BugPendingOverwriteIgnored ==
  ImplementationActions(PendingOverwriteLatest) =
    SpecActions(PendingOverwriteLatest)

BugGetterCountersMismatch ==
  ImplementationActions(GetterProjectsPenaltyCounters) =
    SpecActions(GetterProjectsPenaltyCounters)

BugStatusVrfMismatch ==
  ImplementationActions(StatusProjectsVrfSnapshot) =
    SpecActions(StatusProjectsVrfSnapshot)

BugStatusEpochMismatch ==
  ImplementationActions(StatusProjectsEpochParameters) =
    SpecActions(StatusProjectsEpochParameters)

BugStatusPenaltyCountersMismatch ==
  ImplementationActions(StatusProjectsPenaltyCounters) =
    SpecActions(StatusProjectsPenaltyCounters)

=============================================================================
====
