---- MODULE SumeragiV2AsyncNetworkReplyRoutes ----
EXTENDS SumeragiV2AsyncNetwork

(***************************************************************************
Production reply-route composition for `SumeragiV2AsyncNetwork`.

The established asynchronous consensus spec and proof hierarchy retain their
exact `AsyncSpec` boundary.  This module extends it with the process-local
reply owner coupled to the base model's retained Serve source attempts.
Protocol and route steps explicitly interleave; neither component can
manufacture progress for the other's weak fairness.
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

AsyncProductionVars == <<AsyncAllVars, AsyncReplyRouteVars>>

AsyncReplySemanticOf(item) ==
  AsyncReplySemanticIdentity(item.kind, item.source, item.envelope)

AsyncReplyBaseAttemptMatches(
    attempt, owner, source, semantic) ==
  /\ attempt.key.owner = owner
  /\ attempt.key.identity =
       AsyncServeLogicalRequestIdentity(owner, attempt.request)
  /\ attempt.key.identity.owner = owner
  /\ attempt.key.identity.request = semantic
  /\ attempt.key.source = source
  /\ attempt.request.kind \in AsyncReplyRequestKinds
  /\ attempt.request.envelope.recipient = owner
  /\ AsyncReplySemanticOf(attempt.request) = semantic

AsyncReplySemanticObservedIn(attempts, owner, source, semantic) ==
  \E attempt \in attempts:
    AsyncReplyBaseAttemptMatches(
      attempt, owner, source, semantic)

AsyncReplySemanticObserved(owner, source, semantic) ==
  AsyncReplySemanticObservedIn(
    asyncServeAttempts, owner, source, semantic)

AsyncReplyPositiveServeOutputForAttempt(attempt, tombstone) ==
  /\ tombstone.node = attempt.key.owner
  /\ tombstone.identity = attempt.key.identity
  /\ tombstone.ordinal = attempt.lifecycleOrdinal
  /\ tombstone.outcome = AsyncServeResponseOutcome
  /\ tombstone.outputs # {}
  /\ \A output \in tombstone.outputs:
       AsyncServeTombstoneOutputMatchesIdentity(tombstone, output)

AsyncReplySemanticServiceReady(owner, source, semantic) ==
  \E attempt \in asyncServeAttempts,
     tombstone \in asyncServeTombstones:
    /\ AsyncReplyBaseAttemptMatches(
         attempt, owner, source, semantic)
    /\ attempt.stage = "Complete"
    /\ AsyncReplyPositiveServeOutputForAttempt(
         attempt, tombstone)

AsyncReplyServiceReadyPositiveOutputGuard ==
  \A owner \in ValidatorIds,
     source \in AsyncAuthenticatedDeliverySources,
     semantic \in AsyncReplySemanticIdentities:
    AsyncReplySemanticServiceReady(owner, source, semantic)
      => \E attempt \in asyncServeAttempts,
            tombstone \in asyncServeTombstones:
           /\ AsyncReplyBaseAttemptMatches(
                attempt, owner, source, semantic)
           /\ attempt.stage = "Complete"
           /\ AsyncReplyPositiveServeOutputForAttempt(
                attempt, tombstone)

(***************************************************************************
The route machine cannot infer provenance from the signed request sender.
Every live route attempt and durable lifecycle identity must refine one exact
base `(owner, semantic, authenticated source)` attempt and its retained
positive output.  The attempt history survives same-height restart/family
replacement.  Once the route observes an output, the primed product coupling
also prevents a base-only step from revoking that positive output while the
route occurrence is live; close/cancellation must first retire the route
occurrence.  This is the ownership guard used by the imported bare route
fairness, not an added fairness premise.
***************************************************************************)
AsyncReplyRouteToBaseAttemptCoupling ==
  /\ \A attempt \in asyncReplyAttempts:
       AsyncReplySemanticServiceReady(
         attempt.owner, attempt.source, attempt.semantic)
  /\ \A identity \in asyncReplyAttemptLifecycleIdentities:
       AsyncReplySemanticServiceReady(
         identity.owner, identity.source, identity.semantic)

AsyncReplyRouteBaseAttemptCoupling ==
  /\ AsyncReplyRoute!ReplySources =
       AsyncAuthenticatedDeliverySources
  /\ AsyncUntrustedSource \notin AsyncReplyRoute!ReplySources
  /\ AsyncReplyRouteToBaseAttemptCoupling

(***************************************************************************
The two current-state guards below cover every action named by the inherited
ticket/service fairness.  Ticket acquisition requires a live lifecycle
identity; service requires a live route attempt for its selected source.
Under the product coupling either guard already owns the exact positive,
nonempty base output, so the bare kernel action cannot become enabled while
its production wrapper is disabled by the positive-output guard.
***************************************************************************)
AsyncReplyBareAcquirePositiveBaseGuard ==
  \A owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities,
     source \in AsyncAuthenticatedDeliverySources:
    /\ AsyncReplyRouteBaseAttemptCoupling
    /\ AsyncReplyRoute!
         ReplyAttemptLifecycleIdentityOwned(owner, semantic, source)
    => AsyncReplySemanticServiceReady(owner, source, semantic)

AsyncReplyBareServicePositiveBaseGuard ==
  \A owner \in ValidatorIds,
     semantic \in AsyncReplySemanticIdentities,
     source \in AsyncAuthenticatedDeliverySources:
    /\ AsyncReplyRouteBaseAttemptCoupling
    /\ AsyncReplyRoute!ReplyAttemptOwned(owner, semantic, source)
    => AsyncReplySemanticServiceReady(owner, source, semantic)

AsyncObserveNewReplySource(owner, semantic, source) ==
  /\ AsyncReplySemanticServiceReady(owner, source, semantic)
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

AsyncAcquireReplyTicket(owner, semantic, source) ==
  /\ AsyncReplySemanticServiceReady(owner, source, semantic)
  /\ AsyncReplyRoute!AcquireReplyTicketV2(
       owner, semantic, source)

AsyncReplySelectedServiceSource(owner, semantic) ==
  AsyncReplySourceOrder[
    AsyncReplyRoute!ReplySelectedSourceIndex(owner, semantic)]

AsyncServiceReplyRoute(owner, semantic) ==
  LET source == AsyncReplySelectedServiceSource(owner, semantic)
  IN /\ AsyncReplySemanticServiceReady(owner, source, semantic)
     /\ AsyncReplyRoute!ServiceReplyRouteV2(owner, semantic)

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
       AsyncAcquireReplyTicket(owner, semantic, source)
  \/ \E owner \in ValidatorIds,
       semantic \in AsyncReplySemanticIdentities:
       AsyncServiceReplyRoute(owner, semantic)
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
  /\ AsyncReplyRoute!ReplySources =
       AsyncAuthenticatedDeliverySources
  /\ AsyncReplyRouteBaseAttemptCoupling

(***************************************************************************
Do not invent route fairness.  The product retains exactly the ownership
kernel's existing ticket/service/close premises.  First observation and
stream-epoch persistence remain ordinary deterministic local route actions;
their eventual scheduling is an explicit downstream production-refinement
debt, not a hidden network or relay-delivery assumption.
***************************************************************************)
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
deductive source-isolated fairness discharge.  It retains only the same
ticket/service aggregate already present before base-attempt coupling.
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

(***************************************************************************
The product projections are exact: a base step cannot mutate route state and
a route step cannot mutate consensus/scheduler state.  The primed coupling is
not a fairness premise; it is the inductive provenance check that rejects any
route transition which would manufacture an owner/source coordinate.
***************************************************************************)
AsyncProductionAsyncProjectionStep ==
  /\ AsyncNext
  /\ UNCHANGED AsyncReplyRouteVars
  /\ AsyncReplyRouteBaseAttemptCoupling'

AsyncProductionReplyProjectionStep ==
  /\ AsyncReplyRouteNext
  /\ UNCHANGED AsyncAllVars
  /\ AsyncReplyRouteBaseAttemptCoupling'

AsyncProductionNext ==
  \/ AsyncProductionAsyncProjectionStep
  \/ AsyncProductionReplyProjectionStep

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
