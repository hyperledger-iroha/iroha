---- MODULE SumeragiV2AsyncOutstandingLivenessDebt ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

THEOREM TimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))

(***************************************************************************
Locked-body reproposal progress.

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
StableAvailableRetainedLock(node, lockedRound, subject) ==
  /\ gst
  /\ node \in AsyncCurrentResponsiveVoters \cap up
  /\ lockedRound \in Views
  /\ subject \in Subjects
  /\ lockRank[node] = lockedRound
  /\ lockSubject[node] = subject
  /\ BodyHeldBy(durableBodies, node, context, lockedRound, subject)
  /\ RetainedLockedBodyHeldBy(
       retainedLockedBodies, node, context, subject)

LockedBodyCommittedInOldRound(node, lockedRound, subject) ==
  \E qc \in commitQCs:
    /\ qc.context = context
    /\ qc.phase = "Commit"
    /\ qc.view = lockedRound
    /\ qc.subject = subject
    /\ node \in qc.signers

LockedBodyReproposedUnchangedLater(lockedRound, subject) ==
  \E envelope \in proposalNetwork:
    /\ envelope.proposal.context = context
    /\ envelope.proposal.view > lockedRound
    /\ envelope.proposal.subject = subject

LockedBodyLegitimatelyDecidedOrSuperseded(
    node, lockedRound, subject) ==
  \/ NodeHasDecision(node)
  \/ /\ lockRank[node] > lockedRound
     /\ \E qc \in prepareQCs:
          /\ qc.context = context
          /\ qc.phase = "Prepare"
          /\ qc.view = lockRank[node]
          /\ qc.subject = lockSubject[node]

LockedBodyReproposalOutcome(node, lockedRound, subject) ==
  \/ LockedBodyCommittedInOldRound(node, lockedRound, subject)
  \/ LockedBodyReproposedUnchangedLater(lockedRound, subject)
  \/ LockedBodyLegitimatelyDecidedOrSuperseded(
       node, lockedRound, subject)

LockedBodyReproposalProgressProperty(spec) ==
  spec =>
    \A node \in ValidatorIds, lockedRound \in Views,
       subject \in Subjects:
      StableAvailableRetainedLock(node, lockedRound, subject)
        ~> LockedBodyReproposalOutcome(node, lockedRound, subject)

THEOREM LockedBodyReproposalProgressObligation ==
  \A initialContext:
    LockedBodyReproposalProgressProperty(AsyncLiveSpecAt(initialContext))

THEOREM RotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))

=============================================================================
