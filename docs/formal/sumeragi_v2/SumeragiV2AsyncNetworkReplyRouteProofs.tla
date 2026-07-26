---- MODULE SumeragiV2AsyncNetworkReplyRouteProofs ----
EXTENDS SumeragiV2AsyncNetworkReplyRoutes, TLAPS

(***************************************************************************
Deductive product refinement for production reply routes.

Consensus steps stutter every reply-route variable and route steps stutter
every established asynchronous variable.  The two projections below prevent
the product from borrowing progress across that boundary: production refines
both the existing asynchronous spec and the independently fair route spec.
The final theorem imports the route kernel's deductive ownership/liveness
result through the exact production instantiation.
***************************************************************************)

AsyncReplyRouteProofs ==
  INSTANCE SumeragiV2ReplyRouteOwnershipProofs WITH
    ReplyOwners <- ValidatorIds,
    ReplySourceOrder <- AsyncReplySourceOrder,
    ReplySemantics <- AsyncReplySemanticIdentities,
    ReplyTargets <- ValidatorIds,
    ReplySemanticTarget <- AsyncReplySemanticTarget,
    ReplySourceCapacity <- AsyncReplySourceCapacity,
    ReplyDeliveryOrdinalLimit <- AsyncIngressCapacity,
    ReplyMessageCount <- 2,
    ReplyChunkCount <- AsyncChunkCount,
    rrAttempts <- asyncReplyAttempts,
    rrPayloads <- asyncReplyPayloads,
    rrNextDeliveryOrdinal <- asyncNextReplyDeliveryOrdinal,
    rrConnectionTenure <- asyncReplyConnectionTenure,
    rrSourceActive <- asyncReplySourceActive,
    rrNextServiceIndex <- asyncNextReplyServiceIndex,
    rrSemanticSequence <- asyncReplySemanticSequence,
    rrSemanticHash <- asyncReplySemanticHash,
    rrRequesterNextSequence <- asyncReplyRequesterNextSequence,
    rrRequesterClosedThrough <- asyncReplyRequesterClosedThrough,
    rrClosePendingThrough <- asyncReplyClosePendingThrough,
    rrCloseSentThrough <- asyncReplyCloseSentThrough,
    rrCloseAcknowledgedThrough <- asyncReplyCloseAcknowledgedThrough,
    rrCloseRetryGeneration <- asyncReplyCloseRetryGeneration

THEOREM AsyncReplyRouteInitProvidesSourceGeometry ==
  AsyncReplyRouteInit => AsyncReplyRoute!ReplySources = ValidatorIds
BY Isa
   DEF AsyncReplyRouteInit, AsyncReplyRoute!ReplyRouteInit,
       AsyncReplyRoute!ReplyRouteConfiguration,
       AsyncReplyRoute!ReplySources, AsyncReplySourceOrder,
       ValidatorIds, ModelConfiguration, QuorumConfiguration

THEOREM AsyncReplyRouteNextRefinesOwnershipNext ==
  AsyncReplyRouteNext => AsyncReplyRoute!ReplyRouteNext
BY SMT
   DEF AsyncReplyRouteNext, AsyncObserveNewReplySource,
       AsyncObserveLaterReplyDelivery, AsyncReconnectReplySource,
       AsyncExactReplyCapabilityRetry,
       AsyncReplyRoute!ReplyRouteNext

THEOREM AsyncProductionBracketProjectsAsyncBracket ==
  [AsyncProductionNext]_AsyncProductionVars
    => [AsyncNext]_AsyncAllVars
BY SMT DEF AsyncProductionNext, AsyncProductionVars

THEOREM AsyncProductionBracketProjectsReplyRouteBracket ==
  [AsyncProductionNext]_AsyncProductionVars
    => [AsyncReplyRoute!ReplyRouteNext]_AsyncReplyRouteVars
BY AsyncReplyRouteNextRefinesOwnershipNext, SMT
   DEF AsyncProductionNext, AsyncProductionVars

THEOREM AsyncReplyRouteFairnessProjectsOwnershipFairness ==
  /\ AsyncReplyRouteInit
  /\ AsyncReplyRouteFairness
  => AsyncReplyRoute!ReplyRouteFairness
PROOF
  <1>1. ASSUME AsyncReplyRouteInit,
                AsyncReplyRouteFairness
         PROVE AsyncReplyRoute!ReplyRouteFairness
    <2>1. AsyncReplyRoute!ReplySources = ValidatorIds
      BY <1>1, AsyncReplyRouteInitProvidesSourceGeometry
    <2> QED BY <1>1, <2>1, PTL
         DEF AsyncReplyRouteFairness,
             AsyncReplyRoute!ReplyRouteFairness,
             AsyncReplyRouteVars
  <1> QED BY <1>1

THEOREM AsyncProductionSpecAtProjectsAsyncSpecAt ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext)
      => AsyncSpecAt(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncProductionSpecAt(initialContext)
         PROVE AsyncSpecAt(initialContext)
    <2>1. AsyncInitAt(initialContext)
      BY <1>1 DEF AsyncProductionSpecAt
    <2>2. [][AsyncProductionNext]_AsyncProductionVars
      BY <1>1 DEF AsyncProductionSpecAt
    <2>3. [][AsyncNext]_AsyncAllVars
      BY <2>2, AsyncProductionBracketProjectsAsyncBracket, PTL
    <2>4. AsyncFairnessAt(initialContext)
      BY <1>1 DEF AsyncProductionSpecAt
    <2> QED BY <2>1, <2>3, <2>4 DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncProductionSpecAtProjectsReplyRouteSpec ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext)
      => AsyncReplyRoute!ReplyRouteSpec
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncProductionSpecAt(initialContext)
         PROVE AsyncReplyRoute!ReplyRouteSpec
    <2>1. AsyncReplyRouteInit
      BY <1>1 DEF AsyncProductionSpecAt
    <2>2. [][AsyncProductionNext]_AsyncProductionVars
      BY <1>1 DEF AsyncProductionSpecAt
    <2>3. [][AsyncReplyRoute!ReplyRouteNext]_AsyncReplyRouteVars
      BY <2>2,
         AsyncProductionBracketProjectsReplyRouteBracket, PTL
    <2>4. AsyncReplyRouteFairness
      BY <1>1 DEF AsyncProductionSpecAt
    <2>5. AsyncReplyRoute!ReplyRouteFairness
      BY <2>1, <2>4,
         AsyncReplyRouteFairnessProjectsOwnershipFairness
    <2> QED BY <2>1, <2>3, <2>5
         DEF AsyncReplyRouteInit,
             AsyncReplyRoute!ReplyRouteSpec,
             AsyncReplyRouteVars
  <1> QED BY <1>1

THEOREM AsyncProductionSpecProjectsAsyncSpec ==
  AsyncProductionSpec => AsyncSpec
BY AsyncProductionSpecAtProjectsAsyncSpecAt
   DEF AsyncProductionSpec, AsyncProductionSpecAt,
       AsyncSpec, AsyncSpecAt, AsyncInit, AsyncFairness

THEOREM AsyncProductionSpecProjectsReplyRouteSpec ==
  AsyncProductionSpec => AsyncReplyRoute!ReplyRouteSpec
BY AsyncProductionSpecAtProjectsReplyRouteSpec
   DEF AsyncProductionSpec, AsyncProductionSpecAt,
       AsyncInit, AsyncFairness

THEOREM AsyncProductionSpecAtProvidesReplyRouteProgress ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext) =>
      /\ []AsyncReplyRouteSafetyInvariant
      /\ []AsyncReplyRouteFullSafetyInvariant
      /\ AsyncReplyTenureAwareReplay
      /\ AsyncReplySourceIsolation
      /\ AsyncReplyLifecycleJournal
      /\ \A requester \in ValidatorIds,
            responder \in ValidatorIds:
           AsyncReplyCloseWorkEventuallyTerminates(
             requester, responder)
      /\ \A owner \in ValidatorIds,
            semantic \in AsyncReplySemanticIdentities,
            source \in ValidatorIds:
           AsyncReplySourceEventuallyProgresses(
             owner, semantic, source)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncProductionSpecAt(initialContext)
         PROVE /\ []AsyncReplyRouteSafetyInvariant
               /\ []AsyncReplyRouteFullSafetyInvariant
               /\ AsyncReplyTenureAwareReplay
               /\ AsyncReplySourceIsolation
               /\ AsyncReplyLifecycleJournal
               /\ \A requester \in ValidatorIds,
                     responder \in ValidatorIds:
                    AsyncReplyCloseWorkEventuallyTerminates(
                      requester, responder)
               /\ \A owner \in ValidatorIds,
                     semantic \in AsyncReplySemanticIdentities,
                     source \in ValidatorIds:
                    AsyncReplySourceEventuallyProgresses(
                      owner, semantic, source)
    <2>1. AsyncReplyRoute!ReplyRouteSpec
      BY <1>1, AsyncProductionSpecAtProjectsReplyRouteSpec
    <2>2. /\ []AsyncReplyRoute!ReplyRouteSafetyInvariant
           /\ []AsyncReplyRoute!ReplyRouteFullSafetyInvariant
           /\ AsyncReplyRoute!ReplyTenureAwareReplay
           /\ AsyncReplyRoute!ReplySourceIsolation
           /\ AsyncReplyRoute!ReplyLifecycleJournal
           /\ \A requester \in ValidatorIds,
                 responder \in AsyncReplyRoute!ReplySources:
                AsyncReplyRoute!ReplyCloseWorkEventuallyTerminates(
                  requester, responder)
           /\ \A owner \in ValidatorIds,
                 semantic \in AsyncReplySemanticIdentities,
                 source \in AsyncReplyRoute!ReplySources:
                AsyncReplyRoute!ReplySourceEventuallyProgresses(
                  owner, semantic, source)
      BY <2>1,
         AsyncReplyRouteProofs!ReplyRouteOwnershipModelObligation
    <2>3. AsyncReplyRoute!ReplySources = ValidatorIds
      BY <1>1, AsyncReplyRouteInitProvidesSourceGeometry
         DEF AsyncProductionSpecAt
    <2> QED BY <2>2, <2>3
         DEF AsyncReplyRouteSafetyInvariant,
             AsyncReplyRouteFullSafetyInvariant,
             AsyncReplyTenureAwareReplay,
             AsyncReplySourceIsolation,
             AsyncReplyLifecycleJournal,
             AsyncReplyCloseWorkEventuallyTerminates,
             AsyncReplySourceEventuallyProgresses
  <1> QED BY <1>1

THEOREM AsyncNetworkReplyRouteModelObligation ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext) =>
      /\ AsyncSpecAt(initialContext)
      /\ AsyncReplyRoute!ReplyRouteSpec
      /\ []AsyncReplyRouteSafetyInvariant
      /\ []AsyncReplyRouteFullSafetyInvariant
      /\ AsyncReplyTenureAwareReplay
      /\ AsyncReplySourceIsolation
      /\ AsyncReplyLifecycleJournal
      /\ \A requester \in ValidatorIds,
            responder \in ValidatorIds:
           AsyncReplyCloseWorkEventuallyTerminates(
             requester, responder)
      /\ \A owner \in ValidatorIds,
            semantic \in AsyncReplySemanticIdentities,
            source \in ValidatorIds:
           AsyncReplySourceEventuallyProgresses(
             owner, semantic, source)
BY AsyncProductionSpecAtProjectsAsyncSpecAt,
   AsyncProductionSpecAtProjectsReplyRouteSpec,
   AsyncProductionSpecAtProvidesReplyRouteProgress

=============================================================================
