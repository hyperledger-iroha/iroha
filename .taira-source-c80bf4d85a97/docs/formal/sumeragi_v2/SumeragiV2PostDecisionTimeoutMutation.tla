---- MODULE SumeragiV2PostDecisionTimeoutMutation ----
EXTENDS Naturals, Sequences, TLC

(***************************************************************************
Finite-state adversarial projection of the atomic Decision/timeout boundary.

The trace first installs a durable Decision, then presents every timeout-side
operation which production may observe after that Decision:

  * replay of one durable timeout-signing intent;
  * direct BeginTimeout and BeginInstallTC attempts;
  * one stale signed-local timeout completion;
  * delivery of one authenticated TimeoutVote envelope; and
  * delivery of one authenticated TC envelope.

Signed-local completion and TimeoutVote delivery are the two production
receipt actions.  Each must atomically do nothing after Decision: neither may
admit a receipt, form a TC, open InstallTC persistence, or publish a
PersistInstallTC causal child.  The TC envelope is consumed without receive
pool admission or a BeginInstallTC child.  Every non-Decision precondition is
deliberately assumed satisfied, so no mutant can hide behind an unrelated
quorum or authentication guard.

This bounded matrix is regression evidence, not a replacement for the
unbounded Core/AsyncNetwork proof.  In particular, the replay mutation is
independent of BeginTimeout: a durable timeout intent may predate Decision and
must still be suppressed during recovery.
***************************************************************************)

CONSTANT Mode

Modes ==
  {"Fixed",
   "NoResumeTimeoutGuard",
   "NoBeginTimeoutGuard",
   "NoCompleteTimeoutGuard",
   "LocalTimeoutSuccessorAfterDecision",
   "NoBeginInstallTCGuard",
   "RecordTimeoutAfterDecision",
   "TimeoutSuccessorAfterDecision",
   "RecordTCAfterDecision",
   "TCSuccessorAfterDecision"}

VARIABLES
  phase,
  decided,
  timeoutEnvelope,
  tcEnvelope,
  timeoutConsumed,
  tcConsumed,
  receivedTimeoutVotes,
  receivedTCs,
  pendingTimeouts,
  timeoutSignatures,
  formedTCs,
  pendingInstallTCs,
  causalQueue,
  lastTransition

vars ==
  <<phase,
    decided,
    timeoutEnvelope,
    tcEnvelope,
    timeoutConsumed,
    tcConsumed,
    receivedTimeoutVotes,
    receivedTCs,
    pendingTimeouts,
    timeoutSignatures,
    formedTCs,
    pendingInstallTCs,
    causalQueue,
    lastTransition>>

Phases ==
  {"AwaitDecision",
   "AttemptResumeTimeout",
   "AttemptBeginTimeout",
   "AttemptCompleteTimeoutSignature",
   "DispatchLocalTimeoutSuccessor",
   "AttemptBeginInstallTC",
   "InjectTimeout",
   "DeliverTimeout",
   "DispatchTimeoutSuccessor",
   "InjectTC",
   "DeliverTC",
   "DispatchTCSuccessor",
   "Done"}

Init ==
  /\ phase = "AwaitDecision"
  /\ decided = FALSE
  /\ timeoutEnvelope = FALSE
  /\ tcEnvelope = FALSE
  /\ timeoutConsumed = FALSE
  /\ tcConsumed = FALSE
  /\ receivedTimeoutVotes = 0
  /\ receivedTCs = 0
  /\ pendingTimeouts = 0
  /\ timeoutSignatures = 0
  /\ formedTCs = 0
  /\ pendingInstallTCs = 0
  /\ causalQueue = <<>>
  /\ lastTransition = "Init"

InstallDecision ==
  /\ phase = "AwaitDecision"
  /\ phase' = "AttemptResumeTimeout"
  /\ decided' = TRUE
  /\ lastTransition' = "InstallDecision"
  /\ UNCHANGED <<timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs, causalQueue>>

(***************************************************************************
Recovery may still hold a durable timeout intent which predates Decision.
The replay action needs its own terminal guard; relying on BeginTimeout is
insufficient because replay does not create a new timeout WAL intent.
***************************************************************************)
ResumeTimeoutAllowed ==
  ~decided \/ Mode = "NoResumeTimeoutGuard"

AttemptResumeTimeout ==
  /\ phase = "AttemptResumeTimeout"
  /\ phase' = "AttemptBeginTimeout"
  /\ timeoutSignatures' =
       IF ResumeTimeoutAllowed THEN timeoutSignatures + 1
       ELSE timeoutSignatures
  /\ lastTransition' = "AttemptResumeTimeout"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, formedTCs, pendingInstallTCs,
                  causalQueue>>

BeginTimeoutAllowed ==
  ~decided \/ Mode = "NoBeginTimeoutGuard"

AttemptBeginTimeout ==
  /\ phase = "AttemptBeginTimeout"
  /\ phase' = "AttemptCompleteTimeoutSignature"
  /\ pendingTimeouts' =
       IF BeginTimeoutAllowed THEN pendingTimeouts + 1
       ELSE pendingTimeouts
  /\ lastTransition' = "AttemptBeginTimeout"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  timeoutSignatures, formedTCs, pendingInstallTCs,
                  causalQueue>>

(***************************************************************************
CompleteTimeoutSignature and DeliverTimeout each own receipt admission, TC
formation, and the InstallTC WAL request in one reducer turn.  A stale local
completion after Decision must therefore be an atomic no-op, including at the
causal-successor adapter boundary.
***************************************************************************)
CompleteTimeoutAllowed ==
  ~decided \/ Mode = "NoCompleteTimeoutGuard"

LocalTimeoutSuccessorAllowed ==
  ~decided \/ Mode = "LocalTimeoutSuccessorAfterDecision"

AttemptCompleteTimeoutSignature ==
  /\ phase = "AttemptCompleteTimeoutSignature"
  /\ phase' = "DispatchLocalTimeoutSuccessor"
  /\ receivedTimeoutVotes' =
       IF CompleteTimeoutAllowed
       THEN receivedTimeoutVotes + 1
       ELSE receivedTimeoutVotes
  /\ formedTCs' =
       IF CompleteTimeoutAllowed THEN formedTCs + 1 ELSE formedTCs
  /\ pendingInstallTCs' =
       IF CompleteTimeoutAllowed
       THEN pendingInstallTCs + 1
       ELSE pendingInstallTCs
  /\ causalQueue' =
       IF LocalTimeoutSuccessorAllowed
       THEN Append(causalQueue, "PersistInstallTC")
       ELSE causalQueue
  /\ lastTransition' = "AttemptCompleteTimeoutSignature"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed, receivedTCs,
                  pendingTimeouts, timeoutSignatures>>

DispatchLocalTimeoutSuccessor ==
  /\ phase = "DispatchLocalTimeoutSuccessor"
  /\ phase' = "AttemptBeginInstallTC"
  /\ causalQueue' =
       IF causalQueue # <<>> /\ Head(causalQueue) = "PersistInstallTC"
       THEN Tail(causalQueue)
       ELSE causalQueue
  /\ lastTransition' = "DispatchLocalTimeoutSuccessor"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs>>

BeginInstallTCAllowed ==
  ~decided \/ Mode = "NoBeginInstallTCGuard"

AttemptBeginInstallTC ==
  /\ phase = "AttemptBeginInstallTC"
  /\ phase' = "InjectTimeout"
  /\ pendingInstallTCs' =
       IF BeginInstallTCAllowed THEN pendingInstallTCs + 1
       ELSE pendingInstallTCs
  /\ lastTransition' = "AttemptBeginInstallTC"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  causalQueue>>

InjectTimeoutEnvelope ==
  /\ phase = "InjectTimeout"
  /\ phase' = "DeliverTimeout"
  /\ timeoutEnvelope' = TRUE
  /\ lastTransition' = "InjectTimeoutEnvelope"
  /\ UNCHANGED <<decided, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs, causalQueue>>

TimeoutAdmissionAllowed ==
  ~decided \/ Mode = "RecordTimeoutAfterDecision"

TimeoutSuccessorAllowed ==
  ~decided \/ Mode = "TimeoutSuccessorAfterDecision"

DeliverTimeout ==
  /\ phase = "DeliverTimeout"
  /\ timeoutEnvelope
  /\ phase' = "DispatchTimeoutSuccessor"
  /\ timeoutEnvelope' = FALSE
  /\ timeoutConsumed' = TRUE
  /\ receivedTimeoutVotes' =
       IF TimeoutAdmissionAllowed
       THEN receivedTimeoutVotes + 1
       ELSE receivedTimeoutVotes
  /\ formedTCs' =
       IF TimeoutAdmissionAllowed THEN formedTCs + 1 ELSE formedTCs
  /\ pendingInstallTCs' =
       IF TimeoutAdmissionAllowed
       THEN pendingInstallTCs + 1
       ELSE pendingInstallTCs
  /\ causalQueue' =
       IF TimeoutSuccessorAllowed
       THEN Append(causalQueue, "PersistInstallTC")
       ELSE causalQueue
  /\ lastTransition' = "DeliverTimeout"
  /\ UNCHANGED <<decided, tcEnvelope, tcConsumed, receivedTCs,
                  pendingTimeouts, timeoutSignatures>>

DispatchTimeoutSuccessor ==
  /\ phase = "DispatchTimeoutSuccessor"
  /\ phase' = "InjectTC"
  /\ causalQueue' =
       IF causalQueue # <<>> /\ Head(causalQueue) = "PersistInstallTC"
       THEN Tail(causalQueue)
       ELSE causalQueue
  /\ lastTransition' = "DispatchTimeoutSuccessor"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs>>

InjectTCEnvelope ==
  /\ phase = "InjectTC"
  /\ phase' = "DeliverTC"
  /\ tcEnvelope' = TRUE
  /\ lastTransition' = "InjectTCEnvelope"
  /\ UNCHANGED <<decided, timeoutEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs, causalQueue>>

TCAdmissionAllowed ==
  ~decided \/ Mode = "RecordTCAfterDecision"

TCSuccessorAllowed ==
  ~decided \/ Mode = "TCSuccessorAfterDecision"

DeliverTC ==
  /\ phase = "DeliverTC"
  /\ tcEnvelope
  /\ phase' = "DispatchTCSuccessor"
  /\ tcEnvelope' = FALSE
  /\ tcConsumed' = TRUE
  /\ receivedTCs' =
       IF TCAdmissionAllowed THEN receivedTCs + 1 ELSE receivedTCs
  /\ causalQueue' =
       IF TCSuccessorAllowed
       THEN Append(causalQueue, "BeginInstallTC")
       ELSE causalQueue
  /\ lastTransition' = "DeliverTC"
  /\ UNCHANGED <<decided, timeoutEnvelope, timeoutConsumed,
                  receivedTimeoutVotes, pendingTimeouts, timeoutSignatures,
                  formedTCs, pendingInstallTCs>>

DispatchTCSuccessor ==
  /\ phase = "DispatchTCSuccessor"
  /\ phase' = "Done"
  /\ pendingInstallTCs' =
       IF causalQueue # <<>>
          /\ Head(causalQueue) = "BeginInstallTC"
          /\ BeginInstallTCAllowed
       THEN pendingInstallTCs + 1
       ELSE pendingInstallTCs
  /\ causalQueue' =
       IF causalQueue # <<>> /\ Head(causalQueue) = "BeginInstallTC"
       THEN Tail(causalQueue)
       ELSE causalQueue
  /\ lastTransition' = "DispatchTCSuccessor"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs>>

RemainDone ==
  /\ phase = "Done"
  /\ UNCHANGED vars

Next ==
  \/ InstallDecision
  \/ AttemptResumeTimeout
  \/ AttemptBeginTimeout
  \/ AttemptCompleteTimeoutSignature
  \/ DispatchLocalTimeoutSuccessor
  \/ AttemptBeginInstallTC
  \/ InjectTimeoutEnvelope
  \/ DeliverTimeout
  \/ DispatchTimeoutSuccessor
  \/ InjectTCEnvelope
  \/ DeliverTC
  \/ DispatchTCSuccessor
  \/ RemainDone

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(InstallDecision)
  /\ WF_vars(AttemptResumeTimeout)
  /\ WF_vars(AttemptBeginTimeout)
  /\ WF_vars(AttemptCompleteTimeoutSignature)
  /\ WF_vars(DispatchLocalTimeoutSuccessor)
  /\ WF_vars(AttemptBeginInstallTC)
  /\ WF_vars(InjectTimeoutEnvelope)
  /\ WF_vars(DeliverTimeout)
  /\ WF_vars(DispatchTimeoutSuccessor)
  /\ WF_vars(InjectTCEnvelope)
  /\ WF_vars(DeliverTC)
  /\ WF_vars(DispatchTCSuccessor)

TypeInvariant ==
  /\ Mode \in Modes
  /\ phase \in Phases
  /\ decided \in BOOLEAN
  /\ timeoutEnvelope \in BOOLEAN
  /\ tcEnvelope \in BOOLEAN
  /\ timeoutConsumed \in BOOLEAN
  /\ tcConsumed \in BOOLEAN
  /\ receivedTimeoutVotes \in Nat
  /\ receivedTCs \in Nat
  /\ pendingTimeouts \in Nat
  /\ timeoutSignatures \in Nat
  /\ formedTCs \in Nat
  /\ pendingInstallTCs \in Nat
  /\ causalQueue \in Seq({"PersistInstallTC", "BeginInstallTC"})
  /\ lastTransition \in
       {"Init", "InstallDecision", "AttemptResumeTimeout",
        "AttemptBeginTimeout", "AttemptCompleteTimeoutSignature",
        "DispatchLocalTimeoutSuccessor", "AttemptBeginInstallTC",
        "InjectTimeoutEnvelope", "DeliverTimeout",
        "DispatchTimeoutSuccessor", "InjectTCEnvelope", "DeliverTC",
        "DispatchTCSuccessor"}

NoTimeoutIntentAfterDecision ==
  decided => pendingTimeouts = 0

NoTimeoutSignatureAfterDecision ==
  decided => timeoutSignatures = 0

NoTCFormationAfterDecision ==
  decided => formedTCs = 0

NoTCInstallAfterDecision ==
  decided => pendingInstallTCs = 0

LocalTimeoutCompletionIsAtomicNoOp ==
  lastTransition = "AttemptCompleteTimeoutSignature" =>
    /\ receivedTimeoutVotes = 0
    /\ formedTCs = 0
    /\ pendingInstallTCs = 0

LocalTimeoutCompletionHasNoCausalSuccessor ==
  lastTransition = "AttemptCompleteTimeoutSignature" =>
    causalQueue = <<>>

TimeoutDeliveryConsumesWithoutAtomicAdmission ==
  lastTransition = "DeliverTimeout" =>
    /\ ~timeoutEnvelope
    /\ timeoutConsumed
    /\ receivedTimeoutVotes = 0
    /\ formedTCs = 0
    /\ pendingInstallTCs = 0

TCDeliveryConsumesWithoutAdmission ==
  lastTransition = "DeliverTC" =>
    /\ ~tcEnvelope
    /\ tcConsumed
    /\ receivedTCs = 0

TimeoutDeliveryHasNoCausalSuccessor ==
  lastTransition = "DeliverTimeout" => causalQueue = <<>>

TCDeliveryHasNoCausalSuccessor ==
  lastTransition = "DeliverTC" => causalQueue = <<>>

PostDecisionBoundarySafe ==
  /\ NoTimeoutIntentAfterDecision
  /\ NoTimeoutSignatureAfterDecision
  /\ NoTCFormationAfterDecision
  /\ NoTCInstallAfterDecision
  /\ LocalTimeoutCompletionIsAtomicNoOp
  /\ LocalTimeoutCompletionHasNoCausalSuccessor
  /\ TimeoutDeliveryConsumesWithoutAtomicAdmission
  /\ TCDeliveryConsumesWithoutAdmission
  /\ TimeoutDeliveryHasNoCausalSuccessor
  /\ TCDeliveryHasNoCausalSuccessor

PostDecisionTimeoutTrafficEventuallyConsumed ==
  (decided /\ timeoutEnvelope) ~> timeoutConsumed

PostDecisionTCTrafficEventuallyConsumed ==
  (decided /\ tcEnvelope) ~> tcConsumed

=============================================================================
