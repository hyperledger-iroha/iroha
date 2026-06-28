---- MODULE SumeragiEnginePrepareLockHighestGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact Prepare-QC lock/highest-QC recording.

This slice models the `qc_ref_from_certificate(...)`,
`state.locked_qc = Some(qc)`, and `record_highest_qc(...)` side effects in
`ConsensusEngine::on_prepare_qc(...)`, including the shared
`on_certificate(...)` prefilter and the prepare-vote replay/conflict guard.
Accepted Prepare QCs must lock exactly the derived Prepare QC. They also
record that exact QC as highest when no current highest QC exists or when the
derived Prepare QC improves the stored highest-QC value. Rejected Prepare QCs,
replayed/conflicting same-round Prepare QCs, and pending-finality returns
preserve both stored lock and highest-QC state exactly.
***************************************************************************)

CONSTANTS
  \* @type: Bool;
  BugSkipLockRecord,
  \* @type: Bool;
  BugRecordWrongLock,
  \* @type: Bool;
  BugLockOnRejected,
  \* @type: Bool;
  BugClearLockOnRejected,
  \* @type: Bool;
  BugLockOnReplayConflictPending,
  \* @type: Bool;
  BugClearLockOnReplayConflictPending,
  \* @type: Bool;
  BugSkipNoCurrentRecord,
  \* @type: Bool;
  BugSkipImprovingRecord,
  \* @type: Bool;
  BugRecordWrongHighest,
  \* @type: Bool;
  BugOverwriteLowerHighest,
  \* @type: Bool;
  BugRecordOnRejected,
  \* @type: Bool;
  BugClearOnRejected,
  \* @type: Bool;
  BugRecordOnReplayConflictPending,
  \* @type: Bool;
  BugClearOnReplayConflictPending

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_no_current",
  "safe_improves_height",
  "safe_improves_view",
  "safe_improves_subject",
  "safe_lower_height",
  "safe_lower_phase",
  "safe_equal",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "replay_same_prepare",
  "conflicting_prepare",
  "pending_finality"
}

QcValues == {
  "none",
  "qc_locked_current",
  "qc_highest_current",
  "qc_lower_height",
  "qc_lower_view",
  "qc_prepare_lower_subject",
  "qc_commit_same_slot",
  "qc_prepare_a",
  "qc_prepare_b",
  "qc_wrong"
}

Accepted(candidate) ==
  candidate \in {
    "safe_no_current",
    "safe_improves_height",
    "safe_improves_view",
    "safe_improves_subject",
    "safe_lower_height",
    "safe_lower_phase",
    "safe_equal"
  }

RejectedPrefilter(candidate) ==
  candidate \in {
    "wrong_height",
    "wrong_epoch",
    "wrong_validator_set",
    "wrong_quorum_policy",
    "stale_view",
    "committed_height"
  }

ReplayConflictPending(candidate) ==
  candidate \in {
    "replay_same_prepare",
    "conflicting_prepare",
    "pending_finality"
  }

InitialLocked(candidate) ==
  IF candidate = "safe_no_current"
  THEN "none"
  ELSE "qc_locked_current"

InitialHighest(candidate) ==
  CASE candidate = "safe_no_current" -> "none"
    [] candidate = "safe_improves_height" -> "qc_lower_height"
    [] candidate = "safe_improves_view" -> "qc_lower_view"
    [] candidate = "safe_improves_subject" -> "qc_prepare_lower_subject"
    [] candidate = "safe_lower_height" -> "qc_highest_current"
    [] candidate = "safe_lower_phase" -> "qc_commit_same_slot"
    [] candidate = "safe_equal" -> "qc_prepare_a"
    [] OTHER -> "qc_highest_current"

DerivedPrepareQc(candidate) ==
  IF candidate = "conflicting_prepare"
  THEN "qc_prepare_b"
  ELSE "qc_prepare_a"

NoCurrentHighest(candidate) ==
  InitialHighest(candidate) = "none"

ImprovingHighest(candidate) ==
  candidate \in {
    "safe_improves_height",
    "safe_improves_view",
    "safe_improves_subject"
  }

LowerOrEqualHighest(candidate) ==
  candidate \in {
    "safe_lower_height",
    "safe_lower_phase",
    "safe_equal"
  }

SpecFinalLocked(candidate) ==
  IF Accepted(candidate)
  THEN DerivedPrepareQc(candidate)
  ELSE InitialLocked(candidate)

SpecFinalHighest(candidate) ==
  IF /\ Accepted(candidate)
     /\ (NoCurrentHighest(candidate) \/ ImprovingHighest(candidate))
  THEN DerivedPrepareQc(candidate)
  ELSE InitialHighest(candidate)

ImplementationAcceptedLocked(candidate) ==
  IF BugSkipLockRecord
  THEN InitialLocked(candidate)
  ELSE IF BugRecordWrongLock
       THEN "qc_wrong"
       ELSE DerivedPrepareQc(candidate)

ImplementationIgnoredLocked(candidate) ==
  IF RejectedPrefilter(candidate)
  THEN
    IF BugClearLockOnRejected
    THEN "none"
    ELSE IF BugLockOnRejected
         THEN DerivedPrepareQc(candidate)
         ELSE InitialLocked(candidate)
  ELSE IF ReplayConflictPending(candidate)
       THEN
         IF BugClearLockOnReplayConflictPending
         THEN "none"
         ELSE IF BugLockOnReplayConflictPending
              THEN DerivedPrepareQc(candidate)
              ELSE InitialLocked(candidate)
       ELSE InitialLocked(candidate)

ImplementationFinalLocked(candidate) ==
  IF Accepted(candidate)
  THEN ImplementationAcceptedLocked(candidate)
  ELSE ImplementationIgnoredLocked(candidate)

ImplementationAcceptedHighest(candidate) ==
  IF NoCurrentHighest(candidate)
  THEN
    IF BugSkipNoCurrentRecord
    THEN "none"
    ELSE IF BugRecordWrongHighest
         THEN "qc_wrong"
         ELSE DerivedPrepareQc(candidate)
  ELSE IF ImprovingHighest(candidate)
       THEN
         IF BugSkipImprovingRecord
         THEN InitialHighest(candidate)
         ELSE IF BugRecordWrongHighest
              THEN "qc_wrong"
              ELSE DerivedPrepareQc(candidate)
       ELSE IF LowerOrEqualHighest(candidate)
            THEN
              IF BugOverwriteLowerHighest
              THEN DerivedPrepareQc(candidate)
              ELSE IF BugRecordWrongHighest
                   THEN "qc_wrong"
                   ELSE InitialHighest(candidate)
            ELSE InitialHighest(candidate)

ImplementationIgnoredHighest(candidate) ==
  IF RejectedPrefilter(candidate)
  THEN
    IF BugClearOnRejected
    THEN "none"
    ELSE IF BugRecordOnRejected
         THEN DerivedPrepareQc(candidate)
         ELSE InitialHighest(candidate)
  ELSE IF ReplayConflictPending(candidate)
       THEN
         IF BugClearOnReplayConflictPending
         THEN "none"
         ELSE IF BugRecordOnReplayConflictPending
              THEN DerivedPrepareQc(candidate)
              ELSE InitialHighest(candidate)
       ELSE InitialHighest(candidate)

ImplementationFinalHighest(candidate) ==
  IF Accepted(candidate)
  THEN ImplementationAcceptedHighest(candidate)
  ELSE ImplementationIgnoredHighest(candidate)

TypeInvariant ==
  /\ BugSkipLockRecord \in BOOLEAN
  /\ BugRecordWrongLock \in BOOLEAN
  /\ BugLockOnRejected \in BOOLEAN
  /\ BugClearLockOnRejected \in BOOLEAN
  /\ BugLockOnReplayConflictPending \in BOOLEAN
  /\ BugClearLockOnReplayConflictPending \in BOOLEAN
  /\ BugSkipNoCurrentRecord \in BOOLEAN
  /\ BugSkipImprovingRecord \in BOOLEAN
  /\ BugRecordWrongHighest \in BOOLEAN
  /\ BugOverwriteLowerHighest \in BOOLEAN
  /\ BugRecordOnRejected \in BOOLEAN
  /\ BugClearOnRejected \in BOOLEAN
  /\ BugRecordOnReplayConflictPending \in BOOLEAN
  /\ BugClearOnReplayConflictPending \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialLocked(candidate) \in QcValues
    /\ InitialHighest(candidate) \in QcValues
    /\ DerivedPrepareQc(candidate) \in QcValues
    /\ SpecFinalLocked(candidate) \in QcValues
    /\ SpecFinalHighest(candidate) \in QcValues
    /\ ImplementationFinalLocked(candidate) \in QcValues
    /\ ImplementationFinalHighest(candidate) \in QcValues

Init ==
  tried = {}

TryCandidate(candidate) ==
  /\ candidate \in Cases \ tried
  /\ tried' = tried \cup {candidate}

Stable ==
  UNCHANGED vars

Next ==
  \/ \E candidate \in Cases: TryCandidate(candidate)
  \/ Stable

FinalLockedMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalLocked(candidate) = SpecFinalLocked(candidate)

FinalHighestMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalHighest(candidate) = SpecFinalHighest(candidate)

AcceptedPrepareLocksExactDerivedQc ==
  \A candidate \in tried:
    Accepted(candidate) =>
      ImplementationFinalLocked(candidate) = DerivedPrepareQc(candidate)

AcceptedNoCurrentHighestRecordsExactPrepareQc ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ NoCurrentHighest(candidate)
    => ImplementationFinalHighest(candidate) = DerivedPrepareQc(candidate)

AcceptedImprovingHighestRecordsExactPrepareQc ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ ImprovingHighest(candidate)
    => ImplementationFinalHighest(candidate) = DerivedPrepareQc(candidate)

AcceptedLowerOrEqualHighestPreservesStoredHighest ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ LowerOrEqualHighest(candidate)
    => ImplementationFinalHighest(candidate) = InitialHighest(candidate)

RejectedPrefilterPreservesStoredLockAndHighest ==
  \A candidate \in tried:
    RejectedPrefilter(candidate) =>
      /\ ImplementationFinalLocked(candidate) = InitialLocked(candidate)
      /\ ImplementationFinalHighest(candidate) = InitialHighest(candidate)

ReplayConflictPendingPreservesStoredLockAndHighest ==
  \A candidate \in tried:
    ReplayConflictPending(candidate) =>
      /\ ImplementationFinalLocked(candidate) = InitialLocked(candidate)
      /\ ImplementationFinalHighest(candidate) = InitialHighest(candidate)

IgnoredPrepareQcsNeverRecord ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      /\ ImplementationFinalLocked(candidate) = InitialLocked(candidate)
      /\ ImplementationFinalHighest(candidate) = InitialHighest(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialLocked(candidate) \in QcValues
    /\ InitialHighest(candidate) \in QcValues
    /\ DerivedPrepareQc(candidate) \in QcValues
    /\ SpecFinalLocked(candidate) \in QcValues
    /\ SpecFinalHighest(candidate) \in QcValues
    /\ ImplementationFinalLocked(candidate) \in QcValues
    /\ ImplementationFinalHighest(candidate) \in QcValues

EnginePrepareLockHighestExactness ==
  /\ FinalLockedMatchesSpec
  /\ FinalHighestMatchesSpec
  /\ AcceptedPrepareLocksExactDerivedQc
  /\ AcceptedNoCurrentHighestRecordsExactPrepareQc
  /\ AcceptedImprovingHighestRecordsExactPrepareQc
  /\ AcceptedLowerOrEqualHighestPreservesStoredHighest
  /\ RejectedPrefilterPreservesStoredLockAndHighest
  /\ ReplayConflictPendingPreservesStoredLockAndHighest
  /\ IgnoredPrepareQcsNeverRecord
  /\ ValuesStayInDomain

Safety ==
  EnginePrepareLockHighestExactness

EnginePrepareLockHighestCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EnginePrepareLockHighestExactness

SafetyFast == EnginePrepareLockHighestExactness

====
