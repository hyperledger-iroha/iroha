---- MODULE SumeragiV2ChainEpochRefinementShard15 ----
EXTENDS SumeragiV2ChainEpochRefinementShard14

THEOREM IndexedReachedAncestorEventuallyJoinsEveryResponsivePrefix ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         \A limit \in Nat:
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> IndexedResponsiveJoinPrefixAt(
                  IndexedAncestorContext(targetContext, blockHeight), limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              NEW blockHeight \in 0..targetContext.height
         PROVE \A limit \in Nat:
                 (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                   ~> IndexedResponsiveJoinPrefixAt(
                        IndexedAncestorContext(targetContext, blockHeight),
                        limit)
    <2> DEFINE P(limit) ==
           (IndexedTargetJoined(targetContext)
             /\ IndexedResponsiveHeightReached(blockHeight))
             ~> IndexedResponsiveJoinPrefixAt(
                  IndexedAncestorContext(targetContext, blockHeight), limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        BY <1>1, <3>1,
           IndexedReachedAncestorEventuallyJoinsResponsiveNode, PTL
           DEF P, IndexedResponsiveJoinPrefixAt
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, IndexedResponsiveJoinPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. (IndexedTargetJoined(targetContext)
                 /\ IndexedResponsiveHeightReached(blockHeight))
                 ~> limit + 1 \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]
          BY <1>1, <3>1,
             IndexedReachedAncestorEventuallyJoinsResponsiveNode
        <4>2. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 /\ [IndexedChainNext]_IndexedChainVars
                 => IndexedResponsiveJoinPrefixAt(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit)'
          BY <1>1, <2>2, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedResponsiveJoinPrefixAtIsStable
        <4>3. limit + 1 \in joinedByContext[
                 IndexedAncestorContext(targetContext, blockHeight)]
                 /\ [IndexedChainNext]_IndexedChainVars
                 => limit + 1 \in joinedByContext[
                      IndexedAncestorContext(targetContext, blockHeight)]'
          BY <1>1, <3>1, IndexedAdmissibleTargetHasAdmissibleAncestors,
             IndexedNodeJoinIsStable
        <4>4. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight),
                 limit + 1)
                 <=> /\ IndexedResponsiveJoinPrefixAt(
                           IndexedAncestorContext(targetContext, blockHeight),
                           limit)
                     /\ limit + 1 \in joinedByContext[
                           IndexedAncestorContext(targetContext, blockHeight)]
          BY <2>2, <3>1, Isa DEF IndexedResponsiveJoinPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL DEF P
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedResponsiveJoinPrefixAt(
                 IndexedAncestorContext(targetContext, blockHeight), limit)
                 => IndexedResponsiveJoinPrefixAt(
                      IndexedAncestorContext(targetContext, blockHeight),
                      limit + 1)
          BY <2>2, <3>2, Isa DEF IndexedResponsiveJoinPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       \A blockHeight \in 0..targetContext.height:
         (IndexedTargetJoined(targetContext)
           /\ IndexedResponsiveHeightReached(blockHeight))
           ~> IndexedAllResponsiveJoined(
                IndexedAncestorContext(targetContext, blockHeight))
BY IndexedReachedAncestorEventuallyJoinsEveryResponsivePrefix, SMT
   DEF IndexedResponsiveJoinPrefixAt,
       IndexedAllResponsiveJoined,
       ModelConfiguration, ValidatorIds

(***************************************************************************
Strict-ancestor catchup kernel.

The older finite-height induction above consumes the global
`IndexedExactHistoricalRecoveryProgress` property.  That interface is too
broad for proving historical source acquisition itself: recovery at the
frozen target height would then be one of its own premises.

`IndexedStrictAncestorRecoveryAdvance` removes that cycle.  For one frozen
joined target it assumes only that an outstanding responsive node eventually
passes each strict ancestor.  Existing indexed safety classifies every node
as already past, pending the typed successor lifecycle, or outstanding at
that exact ancestor.  The finite responsive-prefix and height inductions
below then reach the target height and join every responsive node to the
target.  No current-height recovery, one-height completion, or indexed
height-liveness property is consumed.
***************************************************************************)

IndexedStrictAncestorRecoveryAdvance(targetContext) ==
  \A blockHeight \in 0..targetContext.height:
    blockHeight < targetContext.height
      => \A node \in Responsive:
           HistoricalRecoveryOutstanding(
             IndexedAncestorContext(targetContext, blockHeight), node)
             ~> IndexedNodePastContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)

THEOREM IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              \A node \in Responsive:
                blockHeight < targetContext.height
                  => IndexedTargetHeightStepPremise(
                       targetContext, blockHeight)
                       ~> IndexedNodePastContext(
                            IndexedAncestorContext(
                              targetContext, blockHeight),
                            node)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
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
      BY <1>1, IndexedTargetStepEitherPassedOrRecoveryOutstanding
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
      BY <1>1 DEF IndexedStrictAncestorRecoveryAdvance
    <2> QED BY <2>1, <2>2, <2>3, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedTargetStepPassesEveryResponsivePrefixFromStrictAncestorRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              \A limit \in Nat:
                blockHeight < targetContext.height
                  => IndexedTargetHeightStepPremise(
                       targetContext, blockHeight)
                       ~> IndexedResponsivePrefixPast(
                            IndexedAncestorContext(
                              targetContext, blockHeight),
                            limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
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
             IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery
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
             IndexedTargetStepPassesEachResponsiveNodeFromStrictAncestorRecovery
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

THEOREM IndexedJoinedTargetAdvancesAncestorFromStrictRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              blockHeight < targetContext.height
                => (IndexedTargetJoined(targetContext)
                      /\ IndexedResponsiveHeightReached(blockHeight))
                     ~> IndexedResponsiveHeightReached(blockHeight + 1)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext),
              NEW blockHeight \in 0..targetContext.height,
              blockHeight < targetContext.height
         PROVE (IndexedTargetJoined(targetContext)
                   /\ IndexedResponsiveHeightReached(blockHeight))
                  ~> IndexedResponsiveHeightReached(blockHeight + 1)
    <2>1. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>2. IndexedResponsiveHeightReached(blockHeight)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(blockHeight)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF Heights, AdmissibleContextRecords,
                 FrozenContextAdmissible, ContextRecords
    <2>3. IndexedTargetHeightStepPremise(targetContext, blockHeight)
             ~> IndexedResponsivePrefixPast(
                  IndexedAncestorContext(targetContext, blockHeight),
                  N - 1)
      BY <1>1,
         IndexedTargetStepPassesEveryResponsivePrefixFromStrictAncestorRecovery,
         SMT DEF ModelConfiguration
    <2>4. IndexedResponsivePrefixPast(
             IndexedAncestorContext(targetContext, blockHeight), N - 1)
             => IndexedResponsiveHeightReached(blockHeight + 1)
      BY <1>1, SMT
         DEF IndexedResponsivePrefixPast,
             IndexedResponsiveHeightReached,
             IndexedNodePastContext, IndexedAncestorContext,
             ModelConfiguration, ValidatorIds,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2> QED BY <2>1, <2>2, <2>3, <2>4, PTL
         DEF IndexedTargetHeightStepPremise
  <1> QED BY <1>1

THEOREM IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => \A blockHeight \in 0..targetContext.height:
              IndexedTargetJoined(targetContext)
                ~> IndexedResponsiveHeightReached(blockHeight)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext)
         PROVE \A blockHeight \in 0..targetContext.height:
                 IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight)
    <2> DEFINE P(blockHeight) ==
           blockHeight <= targetContext.height
             => (IndexedTargetJoined(targetContext)
                   ~> IndexedResponsiveHeightReached(blockHeight))
    <2>1. P(0)
      BY <1>1, SMT
         DEF P, IndexedResponsiveHeightReached,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights, ModelConfiguration,
             ValidatorIds
    <2>2. ASSUME NEW blockHeight \in Nat, P(blockHeight)
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
             IndexedJoinedTargetAdvancesAncestorFromStrictRecovery
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

THEOREM IndexedStrictAncestorRecoveryEventuallyJoinsTarget ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => IndexedTargetJoined(targetContext)
              ~> IndexedAllResponsiveJoined(targetContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW targetContext \in AdmissibleContextRecords,
              IndexedStrictAncestorRecoveryAdvance(targetContext)
         PROVE IndexedTargetJoined(targetContext)
                 ~> IndexedAllResponsiveJoined(targetContext)
    <2>1. targetContext.height \in 0..targetContext.height
      BY <1>1, Isa
         DEF AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, Heights
    <2>2. IndexedTargetJoined(targetContext)
             ~> IndexedResponsiveHeightReached(targetContext.height)
      BY <1>1, <2>1,
         IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery
    <2>3. IndexedTargetJoined(targetContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(targetContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>4. (IndexedTargetJoined(targetContext)
              /\ IndexedResponsiveHeightReached(targetContext.height))
             ~> IndexedAllResponsiveJoined(
                  IndexedAncestorContext(
                    targetContext, targetContext.height))
      BY <1>1, <2>1,
         IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode
    <2>5. IndexedAncestorContext(targetContext, targetContext.height)
             = targetContext
      BY <1>1, Isa
         DEF IndexedAncestorContext, AdmissibleContextRecords,
             FrozenContextAdmissible, ContextRecords, LineagesAt,
             Heights, ContextRecord
    <2> QED BY <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A targetContext \in AdmissibleContextRecords:
       IndexedStrictAncestorRecoveryAdvance(targetContext)
         => IndexedTargetJoined(targetContext)
              ~> (Responsive \subseteq
                    IndexedAsync(targetContext)!AsyncActiveServiceNodes)
BY IndexedStrictAncestorRecoveryEventuallyJoinsTarget,
   IndexedChainSpecEstablishesCompositionInvariant,
   IndexedAllResponsiveJoinedHasActiveRoster, PTL

IndexedAllResponsiveExactApplicationsAt(initialContext) ==
  \A node \in Responsive:
    IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedContextCompleted(initialContext) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)
  ELSE \A node \in Responsive:
         nodeHeight[node] > initialContext.height

THEOREM IndexedAllResponsiveExactApplicationsImpliesContextCompleted ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedAllResponsiveExactApplicationsAt(initialContext)
      => IndexedContextCompleted(initialContext)
BY Isa DEF IndexedCompositionInvariant,
           IndexedApplicationsRespectNodeHeight,
           IndexedAllResponsiveExactApplicationsAt,
           IndexedContextCompleted,
           IndexedAsync!NodeHasApplication

THEOREM IndexedAllResponsiveExactApplicationsIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAllResponsiveExactApplicationsAt(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedAllResponsiveExactApplicationsAt(initialContext)'
BY Isa DEF IndexedAllResponsiveExactApplicationsAt,
           IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt,
           IndexedApplications, IndexedAsync!NodeHasApplication

THEOREM IndexedContextCompletedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedContextCompleted(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedContextCompleted(initialContext)'
BY Isa, IndexedAllResponsiveExactApplicationsIsStable,
   IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedContextCompleted,
       IndexedAllResponsiveExactApplicationsAt,
       ModelConfiguration, ValidatorIds, Heights,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords

THEOREM VerificationSuccessorHeightImpliesContextCompleted ==
  VerificationContext \in AdmissibleContextRecords
    /\ VerificationContext.height < MaxHeight
    /\ IndexedResponsiveHeightReached(VerificationContext.height + 1)
    => IndexedContextCompleted(VerificationContext)
BY Isa DEF IndexedResponsiveHeightReached,
           IndexedContextCompleted,
           IndexedAllResponsiveExactApplicationsAt,
           ModelConfiguration, ValidatorIds,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

(***************************************************************************
Once the voting roster has applied the exact frontier receipt, every remaining
responsive observer is either already past the context or has an exact
historical-recovery source/target. Finiteness of Responsive plus
IndexedExactHistoricalRecoveryProgress closes those observers one at a time.
At MaxHeight the outcome is exact per-context application evidence; below the
horizon the same receipt handoff advances nodeHeight.
***************************************************************************)
THEOREM VerificationAppliedFrontierEventuallyCompletes ==
  /\ IndexedChainSpec
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  /\ VerificationContext \in AdmissibleContextRecords
  => (/\ IndexedTargetJoined(VerificationContext)
      /\ IndexedResponsiveHeightReached(VerificationContext.height)
      /\ IndexedAsync(VerificationContext)!
           AsyncAllResponsiveAppliedAt(VerificationContext))
       ~> IndexedContextCompleted(VerificationContext)
BY IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode,
   IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
   IndexedHistoricalRecoveryAdvancesResponsiveNode,
   IndexedContextCompletedIsStable, PTL
   DEF IndexedTargetJoined, IndexedResponsiveHeightReached,
       IndexedContextCompleted,
       IndexedAllResponsiveExactApplicationsAt,
       HistoricalRecoveryOutstanding,
       IndexedHistoricalRecoveryReady,
       HistoricalRecoveryComplete, IndexedAsync!AsyncVotersAt,
       IndexedAsync!AsyncAllResponsiveAppliedAt,
       IndexedAsync!NodeHasApplication,
       ModelConfiguration, ValidatorIds

THEOREM VerificationAdvanceReadyEventuallyCompletes ==
  /\ IndexedChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => IndexedContextAdvanceReady(VerificationContext)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationContext \in AdmissibleContextRecords
         PROVE IndexedContextAdvanceReady(VerificationContext)
                 ~> IndexedContextCompleted(VerificationContext)
    <2>1. IndexedContextAdvanceReady(VerificationContext)
             ~> IndexedResponsiveHeightReached(
                  VerificationContext.height + 1)
      BY <1>1, IndexedAdvanceReadyReachesSuccessorHeight
    <2>2. IndexedContextAdvanceReady(VerificationContext)
             => VerificationContext.height < MaxHeight
      BY <1>1, JoinedCanonicalDescendantStaysWithinHorizon
         DEF IndexedContextAdvanceReady
    <2>3. VerificationContext.height < MaxHeight
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height + 1)
             => IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationSuccessorHeightImpliesContextCompleted
    <2> QED BY <2>1, <2>2, <2>3, PTL
  <1> QED BY <1>1

THEOREM VerificationReachedEscapeEventuallyCompletes ==
  /\ IndexedChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationContext \in AdmissibleContextRecords
    => (/\ IndexedTargetJoined(VerificationContext)
        /\ IndexedResponsiveHeightReached(VerificationContext.height)
        /\ VerificationFrontierEscape)
         ~> IndexedContextCompleted(VerificationContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationContext \in AdmissibleContextRecords
         PROVE (/\ IndexedTargetJoined(VerificationContext)
                /\ IndexedResponsiveHeightReached(
                     VerificationContext.height)
                /\ VerificationFrontierEscape)
                 ~> IndexedContextCompleted(VerificationContext)
    <2>1. (/\ IndexedTargetJoined(VerificationContext)
            /\ IndexedResponsiveHeightReached(
                 VerificationContext.height)
            /\ JoinedCanonicalDescendant(VerificationContext))
             => IndexedContextAdvanceReady(VerificationContext)
      BY <1>1 DEF IndexedContextAdvanceReady, IndexedTargetJoined
    <2>2. IndexedContextAdvanceReady(VerificationContext)
             ~> IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationAdvanceReadyEventuallyCompletes
    <2>3. (/\ IndexedTargetJoined(VerificationContext)
            /\ IndexedResponsiveHeightReached(
                 VerificationContext.height)
            /\ IndexedAsync(VerificationContext)!
                 AsyncAllResponsiveAppliedAt(VerificationContext))
             ~> IndexedContextCompleted(VerificationContext)
      BY <1>1, VerificationAppliedFrontierEventuallyCompletes
    <2> QED BY <2>1, <2>2, <2>3, PTL
         DEF VerificationFrontierEscape
  <1> QED BY <1>1

THEOREM VerificationJoinedTargetEventuallyReachesAndEscapes ==
  /\ IndexedLiveChainSpec
    /\ IndexedExactHistoricalRecoveryProgress
    /\ IndexedSuccessorActivationProgress
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    => IndexedTargetJoined(VerificationContext)
         ~> (/\ IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height)
             /\ VerificationFrontierEscape)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7))
         PROVE IndexedTargetJoined(VerificationContext)
                 ~> (/\ IndexedTargetJoined(VerificationContext)
                     /\ IndexedResponsiveHeightReached(
                          VerificationContext.height)
                     /\ VerificationFrontierEscape)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <2>0, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedTargetJoined(VerificationContext)
             ~> IndexedResponsiveHeightReached(
                  VerificationContext.height)
      BY <1>1, <2>0,
         IndexedJoinedTargetEventuallyReachesEveryAncestorHeight,
         Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>4. IndexedTargetJoined(VerificationContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedTargetJoined(VerificationContext)'
      BY <1>1, IndexedTargetJoinedIsStable
    <2>5. IndexedResponsiveHeightReached(VerificationContext.height)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedResponsiveHeightReached(
                  VerificationContext.height)'
      BY <1>1, IndexedResponsiveHeightReachedIsStable,
         Isa DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, Heights
    <2>6. (IndexedTargetJoined(VerificationContext)
             /\ IndexedResponsiveHeightReached(
                  VerificationContext.height))
             ~> IndexedAllResponsiveJoined(VerificationContext)
      BY <1>1,
         <2>0,
         IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode,
         IndexedJoinedTargetIdentifiesEveryCanonicalAncestor,
         PTL DEF IndexedAncestorContext
    <2>7. IndexedAllResponsiveJoined(VerificationContext)
             ~> VerificationFrontierEscape
      BY <1>1, VerificationActivatedFrontierEventuallyEscapes
    <2>8. IndexedCompositionInvariant
             /\ VerificationFrontierEscape
             /\ [IndexedChainNext]_IndexedChainVars
             => VerificationFrontierEscape'
      BY <1>1, VerificationFrontierEscapeIsStable
    <2> QED BY <2>1, <2>2, <2>3, <2>4,
                 <2>5, <2>6, <2>7, <2>8, PTL
  <1> QED BY <1>1

IndexedHeightLivenessProperty ==
  (/\ VerificationContext \in AdmissibleContextRecords
   /\ VerificationContext \in JoinedContexts
   /\ IndexedCore(VerificationContext, 7))
    ~> IndexedContextCompleted(VerificationContext)

=============================================================================
