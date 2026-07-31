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

The six mutation pairs isolate independent defects:

  * a separate Serve ordinal makes every ticket appear no later than the
    frozen timeout, so target-first drain/retransmit can lasso; and
  * even with the repaired shared order, an always-target-first runner can
    drain/retransmit forever without taking the older Runtime episode.
  * the same target-first shortcut at Local can drain/retransmit forever while
    an already-admitted local completion keeps its lower shared ordinal.
  * a continuation admitted strictly after an exact ingress target can be
    selected only by the mutant.  The repaired selector still permits an
    equal/earlier continuation to run once, then returns to the frozen ingress
    target; the mutant repeatedly exposes the later continuation and leaves
    the admitted target in a stuttering lasso.
  * a dormant lifecycle may retain an older logical ordinal, but its replay
    receives a fresh physical carrier after an already-admitted claim.  The
    mutant selects by the stale logical ordinal and leaves the claim pending.
  * installing a producer continuation is a named residual, not claim
    progress.  The repaired kernel services that residual and re-enters the
    same claim; the mutant treats a raw lower rank as an exit and loses the
    only enabled claim owner.

Weak fairness is placed on the whole runner action, matching production.  It
cannot rescue either mutant by selecting an internal branch which the mutant
never exposes.
***************************************************************************)

RunnerPhases == {"Runtime", "Local", "Ingress"}
TransitionNames ==
  {"Initial", "AdmitShared", "AdmitSeparate", "DuplicateCoalesced",
   "AdmitSharedLocal", "OlderRuntime", "OlderLocal", "TargetOnly",
   "AlwaysTargetFirst", "Drain", "DrainLocal",
   "IngressBeforeLaterContinuation", "PermittedContinuation",
   "IngressAfterPermittedContinuation", "ContinuationOvertake",
   "ClaimBeforeDormantReplay", "DormantLogicalOvertake",
   "InstallContinuationResidual", "ContinuationRankedReentry",
   "ClaimAuxProgress", "RawRankDescent"}

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

(***************************************************************************
Continuation/ingress cut regression.

The initial state nondeterministically covers both selector boundaries.  A
strictly later continuation must yield immediately to the frozen ingress
target.  An equal/earlier continuation remains a legal predecessor, but its
single coalesced lifecycle is consumed before the target runs.  The mutant
exposes the later continuation as a target-first runner arm; after the first
observable overtake that arm is a stutter and admits the expected temporal
counterexample without inventing stronger branch fairness.
***************************************************************************)
ContinuationCutMutationInit ==
  /\ runnerPhase = "Ingress"
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater \in BOOLEAN
  /\ ticketEpoch = FALSE
  /\ lastDrainedEpoch = FALSE
  /\ ~sharedHighWatermarkAdvanced
  /\ lastTransition = "Initial"

AdmittedIngressPrecedesLaterContinuation ==
  /\ timeoutPending
  /\ ticketActive
  /\ localPending
  /\ ticketIsLater
  /\ timeoutPending' = FALSE
  /\ ticketActive' = FALSE
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "IngressBeforeLaterContinuation"
  /\ UNCHANGED <<runnerPhase, localPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch>>

PermittedContinuationPrecedesIngress ==
  /\ timeoutPending
  /\ ticketActive
  /\ localPending
  /\ ~ticketIsLater
  /\ localPending' = FALSE
  /\ lastTransition' = "PermittedContinuation"
  /\ UNCHANGED <<runnerPhase, timeoutPending, ticketActive, ticketIsLater,
                  ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

AdmittedIngressRunsAfterPermittedContinuation ==
  /\ timeoutPending
  /\ ticketActive
  /\ ~localPending
  /\ timeoutPending' = FALSE
  /\ ticketActive' = FALSE
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "IngressAfterPermittedContinuation"
  /\ UNCHANGED <<runnerPhase, localPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch>>

LaterContinuationOvertakesAdmittedIngress ==
  /\ timeoutPending
  /\ ticketActive
  /\ localPending
  /\ ticketIsLater
  /\ lastTransition' = "ContinuationOvertake"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending,
                  ticketActive, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch, sharedHighWatermarkAdvanced>>

ContinuationCutFixedRunner ==
  \/ AdmittedIngressPrecedesLaterContinuation
  \/ PermittedContinuationPrecedesIngress
  \/ AdmittedIngressRunsAfterPermittedContinuation

ContinuationOvertakeBugRunner ==
  \/ LaterContinuationOvertakesAdmittedIngress
  \/ PermittedContinuationPrecedesIngress
  \/ AdmittedIngressRunsAfterPermittedContinuation

(***************************************************************************
Claim physical-cut regression.

`localPending` represents a dormant leader-wire identity whose immutable
logical ordinal predates the certified-response claim.  `ticketIsLater`
records the repaired admission fact: reactivation allocated a fresh physical
carrier and current lane-prefix snapshot after the claim's frozen physical
cut.  The fixed runner consumes the claim first.  The mutant consults only
the stale logical ordinal and repeatedly exposes the dormant carrier ahead of
the claim.
***************************************************************************)
ClaimPhysicalCutMutationInit ==
  /\ runnerPhase = "Ingress"
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater
  /\ ticketEpoch = FALSE
  /\ lastDrainedEpoch = FALSE
  /\ ~sharedHighWatermarkAdvanced
  /\ lastTransition = "Initial"

ClaimRunsBeforeFreshDormantCarrier ==
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater
  /\ timeoutPending' = FALSE
  /\ ticketActive' = FALSE
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "ClaimBeforeDormantReplay"
  /\ UNCHANGED <<runnerPhase, localPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch>>

DormantLogicalIdentityOvertakesClaim ==
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater
  /\ lastTransition' = "DormantLogicalOvertake"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending, ticketActive,
                  ticketIsLater, ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

ClaimPhysicalCutFixedRunner ==
  ClaimRunsBeforeFreshDormantCarrier

ClaimLogicalOvertakeBugRunner ==
  DormantLogicalIdentityOvertakesClaim

(***************************************************************************
Claim ranked-kernel re-entry regression.

The first fixed step lowers the structural rank only by installing an exact
producer-continuation residual.  The next fair turn services that residual
and re-enters the same ranked claim kernel; only the final step is auxiliary
claim progress.  The mutant discards the ranked kernel on a raw lower-rank
observation without publishing the continuation owner, leaving no enabled
path to the retained claim.
***************************************************************************)
ClaimRankedReentryMutationInit ==
  /\ runnerPhase = "Runtime"
  /\ timeoutPending
  /\ ~localPending
  /\ ticketActive
  /\ ~ticketIsLater
  /\ ticketEpoch = FALSE
  /\ lastDrainedEpoch = FALSE
  /\ ~sharedHighWatermarkAdvanced
  /\ lastTransition = "Initial"

InstallClaimContinuationResidual ==
  /\ timeoutPending
  /\ ticketActive
  /\ ~localPending
  /\ lastTransition = "Initial"
  /\ ticketActive' = FALSE
  /\ localPending' = TRUE
  /\ lastTransition' = "InstallContinuationResidual"
  /\ UNCHANGED <<runnerPhase, timeoutPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch, sharedHighWatermarkAdvanced>>

ServiceClaimContinuationAndReenterRankedKernel ==
  /\ timeoutPending
  /\ ~ticketActive
  /\ localPending
  /\ lastTransition = "InstallContinuationResidual"
  /\ ticketActive' = TRUE
  /\ localPending' = FALSE
  /\ lastTransition' = "ContinuationRankedReentry"
  /\ UNCHANGED <<runnerPhase, timeoutPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch, sharedHighWatermarkAdvanced>>

ClaimAuxProgressAfterRankedReentry ==
  /\ timeoutPending
  /\ ticketActive
  /\ ~localPending
  /\ lastTransition = "ContinuationRankedReentry"
  /\ timeoutPending' = FALSE
  /\ ticketActive' = FALSE
  /\ sharedHighWatermarkAdvanced' = TRUE
  /\ lastTransition' = "ClaimAuxProgress"
  /\ UNCHANGED <<runnerPhase, localPending, ticketIsLater, ticketEpoch,
                  lastDrainedEpoch>>

RawLowerRankDropsClaimKernel ==
  /\ timeoutPending
  /\ ticketActive
  /\ ~localPending
  /\ lastTransition = "Initial"
  /\ ticketActive' = FALSE
  /\ lastTransition' = "RawRankDescent"
  /\ UNCHANGED <<runnerPhase, timeoutPending, localPending, ticketIsLater,
                  ticketEpoch, lastDrainedEpoch,
                  sharedHighWatermarkAdvanced>>

ClaimRankedReentryFixedRunner ==
  \/ InstallClaimContinuationResidual
  \/ ServiceClaimContinuationAndReenterRankedKernel
  \/ ClaimAuxProgressAfterRankedReentry

ClaimRawDescentBugRunner ==
  RawLowerRankDropsClaimKernel

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

ContinuationCutFixedSpec ==
  /\ ContinuationCutMutationInit
  /\ [][ContinuationCutFixedRunner]_MutationVars
  /\ WF_MutationVars(ContinuationCutFixedRunner)

ContinuationOvertakeBugSpec ==
  /\ ContinuationCutMutationInit
  /\ [][ContinuationOvertakeBugRunner]_MutationVars
  /\ WF_MutationVars(ContinuationOvertakeBugRunner)

ClaimPhysicalCutFixedSpec ==
  /\ ClaimPhysicalCutMutationInit
  /\ [][ClaimPhysicalCutFixedRunner]_MutationVars
  /\ WF_MutationVars(ClaimPhysicalCutFixedRunner)

ClaimLogicalOvertakeBugSpec ==
  /\ ClaimPhysicalCutMutationInit
  /\ [][ClaimLogicalOvertakeBugRunner]_MutationVars
  /\ WF_MutationVars(ClaimLogicalOvertakeBugRunner)

ClaimRankedReentryFixedSpec ==
  /\ ClaimRankedReentryMutationInit
  /\ [][ClaimRankedReentryFixedRunner]_MutationVars
  /\ WF_MutationVars(ClaimRankedReentryFixedRunner)

ClaimRawDescentBugSpec ==
  /\ ClaimRankedReentryMutationInit
  /\ [][ClaimRawDescentBugRunner]_MutationVars
  /\ WF_MutationVars(ClaimRawDescentBugRunner)

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

LaterContinuationCannotOwnTurnAheadOfAdmittedIngress ==
  /\ timeoutPending
  /\ ticketActive
  /\ localPending
  /\ ticketIsLater
    => lastTransition # "ContinuationOvertake"

PermittedContinuationRetainsFrozenIngressTarget ==
  lastTransition = "PermittedContinuation"
    => /\ timeoutPending
       /\ ticketActive
       /\ ~sharedHighWatermarkAdvanced

EventuallyAdmittedIngressRuns == <>sharedHighWatermarkAdvanced

FreshDormantReplayUsesPostClaimPhysicalPosition ==
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
    => ticketIsLater

DormantLogicalIdentityCannotOwnClaimTurn ==
  /\ timeoutPending
  /\ localPending
  /\ ticketActive
  /\ ticketIsLater
    => lastTransition # "DormantLogicalOvertake"

ClaimCutPersistsUntilAuxProgress ==
  timeoutPending => ticketActive

EventuallyClaimRunsBeforeDormantReplay == <>(~timeoutPending)

ExplicitContinuationOwnsLowerRankExit ==
  /\ timeoutPending
  /\ ~ticketActive
    => localPending

ContinuationReentryRetainsClaim ==
  lastTransition = "ContinuationRankedReentry"
    => /\ timeoutPending
       /\ ticketActive
       /\ ~localPending

RawRankDescentCannotEraseClaimKernel ==
  lastTransition # "RawRankDescent"

EventuallyClaimAuxProgressAfterContinuation == <>(~timeoutPending)

=============================================================================
