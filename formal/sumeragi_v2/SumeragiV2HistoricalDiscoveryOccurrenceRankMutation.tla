---- MODULE SumeragiV2HistoricalDiscoveryOccurrenceRankMutation ----
EXTENDS Naturals, FiniteSets, TLC

(***************************************************************************
Finite mutation for historical fixed-clock candidate/Serve removal.

A plain minimum is not a removal rank: after servicing owner 1, owner 2
becomes the minimum and the numeric rank rises from 1 to 2.  The repaired
rank places the exact occurrence count first.  Its finite encoding below is
`10 * count + minimum`, so the same transition falls from 21 to 12 even
though the surviving minimum is larger.

The production proof uses the lexicographic
`HistoricalDiscoveryOccurrenceDebtOrdering`; the arithmetic encoding is only
an executable mutation witness over this two-owner carrier.
***************************************************************************)

CONSTANT CountOccurrences

ASSUME CountOccurrences \in BOOLEAN

OwnerCarrier == {1, 2}

VARIABLES phase, owners, previousRank

vars == <<phase, owners, previousRank>>

OwnerMinimum(ownerSet) ==
  IF 1 \in ownerSet THEN 1 ELSE IF 2 \in ownerSet THEN 2 ELSE 0

OccurrenceRank(ownerSet) ==
  IF CountOccurrences
  THEN 10 * Cardinality(ownerSet) + OwnerMinimum(ownerSet)
  ELSE OwnerMinimum(ownerSet)

TypeInvariant ==
  /\ phase \in {"BothLive", "LowerServiced", "Drained"}
  /\ owners \subseteq OwnerCarrier
  /\ previousRank \in Nat

RankNeverIncreases ==
  OccurrenceRank(owners) <= previousRank

LowerServiceStrictlyDescends ==
  phase = "LowerServiced"
    => /\ owners = {2}
       /\ OccurrenceRank(owners) < previousRank

DrainReachesBottom ==
  phase = "Drained"
    => /\ owners = {}
       /\ OccurrenceRank(owners) = 0

Init ==
  /\ phase = "BothLive"
  /\ owners = OwnerCarrier
  /\ previousRank = OccurrenceRank(OwnerCarrier)

ServiceLowerOwner ==
  /\ phase = "BothLive"
  /\ phase' = "LowerServiced"
  /\ owners' = owners \ {1}
  /\ previousRank' = OccurrenceRank(owners)

ServiceRemainingOwner ==
  /\ phase = "LowerServiced"
  /\ phase' = "Drained"
  /\ owners' = {}
  /\ previousRank' = OccurrenceRank(owners)

Next ==
  \/ ServiceLowerOwner
  \/ ServiceRemainingOwner

=============================================================================
