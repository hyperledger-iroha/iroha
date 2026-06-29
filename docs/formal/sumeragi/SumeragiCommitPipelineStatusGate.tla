---- MODULE SumeragiCommitPipelineStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-pipeline status accounting.

This slice captures `record_commit_pipeline_sample(...)`, the
`commit_pipeline_snapshot()` projection, `TimingEma` initialization/update
behavior for the fields that have EMAs, and the test-only
`reset_commit_pipeline_status_for_tests()` helper from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
FirstSampleStoresCoreFields == 2
FirstSampleStoresDrainFields == 3
FirstSampleInitializesEmaFields == 4
SecondSampleOverwritesLastFields == 5
SecondSampleBlendsEmaFields == 6
QcRebuildHasNoEma == 7
DrainFieldsHaveNoEma == 8
SnapshotProjectsCoreFields == 9
SnapshotProjectsDrainFields == 10
SnapshotProjectsEmaFields == 11
ResetAfterRecordsClears == 12

Candidates == 1..12

ResetSnapshot == 1
ResetEmaState == 2
LastTotalStored == 3
LastValidationStored == 4
LastQcRebuildStored == 5
LastGateStored == 6
LastFinalizeStored == 7
LastDrainResultsStored == 8
LastDrainQcVerifyStored == 9
LastDrainPersistStored == 10
LastDrainKuraStoreStored == 11
LastDrainStateApplyStored == 12
LastDrainStateCommitStored == 13
FirstEmaTotalEqualsSample == 14
FirstEmaValidationEqualsSample == 15
FirstEmaGateEqualsSample == 16
FirstEmaFinalizeEqualsSample == 17
SecondEmaBlended == 18
QcRebuildNoEma == 19
DrainFieldsNoEma == 20
LastFieldsOverwritten == 21
SnapshotCoreMatch == 22
SnapshotDrainMatch == 23
SnapshotEmaMatch == 24

Actions == 1..24

AllResetActions == {ResetSnapshot, ResetEmaState}

AllCoreLastActions ==
  {LastTotalStored, LastValidationStored, LastQcRebuildStored, LastGateStored,
   LastFinalizeStored}

AllDrainLastActions ==
  {LastDrainResultsStored, LastDrainQcVerifyStored, LastDrainPersistStored,
   LastDrainKuraStoreStored, LastDrainStateApplyStored,
   LastDrainStateCommitStored}

AllLastActions == AllCoreLastActions \cup AllDrainLastActions

AllFirstEmaActions ==
  {FirstEmaTotalEqualsSample, FirstEmaValidationEqualsSample,
   FirstEmaGateEqualsSample, FirstEmaFinalizeEqualsSample}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = FirstSampleStoresCoreFields ->
      AllCoreLastActions \cup {SnapshotCoreMatch}
    [] candidate = FirstSampleStoresDrainFields ->
      AllDrainLastActions \cup {SnapshotDrainMatch}
    [] candidate = FirstSampleInitializesEmaFields ->
      AllFirstEmaActions \cup {SnapshotEmaMatch}
    [] candidate = SecondSampleOverwritesLastFields ->
      AllLastActions \cup {LastFieldsOverwritten, SnapshotCoreMatch,
       SnapshotDrainMatch}
    [] candidate = SecondSampleBlendsEmaFields ->
      {SecondEmaBlended, SnapshotEmaMatch}
    [] candidate = QcRebuildHasNoEma ->
      {LastQcRebuildStored, QcRebuildNoEma}
    [] candidate = DrainFieldsHaveNoEma ->
      AllDrainLastActions \cup {DrainFieldsNoEma}
    [] candidate = SnapshotProjectsCoreFields ->
      AllCoreLastActions \cup {SnapshotCoreMatch}
    [] candidate = SnapshotProjectsDrainFields ->
      AllDrainLastActions \cup {SnapshotDrainMatch}
    [] candidate = SnapshotProjectsEmaFields ->
      AllFirstEmaActions \cup {SnapshotEmaMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_snapshot" ->
      spec \ {ResetSnapshot}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_ema" ->
      spec \ {ResetEmaState}
    [] candidate = FirstSampleStoresCoreFields /\ Bug = "total_not_stored" ->
      spec \ {LastTotalStored}
    [] candidate = FirstSampleStoresCoreFields /\
          Bug = "validation_not_stored" ->
      spec \ {LastValidationStored}
    [] candidate = FirstSampleStoresCoreFields /\
          Bug = "qc_rebuild_not_stored" ->
      spec \ {LastQcRebuildStored}
    [] candidate = FirstSampleStoresCoreFields /\ Bug = "gate_not_stored" ->
      spec \ {LastGateStored}
    [] candidate = FirstSampleStoresCoreFields /\
          Bug = "finalize_not_stored" ->
      spec \ {LastFinalizeStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_results_not_stored" ->
      spec \ {LastDrainResultsStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_qc_not_stored" ->
      spec \ {LastDrainQcVerifyStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_persist_not_stored" ->
      spec \ {LastDrainPersistStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_kura_not_stored" ->
      spec \ {LastDrainKuraStoreStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_state_apply_not_stored" ->
      spec \ {LastDrainStateApplyStored}
    [] candidate = FirstSampleStoresDrainFields /\
          Bug = "drain_state_commit_not_stored" ->
      spec \ {LastDrainStateCommitStored}
    [] candidate = FirstSampleInitializesEmaFields /\
          Bug = "first_ema_total_not_initialized" ->
      spec \ {FirstEmaTotalEqualsSample}
    [] candidate = FirstSampleInitializesEmaFields /\
          Bug = "first_ema_validation_not_initialized" ->
      spec \ {FirstEmaValidationEqualsSample}
    [] candidate = FirstSampleInitializesEmaFields /\
          Bug = "first_ema_gate_not_initialized" ->
      spec \ {FirstEmaGateEqualsSample}
    [] candidate = FirstSampleInitializesEmaFields /\
          Bug = "first_ema_finalize_not_initialized" ->
      spec \ {FirstEmaFinalizeEqualsSample}
    [] candidate = SecondSampleOverwritesLastFields /\
          Bug = "second_sample_keeps_old_last" ->
      spec \ {LastFieldsOverwritten}
    [] candidate = SecondSampleBlendsEmaFields /\
          Bug = "second_ema_overwrites_without_blend" ->
      spec \ {SecondEmaBlended}
    [] candidate = QcRebuildHasNoEma /\ Bug = "qc_rebuild_updates_ema" ->
      spec \ {QcRebuildNoEma}
    [] candidate = DrainFieldsHaveNoEma /\ Bug = "drain_fields_update_ema" ->
      spec \ {DrainFieldsNoEma}
    [] candidate = SnapshotProjectsCoreFields /\
          Bug = "snapshot_core_mismatch" ->
      spec \ {SnapshotCoreMatch}
    [] candidate = SnapshotProjectsDrainFields /\
          Bug = "snapshot_drain_mismatch" ->
      spec \ {SnapshotDrainMatch}
    [] candidate = SnapshotProjectsEmaFields /\ Bug = "snapshot_ema_mismatch" ->
      spec \ {SnapshotEmaMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_snapshot" ->
      spec \ {ResetSnapshot}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_ema" ->
      spec \ {ResetEmaState}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 12
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..12

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

CommitPipelineStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

CommitPipelineStatusExactness ==
  /\ CommitPipelineStatusActionsMatchSpec

CommitPipelineStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CommitPipelineStatusExactness

BugResetEmptyKeepsSnapshot ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsEma ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugTotalNotStored ==
  ImplementationActions(FirstSampleStoresCoreFields) =
    SpecActions(FirstSampleStoresCoreFields)

BugValidationNotStored ==
  ImplementationActions(FirstSampleStoresCoreFields) =
    SpecActions(FirstSampleStoresCoreFields)

BugQcRebuildNotStored ==
  ImplementationActions(FirstSampleStoresCoreFields) =
    SpecActions(FirstSampleStoresCoreFields)

BugGateNotStored ==
  ImplementationActions(FirstSampleStoresCoreFields) =
    SpecActions(FirstSampleStoresCoreFields)

BugFinalizeNotStored ==
  ImplementationActions(FirstSampleStoresCoreFields) =
    SpecActions(FirstSampleStoresCoreFields)

BugDrainResultsNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugDrainQcNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugDrainPersistNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugDrainKuraNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugDrainStateApplyNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugDrainStateCommitNotStored ==
  ImplementationActions(FirstSampleStoresDrainFields) =
    SpecActions(FirstSampleStoresDrainFields)

BugFirstEmaTotalNotInitialized ==
  ImplementationActions(FirstSampleInitializesEmaFields) =
    SpecActions(FirstSampleInitializesEmaFields)

BugFirstEmaValidationNotInitialized ==
  ImplementationActions(FirstSampleInitializesEmaFields) =
    SpecActions(FirstSampleInitializesEmaFields)

BugFirstEmaGateNotInitialized ==
  ImplementationActions(FirstSampleInitializesEmaFields) =
    SpecActions(FirstSampleInitializesEmaFields)

BugFirstEmaFinalizeNotInitialized ==
  ImplementationActions(FirstSampleInitializesEmaFields) =
    SpecActions(FirstSampleInitializesEmaFields)

BugSecondSampleKeepsOldLast ==
  ImplementationActions(SecondSampleOverwritesLastFields) =
    SpecActions(SecondSampleOverwritesLastFields)

BugSecondEmaOverwritesWithoutBlend ==
  ImplementationActions(SecondSampleBlendsEmaFields) =
    SpecActions(SecondSampleBlendsEmaFields)

BugQcRebuildUpdatesEma ==
  ImplementationActions(QcRebuildHasNoEma) =
    SpecActions(QcRebuildHasNoEma)

BugDrainFieldsUpdateEma ==
  ImplementationActions(DrainFieldsHaveNoEma) =
    SpecActions(DrainFieldsHaveNoEma)

BugSnapshotCoreMismatch ==
  ImplementationActions(SnapshotProjectsCoreFields) =
    SpecActions(SnapshotProjectsCoreFields)

BugSnapshotDrainMismatch ==
  ImplementationActions(SnapshotProjectsDrainFields) =
    SpecActions(SnapshotProjectsDrainFields)

BugSnapshotEmaMismatch ==
  ImplementationActions(SnapshotProjectsEmaFields) =
    SpecActions(SnapshotProjectsEmaFields)

BugResetAfterRecordsKeepsSnapshot ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsEma ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
