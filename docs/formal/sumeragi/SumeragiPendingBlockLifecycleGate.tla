---- MODULE SumeragiPendingBlockLifecycleGate ----
EXTENDS FiniteSets, Naturals

(***************************************************************************
A bounded abstract model for PendingBlock lifecycle helpers.

This slice captures the field-reset contracts in
`PendingBlock::new(...)`, `replace_block(...)`,
`replace_block_with_payload_bytes(...)`, `revive_after_abort(...)`,
`revive_after_abort_with_payload_bytes(...)`, `retire_same_height(...)`,
`mark_aborted(...)`, and `refresh_retired_payload_with_payload_bytes(...)`.
The model abstracts concrete blocks, hashes, timestamps, and OnceLock payload
storage into observable lifecycle actions: subject/payload replacement,
active/aborted/retired accessors, commit-stage reset or preservation,
validation/artifact/root cleanup, DA-gate cleanup, Kura retry cleanup,
scheduler cleanup, commit-evidence replay cleanup, and progress-window refresh.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewDefaults == "new_defaults"
ReplaceSame == "replace_same_subject"
ReplaceSamePayload == "replace_same_subject_payload"
ReplaceDiff == "replace_diff_subject"
ReplaceDiffPayload == "replace_diff_subject_payload"
Revive == "revive_after_abort"
RevivePayload == "revive_after_abort_payload"
Retire == "retire_same_height"
Abort == "mark_aborted"
RefreshRetired == "refresh_retired_payload"

Cases == {
  NewDefaults,
  ReplaceSame,
  ReplaceSamePayload,
  ReplaceDiff,
  ReplaceDiffPayload,
  Revive,
  RevivePayload,
  Retire,
  Abort,
  RefreshRetired
}

Active == 1
Inactive == 2
RetryAborted == 3
RetiredSameHeight == 4
NotRetired == 5
SubjectUpdated == 6
CanonicalPayloadReset == 7
SeededPayloadInstalled == 8
PreserveLifecycle == 9
StageAwaiting == 10
CommitStagePreserved == 11
QcEpochCleared == 12
ValidationPending == 13
ArtifactCleared == 14
RootsCleared == 15
GatesCleared == 16
RetryCleared == 17
KuraPersistedFalse == 18
KuraPersistedPreserved == 19
NetworkRetryTimersCleared == 20
QuorumRescheduleCleared == 21
EvidenceReplayCleared == 22
ProgressRefreshed == 23

Actions == 1..23

ConstructorReset ==
  {Active, NotRetired, StageAwaiting, QcEpochCleared, ValidationPending,
   ArtifactCleared, RootsCleared, GatesCleared, RetryCleared,
   KuraPersistedFalse, NetworkRetryTimersCleared, QuorumRescheduleCleared,
   EvidenceReplayCleared, ProgressRefreshed}

DifferentSubjectReset ==
  ConstructorReset \cup {SubjectUpdated}

ReviveReset ==
  (ConstructorReset \ {KuraPersistedFalse})
    \cup {SubjectUpdated, KuraPersistedPreserved}

RetiredReset ==
  {Inactive, RetiredSameHeight, CommitStagePreserved, GatesCleared,
   RetryCleared, NetworkRetryTimersCleared, QuorumRescheduleCleared,
   EvidenceReplayCleared, ProgressRefreshed}

AbortReset ==
  {Inactive, RetryAborted, NotRetired, StageAwaiting, QcEpochCleared,
   ArtifactCleared, RootsCleared, GatesCleared, RetryCleared,
   NetworkRetryTimersCleared, EvidenceReplayCleared, ProgressRefreshed}

RefreshRetiredReset ==
  {Inactive, RetiredSameHeight, SubjectUpdated, SeededPayloadInstalled,
   CommitStagePreserved, GatesCleared, NetworkRetryTimersCleared,
   QuorumRescheduleCleared, EvidenceReplayCleared, ProgressRefreshed}

SpecActions(c) ==
  CASE c = NewDefaults -> ConstructorReset
    [] c = ReplaceSame ->
       {SubjectUpdated, CanonicalPayloadReset, PreserveLifecycle}
    [] c = ReplaceSamePayload ->
       {SubjectUpdated, SeededPayloadInstalled, PreserveLifecycle}
    [] c = ReplaceDiff ->
       DifferentSubjectReset \cup {CanonicalPayloadReset}
    [] c = ReplaceDiffPayload ->
       DifferentSubjectReset \cup {SeededPayloadInstalled}
    [] c = Revive ->
       ReviveReset \cup {CanonicalPayloadReset}
    [] c = RevivePayload ->
       ReviveReset \cup {SeededPayloadInstalled}
    [] c = Retire -> RetiredReset
    [] c = Abort -> AbortReset
    [] c = RefreshRetired -> RefreshRetiredReset
    [] OTHER -> {}

FullResetCases == {
  NewDefaults,
  ReplaceDiff,
  ReplaceDiffPayload,
  Revive,
  RevivePayload
}

ReplaceDifferentCases == {ReplaceDiff, ReplaceDiffPayload}
ReviveCases == {Revive, RevivePayload}
ReplaceSameCases == {ReplaceSame, ReplaceSamePayload}

LifecycleResetFields ==
  {StageAwaiting, QcEpochCleared, ValidationPending, ArtifactCleared,
   RootsCleared, GatesCleared, RetryCleared, NetworkRetryTimersCleared,
   QuorumRescheduleCleared, EvidenceReplayCleared, ProgressRefreshed}

SchedulerFields == {NetworkRetryTimersCleared, QuorumRescheduleCleared}
CommitMaterialFields == {ArtifactCleared, RootsCleared}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "new_not_pending" /\ c = NewDefaults ->
      spec \ {ValidationPending}
    [] Bug = "new_sets_inactive" /\ c = NewDefaults ->
      (spec \ {Active}) \cup {Inactive, RetryAborted}
    [] Bug = "replace_same_resets_lifecycle" /\ c \in ReplaceSameCases ->
      (spec \ {PreserveLifecycle}) \cup {StageAwaiting, GatesCleared,
        RetryCleared}
    [] Bug = "replace_same_keeps_stale_payload" /\ c \in ReplaceSameCases ->
      spec \ {CanonicalPayloadReset, SeededPayloadInstalled}
    [] Bug = "replace_diff_preserves_old_lifecycle"
       /\ c \in ReplaceDifferentCases ->
      (spec \ LifecycleResetFields) \cup {PreserveLifecycle}
    [] Bug = "replace_diff_keeps_gates"
       /\ c \in ReplaceDifferentCases ->
      spec \ {GatesCleared}
    [] Bug = "replace_diff_keeps_roots_artifact"
       /\ c \in ReplaceDifferentCases ->
      spec \ CommitMaterialFields
    [] Bug = "replace_diff_keeps_kura_persisted"
       /\ c \in ReplaceDifferentCases ->
      (spec \ {KuraPersistedFalse}) \cup {KuraPersistedPreserved}
    [] Bug = "replace_diff_keeps_schedulers"
       /\ c \in ReplaceDifferentCases ->
      spec \ SchedulerFields
    [] Bug = "revive_keeps_aborted" /\ c \in ReviveCases ->
      (spec \ {Active}) \cup {Inactive, RetryAborted}
    [] Bug = "revive_clears_kura_persisted" /\ c \in ReviveCases ->
      (spec \ {KuraPersistedPreserved}) \cup {KuraPersistedFalse}
    [] Bug = "revive_keeps_validation_artifact" /\ c \in ReviveCases ->
      spec \ {ValidationPending, ArtifactCleared}
    [] Bug = "retire_not_inactive" /\ c = Retire ->
      spec \ {Inactive}
    [] Bug = "retire_not_retired" /\ c = Retire ->
      spec \ {RetiredSameHeight}
    [] Bug = "retire_resets_commit_stage" /\ c = Retire ->
      (spec \ {CommitStagePreserved}) \cup {StageAwaiting, QcEpochCleared}
    [] Bug = "retire_keeps_gate" /\ c = Retire ->
      spec \ {GatesCleared}
    [] Bug = "abort_not_retry_aborted" /\ c = Abort ->
      spec \ {RetryAborted}
    [] Bug = "abort_keeps_commit_stage" /\ c = Abort ->
      spec \ {StageAwaiting, QcEpochCleared}
    [] Bug = "abort_keeps_roots_artifact" /\ c = Abort ->
      spec \ CommitMaterialFields
    [] Bug = "abort_keeps_gate" /\ c = Abort ->
      spec \ {GatesCleared}
    [] Bug = "abort_keeps_retry_state" /\ c = Abort ->
      spec \ {RetryCleared}
    [] Bug = "refresh_retired_not_retired" /\ c = RefreshRetired ->
      spec \ {RetiredSameHeight}
    [] Bug = "refresh_retired_not_inactive" /\ c = RefreshRetired ->
      spec \ {Inactive}
    [] Bug = "refresh_retired_resets_commit_stage" /\ c = RefreshRetired ->
      (spec \ {CommitStagePreserved}) \cup {StageAwaiting, QcEpochCleared}
    [] Bug = "refresh_retired_keeps_schedulers" /\ c = RefreshRetired ->
      spec \ SchedulerFields
    [] OTHER -> spec

Bugs == {
  "none",
  "new_not_pending",
  "new_sets_inactive",
  "replace_same_resets_lifecycle",
  "replace_same_keeps_stale_payload",
  "replace_diff_preserves_old_lifecycle",
  "replace_diff_keeps_gates",
  "replace_diff_keeps_roots_artifact",
  "replace_diff_keeps_kura_persisted",
  "replace_diff_keeps_schedulers",
  "revive_keeps_aborted",
  "revive_clears_kura_persisted",
  "revive_keeps_validation_artifact",
  "retire_not_inactive",
  "retire_not_retired",
  "retire_resets_commit_stage",
  "retire_keeps_gate",
  "abort_not_retry_aborted",
  "abort_keeps_commit_stage",
  "abort_keeps_roots_artifact",
  "abort_keeps_gate",
  "abort_keeps_retry_state",
  "refresh_retired_not_retired",
  "refresh_retired_not_inactive",
  "refresh_retired_resets_commit_stage",
  "refresh_retired_keeps_schedulers"
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

PendingBlockConstructorExact ==
  ImplementationActions(NewDefaults) = SpecActions(NewDefaults)

PendingBlockSameSubjectReplacementExact ==
  \A c \in ReplaceSameCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockDifferentSubjectReplacementExact ==
  \A c \in ReplaceDifferentCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockReviveExact ==
  \A c \in ReviveCases:
    ImplementationActions(c) = SpecActions(c)

PendingBlockRetireAbortExact ==
  /\ ImplementationActions(Retire) = SpecActions(Retire)
  /\ ImplementationActions(Abort) = SpecActions(Abort)

PendingBlockRetiredPayloadRefreshExact ==
  ImplementationActions(RefreshRetired) = SpecActions(RefreshRetired)

PendingBlockLifecycleExactness ==
  /\ PendingBlockConstructorExact
  /\ PendingBlockSameSubjectReplacementExact
  /\ PendingBlockDifferentSubjectReplacementExact
  /\ PendingBlockReviveExact
  /\ PendingBlockRetireAbortExact
  /\ PendingBlockRetiredPayloadRefreshExact

PendingBlockLifecycleCoreSafety ==
  PendingBlockLifecycleExactness

PendingBlockLifecycleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PendingBlockLifecycleExactness

NoBugInvariant == PendingBlockLifecycleCoreSafety

SafetyFast == PendingBlockLifecycleCorrectnessEnvelope

BugNewNotPending == NoBugInvariant
BugNewSetsInactive == NoBugInvariant
BugReplaceSameResetsLifecycle == NoBugInvariant
BugReplaceSameKeepsStalePayload == NoBugInvariant
BugReplaceDiffPreservesOldLifecycle == NoBugInvariant
BugReplaceDiffKeepsGates == NoBugInvariant
BugReplaceDiffKeepsRootsArtifact == NoBugInvariant
BugReplaceDiffKeepsKuraPersisted == NoBugInvariant
BugReplaceDiffKeepsSchedulers == NoBugInvariant
BugReviveKeepsAborted == NoBugInvariant
BugReviveClearsKuraPersisted == NoBugInvariant
BugReviveKeepsValidationArtifact == NoBugInvariant
BugRetireNotInactive == NoBugInvariant
BugRetireNotRetired == NoBugInvariant
BugRetireResetsCommitStage == NoBugInvariant
BugRetireKeepsGate == NoBugInvariant
BugAbortNotRetryAborted == NoBugInvariant
BugAbortKeepsCommitStage == NoBugInvariant
BugAbortKeepsRootsArtifact == NoBugInvariant
BugAbortKeepsGate == NoBugInvariant
BugAbortKeepsRetryState == NoBugInvariant
BugRefreshRetiredNotRetired == NoBugInvariant
BugRefreshRetiredNotInactive == NoBugInvariant
BugRefreshRetiredResetsCommitStage == NoBugInvariant
BugRefreshRetiredKeepsSchedulers == NoBugInvariant

====
