---- MODULE SumeragiV2TemporalLemmas ----
EXTENDS WellFoundedInduction, TLAPS

(***************************************************************************
Mechanized first-order temporal lemmas used by the Sumeragi liveness proof.

The theorem below is the LATTICE rule from the Temporal Logic of Actions: a
state predicate that always leads either to the goal or to a strictly smaller
element of a well-founded carrier leads to the goal.  The proof is included
here so release verification checks the rule instead of importing it as an
unproved theorem declaration.
***************************************************************************)

THEOREM WellFoundedLeadsTo == ASSUME NEW R, NEW S, IsWellFoundedOn(R, S),
         TEMPORAL F(_), TEMPORAL G
  PROVE
    (\A x \in S:
       F(x) ~> (G \/ \E y \in SetLessThan(x, R, S): F(y)))
      => \A x \in S: F(x) ~> G
<1> DEFINE H(x) == F(x) ~> G
           LT(x) ==
             F(x) ~> (G \/ \E y \in SetLessThan(x, R, S): F(y))
<1>1. ASSUME NEW z \in S
      PROVE
        (\A x \in S: LT(x))
          => ((\A y \in SetLessThan(z, R, S): H(y)) => H(z))
  <2>0. (\A x \in S: LT(x)) => LT(z)
    <3> HIDE DEF LT
    <3> QED OBVIOUS
  <2>1. (\A y \in SetLessThan(z, R, S): F(y) => <>G)
          <=> ((\E y \in SetLessThan(z, R, S): F(y)) => <>G)
    OBVIOUS
  <2>2. [](\A y \in SetLessThan(z, R, S): F(y) => <>G)
          <=> []((\E y \in SetLessThan(z, R, S): F(y)) => <>G)
    BY <2>1, PTL
  <2>3. (\A y \in SetLessThan(z, R, S): [](F(y) => <>G))
          <=> [](\A y \in SetLessThan(z, R, S): F(y) => <>G)
    OBVIOUS
  <2> QED BY <2>0, <2>2, <2>3, PTL
<1>2. QED
  <2> HIDE DEF H
  <2> (\A x \in S: LT(x)) => \A x \in S: H(x)
    BY <1>1, WFInduction, IsaM("blast")
  <2> QED BY DEF H

=============================================================================
