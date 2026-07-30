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

`AsyncReplySemanticTarget` is the production refinement's semantic-origin
projection for a canonical request identity.  It is deliberately independent
of the authenticated delivery source: a hub occupies the source lane while the
reply remains addressed to the request origin.
***************************************************************************)

CONSTANT AsyncReplySemanticTarget(_)

VARIABLES
  asyncReplyAttempts,
  asyncReplyPayloads,
  asyncNextReplyDeliveryOrdinal,
  asyncReplyConnectionTenure,
  asyncReplySourceActive,
  asyncNextReplyServiceIndex,
  asyncReplySemanticSequence,
  asyncReplySemanticHash,
  asyncReplyRequesterNextSequence,
  asyncReplyRequesterClosedThrough,
  asyncReplyClosePendingThrough,
  asyncReplyCloseSentThrough,
  asyncReplyCloseAcknowledgedThrough,
  asyncReplyCloseRetryGeneration,
  asyncReplyServiceGeneration,
  asyncReplyResponderGeneration,
  asyncReplyDurableResponderGeneration,
  asyncReplyRequesterNextStreamEpoch,
  asyncReplyRequesterStreamEpoch,
  asyncReplyCloseStreamEpoch,
  asyncReplyClosedPrefix,
  asyncReplyAttemptLifecycleIdentities,
  asyncReplyPendingHintResets,
  asyncReplyDiscardedPartialIdentities

AsyncReplyRoute ==
  INSTANCE SumeragiV2ReplyRouteOwnership WITH
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
    rrCloseRetryGeneration <- asyncReplyCloseRetryGeneration,
    rrServiceGeneration <- asyncReplyServiceGeneration,
    rrResponderGeneration <- asyncReplyResponderGeneration,
    rrDurableResponderGeneration <-
      asyncReplyDurableResponderGeneration,
    rrRequesterNextStreamEpoch <-
      asyncReplyRequesterNextStreamEpoch,
    rrRequesterStreamEpoch <- asyncReplyRequesterStreamEpoch,
    rrCloseStreamEpoch <- asyncReplyCloseStreamEpoch,
    rrClosedPrefix <- asyncReplyClosedPrefix,
    rrAttemptLifecycleIdentities <-
      asyncReplyAttemptLifecycleIdentities,
    rrPendingHintResets <- asyncReplyPendingHintResets,
    rrDiscardedPartialIdentities <-
      asyncReplyDiscardedPartialIdentities

AsyncReplyRouteVars ==
  AsyncReplyRoute!ReplyRouteV2Vars

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
  /\ AsyncReplyRoute!ObserveNewReplySourceV2(owner, semantic, source)

AsyncObserveLaterReplyDelivery(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!ObserveLaterReplyDeliveryV2(
       owner, semantic, source)

AsyncReconnectReplySource(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!ReconnectReplySourceV2(owner, semantic, source)

(***************************************************************************
An exact capability retry is a route-state stutter.  In particular, it cannot
move either cursor backward or reset the source's round-robin age.
***************************************************************************)
AsyncExactReplyCapabilityRetry(owner, semantic, source) ==
  /\ AsyncReplySemanticObserved(owner, source, semantic)
  /\ AsyncReplyRoute!RetryExactReplySourceV2(
       owner, semantic, source)

AsyncReplyRouteNext ==
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in AsyncReplyRoute!ReplySources:
       AsyncObserveNewReplySource(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in AsyncReplyRoute!ReplySources:
       AsyncObserveLaterReplyDelivery(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!RetireReplySourceV2(owner, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in AsyncReplyRoute!ReplySources:
       AsyncReconnectReplySource(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!AcquireReplyTicketV2(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities:
       AsyncReplyRoute!ServiceReplyRouteV2(owner, semantic)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities,
       source \in AsyncReplyRoute!ReplySources:
       AsyncExactReplyCapabilityRetry(owner, semantic, source)
  \/ \E witness \in AsyncReplyRoute!ReplyCloseWitnessSet:
       AsyncReplyRoute!CloseSemanticRequestV2(witness)
  \/ \E witness \in AsyncReplyRoute!ReplyCloseWitnessSet:
       AsyncReplyRoute!PiggybackCloseSemanticRequestV2(witness)
  \/ \E witness \in AsyncReplyRoute!ReplyCloseWitnessSet:
       AsyncReplyRoute!RetryCloseSemanticRequestV2(witness)
  \/ \E acknowledgement
       \in AsyncReplyRoute!ReplyCloseAcknowledgementSet:
       AsyncReplyRoute!AcknowledgeCloseSemanticRequestV2(
         acknowledgement)
  \/ \E owner \in ValidatorIds,
       source \in AsyncReplyRoute!ReplySources:
       /\ AsyncReplyRoute!RecoverReplyRouteState(owner, source)
       /\ UNCHANGED AsyncReplyRoute!ReplyCoordinateVars
  \/ \E owner \in ValidatorIds,
       source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!PersistFreshRequesterStreamEpoch(owner, source)
  \/ \E hint \in AsyncReplyRoute!ReplyGenerationHintSet:
       AsyncReplyRoute!PersistFreshEpochForGenerationHint(hint)
  \/ \E reset \in AsyncReplyRoute!ReplyHintResetSet:
       AsyncReplyRoute!DiscardPersistedHintPartialState(reset)
  \/ \E source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!PersistTerminalResponderGeneration(source)
  \/ \E source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!InstallPersistedResponderGeneration(source)
  \/ \E requester \in ValidatorIds,
       responder \in AsyncReplyRoute!ReplySources,
       inputGeneration \in AsyncReplyRoute!ReplyServiceGenerations:
       AsyncReplyRoute!RejectFutureGenerationWithoutMutation(
         requester, responder, inputGeneration)
  \/ \E requester \in ValidatorIds:
       AsyncReplyRoute!
         RejectRequesterEpochOverflowWithoutMutation(requester)
  \/ \E source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!
         RejectNonTerminalResponderCompactionWithoutMutation(source)
  \/ \E requester \in ValidatorIds,
       responder \in AsyncReplyRoute!ReplySources,
       observedMessageHash
         \in SUBSET
              (AsyncReplyRoute!ReplyRequestIdentitySet
                 \cup AsyncReplyRoute!ReplyCloseIdentitySet):
       AsyncReplyRoute!ReturnOlderGenerationHintWithoutRoute(
         requester, responder, observedMessageHash)
  \/ \E source \in AsyncReplyRoute!ReplySources:
       AsyncReplyRoute!RejectResponderGenerationOverflow(source)

AsyncReplyRouteInit ==
  /\ AsyncReplyRoute!ReplyRouteV2Init
  /\ AsyncReplyRoute!ReplySources = ValidatorIds

(*
Keep the production premise definitionally identical to the V2 ownership
kernel.  A copied subset can silently omit a terminal-close action or use the
legacy state vector, allowing coordinate-only steps to masquerade as service.
The premise remains local scheduling only; it does not assert network
responsiveness.
*)
AsyncReplyRouteFairness ==
  AsyncReplyRoute!ReplyRouteV2Fairness

AsyncAcquireAnyReplyTicket ==
  \E owner \in ValidatorIds,
    semantic \in AsyncReplySemanticIdentities,
    source \in AsyncReplyRoute!ReplySources:
    AsyncReplyRoute!AcquireReplyTicketV2(owner, semantic, source)

AsyncServiceAnyReplyRoute ==
  \E owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities:
    AsyncReplyRoute!ServiceReplyRouteV2(owner, semantic)

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
AsyncReplyRouteFullSafetyInvariant ==
  AsyncReplyRoute!ReplyRouteFullSafetyInvariant
AsyncReplyRouteV2SafetyInvariant ==
  AsyncReplyRoute!ReplyRouteV2SafetyInvariant
AsyncReplyTenureAwareReplayStep ==
  AsyncReplyRoute!ReplyTenureAwareReplayStep
AsyncReplyTenureAwareReplay ==
  AsyncReplyRoute!ReplyTenureAwareReplay
AsyncReplySourceIsolationStep ==
  AsyncReplyRoute!ReplySourceIsolationStep
AsyncReplySourceIsolation ==
  AsyncReplyRoute!ReplySourceIsolation
AsyncReplySourceRouteStable(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceRouteStable(owner, semantic, source)
AsyncReplySourceStableResponsive(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceStableResponsive(owner, semantic, source)
AsyncReplySourceIndex(source) ==
  AsyncReplyRoute!ReplySourceIndex(source)
AsyncReplySourceRoundRobinRank(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceRoundRobinRank(owner, semantic, source)
AsyncReplySourceEventuallyProgresses(owner, semantic, source) ==
  AsyncReplyRoute!ReplySourceEventuallyProgresses(owner, semantic, source)
AsyncReplyLifecycleJournal ==
  AsyncReplyRoute!ReplyLifecycleJournal
AsyncReplyCloseWorkEventuallyTerminates(requester, responder) ==
  AsyncReplyRoute!ReplyCloseWorkEventuallyTerminates(requester, responder)

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
