---- MODULE SumeragiV2LockedBodyProposalActionProofs ----
EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs

(***************************************************************************
Minimal locked-body proposal corridor frames used by the action proof.
The retained-lock and legitimate terminal predicates are defined by the
lower liveness vocabulary; this leaf imports only the proof-bearing shard
prefix below the outstanding temporal debt.  No scheduler or temporal
assumptions are introduced here.
***************************************************************************)

(***************************************************************************
Target-indexed retained-lock authority.

`target` is the responsive validator whose durable lock creates the liveness
obligation.  It is not required to be the leader which eventually proposes.
The complete PrepareQC is retained as the authority identity; rank and subject
alone are not enough to authorize certified-body recovery.  A later leader
receives that exact authority through a TC for its own predecessor view and
then becomes the ordinary `HistoricalLockedPrepareSource` which owns the
certified request/Serve/response pipeline.

There is intentionally no equality between `nodeView[target]` and
`leaderView`.  Delayed TC delivery may advance either process past several
views, so synchronization is indexed by the target leader episode and exact
certificate identity, not by a coincident source clock.
***************************************************************************)

LockedBodySourcePrepareAuthority(
    target, lockedRound, subject, prepareQc) ==
  /\ StableAvailableRetainedLock(target, lockedRound, subject)
  /\ prepareQc \in prepareQCs
  /\ prepareQc = lockPrepareQc[target]
  /\ prepareQc.context = context
  /\ prepareQc.height = context.height
  /\ prepareQc.phase = "Prepare"
  /\ prepareQc.view = lockedRound
  /\ prepareQc.subject = subject

LockedBodyExactAuthorityTC(
    tc, lockedRound, subject, prepareQc, leaderView) ==
  /\ tc \in TcRecordSet
  /\ leaderView \in Views
  /\ leaderView > lockedRound
  /\ tc.context = context
  /\ tc.height = context.height
  /\ tc.view + 1 = leaderView
  /\ tc.highestPrepareQc = prepareQc
  /\ prepareQc.context = context
  /\ prepareQc.height = context.height
  /\ prepareQc.phase = "Prepare"
  /\ prepareQc.view = lockedRound
  /\ prepareQc.subject = subject
  /\ TCValid(tc)

LockedBodyExactAuthorityTcTargetCorridor(
    target, leader, lockedRound, subject, prepareQc, leaderView, tc) ==
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ leader \in AsyncCurrentResponsiveVoters \cap up
  /\ Leader(context, leaderView) = leader
  /\ LockedBodyExactAuthorityTC(
       tc, lockedRound, subject, prepareQc, leaderView)
  /\ \E carrier \in AsyncCurrentResponsiveVoters:
       \/ TimeoutCertificateDelivery(carrier, leader, tc)
       \/ TimeoutCertificateInstallOwner(leader, tc)

LockedBodyResponsiveLeaderAuthority(
    target, leader, lockedRound, subject, prepareQc, leaderView) ==
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ leader \in AsyncCurrentResponsiveVoters \cap up
  /\ leaderView \in Views
  /\ leaderView > lockedRound
  /\ nodeView[leader] = leaderView
  /\ Leader(context, leaderView) = leader
  /\ highestPrepareQc[leader] = prepareQc
  /\ highestRank[leader] = lockedRound
  /\ highestSubject[leader] = subject
  /\ HistoricalLockedPrepareSource(leader, prepareQc)
  /\ \E installed \in installedTCs:
       /\ installed.node = leader
       /\ installed.tc = lastInstalledTc[leader]
       /\ installed.tc.view + 1 = leaderView
       /\ installed.tc.highestPrepareQc = prepareQc

LockedBodyFreshSourceAuthority(
    target, lockedRound, subject, prepareQc, sourceView) ==
  /\ LockedBodySourcePrepareAuthority(
       target, lockedRound, subject, prepareQc)
  /\ sourceView = nodeView[target]
  /\ AsyncViewTimeout(sourceView) > AsyncWorstCaseServiceBudget
  /\ AsyncFreshNodeServiceWindow(target, context, sourceView)

LockedBodyFreshResponsiveLeaderAuthority(
    target, leader, lockedRound, subject, prepareQc, leaderView) ==
  /\ LockedBodyResponsiveLeaderAuthority(
       target, leader, lockedRound, subject, prepareQc, leaderView)
  /\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget
  /\ AsyncFreshNodeServiceWindow(leader, context, leaderView)

(***************************************************************************
The durable lock does not merely determine a rank/subject projection.  Under
the already-proved Core invariant it names one concrete PrepareQC, including
the original signer evidence.  This removes certificate selection from the
source-exposure temporal seam: only reaching a fresh service clock remains.
***************************************************************************)

THEOREM StableRetainedLockBindsExactSourcePrepareAuthority ==
  \A target \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects:
    /\ StrongInductiveInvariant
    /\ StableAvailableRetainedLock(target, lockedRound, subject)
    => LockedBodySourcePrepareAuthority(
         target, lockedRound, subject, lockPrepareQc[target])
BY IsaT(180)
   DEF LockedBodySourcePrepareAuthority,
       StableAvailableRetainedLock,
       StrongInductiveInvariant, ReducerProvenanceInvariant,
       HighestAndLockAreCertified, CertificatesBackedByIntents,
       HistoricalQcValid, PrepareQcRank, PrepareQcSubject,
       TypeInvariant, ModelConfiguration, NoRank

THEOREM LockedBodyExactAuthorityTcBindsTargetLeaderView ==
  \A lockedRound \in Views, subject \in Subjects,
     leaderView \in Views:
    \A tc, prepareQc:
      LockedBodyExactAuthorityTC(
        tc, lockedRound, subject, prepareQc, leaderView)
        => /\ tc.view = leaderView - 1
           /\ tc.highestPrepareQc = prepareQc
           /\ prepareQc.view = lockedRound
           /\ prepareQc.subject = subject
BY SMT DEF LockedBodyExactAuthorityTC

THEOREM LockedBodyFreshAuthorityWindowsExcludeOlderTimeoutOwners ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, sourceView, leaderView \in Views:
    \A prepareQc:
      /\ LockedBodyFreshSourceAuthority(
           target, lockedRound, subject, prepareQc, sourceView)
      /\ LockedBodyFreshResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      => /\ ~AsyncOlderOrEqualTimeoutLifecycleOwned(
                target, context, sourceView)
         /\ ~AsyncOlderOrEqualTimeoutLifecycleOwned(
                leader, context, leaderView)
         /\ nodeView[target] = sourceView
         /\ nodeView[leader] = leaderView
BY DEF LockedBodyFreshSourceAuthority,
       LockedBodyFreshResponsiveLeaderAuthority,
       AsyncFreshNodeServiceWindow

THEOREM DeliverLockedBodyExactAuthorityTcHandsOffToInstallOwner ==
  \A leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, tc:
      /\ LockedBodyExactAuthorityTC(
           tc, lockedRound, subject, prepareQc, leaderView)
      /\ DeliverTC(TcEnvelope(leader, tc))
      => \/ TimeoutCertificateInstallOwner(leader, tc)'
         \/ NodeHasDecision(leader)'
BY Isa
   DEF LockedBodyExactAuthorityTC,
       TimeoutCertificateInstallOwner,
       DeliverTC, TcAt, NodeHasDecision, NoDecisionForNode

THEOREM BeginLockedBodyExactAuthorityTcHandsOffToInstallWal ==
  \A leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, tc:
      /\ LockedBodyExactAuthorityTC(
           tc, lockedRound, subject, prepareQc, leaderView)
      /\ BeginInstallTC(leader, tc)
      => TimeoutCertificateInstallOwner(leader, tc)'
BY Isa
   DEF LockedBodyExactAuthorityTC,
       TimeoutCertificateInstallOwner,
       BeginInstallTC, InstallTcWal

(***************************************************************************
This action fact covers the distinct-target handoff.  The local fast path
`target = leader` is covered by the original proposal-attempt lemmas below.
The strict pre-install inequalities select the case in which this exact QC,
rather than an older local high, becomes both durable Prepare frontiers.
Fresh clock ownership is deliberately not claimed here: the asynchronous
wrapper must first retire the executing timeout lifecycle and reset the new
view deadline.
***************************************************************************)

THEOREM PersistExactAuthorityTcEstablishesResponsiveLeaderAuthority ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, tc:
      \A request \in InstallTcWalSet:
        /\ target # leader
        /\ LockedBodySourcePrepareAuthority(
             target, lockedRound, subject, prepareQc)
        /\ leader \in AsyncCurrentResponsiveVoters \cap up
        /\ Leader(context, leaderView) = leader
        /\ LockedBodyExactAuthorityTC(
             tc, lockedRound, subject, prepareQc, leaderView)
        /\ request.node = leader
        /\ request.tc = tc
        /\ tc.view >= nodeView[leader]
        /\ lockedRound > lockRank[leader]
        /\ lockedRound > highestRank[leader]
        /\ NoDecisionForNode(leader)
        /\ PersistInstallTC(request)
        => LockedBodyResponsiveLeaderAuthority(
             target, leader, lockedRound, subject,
             prepareQc, leaderView)'
BY IsaT(240)
   DEF LockedBodySourcePrepareAuthority,
       LockedBodyExactAuthorityTC,
       LockedBodyResponsiveLeaderAuthority,
       StableAvailableRetainedLock,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor,
       AsyncProposalSubject,
       PersistInstallTC, StrictSameRoundTcUpgrade,
       TcHighRank, TcHighSubject,
       PrepareQcRank, PrepareQcSubject,
       NoDecisionForNode, RetainedLockedBodyHeldBy,
       BodyHeldBy, CurrentVoters, CurrentEpoch,
       AsyncCurrentResponsiveVoters

THEOREM LockedBodyResponsiveLeaderAuthorityProjectsRecoveryStage ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc:
      /\ HistoricalLockedBodyRecoveryStageInvariant
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      => HistoricalLockedBodyRecoveryStage(leader, prepareQc)
BY Isa
   DEF LockedBodyResponsiveLeaderAuthority,
       HistoricalLockedBodyRecoveryStageInvariant,
       AsyncCurrentResponsiveVoters

THEOREM LockedBodyResponsiveLeaderRecoveryStageStep ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc:
      /\ AsyncStrongTypeInvariant
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      /\ HistoricalLockedBodyRecoveryStage(leader, prepareQc)
      /\ [AsyncNext]_AsyncAllVars
      => \/ ~HistoricalLockedPrepareSource(leader, prepareQc)'
         \/ HistoricalLockedBodyRecoveryStage(leader, prepareQc)'
BY HistoricalLockedBodyExistingSourceStepPreservation, Isa
   DEF LockedBodyResponsiveLeaderAuthority,
       HistoricalLockedBodySourceRetired

(***************************************************************************
The terminal producer action is exact and target-neutral: once the frozen
later-view SignProposal command executes, Core appends that same subject and
view to `proposalNetwork`.  The retained-lock release goal follows without
Decision convergence or a different producer occurrence.
***************************************************************************)

THEOREM ExecuteRetainedLockSignProposalBroadcastsExactLockedSubject ==
  \A leader \in ValidatorIds, lockedRound, leaderView \in Views,
     subject \in Subjects:
    \A candidate \in AsyncCandidateSet:
      /\ TypeInvariant
      /\ candidate.kind = "SignProposal"
      /\ candidate.node = leader
      /\ candidate.consumerContext = context
      /\ candidate.view = leaderView
      /\ candidate.subject = subject
      /\ leaderView > lockedRound
      /\ ExecuteSignProposal(candidate)
      => LockedBodyReproposedUnchangedLater(lockedRound, subject)'
BY IsaT(180)
   DEF ExecuteSignProposal, CommandMatches,
       CompleteProposalSignature, BroadcastProposals,
       LockedBodyReproposedUnchangedLater,
       ProposalEnvelope, CurrentVoters, CurrentEpoch,
       TypeInvariant, ModelConfiguration

(***************************************************************************
Exact proposal-producer action corridor.

The three nonterminal reducer stages each declare one causal child.  The
child keeps the immutable first-admission origin and every exact body and
authority coordinate; only the work kind changes.  `AssembleBody` cannot
take its Decision/Apply branch while the retained Prepare authority is live,
because `HistoricalLockedPrepareSource` includes `NoDecisionForNode`.

These are action facts, not temporal progress.  In particular, a producer
which is already semantically covered may be discarded instead of executing;
the final theorem in this section classifies that exit into the existing
same-origin, monotone-coverage, or terminal-tombstone lifecycle outcomes.
***************************************************************************)

LockedBodyProposalProducerKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal", "SignProposal"}

LockedBodyProposalPrefixKinds ==
  {"AssembleBody", "BeginProposal", "PersistProposal"}

LockedBodyNextProposalProducerKind(kind) ==
  CASE kind = "AssembleBody" -> "BeginProposal"
    [] kind = "BeginProposal" -> "PersistProposal"
    [] OTHER -> "SignProposal"

LockedBodyProposalCausalSuccessor(command) ==
  CausalCandidate(
    "Completion",
    LockedBodyNextProposalProducerKind(command.kind),
    command)

LockedBodyProposalIntentCovered(candidate) ==
  \E proposal \in proposalIntents:
    /\ proposal.proposer = candidate.node
    /\ proposal.context = candidate.consumerContext
    /\ proposal.height = candidate.height
    /\ proposal.view = candidate.view
    /\ proposal.subject = candidate.subject

LockedBodyProposalProducerDispositionAfter(candidate) ==
  \/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate)
  \/ AsyncCandidateMonotoneSemanticCoverageAfterIn(
       asyncControlServiceState', candidate)
  \/ AsyncCandidateTerminalTombstoned(candidate)'

THEOREM LockedBodyProposalPrefixDeclaresExactSameOriginSuccessor ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, candidate:
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      /\ candidate.kind \in LockedBodyProposalPrefixKinds
      /\ candidate.node = leader
      /\ candidate.consumerContext = context
      /\ candidate.height = context.height
      /\ candidate.view = leaderView
      /\ candidate.subject = subject
      => LET successor == LockedBodyProposalCausalSuccessor(candidate)
         IN /\ CommandSuccessors(candidate) = <<successor>>
            /\ successor.class = "Completion"
            /\ successor.kind =
                 LockedBodyNextProposalProducerKind(candidate.kind)
            /\ successor.node = candidate.node
            /\ successor.height = candidate.height
            /\ successor.view = candidate.view
            /\ successor.subject = candidate.subject
            /\ successor.item = NoAsyncItem
            /\ successor.consumerContext = candidate.consumerContext
            /\ successor.consumerView = candidate.consumerView
            /\ successor.consumerGeneration =
                 candidate.consumerGeneration
            /\ successor.evidence = candidate.evidence
            /\ successor.bodyIdentity = candidate.bodyIdentity
            /\ successor.manifestIdentity = candidate.manifestIdentity
            /\ successor.commitmentIdentity =
                 candidate.commitmentIdentity
            /\ successor.causalOrigin = candidate.causalOrigin
BY IsaT(240)
   DEF LockedBodyResponsiveLeaderAuthority,
       HistoricalLockedPrepareSource, NoDecisionForNode,
       LockedBodyProposalPrefixKinds,
       LockedBodyNextProposalProducerKind,
       LockedBodyProposalCausalSuccessor,
       CommandSuccessors, ExactDecidedLocalBody,
       CausalCandidate, AsyncCandidateFrom,
       AsyncCandidateWithIdentityAndOrigin

THEOREM ExecuteLockedBodyProposalPrefixPreservesResponsiveLeaderAuthority ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc, candidate:
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      /\ candidate.kind \in LockedBodyProposalPrefixKinds
      /\ candidate.node = leader
      /\ candidate.consumerContext = context
      /\ candidate.height = context.height
      /\ candidate.view = leaderView
      /\ candidate.subject = subject
      /\ ExecuteCommand(candidate)
      => LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject,
           prepareQc, leaderView)'
BY IsaT(360)
   DEF LockedBodyProposalPrefixKinds,
       LockedBodyResponsiveLeaderAuthority,
       LockedBodySourcePrepareAuthority,
       StableAvailableRetainedLock,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor,
       ExactLockedCommitIntents,
       NoDecisionForNode,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       AssembleLocalBody, BeginLocalProposal, PersistProposal,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch

THEOREM LockedBodyIgnoredProposalProducerHasDurableDisposition ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc:
      \A candidate \in AsyncCandidateSet:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      /\ candidate.kind \in LockedBodyProposalProducerKinds
      /\ candidate.node = leader
      /\ candidate.consumerContext = context
      /\ candidate.height = context.height
      /\ candidate.view = leaderView
      /\ candidate.subject = subject
      /\ candidate.item = NoAsyncItem
      /\ CandidateConsumerCurrent(candidate)
      /\ (candidate.kind = "SignProposal"
            => LockedBodyProposalIntentCovered(candidate))
      /\ gst
      /\ AsyncNext
      /\ AsyncCandidateIgnoredWithoutApplicationThisStep(candidate)
      => LockedBodyProposalProducerDispositionAfter(candidate)
BY AsyncCandidateDiscardInstallsTerminalTombstone, IsaT(900)
   DEF LockedBodyProposalProducerKinds,
       LockedBodyProposalProducerDispositionAfter,
       LockedBodyResponsiveLeaderAuthority,
       HistoricalLockedPrepareSource, NoDecisionForNode,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateMonotoneSemanticCoverageAfterIn,
       AsyncCandidateReducerStageCoveredAfterIn,
       AsyncCandidateProposalStageCoveredAfter,
       AsyncCandidateConsumerEpisodeObsoleteAfter,
       AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter,
       LockedBodyProposalIntentCovered,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteSignProposal, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature

THEOREM LockedBodyScheduledProposalProducerDepartureIsClassified ==
  \A target, leader \in ValidatorIds, lockedRound \in Views,
     subject \in Subjects, leaderView \in Views:
    \A prepareQc:
      \A candidate \in AsyncCandidateSet:
      /\ AsyncStrongTypeInvariant
      /\ AsyncProgressOwnershipInvariant
      /\ AsyncCandidateServiceLifecycleInvariant
      /\ LockedBodyResponsiveLeaderAuthority(
           target, leader, lockedRound, subject, prepareQc, leaderView)
      /\ candidate.kind \in LockedBodyProposalProducerKinds
      /\ candidate.node = leader
      /\ candidate.consumerContext = context
      /\ candidate.height = context.height
      /\ candidate.view = leaderView
      /\ candidate.subject = subject
      /\ candidate.item = NoAsyncItem
      /\ CandidateConsumerCurrent(candidate)
      /\ (candidate.kind = "SignProposal"
            => LockedBodyProposalIntentCovered(candidate))
      /\ gst
      /\ AsyncNext
      /\ CandidateScheduled(candidate)
      /\ ~CandidateScheduledAfter(candidate)
      => \/ AsyncCandidateSuccessfullyServicedThisStep(candidate)
         \/ LockedBodyProposalProducerDispositionAfter(candidate)
BY AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   LockedBodyIgnoredProposalProducerHasDurableDisposition, Isa
   DEF LockedBodyProposalProducerKinds,
       LockedBodyProposalProducerDispositionAfter

(***************************************************************************
The original single-node frame remains the exact local fast-path action
lemma.  The temporal retained-lock proof below no longer assumes that this
same node must eventually land on one of its own leader views.
***************************************************************************)

LockedBodyProposalAttemptStableFrame(
    node, leaderView, lockedRound, subject) ==
  /\ StableAvailableRetainedLock(node, lockedRound, subject)
  /\ nodeView[node] = leaderView
  /\ Leader(context, leaderView) = node
  /\ leaderView > lockedRound
  /\ NodeInstalledTC(node, leaderView - 1)
  /\ AsyncProposalSubject(node) = subject

LockedBodyProposalAttemptViewExit(node, leaderView) ==
  nodeView[node] # leaderView

LockedBodyProposalCertifiedHighExit(node, lockedRound, subject) ==
  /\ AsyncProposalSubject(node) # subject
  /\ highestRank[node] > lockedRound
  /\ \E qc \in prepareQCs:
       /\ qc.context = context
       /\ qc.phase = "Prepare"
       /\ qc.view = highestRank[node]
       /\ qc.subject = highestSubject[node]

(***************************************************************************
Exact Core-action decomposition for the stable locked-body proposer frame.

The asynchronous relation admits process crashes only before GST.  Projecting
it immediately to `[Next]_vars` would forget that guard and admit a spurious
post-GST crash.  The leaves below therefore retain the exact post-GST `up`
frame before classifying the only Core actions which can mutate a stable
locked-body proposal attempt.
***************************************************************************)

LockedBodyProposalStableVars ==
  <<context, nodeView, up, gst, durableBodies, retainedLockedBodies,
    installedTCs, lockRank, lockSubject, highestRank, highestSubject>>

THEOREM PostGstAsyncNextLeavesUp ==
  gst /\ AsyncNext => UNCHANGED up
BY Isa
   DEF AsyncNext, AsyncNonCrashStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery

THEOREM NextEitherSetsGstOrLeavesIt ==
  Next => SetGST \/ UNCHANGED gst
BY IsaM("blast")
   DEF Next, AssembleLocalBody, ByzantineBroadcastProposal,
       DeliverProposal, FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody, ValidateLockedBody,
       BeginLocalProposal, PersistProposal, CompleteProposalSignature,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, DeliverTC, BeginInstallTC, PersistInstallTC,
       FetchCertifiedBody, ApplyDecision, Crash, Restart,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal

THEOREM PostGstCoreBracketLeavesGst ==
  gst /\ [Next]_vars => UNCHANGED gst
BY NextEitherSetsGstOrLeavesIt, Isa DEF SetGST, vars

THEOREM PostGstAsyncNextLeavesUpAndGst ==
  gst /\ AsyncNext => UNCHANGED <<up, gst>>
BY PostGstAsyncNextLeavesUp, PostGstCoreBracketLeavesGst
   DEF AsyncNext

LockedBodyStableVarActionClassification ==
  \/ UNCHANGED LockedBodyProposalStableVars
  \/ SetGST
  \/ \E node \in ValidatorIds, subject \in Subjects:
       AssembleLocalBody(node, subject)
  \/ \E node \in ValidatorIds, roundView \in Views,
        subject \in Subjects:
       StoreBody(node, roundView, subject)
  \/ \E request \in pendingObservePrepare:
       PersistObservePrepare(request)
  \/ \E request \in pendingLockCommit:
       PersistLockCommit(request)
  \/ \E request \in pendingInstallTC:
       PersistInstallTC(request)
  \/ \E node \in ValidatorIds: Crash(node)
  \/ \E node \in ValidatorIds: Restart(node)

THEOREM NextLockedBodyStableVarActionClassification ==
  Next => LockedBodyStableVarActionClassification
BY IsaM("blast")
   DEF Next, ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, ValidateBody, RejectBody,
       ValidateDecidedBody, ValidateLockedBody,
       BeginLocalProposal, PersistProposal, CompleteProposalSignature,
       BeginPrepare, PersistPrepare, CompleteVoteSignature,
       ByzantineBroadcastVote, DeliverVote, FormPrepareQC,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, BeginLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout, PersistTimeout,
       CompleteTimeoutSignature, ByzantineBroadcastTimeout,
       DeliverTimeout, DeliverTC, BeginInstallTC,
       FetchCertifiedBody, ApplyDecision,
       ResumeProposal, ResumeVote, ResumeTimeout, DropProposal,
       LockedBodyProposalStableVars,
       LockedBodyStableVarActionClassification

THEOREM StableVarsStutterPreservesLockedBodyProposalAttemptStableFrame ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    /\ LockedBodyProposalAttemptStableFrame(
         node, leaderView, lockedRound, subject)
    /\ UNCHANGED LockedBodyProposalStableVars
    => LockedBodyProposalAttemptStableFrame(
         node, leaderView, lockedRound, subject)'
BY Isa
   DEF LockedBodyProposalAttemptStableFrame,
       StableAvailableRetainedLock, AsyncProposalSubject,
       AsyncCurrentResponsiveVoters, NodeInstalledTC,
       CurrentVoters, CurrentEpoch, LockedBodyProposalStableVars

THEOREM AssemblePreservesLockedBodyProposalAttemptStableFrame ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    \A assembler \in ValidatorIds, assembled \in Subjects:
      /\ LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)
      /\ AssembleLocalBody(assembler, assembled)
      => LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)'
BY Isa
   DEF LockedBodyProposalAttemptStableFrame,
       StableAvailableRetainedLock, AsyncProposalSubject,
       AsyncCurrentResponsiveVoters, NodeInstalledTC,
       CurrentVoters, CurrentEpoch, AssembleLocalBody, BodyHeldBy

THEOREM StorePreservesLockedBodyProposalAttemptStableFrame ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    \A storer \in ValidatorIds, roundView \in Views,
       stored \in Subjects:
      /\ LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)
      /\ StoreBody(storer, roundView, stored)
      => LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)'
BY Isa
   DEF LockedBodyProposalAttemptStableFrame,
       StableAvailableRetainedLock, AsyncProposalSubject,
       AsyncCurrentResponsiveVoters, NodeInstalledTC,
       CurrentVoters, CurrentEpoch, StoreBody, BodyHeldBy

PersistObserveFixedPointFrame(node) ==
  /\ UNCHANGED
       <<context, nodeView, up, gst, durableBodies, retainedLockedBodies,
         prepareQCs, installedTCs, lockRank, lockSubject>>
  /\ highestRank'[node] = highestRank[node]
  /\ highestSubject'[node] = highestSubject[node]

PersistObserveTargetPointFrame(node, request) ==
  /\ UNCHANGED
       <<context, nodeView, up, gst, durableBodies, retainedLockedBodies,
         prepareQCs, installedTCs, lockRank, lockSubject>>
  /\ highestRank'[node] = request.qc.view
  /\ highestSubject'[node] = request.qc.subject

THEOREM ExceptAtDifferentKeyPreservesPoint ==
  \A f \in [ValidatorIds -> Ranks]:
    \A replacement:
      \A changedKey, observedKey \in ValidatorIds:
        changedKey # observedKey
          => [f EXCEPT ![changedKey] = replacement][observedKey]
               = f[observedKey]
BY Isa

THEOREM SubjectExceptAtDifferentKeyPreservesPoint ==
  \A f \in [ValidatorIds -> SubjectOrNone]:
    \A replacement:
      \A changedKey, observedKey \in ValidatorIds:
        changedKey # observedKey
          => [f EXCEPT ![changedKey] = replacement][observedKey]
               = f[observedKey]
BY Isa

THEOREM ExceptAtSameKeyReturnsReplacement ==
  \A f \in [ValidatorIds -> Ranks]:
    \A replacement:
      \A key \in ValidatorIds:
        [f EXCEPT ![key] = replacement][key] = replacement
BY Isa

THEOREM SubjectExceptAtSameKeyReturnsReplacement ==
  \A f \in [ValidatorIds -> SubjectOrNone]:
    \A replacement:
      \A key \in ValidatorIds:
        [f EXCEPT ![key] = replacement][key] = replacement
BY Isa

THEOREM PersistObserveOtherNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingObservePrepare:
      /\ TypeInvariant
      /\ request.node # node
      /\ PersistObservePrepare(request)
      => PersistObserveFixedPointFrame(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingObservePrepare,
                TypeInvariant,
                request.node # node,
                PersistObservePrepare(request)
         PROVE PersistObserveFixedPointFrame(node)
    <2>1. /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
           /\ request.node \in ValidatorIds
      BY <1>1, Isa DEF TypeInvariant, ObservePrepareWalSet
    <2>2. UNCHANGED
             <<context, nodeView, up, gst, durableBodies,
               retainedLockedBodies, prepareQCs, installedTCs,
               lockRank, lockSubject>>
      BY <1>1, Isa DEF PersistObservePrepare
    <2>3. highestRank'[node] = highestRank[node]
      BY <1>1, <2>1, ExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistObservePrepare
    <2>4. highestSubject'[node] = highestSubject[node]
      BY <1>1, <2>1, SubjectExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistObservePrepare
    <2> QED
      BY <2>2, <2>3, <2>4
         DEF PersistObserveFixedPointFrame
  <1> QED BY <1>1

THEOREM PersistObserveRequestNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingObservePrepare:
      /\ TypeInvariant
      /\ request.node = node
      /\ PersistObservePrepare(request)
      => PersistObserveTargetPointFrame(node, request)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingObservePrepare,
                TypeInvariant,
                request.node = node,
                PersistObservePrepare(request)
         PROVE PersistObserveTargetPointFrame(node, request)
    <2>1. /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, Isa DEF TypeInvariant
    <2>2. UNCHANGED
             <<context, nodeView, up, gst, durableBodies,
               retainedLockedBodies, prepareQCs, installedTCs,
               lockRank, lockSubject>>
      BY <1>1, Isa DEF PersistObservePrepare
    <2>3. highestRank'[node] = request.qc.view
      BY <1>1, <2>1, ExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistObservePrepare
    <2>4. highestSubject'[node] = request.qc.subject
      BY <1>1, <2>1, SubjectExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistObservePrepare
    <2> QED
      BY <2>2, <2>3, <2>4
         DEF PersistObserveTargetPointFrame
  <1> QED BY <1>1

THEOREM PersistObserveTargetFrameSetsProposalSubject ==
  \A node \in ValidatorIds, subject \in Subjects:
    \A request \in pendingObservePrepare:
      /\ TypeInvariant
      /\ request.qc.view \in Views
      /\ request.qc.subject = subject
      /\ PersistObserveTargetPointFrame(node, request)
      => (IF highestRank'[node] = NoRank
          THEN AsyncHeartbeatSubject
          ELSE highestSubject'[node]) = subject
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW subject \in Subjects,
                NEW request \in pendingObservePrepare,
                TypeInvariant,
                request.qc.view \in Views,
                request.qc.subject = subject,
                PersistObserveTargetPointFrame(node, request)
         PROVE (IF highestRank'[node] = NoRank
                THEN AsyncHeartbeatSubject
                ELSE highestSubject'[node]) = subject
    <2>1. ModelConfiguration
      BY <1>1 DEF TypeInvariant
    <2>2. request.qc.view # NoRank
      BY <1>1, <2>1, ViewIsNotNoRank
    <2> QED
      BY <1>1, <2>2, Isa
         DEF PersistObserveTargetPointFrame
  <1> QED BY <1>1

THEOREM PersistObserveRankStrictlyExceedsRetainedLock ==
  \A node \in ValidatorIds, lockedRound \in Views:
    \A request \in pendingObservePrepare:
      /\ TypeInvariant
      /\ request.qc.view \in Views
      /\ request.qc.view > highestRank[request.node]
      /\ lockedRound <= highestRank[node]
      /\ request.node = node
      => request.qc.view > lockedRound
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW lockedRound \in Views,
                NEW request \in pendingObservePrepare,
                TypeInvariant,
                request.qc.view \in Views,
                request.qc.view > highestRank[request.node],
                lockedRound <= highestRank[node],
                request.node = node
         PROVE request.qc.view > lockedRound
    <2>1. highestRank[request.node] = highestRank[node]
      BY <1>1
    <2>2. request.qc.view > highestRank[node]
      BY <1>1, <2>1
    <2>3. highestRank[node] \in Ranks
      BY <1>1, Isa DEF TypeInvariant
    <2> QED BY <1>1, <2>2, <2>3, SMT
         DEF TypeInvariant, ModelConfiguration, Views, Ranks, NoRank
  <1> QED BY <1>1

THEOREM PersistObservePreservesLockedBodyProposalAttemptOrCertifiedExit ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    \A request \in pendingObservePrepare:
      /\ StrongInductiveInvariant
      /\ LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)
      /\ PersistObservePrepare(request)
      => \/ LockedBodyProposalAttemptStableFrame(
              node, leaderView, lockedRound, subject)'
         \/ LockedBodyProposalCertifiedHighExit(
              node, lockedRound, subject)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW leaderView \in Views,
                NEW lockedRound \in Views,
                NEW subject \in Subjects,
                NEW request \in pendingObservePrepare,
                StrongInductiveInvariant,
                LockedBodyProposalAttemptStableFrame(
                  node, leaderView, lockedRound, subject),
                PersistObservePrepare(request)
         PROVE \/ LockedBodyProposalAttemptStableFrame(
                   node, leaderView, lockedRound, subject)'
               \/ LockedBodyProposalCertifiedHighExit(
                    node, lockedRound, subject)'
    <2>1. /\ request.qc \in prepareQCs
           /\ request.qc.context = context
           /\ request.qc.view \in Views
           /\ request.qc.phase = "Prepare"
           /\ request.qc.view > highestRank[request.node]
           /\ lockedRound <= highestRank[node]
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized, CertificatesBackedByIntents,
             HistoricalQcValid, LineageInvariant, CertificatePhasesCorrect,
             Safety, LockBelowHighest,
             LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock
    <2>2. CASE request.node # node
      BY <1>1, <2>2, StrongInvariantProjectsType,
         PersistObserveOtherNodePointFrame, Isa
         DEF PersistObserveFixedPointFrame,
             LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock, AsyncProposalSubject,
             AsyncCurrentResponsiveVoters, NodeInstalledTC,
             CurrentVoters, CurrentEpoch
    <2>3. CASE request.node = node
      <3>1. request.qc.view > lockedRound
        BY <1>1, <2>1, <2>3, StrongInvariantProjectsType,
           PersistObserveRankStrictlyExceedsRetainedLock
      <3>2. CASE request.qc.subject = subject
        <4>1. TypeInvariant
          BY <1>1, StrongInvariantProjectsType
        <4>2. PersistObserveTargetPointFrame(node, request)
          BY <1>1, <2>3, <4>1,
             PersistObserveRequestNodePointFrame
        <4>3. (IF highestRank'[node] = NoRank
                THEN AsyncHeartbeatSubject
                ELSE highestSubject'[node]) = subject
          BY <2>1, <3>2, <4>1, <4>2,
             PersistObserveTargetFrameSetsProposalSubject
        <4>4. LockedBodyProposalAttemptStableFrame(
                 node, leaderView, lockedRound, subject)'
          BY <1>1, <4>2, <4>3, Isa
             DEF PersistObserveTargetPointFrame,
                 LockedBodyProposalAttemptStableFrame,
                 StableAvailableRetainedLock,
                 AsyncProposalSubject,
                 AsyncCurrentResponsiveVoters, NodeInstalledTC,
                 CurrentVoters, CurrentEpoch
        <4> QED BY <4>4
      <3>3. CASE request.qc.subject # subject
        <4>1. TypeInvariant
          BY <1>1, StrongInvariantProjectsType
        <4>2. PersistObserveTargetPointFrame(node, request)
          BY <1>1, <2>3, <4>1,
             PersistObserveRequestNodePointFrame
        <4>3. ModelConfiguration
          BY <4>1 DEF TypeInvariant
        <4>4. request.qc.view # NoRank
          BY <2>1, <4>3, ViewIsNotNoRank
        <4>5. LockedBodyProposalCertifiedHighExit(
                 node, lockedRound, subject)'
          BY <1>1, <2>1, <3>1, <3>3, <4>2, <4>4, Isa
             DEF PersistObserveTargetPointFrame,
                 LockedBodyProposalCertifiedHighExit,
                 AsyncProposalSubject
        <4> QED BY <4>5
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

PersistLockFixedPointFrame(node) ==
  /\ UNCHANGED
       <<context, nodeView, up, gst, durableBodies, prepareQCs,
         installedTCs>>
  /\ retainedLockedBodies \subseteq retainedLockedBodies'
  /\ lockRank'[node] = lockRank[node]
  /\ lockSubject'[node] = lockSubject[node]
  /\ highestRank'[node] = highestRank[node]
  /\ highestSubject'[node] = highestSubject[node]

PersistLockTargetPointFrame(node, request) ==
  /\ UNCHANGED
       <<context, nodeView, up, gst, durableBodies, prepareQCs,
         installedTCs>>
  /\ retainedLockedBodies \subseteq retainedLockedBodies'
  /\ lockRank'[node] = request.qc.view
  /\ lockSubject'[node] = request.qc.subject
  /\ highestRank'[node] =
       IF request.qc.view > highestRank[node]
       THEN request.qc.view
       ELSE highestRank[node]
  /\ highestSubject'[node] =
       IF request.qc.view > highestRank[node]
       THEN request.qc.subject
       ELSE highestSubject[node]

THEOREM PersistLockOtherNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingLockCommit:
      /\ TypeInvariant
      /\ request.node # node
      /\ PersistLockCommit(request)
      => PersistLockFixedPointFrame(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingLockCommit,
                TypeInvariant,
                request.node # node,
                PersistLockCommit(request)
         PROVE PersistLockFixedPointFrame(node)
    <2>1. /\ lockRank \in [ValidatorIds -> Ranks]
           /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
           /\ request.node \in ValidatorIds
      BY <1>1, Isa DEF TypeInvariant, LockCommitWalSet
    <2>2. /\ UNCHANGED
               <<context, nodeView, up, gst, durableBodies, prepareQCs,
                 installedTCs>>
           /\ retainedLockedBodies \subseteq retainedLockedBodies'
      BY <1>1, Isa DEF PersistLockCommit
    <2>3. lockRank'[node] = lockRank[node]
      BY <1>1, <2>1, ExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistLockCommit
    <2>4. lockSubject'[node] = lockSubject[node]
      BY <1>1, <2>1, SubjectExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistLockCommit
    <2>5. highestRank'[node] = highestRank[node]
      BY <1>1, <2>1, ExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistLockCommit
    <2>6. highestSubject'[node] = highestSubject[node]
      BY <1>1, <2>1, SubjectExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistLockCommit
    <2> QED
      BY <2>2, <2>3, <2>4, <2>5, <2>6
         DEF PersistLockFixedPointFrame
  <1> QED BY <1>1

THEOREM PersistLockRequestNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingLockCommit:
      /\ TypeInvariant
      /\ request.node = node
      /\ PersistLockCommit(request)
      => PersistLockTargetPointFrame(node, request)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingLockCommit,
                TypeInvariant,
                request.node = node,
                PersistLockCommit(request)
         PROVE PersistLockTargetPointFrame(node, request)
    <2>1. /\ lockRank \in [ValidatorIds -> Ranks]
           /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, Isa DEF TypeInvariant
    <2>2. /\ UNCHANGED
               <<context, nodeView, up, gst, durableBodies, prepareQCs,
                 installedTCs>>
           /\ retainedLockedBodies \subseteq retainedLockedBodies'
      BY <1>1, Isa DEF PersistLockCommit
    <2>3. lockRank'[node] = request.qc.view
      BY <1>1, <2>1, ExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistLockCommit
    <2>4. lockSubject'[node] = request.qc.subject
      BY <1>1, <2>1, SubjectExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistLockCommit
    <2>5. highestRank'[node] =
             IF request.qc.view > highestRank[node]
             THEN request.qc.view
             ELSE highestRank[node]
      BY <1>1, <2>1, ExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistLockCommit
    <2>6. highestSubject'[node] =
             IF request.qc.view > highestRank[node]
             THEN request.qc.subject
             ELSE highestSubject[node]
      BY <1>1, <2>1, SubjectExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistLockCommit
    <2> QED
      BY <2>2, <2>3, <2>4, <2>5, <2>6
         DEF PersistLockTargetPointFrame
  <1> QED BY <1>1

THEOREM PersistLockPreservesLockedBodyProposalAttemptOrSupersedes ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    \A request \in pendingLockCommit:
      /\ StrongInductiveInvariant
      /\ LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)
      /\ PersistLockCommit(request)
      => \/ LockedBodyProposalAttemptStableFrame(
              node, leaderView, lockedRound, subject)'
         \/ LockedBodyLegitimatelyDecidedOrSuperseded(
              node, lockedRound, subject)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW leaderView \in Views,
                NEW lockedRound \in Views,
                NEW subject \in Subjects,
                NEW request \in pendingLockCommit,
                StrongInductiveInvariant,
                LockedBodyProposalAttemptStableFrame(
                  node, leaderView, lockedRound, subject),
                PersistLockCommit(request)
         PROVE \/ LockedBodyProposalAttemptStableFrame(
                   node, leaderView, lockedRound, subject)'
               \/ LockedBodyLegitimatelyDecidedOrSuperseded(
                    node, lockedRound, subject)'
    <2>1. /\ request.qc \in prepareQCs
           /\ request.qc.context = context
           /\ request.qc.view \in Views
           /\ request.qc.phase = "Prepare"
           /\ request.qc.view >= lockRank[request.node]
           /\ (request.qc.view = lockRank[request.node]
                 => request.qc.subject = lockSubject[request.node])
           /\ lockRank[node] = lockedRound
           /\ lockSubject[node] = subject
           /\ lockedRound <= highestRank[node]
      BY <1>1, Isa
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingVoteWritesAuthorized, CertificatesBackedByIntents,
             HistoricalQcValid, LineageInvariant, CertificatePhasesCorrect,
             Safety, LockBelowHighest,
             LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock
    <2>2. CASE request.node # node
      <3>1. TypeInvariant
        BY <1>1, StrongInvariantProjectsType
      <3>2. PersistLockFixedPointFrame(node)
        BY <1>1, <2>2, <3>1, PersistLockOtherNodePointFrame
      <3>3. LockedBodyProposalAttemptStableFrame(
               node, leaderView, lockedRound, subject)'
        BY <1>1, <3>2, Isa
           DEF PersistLockFixedPointFrame,
               LockedBodyProposalAttemptStableFrame,
               StableAvailableRetainedLock, AsyncProposalSubject,
               AsyncCurrentResponsiveVoters, NodeInstalledTC,
               CurrentVoters, CurrentEpoch,
               RetainedLockedBodyHeldBy
      <3> QED BY <3>3
    <2>3. CASE request.node = node
      <3>1. TypeInvariant
        BY <1>1, StrongInvariantProjectsType
      <3>2. PersistLockTargetPointFrame(node, request)
        BY <1>1, <2>3, <3>1, PersistLockRequestNodePointFrame
      <3>3. request.qc.view >= lockedRound
        BY <2>1, <2>3, Isa
      <3>4. CASE request.qc.view = lockedRound
        <4>1. /\ request.qc.subject = subject
               /\ request.qc.view <= highestRank[node]
          BY <2>1, <2>3, <3>4, Isa
        <4>2. /\ ModelConfiguration
               /\ highestRank[node] \in Ranks
          BY <3>1, Isa DEF TypeInvariant
        <4>3. /\ request.qc.view \in Int
               /\ highestRank[node] \in Int
          BY <2>1, <4>2, ModelRanksAreIntegers,
             ViewsAreRanks, SMT
        <4>4. ~(request.qc.view > highestRank[node])
          BY <4>1, <4>3, SMT
        <4>5. /\ lockRank'[node] = lockedRound
               /\ lockSubject'[node] = subject
               /\ highestRank'[node] = highestRank[node]
               /\ highestSubject'[node] = highestSubject[node]
          BY <3>2, <3>4, <4>1, <4>4, Isa
             DEF PersistLockTargetPointFrame
        <4>6. RetainedLockedBodyHeldBy(
                 retainedLockedBodies', node, context', subject)
          BY <1>1, <3>2, Isa
             DEF PersistLockTargetPointFrame,
                 LockedBodyProposalAttemptStableFrame,
                 StableAvailableRetainedLock,
                 RetainedLockedBodyHeldBy
        <4>7. StableAvailableRetainedLock(
                 node, lockedRound, subject)'
          BY <1>1, <3>2, <4>5, <4>6, Isa
             DEF PersistLockTargetPointFrame,
                 LockedBodyProposalAttemptStableFrame,
                 StableAvailableRetainedLock,
                 AsyncCurrentResponsiveVoters,
                 CurrentVoters, CurrentEpoch
        <4>8. (IF highestRank'[node] = NoRank
                THEN AsyncHeartbeatSubject
                ELSE highestSubject'[node]) = subject
          BY <1>1, <4>5, Isa
             DEF LockedBodyProposalAttemptStableFrame,
                 AsyncProposalSubject
        <4>9. LockedBodyProposalAttemptStableFrame(
                 node, leaderView, lockedRound, subject)'
          BY <1>1, <3>2, <4>7, <4>8, Isa
             DEF PersistLockTargetPointFrame,
                 LockedBodyProposalAttemptStableFrame,
                 AsyncProposalSubject,
                 NodeInstalledTC
        <4> QED BY <4>9
      <3>5. CASE request.qc.view # lockedRound
        <4>1. request.qc.view > lockedRound
          BY <3>3, <3>5, Isa
        <4>2. LockedBodyLegitimatelyDecidedOrSuperseded(
                 node, lockedRound, subject)'
          BY <2>1, <3>2, <4>1, Isa
             DEF PersistLockTargetPointFrame,
                 LockedBodyLegitimatelyDecidedOrSuperseded
        <4> QED BY <4>2
      <3> QED BY <3>4, <3>5
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ViewExceptAtDifferentKeyPreservesPoint ==
  \A f \in [ValidatorIds -> Views]:
    \A replacement:
      \A changedKey, observedKey \in ValidatorIds:
        changedKey # observedKey
          => [f EXCEPT ![changedKey] = replacement][observedKey]
               = f[observedKey]
BY Isa

THEOREM ViewExceptAtSameKeyReturnsReplacement ==
  \A f \in [ValidatorIds -> Views]:
    \A replacement:
      \A key \in ValidatorIds:
        [f EXCEPT ![key] = replacement][key] = replacement
BY Isa

PersistInstallFixedPointFrame(node) ==
  /\ UNCHANGED
       <<context, up, gst, durableBodies, retainedLockedBodies, prepareQCs>>
  /\ installedTCs \subseteq installedTCs'
  /\ nodeView'[node] = nodeView[node]
  /\ lockRank'[node] = lockRank[node]
  /\ lockSubject'[node] = lockSubject[node]
  /\ highestRank'[node] = highestRank[node]
  /\ highestSubject'[node] = highestSubject[node]

PersistInstallTargetPointFrame(node, request) ==
  LET selectedRank == TcHighRank(request.tc)
      selectedSubject == TcHighSubject(request.tc)
      sameRoundUpgrade == StrictSameRoundTcUpgrade(node, request.tc)
  IN /\ UNCHANGED
           <<context, up, gst, durableBodies, retainedLockedBodies,
             prepareQCs>>
     /\ installedTCs \subseteq installedTCs'
     /\ nodeView'[node] =
          IF sameRoundUpgrade THEN nodeView[node]
          ELSE request.tc.view + 1
     /\ lockRank'[node] =
          IF selectedRank > lockRank[node]
          THEN selectedRank
          ELSE lockRank[node]
     /\ lockSubject'[node] =
          IF selectedRank > lockRank[node]
          THEN selectedSubject
          ELSE lockSubject[node]
     /\ highestRank'[node] =
          IF selectedRank > highestRank[node]
          THEN selectedRank
          ELSE highestRank[node]
     /\ highestSubject'[node] =
          IF selectedRank > highestRank[node]
          THEN selectedSubject
          ELSE highestSubject[node]

THEOREM PersistInstallOtherNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingInstallTC:
      /\ TypeInvariant
      /\ request.node # node
      /\ PersistInstallTC(request)
      => PersistInstallFixedPointFrame(node)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingInstallTC,
                TypeInvariant,
                request.node # node,
                PersistInstallTC(request)
         PROVE PersistInstallFixedPointFrame(node)
    <2>1. /\ nodeView \in [ValidatorIds -> Views]
           /\ lockRank \in [ValidatorIds -> Ranks]
           /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
           /\ request.node \in ValidatorIds
      BY <1>1, Isa DEF TypeInvariant, InstallTcWalSet
    <2>2. /\ UNCHANGED
               <<context, up, gst, durableBodies, retainedLockedBodies,
                 prepareQCs>>
           /\ installedTCs \subseteq installedTCs'
      BY <1>1, Isa DEF PersistInstallTC
    <2>3. nodeView'[node] = nodeView[node]
      BY <1>1, <2>1, ViewExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistInstallTC
    <2>4. lockRank'[node] = lockRank[node]
      BY <1>1, <2>1, ExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistInstallTC
    <2>5. lockSubject'[node] = lockSubject[node]
      BY <1>1, <2>1, SubjectExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistInstallTC
    <2>6. highestRank'[node] = highestRank[node]
      BY <1>1, <2>1, ExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistInstallTC
    <2>7. highestSubject'[node] = highestSubject[node]
      BY <1>1, <2>1, SubjectExceptAtDifferentKeyPreservesPoint, Isa
         DEF PersistInstallTC
    <2> QED
      BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF PersistInstallFixedPointFrame
  <1> QED BY <1>1

THEOREM PersistInstallRequestNodePointFrame ==
  \A node \in ValidatorIds:
    \A request \in pendingInstallTC:
      /\ TypeInvariant
      /\ request.node = node
      /\ PersistInstallTC(request)
      => PersistInstallTargetPointFrame(node, request)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW request \in pendingInstallTC,
                TypeInvariant,
                request.node = node,
                PersistInstallTC(request)
         PROVE PersistInstallTargetPointFrame(node, request)
    <2>1. /\ nodeView \in [ValidatorIds -> Views]
           /\ lockRank \in [ValidatorIds -> Ranks]
           /\ lockSubject \in [ValidatorIds -> SubjectOrNone]
           /\ highestRank \in [ValidatorIds -> Ranks]
           /\ highestSubject \in [ValidatorIds -> SubjectOrNone]
      BY <1>1, Isa DEF TypeInvariant
    <2>2. /\ UNCHANGED
               <<context, up, gst, durableBodies, retainedLockedBodies,
                 prepareQCs>>
           /\ installedTCs \subseteq installedTCs'
      BY <1>1, Isa DEF PersistInstallTC
    <2>3. nodeView'[node] =
             IF StrictSameRoundTcUpgrade(node, request.tc)
             THEN nodeView[node]
             ELSE request.tc.view + 1
      BY <1>1, <2>1, ViewExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistInstallTC
    <2>4. lockRank'[node] =
             IF TcHighRank(request.tc) > lockRank[node]
             THEN TcHighRank(request.tc)
             ELSE lockRank[node]
      BY <1>1, <2>1, ExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistInstallTC
    <2>5. lockSubject'[node] =
             IF TcHighRank(request.tc) > lockRank[node]
             THEN TcHighSubject(request.tc)
             ELSE lockSubject[node]
      BY <1>1, <2>1, SubjectExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistInstallTC
    <2>6. highestRank'[node] =
             IF TcHighRank(request.tc) > highestRank[node]
             THEN TcHighRank(request.tc)
             ELSE highestRank[node]
      BY <1>1, <2>1, ExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistInstallTC
    <2>7. highestSubject'[node] =
             IF TcHighRank(request.tc) > highestRank[node]
             THEN TcHighSubject(request.tc)
             ELSE highestSubject[node]
      BY <1>1, <2>1, SubjectExceptAtSameKeyReturnsReplacement, Isa
         DEF PersistInstallTC
    <2> QED
      BY <2>2, <2>3, <2>4, <2>5, <2>6, <2>7
         DEF PersistInstallTargetPointFrame
  <1> QED BY <1>1

THEOREM StrongInvariantPrepareQCsHavePreparePhase ==
  StrongInductiveInvariant
    => \A qc \in prepareQCs: qc.phase = "Prepare"
BY Isa
   DEF StrongInductiveInvariant, LineageInvariant,
       CertificatePhasesCorrect

THEOREM PersistInstallPreservesLockedBodyProposalAttemptOrExits ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    \A request \in pendingInstallTC:
      /\ StrongInductiveInvariant
      /\ LockedBodyProposalAttemptStableFrame(
           node, leaderView, lockedRound, subject)
      /\ PersistInstallTC(request)
      => \/ LockedBodyProposalAttemptStableFrame(
              node, leaderView, lockedRound, subject)'
         \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
         \/ LockedBodyLegitimatelyDecidedOrSuperseded(
              node, lockedRound, subject)'
         \/ LockedBodyProposalCertifiedHighExit(
              node, lockedRound, subject)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW leaderView \in Views,
                NEW lockedRound \in Views,
                NEW subject \in Subjects,
                NEW request \in pendingInstallTC,
                StrongInductiveInvariant,
                LockedBodyProposalAttemptStableFrame(
                  node, leaderView, lockedRound, subject),
                PersistInstallTC(request)
         PROVE \/ LockedBodyProposalAttemptStableFrame(
                   node, leaderView, lockedRound, subject)'
               \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
               \/ LockedBodyLegitimatelyDecidedOrSuperseded(
                    node, lockedRound, subject)'
               \/ LockedBodyProposalCertifiedHighExit(
                    node, lockedRound, subject)'
    <2>1. TypeInvariant
      BY <1>1, StrongInvariantProjectsType
    <2>2. /\ request.node \in ValidatorIds
           /\ nodeView[request.node] \in Views
      BY <1>1, <2>1, Isa DEF TypeInvariant, InstallTcWalSet
    <2>3. /\ request.tc \in formedTCs
           /\ request.tc.context = context
           /\ TCValid(request.tc)
           /\ request.tc.votes # {}
           /\ request.tc.view + 1 \in Views
           /\ request.tc.view + 1 >= nodeView[request.node]
      BY <1>1
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             PendingCertificateWritesAuthorized
    <2>4. request.tc.view \in Views
      BY <2>3 DEF TCValid
    <2>5. lockRank[node] = lockedRound
      BY <1>1
         DEF LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock
    <2>6. HighestTimeoutVote(request.tc.votes) \in request.tc.votes
      BY <1>1, <2>3,
         StrongInvariantImpliesTimeoutCertificateSelectorsSound
         DEF TimeoutCertificateSelectorsSound
    <2>7. AuthenticatedHighRef(
             TcHighRank(request.tc), TcHighSubject(request.tc))
      BY <2>3, <2>6
         DEF TCValid, TcHighRank, TcHighSubject
    <2>8. CASE request.node # node
      <3>1. PersistInstallFixedPointFrame(node)
        BY <1>1, <2>1, <2>8, PersistInstallOtherNodePointFrame
      <3>2. LockedBodyProposalAttemptStableFrame(
               node, leaderView, lockedRound, subject)'
        BY <1>1, <3>1, Isa
           DEF PersistInstallFixedPointFrame,
               LockedBodyProposalAttemptStableFrame,
               StableAvailableRetainedLock, AsyncProposalSubject,
               AsyncCurrentResponsiveVoters, NodeInstalledTC,
               CurrentVoters, CurrentEpoch
      <3> QED BY <3>2
    <2>9. CASE request.node = node
      <3>1. PersistInstallTargetPointFrame(node, request)
        BY <1>1, <2>1, <2>9, PersistInstallRequestNodePointFrame
      <3>2. CASE StrictSameRoundTcUpgrade(node, request.tc)
        <4>1. /\ TcHighRank(request.tc) > lockedRound
               /\ lockRank'[node] = TcHighRank(request.tc)
               /\ lockSubject'[node] = TcHighSubject(request.tc)
          BY <2>5, <2>9, <3>1, <3>2, Isa
             DEF PersistInstallTargetPointFrame,
                 StrictSameRoundTcUpgrade
        <4>2. TcHighRank(request.tc) # NoRank
          BY <1>1, <2>1, <4>1, ModelViewsAreNaturals, SMT
             DEF TypeInvariant, ModelConfiguration, NoRank
        <4>3. \E qc \in prepareQCs:
                 /\ qc.context = context
                 /\ qc.phase = "Prepare"
                 /\ qc.view = TcHighRank(request.tc)
                 /\ qc.subject = TcHighSubject(request.tc)
          BY <1>1, <2>7, <4>2,
             StrongInvariantPrepareQCsHavePreparePhase, Isa
             DEF AuthenticatedHighRef, HighRefValid
        <4>4. LockedBodyLegitimatelyDecidedOrSuperseded(
                 node, lockedRound, subject)'
          BY <3>1, <4>1, <4>3, Isa
             DEF PersistInstallTargetPointFrame,
                 LockedBodyLegitimatelyDecidedOrSuperseded
        <4> QED BY <4>4
      <3>3. CASE ~StrictSameRoundTcUpgrade(node, request.tc)
        <4>1. request.tc.view >= nodeView[node]
          BY <1>1, <2>9, <3>3 DEF PersistInstallTC
        <4>2. ModelConfiguration
          BY <2>1 DEF TypeInvariant
        <4>3. Views \subseteq Nat
          BY <4>2, ModelViewsAreNaturals
        <4>4. /\ request.tc.view \in Nat
               /\ nodeView[node] \in Nat
          BY <2>2, <2>4, <2>9, <4>3, Isa
        <4>5. nodeView'[node] # leaderView
          BY <1>1, <3>1, <3>3, <4>1, <4>4, SMT
             DEF PersistInstallTargetPointFrame,
                 LockedBodyProposalAttemptStableFrame
        <4>6. LockedBodyProposalAttemptViewExit(node, leaderView)'
          BY <4>5 DEF LockedBodyProposalAttemptViewExit
        <4> QED BY <4>6
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>8, <2>9
  <1> QED BY <1>1

THEOREM NonCrashNextPreservesLockedBodyProposalAttemptOrExits ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    /\ StrongInductiveInvariant
    /\ LockedBodyProposalAttemptStableFrame(
         node, leaderView, lockedRound, subject)
    /\ Next
    /\ UNCHANGED <<up, gst>>
    => \/ LockedBodyProposalAttemptStableFrame(
            node, leaderView, lockedRound, subject)'
       \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
       \/ LockedBodyLegitimatelyDecidedOrSuperseded(
            node, lockedRound, subject)'
       \/ LockedBodyProposalCertifiedHighExit(
            node, lockedRound, subject)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW leaderView \in Views,
                NEW lockedRound \in Views,
                NEW subject \in Subjects,
                StrongInductiveInvariant,
                LockedBodyProposalAttemptStableFrame(
                  node, leaderView, lockedRound, subject),
                Next,
                UNCHANGED <<up, gst>>
         PROVE \/ LockedBodyProposalAttemptStableFrame(
                   node, leaderView, lockedRound, subject)'
               \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
               \/ LockedBodyLegitimatelyDecidedOrSuperseded(
                    node, lockedRound, subject)'
               \/ LockedBodyProposalCertifiedHighExit(
                    node, lockedRound, subject)'
    <2>1. LockedBodyStableVarActionClassification
      BY <1>1, NextLockedBodyStableVarActionClassification
    <2>2. CASE UNCHANGED LockedBodyProposalStableVars
      BY <1>1, <2>2,
         StableVarsStutterPreservesLockedBodyProposalAttemptStableFrame,
         Isa
    <2>3. CASE SetGST
      BY <1>1, <2>3, Isa
         DEF LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock, SetGST
    <2>4. CASE \E assembler \in ValidatorIds, assembled \in Subjects:
                  AssembleLocalBody(assembler, assembled)
      BY <1>1, <2>4,
         AssemblePreservesLockedBodyProposalAttemptStableFrame, Isa
    <2>5. CASE \E storer \in ValidatorIds, roundView \in Views,
                    stored \in Subjects:
                  StoreBody(storer, roundView, stored)
      BY <1>1, <2>5,
         StorePreservesLockedBodyProposalAttemptStableFrame, Isa
    <2>6. CASE \E observeRequest \in pendingObservePrepare:
                  PersistObservePrepare(observeRequest)
      BY <1>1, <2>6,
         PersistObservePreservesLockedBodyProposalAttemptOrCertifiedExit,
         Isa
    <2>7. CASE \E lockRequest \in pendingLockCommit:
                  PersistLockCommit(lockRequest)
      BY <1>1, <2>7,
         PersistLockPreservesLockedBodyProposalAttemptOrSupersedes, Isa
    <2>8. CASE \E installRequest \in pendingInstallTC:
                  PersistInstallTC(installRequest)
      BY <1>1, <2>8,
         PersistInstallPreservesLockedBodyProposalAttemptOrExits, Isa
    <2>9. CASE \E crashed \in ValidatorIds: Crash(crashed)
      BY <1>1, <2>9, Isa DEF Crash
    <2>10. CASE \E restarted \in ValidatorIds: Restart(restarted)
      <3>1. PICK restarted \in ValidatorIds: Restart(restarted)
        BY <2>10
      <3>2. /\ restarted \notin up
             /\ up' = up \cup {restarted}
        BY <3>1 DEF Restart
      <3>3. up' = up
        BY <1>1
      <3>4. restarted \in up'
        BY <3>2, Isa
      <3>5. restarted \notin up'
        BY <3>2, <3>3, Isa
      <3>6. FALSE
        BY <3>4, <3>5
      <3> QED BY <3>6
    <2> QED
      BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         <2>7, <2>8, <2>9, <2>10
         DEF LockedBodyStableVarActionClassification
  <1> QED BY <1>1

THEOREM PostGstAsyncNextPreservesLockedBodyProposalAttemptOrExits ==
  \A node \in ValidatorIds, leaderView \in Views,
     lockedRound \in Views, subject \in Subjects:
    /\ StrongInductiveInvariant
    /\ LockedBodyProposalAttemptStableFrame(
         node, leaderView, lockedRound, subject)
    /\ AsyncNext
    => \/ LockedBodyProposalAttemptStableFrame(
            node, leaderView, lockedRound, subject)'
       \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
       \/ LockedBodyLegitimatelyDecidedOrSuperseded(
            node, lockedRound, subject)'
       \/ LockedBodyProposalCertifiedHighExit(
            node, lockedRound, subject)'
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW leaderView \in Views,
                NEW lockedRound \in Views,
                NEW subject \in Subjects,
                StrongInductiveInvariant,
                LockedBodyProposalAttemptStableFrame(
                  node, leaderView, lockedRound, subject),
                AsyncNext
         PROVE \/ LockedBodyProposalAttemptStableFrame(
                   node, leaderView, lockedRound, subject)'
               \/ LockedBodyProposalAttemptViewExit(node, leaderView)'
               \/ LockedBodyLegitimatelyDecidedOrSuperseded(
                    node, lockedRound, subject)'
               \/ LockedBodyProposalCertifiedHighExit(
                    node, lockedRound, subject)'
    <2>1. gst
      BY <1>1
         DEF LockedBodyProposalAttemptStableFrame,
             StableAvailableRetainedLock
    <2>2. UNCHANGED <<up, gst>>
      BY <1>1, <2>1, PostGstAsyncNextLeavesUpAndGst
    <2>3. [Next]_vars
      BY <1>1 DEF AsyncNext
    <2>4. CASE Next
      BY <1>1, <2>2, <2>4,
         NonCrashNextPreservesLockedBodyProposalAttemptOrExits
    <2>5. CASE UNCHANGED vars
      <3>1. UNCHANGED LockedBodyProposalStableVars
        BY <2>5 DEF vars, LockedBodyProposalStableVars
      <3>2. LockedBodyProposalAttemptStableFrame(
               node, leaderView, lockedRound, subject)'
        BY <1>1, <3>1,
           StableVarsStutterPreservesLockedBodyProposalAttemptStableFrame
      <3> QED BY <3>2
    <2> QED BY <2>3, <2>4, <2>5 DEF vars
  <1> QED BY <1>1

=============================================================================
