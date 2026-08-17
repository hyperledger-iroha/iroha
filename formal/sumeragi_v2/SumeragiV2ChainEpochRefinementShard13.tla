---- MODULE SumeragiV2ChainEpochRefinementShard13 ----
EXTENDS SumeragiV2ChainEpochRefinementShard12

THEOREM IndexedAllResponsiveAppliedIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedAsync(initialContext)!
      AsyncAllResponsiveAppliedAt(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => (IndexedAsync(initialContext)!
            AsyncAllResponsiveAppliedAt(initialContext))'
BY Isa DEF IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt, IndexedApplications,
           IndexedAsync!AsyncAllResponsiveAppliedAt,
           IndexedAsync!AsyncVotersAt,
           IndexedAsync!NodeHasApplication

THEOREM VerificationFrontierActivatedInstanceEventuallyApplies ==
  /\ IndexedLiveChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    /\ []~JoinedCanonicalDescendant(VerificationContext)
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> IndexedAsync(VerificationContext)!
               AsyncAllResponsiveAppliedAt(VerificationContext)
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7)),
              []~JoinedCanonicalDescendant(VerificationContext)
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> IndexedAsync(VerificationContext)!
                       AsyncAllResponsiveAppliedAt(VerificationContext)
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedAllResponsiveJoined(VerificationContext)
             /\ [IndexedChainNext]_IndexedChainVars
             => IndexedAllResponsiveJoined(VerificationContext)'
      BY <1>1, IndexedAllResponsiveJoinedIsStable
    <2>2. <>IndexedAllResponsiveJoined(VerificationContext)
             => (TRUE ~> IndexedAllResponsiveJoined(VerificationContext))
      BY <2>0, <2>1, PTL DEF IndexedChainSpec
    <2>3. (/\ IndexedLiveChainSpec
            /\ TRUE ~> IndexedAllResponsiveJoined(VerificationContext)
            /\ []~JoinedCanonicalDescendant(VerificationContext))
             => IndexedAsync(VerificationContext)!
                  AsyncLiveSpecAt(VerificationContext)
      BY <1>1, IndexedLiveInstanceActivationObligation
    <2>4. IndexedAsync(VerificationContext)!
             AsyncLiveSpecAt(VerificationContext)
             => <>IndexedCore(VerificationContext, 7)
      BY <1>1
    <2>5. VerificationOneHeightCompletion
      BY <1>1
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, PTL
  <1> QED BY <1>1

VerificationFrontierEscape ==
  \/ JoinedCanonicalDescendant(VerificationContext)
  \/ IndexedAsync(VerificationContext)!
       AsyncAllResponsiveAppliedAt(VerificationContext)

THEOREM VerificationFrontierEscapeIsStable ==
  IndexedCompositionInvariant
    /\ VerificationContext \in AdmissibleContextRecords
    /\ VerificationFrontierEscape
    /\ [IndexedChainNext]_IndexedChainVars
    => VerificationFrontierEscape'
BY Isa, JoinedCanonicalDescendantIsStable,
   IndexedAllResponsiveAppliedIsStable
   DEF VerificationFrontierEscape

THEOREM VerificationActivatedFrontierEventuallyEscapes ==
  /\ IndexedLiveChainSpec
    /\ VerificationOneHeightCompletion
    /\ VerificationContext \in AdmissibleContextRecords
    /\ (IndexedAsync(VerificationContext)!
          AsyncLiveSpecAt(VerificationContext)
          => <>IndexedCore(VerificationContext, 7))
    => IndexedAllResponsiveJoined(VerificationContext)
         ~> VerificationFrontierEscape
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              VerificationOneHeightCompletion,
              VerificationContext \in AdmissibleContextRecords,
              (IndexedAsync(VerificationContext)!
                 AsyncLiveSpecAt(VerificationContext)
                 => <>IndexedCore(VerificationContext, 7))
         PROVE IndexedAllResponsiveJoined(VerificationContext)
                 ~> VerificationFrontierEscape
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. IndexedCompositionInvariant
             /\ VerificationFrontierEscape
             /\ [IndexedChainNext]_IndexedChainVars
             => VerificationFrontierEscape'
      BY <1>1, VerificationFrontierEscapeIsStable
    <2>3. <>JoinedCanonicalDescendant(VerificationContext)
             => (IndexedAllResponsiveJoined(VerificationContext)
                   ~> VerificationFrontierEscape)
      BY <2>0, <2>1, <2>2, PTL
         DEF VerificationFrontierEscape, IndexedChainSpec
    <2>4. []~JoinedCanonicalDescendant(VerificationContext)
             => (IndexedAllResponsiveJoined(VerificationContext)
                   ~> VerificationFrontierEscape)
      BY <1>1,
         VerificationFrontierActivatedInstanceEventuallyApplies,
         PTL DEF VerificationFrontierEscape
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM IndexedDecisionEvidenceMemberClassification ==
  \A decision:
    decision \in IndexedDecisionEvidence
      => \E sourceContext \in AdmissibleContextRecords:
           decision \in IndexedCurrentDecisions(sourceContext)
BY Isa DEF IndexedDecisionEvidence

THEOREM IndexedCurrentCanonicalDecisionIdentifiesContext ==
  \A initialContext \in AdmissibleContextRecords,
     sourceContext \in AdmissibleContextRecords,
     decision \in Chain!DecisionEvidenceSet:
    (/\ JoinedContextCertificationInvariant
     /\ initialContext \in JoinedContexts
     /\ decision \in IndexedCurrentDecisions(sourceContext)
     /\ Chain!CanonicalCommitForSlot(
          decision.qc, initialContext.height + 1))
      => sourceContext = initialContext
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW sourceContext \in AdmissibleContextRecords,
              NEW decision \in Chain!DecisionEvidenceSet,
              JoinedContextCertificationInvariant,
              initialContext \in JoinedContexts,
              decision \in IndexedCurrentDecisions(sourceContext),
              Chain!CanonicalCommitForSlot(
                decision.qc, initialContext.height + 1)
         PROVE sourceContext = initialContext
    <2>1. initialContext \in ContextRecords
      BY <1>1 DEF AdmissibleContextRecords
    <2>2. initialContext.height \in Heights
      BY <2>1, ContextRecordHeightTyped
    <2>3. (initialContext.height + 1) - 1 = initialContext.height
      BY <2>2, Isa DEF Heights
    <2>4. initialContext =
             Chain!ContextRecord(
               initialContext.height,
               Chain!HistoryThrough(initialContext.height))
      BY <1>1 DEF JoinedContextCertificationInvariant
    <2>5. decision.qc.context =
             Chain!ContextRecord(
               (initialContext.height + 1) - 1,
               Chain!HistoryThrough((initialContext.height + 1) - 1))
      BY <1>1 DEF Chain!CanonicalCommitForSlot
    <2>6. Chain!ContextRecord(
             (initialContext.height + 1) - 1,
             Chain!HistoryThrough((initialContext.height + 1) - 1))
           = Chain!ContextRecord(
               initialContext.height,
               Chain!HistoryThrough(initialContext.height))
      BY <2>3
    <2>7. decision.qc.context = initialContext
      BY <2>4, <2>5, <2>6
    <2> QED BY <1>1, <2>7, Isa DEF IndexedCurrentDecisions
  <1> QED BY <1>1

THEOREM JoinedCanonicalDescendantStaysWithinHorizon ==
  \A initialContext \in AdmissibleContextRecords:
    JoinedCanonicalDescendant(initialContext)
      => initialContext.height < MaxHeight
BY Isa DEF JoinedCanonicalDescendant, JoinedContexts,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

THEOREM JoinedCanonicalDescendantBoundsImmediateSuccessor ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ JoinedCanonicalDescendant(initialContext)
      => /\ initialContext.height < MaxHeight
         /\ initialContext.height < certifiedHeight
         /\ initialContext.height + 1 \in 1..certifiedHeight
BY Isa DEF JoinedCanonicalDescendant,
           IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           JoinedContexts, CanonicalIndexedContext,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!ContextRecord, Chain!HistoryThrough,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

IndexedResponsiveLagAt(initialContext, node) ==
  /\ initialContext.height < MaxHeight
  /\ node \in Responsive
  /\ nodeHeight[node] = initialContext.height

THEOREM IndexedHistoricalRecoveryAdvancesResponsiveNode ==
  IndexedExactHistoricalRecoveryProgress
  =>
    \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
      initialContext.height < MaxHeight
        => HistoricalRecoveryOutstanding(initialContext, node)
             ~> nodeHeight[node] > initialContext.height
BY PTL DEF IndexedExactHistoricalRecoveryProgress,
           HistoricalRecoveryComplete

IndexedResponsiveHeightReached(blockHeight) ==
  \A node \in Responsive: nodeHeight[node] >= blockHeight

IndexedNodePastContext(initialContext, node) ==
  nodeHeight[node] > initialContext.height

IndexedContextAdvanceReady(initialContext) ==
  /\ initialContext \in AdmissibleContextRecords
  /\ initialContext \in JoinedContexts
  /\ JoinedCanonicalDescendant(initialContext)
  /\ IndexedResponsiveHeightReached(initialContext.height)

IndexedResponsivePrefixPast(initialContext, limit) ==
  \A node \in Responsive \cap (0..limit):
    IndexedNodePastContext(initialContext, node)

THEOREM IndexedNodePastContextIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedNodePastContext(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedNodePastContext(initialContext, node)'
BY IndexedBracketStepKeepsNodeHeightsMonotone, SMT
   DEF IndexedNodePastContext, Heights, AdmissibleContextRecords,
       FrozenContextAdmissible, ContextRecords

THEOREM IndexedResponsivePrefixPastIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     limit \in Nat:
    IndexedResponsivePrefixPast(initialContext, limit)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsivePrefixPast(initialContext, limit)'
BY Isa, IndexedNodePastContextIsStable
   DEF IndexedResponsivePrefixPast

IndexedAncestorContext(targetContext, blockHeight) ==
  ContextRecord(
    blockHeight,
    [index \in 1..blockHeight |-> targetContext.lineage[index]])

IndexedTargetJoined(targetContext) ==
  targetContext \in JoinedContexts

THEOREM IndexedAdmissibleTargetHasAdmissibleAncestors ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedAncestorContext(targetContext, blockHeight)
        \in AdmissibleContextRecords
BY Isa DEF IndexedAncestorContext, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Heights, ContextRecord

THEOREM IndexedTargetJoinedIsStable ==
  \A targetContext \in AdmissibleContextRecords:
    IndexedTargetJoined(targetContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedTargetJoined(targetContext)'
BY Isa, JoinedMembershipIsMonotone
   DEF IndexedTargetJoined, JoinedContexts, IndexedChainVars

THEOREM IndexedResponsiveHeightReachedIsStable ==
  \A blockHeight \in Heights:
    IndexedResponsiveHeightReached(blockHeight)
      /\ [IndexedChainNext]_IndexedChainVars
      => IndexedResponsiveHeightReached(blockHeight)'
BY Isa, IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedResponsiveHeightReached, ModelConfiguration,
       ValidatorIds

THEOREM IndexedJoinedTargetIdentifiesEveryCanonicalAncestor ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedTargetJoined(targetContext)
        => /\ IndexedAncestorContext(targetContext, blockHeight)
                 \in AdmissibleContextRecords
           /\ IndexedAncestorContext(targetContext, blockHeight)
                 = CanonicalIndexedContext(blockHeight)
BY Isa DEF IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           IndexedTargetJoined, JoinedContexts,
           IndexedAncestorContext, CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights,
           Chain!HistoryThrough, Chain!ContextRecord

IndexedActivationPendingIntoContext(initialContext, node) ==
  IF initialContext.height = 0
  THEN FALSE
  ELSE /\ initialContext =
            CanonicalIndexedContext(initialContext.height)
       /\ IndexedSuccessorActivationPending(
            CanonicalIndexedContext(initialContext.height - 1), node)

THEOREM IndexedReachedAncestorClassifiesEveryResponsiveNode ==
  \A targetContext \in AdmissibleContextRecords:
    \A blockHeight \in 0..targetContext.height:
      IndexedCompositionInvariant
        /\ IndexedJoinedThroughLocalHeight
        /\ IndexedTargetJoined(targetContext)
        /\ IndexedResponsiveHeightReached(blockHeight)
        => \A node \in Responsive:
             \/ node \in joinedByContext[
                  IndexedAncestorContext(targetContext, blockHeight)]
             \/ IndexedActivationPendingIntoContext(
                  IndexedAncestorContext(targetContext, blockHeight), node)
BY Isa, IndexedJoinedTargetIdentifiesEveryCanonicalAncestor
   DEF IndexedJoinedThroughLocalHeight,
       IndexedResponsiveHeightReached,
       IndexedActivationPendingIntoContext,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       ModelConfiguration, ValidatorIds, Heights

=============================================================================
