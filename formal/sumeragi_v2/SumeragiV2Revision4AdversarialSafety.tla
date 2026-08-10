---- MODULE SumeragiV2Revision4AdversarialSafety ----
\* Bounded adversarial safety kernel for Sumeragi revision 4.  This model is
\* deliberately independent of proposal/lock progress: it lets a Byzantine
\* validator vote for both candidate bodies, lets honest validators split
\* their votes, and keeps every locally enabled action open after the first
\* CommitQC or decision.  TLC therefore searches the quorum-intersection
\* argument instead of obtaining agreement from a globally single proposal or
\* from stopping the transition system after its first certificate.

EXTENDS FiniteSets, Naturals, TLC

CONSTANTS Validators, Faulty, Bodies

N == Cardinality(Validators)
F == (N - 1) \div 3
Q == 2 * F + 1
Honest == Validators \ Faulty

ConstantOK ==
    /\ N = 4
    /\ F = 1
    /\ Q = 3
    /\ Faulty \subseteq Validators
    /\ Cardinality(Faulty) = 1
    /\ Cardinality(Bodies) = 2

VARIABLES
    fullBodies,
    commitVotes,
    commitQCs,
    decisions

vars == <<fullBodies, commitVotes, commitQCs, decisions>>

VoteBodies(validator) ==
    {body \in Bodies : <<validator, body>> \in commitVotes}

VoteCount(body) ==
    Cardinality(
        {validator \in Validators : <<validator, body>> \in commitVotes})

TypeOK ==
    /\ fullBodies \subseteq Honest \X Bodies
    /\ commitVotes \subseteq Validators \X Bodies
    /\ commitQCs \subseteq Bodies
    /\ decisions \subseteq Bodies

Init ==
    /\ ConstantOK
    /\ fullBodies = {}
    /\ commitVotes = {}
    /\ commitQCs = {}
    /\ decisions = {}

\* Full-body delivery is modeled only for honest validators.  Byzantine votes
\* are adversarial inputs and need not satisfy an availability precondition.
DeliverFullBody(validator, body) ==
    /\ validator \in Honest
    /\ body \in Bodies
    /\ <<validator, body>> \notin fullBodies
    /\ fullBodies' = fullBodies \cup {<<validator, body>>}
    /\ UNCHANGED <<commitVotes, commitQCs, decisions>>

\* This is the durable same-round sign-once rule implemented by the Commit
\* intent WAL.  Honest validators may choose different bodies, but each honest
\* validator can contribute to at most one body's quorum in this round.
HonestCommitVote(validator, body) ==
    /\ validator \in Honest
    /\ body \in Bodies
    /\ <<validator, body>> \in fullBodies
    /\ VoteBodies(validator) = {}
    /\ commitVotes' = commitVotes \cup {<<validator, body>>}
    /\ UNCHANGED <<fullBodies, commitQCs, decisions>>

\* The sole Byzantine validator may vote for both bodies.  There is
\* intentionally no sign-once, body-availability, QC, or decision guard.
ByzantineCommitVote(validator, body) ==
    /\ validator \in Faulty
    /\ body \in Bodies
    /\ <<validator, body>> \notin commitVotes
    /\ commitVotes' = commitVotes \cup {<<validator, body>>}
    /\ UNCHANGED <<fullBodies, commitQCs, decisions>>

\* Certificate formation remains enabled after another body is certified.
\* A pair of conflicting entries in commitQCs is the counterexample target.
FormCommitQC(body) ==
    /\ body \in Bodies
    /\ body \notin commitQCs
    /\ VoteCount(body) >= Q
    /\ commitQCs' = commitQCs \cup {body}
    /\ UNCHANGED <<fullBodies, commitVotes, decisions>>

\* Decision processing likewise continues after the first decision.
Decide(body) ==
    /\ body \in commitQCs
    /\ body \notin decisions
    /\ decisions' = decisions \cup {body}
    /\ UNCHANGED <<fullBodies, commitVotes, commitQCs>>

ProtocolNext ==
    \/ \E validator \in Honest, body \in Bodies :
          DeliverFullBody(validator, body)
    \/ \E validator \in Honest, body \in Bodies :
          HonestCommitVote(validator, body)
    \/ \E validator \in Faulty, body \in Bodies :
          ByzantineCommitVote(validator, body)
    \/ \E body \in Bodies : FormCommitQC(body)
    \/ \E body \in Bodies : Decide(body)

\* Once a decision exists and every locally enabled adversarial action has
\* been exhausted, the bounded one-round kernel is complete.  The absorbing
\* action keeps TLC deadlock checking active for every earlier state.
TerminalComplete ==
    /\ decisions /= {}
    /\ ~ENABLED ProtocolNext
    /\ UNCHANGED vars

Next == ProtocolNext \/ TerminalComplete

Spec == Init /\ [][Next]_vars

FixedAdversarialGeometry ==
    /\ Cardinality(Validators) = 4
    /\ Cardinality(Honest) = 3
    /\ Cardinality(Faulty) = 1
    /\ Q = 3

HonestSignOncePerRound ==
    \A validator \in Honest : Cardinality(VoteBodies(validator)) <= 1

ByzantineEquivocationRemainsEnabled ==
    \A validator \in Faulty, body \in Bodies :
        <<validator, body>> \notin commitVotes =>
            ENABLED ByzantineCommitVote(validator, body)

CommitQCsHaveQuorum ==
    \A body \in commitQCs : VoteCount(body) >= Q

DecisionsHaveCommitQC == decisions \subseteq commitQCs

\* This enabledness invariant makes a post-certificate global stop visible:
\* after the first QC, every action whose local preconditions still hold must
\* remain enabled.  In particular, the Byzantine validator can add its other
\* vote and TLC can continue trying to assemble the conflicting certificate.
PostQCExecutionRemainsOpen ==
    commitQCs /= {} =>
        /\ \A validator \in Honest, body \in Bodies :
              <<validator, body>> \notin fullBodies =>
                  ENABLED DeliverFullBody(validator, body)
        /\ \A validator \in Honest, body \in Bodies :
              (<<validator, body>> \in fullBodies
                /\ VoteBodies(validator) = {}) =>
                  ENABLED HonestCommitVote(validator, body)
        /\ \A validator \in Faulty, body \in Bodies :
              <<validator, body>> \notin commitVotes =>
                  ENABLED ByzantineCommitVote(validator, body)
        /\ \A body \in Bodies :
              (body \notin commitQCs /\ VoteCount(body) >= Q) =>
                  ENABLED FormCommitQC(body)
        /\ \A body \in commitQCs \ decisions : ENABLED Decide(body)

ConflictingCommitQCsImpossible == Cardinality(commitQCs) <= 1

DecisionAgreement == Cardinality(decisions) <= 1

=============================================================================
