---- MODULE SumeragiV2AppliedPhaseAdmissionMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded mutation model for evidence-bearing completion admission after reducer
apply.

The finite model deliberately covers the surviving phase for which the sealed
production Busy-owner regression retains complete callback evidence:

  BodyStored.

For an arbitrary active phase, the finite prefix is:

  1. a callback arrives while the reducer is Busy and moves from unseen to
     physically_owned with exactly one serviceable owner and one ordinal;
  2. an exact Busy retry coalesces with that owner;
  3. service applies the callback and retires physical ownership; and
  4. one adversarial probe retries the applied callback, conflicts in
     storage payload/owner evidence, supplies a malformed callback under a
     current or stale tag, or supplies a well-formed stale callback.

`step` and the `*Observed` variables are trace instrumentation only.  An exact
post-apply retry changes those trace variables so TLC can cover the edge, but
leaves every variable in `semanticVars` unchanged when
SuppressAppliedBeforeOrdinalAllocation is TRUE.

`callbackWellFormed` and `tagClass` describe only the step-four adversarial
probe and make the validation-order cross product explicit.  A malformed
callback fails closed under both current and stale tags.  A well-formed stale
callback is discarded before admission and is marker-free.  Looking at the
stale tag before validating the complete callback would hide the
malformed-plus-stale case; looking past a stale tag would turn well-formed
obsolete input into a current physical owner.

The production source seal separately retains a two-phase Rust regression for
exact post-apply suppression of BodyAvailable and BodyStored. This TLA+ matrix
makes no conflicting-evidence or Busy-owner claim for BodyAvailable.

This is a compact live-process admission obligation.  It does not claim a
crash/restart refinement, Byzantine wire validation, or a deductive proof of
the Rust implementation.
***************************************************************************)

CONSTANTS EnabledScenarios,
          SuppressAppliedBeforeOrdinalAllocation,
          ClearPhysicalOwnerOnApply,
          RejectConflictingEvidence,
          ValidateCallbackBeforeStaleTagCoalescing,
          CoalesceStaleTagBeforeAdmission

Phases == {"BodyStored"}

CurrentTag == "CurrentTag"
StaleTag == "StaleTag"
TagClasses == {CurrentTag, StaleTag}

Unseen == "unseen"
PhysicallyOwned == "physically_owned"
Applied == "applied"
PhaseStates == {Unseen, PhysicallyOwned, Applied}

ExactAppliedRetry == "ExactAppliedRetry"
ConflictEvidence == "ConflictEvidence"
ConflictOwner == "ConflictOwner"
MalformedCallbackCurrentTag == "MalformedCallbackCurrentTag"
MalformedCallbackStaleTag == "MalformedCallbackStaleTag"
WellFormedStaleTag == "WellFormedStaleTag"

ConflictScenarios == {ConflictEvidence, ConflictOwner}
MalformedCallbackScenarios ==
  {MalformedCallbackCurrentTag, MalformedCallbackStaleTag}
Scenarios ==
  {ExactAppliedRetry,
   ConflictEvidence,
   ConflictOwner,
   MalformedCallbackCurrentTag,
   MalformedCallbackStaleTag,
   WellFormedStaleTag}

ASSUME /\ EnabledScenarios \in (SUBSET Scenarios)
       /\ EnabledScenarios # {}
       /\ SuppressAppliedBeforeOrdinalAllocation \in BOOLEAN
       /\ ClearPhysicalOwnerOnApply \in BOOLEAN
       /\ RejectConflictingEvidence \in BOOLEAN
       /\ ValidateCallbackBeforeStaleTagCoalescing \in BOOLEAN
       /\ CoalesceStaleTagBeforeAdmission \in BOOLEAN

CanonicalPayload == "CanonicalPayload"
ConflictingPayload == "ConflictingPayload"
CanonicalOwner == "CanonicalOwner"
ForeignOwner == "ForeignOwner"

Payloads == {CanonicalPayload, ConflictingPayload}
Owners == {CanonicalOwner, ForeignOwner}

NoEvidence ==
  [phase |-> "NoPhase",
   payload |-> "NoPayload",
   owner |-> "NoOwner"]

EvidenceDomain ==
  { [phase |-> phase,
     payload |-> payload,
     owner |-> owner]:
       phase \in Phases,
       payload \in Payloads,
       owner \in Owners }
    \cup {NoEvidence}

CanonicalEvidence(phase) ==
  [phase |-> phase,
   payload |-> CanonicalPayload,
   owner |-> CanonicalOwner]

ConflictingEvidence(phase, conflictScenario) ==
  CASE conflictScenario = ConflictEvidence ->
         [phase |-> phase,
          payload |-> ConflictingPayload,
          owner |-> CanonicalOwner]
    [] conflictScenario = ConflictOwner ->
         [phase |-> phase,
          payload |-> CanonicalPayload,
          owner |-> ForeignOwner]
    [] OTHER -> NoEvidence

InitialAdmissionOrdinal == 1
OrdinalCeiling == 3

VARIABLES activePhase,
          scenario,
          callbackWellFormed,
          tagClass,
          step,
          phaseState,
          physicalOwnerCount,
          ownerServiceable,
          phaseEvidence,
          stalePhysicalOwnerCount,
          staleOwnerServiceable,
          stalePhaseEvidence,
          nextAdmissionOrdinal,
          queueInsertions,
          failClosed,
          busyRetryObserved,
          appliedRetryObserved,
          conflictObserved,
          malformedCallbackObserved,
          staleTagObserved

semanticVars ==
  <<callbackWellFormed,
    tagClass,
    phaseState,
    physicalOwnerCount,
    ownerServiceable,
    phaseEvidence,
    stalePhysicalOwnerCount,
    staleOwnerServiceable,
    stalePhaseEvidence,
    nextAdmissionOrdinal,
    queueInsertions,
    failClosed>>

vars ==
  <<activePhase,
    scenario,
    callbackWellFormed,
    tagClass,
    step,
    phaseState,
    physicalOwnerCount,
    ownerServiceable,
    phaseEvidence,
    stalePhysicalOwnerCount,
    staleOwnerServiceable,
    stalePhaseEvidence,
    nextAdmissionOrdinal,
    queueInsertions,
    failClosed,
    busyRetryObserved,
    appliedRetryObserved,
    conflictObserved,
    malformedCallbackObserved,
    staleTagObserved>>

TypeInvariant ==
  /\ activePhase \in Phases
  /\ scenario \in EnabledScenarios
  /\ callbackWellFormed \in BOOLEAN
  /\ tagClass \in TagClasses
  /\ step \in 0..4
  /\ phaseState \in [Phases -> PhaseStates]
  /\ physicalOwnerCount \in [Phases -> 0..1]
  /\ ownerServiceable \in [Phases -> BOOLEAN]
  /\ phaseEvidence \in [Phases -> EvidenceDomain]
  /\ stalePhysicalOwnerCount \in [Phases -> 0..1]
  /\ staleOwnerServiceable \in [Phases -> BOOLEAN]
  /\ stalePhaseEvidence \in [Phases -> EvidenceDomain]
  /\ nextAdmissionOrdinal \in InitialAdmissionOrdinal..OrdinalCeiling
  /\ queueInsertions \in 0..2
  /\ failClosed \in BOOLEAN
  /\ busyRetryObserved \in BOOLEAN
  /\ appliedRetryObserved \in BOOLEAN
  /\ conflictObserved \in BOOLEAN
  /\ malformedCallbackObserved \in BOOLEAN
  /\ staleTagObserved \in BOOLEAN

PhaseEvidenceTracksIdentity ==
  \A phase \in Phases:
    \/ /\ phaseState[phase] = Unseen
       /\ phaseEvidence[phase] = NoEvidence
    \/ /\ phaseState[phase] \in {PhysicallyOwned, Applied}
       /\ phaseEvidence[phase] = CanonicalEvidence(phase)

AtMostOnePhysicalOwnerPerPhase ==
  \A phase \in Phases:
    /\ physicalOwnerCount[phase] \in 0..1
    /\ stalePhysicalOwnerCount[phase] \in 0..1

UnseenPhaseIsMarkerFree ==
  \A phase \in Phases:
    phaseState[phase] = Unseen =>
      /\ physicalOwnerCount[phase] = 0
      /\ ~ownerServiceable[phase]
      /\ phaseEvidence[phase] = NoEvidence

BusyUnappliedHasExactlyOneServiceableOwner ==
  \A phase \in Phases:
    phaseState[phase] = PhysicallyOwned =>
      /\ physicalOwnerCount[phase] = 1
      /\ ownerServiceable[phase]
      /\ phaseEvidence[phase] = CanonicalEvidence(phase)

AppliedPhaseHasNoPhysicalOwner ==
  \A phase \in Phases:
    phaseState[phase] = Applied =>
      /\ physicalOwnerCount[phase] = 0
      /\ ~ownerServiceable[phase]
      /\ phaseEvidence[phase] = CanonicalEvidence(phase)

BusyExactRetryKeepsOneServiceableOwner ==
  (busyRetryObserved /\ step = 2) =>
    /\ phaseState[activePhase] = PhysicallyOwned
    /\ physicalOwnerCount[activePhase] = 1
    /\ ownerServiceable[activePhase]
    /\ nextAdmissionOrdinal = InitialAdmissionOrdinal + 1
    /\ queueInsertions = 1

AppliedExactRetryPreservesOrdinal ==
  appliedRetryObserved =>
    nextAdmissionOrdinal = InitialAdmissionOrdinal + 1

AppliedExactRetryDoesNotInsert ==
  appliedRetryObserved => queueInsertions = 1

ConflictingEvidenceFailsClosed ==
  conflictObserved => failClosed

MalformedCallbackFailsClosed ==
  malformedCallbackObserved => failClosed

WellFormedStaleTagIsMarkerFree ==
  staleTagObserved =>
    /\ stalePhysicalOwnerCount[activePhase] = 0
    /\ ~staleOwnerServiceable[activePhase]
    /\ stalePhaseEvidence[activePhase] = NoEvidence

WellFormedStaleTagPreservesOrdinal ==
  staleTagObserved =>
    /\ nextAdmissionOrdinal = InitialAdmissionOrdinal + 1
    /\ queueInsertions = 1

Init ==
  /\ activePhase \in Phases
  /\ scenario \in EnabledScenarios
  /\ (scenario \in ConflictScenarios =>
        activePhase = "BodyStored")
  /\ callbackWellFormed = ~(scenario \in MalformedCallbackScenarios)
  /\ tagClass =
       IF scenario \in {MalformedCallbackStaleTag, WellFormedStaleTag}
       THEN StaleTag
       ELSE CurrentTag
  /\ step = 0
  /\ phaseState = [phase \in Phases |-> Unseen]
  /\ physicalOwnerCount = [phase \in Phases |-> 0]
  /\ ownerServiceable = [phase \in Phases |-> FALSE]
  /\ phaseEvidence = [phase \in Phases |-> NoEvidence]
  /\ stalePhysicalOwnerCount = [phase \in Phases |-> 0]
  /\ staleOwnerServiceable = [phase \in Phases |-> FALSE]
  /\ stalePhaseEvidence = [phase \in Phases |-> NoEvidence]
  /\ nextAdmissionOrdinal = InitialAdmissionOrdinal
  /\ queueInsertions = 0
  /\ failClosed = FALSE
  /\ busyRetryObserved = FALSE
  /\ appliedRetryObserved = FALSE
  /\ conflictObserved = FALSE
  /\ malformedCallbackObserved = FALSE
  /\ staleTagObserved = FALSE

AdmitExactCallbackWhileBusy ==
  /\ ~failClosed
  /\ step = 0
  /\ phaseState[activePhase] = Unseen
  /\ phaseState' =
       [phaseState EXCEPT ![activePhase] = PhysicallyOwned]
  /\ physicalOwnerCount' =
       [physicalOwnerCount EXCEPT ![activePhase] = 1]
  /\ ownerServiceable' =
       [ownerServiceable EXCEPT ![activePhase] = TRUE]
  /\ phaseEvidence' =
       [phaseEvidence EXCEPT ![activePhase] = CanonicalEvidence(activePhase)]
  /\ nextAdmissionOrdinal' = nextAdmissionOrdinal + 1
  /\ queueInsertions' = queueInsertions + 1
  /\ step' = 1
  /\ UNCHANGED
       <<activePhase,
         scenario,
         callbackWellFormed,
         tagClass,
         stalePhysicalOwnerCount,
         staleOwnerServiceable,
         stalePhaseEvidence,
         failClosed,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         malformedCallbackObserved,
         staleTagObserved>>

CoalesceExactBusyRetry ==
  /\ ~failClosed
  /\ step = 1
  /\ phaseState[activePhase] = PhysicallyOwned
  /\ physicalOwnerCount[activePhase] = 1
  /\ ownerServiceable[activePhase]
  /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
  /\ step' = 2
  /\ busyRetryObserved' = TRUE
  /\ UNCHANGED semanticVars
  /\ UNCHANGED
       <<activePhase,
         scenario,
         appliedRetryObserved,
         conflictObserved,
         malformedCallbackObserved,
         staleTagObserved>>

ApplyOwnedCallback ==
  /\ ~failClosed
  /\ step = 2
  /\ phaseState[activePhase] = PhysicallyOwned
  /\ physicalOwnerCount[activePhase] = 1
  /\ ownerServiceable[activePhase]
  /\ phaseState' = [phaseState EXCEPT ![activePhase] = Applied]
  /\ physicalOwnerCount' =
       [physicalOwnerCount EXCEPT
          ![activePhase] = IF ClearPhysicalOwnerOnApply THEN 0 ELSE 1]
  /\ ownerServiceable' =
       [ownerServiceable EXCEPT
          ![activePhase] = IF ClearPhysicalOwnerOnApply THEN FALSE ELSE TRUE]
  /\ step' = 3
  /\ UNCHANGED
       <<activePhase,
         scenario,
         callbackWellFormed,
         tagClass,
         phaseEvidence,
         stalePhysicalOwnerCount,
         staleOwnerServiceable,
         stalePhaseEvidence,
         nextAdmissionOrdinal,
         queueInsertions,
         failClosed,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         malformedCallbackObserved,
         staleTagObserved>>

SuppressExactAppliedRetry ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario = ExactAppliedRetry
  /\ callbackWellFormed
  /\ tagClass = CurrentTag
  /\ phaseState[activePhase] = Applied
  /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
  /\ nextAdmissionOrdinal' =
       IF SuppressAppliedBeforeOrdinalAllocation
       THEN nextAdmissionOrdinal
       ELSE nextAdmissionOrdinal + 1
  /\ appliedRetryObserved' = TRUE
  /\ step' = 4
  /\ UNCHANGED
       <<activePhase,
         scenario,
         callbackWellFormed,
         tagClass,
         phaseState,
         physicalOwnerCount,
         ownerServiceable,
         phaseEvidence,
         stalePhysicalOwnerCount,
         staleOwnerServiceable,
         stalePhaseEvidence,
         queueInsertions,
         failClosed,
         busyRetryObserved,
         conflictObserved,
         malformedCallbackObserved,
         staleTagObserved>>

ObserveConflictingEvidence ==
  LET candidate == ConflictingEvidence(activePhase, scenario)
  IN /\ ~failClosed
     /\ step = 3
     /\ scenario \in ConflictScenarios
     /\ callbackWellFormed
     /\ tagClass = CurrentTag
     /\ phaseState[activePhase] = Applied
     /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
     /\ candidate # phaseEvidence[activePhase]
     /\ failClosed' = IF RejectConflictingEvidence THEN TRUE ELSE FALSE
     /\ conflictObserved' = TRUE
     /\ step' = 4
     /\ UNCHANGED
          <<activePhase,
            scenario,
            callbackWellFormed,
            tagClass,
            phaseState,
            physicalOwnerCount,
            ownerServiceable,
            phaseEvidence,
            stalePhysicalOwnerCount,
            staleOwnerServiceable,
            stalePhaseEvidence,
            nextAdmissionOrdinal,
            queueInsertions,
            busyRetryObserved,
            appliedRetryObserved,
            malformedCallbackObserved,
            staleTagObserved>>

ObserveMalformedCallback ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario \in MalformedCallbackScenarios
  /\ ~callbackWellFormed
  /\ tagClass =
       IF scenario = MalformedCallbackStaleTag THEN StaleTag ELSE CurrentTag
  /\ phaseState[activePhase] = Applied
  /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
  /\ failClosed' =
       IF tagClass = CurrentTag \/ ValidateCallbackBeforeStaleTagCoalescing
       THEN TRUE
       ELSE FALSE
  /\ malformedCallbackObserved' = TRUE
  /\ step' = 4
  /\ UNCHANGED
       <<activePhase,
         scenario,
         callbackWellFormed,
         tagClass,
         phaseState,
         physicalOwnerCount,
         ownerServiceable,
         phaseEvidence,
         stalePhysicalOwnerCount,
         staleOwnerServiceable,
         stalePhaseEvidence,
         nextAdmissionOrdinal,
         queueInsertions,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         staleTagObserved>>

ObserveWellFormedStaleTag ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario = WellFormedStaleTag
  /\ callbackWellFormed
  /\ tagClass = StaleTag
  /\ phaseState[activePhase] = Applied
  /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
  /\ stalePhysicalOwnerCount[activePhase] = 0
  /\ stalePhaseEvidence[activePhase] = NoEvidence
  /\ stalePhysicalOwnerCount' =
       IF CoalesceStaleTagBeforeAdmission
       THEN stalePhysicalOwnerCount
       ELSE [stalePhysicalOwnerCount EXCEPT ![activePhase] = 1]
  /\ staleOwnerServiceable' =
       IF CoalesceStaleTagBeforeAdmission
       THEN staleOwnerServiceable
       ELSE [staleOwnerServiceable EXCEPT ![activePhase] = TRUE]
  /\ stalePhaseEvidence' =
       IF CoalesceStaleTagBeforeAdmission
       THEN stalePhaseEvidence
       ELSE [stalePhaseEvidence EXCEPT
               ![activePhase] = CanonicalEvidence(activePhase)]
  /\ nextAdmissionOrdinal' =
       IF CoalesceStaleTagBeforeAdmission
       THEN nextAdmissionOrdinal
       ELSE nextAdmissionOrdinal + 1
  /\ queueInsertions' =
       IF CoalesceStaleTagBeforeAdmission
       THEN queueInsertions
       ELSE queueInsertions + 1
  /\ staleTagObserved' = TRUE
  /\ step' = 4
  /\ UNCHANGED
       <<activePhase,
         scenario,
         callbackWellFormed,
         tagClass,
         phaseState,
         physicalOwnerCount,
         ownerServiceable,
         phaseEvidence,
         failClosed,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         malformedCallbackObserved>>

Next ==
  \/ AdmitExactCallbackWhileBusy
  \/ CoalesceExactBusyRetry
  \/ ApplyOwnedCallback
  \/ SuppressExactAppliedRetry
  \/ ObserveConflictingEvidence
  \/ ObserveMalformedCallback
  \/ ObserveWellFormedStaleTag

=============================================================================
