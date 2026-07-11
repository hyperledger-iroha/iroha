---- MODULE SumeragiV2QuorumProofs ----
EXTENDS SumeragiV2Quorums, FiniteSetTheorems, TLAPS

(***************************************************************************
Deductive dual-quorum proofs.  This module is imported only by TLAPS proof
modules, keeping the executable TLC model independent of the TLAPS library.
***************************************************************************)

THEOREM DualQuorumCarriesBothThresholds ==
  \A epoch \in Epochs:
    \A signers \in SUBSET VotingRoster(epoch):
      DualQuorum(epoch, signers)
        => CountQuorum(epoch, signers) /\ PowerQuorum(epoch, signers)
BY DEF DualQuorum

(***************************************************************************
PowerUnits is a faithful set representation of stake.  The following proofs
use witnesses explicitly; no summation, monotonicity, or set-algebra axiom is
assumed.
***************************************************************************)

THEOREM PowerUnitsMonotone ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET ValidatorIds:
      left \subseteq right
        => PowerUnits(epoch, left) \subseteq PowerUnits(epoch, right)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW left \in SUBSET ValidatorIds,
              NEW right \in SUBSET ValidatorIds,
              left \subseteq right
         PROVE PowerUnits(epoch, left) \subseteq PowerUnits(epoch, right)
    <2>1. ASSUME NEW token \in PowerUnits(epoch, left)
           PROVE token \in PowerUnits(epoch, right)
      <3>1. PICK validator \in left:
               token \in {validator} \X
                            (1..VotingPower(epoch, validator))
        BY <2>1 DEF PowerUnits
      <3>2. validator \in right
        BY <1>1, <3>1
      <3> QED BY <3>1, <3>2 DEF PowerUnits
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM PowerUnitsIntersection ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET ValidatorIds:
      PowerUnits(epoch, left \cap right)
        = PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
PROOF
  <1>1. ASSUME NEW epoch \in Epochs,
              NEW left \in SUBSET ValidatorIds,
              NEW right \in SUBSET ValidatorIds
         PROVE PowerUnits(epoch, left \cap right)
                 = PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
    <2>1. PowerUnits(epoch, left \cap right)
             \subseteq PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
      <3>1. /\ left \cap right \in SUBSET ValidatorIds
            /\ left \cap right \subseteq left
            /\ left \cap right \subseteq right
        BY <1>1, Isa
      <3>2. PowerUnits(epoch, left \cap right)
               \subseteq PowerUnits(epoch, left)
        BY <1>1, <3>1, PowerUnitsMonotone
      <3>3. PowerUnits(epoch, left \cap right)
               \subseteq PowerUnits(epoch, right)
        BY <1>1, <3>1, PowerUnitsMonotone
      <3> QED BY <3>2, <3>3, Isa
    <2>2. ASSUME NEW token \in
                     PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
           PROVE token \in PowerUnits(epoch, left \cap right)
      <3>1. PICK leftValidator \in left:
               token \in {leftValidator} \X
                            (1..VotingPower(epoch, leftValidator))
        BY <2>2 DEF PowerUnits
      <3>2. PICK rightValidator \in right:
               token \in {rightValidator} \X
                            (1..VotingPower(epoch, rightValidator))
        BY <2>2 DEF PowerUnits
      <3>3. leftValidator = rightValidator
        BY <3>1, <3>2, Isa
      <3> QED BY <3>1, <3>2, <3>3 DEF PowerUnits
    <2>3. PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
             \subseteq PowerUnits(epoch, left \cap right)
      BY <2>2
    <2> QED BY <2>1, <2>3, Isa
  <1> QED BY <1>1

THEOREM PowerUnitsFinite ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         \A signers \in SUBSET VotingRoster(epoch):
           IsFiniteSet(PowerUnits(epoch, signers))
PROOF
  <1>1. ASSUME QuorumConfiguration,
              NEW epoch \in Epochs,
              NEW signers \in SUBSET VotingRoster(epoch)
         PROVE IsFiniteSet(PowerUnits(epoch, signers))
    <2>1. IsFiniteSet(PowerUnits(epoch, VotingRoster(epoch)))
      BY <1>1 DEF QuorumConfiguration
    <2>2. /\ VotingRoster(epoch) \in SUBSET ValidatorIds
          /\ signers \in SUBSET ValidatorIds
      BY <1>1 DEF QuorumConfiguration, VotingRoster
    <2>3. PowerUnits(epoch, signers)
             \subseteq PowerUnits(epoch, VotingRoster(epoch))
      BY <1>1, <2>2, PowerUnitsMonotone
    <2> QED BY <2>1, <2>3, FS_Subset
  <1> QED BY <1>1

(***************************************************************************
Count intersection.  Inclusion/exclusion plus the strict adversary bound
shows that the signer intersection is larger than the whole Byzantine set.
***************************************************************************)

THEOREM CountQuorumHonestIntersection ==
  QuorumConfiguration => CountQuorumIntersectionHasHonest
PROOF
  <1>1. ASSUME QuorumConfiguration,
              NEW epoch \in Epochs,
              NEW left \in SUBSET VotingRoster(epoch),
              NEW right \in SUBSET VotingRoster(epoch),
              CountQuorum(epoch, left),
              CountQuorum(epoch, right)
         PROVE (left \cap right \cap Honest) # {}
    <2>1. IsFiniteSet(VotingRoster(epoch))
      BY <1>1 DEF QuorumConfiguration
    <2>2. /\ left \subseteq VotingRoster(epoch)
          /\ right \subseteq VotingRoster(epoch)
          /\ Byzantine(epoch) \subseteq VotingRoster(epoch)
      BY <1>1 DEF CountQuorum, Byzantine
    <2>3. /\ IsFiniteSet(left)
          /\ IsFiniteSet(right)
          /\ IsFiniteSet(Byzantine(epoch))
      BY <2>1, <2>2, FS_Subset
    <2>4. /\ Cardinality(VotingRoster(epoch)) \in Nat
          /\ Cardinality(left) \in Nat
          /\ Cardinality(right) \in Nat
          /\ Cardinality(Byzantine(epoch)) \in Nat
      BY <2>1, <2>3, FS_CardinalityType
    <2>5. Cardinality(left) + Cardinality(right)
            > Cardinality(VotingRoster(epoch))
                + Cardinality(Byzantine(epoch))
      BY <1>1, <2>4, SMT DEF QuorumConfiguration, CountQuorum
    <2>6. /\ IsFiniteSet(left \cup right)
          /\ Cardinality(left \cup right)
               = Cardinality(left) + Cardinality(right)
                   - Cardinality(left \cap right)
      BY <2>3, FS_Union
    <2>7. left \cup right \subseteq VotingRoster(epoch)
      BY <2>2, Isa
    <2>8. Cardinality(left \cup right)
             <= Cardinality(VotingRoster(epoch))
      BY <2>1, <2>7, FS_Subset
    <2>9. IsFiniteSet(left \cap right)
      BY <2>3, FS_Intersection
    <2>10. /\ Cardinality(left \cup right) \in Nat
           /\ Cardinality(left \cap right) \in Nat
      BY <2>6, <2>9, FS_CardinalityType
    <2>11. Cardinality(left \cap right)
            > Cardinality(Byzantine(epoch))
      BY <2>4, <2>5, <2>6, <2>8, <2>10, SMT
    <2>12. (left \cap right \subseteq Byzantine(epoch))
               => Cardinality(left \cap right)
                    <= Cardinality(Byzantine(epoch))
      BY <2>3, <2>9, FS_Subset
    <2>13. ~(Cardinality(left \cap right)
                <= Cardinality(Byzantine(epoch)))
      BY <2>4, <2>10, <2>11, SMT
    <2>14. ~(left \cap right \subseteq Byzantine(epoch))
      BY <2>12, <2>13, Zenon
    <2>15. (left \cap right \cap Honest) = {}
               <=> left \cap right \subseteq Byzantine(epoch)
      BY <2>2, Isa DEF Byzantine
    <2> QED BY <2>14, <2>15, Zenon
  <1> QED BY <1>1 DEF CountQuorumIntersectionHasHonest

(***************************************************************************
Power intersection repeats the exact argument over finite power-unit sets.
One surviving non-Byzantine token identifies an honest validator in both
quorums.
***************************************************************************)

THEOREM PowerQuorumHonestIntersection ==
  QuorumConfiguration => PowerQuorumIntersectionHasHonest
PROOF
  <1>1. ASSUME QuorumConfiguration,
              NEW epoch \in Epochs,
              NEW left \in SUBSET VotingRoster(epoch),
              NEW right \in SUBSET VotingRoster(epoch),
              PowerQuorum(epoch, left),
              PowerQuorum(epoch, right)
         PROVE (left \cap right \cap Honest) # {}
    <2> DEFINE Total == PowerUnits(epoch, VotingRoster(epoch))
    <2> DEFINE LeftUnits == PowerUnits(epoch, left)
    <2> DEFINE RightUnits == PowerUnits(epoch, right)
    <2> DEFINE BadUnits == PowerUnits(epoch, Byzantine(epoch))
    <2>1. /\ IsFiniteSet(Total)
          /\ IsFiniteSet(LeftUnits)
          /\ IsFiniteSet(RightUnits)
          /\ IsFiniteSet(BadUnits)
      BY <1>1, PowerUnitsFinite
         DEF QuorumConfiguration, PowerQuorum,
             Total, LeftUnits, RightUnits, BadUnits, Byzantine
    <2>2. /\ Cardinality(Total) \in Nat
          /\ Cardinality(LeftUnits) \in Nat
          /\ Cardinality(RightUnits) \in Nat
          /\ Cardinality(BadUnits) \in Nat
      BY <2>1, FS_CardinalityType
    <2>3. Cardinality(LeftUnits) + Cardinality(RightUnits)
            > Cardinality(Total) + Cardinality(BadUnits)
      BY <1>1, <2>2, SMT
         DEF QuorumConfiguration, PowerQuorum, PowerOf,
             Total, LeftUnits, RightUnits, BadUnits
    <2>4. /\ IsFiniteSet(LeftUnits \cup RightUnits)
          /\ Cardinality(LeftUnits \cup RightUnits)
               = Cardinality(LeftUnits) + Cardinality(RightUnits)
                   - Cardinality(LeftUnits \cap RightUnits)
      BY <2>1, FS_Union
    <2>5. /\ VotingRoster(epoch) \in SUBSET ValidatorIds
          /\ left \in SUBSET ValidatorIds
          /\ right \in SUBSET ValidatorIds
      BY <1>1 DEF QuorumConfiguration, PowerQuorum, VotingRoster
    <2>6. /\ LeftUnits \subseteq Total
          /\ RightUnits \subseteq Total
      BY <1>1, <2>5, PowerUnitsMonotone
         DEF Total, LeftUnits, RightUnits
    <2>7. LeftUnits \cup RightUnits \subseteq Total
      BY <2>6, Isa
    <2>8. Cardinality(LeftUnits \cup RightUnits) <= Cardinality(Total)
      BY <2>1, <2>7, FS_Subset
    <2>9. IsFiniteSet(LeftUnits \cap RightUnits)
      BY <2>1, FS_Intersection
    <2>10. /\ Cardinality(LeftUnits \cup RightUnits) \in Nat
           /\ Cardinality(LeftUnits \cap RightUnits) \in Nat
      BY <2>4, <2>9, FS_CardinalityType
    <2>11. Cardinality(LeftUnits \cap RightUnits)
            > Cardinality(BadUnits)
      BY <2>2, <2>3, <2>4, <2>8, <2>10, SMT
    <2>12. LeftUnits \cap RightUnits
            = PowerUnits(epoch, left \cap right)
      BY <1>1, <2>5, PowerUnitsIntersection
         DEF LeftUnits, RightUnits
    <2>13. /\ left \cap right \in SUBSET ValidatorIds
           /\ Byzantine(epoch) \in SUBSET ValidatorIds
      BY <1>1, <2>5, Isa DEF Byzantine, PowerQuorum
    <2>14. (LeftUnits \cap RightUnits \subseteq BadUnits)
               => Cardinality(LeftUnits \cap RightUnits)
                    <= Cardinality(BadUnits)
      BY <2>1, <2>9, FS_Subset
    <2>15. ~(Cardinality(LeftUnits \cap RightUnits)
                <= Cardinality(BadUnits))
      BY <2>2, <2>10, <2>11, SMT
    <2>16. ~(LeftUnits \cap RightUnits \subseteq BadUnits)
      BY <2>14, <2>15, Zenon
    <2>17. ~(PowerUnits(epoch, left \cap right) \subseteq BadUnits)
      BY <2>12, <2>16
    <2>18. ~(left \cap right \subseteq Byzantine(epoch))
      BY <2>13, <2>17, PowerUnitsMonotone
         DEF BadUnits
    <2>19. (left \cap right \cap Honest) = {}
               <=> left \cap right \subseteq Byzantine(epoch)
      BY <1>1, Isa DEF Byzantine, PowerQuorum
    <2> QED BY <2>18, <2>19, Zenon
  <1> QED BY <1>1 DEF PowerQuorumIntersectionHasHonest

THEOREM DualQuorumHonestIntersection ==
  QuorumConfiguration => DualQuorumIntersectionHasHonest
PROOF
  <1>1. QuorumConfiguration => CountQuorumIntersectionHasHonest
    BY CountQuorumHonestIntersection
  <1>2. ASSUME QuorumConfiguration,
              NEW epoch \in Epochs,
              NEW left \in SUBSET VotingRoster(epoch),
              NEW right \in SUBSET VotingRoster(epoch),
              DualQuorum(epoch, left),
              DualQuorum(epoch, right)
         PROVE (left \cap right \cap Honest) # {}
    <2>1. CountQuorum(epoch, left) /\ CountQuorum(epoch, right)
      BY <1>2 DEF DualQuorum
    <2>2. CountQuorumIntersectionHasHonest
      BY <1>1, <1>2
    <2> QED BY <1>2, <2>1, <2>2
       DEF CountQuorumIntersectionHasHonest
  <1> QED BY <1>2 DEF DualQuorumIntersectionHasHonest

THEOREM AllQuorumIntersectionForms ==
  QuorumConfiguration
    => /\ CountQuorumIntersectionHasHonest
       /\ PowerQuorumIntersectionHasHonest
       /\ DualQuorumIntersectionHasHonest
BY CountQuorumHonestIntersection,
   PowerQuorumHonestIntersection,
   DualQuorumHonestIntersection

=============================================================================
