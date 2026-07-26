---- MODULE SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryServiceClosureProofs,
        SumeragiV2AsyncHistoricalRecoveryClockActionProofs

(***************************************************************************
Action-local owner-prefix edges for the historical discovery clock.

The first fixed-clock component charges responsive validators which have not
yet entered `AsyncTimedServiceNodes`.  After GST, responsive membership,
online membership, and the current roster cannot regress.  Opening historical
recovery adds one target.

The requested global timed-owner monotonicity edge is false in the concrete
relation.  `ExecuteApply(command)` chooses a matching member of
`DecisionQcValues`, but does not require either `qc.context = context` or
`command.evidence = qc`.  An old-context Decision with the same view/subject
can therefore remove an out-of-roster historical target without adding a
current-context `NodeHasApplication`.  The exact mismatch action and its
one-step latent-debt increase are exposed below.  The sound current-context
Apply transfer is proved separately.

The next local handoffs cover stale empty I/O gates and due node/I/O service:

  * every concrete enqueue owner removes a due empty gate before making that
    node an active I/O blocker;
  * service of the last due I/O job empties the queue and moves its I/O
    deadline strictly above the frozen clock; and
  * every due runner service action moves its node deadline strictly above the
    frozen clock.

All results are state/action implications.  There are no temporal closure
claims, fairness unions, or Decision-based escapes.
***************************************************************************)

(***************************************************************************
Post-GST owner-set monotonicity.
***************************************************************************)

CoreApplicationEvidenceActionClassification ==
  \/ UNCHANGED applied
  \/ \E application:
       applied' = applied \cup {application}

THEOREM CoreNextClassifiesApplicationEvidence ==
  Next => CoreApplicationEvidenceActionClassification
BY IsaM("blast")
   DEF Next, SetGST, AssembleLocalBody, BeginLocalProposal,
       PersistProposal, CompleteProposalSignature,
       ByzantineBroadcastProposal, DeliverProposal,
       FetchBody, RebindRetainedBody, StoreBody,
       ValidateBody, RejectBody, ValidateDecidedBody,
       ValidateLockedBody, BeginPrepare, PersistPrepare,
       CompleteVoteSignature, ByzantineBroadcastVote,
       DeliverVote, FormPrepareQC,
       ImportAuthenticatedCommitCertificate, DeliverQC,
       BeginObservePrepare, PersistObservePrepare,
       BeginLockCommit, PersistLockCommit, FormCommitQC,
       BeginDecision, PersistDecision, BeginTimeout,
       PersistTimeout, CompleteTimeoutSignature,
       ByzantineBroadcastTimeout, DeliverTimeout, DeliverTC,
       BeginInstallTC, PersistInstallTC, FetchCertifiedBody,
       AcceptCertifiedResponseCapability, ApplyDecision,
       Crash, Restart, ResumeProposal, ResumeVote,
       ResumeTimeout, DropProposal,
       CoreApplicationEvidenceActionClassification

THEOREM CoreBracketApplicationEvidenceIsMonotone ==
  [Next]_vars => applied \subseteq applied'
BY CoreNextClassifiesApplicationEvidence, Isa DEF vars

THEOREM AsyncBracketApplicationEvidenceIsMonotone ==
  [AsyncNext]_AsyncAllVars => applied \subseteq applied'
BY AsyncStepRefinementObligation,
   CoreBracketApplicationEvidenceIsMonotone, Isa
   DEF AsyncAllVars, AsyncSchedulerVars, AsyncRecoveryVars, vars

THEOREM HistoricalDiscoveryPostGstUpAndRosterAreStable ==
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => /\ UNCHANGED <<up, gst>>
     /\ UNCHANGED context
     /\ CurrentVoters' = CurrentVoters
     /\ AsyncCurrentResponsiveVoters' =
          AsyncCurrentResponsiveVoters
BY GstAsyncStepIsMonotone, Isa
   DEF AsyncNext, AsyncNonCrashStep,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       RearmResponsiveRecovery, AsyncAllVars,
       AsyncCurrentResponsiveVoters, CurrentVoters, CurrentEpoch,
       vars

THEOREM OpenHistoricalRecoveryAddsTimedServiceOwner ==
  \A node \in ValidatorIds:
    OpenHistoricalRecovery(node)
      => /\ asyncHistoricalRecoveryTargets' =
               asyncHistoricalRecoveryTargets \cup {node}
         /\ AsyncResponsiveAppliedArchiveServers' =
               AsyncResponsiveAppliedArchiveServers
         /\ AsyncCurrentResponsiveVoters' =
               AsyncCurrentResponsiveVoters
         /\ AsyncTimedServiceNodes' =
               AsyncTimedServiceNodes \cup {node}
BY Isa
   DEF OpenHistoricalRecovery,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncCurrentResponsiveVoters,
       NodeHasApplication, CurrentVoters, CurrentEpoch,
       AsyncSchedulerExceptHistoricalRecoveryTargets, vars

HistoricalExecuteApplyWithQc(command, qc) ==
  /\ ExecuteApply(command)
  /\ qc \in DecisionQcValues
  /\ CommandMatches(
       command, command.node, qc.view, qc.subject)
  /\ ApplyDecision(command.node, qc)

HistoricalExecuteApplyCurrentContext(command, qc) ==
  /\ HistoricalExecuteApplyWithQc(command, qc)
  /\ qc.context = context

HistoricalExecuteApplyContextMismatch(command, qc) ==
  /\ HistoricalExecuteApplyWithQc(command, qc)
  /\ qc.context # context

THEOREM HistoricalCurrentContextApplyTransfersTimedServiceOwner ==
  \A command, qc:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(command.node)
    /\ HistoricalExecuteApplyCurrentContext(command, qc)
    => /\ ~HistoricalRecoveryTarget(command.node)'
       /\ command.node \in AsyncResponsiveAppliedArchiveServers'
       /\ AsyncTimedServiceNodes' = AsyncTimedServiceNodes
BY AsyncStrongTypeProjectsAsyncType, HistoricalRecoveryTargetsAreValidators,
   Isa
   DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncHistoricalRecoveryTypeInvariant,
       HistoricalExecuteApplyCurrentContext,
       HistoricalExecuteApplyWithQc,
       HistoricalRecoveryTarget, ExecuteApply, ApplyDecision,
       CommandMatches, NodeHasApplication,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveServerIds, CurrentVoters, CurrentEpoch,
       vars

THEOREM HistoricalContextMismatchApplyDropsNonVoterTimedOwner ==
  \A command, qc:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(command.node)
    /\ command.node \notin AsyncCurrentResponsiveVoters
    /\ HistoricalExecuteApplyContextMismatch(command, qc)
    => /\ command.node \in AsyncTimedServiceNodes
       /\ ~HistoricalRecoveryTarget(command.node)'
       /\ command.node
            \notin AsyncResponsiveAppliedArchiveServers'
       /\ command.node \notin AsyncTimedServiceNodes'
       /\ AsyncTimedServiceNodes
            \not\subseteq AsyncTimedServiceNodes'
BY AsyncStrongTypeProjectsAsyncType, HistoricalRecoveryTargetsAreValidators,
   Isa
   DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncHistoricalRecoveryTypeInvariant,
       HistoricalExecuteApplyContextMismatch,
       HistoricalExecuteApplyWithQc,
       HistoricalRecoveryTarget, ExecuteApply, ApplyDecision,
       CommandMatches, NodeHasApplication,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveServerIds, CurrentVoters, CurrentEpoch,
       vars

THEOREM HistoricalTimedOwnerMonotonicityOrExactApplyMismatch ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => \/ AsyncTimedServiceNodes
          \subseteq AsyncTimedServiceNodes'
     \/ \E command, qc:
          /\ HistoricalRecoveryTarget(command.node)
          /\ command.node
               \notin AsyncCurrentResponsiveVoters
          /\ HistoricalExecuteApplyContextMismatch(command, qc)
BY HistoricalCurrentContextApplyTransfersTimedServiceOwner,
   HistoricalContextMismatchApplyDropsNonVoterTimedOwner, Isa
   DEF HistoricalRecoveryTarget, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep, FifoRuntimeStep,
       ExecuteCommand, ExecuteApply, OpenHistoricalRecovery,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       HistoricalExecuteApplyContextMismatch,
       HistoricalExecuteApplyWithQc,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncAllVars

THEOREM HistoricalAppliedArchiveOwnersAreMonotoneAfterGst ==
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => AsyncResponsiveAppliedArchiveServers
       \subseteq AsyncResponsiveAppliedArchiveServers'
BY AsyncBracketApplicationEvidenceIsMonotone,
   HistoricalDiscoveryPostGstUpAndRosterAreStable, Isa
   DEF AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       NodeHasApplication, AsyncArchiveServerIds

(***************************************************************************
Latent-owner debt.
***************************************************************************)

THEOREM HistoricalContextMismatchApplyIncreasesLatentDebtAtFixedClock ==
  \A command, qc, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ asyncNow = clockValue
    /\ asyncNow' = clockValue
    /\ HistoricalRecoveryTarget(command.node)
    /\ command.node \notin AsyncCurrentResponsiveVoters
    /\ HistoricalExecuteApplyContextMismatch(command, qc)
    => /\ HistoricalDiscoveryLatentTimedOwners' =
             HistoricalDiscoveryLatentTimedOwners
               \cup {command.node}
       /\ HistoricalDiscoveryLatentOwnerDebt' =
             HistoricalDiscoveryLatentOwnerDebt + 1
BY HistoricalContextMismatchApplyDropsNonVoterTimedOwner,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_AddElement, FS_CardinalityType, Isa
   DEF HistoricalDiscoveryLatentTimedOwners,
       HistoricalDiscoveryLatentOwnerDebt,
       HistoricalDiscoveryPotentialServiceCohort,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncCurrentResponsiveVoters,
       HistoricalExecuteApplyContextMismatch,
       HistoricalExecuteApplyWithQc,
       HistoricalRecoveryTarget, ExecuteApply, ApplyDecision,
       NodeHasApplication, CurrentVoters, CurrentEpoch, vars

THEOREM OpenPreviouslyLatentOwnerSpendsExactlyOneDebt ==
  \A owner \in Responsive, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ asyncNow = clockValue
    /\ owner \in HistoricalDiscoveryLatentTimedOwners
    /\ OpenHistoricalRecovery(owner)
    => /\ asyncNow' = clockValue
       /\ HistoricalDiscoveryLatentTimedOwners' =
            HistoricalDiscoveryLatentTimedOwners \ {owner}
       /\ HistoricalDiscoveryLatentOwnerDebt' + 1 =
            HistoricalDiscoveryLatentOwnerDebt
BY OpenHistoricalRecoveryAddsTimedServiceOwner,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType, Isa
   DEF OpenHistoricalRecovery,
       HistoricalDiscoveryLatentTimedOwners,
       HistoricalDiscoveryLatentOwnerDebt

(***************************************************************************
Dormant empty-I/O handoff.

The four predicates below are the exact mutation owners of an I/O FIFO append:
local Control admission, causal Completion admission, ordinary authenticated
request ingress, and post-application historical request ingress.
***************************************************************************)

HistoricalDiscoveryDueDormantIoGate(node, clockValue) ==
  /\ asyncNow = clockValue
  /\ node \in AsyncTimedServiceNodes
  /\ AsyncIoQueueDepth(node) = 0
  /\ asyncIoServiceDeadlines[node] <= clockValue

HistoricalDiscoverySingleIoEnqueue(node, job, clockValue) ==
  /\ HistoricalDiscoveryDueDormantIoGate(node, clockValue)
  /\ UNCHANGED asyncNow
  /\ asyncIoQueues' =
       [asyncIoQueues EXCEPT ![node] = Append(@, job)]
  /\ UNCHANGED asyncIoServiceDeadlines
  /\ AsyncTimedServiceNodes' = AsyncTimedServiceNodes

HistoricalDiscoveryDormantGateHandoff(node, clockValue) ==
  /\ asyncNow' = clockValue
  /\ node \notin
       HistoricalDiscoveryDormantIoGatesAt(clockValue)'
  /\ node \in
       HistoricalDiscoveryActiveIoBlockersAt(clockValue)'
  /\ HistoricalDiscoveryDormantIoGatesAt(clockValue)' =
       HistoricalDiscoveryDormantIoGatesAt(clockValue) \ {node}
  /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue)' =
       HistoricalDiscoveryActiveIoBlockersAt(clockValue) \cup {node}
  /\ HistoricalDiscoveryDormantIoDebt(clockValue)' + 1 =
       HistoricalDiscoveryDormantIoDebt(clockValue)

THEOREM SingleIoEnqueueSpendsDueDormantGate ==
  \A node \in ValidatorIds, job, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoverySingleIoEnqueue(
         node, job, clockValue)
    => HistoricalDiscoveryDormantGateHandoff(node, clockValue)
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_AddElement, FS_CardinalityType,
   LenProperties, Isa
   DEF HistoricalDiscoverySingleIoEnqueue,
       HistoricalDiscoveryDueDormantIoGate,
       HistoricalDiscoveryDormantGateHandoff,
       HistoricalDiscoveryDormantIoGatesAt,
       HistoricalDiscoveryActiveIoBlockersAt,
       HistoricalDiscoveryDormantIoDebt,
       AsyncIoQueueDepth

HistoricalDiscoveryCausalCompletionIoEnqueue(node) ==
  LET candidate == HeadCausalCandidate(node)
      duplicate == CandidateInFlight(candidate)
  IN /\ AdmitCausalHead(node)
     /\ ~duplicate
     /\ candidate.class = "Completion"

HistoricalDiscoveryFairIngressRequestIoEnqueue(node) ==
  LET index == FirstDrainableIngressIndex(node)
      item == SelectedIngressItemAt(node, index)
  IN /\ DrainFairIngressSelected(node)
     /\ item.kind \in {"CertifiedRequest",
                        "CommitCertificateRequest"}
     /\ IF item.kind = "CertifiedRequest"
        THEN CertifiedRequestAuthorized(item)
        ELSE CommitCertificateRequestAuthorized(item)

HistoricalDiscoveryHistoricalIngressRequestIoEnqueue(node) ==
  LET index == FirstHistoricalDrainableIngressIndex(node)
      item == HistoricalSelectedIngressItemAt(node, index)
  IN /\ DrainHistoricalIngressSelected(node)
     /\ \/ /\ item.kind = "CertifiedRequest"
              /\ item \in asyncSentItems
              /\ CertifiedRequestAuthorized(item)
        \/ /\ item.kind = "CommitCertificateRequest"
              /\ item \in asyncSentItems
              /\ CommitCertificateRequestAuthorized(item)

THEOREM ConcreteIoEnqueueOwnersHaveSingleEnqueueFrame ==
  \A node \in ValidatorIds, clockValue \in Nat:
    HistoricalDiscoveryDueDormantIoGate(node, clockValue)
      => /\ (EnqueueIoLocalControlWork(node)
                => HistoricalDiscoverySingleIoEnqueue(
                     node, AsyncIoControlJob, clockValue))
         /\ (HistoricalDiscoveryCausalCompletionIoEnqueue(node)
                => HistoricalDiscoverySingleIoEnqueue(
                     node,
                     AsyncIoConsensusJob(
                       HeadCausalCandidate(node)),
                     clockValue))
         /\ (HistoricalDiscoveryFairIngressRequestIoEnqueue(node)
                => HistoricalDiscoverySingleIoEnqueue(
                     node,
                     AsyncIoCertifiedServeJob(
                       node,
                       DeliveryCandidate(
                         SelectedIngressItemAt(
                           node,
                           FirstDrainableIngressIndex(node)))),
                     clockValue))
         /\ (HistoricalDiscoveryHistoricalIngressRequestIoEnqueue(node)
                => HistoricalDiscoverySingleIoEnqueue(
                     node,
                     AsyncIoCertifiedServeJob(
                       node,
                       DeliveryCandidate(
                         HistoricalSelectedIngressItemAt(
                           node,
                           FirstHistoricalDrainableIngressIndex(node)))),
                     clockValue))
BY Isa
   DEF HistoricalDiscoveryDueDormantIoGate,
       HistoricalDiscoverySingleIoEnqueue,
       HistoricalDiscoveryCausalCompletionIoEnqueue,
       HistoricalDiscoveryFairIngressRequestIoEnqueue,
       HistoricalDiscoveryHistoricalIngressRequestIoEnqueue,
       EnqueueIoLocalControlWork, AdmitCausalHead,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       PopSelectedIngress, ImportAuthenticatedCommitCertificate,
       EnqueueCandidate, LeaveCausalQueues,
       AsyncIoVars, AsyncDeferredVars, AsyncLocalAdmissionVars,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers, NodeHasApplication,
       CurrentVoters, CurrentEpoch, vars

THEOREM ConcreteIoEnqueueOwnersSpendDueDormantGate ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueDormantIoGate(node, clockValue)
    => /\ (EnqueueIoLocalControlWork(node)
              => HistoricalDiscoveryDormantGateHandoff(
                   node, clockValue))
       /\ (HistoricalDiscoveryCausalCompletionIoEnqueue(node)
              => HistoricalDiscoveryDormantGateHandoff(
                   node, clockValue))
       /\ (HistoricalDiscoveryFairIngressRequestIoEnqueue(node)
              => HistoricalDiscoveryDormantGateHandoff(
                   node, clockValue))
       /\ (HistoricalDiscoveryHistoricalIngressRequestIoEnqueue(node)
              => HistoricalDiscoveryDormantGateHandoff(
                   node, clockValue))
BY ConcreteIoEnqueueOwnersHaveSingleEnqueueFrame,
   SingleIoEnqueueSpendsDueDormantGate

(***************************************************************************
Last due I/O job service.
***************************************************************************)

HistoricalDiscoveryDueLastIoJob(node, clockValue) ==
  /\ asyncNow = clockValue
  /\ node \in AsyncTimedServiceNodes
  /\ AsyncIoQueueDepth(node) = 1
  /\ asyncIoServiceDeadlines[node] <= clockValue

HistoricalDiscoveryLastIoJobServiceOutcome(node, clockValue) ==
  /\ asyncNow' = clockValue
  /\ AsyncIoQueueDepth(node)' = 0
  /\ asyncIoServiceDeadlines'[node] > clockValue
  /\ node \notin
       HistoricalDiscoveryDormantIoGatesAt(clockValue)'
  /\ node \notin
       HistoricalDiscoveryActiveIoBlockersAt(clockValue)'
  /\ HistoricalDiscoveryDormantIoGatesAt(clockValue)' =
       HistoricalDiscoveryDormantIoGatesAt(clockValue)
  /\ HistoricalDiscoveryActiveIoBlockersAt(clockValue)' =
       HistoricalDiscoveryActiveIoBlockersAt(clockValue) \ {node}
  /\ HistoricalDiscoveryDormantIoDebt(clockValue)' =
       HistoricalDiscoveryDormantIoDebt(clockValue)
  /\ HistoricalDiscoveryActiveIoBlockerDebt(clockValue)' + 1 =
       HistoricalDiscoveryActiveIoBlockerDebt(clockValue)

THEOREM ServiceLastDueIoJobCannotRefillDormantDebt ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueLastIoJob(node, clockValue)
    /\ ServiceIoWorkerWork(node)
    => HistoricalDiscoveryLastIoJobServiceOutcome(
         node, clockValue)
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType,
   HeadTailProperties, LenProperties, Isa
   DEF HistoricalDiscoveryDueLastIoJob,
       HistoricalDiscoveryLastIoJobServiceOutcome,
       HistoricalDiscoveryDormantIoGatesAt,
       HistoricalDiscoveryActiveIoBlockersAt,
       HistoricalDiscoveryDormantIoDebt,
       HistoricalDiscoveryActiveIoBlockerDebt,
       ServiceIoWorkerWork, PublishEphemeralItems,
       AsyncIoQueueDepth, AsyncTimedServiceNodes,
       AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncConfiguration, NodeHasApplication,
       CurrentVoters, CurrentEpoch, vars

THEOREM ConcreteLastDueIoServiceActionsCannotRefillDormantDebt ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueLastIoJob(node, clockValue)
    => /\ (ServiceIoWorker(node)
              => HistoricalDiscoveryLastIoJobServiceOutcome(
                   node, clockValue))
       /\ (ServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryLastIoJobServiceOutcome(
                   node, clockValue))
       /\ (PostGstServiceIoWorker(node)
              => HistoricalDiscoveryLastIoJobServiceOutcome(
                   node, clockValue))
       /\ (PostGstServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryLastIoJobServiceOutcome(
                   node, clockValue))
BY ServiceLastDueIoJobCannotRefillDormantDebt
   DEF ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker

(***************************************************************************
Due node-service reset.
***************************************************************************)

HistoricalDiscoveryDueNodeService(node, clockValue) ==
  /\ asyncNow = clockValue
  /\ node \in AsyncTimedServiceNodes
  /\ asyncNodeServiceDeadlines[node] <= clockValue

HistoricalDiscoveryNodeServiceOutcome(node, clockValue) ==
  /\ asyncNow' = clockValue
  /\ asyncNodeServiceDeadlines'[node] > clockValue
  /\ node \notin
       HistoricalDiscoveryNodeBlockersAt(clockValue)'

THEOREM DueNodeServiceWorkResetsDeadlineAboveFixedClock ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueNodeService(node, clockValue)
    => /\ (RunNodeWork(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (RunHistoricalServer(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
BY Isa
   DEF HistoricalDiscoveryDueNodeService,
       HistoricalDiscoveryNodeServiceOutcome,
       HistoricalDiscoveryNodeBlockersAt,
       RunNodeWork, RunHistoricalServer,
       AsyncConfiguration

THEOREM ConcreteDueNodeServiceActionsResetDeadlineAboveFixedClock ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueNodeService(node, clockValue)
    => /\ (RunNode(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (RunHistoricalRecoveryNode(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (RunHistoricalServer(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (PostGstRunNode(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (PostGstRunHistoricalRecoveryNode(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
       /\ (PostGstRunHistoricalServer(node)
              => HistoricalDiscoveryNodeServiceOutcome(
                   node, clockValue))
BY DueNodeServiceWorkResetsDeadlineAboveFixedClock
   DEF RunNode, RunHistoricalRecoveryNode,
       PostGstRunNode, PostGstRunHistoricalRecoveryNode,
       PostGstRunHistoricalServer

(***************************************************************************
Coverage and exact failed edge.

The requested global monotonicity and latent-debt non-refill claims are false.
`HistoricalExecuteApplyContextMismatch` is the exact action branch: for an
out-of-roster historical target, a matching old-context Decision removes the
target but does not satisfy current-context `NodeHasApplication`.  The node
leaves `AsyncTimedServiceNodes`, and the latent-owner debt grows by one at the
same clock.  Current-context Apply still performs the intended atomic
target-to-applied-archive transfer.

The remaining requested local edges are closed:

  * Open strictly spends one latent-owner debt for a previously latent owner;
  * all four I/O FIFO append owners spend a stale empty gate first;
  * last-job I/O service cannot recreate a dormant stale gate; and
  * ordinary, historical-target, and applied-archive runner actions reset the
    serviced node deadline above the frozen clock.

These are only owner-prefix edges.  The lower capacity, timeout-byte,
shared-completion, claim, runner-reach, auxiliary, Stage-4, candidate, and
Serve dependency edges remain outside this module.
***************************************************************************)

=============================================================================
