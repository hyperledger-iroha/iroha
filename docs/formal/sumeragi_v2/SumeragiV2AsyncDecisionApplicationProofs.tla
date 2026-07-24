---- MODULE SumeragiV2AsyncDecisionApplicationProofs ----
EXTENDS SumeragiV2AsyncOutstandingLivenessDebt

(***************************************************************************
Exact Decision-to-application temporal closure.

This cone deliberately starts below the release-facing application theorem.
The recovery-aware invariant supplies one semantic Decision owner; post-GST
quarantine exclusion removes the only crash/replay alternative.  A scheduled
owner then leaves only through the exact FetchBody / RequestCertifiedBody /
FetchCertifiedBody / StoreBody / ValidateBody / Apply handoff, while the
protected-service theorem supplies starvation freedom for that occurrence.

The certified-request leg is not represented by an unproved leaf property.
`DecisionCertifiedRequestCompletenessInvariant` records the reachable-state
fact created by `PublishCertifiedRequests`: until an authenticated matching
response atomically retires the request set and installs FetchCertifiedBody,
the request to every CommitQC signer remains active.  The responsive-quorum
availability lemma therefore selects a responsive signer which actually owns
the durable body, and the fair retransmit / ingress / Serve / response path
below names every concrete action which moves that exact request.  No
production-refinement proposition participates in this model proof.

This theorem is intentionally scoped to certified-body traffic between the
frozen responsive validator set.  The production daemon admits non-roster
hubs through a separate per-authenticated-source credit/RR stage before the
Core model's aggregate `Untrusted` lane; preserving distinct-hub isolation
across those two stages remains a separate production-refinement obligation
and is not inferred from `AsyncIngressSources` here.
***************************************************************************)

DecisionPipelineKinds ==
  {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody", "StoreBody",
   "ValidateBody", "Apply"}

DecisionCertifiedRequestSetActive(node, qc) ==
  /\ CertifiedRequestOutbox(node, qc) # {}
  /\ CertifiedRequestOutbox(node, qc) \subseteq asyncActiveRequests

DecisionCertifiedRequestCompletenessInvariant ==
  \A decision \in decisions:
    /\ decision.node \in AsyncCurrentResponsiveVoters
    /\ decision.qc.context = context
    /\ DecisionCertifiedRequestActive(decision.node, decision.qc)
    => DecisionCertifiedRequestSetActive(decision.node, decision.qc)

THEOREM AsyncInitEstablishesDecisionCertifiedRequestCompleteness ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => DecisionCertifiedRequestCompletenessInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       DecisionCertifiedRequestCompletenessInvariant,
       DecisionCertifiedRequestActive

THEOREM AsyncAllVarsStutterPreservesDecisionCertifiedRequestCompleteness ==
  /\ DecisionCertifiedRequestCompletenessInvariant
  /\ UNCHANGED AsyncAllVars
  => DecisionCertifiedRequestCompletenessInvariant'
BY Isa
   DEF DecisionCertifiedRequestCompletenessInvariant,
       DecisionCertifiedRequestSetActive,
       DecisionCertifiedRequestActive, DecisionRecoveryCertificate,
       CertifiedRequestOutbox, AsyncAllVars, AsyncSchedulerVars, vars

(***************************************************************************
Only four transitions can change the certified-request lifecycle: publishing
the complete signer outbox, superseding a pre-Decision lock during TC install,
admitting a matching authenticated response, and pre-GST restart reset.  The
last three either leave a complete current Decision set, install the exact
FetchCertifiedBody successor while removing it, or enter the durable replay
authority.  This is the smallest additional invariant needed by the network
leg; it says nothing about arbitrary traffic or non-Decision requests.
***************************************************************************)
THEOREM AsyncNextPreservesDecisionCertifiedRequestCompleteness ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionCertifiedRequestCompletenessInvariant
  /\ AsyncNext
  => DecisionCertifiedRequestCompletenessInvariant'
BY DecisionRecoveryCertificateHasResponsiveRemoteBodySource,
   PersistDecisionRecoveryUsesCompletionFetchBody,
   CompletionDeferralRetainsCandidate,
   ExactDurableDecisionRecoveryLifecycleTransition,
   IsaT(600)
   DEF DecisionCertifiedRequestCompletenessInvariant,
       DecisionCertifiedRequestSetActive,
       DecisionCertifiedRequestActive, DecisionRecoveryCertificate,
       DecisionPipelineCandidate, DecisionTimeoutFrontierInvariant,
       DecisionFrontierUniquenessInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, DrainFairIngressSelected,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedResponseCandidate, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteDecisionFetch,
       ExecuteRequestCertifiedBody, ExecutePersistDecision,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       PersistInstalledControlAfterInstall,
       ResetNodeSchedulerForRestart, RestartReplay,
       RestartDecisionReplay, AsyncAllVars

THEOREM AsyncBracketPreservesDecisionCertifiedRequestCompleteness ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ DecisionTimeoutFrontierInvariant
  /\ DecisionFrontierUniquenessInvariant
  /\ DecisionCertifiedRequestCompletenessInvariant
  /\ [AsyncNext]_AsyncAllVars
  => DecisionCertifiedRequestCompletenessInvariant'
PROOF
  <1>1. CASE AsyncNext
    BY <1>1, AsyncNextPreservesDecisionCertifiedRequestCompleteness
  <1>2. CASE UNCHANGED AsyncAllVars
    BY <1>2,
       AsyncAllVarsStutterPreservesDecisionCertifiedRequestCompleteness
  <1> QED BY <1>1, <1>2

THEOREM DecisionCertifiedRequestCompletenessObligation ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []DecisionCertifiedRequestCompletenessInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE AsyncSpecAt(initialContext)
                 => []DecisionCertifiedRequestCompletenessInvariant
    <2> DEFINE Inductive ==
           /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ DecisionTimeoutFrontierInvariant
           /\ DecisionFrontierUniquenessInvariant
           /\ DecisionCertifiedRequestCompletenessInvariant
    <2>1. AsyncInitAt(initialContext) => Inductive
      BY AsyncInitEstablishesStrongTypeInvariant,
         AsyncInitEstablishesProgressOwnership,
         AsyncInitEstablishesDecisionTimeoutFrontier,
         AsyncInitEstablishesDecisionFrontierUniqueness,
         AsyncInitEstablishesDecisionCertifiedRequestCompleteness
         DEF Inductive
    <2>2. Inductive /\ [AsyncNext]_AsyncAllVars => Inductive'
      BY AsyncBracketNextPreservesStrongTypeInvariant,
         AsyncBracketNextPreservesProgressOwnership,
         AsyncBracketPreservesDecisionTimeoutFrontier,
         AsyncBracketPreservesStrongDecisionFrontier,
         AsyncBracketPreservesDecisionCertifiedRequestCompleteness
         DEF Inductive
    <2>3. AsyncSpecAt(initialContext) => []Inductive
      BY <2>1, <2>2, PTL DEF AsyncSpecAt
    <2>4. Inductive => DecisionCertifiedRequestCompletenessInvariant
      BY DEF Inductive
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM PostGstExcludesDecisionRecoveryAuthority ==
  \A node, qc:
    /\ gst
    /\ PostGstReplayQuarantineExcluded
    => ~DecisionRecoveryAuthority(node, qc)
BY Isa
   DEF PostGstReplayQuarantineExcluded, DecisionRecoveryAuthority,
       DurableDecisionRecoveryAuthority

DecisionApplicationFrontier(node, qc) ==
  \/ NodeHasApplication(node)
  \/ DecisionCertifiedRequestActive(node, qc)
  \/ \E candidate \in AsyncCandidateSet:
       DecisionPipelineCandidate(node, qc, candidate)

THEOREM RecoveryAwareDecisionWitnessProjectsApplicationFrontier ==
  \A node, qc:
    /\ gst
    /\ PostGstReplayQuarantineExcluded
    /\ AsyncDecisionCompletionWitness(node, qc)
    => DecisionApplicationFrontier(node, qc)
BY PostGstExcludesDecisionRecoveryAuthority, Isa
   DEF AsyncDecisionCompletionWitness, DecisionCompletionWitness,
       DecisionApplicationFrontier

DecisionPipelineStageOutcome(node, qc, kind) ==
  \/ NodeHasApplication(node)
  \/ CASE kind = "FetchBody" ->
            \/ DecisionCertifiedRequestActive(node, qc)
            \/ DecisionPipelineKindOwned(node, qc, "ValidateBody")
       [] kind = "RequestCertifiedBody" ->
            DecisionCertifiedRequestActive(node, qc)
       [] kind = "FetchCertifiedBody" ->
            DecisionPipelineKindOwned(node, qc, "StoreBody")
       [] kind = "StoreBody" ->
            DecisionPipelineKindOwned(node, qc, "ValidateBody")
       [] kind = "ValidateBody" ->
            DecisionPipelineKindOwned(node, qc, "Apply")
       [] kind = "Apply" -> NodeHasApplication(node)
       [] OTHER -> FALSE

DecisionPipelineStagePending(node, qc, kind, candidate) ==
  /\ gst
  /\ ~NodeHasApplication(node)
  /\ candidate.kind = kind
  /\ DecisionPipelineCandidate(node, qc, candidate)

THEOREM DecisionPipelineStagePendingIsProtected ==
  \A node \in AsyncCurrentResponsiveVoters,
     qc, kind \in DecisionPipelineKinds,
     candidate \in AsyncCandidateSet:
    DecisionPipelineStagePending(node, qc, kind, candidate)
      => ResponsiveProtectedCandidateOwned(candidate)
BY Isa
   DEF DecisionPipelineStagePending, DecisionPipelineCandidate,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       ProtectedServiceCandidate

(***************************************************************************
The exact one-step handoff kernel.  Removing a current-consumer Decision
candidate without applying is permitted only when its concrete execution has
created the next semantic stage.  The proof expands the real serialized
dispatch and all six production-model actions; generic ownership or a changed
consumer epoch is not accepted as a successor.
***************************************************************************)
THEOREM DecisionPipelineStagePersistsUntilExactHandoff ==
  \A node \in AsyncCurrentResponsiveVoters,
     qc, kind \in DecisionPipelineKinds,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ DecisionTimeoutFrontierInvariant
    /\ DecisionFrontierUniquenessInvariant
    /\ PostGstReplayQuarantineExcluded
    /\ DecisionPipelineStagePending(node, qc, kind, candidate)
    /\ [AsyncNext]_AsyncAllVars
    => \/ DecisionPipelineStagePending(node, qc, kind, candidate)'
       \/ DecisionPipelineStageOutcome(node, qc, kind)'
BY CompletionDeferralRetainsCandidate,
   CoreBracketStepPreservesNodeApplication,
   IsaT(600)
   DEF DecisionPipelineStagePending, DecisionPipelineStageOutcome,
       DecisionPipelineKinds, DecisionPipelineKindOwned,
       DecisionPipelineCandidate, CandidateConsumerCurrent,
       CandidateScheduled, QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       DecisionCertifiedRequestActive, DecisionRecoveryCertificate,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork, RunHistoricalServer, OpenHistoricalRecovery,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       LocalAdmissionStep, IngressDrainStep, SerializedRuntimeStep,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand,
       ExecuteDecisionFetch, ExecuteRequestCertifiedBody, ExecuteApply,
       FetchCertifiedBody, StoreBody, ValidateDecidedBody, ApplyDecision,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       AppendCausalSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, CommandSuccessors,
       AsyncAllVars

THEOREM DecisionPipelineStageReachesExactHandoff ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext), qc,
       kind \in DecisionPipelineKinds,
       candidate \in AsyncCandidateSet:
      /\ AsyncSpecAt(initialContext)
      /\ StarvationFreedomProperty(AsyncSpecAt(initialContext))
      => DecisionPipelineStagePending(node, qc, kind, candidate)
           ~> DecisionPipelineStageOutcome(node, qc, kind)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                NEW qc,
                NEW kind \in DecisionPipelineKinds,
                NEW candidate \in AsyncCandidateSet,
                AsyncSpecAt(initialContext),
                StarvationFreedomProperty(AsyncSpecAt(initialContext))
         PROVE DecisionPipelineStagePending(node, qc, kind, candidate)
                 ~> DecisionPipelineStageOutcome(node, qc, kind)
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncProgressOwnershipInvariant
      BY AsyncSpecAlwaysProgressOwnershipInvariant
    <2>3. AsyncSpecAt(initialContext)
             => []DecisionTimeoutFrontierInvariant
      BY DecisionTimeoutFrontierInvariantFromAsyncSpec
    <2>4. AsyncSpecAt(initialContext)
             => []DecisionFrontierUniquenessInvariant
      BY DecisionFrontierUniquenessInvariantFromAsyncSpec
    <2>5. AsyncSpecAt(initialContext)
             => []PostGstReplayQuarantineExcluded
      BY AsyncSpecAlwaysExcludesPostGstReplayQuarantine
    <2>6. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>7. DecisionPipelineStagePending(node, qc, kind, candidate)
             ~> ~ResponsiveProtectedCandidateOwned(candidate)
      BY <1>1, <2>6, DecisionPipelineStagePendingIsProtected, PTL
         DEF StarvationFreedomProperty
    <2>8. DecisionPipelineStagePending(node, qc, kind, candidate)
             /\ [AsyncNext]_AsyncAllVars
            => \/ DecisionPipelineStagePending(
                    node, qc, kind, candidate)'
               \/ DecisionPipelineStageOutcome(node, qc, kind)'
      BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         DecisionPipelineStagePersistsUntilExactHandoff, PTL
    <2>9. DecisionPipelineStagePending(node, qc, kind, candidate)
             => ResponsiveProtectedCandidateOwned(candidate)
      BY <1>1, <2>6, DecisionPipelineStagePendingIsProtected, PTL
    <2> QED BY <1>1, <2>7, <2>8, <2>9, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

DecisionCertifiedResponseOutcome(node, qc) ==
  \/ NodeHasApplication(node)
  \/ DecisionCertifiedFetchOwned(node, qc)

(***************************************************************************
Authenticated certified-body round trip.

The proof consumes the complete active signer outbox, selects the responsive
CommitQC signer with the durable body, and follows the exact request packet,
fair per-source ingress lane, fresh-nonce Serve FIFO, response packet, and
requester ingress admission.  `StarvationFreedomProperty` is used only for
the admitted Serve occurrence; packet and runner movement comes directly from
the weak-fair actions in `AsyncFairnessAt` and their bounded deadline gates.
***************************************************************************)
THEOREM ActiveDecisionCertifiedRequestReachesCertifiedFetch ==
  \A initialContext:
    \A node \in AsyncVotersAt(initialContext), qc \in QcRecordSet:
      /\ AsyncSpecAt(initialContext)
      /\ StarvationFreedomProperty(AsyncSpecAt(initialContext))
      /\ []DecisionCertifiedRequestCompletenessInvariant
      => (gst /\ DecisionCertifiedRequestActive(node, qc))
           ~> DecisionCertifiedResponseOutcome(node, qc)
PROOF
  <1>1. ASSUME NEW initialContext,
                NEW node \in AsyncVotersAt(initialContext),
                NEW qc \in QcRecordSet,
                AsyncSpecAt(initialContext),
                StarvationFreedomProperty(AsyncSpecAt(initialContext)),
                []DecisionCertifiedRequestCompletenessInvariant
         PROVE (gst /\ DecisionCertifiedRequestActive(node, qc))
                 ~> DecisionCertifiedResponseOutcome(node, qc)
    <2>1. AsyncSpecAt(initialContext) => []AsyncStrongTypeInvariant
      BY AsyncSpecAlwaysStrongTypeInvariant
    <2>2. AsyncSpecAt(initialContext)
             => []PostGstReplayQuarantineExcluded
      BY AsyncSpecAlwaysExcludesPostGstReplayQuarantine
    <2>3. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>4. AsyncSpecAt(initialContext) => [](gst => []gst)
      BY AsyncSpecKeepsGstOnceSet
    <2>5. (gst /\ DecisionCertifiedRequestActive(node, qc))
             => DecisionCertifiedRequestSetActive(node, qc)
      BY <1>1, PTL
         DEF DecisionCertifiedRequestCompletenessInvariant,
             DecisionCertifiedRequestActive,
             DecisionRecoveryCertificate
    <2>6. (gst /\ DecisionCertifiedRequestActive(node, qc))
             ~> DecisionCertifiedResponseOutcome(node, qc)
      BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5,
         DecisionRecoveryCertificateHasResponsiveRemoteBodySource,
         StarvationFreedomObligation, PTL, IsaT(600)
         DEF DecisionCertifiedRequestSetActive,
             DecisionCertifiedRequestActive,
             DecisionCertifiedResponseOutcome,
             DecisionCertifiedFetchOwned,
             DecisionRecoveryCertificate, DecisionRecoveryCertificate,
             DecisionPipelineKindOwned, DecisionPipelineCandidate,
             CertifiedRequestOutbox, CertifiedResponseItem,
             CertifiedRequestAuthorized, CertifiedResponseAuthorized,
             MatchingCertifiedRequests, CertifiedResponseCandidate,
             CertifiedServeCanRespond, AsyncFairnessAt,
             PostGstRunNode, RunNode, RunNodeWork,
             DirectRetransmitStep, DeferredRetransmitStep,
             SendNodeRetransmissions, RetryableItems,
             ActiveRequestItems, PacketsForItems,
             PostGstAdmitHiddenPacket, AdmitIngressPacket,
             DrainFairIngressSelected, IngressDrainStep,
             AsyncIoCertifiedServeJob, ResponsiveProtectedServeJobOwned,
             PostGstServiceIoWorker, ServiceIoWorker,
             ServiceIoWorkerWork, PublishEphemeralItems,
             CandidateScheduled, CandidateConsumerCurrent,
             AsyncTick, AsyncTickEnabled, RetransmitDue,
             AsyncSpecAt
    <2> QED BY <2>6
  <1> QED BY <1>1

THEOREM ResponsiveDecisionReachesApplicationFromExactCorridor ==
  \A initialContext:
    /\ AsyncSpecAt(initialContext)
    /\ StarvationFreedomProperty(AsyncSpecAt(initialContext))
    /\ []AsyncDurableDecisionProgressWitness
    /\ []DecisionCertifiedRequestCompletenessInvariant
    => \A node \in AsyncVotersAt(initialContext):
         (gst /\ NodeHasDecision(node)) ~> NodeHasApplication(node)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext),
                StarvationFreedomProperty(AsyncSpecAt(initialContext)),
                []AsyncDurableDecisionProgressWitness,
                []DecisionCertifiedRequestCompletenessInvariant
         PROVE \A node \in AsyncVotersAt(initialContext):
                 (gst /\ NodeHasDecision(node))
                   ~> NodeHasApplication(node)
    <2>1. AsyncSpecAt(initialContext)
             => []PostGstReplayQuarantineExcluded
      BY AsyncSpecAlwaysExcludesPostGstReplayQuarantine
    <2>2. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2>3. \A node \in AsyncVotersAt(initialContext):
             (gst /\ NodeHasDecision(node))
               ~> DecisionApplicationFrontier(
                    node,
                    (CHOOSE decision \in decisions:
                       /\ decision.node = node
                       /\ decision.qc.context = context).qc)
      BY <1>1, <2>1, <2>2,
         RecoveryAwareDecisionWitnessProjectsApplicationFrontier, PTL
         DEF AsyncDurableDecisionProgressWitness, NodeHasDecision
    <2>4. \A node \in AsyncVotersAt(initialContext),
              qc \in QcRecordSet:
             (gst /\ DecisionCertifiedRequestActive(node, qc))
               ~> DecisionCertifiedResponseOutcome(node, qc)
      BY <1>1, ActiveDecisionCertifiedRequestReachesCertifiedFetch
    <2>5. \A node \in AsyncVotersAt(initialContext),
              qc \in QcRecordSet,
              kind \in DecisionPipelineKinds,
              candidate \in AsyncCandidateSet:
             DecisionPipelineStagePending(node, qc, kind, candidate)
               ~> DecisionPipelineStageOutcome(node, qc, kind)
      BY <1>1, DecisionPipelineStageReachesExactHandoff
    <2>6. \A node \in AsyncVotersAt(initialContext):
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
      BY <1>1, <2>2, <2>3, <2>4, <2>5, PTL
         DEF DecisionApplicationFrontier,
             DecisionCertifiedResponseOutcome,
             DecisionPipelineStagePending,
             DecisionPipelineStageOutcome,
             DecisionPipelineKinds, DecisionPipelineKindOwned,
             DecisionPipelineCandidate
    <2> QED BY <2>6
  <1> QED BY <1>1

THEOREM ApplicationCompletionProgressObligation ==
  \A initialContext:
    ApplicationCompletionProgressProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ApplicationCompletionProgressProperty(
                 AsyncSpecAt(initialContext))
    <2>1. StarvationFreedomProperty(AsyncSpecAt(initialContext))
      BY StarvationFreedomObligation
    <2>2. AsyncSpecAt(initialContext)
             => []AsyncDurableDecisionProgressWitness
      BY RecoveryAwareDecisionProgressWitnessObligation
    <2>3. AsyncSpecAt(initialContext)
             => []DecisionCertifiedRequestCompletenessInvariant
      BY DecisionCertifiedRequestCompletenessObligation
    <2>4. \A node \in AsyncVotersAt(initialContext):
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
      BY <2>1, <2>2, <2>3,
         ResponsiveDecisionReachesApplicationFromExactCorridor
    <2>5. AsyncSpecAt(initialContext)
             => [](AsyncCurrentResponsiveVoters
                    = AsyncVotersAt(initialContext))
      BY AsyncSpecAlwaysUsesFixedResponsiveVoters
    <2> QED BY <2>4, <2>5, PTL
         DEF ApplicationCompletionProgressProperty
  <1> QED BY <1>1

THEOREM ApplicationLivenessObligation ==
  \A initialContext:
    ApplicationLivenessProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ApplicationLivenessProperty(AsyncSpecAt(initialContext))
    <2>1. ApplicationCompletionProgressProperty(
             AsyncSpecAt(initialContext))
      BY ApplicationCompletionProgressObligation
    <2>2. AsyncSpecAt(initialContext)
            => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
      BY <2>1, ApplicationCompletionProgressImpliesAggregateApplication, PTL
    <2> QED BY <2>1, <2>2, PTL
         DEF ApplicationCompletionProgressProperty,
             ApplicationLivenessProperty
  <1> QED BY <1>1

THEOREM OneHeightCompletionObligation ==
  \A initialContext:
    OneHeightCompletionLiveness(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE OneHeightCompletionLiveness(initialContext)
    <2>1. RotatingLeaderProgressProperty(AsyncSpecAt(initialContext))
      BY RotatingLeaderProgressObligation
    <2>2. ApplicationLivenessProperty(AsyncSpecAt(initialContext))
      BY ApplicationLivenessObligation
    <2> QED BY <2>1, <2>2,
         OneHeightCompletionFromProgressProperties, PTL
         DEF OneHeightCompletionLiveness
  <1> QED BY <1>1

=============================================================================
