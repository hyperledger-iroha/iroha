---- MODULE SumeragiV2FixedCorridorReceiptAcquisitionMutation ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Finite mutation for atomic leader-key receipt acquisition.

The repaired transition changes one prospective self leader from view 1 to
view 2, retires only that leader's old receipt, and atomically arms the new
key from the post-state clock.  The unrelated leader keeps its immutable
view-1 receipt.

`PreStateOnly` reproduces the one-step acquisition gap: the new leader key was
not armable before the install, so retiring the old key leaves the post-state
corridor uncovered.  `RetireEveryLeader` reproduces the former blanket
retirement rule: the changed leader is rearmed, but an unrelated leader loses
its still-valid receipt.  This bounded pair is transition regression evidence,
not a temporal proof of the production service theorem.
***************************************************************************)

CONSTANT ReceiptMode

ReceiptModes == {"LeaderKeyedPostArm", "PreStateOnly", "RetireEveryLeader"}

LeaderReceipt(view, armedAt) ==
  [leader |-> "leader", view |-> view, armedAt |-> armedAt]

OtherReceipt(view, armedAt) ==
  [leader |-> "other", view |-> view, armedAt |-> armedAt]

VARIABLES phase, now, leaderView, otherView, leaderArmable, receipts

vars == <<phase, now, leaderView, otherView, leaderArmable, receipts>>

Init ==
  /\ ReceiptMode \in ReceiptModes
  /\ phase = "BeforeInstall"
  /\ now = 3
  /\ leaderView = 1
  /\ otherView = 1
  /\ leaderArmable = FALSE
  /\ receipts = {LeaderReceipt(1, 0), OtherReceipt(1, 0)}

InstallSynchronizedLeaderView ==
  /\ phase = "BeforeInstall"
  /\ phase' = "AfterInstall"
  /\ now' = now
  /\ leaderView' = 2
  /\ otherView' = otherView
  /\ leaderArmable' = TRUE
  /\ receipts' =
       CASE ReceiptMode = "PreStateOnly" ->
              {OtherReceipt(otherView, 0)}
         [] ReceiptMode = "RetireEveryLeader" ->
              {LeaderReceipt(2, now)}
         [] OTHER ->
              {LeaderReceipt(2, now), OtherReceipt(otherView, 0)}

RemainAfterInstall ==
  /\ phase = "AfterInstall"
  /\ UNCHANGED vars

Next ==
  \/ InstallSynchronizedLeaderView
  \/ RemainAfterInstall

Spec == Init /\ [][Next]_vars

TypeInvariant ==
  /\ ReceiptMode \in ReceiptModes
  /\ phase \in {"BeforeInstall", "AfterInstall"}
  /\ now \in Nat
  /\ leaderView \in Nat
  /\ otherView \in Nat
  /\ leaderArmable \in BOOLEAN
  /\ IsFiniteSet(receipts)

ArmableLeaderKeyHasReceipt ==
  leaderArmable => LeaderReceipt(leaderView, now) \in receipts

UnchangedLeaderKeyRetainsReceipt ==
  OtherReceipt(otherView, 0) \in receipts

ReceiptAcquisitionAndRetention ==
  /\ ArmableLeaderKeyHasReceipt
  /\ UnchangedLeaderKeyRetainsReceipt

=============================================================================
