---- MODULE SumeragiV2ProgressWitnessPreservationProofs ----
EXTENDS SumeragiV2AsyncRankClosureProofs

(***************************************************************************
Inductive decomposition of the release-facing progress-witness seam.

Three conjuncts already have complete inductive proofs in the imported
modules: the recovery-aware Commit owner, durable-Decision uniqueness, and
the bounded protected-deferred owner.  The remaining preservation kernel is
kept explicit below.  In particular, this module does not promote the open
kernel to an invariant and does not assume any action-preservation fact.
***************************************************************************)

ProvedProgressWitnessKernelInvariant ==
  /\ AsyncDurableCommitProgressWitness
  /\ DecisionsUniqueByNodeContext
  /\ ProtectedDeferredProgressInvariant

OpenProgressWitnessKernelInvariant ==
  /\ HistoricalLockedCommitRecoveryProgress
  /\ AsyncDurableDecisionProgressWitness
  /\ HistoricalLockedBodyRecoveryStageInvariant

ReleaseProgressWitnessInvariant ==
  /\ AsyncProgressWitnessInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant

THEOREM ReleaseProgressWitnessInvariantDecomposition ==
  ReleaseProgressWitnessInvariant
    <=> /\ ProvedProgressWitnessKernelInvariant
        /\ OpenProgressWitnessKernelInvariant
BY DEF ReleaseProgressWitnessInvariant,
       ProvedProgressWitnessKernelInvariant,
       OpenProgressWitnessKernelInvariant,
       AsyncProgressWitnessInvariant

(***************************************************************************
The already-closed kernel really is invariant under the bare asynchronous
specification.  This theorem is intentionally assembled only from imported
proof-bearing obligations.
***************************************************************************)

THEOREM ProvedProgressWitnessKernelObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ProvedProgressWitnessKernelInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []ProvedProgressWitnessKernelInvariant
    <2>1. AsyncSpecAt(initialContext)
             => [](/\ AsyncDurableCommitProgressWitness
                    /\ ProtectedDeferredProgressInvariant)
      BY AsyncCrashAwareProgressWitnessComponentsObligation
    <2>2. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>3. DecisionFrontierUniquenessInvariant
             => DecisionsUniqueByNodeContext
      BY DEF DecisionFrontierUniquenessInvariant
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF ProvedProgressWitnessKernelInvariant
  <1> QED BY <1>1

(***************************************************************************
Consequently the ledger theorem is exactly, rather than merely approximately,
the preservation of the three-conjunct open kernel.  The implication from
right to left uses the closed kernel above; the reverse implication is a
projection of the release invariant.
***************************************************************************)

THEOREM ProgressWitnessClosureEquivalentToOpenKernel ==
  \A initialContext:
    AsyncProgressWitnessAndHistoricalRecoveryProperty(
      AsyncSpecAt(initialContext))
      <=> (AsyncSpecAt(initialContext)
             => []OpenProgressWitnessKernelInvariant)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncProgressWitnessAndHistoricalRecoveryProperty(
                   AsyncSpecAt(initialContext))
                 <=> (AsyncSpecAt(initialContext)
                        => []OpenProgressWitnessKernelInvariant)
    <2>1. AsyncSpecAt(initialContext)
             => []ProvedProgressWitnessKernelInvariant
      BY ProvedProgressWitnessKernelObligation
    <2>2. ReleaseProgressWitnessInvariant
             <=> /\ ProvedProgressWitnessKernelInvariant
                 /\ OpenProgressWitnessKernelInvariant
      BY ReleaseProgressWitnessInvariantDecomposition
    <2>3. AsyncProgressWitnessAndHistoricalRecoveryProperty(
             AsyncSpecAt(initialContext))
             <=> (AsyncSpecAt(initialContext)
                    => []ReleaseProgressWitnessInvariant)
      BY PTL
         DEF AsyncProgressWitnessAndHistoricalRecoveryProperty,
             AsyncProgressWitnessProperty,
             HistoricalLockedBodyRecoveryProperty,
             ReleaseProgressWitnessInvariant
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

(***************************************************************************
The complete release invariant has a concrete base case.  Non-genesis
initialization carries only parent-context Prepare evidence, so it cannot be a
current-context historical locked-Prepare source.  The recovery-aware Commit
and Decision witnesses include their ordinary counterparts as disjuncts.
***************************************************************************)

THEOREM ProgressWitnessWithDecisionUniquenessProjectsAsyncWitness ==
  /\ ProgressWitnessInvariant
  /\ DecisionsUniqueByNodeContext
  => AsyncProgressWitnessInvariant
BY DEF ProgressWitnessInvariant, AsyncProgressWitnessInvariant,
       AsyncDurableCommitProgressWitness,
       AsyncCommitIntentProgressWitness,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness

THEOREM AsyncInitHasNoHistoricalLockedPrepareSource ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
           ~HistoricalLockedPrepareSource(node, qc)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE \A node \in AsyncCurrentResponsiveVoters,
                   qc \in prepareQCs:
                 ~HistoricalLockedPrepareSource(node, qc)
    <2>1. /\ context = initialContext
           /\ (initialContext.height = 0 => prepareQCs = {})
           /\ (initialContext.height > 0
                 => prepareQCs =
                      {BootstrapParentPrepareQC(initialContext)})
           /\ (initialContext.height > 0
                 => BootstrapParentContext(initialContext)
                      # initialContext)
      BY <1>1, BootstrapParentContextPrecedes, SMT
         DEF AsyncInitAt, AsyncBaseInitAt, InitAt
    <2>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters,
                  NEW qc \in prepareQCs
           PROVE ~HistoricalLockedPrepareSource(node, qc)
      <3>1. CASE initialContext.height = 0
        BY <2>1, <2>2, <3>1
      <3>2. CASE initialContext.height > 0
        <4>1. qc = BootstrapParentPrepareQC(initialContext)
          BY <2>1, <2>2, <3>2
        <4>2. qc.context =
                 BootstrapParentContext(initialContext)
          BY <4>1 DEF BootstrapParentPrepareQC, QC
        <4>3. qc.context # context
          BY <2>1, <3>2, <4>2
        <4> QED BY <4>3
             DEF HistoricalLockedPrepareSource,
                 LockedPrepareRecoverySource
      <3> QED BY <3>1, <3>2, SMT
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM AsyncInitEstablishesReleaseProgressWitness ==
  \A initialContext:
    AsyncInitAt(initialContext) => ReleaseProgressWitnessInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncInitAt(initialContext)
         PROVE ReleaseProgressWitnessInvariant
    <2>1. ProgressWitnessInvariant
      BY <1>1, AsyncInitEstablishesProgressWitness
    <2>2. DecisionFrontierUniquenessInvariant
      BY <1>1, AsyncInitEstablishesDecisionFrontierUniqueness
    <2>3. DecisionsUniqueByNodeContext
      BY <2>2 DEF DecisionFrontierUniquenessInvariant
    <2>4. AsyncProgressWitnessInvariant
      BY <2>1, <2>3,
         ProgressWitnessWithDecisionUniquenessProjectsAsyncWitness
    <2>5. \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
             ~HistoricalLockedPrepareSource(node, qc)
      BY <1>1, AsyncInitHasNoHistoricalLockedPrepareSource
    <2>6. HistoricalLockedBodyRecoveryStageInvariant
      BY <2>5 DEF HistoricalLockedBodyRecoveryStageInvariant
    <2> QED BY <2>4, <2>6 DEF ReleaseProgressWitnessInvariant
  <1> QED BY <1>1

(***************************************************************************
Exact state support of the open preservation kernel.  Keeping this tuple
smaller than AsyncAllVars makes genuinely irrelevant transitions mechanically
dischargeable without claiming anything about the owner-moving actions.
***************************************************************************)

OpenProgressWitnessKernelVars ==
  <<context, nodeView, generation,
    availableBodies, durableBodies, validatedBodies,
    prepareIntents, commitIntents, prepareQCs, commitQCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingLockCommit, decisions, applied,
    asyncCommandQueues, asyncOutstandingWork,
    asyncDeferredCompletionQueues, asyncDeferredProgressQueues,
    asyncDeferredNormalQueues, asyncCausalQueues,
    asyncActiveRequests, asyncSentItems, AsyncRecoveryVars>>

(***************************************************************************
An authenticated CertifiedResponse occurrence is a persistent historical
fact: its exact signed envelope is witnessed by one concrete member of the
append-only sent history.  Publication may add other items, but cannot
invalidate that same occurrence.  The second lemma lifts this fact through
the exact historical BeginLock evidence predicate while keeping the response
type universe and archive route fixed.
***************************************************************************)

THEOREM CertifiedResponseAuthenticatedOccurrencePersistsUnderSentHistoryGrowth ==
  \A evidence:
    /\ CertifiedResponseAuthenticatedOccurrence(evidence)
    /\ asyncSentItems \subseteq asyncSentItems'
    => CertifiedResponseAuthenticatedOccurrence(evidence)'
BY Isa
   DEF CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection

THEOREM AsyncNetworkItemsStableUnderContextAndViewFrame ==
  UNCHANGED <<context, nodeView>> => UNCHANGED AsyncNetworkItems
BY Isa
   DEF AsyncNetworkItems, AsyncUntrustedTransportCompletionItem

THEOREM CertifiedArchiveRoutesStableUnderContextFrame ==
  \A node, qc:
    UNCHANGED context => UNCHANGED CertifiedArchiveRoutes(node, qc)
BY Isa
   DEF CertifiedArchiveRoutes, CurrentVoters, CurrentEpoch

THEOREM HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth ==
  \A node, qc, evidence:
    /\ HistoricalBeginLockRecoveryEvidence(node, qc, evidence)
    /\ UNCHANGED <<context, nodeView>>
    /\ asyncSentItems \subseteq asyncSentItems'
    => HistoricalBeginLockRecoveryEvidence(node, qc, evidence)'
BY AsyncNetworkItemsStableUnderContextAndViewFrame,
   CertifiedArchiveRoutesStableUnderContextFrame,
   CertifiedResponseAuthenticatedOccurrencePersistsUnderSentHistoryGrowth,
   IsaT(90)
   DEF HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection

THEOREM OpenProgressWitnessKernelFramePreservesInvariant ==
  /\ OpenProgressWitnessKernelInvariant
  /\ UNCHANGED OpenProgressWitnessKernelVars
  => OpenProgressWitnessKernelInvariant'
BY Isa
   DEF OpenProgressWitnessKernelInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource,
       LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor,
       ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness,
       DecisionCompletionWitness,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions, ActiveLockedCommitIntent,
       NodeHasApplication, CandidateScheduled,
       CandidateConsumerCurrent, DecisionPipelineCandidate,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       OpenProgressWitnessKernelVars

(***************************************************************************
Local admission does not change any semantic source, consumer epoch, active
certified request, sent-history occurrence, or recovery authority used by the
open kernel.  Service/publication actions may instead grow the two persistent
network sets.  The following narrower frame records both cases while allowing
only monotone growth of exact network witnesses and scheduled candidates.
***************************************************************************)

OpenProgressWitnessSemanticVars ==
  <<context, nodeView, generation,
    availableBodies, durableBodies, validatedBodies,
    prepareIntents, commitIntents, prepareQCs, commitQCs, installedTCs,
    lockRank, lockSubject, highestRank, highestSubject,
    pendingLockCommit, decisions, applied,
    AsyncRecoveryVars>>

ScheduledCandidateSet ==
  QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
    \cup TrackedWorkCandidates

(***************************************************************************
Separate the concrete historical locked-body carriers from controller
authority.  During Replaying this auxiliary says that every locked Prepare
source of the recovery node already has a durable Commit owner, an exact
certified request, a scheduled body pipeline owner, or a terminal outcome.
It is deliberately not added to the open kernel: its preservation across the
remaining owner-moving runtime actions is part of the explicit frontier.
***************************************************************************)

HistoricalLockedBodyNonAuthorityCarrier(node, qc) ==
  \/ HistoricalLockedCommitRecoveryWitness(node, qc)
  \/ HistoricalLockedCertifiedRequestActive(node, qc)
  \/ \E candidate \in AsyncCandidateSet:
       HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
  \/ HistoricalLockedBodyRecoveryTerminal(node, qc)

THEOREM HistoricalLockedBodyRecoveryStageDecomposition ==
  \A node, qc:
    HistoricalLockedBodyRecoveryStage(node, qc)
      <=> \/ HistoricalLockedBodyRecoveryAuthority(node, qc)
          \/ HistoricalLockedBodyNonAuthorityCarrier(node, qc)
BY Zenon
   DEF HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyNonAuthorityCarrier

ResponsiveReplayLockedBodyCarrierInvariant ==
  asyncRecoveryPhase = "Replaying"
    => \A qc \in RestartLockedPrepareQCs(asyncRecoveryNode):
         HistoricalLockedBodyNonAuthorityCarrier(asyncRecoveryNode, qc)

OpenProgressWitnessCarrierFrame ==
  /\ UNCHANGED OpenProgressWitnessSemanticVars
  /\ asyncActiveRequests \subseteq asyncActiveRequests'
  /\ asyncSentItems \subseteq asyncSentItems'
  /\ ScheduledCandidateSet \subseteq ScheduledCandidateSet'

THEOREM OpenProgressWitnessKernelFrameEstablishesCarrierFrame ==
  UNCHANGED OpenProgressWitnessKernelVars
    => OpenProgressWitnessCarrierFrame
BY Isa
   DEF OpenProgressWitnessKernelVars,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier ==
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ OpenProgressWitnessCarrierFrame
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(150)
   DEF ResponsiveReplayLockedBodyCarrierInvariant,
       HistoricalLockedBodyNonAuthorityCarrier,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       RestartLockedPrepareQCs, CertifiedRequestOutbox,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet

THEOREM OpenProgressWitnessKernelFramePreservesResponsiveReplayLockedBodyCarrier ==
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ UNCHANGED OpenProgressWitnessKernelVars
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY OpenProgressWitnessKernelFrameEstablishesCarrierFrame,
   OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier

THEOREM OpenProgressWitnessCarrierFramePreservesInvariant ==
  /\ OpenProgressWitnessKernelInvariant
  /\ OpenProgressWitnessCarrierFrame
  => OpenProgressWitnessKernelInvariant'
PROOF
  <1>1. ASSUME OpenProgressWitnessKernelInvariant,
                OpenProgressWitnessCarrierFrame
         PROVE OpenProgressWitnessKernelInvariant'
    <2>1. HistoricalLockedCommitRecoveryProgress'
      BY <1>1,
         HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
         IsaT(90)
         DEF OpenProgressWitnessKernelInvariant,
             OpenProgressWitnessCarrierFrame,
             OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
             HistoricalLockedCommitRecoveryProgress,
             HistoricalLockedCommitRecoveryWitness,
             HistoricalBeginLockRecoveryCandidate,
             HistoricalBeginLockRecoveryEvidence,
             HistoricalCertifiedResponseRecoveryEvidence,
             HistoricalLockedPrepareForCommit,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
             HistoricalLockedPrepareRecoveryProvenance,
             InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
             NoHigherConflictingPrepareKnown,
             CandidateScheduled, QueuedCandidates, DeferredCandidates,
             CausalCandidates, TrackedWorkCandidates, SequenceSet,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>2. AsyncDurableDecisionProgressWitness'
      BY <1>1, IsaT(120)
         DEF OpenProgressWitnessKernelInvariant,
             OpenProgressWitnessCarrierFrame,
             OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
             AsyncDurableDecisionProgressWitness,
             AsyncDecisionCompletionWitness, DecisionCompletionWitness,
             DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
             DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
             DecisionPipelineCandidate, CandidateConsumerCurrent,
             NodeHasApplication, CandidateScheduled,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2>3. HistoricalLockedBodyRecoveryStageInvariant'
      BY <1>1,
         HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
         IsaT(150)
         DEF OpenProgressWitnessKernelInvariant,
             OpenProgressWitnessCarrierFrame,
             OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
             HistoricalLockedBodyRecoveryStageInvariant,
             HistoricalLockedBodyRecoveryStage,
             HistoricalLockedBodyRecoveryAuthority,
             HistoricalLockedCertifiedRequestActive,
             HistoricalLockedBodyPipelineCandidate,
             HistoricalLockedBodyRecoveryTerminal,
             HistoricalLockedCommitRecoveryWitness,
             HistoricalBeginLockRecoveryCandidate,
             HistoricalBeginLockRecoveryEvidence,
             HistoricalCertifiedResponseRecoveryEvidence,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
             HistoricalLockedPrepareRecoveryProvenance,
             InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
             NoHigherConflictingPrepareKnown,
             CandidateConsumerCurrent, CandidateScheduled,
             QueuedCandidates, DeferredCandidates, CausalCandidates,
             TrackedWorkCandidates, SequenceSet,
             AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch
    <2> QED BY <2>1, <2>2, <2>3
         DEF OpenProgressWitnessKernelInvariant
  <1> QED BY <1>1

(***************************************************************************
Each local-admission arm preserves the set of scheduled logical candidates.
The ownership invariant makes the causal head fresh with respect to the FIFO,
deferred, and tracked carriers.  Producer admission performs the dual move:
the selected ready completion is already tracked and is appended to the FIFO
before its executor ownership is retired.
***************************************************************************)

THEOREM LocalPhaseAdvancePreservesScheduledCandidateSet ==
  \A node \in ValidatorIds:
    /\ LocalAdmissionStep(node)
    /\ ~LocalAdmissionCanAdvance(node)
    => ScheduledCandidateSet' = ScheduledCandidateSet
BY IsaT(60)
   DEF ScheduledCandidateSet, LocalAdmissionStep, LeaveCausalQueues,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars

THEOREM ProducerAdmissionPreservesScheduledCandidateSet ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Producer"
    => ScheduledCandidateSet' = ScheduledCandidateSet
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Producer"
         PROVE ScheduledCandidateSet' = ScheduledCandidateSet
    <2> DEFINE Candidate == SelectedCompletionCandidate(node)
    <2> DEFINE Selected == ProducerSelectedReadyQueue(node)
    <2> DEFINE Other == ProducerOtherReadyQueue(node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
           /\ AsyncIoWorkContentTypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant,
             AsyncIoContentTypeInvariant
    <2>2. /\ ProducerCompletionCanAdmit(node)
           /\ AdmitProducerCompletion(node)
           /\ LeaveCausalQueues
      BY <1>1, SelectedProducerCanAdmit
         DEF SelectedLocalAdmissionAdvance
    <2>3. /\ Candidate \in asyncOutstandingWork[node]
           /\ Candidate \in TrackedWorkCandidates
           /\ DOMAIN asyncOutstandingWork = ValidatorIds
           /\ asyncCommandQueues[node] \in
                Seq(Range(asyncCommandQueues[node]))
           /\ \A owner \in ValidatorIds:
                \A candidate \in asyncOutstandingWork[owner]:
                  candidate.node = owner
      <3>1. /\ AsyncCompletionSequenceTyped(Selected)
             /\ Len(Selected) = Cardinality(SequenceSet(Selected))
             /\ Len(Selected) > 0
             /\ Candidate = Head(Selected)
             /\ Candidate \in SequenceSet(Selected)
             /\ Candidate \in asyncOutstandingWork[node]
             /\ Candidate \notin SequenceSet(Other)
        BY <2>1, <2>2, ProducerSelectedCompletionFacts
           DEF Candidate, Selected, Other
      <3>2. Candidate \in TrackedWorkCandidates
        BY <3>1 DEF TrackedWorkCandidates
      <3>3. /\ DOMAIN asyncOutstandingWork = ValidatorIds
             /\ asyncCommandQueues[node] \in
                  Seq(Range(asyncCommandQueues[node]))
             /\ \A owner \in ValidatorIds:
                  \A candidate \in asyncOutstandingWork[owner]:
                    candidate.node = owner
        BY <2>1
           DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped,
               AsyncIoTopologyTypeInvariant,
               AsyncIoWorkContentTypeInvariant
      <3> QED BY <3>1, <3>2, <3>3
    <2>4. /\ asyncCommandQueues' =
                [asyncCommandQueues EXCEPT
                   ![node] = Append(@, Candidate)]
           /\ asyncOutstandingWork' =
                [asyncOutstandingWork EXCEPT
                   ![node] = @ \ {Candidate}]
           /\ UNCHANGED
                <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues, asyncCausalQueues>>
      BY <2>2, Isa
         DEF AdmitProducerCompletion, EnqueueCandidate,
             Candidate, AsyncDeferredVars
    <2>5. /\ QueuedCandidates' =
                  QueuedCandidates \cup {Candidate}
           /\ TrackedWorkCandidates' =
                  TrackedWorkCandidates \ {Candidate}
           /\ DeferredCandidates' = DeferredCandidates
           /\ CausalCandidates' = CausalCandidates
      <3>1. QueuedCandidates' =
               QueuedCandidates \cup {Candidate}
        BY <1>1, <2>3, <2>4,
           UnionOfSequenceSetsAfterAppendAtKey
           DEF AsyncRuntimeScalarTypeInvariant, QueuedCandidates
      <3>2. TrackedWorkCandidates' =
               TrackedWorkCandidates \ {Candidate}
        BY <2>3, <2>4, UnionOfOwnedSetsAfterRemoveAtKey
           DEF TrackedWorkCandidates
      <3>3. /\ DeferredCandidates' = DeferredCandidates
             /\ CausalCandidates' = CausalCandidates
        BY <2>4, Isa
           DEF DeferredCandidates, CausalCandidates, SequenceSet
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>3, <2>5, Isa DEF ScheduledCandidateSet
  <1> QED BY <1>1

THEOREM CausalAdmissionPreservesScheduledCandidateSet ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    /\ LocalAdmissionCanAdvance(node)
    /\ SelectedLocalSource(node) = "Causal"
    => ScheduledCandidateSet' = ScheduledCandidateSet
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SelectedLocalAdmissionAdvance(node),
                LocalAdmissionCanAdvance(node),
                SelectedLocalSource(node) = "Causal"
         PROVE ScheduledCandidateSet' = ScheduledCandidateSet
    <2> DEFINE Candidate == HeadCausalCandidate(node)
    <2>1. /\ AsyncRuntimeScalarTypeInvariant
           /\ AsyncCausalTypeInvariant
           /\ AsyncIoTopologyTypeInvariant
      BY <1>1
         DEF AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
             AsyncRuntimeTypeInvariant, AsyncIoTypeInvariant
    <2>2. /\ CausalHeadCanAdvance(node)
           /\ CausalQueueNonempty(node)
           /\ AdmitCausalHead(node)
      <3>1. CausalHeadCanAdvance(node)
        BY <1>1, SelectedCausalCanAdvance
      <3>2. CausalQueueNonempty(node)
        BY <3>1 DEF CausalHeadCanAdvance
      <3>3. AdmitCausalHead(node)
        BY <1>1, <3>1, Isa DEF SelectedLocalAdmissionAdvance
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. /\ Candidate.node = node
           /\ Candidate \in SequenceSet(asyncCausalQueues[node])
           /\ Candidate \in CausalCandidates
           /\ Candidate \notin QueuedCandidates
           /\ Candidate \notin DeferredCandidates
           /\ Candidate \notin TrackedWorkCandidates
           /\ ~CandidateInFlight(Candidate)
      <3>1. Candidate.node = node
        BY <2>1, <2>2, CausalHeadCandidateIsOwned DEF Candidate
      <3>2. Candidate \in SequenceSet(asyncCausalQueues[node])
        <4>1. /\ asyncCausalQueues[node] \in
                       Seq(Range(asyncCausalQueues[node]))
               /\ Len(asyncCausalQueues[node]) > 0
          BY <2>1, <2>2
             DEF AsyncCausalTypeInvariant, AsyncQueueTyped,
                 CausalQueueNonempty
        <4>2. asyncCausalQueues[node] # <<>>
          BY <4>1, EmptySeq, SMT
        <4>3. Head(asyncCausalQueues[node]) \in
                 Range(asyncCausalQueues[node])
          BY <4>1, <4>2, HeadTailProperties
        <4>4. SequenceSet(asyncCausalQueues[node]) =
                 Range(asyncCausalQueues[node])
          BY <4>1, RangeEquality DEF SequenceSet
        <4> QED BY <4>3, <4>4
             DEF Candidate, HeadCausalCandidate
      <3>3. Candidate \in CausalCandidates
        BY <3>2 DEF CausalCandidates
      <3>4. /\ Candidate \notin QueuedCandidates
             /\ Candidate \notin DeferredCandidates
             /\ Candidate \notin TrackedWorkCandidates
        BY <1>1, <3>3, Isa
           DEF AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant
      <3>5. ~CandidateInFlight(Candidate)
        BY <3>4 DEF CandidateInFlight
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
    <2>4. /\ SequenceSet(Tail(asyncCausalQueues[node])) =
                  SequenceSet(asyncCausalQueues[node]) \ {Candidate}
           /\ SequenceHasUniqueValues(Tail(asyncCausalQueues[node]))
      <3>1. /\ asyncCausalQueues[node] \in
                       Seq(Range(asyncCausalQueues[node]))
             /\ SequenceHasUniqueValues(asyncCausalQueues[node])
             /\ Len(asyncCausalQueues[node]) > 0
        BY <1>1, <2>1, <2>2
           DEF AsyncCausalTypeInvariant,
               AsyncProgressOwnershipInvariant,
               AsyncLogicalCandidateOwnershipInvariant,
               AsyncQueueTyped, CausalQueueNonempty
      <3>2. /\ SequenceSet(Tail(asyncCausalQueues[node])) =
                    SequenceSet(asyncCausalQueues[node])
                      \ {Head(asyncCausalQueues[node])}
             /\ SequenceHasUniqueValues(
                  Tail(asyncCausalQueues[node]))
        BY <3>1, UniqueSequenceTailSetFacts
      <3> QED BY <3>2 DEF Candidate, HeadCausalCandidate
    <2>5. /\ asyncCausalQueues' =
                [asyncCausalQueues EXCEPT ![node] = Tail(@)]
           /\ UNCHANGED
                <<asyncDeferredCompletionQueues,
                  asyncDeferredProgressQueues,
                  asyncDeferredNormalQueues>>
      <3>1. asyncCausalQueues' =
               [asyncCausalQueues EXCEPT ![node] = Tail(@)]
        BY <2>2 DEF AdmitCausalHead
      <3>2. UNCHANGED
               <<asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues>>
        BY <1>1 DEF SelectedLocalAdmissionAdvance, AsyncDeferredVars
      <3> QED BY <3>1, <3>2
    <2>6. CausalCandidates' = CausalCandidates \ {Candidate}
      <3>1. /\ DOMAIN asyncCausalQueues = ValidatorIds
             /\ Candidate.node = node
             /\ \A owner \in ValidatorIds:
                  \A candidate \in
                       SequenceSet(asyncCausalQueues[owner]):
                    candidate.node = owner
        BY <2>1, <2>3
           DEF AsyncCausalTypeInvariant,
               AsyncCausalQueueOwnership
      <3>2. UNION
               {SequenceSet(asyncCausalQueues'[owner]):
                  owner \in ValidatorIds} =
               (UNION
                  {SequenceSet(asyncCausalQueues[owner]):
                     owner \in ValidatorIds})
                 \ {Candidate}
        BY <1>1, <2>4, <2>5, <3>1,
           UnionOfSequenceSetsAfterTailAtKey
      <3> QED BY <3>2 DEF CausalCandidates
    <2>7. CASE Candidate.class = "Completion"
      <3>1. /\ asyncCommandQueues' = asyncCommandQueues
             /\ asyncOutstandingWork' =
                  [asyncOutstandingWork EXCEPT
                     ![node] = @ \cup {Candidate}]
        BY <2>2, <2>3, <2>7, Isa
           DEF AdmitCausalHead, Candidate
      <3>2. /\ QueuedCandidates' = QueuedCandidates
             /\ DeferredCandidates' = DeferredCandidates
             /\ TrackedWorkCandidates' =
                  TrackedWorkCandidates \cup {Candidate}
        <4>1. QueuedCandidates' = QueuedCandidates
          BY <3>1 DEF QueuedCandidates
        <4>2. DeferredCandidates' = DeferredCandidates
          BY <2>5 DEF DeferredCandidates
        <4>3. TrackedWorkCandidates' =
                 TrackedWorkCandidates \cup {Candidate}
          <5>1. DOMAIN asyncOutstandingWork = ValidatorIds
            BY <2>1 DEF AsyncIoTopologyTypeInvariant
          <5>2. UNION
                   {asyncOutstandingWork'[owner]:
                      owner \in ValidatorIds} =
                   (UNION
                      {asyncOutstandingWork[owner]:
                         owner \in ValidatorIds})
                     \cup {Candidate}
            BY <1>1, <3>1, <5>1, UnionOfSetsAfterAddAtKey
          <5> QED BY <5>2 DEF TrackedWorkCandidates
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <2>3, <2>6, <3>2, Isa
           DEF ScheduledCandidateSet
    <2>8. CASE Candidate.class # "Completion"
      <3>1. /\ asyncCommandQueues' =
                  [asyncCommandQueues EXCEPT
                     ![node] = Append(@, Candidate)]
             /\ UNCHANGED AsyncIoVars
        BY <2>2, <2>3, <2>8, Isa
           DEF AdmitCausalHead, Candidate, EnqueueCandidate
      <3>2. /\ QueuedCandidates' =
                    QueuedCandidates \cup {Candidate}
             /\ DeferredCandidates' = DeferredCandidates
             /\ TrackedWorkCandidates' = TrackedWorkCandidates
        <4>1. QueuedCandidates' =
                 QueuedCandidates \cup {Candidate}
          <5>1. /\ DOMAIN asyncCommandQueues = ValidatorIds
                 /\ asyncCommandQueues[node] \in
                      Seq(Range(asyncCommandQueues[node]))
            BY <2>1
               DEF AsyncRuntimeScalarTypeInvariant, AsyncQueueTyped
          <5>2. UNION
                   {SequenceSet(asyncCommandQueues'[owner]):
                      owner \in ValidatorIds} =
                   (UNION
                      {SequenceSet(asyncCommandQueues[owner]):
                         owner \in ValidatorIds})
                     \cup {Candidate}
            BY <1>1, <3>1, <5>1,
               UnionOfSequenceSetsAfterAppendAtKey
          <5> QED BY <5>2 DEF QueuedCandidates
        <4>2. DeferredCandidates' = DeferredCandidates
          BY <2>5 DEF DeferredCandidates
        <4>3. TrackedWorkCandidates' = TrackedWorkCandidates
          BY <3>1 DEF AsyncIoVars, TrackedWorkCandidates
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <2>3, <2>6, <3>2, Isa
           DEF ScheduledCandidateSet
    <2> QED BY <2>7, <2>8
  <1> QED BY <1>1

THEOREM LocalAdmissionPreservesScheduledCandidateSet ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ LocalAdmissionStep(node)
    => ScheduledCandidateSet' = ScheduledCandidateSet
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                LocalAdmissionStep(node)
         PROVE ScheduledCandidateSet' = ScheduledCandidateSet
    <2>1. CASE ~LocalAdmissionCanAdvance(node)
      BY <1>1, <2>1,
         LocalPhaseAdvancePreservesScheduledCandidateSet
    <2>2. CASE LocalAdmissionCanAdvance(node)
               /\ SelectedLocalSource(node) = "Producer"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>2, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>2, <3>1,
           ProducerAdmissionPreservesScheduledCandidateSet
    <2>3. CASE LocalAdmissionCanAdvance(node)
               /\ SelectedLocalSource(node) = "Causal"
      <3>1. SelectedLocalAdmissionAdvance(node)
        BY <1>1, <2>3, LocalAdmissionAdvanceSelectsAtomicWork
      <3> QED BY <1>1, <2>3, <3>1,
           CausalAdmissionPreservesScheduledCandidateSet
    <2> QED BY <2>1, <2>2, <2>3, Isa
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource, AsyncLocalSources
  <1> QED BY <1>1

THEOREM SelectedLocalAdmissionAdvancePreservesScheduledCandidateSet ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ SelectedLocalAdmissionAdvance(node)
    => ScheduledCandidateSet' = ScheduledCandidateSet
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                SelectedLocalAdmissionAdvance(node)
         PROVE ScheduledCandidateSet' = ScheduledCandidateSet
    <2>1. LocalAdmissionCanAdvance(node)
      BY <1>1 DEF SelectedLocalAdmissionAdvance
    <2>2. CASE SelectedLocalSource(node) = "Producer"
      BY <1>1, <2>1, <2>2,
         ProducerAdmissionPreservesScheduledCandidateSet
    <2>3. CASE SelectedLocalSource(node) = "Causal"
      BY <1>1, <2>1, <2>3,
         CausalAdmissionPreservesScheduledCandidateSet
    <2> QED BY <2>2, <2>3, Isa
         DEF SelectedLocalSource, PreferredLocalSource,
             OtherLocalSource, AsyncLocalSources
  <1> QED BY <1>1

THEOREM LocalAdmissionStepEstablishesOpenProgressWitnessCarrierFrame ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ LocalAdmissionStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessCarrierFrame
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                AsyncStrongTypeInvariant,
                AsyncProgressOwnershipInvariant,
                LocalAdmissionStep(node),
                UNCHANGED AsyncRecoveryVars
         PROVE OpenProgressWitnessCarrierFrame
    <2>1. UNCHANGED OpenProgressWitnessSemanticVars
      BY <1>1, IsaT(90)
         DEF OpenProgressWitnessSemanticVars, LocalAdmissionStep,
             AdmitProducerCompletion, AdmitCausalHead,
             LeaveCausalQueues, AsyncIoVars, AsyncDeferredVars,
             AsyncLocalAdmissionVars, vars
    <2>2. ScheduledCandidateSet' = ScheduledCandidateSet
      BY <1>1, LocalAdmissionPreservesScheduledCandidateSet
    <2>3. asyncActiveRequests \subseteq asyncActiveRequests'
      BY <1>1, Isa DEF LocalAdmissionStep, AdmitProducerCompletion,
                            AdmitCausalHead, LeaveCausalQueues,
                            AsyncIoVars, AsyncDeferredVars,
                            AsyncLocalAdmissionVars, vars
    <2>4. asyncSentItems \subseteq asyncSentItems'
      BY <1>1, Isa DEF LocalAdmissionStep, AdmitProducerCompletion,
                            AdmitCausalHead, LeaveCausalQueues,
                            AsyncIoVars, AsyncDeferredVars,
                            AsyncLocalAdmissionVars, vars
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF OpenProgressWitnessCarrierFrame
  <1> QED BY <1>1

THEOREM LocalAdmissionStepPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ OpenProgressWitnessKernelInvariant
    /\ LocalAdmissionStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY LocalAdmissionStepEstablishesOpenProgressWitnessCarrierFrame,
   OpenProgressWitnessCarrierFramePreservesInvariant

THEOREM LocalAdmissionStepPreservesResponsiveReplayLockedBodyCarrier ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ResponsiveReplayLockedBodyCarrierInvariant
    /\ LocalAdmissionStep(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyCarrierInvariant'
BY LocalAdmissionStepEstablishesOpenProgressWitnessCarrierFrame,
   OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier

THEOREM AsyncAllVarsStutterPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ UNCHANGED AsyncAllVars
  => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF OpenProgressWitnessKernelVars, AsyncAllVars,
       AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncSetGstPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ AsyncSetGST
  => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF AsyncSetGST, SetGST, OpenProgressWitnessKernelVars,
       AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncTickPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ AsyncTick
  => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF AsyncTick, AsyncNonClockVars, AsyncIoVars, AsyncDeferredVars,
       AsyncLocalAdmissionVars, OpenProgressWitnessKernelVars,
       AsyncRecoveryVars, vars

THEOREM AdmitIngressPacketPreservesOpenProgressWitnessKernel ==
  \A recipient \in ValidatorIds, source \in AsyncIngressSources:
    /\ OpenProgressWitnessKernelInvariant
    /\ AdmitIngressPacket(recipient, source)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars, OpenProgressWitnessKernelVars,
       AsyncRecoveryVars, vars

THEOREM OpenHistoricalRecoveryPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ OpenHistoricalRecovery(node)
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF OpenHistoricalRecovery, OpenProgressWitnessKernelVars,
       AsyncSchedulerExceptHistoricalRecoveryTargets, vars

(***************************************************************************
The worker, direct-publication, ingress-admission, and transport inner actions
do not bind the recovery controller by themselves.  Their preservation
theorems therefore require the exact `UNCHANGED AsyncRecoveryVars` frame which
`AsyncNonCrashStep` supplies in the executable relation.  Omitting that premise
would let an unconstrained primed recovery phase destroy an authority witness.
***************************************************************************)

THEOREM ServiceIoWorkerPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ ServiceIoWorkerWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessCarrierFramePreservesInvariant, Isa
   DEF OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       ServiceIoWorkerWork, PublishEphemeralItems,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM ServiceIoWorkerPreservesResponsiveReplayLockedBodyCarrier ==
  \A node \in ValidatorIds:
    /\ ResponsiveReplayLockedBodyCarrierInvariant
    /\ ServiceIoWorkerWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyCarrierInvariant'
BY OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier,
   Isa
   DEF OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       ServiceIoWorkerWork, PublishEphemeralItems,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM ResponsiveReplayServiceIoWorkerPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ ResponsiveReplayServiceIoWorker
  => OpenProgressWitnessKernelInvariant'
BY ServiceIoWorkerPreservesOpenProgressWitnessKernel, Isa
   DEF ResponsiveReplayServiceIoWorker, ServiceIoWorker,
       AsyncNonRunnerOuterFrame, AsyncNonCrashOuterFrame

THEOREM ResponsiveReplayServiceIoWorkerPreservesLockedBodyCarrier ==
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ ResponsiveReplayServiceIoWorker
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY ServiceIoWorkerPreservesResponsiveReplayLockedBodyCarrier, Isa
   DEF ResponsiveReplayServiceIoWorker, ServiceIoWorker,
       AsyncNonRunnerOuterFrame, AsyncNonCrashOuterFrame

THEOREM EnqueueIoControlPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ EnqueueIoLocalControlWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF EnqueueIoLocalControlWork, OpenProgressWitnessKernelVars,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM RunHistoricalServerPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ RunHistoricalServer(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessKernelFramePreservesInvariant, Isa
   DEF RunHistoricalServer, DrainHistoricalIngressSelected,
       HistoricalIdleStep, PopSelectedIngress,
       OpenProgressWitnessKernelVars, AsyncIoVars,
       AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM CommitCertificateDiscoveryPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ CommitCertificateDiscoveryStepWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessCarrierFramePreservesInvariant, Isa
   DEF OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       CommitCertificateDiscoveryStepWork,
       PublishCommitCertificateRequests,
       CommitCertificateRequestOutbox,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

THEOREM CommitCertificateDiscoveryPreservesResponsiveReplayLockedBodyCarrier ==
  \A node \in ValidatorIds:
    /\ ResponsiveReplayLockedBodyCarrierInvariant
    /\ CommitCertificateDiscoveryStepWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveReplayLockedBodyCarrierInvariant'
BY OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier,
   Isa
   DEF OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       CommitCertificateDiscoveryStepWork,
       PublishCommitCertificateRequests,
       CommitCertificateRequestOutbox,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars, vars

TransportOnlyProgressWitnessStep ==
  \/ \E packet: PreGstLosePacket(packet)
  \/ \E source, recipient, nonce:
       InjectByzantineNoise(source, recipient, nonce)
  \/ \E kind, recipient, nonce:
       InjectUntrustedTransportCompletion(kind, recipient, nonce)
  \/ \E kind, source, recipient, nonce:
       InjectAuthenticatedJunk(kind, source, recipient, nonce)
  \/ \E source, recipient, qc, nonce:
       InjectByzantineCertifiedRequest(source, recipient, qc, nonce)
  \/ \E signer, roundView, subject, timeoutCertificate, highestPrepare:
       AsyncByzantineProposal(
         signer, roundView, subject,
         timeoutCertificate, highestPrepare)
  \/ \E signer, roundView, phase, subject:
       AsyncByzantineVote(signer, roundView, phase, subject)
  \/ \E signer, roundView, highestPrepare:
       AsyncByzantineTimeout(signer, roundView, highestPrepare)

THEOREM TransportOnlyStepPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ TransportOnlyProgressWitnessStep
  /\ UNCHANGED AsyncRecoveryVars
  => OpenProgressWitnessKernelInvariant'
BY OpenProgressWitnessCarrierFramePreservesInvariant, Isa
   DEF TransportOnlyProgressWitnessStep,
       PreGstLosePacket, InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, ByzantineBroadcastProposal,
       ByzantineBroadcastVote, ByzantineBroadcastTimeout,
       PublishEphemeralItems, PacketsForItems, NoSendItem,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars,
       AsyncAuxVars, AsyncRecoveryVars, vars

THEOREM TransportOnlyStepPreservesResponsiveReplayLockedBodyCarrier ==
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ TransportOnlyProgressWitnessStep
  /\ UNCHANGED AsyncRecoveryVars
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY OpenProgressWitnessCarrierFramePreservesResponsiveReplayLockedBodyCarrier,
   Isa
   DEF TransportOnlyProgressWitnessStep,
       PreGstLosePacket, InjectByzantineNoise,
       InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout, ByzantineBroadcastProposal,
       ByzantineBroadcastVote, ByzantineBroadcastTimeout,
       PublishEphemeralItems, PacketsForItems, NoSendItem,
       OpenProgressWitnessCarrierFrame,
       OpenProgressWitnessSemanticVars, ScheduledCandidateSet,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncIoVars, AsyncDeferredVars, LeaveCausalQueues,
       AsyncLocalAdmissionVars,
       AsyncAuxVars, AsyncRecoveryVars, vars

THEOREM NonResponsiveCrashPreservesOpenProgressWitnessKernel ==
  \A node \in ValidatorIds:
    /\ OpenProgressWitnessKernelInvariant
    /\ PreGstCrash(node)
    => OpenProgressWitnessKernelInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(120)
   DEF OpenProgressWitnessKernelInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource,
       LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor,
       ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness,
       DecisionCompletionWitness,
       DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent,
       RestartDecisions, NodeHasApplication,
       CandidateScheduled, CandidateConsumerCurrent,
       DecisionPipelineCandidate,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstCrash, Crash, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM AsyncFaultStepPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ AsyncFaultStep
  /\ UNCHANGED AsyncRecoveryVars
  => OpenProgressWitnessKernelInvariant'
PROOF
  <1>1. ASSUME OpenProgressWitnessKernelInvariant,
                AsyncFaultStep,
                UNCHANGED AsyncRecoveryVars
         PROVE OpenProgressWitnessKernelInvariant'
    <2>1. CASE \E node \in ValidatorIds: PreGstCrash(node)
      BY <1>1, <2>1,
         NonResponsiveCrashPreservesOpenProgressWitnessKernel
    <2>2. CASE TransportOnlyProgressWitnessStep
      BY <1>1, <2>2,
         TransportOnlyStepPreservesOpenProgressWitnessKernel
    <2> QED BY <1>1, <2>1, <2>2
         DEF AsyncFaultStep, TransportOnlyProgressWitnessStep
  <1> QED BY <1>1

THEOREM AsyncNonRunnerStepPreservesOpenProgressWitnessKernel ==
  /\ OpenProgressWitnessKernelInvariant
  /\ AsyncNonRunnerStep
  /\ UNCHANGED AsyncRecoveryVars
  => OpenProgressWitnessKernelInvariant'
PROOF
  <1>1. ASSUME OpenProgressWitnessKernelInvariant,
                AsyncNonRunnerStep,
                UNCHANGED AsyncRecoveryVars
         PROVE OpenProgressWitnessKernelInvariant'
    <2>1. CASE AsyncSetGST
      BY <1>1, <2>1,
         AsyncSetGstPreservesOpenProgressWitnessKernel
    <2>2. CASE AsyncTick
      BY <1>1, <2>2,
         AsyncTickPreservesOpenProgressWitnessKernel
    <2>3. CASE \E node \in ValidatorIds:
                    OpenHistoricalRecovery(node)
      BY <1>1, <2>3,
         OpenHistoricalRecoveryPreservesOpenProgressWitnessKernel
    <2>4. CASE \E node \in AsyncCurrentResponsiveVoters:
                    DirectCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>4,
         CommitCertificateDiscoveryPreservesOpenProgressWitnessKernel
         DEF DirectCommitCertificateDiscoveryStep
    <2>5. CASE \E node \in asyncHistoricalRecoveryTargets:
                    DirectHistoricalCommitCertificateDiscoveryStep(node)
      BY <1>1, <2>5,
         CommitCertificateDiscoveryPreservesOpenProgressWitnessKernel
         DEF DirectHistoricalCommitCertificateDiscoveryStep
    <2>6. CASE \E node \in AsyncCurrentResponsiveVoters:
                    ServiceIoWorker(node)
      BY <1>1, <2>6,
         ServiceIoWorkerPreservesOpenProgressWitnessKernel
         DEF ServiceIoWorker
    <2>7. CASE \E node \in asyncHistoricalRecoveryTargets:
                    ServiceHistoricalRecoveryIoWorker(node)
      BY <1>1, <2>7,
         ServiceIoWorkerPreservesOpenProgressWitnessKernel
         DEF ServiceHistoricalRecoveryIoWorker
    <2>8. CASE \E node \in AsyncCurrentResponsiveVoters:
                    EnqueueIoLocalControl(node)
      BY <1>1, <2>8,
         EnqueueIoControlPreservesOpenProgressWitnessKernel
         DEF EnqueueIoLocalControl
    <2>9. CASE \E node \in asyncHistoricalRecoveryTargets:
                    EnqueueHistoricalRecoveryIoLocalControl(node)
      BY <1>1, <2>9,
         EnqueueIoControlPreservesOpenProgressWitnessKernel
         DEF EnqueueHistoricalRecoveryIoLocalControl
    <2>10. CASE AsyncNetworkStep
      BY <1>1, <2>10,
         AdmitIngressPacketPreservesOpenProgressWitnessKernel
         DEF AsyncNetworkStep
    <2>11. CASE AsyncFaultStep
      BY <1>1, <2>11,
         AsyncFaultStepPreservesOpenProgressWitnessKernel
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
                <2>7, <2>8, <2>9, <2>10, <2>11
         DEF AsyncNonRunnerStep
  <1> QED BY <1>1

(***************************************************************************
Reachable volatile-evidence clearing for responsive recovery.

Crash removes every validation receipt owned by the selected process.
Restart frames the resulting empty slice.  ReplayRequired is quarantined, so
the recovered process cannot run an ordinary ValidateBody command before the
one atomic ResetNodeSchedulerForRestart transition.  Replaying is
deliberately absent from this invariant: the locked-body replay corridor is
allowed to create a fresh current-generation validation while it drains.
***************************************************************************)

RecoveryNodeValidationCleared(node) ==
  \A validation \in validatedBodies:
    validation.node # node

ResponsiveRecoveryValidationClearedInvariant ==
  asyncRecoveryPhase \in {"RestartRequired", "ReplayRequired"}
    => RecoveryNodeValidationCleared(asyncRecoveryNode)

NewValidationOwnedBy(node) ==
  \A validation \in validatedBodies' \ validatedBodies:
    validation.node = node

(***************************************************************************
Only the ValidateBody reducer branch can grow validatedBodies, and all three
successful validation actions construct the new record for command.node.
The remaining command branches either frame or shrink the carrier.
***************************************************************************)

THEOREM ExecuteCommandCreatesValidationOnlyForCommandOwner ==
  \A command:
    ExecuteCommand(command) => NewValidationOwnedBy(command.node)
BY ValidationCommandSelectsValidationAction, IsaT(180)
   DEF NewValidationOwnedBy, ExecuteCommand, ExecuteRegularCommand,
       RegularCoreCommand, CommandMatches,
       ValidateBody, ValidateDecidedBody, ValidateLockedBody, RejectBody,
       AssembleLocalBody, BeginLocalProposal, PersistProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       BeginPrepare, PersistPrepare, BeginObservePrepare,
       PersistObservePrepare, BeginLockCommit, PersistLockCommit,
       FormCommitQC, BeginDecision, PersistTimeout, FormTC,
       BeginInstallTC, FetchCertifiedBody,
       AcceptCertifiedResponseCapability, InstallCertifiedBodyEffect,
       ExecuteDecisionFetch, ExecuteSignProposal, ExecuteSignVote,
       ExecuteFormPrepareQC, ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       CompleteProposalSignature, CompleteVoteSignature,
       CompleteTimeoutSignature, FormPrepareQC, PersistInstallTC,
       PersistDecision, ApplyDecision, DeliverProposal, DeliverVote,
       DeliverQC, DeliverTimeout, DeliverTC, vars

THEOREM FifoRuntimeCreatesValidationOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ FifoRuntimeStep(node)
    => NewValidationOwnedBy(node)
BY RuntimeSelectedCommandsAreTyped,
   ExecuteCommandCreatesValidationOnlyForCommandOwner,
   AsyncStrongTypeProjectsAsyncType, IsaT(90)
   DEF NewValidationOwnedBy, FifoRuntimeStep,
       DeferCommand, DiscardCommand, vars

THEOREM DeferredDrainCreatesValidationOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ DeferredDrainStep(node)
    => NewValidationOwnedBy(node)
BY RuntimeSelectedCommandsAreTyped,
   ExecuteCommandCreatesValidationOnlyForCommandOwner,
   AsyncStrongTypeProjectsAsyncType, IsaT(90)
   DEF NewValidationOwnedBy, DeferredDrainStep,
       DiscardCommand, vars

THEOREM RuntimeStepCreatesValidationOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ RuntimeStep(node)
    => NewValidationOwnedBy(node)
BY FifoRuntimeCreatesValidationOnlyForRunner,
   DeferredDrainCreatesValidationOnlyForRunner, IsaT(120)
   DEF NewValidationOwnedBy, RuntimeStep,
       DeferredTagStep, DeferredTimeoutStep, DeferredRetransmitStep,
       DirectTimeoutStep, DirectRetransmitStep, IdleRuntimeStep,
       BeginTimeout, vars

THEOREM RunNodeWorkCreatesValidationOnlyForRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ RunNodeWork(node)
    => NewValidationOwnedBy(node)
BY RuntimeStepCreatesValidationOnlyForRunner, IsaT(90)
   DEF NewValidationOwnedBy, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       AdmitProducerCompletion, AdmitCausalHead,
       DrainFairIngressSelected, vars

(***************************************************************************
The explicit up fact distinguishes the two cleared phases.  In
RestartRequired the recovery node is down, contradicting RunNodeWork's
node-in-up guard.  In ReplayRequired it is up but quarantined, while only the
Replaying phase satisfies ResponsiveReplayDraining.
***************************************************************************)

THEOREM ClearedRecoveryPhaseExcludesRecoveryNodeRunner ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ asyncRecoveryPhase \in {"RestartRequired", "ReplayRequired"}
    /\ node \in up
    /\ RunNodeWork(node)
    => node # asyncRecoveryNode
BY IsaT(60)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       RunNodeWork, ResponsiveReplayQuarantined,
       ResponsiveReplayDraining

THEOREM RunNodeWorkPreservesRecoveryValidationClearing ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ node \in up
    /\ RunNodeWork(node)
    /\ UNCHANGED AsyncRecoveryVars
    => ResponsiveRecoveryValidationClearedInvariant'
BY RunNodeWorkCreatesValidationOnlyForRunner,
   ClearedRecoveryPhaseExcludesRecoveryNodeRunner, IsaT(90)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared, NewValidationOwnedBy,
       AsyncRecoveryVars

THEOREM AsyncNonRunnerStepDoesNotCreateValidation ==
  AsyncNonRunnerStep => validatedBodies' \subseteq validatedBodies
BY IsaT(150)
   DEF AsyncNonRunnerStep, AsyncSetGST, SetGST, AsyncTick,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       EnqueueIoLocalControlWork, AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket, AsyncFaultStep,
       PreGstLosePacket, PreGstCrash, Crash,
       InjectByzantineNoise, InjectUntrustedTransportCompletion,
       InjectAuthenticatedJunk, InjectByzantineCertifiedRequest,
       AsyncByzantineProposal, AsyncByzantineVote,
       AsyncByzantineTimeout,
       ByzantineBroadcastProposal, ByzantineBroadcastVote,
       ByzantineBroadcastTimeout, vars

THEOREM AsyncOrdinaryStepPreservesRecoveryValidationClearing ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED <<up, AsyncRecoveryVars>>
  => ResponsiveRecoveryValidationClearedInvariant'
BY RunNodeWorkPreservesRecoveryValidationClearing,
   AsyncNonRunnerStepDoesNotCreateValidation, IsaT(120)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunHistoricalServer, HistoricalRecoveryTarget,
       AsyncRecoveryVars, vars

THEOREM ResponsiveCrashEstablishesRecoveryValidationClearing ==
  \A node \in ValidatorIds:
    PreGstResponsiveCrash(node)
      => ResponsiveRecoveryValidationClearedInvariant'
BY Isa
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       PreGstResponsiveCrash, Crash

THEOREM ResponsiveRestartPreservesRecoveryValidationClearing ==
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ PreGstResponsiveRestart
  => ResponsiveRecoveryValidationClearedInvariant'
BY Isa
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       PreGstResponsiveRestart, Restart

THEOREM ResponsiveReplayLeavesClearedPhases ==
  PreGstResponsiveReplay
    => asyncRecoveryPhase'
         \notin {"RestartRequired", "ReplayRequired"}
BY Isa DEF PreGstResponsiveReplay

THEOREM ResponsiveReplayEstablishesRecoveryValidationClearing ==
  PreGstResponsiveReplay
    => ResponsiveRecoveryValidationClearedInvariant'
BY ResponsiveReplayLeavesClearedPhases, Isa
   DEF ResponsiveRecoveryValidationClearedInvariant

THEOREM ResponsiveReplayContinuationEstablishesRecoveryValidationClearing ==
  DriveResponsiveReplayHead
    \/ FinishResponsiveReplay
    \/ RearmResponsiveRecovery
    => ResponsiveRecoveryValidationClearedInvariant'
BY Isa
   DEF ResponsiveRecoveryValidationClearedInvariant,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery

THEOREM NonResponsiveCrashPreservesRecoveryValidationClearing ==
  \A node \in ValidatorIds:
    /\ ResponsiveRecoveryValidationClearedInvariant
    /\ PreGstCrash(node)
    => ResponsiveRecoveryValidationClearedInvariant'
BY Isa
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       PreGstCrash, Crash, AsyncRecoveryVars

THEOREM AsyncNextPreservesRecoveryValidationClearing ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ AsyncNext
  => ResponsiveRecoveryValidationClearedInvariant'
BY AsyncOrdinaryStepPreservesRecoveryValidationClearing,
   ResponsiveCrashEstablishesRecoveryValidationClearing,
   ResponsiveRestartPreservesRecoveryValidationClearing,
   ResponsiveReplayEstablishesRecoveryValidationClearing,
   ResponsiveReplayContinuationEstablishesRecoveryValidationClearing,
   NonResponsiveCrashPreservesRecoveryValidationClearing, IsaT(120)
   DEF AsyncNext, AsyncNonCrashStep

THEOREM AsyncAllVarsStutterPreservesRecoveryValidationClearing ==
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ UNCHANGED AsyncAllVars
  => ResponsiveRecoveryValidationClearedInvariant'
BY Isa
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncAllVars, AsyncRecoveryVars, vars

THEOREM AsyncBracketNextPreservesRecoveryValidationClearing ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ [AsyncNext]_AsyncAllVars
  => ResponsiveRecoveryValidationClearedInvariant'
BY AsyncNextPreservesRecoveryValidationClearing,
   AsyncAllVarsStutterPreservesRecoveryValidationClearing, Isa

THEOREM AsyncInitEstablishesRecoveryValidationClearing ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => ResponsiveRecoveryValidationClearedInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncRecoveryInit,
       ResponsiveRecoveryValidationClearedInvariant

THEOREM ResponsiveRecoveryValidationClearedInvariantObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []ResponsiveRecoveryValidationClearedInvariant
PROOF
  <1>1. \A initialContext:
          AsyncInitAt(initialContext)
            => ResponsiveRecoveryValidationClearedInvariant
    BY AsyncInitEstablishesRecoveryValidationClearing
  <1>2. \A initialContext:
          AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
    BY AsyncSpecAlwaysStrongTypeInvariant
  <1>3. /\ AsyncStrongTypeInvariant
         /\ ResponsiveRecoveryValidationClearedInvariant
         /\ [AsyncNext]_AsyncAllVars
         => ResponsiveRecoveryValidationClearedInvariant'
    BY AsyncBracketNextPreservesRecoveryValidationClearing
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF AsyncSpecAt

(***************************************************************************
Responsive crash/restart/replay preservation of the open kernel.

The three controller actions have different handoffs:

  * Crash makes the historical-Commit antecedent false by clearing the
    selected node's validation receipts, while durable Decision and locked
    Prepare sources acquire exact recovery authority.
  * Restart advances both generation and recoveryGeneration to the same fresh
    process value.  The
    reachable clearing invariant prevents the framed old receipt set from
    becoming current merely because the generation changed.
  * Replay leaves the historical-Commit antecedent false.  A unique durable
    Decision receives the exact current-consumer FetchBody candidate, while a
    locked Prepare either retains Replaying authority or receives the
    locked-body FetchBody prefix installed by the reset.
***************************************************************************)

THEOREM ResponsiveCrashPreservesHistoricalLockedCommitRecovery ==
  \A crashNode \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedCommitRecoveryProgress
    /\ PreGstResponsiveCrash(crashNode)
    => HistoricalLockedCommitRecoveryProgress'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(120)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       TypeInvariant, HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveCrash, Crash

THEOREM ResponsiveCrashPreservesDurableDecisionProgress ==
  \A crashNode \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncDurableDecisionProgressWitness
    /\ PreGstResponsiveCrash(crashNode)
    => AsyncDurableDecisionProgressWitness'
BY ResponsiveCrashInstallsExactDurableDecisionAuthority, IsaT(150)
   DEF AsyncStrongTypeInvariant, StrongInductiveInvariant, Safety,
       TypeInvariant, AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       DecisionPipelineCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, NodeHasApplication,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveCrash, Crash

THEOREM ResponsiveCrashPreservesHistoricalLockedBodyRecovery ==
  \A crashNode \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedBodyRecoveryStageInvariant
    /\ PreGstResponsiveCrash(crashNode)
    => HistoricalLockedBodyRecoveryStageInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(150)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       RestartLockedPrepareQCs, RestartLockedCertifiedRequest,
       CertifiedRequestOutbox,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveCrash, Crash

THEOREM PreGstResponsiveCrashPreservesOpenProgressWitnessKernel ==
  \A crashNode \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ OpenProgressWitnessKernelInvariant
    /\ PreGstResponsiveCrash(crashNode)
    => OpenProgressWitnessKernelInvariant'
BY ResponsiveCrashPreservesHistoricalLockedCommitRecovery,
   ResponsiveCrashPreservesDurableDecisionProgress,
   ResponsiveCrashPreservesHistoricalLockedBodyRecovery,
   Isa DEF OpenProgressWitnessKernelInvariant

THEOREM ResponsiveRestartPreservesHistoricalLockedCommitRecovery ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ HistoricalLockedCommitRecoveryProgress
  /\ PreGstResponsiveRestart
  => HistoricalLockedCommitRecoveryProgress'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(120)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveRestart, Restart

THEOREM ResponsiveRestartPreservesDurableDecisionProgress ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncDurableDecisionProgressWitness
  /\ PreGstResponsiveRestart
  => AsyncDurableDecisionProgressWitness'
BY ResponsiveRestartAdvancesExactDurableDecisionAuthority, IsaT(150)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       DecisionPipelineCandidate, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, NodeHasApplication,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveRestart, Restart

THEOREM ResponsiveRestartPreservesHistoricalLockedBodyRecovery ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ PreGstResponsiveRestart
  => HistoricalLockedBodyRecoveryStageInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(150)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       RestartLockedPrepareQCs, RestartLockedCertifiedRequest,
       CertifiedRequestOutbox,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveRestart, Restart

THEOREM PreGstResponsiveRestartPreservesOpenProgressWitnessKernel ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ OpenProgressWitnessKernelInvariant
  /\ PreGstResponsiveRestart
  => OpenProgressWitnessKernelInvariant'
BY ResponsiveRestartPreservesHistoricalLockedCommitRecovery,
   ResponsiveRestartPreservesDurableDecisionProgress,
   ResponsiveRestartPreservesHistoricalLockedBodyRecovery,
   Isa DEF OpenProgressWitnessKernelInvariant

(***************************************************************************
Every locked Prepare restart source is fixed to the recovering node's lock
rank and subject.  RestartLockedBodyReplay may choose any one such QC, but its
FetchBody candidate therefore witnesses every source with those same recovery
coordinates; the historical stage predicate does not require evidence-object
identity.
***************************************************************************)

THEOREM RestartLockedPrepareSourcesShareRecoveryCoordinates ==
  \A node \in ValidatorIds:
    \A left \in RestartLockedPrepareQCs(node):
      \A right \in RestartLockedPrepareQCs(node):
        /\ left.context = right.context
        /\ left.view = right.view
        /\ left.subject = right.subject
BY SMT
   DEF RestartLockedPrepareQCs, LockedPrepareRecoverySource

(***************************************************************************
When signature replay is nonempty, RestartReplay places the locked-body
Fetch prefix before its first signature.  The reset installs that whole
prefix in the recovering node's causal queue.  Because all locked Prepare
sources share recovery coordinates, that one scheduled Fetch witnesses the
non-authority carrier for every such source, independently of which exact QC
the deterministic CHOOSE selected.
***************************************************************************)

THEOREM PreGstResponsiveReplayEstablishesResponsiveReplayLockedBodyCarrier ==
  /\ AsyncStrongTypeInvariant
  /\ PreGstResponsiveReplay
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   RestartLockedPrepareChoiceIsAvailable,
   RestartLockedPrepareSourcesShareRecoveryCoordinates,
   RestartLockedBodyReplayProperties,
   RestartLockedBodyReplayCandidateShape,
   RangeConcatenation, RangeEquality, IsaT(300)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ResponsiveReplayLockedBodyCarrierInvariant,
       HistoricalLockedBodyNonAuthorityCarrier,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       RestartLockedPrepareQCs, RestartLockedPrepareQC,
       RestartLockedBodyReplay, RestartLockedBodyPipelineCandidate,
       RestartSignatureReplay, RestartReplay, RestartDecisionReplay,
       RestartRunnerAssembly, RestartCandidate,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, AsyncQueueTyped, AsyncCandidateTyped,
       AsyncCandidateSet, SequenceSet,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart

THEOREM ResponsiveReplayPreservesHistoricalLockedCommitRecovery ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ HistoricalLockedCommitRecoveryProgress
  /\ PreGstResponsiveReplay
  => HistoricalLockedCommitRecoveryProgress'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(150)
   DEF ResponsiveRecoveryValidationClearedInvariant,
       RecoveryNodeValidationCleared,
       AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart

THEOREM ResponsiveReplayPreservesDurableDecisionProgress ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionsUniqueByNodeContext
  /\ AsyncDurableDecisionProgressWitness
  /\ PreGstResponsiveReplay
  => AsyncDurableDecisionProgressWitness'
BY ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate,
   UniqueUnappliedDecisionExcludesNodeApplication, IsaT(180)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       ExactCurrentDecisionFetchUpdate, DecisionFetchCandidateAt,
       DecisionPipelineCandidate, CandidateScheduled,
       CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet, NodeHasApplication,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart, RestartSignatureReplay,
       RestartReplay, RestartDecisionReplay, RestartCandidate

THEOREM ResponsiveReplayPreservesHistoricalLockedBodyRecovery ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ PreGstResponsiveReplay
  => HistoricalLockedBodyRecoveryStageInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   RestartLockedPrepareSourcesShareRecoveryCoordinates, IsaT(240)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown, NoDecisionForNode,
       RestartLockedPrepareQCs, RestartLockedPrepareQC,
       RestartLockedBodyReplay, RestartLockedBodyPipelineCandidate,
       RestartLockedCertifiedRequest,
       RestartSignatureReplay, RestartReplay, RestartDecisionReplay,
       RestartRunnerAssembly, RestartCandidate,
       CertifiedRequestOutbox,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       PreGstResponsiveReplay, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       ResetNodeSchedulerForRestart

THEOREM PreGstResponsiveReplayPreservesOpenProgressWitnessKernel ==
  /\ AsyncStrongTypeInvariant
  /\ DecisionsUniqueByNodeContext
  /\ ResponsiveRecoveryValidationClearedInvariant
  /\ OpenProgressWitnessKernelInvariant
  /\ PreGstResponsiveReplay
  => OpenProgressWitnessKernelInvariant'
BY ResponsiveReplayPreservesHistoricalLockedCommitRecovery,
   ResponsiveReplayPreservesDurableDecisionProgress,
   ResponsiveReplayPreservesHistoricalLockedBodyRecovery,
   Isa DEF OpenProgressWitnessKernelInvariant

(***************************************************************************
Continuation edges that do not discharge Replaying authority.
DriveResponsiveReplayHead keeps the phase and recovery identity fixed, so a
recovering locked-Prepare source retains authority.  Rearm starts in
Recovered, where neither Decision nor locked-body recovery authority can be a
pre-state witness, and frames every ordinary witness carrier.
***************************************************************************)

THEOREM DriveResponsiveReplayHeadPreservesOpenProgressWitnessKernel ==
  /\ AsyncStrongTypeInvariant
  /\ OpenProgressWitnessKernelInvariant
  /\ DriveResponsiveReplayHead
  => OpenProgressWitnessKernelInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   RestartSignatureReplayProperties, IsaT(180)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       OpenProgressWitnessKernelInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       DecisionPipelineCandidate, NodeHasApplication,
       RestartLockedPrepareQCs, RestartLockedCertifiedRequest,
       RestartLockedBodyPipelineCandidate, CertifiedRequestOutbox,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       FreshCandidateSequence, AsyncRecoveryLifecycleVars

THEOREM DriveResponsiveReplayHeadPreservesResponsiveReplayLockedBodyCarrier ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ DriveResponsiveReplayHead
  => ResponsiveReplayLockedBodyCarrierInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   RestartSignatureReplayProperties, RangeConcatenation, RangeEquality,
   FunctionalConcatUpdateAtKey, IsaT(180)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ResponsiveReplayLockedBodyCarrierInvariant,
       HistoricalLockedBodyNonAuthorityCarrier,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       RestartLockedPrepareQCs, CertifiedRequestOutbox,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       DriveResponsiveReplayHead, RecoveryCoreReplay,
       ResumeProposal, ResumeVote, ResumeTimeout,
       FreshCandidateSequence, AsyncRecoveryLifecycleVars

THEOREM RearmResponsiveRecoveryPreservesOpenProgressWitnessKernel ==
  /\ AsyncStrongTypeInvariant
  /\ OpenProgressWitnessKernelInvariant
  /\ RearmResponsiveRecovery
  => OpenProgressWitnessKernelInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(120)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       OpenProgressWitnessKernelInvariant,
       HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       DecisionPipelineCandidate, NodeHasApplication,
       RestartLockedPrepareQCs, RestartLockedCertifiedRequest,
       CertifiedRequestOutbox,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       RearmResponsiveRecovery, AsyncSchedulerVars, AsyncRecoveryVars

(***************************************************************************
Finish frames Core and only appends an optional AssembleBody candidate, so it
preserves the Commit and Decision conjuncts directly.  It also removes the
Replaying authority disjunct.  The scoped carrier invariant above supplies a
concrete non-authority witness for each locked Prepare source of that recovery
node, which is exactly the extra premise needed to close the locked-body and
full open-kernel Finish cases.  This conditional result is not a claim that
the auxiliary is invariant under the still-open runtime actions.
***************************************************************************)

THEOREM FinishResponsiveReplayPreservesHistoricalLockedCommitRecovery ==
  /\ HistoricalLockedCommitRecoveryProgress
  /\ FinishResponsiveReplay
  => HistoricalLockedCommitRecoveryProgress'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   IsaT(90)
   DEF HistoricalLockedCommitRecoveryProgress,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       FinishResponsiveReplay, RestartRunnerAssembly,
       FreshCandidateSequence, vars

THEOREM FinishResponsiveReplayPreservesDurableDecisionProgress ==
  /\ AsyncDurableDecisionProgressWitness
  /\ FinishResponsiveReplay
  => AsyncDurableDecisionProgressWitness'
BY IsaT(90)
   DEF AsyncDurableDecisionProgressWitness,
       AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionRecoveryAuthority, DurableDecisionRecoveryAuthority,
       DurableDecisionRecoveryExecutorCurrent, RestartDecisions,
       DecisionPipelineCandidate, NodeHasApplication,
       CandidateScheduled, CandidateConsumerCurrent,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       FinishResponsiveReplay, RestartRunnerAssembly,
       FreshCandidateSequence, vars

THEOREM FinishResponsiveReplayPreservesHistoricalLockedBodyRecovery ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ FinishResponsiveReplay
  => HistoricalLockedBodyRecoveryStageInvariant'
BY HistoricalBeginLockRecoveryEvidencePersistsUnderSentHistoryGrowth,
   HistoricalLockedBodyRecoveryStageDecomposition,
   RangeConcatenation, RangeEquality, FunctionalConcatUpdateAtKey,
   IsaT(240)
   DEF AsyncStrongTypeInvariant, AsyncRecoveryTypeInvariant,
       StrongInductiveInvariant, Safety, TypeInvariant,
       ResponsiveReplayLockedBodyCarrierInvariant,
       HistoricalLockedBodyNonAuthorityCarrier,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryAuthority,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalBeginLockRecoveryCandidate,
       HistoricalBeginLockRecoveryEvidence,
       HistoricalCertifiedResponseRecoveryEvidence,
       CertifiedResponseAuthenticatedOccurrence,
       AsyncCertifiedResponseAuthProjection,
       HistoricalLockedPrepareForCommit,
       HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoHigherConflictingPrepareKnown,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       RestartLockedPrepareQCs, CertifiedRequestOutbox,
       CandidateConsumerCurrent, CandidateScheduled,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, SequenceSet,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       FinishResponsiveReplay, RestartRunnerAssembly,
       FreshCandidateSequence, vars

THEOREM FinishResponsiveReplayPreservesOpenProgressWitnessKernel ==
  /\ AsyncStrongTypeInvariant
  /\ ResponsiveReplayLockedBodyCarrierInvariant
  /\ OpenProgressWitnessKernelInvariant
  /\ FinishResponsiveReplay
  => OpenProgressWitnessKernelInvariant'
BY FinishResponsiveReplayPreservesHistoricalLockedCommitRecovery,
   FinishResponsiveReplayPreservesDurableDecisionProgress,
   FinishResponsiveReplayPreservesHistoricalLockedBodyRecovery,
   Isa DEF OpenProgressWitnessKernelInvariant

(***************************************************************************
Exact remaining inductive frontier.

The fully framed non-runner slice
`AsyncNonRunnerStep /\ UNCHANGED AsyncRecoveryVars`, bracket stuttering,
responsive crash, authenticated restart, atomic replay reset, replay-head
drive, and recovery rearm are closed above.  The explicit recovery frame is
essential: the inner worker/publication actions do not bind controller state
until `AsyncNonCrashStep` supplies that outer frame.  Exact sent-history
frames and monotone publication by service, commit-certificate discovery, and
transport faults all preserve the same authenticated CertifiedResponse
occurrence; replay actions retain the exact sent history.

The reachable RestartRequired/ReplayRequired validation-clearing invariant is
also closed under the full bracketed asynchronous relation.
`LocalAdmissionStep` is closed above by exact carrier-set preservation; no
ingress or runtime conclusion is bundled into that theorem.  Replay entry now
establishes the scoped non-authority carrier from RestartReplay's locked Fetch
prefix, and Finish is closed above only under that auxiliary.  A complete
proof must still discharge `RunNodeWork` for both `RunNode` and
`RunHistoricalRecoveryNode`, restricted now to `IngressDrainStep` and
`SerializedRuntimeStep`, together with preservation of
`ResponsiveReplayLockedBodyCarrierInvariant` across those same owner-moving
actions.

The remaining cases are authenticated ingress drain, command
deferral/discard, and successful command execution plus
`AppendCausalSuccessors`.  The semantic handoffs that can create or consume an
open-kernel or replay-carrier witness are PersistInstallTC, PersistDecision,
FetchBody/FetchCertifiedBody, StoreBody, ValidateBody,
BeginLockCommit/PersistLockCommit, and Apply.  `RequestCertifiedBody` remains
dead command vocabulary: no `AsyncNext` constructor emits such a candidate,
so it is not a reachable semantic handoff.  Closing the reachable cases still
requires:

  1. the source-lineage facts that carry an exact certified request or body
     pipeline owner between these commands.  The `CertificateRef` handoff
     itself is closed: `HistoricalBeginLockExecutionCreatesSameRefPending`
     proves that executing the coordinate-matching BeginLockCommit candidate
     creates a pending request with the same stable Prepare reference;

  2. command-specific discard facts showing that a disabled current
     Decision/locked-body pipeline candidate, or historical BeginLockCommit
     candidate, has already advanced or become terminal; and

  3. the mechanical coalescing fact that every relevant `CommandSuccessor` is
     scheduled in the post-state, followed by the `RunNodeWork` preservation
     proof for both the open kernel and the scoped replay carrier auxiliary.

The earlier validation-generation and Progress-capacity prerequisites are no
longer on this frontier: PersistInstallTC clears the installing node's
validation receipts when it advances the generation, and BeginLockCommit is
a Completion-class candidate rather than a capacity-bounded Progress
candidate.

No theorem below this comment imports or postulates this frontier, and no
theorem in this module claims the full open-kernel invariant.
***************************************************************************)

=============================================================================
