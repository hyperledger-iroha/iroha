---- MODULE SumeragiTxQueueBackpressureStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for transaction queue backpressure status.

This slice captures `set_tx_queue_backpressure(...)`,
`tx_queue_backpressure()`, and the `snapshot().tx_queue_*` status fields. It
pins the status-only contract: depth and capacity start at zero, saturation
starts false, healthy samples store the latest depth and capacity while clearing
the saturated flag, saturated samples store the latest depth and capacity while
setting the flag, getter and snapshot projections expose all three fields
exactly, and saturation is an explicit state rather than an inference from
depth/capacity equality.
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
HealthyStoresDepthCapacity == 2
HealthyClearsSaturated == 3
SaturatedStoresDepthCapacity == 4
SaturatedSetsSaturated == 5
GetterProjectsHealthy == 6
GetterProjectsSaturated == 7
SnapshotProjectsHealthy == 8
SnapshotProjectsSaturated == 9
HealthyToSaturatedOverwrite == 10
SaturatedToHealthyOverwrite == 11
DistinctDepthCapacityPreserved == 12
SaturationStateExplicit == 13

Candidates == 1..13

InitialDepthZero == 1
InitialCapacityZero == 2
InitialSaturatedFalse == 3
HealthyDepthStored == 4
HealthyCapacityStored == 5
HealthySaturatedFalse == 6
SaturatedDepthStored == 7
SaturatedCapacityStored == 8
SaturatedSaturatedTrue == 9
GetterDepthMatches == 10
GetterCapacityMatches == 11
GetterSaturatedMatches == 12
SnapshotDepthMatches == 13
SnapshotCapacityMatches == 14
SnapshotSaturatedMatches == 15
HealthyToSaturatedDepthLatest == 16
HealthyToSaturatedCapacityLatest == 17
HealthyToSaturatedFlagLatest == 18
SaturatedToHealthyDepthLatest == 19
SaturatedToHealthyCapacityLatest == 20
SaturatedToHealthyFlagCleared == 21
DistinctDepthCapacityKept == 22
HealthyFullStillClear == 23
SaturatedNonFullStillSet == 24

Actions == 1..24

InitialActions ==
  {InitialDepthZero, InitialCapacityZero, InitialSaturatedFalse}

HealthyStoreActions == {HealthyDepthStored, HealthyCapacityStored}

SaturatedStoreActions == {SaturatedDepthStored, SaturatedCapacityStored}

GetterActions ==
  {GetterDepthMatches, GetterCapacityMatches, GetterSaturatedMatches}

SnapshotActions ==
  {SnapshotDepthMatches, SnapshotCapacityMatches, SnapshotSaturatedMatches}

SpecActions(candidate) ==
  CASE candidate = InitialZero ->
      InitialActions
    [] candidate = HealthyStoresDepthCapacity ->
      HealthyStoreActions
    [] candidate = HealthyClearsSaturated ->
      {HealthySaturatedFalse}
    [] candidate = SaturatedStoresDepthCapacity ->
      SaturatedStoreActions
    [] candidate = SaturatedSetsSaturated ->
      {SaturatedSaturatedTrue}
    [] candidate = GetterProjectsHealthy ->
      GetterActions
    [] candidate = GetterProjectsSaturated ->
      GetterActions
    [] candidate = SnapshotProjectsHealthy ->
      SnapshotActions
    [] candidate = SnapshotProjectsSaturated ->
      SnapshotActions
    [] candidate = HealthyToSaturatedOverwrite ->
      SaturatedStoreActions \cup
        {HealthyToSaturatedDepthLatest, HealthyToSaturatedCapacityLatest,
         HealthyToSaturatedFlagLatest}
    [] candidate = SaturatedToHealthyOverwrite ->
      HealthyStoreActions \cup
        {SaturatedToHealthyDepthLatest, SaturatedToHealthyCapacityLatest,
         SaturatedToHealthyFlagCleared}
    [] candidate = DistinctDepthCapacityPreserved ->
      {DistinctDepthCapacityKept}
    [] candidate = SaturationStateExplicit ->
      {HealthyFullStillClear, SaturatedNonFullStillSet}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = InitialZero /\ Bug = "initial_depth_nonzero" ->
      spec \ {InitialDepthZero}
    [] candidate = InitialZero /\ Bug = "initial_capacity_nonzero" ->
      spec \ {InitialCapacityZero}
    [] candidate = InitialZero /\ Bug = "initial_saturated_true" ->
      spec \ {InitialSaturatedFalse}
    [] candidate \in {HealthyStoresDepthCapacity, SaturatedToHealthyOverwrite} /\
          Bug = "healthy_depth_not_stored" ->
      spec \ {HealthyDepthStored}
    [] candidate \in {HealthyStoresDepthCapacity, SaturatedToHealthyOverwrite} /\
          Bug = "healthy_capacity_not_stored" ->
      spec \ {HealthyCapacityStored}
    [] candidate = HealthyClearsSaturated /\
          Bug = "healthy_keeps_saturated" ->
      spec \ {HealthySaturatedFalse}
    [] candidate \in {SaturatedStoresDepthCapacity, HealthyToSaturatedOverwrite} /\
          Bug = "saturated_depth_not_stored" ->
      spec \ {SaturatedDepthStored}
    [] candidate \in {SaturatedStoresDepthCapacity, HealthyToSaturatedOverwrite} /\
          Bug = "saturated_capacity_not_stored" ->
      spec \ {SaturatedCapacityStored}
    [] candidate = SaturatedSetsSaturated /\
          Bug = "saturated_flag_not_set" ->
      spec \ {SaturatedSaturatedTrue}
    [] candidate \in {GetterProjectsHealthy, GetterProjectsSaturated} /\
          Bug = "getter_depth_mismatch" ->
      spec \ {GetterDepthMatches}
    [] candidate \in {GetterProjectsHealthy, GetterProjectsSaturated} /\
          Bug = "getter_capacity_mismatch" ->
      spec \ {GetterCapacityMatches}
    [] candidate \in {GetterProjectsHealthy, GetterProjectsSaturated} /\
          Bug = "getter_saturation_mismatch" ->
      spec \ {GetterSaturatedMatches}
    [] candidate \in {SnapshotProjectsHealthy, SnapshotProjectsSaturated} /\
          Bug = "snapshot_depth_mismatch" ->
      spec \ {SnapshotDepthMatches}
    [] candidate \in {SnapshotProjectsHealthy, SnapshotProjectsSaturated} /\
          Bug = "snapshot_capacity_mismatch" ->
      spec \ {SnapshotCapacityMatches}
    [] candidate \in {SnapshotProjectsHealthy, SnapshotProjectsSaturated} /\
          Bug = "snapshot_saturation_mismatch" ->
      spec \ {SnapshotSaturatedMatches}
    [] candidate = HealthyToSaturatedOverwrite /\
          Bug = "healthy_to_saturated_overwrite_ignored" ->
      spec \ {HealthyToSaturatedDepthLatest, HealthyToSaturatedCapacityLatest,
              HealthyToSaturatedFlagLatest}
    [] candidate = SaturatedToHealthyOverwrite /\
          Bug = "saturated_to_healthy_overwrite_ignored" ->
      spec \ {SaturatedToHealthyDepthLatest, SaturatedToHealthyCapacityLatest}
    [] candidate = SaturatedToHealthyOverwrite /\
          Bug = "saturated_to_healthy_keeps_flag" ->
      spec \ {SaturatedToHealthyFlagCleared}
    [] candidate = DistinctDepthCapacityPreserved /\
          Bug = "depth_capacity_swapped" ->
      spec \ {DistinctDepthCapacityKept}
    [] candidate = SaturationStateExplicit /\
          Bug = "saturation_inferred_from_depth_eq_capacity" ->
      spec \ {HealthyFullStillClear, SaturatedNonFullStillSet}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 13
  /\ checked' = checked + 1

TypeInvariant ==
  /\ Bug \in {
       "none",
       "initial_depth_nonzero",
       "initial_capacity_nonzero",
       "initial_saturated_true",
       "healthy_depth_not_stored",
       "healthy_capacity_not_stored",
       "healthy_keeps_saturated",
       "saturated_depth_not_stored",
       "saturated_capacity_not_stored",
       "saturated_flag_not_set",
       "getter_depth_mismatch",
       "getter_capacity_mismatch",
       "getter_saturation_mismatch",
       "snapshot_depth_mismatch",
       "snapshot_capacity_mismatch",
       "snapshot_saturation_mismatch",
       "healthy_to_saturated_overwrite_ignored",
       "saturated_to_healthy_overwrite_ignored",
       "saturated_to_healthy_keeps_flag",
       "depth_capacity_swapped",
       "saturation_inferred_from_depth_eq_capacity"
     }
  /\ checked \in 0..13
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

TxQueueBackpressureStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

TxQueueBackpressureStatusExactness ==
  /\ TxQueueBackpressureStatusActionsMatchSpec

TxQueueBackpressureStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ TxQueueBackpressureStatusExactness

BugInitialDepthNonzero ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugInitialCapacityNonzero ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugInitialSaturatedTrue ==
  ImplementationActions(InitialZero) = SpecActions(InitialZero)

BugHealthyDepthNotStored ==
  ImplementationActions(HealthyStoresDepthCapacity) =
    SpecActions(HealthyStoresDepthCapacity)

BugHealthyCapacityNotStored ==
  ImplementationActions(HealthyStoresDepthCapacity) =
    SpecActions(HealthyStoresDepthCapacity)

BugHealthyKeepsSaturated ==
  ImplementationActions(HealthyClearsSaturated) =
    SpecActions(HealthyClearsSaturated)

BugSaturatedDepthNotStored ==
  ImplementationActions(SaturatedStoresDepthCapacity) =
    SpecActions(SaturatedStoresDepthCapacity)

BugSaturatedCapacityNotStored ==
  ImplementationActions(SaturatedStoresDepthCapacity) =
    SpecActions(SaturatedStoresDepthCapacity)

BugSaturatedFlagNotSet ==
  ImplementationActions(SaturatedSetsSaturated) =
    SpecActions(SaturatedSetsSaturated)

BugGetterDepthMismatch ==
  ImplementationActions(GetterProjectsHealthy) =
    SpecActions(GetterProjectsHealthy)

BugGetterCapacityMismatch ==
  ImplementationActions(GetterProjectsHealthy) =
    SpecActions(GetterProjectsHealthy)

BugGetterSaturationMismatch ==
  ImplementationActions(GetterProjectsHealthy) =
    SpecActions(GetterProjectsHealthy)

BugSnapshotDepthMismatch ==
  ImplementationActions(SnapshotProjectsHealthy) =
    SpecActions(SnapshotProjectsHealthy)

BugSnapshotCapacityMismatch ==
  ImplementationActions(SnapshotProjectsHealthy) =
    SpecActions(SnapshotProjectsHealthy)

BugSnapshotSaturationMismatch ==
  ImplementationActions(SnapshotProjectsHealthy) =
    SpecActions(SnapshotProjectsHealthy)

BugHealthyToSaturatedOverwriteIgnored ==
  ImplementationActions(HealthyToSaturatedOverwrite) =
    SpecActions(HealthyToSaturatedOverwrite)

BugSaturatedToHealthyOverwriteIgnored ==
  ImplementationActions(SaturatedToHealthyOverwrite) =
    SpecActions(SaturatedToHealthyOverwrite)

BugSaturatedToHealthyKeepsFlag ==
  ImplementationActions(SaturatedToHealthyOverwrite) =
    SpecActions(SaturatedToHealthyOverwrite)

BugDepthCapacitySwapped ==
  ImplementationActions(DistinctDepthCapacityPreserved) =
    SpecActions(DistinctDepthCapacityPreserved)

BugSaturationInferredFromDepthEqCapacity ==
  ImplementationActions(SaturationStateExplicit) =
    SpecActions(SaturationStateExplicit)

====
