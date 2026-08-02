# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# A fixed module/symbol binding is insufficient when a theorem can retain its
# name while its statement is weakened.  These release seams therefore pin
# the complete normalized theorem statement. Historical residual declarations
# are support leaves: whether a leaf has gained a proof does not by itself
# promote its reviewed top-level consumer.
EXACT_FIXED_PROOF_OBLIGATION_STATEMENTS = {
    "async-runner-scheduler-preservation": (
        "/\\ StrongInductiveInvariant "
        "/\\ AsyncTypeInvariant "
        "/\\ AsyncControlServiceStateTypeInvariant "
        "/\\ AsyncControlServiceSlotTransition "
        "/\\ AsyncRunnerStep "
        "=> AsyncSchedulerTypeInvariant'"
    ),
    "chain-durable-receipt-agreement": (
        "IndexedChainSpec "
        "=> []ExactPerSlotDurableCommitReceiptSubjectAgreement"
    ),
    "terminal-ingress-process-lifetime-absorbency": (
        "TerminalIngressLifecycleSpec "
        "=> TerminalIngressProcessLifetimeAbsorbencyProperty"
    ),
    "adequate-leader-exact-closure-residual": (
        "\\A initialContext: "
        "AdequateLeaderExactClosureResidualProperty("
        "AsyncLiveSpecAt(initialContext))"
    ),
    "exact-decision-off-scheduler-residual-convergence": (
        "\\A initialContext: "
        "ExactDecisionOffSchedulerResidualConvergenceProperty("
        " AsyncSpecAt(initialContext))"
    ),
    "historical-recovery-authority-acquisition": (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "=> IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty"
    ),
    "historical-recovery-certificate-rank-progress": (
        "IndexedLiveChainSpec "
        "=> IndexedHistoricalCertificateRankProgressResidualProperty"
    ),
    "historical-recovery-decision-stage-ownership": (
        "IndexedChainSpec "
        "=> IndexedHistoricalDecisionStageOwnershipResidualProperty"
    ),
    "historical-recovery-decision-rank-progress": (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "=> IndexedHistoricalDecisionRankProgressResidualProperty"
    ),
    "genesis-height-successor-handoff": (
        "AsyncLiveChainSpec => GenesisHeightSuccessorHandoffProperty"
    ),
    "height-liveness": (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedGstEventuallyCondition "
        "=> IndexedHeightLivenessProperty"
    ),
}

# Pin the direct property surfaces consumed by the six theorem statements.
# Without these contracts, replacing a property with TRUE would preserve the
# theorem declaration and silently turn a real release obligation into a
# tautology.
EXACT_FIXED_PROOF_PROPERTY_OPERATOR_BODIES = {
    (
        "SumeragiV2AsyncNetwork",
        "AsyncProgressOwnershipInvariant",
    ): (
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncOutstandingCarrierInvariant "
        "/\\ SerializedBusyOwnershipInvariant "
        "/\\ BusyCompletionWitnessInvariant"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncActiveFixedCorridorDeadlineReceipts",
    ): (
        "{receipt \\in AsyncFixedCorridorDeadlineReceipts: "
        "LET roster == Responsive \\cap VotingRoster(receipt.context.epoch) "
        "IN /\\ receipt.context = context /\\ roster # {} "
        "/\\ DualQuorum(receipt.context.epoch, roster) "
        "/\\ receipt.target \\in roster \\cap Honest "
        "/\\ receipt.view = nodeView[receipt.target] "
        "/\\ Leader(receipt.context, receipt.view) = receipt.target "
        "/\\ \\A node \\in roster: nodeView[node] = receipt.view "
        "/\\ asyncNow < receipt.deadline}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderLocalTargetDecisionSource",
    ): (
        "/\\ gst /\\ target \\in AsyncCurrentResponsiveVoters "
        "/\\ target \\in AsyncActiveServiceNodes "
        "/\\ ~NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "LeaderWirePhysicalDependencyCertificate",
    ): (
        "LET item == packet.item IN "
        "[stage |-> LeaderWirePhysicalLifecycleStageRank(packet, item), "
        "packetRank |-> LeaderWirePhysicalPacketDependencyRank(snapshot, packet), "
        "ingressRank |-> LeaderWirePhysicalIngressDependencyRank(item), "
        "predecessors |-> snapshot.predecessors, "
        "schedulerCuts |-> snapshot.schedulerCuts, "
        "physicalCuts |-> snapshot.physicalCuts, "
        "causalProducerRank |-> LeaderWirePhysicalCausalProducerRank(snapshot, item), "
        "causalProducerCarrier |-> "
        "ExactDecisionTargetNeutralComposedCausalEpisodeCarrier, "
        "producerBudget |-> "
        "ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "LeaderWirePhysicalFrozenCertificateFrontier",
    ): (
        "LET certificate == "
        "LeaderWirePhysicalDependencyCertificate(snapshot, packet) IN "
        "/\\ snapshot.clock \\in Nat "
        "/\\ asyncNow = snapshot.clock /\\ gst "
        "/\\ ExactDecisionTargetNeutralSnapshotActive( "
        "snapshot, snapshot.clock) "
        "/\\ packet \\in snapshot.packets "
        "/\\ packet \\in OverdueResponsivePackets "
        "/\\ \\/ LeaderWireCurrentContextWitnessIdentity(packet.item) "
        "\\/ LeaderWireProductiveTransportIdentity(packet.item) "
        "/\\ certificate.predecessors = snapshot.predecessors "
        "/\\ certificate.schedulerCuts = snapshot.schedulerCuts "
        "/\\ certificate.physicalCuts = snapshot.physicalCuts "
        "/\\ certificate.packetRank = "
        "LeaderWirePhysicalPacketDependencyRank(snapshot, packet) "
        "/\\ certificate.causalProducerRank = "
        "LeaderWirePhysicalCausalProducerRank(snapshot, packet.item) "
        "/\\ certificate.producerBudget = "
        "ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFreshNodeServiceWindow",
    ): (
        "/\\ node \\in ValidatorIds "
        "/\\ leaderContext = context "
        "/\\ leaderView \\in Views "
        "/\\ nodeView[node] = leaderView "
        "/\\ asyncNow + AsyncFixedCorridorServiceBudget "
        "< asyncNodeDeadlines[node] "
        "/\\ ~NodeTimedOut(node, leaderView) "
        "/\\ ~asyncTimeoutEmitted[node] "
        '/\\ "TimeoutElapsed" \\notin asyncOutstandingTags[node] '
        "/\\ ~AdequateLeaderOlderOrEqualTimeoutLifecycleOwned( "
        "node, leaderContext, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFreshSynchronizedTargetCorridor",
    ): (
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ \\A node \\in "
        "AdequateLeaderFrozenResponsiveRoster(leaderContext): "
        "AdequateLeaderFreshNodeServiceWindow( "
        "node, leaderContext, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderTargetFreshSelfCorridorGoal",
    ): (
        "\\/ NodeHasDecision(target) "
        "\\/ \\E leaderView \\in Views: "
        "AdequateLeaderFreshSynchronizedTargetCorridor( "
        "target, context, target, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderFrozenViewEpisodeSource",
    ): (
        "/\\ AdequateLeaderLocalTargetDecisionSource(target) "
        "/\\ residentTcs = formedTCs "
        "/\\ residentOrigins = timeoutIntents "
        "/\\ IsFiniteSet(residentTcs) "
        "/\\ IsFiniteSet(residentOrigins)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRank",
    ): (
        "<<AdequateLeaderResidentFutureDebt( "
        "target, residentTcs, residentOrigins), "
        "AdequateLeaderTargetSelfViewDistance(target)>>"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankCarrier",
    ): "Nat \\X Nat",
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankOrdering",
    ): (
        "LexPairOrdering( "
        "OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankFrontier",
    ): (
        "/\\ AdequateLeaderLocalTargetDecisionSource(target) "
        "/\\ IsFiniteSet(residentTcs) "
        "/\\ IsFiniteSet(residentOrigins) "
        "/\\ ~AdequateLeaderTargetFreshSelfCorridorGoal(target) "
        "/\\ rank = AdequateLeaderViewExposureRank( "
        "target, residentTcs, residentOrigins)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureStrictDescentGoal",
    ): (
        "\\/ AdequateLeaderTargetFreshSelfCorridorGoal(target) "
        "\\/ \\E lowerRank \\in SetLessThan( "
        "sourceRank, AdequateLeaderViewExposureRankOrdering, "
        "AdequateLeaderViewExposureRankCarrier): "
        "AdequateLeaderViewExposureRankFrontier( "
        "target, residentTcs, residentOrigins, lowerRank)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankStepProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "residentTcs \\in SUBSET TcRecordSet, "
        "residentOrigins \\in SUBSET TimeoutVoteRecordSet, "
        "sourceRank \\in AdequateLeaderViewExposureRankCarrier: "
        "AdequateLeaderViewExposureRankFrontier( "
        "target, residentTcs, residentOrigins, sourceRank) "
        "~> AdequateLeaderViewExposureStrictDescentGoal( "
        "target, residentTcs, residentOrigins, sourceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderLocalFreshSelfCorridorExposureProperty",
    ): (
        "specification => \\A target \\in ValidatorIds: "
        "AdequateLeaderLocalTargetDecisionSource(target) "
        "~> AdequateLeaderTargetFreshSelfCorridorGoal(target)"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "GenesisHeightSuccessorHandoffProperty",
    ): (
        "ContextRecord(0, <<>>).height < MaxHeight "
        "=> \\A node \\in AsyncCurrentResponsiveVoters: "
        "gst ~> NeedsSuccessorAsyncInstance(node)"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedPostGstResponsiveActiveRosterCoherence",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAsync(initialContext)!gst "
        "=> Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedHeightLivenessProperty",
    ): (
        "(/\\ VerificationContext \\in AdmissibleContextRecords "
        "/\\ VerificationContext \\in JoinedContexts "
        "/\\ IndexedCore(VerificationContext, 7)) "
        "~> IndexedContextCompleted(VerificationContext)"
    ),
    (
        "SumeragiV2ChainLivenessProofs",
        "IndexedExactHeightLivenessProperty",
    ): (
        "(/\\ VerificationContext \\in AdmissibleContextRecords "
        "/\\ VerificationContext \\in JoinedContexts "
        "/\\ IndexedCore(VerificationContext, 7)) "
        "~> IndexedExactContextCompleted(VerificationContext)"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
        "HistoricalDiscoveryPacketConcreteActionKindCarrier",
    ): (
        '{"Admit", "AdmitHistorical", "RunNode", '
        '"RunHistoricalRecovery", "RunHistoricalServer", '
        '"ServiceIo", "ServiceHistoricalIo"}'
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
        "HistoricalDiscoveryPacketConcreteAction",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN CASE actionKind = \"Admit\" -> "
        "PostGstAdmitHiddenPacket(recipient, actionSource) "
        "[] actionKind = \"AdmitHistorical\" -> "
        "PostGstAdmitHistoricalRecoveryPacket( recipient, actionSource) "
        "[] actionKind = \"RunNode\" -> PostGstRunNode(recipient) "
        "[] actionKind = \"RunHistoricalRecovery\" -> "
        "PostGstRunHistoricalRecoveryNode(recipient) "
        "[] actionKind = \"RunHistoricalServer\" -> "
        "PostGstRunHistoricalServer(recipient) "
        "[] actionKind = \"ServiceIo\" -> "
        "PostGstServiceIoWorker(recipient) "
        "[] actionKind = \"ServiceHistoricalIo\" -> "
        "PostGstServiceHistoricalRecoveryIoWorker(recipient) "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
        "HistoricalDiscoveryPacketConcreteActionPending",
    ): (
        "/\\ HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget( "
        "node, clockValue, sourceRank, packet, known, budget) "
        "/\\ HistoricalDiscoveryPacketProducerIdentitySet(packet) = {} "
        "/\\ dependencyRank = HistoricalDiscoveryPacketDependencyRank(packet) "
        "/\\ dependencyRank \\in "
        "HistoricalDiscoveryPacketDependencyCarrier "
        "/\\ actionKind \\in "
        "HistoricalDiscoveryPacketConcreteActionKindCarrier "
        "/\\ actionSource \\in AsyncIngressSources "
        "/\\ ENABLED HistoricalDiscoveryPacketConcreteAction( "
        "packet, actionKind, actionSource)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutFixedClockPacketConcreteActionPending",
    ): (
        "/\\ TimeoutFixedClockLifecycleEpisodeAtBudget( "
        "source, sourceView, clockValue, deadlineValue, "
        "sourceRank, packet, known, budget) "
        "/\\ TimeoutFixedPacketLiveOwners(packet) = {} "
        "/\\ actionKind \\in "
        "HistoricalDiscoveryPacketConcreteActionKindCarrier "
        "/\\ actionSource \\in AsyncIngressSources "
        "/\\ ENABLED HistoricalDiscoveryPacketConcreteAction( "
        "packet, actionKind, actionSource)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalPacketConcreteProductAction",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN CASE actionKind = \"Admit\" -> "
        "IndexedAdmitPacketStep( initialContext, recipient, actionSource) "
        "[] actionKind = \"AdmitHistorical\" -> "
        "IndexedAdmitHistoricalRecoveryPacketStep( "
        "initialContext, recipient, actionSource) "
        "[] actionKind = \"RunNode\" -> "
        "IndexedRunNodeStep(initialContext, recipient) "
        "[] actionKind = \"RunHistoricalRecovery\" -> "
        "IndexedRunHistoricalRecoveryStep( initialContext, recipient) "
        "[] actionKind = \"RunHistoricalServer\" -> "
        "IndexedHistoricalServerStep(initialContext, recipient) "
        "[] actionKind = \"ServiceIo\" -> "
        "IndexedIoWorkerStep(initialContext, recipient) "
        "[] actionKind = \"ServiceHistoricalIo\" -> "
        "IndexedHistoricalRecoveryIoWorkerStep( "
        "initialContext, recipient) "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalPacketConcreteActionFairDomain",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "ingressSources == "
        "IndexedHistoricalTransport(initialContext)!AsyncIngressSources "
        "voters == IndexedHistoricalTransport(initialContext)! "
        "AsyncVotersAt(initialContext) "
        "IN CASE actionKind = \"Admit\" -> "
        "/\\ recipient \\in Responsive "
        "/\\ actionSource \\in ingressSources "
        "[] actionKind = \"AdmitHistorical\" -> "
        "/\\ recipient \\in ValidatorIds "
        "/\\ actionSource \\in ingressSources "
        "[] actionKind = \"RunNode\" -> recipient \\in voters "
        "[] actionKind \\in "
        "{\"RunHistoricalRecovery\", \"RunHistoricalServer\", "
        "\"ServiceIo\", \"ServiceHistoricalIo\"} "
        "-> recipient \\in Responsive "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2AsyncTimeoutOwnershipProofs",
        "RetainedViewCertificateAuthority",
    ): (
        "/\\ source \\in AsyncCurrentResponsiveVoters "
        "/\\ ~NodeHasDecision(source) "
        "/\\ \\E tc \\in TcRecordSet: "
        "/\\ TimeoutCertificateSemanticIdentity(tc, minimumView) "
        "/\\ nodeView[source] = tc.view + 1 "
        "/\\ tc = lastInstalledTc[source] "
        "/\\ TcOutbox(source, tc) \\subseteq asyncRetainedControl"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderExactClosureResidualProperty",
    ): (
        "/\\ AdequateLeaderExactResidualKernelProperty(specification) "
        "/\\ AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderWirePhysicalConvergenceProperty",
    ): (
        "specification => /\\ \\A packet \\in AsyncPacketSet: "
        "(gst "
        "/\\ LeaderWireCurrentContextWitnessIdentity(packet.item) "
        "/\\ packet \\in OverdueResponsivePackets) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireTransportResolution(packet)) "
        "/\\ \\A packet \\in AsyncPacketSet: "
        "(gst "
        "/\\ LeaderWireProductiveTransportIdentity(packet.item) "
        "/\\ packet \\in OverdueResponsivePackets) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireTransportHandoff(packet)) "
        "/\\ \\A item \\in AsyncNetworkItems: "
        "(gst "
        "/\\ LeaderWireCurrentContextWitnessIdentity(item) "
        "/\\ LeaderWireIngressOwned(item)) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireRunnerAdmissionHandoff(item))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTimeoutRotationConvergenceProperty",
    ): (
        "specification => \\A node \\in ValidatorIds, "
        "roundView \\in Views: "
        "(gst "
        "/\\ TimeoutQuorumViewRotationResidual(node, roundView)) "
        "~> (ResponsiveNodesDecide "
        "\\/ TimeoutQuorumViewRotationHandoff(node, roundView))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderOpenPhysicalResidualConvergenceProperty",
    ): (
        "specification => /\\ \\A packet \\in AsyncPacketSet: "
        "(gst "
        "/\\ LeaderWireCurrentContextWitnessIdentity(packet.item) "
        "/\\ LeaderWireDueTransportResidual(packet)) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireTransportResolution(packet)) "
        "/\\ \\A packet \\in AsyncPacketSet: "
        "(gst "
        "/\\ LeaderWireProductiveTransportIdentity(packet.item) "
        "/\\ LeaderWireDueTransportResidual(packet)) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireTransportHandoff(packet)) "
        "/\\ \\A item \\in AsyncNetworkItems: "
        "(gst "
        "/\\ LeaderWireCurrentContextWitnessIdentity(item) "
        "/\\ LeaderWireRunnerAdmissionResidual(item)) "
        "~> (ResponsiveNodesDecide "
        "\\/ LeaderWireRunnerAdmissionHandoff(item)) "
        "/\\ \\A node \\in ValidatorIds, roundView \\in Views: "
        "(gst "
        "/\\ TimeoutQuorumViewRotationResidual(node, roundView)) "
        "~> (ResponsiveNodesDecide "
        "\\/ TimeoutQuorumViewRotationHandoff(node, roundView))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderExactPhysicalResidualConvergenceProperty",
    ): (
        "/\\ AdequateLeaderOpenPhysicalResidualConvergenceProperty( "
        "specification) "
        "/\\ CertifiedResponsePhysicalDebtConvergenceProperty(specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderExactResidualKernelProperty",
    ): (
        "/\\ ExactLeaderSchedulerOriginReadinessProperty(specification) "
        "/\\ AdequateLeaderOpenPhysicalResidualConvergenceProperty(specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSemanticCompositionProperty",
    ): (
        "/\\ AdequateLeaderViewReachCompositionProperty(specification) "
        "/\\ AdequateLeaderTargetSemanticCompositionProperty(specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncCandidateIdentityBudgetBridgeProperty",
    ): (
        "/\\ (specification => "
        "[]AsyncCandidateServiceTombstoneLifecycleInvariant) /\\ "
        "(specification => \\A carrier: IsFiniteSet(carrier) => []( "
        "Cardinality( "
        "AsyncCandidateServiceTombstonesInIdentityCarrier( "
        "carrier)) <= Cardinality(carrier))) /\\ (specification => [](\\A "
        "candidate \\in AsyncCandidateSet: /\\ "
        "AsyncCandidateServiceActiveTombstone(candidate) /\\ "
        "[AsyncNext]_AsyncAllVars /\\ "
        "~AsyncCandidateServiceExitThisStep(candidate) => "
        "AsyncCandidateServiceActiveTombstone(candidate)')) /\\ (specification "
        "=> [](\\A left, right \\in AsyncCandidateSet: /\\ left.node = "
        "right.node /\\ left.consumerContext = right.consumerContext /\\ "
        "left.height = right.height /\\ left.view = right.view /\\ left.subject "
        "= right.subject /\\ left.kind = right.kind /\\ left.class = "
        "right.class /\\ left.item # NoAsyncItem /\\ right.item # NoAsyncItem "
        "/\\ left.item.kind = \"CertifiedResponse\" /\\ right.item = [left.item "
        "EXCEPT !.source = right.item.source] /\\ "
        "AsyncRouteNeutralCandidateEvidence(left.evidence) = "
        "AsyncRouteNeutralCandidateEvidence(right.evidence) /\\ "
        "left.bodyIdentity = right.bodyIdentity /\\ left.manifestIdentity = "
        "right.manifestIdentity /\\ left.commitmentIdentity = "
        "right.commitmentIdentity => AsyncCandidateServiceIdentity(left) = "
        "AsyncCandidateServiceIdentity(right))) /\\ (specification => [](\\A "
        "identity \\in AsyncCandidateAdmissionIdentitySet: /\\ "
        "AsyncCandidateAdmissionIdentityObsolete(identity) /\\ identity \\notin "
        "AsyncScheduledCandidateAdmissionIdentities /\\ gst /\\ "
        "[AsyncNext]_AsyncAllVars => /\\ "
        "AsyncCandidateAdmissionIdentityObsolete(identity)' /\\ identity "
        "\\notin AsyncScheduledCandidateAdmissionIdentities')) /\\ "
        "(specification => [](\\A identity \\in "
        "AsyncCandidateAdmissionIdentitySet: /\\ "
        "AsyncCandidateAdmissionIdentityTerminallyCovered(identity) /\\ "
        "identity \\notin AsyncScheduledCandidateAdmissionIdentities /\\ gst /\\ "
        "[AsyncNext]_AsyncAllVars => /\\ "
        "AsyncCandidateAdmissionIdentityTerminallyCovered( identity)' /\\ "
        "identity \\notin AsyncScheduledCandidateAdmissionIdentities')) /\\ "
        "(specification => [](\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant /\\ gst "
        "/\\ AsyncNext /\\ CandidateScheduled(candidate) "
        "/\\ ~CandidateScheduledAfter(candidate) => "
        "\\/ AsyncCandidateIgnoredWithoutApplicationThisStep( candidate) "
        "\\/ AsyncCandidateServiceTombstoned(candidate)' "
        "\\/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter( candidate) "
        "\\/ AsyncCandidateMonotoneSemanticCoverageAfterIn( "
        "asyncControlServiceState', candidate) "
        "\\/ AsyncCandidateTerminalTombstoned(candidate)'))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderViewReachCompositionProperty",
    ): (
        "specification => /\\ \\A target \\in ValidatorIds: "
        "AdequateLeaderLocalTargetDecisionSource(target) "
        "~> (NodeHasDecision(target) "
        "\\/ AdequateLeaderTargetDecisionSource(target)) "
        "/\\ \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views: "
        "AdequateLeaderTargetAnyCorridorExitHandoff( "
        "target, leaderContext, leader, leaderView) "
        "~> NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticCompositionProperty",
    ): (
        "/\\ AdequateLeaderTargetCorridorEntryProperty(specification) "
        "/\\ AdequateLeaderTargetProducerTransportClosureProperty(specification) "
        "/\\ AdequateLeaderTargetOccurrenceRankServiceProperty(specification) "
        "/\\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty( "
        "specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCorridorEntryProperty",
    ): (
        "specification => \\A target \\in ValidatorIds: "
        "AdequateLeaderTargetDecisionSource(target) "
        "~> (NodeHasDecision(target) "
        "\\/ \\E leaderContext \\in ContextRecords, "
        "leader \\in ValidatorIds, leaderView \\in Views, "
        "subject \\in Subjects: AdequateLeaderTargetProductiveSubjectOpenFrontier( "
        "target, leaderContext, leader, leaderView, subject))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerTransportClosureProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects: "
        "/\\ AdequateLeaderTargetProtocolSubjectSource( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ AdequateLeaderTargetProducerTransportResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "~> (NodeHasDecision(target) "
        "\\/ \\E nextSubject \\in Subjects, occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: /\\ "
        "AdequateLeaderTargetProtocolSubjectSource( target, leaderContext, "
        "leader, leaderView, nextSubject) /\\ "
        "AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, nextSubject, occurrenceRank))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankServiceExitProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, "
        "occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier, "
        "owner \\in AdequateLeaderFrozenCandidateOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject): "
        "/\\ AdequateLeaderTargetProtocolSubjectSource( "
        "target, leaderContext, leader, leaderView, subject) /\\ "
        "AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank) "
        "/\\ AdequateLeaderTargetOccurrenceOwnerSelected( "
        "target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, owner) "
        "~> AdequateLeaderTargetOccurrenceRankServiceExitGoal( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "owner)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCorridorAuthorityReceipt",
    ): (
        "[target |-> target, context |-> leaderContext, leader |-> leader, "
        "view |-> leaderView, roster |-> "
        "AdequateLeaderFrozenResponsiveRoster(leaderContext)]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCorridorAuthorityReceiptValid",
    ): (
        "/\\ receipt.context \\in ContextRecords "
        "/\\ receipt.roster = "
        "AdequateLeaderFrozenResponsiveRoster(receipt.context) "
        "/\\ receipt.roster # {} "
        "/\\ receipt.target \\in receipt.roster "
        "/\\ receipt.leader \\in receipt.roster \\cap Honest "
        "/\\ receipt.view \\in Views "
        "/\\ Leader(receipt.context, receipt.view) = receipt.leader "
        "/\\ AsyncViewTimeout(receipt.view) > AsyncWorstCaseServiceBudget"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderResponsiveViewSynchronized",
    ): (
        "/\\ AdequateLeaderFrozenResponsiveRoster(leaderContext) # {} "
        "/\\ \\A node \\in "
        "AdequateLeaderFrozenResponsiveRoster(leaderContext): "
        "AdequateLeaderActiveNodeServiceWindow( "
        "node, leaderContext, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderActiveTargetLeaderServiceWindow",
    ): (
        "/\\ AdequateLeaderActiveNodeServiceWindow( "
        "target, leaderContext, leaderView) "
        "/\\ AdequateLeaderActiveNodeServiceWindow( "
        "leader, leaderContext, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetCorridor",
    ): (
        "LET authority == AdequateLeaderCorridorAuthorityReceipt( "
        "target, leaderContext, leader, leaderView) IN "
        "/\\ gst "
        "/\\ AdequateLeaderCorridorAuthorityReceiptValid(authority) "
        "/\\ authority.context = context "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leaderContext = context "
        "/\\ target \\in Responsive \\cap VotingRoster(leaderContext.epoch) "
        "/\\ leader \\in Responsive \\cap VotingRoster(leaderContext.epoch) "
        "/\\ leader \\in Honest /\\ leaderView \\in Views "
        "/\\ nodeView[leader] = leaderView "
        "/\\ nodeView[target] = leaderView "
        "/\\ Leader(leaderContext, leaderView) = leader "
        "/\\ AsyncViewTimeout(leaderView) > AsyncWorstCaseServiceBudget "
        "/\\ AdequateLeaderResponsiveViewSynchronized( "
        "leaderContext, leaderView) "
        "/\\ AdequateLeaderActiveTargetLeaderServiceWindow( "
        "target, leaderContext, leader, leaderView) "
        "/\\ ~NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateRole",
    ): (
        "/\\ candidate.node \\in {target, leader} "
        "/\\ IF candidate.kind \\in {\"BeginDecision\", \"PersistDecision\"} "
        "THEN candidate.node = target "
        "ELSE IF candidate.node # target "
        "THEN ~NodeHasDecision(candidate.node) ELSE TRUE"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateIdentity",
    ): (
        "/\\ rank \\in AdequateLeaderTargetSemanticRankCarrier "
        "/\\ subject \\in Subjects "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ ExactLeaderCurrentRankWitness( candidate, rank, leaderContext, "
        "candidate.node, leaderView, subject) "
        "/\\ AdequateLeaderCandidatePayloadWithinFrozenView( "
        "candidate, leaderView) "
        "/\\ AdequateLeaderFrozenCandidateRootConstructed( "
        "candidate, target, leaderContext, leader, leaderView) "
        "/\\ AdequateLeaderTargetCandidateRole(candidate, target, leader)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetCandidateRole",
    ): (
        "/\\ candidate.node \\in {target, leader} "
        "/\\ (candidate.kind \\in "
        '{"BeginDecision", "PersistDecision"} '
        "=> candidate.node = target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetCandidateIdentity",
    ): (
        "/\\ rank \\in AdequateLeaderTargetSemanticRankCarrier "
        "/\\ subject \\in Subjects "
        "/\\ ExactLeaderFrozenSemanticIdentity( "
        "candidate, rank, leaderContext, candidate.node, "
        "leaderView, subject) "
        "/\\ AdequateLeaderCandidatePayloadWithinFrozenView( "
        "candidate, leaderView) "
        "/\\ AdequateLeaderFrozenCandidateRootConstructed( "
        "candidate, target, leaderContext, leader, leaderView) "
        "/\\ AdequateLeaderFrozenTargetCandidateRole( "
        "candidate, target, leader)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidatePayloadWithinFrozenView",
    ): (
        "/\\ AdequateLeaderCandidateItemWithinFrozenView( "
        "candidate.item, leaderView) "
        "/\\ AdequateLeaderCandidateEvidenceWithinFrozenView( "
        "candidate.evidence, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCommitRequestItemPayload",
    ): (
        "[kind |-> item.kind, source |-> item.source, "
        "recipient |-> item.envelope.recipient, "
        "height |-> item.envelope.height, "
        "view |-> AdequateLeaderFrozenViewCoordinate( "
        "item.envelope.view, leaderView), "
        "subject |-> item.envelope.subject, "
        "chunk |-> item.envelope.chunk, "
        "nonce |-> item.envelope.nonce]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidatePayload",
    ): (
        "[class |-> candidate.class, workKind |-> candidate.kind, "
        "causalOrigin |-> candidate.causalOrigin, "
        "item |-> AdequateLeaderFrozenCandidateItemPayload( "
        "candidate.item, leaderView), "
        "evidence |-> AdequateLeaderFrozenCandidateEvidencePayload( "
        "candidate.evidence, leaderView), "
        "body |-> candidate.bodyIdentity, "
        "manifest |-> candidate.manifestIdentity, "
        "commitment |-> candidate.commitmentIdentity]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderImmutableCandidatePayload",
    ): (
        "[class |-> candidate.class, workKind |-> candidate.kind, "
        "causalOrigin |-> candidate.causalOrigin, "
        "item |-> AdequateLeaderRouteNeutralCandidateItem(candidate.item), "
        "evidence |-> "
        "AdequateLeaderRouteNeutralCandidateEvidence(candidate.evidence), "
        "body |-> candidate.bodyIdentity, "
        "manifest |-> candidate.manifestIdentity, "
        "commitment |-> candidate.commitmentIdentity]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidatePayloadCarrier",
    ): (
        "IF /\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds "
        "/\\ leaderView \\in Nat "
        "/\\ subject \\in Subjects "
        "THEN [class: AsyncCommandClasses, workKind: AsyncWorkKinds, "
        "causalOrigin: AdequateLeaderFrozenCandidateCausalOriginCarrier( "
        "target, leaderContext, leader, leaderView, subject), "
        "item: AdequateLeaderFrozenCandidateItemPayloadCarrier(leaderView), "
        "evidence: "
        "AdequateLeaderFrozenCandidateEvidencePayloadCarrier(leaderView), "
        "body: SubjectOrNone, manifest: SubjectOrNone, "
        "commitment: SubjectOrNone] ELSE {}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateOwnerIdentityFromPayload",
    ): (
        "[target |-> target, context |-> leaderContext, "
        "leader |-> leader, view |-> leaderView, "
        "subject |-> subject, phase |-> rank, "
        "authority |-> AdequateLeaderCorridorAuthorityReceipt( "
        "target, leaderContext, leader, leaderView), owner |-> owner, "
        'kind |-> "Candidate", payload |-> payload]'
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticRankCarrier",
    ): "(1..4) \\X (0..9)",
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticRankOrdering",
    ): (
        "LexPairOrdering( OpToRel(<, Nat), OpToRel(<, Nat), "
        "1..4, 0..9)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankFrontier",
    ): (
        "\\E candidate \\in AsyncCandidateSet: "
        "AdequateLeaderTargetCandidateIdentity( "
        "candidate, rank, target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankOwnerSet",
    ): (
        "{candidate \\in AsyncCandidateSet: "
        "AdequateLeaderTargetCandidateIdentity( candidate, rank, target, "
        "leaderContext, leader, leaderView, subject)}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankOwnerIdentitySet",
    ): (
        "{AdequateLeaderFrozenCandidateOwnerIdentity( "
        "candidate, rank, target, leaderContext, "
        "leader, leaderView, subject): "
        "candidate \\in AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, rank)}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankOwnerCount",
    ): (
        "Cardinality( AdequateLeaderTargetRankOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, rank))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankCarrier",
    ): "AdequateLeaderTargetSemanticRankCarrier \\X Nat",
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankOrdering",
    ): (
        "LexPairOrdering( AdequateLeaderTargetSemanticRankOrdering, "
        "OpToRel(<, Nat), AdequateLeaderTargetSemanticRankCarrier, Nat)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankFrontier",
    ): (
        "/\\ occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier "
        "/\\ occurrenceRank[2] > 0 "
        "/\\ IsFiniteSet( AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, "
        "subject, occurrenceRank[1])) "
        "/\\ AdequateLeaderTargetRankFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, occurrenceRank[1]) "
        "/\\ occurrenceRank[2] = AdequateLeaderTargetRankOwnerCount( "
        "target, leaderContext, leader, leaderView, "
        "subject, occurrenceRank[1])"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetWireIdentity",
    ): (
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leaderView \\in Views "
        "/\\ subject \\in Subjects "
        "/\\ item.kind \\in LeaderWireKinds "
        "/\\ item.envelope.recipient \\in {target, leader} "
        "/\\ DeliveryView(item) = leaderView "
        "/\\ DeliverySubject(item) = subject "
        "/\\ LeaderWireCarriesContext(item, leaderContext) "
        '/\\ IF item.kind = "CertifiedResponse" '
        "THEN /\\ item.envelope.archiveServer \\in AsyncArchiveServerIds "
        "/\\ item.envelope.signatureOwner = item.envelope.archiveServer "
        "ELSE TRUE"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWirePayloadIdentity",
    ): (
        "[source |-> "
        'IF item.kind = "CertifiedResponse" '
        "THEN AsyncUntrustedSource ELSE item.source, "
        "detail |-> "
        'CASE item.kind = "CertifiedResponse" '
        "-> item.envelope.archiveServer "
        '[] item.kind = "Chunk" -> item.envelope.chunk '
        "[] OTHER -> NoAsyncChunk]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWirePayloadCarrier",
    ): (
        "[source: AsyncIngressSources, "
        "detail: AsyncArchiveServerIds \\cup AsyncChunks "
        "\\cup {NoAsyncChunk}]"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWireOwnerIdentityFromCoordinates",
    ): (
        "[target |-> target, context |-> leaderContext, "
        "leader |-> leader, view |-> leaderView, "
        "subject |-> subject, phase |-> wireKind, "
        "authority |-> AdequateLeaderCorridorAuthorityReceipt( "
        "target, leaderContext, leader, leaderView), "
        "owner |-> recipient, "
        'kind |-> "Wire", payload |-> payload]'
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
    ): (
        "{AdequateLeaderFrozenCandidateOwnerIdentityFromPayload( "
        "payload, owner, rank, target, leaderContext, "
        "leader, leaderView, subject): "
        "payload \\in AdequateLeaderFrozenCandidatePayloadCarrier( "
        "target, leaderContext, leader, leaderView, subject), "
        "owner \\in {target, leader}, "
        "rank \\in AdequateLeaderTargetSemanticRankCarrier}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWireOwnerUniverse",
    ): (
        "{AdequateLeaderFrozenWireOwnerIdentityFromCoordinates( "
        "wireKind, recipient, payload, target, "
        "leaderContext, leader, leaderView, subject): "
        "wireKind \\in LeaderWireKinds, "
        "recipient \\in {target, leader}, "
        "payload \\in AdequateLeaderFrozenWirePayloadCarrier}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverse",
    ): (
        "AdequateLeaderFrozenCandidateOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\cup AdequateLeaderFrozenWireOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
    ): (
        "UNION { "
        "AdequateLeaderTargetRankOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, rank): "
        "rank \\in AdequateLeaderTargetSemanticRankCarrier}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetLiveWireOwnerIdentitySet",
    ): (
        "{AdequateLeaderFrozenWireOwnerIdentity( "
        "item, target, leaderContext, leader, leaderView, subject): "
        "item \\in {wire \\in AsyncNetworkItems: "
        "/\\ AdequateLeaderTargetWireIdentity( "
        "wire, target, leaderContext, leader, leaderView, subject) "
        "/\\ LeaderWireLogicalServiceActive(wire) "
        "/\\ \\/ ItemHasPacket(wire) "
        "\\/ LeaderWireIngressOwned(wire) "
        "\\/ LeaderWireCandidateOwned(wire) "
        "\\/ LeaderWireLiveControlServiceOwner(wire)}}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetLiveOwnerIdentitySet",
    ): (
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\cup AdequateLeaderTargetLiveWireOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankReplenishmentAction",
    ): (
        "AdequateLeaderTargetCountIncreasingReplenishmentAction( "
        "target, leaderContext, leader, leaderView, subject, rank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetRankReplenishmentResidual",
    ): (
        "/\\ AdequateLeaderTargetRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, rank) "
        "/\\ ENABLED <<AdequateLeaderTargetRankReplenishmentAction( "
        "target, leaderContext, leader, leaderView, "
        "subject, rank)>>_AsyncAllVars"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetEqualCountOwnerReplacementAction",
    ): (
        "/\\ rank \\in AdequateLeaderTargetSemanticRankCarrier "
        "/\\ subject \\in Subjects "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ IsFiniteSet( AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, rank)) "
        "/\\ IsFiniteSet( AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, rank)') "
        "/\\ AsyncNext "
        "/\\ ~NodeHasDecision(target)' "
        "/\\ AdequateLeaderTargetRankOwnerCount( "
        "target, leaderContext, leader, leaderView, subject, rank)' "
        "= AdequateLeaderTargetRankOwnerCount( "
        "target, leaderContext, leader, leaderView, subject, rank) "
        "/\\ AdequateLeaderTargetRankOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, rank)' "
        "# AdequateLeaderTargetRankOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, rank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCountIncreasingReplenishmentAction",
    ): (
        "/\\ rank \\in AdequateLeaderTargetSemanticRankCarrier "
        "/\\ subject \\in Subjects "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ IsFiniteSet( AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, rank)) "
        "/\\ IsFiniteSet( AdequateLeaderTargetRankOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, rank)') "
        "/\\ AsyncNext "
        "/\\ ~NodeHasDecision(target)' "
        "/\\ AdequateLeaderTargetRankOwnerCount( "
        "target, leaderContext, leader, leaderView, subject, rank)' "
        "> AdequateLeaderTargetRankOwnerCount( "
        "target, leaderContext, leader, leaderView, subject, rank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetStrictOccurrenceDescentGoal",
    ): (
        "\\/ NodeHasDecision(target) "
        "\\/ \\E lowerOccurrenceRank \\in SetLessThan( "
        "occurrenceRank, AdequateLeaderTargetOccurrenceRankOrdering, "
        "AdequateLeaderTargetOccurrenceRankCarrier): "
        "AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, lowerOccurrenceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeAction",
    ): (
        "\\/ AdequateLeaderTargetEqualCountOwnerReplacementAction( "
        "target, leaderContext, leader, leaderView, subject, rank) "
        "\\/ AdequateLeaderTargetCountIncreasingReplenishmentAction( "
        "target, leaderContext, leader, leaderView, subject, rank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudget",
    ): (
        "Cardinality( AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject) \\ known)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOwnerIdentityRetirementAction",
    ): (
        "/\\ AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, occurrenceRank) "
        "/\\ identity \\in AdequateLeaderTargetLiveOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ AsyncNext "
        "/\\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal( "
        "target, leaderContext, leader, leaderView, "
        "subject, occurrenceRank)' "
        "/\\ identity \\notin AdequateLeaderTargetLiveOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeResidual",
    ): (
        "/\\ AdequateLeaderTargetEpisodeKnownOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, known) "
        "/\\ ~AdequateLeaderTargetStrictOccurrenceDescentGoal( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank) "
        "/\\ AdequateLeaderTargetOccurrenceEpisodeActive( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank) "
        "/\\ AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, known) # {}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeAtBudget",
    ): (
        "/\\ AdequateLeaderTargetNonDescentEpisodeFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank, known) "
        "/\\ budget = AdequateLeaderTargetNonDescentEpisodeBudget( "
        "target, leaderContext, leader, leaderView, subject, known)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
    ): (
        "/\\ occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier "
        "/\\ AdequateLeaderFrozenTargetWireIdentity( item, target, "
        "leaderContext, leader, leaderView, subject) /\\ "
        "AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity( item, "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank) "
        "\\notin AdequateLeaderTargetLiveCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) /\\ \\/ "
        "AsyncControlServiceOccurrenceIsCurrentOwner(item) \\/ "
        "AsyncControlServiceIdentityServicedOrAdvanced(item)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty",
    ): (
        "specification => [](\\A item \\in AsyncNetworkItems, target \\in "
        "ValidatorIds, leaderContext \\in ContextRecords, leader \\in "
        "ValidatorIds, leaderView \\in Views, subject \\in Subjects, "
        "occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier: /\\ "
        "gst /\\ AdequateLeaderTargetOffSubjectControlOccurrenceIdentity( "
        "item, target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank) /\\ "
        "AdequateLeaderTargetOffSubjectControlRetirementMemory( item, target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank) => "
        "[]AdequateLeaderTargetOffSubjectControlRetirementClosed( item, "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceOwnerSelected",
    ): (
        "/\\ AdequateLeaderTargetOccurrenceRankFrontier( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank) /\\ "
        "owner \\in AdequateLeaderTargetOccurrenceOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateProducerContinuationRetirementMemory",
    ): (
        "\\/ AsyncCandidateProducerContinuationTerminalForIdentity( "
        "AsyncCandidateServiceIdentity(candidate)) \\/ \\E record \\in "
        "AsyncCandidateProducerContinuations: /\\ record.node = "
        "candidate.node /\\ record.context = candidate.consumerContext "
        "/\\ record.height = candidate.height /\\ record.address.stage = "
        "AsyncCandidateServiceStageForKind(candidate.kind) /\\ "
        "record.view > candidate.view"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
    ): (
        "{AdequateLeaderFrozenCandidateOwnerIdentity( candidate, rank, "
        "target, leaderContext, leader, leaderView, subject): candidate "
        "\\in AsyncCandidateSet, rank \\in "
        "AdequateLeaderTargetSemanticRankCarrier, "
        "AdequateLeaderFrozenTargetCandidateIdentity( candidate, rank, "
        "target, leaderContext, leader, leaderView, subject), "
        "AdequateLeaderCandidateProducerContinuationRetirementMemory("
        "candidate)}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceOwnerRetirementClosed",
    ): (
        "/\\ owner \\in AdequateLeaderFrozenCandidateOwnerUniverse( target, "
        "leaderContext, leader, leaderView, subject) /\\ owner \\notin "
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) /\\ \\/ owner \\in "
        "AdequateLeaderTargetServicedCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) \\/ owner \\in "
        "AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) \\/ owner \\in "
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) \\/ owner \\in "
        "AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank) \\/ "
        "NodeHasDecision(owner.owner)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDurablyRetiredOwnerIdentitySet",
    ): (
        "{identity \\in AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView): \\E subject \\in "
        "Subjects: \\/ identity \\in "
        "AdequateLeaderTargetServicedCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) \\/ identity \\in "
        "AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) \\/ identity "
        "\\in "
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) \\/ \\E "
        "occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier: "
        "identity \\in "
        "AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank) "
        "\\/ /\\ identity \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject) /\\ NodeHasDecision(identity.owner)}"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCarriedOwnerEpisodeAtBudget",
    ): (
        "/\\ AdequateLeaderTargetSubjectSwitchRetiredOwnerSet( target, "
        "leaderContext, leader, leaderView, retired) /\\ budget = "
        "AdequateLeaderTargetSubjectSwitchRemainingBudget( target, "
        "leaderContext, leader, leaderView, retired) /\\ "
        "AdequateLeaderFrozenTargetCorridor( target, leaderContext, leader, "
        "leaderView) /\\ subject \\in Subjects /\\ "
        "AdequateLeaderTargetOccurrenceOwnerSelected( target, leaderContext, "
        "leader, leaderView, subject, occurrenceRank, owner) /\\ owner \\in "
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) /\\ owner \\in "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( target, "
        "leaderContext, leader, leaderView) \\ retired /\\ "
        "~AdequateLeaderTargetStrictOccurrenceDescentGoal( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
    ): (
        "/\\ AdequateLeaderTargetCarriedOwnerEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "owner, retired, budget) "
        "/\\ subject # AsyncProposalSubject(leader)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProductiveOwnerEpisodeAtBudget",
    ): (
        "/\\ AdequateLeaderTargetCarriedOwnerEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "owner, retired, budget) "
        "/\\ AdequateLeaderTargetProtocolSubjectSource( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
    ): "{owner} \\ retired",
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
    ): (
        "\\E discovered, retired2 \\in SUBSET "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( target, "
        "leaderContext, leader, leaderView), budget2 \\in Nat, nextSubject "
        "\\in Subjects, nextOccurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier, nextOwner \\in "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( target, "
        "leaderContext, leader, leaderView): /\\ discovered = "
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet( owner, retired) "
        "/\\ discovered # {} /\\ "
        "retired \\cup discovered \\subseteq retired2 /\\ "
        "owner \\in retired2 /\\ budget2 = "
        "AdequateLeaderTargetSubjectSwitchRemainingBudget( target, "
        "leaderContext, leader, leaderView, retired2) /\\ budget2 < budget "
        "/\\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget( target, "
        "leaderContext, leader, leaderView, nextSubject, nextOccurrenceRank, "
        "nextOwner, retired2, budget2)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, "
        "occurrenceRank \\in AdequateLeaderTargetOccurrenceRankCarrier, "
        "retired \\in SUBSET AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView), owner \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject), budget \\in Nat: "
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "owner, retired, budget) "
        "~> AdequateLeaderTargetSubjectSwitchBudgetDescentGoal( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "owner, retired, budget)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier",
    ): (
        "/\\ currentBudget \\in Nat "
        "/\\ anchorRetired \\subseteq "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView) "
        "/\\ anchorOwner \\in "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView) \\ anchorRetired "
        "/\\ anchorBudget = "
        "AdequateLeaderTargetSubjectSwitchRemainingBudget( "
        "target, leaderContext, leader, leaderView, anchorRetired) "
        "/\\ \\/ /\\ currentBudget = anchorBudget "
        "/\\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, anchorSubject, "
        "anchorOccurrenceRank, anchorOwner, anchorRetired, anchorBudget) "
        "\\/ /\\ currentBudget < anchorBudget "
        "/\\ \\E currentSubject \\in Subjects, "
        "currentOccurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier, currentOwner \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, currentSubject), currentRetired \\in SUBSET "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView): "
        "/\\ anchorRetired \\cup {anchorOwner} \\subseteq currentRetired "
        "/\\ AdequateLeaderTargetSubjectSwitchEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, currentSubject, "
        "currentOccurrenceRank, currentOwner, currentRetired, currentBudget)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal",
    ): (
        "\\/ AdequateLeaderTargetOffSubjectRetirementAndReentryGoal( "
        "target, leaderContext, leader, leaderView, anchorSubject, "
        "anchorOccurrenceRank, anchorOwner, anchorRetired, anchorBudget) "
        "\\/ \\E lowerBudget \\in "
        "SetLessThan(currentBudget, OpToRel(<, Nat), Nat): "
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier( "
        "target, leaderContext, leader, leaderView, anchorSubject, "
        "anchorOccurrenceRank, anchorOwner, anchorRetired, anchorBudget, "
        "lowerBudget)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, anchorSubject \\in Subjects, "
        "anchorOccurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier, anchorOwner \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, anchorSubject), anchorRetired \\in SUBSET "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( target, "
        "leaderContext, leader, leaderView), anchorBudget \\in Nat, "
        "currentBudget \\in Nat: "
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier( "
        "target, leaderContext, leader, leaderView, anchorSubject, "
        "anchorOccurrenceRank, anchorOwner, anchorRetired, anchorBudget, "
        "currentBudget) "
        "~> AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal( "
        "target, leaderContext, leader, leaderView, anchorSubject, "
        "anchorOccurrenceRank, anchorOwner, anchorRetired, anchorBudget, "
        "currentBudget)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, "
        "sourceOccurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier, "
        "known \\in SUBSET AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject), "
        "budget \\in Nat, owner \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject): "
        "/\\ AdequateLeaderTargetNonDescentEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank, known, budget) "
        "/\\ AdequateLeaderTargetSameOrHigherOccurrenceFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank) /\\ "
        "AdequateLeaderTargetOccurrenceOwnerSelected( target, leaderContext, "
        "leader, leaderView, subject, sourceOccurrenceRank, owner) ~> "
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal( target, "
        "leaderContext, leader, leaderView, subject, sourceOccurrenceRank, "
        "known, owner)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudgetFrontier",
    ): (
        "/\\ known \\in SUBSET AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ AdequateLeaderTargetNonDescentEpisodeAtBudget( "
        "target, leaderContext, leader, leaderView, subject, "
        "sourceOccurrenceRank, known, budget) "
        "/\\ AdequateLeaderTargetOccurrenceOwnerCarried( "
        "target, leaderContext, leader, leaderView, subject, "
        "sourceOccurrenceRank, known, owner)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, "
        "sourceOccurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier, "
        "owner \\in AdequateLeaderFrozenCandidateOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject), "
        "known \\in SUBSET AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject), "
        "budget \\in Nat: /\\ "
        "AdequateLeaderTargetProtocolSubjectSource( target, leaderContext, "
        "leader, leaderView, subject) /\\ "
        "AdequateLeaderTargetNonDescentEpisodeBudgetFrontier( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank, owner, known, budget) "
        "~> AdequateLeaderTargetOccurrenceRankServiceExitGoal( "
        "target, leaderContext, leader, leaderView, "
        "subject, sourceOccurrenceRank, owner)"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderAuthorityBoundFixedDeadlineReceipt",
    ): (
        "LET authority == AdequateLeaderCorridorAuthorityReceipt( "
        "target, leaderContext, leader, leaderView) IN "
        "/\\ receipt \\in AsyncFixedCorridorDeadlineReceipts "
        "/\\ authority.target = target "
        "/\\ authority.context = leaderContext "
        "/\\ authority.leader = leader "
        "/\\ authority.view = leaderView "
        "/\\ receipt.target = authority.leader "
        "/\\ receipt.context = authority.context "
        "/\\ receipt.view = authority.view "
        "/\\ receipt.deadline = "
        "receipt.armedAt + AsyncFixedCorridorServiceBudget + 1 "
        "/\\ receipt.armedAt <= asyncNow "
        "/\\ asyncNow < receipt.deadline"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderTargetAuthorityBoundActiveReceiptSource",
    ): (
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ ~NodeHasDecision(target) "
        "/\\ \\E receipt \\in AsyncActiveFixedCorridorDeadlineReceipts: "
        "AdequateLeaderAuthorityBoundFixedDeadlineReceipt( "
        "target, leaderContext, leader, leaderView, receipt)"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderAuthorityBoundReceiptAcquisitionProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views: AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "~> (NodeHasDecision(target) "
        "\\/ AdequateLeaderTargetAuthorityBoundActiveReceiptSource( "
        "target, leaderContext, leader, leaderView))"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderAuthorityBoundActiveReceiptServiceProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views: "
        "AdequateLeaderTargetAuthorityBoundActiveReceiptSource( "
        "target, leaderContext, leader, leaderView) "
        "~> NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
    ): (
        "/\\ AdequateLeaderAuthorityBoundReceiptAcquisitionProperty("
        "specification) "
        "/\\ AdequateLeaderAuthorityBoundActiveReceiptServiceProperty("
        "specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateIdentityTombstoneProperty",
    ): (
        "/\\ AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty( "
        "specification) /\\ (specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: /\\ [](\\A identity \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject): "
        "AdequateLeaderTargetCandidateServicedRetirementAction( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, identity) "
        "=> /\\ AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity)' /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity)') /\\ [](\\A identity "
        "\\in AdequateLeaderFrozenCandidateOwnerUniverse( target, "
        "leaderContext, leader, leaderView, subject): /\\ gst /\\ "
        "AdequateLeaderServicedCandidateMemory( target, leaderContext, leader, "
        "leaderView, subject, identity) /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity) => "
        "[](AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity) /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity))))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: [](\\A identity \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject): "
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "identity) => AdequateLeaderServicedCandidateMemory( target, "
        "leaderContext, leader, leaderView, subject, identity)')"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateTerminalTombstoneProperty",
    ): (
        "/\\ AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty( "
        "specification) /\\ (specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: /\\ [](\\A identity \\in "
        "AdequateLeaderFrozenCandidateOwnerUniverse( target, leaderContext, "
        "leader, leaderView, subject): "
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, identity) "
        "=> /\\ AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity)' /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity)') /\\ [](\\A identity "
        "\\in AdequateLeaderFrozenCandidateOwnerUniverse( target, "
        "leaderContext, leader, leaderView, subject): /\\ gst /\\ "
        "AdequateLeaderServicedCandidateMemory( target, leaderContext, leader, "
        "leaderView, subject, identity) /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity) => "
        "[](AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity) /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity))))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetComposedRankDescentProperty",
    ): (
        "/\\ AdequateLeaderProtectedPeriodicEpisodeClosureProperty("
        "specification) "
        "/\\ AdequateLeaderTargetOccurrenceRankServiceProperty(specification) "
        "/\\ AdequateLeaderTargetProducerTransportOccurrenceClosureProperty( "
        "specification) "
        "/\\ AdequateLeaderTargetNonDescentKnownAdvanceProperty(specification)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCommitQcRebroadcastResidual",
    ): (
        "/\\ target # leader "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ subject \\in Subjects "
        "/\\ \\E candidate \\in AsyncCandidateSet: "
        "/\\ ExactLeaderCurrentRankWitness( "
        "candidate, DecisionSemanticRank(2), "
        "leaderContext, leader, leaderView, subject) "
        "/\\ candidate.kind = \"PersistDecision\" "
        "/\\ \\E request \\in PersistDecisionRequests(candidate): "
        "request.rebroadcast"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerResidual",
    ): (
        "/\\ AdequateLeaderTargetProtocolSubjectSource( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ ~(\\E rank \\in AdequateLeaderTargetSemanticRankCarrier: "
        "AdequateLeaderTargetRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, rank)) "
        "/\\ ~AdequateLeaderTargetCommitQcRebroadcastResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ ~AdequateLeaderTargetDueTransportResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ ~AdequateLeaderTargetRunnerAdmissionResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ ~AdequateLeaderTargetCertifiedResponseCapacityResidual( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerTransportResidual",
    ): (
        "\\/ AdequateLeaderTargetCommitQcRebroadcastResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\/ AdequateLeaderTargetDueTransportResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\/ AdequateLeaderTargetRunnerAdmissionResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\/ AdequateLeaderTargetCertifiedResponseCapacityResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\/ AdequateLeaderTargetProducerResidual( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOpenFrontier",
    ): (
        "\\/ AdequateLeaderTargetProducerTransportResidual( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\/ \\E occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: "
        "AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDecisionSource",
    ): (
        "/\\ gst /\\ AdequateResponsiveHonestLeaderViewReached "
        "/\\ target \\in AsyncCurrentResponsiveVoters "
        "/\\ ~NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDecisionConvergenceProperty",
    ): (
        "specification => \\A target \\in ValidatorIds: "
        "AdequateLeaderTargetDecisionSource(target) "
        "~> NodeHasDecision(target)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderDecisionPrefixAt",
    ): (
        "\\A target \\in AsyncVotersAt(initialContext) \\cap (0..limit): "
        "NodeHasDecision(target)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestPacketEmissionResidual",
    ): (
        "/\\ ExactDecisionActiveRequestOwner(node, qc) "
        "/\\ ~ExactDecisionRequestPacketEmissionGoal(node, qc)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionOffSchedulerResidualConvergenceProperty",
    ): (
        "/\\ ExactDecisionRequestClockOwnerConvergenceProperty(specification) "
        "/\\ ExactDecisionRequestRuntimePrefixConvergenceProperty(specification) "
        "/\\ ExactDecisionRequestHeadGateOwnerConvergenceProperty(specification) "
        "/\\ ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty( "
        "specification) "
        "/\\ ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty( "
        "specification)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestClockOwnerConvergenceProperty",
    ): (
        "specification => \\A node, qc: "
        "ExactDecisionRequestPacketEmissionResidual(node, qc) "
        "~> (ExactDecisionRequestPacketEmissionGoal(node, qc) "
        "\\/ ExactDecisionRequestRetransmitArmedResidual( node, qc))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestRuntimePrefixConvergenceProperty",
    ): (
        "specification => \\A node, qc: "
        "/\\ ExactDecisionRequestRetransmitArmedResidual(node, qc) "
        "~> (ExactDecisionRequestPacketEmissionGoal(node, qc) "
        "\\/ ExactDecisionRequestSendingRetransmitReady( node, qc)) "
        "/\\ ExactDecisionRequestSendingRetransmitReady(node, qc) "
        "~> ExactDecisionRequestPacketEmissionGoal(node, qc)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestHeadGateOwnerConvergenceProperty",
    ): (
        "specification => \\A node, qc, archive, request, packet: "
        "ExactDecisionRequestHeadGateOwnerResidual( "
        "node, qc, archive, request, packet) "
        "~> (ExactDecisionRequestIngressGoal( "
        "node, qc, archive, request) "
        "\\/ ExactDecisionRequestPacketAdmissionReady( "
        "node, qc, archive, request, packet))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleGoal",
    ): (
        "ExactDecisionRequestIngressGoal( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleResidual",
    ): (
        "ExactDecisionRequestIngressLaneResidual( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleStage",
    ): (
        "IF ExactDecisionRequestLifecycleGoal( "
        "node, qc, archive, request) THEN 0 "
        "ELSE IF ExactDecisionServeTombstoneOwned( "
        "node, qc, archive, request) THEN 1 "
        "ELSE IF ExactDecisionServeAdmissionOwned( "
        "archive, request) THEN 2 ELSE 3"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
    ): (
        "LET identity == "
        "ExactDecisionServeLifecycleIdentity(archive, request) "
        'IN ({"Io"} \\X AsyncServeFrozenPredecessorSet( '
        "archive, identity)) "
        '\\cup ({"Ingress"} \\X '
        "AsyncServeIngressAdmissionPredecessorDebtSlots( "
        "archive, identity)) "
        "\\cup AsyncServePreexistingIngressOwnerPredecessorDebtSet( "
        "archive, identity) "
        "\\cup AsyncServePreexistingIngressBarrierPredecessorDebtSet( "
        "archive, identity)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
    ): (
        "AsyncServePreexistingIngressBarrierIdentities( "
        "archive, ExactDecisionServeLifecycleIdentity(archive, request))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
    ): (
        "CHOOSE identity \\in "
        "ExactDecisionRequestFrozenServeBarrierIdentities(archive, request): "
        "TRUE"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
    ): (
        "LET normalItem == SelectedIngressItemAt( "
        "archive, FirstDrainableIngressIndex(archive)) "
        "historicalItem == HistoricalSelectedIngressItemAt( "
        "archive, FirstHistoricalDrainableIngressIndex(archive)) "
        "IN \\E barrierIdentity \\in "
        "ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request): "
        "\\/ /\\ PostGstRunNode(archive) "
        "/\\ DrainFairIngressSelected(archive) "
        "/\\ normalItem.kind \\in AsyncReplyRequestKinds "
        "/\\ AsyncServeLogicalRequestIdentity( archive, normalItem) "
        "= barrierIdentity "
        "\\/ /\\ PostGstRunHistoricalServer(archive) "
        "/\\ DrainHistoricalIngressSelected(archive) "
        "/\\ historicalItem.kind \\in AsyncReplyRequestKinds "
        "/\\ AsyncServeLogicalRequestIdentity( archive, historicalItem) "
        "= barrierIdentity"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet",
    ): (
        "LET laneOwners == "
        "IF ExactDecisionRequestIngressLaneResidual( "
        "node, qc, archive, request) "
        'THEN ({"Lane"} \\X '
        "(1..ExactDecisionRequestIngressLanePosition( "
        "archive, request))) "
        '\\cup ({"Source"} \\X '
        "(1..ExactDecisionRequestIngressSourcePosition( "
        "archive, request))) "
        '\\cup ({"Runner"} \\X '
        "(1..ExactDecisionRequestIngressReachRank(archive))) "
        "ELSE {} "
        "IN ExactDecisionRequestLifecycleFrozenPredecessorSet( "
        "archive, request) "
        '\\cup ({"Mode"} \\X '
        "(1..ExactDecisionRequestIngressModeRank(archive))) "
        '\\cup ({"Capacity"} \\X '
        "(1..ExactDecisionRequestIngressTargetServeCapacityDebt( "
        "archive, request))) "
        '\\cup ({"Selector"} \\X '
        "(1..ExactDecisionRequestIngressPriorityDebt(archive))) "
        "\\cup laneOwners"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressProducerEpisodeBudget",
    ): (
        "Cardinality( "
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet( "
        "node, qc, archive, request))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIngressRank",
    ): (
        "<<ExactDecisionRequestLifecycleStage( "
        "node, qc, archive, request), "
        "<<ExactDecisionRequestLifecycleFrozenPredecessorDebt( "
        "archive, request), "
        "ExactDecisionRequestLifecycleNestedIngressRank( "
        "node, qc, archive, request)>>>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleStepClassification",
    ): (
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ AsyncNext "
        "=> \\/ ExactDecisionRequestLifecycleGoal( "
        "node, qc, archive, request)' "
        "\\/ <<ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request)', "
        "ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request)>> "
        "\\in ExactDecisionRequestLifecycleIngressRankOrdering "
        "\\/ ExactDecisionRequestIngressFiniteProducerEpisodeAction( "
        "node, qc, archive, request) "
        "\\/ ExactDecisionRequestLifecycleNoninterferenceAction( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteFairOwnerKinds",
    ): (
        '{"NormalRunner", "HistoricalServer", "IoWorker"}'
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
    ): (
        "LET identity == "
        "ExactDecisionServeLifecycleIdentity(archive, request) "
        "barriers == ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request) "
        "IN \\/ /\\ AsyncServeLiveReservationOwned(archive, identity) "
        "/\\ ~AsyncServeJobQueued(archive, identity) "
        "/\\ ~CanResumeExactServeCapacity(archive, identity) "
        "\\/ /\\ barriers # {} "
        "/\\ ~CanResumeExactServeCapacity( "
        "archive, ExactDecisionRequestFrozenServeBarrierIdentity( "
        "archive, request))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteFairOwner",
    ): (
        "IF ExactDecisionRequestLifecycleIoOwnerRequired(archive, request) "
        'THEN "IoWorker" '
        "ELSE IF NodeHasApplication(archive) "
        'THEN "HistoricalServer" ELSE "NormalRunner"'
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteFairAction",
    ): (
        'CASE ownerKind = "NormalRunner" -> PostGstRunNode(archive) '
        '[] ownerKind = "HistoricalServer" -> '
        "PostGstRunHistoricalServer(archive) "
        '[] ownerKind = "IoWorker" -> PostGstServiceIoWorker(archive) '
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleSelectedConcreteFairAction",
    ): (
        "ExactDecisionRequestLifecycleConcreteFairAction( "
        "archive, ExactDecisionRequestLifecycleConcreteFairOwner("
        "archive, request))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankCellOutcome",
    ): (
        "\\/ ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "\\/ \\E lowerBudget \\in SetLessThan( "
        "budget, OpToRel(<, Nat), Nat): "
        "ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, lowerBudget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankCellClosureProperty",
    ): (
        "specification => \\A node, qc, archive, request, "
        "rank \\in ExactDecisionRequestLifecycleIngressRankCarrier: "
        "ExactDecisionRequestLifecycleAtRank( "
        "node, qc, archive, request, rank) "
        "~> (ExactDecisionRequestLifecycleGoal( "
        "node, qc, archive, request) "
        "\\/ <<ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request), rank>> "
        "\\in ExactDecisionRequestLifecycleIngressRankOrdering)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionNormalRequestIngressContinuationPrefixBlocked",
    ): (
        "/\\ ExactDecisionRequestIngressLaneResidual( "
        "node, qc, archive, request) "
        "/\\ ~NodeHasApplication(archive) "
        "/\\ AsyncCandidateProducerContinuationRunnerResolutionRequired(archive)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixCleared",
    ): (
        "\\/ NodeHasApplication(archive) "
        "\\/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(archive)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixGoal",
    ): (
        "\\/ ExactDecisionRequestLifecycleGoal(node, qc, archive, request) "
        "\\/ ExactDecisionRequestIngressContinuationPrefixCleared(archive)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixAtBudget",
    ): (
        "/\\ ExactDecisionNormalRequestIngressContinuationPrefixBlocked( "
        "node, qc, archive, request) "
        "/\\ record = "
        "AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord( "
        "archive) "
        "/\\ status = record.status "
        '/\\ status \\in {"Reserved", "Materialized"} '
        "/\\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget( "
        "archive, record.identity, record.ordinal, record.address.stage, "
        "status, budget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixClosureProperty",
    ): (
        "specification => \\A node, qc, archive, request: "
        "ExactDecisionNormalRequestIngressContinuationPrefixBlocked( "
        "node, qc, archive, request) "
        "~> ExactDecisionRequestIngressContinuationPrefixGoal( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
    ): (
        "specification => \\A node, qc, archive, request, "
        "rank \\in ExactDecisionRequestLifecycleIngressRankCarrier, "
        "budget \\in Nat: "
        "ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "~> (ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "\\/ \\E lowerBudget \\in SetLessThan( "
        "budget, OpToRel(<, Nat), Nat): "
        "ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, lowerBudget))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
    ): (
        "/\\ specification => []("
        "\\A node, qc, archive, request, "
        "rank \\in ExactDecisionRequestLifecycleIngressRankCarrier, "
        "budget \\in Nat: "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixCleared( archive) "
        "=> /\\ ExactDecisionRequestLifecycleConcreteFairOwner( "
        "archive, request) "
        "\\in ExactDecisionRequestLifecycleConcreteFairOwnerKinds "
        "/\\ ENABLED "
        "<<ExactDecisionRequestLifecycleSelectedConcreteFairAction( "
        "archive, request)>>_AsyncAllVars) "
        "/\\ specification => []("
        "\\A node, qc, archive, request, "
        "rank \\in ExactDecisionRequestLifecycleIngressRankCarrier, "
        "budget \\in Nat: "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> \\/ ExactDecisionRequestLifecycleRankCellOutcome( "
        "node, qc, archive, request, rank, budget)' "
        "\\/ /\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget)' "
        "/\\ ExactDecisionRequestLifecycleConcreteFairOwner( "
        "archive, request)' "
        "= ExactDecisionRequestLifecycleConcreteFairOwner( "
        "archive, request)) "
        "/\\ specification => []("
        "\\A node, qc, archive, request, "
        "rank \\in ExactDecisionRequestLifecycleIngressRankCarrier, "
        "budget \\in Nat: "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixCleared( archive) "
        "/\\ <<ExactDecisionRequestLifecycleSelectedConcreteFairAction( "
        "archive, request)>>_AsyncAllVars "
        "=> ExactDecisionRequestLifecycleRankCellOutcome( "
        "node, qc, archive, request, rank, budget)')"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankDescentProperty",
    ): (
        "/\\ specification => []("
        "\\A node, qc, archive, request: "
        "ExactDecisionRequestLifecycleStepClassification( "
        "node, qc, archive, request)) "
        "/\\ ExactDecisionRequestLifecycleConcreteActionOriginProperty("
        "specification)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
    ): (
        "/\\ ExactDecisionRequestLifecycleRankDescentProperty(specification) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixClosureProperty( "
        "specification)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressRankReplenishmentResidual",
    ): (
        "\\/ ExactDecisionRequestIngressCausalReplenishmentResidual( "
        "node, qc, archive, request) \\/ "
        "ExactDecisionRequestIngressServeReplenishmentResidual( "
        "node, qc, archive, request) \\/ "
        "ExactDecisionRequestIngressPriorityReplenishmentResidual( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressLaneRunnerConvergenceProperty",
    ): (
        "specification => \\A node, qc, archive, request: "
        "ExactDecisionRequestIngressLaneResidual( "
        "node, qc, archive, request) "
        "~> ExactDecisionRequestIngressGoal( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty",
    ): (
        "specification => \\A node, qc, archive, request, response, packet: "
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual( "
        "node, qc, archive, request, response, packet) "
        "~> (ExactDecisionResponseAdmissionGoal(node, qc) "
        "\\/ ExactDecisionResponsePacketAdmissionReady( "
        "node, qc, archive, request, response, packet))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralPacketDependencyRank",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN <<OlderDueNonOverdueShadowDebt(packet), "
        "<<FreshIngressCapacityOwnerDebt(packet.item), "
        "<<TimeoutVoteByteOwnerDebt(packet.item), "
        "<<TransportCompletionOwnerDebt(packet.item), "
        "<<BoundedTransportServiceRank( "
        "packet.item.envelope.recipient, packet.item.source), "
        "<<ResetAwareIngressReachRank(recipient), "
        "<<ReadyRunAuxRank(recipient), "
        "<<Stage4CapacityRank(recipient), "
        "<<ExactDecisionTargetNeutralCandidateOccurrenceRank( packet), "
        "ExactDecisionTargetNeutralServeOccurrenceRank( packet)"
        ">>>>>>>>>>>>>>>>>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralConcreteFixedClockRank",
    ): (
        "HistoricalDiscoveryFixedClockRank( clockValue, "
        "ExactDecisionTargetNeutralConcreteBlockerStage(clockValue), "
        "ExactDecisionTargetNeutralConcreteDependencyRank(clockValue))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockCarrier",
    ): "HistoricalDiscoveryFixedClockBlockerCarrier",
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockOrdering",
    ): "HistoricalDiscoveryFixedClockBlockerOrdering",
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedPredecessorSet",
    ): (
        '({"Packet"} \\X HistoricalDiscoveryDuePacketsAt(clockValue)) '
        '\\cup ({"Candidate"} \\X '
        "ExactDecisionTargetNeutralFrozenLiveCandidateIdentitySet) "
        '\\cup ({"Serve"} \\X '
        "ExactDecisionTargetNeutralLiveServeIdentitySet)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockSnapshot",
    ): (
        "[clock |-> clockValue, "
        "packets |-> HistoricalDiscoveryDuePacketsAt(clockValue), "
        "predecessors |-> "
        "ExactDecisionTargetNeutralFixedPredecessorSet(clockValue), "
        "candidateIdentities |-> "
        "ExactDecisionTargetNeutralFrozenLiveCandidateIdentitySet, "
        "serveIdentities |-> "
        "ExactDecisionTargetNeutralLiveServeIdentitySet, "
        "candidateStart |-> "
        "[node \\in Responsive |-> "
        "AsyncNextCandidateServiceOrdinal(node)], "
        "serveStart |-> "
        "[node \\in Responsive |-> "
        "asyncNextServeAdmissionOrdinal[node]], "
        "candidateCeiling |-> "
        "[node \\in Responsive |-> "
        "AsyncNextCandidateServiceOrdinal(node) "
        "+ AsyncCandidateProducerEpisodeBudget], "
        "serveCeiling |-> "
        "[node \\in Responsive |-> "
        "asyncNextServeAdmissionOrdinal[node] "
        "+ AsyncServeLifecycleFamilyBudget]]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralProducerEpisodeTokens",
    ): (
        "ExactDecisionTargetNeutralCandidateOrdinalTokens(snapshot) "
        "\\cup ExactDecisionTargetNeutralServeOrdinalTokens(snapshot)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralProducerEpisodeBudget",
    ): (
        "Cardinality( "
        "ExactDecisionTargetNeutralProducerEpisodeTokens(snapshot))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSnapshotActive",
    ): (
        "/\\ snapshot.clock = clockValue "
        "/\\ IsFiniteSet(snapshot.packets) "
        "/\\ IsFiniteSet(snapshot.predecessors) "
        "/\\ IsFiniteSet(snapshot.candidateIdentities) "
        "/\\ IsFiniteSet(snapshot.serveIdentities) "
        "/\\ snapshot.candidateIdentities "
        "\\subseteq "
        "ExactDecisionTargetNeutralFrozenCandidateOwnerIdentitySet "
        "/\\ snapshot.serveIdentities "
        "\\subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet "
        "/\\ snapshot.predecessors = "
        '({"Packet"} \\X snapshot.packets) '
        '\\cup ({"Candidate"} \\X snapshot.candidateIdentities) '
        '\\cup ({"Serve"} \\X snapshot.serveIdentities) '
        "/\\ ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered("
        "snapshot) "
        "/\\ ExactDecisionTargetNeutralFrozenServeLifecycleCovered("
        "snapshot) "
        "/\\ snapshot.candidateCeiling \\in [Responsive -> Nat] "
        "/\\ snapshot.serveCeiling \\in [Responsive -> Nat] "
        "/\\ snapshot.candidateStart \\in [Responsive -> Nat] "
        "/\\ snapshot.serveStart \\in [Responsive -> Nat] "
        "/\\ HistoricalDiscoveryDuePacketsAt(clockValue) "
        "\\subseteq snapshot.packets "
        "/\\ \\A node \\in Responsive: "
        "/\\ snapshot.candidateCeiling[node] = "
        "snapshot.candidateStart[node] "
        "+ AsyncCandidateProducerEpisodeBudget "
        "/\\ snapshot.serveCeiling[node] = "
        "snapshot.serveStart[node] "
        "+ AsyncServeLifecycleFamilyBudget "
        "/\\ snapshot.candidateStart[node] "
        "<= AsyncNextCandidateServiceOrdinal(node) "
        "/\\ AsyncNextCandidateServiceOrdinal(node) "
        "<= snapshot.candidateCeiling[node] "
        "/\\ snapshot.serveStart[node] "
        "<= asyncNextServeAdmissionOrdinal[node] "
        "/\\ asyncNextServeAdmissionOrdinal[node] "
        "<= snapshot.serveCeiling[node]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralResidual",
    ): (
        'CASE mode = "RequestClock" -> '
        "ExactDecisionRequestPacketEmissionResidual(node, qc) "
        '[] mode = "RequestHead" -> '
        "ExactDecisionRequestHeadGateOwnerResidual( "
        "node, qc, archive, request, packet) "
        '[] mode = "ResponseHead" -> '
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerResidual( "
        "node, qc, archive, request, response, packet) "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralGoal",
    ): (
        'CASE mode = "RequestClock" -> '
        "\\/ ExactDecisionRequestPacketEmissionGoal(node, qc) "
        "\\/ ExactDecisionRequestRetransmitArmedResidual(node, qc) "
        '[] mode = "RequestHead" -> '
        "\\/ ExactDecisionRequestIngressGoal( "
        "node, qc, archive, request) "
        "\\/ ExactDecisionRequestPacketAdmissionReady( "
        "node, qc, archive, request, packet) "
        '[] mode = "ResponseHead" -> '
        "\\/ ExactDecisionResponseAdmissionGoal(node, qc) "
        "\\/ ExactDecisionResponsePacketAdmissionReady( "
        "node, qc, archive, request, response, packet) "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDeadline",
    ): (
        'IF mode = "RequestClock" '
        "THEN asyncRetransmitDeadlines[node] ELSE packet.deadline"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairOwnerSet",
    ): (
        "{ExactDecisionTargetNeutralFairOwner( "
        '"Tick", 0, AsyncUntrustedSource)} '
        "\\cup {ExactDecisionTargetNeutralFairOwner( "
        '"RunNode", node, AsyncUntrustedSource): '
        "node \\in AsyncVotersAt(initialContext)} "
        "\\cup {ExactDecisionTargetNeutralFairOwner( "
        "ownerKind, node, AsyncUntrustedSource): "
        "ownerKind "
        '\\in {"RunHistoricalRecovery", "RunHistoricalServer", '
        '"ServiceIo", "ServiceHistoricalIo"}, '
        "node \\in Responsive} "
        "\\cup {ExactDecisionTargetNeutralFairOwner( "
        '"Admit", recipient, source): '
        "recipient \\in Responsive, "
        "source \\in AsyncIngressSources} "
        "\\cup {ExactDecisionTargetNeutralFairOwner( "
        '"AdmitHistorical", recipient, source): '
        "recipient \\in ValidatorIds, "
        "source \\in AsyncIngressSources}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairAction",
    ): (
        'CASE owner.ownerKind = "Tick" -> AsyncTick '
        '[] owner.ownerKind = "RunNode" -> '
        "PostGstRunNode(owner.node) "
        '[] owner.ownerKind = "RunHistoricalRecovery" -> '
        "PostGstRunHistoricalRecoveryNode(owner.node) "
        '[] owner.ownerKind = "RunHistoricalServer" -> '
        "PostGstRunHistoricalServer(owner.node) "
        '[] owner.ownerKind = "ServiceIo" -> '
        "PostGstServiceIoWorker(owner.node) "
        '[] owner.ownerKind = "ServiceHistoricalIo" -> '
        "PostGstServiceHistoricalRecoveryIoWorker(owner.node) "
        '[] owner.ownerKind = "Admit" -> '
        "PostGstAdmitHiddenPacket(owner.node, owner.source) "
        "[] OTHER -> PostGstAdmitHistoricalRecoveryPacket( "
        "owner.node, owner.source)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedFairOwner",
    ): (
        "CHOOSE owner \\in "
        "ExactDecisionTargetNeutralFairOwnerSet(initialContext): "
        "ExactDecisionTargetNeutralOwnerReadyForRankCell( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget, owner)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralProducerEpisodeOwnedBy",
    ): (
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ owner = "
        "ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralClockBudgetFrontier",
    ): (
        "/\\ mode \\in ExactDecisionTargetNeutralModeSet "
        "/\\ gst "
        "/\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ ~ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ asyncNow \\in Nat "
        "/\\ budget \\in Nat "
        "/\\ asyncNow + budget = "
        "ExactDecisionTargetNeutralDeadline(mode, node, packet)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralClockBudgetGoal",
    ): (
        "\\/ ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "\\/ \\E lowerBudget \\in "
        "SetLessThan(budget, OpToRel(<, Nat), Nat): "
        "ExactDecisionTargetNeutralClockBudgetFrontier( "
        "mode, node, qc, archive, request, response, "
        "packet, lowerBudget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateOwners",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN {candidate \\in ActiveScheduledCandidates: "
        "/\\ candidate.node = recipient "
        "/\\ candidate.node \\in AsyncTimedServiceNodes "
        "/\\ ProtectedCandidateOwned(candidate)}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOwners",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN {job \\in ActiveIoJobs: "
        "/\\ job \\in SequenceSet(asyncIoQueues[recipient]) "
        '/\\ job.class = "Serve"}'
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateRanks",
    ): (
        "{CandidateServiceRank(candidate): "
        "candidate \\in ExactDecisionTargetNeutralCandidateOwners(packet)}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeRanks",
    ): (
        "LET recipient == packet.item.envelope.recipient "
        "IN {ServeJobRank(recipient, job): "
        "job \\in ExactDecisionTargetNeutralServeOwners(packet)}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateDebtRank",
    ): (
        "LET ranks == ExactDecisionTargetNeutralCandidateRanks(packet) "
        "IN IF ranks = {} "
        "THEN HistoricalDiscoveryCandidateDebtBottom "
        "ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeDebtRank",
    ): (
        "LET ranks == ExactDecisionTargetNeutralServeRanks(packet) "
        "IN IF ranks = {} "
        "THEN HistoricalDiscoveryServeDebtBottom "
        "ELSE HistoricalDiscoveryOwnedRankMinimum(ranks)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateOccurrenceRank",
    ): (
        "<<Cardinality( "
        "ExactDecisionTargetNeutralCandidateOwners(packet)), "
        "ExactDecisionTargetNeutralCandidateDebtRank(packet)>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOccurrenceRank",
    ): (
        "<<Cardinality( "
        "ExactDecisionTargetNeutralServeOwners(packet)), "
        "ExactDecisionTargetNeutralServeDebtRank(packet)>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedOverduePacket",
    ): "CHOOSE packet \\in OverdueResponsivePackets: TRUE",
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedPacketDependencyRank",
    ): (
        "ExactDecisionTargetNeutralPacketDependencyRank( "
        "ExactDecisionTargetNeutralSelectedOverduePacket)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralConcreteBlockerStage",
    ): (
        "IF OverdueResponsivePackets # {} THEN 1 "
        "ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {} "
        "THEN 3 "
        "ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {} "
        "THEN 2 ELSE 0"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralConcreteDependencyRank",
    ): (
        "IF OverdueResponsivePackets # {} "
        "THEN ExactDecisionTargetNeutralSelectedPacketDependencyRank "
        "ELSE IF HistoricalDiscoveryNodeBlockersAt(clockValue) # {} "
        "THEN HistoricalDiscoveryIngressCounterRank( "
        "HistoricalDiscoveryNodeBlockerDebt(clockValue)) "
        "ELSE IF HistoricalDiscoveryActiveIoBlockersAt(clockValue) # {} "
        "THEN HistoricalDiscoveryIngressCounterRank( "
        "HistoricalDiscoveryActiveIoBlockerDebt(clockValue)) "
        "ELSE HistoricalDiscoveryIngressCounterRank(0)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDependencyProducerPrefix",
    ): (
        "<<dependencyRank[1], "
        "dependencyRank[2][1], "
        "dependencyRank[2][2][1], "
        "dependencyRank[2][2][2][1], "
        "dependencyRank[2][2][2][2][1], "
        "dependencyRank[2][2][2][2][2][1], "
        "dependencyRank[2][2][2][2][2][2][1], "
        "dependencyRank[2][2][2][2][2][2][2][1]>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralProducerPrefix",
    ): (
        "<<rank[1], rank[2][1], rank[2][2][1], "
        "rank[2][2][2][1], "
        "ExactDecisionTargetNeutralDependencyProducerPrefix( "
        "rank[2][2][2][2])>>"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateOwnerIdentity",
    ): (
        '[ownerKind |-> "Candidate", '
        "identity |-> AsyncCandidateAdmissionIdentity(candidate)]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateOwnerIdentitySet",
    ): (
        '{[ownerKind |-> "Candidate", identity |-> identity]: '
        "identity \\in AsyncCandidateAdmissionIdentitySet}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenCandidateOwnerIdentitySet",
    ): (
        "{owner \\in ExactDecisionTargetNeutralCandidateOwnerIdentitySet: "
        'owner.identity.service.phase = "DeliverChunk"}'
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenLiveCandidateIdentitySet",
    ): (
        "{owner \\in ExactDecisionTargetNeutralLiveCandidateIdentitySet: "
        "owner \\in "
        "ExactDecisionTargetNeutralFrozenCandidateOwnerIdentitySet}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateIdentityCoalesced",
    ): (
        '/\\ owner.ownerKind = "Candidate" '
        "/\\ \\/ AsyncCandidateTransientServiceIdentityMarked( "
        "owner.identity.service) "
        "\\/ AsyncCandidateTerminalIdentityTombstoned( "
        "owner.identity.service)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateIdentityObsolete",
    ): (
        '/\\ owner.ownerKind = "Candidate" '
        "/\\ AsyncCandidateAdmissionIdentityObsolete(owner.identity)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOwnerIdentitySet",
    ): (
        '{[ownerKind |-> "Serve", identity |-> identity]: '
        "identity \\in AsyncServeLogicalRequestIdentities}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeIdentityRetired",
    ): (
        '/\\ owner.ownerKind = "Serve" '
        "/\\ AsyncServeLogicalIdentityRetiredOrSuperseded( "
        "owner.identity.owner, owner.identity)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered",
    ): (
        "\\A owner \\in snapshot.candidateIdentities: "
        "\\/ owner \\in "
        "ExactDecisionTargetNeutralFrozenLiveCandidateIdentitySet "
        "\\/ ExactDecisionTargetNeutralCandidateIdentityCoalesced(owner) "
        "\\/ ExactDecisionTargetNeutralCandidateIdentityObsolete(owner)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenServeLifecycleCovered",
    ): (
        "\\A owner \\in snapshot.serveIdentities: "
        "\\/ owner \\in ExactDecisionTargetNeutralLiveServeIdentitySet "
        "\\/ ExactDecisionTargetNeutralServeIdentityRetired(owner)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOwnerIdentity",
    ): (
        '[ownerKind |-> "Serve", '
        "identity |-> AsyncIoServeJobIdentity(node, job)]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLiveCandidateIdentitySet",
    ): (
        "{ExactDecisionTargetNeutralCandidateOwnerIdentity(candidate): "
        "candidate \\in ActiveScheduledCandidates, "
        "candidate.node \\in Responsive}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLiveServeIdentitySet",
    ): (
        "{ExactDecisionTargetNeutralServeOwnerIdentity(node, job): "
        "node \\in Responsive, "
        "job \\in SequenceSet(asyncIoQueues[node]), "
        'job.class = "Serve"}'
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLiveProducerIdentitySet",
    ): (
        "ExactDecisionTargetNeutralLiveCandidateIdentitySet "
        "\\cup ExactDecisionTargetNeutralLiveServeIdentitySet"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralCandidateOrdinalTokens",
    ): (
        '{[ownerKind |-> "Candidate", node |-> node, '
        "ordinal |-> ordinal]: node \\in Responsive, "
        "ordinal \\in AsyncNextCandidateServiceOrdinal(node) "
        "..(snapshot.candidateCeiling[node] - 1)}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOrdinalTokens",
    ): (
        '{[ownerKind |-> "Serve", node |-> node, '
        "ordinal |-> ordinal]: node \\in Responsive, "
        "ordinal \\in asyncNextServeAdmissionOrdinal[node] "
        "..(snapshot.serveCeiling[node] - 1)}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralModeSet",
    ): '{"RequestClock", "RequestHead", "ResponseHead"}',
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockPending",
    ): (
        "/\\ mode \\in ExactDecisionTargetNeutralModeSet "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ gst "
        "/\\ ExactDecisionTargetNeutralSnapshotActive(snapshot, clockValue) "
        "/\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ ~ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ asyncNow = clockValue"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockExit",
    ): (
        "\\/ ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "\\/ asyncNow > clockValue"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockBlockedAtRank",
    ): (
        "/\\ ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "/\\ rank \\in ExactDecisionTargetNeutralFixedClockCarrier "
        "/\\ ExactDecisionTargetNeutralConcreteFixedClockRank(clockValue) "
        "= rank"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockStrictRankGoal",
    ): (
        "\\/ ExactDecisionTargetNeutralFixedClockExit( "
        "mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "\\/ \\E lowerRank \\in SetLessThan( "
        "sourceRank, ExactDecisionTargetNeutralFixedClockOrdering, "
        "ExactDecisionTargetNeutralFixedClockCarrier): "
        "ExactDecisionTargetNeutralFixedClockBlockedAtRank( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, lowerRank)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralProducerEpisodeAtBudget",
    ): (
        "LET currentRank == "
        "ExactDecisionTargetNeutralConcreteFixedClockRank(clockValue) "
        "IN /\\ ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "/\\ sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier "
        "/\\ currentRank \\in ExactDecisionTargetNeutralFixedClockCarrier "
        "/\\ ~ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank) "
        "/\\ ExactDecisionTargetNeutralProducerPrefix(currentRank) "
        "= ExactDecisionTargetNeutralProducerPrefix(sourceRank) "
        "/\\ budget = "
        "ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralRankCellOutcome",
    ): (
        "\\/ ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank) "
        "\\/ \\E lowerBudget \\in "
        "SetLessThan(budget, OpToRel(<, Nat), Nat) "
        "\\cap (Nat \\ {0}): "
        "ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, lowerBudget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairOwner",
    ): "[ownerKind |-> ownerKind, node |-> node, source |-> source]",
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralOwnerReadyForRankCell",
    ): (
        "/\\ owner \\in "
        "ExactDecisionTargetNeutralFairOwnerSet(initialContext) "
        "/\\ ENABLED (ExactDecisionTargetNeutralFairAction(owner) "
        "/\\ ExactDecisionTargetNeutralRankCellOutcome( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget)')"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlItem",
    ): (
        "/\\ item \\in AsyncNetworkItems "
        "/\\ \\/ \\E vote \\in TimeoutVoteRecordSet, "
        "recipient \\in AsyncCurrentResponsiveVoters: "
        "/\\ TimeoutVoteDeliveryKernelSource(vote, recipient) "
        "/\\ item = TimeoutVoteItem(vote, recipient) "
        "\\/ \\E source, target, tc: "
        "/\\ TimeoutTcKernelSource(source, target, tc, tc.view) "
        "/\\ item = TimeoutCertificateItem(source, target, tc) "
        "\\/ \\E source, target, qc: "
        "/\\ TimeoutDecisionKernelSource(source, target, qc) "
        "/\\ item = CommitCertificateItem(source, target, qc)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlLifecycleStageRank",
    ): (
        "IF TimeoutPhysicalControlGoal(item) THEN 0 "
        "ELSE IF TimeoutPhysicalControlIngressOwner(item) THEN 1 "
        "ELSE IF TimeoutPhysicalControlPacketOwner(item) THEN 2 ELSE 3"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlPacketDependencyRank",
    ): (
        "ExactDecisionTargetNeutralPacketDependencyRankForSnapshot( "
        "snapshot, TimeoutPhysicalControlSelectedPacket(item))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlIngressDependencyRank",
    ): (
        "ExactDecisionRequestIngressRank("
        "item.envelope.recipient, item)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlFrozenSnapshot",
    ): "ExactDecisionTargetNeutralFixedClockSnapshot(clockValue)",
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlFrozenPredecessorSet",
    ): "snapshot.predecessors",
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlFrozenProducerEpisodeRank",
    ): (
        "ExactDecisionTargetNeutralProducerEpisodeRank(snapshot)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlTransportKernelProperties",
    ): (
        "/\\ TimeoutPhysicalControlRetainedKernelProperty(specification) "
        "/\\ TimeoutPhysicalControlPacketKernelProperty(specification) "
        "/\\ TimeoutPhysicalControlIngressKernelProperty(specification)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlRetainedClockAtRank",
    ): (
        "/\\ rank \\in Nat /\\ rank > 0 "
        "/\\ TimeoutPhysicalControlRetainedOwner(item) "
        "/\\ ~TimeoutPhysicalControlGoal(item) "
        "/\\ ~TimeoutPhysicalControlPacketOwner(item) "
        "/\\ ~TimeoutPhysicalControlIngressOwner(item) "
        "/\\ asyncNow < asyncRetransmitDeadlines[item.source] "
        "/\\ asyncRetransmitDeadlines[item.source] = asyncNow + rank"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlRetainedDueOwner",
    ): (
        "/\\ TimeoutPhysicalControlRetainedOwner(item) "
        "/\\ ~TimeoutPhysicalControlGoal(item) "
        "/\\ ~TimeoutPhysicalControlPacketOwner(item) "
        "/\\ ~TimeoutPhysicalControlIngressOwner(item) "
        "/\\ asyncNow >= asyncRetransmitDeadlines[item.source]"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutRetiredFormTcCandidateAbsent",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        'candidate.kind = "FormTC" => ~CandidateScheduled(candidate)'
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderLocalTimeoutViewStepProperty",
    ): (
        "specification => \\A target \\in AsyncCurrentResponsiveVoters, "
        "roundView \\in Views: "
        "(/\\ AdequateLeaderLocalTargetDecisionSource(target) "
        "/\\ nodeView[target] = roundView) "
        "~> (NodeHasDecision(target) \\/ nodeView[target] > roundView)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutViewClosureResidualProperty",
    ): (
        "/\\ TimeoutVoteDeliveryPhysicalKernelProperties(specification) "
        "/\\ TimeoutCertificateDecisionPhysicalKernelProperties(specification)"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOutcomeOrHigherLeaderProgressProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, lockedRound \\in Views, "
        "subject \\in Subjects: RetainedLockModeSource(target, lockedRound, "
        "subject) ~> (RetainedLockModeGoal(target, lockedRound, subject) "
        "\\/ RetainedLockStrictHigherFreshLeaderAuthorityFrontier( target, "
        "lockedRound, subject))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockSourceAuthorityExposureProperty",
    ): (
        "TimeoutViewProgressProperty(specification) "
        "=> (specification => \\A target \\in ValidatorIds, "
        "lockedRound \\in Views, subject \\in Subjects: "
        "RetainedLockModeSource(target, lockedRound, subject) "
        "~> (RetainedLockModeGoal( target, lockedRound, subject) "
        "\\/ RetainedLockSourceExposureFrontier("
        " target, lockedRound, subject)))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockPrepareAuthorityTransportProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "lockedRound \\in Views, subject \\in Subjects, "
        "prepareQc \\in QcRecordSet, sourceView \\in Views: "
        "RetainedLockFreshSourceAuthorityFrontier( target, lockedRound, "
        "subject, prepareQc, sourceView) ~> (RetainedLockModeGoal(target, "
        "lockedRound, subject) \\/ RetainedLockAuthorityTransportFrontierFor( "
        "target, lockedRound, subject, prepareQc))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockTargetLeaderFreshActivationProperty",
    ): (
        "specification => \\A target \\in ValidatorIds, "
        "lockedRound \\in Views, subject \\in Subjects, "
        "prepareQc \\in QcRecordSet: "
        "RetainedLockAuthorityTransportFrontierFor( target, lockedRound, "
        "subject, prepareQc) ~> (RetainedLockModeGoal(target, lockedRound, "
        "subject) \\/ RetainedLockFreshLeaderAuthorityFrontierFor( target, "
        "lockedRound, subject, prepareQc))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockLeaderProducerOriginProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, "
        "lockedRound \\in Views, subject \\in Subjects, "
        "prepareQc \\in QcRecordSet, leaderView \\in Views: "
        "LockedBodyFreshResponsiveLeaderAuthority( target, leader, "
        "lockedRound, subject, prepareQc, leaderView) ~> ("
        "RetainedLockModeGoal(target, lockedRound, subject) \\/ \\E "
        "causalOrigin \\in AsyncCandidateCausalOriginSet: "
        "RetainedLockRankedEpisodeFrontier( target, leader, lockedRound, "
        "subject, prepareQc, leaderView, causalOrigin))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockRankHandoffProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, "
        "lockedRound \\in Views, subject \\in Subjects, "
        "prepareQc \\in QcRecordSet, leaderView \\in Views, "
        "causalOrigin \\in AsyncCandidateCausalOriginSet, "
        "rank \\in ExactLeaderSemanticRankCarrier: "
        "RetainedLockCandidateRankFrontier("
        " target, leader, lockedRound, subject, prepareQc, leaderView, "
        "causalOrigin, rank) ~> (RetainedLockModeGoal(target, lockedRound, subject) "
        "\\/ \\E lowerRank \\in SetLessThan("
        " rank, ExactLeaderSemanticRankOrdering, ExactLeaderSemanticRankCarrier): "
        "RetainedLockCandidateRankFrontier("
        " target, leader, lockedRound, subject, prepareQc, leaderView, "
        "causalOrigin, lowerRank))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockSameOriginLifecycleDispositionClosureProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, lockedRound "
        "\\in Views, subject \\in Subjects, prepareQc \\in QcRecordSet, "
        "leaderView \\in Views, causalOrigin \\in "
        "AsyncCandidateCausalOriginSet, candidate \\in AsyncCandidateSet, "
        "rank \\in ExactLeaderSemanticRankCarrier: "
        "(/\\ RetainedLockProducerEpisodeCoordinates( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, causalOrigin, candidate, "
        "rank) /\\ RetainedLockProducerLifecycleDisposition(candidate)) ~> "
        "RetainedLockProducerEpisodeExitGoal( target, leader, lockedRound, "
        "subject, prepareQc, leaderView, rank)"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockCrossOriginProducerReplacementClosureProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, lockedRound "
        "\\in Views, subject \\in Subjects, prepareQc \\in QcRecordSet, "
        "leaderView \\in Views, causalOrigin \\in "
        "AsyncCandidateCausalOriginSet, candidate \\in AsyncCandidateSet, "
        "rank \\in ExactLeaderSemanticRankCarrier: "
        "(/\\ RetainedLockProducerEpisodeCoordinates( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, causalOrigin, candidate, "
        "rank) /\\ RetainedLockProducerCrossOriginReplacementFrontier( target, "
        "leader, lockedRound, subject, prepareQc, leaderView, causalOrigin, "
        "rank)) ~> RetainedLockProducerEpisodeExitGoal( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, rank)"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockProducerExactReentryClosureProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, lockedRound "
        "\\in Views, subject \\in Subjects, prepareQc \\in QcRecordSet, "
        "leaderView \\in Views, causalOrigin \\in "
        "AsyncCandidateCausalOriginSet, candidate \\in AsyncCandidateSet, "
        "rank \\in ExactLeaderSemanticRankCarrier: "
        "(/\\ RetainedLockProducerEpisodeCoordinates( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, causalOrigin, candidate, "
        "rank) /\\ RetainedLockProducerExactReentryFrontier( target, leader, "
        "lockedRound, subject, prepareQc, leaderView)) ~> "
        "RetainedLockProducerEpisodeExitGoal( target, leader, lockedRound, "
        "subject, prepareQc, leaderView, rank)"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockProducerNonDescentEpisodeClosureProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, lockedRound "
        "\\in Views, subject \\in Subjects, prepareQc \\in QcRecordSet, "
        "leaderView \\in Views, causalOrigin \\in "
        "AsyncCandidateCausalOriginSet, candidate \\in AsyncCandidateSet, "
        "rank \\in ExactLeaderSemanticRankCarrier: "
        "RetainedLockProducerNonDescentEpisodeResidual( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, causalOrigin, candidate, "
        "rank) ~> (RetainedLockModeGoal(target, lockedRound, subject) \\/ "
        "\\E lowerRank \\in SetLessThan( rank, "
        "ExactLeaderSemanticRankOrdering, ExactLeaderSemanticRankCarrier): "
        "RetainedLockOwnerNeutralCandidateRankFrontier( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, lowerRank) "
        "\\/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor( target, "
        "lockedRound, subject, prepareQc, leaderView))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOwnerNeutralRankHandoffProperty",
    ): (
        "specification => \\A target, leader \\in ValidatorIds, lockedRound "
        "\\in Views, subject \\in Subjects, prepareQc \\in QcRecordSet, "
        "leaderView \\in Views, rank \\in ExactLeaderSemanticRankCarrier: "
        "RetainedLockOwnerNeutralCandidateRankFrontier( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, rank) ~> "
        "(RetainedLockModeGoal(target, lockedRound, subject) \\/ \\E lowerRank "
        "\\in SetLessThan( rank, ExactLeaderSemanticRankOrdering, "
        "ExactLeaderSemanticRankCarrier): "
        "RetainedLockOwnerNeutralCandidateRankFrontier( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, lowerRank) "
        "\\/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor( target, "
        "lockedRound, subject, prepareQc, leaderView))"
    ),
    (
        "SumeragiV2DecisionWitnessPreservationProofs",
        "DecisionExactSourceOwner",
    ): (
        "\\/ node \\in AsyncCurrentResponsiveVoters "
        "\\/ HistoricalRecoveryTarget(node)"
    ),
    (
        "SumeragiV2DecisionWitnessPreservationProofs",
        "DecisionExactRetentionFrame",
    ): (
        "/\\ UNCHANGED <<context, nodeView, generation, decisions, applied, "
        "availableBodies, durableBodies, validatedBodies, AsyncRecoveryVars>> "
        "/\\ (AsyncCurrentResponsiveVoters' "
        "\\cup asyncHistoricalRecoveryTargets') "
        "\\subseteq (AsyncCurrentResponsiveVoters "
        "\\cup asyncHistoricalRecoveryTargets) "
        "/\\ DecisionExactAuthenticatedHistoryRetained "
        "/\\ DecisionExactCertifiedRequestsRetained "
        "/\\ DecisionExactScheduledCandidatesRetained"
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "FinalWitnessMonotoneCarrierFrame",
    ): (
        "/\\ OpenProgressWitnessCarrierFrame "
        "/\\ (AsyncCurrentResponsiveVoters' "
        "\\cup asyncHistoricalRecoveryTargets') "
        "\\subseteq (AsyncCurrentResponsiveVoters "
        "\\cup asyncHistoricalRecoveryTargets)"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "DurableCommitReceiptEvidence",
    ): "durableDecisionEvidence \\cup durableApplicationEvidence",
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "CommitReceiptSlot",
    ): "receipt.qc.context.height + 1",
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedDecisionReceiptSourceOwnership",
    ): (
        "\\A decision \\in IndexedDecisionEvidence: "
        "\\E sourceContext \\in JoinedContexts: "
        "decision \\in IndexedCurrentDecisions(sourceContext)"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "ExactPerSlotDurableCommitReceiptSubjectAgreement",
    ): (
        "\\A left, right \\in DurableCommitReceiptEvidence: "
        "CommitReceiptSlot(left) = CommitReceiptSlot(right) "
        "=> /\\ left.qc.context = right.qc.context "
        "/\\ left.qc.subject = right.qc.subject"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "terminalIngressVars",
    ): (
        "<<terminalIngressMode, terminalServiceOwner, "
        "terminalIngressOwners, terminalDetachedOwners, "
        "terminalSuccessfulAdmissions>>"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalClosed",
    ): '"Closed"',
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalReadOnly",
    ): '"TerminalReadOnly"',
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalRetired",
    ): '"TerminalRetired"',
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressModes",
    ): "{TerminalClosed, TerminalReadOnly, TerminalRetired}",
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalAbsorbingModes",
    ): "{TerminalReadOnly, TerminalRetired}",
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressLifecycleInit",
    ): (
        "/\\ terminalIngressMode = TerminalClosed "
        "/\\ terminalServiceOwner = FALSE "
        "/\\ terminalIngressOwners = 0 "
        "/\\ terminalDetachedOwners = 0 "
        "/\\ terminalSuccessfulAdmissions = 0"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "EnterTerminalReadOnly",
    ): (
        "/\\ terminalIngressMode = TerminalClosed "
        "/\\ ~terminalServiceOwner "
        "/\\ \\E retainedHistory \\in Nat: "
        "/\\ terminalIngressMode' = TerminalReadOnly "
        "/\\ terminalServiceOwner' = TRUE "
        "/\\ terminalIngressOwners' = 0 "
        "/\\ terminalDetachedOwners' = retainedHistory "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "AdmitTerminalHistoryEnqueue",
    ): (
        "/\\ terminalIngressMode = TerminalReadOnly "
        "/\\ terminalServiceOwner "
        "/\\ terminalIngressMode' = terminalIngressMode "
        "/\\ terminalServiceOwner' = terminalServiceOwner "
        "/\\ terminalIngressOwners' = terminalIngressOwners + 1 "
        "/\\ terminalDetachedOwners' = terminalDetachedOwners "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions + 1"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "AdmitTerminalHistoryCoalesce",
    ): (
        "/\\ terminalIngressMode = TerminalReadOnly "
        "/\\ terminalServiceOwner "
        "/\\ terminalIngressOwners > 0 "
        "/\\ terminalIngressMode' = terminalIngressMode "
        "/\\ terminalServiceOwner' = terminalServiceOwner "
        "/\\ terminalIngressOwners' = terminalIngressOwners "
        "/\\ terminalDetachedOwners' = terminalDetachedOwners "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions + 1"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "RejectTerminalAdmission",
    ): (
        "/\\ terminalIngressMode \\in TerminalAbsorbingModes "
        "/\\ UNCHANGED terminalIngressVars"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "DequeueTerminalHistory",
    ): (
        "/\\ terminalIngressMode = TerminalReadOnly "
        "/\\ terminalServiceOwner "
        "/\\ terminalDetachedOwners + terminalIngressOwners > 0 "
        "/\\ terminalIngressMode' = terminalIngressMode "
        "/\\ terminalServiceOwner' = terminalServiceOwner "
        "/\\ terminalDetachedOwners' = "
        "IF terminalDetachedOwners > 0 "
        "THEN terminalDetachedOwners - 1 "
        "ELSE terminalDetachedOwners "
        "/\\ terminalIngressOwners' = "
        "IF terminalDetachedOwners > 0 "
        "THEN terminalIngressOwners "
        "ELSE terminalIngressOwners - 1 "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalControlNoOp",
    ): (
        "/\\ terminalIngressMode \\in TerminalAbsorbingModes "
        "/\\ UNCHANGED terminalIngressVars"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "ExitTerminalHistoryService",
    ): (
        "/\\ terminalIngressMode = TerminalReadOnly "
        "/\\ terminalServiceOwner "
        "/\\ terminalIngressMode' = TerminalRetired "
        "/\\ terminalServiceOwner' = FALSE "
        "/\\ terminalIngressOwners' = 0 "
        "/\\ terminalDetachedOwners' = 0 "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "IdempotentTerminalRetire",
    ): (
        "/\\ terminalIngressMode = TerminalRetired "
        "/\\ ~terminalServiceOwner "
        "/\\ UNCHANGED terminalIngressVars"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressStutter",
    ): "UNCHANGED terminalIngressVars",
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressLifecycleNext",
    ): (
        "\\/ EnterTerminalReadOnly "
        "\\/ AdmitTerminalHistoryEnqueue "
        "\\/ AdmitTerminalHistoryCoalesce "
        "\\/ RejectTerminalAdmission "
        "\\/ DequeueTerminalHistory "
        "\\/ TerminalControlNoOp "
        "\\/ ExitTerminalHistoryService "
        "\\/ IdempotentTerminalRetire "
        "\\/ TerminalIngressStutter"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressLifecycleSpec",
    ): (
        "/\\ TerminalIngressLifecycleInit "
        "/\\ [][TerminalIngressLifecycleNext]_terminalIngressVars"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressAbsorbencyInvariant",
    ): (
        "/\\ terminalIngressMode \\in TerminalIngressModes "
        "/\\ terminalServiceOwner \\in BOOLEAN "
        "/\\ terminalIngressOwners \\in Nat "
        "/\\ terminalDetachedOwners \\in Nat "
        "/\\ terminalSuccessfulAdmissions \\in Nat "
        "/\\ (terminalServiceOwner "
        "<=> terminalIngressMode = TerminalReadOnly) "
        "/\\ (terminalIngressMode = TerminalClosed "
        "=> /\\ terminalIngressOwners = 0 "
        "/\\ terminalDetachedOwners = 0) "
        "/\\ (terminalIngressMode = TerminalRetired "
        "=> /\\ ~terminalServiceOwner "
        "/\\ terminalIngressOwners = 0 "
        "/\\ terminalDetachedOwners = 0)"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalModeAbsorbingStep",
    ): (
        "terminalIngressMode \\in TerminalAbsorbingModes "
        "=> terminalIngressMode' \\in TerminalAbsorbingModes"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalRetiredAbsorbingStep",
    ): (
        "terminalIngressMode = TerminalRetired "
        "=> terminalIngressMode' = TerminalRetired"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "EveryServiceOwnerExitRetiresStep",
    ): (
        "terminalServiceOwner /\\ ~terminalServiceOwner' "
        "=> /\\ terminalIngressMode = TerminalReadOnly "
        "/\\ terminalIngressMode' = TerminalRetired "
        "/\\ terminalIngressOwners' = 0 "
        "/\\ terminalDetachedOwners' = 0 "
        "/\\ terminalSuccessfulAdmissions' = "
        "terminalSuccessfulAdmissions"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "NoPostOwnerAdmissionStep",
    ): (
        "~terminalServiceOwner "
        "/\\ terminalIngressMode \\in TerminalAbsorbingModes "
        "=> terminalSuccessfulAdmissions' = terminalSuccessfulAdmissions"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressAbsorbencyStepProperties",
    ): (
        "/\\ TerminalModeAbsorbingStep "
        "/\\ TerminalRetiredAbsorbingStep "
        "/\\ EveryServiceOwnerExitRetiresStep "
        "/\\ NoPostOwnerAdmissionStep"
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressProcessLifetimeAbsorbencyProperty",
    ): (
        "/\\ []TerminalIngressAbsorbencyInvariant "
        "/\\ [][TerminalModeAbsorbingStep]_terminalIngressVars "
        "/\\ [][TerminalRetiredAbsorbingStep]_terminalIngressVars "
        "/\\ [][EveryServiceOwnerExitRetiresStep]_terminalIngressVars "
        "/\\ [][NoPostOwnerAdmissionStep]_terminalIngressVars"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalExactApplication",
    ): (
        "/\\ initialContext \\in AdmissibleContextRecords "
        "/\\ node \\in Responsive "
        "/\\ IndexedAsync(initialContext)!NodeHasApplication(node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryRunnerOwned",
    ): (
        "\\/ node \\in "
        "IndexedAsync(initialContext)!AsyncCurrentResponsiveVoters "
        "\\/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryOpenable",
    ): (
        "/\\ IndexedCore(initialContext, 7) "
        "/\\ IndexedHistoricalRecoveryTargetReady(initialContext, node) "
        "/\\ \\E server \\in ValidatorIds, "
        "source \\in Chain!DecisionEvidenceSet: "
        "IndexedHistoricalRecoverySourceReady( "
        "initialContext, server, source)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryTargetOwned",
    ): (
        "/\\ HistoricalRecoveryOutstanding(initialContext, node) "
        "/\\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionOwned",
    ): (
        "/\\ HistoricalRecoveryOutstanding(initialContext, node) "
        "/\\ IndexedHistoricalRecoveryRunnerOwned(initialContext, node) "
        "/\\ IndexedAsync(initialContext)!NodeHasDecision(node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryEntryGoal",
    ): (
        "\\/ IndexedHistoricalExactApplication(initialContext, node) "
        "\\/ IndexedHistoricalDecisionOwned(initialContext, node) "
        "\\/ IndexedHistoricalRecoveryOpenable(initialContext, node) "
        "\\/ IndexedHistoricalRecoveryTargetOwned(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual",
    ): (
        "/\\ HistoricalRecoveryOutstanding(initialContext, node) "
        "/\\ ~IndexedHistoricalRecoveryEntryGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node) "
        "~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCommitRequestIdentity",
    ): (
        '/\\ request.kind = "CommitCertificateRequest" '
        "/\\ request.source = node "
        "/\\ request.envelope.height = initialContext.height "
        "/\\ request.envelope.recipient \\in "
        "(IndexedAsync(initialContext)!CurrentVoters \\ {node}) "
        "\\cap IndexedAsync(initialContext)! "
        "AsyncResponsiveAppliedArchiveServers"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRequestInIngress",
    ): (
        "\\E source \\in "
        "IndexedAsync(initialContext)!AsyncIngressSources: "
        "request \\in SequenceSet(IndexedScheduler(initialContext, 40) "
        "[request.envelope.recipient][source])"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRequestInServeQueue",
    ): (
        "\\E job \\in SequenceSet( "
        "IndexedScheduler(initialContext, 10) "
        "[request.envelope.recipient]): "
        '/\\ job.class = "Serve" '
        "/\\ job.candidate.item = request"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRequestPhysicalOwner",
    ): (
        "\\/ request \\in IndexedScheduler(initialContext, 37) "
        "\\/ \\E packet \\in IndexedScheduler(initialContext, 39): "
        "packet.item = request "
        "\\/ IndexedHistoricalRequestInIngress(initialContext, request) "
        "\\/ IndexedHistoricalRequestInServeQueue(initialContext, request)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCommitRequestOwned",
    ): (
        "/\\ IndexedHistoricalRecoveryTargetOwned(initialContext, node) "
        "/\\ \\E request: "
        "/\\ IndexedHistoricalCommitRequestIdentity( "
        "initialContext, node, request) "
        "/\\ IndexedHistoricalRequestPhysicalOwner( "
        "initialContext, request)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCommitResponseIdentity",
    ): (
        "/\\ IndexedHistoricalCommitRequestIdentity( "
        "initialContext, node, request) "
        "/\\ qc.context = initialContext "
        '/\\ qc.phase = "Commit" '
        "/\\ response = IndexedAsync(initialContext)! "
        "CommitCertificateResponseItem(request, qc) "
        "/\\ response.source = "
        "IndexedAsync(initialContext)!AsyncUntrustedSource "
        "/\\ response.envelope.recipient = node "
        "/\\ response.envelope.request = request"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCommitResponsePublished",
    ): (
        "/\\ IndexedHistoricalRecoveryTargetOwned(initialContext, node) "
        "/\\ \\E request, qc, response: "
        "/\\ response \\in IndexedScheduler(initialContext, 35) "
        "/\\ IndexedHistoricalCommitResponseIdentity( "
        "initialContext, node, request, qc, response)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateCommandFor",
    ): (
        "/\\ IndexedHistoricalRecoveryTargetOwned(initialContext, node) "
        "/\\ IndexedHistoricalCertificateLineageCandidateFor( "
        "initialContext, node, qc, candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateReceivedQcLineageSource",
    ): (
        "/\\ qc.context = initialContext /\\ qc.phase = \"Commit\" "
        "/\\ IndexedAsync(initialContext)!QcAt(node, qc) "
        "\\in IndexedCore(initialContext, 15)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateDecisionWalLineageSource",
    ): (
        "/\\ qc.context = initialContext /\\ qc.phase = \"Commit\" "
        "/\\ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE) "
        "\\in IndexedCore(initialContext, 39)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedResponsiveActiveRosterAt",
    ): (
        "Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryActivationPrefixResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node) "
        "~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node) "
        "\\/ /\\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext) "
        "/\\ IndexedResponsiveActiveRosterAt(initialContext) "
        "/\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "/\\ IndexedHistoricalRecoveryArchiveOwnerJoined(initialContext) "
        "/\\ IndexedResponsiveActiveRosterAt(initialContext) "
        "/\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node) "
        "~> (IndexedHistoricalRecoveryEntryGoal(initialContext, node) "
        "\\/ /\\ IndexedHistoricalRecoveryTypedArchiveAuthority( "
        "initialContext) "
        "/\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "/\\ IndexedHistoricalRecoveryTypedArchiveAuthority(initialContext) "
        "/\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node) "
        "~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryEntryCompletionAt",
    ): (
        "\\A node \\in Responsive: "
        "IndexedHistoricalRecoveryEntryGoal(initialContext, node) "
        "~> HistoricalRecoveryComplete(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryEntryCompletionBelow",
    ): (
        "\\A blockHeight \\in 0..targetContext.height: "
        "blockHeight < targetContext.height "
        "=> IndexedHistoricalRecoveryEntryCompletionAt( "
        "IndexedAncestorContext(targetContext, blockHeight))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityProgressAt",
    ): (
        "\\A node \\in Responsive: "
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "initialContext, node) "
        "~> IndexedHistoricalRecoveryEntryGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityProgressBelow",
    ): (
        "\\A blockHeight \\in 0..targetContext.height: "
        "blockHeight < targetContext.height "
        "=> \\A node \\in Responsive: "
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "IndexedAncestorContext(targetContext, blockHeight), node) "
        "~> IndexedHistoricalRecoveryEntryGoal( "
        "IndexedAncestorContext(targetContext, blockHeight), node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressAtContext",
    ): (
        "\\A node \\in Responsive, rank \\in 1..6: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, rank)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryJointProgressAt",
    ): (
        "/\\ IndexedHistoricalRecoveryAuthorityProgressAt(initialContext) "
        "/\\ IndexedHistoricalRecoveryEntryCompletionAt(initialContext) "
        "/\\ IndexedHistoricalDecisionRankProgressAtContext(initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryJointProgressBelow",
    ): (
        "\\A blockHeight \\in 0..targetContext.height: "
        "blockHeight < targetContext.height "
        "=> IndexedHistoricalRecoveryJointProgressAt( "
        "IndexedAncestorContext(targetContext, blockHeight))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryJointProgressThroughHeight",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "initialContext.height <= limit "
        "=> IndexedHistoricalRecoveryJointProgressAt(initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryJointProgressProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalRecoveryJointProgressAt(initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateReceivedQcLineageInvariantAt",
    ): (
        "\\A node \\in Responsive, qc: "
        "IndexedHistoricalCertificateReceivedQcLineageSource( "
        "initialContext, node, qc) => "
        "\\/ IndexedDecisionWitness(initialContext)!NodeHasDecision(node) "
        "\\/ IndexedDecisionWitness(initialContext)!NodeHasApplication(node) "
        "\\/ \\E candidate: "
        "IndexedHistoricalCertificateLineageCandidateFor( "
        "initialContext, node, qc, candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateDecisionWalLineageInvariantAt",
    ): (
        "\\A node \\in Responsive, qc: "
        "IndexedHistoricalCertificateDecisionWalLineageSource( "
        "initialContext, node, qc) => "
        "\\/ IndexedDecisionWitness(initialContext)!NodeHasDecision(node) "
        "\\/ IndexedDecisionWitness(initialContext)!NodeHasApplication(node) "
        "\\/ \\E candidate: "
        "IndexedHistoricalCertificateLineageCandidateFor( "
        "initialContext, node, qc, candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt",
    ): (
        "\\A candidate: /\\ candidate \\in "
        "IndexedDecisionWitness(initialContext)!AsyncCandidateSet "
        "/\\ IndexedDecisionWitness(initialContext)! "
        "CandidateConsumerCurrent(candidate) "
        "/\\ IndexedDecisionWitness(initialContext)!CandidateScheduled(candidate) "
        "/\\ IndexedDecisionWitness(initialContext)! "
        "AsyncCommitImportExecutionNeedsLineage(candidate) => "
        "IndexedDecisionWitness(initialContext)! "
        "AsyncCommitImportExecutionProvenance(candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalLineageInvariantAt",
    ): (
        "/\\ IndexedHistoricalCertificateReceivedQcLineageInvariantAt( "
        "initialContext) "
        "/\\ IndexedHistoricalCertificateDecisionWalLineageInvariantAt( "
        "initialContext) "
        "/\\ IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt( "
        "initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalLineageInvariant",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCommitCertificateImported",
    ): (
        "/\\ IndexedHistoricalRecoveryTargetOwned(initialContext, node) "
        "/\\ \\E qc \\in IndexedCore(initialContext, 23): "
        "/\\ qc.context = initialContext "
        '/\\ qc.phase = "Commit" '
        "/\\ \\/ IndexedAsync(initialContext)!QcAt(node, qc) "
        "\\in IndexedCore(initialContext, 15) "
        "\\/ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE) "
        "\\in IndexedCore(initialContext, 39) "
        "\\/ \\E candidate: "
        "IndexedHistoricalCertificateCommandFor( "
        "initialContext, node, qc, candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateStageAt",
    ): (
        "/\\ rank \\in 1..4 "
        "/\\ IndexedHistoricalRecoveryTargetOwned(initialContext, node) "
        "/\\ ~IndexedHistoricalDecisionOwned(initialContext, node) "
        "/\\ CASE rank = 4 -> "
        "/\\ ~IndexedHistoricalCommitRequestOwned( "
        "initialContext, node) "
        "/\\ ~IndexedHistoricalCommitResponsePublished( "
        "initialContext, node) "
        "/\\ ~IndexedHistoricalCommitCertificateImported( "
        "initialContext, node) "
        "[] rank = 3 -> "
        "/\\ IndexedHistoricalCommitRequestOwned( "
        "initialContext, node) "
        "/\\ ~IndexedHistoricalCommitResponsePublished( "
        "initialContext, node) "
        "/\\ ~IndexedHistoricalCommitCertificateImported( "
        "initialContext, node) "
        "[] rank = 2 -> "
        "/\\ IndexedHistoricalCommitResponsePublished( "
        "initialContext, node) "
        "/\\ ~IndexedHistoricalCommitCertificateImported( "
        "initialContext, node) "
        "[] rank = 1 -> "
        "IndexedHistoricalCommitCertificateImported( "
        "initialContext, node) "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateGoal",
    ): (
        "\\/ IndexedHistoricalExactApplication(initialContext, node) "
        "\\/ IndexedHistoricalDecisionOwned(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankProgressAt",
    ): (
        "IndexedHistoricalCertificateStageAt( "
        "initialContext, node, rank) "
        "~> (IndexedHistoricalCertificateGoal(initialContext, node) "
        "\\/ \\E lower \\in SetLessThan( "
        "rank, OpToRel(<, Nat), Nat): "
        "IndexedHistoricalCertificateStageAt( "
        "initialContext, node, lower))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateDiscoveryRunnerResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateRankProgressAt( "
        "initialContext, node, 4)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRequestServiceResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateRankProgressAt( "
        "initialContext, node, 3)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateResponseImportResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateRankProgressAt( "
        "initialContext, node, 2)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateImportedDecisionResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateRankProgressAt( "
        "initialContext, node, 1)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankProgressResidualProperty",
    ): (
        "/\\ IndexedHistoricalCertificateDiscoveryRunnerResidualProperty "
        "/\\ IndexedHistoricalCertificateRequestServiceResidualProperty "
        "/\\ IndexedHistoricalCertificateResponseImportResidualProperty "
        "/\\ IndexedHistoricalCertificateImportedDecisionResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRecord",
    ): (
        "/\\ [node |-> node, qc |-> qc] "
        "\\in IndexedCore(initialContext, 48) "
        "/\\ qc.context = initialContext "
        '/\\ qc.phase = "Commit"'
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionCertifiedRequestActiveExact",
    ): (
        "\\E request \\in IndexedScheduler(initialContext, 37): "
        "request \\in IndexedAsync(initialContext)!"
        "CertifiedRequestOutbox(node, qc)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionCandidateFor",
    ): (
        "/\\ candidate \\in "
        "IndexedAsync(initialContext)!AsyncCandidateSet "
        '/\\ candidate.class = "Completion" '
        "/\\ candidate.node = node "
        "/\\ candidate.height = qc.context.height "
        "/\\ candidate.view = qc.view "
        "/\\ candidate.subject = qc.subject "
        "/\\ IndexedAsync(initialContext)! "
        "CandidateConsumerCurrent(candidate) "
        "/\\ IndexedAsync(initialContext)!CandidateScheduled(candidate) "
        "/\\ candidate.kind = commandKind "
        '/\\ CASE commandKind = "FetchBody" '
        "-> candidate.evidence = qc "
        '[] commandKind = "FetchCertifiedBody" '
        "-> /\\ candidate.item.kind = "
        '"CertifiedResponse" '
        "/\\ candidate.item.envelope.recipient = node "
        "/\\ candidate.item.envelope.height = initialContext.height "
        "/\\ candidate.item.envelope.view = qc.view "
        "/\\ candidate.item.envelope.subject = qc.subject "
        "/\\ candidate.item.envelope.requestHash = "
        "IndexedAsync(initialContext)! "
        "AsyncCertifiedRequestHashOf(node, qc, 0) "
        "/\\ candidate.item.envelope.signatureOwner = "
        "candidate.item.envelope.archiveServer "
        "/\\ candidate.item.envelope.citedResponder \\in qc.signers "
        "/\\ IndexedAsync(initialContext)! "
        "CertifiedResponseAuthenticatedOccurrence( "
        "candidate.item) "
        "/\\ IndexedAsync(initialContext)! "
        "CertifiedResponseCapabilityAuthorized( "
        "candidate.item) "
        "/\\ candidate = IndexedAsync(initialContext)! "
        "CertifiedResponseCandidate(candidate.item) "
        '[] commandKind \\in {"StoreBody", "ValidateBody", "Apply"} '
        "-> TRUE "
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageAt",
    ): (
        "/\\ rank \\in 1..6 "
        "/\\ IndexedHistoricalDecisionOwned(initialContext, node) "
        "/\\ \\E qc: "
        "/\\ IndexedHistoricalDecisionRecord( "
        "initialContext, node, qc) "
        "/\\ CASE rank = 6 -> \\E candidate: "
        "IndexedHistoricalDecisionCandidateFor( "
        'initialContext, node, qc, candidate, "FetchBody") '
        "[] rank = 5 -> "
        "IndexedHistoricalDecisionCertifiedRequestActiveExact( "
        "initialContext, node, qc) "
        "[] rank = 4 -> \\E candidate: "
        "IndexedHistoricalDecisionCandidateFor( "
        "initialContext, node, qc, candidate, "
        '"FetchCertifiedBody") '
        "[] rank = 3 -> \\E candidate: "
        "IndexedHistoricalDecisionCandidateFor( "
        'initialContext, node, qc, candidate, "StoreBody") '
        "[] rank = 2 -> \\E candidate: "
        "IndexedHistoricalDecisionCandidateFor( "
        'initialContext, node, qc, candidate, "ValidateBody") '
        "[] rank = 1 -> \\E candidate: "
        "IndexedHistoricalDecisionCandidateFor( "
        'initialContext, node, qc, candidate, "Apply") '
        "[] OTHER -> FALSE"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageGoal",
    ): (
        "\\/ IndexedHistoricalExactApplication(initialContext, node) "
        "\\/ \\E rank \\in 1..6: "
        "IndexedHistoricalDecisionStageAt( "
        "initialContext, node, rank)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageOwnershipResidual",
    ): (
        "/\\ IndexedHistoricalDecisionOwned(initialContext, node) "
        "/\\ ~IndexedHistoricalDecisionStageGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageOwnershipResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionStageOwnershipResidual( "
        "initialContext, node) "
        "~> IndexedHistoricalDecisionStageGoal(initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressAt",
    ): (
        "IndexedHistoricalDecisionStageAt(initialContext, node, rank) "
        "~> (IndexedHistoricalExactApplication(initialContext, node) "
        "\\/ \\E lower \\in SetLessThan( "
        "rank, OpToRel(<, Nat), Nat): "
        "IndexedHistoricalDecisionStageAt( "
        "initialContext, node, lower))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionFetchBodyResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 6)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionCertifiedRequestResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 5)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 4)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStoreBodyResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 3)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionValidateBodyResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 2)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionApplyResidualProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalDecisionRankProgressAt( "
        "initialContext, node, 1)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressResidualProperty",
    ): (
        "/\\ IndexedHistoricalDecisionFetchBodyResidualProperty "
        "/\\ IndexedHistoricalDecisionCertifiedRequestResidualProperty "
        "/\\ IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty "
        "/\\ IndexedHistoricalDecisionStoreBodyResidualProperty "
        "/\\ IndexedHistoricalDecisionValidateBodyResidualProperty "
        "/\\ IndexedHistoricalDecisionApplyResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryTemporalResidualKernels",
    ): (
        "/\\ IndexedHistoricalFixedClockPacketRemainingTemporalResidual "
        "/\\ IndexedHistoricalCommitTransportResidualKernelProperties "
        "/\\ IndexedHistoricalDecisionTransportResidualKernelProperties"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier",
    ): (
        "{record \\in AsyncCandidateServiceTombstones: record.identity \\in "
        "carrier}"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalCandidateIdentityBudgetBridgeProperty",
    ): (
        "/\\ (specification => "
        "[]AsyncCandidateServiceTombstoneLifecycleInvariant) /\\ "
        "(specification => \\A carrier: IsFiniteSet(carrier) => []( "
        "Cardinality( "
        "HistoricalTemporalCandidateServiceTombstonesInIdentityCarrier( "
        "carrier)) <= Cardinality(carrier))) /\\ (specification => [](\\A "
        "candidate \\in AsyncCandidateSet: /\\ "
        "AsyncCandidateServiceActiveTombstone(candidate) /\\ "
        "[AsyncNext]_AsyncAllVars /\\ "
        "~AsyncCandidateServiceExitThisStep(candidate) => "
        "AsyncCandidateServiceActiveTombstone(candidate)')) /\\ (specification "
        "=> [](\\A left, right \\in AsyncCandidateSet: /\\ left.node = "
        "right.node /\\ left.consumerContext = right.consumerContext /\\ "
        "left.height = right.height /\\ left.view = right.view /\\ left.subject "
        "= right.subject /\\ left.kind = right.kind /\\ left.class = "
        "right.class /\\ left.item # NoAsyncItem /\\ right.item # NoAsyncItem "
        "/\\ left.item.kind = \"CertifiedResponse\" /\\ right.item = [left.item "
        "EXCEPT !.source = right.item.source] /\\ "
        "AsyncRouteNeutralCandidateEvidence(left.evidence) = "
        "AsyncRouteNeutralCandidateEvidence(right.evidence) /\\ "
        "left.causalOrigin = right.causalOrigin /\\ "
        "left.bodyIdentity = right.bodyIdentity /\\ left.manifestIdentity = "
        "right.manifestIdentity /\\ left.commitmentIdentity = "
        "right.commitmentIdentity => AsyncCandidateServiceIdentity(left) = "
        "AsyncCandidateServiceIdentity(right))) /\\ (specification => [](\\A "
        "identity \\in AsyncCandidateAdmissionIdentitySet: /\\ "
        "AsyncCandidateAdmissionIdentityObsolete(identity) /\\ identity \\notin "
        "AsyncScheduledCandidateAdmissionIdentities /\\ gst /\\ "
        "[AsyncNext]_AsyncAllVars => /\\ "
        "AsyncCandidateAdmissionIdentityObsolete(identity)' /\\ identity "
        "\\notin AsyncScheduledCandidateAdmissionIdentities')) /\\ "
        "(specification => [](\\A identity \\in "
        "AsyncCandidateAdmissionIdentitySet: /\\ identity.service.phase = "
        "\"DeliverChunk\" /\\ "
        "AsyncCandidateAdmissionIdentityTerminallyCovered(identity) /\\ "
        "identity \\notin AsyncScheduledCandidateAdmissionIdentities /\\ gst /\\ "
        "[AsyncNext]_AsyncAllVars => /\\ "
        "AsyncCandidateAdmissionIdentityTerminallyCovered( identity)' /\\ "
        "identity \\notin AsyncScheduledCandidateAdmissionIdentities')) /\\ "
        "(specification => [](\\A identity \\in "
        "AsyncCandidateAdmissionIdentitySet: /\\ identity.service.phase = "
        "\"DeliverChunk\" /\\ identity \\in "
        "AsyncScheduledCandidateAdmissionIdentities /\\ gst /\\ "
        "[AsyncNext]_AsyncAllVars /\\ identity \\notin "
        "AsyncScheduledCandidateAdmissionIdentities' => "
        "AsyncCandidateAdmissionIdentityLifecycleCovered( identity)'))"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeReservationsInIdentityCarrier",
    ): (
        "{reservation \\in asyncServeReservations: reservation.identity \\in "
        "carrier}"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeTombstonesInIdentityCarrier",
    ): (
        "{tombstone \\in asyncServeTombstones: tombstone.identity \\in carrier}"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeRollbackTombstonesInIdentityCarrier",
    ): (
        "UNION { {tombstone \\in reservation.rollbackTombstones: "
        "tombstone.identity \\in carrier}: reservation \\in "
        "asyncServeReservations}"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeRetiredRecordsInIdentityCarrier",
    ): (
        "HistoricalTemporalServeTombstonesInIdentityCarrier(carrier) \\cup "
        "HistoricalTemporalServeRollbackTombstonesInIdentityCarrier(carrier)"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeExactRetryCoalescingAction",
    ): (
        "\\/ CoalesceExactServeIngressCapacity(node, candidate) \\/ "
        "ResumeExactServeCapacity(node, candidate) \\/ "
        "CoalesceExactServeCapacity(node, candidate) \\/ "
        "CoalesceSupersededExactServeRequest(node, candidate) \\/ "
        "RejectConflictingExactServeRequest(node, candidate)"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeIdentityBudgetBridgeProperty",
    ): (
        "/\\ (specification => []AsyncServeLifecycleTypeInvariant) /\\ "
        "(specification => [](/\\ IsFiniteSet(asyncServeReservations) /\\ "
        "IsFiniteSet(asyncServeTombstones) /\\ "
        "Cardinality(asyncServeTombstones) <= "
        "Cardinality(AsyncServeLifecycleFamilies))) /\\ (specification => \\A "
        "carrier: IsFiniteSet(carrier) => [](/\\ IsFiniteSet( "
        "HistoricalTemporalServeReservationsInIdentityCarrier( carrier)) /\\ "
        "IsFiniteSet( HistoricalTemporalServeTombstonesInIdentityCarrier( "
        "carrier)) /\\ IsFiniteSet( "
        "HistoricalTemporalServeRollbackTombstonesInIdentityCarrier( "
        "carrier)) /\\ IsFiniteSet( "
        "HistoricalTemporalServeRetiredRecordsInIdentityCarrier( carrier)) /\\ "
        "Cardinality( HistoricalTemporalServeTombstonesInIdentityCarrier( "
        "carrier)) <= Cardinality(carrier))) /\\ (specification => [](\\A node "
        "\\in ValidatorIds, identity \\in AsyncServeLogicalRequestIdentities: "
        "AsyncServeLiveReservationOwned(node, identity) => /\\ Cardinality( "
        "AsyncServeReservationRecords(node, identity)) = 1 /\\ "
        "AsyncServeAdmissionOrdinal(node, identity) < "
        "asyncNextServeAdmissionOrdinal[node])) /\\ (specification => [](\\A "
        "node \\in ValidatorIds, family \\in AsyncServeLifecycleFamilies: "
        "AsyncServeLifecycleFamilyOwned(node, family) => /\\ Cardinality( "
        "AsyncServeFamilyAdmissionRecords(node, family) \\cup "
        "AsyncServeFamilyTombstoneRecords(node, family)) = 1 /\\ "
        "AsyncServeFamilyOwnerIdentity(node, family) \\in "
        "AsyncServeLogicalRequestIdentities /\\ "
        "AsyncServeFamilyHighWatermark(node, family) \\in Views)) /\\ "
        "(specification => [](\\A node \\in ValidatorIds, identity \\in "
        "AsyncServeLogicalRequestIdentities: /\\ AsyncServeJobQueued(node, "
        "identity) /\\ gst /\\ [AsyncNext]_AsyncAllVars /\\ "
        "~AsyncServeJobQueued(node, identity)' => "
        "AsyncServeLifecycleTombstone(node, identity)')) /\\ (specification => "
        "[](\\A node \\in ValidatorIds, identity \\in "
        "AsyncServeLogicalRequestIdentities: /\\ "
        "AsyncServeLogicalIdentityRetiredOrSuperseded( node, identity) /\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars => /\\ "
        "AsyncServeLogicalIdentityRetiredOrSuperseded( node, identity)' /\\ "
        "~AsyncServeJobQueued(node, identity)')) /\\ (specification => [](\\A "
        "node, candidate: HistoricalTemporalServeExactRetryCoalescingAction( "
        "node, candidate) => UNCHANGED asyncNextServeAdmissionOrdinal)) /\\ "
        "(specification => [](\\A node \\in ValidatorIds, left, right \\in "
        "AsyncCertifiedRequestItems \\cup AsyncCommitCertificateRequestItems: "
        "AsyncServeLogicalRequestIdentity(node, left) = "
        "AsyncServeLogicalRequestIdentity(node, right) => LET identity == "
        "AsyncServeLogicalRequestIdentity(node, left) IN /\\ "
        "AsyncServeReservationRecords(node, identity) = "
        "AsyncServeReservationRecords( node, "
        "AsyncServeLogicalRequestIdentity(node, right)) /\\ "
        "AsyncServeTombstoneRecords(node, identity) = "
        "AsyncServeTombstoneRecords( node, "
        "AsyncServeLogicalRequestIdentity(node, right)) /\\ "
        "(AsyncServeLifecycleOwned(node, identity) => "
        "AsyncServeAdmissionOrdinal(node, identity) = "
        "AsyncServeAdmissionOrdinal( node, AsyncServeLogicalRequestIdentity( "
        "node, right)))))"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty",
    ): (
        "/\\ "
        "HistoricalTemporalCandidateIdentityBudgetBridgeProperty(specification) "
        "/\\ "
        "HistoricalTemporalServeIdentityBudgetBridgeProperty(specification)"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalIdentityLifecycleInvariant",
    ): (
        "/\\ AsyncCandidateServiceTombstoneLifecycleInvariant /\\ "
        "AsyncServeLifecycleTypeInvariant"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs",
        "HistoricalCommitTransportResidualKernels",
    ): (
        "/\\ HistoricalCommitArchiveRouteAvailabilityProperty(specification) "
        "/\\ HistoricalCommitPhysicalTransportKernelProperties(specification)"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs",
        "HistoricalDecisionCertifiedTransportResidualKernels",
    ): "HistoricalDecisionCertifiedTransportKernelProperties(specification)",
    (
        "SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs",
        "HistoricalRecoveryTransportResidualKernels",
    ): (
        "/\\ HistoricalCommitTransportResidualKernels(specification) "
        "/\\ HistoricalDecisionCertifiedTransportResidualKernels(specification)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTemporalSupportAt",
    ): (
        "/\\ IndexedHistoricalTransport(initialContext)! "
        "AsyncFrozenContextAt(initialContext) /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "AsyncStrongTypeInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "AsyncProgressOwnershipInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "AsyncCandidateProducerContinuationExternalCoverageInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "AsyncCandidateProducerContinuationLocalReplayCapacityInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "DecisionTimeoutFrontierInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "DecisionFrontierUniquenessInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "PostGstReplayQuarantineExcluded /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "ExactDecisionFanoutRetentionInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "Stage2BusyKernelInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "AsyncDeferredHandoffOwnershipInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalTemporalIdentityLifecycleInvariant /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitCertificateRequestCompletenessInvariant"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockIdentityBridgeProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty( "
        "IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockTemporalLeafProperties",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalTemporalFixedClockLeaves(IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual",
    ): (
        "/\\ \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryPacketConcreteActionServiceProperty( "
        "IndexedChainSpec) "
        "/\\ IndexedHistoricalCandidateCausalDagTemporalResidual "
        "/\\ \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryServeExactWorkerStepProperty( IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPacketConcreteActionServiceResidual",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryPacketConcreteActionServiceProperty( "
        "IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTimedCandidateStarvationResidual",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryTimedCandidateStarvationProperty( IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalCandidateCausalDagTemporalResidual",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "/\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryCandidateExactRunnerStepProperty( IndexedChainSpec) "
        "/\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty( "
        "IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalServeExactWorkerTemporalProperties",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryServeExactWorkerStepProperty( IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPacketRemainingTemporalResidual",
    ): (
        "/\\ IndexedHistoricalFixedClockPacketConcreteActionServiceResidual "
        "/\\ IndexedHistoricalTimedCandidateStarvationResidual"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockNonPacketServiceProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockNonPacketServiceProperty( "
        "IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalCommitTransportResidualKernelProperties",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitRequestPacketEmissionKernelProperty( "
        "IndexedChainSpec) /\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitRequestIngressKernelProperty(IndexedChainSpec) /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitResponseAdmissionKernelProperty(IndexedChainSpec)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalDecisionTransportResidualKernelProperties",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDecisionRequestPacketEmissionKernelProperty( "
        "IndexedChainSpec) /\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalDecisionRequestIngressKernelProperty(IndexedChainSpec) /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDecisionResponseAdmissionKernelProperty(IndexedChainSpec)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificatePhysicalResidualKernels",
    ): (
        "/\\ IndexedHistoricalFixedClockPacketRemainingTemporalResidual "
        "/\\ IndexedHistoricalCommitTransportResidualKernelProperties"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPrerequisiteSurface",
    ): (
        "/\\ IndexedHistoricalFixedClockIdentityBridgeProperty /\\ \\A "
        "initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockTemporalPrerequisites( "
        "IndexedChainSpec)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionCandidateRankProgressResidualProperty",
    ): (
        "/\\ IndexedHistoricalDecisionFetchBodyResidualProperty /\\ "
        "IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty /\\ "
        "IndexedHistoricalDecisionStoreBodyResidualProperty /\\ "
        "IndexedHistoricalDecisionValidateBodyResidualProperty /\\ "
        "IndexedHistoricalDecisionApplyResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateCandidateTailAt",
    ): (
        "/\\ IndexedHistoricalCertificateStageAt(initialContext, node, 1) /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitDecisionCandidateOwned(node, kind)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateCandidateTailGoal",
    ): (
        "\\/ IndexedHistoricalCertificateGoal(initialContext, node) \\/ CASE "
        "kind = \"DeliverQC\" -> IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitDecisionCandidateOwned( node, \"BeginDecision\") [] "
        "kind = \"BeginDecision\" -> "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalCommitDecisionCandidateOwned( node, \"PersistDecision\") [] "
        "kind = \"PersistDecision\" -> FALSE [] OTHER -> FALSE"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateCandidateTailProgressProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in Responsive, "
        "kind \\in {\"DeliverQC\", \"BeginDecision\", \"PersistDecision\"}: "
        "IndexedHistoricalCertificateCandidateTailAt( initialContext, node, "
        "kind) ~> IndexedHistoricalCertificateCandidateTailGoal( "
        "initialContext, node, kind)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalImportAt",
    ): (
        "/\\ IndexedHistoricalCertificateStageAt(initialContext, node, 1) "
        "/\\ \\E qc \\in IndexedCore(initialContext, 23): "
        "/\\ qc.context = initialContext "
        '/\\ qc.phase = "Commit" '
        "/\\ \\/ IndexedAsync(initialContext)!QcAt(node, qc) "
        "\\in IndexedCore(initialContext, 15) "
        "\\/ IndexedAsync(initialContext)!DecisionWal(node, qc, FALSE) "
        "\\in IndexedCore(initialContext, 39) "
        "\\/ \\E candidate: "
        "IndexedHistoricalCertificateCommandFor( "
        "initialContext, node, qc, candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalImportCandidateEntryProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateLocalImportAt(initialContext, node) ~> "
        "IndexedHistoricalCertificateCandidateEntryGoal( "
        "initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in Responsive: "
        "IndexedHistoricalCertificateStageAt(initialContext, node, 1) ~> "
        "IndexedHistoricalCertificateCandidateEntryGoal( "
        "initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateReceivedQcLocalImportAt( "
        "initialContext, node) ~> "
        "IndexedHistoricalCertificateCandidateEntryGoal( "
        "initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateDecisionWalLocalImportAt( "
        "initialContext, node) ~> "
        "IndexedHistoricalCertificateCandidateEntryGoal( "
        "initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRemainingCorridorProperty",
    ): (
        "/\\ IndexedHistoricalCertificateDiscoveryRunnerResidualProperty /\\ "
        "IndexedHistoricalCertificateRequestServiceResidualProperty /\\ "
        "IndexedHistoricalCertificateResponseImportResidualProperty"
    ),
}
