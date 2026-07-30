---- MODULE SumeragiV2RepresentativeLiveScopeMutation ----
EXTENDS TLC, Naturals

(***************************************************************************
Finite mutation for the release-liveness peer-count boundary.

Generic safety and bounded transition checks may instantiate any positive
roster.  A release liveness claim is admitted only for at least four peers,
matching the representative Sumeragi network requirement.  The mutation
drops that live-only premise and therefore labels a one-peer instance as
release evidence.
***************************************************************************)

CONSTANTS NodeCount, RequireRepresentativeLiveScope

ASSUME
  /\ NodeCount \in Nat \ {0}
  /\ RequireRepresentativeLiveScope \in BOOLEAN

VARIABLES phase, liveClaim

vars == <<phase, liveClaim>>

TypeInvariant ==
  /\ phase \in {"GenericSafety", "LiveEvidence"}
  /\ liveClaim \in BOOLEAN

ReleaseLiveEvidenceIsRepresentative ==
  liveClaim => NodeCount >= 4

Init ==
  /\ phase = "GenericSafety"
  /\ ~liveClaim

AdmitReleaseLiveEvidence ==
  /\ phase = "GenericSafety"
  /\ phase' = "LiveEvidence"
  /\ liveClaim' =
       IF RequireRepresentativeLiveScope
       THEN NodeCount >= 4
       ELSE TRUE

Next == AdmitReleaseLiveEvidence

=============================================================================
