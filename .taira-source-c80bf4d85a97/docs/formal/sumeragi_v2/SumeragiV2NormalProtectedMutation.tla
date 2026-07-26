---- MODULE SumeragiV2NormalProtectedMutation ----
EXTENDS TLC

(***************************************************************************
Bounded mutation for the Normal proposal/Prepare protection boundary.  The
old predicate admitted only Completion and Progress work, so every reachable
Normal source below could remain scheduler-owned forever.  A second mutation
recomputes a stored CommitVote's class after a TC and thereby drops it from
Normal protection.  The repaired gate freezes the admission-time class,
admits exactly these four production-created kinds, and weak fairness then
retires every nondeterministically selected source.
***************************************************************************)

CONSTANTS ProtectNormal, RecomputeNormalClass

NormalProposalPrepareKinds ==
  {"AssembleBody", "DeliverProposal", "BeginPrepare", "DeliverVote"}

VARIABLES kind, scheduled, historical

vars == <<kind, scheduled, historical>>

TypeInvariant ==
  /\ kind \in NormalProposalPrepareKinds
  /\ scheduled \in BOOLEAN
  /\ historical \in BOOLEAN

Init ==
  /\ kind \in NormalProposalPrepareKinds
  /\ scheduled = TRUE
  /\ historical = FALSE

DynamicDeliveryClass == IF historical THEN "Progress" ELSE "Normal"

ProtectedNormal ==
  /\ ProtectNormal
  /\ kind \in NormalProposalPrepareKinds
  /\ (~RecomputeNormalClass \/ DynamicDeliveryClass = "Normal")

AdvanceTC ==
  /\ scheduled
  /\ kind = "DeliverVote"
  /\ ~historical
  /\ historical' = TRUE
  /\ UNCHANGED <<kind, scheduled>>

Service ==
  /\ scheduled
  /\ ProtectedNormal
  /\ scheduled' = FALSE
  /\ UNCHANGED <<kind, historical>>

Next == Service \/ AdvanceTC

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(Service)

NormalEventuallyServiced == <>~scheduled

StoredNormalRemainsProtected ==
  historical /\ scheduled /\ kind = "DeliverVote" => ProtectedNormal

=============================================================================
