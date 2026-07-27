---- MODULE SumeragiV2AsyncOutstandingLivenessDebt ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

THEOREM TimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))

(***************************************************************************
Locked-body reproposal progress.

The exact source and outcome operators are declared in
`SumeragiV2LivenessProofs`, below this ordered shard chain.  Keeping vocabulary
below the debt theorem lets the direct retained-lock proof leaf import the
same predicates without flowing this proofless theorem back into its own
premises.

Timeout/view progress alone does not guarantee that an available lock is ever
used again.  For every responsive validator, a retained durable body at the
validator's exact lock round must eventually reach one of three explicit
outcomes: an old-round CommitQC, an unchanged later-round proposal, or a
legitimate terminal Decision / higher certified Prepare lock.  Merely changing
view, retaining bytes, or observing an unrelated proposal is not an outcome.

This temporal obligation is intentionally proofless.  It is the new dependency
between timeout/view progress and rotating-leader progress and remains
`specified_unproved` until the retained-body service ranks, proposer selection,
and the later-round rebind path are deductively composed.
***************************************************************************)
THEOREM LockedBodyReproposalProgressObligation ==
  \A initialContext:
    LockedBodyReproposalProgressProperty(AsyncLiveSpecAt(initialContext))

THEOREM RotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))

=============================================================================
