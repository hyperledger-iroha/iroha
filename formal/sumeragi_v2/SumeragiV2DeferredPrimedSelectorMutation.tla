---- MODULE SumeragiV2DeferredPrimedSelectorMutation ----
EXTENDS Sequences

(***************************************************************************
Exact regression for priming the deferred selector instead of a rigid class.

Initially the cyclic cursor selects a singleton Completion queue while a
Progress command also waits.  The only non-stuttering transition removes the
Completion head and advances the cursor to Progress.  Consequently

  (DeferredClassQueue(node, SelectedDeferredClass(node)))'

denotes the still-nonempty *Progress* queue, not the post-state Completion
queue.  Comparing that expression with the pre-state Completion tail is the
retired false claim.

The repaired claim quantifies a rigid targetClass and branches on whether
that class equals the pre-state selection.  RepairedInit ranges targetClass
over every deferred class, so the positive configuration exhaustively checks
both the selected-class and foreign-class branches.
***************************************************************************)

Node == "node"
Nodes == {Node}
AsyncCommandClasses == {"Completion", "Progress", "Normal"}

VARIABLES completionQueues, progressQueues, normalQueues,
          nextDeferredClass, phase, targetClass,
          oldClaimObserved, rigidClaimObserved

vars ==
  <<completionQueues, progressQueues, normalQueues,
    nextDeferredClass, phase, targetClass,
    oldClaimObserved, rigidClaimObserved>>

CompletionCommand == "completion"
ProgressCommand == "progress"

DeferredClassQueue(node, commandClass) ==
  CASE commandClass = "Completion" -> completionQueues[node]
    [] commandClass = "Progress" -> progressQueues[node]
    [] OTHER -> normalQueues[node]

DeferredClassNonempty(node, commandClass) ==
  DeferredClassQueue(node, commandClass) # <<>>

NextCommandClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"

SelectedDeferredClass(node) ==
  IF DeferredClassNonempty(node, nextDeferredClass[node])
  THEN nextDeferredClass[node]
  ELSE
    LET secondClass == NextCommandClass(nextDeferredClass[node])
        thirdClass == NextCommandClass(secondClass)
    IN IF DeferredClassNonempty(node, secondClass)
       THEN secondClass
       ELSE thirdClass

(***************************************************************************
Retired statement: the prime covers both the queue and its state-dependent
selector.  The right side continues to denote the pre-state selected queue.
***************************************************************************)
OldPrimedSelectorClaim(node) ==
  (DeferredClassQueue(node, SelectedDeferredClass(node)))'
    = Tail(DeferredClassQueue(node, SelectedDeferredClass(node)))

(***************************************************************************
Repaired statement: targetClass is rigid across the transition.  The selected
class loses exactly its head and every foreign class is unchanged.
***************************************************************************)
RigidTargetClassConditional(node, rigidTargetClass) ==
  /\ (rigidTargetClass = SelectedDeferredClass(node)
        => (DeferredClassQueue(node, rigidTargetClass))'
             = Tail(DeferredClassQueue(node, rigidTargetClass)))
  /\ (rigidTargetClass # SelectedDeferredClass(node)
        => (DeferredClassQueue(node, rigidTargetClass))'
             = DeferredClassQueue(node, rigidTargetClass))

InitFor(targetClasses) ==
  /\ completionQueues = [node \in Nodes |-> <<CompletionCommand>>]
  /\ progressQueues = [node \in Nodes |-> <<ProgressCommand>>]
  /\ normalQueues = [node \in Nodes |-> <<>>]
  /\ nextDeferredClass = [node \in Nodes |-> "Completion"]
  /\ phase = "Ready"
  /\ targetClass \in targetClasses
  /\ oldClaimObserved = TRUE
  /\ rigidClaimObserved = TRUE

OldInit == InitFor({"Completion"})
RepairedInit == InitFor(AsyncCommandClasses)

DrainSelectedCompletion ==
  /\ phase = "Ready"
  /\ SelectedDeferredClass(Node) = "Completion"
  /\ completionQueues'
       = [completionQueues EXCEPT ![Node] = Tail(@)]
  /\ UNCHANGED <<progressQueues, normalQueues>>
  /\ nextDeferredClass'
       = [nextDeferredClass EXCEPT
            ![Node] = NextCommandClass(SelectedDeferredClass(Node))]
  /\ phase' = "Drained"
  /\ UNCHANGED targetClass
  /\ oldClaimObserved' = OldPrimedSelectorClaim(Node)
  /\ rigidClaimObserved'
       = RigidTargetClassConditional(Node, targetClass)

TerminalStutter ==
  /\ phase = "Drained"
  /\ UNCHANGED vars

Next == DrainSelectedCompletion \/ TerminalStutter

OldSpec == OldInit /\ [][Next]_vars
RepairedSpec == RepairedInit /\ [][Next]_vars

OldPrimedSelectorClaimHeld == oldClaimObserved

RepairedRigidTargetClassConditionalHeld == rigidClaimObserved

=============================================================================
