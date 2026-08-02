---- MODULE SumeragiV2LockedBodyReproposalProgressProofs ----
EXTENDS SumeragiV2ExactLeaderSemanticRanks,
        SumeragiV2LockedBodyProposalActionProofs

(***************************************************************************
Direct retained-lock decomposition with separate source and leader clocks.

This leaf sits between timeout/view progress and rotating-leader progress, so
it cannot consume aggregate Decision convergence.  The retained-lock target
is instead followed through five explicit corridors:

  1. expose its exact PrepareQC under a fresh source service window, or finish
     its already-durable old-round Commit owner;
  2. carry that complete QC in a TC targeted at a responsive future leader;
  3. install the exact authority at that leader and obtain a fresh leader
     service window;
  4. activate one leader-owned certified-body/proposal producer origin; and
  5. descend each finite same-origin producer episode, with cross-origin
     replacement and exact re-entry kept as separate provider boundaries.

The target and leader are deliberately distinct coordinates.  In particular,
there is no requirement that `nodeView[target] = leaderView`: delayed or
reordered TCs may make either node skip views.  The transport frontier is
indexed by the recipient leader, its own view, the subject, and the complete
PrepareQC identity.  This is the weakest boundary that still authorizes the
existing certified request/Serve/response pipeline.

The four corridor properties and three separated producer-episode properties
remain explicit direct-refinement boundaries.  None treats a replenished
transport occurrence as progress.  The lower action facts prove exact
proposal-prefix successors and terminal proposal broadcast, while the
protected-owner starvation theorem closes physical candidate exit and the
same-origin semantic handoff.  Lifecycle disposition, cross-origin
replacement, and exact re-entry are paid for separately before the derived
owner-neutral semantic rank may descend.  A strict higher view remains a
non-progress handoff, so these local conditional shards are not fed into the
release theorem.  The higher temporal module closes the advertised retained-
lock property only after independently proving rotating-leader convergence.
The stronger fixed-causal-origin handoff is retained only as nonrelease
compatibility vocabulary: a legitimate replacement is not required to
resurrect the departed origin.
***************************************************************************)

(***************************************************************************
Generic convergence vocabulary remains visible to the higher rotating-leader
leaf.  It is not used anywhere in the locked-body reduction below.
***************************************************************************)

ResponsiveDecisionConvergenceProperty(specification) ==
  specification
    => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide

RetainedLockModeSource(node, lockedRound, subject) ==
  StableAvailableRetainedLock(node, lockedRound, subject)

RetainedLockModeGoal(node, lockedRound, subject) ==
  LockedBodyReproposalOutcome(node, lockedRound, subject)

RetainedLockModeActive(node, lockedRound, subject) ==
  /\ RetainedLockModeSource(node, lockedRound, subject)
  /\ ~RetainedLockModeGoal(node, lockedRound, subject)

(***************************************************************************
Source, transport, and responsive-leader frontiers.

Freshness is sequential rather than simultaneous.  The target first owns an
exact source authority under its own clock.  The TC corridor then names a
recipient leader and that leader's predecessor view.  Only after installation
does the leader require its own fresh service window.  This avoids the false
source/leader view equality which reordered delivery can invalidate.
***************************************************************************)

RetainedLockFreshSourceAuthorityFrontier(
    target, lockedRound, subject, prepareQc, sourceView) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ sourceView \in Views
  /\ LockedBodyFreshSourceAuthority(
       target, lockedRound, subject, prepareQc, sourceView)

RetainedLockAuthorityTransportFrontierFor(
    target, lockedRound, subject, prepareQc) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ \/ \E leader \in ValidatorIds, leaderView \in Views:
          \E tc:
            LockedBodyExactAuthorityTcTargetCorridor(
              target, leader, lockedRound, subject,
              prepareQc, leaderView, tc)
     \/ \E leader \in ValidatorIds, leaderView \in Views:
          LockedBodyResponsiveLeaderAuthority(
            target, leader, lockedRound, subject,
            prepareQc, leaderView)

RetainedLockFreshLeaderAuthorityFrontierFor(
    target, lockedRound, subject, prepareQc) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ \E leader \in ValidatorIds, leaderView \in Views:
       LockedBodyFreshResponsiveLeaderAuthority(
         target, leader, lockedRound, subject,
         prepareQc, leaderView)

\* A producer episode which loses its installed authority may re-enter only
\* through the same retained target and complete PrepareQC at a strictly later
\* fresh leader view.  Returning to the obsolete view is impossible after the
\* monotone local view advance, while returning to an arbitrary fresh leader
\* would forget the authority identity.  The strict view increase is an
\* explicit handoff to the unbounded rotating-leader closure; it is not
\* semantic-rank progress and is never charged to `AsyncMaximumView`.
RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
    target, lockedRound, subject, prepareQc, sourceLeaderView) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ sourceLeaderView \in Views
  /\ \E nextLeader \in ValidatorIds, nextLeaderView \in Views:
       /\ nextLeaderView > sourceLeaderView
       /\ LockedBodyFreshResponsiveLeaderAuthority(
            target, nextLeader, lockedRound, subject,
            prepareQc, nextLeaderView)

RetainedLockStrictHigherFreshLeaderAuthorityFrontier(
    target, lockedRound, subject) ==
  \E prepareQc \in QcRecordSet, sourceLeaderView \in Views:
    RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
      target, lockedRound, subject, prepareQc, sourceLeaderView)

RetainedLockOutcomeOrHigherLeaderProgressProperty(specification) ==
  specification
    => \A target \in ValidatorIds, lockedRound \in Views,
          subject \in Subjects:
         RetainedLockModeSource(target, lockedRound, subject)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontier(
                     target, lockedRound, subject))

(***************************************************************************
One exact producer episode.

The causal origin, target leader, leader view, and complete PrepareQC remain
fixed across rank descent.  PersistInstallTC may create body-recovery and
Commit-sign siblings at the QC's old view, but they are not proposal-rank
descent.  The ranked episode therefore follows only Assemble -> BeginProposal
-> PersistProposal -> SignProposal at `leaderView`; the immutable causal
origin is its episode key.
***************************************************************************)

RetainedLockFrozenCausalOriginCarrier(owner, subject) ==
  {origin \in AsyncCandidateCausalOriginSet:
     /\ AsyncCandidateCausalOriginTyped(origin)
     /\ origin.target = owner
     /\ origin.owner = owner
     /\ origin.context = context
     /\ origin.height = context.height
     /\ origin.subject \in {NoSubject, subject}}

RetainedLockFrozenCandidateIdentity(
    candidate, owner, subject, causalOrigin) ==
  /\ candidate.node = owner
  /\ candidate.consumerContext = context
  /\ candidate.height = context.height
  /\ candidate.subject = subject
  /\ candidate.bodyIdentity = subject
  /\ candidate.manifestIdentity = subject
  /\ candidate.commitmentIdentity = subject
  /\ candidate.causalOrigin = causalOrigin
  /\ causalOrigin
       \in RetainedLockFrozenCausalOriginCarrier(owner, subject)

RetainedLockExactTcEvidence(evidence, prepareQc, leaderView) ==
  \/ /\ evidence \in TcRecordSet
        /\ evidence.context = context
        /\ evidence.view + 1 = leaderView
        /\ evidence.highestPrepareQc = prepareQc
  \/ /\ evidence \in AsyncNetworkItems
        /\ evidence.kind = "TimeoutCertificate"
        /\ evidence.envelope.tc.context = context
        /\ evidence.envelope.tc.view + 1 = leaderView
        /\ evidence.envelope.tc.highestPrepareQc = prepareQc

RetainedLockCandidateCarriesPrepareAuthority(
    candidate, prepareQc, leaderView) ==
  \/ candidate.evidence = prepareQc
  \/ RetainedLockExactTcEvidence(
       candidate.evidence, prepareQc, leaderView)
  \/ /\ candidate.evidence \in ProposalRecordSet
        /\ candidate.evidence.context = context
        /\ candidate.evidence.view = leaderView
        /\ candidate.evidence.subject = prepareQc.subject
        /\ candidate.evidence.highestPrepareQc = prepareQc
        /\ candidate.evidence.timeoutCertificate
             # NoTimeoutCertificate
        /\ candidate.evidence.timeoutCertificate.highestPrepareQc
             = prepareQc

RetainedLockOldRoundCommitCandidateRank(
    target, lockedRound, subject, causalOrigin, candidate, rank) ==
  /\ RetainedLockFrozenCandidateIdentity(
       candidate, target, subject, causalOrigin)
  /\ candidate.view = lockedRound
  /\ ExactLeaderCommitRank(candidate, rank)

RetainedLockLeaderProducerKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal", "SignProposal"}

(***************************************************************************
Only the actual proposal producer is ranked here.  PersistInstallTC also
emits locked-body Fetch and Commit-sign siblings, but those are independent
owners: Fetch/Store/Validate terminates at durable body evidence and may leave
the higher Assemble sibling untouched.  Treating those siblings as one
strictly descending proposal rank was the replenishment lasso this leaf must
not hide.  Their finite replacement episode is isolated below instead.
***************************************************************************)

RetainedLockFrozenProducerCausalOriginCarrier(
    owner, lockedRound, subject, prepareQc, leaderView) ==
  {origin \in RetainedLockFrozenCausalOriginCarrier(owner, subject):
    /\ origin.view \in {lockedRound, leaderView - 1, leaderView}
    /\ origin.payload.body \in {NoSubject, subject}
    /\ origin.payload.manifest \in {NoSubject, subject}
    /\ origin.payload.commitment \in {NoSubject, subject}
    /\ \E evidence \in AsyncEvidenceSet:
         /\ origin.payload.authority
              = AsyncRouteNeutralCandidateEvidence(evidence)
         /\ \/ evidence = prepareQc
            \/ RetainedLockExactTcEvidence(
                 evidence, prepareQc, leaderView)
            \/ /\ evidence \in ProposalRecordSet
                  /\ evidence.context = context
                  /\ evidence.view = leaderView
                  /\ evidence.subject = subject
                  /\ evidence.highestPrepareQc = prepareQc
                  /\ evidence.timeoutCertificate
                       # NoTimeoutCertificate
                  /\ evidence.timeoutCertificate.highestPrepareQc
                       = prepareQc}

RetainedLockFrozenProducerCandidateIdentity(
    candidate, owner, lockedRound, subject, prepareQc, leaderView,
    causalOrigin) ==
  /\ RetainedLockFrozenCandidateIdentity(
       candidate, owner, subject, causalOrigin)
  /\ candidate.item = NoAsyncItem
  /\ AsyncRouteNeutralCandidateEvidence(candidate.evidence)
       = causalOrigin.payload.authority
  /\ causalOrigin
       \in RetainedLockFrozenProducerCausalOriginCarrier(
            owner, lockedRound, subject, prepareQc, leaderView)

(***************************************************************************
Immutable physical owner identities for one causal producer episode.

`AsyncCandidateServiceIdentity` intentionally omits consumer view and
generation.  Once the causal origin, target context, producer view, subject,
normalized authority, body identity, class, and stage are frozen, a retry has
the same identity even after a local restart.  The episode universe below is
therefore the finite product of the three command classes and four proposal
stages.  It does not include a replacement with a different causal origin;
that is the separate re-entry residual below and is never counted as rank
descent.
***************************************************************************)

RetainedLockFrozenProducerOwnerIdentity(
    commandClass, producerKind, owner, producerContext,
    subject, leaderView, causalOrigin) ==
  [target |-> owner,
   context |-> producerContext,
   height |-> producerContext.height,
   leader |-> Leader(producerContext, leaderView),
   view |-> leaderView,
   subject |-> subject,
   phase |-> producerKind,
   owner |-> owner,
   kind |-> "Candidate",
   payload |->
     [class |-> commandClass,
      workKind |-> producerKind,
      causalOrigin |-> causalOrigin,
      item |-> AsyncRouteNeutralCandidateItem(NoAsyncItem),
      evidence |-> causalOrigin.payload.authority,
      body |-> subject,
      manifest |-> subject,
      commitment |-> subject]]

RetainedLockFrozenProducerOwnerUniverse(
    owner, producerContext, subject, leaderView, causalOrigin) ==
  {RetainedLockFrozenProducerOwnerIdentity(
     commandClass, producerKind, owner, producerContext,
     subject, leaderView, causalOrigin):
     commandClass \in AsyncCommandClasses,
     producerKind \in RetainedLockLeaderProducerKinds}

RetainedLockSameOriginLiveProducerOwnerIdentitySet(
    owner, lockedRound, subject, prepareQc, leaderView, causalOrigin) ==
  {AsyncCandidateServiceIdentity(candidate):
     candidate \in
       {scheduled \in AsyncCandidateSet:
          /\ CandidateScheduled(scheduled)
          /\ RetainedLockFrozenProducerCandidateIdentity(
               scheduled, owner, lockedRound, subject, prepareQc, leaderView,
               causalOrigin)
          /\ scheduled.view = leaderView
          /\ scheduled.kind \in RetainedLockLeaderProducerKinds}}

RetainedLockSameOriginProducerEpisodeLiveOwners(
    owner, lockedRound, subject, prepareQc, leaderView, causalOrigin) ==
  {"Candidate"}
    \X RetainedLockSameOriginLiveProducerOwnerIdentitySet(
         owner, lockedRound, subject, prepareQc, leaderView, causalOrigin)

RetainedLockSameOriginProducerEpisodeAtBudget(
    owner, producerContext, lockedRound, subject, prepareQc,
    leaderView, causalOrigin, known, budget) ==
  AsyncTargetNeutralLifecycleEpisodeAtBudget(
    RetainedLockFrozenProducerOwnerUniverse(
      owner, producerContext, subject, leaderView, causalOrigin),
    {},
    RetainedLockSameOriginProducerEpisodeLiveOwners(
      owner, lockedRound, subject, prepareQc, leaderView, causalOrigin),
    known, budget)

RetainedLockLeaderProducerCandidateRank(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, candidate, rank) ==
  /\ LockedBodyResponsiveLeaderAuthority(
       target, leader, lockedRound, subject, prepareQc, leaderView)
  /\ RetainedLockFrozenProducerCandidateIdentity(
       candidate, leader, lockedRound, subject, prepareQc, leaderView,
       causalOrigin)
  /\ candidate.consumerView = leaderView
  /\ candidate.view = leaderView
  /\ candidate.kind \in RetainedLockLeaderProducerKinds
  /\ RetainedLockCandidateCarriesPrepareAuthority(
       candidate, prepareQc, leaderView)
  /\ ExactLeaderProposalRank(candidate, rank)

RetainedLockExactCandidateRank(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, candidate, rank) ==
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ ExactLeaderCandidateRank(candidate, rank)
  /\ RetainedLockLeaderProducerCandidateRank(
       target, leader, lockedRound, subject, prepareQc, leaderView,
       causalOrigin, candidate, rank)

RetainedLockCandidateRankFrontier(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, rank) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ leader \in ValidatorIds
  /\ leaderView \in Views
  /\ prepareQc \in QcRecordSet
  /\ causalOrigin \in AsyncCandidateCausalOriginSet
  /\ rank \in ExactLeaderSemanticRankCarrier
  /\ \E candidate \in AsyncCandidateSet:
       RetainedLockExactCandidateRank(
         target, leader, lockedRound, subject, prepareQc, leaderView,
         causalOrigin, candidate, rank)

RetainedLockRankedEpisodeFrontier(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin) ==
  \E rank \in ExactLeaderSemanticRankCarrier:
    RetainedLockCandidateRankFrontier(
      target, leader, lockedRound, subject, prepareQc, leaderView,
      causalOrigin, rank)

RetainedLockOwnerNeutralCandidateRankFrontier(
    target, leader, lockedRound, subject, prepareQc, leaderView, rank) ==
  \E causalOrigin \in AsyncCandidateCausalOriginSet:
    RetainedLockCandidateRankFrontier(
      target, leader, lockedRound, subject, prepareQc, leaderView,
      causalOrigin, rank)

RetainedLockRankedFrontier(target, lockedRound, subject) ==
  \E leader \in ValidatorIds, leaderView \in Views,
     prepareQc \in QcRecordSet,
     causalOrigin \in AsyncCandidateCausalOriginSet:
    RetainedLockRankedEpisodeFrontier(
      target, leader, lockedRound, subject, prepareQc, leaderView,
      causalOrigin)

RetainedLockSourceExposureFrontier(target, lockedRound, subject) ==
  \/ RetainedLockRankedFrontier(target, lockedRound, subject)
  \/ \E prepareQc \in QcRecordSet, sourceView \in Views:
       RetainedLockFreshSourceAuthorityFrontier(
         target, lockedRound, subject, prepareQc, sourceView)

(***************************************************************************
Static safety anchors.
***************************************************************************)

THEOREM RetainedLockSourceModeBindsExactPrepareAuthority ==
  \A target \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects:
    /\ StrongInductiveInvariant
    /\ RetainedLockModeSource(target, lockedRound, subject)
    => LockedBodySourcePrepareAuthority(
         target, lockedRound, subject, lockPrepareQc[target])
BY StableRetainedLockBindsExactSourcePrepareAuthority
   DEF RetainedLockModeSource

THEOREM RetainedLockLaterProposalIsSafeForDurableLock ==
  \A node \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects:
    \A proposal:
      /\ StableAvailableRetainedLock(node, lockedRound, subject)
      /\ proposal.view > lockedRound
      /\ proposal.subject = subject
      => DurableProposalSafeForLock(node, proposal)
BY Isa
   DEF StableAvailableRetainedLock,
       DurableProposalSafeForLock

THEOREM OldRoundCommitFrontierRejectsSplitRoundCommit ==
  \A target \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects:
    \A causalOrigin, candidate, rank:
      RetainedLockOldRoundCommitCandidateRank(
        target, lockedRound, subject, causalOrigin, candidate, rank)
        => candidate.view = lockedRound
BY DEF RetainedLockOldRoundCommitCandidateRank

THEOREM RetainedLockFreshSourceAuthorityHasExactClockOwner ==
  \A target \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, sourceView \in Views:
    \A prepareQc:
      RetainedLockFreshSourceAuthorityFrontier(
        target, lockedRound, subject, prepareQc, sourceView)
        => /\ LockedBodySourcePrepareAuthority(
                 target, lockedRound, subject, prepareQc)
           /\ nodeView[target] = sourceView
           /\ ~AsyncOlderOrEqualTimeoutLifecycleOwned(
                target, context, sourceView)
BY DEF RetainedLockFreshSourceAuthorityFrontier,
       LockedBodyFreshSourceAuthority,
       AsyncFreshNodeServiceWindow

THEOREM RetainedLockFreshLeaderAuthorityHasExactTargetedOrigin ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc:
      LockedBodyFreshResponsiveLeaderAuthority(
        target, leader, lockedRound, subject, prepareQc, leaderView)
        => /\ LockedBodySourcePrepareAuthority(
                 target, lockedRound, subject, prepareQc)
           /\ HistoricalLockedPrepareSource(leader, prepareQc)
           /\ Leader(context, leaderView) = leader
           /\ nodeView[leader] = leaderView
           /\ prepareQc.view = lockedRound
           /\ prepareQc.subject = subject
           /\ ~AsyncOlderOrEqualTimeoutLifecycleOwned(
                leader, context, leaderView)
BY DEF LockedBodyFreshResponsiveLeaderAuthority,
       LockedBodyResponsiveLeaderAuthority,
       LockedBodySourcePrepareAuthority,
       AsyncFreshNodeServiceWindow

THEOREM RetainedLockFrozenProducerOwnerUniverseIsFinite ==
  \A owner, producerContext, subject, leaderView, causalOrigin:
    IsFiniteSet(
      RetainedLockFrozenProducerOwnerUniverse(
        owner, producerContext, subject, leaderView, causalOrigin))
BY FS_Product, FS_Image, Isa
   DEF RetainedLockFrozenProducerOwnerUniverse,
       RetainedLockLeaderProducerKinds, AsyncCommandClasses

THEOREM RetainedLockFrozenProducerOwnerUniverseIsPrimeInvariant ==
  \A owner, producerContext, subject, leaderView, causalOrigin:
    RetainedLockFrozenProducerOwnerUniverse(
      owner, producerContext, subject, leaderView, causalOrigin)'
      = RetainedLockFrozenProducerOwnerUniverse(
          owner, producerContext, subject, leaderView, causalOrigin)
BY Isa
   DEF RetainedLockFrozenProducerOwnerUniverse,
       RetainedLockFrozenProducerOwnerIdentity

THEOREM RetainedLockRankedCandidateHasExactFrozenOwnerIdentity ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      RetainedLockExactCandidateRank(
        target, leader, lockedRound, subject, prepareQc, leaderView,
        causalOrigin, candidate, rank)
        => AsyncCandidateServiceIdentity(candidate)
             = RetainedLockFrozenProducerOwnerIdentity(
                 candidate.class, candidate.kind, leader, context,
                 subject, leaderView, causalOrigin)
BY Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       RetainedLockFrozenProducerOwnerIdentity,
       AsyncCandidateServiceIdentity,
       AsyncCandidateServicePayload

THEOREM RetainedLockRankedCandidateOwnerIdentityIsInFrozenUniverse ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      RetainedLockExactCandidateRank(
        target, leader, lockedRound, subject, prepareQc, leaderView,
        causalOrigin, candidate, rank)
        => AsyncCandidateServiceIdentity(candidate)
             \in RetainedLockFrozenProducerOwnerUniverse(
                  leader, context, subject, leaderView, causalOrigin)
BY RetainedLockRankedCandidateHasExactFrozenOwnerIdentity, Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerOwnerUniverse,
       AsyncCandidateSet, AsyncCandidateTyped

THEOREM RetainedLockFrozenProducerRetryIdentityIsStable ==
  \A owner, lockedRound, subject, prepareQc, leaderView, causalOrigin:
    \A left, right \in AsyncCandidateSet:
      /\ RetainedLockFrozenProducerCandidateIdentity(
           left, owner, lockedRound, subject, prepareQc, leaderView,
           causalOrigin)
      /\ RetainedLockFrozenProducerCandidateIdentity(
           right, owner, lockedRound, subject, prepareQc, leaderView,
           causalOrigin)
      /\ left.class = right.class
      /\ left.kind = right.kind
      /\ left.view = leaderView
      /\ right.view = leaderView
      => AsyncCandidateServiceIdentity(left)
           = AsyncCandidateServiceIdentity(right)
BY Isa
   DEF RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       AsyncCandidateServiceIdentity, AsyncCandidateServicePayload

THEOREM RetainedLockFrozenProducerRetryCoalescesAgainstExistingOwner ==
  \A owner, lockedRound, subject, prepareQc, leaderView, causalOrigin:
    \A existing, retry \in AsyncCandidateSet:
      /\ RetainedLockFrozenProducerCandidateIdentity(
           existing, owner, lockedRound, subject, prepareQc, leaderView,
           causalOrigin)
      /\ RetainedLockFrozenProducerCandidateIdentity(
           retry, owner, lockedRound, subject, prepareQc, leaderView,
           causalOrigin)
      /\ existing.class = retry.class
      /\ existing.kind = retry.kind
      /\ existing.view = leaderView
      /\ retry.view = leaderView
      /\ CandidateAdmissionCoalesced(existing)
      => CandidateAdmissionCoalesced(retry)
BY RetainedLockFrozenProducerRetryIdentityIsStable, Isa
   DEF CandidateAdmissionCoalesced,
       AsyncCandidateServiceIdentityScheduled,
       AsyncCandidateServiceCoalesced,
       AsyncCandidateTransientServiceMarked,
       AsyncCandidateTerminalTombstoned,
       AsyncCandidateTransientServiceRecordsFor,
       AsyncCandidateTerminalRecordsFor

THEOREM RetainedLockSameOriginLiveOwnersStayInsideFrozenUniverse ==
  \A owner, lockedRound, subject, prepareQc, leaderView, causalOrigin:
    RetainedLockSameOriginLiveProducerOwnerIdentitySet(
      owner, lockedRound, subject, prepareQc, leaderView, causalOrigin)
      \subseteq RetainedLockFrozenProducerOwnerUniverse(
                    owner, context, subject, leaderView, causalOrigin)
BY Isa
   DEF RetainedLockSameOriginLiveProducerOwnerIdentitySet,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       RetainedLockFrozenProducerOwnerUniverse,
       RetainedLockFrozenProducerOwnerIdentity,
       AsyncCandidateServiceIdentity, AsyncCandidateServicePayload,
       AsyncCandidateSet, AsyncCandidateTyped

THEOREM RetainedLockSameOriginProducerEpisodeBudgetIsFiniteAndCoalesced ==
  \A owner, producerContext, lockedRound, subject, prepareQc,
     leaderView, causalOrigin, known, budget:
    RetainedLockSameOriginProducerEpisodeAtBudget(
      owner, producerContext, lockedRound, subject, prepareQc,
      leaderView, causalOrigin, known, budget)
      => /\ budget \in Nat
         /\ (RetainedLockSameOriginProducerEpisodeLiveOwners(
                owner, lockedRound, subject, prepareQc,
                leaderView, causalOrigin)
                \subseteq known
               <=> AsyncTargetNeutralLifecycleDiscoveredOwnerSet(
                     RetainedLockSameOriginProducerEpisodeLiveOwners(
                       owner, lockedRound, subject, prepareQc,
                       leaderView, causalOrigin),
                     known) = {})
BY AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced
   DEF RetainedLockSameOriginProducerEpisodeAtBudget

(***************************************************************************
Local candidate-service lifecycle induction.

The same invariant is used by higher exact-Decision and adequate-leader
modules, but this retained-lock leaf cannot import either one without creating
a dependency cycle.  Its initialization and preservation ingredients are all
proved in the lower asynchronous transition modules, so the narrow induction
is repeated here.  It supplies only transient service markers and terminal
tombstones; it supplies no temporal protocol progress.
***************************************************************************)

THEOREM RetainedLockAsyncInitEstablishesCandidateServiceLifecycle ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateServiceLifecycleInvariant
BY AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision,
   Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AsyncCandidateLifecycleAdmissions,
       AsyncInitialCandidateLifecycleAdmissions,
       AsyncCandidateLifecycleAdmission,
       AsyncCandidateProducerContinuationScheduledExclusionInvariant,
       AsyncCandidateProducerContinuationBlocks,
       AsyncCandidateProducerContinuations,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet

THEOREM RetainedLockAsyncNextPreservesCandidateServiceLifecycle ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ AsyncNext
  => AsyncCandidateServiceLifecycleInvariant'
BY AsyncNextPreservesControlServiceStateTypeInvariant,
   AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision,
   AsyncControlServiceTransitionPreservesSemanticHandoffCoverage,
   AsyncNextPreservesCandidateProducerContinuationScheduledExclusion,
   AsyncCandidateServicesThisStepIsSingleton,
   AsyncCandidateSuccessfulServiceInstallsTransientMarker,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   AsyncCandidateDiscardIsNotSemanticService,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateDiscardRetiresLogicalLifecycle,
   AsyncCandidateTransientMarkerCoalescesFreshCandidate,
   AsyncCandidateTerminalTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateSameHeightRestartPreservesTombstone,
   AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction,
   IsaT(600)
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncStrongTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, SelectedLocalAdmissionAdvance,
       SerializedLocalPrecedesServeIngressStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, ServiceIoWorkerWork,
       FifoRuntimeStep, DeferredDrainStep,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter

THEOREM RetainedLockAsyncSpecAlwaysCandidateServiceLifecycle ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateServiceLifecycleInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateServiceLifecycleInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateServiceLifecycleInvariant
      BY RetainedLockAsyncInitEstablishesCandidateServiceLifecycle
    <2>2. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant)
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant, PTL
    <2>3. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceLifecycleInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateServiceLifecycleInvariant'
      BY RetainedLockAsyncNextPreservesCandidateServiceLifecycle, Isa
         DEF AsyncAllVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Capacity-derived replacement bound and non-resurrection anchors.

Cross-origin producer values need not be enumerated: every live or dormant
serviced lifecycle owns an injective internal slot whose bound is derived from
the configured queue/deferred/ingress geometry.  A serviced exact identity
retains that reservation until a strict context/view/Decision exit, and a
terminal identity cannot be admitted again at the retired stage.  Thus a
replacement is charged to a finite internal slot; merely installing it is not
semantic progress.
***************************************************************************)

THEOREM RetainedLockProducerServicedLifecycleCarrierIsCapacityBounded ==
  \A leader \in ValidatorIds:
    /\ AsyncCandidateServiceOwnerPartitionInvariantIn(
         asyncControlServiceState)
    /\ AsyncCandidateLifecycleSlotInjectionInvariantIn(
         asyncControlServiceState)
    => Cardinality(
         AsyncCandidateLifecycleServiceOwnerTokensForNodeIn(
           asyncControlServiceState, leader))
         <= AsyncServicedCandidateLifecycleCapacity
BY AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded

THEOREM RetainedLockServicedProducerLifecycleRetainsReservationUntilExit ==
  \A state, record, leader, lockedRound, subject, prepareQc, leaderView:
    /\ record \in state.candidateLifecycleAdmissions
    /\ record.node = leader
    /\ record.origin
         \in RetainedLockFrozenProducerCausalOriginCarrier(
              leader, lockedRound, subject, prepareQc, leaderView)
    /\ record.retired
    /\ AsyncCandidateLifecycleServiceRecordCoversIn(state, record)
    /\ ~AsyncCandidateLifecyclePermanentlyObsoleteAfter(record)
    => record \in
         (AsyncCandidateLifecycleStateAfterCompaction(state))
           .candidateLifecycleAdmissions
BY AsyncCandidateLifecycleTransientMarkerRetainsItsReservation

THEOREM RetainedLockTerminalProducerIdentityCannotResurrect ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      LET identity == AsyncCandidateAdmissionIdentity(candidate)
      IN /\ AsyncCandidateServiceLifecycleInvariant
         /\ RetainedLockExactCandidateRank(
              target, leader, lockedRound, subject, prepareQc, leaderView,
              causalOrigin, candidate, rank)
         /\ AsyncCandidateTerminalIdentityTombstoned(identity.service)
         /\ identity \notin AsyncScheduledCandidateAdmissionIdentities
         /\ gst
         /\ [AsyncNext]_AsyncAllVars
         => /\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)'
            /\ identity
                 \notin AsyncScheduledCandidateAdmissionIdentities'
BY AsyncCandidateTerminalIdentityCannotReactivateAtGst, Isa
   DEF RetainedLockExactCandidateRank, ExactLeaderCandidateRank,
       AsyncCandidateAdmissionIdentitySet

THEOREM RetainedLockServicedProducerIdentityCannotReplenishSameGeneration ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ AsyncCandidateTransientServiceActive(candidate)
      /\ candidate.consumerGeneration = generation[candidate.node]
      /\ gst
      /\ [AsyncNext]_AsyncAllVars
      /\ ~AsyncCandidateTransientMarkerExitThisStep(candidate)
      => /\ AsyncCandidateTransientServiceActive(candidate)'
         /\ ~CandidateScheduled(candidate)'
BY AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst, Isa
   DEF RetainedLockExactCandidateRank, ExactLeaderCandidateRank

THEOREM RetainedLockLeaderRankContainsOnlyExactProposalProducer ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      RetainedLockLeaderProducerCandidateRank(
        target, leader, lockedRound, subject, prepareQc, leaderView,
        causalOrigin, candidate, rank)
        => /\ candidate.kind \in RetainedLockLeaderProducerKinds
           /\ candidate.view = leaderView
           /\ candidate.causalOrigin = causalOrigin
           /\ causalOrigin
                \in RetainedLockFrozenProducerCausalOriginCarrier(
                     leader, lockedRound, subject,
                     prepareQc, leaderView)
           /\ RetainedLockCandidateCarriesPrepareAuthority(
                candidate, prepareQc, leaderView)
BY DEF RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity

THEOREM RetainedLockRankedCandidateBindsFrozenTargetLeaderEpisode ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      RetainedLockExactCandidateRank(
        target, leader, lockedRound, subject, prepareQc, leaderView,
        causalOrigin, candidate, rank)
        => /\ candidate.causalOrigin = causalOrigin
           /\ causalOrigin.target = candidate.node
           /\ causalOrigin.owner = candidate.node
           /\ causalOrigin.context = context
           /\ causalOrigin.height = context.height
           /\ causalOrigin.leader =
                Leader(causalOrigin.context, causalOrigin.view)
           /\ causalOrigin.view \in Views
           /\ causalOrigin.subject \in {NoSubject, subject}
           /\ causalOrigin.phase \in AsyncWorkKinds
           /\ causalOrigin.payload.workKind = causalOrigin.phase
           /\ candidate.consumerContext = context
           /\ candidate.height = context.height
           /\ candidate.subject = subject
BY Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockOldRoundCommitCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenProducerCausalOriginCarrier,
       RetainedLockFrozenCandidateIdentity,
       RetainedLockFrozenCausalOriginCarrier,
       AsyncCandidateCausalOriginTyped

THEOREM InstallProposalSuccessorRetainsExactEvidenceAndCausalOrigin ==
  \A command:
    LET successor == InstallProposalSuccessor(command)
    IN /\ successor.kind = "AssembleBody"
       /\ successor.node = command.node
       /\ successor.view = command.view + 1
       /\ successor.consumerContext = context
       /\ successor.consumerView = command.view + 1
       /\ successor.evidence = command.evidence
       /\ successor.causalOrigin = command.causalOrigin
BY DEF InstallProposalSuccessor,
       AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
       AsyncCandidateSuccessorProposalRound,
       AsyncCandidateWithIdentityAndOrigin

THEOREM InstallProposalSuccessorRetainsExactPrepareAuthority ==
  \A command, prepareQc, leaderView:
    RetainedLockCandidateCarriesPrepareAuthority(
      command, prepareQc, leaderView)
      => RetainedLockCandidateCarriesPrepareAuthority(
           InstallProposalSuccessor(command), prepareQc, leaderView)
BY InstallProposalSuccessorRetainsExactEvidenceAndCausalOrigin
   DEF RetainedLockCandidateCarriesPrepareAuthority

THEOREM RetainedLockProposalSuccessorsRetainFrozenCausalOrigin ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      \A successor \in SequenceSet(CommandSuccessors(candidate)):
        RetainedLockExactCandidateRank(
          target, leader, lockedRound, subject, prepareQc, leaderView,
          causalOrigin, candidate, rank)
          => successor.causalOrigin = causalOrigin
BY CommandSuccessorsRetainCausalOrigin, Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity

RetainedLockProposalProducerRankForKind(kind) ==
  CASE kind = "AssembleBody" -> ProposalSemanticRank(9)
    [] kind = "BeginProposal" -> ProposalSemanticRank(8)
    [] kind = "PersistProposal" -> ProposalSemanticRank(7)
    [] OTHER -> ProposalSemanticRank(6)

THEOREM RetainedLockRankedProducerHasKindExactRank ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      RetainedLockExactCandidateRank(
        target, leader, lockedRound, subject, prepareQc, leaderView,
        causalOrigin, candidate, rank)
        => rank = RetainedLockProposalProducerRankForKind(candidate.kind)
BY Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockLeaderProducerKinds,
       RetainedLockProposalProducerRankForKind,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       ExactLeaderProposalRank, ExactLeaderViewChangeRank,
       ExactLeaderPrepareRank, ExactLeaderPrepareStaticRank,
       ExactLeaderPrepareSignRank, ExactLeaderCommitRank,
       ExactLeaderCommitStaticRank, ExactLeaderCommitSignRank,
       ExactLeaderDecisionRank

THEOREM RetainedLockProposalPrefixNextRankIsStrictlyLower ==
  \A producerKind \in LockedBodyProposalPrefixKinds:
    RetainedLockProposalProducerRankForKind(
      LockedBodyNextProposalProducerKind(producerKind))
      \in SetLessThan(
           RetainedLockProposalProducerRankForKind(producerKind),
           ExactLeaderSemanticRankOrdering,
           ExactLeaderSemanticRankCarrier)
BY Isa
   DEF LockedBodyProposalPrefixKinds,
       LockedBodyNextProposalProducerKind,
       RetainedLockProposalProducerRankForKind,
       ProposalSemanticRank, SetLessThan,
       ExactLeaderSemanticRankOrdering,
       ExactLeaderSemanticRankCarrier,
       LexPairOrdering, OpToRel

THEOREM RetainedLockSuccessfulProducerPrefixSchedulesExactSuccessor ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ candidate.kind \in LockedBodyProposalPrefixKinds
      /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
      => CandidateScheduled(
           LockedBodyProposalCausalSuccessor(candidate))'
BY FifoSuccessfulExecutionSchedulesEverySuccessor,
   DeferredSuccessfulExecutionSchedulesEverySuccessor,
   LockedBodyProposalPrefixDeclaresExactSameOriginSuccessor,
   IsaT(600)
   DEF AsyncCandidateSuccessfullyServicedThisStep,
       CommandSuccessorsScheduledAfter

THEOREM RetainedLockSuccessfulProducerPrefixPreservesExactAuthority ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ candidate.kind \in LockedBodyProposalPrefixKinds
      /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
      => LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject,
           prepareQc, leaderView)'
BY ExecuteLockedBodyProposalPrefixPreservesResponsiveLeaderAuthority,
   Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       AsyncCandidateSuccessfullyServicedThisStep,
       FifoRuntimeStep, DeferredDrainStep

THEOREM RetainedLockSuccessfulProducerPrefixReachesStrictLowerRank ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncNext
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ candidate.kind \in LockedBodyProposalPrefixKinds
      /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
      => \E lowerRank \in
             SetLessThan(
               rank,
               ExactLeaderSemanticRankOrdering,
               ExactLeaderSemanticRankCarrier):
           RetainedLockCandidateRankFrontier(
             target, leader, lockedRound, subject, prepareQc, leaderView,
             causalOrigin, lowerRank)'
BY RetainedLockRankedProducerHasKindExactRank,
   RetainedLockProposalPrefixNextRankIsStrictlyLower,
   RetainedLockSuccessfulProducerPrefixSchedulesExactSuccessor,
   RetainedLockSuccessfulProducerPrefixPreservesExactAuthority,
   AsyncBracketNextPreservesStrongTypeInvariant, IsaT(1200)
   DEF RetainedLockCandidateRankFrontier,
       RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       RetainedLockModeActive, RetainedLockModeGoal,
       LockedBodyProposalCausalSuccessor,
       LockedBodyNextProposalProducerKind,
       RetainedLockProposalProducerRankForKind,
       ExactLeaderCandidateRank, ExactLeaderPhaseRank,
       ExactLeaderProposalRank, CandidateConsumerCurrent,
       ResponsiveProtectedCandidateOwned,
       CommandSuccessorsScheduledAfter,
       AsyncCandidateSuccessfullyServicedThisStep,
       FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       AssembleLocalBody, BeginLocalProposal, PersistProposal,
       SafeProposalSignIntentAt, ProposalSignIntentsAt,
       DurableProposalSafeForLock,
       CausalCandidate, AsyncCandidateFrom,
       AsyncCandidateWithIdentityAndOrigin

THEOREM RetainedLockExactSignProposalExecutionReachesGoal ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ TypeInvariant
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ candidate.kind = "SignProposal"
      /\ ExecuteSignProposal(candidate)
      => RetainedLockModeGoal(target, lockedRound, subject)'
BY ExecuteRetainedLockSignProposalBroadcastsExactLockedSubject, Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       LockedBodyResponsiveLeaderAuthority,
       RetainedLockModeGoal

THEOREM RetainedLockSuccessfulSignProposalServiceReachesGoal ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ candidate.kind = "SignProposal"
      /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
      => RetainedLockModeGoal(target, lockedRound, subject)'
BY RetainedLockExactSignProposalExecutionReachesGoal, Isa
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant,
       AsyncCandidateSuccessfullyServicedThisStep,
       FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand

THEOREM RetainedLockSuccessfulRankedProducerServiceMakesStrictProgress ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncNext
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ AsyncCandidateSuccessfullyServicedThisStep(candidate)
      => \/ RetainedLockModeGoal(target, lockedRound, subject)'
         \/ \E lowerRank \in
                SetLessThan(
                  rank,
                  ExactLeaderSemanticRankOrdering,
                  ExactLeaderSemanticRankCarrier):
              RetainedLockCandidateRankFrontier(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, causalOrigin, lowerRank)'
BY RetainedLockSuccessfulProducerPrefixReachesStrictLowerRank,
   RetainedLockSuccessfulSignProposalServiceReachesGoal, Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockLeaderProducerKinds,
       LockedBodyProposalPrefixKinds

(***************************************************************************
The scheduler-exit half of exact producer service is already closed by the
proved protected-candidate starvation theorem.  This statement freezes the
complete target/leader/QC/origin coordinates; it deliberately concludes only
that the particular physical candidate owner leaves.  It does not infer a
causal successor from disappearance.  Its dependency is acyclic:
`StarvationFreedomObligation` is defined in `SumeragiV2AsyncDeadlockProofs`
solely from protected service-rank progress and the generic starvation
lifting theorem, below both retained-lock and rotating-leader progress.
***************************************************************************)

RetainedLockRankedCandidateExitProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          candidate \in AsyncCandidateSet,
          rank \in ExactLeaderSemanticRankCarrier:
         (gst
           /\ RetainedLockModeActive(target, lockedRound, subject)
           /\ RetainedLockExactCandidateRank(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, causalOrigin, candidate, rank))
           ~> ~ResponsiveProtectedCandidateOwned(candidate)

THEOREM AsyncLiveClosesRetainedLockRankedCandidateExit ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
      => RetainedLockRankedCandidateExitProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ProtectedServiceFiniteRunnerEpisodeClosureProperty(
                  AsyncSpecAt(initialContext))
         PROVE RetainedLockRankedCandidateExitProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. StarvationFreedomProperty(
             AsyncLiveSpecAt(initialContext))
      BY StarvationFreedomObligation
    <2>2. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A target, leader \in ValidatorIds,
                     lockedRound \in Views,
                     subject \in Subjects,
                     prepareQc \in QcRecordSet,
                     leaderView \in Views,
                     causalOrigin \in AsyncCandidateCausalOriginSet,
                     candidate \in AsyncCandidateSet,
                     rank \in ExactLeaderSemanticRankCarrier:
              (gst
                /\ RetainedLockModeActive(
                     target, lockedRound, subject)
                /\ RetainedLockExactCandidateRank(
                     target, leader, lockedRound, subject,
                     prepareQc, leaderView,
                     causalOrigin, candidate, rank))
                ~> ~ResponsiveProtectedCandidateOwned(candidate)
      <3>1. ASSUME NEW target \in ValidatorIds,
                    NEW leader \in ValidatorIds,
                    NEW lockedRound \in Views,
                    NEW subject \in Subjects,
                    NEW prepareQc \in QcRecordSet,
                    NEW leaderView \in Views,
                    NEW causalOrigin \in AsyncCandidateCausalOriginSet,
                    NEW candidate \in AsyncCandidateSet,
                    NEW rank \in ExactLeaderSemanticRankCarrier
             PROVE (gst
                      /\ RetainedLockModeActive(
                           target, lockedRound, subject)
                      /\ RetainedLockExactCandidateRank(
                           target, leader, lockedRound, subject,
                           prepareQc, leaderView,
                           causalOrigin, candidate, rank))
                     ~> ~ResponsiveProtectedCandidateOwned(candidate)
        <4>1. (gst /\ ResponsiveProtectedCandidateOwned(candidate))
                 ~> ~ResponsiveProtectedCandidateOwned(candidate)
          BY <2>1, <2>2 DEF StarvationFreedomProperty
        <4>2. RetainedLockExactCandidateRank(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, causalOrigin, candidate, rank)
                => ResponsiveProtectedCandidateOwned(candidate)
          BY DEF RetainedLockExactCandidateRank,
                 ExactLeaderCandidateRank
        <4> QED BY <4>1, <4>2, PTL
      <3> QED BY <3>1
    <2> QED BY <2>2
         DEF RetainedLockRankedCandidateExitProperty
  <1> QED BY <1>1

THEOREM AsyncLiveProvidesRetainedLockRankedCandidateExit ==
  \A initialContext:
    RetainedLockRankedCandidateExitProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure,
   AsyncLiveClosesRetainedLockRankedCandidateExit

(***************************************************************************
Explicit temporal proof boundaries.

Timeout/view progress is consumed first, but strict view increase alone does
not imply that any fixed process lands on a self-leader view.  The properties
below therefore expose the exact missing boundaries without inventing
leader-slot fairness: source authority, authority-bearing TC transport,
fresh target-leader activation, leader producer scheduling, and rank handoff.
The fixed-origin handoff remains declared below only for source compatibility;
the release decomposition instead names owner-neutral descent, cross-origin
replacement, and exact re-entry.
***************************************************************************)

RetainedLockSourceAuthorityExposureProperty(specification) ==
  TimeoutViewProgressProperty(specification)
    => (specification
          => \A target \in ValidatorIds,
                lockedRound \in Views,
                subject \in Subjects:
               RetainedLockModeSource(target, lockedRound, subject)
                 ~> (RetainedLockModeGoal(
                       target, lockedRound, subject)
                      \/ RetainedLockSourceExposureFrontier(
                           target, lockedRound, subject)))

\* Direct-decomposition compatibility boundary (not release-facing): exact
\* PrepareQC identity is closed by
\* `RetainedLockSourceModeBindsExactPrepareAuthority`.  A consumer of this
\* stronger vocabulary must separately compose source-lock preservation,
\* old-round Commit convergence, and a fresh source service window.  This
\* boundary does not select a future leader.

RetainedLockPrepareAuthorityTransportProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          sourceView \in Views:
         RetainedLockFreshSourceAuthorityFrontier(
           target, lockedRound, subject, prepareQc, sourceView)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ RetainedLockAuthorityTransportFrontierFor(
                     target, lockedRound, subject, prepareQc))

\* Direct-decomposition compatibility boundary (not release-facing): a
\* consumer must supply transport showing that retransmitted exact Prepare
\* authority reaches a TC whose next-view leader is responsive.  The QC and
\* TC are complete identities; a same-rank replacement or a rank/subject
\* projection is not sufficient.

RetainedLockTargetLeaderFreshActivationProperty(specification) ==
  specification
    => \A target \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet:
         RetainedLockAuthorityTransportFrontierFor(
           target, lockedRound, subject, prepareQc)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ RetainedLockFreshLeaderAuthorityFrontierFor(
                     target, lockedRound, subject, prepareQc))

\* Direct-decomposition compatibility boundary (not release-facing): a
\* consumer must compose exact TC delivery, BeginInstall, PersistInstall,
\* timeout lifecycle retirement, and the new-view deadline reset.  A target
\* whose selected episode is overtaken must re-enter through another exact
\* target-indexed transport episode; view increase alone is not this
\* boundary.

RetainedLockLeaderProducerOriginProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views:
         LockedBodyFreshResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ \E causalOrigin \in AsyncCandidateCausalOriginSet:
                     RetainedLockRankedEpisodeFrontier(
                       target, leader, lockedRound, subject,
                       prepareQc, leaderView, causalOrigin))

\* Direct-decomposition compatibility boundary (not release-facing): a
\* consumer must compose exact PersistInstall/local/restart producer
\* activation with the Assemble/Begin/Persist/safe-Sign Proposal chain.
\* Causal successors retain the frozen origin and protected physical owners
\* eventually exit; the remaining boundary is the finite sibling/replacement
\* classification below.

RetainedLockRankHandoffProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          rank \in ExactLeaderSemanticRankCarrier:
         RetainedLockCandidateRankFrontier(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, rank)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ \E lowerRank \in
                       SetLessThan(
                         rank,
                         ExactLeaderSemanticRankOrdering,
                         ExactLeaderSemanticRankCarrier):
                     RetainedLockCandidateRankFrontier(
                       target, leader, lockedRound, subject,
                       prepareQc, leaderView, causalOrigin, lowerRank))

(***************************************************************************
State form of the scheduler's producer-lifecycle disposition.

The lower network module states this classification with primed helpers at
the exact departure transition.  These operators provide the corresponding
unprimed frontier so it can be used as a temporal endpoint.  A semantically
covered stage is deliberately not called lower-rank progress: another causal
origin may own the pending WAL/intent which created that coverage.
***************************************************************************)

RetainedLockProducerDurableReplayOriginOwned(candidate) ==
  candidate.causalOrigin
    \in
      {replay.causalOrigin:
         replay \in
           {replayCandidate \in
              SequenceSet(
                FreshRestartCandidateSequence(
                  RestartReplay(candidate.node))):
              replayCandidate.causalOrigin
                \notin
                  AsyncScheduledCandidateOriginsForNode(candidate.node)}}
        \cup
      {replay.causalOrigin:
         replay \in
           {replayCandidate \in
              SequenceSet(
                HistoricalLockedRetransmitSuccessors(candidate.node)):
              replayCandidate.causalOrigin
                \notin
                  AsyncScheduledCandidateOriginsForNode(candidate.node)}}

RetainedLockProducerSameOriginPhysicalOrDurableOwner(candidate) ==
  \/ candidate.causalOrigin
       \in AsyncScheduledCandidateOriginsForNode(candidate.node)
  \/ RetainedLockProducerDurableReplayOriginOwned(candidate)

RetainedLockProducerConsumerEpisodeObsolete(candidate) ==
  \/ candidate.consumerContext # context
  \/ candidate.height # height
  \/ candidate.consumerView < nodeView[candidate.node]
  \/ candidate.consumerGeneration # generation[candidate.node]
  \/ NodeHasDecision(candidate.node)

RetainedLockProducerBodyStageCovered(candidate) ==
  LET body ==
        BodyRecord(candidate.node, candidate.consumerContext,
                   candidate.view, candidate.subject)
  IN \/ body \in availableBodies
     \/ BodyHeldBy(durableBodies, candidate.node,
                    candidate.consumerContext,
                    candidate.view, candidate.subject)

RetainedLockProducerValidationStageCovered(candidate) ==
  \/ \E validation \in validatedBodies:
       /\ validation.node = candidate.node
       /\ validation.context = candidate.consumerContext
       /\ validation.view = candidate.view
       /\ validation.subject = candidate.subject
  \/ BodyRecord(candidate.node, candidate.consumerContext,
                candidate.view, candidate.subject)
       \in invalidBodies

RetainedLockProducerProposalStageCovered(candidate) ==
  \/ \E request \in pendingProposal:
       /\ request.node = candidate.node
       /\ request.proposal.context = candidate.consumerContext
       /\ request.proposal.view = candidate.view
       /\ request.proposal.subject = candidate.subject
  \/ \E proposal \in proposalIntents:
       /\ proposal.proposer = candidate.node
       /\ proposal.context = candidate.consumerContext
       /\ proposal.view = candidate.view
       /\ proposal.subject = candidate.subject

RetainedLockProducerMonotoneStageCovered(candidate) ==
  CASE candidate.kind = "AssembleBody" ->
         /\ RetainedLockProducerBodyStageCovered(candidate)
         /\ RetainedLockProducerValidationStageCovered(candidate)
    [] candidate.kind \in
         {"BeginProposal", "PersistProposal", "SignProposal"} ->
         RetainedLockProducerProposalStageCovered(candidate)
    [] OTHER -> FALSE

RetainedLockProducerLifecycleDisposition(candidate) ==
  \/ RetainedLockProducerSameOriginPhysicalOrDurableOwner(candidate)
  \/ RetainedLockProducerConsumerEpisodeObsolete(candidate)
  \/ RetainedLockProducerMonotoneStageCovered(candidate)
  \/ AsyncCandidateTerminalTombstoned(candidate)

RetainedLockFrozenProducerEpisodeIdentity(
    leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, candidate, rank) ==
  /\ RetainedLockFrozenProducerCandidateIdentity(
       candidate, leader, lockedRound, subject, prepareQc, leaderView,
       causalOrigin)
  /\ candidate.consumerView = leaderView
  /\ candidate.view = leaderView
  /\ candidate.kind \in RetainedLockLeaderProducerKinds
  /\ RetainedLockCandidateCarriesPrepareAuthority(
       candidate, prepareQc, leaderView)
  /\ rank = RetainedLockProposalProducerRankForKind(candidate.kind)

RetainedLockProducerEpisodeCoordinates(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, candidate, rank) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ RetainedLockFrozenProducerEpisodeIdentity(
       leader, lockedRound, subject, prepareQc, leaderView,
       causalOrigin, candidate, rank)

RetainedLockProducerCrossOriginReplacementFrontier(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    sourceOrigin, sourceRank) ==
  \E replacementOrigin \in AsyncCandidateCausalOriginSet,
     replacementRank \in ExactLeaderSemanticRankCarrier:
    /\ replacementOrigin # sourceOrigin
    /\ replacementRank
         \notin SetLessThan(
                  sourceRank,
                  ExactLeaderSemanticRankOrdering,
                  ExactLeaderSemanticRankCarrier)
    /\ RetainedLockCandidateRankFrontier(
         target, leader, lockedRound, subject, prepareQc, leaderView,
         replacementOrigin, replacementRank)

RetainedLockProducerExactReentryFrontier(
    target, leader, lockedRound, subject, prepareQc, leaderView) ==
  /\ RetainedLockModeActive(target, lockedRound, subject)
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ \/ ~LockedBodyResponsiveLeaderAuthority(
            target, leader, lockedRound, subject,
            prepareQc, leaderView)
     \/ HistoricalLockedBodyRecoveryStage(leader, prepareQc)

RetainedLockProducerNonDescentEpisodeResidual(
    target, leader, lockedRound, subject, prepareQc, leaderView,
    causalOrigin, candidate, rank) ==
  /\ RetainedLockProducerEpisodeCoordinates(
       target, leader, lockedRound, subject, prepareQc, leaderView,
       causalOrigin, candidate, rank)
  /\ \/ RetainedLockProducerLifecycleDisposition(candidate)
     \/ RetainedLockProducerCrossOriginReplacementFrontier(
          target, leader, lockedRound, subject, prepareQc, leaderView,
          causalOrigin, rank)
     \/ RetainedLockProducerExactReentryFrontier(
          target, leader, lockedRound, subject, prepareQc, leaderView)

THEOREM RetainedLockActionDispositionProjectsLifecycleFrontier ==
  \A candidate \in AsyncCandidateSet:
    /\ candidate.kind \in RetainedLockLeaderProducerKinds
    /\ LockedBodyProposalProducerDispositionAfter(candidate)
    => RetainedLockProducerLifecycleDisposition(candidate)'
BY IsaT(300)
   DEF LockedBodyProposalProducerDispositionAfter,
       RetainedLockProducerLifecycleDisposition,
       RetainedLockProducerSameOriginPhysicalOrDurableOwner,
       RetainedLockProducerDurableReplayOriginOwned,
       RetainedLockProducerConsumerEpisodeObsolete,
       RetainedLockProducerMonotoneStageCovered,
       RetainedLockProducerBodyStageCovered,
       RetainedLockProducerValidationStageCovered,
       RetainedLockProducerProposalStageCovered,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       AsyncCandidateSameOriginScheduledAfter,
       AsyncCandidateSameOriginDurableReplayAfter,
       AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateBodyStageCoveredAfter,
       AsyncCandidateValidationStageCoveredAfter,
       AsyncCandidateProposalStageCoveredAfter

THEOREM RetainedLockDepartingRankedProducerMakesProgressOrDisposition ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncNext
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ ~CandidateScheduledAfter(candidate)
      => \/ RetainedLockModeGoal(target, lockedRound, subject)'
         \/ \E lowerRank \in
                SetLessThan(
                  rank,
                  ExactLeaderSemanticRankOrdering,
                  ExactLeaderSemanticRankCarrier):
              RetainedLockCandidateRankFrontier(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, causalOrigin, lowerRank)'
         \/ RetainedLockProducerLifecycleDisposition(candidate)'
BY LockedBodyScheduledProposalProducerDepartureIsClassified,
   RetainedLockSuccessfulRankedProducerServiceMakesStrictProgress,
   RetainedLockActionDispositionProjectsLifecycleFrontier, Isa
   DEF RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       RetainedLockFrozenProducerCandidateIdentity,
       RetainedLockFrozenCandidateIdentity,
       ExactLeaderCandidateRank,
       ResponsiveProtectedCandidateOwned,
       LockedBodyProposalIntentCovered,
       SafeProposalSignIntentAt, ProposalSignIntentsAt

(***************************************************************************
The physical exit theorem is useful only after classifying the first state in
which the exact producer is no longer protected.  If the candidate remains
scheduled, loss of protected ownership is a genuine terminal locked-body
outcome (the fixed responsive owner and height cannot silently disappear after
GST).  Otherwise the proved departure classification supplies either strict
same-origin rank descent or the explicit lifecycle disposition.  In the latter
case the frozen target/leader/QC/origin coordinates survive unless the target
has already reached the terminal outcome, so the state is exactly the
non-descent residual below.  This theorem does not call replenishment progress.
***************************************************************************)

THEOREM RetainedLockRankedProducerPhysicalExitReachesSameOriginEndpoint ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, causalOrigin, candidate, rank:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ AsyncNext
      /\ gst
      /\ RetainedLockModeActive(target, lockedRound, subject)
      /\ RetainedLockExactCandidateRank(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
      /\ ~ResponsiveProtectedCandidateOwned(candidate)'
      => \/ RetainedLockModeGoal(target, lockedRound, subject)'
         \/ \E lowerRank \in
                SetLessThan(
                  rank,
                  ExactLeaderSemanticRankOrdering,
                  ExactLeaderSemanticRankCarrier):
              RetainedLockCandidateRankFrontier(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, causalOrigin, lowerRank)'
         \/ RetainedLockProducerNonDescentEpisodeResidual(
              target, leader, lockedRound, subject, prepareQc, leaderView,
              causalOrigin, candidate, rank)'
BY RetainedLockDepartingRankedProducerMakesProgressOrDisposition,
   IsaT(900)
   DEF ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       RetainedLockProducerNonDescentEpisodeResidual,
       RetainedLockProducerEpisodeCoordinates,
       RetainedLockFrozenProducerEpisodeIdentity,
       RetainedLockModeActive, RetainedLockModeGoal,
       LockedBodyReproposalOutcome,
       LockedBodyLegitimatelyDecidedOrSuperseded,
       LockedBodySourcePrepareAuthority,
       RetainedLockExactCandidateRank,
       RetainedLockLeaderProducerCandidateRank,
       CandidateScheduledAfter

(***************************************************************************
Minimal non-descent episode residual.

Physical candidate exit is proved above.  What remains is the semantic
classification of that exit for one fixed producer occurrence.  The residual
is pointwise in the candidate, not an aggregate leader-service premise:

  * a successful Assemble/Begin/Persist execution must expose the exact
    same-origin successor;
  * SignProposal is already closed by
    `RetainedLockExactSignProposalExecutionReachesGoal`;
  * an equal/higher sibling or replacement is a finite non-descent episode,
    not rank progress; and
  * consumer retirement or a leader-local Decision must retain an exact
    target transport/re-entry owner before the producer rank disappears.

The enclosing temporal property requires the proved scheduler exit, while the
frozen-coordinate conjunct prevents any provider from being applied to an
unrelated candidate, origin, or rank.  No Decision/application aggregate or
rotating-leader theorem is available on this dependency path.
***************************************************************************)

RetainedLockSameOriginProducerNonDescentClosureProperty(specification) ==
  RetainedLockRankedCandidateExitProperty(specification)
    => (specification
          => \A target, leader \in ValidatorIds,
                lockedRound \in Views,
                subject \in Subjects,
                prepareQc \in QcRecordSet,
                leaderView \in Views,
                causalOrigin \in AsyncCandidateCausalOriginSet,
                candidate \in AsyncCandidateSet,
                rank \in ExactLeaderSemanticRankCarrier:
               (gst
                 /\ RetainedLockModeActive(
                      target, lockedRound, subject)
                 /\ RetainedLockExactCandidateRank(
                      target, leader, lockedRound, subject,
                      prepareQc, leaderView,
                      causalOrigin, candidate, rank))
                 ~> (RetainedLockModeGoal(
                       target, lockedRound, subject)
                      \/ \E lowerRank \in
                           SetLessThan(
                             rank,
                             ExactLeaderSemanticRankOrdering,
                             ExactLeaderSemanticRankCarrier):
                           RetainedLockCandidateRankFrontier(
                             target, leader, lockedRound, subject,
                             prepareQc, leaderView,
                             causalOrigin, lowerRank)
                      \/ RetainedLockProducerNonDescentEpisodeResidual(
                           target, leader, lockedRound, subject,
                           prepareQc, leaderView,
                           causalOrigin, candidate, rank)))

THEOREM AsyncLiveProvidesRetainedLockSameOriginProducerNonDescentClosure ==
  \A initialContext:
    RetainedLockSameOriginProducerNonDescentClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesRetainedLockRankedCandidateExit,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   RetainedLockAsyncSpecAlwaysCandidateServiceLifecycle,
   AsyncLiveSpecProjectsAsyncSpec,
   RetainedLockRankedProducerPhysicalExitReachesSameOriginEndpoint,
   PTL
   DEF RetainedLockSameOriginProducerNonDescentClosureProperty,
       RetainedLockRankedCandidateExitProperty

RetainedLockProducerNonDescentEpisodeClosureProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          candidate \in AsyncCandidateSet,
          rank \in ExactLeaderSemanticRankCarrier:
         RetainedLockProducerNonDescentEpisodeResidual(
           target, leader, lockedRound, subject, prepareQc, leaderView,
           causalOrigin, candidate, rank)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ \E lowerRank \in
                       SetLessThan(
                         rank,
                         ExactLeaderSemanticRankOrdering,
                         ExactLeaderSemanticRankCarrier):
                     RetainedLockOwnerNeutralCandidateRankFrontier(
                       target, leader, lockedRound, subject,
                       prepareQc, leaderView, lowerRank)
                \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
                     target, lockedRound, subject,
                     prepareQc, leaderView))

RetainedLockProducerEpisodeExitGoal(
    target, leader, lockedRound, subject, prepareQc, leaderView, rank) ==
  \/ RetainedLockModeGoal(target, lockedRound, subject)
  \/ \E lowerRank \in
       SetLessThan(
         rank,
         ExactLeaderSemanticRankOrdering,
         ExactLeaderSemanticRankCarrier):
       RetainedLockOwnerNeutralCandidateRankFrontier(
         target, leader, lockedRound, subject,
         prepareQc, leaderView, lowerRank)
  \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
       target, lockedRound, subject, prepareQc, leaderView)

\* Direct-decomposition compatibility boundaries (not release-facing): any
\* consumer of the stronger owner-neutral handoff must supply these three
\* exact providers after successful-service identity persists through strict
\* exit.  They intentionally separate same-origin durable disposition,
\* cross-origin replacement, and target-indexed re-entry; none may be inferred
\* merely from physical candidate disappearance.

RetainedLockSameOriginLifecycleDispositionClosureProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          candidate \in AsyncCandidateSet,
          rank \in ExactLeaderSemanticRankCarrier:
         (/\ RetainedLockProducerEpisodeCoordinates(
               target, leader, lockedRound, subject,
               prepareQc, leaderView, causalOrigin, candidate, rank)
          /\ RetainedLockProducerLifecycleDisposition(candidate))
           ~> RetainedLockProducerEpisodeExitGoal(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, rank)

RetainedLockCrossOriginProducerReplacementClosureProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          candidate \in AsyncCandidateSet,
          rank \in ExactLeaderSemanticRankCarrier:
         (/\ RetainedLockProducerEpisodeCoordinates(
               target, leader, lockedRound, subject,
               prepareQc, leaderView, causalOrigin, candidate, rank)
          /\ RetainedLockProducerCrossOriginReplacementFrontier(
               target, leader, lockedRound, subject,
               prepareQc, leaderView, causalOrigin, rank))
           ~> RetainedLockProducerEpisodeExitGoal(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, rank)

RetainedLockProducerExactReentryClosureProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          causalOrigin \in AsyncCandidateCausalOriginSet,
          candidate \in AsyncCandidateSet,
          rank \in ExactLeaderSemanticRankCarrier:
         (/\ RetainedLockProducerEpisodeCoordinates(
               target, leader, lockedRound, subject,
               prepareQc, leaderView, causalOrigin, candidate, rank)
          /\ RetainedLockProducerExactReentryFrontier(
               target, leader, lockedRound, subject,
               prepareQc, leaderView))
           ~> RetainedLockProducerEpisodeExitGoal(
                target, leader, lockedRound, subject,
                prepareQc, leaderView, rank)

THEOREM RetainedLockSeparatedProducerProvidersCloseEpisodeResidual ==
  \A initialContext:
    /\ RetainedLockSameOriginLifecycleDispositionClosureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockCrossOriginProducerReplacementClosureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockProducerExactReentryClosureProperty(
         AsyncLiveSpecAt(initialContext))
      => RetainedLockProducerNonDescentEpisodeClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY PTL
   DEF RetainedLockSameOriginLifecycleDispositionClosureProperty,
       RetainedLockCrossOriginProducerReplacementClosureProperty,
       RetainedLockProducerExactReentryClosureProperty,
       RetainedLockProducerNonDescentEpisodeClosureProperty,
       RetainedLockProducerNonDescentEpisodeResidual,
       RetainedLockProducerEpisodeExitGoal

RetainedLockOwnerNeutralRankHandoffProperty(specification) ==
  specification
    => \A target, leader \in ValidatorIds,
          lockedRound \in Views,
          subject \in Subjects,
          prepareQc \in QcRecordSet,
          leaderView \in Views,
          rank \in ExactLeaderSemanticRankCarrier:
         RetainedLockOwnerNeutralCandidateRankFrontier(
           target, leader, lockedRound, subject,
           prepareQc, leaderView, rank)
           ~> (RetainedLockModeGoal(target, lockedRound, subject)
                \/ \E lowerRank \in
                       SetLessThan(
                         rank,
                         ExactLeaderSemanticRankOrdering,
                         ExactLeaderSemanticRankCarrier):
                     RetainedLockOwnerNeutralCandidateRankFrontier(
                       target, leader, lockedRound, subject,
                       prepareQc, leaderView, lowerRank)
                \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
                     target, lockedRound, subject,
                     prepareQc, leaderView))

THEOREM RetainedLockNonDescentClosureClosesRankHandoff ==
  \A initialContext:
    RetainedLockProducerNonDescentEpisodeClosureProperty(
         AsyncLiveSpecAt(initialContext))
      => RetainedLockOwnerNeutralRankHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesRetainedLockSameOriginProducerNonDescentClosure, PTL
   DEF RetainedLockSameOriginProducerNonDescentClosureProperty,
       RetainedLockProducerNonDescentEpisodeClosureProperty,
       RetainedLockRankedCandidateExitProperty,
       RetainedLockOwnerNeutralRankHandoffProperty,
       RetainedLockOwnerNeutralCandidateRankFrontier,
       RetainedLockCandidateRankFrontier

THEOREM RetainedLockSeparatedProducerProvidersCloseOwnerNeutralRankHandoff ==
  \A initialContext:
    RetainedLockProducerNonDescentEpisodeClosureProperty(
      AsyncLiveSpecAt(initialContext))
      => RetainedLockOwnerNeutralRankHandoffProperty(
           AsyncLiveSpecAt(initialContext))
BY RetainedLockNonDescentClosureClosesRankHandoff

THEOREM RetainedLockSemanticRankOrderingWellFounded ==
  IsWellFoundedOn(
    ExactLeaderSemanticRankOrdering,
    ExactLeaderSemanticRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset,
   WFLexPairOrdering, SMT
   DEF ExactLeaderSemanticRankOrdering,
       ExactLeaderSemanticRankCarrier

THEOREM RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor ==
  \A initialContext:
    RetainedLockOwnerNeutralRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target, leader \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects,
                  prepareQc \in QcRecordSet,
                  leaderView \in Views,
                  rank \in ExactLeaderSemanticRankCarrier:
                 RetainedLockOwnerNeutralCandidateRankFrontier(
                   target, leader, lockedRound, subject,
                   prepareQc, leaderView, rank)
                   ~> (RetainedLockModeGoal(
                         target, lockedRound, subject)
                        \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor(
                             target, lockedRound, subject,
                             prepareQc, leaderView)))
BY RetainedLockSemanticRankOrderingWellFounded,
   WellFoundedLeadsTo
   DEF RetainedLockOwnerNeutralRankHandoffProperty

(***************************************************************************
Deductive rank closure.

The finite lexicographic carrier closes either the legacy supplied exact-origin
handoff or the owner-neutral handoff assembled from the separated producer
providers.  Only the owner-neutral path is release-facing.  A strict higher
view remains an explicit handoff: production uses ViewDomain = Nat, so this
leaf must not turn a configured/TLC maximum view into a well-founded liveness
rank.  The higher release closure composes that handoff with independently
proved rotating-leader convergence.
***************************************************************************)

THEOREM RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier ==
  \A initialContext:
    RetainedLockOwnerNeutralRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects:
                 RetainedLockRankedFrontier(
                   target, lockedRound, subject)
                   ~> (RetainedLockModeGoal(
                         target, lockedRound, subject)
                        \/ RetainedLockStrictHigherFreshLeaderAuthorityFrontier(
                             target, lockedRound, subject)))
BY RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor, PTL
   DEF RetainedLockRankedFrontier,
       RetainedLockRankedEpisodeFrontier,
       RetainedLockOwnerNeutralCandidateRankFrontier,
       RetainedLockStrictHigherFreshLeaderAuthorityFrontier

THEOREM RetainedLockRankHandoffClosesExactOrigin ==
  \A initialContext:
    RetainedLockRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target, leader \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects,
                  prepareQc \in QcRecordSet,
                  leaderView \in Views,
                  causalOrigin \in AsyncCandidateCausalOriginSet,
                  rank \in ExactLeaderSemanticRankCarrier:
                 RetainedLockCandidateRankFrontier(
                   target, leader, lockedRound, subject,
                   prepareQc, leaderView, causalOrigin, rank)
                   ~> RetainedLockModeGoal(
                        target, lockedRound, subject))
BY RetainedLockSemanticRankOrderingWellFounded,
   WellFoundedLeadsTo
   DEF RetainedLockRankHandoffProperty

THEOREM RetainedLockRankHandoffClosesRankedFrontier ==
  \A initialContext:
    RetainedLockRankHandoffProperty(
      AsyncLiveSpecAt(initialContext))
      => (AsyncLiveSpecAt(initialContext)
            => \A target \in ValidatorIds,
                  lockedRound \in Views,
                  subject \in Subjects:
                 RetainedLockRankedFrontier(
                   target, lockedRound, subject)
                   ~> RetainedLockModeGoal(
                        target, lockedRound, subject))
BY RetainedLockRankHandoffClosesExactOrigin, PTL
   DEF RetainedLockRankedFrontier,
       RetainedLockRankedEpisodeFrontier

THEOREM DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader ==
  \A initialContext:
    /\ TimeoutViewProgressProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockSourceAuthorityExposureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockPrepareAuthorityTransportProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockTargetLeaderFreshActivationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockLeaderProducerOriginProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockOwnerNeutralRankHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    => RetainedLockOutcomeOrHigherLeaderProgressProperty(
         AsyncLiveSpecAt(initialContext))
BY RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier, PTL
   DEF RetainedLockSourceAuthorityExposureProperty,
       RetainedLockPrepareAuthorityTransportProperty,
       RetainedLockTargetLeaderFreshActivationProperty,
       RetainedLockLeaderProducerOriginProperty,
       RetainedLockSourceExposureFrontier,
       RetainedLockFreshLeaderAuthorityFrontierFor,
       RetainedLockRankedFrontier,
       RetainedLockRankedEpisodeFrontier,
       RetainedLockModeSource,
       RetainedLockModeGoal,
       RetainedLockOutcomeOrHigherLeaderProgressProperty,
       RetainedLockStrictHigherFreshLeaderAuthorityFrontier

THEOREM DirectRetainedLockOwnerNeutralDecompositionReachesHigherClosure ==
  \A initialContext:
    /\ TimeoutViewProgressProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockSourceAuthorityExposureProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockPrepareAuthorityTransportProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockTargetLeaderFreshActivationProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockLeaderProducerOriginProperty(
         AsyncLiveSpecAt(initialContext))
    /\ RetainedLockOwnerNeutralRankHandoffProperty(
         AsyncLiveSpecAt(initialContext))
    => RetainedLockOutcomeOrHigherLeaderProgressProperty(
         AsyncLiveSpecAt(initialContext))
BY DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader

=============================================================================
