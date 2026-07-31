-------------- MODULE SumeragiV2LocalIngressSchedulerReservationMutation --------------
EXTENDS Naturals

(***************************************************************************
Focused local Fair-ingress scheduler mutation.

The in-flight target owns only its stable transport identity and bounded
retained-queue episode.  Unrelated local work may therefore advance the
shared scheduler before the target reaches the capacity-proven Fair-ingress
cut.  At that cut the repaired branch freezes the current ordinal and advances
the shared high-watermark atomically.  The mutation advances the same
high-watermark but later reconstructs the target's ordinal as mutable
`nextSchedulerOrdinal - 1`.  One later local admission then rewrites the
already accepted target's effective position.

This model deliberately contains no fairness.  It checks the source-level
ownership transition itself: the failing configuration reaches the intended
bad state through the finite prefix InFlight -> CapacityProven -> Accepted ->
LaterWork.
***************************************************************************)

CONSTANT FreezeAtLocalAcceptance

Phases ==
  {"InFlight", "CapacityProven", "Accepted", "LaterWork",
   "Retried", "Runtime", "Done"}

VARIABLES
  phase,
  nextSchedulerOrdinal,
  acceptedOrdinal,
  retainedLocalOrdinal,
  retryOrdinal,
  runtimeOrdinal

mutationVars ==
  <<phase, nextSchedulerOrdinal, acceptedOrdinal,
    retainedLocalOrdinal, retryOrdinal, runtimeOrdinal>>

EffectiveAcceptedSchedulerOrdinal ==
  IF FreezeAtLocalAcceptance
  THEN retainedLocalOrdinal
  ELSE nextSchedulerOrdinal - 1

Init ==
  /\ phase = "InFlight"
  /\ nextSchedulerOrdinal = 1
  /\ acceptedOrdinal = 0
  /\ retainedLocalOrdinal = 0
  /\ retryOrdinal = 0
  /\ runtimeOrdinal = 0

ConsumeFinitePreAcceptanceEpisode ==
  /\ phase = "InFlight"
  /\ phase' = "CapacityProven"
  /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
  /\ UNCHANGED
       <<acceptedOrdinal, retainedLocalOrdinal,
         retryOrdinal, runtimeOrdinal>>

AcceptAtFairIngress ==
  /\ phase = "CapacityProven"
  /\ phase' = "Accepted"
  /\ acceptedOrdinal' = nextSchedulerOrdinal
  /\ retainedLocalOrdinal' =
       IF FreezeAtLocalAcceptance
       THEN nextSchedulerOrdinal
       ELSE 0
  /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
  /\ UNCHANGED <<retryOrdinal, runtimeOrdinal>>

AdmitLaterLocalWork ==
  /\ phase = "Accepted"
  /\ phase' = "LaterWork"
  /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
  /\ UNCHANGED
       <<acceptedOrdinal, retainedLocalOrdinal,
         retryOrdinal, runtimeOrdinal>>

RetransmitExactTarget ==
  /\ phase = "LaterWork"
  /\ phase' = "Retried"
  /\ retryOrdinal' = EffectiveAcceptedSchedulerOrdinal
  /\ UNCHANGED
       <<nextSchedulerOrdinal, acceptedOrdinal,
         retainedLocalOrdinal, runtimeOrdinal>>

EnterRuntime ==
  /\ phase = "Retried"
  /\ phase' = "Runtime"
  /\ runtimeOrdinal' = retryOrdinal
  /\ UNCHANGED
       <<nextSchedulerOrdinal, acceptedOrdinal,
         retainedLocalOrdinal, retryOrdinal>>

CompleteTarget ==
  /\ phase = "Runtime"
  /\ phase' = "Done"
  /\ UNCHANGED
       <<nextSchedulerOrdinal, acceptedOrdinal,
         retainedLocalOrdinal, retryOrdinal, runtimeOrdinal>>

Next ==
  \/ ConsumeFinitePreAcceptanceEpisode
  \/ AcceptAtFairIngress
  \/ AdmitLaterLocalWork
  \/ RetransmitExactTarget
  \/ EnterRuntime
  \/ CompleteTarget

Spec == Init /\ [][Next]_mutationVars

TypeInvariant ==
  /\ FreezeAtLocalAcceptance \in BOOLEAN
  /\ phase \in Phases
  /\ nextSchedulerOrdinal \in 1..4
  /\ acceptedOrdinal \in 0..2
  /\ retainedLocalOrdinal \in 0..2
  /\ retryOrdinal \in 0..3
  /\ runtimeOrdinal \in 0..3

AcceptedLocalOwnerKeepsOrdinal ==
  phase \in {"InFlight", "CapacityProven"}
    \/ EffectiveAcceptedSchedulerOrdinal = acceptedOrdinal

LaterLocalWorkCannotRewriteAcceptedOrdinal ==
  phase \notin {"LaterWork", "Retried", "Runtime", "Done"}
    \/ EffectiveAcceptedSchedulerOrdinal = acceptedOrdinal

ExactRetryReusesAcceptedOrdinal ==
  phase \notin {"Retried", "Runtime", "Done"}
    \/ retryOrdinal = acceptedOrdinal

RuntimeConsumesAcceptedOrdinal ==
  phase \notin {"Runtime", "Done"}
    \/ runtimeOrdinal = acceptedOrdinal

=============================================================================
