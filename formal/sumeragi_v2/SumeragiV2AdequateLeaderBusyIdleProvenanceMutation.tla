---- MODULE SumeragiV2AdequateLeaderBusyIdleProvenanceMutation ----
EXTENDS TLC

(***************************************************************************
Bounded negative model for busy-to-idle source preservation.

An authenticated but reducer-disabled delivery waits while another local
completion keeps the node busy.  Completing that owner makes the node idle
without consuming the delivery.  The repaired invariant retains the exact
immutable item/evidence identity in append-only sent history; the mutation
forgets that provenance and exposes the unexplained idle owner.
***************************************************************************)

CONSTANT RetainExactAuthenticatedProvenance

ASSUME RetainExactAuthenticatedProvenance \in BOOLEAN

WireItem ==
  [kind |-> "Proposal",
   payloadIdentity |-> "signed-proposal-body-7",
   recipient |-> "ValidatorA"]

Candidate ==
  [kind |-> "DeliverProposal",
   item |-> WireItem,
   evidence |-> WireItem]

VARIABLES phase, busy, scheduled, sentHistory

vars == <<phase, busy, scheduled, sentHistory>>

NodeIdle == ~busy

\* The authenticated proposal is intentionally invalid for the current local
\* reducer state, so it is cleanup rather than phase progress.
CommandDispatchable == FALSE

AuthenticatedLeaderDiscardProvenance ==
  /\ RetainExactAuthenticatedProvenance
  /\ Candidate.item = WireItem
  /\ Candidate.evidence = WireItem
  /\ WireItem \in sentHistory

ExactLeaderSchedulerOriginReadiness ==
  scheduled /\ NodeIdle
    => \/ CommandDispatchable
       \/ AuthenticatedLeaderDiscardProvenance

ExactIdentityRetained ==
  RetainExactAuthenticatedProvenance
    => /\ Candidate.item = WireItem
       /\ Candidate.evidence = WireItem
       /\ WireItem \in sentHistory

TypeInvariant ==
  /\ phase \in {"Busy", "Idle"}
  /\ busy \in BOOLEAN
  /\ scheduled \in BOOLEAN
  /\ sentHistory \subseteq {WireItem}

Init ==
  /\ phase = "Busy"
  /\ busy = TRUE
  /\ scheduled = TRUE
  /\ sentHistory = {WireItem}

CompleteOtherLocalOwner ==
  /\ phase = "Busy"
  /\ phase' = "Idle"
  /\ busy' = FALSE
  /\ UNCHANGED <<scheduled, sentHistory>>

Next == CompleteOtherLocalOwner

=============================================================================
