---- MODULE SumeragiEngineCommitQcHighestRecordGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact Commit-QC highest-QC recording.

This slice models the `qc_ref_from_certificate(...)` plus
`record_highest_qc(...)` side effect in
`ConsensusEngine::on_commit_qc(...)`, including the shared
`on_certificate(...)` prefilter. Accepted Commit QCs, whether the certified
payload is locally available or must be fetched, record exactly the derived
Commit QC reference only when it improves the stored highest-QC value or when
no highest QC is stored. Rejected Commit QCs, committed-height notifications,
and pending-finality replay/conflict returns preserve the stored highest-QC
state exactly.
***************************************************************************)

CONSTANTS
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
  BugRecordOnPendingReplay,
  \* @type: Bool;
  BugRecordOnPendingConflict,
  \* @type: Bool;
  BugClearOnPendingReplayConflict

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_available_no_current",
  "safe_missing_no_current",
  "safe_available_improves_height",
  "safe_missing_improves_phase",
  "safe_available_lower_height",
  "safe_missing_lower_subject",
  "safe_available_equal",
  "wrong_height",
  "wrong_epoch",
  "wrong_validator_set",
  "wrong_quorum_policy",
  "stale_view",
  "committed_height",
  "pending_replay",
  "pending_conflict"
}

QcValues == {
  "none",
  "qc_current",
  "qc_lower_height",
  "qc_prepare_same_slot",
  "qc_commit_higher_subject",
  "qc_commit_a",
  "qc_commit_b",
  "qc_wrong"
}

Accepted(candidate) ==
  candidate \in {
    "safe_available_no_current",
    "safe_missing_no_current",
    "safe_available_improves_height",
    "safe_missing_improves_phase",
    "safe_available_lower_height",
    "safe_missing_lower_subject",
    "safe_available_equal"
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

PendingReplay(candidate) ==
  candidate = "pending_replay"

PendingConflict(candidate) ==
  candidate = "pending_conflict"

InitialHighest(candidate) ==
  CASE candidate \in {
      "safe_available_no_current",
      "safe_missing_no_current"
    } -> "none"
    [] candidate = "safe_available_improves_height" -> "qc_lower_height"
    [] candidate = "safe_missing_improves_phase" -> "qc_prepare_same_slot"
    [] candidate = "safe_available_lower_height" -> "qc_current"
    [] candidate = "safe_missing_lower_subject" -> "qc_commit_higher_subject"
    [] candidate = "safe_available_equal" -> "qc_commit_a"
    [] OTHER -> "qc_current"

DerivedCommitQc(candidate) ==
  IF candidate = "pending_conflict"
  THEN "qc_commit_b"
  ELSE "qc_commit_a"

NoCurrent(candidate) ==
  InitialHighest(candidate) = "none"

Improving(candidate) ==
  candidate \in {
    "safe_available_improves_height",
    "safe_missing_improves_phase"
  }

LowerOrEqual(candidate) ==
  candidate \in {
    "safe_available_lower_height",
    "safe_missing_lower_subject",
    "safe_available_equal"
  }

SpecFinalHighest(candidate) ==
  IF /\ Accepted(candidate)
     /\ (NoCurrent(candidate) \/ Improving(candidate))
  THEN DerivedCommitQc(candidate)
  ELSE InitialHighest(candidate)

ImplementationAcceptedFinal(candidate) ==
  IF NoCurrent(candidate)
  THEN
    IF BugSkipNoCurrentRecord
    THEN "none"
    ELSE IF BugRecordWrongHighest
         THEN "qc_wrong"
         ELSE DerivedCommitQc(candidate)
  ELSE IF Improving(candidate)
       THEN
         IF BugSkipImprovingRecord
         THEN InitialHighest(candidate)
         ELSE IF BugRecordWrongHighest
              THEN "qc_wrong"
              ELSE DerivedCommitQc(candidate)
       ELSE IF LowerOrEqual(candidate)
            THEN
              IF BugOverwriteLowerHighest
              THEN DerivedCommitQc(candidate)
              ELSE IF BugRecordWrongHighest
                   THEN "qc_wrong"
                   ELSE InitialHighest(candidate)
            ELSE InitialHighest(candidate)

ImplementationRejectedFinal(candidate) ==
  IF BugClearOnRejected
  THEN "none"
  ELSE IF RejectedPrefilter(candidate) /\ BugRecordOnRejected
       THEN DerivedCommitQc(candidate)
       ELSE IF PendingReplay(candidate) /\ BugRecordOnPendingReplay
            THEN DerivedCommitQc(candidate)
            ELSE IF PendingConflict(candidate) /\ BugRecordOnPendingConflict
                 THEN DerivedCommitQc(candidate)
                 ELSE IF (PendingReplay(candidate) \/ PendingConflict(candidate)) /\
                         BugClearOnPendingReplayConflict
                      THEN "none"
                      ELSE InitialHighest(candidate)

ImplementationFinalHighest(candidate) ==
  IF Accepted(candidate)
  THEN ImplementationAcceptedFinal(candidate)
  ELSE ImplementationRejectedFinal(candidate)

TypeInvariant ==
  /\ BugSkipNoCurrentRecord \in BOOLEAN
  /\ BugSkipImprovingRecord \in BOOLEAN
  /\ BugRecordWrongHighest \in BOOLEAN
  /\ BugOverwriteLowerHighest \in BOOLEAN
  /\ BugRecordOnRejected \in BOOLEAN
  /\ BugClearOnRejected \in BOOLEAN
  /\ BugRecordOnPendingReplay \in BOOLEAN
  /\ BugRecordOnPendingConflict \in BOOLEAN
  /\ BugClearOnPendingReplayConflict \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialHighest(candidate) \in QcValues
    /\ DerivedCommitQc(candidate) \in QcValues
    /\ SpecFinalHighest(candidate) \in QcValues
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

FinalHighestMatchesSpec ==
  \A candidate \in tried:
    ImplementationFinalHighest(candidate) = SpecFinalHighest(candidate)

AcceptedNoCurrentRecordsExactCommitQc ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ NoCurrent(candidate)
    => ImplementationFinalHighest(candidate) = DerivedCommitQc(candidate)

AcceptedImprovingRecordsExactCommitQc ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ Improving(candidate)
    => ImplementationFinalHighest(candidate) = DerivedCommitQc(candidate)

AcceptedLowerOrEqualPreservesStoredHighest ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ LowerOrEqual(candidate)
    => ImplementationFinalHighest(candidate) = InitialHighest(candidate)

RejectedPrefilterPreservesStoredHighest ==
  \A candidate \in tried:
    RejectedPrefilter(candidate) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

PendingReplayConflictPreservesStoredHighest ==
  \A candidate \in tried:
    (PendingReplay(candidate) \/ PendingConflict(candidate)) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

IgnoredCommitQcsNeverRecord ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialHighest(candidate) \in QcValues
    /\ DerivedCommitQc(candidate) \in QcValues
    /\ SpecFinalHighest(candidate) \in QcValues
    /\ ImplementationFinalHighest(candidate) \in QcValues

Safety ==
  /\ FinalHighestMatchesSpec
  /\ AcceptedNoCurrentRecordsExactCommitQc
  /\ AcceptedImprovingRecordsExactCommitQc
  /\ AcceptedLowerOrEqualPreservesStoredHighest
  /\ RejectedPrefilterPreservesStoredHighest
  /\ PendingReplayConflictPreservesStoredHighest
  /\ IgnoredCommitQcsNeverRecord
  /\ ValuesStayInDomain

====
