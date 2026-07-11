---- MODULE SumeragiMergeExecutionOrder ----
EXTENDS FiniteSets, Integers

(***************************************************************************
A bounded model of the safety barrier between independently arriving lane QCs
and shared-state execution.

Two replicas may receive the same certified lane payloads in opposite orders.
Neither arrival order nor restart timing is a commit order.  A merge QC fixes
the total order <<lane 1, lane 2>>; each replica journals and applies only the
next item in that order.  The deliberately non-commutative state transition
`state' = 10 * state + lane` makes an arrival-order fork immediately visible.

`Bug` selects expected-failure mutations.  `"none"` is the production model.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

Nodes == {0, 1}
Lanes == {1, 2}
MergeLength == 2

VARIABLES
  \* Locally available, hash-verified payloads.  Delivery order is arbitrary.
  \* @type: Int -> Set(Int);
  received,
  \* Whether the node has authenticated the merge QC fixing the total order.
  \* @type: Int -> Bool;
  certified,
  \* Number of certified merge items durably applied by each node.
  \* @type: Int -> Int;
  cursor,
  \* Observable shared-state sentinel.  Lane effects intentionally do not commute.
  \* @type: Int -> Int;
  state,
  \* Zero means clean; otherwise the exact next lane is write-ahead journaled.
  \* @type: Int -> Int;
  journal,
  \* Sticky evidence that a node ever applied without a merge certificate.
  \* @type: Int -> Bool;
  uncertifiedApply

vars == <<received, certified, cursor, state, journal, uncertifiedApply>>

NextLane(position) == IF position = 0 THEN 1 ELSE 2

LastLane(position) == IF position = 1 THEN 1 ELSE 2

ExpectedState(position) ==
  CASE position = 0 -> 0
    [] position = 1 -> 1
    [] position = 2 -> 12
    [] OTHER -> -1

Init ==
  /\ received = [n \in Nodes |-> {}]
  /\ certified = [n \in Nodes |-> FALSE]
  /\ cursor = [n \in Nodes |-> 0]
  /\ state = [n \in Nodes |-> 0]
  /\ journal = [n \in Nodes |-> 0]
  /\ uncertifiedApply = [n \in Nodes |-> FALSE]

ReceivePayload(n, lane) ==
  /\ lane \notin received[n]
  /\ received' = [received EXCEPT ![n] = @ \cup {lane}]
  /\ UNCHANGED <<certified, cursor, state, journal, uncertifiedApply>>

InstallMergeCertificate(n) ==
  /\ ~certified[n]
  /\ certified' = [certified EXCEPT ![n] = TRUE]
  /\ UNCHANGED <<received, cursor, state, journal, uncertifiedApply>>

PrepareNext(n, lane) ==
  /\ cursor[n] < MergeLength
  /\ journal[n] = 0
  /\ lane \in received[n]
  /\ (certified[n] \/ Bug = "apply_before_certificate")
  /\ (lane = NextLane(cursor[n]) \/ Bug = "arrival_order")
  /\ journal' = [journal EXCEPT ![n] = lane]
  /\ UNCHANGED <<received, certified, cursor, state, uncertifiedApply>>

CommitPrepared(n) ==
  /\ journal[n] \in Lanes
  /\ LET appliedLane == IF Bug = "wrong_payload" THEN 3 - journal[n] ELSE journal[n]
     IN state' =
          [state EXCEPT ![n] =
             IF Bug = "torn_commit" THEN @ ELSE 10 * @ + appliedLane]
  /\ cursor' = [cursor EXCEPT ![n] = @ + 1]
  /\ journal' = [journal EXCEPT ![n] = 0]
  /\ uncertifiedApply' =
       [uncertifiedApply EXCEPT ![n] = @ \/ ~certified[n]]
  /\ UNCHANGED <<received, certified>>

CrashRecoverPrepared(n) ==
  /\ journal[n] \in Lanes
  /\ LET appliedLane == IF Bug = "wrong_payload" THEN 3 - journal[n] ELSE journal[n]
     IN state' =
          [state EXCEPT ![n] =
             IF Bug = "torn_commit" THEN @ ELSE 10 * @ + appliedLane]
  /\ cursor' = [cursor EXCEPT ![n] = @ + 1]
  /\ journal' = [journal EXCEPT ![n] = 0]
  /\ uncertifiedApply' =
       [uncertifiedApply EXCEPT ![n] = @ \/ ~certified[n]]
  /\ UNCHANGED <<received, certified>>

DuplicateApply(n) ==
  /\ Bug = "duplicate_apply"
  /\ cursor[n] > 0
  /\ journal[n] = 0
  /\ state' = [state EXCEPT ![n] = 10 * @ + LastLane(cursor[n])]
  /\ UNCHANGED <<received, certified, cursor, journal, uncertifiedApply>>

RestartReplaysApplied(n) ==
  /\ Bug = "restart_replays_applied"
  /\ cursor[n] > 0
  /\ journal[n] = 0
  /\ state' = [state EXCEPT ![n] = 10 * @ + LastLane(cursor[n])]
  /\ UNCHANGED <<received, certified, cursor, journal, uncertifiedApply>>

Stable == UNCHANGED vars

Next ==
  \/ \E n \in Nodes, lane \in Lanes: ReceivePayload(n, lane)
  \/ \E n \in Nodes: InstallMergeCertificate(n)
  \/ \E n \in Nodes, lane \in Lanes: PrepareNext(n, lane)
  \/ \E n \in Nodes: CommitPrepared(n)
  \/ \E n \in Nodes: CrashRecoverPrepared(n)
  \/ \E n \in Nodes: DuplicateApply(n)
  \/ \E n \in Nodes: RestartReplaysApplied(n)
  \/ Stable

TypeInvariant ==
  /\ received \in [Nodes -> SUBSET Lanes]
  /\ certified \in [Nodes -> BOOLEAN]
  /\ cursor \in [Nodes -> 0..MergeLength]
  /\ state \in [Nodes -> 0..1212]
  /\ journal \in [Nodes -> ({0} \cup Lanes)]
  /\ uncertifiedApply \in [Nodes -> BOOLEAN]

CertifiedOrderIsExact ==
  \A n \in Nodes:
    /\ state[n] = ExpectedState(cursor[n])
    /\ (journal[n] # 0 => journal[n] = NextLane(cursor[n]))

NoExecutionBeforeMergeCertificate ==
  \A n \in Nodes: ~uncertifiedApply[n]

CompletedReplicasConverge ==
  (cursor[0] = MergeLength /\ cursor[1] = MergeLength) => state[0] = state[1]

MergeExecutionOrderExactness ==
  /\ CertifiedOrderIsExact
  /\ NoExecutionBeforeMergeCertificate
  /\ CompletedReplicasConverge

MergeExecutionOrderCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ MergeExecutionOrderExactness

SafetyFast == MergeExecutionOrderCorrectnessEnvelope

Spec == Init /\ [][Next]_vars

====
