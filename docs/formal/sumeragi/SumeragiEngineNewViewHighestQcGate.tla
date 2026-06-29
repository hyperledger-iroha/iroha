---- MODULE SumeragiEngineNewViewHighestQcGate ----
EXTENDS FiniteSets

(***************************************************************************
A bounded abstract model for exact NewView-QC highest-QC recording.

This slice models the `certificate.highest_qc` side effect in
`ConsensusEngine::on_new_view_qc(...)`, including the shared
`on_certificate(...)` prefilter. Accepted NewView certificates without a
carried highest QC preserve the current stored highest QC. Accepted compatible
NewView certificates with a carried highest QC record exactly that carried QC
only when it improves the stored value according to `record_highest_qc(...)`.
Rejected NewView certificates, including stale/same-view certificates,
incompatible carried highest-QC references, wrong round context, and wrong
quorum policy, preserve the stored highest-QC state exactly.
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
  BugClearOnNoHighest,
  \* @type: Bool;
  BugRecordWithoutHighest,
  \* @type: Bool;
  BugClearOnRejected,
  \* @type: Bool;
  BugRecordOnStale,
  \* @type: Bool;
  BugRecordOnIncompatible,
  \* @type: Bool;
  BugRecordOnWrongContext,
  \* @type: Bool;
  BugRecordOnWrongQuorum

VARIABLES
  \* @type: Set(Str);
  tried

\* @type: <<Set(Str)>>;
vars == <<tried>>

Cases == {
  "safe_no_highest_none",
  "safe_no_highest_existing",
  "safe_highest_no_current",
  "safe_highest_higher_height",
  "safe_highest_higher_view",
  "safe_highest_lower",
  "safe_highest_equal",
  "stale_highest_higher",
  "same_view_highest_higher",
  "incompatible_future_height",
  "incompatible_future_view",
  "incompatible_wrong_epoch",
  "wrong_context_highest_higher",
  "wrong_quorum_highest_higher"
}

QcValues == {
  "none",
  "qc_current",
  "qc_higher_height",
  "qc_higher_view",
  "qc_lower",
  "qc_incompatible_future_height",
  "qc_incompatible_future_view",
  "qc_incompatible_wrong_epoch",
  "qc_wrong"
}

Accepted(candidate) ==
  candidate \in {
    "safe_no_highest_none",
    "safe_no_highest_existing",
    "safe_highest_no_current",
    "safe_highest_higher_height",
    "safe_highest_higher_view",
    "safe_highest_lower",
    "safe_highest_equal"
  }

StaleOrSameView(candidate) ==
  candidate \in {"stale_highest_higher", "same_view_highest_higher"}

IncompatibleHighest(candidate) ==
  candidate \in {
    "incompatible_future_height",
    "incompatible_future_view",
    "incompatible_wrong_epoch"
  }

WrongContext(candidate) ==
  candidate = "wrong_context_highest_higher"

WrongQuorum(candidate) ==
  candidate = "wrong_quorum_highest_higher"

InitialHighest(candidate) ==
  IF candidate \in {"safe_no_highest_none", "safe_highest_no_current"}
  THEN "none"
  ELSE "qc_current"

CarriedHighest(candidate) ==
  CASE candidate \in {
      "safe_no_highest_none",
      "safe_no_highest_existing"
    } -> "none"
    [] candidate \in {
      "safe_highest_higher_height",
      "stale_highest_higher",
      "same_view_highest_higher",
      "wrong_context_highest_higher",
      "wrong_quorum_highest_higher"
    } -> "qc_higher_height"
    [] candidate = "safe_highest_higher_view" -> "qc_higher_view"
    [] candidate = "safe_highest_lower" -> "qc_lower"
    [] candidate = "incompatible_future_height" -> "qc_incompatible_future_height"
    [] candidate = "incompatible_future_view" -> "qc_incompatible_future_view"
    [] candidate = "incompatible_wrong_epoch" -> "qc_incompatible_wrong_epoch"
    [] OTHER -> "qc_current"

HasCarriedHighest(candidate) ==
  CarriedHighest(candidate) # "none"

NoCurrent(candidate) ==
  InitialHighest(candidate) = "none"

ImprovingHighest(candidate) ==
  candidate \in {
    "safe_highest_higher_height",
    "safe_highest_higher_view"
  }

LowerOrEqualHighest(candidate) ==
  candidate \in {"safe_highest_lower", "safe_highest_equal"}

SpecFinalHighest(candidate) ==
  IF /\ Accepted(candidate)
     /\ HasCarriedHighest(candidate)
     /\ (NoCurrent(candidate) \/ ImprovingHighest(candidate))
  THEN CarriedHighest(candidate)
  ELSE InitialHighest(candidate)

ImplementationAcceptedFinal(candidate) ==
  IF ~HasCarriedHighest(candidate)
  THEN
    IF BugClearOnNoHighest
    THEN "none"
    ELSE IF BugRecordWithoutHighest
         THEN "qc_wrong"
         ELSE InitialHighest(candidate)
  ELSE IF NoCurrent(candidate)
       THEN
         IF BugSkipNoCurrentRecord
         THEN "none"
         ELSE IF BugRecordWrongHighest
              THEN "qc_wrong"
              ELSE CarriedHighest(candidate)
       ELSE IF ImprovingHighest(candidate)
            THEN
              IF BugSkipImprovingRecord
              THEN InitialHighest(candidate)
              ELSE IF BugRecordWrongHighest
                   THEN "qc_wrong"
                   ELSE CarriedHighest(candidate)
            ELSE IF LowerOrEqualHighest(candidate)
                 THEN
                   IF BugOverwriteLowerHighest
                   THEN CarriedHighest(candidate)
                   ELSE IF BugRecordWrongHighest
                        THEN "qc_wrong"
                        ELSE InitialHighest(candidate)
                 ELSE InitialHighest(candidate)

ImplementationRejectedFinal(candidate) ==
  IF BugClearOnRejected
  THEN "none"
  ELSE IF StaleOrSameView(candidate) /\ BugRecordOnStale
       THEN CarriedHighest(candidate)
       ELSE IF IncompatibleHighest(candidate) /\ BugRecordOnIncompatible
            THEN CarriedHighest(candidate)
            ELSE IF WrongContext(candidate) /\ BugRecordOnWrongContext
                 THEN CarriedHighest(candidate)
                 ELSE IF WrongQuorum(candidate) /\ BugRecordOnWrongQuorum
                      THEN CarriedHighest(candidate)
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
  /\ BugClearOnNoHighest \in BOOLEAN
  /\ BugRecordWithoutHighest \in BOOLEAN
  /\ BugClearOnRejected \in BOOLEAN
  /\ BugRecordOnStale \in BOOLEAN
  /\ BugRecordOnIncompatible \in BOOLEAN
  /\ BugRecordOnWrongContext \in BOOLEAN
  /\ BugRecordOnWrongQuorum \in BOOLEAN
  /\ tried \subseteq Cases
  /\ \A candidate \in tried:
    /\ InitialHighest(candidate) \in QcValues
    /\ CarriedHighest(candidate) \in QcValues
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

AcceptedNoHighestPreservesStoredHighest ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ ~HasCarriedHighest(candidate)
    => ImplementationFinalHighest(candidate) = InitialHighest(candidate)

AcceptedImprovingHighestRecordsExactCarriedQc ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ HasCarriedHighest(candidate)
    /\ (NoCurrent(candidate) \/ ImprovingHighest(candidate))
    => ImplementationFinalHighest(candidate) = CarriedHighest(candidate)

AcceptedLowerOrEqualHighestPreservesStoredHighest ==
  \A candidate \in tried:
    /\ Accepted(candidate)
    /\ LowerOrEqualHighest(candidate)
    => ImplementationFinalHighest(candidate) = InitialHighest(candidate)

RejectedNewViewPreservesStoredHighest ==
  \A candidate \in tried:
    ~Accepted(candidate) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

IncompatibleHighestNeverRecords ==
  \A candidate \in tried:
    IncompatibleHighest(candidate) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

WrongContextNeverRecords ==
  \A candidate \in tried:
    (WrongContext(candidate) \/ WrongQuorum(candidate)) =>
      ImplementationFinalHighest(candidate) = InitialHighest(candidate)

ValuesStayInDomain ==
  \A candidate \in tried:
    /\ InitialHighest(candidate) \in QcValues
    /\ CarriedHighest(candidate) \in QcValues
    /\ SpecFinalHighest(candidate) \in QcValues
    /\ ImplementationFinalHighest(candidate) \in QcValues

EngineNewViewHighestQcExactness ==
  /\ FinalHighestMatchesSpec
  /\ AcceptedNoHighestPreservesStoredHighest
  /\ AcceptedImprovingHighestRecordsExactCarriedQc
  /\ AcceptedLowerOrEqualHighestPreservesStoredHighest
  /\ RejectedNewViewPreservesStoredHighest
  /\ IncompatibleHighestNeverRecords
  /\ WrongContextNeverRecords
  /\ ValuesStayInDomain

Safety ==
  EngineNewViewHighestQcExactness

EngineNewViewHighestQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ EngineNewViewHighestQcExactness

SafetyFast == EngineNewViewHighestQcExactness

====
