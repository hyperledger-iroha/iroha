---- MODULE SumeragiV2AsyncNetworkReplyRoutes ----
EXTENDS SumeragiV2AsyncNetwork

(***************************************************************************
Production reply-route composition for `SumeragiV2AsyncNetwork`.

The established asynchronous consensus spec and proof hierarchy retain their
exact `AsyncSpec` boundary.  This module extends it with the independent
process-local reply owner.  Protocol and route steps explicitly interleave;
neither component can manufacture progress for the other's weak fairness.
Exact retries and later same-tenure deliveries preserve reply progress.  A new
connection tenure invalidates the old ticket while preserving the affected
attempt's current cursor and every independent source attempt.
***************************************************************************)

VARIABLES
  asyncReplyAttempts,
  asyncReplyPayloads,
  asyncNextReplyDeliveryOrdinal,
  asyncReplyConnectionTenure,
  asyncReplySourceActive,
  asyncNextReplyServiceIndex

AsyncReplyRoute ==
  INSTANCE SumeragiV2ReplyRouteOwnership WITH
    ReplyOwners <- ValidatorIds,
    ReplySourceOrder <- AsyncReplySourceOrder,
    ReplySemantics <- AsyncReplySemanticIdentities,
    ReplySourceCapacity <- AsyncReplySourceCapacity,
    ReplyDeliveryOrdinalLimit <- AsyncIngressCapacity,
    ReplyMessageCount <- 2,
    ReplyChunkCount <- AsyncChunkCount,
    rrAttempts <- asyncReplyAttempts,
    rrPayloads <- asyncReplyPayloads,
    rrNextDeliveryOrdinal <- asyncNextReplyDeliveryOrdinal,
    rrConnectionTenure <- asyncReplyConnectionTenure,
    rrSourceActive <- asyncReplySourceActive,
    rrNextServiceIndex <- asyncNextReplyServiceIndex

AsyncReplyRouteVars ==
  <<asyncReplyAttempts, asyncReplyPayloads,
    asyncNextReplyDeliveryOrdinal, asyncReplyConnectionTenure,
    asyncReplySourceActive, asyncNextReplyServiceIndex>>

AsyncReplySemanticOf(item) ==
  AsyncReplySemanticIdentity(item.kind, item.envelope)

AsyncReplySemanticObserved(owner, source, semantic) ==
  \E item \in asyncSentItems:
    /\ item.kind \in AsyncReplyRequestKinds
    /\ item.envelope.recipient = owner
    /\ item.source = source
    /\ AsyncReplySemanticOf(item) = semantic

AsyncObserveNewReplySource(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!ObserveNewReplySource(owner, semantic, source)

AsyncObserveLaterReplyDelivery(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!ObserveLaterReplyDelivery(owner, semantic, source)

AsyncReconnectReplySource(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!ReconnectReplySource(owner, semantic, source)

(***************************************************************************
An exact capability retry is a route-state stutter.  In particular, it cannot
move either cursor backward or reset the source's round-robin age.
***************************************************************************)
AsyncExactReplyCapabilityRetry(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!RetryExactReplySource(owner, semantic, source)

AsyncReplyRouteNext ==
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       AsyncObserveNewReplySource(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       AsyncObserveLaterReplyDelivery(owner, semantic, source)
  \/ \E owner \in ValidatorIds, source \in ValidatorIds:
       AsyncReplyRoute!RetireReplySource(owner, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       AsyncReconnectReplySource(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       AsyncReplyRoute!AcquireReplyTicket(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities:
       AsyncReplyRoute!ServiceReplyRoute(owner, semantic)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       AsyncExactReplyCapabilityRetry(owner, semantic, source)

AsyncReplyRouteInit == AsyncReplyRoute!ReplyRouteInit

AsyncReplyRouteFairness ==
  /\ \A owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in ValidatorIds:
       WF_AsyncReplyRouteVars(
         AsyncReplyRoute!AcquireReplyTicket(owner, semantic, source))
  /\ \A owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities:
       WF_AsyncReplyRouteVars(
         AsyncReplyRoute!ServiceReplyRoute(owner, semantic))

AsyncAcquireAnyReplyTicket ==
  \E owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities,
     source \in ValidatorIds:
    AsyncReplyRoute!AcquireReplyTicket(owner, semantic, source)

AsyncServiceAnyReplyRoute ==
  \E owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities:
    AsyncReplyRoute!ServiceReplyRoute(owner, semantic)

(***************************************************************************
The finite TLC configuration checks safety and counterexamples, not a
deductive source-isolated fairness discharge.  Its aggregate clauses avoid a
temporal tableau branch for every member of the finite payload universe.  The
unbounded production spec uses `AsyncReplyRouteFairness` above; the shared
mutation model checks the actual per-semantic round-robin transition.
***************************************************************************)
AsyncFiniteReplyRouteFairness ==
  /\ WF_AsyncReplyRouteVars(AsyncAcquireAnyReplyTicket)
  /\ WF_AsyncReplyRouteVars(AsyncServiceAnyReplyRoute)

AsyncReplyRouteTypeInvariant == AsyncReplyRoute!ReplyRouteTypeInvariant
AsyncReplyRouteOwnershipInvariant ==
  AsyncReplyRoute!ReplyRouteOwnershipInvariant
AsyncReplyRouteSafetyInvariant ==
  AsyncReplyRoute!ReplyRouteSafetyInvariant
AsyncReplyTenureAwareReplayStep ==
  AsyncReplyRoute!ReplyTenureAwareReplayStep
AsyncReplyTenureAwareReplay ==
  AsyncReplyRoute!ReplyTenureAwareReplay
AsyncReplySourceIsolationStep ==
  AsyncReplyRoute!ReplySourceIsolationStep
AsyncReplySourceIsolation ==
  AsyncReplyRoute!ReplySourceIsolation
AsyncReplySourceStableResponsive(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceStableResponsive(owner, semantic, source)
AsyncReplySourceEventuallyProgresses(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceEventuallyProgresses(owner, semantic, source)

AsyncProductionVars == <<AsyncAllVars, AsyncReplyRouteVars>>

AsyncProductionNext ==
  \/ /\ AsyncNext
     /\ UNCHANGED AsyncReplyRouteVars
  \/ /\ AsyncReplyRouteNext
     /\ UNCHANGED AsyncAllVars

AsyncProductionSpec ==
  AsyncInit
    /\ AsyncReplyRouteInit
    /\ [][AsyncProductionNext]_AsyncProductionVars
    /\ AsyncFairness
    /\ AsyncReplyRouteFairness

AsyncProductionSpecAt(initialContext) ==
  AsyncInitAt(initialContext)
    /\ AsyncReplyRouteInit
    /\ [][AsyncProductionNext]_AsyncProductionVars
    /\ AsyncFairnessAt(initialContext)
    /\ AsyncReplyRouteFairness

AsyncFiniteProductionSpec ==
  AsyncFiniteInit
    /\ AsyncReplyRouteInit
    /\ [][AsyncProductionNext]_AsyncProductionVars
    /\ AsyncFairness
    /\ AsyncFiniteReplyRouteFairness

AsyncFiniteProductionSpecAt(initialContext) ==
  AsyncFiniteInitAt(initialContext)
    /\ AsyncReplyRouteInit
    /\ [][AsyncProductionNext]_AsyncProductionVars
    /\ AsyncFairnessAt(initialContext)
    /\ AsyncFiniteReplyRouteFairness

=============================================================================
