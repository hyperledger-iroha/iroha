---- MODULE SumeragiV2ServeSchedulerOrdinalMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite regression for the shared exact-Serve/Runtime admission order.

The production model allocates every fresh exact Serve ingress record from
the same monotone per-node high-watermark as Candidate and timeout roots.  A
duplicate carrier which meets the same live ingress record coalesces without
changing its scheduler ordinal.  After that record drains, a retransmission
of the same logical Serve identity creates a fresh physical ingress record
with a fresh, strictly later scheduler ordinal; the logical lifecycle and its
tombstone remain unchanged, so the old priority position is never resurrected.

This finite model quotients the unbounded sequence of fresh post-drain
ordinals by the only ordering fact used by the runner: `ticketIsLater` says
whether the current fresh record was admitted after the frozen timeout root.
`ticketEpoch` toggles at each post-drain admission and is unchanged by a live
duplicate, making the record boundary executable without bounding the number
of retransmissions.  The production arithmetic is checked separately by
`AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal`.

The quotient starts after the finite frozen predecessor prefix of a ticket
has drained.  Rust waiter records pre-reserved before the timeout cut refine
to that bounded prefix and are not repeated admissions here.  Consequently,
the admission actions below represent only a distinct post-cut ticket: they
cannot replenish the old prefix, and shared admission must place that ticket
strictly after the frozen timeout owner.

The three mutation pairs isolate independent defects:

  * a separate Serve ordinal makes every ticket appear no later than the
    frozen timeout, so target-first drain/retransmit can lasso; and
  * even with the repaired shared order, an always-target-first runner can
    drain/retransmit forever without taking the older Runtime episode.
  * the same target-first shortcut at Local can drain/retransmit forever while
    an already-admitted local completion keeps its lower shared ordinal.

Weak fairness is placed on the whole runner action, matching production.  It
cannot rescue either mutant by selecting an internal branch which the mutant
never exposes.
***************************************************************************)

RunnerPhases == {"Runtime", "Local", "Ingress"}
TransitionNames ==
  {"Initial", "AdmitShared", "AdmitSeparate", "DuplicateCoalesced",
   "AdmitSharedLocal", "OlderRuntime", "OlderLocal", "TargetOnly",
   "AlwaysTargetFirst", "Drain", "DrainLocal"}

VARIABLES
  runnerPhase,
  timeoutPending,
  localPending,
  ticketActive,
  ticketIsLater,
  ticketEpoch,
  lastDrainedEpoch,
  sharedHighWatermarkAdvanced,
  lastTransition

MutationVars ==
  <<runnerPhase, timeoutPending, localPending, ticketActive, ticketIsLater,
    ticketEpoch, lastDrainedEpoch, sharedHighWatermarkAdvanced,
    lastTransition>>

MutationTypeInvariant ==
  /\ runnerPhase \in RunnerPhases
  /\ timeoutPending \in BOOLEAN
  /\ localPending \in BOOLEAN
  /\ ticketActive \in BOOLEAN
  /\ ticketIsLater \in BOOLEAN
  /\ ticketEpoch \in BOOLEAN
  /\ lastDrainedEpoch \in BOOLEAN
  /\ sharedHighWatermarkAdvanced \in BOOLEAN
  /\ lastTransition \in TransitionNames

MutationInit ==
  /\ runnerPhase = "Runtime"
  /\ timeoutPending
  /\ ~localPending
  /\ ~ticketActive
  /\ ~ticketIsLater
  /\ ticketEpoch = FALSE
  /\ lastDrainedEpoch = FALSE
  /\ ~sharedHighWatermarkAdvanced
  /\ lastTransition = "Initial"

(***************************************************************************
The parity epoch is a finite quotient of fresh immutable ordinals.  It differs
from the immediately drained record on every post-drain admission.  A live
duplicate does not take either admission action and retains all ticket fields.
***************************************************************************)
AdmitWithSharedSchedulerOrdinal ==
  /\ ~ticketActive
  /\ runnerPhase = "Runtime"
  /\ ticketActive' = TRUE
  /\ ticketIsLater' = timeoutPending
  /\ ticketEpoch' = ~lastDrainedEpoch
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "AdmitShared"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending,
                  lastDrainedEpoch>>

AdmitWithSeparateServeOrdinal ==
  /\ ~ticketActive
  /\ runnerPhase = "Runtime"
  /\ ticketActive' = TRUE
  /\ ticketIsLater' = FALSE
  /\ ticketEpoch' = ~lastDrainedEpoch
  /\ lastTransition' = "AdmitSeparate"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

DuplicateCarrierCoalesces ==
  /\ ticketActive
  /\ lastTransition' = "DuplicateCoalesced"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending, ticketActive,
                  ticketIsLater, ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

OlderRuntimePrecedesServeIngress ==
  /\ runnerPhase = "Runtime"
  /\ ticketActive
  /\ timeoutPending
  /\ ticketIsLater
  /\ runnerPhase' = "Local"
  /\ timeoutPending' = FALSE
  /\ lastTransition' = "OlderRuntime"
  /\ UNCHANGED <<localPending, ticketActive, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch, sharedHighWatermarkAdvanced>>

TargetOnlyAfterOlderEpisode ==
  /\ ticketActive
  /\ \/ runnerPhase = "Local"
     \/ /\ runnerPhase = "Runtime"
        /\ ~(timeoutPending /\ ticketIsLater)
  /\ runnerPhase' = "Ingress"
  /\ lastTransition' = "TargetOnly"
  /\ UNCHANGED <<timeoutPending, localPending, ticketActive, ticketIsLater,
                  ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

AlwaysTargetFirst ==
  /\ ticketActive
  /\ runnerPhase \in {"Runtime", "Local"}
  /\ runnerPhase' = "Ingress"
  /\ lastTransition' = "AlwaysTargetFirst"
  /\ UNCHANGED <<timeoutPending, localPending, ticketActive, ticketIsLater,
                  ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

DrainExactIngressRecord ==
  /\ ticketActive
  /\ runnerPhase = "Ingress"
  /\ runnerPhase' = "Runtime"
  /\ ticketActive' = FALSE
  /\ ticketIsLater' = FALSE
  /\ lastDrainedEpoch' = ticketEpoch
  /\ lastTransition' = "Drain"
  /\ UNCHANGED <<timeoutPending, localPending, ticketEpoch,
                  sharedHighWatermarkAdvanced>>

(***************************************************************************
The Local regression starts with one immutable completion lifecycle already
admitted before the exact request.  A fresh request therefore receives a
strictly later scheduler ordinal.  The repaired runner admits that one named
predecessor without consuming or replacing the ticket; the mutant jumps
directly to Ingress and can recreate a fresh later carrier after every drain.
***************************************************************************)
LocalMutationInit ==
  /\ runnerPhase = "Local"
  /\ ~timeoutPending
  /\ localPending
  /\ ~ticketActive
  /\ ~ticketIsLater
  /\ ticketEpoch = FALSE
  /\ lastDrainedEpoch = FALSE
  /\ ~sharedHighWatermarkAdvanced
  /\ lastTransition = "Initial"

AdmitWithSharedSchedulerOrdinalAtLocal ==
  /\ ~ticketActive
  /\ runnerPhase = "Local"
  /\ ticketActive' = TRUE
  /\ ticketIsLater' = localPending
  /\ ticketEpoch' = ~lastDrainedEpoch
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "AdmitSharedLocal"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending,
                  lastDrainedEpoch>>

OlderLocalPrecedesServeIngress ==
  /\ runnerPhase = "Local"
  /\ ticketActive
  /\ localPending
  /\ ticketIsLater
  /\ localPending' = FALSE
  /\ lastTransition' = "OlderLocal"
  /\ UNCHANGED <<runnerPhase, timeoutPending, ticketActive,
                  ticketIsLater, ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

TargetOnlyAfterOlderLocalEpisode ==
  /\ ticketActive
  /\ runnerPhase = "Local"
  /\ ~(localPending /\ ticketIsLater)
  /\ runnerPhase' = "Ingress"
  /\ lastTransition' = "TargetOnly"
  /\ UNCHANGED <<timeoutPending, localPending, ticketActive,
                  ticketIsLater, ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

DrainExactIngressRecordToLocal ==
  /\ ticketActive
  /\ runnerPhase = "Ingress"
  /\ runnerPhase' = "Local"
  /\ ticketActive' = FALSE
  /\ ticketIsLater' = FALSE
  /\ lastDrainedEpoch' = ticketEpoch
  /\ lastTransition' = "DrainLocal"
  /\ UNCHANGED <<timeoutPending, localPending, ticketEpoch,
                  sharedHighWatermarkAdvanced>>

FixedRunner ==
  \/ OlderRuntimePrecedesServeIngress
  \/ TargetOnlyAfterOlderEpisode

SeparateOrdinalRunner == TargetOnlyAfterOlderEpisode
AlwaysTargetFirstRunner == AlwaysTargetFirst

SharedOrdinalFixedNext ==
  \/ AdmitWithSharedSchedulerOrdinal
  \/ DuplicateCarrierCoalesces
  \/ FixedRunner
  \/ DrainExactIngressRecord

SeparateOrdinalBugNext ==
  \/ AdmitWithSeparateServeOrdinal
  \/ DuplicateCarrierCoalesces
  \/ SeparateOrdinalRunner
  \/ DrainExactIngressRecord

OlderRuntimeFixedNext == SharedOrdinalFixedNext

AlwaysTargetFirstBugNext ==
  \/ AdmitWithSharedSchedulerOrdinal
  \/ DuplicateCarrierCoalesces
  \/ AlwaysTargetFirstRunner
  \/ DrainExactIngressRecord

OlderLocalFixedRunner ==
  \/ OlderLocalPrecedesServeIngress
  \/ TargetOnlyAfterOlderLocalEpisode

OlderLocalFixedNext ==
  \/ AdmitWithSharedSchedulerOrdinalAtLocal
  \/ DuplicateCarrierCoalesces
  \/ OlderLocalFixedRunner
  \/ DrainExactIngressRecordToLocal

LocalTargetFirstBugNext ==
  \/ AdmitWithSharedSchedulerOrdinalAtLocal
  \/ DuplicateCarrierCoalesces
  \/ AlwaysTargetFirstRunner
  \/ DrainExactIngressRecordToLocal

SharedOrdinalFixedSpec ==
  /\ MutationInit
  /\ [][SharedOrdinalFixedNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSharedSchedulerOrdinal)
  /\ WF_MutationVars(FixedRunner)
  /\ WF_MutationVars(DrainExactIngressRecord)

SeparateOrdinalBugSpec ==
  /\ MutationInit
  /\ [][SeparateOrdinalBugNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSeparateServeOrdinal)
  /\ WF_MutationVars(SeparateOrdinalRunner)
  /\ WF_MutationVars(DrainExactIngressRecord)

OlderRuntimeFixedSpec ==
  /\ MutationInit
  /\ [][OlderRuntimeFixedNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSharedSchedulerOrdinal)
  /\ WF_MutationVars(FixedRunner)
  /\ WF_MutationVars(DrainExactIngressRecord)

AlwaysTargetFirstBugSpec ==
  /\ MutationInit
  /\ [][AlwaysTargetFirstBugNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSharedSchedulerOrdinal)
  /\ WF_MutationVars(AlwaysTargetFirstRunner)
  /\ WF_MutationVars(DrainExactIngressRecord)

OlderLocalFixedSpec ==
  /\ LocalMutationInit
  /\ [][OlderLocalFixedNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSharedSchedulerOrdinalAtLocal)
  /\ WF_MutationVars(OlderLocalFixedRunner)
  /\ WF_MutationVars(DrainExactIngressRecordToLocal)

LocalTargetFirstBugSpec ==
  /\ LocalMutationInit
  /\ [][LocalTargetFirstBugNext]_MutationVars
  /\ WF_MutationVars(AdmitWithSharedSchedulerOrdinalAtLocal)
  /\ WF_MutationVars(AlwaysTargetFirstRunner)
  /\ WF_MutationVars(DrainExactIngressRecordToLocal)

FreshPostDrainRecordDoesNotReuseImmediateEpoch ==
  ticketActive => ticketEpoch # lastDrainedEpoch

SharedTicketIsStrictlyAfterFrozenTimeout ==
  /\ ticketActive
  /\ timeoutPending
  /\ lastTransition # "AdmitSeparate"
  => ticketIsLater

TargetOnlyCannotOvertakeOlderTimeout ==
  /\ timeoutPending
  /\ ticketActive
  /\ ticketIsLater
  => lastTransition # "TargetOnly"

TargetOnlyCannotOvertakeOlderLocal ==
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater
  => lastTransition # "TargetOnly"

LiveDuplicateRetainsSchedulerOwnership ==
  lastTransition = "DuplicateCoalesced" => ticketActive

SharedAdmissionAdvancesSchedulerHighWatermark ==
  lastTransition \in {"AdmitShared", "AdmitSharedLocal"}
    => sharedHighWatermarkAdvanced

EventuallyOlderTimeoutEpisodeRuns == <>(~timeoutPending)

EventuallyOlderLocalEpisodeRuns == <>(~localPending)

=============================================================================
