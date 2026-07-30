---- MODULE SumeragiV2AppliedPhaseAdmissionMutation ----
EXTENDS Naturals

(***************************************************************************
Bounded mutation model for runtime completion admission after reducer apply.

The four production body-pipeline phases are modeled independently:

  BodyAvailable, BodyStored, ValidationSucceeded, SignatureCompleted.

For an arbitrary active phase, the finite prefix is:

  1. a callback arrives while the reducer is Busy and moves from unseen to
     physically_owned with exactly one serviceable owner and one ordinal;
  2. an exact Busy retry coalesces with that owner;
  3. service applies the callback and retires physical ownership; and
  4. one adversarial probe retries the applied callback, conflicts in payload
     or validation polarity, or supplies a malformed/stale phase tag.

`step` and the `*Observed` variables are trace instrumentation only.  An exact
post-apply retry changes those trace variables so TLC can cover the edge, but
leaves every variable in `semanticVars` unchanged when
SuppressAppliedBeforeOrdinalAllocation is TRUE.

The tag probe separates two production outcomes.  A malformed current tag
fails closed even when an exact applied identity exists.  A well-formed stale
tag is discarded before admission and is marker-free.  Looking in the applied
phase ledger before validating the tag can therefore neither hide malformed
input nor turn stale input into a current physical owner.

This is a compact live-process admission obligation.  It does not claim a
crash/restart refinement, Byzantine wire validation, or a deductive proof of
the Rust implementation.
***************************************************************************)

CONSTANTS EnabledScenarios,
          SuppressAppliedBeforeOrdinalAllocation,
          ClearPhysicalOwnerOnApply,
          RejectConflictingEvidence,
          ValidatePhaseTagBeforeLookup

Phases ==
  {"BodyAvailable",
   "BodyStored",
   "ValidationSucceeded",
   "SignatureCompleted"}

Unseen == "unseen"
PhysicallyOwned == "physically_owned"
Applied == "applied"
PhaseStates == {Unseen, PhysicallyOwned, Applied}

ExactAppliedRetry == "ExactAppliedRetry"
ConflictPayload == "ConflictPayload"
ConflictPolarity == "ConflictPolarity"
MalformedPhaseTag == "MalformedPhaseTag"
StalePhaseTag == "StalePhaseTag"

ConflictScenarios == {ConflictPayload, ConflictPolarity}
Scenarios ==
  {ExactAppliedRetry,
   ConflictPayload,
   ConflictPolarity,
   MalformedPhaseTag,
   StalePhaseTag}

ASSUME /\ EnabledScenarios \in (SUBSET Scenarios)
       /\ EnabledScenarios # {}
       /\ SuppressAppliedBeforeOrdinalAllocation \in BOOLEAN
       /\ ClearPhysicalOwnerOnApply \in BOOLEAN
       /\ RejectConflictingEvidence \in BOOLEAN
       /\ ValidatePhaseTagBeforeLookup \in BOOLEAN

CanonicalPayload == "CanonicalPayload"
ConflictingPayload == "ConflictingPayload"
PositivePolarity == "Positive"
NegativePolarity == "Negative"

Payloads == {CanonicalPayload, ConflictingPayload}
Polarities == {PositivePolarity, NegativePolarity}

NoEvidence ==
  [phase |-> "NoPhase",
   payload |-> "NoPayload",
   polarity |-> "NoPolarity"]

EvidenceDomain ==
  { [phase |-> phase,
     payload |-> payload,
     polarity |-> polarity]:
       phase \in Phases,
       payload \in Payloads,
       polarity \in Polarities }
    \cup {NoEvidence}

CanonicalEvidence(phase) ==
  [phase |-> phase,
   payload |-> CanonicalPayload,
   polarity |-> PositivePolarity]

ConflictingEvidence(phase, conflictScenario) ==
  CASE conflictScenario = ConflictPayload ->
         [phase |-> phase,
          payload |-> ConflictingPayload,
          polarity |-> PositivePolarity]
    [] conflictScenario = ConflictPolarity ->
         [phase |-> phase,
          payload |-> CanonicalPayload,
          polarity |-> NegativePolarity]
    [] OTHER -> NoEvidence

InitialAdmissionOrdinal == 1
OrdinalCeiling == 3

VARIABLES activePhase,
          scenario,
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
          malformedTagObserved,
          staleTagObserved

semanticVars ==
  <<phaseState,
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
    malformedTagObserved,
    staleTagObserved>>

TypeInvariant ==
  /\ activePhase \in Phases
  /\ scenario \in EnabledScenarios
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
  /\ malformedTagObserved \in BOOLEAN
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

MalformedPhaseTagFailsClosed ==
  malformedTagObserved => failClosed

StalePhaseTagIsMarkerFree ==
  staleTagObserved =>
    /\ stalePhysicalOwnerCount[activePhase] = 0
    /\ ~staleOwnerServiceable[activePhase]
    /\ stalePhaseEvidence[activePhase] = NoEvidence

StalePhaseTagPreservesOrdinal ==
  staleTagObserved =>
    /\ nextAdmissionOrdinal = InitialAdmissionOrdinal + 1
    /\ queueInsertions = 1

Init ==
  /\ activePhase \in Phases
  /\ scenario \in EnabledScenarios
  /\ (scenario = ConflictPolarity =>
        activePhase = "ValidationSucceeded")
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
  /\ malformedTagObserved = FALSE
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
         stalePhysicalOwnerCount,
         staleOwnerServiceable,
         stalePhaseEvidence,
         failClosed,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         malformedTagObserved,
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
         malformedTagObserved,
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
         malformedTagObserved,
         staleTagObserved>>

SuppressExactAppliedRetry ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario = ExactAppliedRetry
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
         malformedTagObserved,
         staleTagObserved>>

ObserveConflictingEvidence ==
  LET candidate == ConflictingEvidence(activePhase, scenario)
  IN /\ ~failClosed
     /\ step = 3
     /\ scenario \in ConflictScenarios
     /\ phaseState[activePhase] = Applied
     /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
     /\ candidate # phaseEvidence[activePhase]
     /\ failClosed' = IF RejectConflictingEvidence THEN TRUE ELSE FALSE
     /\ conflictObserved' = TRUE
     /\ step' = 4
     /\ UNCHANGED
          <<activePhase,
            scenario,
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
            malformedTagObserved,
            staleTagObserved>>

ObserveMalformedPhaseTag ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario = MalformedPhaseTag
  /\ phaseState[activePhase] = Applied
  /\ phaseEvidence[activePhase] = CanonicalEvidence(activePhase)
  /\ failClosed' = IF ValidatePhaseTagBeforeLookup THEN TRUE ELSE FALSE
  /\ malformedTagObserved' = TRUE
  /\ step' = 4
  /\ UNCHANGED
       <<activePhase,
         scenario,
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

ObserveStalePhaseTag ==
  /\ ~failClosed
  /\ step = 3
  /\ scenario = StalePhaseTag
  /\ stalePhysicalOwnerCount[activePhase] = 0
  /\ stalePhaseEvidence[activePhase] = NoEvidence
  /\ stalePhysicalOwnerCount' =
       IF ValidatePhaseTagBeforeLookup
       THEN stalePhysicalOwnerCount
       ELSE [stalePhysicalOwnerCount EXCEPT ![activePhase] = 1]
  /\ staleOwnerServiceable' =
       IF ValidatePhaseTagBeforeLookup
       THEN staleOwnerServiceable
       ELSE [staleOwnerServiceable EXCEPT ![activePhase] = TRUE]
  /\ stalePhaseEvidence' =
       IF ValidatePhaseTagBeforeLookup
       THEN stalePhaseEvidence
       ELSE [stalePhaseEvidence EXCEPT
               ![activePhase] = CanonicalEvidence(activePhase)]
  /\ nextAdmissionOrdinal' =
       IF ValidatePhaseTagBeforeLookup
       THEN nextAdmissionOrdinal
       ELSE nextAdmissionOrdinal + 1
  /\ queueInsertions' =
       IF ValidatePhaseTagBeforeLookup
       THEN queueInsertions
       ELSE queueInsertions + 1
  /\ staleTagObserved' = TRUE
  /\ step' = 4
  /\ UNCHANGED
       <<activePhase,
         scenario,
         phaseState,
         physicalOwnerCount,
         ownerServiceable,
         phaseEvidence,
         failClosed,
         busyRetryObserved,
         appliedRetryObserved,
         conflictObserved,
         malformedTagObserved>>

Next ==
  \/ AdmitExactCallbackWhileBusy
  \/ CoalesceExactBusyRetry
  \/ ApplyOwnedCallback
  \/ SuppressExactAppliedRetry
  \/ ObserveConflictingEvidence
  \/ ObserveMalformedPhaseTag
  \/ ObserveStalePhaseTag

=============================================================================
