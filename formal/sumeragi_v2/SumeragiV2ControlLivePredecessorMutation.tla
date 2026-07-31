---- MODULE SumeragiV2ControlLivePredecessorMutation ----
EXTENDS TLC, Naturals, Sequences, FiniteSets

(***************************************************************************
Finite mutation model for strict-newer control admission.

An exact TimeoutCertificate for view 0 already owns the bounded abstract
control slot and its immutable Candidate.  A view-1 certificate from the same
source and control class then reaches the head of the abstract source FIFO.
The repaired transition leaves that newer packet pending until the abstract
service boundary consumes the incumbent.  Only then may the slot advance and
allocate the newer Candidate.

Production refines this atomic boundary with two queues.  FairV2Ingress first
prevents view 1 from bypassing view 0 while both are outer-queued.  After view
0 crosses into `BoundedIngress`, view 1 may cross too, but it receives a later
actor-global lifecycle ordinal.  The runtime minimum-lifecycle selector and
its Busy-deferred/effect ownership preserve view 0 until reducer service.  A
FairV2Ingress dequeue is therefore not called service by this mutation model.

The mutation recreates the replenishment lasso cut: it lets packet admission
replace the unconsumed record and acquire a later Candidate before the admitted
target receives its runner turn.  The old Candidate then has no current slot
authority.  This small model checks the ownership cut only; it is regression
evidence and is not used as deductive liveness proof.
***************************************************************************)

CONSTANT RequireConsumedPredecessor

ASSUME RequireConsumedPredecessor \in BOOLEAN

OldItem == "source/TimeoutCertificate/view-0"
NewItem == "source/TimeoutCertificate/view-1"
NoRecord == "none"

ServiceRecord(item, roundView, ordinal, consumed) ==
  [item |-> item,
   view |-> roundView,
   ordinal |-> ordinal,
   consumed |-> consumed]

OldRecord(consumed) == ServiceRecord(OldItem, 0, 1, consumed)
NewRecord(consumed) == ServiceRecord(NewItem, 1, 2, consumed)

RecordCarrier ==
  {NoRecord,
   OldRecord(FALSE), OldRecord(TRUE),
   NewRecord(FALSE), NewRecord(TRUE)}

QueueCarrier ==
  {<<>>, <<OldItem>>, <<NewItem>>, <<OldItem, NewItem>>}

VARIABLES phase, slotRecord, packetQueue, candidateQueue, installedViews

vars ==
  <<phase, slotRecord, packetQueue, candidateQueue, installedViews>>

TypeInvariant ==
  /\ phase \in
       {"Fresh", "OldAdmitted", "NewerAttempted",
        "OldConsumed", "NewAdmitted", "Done"}
  /\ slotRecord \in RecordCarrier
  /\ packetQueue \in QueueCarrier
  /\ candidateQueue \in QueueCarrier
  /\ installedViews \subseteq {1, 2}

OldAdmissionOwnsImmutableFirstPosition ==
  phase = "OldAdmitted"
    => /\ slotRecord = OldRecord(FALSE)
       /\ packetQueue = <<NewItem>>
       /\ candidateQueue = <<OldItem>>

LivePredecessorRetainsSlotAndPacket ==
  phase = "NewerAttempted"
    => /\ slotRecord = OldRecord(FALSE)
       /\ packetQueue = <<NewItem>>
       /\ candidateQueue = <<OldItem>>

ConsumedPredecessorAllowsStrictAdvance ==
  phase = "NewAdmitted"
    => /\ slotRecord = NewRecord(FALSE)
       /\ packetQueue = <<>>
       /\ candidateQueue = <<NewItem>>
       /\ installedViews = {1}

BothCertificatesInstallInFifoOrder ==
  phase = "Done"
    => /\ slotRecord = NewRecord(TRUE)
       /\ packetQueue = <<>>
       /\ candidateQueue = <<>>
       /\ installedViews = {1, 2}

Init ==
  /\ phase = "Fresh"
  /\ slotRecord = NoRecord
  /\ packetQueue = <<OldItem, NewItem>>
  /\ candidateQueue = <<>>
  /\ installedViews = {}

AdmitOld ==
  /\ phase = "Fresh"
  /\ phase' = "OldAdmitted"
  /\ slotRecord' = OldRecord(FALSE)
  /\ packetQueue' = <<NewItem>>
  /\ candidateQueue' = <<OldItem>>
  /\ UNCHANGED installedViews

AttemptNewerBeforeOldService ==
  /\ phase = "OldAdmitted"
  /\ phase' = "NewerAttempted"
  /\ IF RequireConsumedPredecessor
     THEN /\ UNCHANGED <<slotRecord, packetQueue, candidateQueue>>
     ELSE /\ slotRecord' = NewRecord(FALSE)
          /\ packetQueue' = <<>>
          /\ candidateQueue' = <<OldItem, NewItem>>
  /\ UNCHANGED installedViews

ServiceOld ==
  /\ phase = "NewerAttempted"
  /\ slotRecord = OldRecord(FALSE)
  /\ candidateQueue = <<OldItem>>
  /\ phase' = "OldConsumed"
  /\ slotRecord' = OldRecord(TRUE)
  /\ candidateQueue' = <<>>
  /\ installedViews' = {1}
  /\ UNCHANGED packetQueue

AdmitNewerAfterOldService ==
  /\ phase = "OldConsumed"
  /\ slotRecord = OldRecord(TRUE)
  /\ packetQueue = <<NewItem>>
  /\ phase' = "NewAdmitted"
  /\ slotRecord' = NewRecord(FALSE)
  /\ packetQueue' = <<>>
  /\ candidateQueue' = <<NewItem>>
  /\ UNCHANGED installedViews

ServiceNew ==
  /\ phase = "NewAdmitted"
  /\ phase' = "Done"
  /\ slotRecord' = NewRecord(TRUE)
  /\ candidateQueue' = <<>>
  /\ installedViews' = {1, 2}
  /\ UNCHANGED packetQueue

Next ==
  \/ AdmitOld
  \/ AttemptNewerBeforeOldService
  \/ ServiceOld
  \/ AdmitNewerAfterOldService
  \/ ServiceNew

=============================================================================
