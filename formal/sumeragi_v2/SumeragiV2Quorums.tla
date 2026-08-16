---- MODULE SumeragiV2Quorums ----
EXTENDS Naturals, Integers, Sequences, FiniteSets

(***************************************************************************
Dual count-and-power quorum algebra for one frozen height context.

Validator identifiers are canonical roster positions.  Voting power is a
positive natural for voters and zero for observers/non-members.  PowerOf is
defined as the cardinality of a finite set containing one distinct token per
unit of stake.  This is extensionally the ordinary sum of voting powers, but
lets the proof module reuse finite-set inclusion/exclusion for both count and
power intersection without trusting a separate summation axiom.
***************************************************************************)

CONSTANTS
  N,
  MaxEpoch,
  EpochRosters,
  EpochPowers,
  Honest

ValidatorIds == 0..(N - 1)
Epochs == 0..MaxEpoch

VotingPower(epoch, validator) == EpochPowers[epoch + 1][validator + 1]
RosterSequence(epoch) == EpochRosters[epoch + 1]
VotingRoster(epoch) ==
  {RosterSequence(epoch)[index]: index \in 1..Len(RosterSequence(epoch))}
Byzantine(epoch) == VotingRoster(epoch) \ Honest

(***************************************************************************
The first component makes tokens owned by different validators disjoint.
The interval is empty for zero power, so observers contribute no token.
***************************************************************************)
PowerUnits(epoch, signers) ==
  UNION {
    {validator} \X (1..VotingPower(epoch, validator)):
      validator \in signers
  }

PowerOf(epoch, signers) == Cardinality(PowerUnits(epoch, signers))

CountQuorum(epoch, signers) ==
  /\ epoch \in Epochs
  /\ signers \subseteq VotingRoster(epoch)
  /\ 3 * Cardinality(signers) > 2 * Cardinality(VotingRoster(epoch))

PowerQuorum(epoch, signers) ==
  /\ epoch \in Epochs
  /\ signers \subseteq VotingRoster(epoch)
  /\ 3 * PowerOf(epoch, signers) > 2 * PowerOf(epoch, VotingRoster(epoch))

DualQuorum(epoch, signers) ==
  /\ CountQuorum(epoch, signers)
  /\ PowerQuorum(epoch, signers)

(***************************************************************************
DualQuorum is the monotone mathematical sufficiency predicate used by the
intersection proofs.  Revision-4 wire certificates are narrower: they carry
the minimum strict count quorum, never every vote accumulated by a collector.
Keep those two concepts separate so a signer superset remains a mathematical
quorum without becoming a valid serialized certificate.

RosterSequence is the frozen canonical validator order.  Selecting candidates
whose one-based roster rank is at most q is the formal counterpart of the
production collector's ordered `.take(minimum_signer_count())` projection.
Intersecting with the roster makes the operator total even for an untyped
candidate set; the exact wire predicate prevents an incomplete projection from
being admitted as a certificate.
***************************************************************************)
CertificateSignerCount(epoch) ==
  (2 * Cardinality(VotingRoster(epoch))) \div 3 + 1

ExactCertificateQuorum(epoch, signers) ==
  /\ DualQuorum(epoch, signers)
  /\ Cardinality(signers) = CertificateSignerCount(epoch)

RosterIndex(epoch, validator) ==
  CHOOSE index \in 1..Len(RosterSequence(epoch)):
    RosterSequence(epoch)[index] = validator

CanonicalCertificateSigners(epoch, candidates) ==
  LET eligible == candidates \cap VotingRoster(epoch)
      quorumSize == CertificateSignerCount(epoch)
  IN {validator \in eligible:
       Cardinality(
         {other \in eligible:
           RosterIndex(epoch, other) <= RosterIndex(epoch, validator)})
         <= quorumSize}

QuorumConfiguration ==
  /\ N \in Nat \ {0}
  /\ MaxEpoch \in Nat
  /\ Len(EpochRosters) = MaxEpoch + 1
  /\ Len(EpochPowers) = MaxEpoch + 1
  /\ \A epoch \in Epochs:
       /\ Len(RosterSequence(epoch)) \in 1..N
       /\ \A index \in 1..Len(RosterSequence(epoch)):
            RosterSequence(epoch)[index] \in ValidatorIds
       /\ Cardinality(VotingRoster(epoch)) = Len(RosterSequence(epoch))
       /\ Len(EpochPowers[epoch + 1]) = N
       /\ \A index \in 1..N: EpochPowers[epoch + 1][index] \in Nat
       /\ \A validator \in ValidatorIds:
            (validator \in VotingRoster(epoch))
              <=> VotingPower(epoch, validator) > 0
       /\ VotingRoster(epoch) # {}
       /\ IsFiniteSet(VotingRoster(epoch))
       /\ IsFiniteSet(PowerUnits(epoch, VotingRoster(epoch)))
       /\ 3 * Cardinality(Byzantine(epoch))
            < Cardinality(VotingRoster(epoch))
       /\ 3 * PowerOf(epoch, Byzantine(epoch))
            < PowerOf(epoch, VotingRoster(epoch))
  /\ Honest \subseteq ValidatorIds

CountQuorumIntersectionHasHonest ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET VotingRoster(epoch):
      (CountQuorum(epoch, left) /\ CountQuorum(epoch, right))
        => (left \cap right \cap Honest) # {}

PowerQuorumIntersectionHasHonest ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET VotingRoster(epoch):
      (PowerQuorum(epoch, left) /\ PowerQuorum(epoch, right))
        => (left \cap right \cap Honest) # {}

DualQuorumIntersectionHasHonest ==
  \A epoch \in Epochs:
    \A left, right \in SUBSET VotingRoster(epoch):
      (DualQuorum(epoch, left) /\ DualQuorum(epoch, right))
        => (left \cap right \cap Honest) # {}

=============================================================================
