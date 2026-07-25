---- MODULE SumeragiV2AsyncOutstandingLivenessDebt ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

THEOREM TimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncSpecAt(initialContext))

THEOREM RotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncSpecAt(initialContext))

(***************************************************************************
Locked-body reproposal progress.

Timeout/view progress alone does not guarantee that an available lock is ever
used again.  For every responsive validator, a retained durable body at the
validator's exact lock round must eventually reach one of three explicit
outcomes: an old-round CommitQC, an unchanged later-round proposal, or a
legitimate terminal Decision / higher certified Prepare lock.  Merely changing
view, retaining bytes, or observing an unrelated proposal is not an outcome.

The exact release outcome is closed through responsive Decision convergence.
This does not weaken the outcome: `NodeHasDecision(node)` is already the
explicit terminal arm of `LockedBodyLegitimatelyDecidedOrSuperseded`.  The
proof fixes the responsive-voter projection at `initialContext`, proves that
an outstanding stable lock creates aggregate Decision debt, and uses the two
rotating-leader clauses plus GST stability to discharge that debt.  No view
change, retained byte, or unrelated proposal is accepted as progress.
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

LockedBodyDebtResponsiveDecisionConvergenceProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

THEOREM DebtStableLockedBodyOutsideOutcomeCreatesDecisionDebt ==
  \A initialContext, node, lockedRound, subject:
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ StableAvailableRetainedLock(node, lockedRound, subject)
    /\ ~LockedBodyReproposalOutcome(node, lockedRound, subject)
    => /\ node \in AsyncVotersAt(initialContext)
       /\ gst
       /\ ~ResponsiveNodesDecide
BY Isa
   DEF StableAvailableRetainedLock,
       LockedBodyReproposalOutcome,
       LockedBodyLegitimatelyDecidedOrSuperseded,
       ResponsiveNodesDecide

THEOREM DebtFixedResponsiveDecisionIsLockedBodyOutcome ==
  \A initialContext, node, lockedRound, subject:
    /\ AsyncCurrentResponsiveVoters = AsyncVotersAt(initialContext)
    /\ node \in AsyncVotersAt(initialContext)
    /\ ResponsiveNodesDecide
    => LockedBodyReproposalOutcome(node, lockedRound, subject)
BY Isa
   DEF ResponsiveNodesDecide,
       LockedBodyReproposalOutcome,
       LockedBodyLegitimatelyDecidedOrSuperseded

THEOREM DebtRotatingLeaderProgressSuppliesDecisionConvergence ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncSpecAt(initialContext))
      => LockedBodyDebtResponsiveDecisionConvergenceProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                RotatingLeaderProgressProperty(
                  AsyncSpecAt(initialContext))
         PROVE LockedBodyDebtResponsiveDecisionConvergenceProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE (gst /\ ~ResponsiveNodesDecide)
                   ~> ResponsiveNodesDecide
      <3>1. [](gst => []gst)
        BY <2>1, AsyncSpecKeepsGstOnceSet
      <3>2. (gst /\ ~ResponsiveNodesDecide)
               ~> (ResponsiveHonestLeaderViewReached
                     \/ ResponsiveNodesDecide)
        BY <1>1, <2>1 DEF RotatingLeaderProgressProperty
      <3>3. (gst /\ ResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1 DEF RotatingLeaderProgressProperty
      <3>4. (gst /\ ~ResponsiveNodesDecide)
               ~> ((gst /\ ResponsiveHonestLeaderViewReached
                           /\ ~ResponsiveNodesDecide)
                     \/ ResponsiveNodesDecide)
        BY <3>1, <3>2, PTL
      <3>5. ((gst /\ ResponsiveHonestLeaderViewReached
                    /\ ~ResponsiveNodesDecide)
               \/ ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <3>3, PTL
      <3> QED BY <3>4, <3>5, PTL
    <2> QED BY <2>1
         DEF LockedBodyDebtResponsiveDecisionConvergenceProperty
  <1> QED BY <1>1

THEOREM DebtDecisionConvergenceClosesLockedBodyReproposal ==
  \A initialContext:
    LockedBodyDebtResponsiveDecisionConvergenceProperty(
      AsyncSpecAt(initialContext))
      => LockedBodyReproposalProgressProperty(
           AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                LockedBodyDebtResponsiveDecisionConvergenceProperty(
                  AsyncSpecAt(initialContext))
         PROVE LockedBodyReproposalProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. ASSUME AsyncSpecAt(initialContext)
           PROVE \A node \in ValidatorIds,
                     lockedRound \in Views,
                     subject \in Subjects:
              StableAvailableRetainedLock(node, lockedRound, subject)
                ~> LockedBodyReproposalOutcome(
                     node, lockedRound, subject)
      <3>1. [](AsyncCurrentResponsiveVoters
                 = AsyncVotersAt(initialContext))
        BY <2>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
      <3>2. (gst /\ ~ResponsiveNodesDecide)
               ~> ResponsiveNodesDecide
        BY <1>1, <2>1
           DEF LockedBodyDebtResponsiveDecisionConvergenceProperty
      <3>3. ASSUME NEW node \in ValidatorIds,
                    NEW lockedRound \in Views,
                    NEW subject \in Subjects
             PROVE StableAvailableRetainedLock(
                       node, lockedRound, subject)
                     ~> LockedBodyReproposalOutcome(
                          node, lockedRound, subject)
        <4>1. [](StableAvailableRetainedLock(
                    node, lockedRound, subject)
                   /\ ~LockedBodyReproposalOutcome(
                        node, lockedRound, subject)
                  => /\ node \in AsyncVotersAt(initialContext)
                     /\ gst
                     /\ ~ResponsiveNodesDecide)
          BY <3>1,
             DebtStableLockedBodyOutsideOutcomeCreatesDecisionDebt, PTL
        <4>2. [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext)
                   /\ node \in AsyncVotersAt(initialContext)
                   /\ ResponsiveNodesDecide
                  => LockedBodyReproposalOutcome(
                       node, lockedRound, subject))
          BY DebtFixedResponsiveDecisionIsLockedBodyOutcome, PTL
        <4>3. StableAvailableRetainedLock(
                  node, lockedRound, subject)
                 ~> (LockedBodyReproposalOutcome(
                       node, lockedRound, subject)
                       \/ (/\ node \in AsyncVotersAt(initialContext)
                           /\ gst
                           /\ ~ResponsiveNodesDecide))
          BY <4>1, PTL
        <4>4. (/\ node \in AsyncVotersAt(initialContext)
                /\ gst
                /\ ~ResponsiveNodesDecide)
                 ~> LockedBodyReproposalOutcome(
                       node, lockedRound, subject)
          BY <3>1, <3>2, <4>2, PTL
        <4> QED BY <4>3, <4>4, PTL
      <3> QED BY <3>3
    <2> QED BY <2>1
         DEF LockedBodyReproposalProgressProperty
  <1> QED BY <1>1

THEOREM LockedBodyReproposalProgressObligation ==
  \A initialContext:
    LockedBodyReproposalProgressProperty(AsyncSpecAt(initialContext))
BY RotatingLeaderProgressObligation,
   DebtRotatingLeaderProgressSuppliesDecisionConvergence,
   DebtDecisionConvergenceClosesLockedBodyReproposal

=============================================================================
