---- MODULE SumeragiV2ResumeVoteWitness ----
EXTENDS SumeragiV2Inductive

(***************************************************************************
Bounded TLC witness for recovery of an exact locked Commit intent.

The companion configuration has one honest validator, views 0 and 1, and two
generation increments.  Reaching the predicate below therefore requires both
an installed view-0 TC and a crash/restart somewhere in the finite behavior.
The historical Commit request can enter `signVotes` only through `ResumeVote`:
the original request must have completed before the validator could persist its
timeout, and TC installation advances the current view.

TLC is intentionally asked to violate the negated predicate.  As with the
decision trace witness, this is a bounded regression witness, not a safety
invariant or deductive proof obligation.
***************************************************************************)

ResumeWitnessRosters == <<<<0>>>>
ResumeWitnessPowers == <<<<1>>>>

RecoveredHistoricalLockedCommitSigning ==
  \E request \in signVotes:
    /\ request.node \in Honest
    /\ request.vote.signer = request.node
    /\ request.vote.context = context
    /\ request.vote.phase = "Commit"
    /\ request.vote \in commitIntents
    /\ NodeTimedOut(request.node, request.vote.view)
    /\ request.vote.view < nodeView[request.node]
    /\ LockedPrepareRound(request.node, request.vote.view,
                           request.vote.subject)
    /\ generation[request.node] = MaxGeneration

NoRecoveredHistoricalLockedCommitSigning ==
  ~RecoveredHistoricalLockedCommitSigning

=============================================================================
