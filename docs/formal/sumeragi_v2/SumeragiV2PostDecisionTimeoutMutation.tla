---- MODULE SumeragiV2PostDecisionTimeoutMutation ----
EXTENDS Naturals, Sequences, TLC

(***************************************************************************
Finite-state adversarial projection of the Decision/timeout boundary.

The trace first installs a durable Decision, then deliberately presents every
timeout-side operation which production may observe after that Decision:

  * replay of one durable timeout-signing intent;
  * direct BeginTimeout, FormTC, and BeginInstallTC attempts;
  * delivery of one authenticated TimeoutVote envelope; and
  * delivery of one authenticated TC envelope.

In repaired mode the three direct attempts return without creating work.  The
two envelopes are consumed, but are not admitted to the receive pools and do
not create asynchronous causal successors.  Each mutation mode removes one
of those seven restrictions.  Every non-Decision precondition is deliberately
assumed satisfied, so no mutant can be hidden behind an unrelated quorum or
authentication guard.  This model is intentionally a bounded regression
witness, not a replacement for the unbounded Core/AsyncNetwork proof
obligations.  The replay mutation is independent of the live BeginTimeout
guard: a durable timeout intent may predate Decision and must still be
suppressed during recovery.
***************************************************************************)

CONSTANT Mode

Modes ==
  {"Fixed",
   "NoResumeTimeoutGuard",
   "NoBeginTimeoutGuard",
   "NoFormTCGuard",
   "NoBeginInstallTCGuard",
   "RecordTimeoutAfterDecision",
   "RecordTCAfterDecision",
   "TimeoutSuccessorAfterDecision",
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
   "AttemptFormTC",
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
                  pendingInstallTCs,
                  causalQueue>>

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
  /\ phase' = "AttemptFormTC"
  /\ pendingTimeouts' =
       IF BeginTimeoutAllowed THEN pendingTimeouts + 1
       ELSE pendingTimeouts
  /\ lastTransition' = "AttemptBeginTimeout"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  timeoutSignatures, formedTCs, pendingInstallTCs,
                  causalQueue>>

FormTCAllowed ==
  ~decided \/ Mode = "NoFormTCGuard"

AttemptFormTC ==
  /\ phase = "AttemptFormTC"
  /\ phase' = "AttemptBeginInstallTC"
  /\ formedTCs' = IF FormTCAllowed THEN formedTCs + 1 ELSE formedTCs
  /\ lastTransition' = "AttemptFormTC"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures, pendingInstallTCs,
                  causalQueue>>

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
                  pendingInstallTCs,
                  causalQueue>>

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
  /\ causalQueue' =
       IF TimeoutSuccessorAllowed
       THEN Append(causalQueue, "FormTC")
       ELSE causalQueue
  /\ lastTransition' = "DeliverTimeout"
  /\ UNCHANGED <<decided, tcEnvelope, tcConsumed, receivedTCs,
                  pendingTimeouts, timeoutSignatures, formedTCs,
                  pendingInstallTCs>>

DispatchTimeoutSuccessor ==
  /\ phase = "DispatchTimeoutSuccessor"
  /\ phase' = "InjectTC"
  /\ formedTCs' =
       IF causalQueue # <<>>
          /\ Head(causalQueue) = "FormTC"
          /\ FormTCAllowed
       THEN formedTCs + 1
       ELSE formedTCs
  /\ causalQueue' =
       IF causalQueue # <<>> /\ Head(causalQueue) = "FormTC"
       THEN Tail(causalQueue)
       ELSE causalQueue
  /\ lastTransition' = "DispatchTimeoutSuccessor"
  /\ UNCHANGED <<decided, timeoutEnvelope, tcEnvelope,
                  timeoutConsumed, tcConsumed,
                  receivedTimeoutVotes, receivedTCs,
                  pendingTimeouts, timeoutSignatures,
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
                  pendingInstallTCs,
                  causalQueue>>

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
  \/ AttemptFormTC
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
  /\ WF_vars(AttemptFormTC)
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
  /\ causalQueue \in Seq({"FormTC", "BeginInstallTC"})
  /\ lastTransition \in
       {"Init", "InstallDecision", "AttemptResumeTimeout",
        "AttemptBeginTimeout", "AttemptFormTC",
        "AttemptBeginInstallTC", "InjectTimeoutEnvelope", "DeliverTimeout",
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

TimeoutDeliveryConsumesWithoutAdmission ==
  lastTransition = "DeliverTimeout" =>
    /\ ~timeoutEnvelope
    /\ timeoutConsumed
    /\ receivedTimeoutVotes = 0

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
  /\ TimeoutDeliveryConsumesWithoutAdmission
  /\ TCDeliveryConsumesWithoutAdmission
  /\ TimeoutDeliveryHasNoCausalSuccessor
  /\ TCDeliveryHasNoCausalSuccessor

PostDecisionTimeoutTrafficEventuallyConsumed ==
  (decided /\ timeoutEnvelope) ~> timeoutConsumed

PostDecisionTCTrafficEventuallyConsumed ==
  (decided /\ tcEnvelope) ~> tcConsumed

=============================================================================
