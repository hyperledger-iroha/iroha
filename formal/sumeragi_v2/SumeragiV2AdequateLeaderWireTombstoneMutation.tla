---- MODULE SumeragiV2AdequateLeaderWireTombstoneMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Finite mutation model for the bounded adequate-leader wire lifecycle.

One configured protocol slot admits identity A with immutable receiver-local
admission/scheduler ordinals.  An exact retry coalesces, a same-view foreign
identity cannot replace it, and only exact terminal evidence retires it.

Same-height restart is deliberately split from transport retransmission and
local acceptance.  Restart preserves the record, token, and ordinals but
parks an unconsumed record Dormant: it owns neither a candidate nor an ingress
selector barrier and restart cannot synthesize packet bytes.  A later real
retransmission makes a packet present while the record remains Dormant.
Capacity-blocked acceptance still leaves it Dormant.  Only a separate
capacity-proven atomic acceptance may consume that real packet and reactivate
Ingress ownership, retaining both ordinals and the restart-cleared predecessor
prefix.  Before retransmission, a later target at scheduler ordinal 2 freezes
the older Dormant identity at ordinal 1 in its finite potential-owner budget.
Reactivation consumes that precharged episode; it is not fresh progress and
cannot leapfrog as unaccounted replenishment.  There is no visible Pending
state.  A stable Terminal record never reopens on restart.  The mutations
below weaken these contracts independently while keeping the state space
finite.
***************************************************************************)

CONSTANTS
  KeepSlotTableBounded,
  RequireStrictlyNewerReplacement,
  CoalesceExactRetry,
  DropConsumedRetry,
  AcceptOccupiedUnconsumedAsCompletion,
  RequireExactTerminalIdentity,
  RestartUnconsumedAsDormant,
  SynthesizePacketOnRestart,
  DormantOwnsImmediatelyAfterRestart,
  RequireCapacityForReactivation,
  ReuseDormantOrdinalsOnReactivation,
  PreserveDormantRestartPrefix,
  PrechargeDormantPotentialBeforeLaterOwner,
  PreserveStableTerminalAcrossRestart,
  ResetAtRollover

ASSUME
  /\ KeepSlotTableBounded \in BOOLEAN
  /\ RequireStrictlyNewerReplacement \in BOOLEAN
  /\ CoalesceExactRetry \in BOOLEAN
  /\ DropConsumedRetry \in BOOLEAN
  /\ AcceptOccupiedUnconsumedAsCompletion \in BOOLEAN
  /\ RequireExactTerminalIdentity \in BOOLEAN
  /\ RestartUnconsumedAsDormant \in BOOLEAN
  /\ SynthesizePacketOnRestart \in BOOLEAN
  /\ DormantOwnsImmediatelyAfterRestart \in BOOLEAN
  /\ RequireCapacityForReactivation \in BOOLEAN
  /\ ReuseDormantOrdinalsOnReactivation \in BOOLEAN
  /\ PreserveDormantRestartPrefix \in BOOLEAN
  /\ PrechargeDormantPotentialBeforeLaterOwner \in BOOLEAN
  /\ PreserveStableTerminalAcrossRestart \in BOOLEAN
  /\ ResetAtRollover \in BOOLEAN

Slot == "recipient/source/Proposal"
Slots == {Slot}
TokenA == "context-0/height-0/view-1/subject-A/Proposal"
TokenB == "context-0/height-0/view-1/subject-B/Proposal"
TokenC == "context-0/height-0/view-2/subject-C/Proposal"
IdentityA == TokenA
IdentityB == TokenB
IdentityC == TokenC
ForeignTerminalIdentities ==
  {"context-0/height-0/view-2/subject-A/Proposal",
   "context-0/height-0/view-1/subject-A/PrepareVote"}

Statuses == {"Dormant", "Ingress", "Runtime", "Terminal"}

ServiceRecord(
    blockHeight, roundView, identity, token,
    admissionOrdinal, schedulerOrdinal, status,
    predecessorPrefix, consumed) ==
  [slot |-> Slot,
   height |-> blockHeight,
   view |-> roundView,
   identity |-> identity,
   token |-> token,
   admissionOrdinal |-> admissionOrdinal,
   schedulerOrdinal |-> schedulerOrdinal,
   status |-> status,
   predecessorPrefix |-> predecessorPrefix,
   consumed |-> consumed]

RecordA(status, predecessorPrefix, consumed) ==
  ServiceRecord(
    0, 1, IdentityA, TokenA, 1, 1,
    status, predecessorPrefix, consumed)

RecordAReallocated ==
  ServiceRecord(
    0, 1, IdentityA, TokenA, 2, 2,
    "Ingress", 1, FALSE)

RecordB ==
  ServiceRecord(
    0, 1, IdentityB, TokenB, 2, 2,
    "Ingress", 1, FALSE)

RecordC ==
  ServiceRecord(
    0, 2, IdentityC, TokenC, 3, 3,
    "Ingress", 1, FALSE)

RecordCarrier ==
  {RecordA(status, predecessorPrefix, consumed):
     status \in Statuses,
     predecessorPrefix \in 0..1,
     consumed \in BOOLEAN}
    \cup {RecordAReallocated, RecordB, RecordC}

VARIABLES
  phase,
  height,
  slotRecords,
  candidateOwned,
  selectorBarrier,
  packetPresent,
  capacityAvailable,
  ownerCount,
  laterTargetOwned,
  laterTargetPrecharged

vars ==
  <<phase, height, slotRecords, candidateOwned, selectorBarrier,
    packetPresent, capacityAvailable, ownerCount,
    laterTargetOwned, laterTargetPrecharged>>

TypeInvariant ==
  /\ phase
       \in {"Fresh", "Active", "ExactRetried",
             "UnconsumedRestarted", "LaterTargetAdmitted",
             "UnconsumedRetransmitted",
             "CapacityChecked", "CapacityReady",
             "UnconsumedReactivated", "SameViewChecked",
             "ForeignTerminalChecked", "Serviced", "ConsumedRetried",
             "Restarted", "HigherViewAdmitted", "RolledOver"}
  /\ height \in 0..1
  /\ slotRecords \subseteq RecordCarrier
  /\ candidateOwned \in BOOLEAN
  /\ selectorBarrier \in BOOLEAN
  /\ packetPresent \in BOOLEAN
  /\ capacityAvailable \in BOOLEAN
  /\ ownerCount \in 0..2
  /\ laterTargetOwned \in BOOLEAN
  /\ laterTargetPrecharged \in BOOLEAN

SlotTableCardinalityIsRosterClassBounded ==
  Cardinality(slotRecords) <= Cardinality(Slots)

ActiveFirstOwnerPassesSlotGuard ==
  phase = "Active"
    => /\ candidateOwned
       /\ selectorBarrier
       /\ ~packetPresent
       /\ slotRecords = {RecordA("Ingress", 1, FALSE)}
       /\ ownerCount = 1

StableCompletionRecorded ==
  \E record \in slotRecords:
    \/ record.consumed
    \/ AcceptOccupiedUnconsumedAsCompletion

OccupiedUnconsumedOwnerIsNotCompletion ==
  phase = "Active" => ~StableCompletionRecorded

ExactRetryCoalescesIntoOneOwner ==
  phase = "ExactRetried"
    => /\ slotRecords = {RecordA("Ingress", 1, FALSE)}
       /\ ownerCount = 1

RestartedUnconsumedIsDormantWithoutAuthority ==
  phase = "UnconsumedRestarted"
    => /\ height = 0
       /\ slotRecords = {RecordA("Dormant", 0, FALSE)}
       /\ ~candidateOwned
       /\ ~selectorBarrier
       /\ ownerCount = 1

RestartDoesNotSynthesizePacket ==
  phase = "UnconsumedRestarted" => ~packetPresent

RealRetransmissionRetainsDormantWithoutAuthority ==
  phase = "UnconsumedRetransmitted"
    => /\ packetPresent
       /\ slotRecords = {RecordA("Dormant", 0, FALSE)}
       /\ ~candidateOwned
       /\ ~selectorBarrier
       /\ ownerCount = 1

CapacityBlockedRetryRemainsDormant ==
  phase = "CapacityChecked"
    => /\ ~capacityAvailable
       /\ packetPresent
       /\ slotRecords = {RecordA("Dormant", 0, FALSE)}
       /\ ~candidateOwned
       /\ ~selectorBarrier

CapacityAcceptanceReactivatesSameOrdinal ==
  phase = "UnconsumedReactivated"
    => /\ capacityAvailable
       /\ ~packetPresent
       /\ \E record \in slotRecords:
            /\ record.identity = IdentityA
            /\ record.admissionOrdinal = 1
            /\ record.schedulerOrdinal = 1
            /\ record.status = "Ingress"
       /\ candidateOwned
       /\ selectorBarrier
       /\ ownerCount = 1

CapacityAcceptanceRetainsRestartClearedPrefix ==
  phase = "UnconsumedReactivated"
    => \E record \in slotRecords:
         /\ record.identity = IdentityA
         /\ record.predecessorPrefix = 0
         /\ record.status = "Ingress"

LaterTargetPrechargesDormantPotentialOwner ==
  phase
    \in {"LaterTargetAdmitted", "UnconsumedRetransmitted",
         "CapacityChecked", "CapacityReady", "UnconsumedReactivated"}
    => /\ laterTargetOwned
       /\ laterTargetPrecharged

DormantReactivationConsumesPrechargedPotential ==
  phase = "UnconsumedReactivated"
    => /\ laterTargetOwned
       /\ laterTargetPrecharged
       /\ slotRecords = {RecordA("Ingress", 0, FALSE)}

SameViewIdentityCannotReplaceFirstOwner ==
  phase = "SameViewChecked"
    => slotRecords = {RecordA("Ingress", 0, FALSE)}

ForeignTerminalCannotRetireExactOwner ==
  phase = "ForeignTerminalChecked"
    => /\ slotRecords = {RecordA("Ingress", 0, FALSE)}
       /\ candidateOwned
       /\ selectorBarrier
       /\ ownerCount = 1

ConsumedSameOrLowerRetryDropsWithoutCandidate ==
  phase = "ConsumedRetried"
    => /\ slotRecords = {RecordA("Terminal", 0, TRUE)}
       /\ ~candidateOwned
       /\ ~selectorBarrier

SameHeightRestartPreservesStableTerminal ==
  phase = "Restarted"
    => /\ height = 0
       /\ slotRecords = {RecordA("Terminal", 0, TRUE)}
       /\ ~candidateOwned
       /\ ~selectorBarrier
       /\ ~packetPresent

StrictlyNewerViewReplacesEqualCountOwner ==
  phase = "HigherViewAdmitted"
    => /\ slotRecords = {RecordC}
       /\ candidateOwned
       /\ selectorBarrier
       /\ ownerCount = 1

SuccessorHeightRolloverResetsSlot ==
  phase = "RolledOver"
    => /\ height = 1
       /\ slotRecords = {}
       /\ ~candidateOwned
       /\ ~selectorBarrier
       /\ ~packetPresent
       /\ ownerCount = 0

Init ==
  /\ phase = "Fresh"
  /\ height = 0
  /\ slotRecords = {}
  /\ ~candidateOwned
  /\ ~selectorBarrier
  /\ ~packetPresent
  /\ ~capacityAvailable
  /\ ownerCount = 0
  /\ ~laterTargetOwned
  /\ ~laterTargetPrecharged

AdmitFirstOwner ==
  /\ phase = "Fresh"
  /\ phase' = "Active"
  /\ slotRecords' = {RecordA("Ingress", 1, FALSE)}
  /\ candidateOwned'
  /\ selectorBarrier'
  /\ ~packetPresent'
  /\ capacityAvailable'
  /\ ownerCount' = 1
  /\ UNCHANGED <<height, laterTargetOwned, laterTargetPrecharged>>

RetransmitExactOwner ==
  /\ phase = "Active"
  /\ phase' = "ExactRetried"
  /\ ownerCount' = IF CoalesceExactRetry THEN 1 ELSE 2
  /\ UNCHANGED
       <<height, slotRecords, candidateOwned, selectorBarrier,
         packetPresent, capacityAvailable,
         laterTargetOwned, laterTargetPrecharged>>

RestartUnconsumedSameHeight ==
  /\ phase = "ExactRetried"
  /\ phase' = "UnconsumedRestarted"
  /\ slotRecords' =
       IF RestartUnconsumedAsDormant
       THEN {RecordA("Dormant", 0, FALSE)}
       ELSE slotRecords
  /\ candidateOwned' = DormantOwnsImmediatelyAfterRestart
  /\ selectorBarrier' = DormantOwnsImmediatelyAfterRestart
  /\ packetPresent' = SynthesizePacketOnRestart
  /\ ~capacityAvailable'
  /\ ownerCount' = 1
  /\ UNCHANGED <<height, laterTargetOwned, laterTargetPrecharged>>

AdmitLaterTargetWhileDormant ==
  /\ phase = "UnconsumedRestarted"
  /\ phase' = "LaterTargetAdmitted"
  /\ laterTargetOwned'
  /\ laterTargetPrecharged' =
       PrechargeDormantPotentialBeforeLaterOwner
  /\ UNCHANGED
       <<height, slotRecords, candidateOwned, selectorBarrier,
         packetPresent, capacityAvailable, ownerCount>>

RetransmitAfterUnconsumedRestart ==
  /\ phase = "LaterTargetAdmitted"
  /\ phase' = "UnconsumedRetransmitted"
  /\ packetPresent'
  /\ UNCHANGED
       <<height, slotRecords, candidateOwned, selectorBarrier,
         capacityAvailable, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

AttemptCapacityBlockedReactivation ==
  /\ phase = "UnconsumedRetransmitted"
  /\ phase' = "CapacityChecked"
  /\ ~capacityAvailable
  /\ IF RequireCapacityForReactivation
     THEN UNCHANGED <<slotRecords, candidateOwned, selectorBarrier>>
     ELSE /\ slotRecords' = {RecordA("Ingress", 1, FALSE)}
          /\ candidateOwned'
          /\ selectorBarrier'
  /\ UNCHANGED
       <<height, packetPresent, capacityAvailable, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

OpenLocalCapacity ==
  /\ phase = "CapacityChecked"
  /\ phase' = "CapacityReady"
  /\ capacityAvailable'
  /\ UNCHANGED
       <<height, slotRecords, candidateOwned, selectorBarrier,
         packetPresent, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

ReactivateAfterCapacityAcceptance ==
  /\ phase = "CapacityReady"
  /\ phase' = "UnconsumedReactivated"
  /\ capacityAvailable
  /\ packetPresent
  /\ slotRecords' =
       IF ReuseDormantOrdinalsOnReactivation
       THEN IF PreserveDormantRestartPrefix
            THEN {RecordA("Ingress", 0, FALSE)}
            ELSE {RecordA("Ingress", 1, FALSE)}
       ELSE {RecordAReallocated}
  /\ candidateOwned'
  /\ selectorBarrier'
  /\ ~packetPresent'
  /\ UNCHANGED
       <<height, capacityAvailable, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

AttemptSameViewReplacement ==
  /\ phase = "UnconsumedReactivated"
  /\ phase' = "SameViewChecked"
  /\ slotRecords' =
       IF RequireStrictlyNewerReplacement
       THEN slotRecords
       ELSE {RecordB}
  /\ UNCHANGED
       <<height, candidateOwned, selectorBarrier,
         packetPresent, capacityAvailable, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

AttemptForeignTerminalEvidence ==
  /\ phase = "SameViewChecked"
  /\ phase' = "ForeignTerminalChecked"
  /\ ForeignTerminalIdentities # {}
  /\ slotRecords' =
       IF RequireExactTerminalIdentity
       THEN slotRecords
       ELSE {RecordA("Terminal", 0, TRUE)}
  /\ candidateOwned' = RequireExactTerminalIdentity
  /\ selectorBarrier' = RequireExactTerminalIdentity
  /\ ownerCount' = 1
  /\ UNCHANGED
       <<height, packetPresent, capacityAvailable,
         laterTargetOwned, laterTargetPrecharged>>

ServiceFirstOwner ==
  /\ phase = "ForeignTerminalChecked"
  /\ phase' = "Serviced"
  /\ slotRecords' = {RecordA("Terminal", 0, TRUE)}
  /\ ~candidateOwned'
  /\ ~selectorBarrier'
  /\ ~packetPresent'
  /\ ownerCount' = 1
  /\ UNCHANGED
       <<height, capacityAvailable,
         laterTargetOwned, laterTargetPrecharged>>

RetransmitConsumedOwner ==
  /\ phase = "Serviced"
  /\ phase' = "ConsumedRetried"
  /\ candidateOwned' = IF DropConsumedRetry THEN FALSE ELSE TRUE
  /\ selectorBarrier' = IF DropConsumedRetry THEN FALSE ELSE TRUE
  /\ ~packetPresent'
  /\ UNCHANGED
       <<height, slotRecords, capacityAvailable, ownerCount,
         laterTargetOwned, laterTargetPrecharged>>

RestartSameHeight ==
  /\ phase = "ConsumedRetried"
  /\ phase' = "Restarted"
  /\ slotRecords' =
       IF PreserveStableTerminalAcrossRestart
       THEN slotRecords
       ELSE {RecordA("Dormant", 0, TRUE)}
  /\ ~candidateOwned'
  /\ ~selectorBarrier'
  /\ ~packetPresent'
  /\ ~capacityAvailable'
  /\ ownerCount' = 1
  /\ UNCHANGED <<height, laterTargetOwned, laterTargetPrecharged>>

AdmitStrictlyNewerView ==
  /\ phase = "Restarted"
  /\ phase' = "HigherViewAdmitted"
  /\ slotRecords' =
       IF KeepSlotTableBounded
       THEN {RecordC}
       ELSE slotRecords \cup {RecordC}
  /\ candidateOwned'
  /\ selectorBarrier'
  /\ packetPresent'
  /\ capacityAvailable'
  /\ ownerCount' = IF KeepSlotTableBounded THEN 1 ELSE 2
  /\ UNCHANGED <<height, laterTargetOwned, laterTargetPrecharged>>

RolloverSuccessorHeight ==
  /\ phase = "HigherViewAdmitted"
  /\ phase' = "RolledOver"
  /\ height' = 1
  /\ slotRecords' = IF ResetAtRollover THEN {} ELSE slotRecords
  /\ ~candidateOwned'
  /\ ~selectorBarrier'
  /\ ~packetPresent'
  /\ ~capacityAvailable'
  /\ ownerCount' = IF ResetAtRollover THEN 0 ELSE ownerCount
  /\ laterTargetOwned' = IF ResetAtRollover THEN FALSE ELSE laterTargetOwned
  /\ laterTargetPrecharged' =
       IF ResetAtRollover THEN FALSE ELSE laterTargetPrecharged

Next ==
  \/ AdmitFirstOwner
  \/ RetransmitExactOwner
  \/ RestartUnconsumedSameHeight
  \/ AdmitLaterTargetWhileDormant
  \/ RetransmitAfterUnconsumedRestart
  \/ AttemptCapacityBlockedReactivation
  \/ OpenLocalCapacity
  \/ ReactivateAfterCapacityAcceptance
  \/ AttemptSameViewReplacement
  \/ AttemptForeignTerminalEvidence
  \/ ServiceFirstOwner
  \/ RetransmitConsumedOwner
  \/ RestartSameHeight
  \/ AdmitStrictlyNewerView
  \/ RolloverSuccessorHeight

=============================================================================
