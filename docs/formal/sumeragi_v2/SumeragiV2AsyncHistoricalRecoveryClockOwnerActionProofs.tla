---- MODULE SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryServiceClosureProofs,
        SumeragiV2AsyncHistoricalRecoveryClockActionProofs

(***************************************************************************
Action-local owner-prefix edges for the historical discovery clock.

The first fixed-clock component charges responsive validators which have not
yet entered `AsyncTimedServiceNodes`.  After GST, responsive membership,
online membership, and the current roster cannot regress.  Opening historical
recovery adds one target.

`ApplyDecision` and `ApplyDecisionReady` require
`DecisionCertifiedBodyRecoveryAuthority`.  Consequently every
`ExecuteApply` witness is a current-context Commit Decision.  Historical
Apply adds the same node to the responsive applied-archive arm before
removing it from the target arm, so the timed owner set is monotone.

The next local handoffs cover stale empty I/O gates and due node/I/O service:

  * every concrete enqueue owner removes a due empty gate before making that
    node an active I/O blocker;
  * service of any due nonempty I/O queue pops exactly one job, moves its I/O
    deadline strictly above the frozen clock, and removes its active blocker;
    the last-job case additionally empties the queue; and
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
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode, AsyncServiceActivationFrameVars,
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

HistoricalExecuteApplyCurrentCommit(command, qc) ==
  /\ qc \in DecisionQcValues
  /\ CommandMatches(
       command, command.node, qc.view, qc.subject)
  /\ DecisionCertifiedBodyRecoveryAuthority(command.node, qc)
  /\ ApplyDecision(command.node, qc)

THEOREM HistoricalExecuteApplySelectsCurrentCommitDecision ==
  \A command:
    ExecuteApply(command)
      => \E qc:
           /\ HistoricalExecuteApplyCurrentCommit(command, qc)
           /\ qc.context = context
           /\ qc.phase = "Commit"
           /\ [node |-> command.node, qc |-> qc]
                \in decisions
BY Isa
   DEF HistoricalExecuteApplyCurrentCommit,
       ExecuteApply, ApplyDecision,
       DecisionCertifiedBodyRecoveryAuthority

THEOREM HistoricalExecuteApplyTransfersTimedServiceOwner ==
  \A command:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalRecoveryTarget(command.node)
    /\ ExecuteApply(command)
    => /\ ~HistoricalRecoveryTarget(command.node)'
       /\ command.node \in AsyncResponsiveAppliedArchiveServers'
       /\ AsyncTimedServiceNodes' = AsyncTimedServiceNodes
BY HistoricalExecuteApplySelectsCurrentCommitDecision,
   AsyncStrongTypeProjectsAsyncType, HistoricalRecoveryTargetsAreValidators,
   Isa
   DEF AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncHistoricalRecoveryTypeInvariant,
       HistoricalExecuteApplyCurrentCommit,
       HistoricalRecoveryTarget, ExecuteApply, ApplyDecision,
       DecisionCertifiedBodyRecoveryAuthority,
       CommandMatches, NodeHasApplication,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveServerIds, CurrentVoters, CurrentEpoch,
       vars

THEOREM HistoricalTargetOwnerSurvivesOrTransfersAfterGst ==
  \A owner \in asyncHistoricalRecoveryTargets:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    => owner \in asyncHistoricalRecoveryTargets'
         \/ owner \in AsyncResponsiveAppliedArchiveServers'
BY HistoricalExecuteApplyTransfersTimedServiceOwner, Isa
   DEF HistoricalRecoveryTarget, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRunnerRuntimeStep, SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep,
       ExecuteCommand, ExecuteApply, OpenHistoricalRecovery,
       AsyncEnterIndexedServiceActivation,
       AsyncActivateServiceNode, AsyncServiceActivationFrameVars,
       PreGstCrash, PreGstResponsiveCrash,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart, AsyncAllVars

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

THEOREM HistoricalTimedServiceNodesAreMonotoneAfterGst ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => AsyncTimedServiceNodes \subseteq AsyncTimedServiceNodes'
BY HistoricalDiscoveryPostGstUpAndRosterAreStable,
   HistoricalAppliedArchiveOwnersAreMonotoneAfterGst,
   HistoricalTargetOwnerSurvivesOrTransfersAfterGst, Isa
   DEF AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes

(***************************************************************************
Latent-owner debt.
***************************************************************************)

THEOREM HistoricalLatentTimedOwnersAreAntitoneAfterGst ==
  /\ AsyncStrongTypeInvariant
  /\ gst
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalDiscoveryLatentTimedOwners'
       \subseteq HistoricalDiscoveryLatentTimedOwners
BY HistoricalTimedServiceNodesAreMonotoneAfterGst, Isa
   DEF HistoricalDiscoveryLatentTimedOwners,
       HistoricalDiscoveryPotentialServiceCohort

THEOREM HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock ==
  \A clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ asyncNow = clockValue
    /\ asyncNow' = clockValue
    /\ [AsyncNext]_AsyncAllVars
    => HistoricalDiscoveryLatentOwnerDebt'
         <= HistoricalDiscoveryLatentOwnerDebt
BY HistoricalLatentTimedOwnersAreAntitoneAfterGst,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_Subset, FS_CardinalityType, SMT
   DEF HistoricalDiscoveryLatentOwnerDebt

THEOREM HistoricalLatentOwnerEntryStrictlyDecreasesDebt ==
  \A owner \in Responsive:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ [AsyncNext]_AsyncAllVars
    /\ owner \in HistoricalDiscoveryLatentTimedOwners
    /\ owner \in AsyncTimedServiceNodes'
    => /\ HistoricalDiscoveryLatentTimedOwners'
             \subseteq HistoricalDiscoveryLatentTimedOwners
       /\ owner \notin HistoricalDiscoveryLatentTimedOwners'
       /\ HistoricalDiscoveryLatentOwnerDebt'
             < HistoricalDiscoveryLatentOwnerDebt
BY HistoricalLatentTimedOwnersAreAntitoneAfterGst,
   StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_Subset, FS_CardinalityType, SMT
   DEF HistoricalDiscoveryLatentTimedOwners,
       HistoricalDiscoveryLatentOwnerDebt

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
  \A job:
    \A node \in ValidatorIds, clockValue \in Nat:
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
Due nonempty I/O queue service.
***************************************************************************)

HistoricalDiscoveryDueIoQueue(node, clockValue) ==
  /\ asyncNow = clockValue
  /\ node \in AsyncTimedServiceNodes
  /\ AsyncIoQueueDepth(node) > 0
  /\ asyncIoServiceDeadlines[node] <= clockValue

HistoricalDiscoveryDueIoQueueServiceOutcome(node, clockValue) ==
  /\ asyncNow' = clockValue
  /\ asyncIoQueues'[node] = Tail(asyncIoQueues[node])
  /\ AsyncIoQueueDepth(node)' + 1 = AsyncIoQueueDepth(node)
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

THEOREM ServiceDueIoQueueWorkRemovesExactActiveBlocker ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueIoQueue(node, clockValue)
    /\ ServiceIoWorkerWork(node)
    => HistoricalDiscoveryDueIoQueueServiceOutcome(
         node, clockValue)
BY StrongTypeHasFiniteHistoricalDiscoveryCohorts,
   FS_RemoveElement, FS_CardinalityType,
   HeadTailProperties, LenProperties, Isa
   DEF HistoricalDiscoveryDueIoQueue,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
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

THEOREM ConcreteDueIoServiceActionsRemoveExactActiveBlocker ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueIoQueue(node, clockValue)
    => /\ (ServiceIoWorker(node)
              => HistoricalDiscoveryDueIoQueueServiceOutcome(
                   node, clockValue))
       /\ (ServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryDueIoQueueServiceOutcome(
                   node, clockValue))
       /\ (PostGstServiceIoWorker(node)
              => HistoricalDiscoveryDueIoQueueServiceOutcome(
                   node, clockValue))
       /\ (PostGstServiceHistoricalRecoveryIoWorker(node)
              => HistoricalDiscoveryDueIoQueueServiceOutcome(
                   node, clockValue))
BY ServiceDueIoQueueWorkRemovesExactActiveBlocker
   DEF ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       PostGstServiceIoWorker,
       PostGstServiceHistoricalRecoveryIoWorker

(***************************************************************************
The former last-job surface remains as a direct corollary of the nonempty
queue result.  Its proofs do not expand the worker action again.
***************************************************************************)

HistoricalDiscoveryDueLastIoJob(node, clockValue) ==
  /\ HistoricalDiscoveryDueIoQueue(node, clockValue)
  /\ AsyncIoQueueDepth(node) = 1

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

THEOREM LastDueIoJobOutcomeFollowsGenericOutcome ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueLastIoJob(node, clockValue)
    /\ HistoricalDiscoveryDueIoQueueServiceOutcome(
         node, clockValue)
    => HistoricalDiscoveryLastIoJobServiceOutcome(
         node, clockValue)
BY HeadTailProperties, LenProperties, Isa
   DEF HistoricalDiscoveryDueLastIoJob,
       HistoricalDiscoveryDueIoQueue,
       HistoricalDiscoveryDueIoQueueServiceOutcome,
       HistoricalDiscoveryLastIoJobServiceOutcome,
       AsyncIoQueueDepth

THEOREM ServiceLastDueIoJobCannotRefillDormantDebt ==
  \A node \in ValidatorIds, clockValue \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalDiscoveryDueLastIoJob(node, clockValue)
    /\ ServiceIoWorkerWork(node)
    => HistoricalDiscoveryLastIoJobServiceOutcome(
         node, clockValue)
BY ServiceDueIoQueueWorkRemovesExactActiveBlocker,
   LastDueIoJobOutcomeFollowsGenericOutcome, Isa
   DEF HistoricalDiscoveryDueLastIoJob

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
BY ConcreteDueIoServiceActionsRemoveExactActiveBlocker,
   LastDueIoJobOutcomeFollowsGenericOutcome, Isa
   DEF HistoricalDiscoveryDueLastIoJob

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
Coverage.

The accepted current-Commit authority guard closes the owner prefix:

  * `ExecuteApply` selects a current Commit Decision;
  * historical Apply atomically transfers target membership to the
    applied-archive arm;
  * `AsyncTimedServiceNodes` is monotone after GST;
  * latent-owner debt is antitone at a fixed clock and strictly descends when
    a previously latent owner enters;
  * Open spends exactly one latent-owner debt for a previously latent owner;
  * all four I/O FIFO append owners spend a stale empty gate first;
  * any due nonempty I/O service pops one job, spends its active blocker, and
    cannot recreate a dormant stale gate; and
  * ordinary, historical-target, and applied-archive runner actions reset the
    serviced node deadline above the frozen clock.

These are only owner-prefix edges.  The lower capacity, timeout-byte,
shared-completion, claim, runner-reach, auxiliary, Stage-4, candidate, and
Serve dependency edges remain outside this module.
***************************************************************************)

=============================================================================
