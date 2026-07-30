---- MODULE SumeragiV2CorridorExitAuthorityReceiptMutation ----
EXTENDS Naturals

(***************************************************************************
Mutation model for adequate-corridor exit authority.

The entry receipt freezes the responsive target together with its exact
context, scheduled leader, and adequate view.  Mutable node state may advance
before an owner leaves the corridor, but the exit handoff must retain the
original receipt.  The bug arm reconstructs it from the mutable exit state
and therefore changes every authority coordinate.
***************************************************************************)

CONSTANT PreserveAuthorityReceipt

VARIABLES phase,
          authorityReceipt,
          currentTarget,
          currentContext,
          currentLeader,
          currentView

vars ==
  <<phase, authorityReceipt, currentTarget,
    currentContext, currentLeader, currentView>>

OriginalAuthorityReceipt ==
  [target |-> "responsive-target",
   context |-> "entry-context",
   leader |-> "responsive-leader",
   view |-> 1]

MutableExitReceipt ==
  [target |-> currentTarget,
   context |-> currentContext,
   leader |-> currentLeader,
   view |-> currentView]

Init ==
  /\ phase = "Corridor"
  /\ authorityReceipt = OriginalAuthorityReceipt
  /\ currentTarget = "responsive-target"
  /\ currentContext = "entry-context"
  /\ currentLeader = "responsive-leader"
  /\ currentView = 1

AdvanceMutableExitState ==
  /\ phase = "Corridor"
  /\ phase' = "ReadyToExit"
  /\ currentTarget' = "nonresponsive-target"
  /\ currentContext' = "later-context"
  /\ currentLeader' = "later-leader"
  /\ currentView' = currentView + 1
  /\ UNCHANGED authorityReceipt

RecordCorridorExit ==
  /\ phase = "ReadyToExit"
  /\ phase' = "Exited"
  /\ authorityReceipt' =
       IF PreserveAuthorityReceipt
       THEN authorityReceipt
       ELSE MutableExitReceipt
  /\ UNCHANGED <<currentTarget, currentContext,
                 currentLeader, currentView>>

Next ==
  \/ AdvanceMutableExitState
  \/ RecordCorridorExit

Spec == Init /\ [][Next]_vars

CorridorExitAuthorityReceiptImmutable ==
  phase = "Exited" => authorityReceipt = OriginalAuthorityReceipt

=============================================================================
