---- MODULE SumeragiV2InFlightFirstRelease ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Finite safety kernel for the first-release, in-flight lane carrier path.

The accepted schema V2 carried by the Rust `LaneExecutablePayloadV1`
container is represented by `payloadBinding`: each validator holds the
*preimage* of the selected `QueuePlanAdmissionBindingV2`, not only a digest.
QueuePlan journal V4 and reservation journal V5 are separate durable facts.
The only forward order is

 QueuePlan journal V4 PutBatch -> reservation journal V5 fsync -> Kura Active ->
 execution-input durability -> READY.

The model has a producer and two independently crashing replicated carriers.
It deliberately contains no Rust projection and no refinement theorem: TLC
and Apalache can establish only the bounded abstract transition system below.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Str;
  Producer,
  \* @type: Str;
  ReplicaOne,
  \* @type: Str;
  ReplicaTwo,
  \* @type: Str;
  BindingA,
  \* @type: Str;
  BindingB

Modes == {"Fixed", "ReservationBeforePutBatch", "KuraBeforeReservation",
          "ReadyBeforeInput", "CrashDropsDurable", "PayloadBindingConflict",
          "CommitScopeConflict", "ReleaseScopeConflict", "DuplicateApply",
          "OversizePutBatch"}

Validators == {Producer, ReplicaOne, ReplicaTwo}
Ownership == {"ProducerSelected", "ReplicatedCarrier"}
OptionalBinding == {"None", BindingA, BindingB}
OptionalValidator == Validators \union {"None"}

Configuration ==
  /\ Mode \in Modes
  /\ Cardinality(Validators) = 3
  /\ Producer # ReplicaOne
  /\ Producer # ReplicaTwo
  /\ ReplicaOne # ReplicaTwo
  /\ BindingA # BindingB

VARIABLES
  \* @type: Str -> Str;
  ownership,
  \* @type: Str -> Str;
  payloadBinding,
  \* @type: Bool;
  queuePlanV4,
  \* @type: Bool;
  reservationV5,
  \* @type: Set(Str);
  kuraActive,
  \* @type: Set(Str);
  inputDurable,
  \* @type: Set(Str);
  ready,
  \* @type: Set(Str);
  bodies,
  \* @type: Set(Str);
  crashed,
  \* @type: Bool;
  producerAlive,
  \* @type: Bool;
  everQueuePlanV4,
  \* @type: Bool;
  everReservationV5,
  \* @type: Set(Str);
  everInputDurable,
  \* @type: Str;
  commitScope,
  \* @type: Str;
  releaseScope,
  \* @type: Str;
  commitOwner,
  \* @type: Str;
  releaseOwner,
  \* @type: Int;
  applicationCount,
  \* @type: Int;
  selectedCount,
  \* @type: Str;
  appliedBy

vars ==
  <<ownership, payloadBinding, queuePlanV4, reservationV5, kuraActive,
    inputDurable, ready, bodies, crashed, producerAlive, everQueuePlanV4,
    everReservationV5, everInputDurable, commitScope, releaseScope,
    commitOwner, releaseOwner, applicationCount, appliedBy, selectedCount>>

Init ==
  /\ Configuration
  /\ ownership = [p \in Validators |->
       IF p = Producer THEN "ProducerSelected" ELSE "ReplicatedCarrier"]
  /\ payloadBinding = [p \in Validators |-> BindingA]
  /\ queuePlanV4 = FALSE
  /\ reservationV5 = FALSE
  /\ kuraActive = {}
  /\ inputDurable = {}
  /\ ready = {}
  /\ bodies = {Producer}
  /\ crashed = {}
  /\ producerAlive = TRUE
  /\ everQueuePlanV4 = FALSE
  /\ everReservationV5 = FALSE
  /\ everInputDurable = {}
  /\ commitScope = "None"
  /\ releaseScope = "None"
  /\ commitOwner = "None"
  /\ releaseOwner = "None"
  /\ applicationCount = 0
  /\ appliedBy = "None"
  /\ selectedCount = 1

PutBatchV4 ==
  /\ ~queuePlanV4
  /\ queuePlanV4' = TRUE
  /\ everQueuePlanV4' = TRUE
  /\ selectedCount' = IF Mode = "OversizePutBatch" THEN 4097 ELSE selectedCount
  /\ UNCHANGED <<ownership, payloadBinding, reservationV5, kuraActive,
                 inputDurable, ready, bodies, crashed, producerAlive,
                 everReservationV5, everInputDurable, commitScope,
                 releaseScope, commitOwner, releaseOwner, applicationCount,
                 appliedBy>>

FsyncReservationV5 ==
  /\ ~reservationV5
  /\ (queuePlanV4 \/ Mode = "ReservationBeforePutBatch")
  /\ reservationV5' = TRUE
  /\ everReservationV5' = TRUE
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, kuraActive,
                 inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everInputDurable, commitScope,
                 releaseScope, commitOwner, releaseOwner, applicationCount,
                 appliedBy, selectedCount>>

ActivateKura(p) ==
  /\ p \in Validators
  /\ p \notin crashed
  /\ (reservationV5 \/ Mode = "KuraBeforeReservation")
  /\ kuraActive' = kuraActive \union {p}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

FanoutFromProducer(p) ==
  /\ p \in Validators \ {Producer}
  /\ producerAlive
  /\ reservationV5
  /\ p \notin crashed
  /\ bodies' = bodies \union {p}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

ServeLateBody(source, target) ==
  /\ source \in Validators
  /\ target \in Validators
  /\ source # target
  /\ source \in bodies
  /\ target \notin crashed
  /\ bodies' = bodies \union {target}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

PersistExecutionInput(p) ==
  /\ p \in kuraActive
  /\ p \in bodies
  /\ p \notin crashed
  /\ inputDurable' = inputDurable \union {p}
  /\ everInputDurable' = everInputDurable \union {p}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, commitScope,
                 releaseScope, commitOwner, releaseOwner, applicationCount,
                 appliedBy, selectedCount>>

MarkReady(p) ==
  /\ p \in Validators
  /\ p \notin crashed
  /\ (p \in inputDurable \/ Mode = "ReadyBeforeInput")
  /\ ready' = ready \union {p}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

Crash(p) ==
  /\ p \in Validators
  /\ p \notin crashed
  /\ crashed' = crashed \union {p}
  /\ ready' = ready \ {p}
  /\ producerAlive' = IF p = Producer THEN FALSE ELSE producerAlive
  /\ IF Mode = "CrashDropsDurable" /\ p = Producer /\ queuePlanV4
     THEN /\ queuePlanV4' = FALSE
          /\ reservationV5' = FALSE
     ELSE /\ UNCHANGED <<queuePlanV4, reservationV5>>
  /\ UNCHANGED <<ownership, payloadBinding, kuraActive, inputDurable, bodies,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

Recover(p) ==
  /\ p \in crashed
  /\ crashed' = crashed \ {p}
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, bodies, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

Commit(p) ==
  /\ p \in ready
  /\ commitOwner = "None"
  /\ releaseOwner = "None"
  /\ payloadBinding[p] = BindingA
  /\ commitOwner' = p
  /\ commitScope' = IF Mode = "CommitScopeConflict" THEN BindingB ELSE BindingA
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 releaseScope, releaseOwner, applicationCount, appliedBy,
                 selectedCount>>

ApplyCarrier(p) ==
  /\ p = commitOwner
  /\ p \in ready
  /\ (applicationCount = 0 \/ Mode = "DuplicateApply")
  /\ applicationCount' = applicationCount + 1
  /\ appliedBy' = p
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 selectedCount>>

Release(p) ==
  /\ p \in Validators
  /\ p \in kuraActive
  /\ commitOwner = "None"
  /\ releaseOwner = "None"
  /\ payloadBinding[p] = BindingA
  /\ releaseOwner' = p
  /\ releaseScope' = IF Mode = "ReleaseScopeConflict" THEN BindingB ELSE BindingA
  /\ UNCHANGED <<ownership, payloadBinding, queuePlanV4, reservationV5,
                 kuraActive, inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, commitOwner, applicationCount, appliedBy,
                 selectedCount>>

ConflictingPayloadBindingMutation ==
  /\ Mode = "PayloadBindingConflict"
  /\ payloadBinding[ReplicaTwo] = BindingA
  /\ payloadBinding' = [payloadBinding EXCEPT ![ReplicaTwo] = BindingB]
  /\ UNCHANGED <<ownership, queuePlanV4, reservationV5, kuraActive,
                 inputDurable, ready, bodies, crashed, producerAlive,
                 everQueuePlanV4, everReservationV5, everInputDurable,
                 commitScope, releaseScope, commitOwner, releaseOwner,
                 applicationCount, appliedBy, selectedCount>>

Next ==
  \/ PutBatchV4
  \/ FsyncReservationV5
  \/ \E p \in Validators: ActivateKura(p)
  \/ \E p \in Validators \ {Producer}: FanoutFromProducer(p)
  \/ \E source \in Validators, target \in Validators: ServeLateBody(source, target)
  \/ \E p \in Validators: PersistExecutionInput(p)
  \/ \E p \in Validators: MarkReady(p)
  \/ \E p \in Validators: Crash(p)
  \/ \E p \in Validators: Recover(p)
  \/ \E p \in Validators: Commit(p)
  \/ \E p \in Validators: ApplyCarrier(p)
  \/ \E p \in Validators: Release(p)
  \/ ConflictingPayloadBindingMutation

FirstReleaseTypeInvariant ==
  /\ Configuration
  /\ ownership \in [Validators -> Ownership]
  /\ payloadBinding \in [Validators -> {BindingA, BindingB}]
  /\ kuraActive \subseteq Validators
  /\ inputDurable \subseteq Validators
  /\ ready \subseteq Validators
  /\ bodies \subseteq Validators
  /\ crashed \subseteq Validators
  /\ everInputDurable \subseteq Validators
  /\ commitScope \in OptionalBinding
  /\ releaseScope \in OptionalBinding
  /\ commitOwner \in OptionalValidator
  /\ releaseOwner \in OptionalValidator
  /\ applicationCount \in Nat
  /\ appliedBy \in OptionalValidator
  /\ selectedCount \in Nat

MLPayloadSchemaV2CarriesExactAdmissionPreimage ==
  \A p \in Validators: payloadBinding[p] = BindingA

MLValidatorCarrierOwnership ==
  /\ ownership[Producer] = "ProducerSelected"
  /\ \A p \in Validators \ {Producer}: ownership[p] = "ReplicatedCarrier"

MLPutBatchV4BeforeReservationV5 == reservationV5 => queuePlanV4
MLReservationV5BeforeKuraActive == kuraActive # {} => reservationV5
MLKuraActiveBeforeExecutionInput == inputDurable \subseteq kuraActive
MLExecutionInputBeforeReady == ready \subseteq inputDurable

MLCrashPrefixLossFree ==
  /\ everQueuePlanV4 => queuePlanV4
  /\ everReservationV5 => reservationV5
  /\ everInputDurable \subseteq inputDurable

MLCommitAndReleaseRetainExactScope ==
  /\ commitScope # "None" => commitScope = BindingA
  /\ releaseScope # "None" => releaseScope = BindingA
  /\ commitOwner # "None" => releaseOwner = "None"

MLExactlyOnceCarrierApplication ==
  /\ applicationCount <= 1
  /\ applicationCount > 0 =>
       /\ appliedBy = commitOwner
       /\ commitScope = BindingA
       /\ appliedBy \in inputDurable

MLQueuePlanV4PutBatchBound4096 == selectedCount <= 4096

InFlightFirstReleaseSafetyInvariant ==
  /\ FirstReleaseTypeInvariant
  /\ MLPayloadSchemaV2CarriesExactAdmissionPreimage
  /\ MLValidatorCarrierOwnership
  /\ MLPutBatchV4BeforeReservationV5
  /\ MLReservationV5BeforeKuraActive
  /\ MLKuraActiveBeforeExecutionInput
  /\ MLExecutionInputBeforeReady
  /\ MLCrashPrefixLossFree
  /\ MLCommitAndReleaseRetainExactScope
  /\ MLExactlyOnceCarrierApplication
  /\ MLQueuePlanV4PutBatchBound4096

InFlightFirstReleaseSpec == Init /\ [][Next]_vars

====
