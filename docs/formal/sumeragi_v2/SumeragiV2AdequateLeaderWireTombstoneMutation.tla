---- MODULE SumeragiV2AdequateLeaderWireTombstoneMutation ----
EXTENDS TLC, Naturals, FiniteSets

(***************************************************************************
Finite mutation model for the bounded adequate-leader control-service table.

One configured protocol-owner slot admits identity A at view 1 with immutable
ordinal 1.  An exact retry must coalesce, a different same-view identity must
not replace the first owner, and successful service flips `consumed`; mere
unconsumed occupancy is not completion.  A later same/lower retry must drain
without a candidate.  Same-height restart retains a terminal slot and retires
an admitted-but-unconsumed slot whose volatile ingress occurrence was cleared,
so a retry cannot coalesce behind a dead owner.  A strictly newer view replaces
exactly one slot, and a successor-height rollover resets the table.  Every
mutation below weakens one of those production contracts while keeping the
state space finite.
***************************************************************************)

CONSTANTS
  KeepSlotTableBounded,
  RequireStrictlyNewerReplacement,
  CoalesceExactRetry,
  DropConsumedRetry,
  AcceptOccupiedUnconsumedAsCompletion,
  PreserveAcrossSameHeightRestart,
  RetireUnconsumedAcrossSameHeightRestart,
  ResetAtRollover

ASSUME
  /\ KeepSlotTableBounded \in BOOLEAN
  /\ RequireStrictlyNewerReplacement \in BOOLEAN
  /\ CoalesceExactRetry \in BOOLEAN
  /\ DropConsumedRetry \in BOOLEAN
  /\ AcceptOccupiedUnconsumedAsCompletion \in BOOLEAN
  /\ PreserveAcrossSameHeightRestart \in BOOLEAN
  /\ RetireUnconsumedAcrossSameHeightRestart \in BOOLEAN
  /\ ResetAtRollover \in BOOLEAN

Slot == "recipient/source/Proposal"
Slots == {Slot}
IdentityA == "context-0/height-0/view-1/subject-A/Proposal"
IdentityB == "context-0/height-0/view-1/subject-B/Proposal"
IdentityC == "context-0/height-0/view-2/subject-C/Proposal"

ServiceRecord(blockHeight, roundView, identity, ordinal, consumed) ==
  [slot |-> Slot,
   height |-> blockHeight,
   view |-> roundView,
   identity |-> identity,
   ordinal |-> ordinal,
   consumed |-> consumed]

RecordA(consumed) == ServiceRecord(0, 1, IdentityA, 1, consumed)
RecordB(consumed) == ServiceRecord(0, 1, IdentityB, 2, consumed)
RecordC(consumed) == ServiceRecord(0, 2, IdentityC, 3, consumed)

RecordCarrier ==
  {RecordA(FALSE), RecordA(TRUE),
   RecordB(FALSE), RecordB(TRUE),
   RecordC(FALSE), RecordC(TRUE)}

VARIABLES phase, height, slotRecords, candidateOwned, ownerCount

vars == <<phase, height, slotRecords, candidateOwned, ownerCount>>

TypeInvariant ==
  /\ phase \in {"Fresh", "Active", "ExactRetried", "SameViewChecked",
                 "Serviced", "ConsumedRetried", "Restarted",
                 "UnconsumedRestarted", "UnconsumedRetried",
                 "HigherViewAdmitted", "RolledOver"}
  /\ height \in 0..1
  /\ slotRecords \subseteq RecordCarrier
  /\ candidateOwned \in BOOLEAN
  /\ ownerCount \in 0..2

SlotTableCardinalityIsRosterClassBounded ==
  Cardinality(slotRecords) <= Cardinality(Slots)

ActiveFirstOwnerPassesSlotGuard ==
  phase = "Active"
    => /\ candidateOwned
       /\ slotRecords = {RecordA(FALSE)}
       /\ ownerCount = 1

StableCompletionRecorded ==
  \E record \in slotRecords:
    \/ record.consumed
    \/ AcceptOccupiedUnconsumedAsCompletion

OccupiedUnconsumedOwnerIsNotCompletion ==
  phase = "Active" => ~StableCompletionRecorded

ExactRetryCoalescesIntoOneOwner ==
  phase = "ExactRetried"
    => /\ slotRecords = {RecordA(FALSE)}
       /\ ownerCount = 1

SameViewIdentityCannotReplaceFirstOwner ==
  phase = "SameViewChecked"
    => slotRecords = {RecordA(FALSE)}

ConsumedSameOrLowerRetryDropsWithoutCandidate ==
  phase = "ConsumedRetried"
    => /\ slotRecords = {RecordA(TRUE)}
       /\ ~candidateOwned

SameHeightRestartPreservesConsumedSlot ==
  phase = "Restarted"
    => /\ height = 0
       /\ slotRecords = {RecordA(TRUE)}
       /\ ~candidateOwned

SameHeightRestartRetiresUnconsumedSlot ==
  phase = "UnconsumedRestarted"
    => /\ height = 0
       /\ slotRecords = {RecordA(TRUE)}
       /\ ~candidateOwned

RetryAfterUnconsumedRestartDropsWithoutCandidate ==
  phase = "UnconsumedRetried"
    => /\ slotRecords = {RecordA(TRUE)}
       /\ ~candidateOwned

StrictlyNewerViewReplacesEqualCountOwner ==
  phase = "HigherViewAdmitted"
    => /\ slotRecords = {RecordC(FALSE)}
       /\ candidateOwned
       /\ ownerCount = 1

SuccessorHeightRolloverResetsSlot ==
  phase = "RolledOver"
    => /\ height = 1
       /\ slotRecords = {}
       /\ ~candidateOwned
       /\ ownerCount = 0

Init ==
  /\ phase = "Fresh"
  /\ height = 0
  /\ slotRecords = {}
  /\ ~candidateOwned
  /\ ownerCount = 0

AdmitFirstOwner ==
  /\ phase = "Fresh"
  /\ phase' = "Active"
  /\ slotRecords' = {RecordA(FALSE)}
  /\ candidateOwned'
  /\ ownerCount' = 1
  /\ UNCHANGED height

RetransmitExactOwner ==
  /\ phase = "Active"
  /\ phase' = "ExactRetried"
  /\ ownerCount' = IF CoalesceExactRetry THEN 1 ELSE 2
  /\ UNCHANGED <<height, slotRecords, candidateOwned>>

RestartUnconsumedSameHeight ==
  /\ phase = "Active"
  /\ phase' = "UnconsumedRestarted"
  /\ slotRecords' =
       IF RetireUnconsumedAcrossSameHeightRestart
       THEN {RecordA(TRUE)}
       ELSE slotRecords
  /\ ~candidateOwned'
  /\ ownerCount' = 1
  /\ UNCHANGED height

RetransmitAfterUnconsumedRestart ==
  /\ phase = "UnconsumedRestarted"
  /\ phase' = "UnconsumedRetried"
  /\ candidateOwned' =
       IF slotRecords = {RecordA(TRUE)} THEN FALSE ELSE TRUE
  /\ UNCHANGED <<height, slotRecords, ownerCount>>

AttemptSameViewReplacement ==
  /\ phase = "ExactRetried"
  /\ phase' = "SameViewChecked"
  /\ slotRecords' =
       IF RequireStrictlyNewerReplacement
       THEN slotRecords
       ELSE {RecordB(FALSE)}
  /\ UNCHANGED <<height, candidateOwned, ownerCount>>

ServiceFirstOwner ==
  /\ phase = "SameViewChecked"
  /\ phase' = "Serviced"
  /\ slotRecords' =
       {[record EXCEPT !.consumed = TRUE]: record \in slotRecords}
  /\ ~candidateOwned'
  /\ ownerCount' = 1
  /\ UNCHANGED height

RetransmitConsumedOwner ==
  /\ phase = "Serviced"
  /\ phase' = "ConsumedRetried"
  /\ candidateOwned' = IF DropConsumedRetry THEN FALSE ELSE TRUE
  /\ UNCHANGED <<height, slotRecords, ownerCount>>

RestartSameHeight ==
  /\ phase = "ConsumedRetried"
  /\ phase' = "Restarted"
  /\ slotRecords' =
       IF PreserveAcrossSameHeightRestart THEN slotRecords ELSE {}
  /\ ~candidateOwned'
  /\ ownerCount' = IF PreserveAcrossSameHeightRestart THEN 1 ELSE 0
  /\ UNCHANGED height

AdmitStrictlyNewerView ==
  /\ phase = "Restarted"
  /\ phase' = "HigherViewAdmitted"
  /\ slotRecords' =
       IF KeepSlotTableBounded
       THEN {RecordC(FALSE)}
       ELSE slotRecords \cup {RecordC(FALSE)}
  /\ candidateOwned'
  /\ ownerCount' = IF KeepSlotTableBounded THEN 1 ELSE 2
  /\ UNCHANGED height

RolloverSuccessorHeight ==
  /\ phase = "HigherViewAdmitted"
  /\ phase' = "RolledOver"
  /\ height' = 1
  /\ slotRecords' = IF ResetAtRollover THEN {} ELSE slotRecords
  /\ ~candidateOwned'
  /\ ownerCount' = IF ResetAtRollover THEN 0 ELSE ownerCount

Next ==
  \/ AdmitFirstOwner
  \/ RetransmitExactOwner
  \/ RestartUnconsumedSameHeight
  \/ RetransmitAfterUnconsumedRestart
  \/ AttemptSameViewReplacement
  \/ ServiceFirstOwner
  \/ RetransmitConsumedOwner
  \/ RestartSameHeight
  \/ AdmitStrictlyNewerView
  \/ RolloverSuccessorHeight

=============================================================================
