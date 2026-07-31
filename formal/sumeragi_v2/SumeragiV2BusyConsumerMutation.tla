---- MODULE SumeragiV2BusyConsumerMutation ----
EXTENDS Integers

(***************************************************************************
Bounded mutation for BusyCompletionCandidates.  The candidate still matches
the serialized pending owner and current height, but belongs to an obsolete
consumer view/generation.  The fixed guard excludes it; the old carrier-only
guard incorrectly counts it as the busy node's executable completion.
***************************************************************************)

CONSTANT UseCurrentConsumerGuard

VARIABLES phase, nodeBusy, pendingOwner, candidatePresent,
          currentHeight, currentView, currentGeneration,
          candidateHeight, candidateWorkView, candidateConsumerView,
          candidateConsumerGeneration, countedBusyWitness

vars ==
  <<phase, nodeBusy, pendingOwner, candidatePresent,
    currentHeight, currentView, currentGeneration,
    candidateHeight, candidateWorkView, candidateConsumerView,
    candidateConsumerGeneration, countedBusyWitness>>

CandidateMatchesPendingOwner ==
  /\ candidatePresent
  /\ pendingOwner
  /\ candidateWorkView = 0

CandidateConsumerCurrent ==
  /\ candidateHeight = currentHeight
  /\ candidateConsumerView = currentView
  /\ candidateConsumerGeneration = currentGeneration

SelectedBusyCandidate ==
  /\ CandidateMatchesPendingOwner
  /\ (IF UseCurrentConsumerGuard THEN CandidateConsumerCurrent ELSE TRUE)

Init ==
  /\ phase = 0
  /\ nodeBusy = TRUE
  /\ pendingOwner = TRUE
  /\ candidatePresent = FALSE
  /\ currentHeight = 1
  /\ currentView = 1
  /\ currentGeneration = 1
  /\ candidateHeight = 1
  /\ candidateWorkView = 0
  /\ candidateConsumerView = 0
  /\ candidateConsumerGeneration = 0
  /\ countedBusyWitness = FALSE

AdmitStaleMatchingCompletion ==
  /\ phase = 0
  /\ phase' = 1
  /\ candidatePresent' = TRUE
  /\ countedBusyWitness' =
       IF UseCurrentConsumerGuard THEN FALSE ELSE TRUE
  /\ UNCHANGED <<nodeBusy, pendingOwner, currentHeight, currentView,
                  currentGeneration, candidateHeight, candidateWorkView,
                  candidateConsumerView, candidateConsumerGeneration>>

Next == AdmitStaleMatchingCompletion \/ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeInvariant ==
  /\ phase \in 0..1
  /\ nodeBusy \in BOOLEAN
  /\ pendingOwner \in BOOLEAN
  /\ candidatePresent \in BOOLEAN
  /\ currentHeight \in 0..1
  /\ currentView \in 0..1
  /\ currentGeneration \in 0..1
  /\ candidateHeight \in 0..1
  /\ candidateWorkView \in 0..1
  /\ candidateConsumerView \in 0..1
  /\ candidateConsumerGeneration \in 0..1
  /\ countedBusyWitness \in BOOLEAN

CountedBusyWitnessIsExecutable ==
  countedBusyWitness => CandidateConsumerCurrent

=============================================================================
