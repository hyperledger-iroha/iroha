---- MODULE SumeragiV2QuorumProofs ----
EXTENDS SumeragiV2Quorums, FiniteSetTheorems, TLAPS

(***************************************************************************
Deductive dual-quorum proofs.  This module is imported only by theorem-bearing
modules, including AsyncNetwork for its in-place proofs; it contributes proof
metadata without changing the executable transition relation.
***************************************************************************)

THEOREM DualQuorumCarriesBothThresholds ==
  \A epoch \in Epochs:
    \A signers \in SUBSET VotingRoster(epoch):
      DualQuorum(epoch, signers)
        => CountQuorum(epoch, signers) /\ PowerQuorum(epoch, signers)
BY DEF DualQuorum

THEOREM ExactCertificateQuorumCarriesSufficiencyAndCardinality ==
  \A epoch \in Epochs:
    \A signers \in SUBSET VotingRoster(epoch):
      ExactCertificateQuorum(epoch, signers)
        => /\ DualQuorum(epoch, signers)
           /\ Cardinality(signers) = CertificateSignerCount(epoch)
BY DEF ExactCertificateQuorum

THEOREM CanonicalCertificateSignersStayWithinCandidatesAndRoster ==
  \A epoch, candidates:
    CanonicalCertificateSigners(epoch, candidates)
      \subseteq candidates \cap VotingRoster(epoch)
BY DEF CanonicalCertificateSigners

THEOREM FiniteInjectiveRankPrefixHasRequestedCardinality ==
  \A members:
    IsFiniteSet(members)
      => \A rank \in [members -> Nat]:
           IsInjective(rank)
             => \A requested \in 1..Cardinality(members):
                  Cardinality(
                    {member \in members:
                      Cardinality(
                        {prior \in members:
                          rank[prior] <= rank[member]})
                        <= requested})
                    = requested
PROOF
  <1>1. ASSUME NEW members,
                IsFiniteSet(members)
         PROVE \A rank \in [members -> Nat]:
                 IsInjective(rank)
                   => \A requested \in 1..Cardinality(members):
                        Cardinality(
                          {member \in members:
                            Cardinality(
                              {prior \in members:
                                rank[prior] <= rank[member]})
                              <= requested})
                          = requested
    <2>1. ASSUME NEW rank \in [members -> Nat],
                IsInjective(rank)
           PROVE \A requested \in 1..Cardinality(members):
                   Cardinality(
                     {member \in members:
                       Cardinality(
                         {prior \in members:
                           rank[prior] <= rank[member]})
                         <= requested})
                     = requested
      <3> DEFINE Before(member) ==
            {prior \in members: rank[prior] <= rank[member]}
      <3> DEFINE Position ==
            [member \in members |-> Cardinality(Before(member))]
      <3>1. Cardinality(members) \in Nat
        BY <1>1, FS_CardinalityType
      <3>2. \A member \in members:
               /\ IsFiniteSet(Before(member))
               /\ Before(member) \subseteq members
               /\ member \in Before(member)
        BY <1>1, <2>1, FS_Subset DEF Before
      <3>3. Position \in [members -> 1..Cardinality(members)]
        <4>1. ASSUME NEW member \in members
               PROVE Cardinality(Before(member))
                       \in 1..Cardinality(members)
          <5>1. /\ Cardinality(Before(member)) \in Nat
                 /\ Cardinality(Before(member))
                      <= Cardinality(members)
            BY <1>1, <3>2, FS_CardinalityType, FS_Subset
          <5>2. Cardinality(Before(member)) # 0
            BY <3>2, FS_EmptySet
          <5> QED BY <3>1, <5>1, <5>2, SMT
        <4> QED BY <4>1 DEF Position
      <3>4. IsInjective(Position)
        <4>1. ASSUME NEW left \in members,
                    NEW right \in members,
                    Position[left] = Position[right]
               PROVE left = right
          <5>1. CASE rank[left] = rank[right]
            <6> QED BY <2>1, <4>1, <5>1 DEF IsInjective
          <5>2. CASE rank[left] < rank[right]
            <6>1. /\ Before(left) \subseteq Before(right)
                   /\ right \in Before(right) \ Before(left)
              BY <2>1, <4>1, <5>2 DEF Before
            <6>2. Cardinality(Before(left))
                     < Cardinality(Before(right))
              BY <1>1, <3>2, <6>1, FS_Subset,
                 FS_CardinalityType, SMT
            <6>3. Position[left] < Position[right]
              BY <4>1, <6>2 DEF Position
            <6> QED BY <4>1, <6>3
          <5>3. CASE rank[right] < rank[left]
            <6>1. /\ Before(right) \subseteq Before(left)
                   /\ left \in Before(left) \ Before(right)
              BY <2>1, <4>1, <5>3 DEF Before
            <6>2. Cardinality(Before(right))
                     < Cardinality(Before(left))
              BY <1>1, <3>2, <6>1, FS_Subset,
                 FS_CardinalityType, SMT
            <6>3. Position[right] < Position[left]
              BY <4>1, <6>2 DEF Position
            <6> QED BY <4>1, <6>3
          <5> QED BY <2>1, <4>1, <5>1, <5>2, <5>3, SMT
        <4> QED BY <4>1 DEF IsInjective
      <3>5. Position
               \in Bijection(members, 1..Cardinality(members))
        <4>1. Position
                 \in Injection(members, 1..Cardinality(members))
          BY <3>3, <3>4 DEF Injection
        <4>2. /\ IsFiniteSet(1..Cardinality(members))
               /\ Cardinality(1..Cardinality(members))
                    = Cardinality(members)
          BY <3>1, FS_Interval, SMT
        <4>3. Position
                 \in Surjection(members, 1..Cardinality(members))
          BY <1>1, <4>1, <4>2, FS_Injection
        <4> QED BY <4>1, <4>3 DEF Bijection
      <3>6. ASSUME NEW requested \in 1..Cardinality(members)
             PROVE Cardinality(
                     {member \in members:
                       Cardinality(
                         {prior \in members:
                           rank[prior] <= rank[member]})
                         <= requested})
                     = requested
        <4> DEFINE Selected ==
              {member \in members: Position[member] <= requested}
        <4> DEFINE SelectedPosition ==
              [member \in Selected |-> Position[member]]
        <4>1. /\ IsFiniteSet(Selected)
               /\ Selected \subseteq members
          BY <1>1, FS_Subset DEF Selected
        <4>2. SelectedPosition \in [Selected -> 1..requested]
          BY <3>3, <3>6 DEF SelectedPosition, Selected
        <4>3. IsInjective(SelectedPosition)
          BY <3>4, <4>1 DEF SelectedPosition, IsInjective
        <4>4. SelectedPosition \in Injection(Selected, 1..requested)
          BY <4>2, <4>3 DEF Injection
        <4>5. \A position \in 1..requested:
                 \E member \in Selected:
                   SelectedPosition[member] = position
          <5>1. ASSUME NEW position \in 1..requested
                 PROVE \E member \in Selected:
                         SelectedPosition[member] = position
            <6>1. position \in 1..Cardinality(members)
              BY <3>1, <3>6, <5>1, SMT
            <6>2. PICK member \in members:
                     Position[member] = position
              BY <3>5, <6>1 DEF Bijection, Surjection
            <6>3. member \in Selected
              BY <5>1, <6>2 DEF Selected
            <6> QED BY <6>2, <6>3 DEF SelectedPosition
          <5> QED BY <5>1
        <4>6. SelectedPosition \in Surjection(Selected, 1..requested)
          BY <4>2, <4>5 DEF Surjection
        <4>7. ExistsBijection(Selected, 1..requested)
          BY <4>4, <4>6 DEF ExistsBijection, Bijection
        <4>8. Cardinality(Selected) = requested
          BY <3>1, <3>6, <4>1, <4>7, FS_Bijection, FS_Interval, SMT
        <4> QED BY <4>8 DEF Selected, Position, Before
      <3> QED BY <3>6
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RosterIndexSelectsTheUniqueRosterPosition ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         \A validator \in VotingRoster(epoch):
           /\ RosterIndex(epoch, validator)
                  \in 1..Len(RosterSequence(epoch))
              /\ RosterSequence(epoch)[
                   RosterIndex(epoch, validator)] = validator
PROOF
  <1>1. ASSUME QuorumConfiguration
         PROVE \A epoch \in Epochs:
                 \A validator \in VotingRoster(epoch):
                   /\ RosterIndex(epoch, validator)
                          \in 1..Len(RosterSequence(epoch))
                      /\ RosterSequence(epoch)[
                           RosterIndex(epoch, validator)] = validator
    <2>1. ASSUME NEW epoch \in Epochs,
                NEW validator \in VotingRoster(epoch)
           PROVE /\ RosterIndex(epoch, validator)
                        \in 1..Len(RosterSequence(epoch))
                    /\ RosterSequence(epoch)[
                         RosterIndex(epoch, validator)] = validator
      <3>1. \E index \in 1..Len(RosterSequence(epoch)):
               RosterSequence(epoch)[index] = validator
        BY <2>1 DEF VotingRoster
      <3> QED BY <3>1 DEF RosterIndex
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM RosterIndexIsInjectiveOnRoster ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         IsInjective(
           [validator \in VotingRoster(epoch) |->
             RosterIndex(epoch, validator)])
PROOF
  <1>1. ASSUME QuorumConfiguration
         PROVE \A epoch \in Epochs:
                 IsInjective(
                   [validator \in VotingRoster(epoch) |->
                     RosterIndex(epoch, validator)])
    <2>1. ASSUME NEW epoch \in Epochs
           PROVE IsInjective(
                   [validator \in VotingRoster(epoch) |->
                     RosterIndex(epoch, validator)])
      <3>1. \A left, right \in VotingRoster(epoch):
               RosterIndex(epoch, left) = RosterIndex(epoch, right)
                 => left = right
        BY <1>1, RosterIndexSelectsTheUniqueRosterPosition
      <3> QED BY <3>1 DEF IsInjective
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM CanonicalCertificateSignersHaveThresholdCardinality ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         \A candidates:
           CertificateSignerCount(epoch)
               <= Cardinality(candidates \cap VotingRoster(epoch))
             => /\ CanonicalCertificateSigners(epoch, candidates)
                      \subseteq candidates \cap VotingRoster(epoch)
                /\ Cardinality(
                     CanonicalCertificateSigners(epoch, candidates))
                      = CertificateSignerCount(epoch)
PROOF
  <1>1. ASSUME QuorumConfiguration
         PROVE \A epoch \in Epochs:
                 \A candidates:
                   CertificateSignerCount(epoch)
                       <= Cardinality(
                            candidates \cap VotingRoster(epoch))
                     => /\ CanonicalCertificateSigners(epoch, candidates)
                              \subseteq
                                candidates \cap VotingRoster(epoch)
                        /\ Cardinality(
                             CanonicalCertificateSigners(epoch, candidates))
                              = CertificateSignerCount(epoch)
    <2>1. ASSUME NEW epoch \in Epochs,
                NEW candidates,
                CertificateSignerCount(epoch)
                  <= Cardinality(candidates \cap VotingRoster(epoch))
           PROVE /\ CanonicalCertificateSigners(epoch, candidates)
                        \subseteq candidates \cap VotingRoster(epoch)
                    /\ Cardinality(
                         CanonicalCertificateSigners(epoch, candidates))
                          = CertificateSignerCount(epoch)
      <3> DEFINE Eligible == candidates \cap VotingRoster(epoch)
      <3> DEFINE Rank ==
            [validator \in Eligible |-> RosterIndex(epoch, validator)]
      <3>1. IsFiniteSet(Eligible)
        BY <1>1, <2>1, FS_Intersection DEF QuorumConfiguration, Eligible
      <3>2. /\ Cardinality(Eligible) \in Nat
            /\ CertificateSignerCount(epoch)
                 \in 1..Cardinality(Eligible)
        BY <1>1, <2>1, <3>1, FS_CardinalityType, SMT
           DEF QuorumConfiguration, CertificateSignerCount, Eligible
      <3>3. Rank \in [Eligible -> Nat]
        BY <1>1, <2>1, RosterIndexSelectsTheUniqueRosterPosition
           DEF Rank, Eligible
      <3>4. IsInjective(Rank)
        <4>1. ASSUME NEW left \in Eligible,
                    NEW right \in Eligible,
                    Rank[left] = Rank[right]
               PROVE left = right
          <5>1. /\ left \in VotingRoster(epoch)
                 /\ right \in VotingRoster(epoch)
            BY <4>1 DEF Eligible
          <5>2. /\ RosterSequence(epoch)[RosterIndex(epoch, left)] = left
                 /\ RosterSequence(epoch)[RosterIndex(epoch, right)] = right
            BY <1>1, <2>1, <5>1,
               RosterIndexSelectsTheUniqueRosterPosition
          <5>3. RosterIndex(epoch, left) = RosterIndex(epoch, right)
            BY <4>1 DEF Rank
          <5> QED BY <5>2, <5>3
        <4> QED BY <4>1 DEF Rank, IsInjective
      <3>5. Cardinality(
               {member \in Eligible:
                 Cardinality(
                   {prior \in Eligible:
                     Rank[prior] <= Rank[member]})
                   <= CertificateSignerCount(epoch)})
                 = CertificateSignerCount(epoch)
        BY <3>1, <3>2, <3>3, <3>4,
           FiniteInjectiveRankPrefixHasRequestedCardinality
      <3>6. CanonicalCertificateSigners(epoch, candidates)
               = {member \in Eligible:
                   Cardinality(
                     {prior \in Eligible:
                       Rank[prior] <= Rank[member]})
                     <= CertificateSignerCount(epoch)}
        BY DEF CanonicalCertificateSigners, Rank, Eligible
      <3> QED BY <3>5, <3>6,
           CanonicalCertificateSignersStayWithinCandidatesAndRoster
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM CertificateSignerCountIsStrictCountThreshold ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         3 * CertificateSignerCount(epoch)
           > 2 * Cardinality(VotingRoster(epoch))
BY SMTT(30)
   DEF QuorumConfiguration, CertificateSignerCount

THEOREM CanonicalCertificateSignersAreExactUnderPowerPremise ==
  QuorumConfiguration
    => \A epoch \in Epochs:
         \A candidates:
           LET projected ==
                 CanonicalCertificateSigners(epoch, candidates)
           IN /\ CertificateSignerCount(epoch)
                    <= Cardinality(candidates \cap VotingRoster(epoch))
              /\ PowerQuorum(epoch, projected)
              => ExactCertificateQuorum(epoch, projected)
PROOF
  <1>1. ASSUME QuorumConfiguration
         PROVE \A epoch \in Epochs:
                 \A candidates:
                   LET projected ==
                         CanonicalCertificateSigners(epoch, candidates)
                   IN /\ CertificateSignerCount(epoch)
                            <= Cardinality(
                                 candidates \cap VotingRoster(epoch))
                      /\ PowerQuorum(epoch, projected)
                      => ExactCertificateQuorum(epoch, projected)
    <2>1. ASSUME NEW epoch \in Epochs,
                NEW candidates,
                CertificateSignerCount(epoch)
                  <= Cardinality(candidates \cap VotingRoster(epoch)),
                PowerQuorum(
                  epoch,
                  CanonicalCertificateSigners(epoch, candidates))
           PROVE ExactCertificateQuorum(
                   epoch,
                   CanonicalCertificateSigners(epoch, candidates))
      <3>1. /\ CanonicalCertificateSigners(epoch, candidates)
                    \subseteq VotingRoster(epoch)
            /\ Cardinality(
                 CanonicalCertificateSigners(epoch, candidates))
                  = CertificateSignerCount(epoch)
        BY <1>1, <2>1,
           CanonicalCertificateSignersHaveThresholdCardinality
      <3>2. CountQuorum(
               epoch,
               CanonicalCertificateSigners(epoch, candidates))
        BY <1>1, <2>1, <3>1,
           CertificateSignerCountIsStrictCountThreshold
           DEF CountQuorum
      <3> QED BY <2>1, <3>1, <3>2
           DEF ExactCertificateQuorum, DualQuorum
    <2> QED BY <2>1
  <1> QED BY <1>1

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
        BY <1>1, Zenon
      <3>2. PowerUnits(epoch, left \cap right)
               \subseteq PowerUnits(epoch, left)
        BY <1>1, <3>1, PowerUnitsMonotone
      <3>3. PowerUnits(epoch, left \cap right)
               \subseteq PowerUnits(epoch, right)
        BY <1>1, <3>1, PowerUnitsMonotone
      <3> QED BY <3>2, <3>3, Zenon
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
        BY <3>1, <3>2, Zenon
      <3> QED BY <3>1, <3>2, <3>3 DEF PowerUnits
    <2>3. PowerUnits(epoch, left) \cap PowerUnits(epoch, right)
             \subseteq PowerUnits(epoch, left \cap right)
      BY <2>2
    <2> QED BY <2>1, <2>3, Zenon
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
      BY <2>2, Zenon
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
      BY <2>2, Zenon DEF Byzantine
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
      BY <2>6, Zenon
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
      BY <1>1, <2>5, Zenon DEF Byzantine, PowerQuorum
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
      BY <1>1, Zenon DEF Byzantine, PowerQuorum
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
