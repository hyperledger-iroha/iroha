---- MODULE SumeragiV2ChainEpochRefinementShard14 ----
EXTENDS SumeragiV2ChainEpochRefinementShard13

THEOREM IndexedActivationOutcomeJoinsExactContext ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ initialContext.height > 0
      /\ initialContext =
           CanonicalIndexedContext(initialContext.height)
      /\ SuccessorPublicationOrSuperseded(
           CanonicalIndexedContext(initialContext.height - 1), node)
      => node \in joinedByContext[initialContext]
BY Isa DEF IndexedCompositionInvariant,
           IndexedJoinedThroughLocalHeight,
           SuccessorPublicationOrSuperseded,
           SuccessorHeightActivated,
           SuccessorActivationMarker,
           CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights,
           Chain!ContextRecord, Chain!HistoryThrough

THEOREM IndexedNodeJoinIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    node \in joinedByContext[initialContext]
      /\ [IndexedChainNext]_IndexedChainVars
      => node \in joinedByContext[initialContext]'
BY Isa, JoinedMembershipIsMonotone DEF IndexedChainVars

THEOREM IndexedActivationPendingIntoContextEventuallyJoins ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       IndexedActivationPendingIntoContext(initialContext, node)
         ~> node \in joinedByContext[initialContext]
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedActivationPendingIntoContext(initialContext, node)
                 ~> node \in joinedByContext[initialContext]
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedActivationPendingIntoContext(initialContext, node)
             ~> SuccessorPublicationOrSuperseded(
                  CanonicalIndexedContext(initialContext.height - 1), node)
      BY <1>1, PTL DEF IndexedSuccessorActivationProgress,
                         IndexedActivationPendingIntoContext
    <2>4. node \in joinedByContext[initialContext]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[initialContext]'
      BY <1>1, IndexedNodeJoinIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                 IndexedActivationOutcomeJoinsExactContext, PTL
         DEF IndexedActivationPendingIntoContext
  <1> QED BY <1>1

THEOREM IndexedActivationOutcomeLeavesPastOrRecoveryOutstanding ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ initialContext.height > 0
      /\ initialContext.height < MaxHeight
      /\ initialContext =
           CanonicalIndexedContext(initialContext.height)
      /\ SuccessorPublicationOrSuperseded(
           CanonicalIndexedContext(initialContext.height - 1), node)
      => \/ IndexedNodePastContext(initialContext, node)
         \/ HistoricalRecoveryOutstanding(initialContext, node)
BY Isa, IndexedActivationOutcomeJoinsExactContext
   DEF IndexedCompositionInvariant,
       JoinedRoutingInvariant, IndexedApplicationsRespectNodeHeight,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       IndexedNodePastContext, HistoricalRecoveryOutstanding,
       IndexedAsync!NodeHasApplication,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories

THEOREM IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       initialContext.height < MaxHeight
         => IndexedActivationPendingIntoContext(initialContext, node)
              ~> (IndexedNodePastContext(initialContext, node)
                   \/ HistoricalRecoveryOutstanding(initialContext, node))
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              initialContext.height < MaxHeight
         PROVE IndexedActivationPendingIntoContext(initialContext, node)
                 ~> (IndexedNodePastContext(initialContext, node)
                      \/ HistoricalRecoveryOutstanding(
                           initialContext, node))
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedActivationPendingIntoContext(initialContext, node)
             ~> SuccessorPublicationOrSuperseded(
                  CanonicalIndexedContext(initialContext.height - 1), node)
      BY <1>1, PTL DEF IndexedSuccessorActivationProgress,
                         IndexedActivationPendingIntoContext
    <2>4. IndexedActivationPendingIntoContext(initialContext, node)
             => initialContext.height > 0
      BY DEF IndexedActivationPendingIntoContext
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4,
                 IndexedActivationOutcomeLeavesPastOrRecoveryOutstanding,
                 PTL DEF IndexedActivationPendingIntoContext
  <1> QED BY <1>1

IndexedTargetHeightStepPremise(targetContext, blockHeight) ==
  /\ IndexedTargetJoined(targetContext)
  /\ IndexedResponsiveHeightReached(blockHeight)

THEOREM IndexedTargetStepEitherPassedOrRecoveryOutstanding ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      \A node \in Responsive:
        IndexedCompositionInvariant
          /\ IndexedJoinedThroughLocalHeight
          /\ IndexedTargetHeightStepPremise(targetContext, blockHeight)
          /\ blockHeight < targetContext.height
          => \/ IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
             \/ HistoricalRecoveryOutstanding(
                  IndexedAncestorContext(targetContext, blockHeight), node)
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedReachedAncestorClassifiesEveryResponsiveNode
   DEF IndexedTargetHeightStepPremise,
       IndexedResponsiveHeightReached, IndexedNodePastContext,
       IndexedResponsiveLagAt,
       HistoricalRecoveryOutstanding,
       IndexedActivationPendingIntoContext,
       IndexedCompositionInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAncestorContext, CanonicalIndexedContext,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       IndexedAsync!NodeHasApplication

THEOREM IndexedAdvanceReadyEitherPassedOrNeedsRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedJoinedThroughLocalHeight
      /\ IndexedContextAdvanceReady(initialContext)
      => \/ IndexedNodePastContext(initialContext, node)
         \/ IndexedActivationPendingIntoContext(initialContext, node)
         \/ HistoricalRecoveryOutstanding(initialContext, node)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedCompositionInvariant,
              IndexedJoinedThroughLocalHeight,
              IndexedContextAdvanceReady(initialContext)
         PROVE \/ IndexedNodePastContext(initialContext, node)
               \/ IndexedActivationPendingIntoContext(initialContext, node)
               \/ HistoricalRecoveryOutstanding(initialContext, node)
    <2>1. PICK descendantContext \in JoinedContexts:
             /\ descendantContext.height > initialContext.height
             /\ descendantContext =
                  Chain!ContextRecord(
                    descendantContext.height,
                    Chain!HistoryThrough(descendantContext.height))
      BY <1>1 DEF IndexedContextAdvanceReady,
                    JoinedCanonicalDescendant
    <2>2. descendantContext \in AdmissibleContextRecords
      BY <2>1 DEF JoinedContexts
    <2>3. initialContext.height \in 0..descendantContext.height
      BY <1>1, <2>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>4. IndexedAncestorContext(
             descendantContext, initialContext.height)
           = CanonicalIndexedContext(initialContext.height)
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedJoinedTargetIdentifiesEveryCanonicalAncestor
         DEF IndexedTargetJoined
    <2>5. initialContext =
             CanonicalIndexedContext(initialContext.height)
      BY <1>1 DEF IndexedContextAdvanceReady,
                    IndexedCompositionInvariant,
                    JoinedContextCertificationInvariant,
                    CanonicalIndexedContext
    <2>6. IndexedAncestorContext(
             descendantContext, initialContext.height)
           = initialContext
      BY <2>4, <2>5
    <2>7. IndexedTargetHeightStepPremise(
             descendantContext, initialContext.height)
      BY <1>1, <2>1
         DEF IndexedContextAdvanceReady,
             IndexedTargetHeightStepPremise, IndexedTargetJoined
    <2>8. \/ IndexedNodePastContext(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
             \/ HistoricalRecoveryOutstanding(
                  IndexedAncestorContext(
                    descendantContext, initialContext.height), node)
      BY <1>1, <2>1, <2>2, <2>3, <2>7,
         IndexedTargetStepEitherPassedOrRecoveryOutstanding
    <2> QED BY <2>6, <2>8
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyEventuallyPassesEachResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedNodePastContext(initialContext, node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedContextAdvanceReady(initialContext)
             => \/ IndexedNodePastContext(initialContext, node)
                \/ IndexedActivationPendingIntoContext(
                     initialContext, node)
                \/ HistoricalRecoveryOutstanding(
                     initialContext, node)
      BY <1>1, IndexedAdvanceReadyEitherPassedOrNeedsRecovery
    <2>4. initialContext.height < MaxHeight
      BY <1>1, JoinedCanonicalDescendantStaysWithinHorizon
         DEF IndexedContextAdvanceReady
    <2>5. IndexedActivationPendingIntoContext(initialContext, node)
             ~> (IndexedNodePastContext(initialContext, node)
                  \/ HistoricalRecoveryOutstanding(initialContext, node))
      BY <1>1, <2>4,
         IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding
    <2>6. HistoricalRecoveryOutstanding(initialContext, node)
             ~> IndexedNodePastContext(initialContext, node)
      BY <1>1, <2>4,
         IndexedHistoricalRecoveryAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       limit \in Nat:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsivePrefixPast(initialContext, limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE \A limit \in Nat:
                 IndexedContextAdvanceReady(initialContext)
                   ~> IndexedResponsivePrefixPast(initialContext, limit)
    <2> DEFINE P(limit) ==
           IndexedContextAdvanceReady(initialContext)
             ~> IndexedResponsivePrefixPast(initialContext, limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        <4>1. IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(initialContext, 0)
          BY <1>1, <3>1,
             IndexedAdvanceReadyEventuallyPassesEachResponsiveNode
        <4> QED BY <4>1, PTL
             DEF P, IndexedResponsivePrefixPast
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsivePrefixPast
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat,
                  P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. IndexedContextAdvanceReady(initialContext)
                 ~> IndexedNodePastContext(
                      initialContext, limit + 1)
          BY <1>1, <3>1,
             IndexedAdvanceReadyEventuallyPassesEachResponsiveNode
        <4>2. IndexedResponsivePrefixPast(initialContext, limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsivePrefixPast(
                      initialContext, limit)'
          BY <1>1, <2>2, IndexedResponsivePrefixPastIsStable
        <4>3. IndexedNodePastContext(initialContext, limit + 1)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedNodePastContext(
                      initialContext, limit + 1)'
          BY <1>1, <3>1, IndexedNodePastContextIsStable,
             Isa DEF ModelConfiguration, ValidatorIds
        <4>4. IndexedResponsivePrefixPast(initialContext, limit + 1)
                 <=> /\ IndexedResponsivePrefixPast(
                           initialContext, limit)
                     /\ IndexedNodePastContext(
                           initialContext, limit + 1)
          BY <2>2, <3>1, Isa
             DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsivePrefixPast(initialContext, limit)
                 => IndexedResponsivePrefixPast(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedAdvanceReadyReachesSuccessorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A initialContext \in AdmissibleContextRecords:
      IndexedContextAdvanceReady(initialContext)
        ~> IndexedResponsiveHeightReached(initialContext.height + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedContextAdvanceReady(initialContext)
                 ~> IndexedResponsiveHeightReached(
                      initialContext.height + 1)
    <2>1. IndexedContextAdvanceReady(initialContext)
             ~> IndexedResponsivePrefixPast(initialContext, N - 1)
      BY <1>1, IndexedAdvanceReadyPassesEveryFiniteResponsivePrefix,
         SMT DEF ModelConfiguration
    <2>2. IndexedResponsivePrefixPast(initialContext, N - 1)
             => IndexedResponsiveHeightReached(
                  initialContext.height + 1)
      BY SMT DEF IndexedResponsivePrefixPast,
                 IndexedResponsiveHeightReached,
                 IndexedNodePastContext, ModelConfiguration,
                 ValidatorIds, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords, Heights
    <2> QED BY <2>1, <2>2, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepEventuallyPassesEachResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        \A node \in Responsive:
          blockHeight < targetContext.height
            => IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              NEW node \in Responsive,
              blockHeight < targetContext.height
         PROVE IndexedTargetHeightStepPremise(
                   targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetHeightStepPremise(
                  targetContext, blockHeight)
             => \/ IndexedNodePastContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ IndexedActivationPendingIntoContext(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
                \/ HistoricalRecoveryOutstanding(
                     IndexedAncestorContext(targetContext, blockHeight),
                     node)
      BY <1>1,
         IndexedTargetStepEitherPassedOrRecoveryOutstanding
    <2>4. IndexedAncestorContext(targetContext, blockHeight).height
             < MaxHeight
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         Isa DEF IndexedAncestorContext,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>5. IndexedActivationPendingIntoContext(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> (IndexedNodePastContext(
                    IndexedAncestorContext(targetContext, blockHeight), node)
                  \/ HistoricalRecoveryOutstanding(
                       IndexedAncestorContext(targetContext, blockHeight),
                       node))
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedActivationPendingEventuallyLeavesPastOrRecoveryOutstanding
    <2>6. HistoricalRecoveryOutstanding(
               IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1, <2>4,
         IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedHistoricalRecoveryAdvancesResponsiveNode
         DEF IndexedNodePastContext
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepPassesEveryFiniteResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        \A limit \in Nat:
          blockHeight < targetContext.height
            => IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE \A limit \in Nat:
                 IndexedTargetHeightStepPremise(targetContext, blockHeight)
                   ~> IndexedResponsivePrefixPast(
                        IndexedAncestorContext(targetContext, blockHeight),
                        limit)
    <2> DEFINE P(limit) ==
           IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight), 0)
          BY <1>1, <3>1,
             IndexedTargetStepEventuallyPassesEachResponsiveNode
        <4> QED BY <4>1, PTL DEF P, IndexedResponsivePrefixPast
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsivePrefixPast
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. IndexedTargetHeightStepPremise(targetContext, blockHeight)
                 ~> IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <1>1, <3>1,
             IndexedTargetStepEventuallyPassesEachResponsiveNode
        <4>2. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)'
          BY <1>1, <2>2, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedResponsivePrefixPastIsStable
        <4>3. IndexedNodePastContext(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedNodePastContext(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)'
          BY <1>1, <3>1,
             IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedNodePastContextIsStable,
             Isa DEF ModelConfiguration, ValidatorIds
        <4>4. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 <=> /\ IndexedResponsivePrefixPast(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit)
                     /\ IndexedNodePastContext(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit + 1)
          BY <2>2, <3>1, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsivePrefixPast(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 => IndexedResponsivePrefixPast(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsivePrefixPast
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetAdvancesOneAncestorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        blockHeight < targetContext.height
          => (IndexedTargetJoined(targetContext)
                /\ IndexedResponsiveHeightReached(blockHeight))
               ~> IndexedResponsiveHeightReached(blockHeight + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                  ~> IndexedResponsiveHeightReached(blockHeight + 1)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>4. IndexedResponsiveHeightReached(blockHeight)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(blockHeight)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF Heights, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords
    <2>5. IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  N - 1)
      BY <1>1, IndexedTargetStepPassesEveryFiniteResponsivePrefix,
         SMT DEF ModelConfiguration
    <2>6. IndexedResponsivePrefixPast(
             IndexedAncestorContext(targetContext, blockHeight), N - 1)
             => IndexedResponsiveHeightReached(blockHeight + 1)
      BY <1>1, SMT
         DEF IndexedResponsivePrefixPast,
             IndexedResponsiveHeightReached,
             IndexedNodePastContext, IndexedAncestorContext,
             ModelConfiguration, ValidatorIds,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2> QED BY <2>3, <2>4, <2>5, <2>6, PTL
         DEF IndexedTargetHeightStepPremise
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetEventuallyReachesEveryAncestorHeight ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  =>
    \A targetContext \in AdmissibleContextRecords:
      \A blockHeight \in 0..targetContext.height:
        IndexedTargetJoined(targetContext)
          ~> IndexedResponsiveHeightReached(blockHeight)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords
         PROVE \A blockHeight \in 0..targetContext.height:
                 IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight)
    <2> DEFINE P(blockHeight) ==
           blockHeight <= targetContext.height
             => (IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight))
    <2>1. P(0)
      BY SMT DEF P, IndexedResponsiveHeightReached,
                 AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights, ModelConfiguration,
                 ValidatorIds
    <2>2. ASSUME NEW blockHeight \in Nat,
                  P(blockHeight)
           PROVE P(blockHeight + 1)
      <3>1. CASE blockHeight < targetContext.height
        <4>1. blockHeight \in 0..targetContext.height
          BY <1>1, <2>2, <3>1, SMT
             DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
        <4>2. (IndexedTargetJoined(targetContext)
                  /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> IndexedResponsiveHeightReached(blockHeight + 1)
          BY <1>1, <3>1, <4>1,
             IndexedJoinedTargetAdvancesOneAncestorHeight
        <4>3. IndexedTargetJoined(targetContext)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedTargetJoined(targetContext)'
          BY <1>1, IndexedTargetJoinedIsStable
        <4> QED BY <2>2, <3>1, <4>2, <4>3, PTL DEF P
      <3>2. CASE blockHeight >= targetContext.height
        BY <3>2, SMT DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A blockHeight \in Nat: P(blockHeight)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

IndexedResponsiveJoinPrefixAt(initialContext, limit) ==
  \A node \in Responsive \cap (0..limit):
    node \in joinedByContext[initialContext]

THEOREM IndexedResponsiveJoinPrefixAtIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     limit \in Nat:
    IndexedResponsiveJoinPrefixAt(initialContext, limit)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsiveJoinPrefixAt(initialContext, limit)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedResponsiveJoinPrefixAt, IndexedChainVars

THEOREM IndexedReachedAncestorEventuallyJoinsResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         \A node \in Responsive:
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height,
              NEW node \in Responsive
         PROVE (IndexedTargetJoined(targetContext)
                  /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> node \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]
    <2>1. IndexedChainSpec => []IndexedCompositionInvariant
      BY IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedChainSpec => []IndexedJoinedThroughLocalHeight
      BY IndexedChainSpecJoinsEveryNodeThroughLocalHeight
    <2>3. IndexedCompositionInvariant
             /\ IndexedJoinedThroughLocalHeight
             /\ IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight)
             => \/ node \in joinedByContext[
                       IndexedAncestorContext(targetContext, blockHeight)]
                \/ IndexedActivationPendingIntoContext(
                     IndexedAncestorContext(targetContext, blockHeight), node)
      BY <1>1, IndexedReachedAncestorClassifiesEveryResponsiveNode
    <2>4. IndexedActivationPendingIntoContext(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedActivationPendingIntoContextEventuallyJoins
    <2>5. node \in joinedByContext[
             IndexedAncestorContext(targetContext, blockHeight)]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]'
      BY <1>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
         IndexedNodeJoinIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

=============================================================================
