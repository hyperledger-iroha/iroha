---- MODULE SumeragiV2PersistInstallGenerationMutation ----
EXTENDS Naturals

(***************************************************************************
Compact mutation kernel for PersistInstallTC at an exhausted generation.

The repaired transition records an overflow rejection while leaving the
complete durable snapshot and pending InstallTC request untouched.  The
retired saturation behavior keeps generation at MaxGeneration, retires the
request, and writes only part of the successor snapshot.
***************************************************************************)

CONSTANTS Mode, MaxGeneration

OldSnapshot ==
  [context |-> [height |-> 7, epoch |-> 2],
   nodeView |-> 4,
   lockRank |-> 3,
   lockSubject |-> "block-7",
   decidedSubject |-> "none"]

InstalledSnapshot ==
  [context |-> [height |-> 8, epoch |-> 3],
   nodeView |-> 0,
   lockRank |-> 0,
   lockSubject |-> "none",
   decidedSubject |-> "block-7"]

PartialSnapshot ==
  [OldSnapshot EXCEPT
     !.context = InstalledSnapshot.context,
     !.nodeView = InstalledSnapshot.nodeView]

VARIABLES generation, durableSnapshot, pendingInstall, outcome

vars ==
  <<generation, durableSnapshot, pendingInstall, outcome>>

Init ==
  /\ generation = MaxGeneration
  /\ durableSnapshot = OldSnapshot
  /\ pendingInstall = TRUE
  /\ outcome = "Pending"

RejectGenerationOverflow ==
  /\ pendingInstall
  /\ outcome = "Pending"
  /\ generation = MaxGeneration
  /\ outcome' = "OverflowRejected"
  /\ UNCHANGED <<generation, durableSnapshot, pendingInstall>>

SaturatingPartialPersistInstall ==
  /\ pendingInstall
  /\ outcome = "Pending"
  /\ generation = MaxGeneration
  /\ generation' = MaxGeneration
  /\ durableSnapshot' = PartialSnapshot
  /\ pendingInstall' = FALSE
  /\ outcome' = "PartiallyCommitted"

SelectedPersistInstall ==
  IF Mode = "Repaired"
  THEN RejectGenerationOverflow
  ELSE SaturatingPartialPersistInstall

Next == SelectedPersistInstall

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SelectedPersistInstall)

TypeInvariant ==
  /\ Mode \in {"Repaired", "SaturatingPartialCommit"}
  /\ MaxGeneration \in Nat
  /\ generation \in 0..MaxGeneration
  /\ durableSnapshot \in
       {OldSnapshot, InstalledSnapshot, PartialSnapshot}
  /\ pendingInstall \in BOOLEAN
  /\ outcome \in
       {"Pending", "OverflowRejected", "PartiallyCommitted"}

OverflowRejectionPreservesCompleteState ==
  generation = MaxGeneration =>
    /\ durableSnapshot = OldSnapshot
    /\ pendingInstall
    /\ outcome \in {"Pending", "OverflowRejected"}

InstallSnapshotIsAtomic ==
  durableSnapshot \in {OldSnapshot, InstalledSnapshot}

OverflowAttemptCompletes ==
  <> (outcome # "Pending")

=============================================================================
