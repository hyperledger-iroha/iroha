---- MODULE SumeragiV2DeferredBusyFenceMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded scheduler-priority witness for a deferred owner behind a serialized
Busy owner.  The retired priority rule repeatedly selects deferred work while
the reducer is Busy; the command is rejected and requeued, so neither the
ordinary matching Completion nor the deferred owner can retire.

The production fence does not select deferred work while Busy.  It services
the matching ordinary Completion first, which clears the serialized owner,
then selects and retires the deferred command without a retry/requeue cycle.
***************************************************************************)

VARIABLES busyOwner, ordinaryCompletionOwned,
          deferredProgressOwned, drainOwed, attemptParity

vars ==
  <<busyOwner, ordinaryCompletionOwned,
    deferredProgressOwned, drainOwed, attemptParity>>

Init ==
  /\ busyOwner = TRUE
  /\ ordinaryCompletionOwned = TRUE
  /\ deferredProgressOwned = TRUE
  /\ drainOwed = TRUE
  /\ attemptParity = FALSE

BusyDeferredRetry ==
  /\ busyOwner = TRUE
  /\ ordinaryCompletionOwned = TRUE
  /\ deferredProgressOwned = TRUE
  /\ drainOwed = TRUE
  /\ UNCHANGED <<busyOwner, ordinaryCompletionOwned,
                  deferredProgressOwned, drainOwed>>
  /\ attemptParity' = ~attemptParity

ServiceOrdinaryCompletion ==
  /\ busyOwner = TRUE
  /\ ordinaryCompletionOwned = TRUE
  /\ busyOwner' = FALSE
  /\ ordinaryCompletionOwned' = FALSE
  /\ UNCHANGED <<deferredProgressOwned, drainOwed>>
  /\ attemptParity' = ~attemptParity

DrainDeferredProgress ==
  /\ busyOwner = FALSE
  /\ ordinaryCompletionOwned = FALSE
  /\ deferredProgressOwned = TRUE
  /\ drainOwed = TRUE
  /\ deferredProgressOwned' = FALSE
  /\ UNCHANGED <<busyOwner, ordinaryCompletionOwned, drainOwed>>
  /\ attemptParity' = ~attemptParity

RetryPrioritySpec ==
  /\ Init
  /\ [][BusyDeferredRetry]_vars
  /\ WF_vars(BusyDeferredRetry)

FencedNext == ServiceOrdinaryCompletion \/ DrainDeferredProgress

FencedSpec ==
  /\ Init
  /\ [][FencedNext]_vars
  /\ WF_vars(ServiceOrdinaryCompletion)
  /\ WF_vars(DrainDeferredProgress)

OrdinaryCompletionEventuallyServiced ==
  (ordinaryCompletionOwned = TRUE)
    ~> (ordinaryCompletionOwned = FALSE)

DeferredProgressEventuallyServiced ==
  (deferredProgressOwned = TRUE)
    ~> (deferredProgressOwned = FALSE)

=============================================================================
