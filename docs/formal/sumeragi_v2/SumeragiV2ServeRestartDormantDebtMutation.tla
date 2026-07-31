---- MODULE SumeragiV2ServeRestartDormantDebtMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite executable mutation kernel for committed Serve ingress ownership.

An undrained persisted waiter survives restart or Pending receiver-close with
the same lifecycle and scheduler IDs, but without a physical carrier or old
prefix.  A physically drained/materialized occurrence has retired its
scheduler ticket: restart or receiver-close retains only the unsealed
lifecycle, and an exact retry reserves a fresh scheduler position.  Volatile
superseded/same-view policy-rejection occurrences have lifecycle ordinal zero
and are dropped instead of becoming dormant debt.  Completion installs a
terminal tombstone which cannot resurrect.
***************************************************************************)

CONSTANTS PreserveRestartDebt, PreserveRetryIds,
          BlockCompletedResurrection, BlockDormantProducerChurn,
          RejectLaterDormantRetry, DropRejectedJunk,
          RetireDrainedScheduler, PreserveCommittedRollbackLifecycle

ASSUME PreserveRestartDebt \in BOOLEAN
ASSUME PreserveRetryIds \in BOOLEAN
ASSUME BlockCompletedResurrection \in BOOLEAN
ASSUME BlockDormantProducerChurn \in BOOLEAN
ASSUME RejectLaterDormantRetry \in BOOLEAN
ASSUME DropRejectedJunk \in BOOLEAN
ASSUME RetireDrainedScheduler \in BOOLEAN
ASSUME PreserveCommittedRollbackLifecycle \in BOOLEAN

None == [kind |-> "None"]

Admission(lifecycleOrdinal) ==
  [kind |-> "Admission", lifecycleOrdinal |-> lifecycleOrdinal]

Reservation(lifecycleOrdinal, status) ==
  [kind |-> "Reservation", lifecycleOrdinal |-> lifecycleOrdinal,
   status |-> status]

Waiter(physicalOrdinal, schedulerOrdinal, lifecycleOrdinal, prefix) ==
  [kind |-> "Waiter", physicalOrdinal |-> physicalOrdinal,
   schedulerOrdinal |-> schedulerOrdinal,
   lifecycleOrdinal |-> lifecycleOrdinal, prefix |-> prefix]

Tombstone(lifecycleOrdinal) ==
  [kind |-> "Tombstone", lifecycleOrdinal |-> lifecycleOrdinal]

VARIABLES phase, nextLifecycleOrdinal, nextSchedulerOrdinal,
          nextPhysicalOrdinal, admission, reservation, waiter,
          laterWaiter, rejectedJunkWaiter, physicalCarrier,
          tombstone, producerChurn

vars ==
  <<phase, nextLifecycleOrdinal, nextSchedulerOrdinal,
    nextPhysicalOrdinal, admission, reservation, waiter,
    laterWaiter, rejectedJunkWaiter, physicalCarrier,
    tombstone, producerChurn>>

Init ==
  /\ phase = "ChooseScenario"
  /\ nextLifecycleOrdinal = 1
  /\ nextSchedulerOrdinal = 1
  /\ nextPhysicalOrdinal = 1
  /\ admission = None
  /\ reservation = None
  /\ waiter = None
  /\ laterWaiter = None
  /\ rejectedJunkWaiter = None
  /\ physicalCarrier = FALSE
  /\ tombstone = None
  /\ producerChurn = 0

BeginPredrainScenario ==
  /\ phase = "ChooseScenario"
  /\ phase' = "ActivePredrain"
  /\ nextLifecycleOrdinal' = 3
  /\ nextSchedulerOrdinal' = 4
  /\ nextPhysicalOrdinal' = 4
  /\ admission' = Admission(1)
  /\ reservation' = Reservation(1, "Unsealed")
  /\ waiter' = Waiter(1, 1, 1, 1)
  /\ laterWaiter' = Waiter(2, 2, 2, 2)
  \* This is a physically admitted deterministic policy rejection, not a
  \* lifecycle owner.  Restart must never convert it to scheduler debt.
  /\ rejectedJunkWaiter' = Waiter(3, 3, 0, 3)
  /\ physicalCarrier' = TRUE
  /\ UNCHANGED <<tombstone, producerChurn>>

BeginMaterializedScenario ==
  /\ phase = "ChooseScenario"
  /\ phase' = "Materialized"
  /\ nextLifecycleOrdinal' = 2
  /\ nextSchedulerOrdinal' = 2
  /\ nextPhysicalOrdinal' = 2
  /\ admission' = Admission(1)
  /\ reservation' = Reservation(1, "Materialized")
  /\ waiter' = None
  /\ laterWaiter' = None
  /\ rejectedJunkWaiter' = None
  /\ physicalCarrier' = TRUE
  /\ UNCHANGED <<tombstone, producerChurn>>

RestartPredrain ==
  /\ phase = "ActivePredrain"
  /\ IF PreserveRestartDebt
     THEN /\ admission' = admission
          /\ reservation' = Reservation(1, "AwaitingRetry")
          /\ waiter' = Waiter(0, 1, 1, 0)
          /\ laterWaiter' = Waiter(0, 2, 2, 0)
     ELSE /\ admission' = None
          /\ reservation' = None
          /\ waiter' = None
          /\ laterWaiter' = None
  /\ rejectedJunkWaiter' =
       IF DropRejectedJunk
       THEN None
       ELSE Waiter(0, 3, 0, 0)
  /\ physicalCarrier' = FALSE
  /\ phase' = "RestartedPredrain"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, tombstone, producerChurn>>

RestartPostDrain ==
  /\ phase = "Materialized"
  /\ admission' =
       IF PreserveRestartDebt THEN admission ELSE None
  \* The old physical occurrence already consumed scheduler ordinal one.
  \* Reset keeps no ingress waiter or future-slot capacity reservation.
  /\ reservation' = None
  /\ waiter' = None
  /\ laterWaiter' = None
  /\ rejectedJunkWaiter' = None
  /\ physicalCarrier' = FALSE
  /\ phase' = "RestartedPostDrain"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, tombstone, producerChurn>>

PendingReceiverClose ==
  /\ phase = "ActivePredrain"
  /\ IF PreserveCommittedRollbackLifecycle
     THEN /\ admission' = admission
          /\ reservation' = Reservation(1, "AwaitingRetry")
          /\ waiter' = Waiter(0, 1, 1, 0)
     ELSE /\ admission' = None
          /\ reservation' = None
          /\ waiter' = None
  /\ laterWaiter' = None
  /\ rejectedJunkWaiter' = None
  /\ physicalCarrier' = FALSE
  /\ phase' = "PendingClosed"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, tombstone, producerChurn>>

MaterializedReceiverClose ==
  /\ phase = "Materialized"
  /\ admission' =
       IF PreserveCommittedRollbackLifecycle
       THEN admission
       ELSE None
  /\ reservation' = None
  /\ waiter' = None
  /\ laterWaiter' = None
  /\ rejectedJunkWaiter' = None
  /\ physicalCarrier' = FALSE
  /\ phase' = "MaterializedClosed"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, tombstone, producerChurn>>

RetryPredrain ==
  /\ phase \in {"RestartedPredrain", "PendingClosed"}
  /\ LET reuse ==
           /\ admission # None
           /\ reservation # None
           /\ waiter # None
           /\ PreserveRetryIds
         lifecycleOrdinal ==
           IF reuse
           THEN admission.lifecycleOrdinal
           ELSE nextLifecycleOrdinal
         schedulerOrdinal ==
           IF reuse
           THEN waiter.schedulerOrdinal
           ELSE nextSchedulerOrdinal
     IN /\ admission' = Admission(lifecycleOrdinal)
        /\ reservation' =
             Reservation(lifecycleOrdinal, "AwaitingRetry")
        /\ waiter' =
             Waiter(
               nextPhysicalOrdinal, schedulerOrdinal,
               lifecycleOrdinal, 0)
        /\ nextLifecycleOrdinal' =
             IF reuse
             THEN nextLifecycleOrdinal
             ELSE nextLifecycleOrdinal + 1
        /\ nextSchedulerOrdinal' =
             IF reuse
             THEN nextSchedulerOrdinal
             ELSE nextSchedulerOrdinal + 1
  /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ physicalCarrier' = TRUE
  /\ phase' = "RetriedPredrain"
  /\ UNCHANGED
       <<laterWaiter, rejectedJunkWaiter,
         tombstone, producerChurn>>

RetryPostDrain ==
  /\ phase \in {"RestartedPostDrain", "MaterializedClosed"}
  /\ LET lifecycleRetained ==
           IF phase = "RestartedPostDrain"
           THEN PreserveRestartDebt
           ELSE PreserveCommittedRollbackLifecycle
         lifecycleOrdinal ==
           IF /\ lifecycleRetained
              /\ PreserveRetryIds
              /\ admission # None
           THEN admission.lifecycleOrdinal
           ELSE nextLifecycleOrdinal
         schedulerOrdinal ==
           IF RetireDrainedScheduler
           THEN nextSchedulerOrdinal
           ELSE 1
     IN /\ admission' = Admission(lifecycleOrdinal)
        /\ reservation' =
             Reservation(lifecycleOrdinal, "AwaitingRetry")
        /\ waiter' =
             Waiter(
               nextPhysicalOrdinal, schedulerOrdinal,
               lifecycleOrdinal, 0)
        /\ nextLifecycleOrdinal' =
             IF lifecycleOrdinal = nextLifecycleOrdinal
             THEN nextLifecycleOrdinal + 1
             ELSE nextLifecycleOrdinal
        /\ nextSchedulerOrdinal' =
             IF RetireDrainedScheduler
             THEN nextSchedulerOrdinal + 1
             ELSE nextSchedulerOrdinal
  /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ physicalCarrier' = TRUE
  /\ phase' = "RetriedPostDrain"
  /\ UNCHANGED
       <<laterWaiter, rejectedJunkWaiter,
         tombstone, producerChurn>>

RetryLaterDormant ==
  /\ phase = "RestartedPredrain"
  /\ ~RejectLaterDormantRetry
  /\ laterWaiter # None
  /\ waiter # None
  /\ laterWaiter.schedulerOrdinal > waiter.schedulerOrdinal
  /\ laterWaiter' =
       Waiter(
         nextPhysicalOrdinal, laterWaiter.schedulerOrdinal,
         laterWaiter.lifecycleOrdinal, 0)
  /\ physicalCarrier' = TRUE
  /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
  /\ phase' = "LaterRetried"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         admission, reservation, waiter, rejectedJunkWaiter,
         tombstone, producerChurn>>

DormantProducerChurn ==
  /\ phase = "RestartedPredrain"
  /\ ~BlockDormantProducerChurn
  /\ producerChurn' = producerChurn + 1
  /\ UNCHANGED
       <<phase, nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, admission, reservation, waiter,
         laterWaiter, rejectedJunkWaiter, physicalCarrier, tombstone>>

Complete ==
  /\ phase = "RetriedPredrain"
  /\ tombstone' = Tombstone(admission.lifecycleOrdinal)
  /\ admission' = None
  /\ reservation' = None
  /\ waiter' = None
  /\ physicalCarrier' = FALSE
  /\ phase' = "Completed"
  /\ UNCHANGED
       <<nextLifecycleOrdinal, nextSchedulerOrdinal,
         nextPhysicalOrdinal, laterWaiter, rejectedJunkWaiter,
         producerChurn>>

RetryCompleted ==
  /\ phase = "Completed"
  /\ IF BlockCompletedResurrection
     THEN /\ admission' = admission
          /\ reservation' = reservation
          /\ waiter' = waiter
          /\ physicalCarrier' = physicalCarrier
          /\ phase' = "Terminal"
          /\ UNCHANGED
               <<nextLifecycleOrdinal, nextSchedulerOrdinal,
                 nextPhysicalOrdinal>>
     ELSE /\ admission' = Admission(nextLifecycleOrdinal)
          /\ reservation' =
               Reservation(nextLifecycleOrdinal, "AwaitingRetry")
          /\ waiter' =
               Waiter(
                 nextPhysicalOrdinal, nextSchedulerOrdinal,
                 nextLifecycleOrdinal, 0)
          /\ physicalCarrier' = TRUE
          /\ nextLifecycleOrdinal' = nextLifecycleOrdinal + 1
          /\ nextSchedulerOrdinal' = nextSchedulerOrdinal + 1
          /\ nextPhysicalOrdinal' = nextPhysicalOrdinal + 1
          /\ phase' = "Resurrected"
  /\ UNCHANGED
       <<laterWaiter, rejectedJunkWaiter,
         tombstone, producerChurn>>

Next ==
  \/ BeginPredrainScenario
  \/ BeginMaterializedScenario
  \/ RestartPredrain
  \/ RestartPostDrain
  \/ PendingReceiverClose
  \/ MaterializedReceiverClose
  \/ RetryPredrain
  \/ RetryPostDrain
  \/ RetryLaterDormant
  \/ DormantProducerChurn
  \/ Complete
  \/ RetryCompleted

Spec == Init /\ [][Next]_vars

RestartRetainsUnsealedServeDebt ==
  phase = "RestartedPredrain"
    => /\ admission = Admission(1)
       /\ reservation = Reservation(1, "AwaitingRetry")

RestartedPredrainWaitersAreDormantWithoutPhysicalRecharge ==
  phase = "RestartedPredrain"
    => /\ waiter = Waiter(0, 1, 1, 0)
       /\ laterWaiter = Waiter(0, 2, 2, 0)
       /\ ~physicalCarrier

RestartDropsLifecycleZeroPolicyRejection ==
  phase = "RestartedPredrain" => rejectedJunkWaiter = None

PostDrainRestartRetainsLifecycleWithoutSchedulerDebt ==
  phase = "RestartedPostDrain"
    => /\ admission = Admission(1)
       /\ reservation = None
       /\ waiter = None
       /\ ~physicalCarrier

ExactPredrainRetryReactivatesSameLogicalIds ==
  phase = "RetriedPredrain"
    => /\ admission.lifecycleOrdinal = 1
       /\ reservation.lifecycleOrdinal = 1
       /\ waiter.schedulerOrdinal = 1
       /\ waiter.lifecycleOrdinal = 1
       /\ waiter.physicalOrdinal = 4
       /\ physicalCarrier

PostDrainRetryRetainsLifecycleWithFreshScheduler ==
  phase = "RetriedPostDrain"
    => /\ admission.lifecycleOrdinal = 1
       /\ reservation.lifecycleOrdinal = 1
       /\ waiter.lifecycleOrdinal = 1
       /\ waiter.schedulerOrdinal = 2
       /\ waiter.physicalOrdinal = 2
       /\ physicalCarrier

PendingCloseRetainsDormantCommittedSchedulerDebt ==
  phase = "PendingClosed"
    => /\ admission = Admission(1)
       /\ reservation = Reservation(1, "AwaitingRetry")
       /\ waiter = Waiter(0, 1, 1, 0)
       /\ ~physicalCarrier

MaterializedCloseRetainsOnlyAwaitingRetryLifecycle ==
  phase = "MaterializedClosed"
    => /\ admission = Admission(1)
       /\ reservation = None
       /\ waiter = None
       /\ ~physicalCarrier

EarliestDormantWaiterOwnsReactivation ==
  phase # "LaterRetried"

DormantDebtBlocksCausalControlCompletionChurn ==
  phase = "RestartedPredrain" => producerChurn = 0

CompletedTombstoneBlocksResurrection ==
  phase \in {"Completed", "Terminal", "Resurrected"}
    => /\ tombstone = Tombstone(1)
       /\ phase # "Resurrected"
       /\ admission = None
       /\ reservation = None
       /\ waiter = None
       /\ ~physicalCarrier

=============================================================================
