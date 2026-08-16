---- MODULE SumeragiV2ChainEpochRefinementShard10 ----
EXTENDS SumeragiV2ChainEpochRefinementShard09

THEOREM IndexedPostGstResponsiveRosterIsActive ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedChainSpec
      => [](IndexedCore(initialContext, 7)
             => Responsive \subseteq
                  IndexedAsync(initialContext)!AsyncActiveServiceNodes)
BY IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive, PTL
   DEF IndexedPostGstResponsiveActiveRosterCoherence

THEOREM IndexedChainSpecAlwaysJoinsEachPostGstContext ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         [](IndexedCore(initialContext, 7)
              => initialContext \in JoinedContexts)
BY IndexedChainSpecAlwaysKeepsPostGstContextsJoined, PTL
   DEF IndexedPostGstContextJoinedCoherence

THEOREM IndexedChainSpecKeepsServiceActivationRestrictionIrreversible ==
  IndexedChainSpec
    => \A initialContext \in AdmissibleContextRecords:
         [](IndexedAsync(initialContext)!AsyncServiceActivationRestricted
              => []IndexedAsync(initialContext)!
                    AsyncServiceActivationRestricted)
PROOF
  <1>1. ASSUME IndexedChainSpec,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           [](IndexedAsync(initialContext)!AsyncServiceActivationRestricted
                => []IndexedAsync(initialContext)!
                      AsyncServiceActivationRestricted)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant
    <2>2. IndexedAsync(initialContext)!
              AsyncServiceActivationRestricted
            /\ [IndexedChainNext]_IndexedChainVars
            => (IndexedAsync(initialContext)!
                  AsyncServiceActivationRestricted)'
      BY <2>1,
         IndexedStepKeepsServiceActivationRestrictionIrreversible,
         PTL
    <2> QED BY <1>1, <2>2, PTL
         DEF IndexedChainSpec
  <1> QED BY <1>1

THEOREM IndexedInitJoinsEveryNodeThroughGenesis ==
  IndexedChainInit => IndexedJoinedThroughLocalHeight
BY Isa DEF IndexedChainInit, IndexedJoinedThroughLocalHeight,
           CanonicalIndexedContext, JoinedByContextShape,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights,
           Chain!ChainEpochInit, Chain!HistoryThrough,
           Chain!ContextRecord

THEOREM IndexedActionPreservesJoinedThroughLocalHeight ==
  IndexedCompositionInvariant
    /\ IndexedJoinedThroughLocalHeight
    /\ IndexedChainNext
    => IndexedJoinedThroughLocalHeight'
BY Isa DEF IndexedJoinedThroughLocalHeight,
           CanonicalIndexedContext, IndexedChainNext,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           SuccessorContextFor,
           IndexedCompositionInvariant,
           JoinedContextCertificationInvariant,
           JoinedRoutingInvariant,
           JoinedContexts, IndexedNodeCurrentAt,
           Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
           Chain!CertifiedPrefixBacked,
           Chain!NodesDoNotOutrunCertificates,
           Chain!ContextsMatchLocalHistories,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication,
           Chain!CanonicalCommitForSlot, Chain!HistoryThrough,
           Chain!ContextRecord, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt,
           Heights

THEOREM IndexedStepPreservesJoinedThroughLocalHeight ==
  IndexedCompositionInvariant
    /\ IndexedJoinedThroughLocalHeight
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedJoinedThroughLocalHeight'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedJoinedThroughLocalHeight,
              [IndexedChainNext]_IndexedChainVars
         PROVE IndexedJoinedThroughLocalHeight'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1, IndexedActionPreservesJoinedThroughLocalHeight
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedJoinedThroughLocalHeight,
             CanonicalIndexedContext, IndexedChainVars,
             Chain!ChainEpochVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecJoinsEveryNodeThroughLocalHeight ==
  IndexedChainSpec => []IndexedJoinedThroughLocalHeight
PROOF
  <1>1. IndexedChainInit => IndexedJoinedThroughLocalHeight
    BY IndexedInitJoinsEveryNodeThroughGenesis
  <1>2. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>3. IndexedCompositionInvariant
           /\ IndexedJoinedThroughLocalHeight
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedJoinedThroughLocalHeight'
    BY IndexedStepPreservesJoinedThroughLocalHeight
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF IndexedChainSpec

THEOREM IndexedSpecPreservesJoinedRouting ==
  IndexedChainSpec => []JoinedRoutingInvariant
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedSpecPreservesJoinedCertification ==
  IndexedChainSpec => []JoinedContextCertificationInvariant
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM JoinedNonCurrentHasApplicationEvidence ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      (IndexedCompositionInvariant
        /\ ~IndexedNodeCurrentAt(initialContext, node))
        => /\ nodeHeight[node] > initialContext.height
           /\ IndexedAsync(initialContext)!NodeHasApplication(node)
BY DEF IndexedCompositionInvariant, JoinedRoutingInvariant

THEOREM IndexedCurrentNodeHasExactLocation ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedNodeCurrentAt(initialContext, node)
    => ExactNodeLocationAt(initialContext, node)
BY Isa
   DEF IndexedCompositionInvariant, IndexedNodeCurrentAt,
       ExactNodeLocationAt,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories

THEOREM JoinedNonCurrentDisablesExactRunNode ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in joinedByContext[initialContext]:
      (IndexedCompositionInvariant
        /\ ~IndexedNodeCurrentAt(initialContext, node))
        => ~IndexedAsync(initialContext)!RunNode(node)
BY Isa, JoinedNonCurrentHasApplicationEvidence
   DEF IndexedCompositionInvariant,
       IndexedAsync!RunNode, IndexedAsync!AsyncVotersAt

THEOREM ExactHistoricalRecoveryTargetOwnsCurrentLocation ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    IndexedCompositionInvariant
      /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)
BY DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence

(***************************************************************************
Product enabledness is proved, not assumed through hiding. The strong exact
instance invariant types a fresh receipt and supplies per-context agreement;
the receipt projection identifies already certified decisions. Joined-context
certification selects RecordCertifiedNext versus RecordKnownDecision, while
routing and the certified height select RecordAppliedNext versus
RecordKnownApplication. AppliedSuccessorIsAdmissible guarantees that the
queued successor context stays inside the pre-created function domain; the
kind-specific activation publication performs the later join.
***************************************************************************)
THEOREM IndexedReceiptFreeActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedReceiptFreeAsyncAction(initialContext))
      => ENABLED
           (/\ IndexedProductActionAt(initialContext)
            /\ IndexedReceiptFreeAsyncAction(initialContext))
BY Isa DEF IndexedProductActionAt, IndexedReceiptFreeAsyncAction,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedCompositionInvariant, IndexedAsyncStateShape,
           JoinedByContextShape,
           IndexedChainVars

THEOREM IndexedFreshReceiptActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedFreshReceiptAsyncAction(initialContext))
      => ENABLED
           (/\ IndexedProductActionAt(initialContext)
            /\ IndexedFreshReceiptAsyncAction(initialContext))
BY Isa, AppliedSuccessorIsAdmissible
   DEF IndexedFreshReceiptAsyncAction, IndexedProductActionAt,
       IndexedReceiptClassification,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedDecisionEvidence, IndexedApplicationEvidence,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       JoinedContexts, SuccessorContextFor,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!DecisionBacksCertifiedSlot,
       Chain!ReceiptOutsideChainHorizon,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedJoinedActionHasProductExtension ==
  \A initialContext \in AdmissibleContextRecords:
    (IndexedCompositionInvariant
      /\ initialContext \in JoinedContexts
      /\ ENABLED IndexedJoinedAsyncNext(initialContext))
      => ENABLED IndexedProductActionAt(initialContext)
BY Isa, IndexedReceiptFreeActionHasProductExtension,
   IndexedFreshReceiptActionHasProductExtension
   DEF IndexedReceiptFreeAsyncAction,
       IndexedFreshReceiptAsyncAction,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!Next, IndexedAsync!PersistDecision,
       IndexedAsync!ApplyDecision,
       NoNewIndexedDurableReceipt,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt

=============================================================================
