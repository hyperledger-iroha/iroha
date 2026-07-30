---- MODULE SumeragiV2DeferredCursorEffectMutation ----
EXTENDS Naturals, Sequences

(***************************************************************************
Exact regression for the deferred Completion/Progress cursor boundary.

Both queues are nonempty and the cyclic cursor points at Progress.  The
production selector must therefore remove the Progress head even though a
Completion command is also waiting.  The retired connection-generation-era
projection incorrectly treated Completion as an unconditional priority and
claimed that the Progress queue was unchanged in this state.

The bug configuration checks that retired projection and must fail after the
single exact removal.  The fixed configuration checks the cursor-aware
projection and must pass.  Keeping both queues and the cursor fixed in Init
prevents either result from being discharged by an empty-queue antecedent.
***************************************************************************)

VARIABLES completionQueue, progressQueue, nextDeferredClass, removed,
          busyRetried

CompletionCommand == [class |-> "Completion", kind |-> "Persist"]
ProgressCommand == [class |-> "Progress", kind |-> "DeliverQC"]

vars ==
  <<completionQueue, progressQueue, nextDeferredClass, removed, busyRetried>>

Init ==
  \/ /\ completionQueue = <<CompletionCommand>>
     /\ progressQueue = <<ProgressCommand>>
     /\ nextDeferredClass = "Progress"
     /\ removed = FALSE
     /\ busyRetried = FALSE
  \/ /\ completionQueue = <<>>
     /\ progressQueue = <<ProgressCommand>>
     /\ nextDeferredClass = "Progress"
     /\ removed = FALSE
     /\ busyRetried = FALSE

RemoveSelectedProgress ==
  /\ ~removed
  /\ completionQueue # <<>>
  /\ progressQueue # <<>>
  /\ nextDeferredClass = "Progress"
  /\ UNCHANGED completionQueue
  /\ progressQueue' = Tail(progressQueue)
  /\ nextDeferredClass' = "Normal"
  /\ removed' = TRUE
  /\ busyRetried' = FALSE

BusyRetrySelectedProgress ==
  /\ ~removed
  /\ ~busyRetried
  /\ completionQueue = <<>>
  /\ progressQueue = <<ProgressCommand>>
  /\ nextDeferredClass = "Progress"
  /\ UNCHANGED <<completionQueue, progressQueue, removed>>
  /\ nextDeferredClass' = "Normal"
  /\ busyRetried' = TRUE

TerminalStutter == (removed \/ busyRetried) /\ UNCHANGED vars

Next ==
  RemoveSelectedProgress \/ BusyRetrySelectedProgress \/ TerminalStutter

Spec == Init /\ [][Next]_vars

CursorBoundaryReached ==
  removed
    => /\ completionQueue = <<CompletionCommand>>
       /\ progressQueue = <<>>
       /\ nextDeferredClass = "Normal"

(***************************************************************************
Retired false projection: Progress was said to drain only when Completion was
empty.  In the exact boundary state Completion remains nonempty while the
Progress head is correctly removed, so this invariant must fail.
***************************************************************************)
GenerationEraProgressQueueEffect ==
  removed => (completionQueue # <<>> => progressQueue = <<ProgressCommand>>)

(***************************************************************************
The Busy branch keeps the selected deferred item in place and advances only
the cyclic cursor.  Therefore the service rank may transiently rise from
3 * 1 + 0 = 3 to 3 * 1 + 2 = 5.  This positive invariant prevents a future
Stage-2 proof from reintroducing the false global-UNLESS shortcut; Busy must
instead be discharged by its own terminating local-work rank.
***************************************************************************)
NextClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

ClassDistance(fromClass, toClass) ==
  IF fromClass = toClass
  THEN 0
  ELSE IF NextClass(fromClass) = toClass THEN 1 ELSE 2

TargetRank ==
  3 * Len(progressQueue)
    + ClassDistance(nextDeferredClass, "Progress")

BusyRetryRaisesTargetRank == busyRetried => TargetRank = 5

RetiredDeferredRankNonregression == TargetRank <= 3

=============================================================================
