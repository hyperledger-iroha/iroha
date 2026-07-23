---- MODULE SumeragiV2ServiceRankLemmas ----
EXTENDS SumeragiV2LivenessProofs, SumeragiV2TemporalLemmas

(***************************************************************************
Well-founded carrier for scheduler-owned service ranks.

The first tuple component is the concrete ownership stage and the second is
the FIFO/class position within that stage.  Moving to a lower stage is
progress even when the new stage has a larger local position, so the proof
uses the exact lexicographic ordering rather than an artificial numeric cap.
***************************************************************************)

ServiceRankCarrier == (0..8) \X Nat

OwnedServiceRankCarrier == (2..6) \X Nat

ServiceRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 0..8, Nat)

OwnedServiceRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat), OpToRel(<, Nat), 2..6, Nat)

THEOREM ServiceRankStageOrderingWellFounded ==
  IsWellFoundedOn(OpToRel(<, Nat), 0..8)
PROOF
  <1>1. 0..8 \subseteq Nat
    BY SMT
  <1> QED
    BY <1>1, NatLessThanWellFounded, IsWellFoundedOnSubset

THEOREM ServiceRankOrderingWellFounded ==
  IsWellFoundedOn(ServiceRankOrdering, ServiceRankCarrier)
BY ServiceRankStageOrderingWellFounded,
   NatLessThanWellFounded,
   WFLexPairOrdering
   DEF ServiceRankOrdering, ServiceRankCarrier

THEOREM OwnedServiceRankStageOrderingWellFounded ==
  IsWellFoundedOn(OpToRel(<, Nat), 2..6)
PROOF
  <1>1. 2..6 \subseteq Nat
    BY SMT
  <1> QED
    BY <1>1, NatLessThanWellFounded, IsWellFoundedOnSubset

THEOREM OwnedServiceRankOrderingWellFounded ==
  IsWellFoundedOn(OwnedServiceRankOrdering, OwnedServiceRankCarrier)
BY OwnedServiceRankStageOrderingWellFounded,
   NatLessThanWellFounded,
   WFLexPairOrdering
   DEF OwnedServiceRankOrdering, OwnedServiceRankCarrier

THEOREM ServiceRankOrderingMatchesLess ==
  \A left, right \in ServiceRankCarrier:
    (<<left, right>> \in ServiceRankOrdering)
      <=> ServiceRankLess(left, right)
BY SMT
   DEF ServiceRankCarrier, ServiceRankOrdering,
       ServiceRankLess, LexPairOrdering, OpToRel

THEOREM OwnedServiceRankOrderingMatchesLess ==
  \A left, right \in OwnedServiceRankCarrier:
    (<<left, right>> \in OwnedServiceRankOrdering)
      <=> ServiceRankLess(left, right)
BY SMT
   DEF OwnedServiceRankCarrier, OwnedServiceRankOrdering,
       ServiceRankLess, LexPairOrdering, OpToRel

=============================================================================
