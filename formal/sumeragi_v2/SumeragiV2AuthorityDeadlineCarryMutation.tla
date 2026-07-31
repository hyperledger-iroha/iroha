---- MODULE SumeragiV2AuthorityDeadlineCarryMutation ----
EXTENDS Naturals, TLC

(***************************************************************************
Finite mutation for the authority-bound common deadline.

The repaired mode starts six exact CommitQC stages under one immutable
receipt.  Every stage uses its reserved two-tick slice, so Decision occurs at
clock 12 before the strict receipt boundary 13.

`AcceptExpiredReceipt` removes the active-window guard and starts the same
logical receipt after its boundary.  Physical service may still finish, but
it can never establish Decision-before-that-receipt.

`RechargeEachKernel` gives every later physical kernel a fresh three-tick
slice.  That equal-authority replacement is not progress: it crosses the
same immutable receipt boundary and falsifies the property.  The pair is
bounded regression evidence only; it is not production temporal proof.
***************************************************************************)

CONSTANT DeadlineMode

DeadlineModes ==
  {"FixedCommonDeadline", "AcceptExpiredReceipt", "RechargeEachKernel"}

VARIABLES now, stage, stageDue, armedAt, receiptDeadline, decided

vars == <<now, stage, stageDue, armedAt, receiptDeadline, decided>>

Init ==
  /\ DeadlineMode \in DeadlineModes
  /\ armedAt = 0
  /\ receiptDeadline = 13
  /\ now =
       IF DeadlineMode = "AcceptExpiredReceipt" THEN 14 ELSE 0
  /\ stage = 6
  /\ stageDue =
       IF DeadlineMode = "AcceptExpiredReceipt" THEN 14 ELSE 2
  /\ decided = FALSE

ReceiptActive ==
  /\ stage \in 1..6
  /\ IF DeadlineMode = "AcceptExpiredReceipt"
     THEN TRUE
     ELSE now < receiptDeadline

Tick ==
  /\ ~decided
  /\ now < stageDue
  /\ now' = now + 1
  /\ UNCHANGED
       <<stage, stageDue, armedAt, receiptDeadline, decided>>

ServiceCommitQcStage ==
  /\ ~decided
  /\ stage \in 1..6
  /\ now >= stageDue
  /\ IF stage = 1
     THEN /\ stage' = 0
          /\ decided' = TRUE
          /\ stageDue' = stageDue
     ELSE /\ stage' = stage - 1
          /\ decided' = FALSE
          /\ stageDue' =
               now
                 + IF DeadlineMode = "RechargeEachKernel"
                   THEN 3
                   ELSE 2
  /\ UNCHANGED <<now, armedAt, receiptDeadline>>

RemainDecided ==
  /\ decided
  /\ UNCHANGED vars

Next ==
  Tick \/ ServiceCommitQcStage \/ RemainDecided

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Tick)
  /\ WF_vars(ServiceCommitQcStage)

TypeInvariant ==
  /\ DeadlineMode \in DeadlineModes
  /\ now \in Nat
  /\ stage \in 0..6
  /\ stageDue \in Nat
  /\ armedAt \in Nat
  /\ receiptDeadline \in Nat \ {0}
  /\ decided \in BOOLEAN

ImmutableReceiptNeverRefreshes ==
  /\ armedAt = 0
  /\ receiptDeadline = 13

DecisionBeforeImmutableReceipt ==
  /\ decided
  /\ now < receiptDeadline

\* Safety monitor for the bounded mutation pair.  The repaired schedule
\* cannot leave the active immutable window without first deciding.
ImmutableReceiptCannotExpireBeforeDecision ==
  ReceiptActive \/ DecisionBeforeImmutableReceipt

AuthorityBoundReceiptEventuallyDecidesBeforeDeadline ==
  ReceiptActive ~> DecisionBeforeImmutableReceipt

=============================================================================
