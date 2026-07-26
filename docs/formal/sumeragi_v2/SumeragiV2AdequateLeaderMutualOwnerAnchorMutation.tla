---- MODULE SumeragiV2AdequateLeaderMutualOwnerAnchorMutation ----
EXTENDS TLC

(***************************************************************************
Bounded negative model for unanchored same-identity ownership.

Two distinct disabled candidates share one semantic identity.  The historical
source guard admits the pair because each can cite the other; discarding A
then strands B.  The repaired guard requires an independently dispatchable
member and rejects the pair before it can become scheduler-owned.
***************************************************************************)

CONSTANT RequireDispatchableOwnerAnchor

ASSUME RequireDispatchableOwnerAnchor \in BOOLEAN

CandidateA == "form-prepare-qc"
CandidateB == "begin-observe-prepare"
Candidates == {CandidateA, CandidateB}

VARIABLES phase, owners

vars == <<phase, owners>>

CommandDispatchable(candidate) ==
  /\ candidate \in Candidates
  /\ FALSE

SameIdentityLeaderOwner(candidate) ==
  \E other \in owners:
    /\ other # candidate
    /\ other \in Candidates

DispatchableSameIdentityLeaderOwner(candidate) ==
  \E other \in owners:
    /\ other # candidate
    /\ other \in Candidates
    /\ CommandDispatchable(other)

PairAdmissionAllowed ==
  IF RequireDispatchableOwnerAnchor
  THEN \E candidate \in Candidates:
         \/ CommandDispatchable(candidate)
            \/ DispatchableSameIdentityLeaderOwner(candidate)
  ELSE \A candidate \in Candidates:
         \E other \in Candidates:
           other # candidate

ExactLeaderSchedulerOriginReadiness ==
  \A candidate \in owners:
    \/ CommandDispatchable(candidate)
       \/ IF RequireDispatchableOwnerAnchor
          THEN DispatchableSameIdentityLeaderOwner(candidate)
          ELSE SameIdentityLeaderOwner(candidate)

TypeInvariant ==
  /\ phase \in {"Start", "Owned", "Rejected", "Discarded"}
  /\ owners \subseteq Candidates

Init ==
  /\ phase = "Start"
  /\ owners = {}

AttemptPairAdmission ==
  /\ phase = "Start"
  /\ phase' = IF PairAdmissionAllowed THEN "Owned" ELSE "Rejected"
  /\ owners' = IF PairAdmissionAllowed THEN Candidates ELSE {}

DiscardCandidateA ==
  /\ phase = "Owned"
  /\ CandidateA \in owners
  /\ phase' = "Discarded"
  /\ owners' = owners \ {CandidateA}

Next ==
  \/ AttemptPairAdmission
  \/ DiscardCandidateA

=============================================================================
