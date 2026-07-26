---- MODULE SumeragiV2ApplyAuthorityMutation ----
EXTENDS Naturals

(***************************************************************************
Compact mutation kernel for the Apply authorization boundary.

The repaired action may retire a non-voter's historical-recovery target only
after applying the exact durable Decision for the current context and Commit
phase.  The mutant keeps the old raw Decision-membership guard: an old-context
Decision with the same command view and subject can then be applied while the
current target is retired, leaving the node without a timed-service owner.

This is a bounded action-boundary counterexample.  It pins the authorization
contract; it does not claim that the old-context state is reachable from the
full asynchronous model's Init predicate.
***************************************************************************)

CONSTANT Mode

Node == "lagging"
CurrentContext == "current"
Contexts == {CurrentContext, "old"}
Phases == {"Commit"}
Subjects == {"block"}
Views == {0}
Heights == 0..1

QC(qcContext, qcHeight) ==
  [context |-> qcContext,
   height |-> qcHeight,
   view |-> 0,
   phase |-> "Commit",
   subject |-> "block"]

CurrentQc == QC(CurrentContext, 1)
OldQc == QC("old", 0)
DecisionQcValues == {CurrentQc, OldQc}

Application(qc) == [node |-> Node, qc |-> qc]
ApplicationSet == [node: {Node}, qc: DecisionQcValues]
Decisions == {Application(CurrentQc), Application(OldQc)}

RawDecisionAuthority(qc) ==
  Application(qc) \in Decisions

CurrentCommitDecisionAuthority(qc) ==
  /\ RawDecisionAuthority(qc)
  /\ qc.context = CurrentContext
  /\ qc.phase = "Commit"

VARIABLES applied, historicalRecoveryTargets, pendingApply

vars == <<applied, historicalRecoveryTargets, pendingApply>>

Init ==
  /\ applied = {}
  /\ historicalRecoveryTargets = {Node}
  /\ pendingApply = TRUE

RepairedApply ==
  /\ pendingApply
  /\ CurrentCommitDecisionAuthority(CurrentQc)
  /\ Application(CurrentQc) \notin applied
  /\ applied' = applied \cup {Application(CurrentQc)}
  /\ historicalRecoveryTargets' =
       historicalRecoveryTargets \ {Node}
  /\ pendingApply' = FALSE

MissingAuthorityApply ==
  /\ pendingApply
  \* This is the retired guard: durable membership without current-context
  \* Commit authority.
  /\ RawDecisionAuthority(OldQc)
  /\ Application(OldQc) \notin applied
  /\ applied' = applied \cup {Application(OldQc)}
  /\ historicalRecoveryTargets' =
       historicalRecoveryTargets \ {Node}
  /\ pendingApply' = FALSE

SelectedApply ==
  IF Mode = "Repaired"
  THEN RepairedApply
  ELSE MissingAuthorityApply

Next == SelectedApply

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(SelectedApply)

NodeHasCurrentApplication ==
  \E application \in applied:
    /\ application.node = Node
    /\ application.qc.context = CurrentContext
    /\ application.qc.phase = "Commit"

CurrentVoters == {}

TimedServiceOwners ==
  CurrentVoters
    \cup historicalRecoveryTargets
    \cup {node \in {Node}: NodeHasCurrentApplication}

TypeInvariant ==
  /\ Mode \in {"Repaired", "MissingAuthority"}
  /\ applied \subseteq ApplicationSet
  /\ historicalRecoveryTargets \subseteq {Node}
  /\ pendingApply \in BOOLEAN

HistoricalRetirementTransfersTimedOwner ==
  Node \notin historicalRecoveryTargets => NodeHasCurrentApplication

TimedOwnerNeverLost ==
  Node \in TimedServiceOwners

ApplyCompletes == <>~pendingApply

=============================================================================
