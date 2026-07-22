---- MODULE SumeragiV2IndexedHeightLivenessMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Bounded adversarial witness for the exact indexed-height boundary.

The model isolates two responsive validators and the first successor context.
The repaired actions publish membership in `joinedByContext[NextHeight]` from
the height-zero predecessor or from the exact historical-recovery owner.  The
numeric mutation advances `nodeHeight` without publishing that membership.
The missing-recovery mutation leaves the lagging validator at genesis after
the fast validator has joined the successor.

`TautologicalHeightImplication` is intentionally satisfiable in the broken
recovery behavior.  It is a diagnostic showing why TLC alone cannot recognize
a theorem whose antecedent was copied into its consequent; the companion
source-contract mutation runner must reject that formula structurally.
***************************************************************************)

Nodes == {"fast", "lagging"}
GenesisHeight == 0
NextHeight == GenesisHeight + 1

VARIABLES nodeHeight, joinedByContext, predecessorPending,
          recoveryOutstanding

vars ==
  <<nodeHeight, joinedByContext, predecessorPending, recoveryOutstanding>>

ExactInit ==
  /\ nodeHeight = [node \in Nodes |-> GenesisHeight]
  /\ joinedByContext =
       [height \in GenesisHeight..NextHeight |->
          IF height = GenesisHeight THEN Nodes ELSE {}]
  /\ predecessorPending = Nodes
  /\ recoveryOutstanding = FALSE

ExactPublishFromPredecessor(node) ==
  /\ node \in predecessorPending
  /\ nodeHeight' = [nodeHeight EXCEPT ![node] = NextHeight]
  /\ joinedByContext' =
       [joinedByContext EXCEPT ![NextHeight] = @ \cup {node}]
  /\ predecessorPending' = predecessorPending \ {node}
  /\ UNCHANGED recoveryOutstanding

ExactNext ==
  \E node \in Nodes: ExactPublishFromPredecessor(node)

ExactSpec ==
  /\ ExactInit
  /\ [][ExactNext]_vars
  /\ \A node \in Nodes: WF_vars(ExactPublishFromPredecessor(node))

WeakNumericAdvance(node) ==
  /\ node \in predecessorPending
  /\ nodeHeight' = [nodeHeight EXCEPT ![node] = NextHeight + 1]
  /\ predecessorPending' = predecessorPending \ {node}
  /\ UNCHANGED <<joinedByContext, recoveryOutstanding>>

WeakNumericNext ==
  \E node \in Nodes: WeakNumericAdvance(node)

WeakNumericSpec ==
  /\ ExactInit
  /\ [][WeakNumericNext]_vars
  /\ \A node \in Nodes: WF_vars(WeakNumericAdvance(node))

RecoveryInit ==
  /\ nodeHeight =
       [node \in Nodes |->
          IF node = "fast" THEN NextHeight ELSE GenesisHeight]
  /\ joinedByContext =
       [height \in GenesisHeight..NextHeight |->
          IF height = GenesisHeight
          THEN Nodes
          ELSE {"fast"}]
  /\ predecessorPending = {}
  /\ recoveryOutstanding = TRUE

ExactHistoricalRecovery ==
  /\ recoveryOutstanding
  /\ nodeHeight' = [nodeHeight EXCEPT !["lagging"] = NextHeight]
  /\ joinedByContext' =
       [joinedByContext EXCEPT ![NextHeight] = @ \cup {"lagging"}]
  /\ recoveryOutstanding' = FALSE
  /\ UNCHANGED predecessorPending

RecoverySpec ==
  /\ RecoveryInit
  /\ [][ExactHistoricalRecovery]_vars
  /\ WF_vars(ExactHistoricalRecovery)

MissingRecoverySpec ==
  /\ RecoveryInit
  /\ [][FALSE]_vars

ExactNextContextJoined ==
  Nodes \subseteq joinedByContext[NextHeight]

ExactNextContextJoinLiveness ==
  TRUE ~> ExactNextContextJoined

WeakNodeHeightLiveness ==
  TRUE ~> \A node \in Nodes:
             nodeHeight[node] > GenesisHeight

ExactRecoveryDependencyLiveness ==
  recoveryOutstanding
    ~> "lagging" \in joinedByContext[NextHeight]

ExactHeightAntecedent ==
  /\ "fast" \in joinedByContext[NextHeight]
  /\ recoveryOutstanding

TautologicalHeightImplication ==
  [](ExactHeightAntecedent => ExactHeightAntecedent)

=============================================================================
