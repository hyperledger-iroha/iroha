---- MODULE SumeragiValidationGate ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
A bounded abstract model for Sumeragi asynchronous proposal-validation safety.

The pure engine accepts a proposal, records the exact block subject currently
being validated, and later receives an asynchronous validation result. Only the
current in-flight validation result may advance the view on failure. Unknown,
completed, replayed, or timeout-stale validation callbacks must be ignored.

This model abstracts block subjects to "A" and "B" and tracks one in-flight
validation at a time. It intentionally focuses on callback ownership and view
advance safety; proposal lock safety and finality are covered by the other
Sumeragi formal slices in this directory.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  MaxView,
  \* @type: Bool;
  BugAdvanceUnknownValidation,
  \* @type: Bool;
  BugAdvanceCompletedReplay,
  \* @type: Bool;
  BugKeepInflightOnTimeout,
  \* @type: Bool;
  BugKeepInflightAfterInvalid

VARIABLES
  \* @type: Int;
  currentView,
  \* @type: Str;
  phase,
  \* @type: Str;
  validating,
  \* @type: Int;
  validationView,
  \* @type: Set(Str);
  completedSubjects,
  \* @type: Set(Str);
  timedOutSubjects,
  \* @type: Set(Str);
  invalidAdvanceSubjects,
  \* @type: Bool;
  unknownFailureAdvanced,
  \* @type: Bool;
  completedReplayAdvanced,
  \* @type: Bool;
  lateFailureAdvanced,
  \* @type: Bool;
  timeoutRetainedInflight,
  \* @type: Bool;
  duplicateInvalidAdvance

vars == <<
  currentView,
  phase,
  validating,
  validationView,
  completedSubjects,
  timedOutSubjects,
  invalidAdvanceSubjects,
  unknownFailureAdvanced,
  completedReplayAdvanced,
  lateFailureAdvanced,
  timeoutRetainedInflight,
  duplicateInvalidAdvance
>>

Subjects == {"A", "B"}
MaybeSubject == Subjects \cup {"None"}
Phases == {"Proposal", "Prepare"}

BadUnknownAdvanceEnabled ==
  BugAdvanceUnknownValidation /\ currentView < MaxView

BadCompletedAdvanceEnabled ==
  BugAdvanceCompletedReplay /\ currentView < MaxView

TypeInvariant ==
  /\ MaxView \in Nat
  /\ MaxView >= 2
  /\ BugAdvanceUnknownValidation \in BOOLEAN
  /\ BugAdvanceCompletedReplay \in BOOLEAN
  /\ BugKeepInflightOnTimeout \in BOOLEAN
  /\ BugKeepInflightAfterInvalid \in BOOLEAN
  /\ currentView \in 0..MaxView
  /\ phase \in Phases
  /\ validating \in MaybeSubject
  /\ validationView \in 0..MaxView
  /\ completedSubjects \subseteq Subjects
  /\ timedOutSubjects \subseteq Subjects
  /\ invalidAdvanceSubjects \subseteq Subjects
  /\ unknownFailureAdvanced \in BOOLEAN
  /\ completedReplayAdvanced \in BOOLEAN
  /\ lateFailureAdvanced \in BOOLEAN
  /\ timeoutRetainedInflight \in BOOLEAN
  /\ duplicateInvalidAdvance \in BOOLEAN
  /\ validating = "None" => validationView = 0

Init ==
  /\ currentView = 0
  /\ phase = "Proposal"
  /\ validating = "None"
  /\ validationView = 0
  /\ completedSubjects = {}
  /\ timedOutSubjects = {}
  /\ invalidAdvanceSubjects = {}
  /\ unknownFailureAdvanced = FALSE
  /\ completedReplayAdvanced = FALSE
  /\ lateFailureAdvanced = FALSE
  /\ timeoutRetainedInflight = FALSE
  /\ duplicateInvalidAdvance = FALSE

AcceptProposal(subject) ==
  /\ subject \in Subjects
  /\ phase = "Proposal"
  /\ validating = "None"
  /\ subject \notin completedSubjects
  /\ subject \notin timedOutSubjects
  /\ phase' = "Prepare"
  /\ validating' = subject
  /\ validationView' = currentView
  /\ UNCHANGED <<
      currentView,
      completedSubjects,
      timedOutSubjects,
      invalidAdvanceSubjects,
      unknownFailureAdvanced,
      completedReplayAdvanced,
      lateFailureAdvanced,
      timeoutRetainedInflight,
      duplicateInvalidAdvance
     >>

CurrentValidationSucceeds ==
  /\ validating \in Subjects
  /\ completedSubjects' = completedSubjects \cup {validating}
  /\ validating' = "None"
  /\ validationView' = 0
  /\ UNCHANGED <<
      currentView,
      phase,
      timedOutSubjects,
      invalidAdvanceSubjects,
      unknownFailureAdvanced,
      completedReplayAdvanced,
      lateFailureAdvanced,
      timeoutRetainedInflight,
      duplicateInvalidAdvance
     >>

CurrentValidationFails ==
  /\ validating \in Subjects
  /\ currentView < MaxView
  /\ currentView' = currentView + 1
  /\ phase' = "Proposal"
  /\ completedSubjects' = completedSubjects \cup {validating}
  /\ invalidAdvanceSubjects' = invalidAdvanceSubjects \cup {validating}
  /\ duplicateInvalidAdvance' =
      (duplicateInvalidAdvance \/ validating \in invalidAdvanceSubjects)
  /\ lateFailureAdvanced' =
      (lateFailureAdvanced \/ validationView < currentView)
  /\ IF BugKeepInflightAfterInvalid
     THEN
       /\ validating' = validating
       /\ validationView' = validationView
     ELSE
       /\ validating' = "None"
       /\ validationView' = 0
  /\ UNCHANGED <<
      timedOutSubjects,
      unknownFailureAdvanced,
      completedReplayAdvanced,
      timeoutRetainedInflight
     >>

TimeoutClearsOrRetainsInflight ==
  /\ validating \in Subjects
  /\ currentView < MaxView
  /\ currentView' = currentView + 1
  /\ phase' = "Proposal"
  /\ timedOutSubjects' = timedOutSubjects \cup {validating}
  /\ timeoutRetainedInflight' =
      (timeoutRetainedInflight \/ BugKeepInflightOnTimeout)
  /\ IF BugKeepInflightOnTimeout
     THEN
       /\ validating' = validating
       /\ validationView' = validationView
     ELSE
       /\ validating' = "None"
       /\ validationView' = 0
  /\ UNCHANGED <<
      completedSubjects,
      invalidAdvanceSubjects,
      unknownFailureAdvanced,
      completedReplayAdvanced,
      lateFailureAdvanced,
      duplicateInvalidAdvance
     >>

UnknownValidationFailure(subject) ==
  /\ subject \in Subjects
  /\ subject # validating
  /\ subject \notin completedSubjects
  /\ subject \notin timedOutSubjects
  /\ currentView' =
      IF BadUnknownAdvanceEnabled THEN currentView + 1 ELSE currentView
  /\ phase' =
      IF BadUnknownAdvanceEnabled THEN "Proposal" ELSE phase
  /\ unknownFailureAdvanced' =
      (unknownFailureAdvanced \/ BadUnknownAdvanceEnabled)
  /\ UNCHANGED <<
      validating,
      validationView,
      completedSubjects,
      timedOutSubjects,
      invalidAdvanceSubjects,
      completedReplayAdvanced,
      lateFailureAdvanced,
      timeoutRetainedInflight,
      duplicateInvalidAdvance
     >>

CompletedValidationReplay(subject) ==
  /\ subject \in completedSubjects
  /\ currentView' =
      IF BadCompletedAdvanceEnabled THEN currentView + 1 ELSE currentView
  /\ phase' =
      IF BadCompletedAdvanceEnabled THEN "Proposal" ELSE phase
  /\ completedReplayAdvanced' =
      (completedReplayAdvanced \/ BadCompletedAdvanceEnabled)
  /\ UNCHANGED <<
      validating,
      validationView,
      completedSubjects,
      timedOutSubjects,
      invalidAdvanceSubjects,
      unknownFailureAdvanced,
      lateFailureAdvanced,
      timeoutRetainedInflight,
      duplicateInvalidAdvance
     >>

LateValidationAfterTimeout(subject) ==
  /\ subject \in timedOutSubjects
  /\ currentView' =
      IF BadUnknownAdvanceEnabled THEN currentView + 1 ELSE currentView
  /\ phase' =
      IF BadUnknownAdvanceEnabled THEN "Proposal" ELSE phase
  /\ lateFailureAdvanced' =
      (lateFailureAdvanced \/ BadUnknownAdvanceEnabled)
  /\ UNCHANGED <<
      validating,
      validationView,
      completedSubjects,
      timedOutSubjects,
      invalidAdvanceSubjects,
      unknownFailureAdvanced,
      completedReplayAdvanced,
      timeoutRetainedInflight,
      duplicateInvalidAdvance
     >>

Stable ==
  UNCHANGED vars

Next ==
  \/ \E subject \in Subjects: AcceptProposal(subject)
  \/ CurrentValidationSucceeds
  \/ CurrentValidationFails
  \/ TimeoutClearsOrRetainsInflight
  \/ \E subject \in Subjects: UnknownValidationFailure(subject)
  \/ \E subject \in Subjects: CompletedValidationReplay(subject)
  \/ \E subject \in Subjects: LateValidationAfterTimeout(subject)
  \/ Stable

UnknownValidationDoesNotAdvance ==
  ~unknownFailureAdvanced

CompletedValidationReplayDoesNotAdvance ==
  ~completedReplayAdvanced

LateValidationAfterTimeoutDoesNotAdvance ==
  ~lateFailureAdvanced

TimeoutClearsInflight ==
  ~timeoutRetainedInflight

InvalidValidationAdvancesAtMostOnce ==
  ~duplicateInvalidAdvance

NoStaleInflightAfterViewAdvance ==
  validating = "None" \/ validationView = currentView

CompletedValidationClearsInflight ==
  validating = "None" \/ validating \notin completedSubjects

====
