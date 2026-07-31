---- MODULE SumeragiV2AsyncNetworkReplyRouteProofs ----
EXTENDS SumeragiV2AsyncNetworkReplyRoutes, TLAPS

(***************************************************************************
Safety projection for the asynchronous product and the V2 reply lifecycle.

The proved clauses below establish only structural projection: consensus
steps stutter every V2 reply variable and reply steps stutter every consensus
variable.  The temporal safety/progress products are named specified-unproved
obligations at the end.  No rotating-leader or network delivery liveness is
derived from local reply-route fairness.
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

THEOREM AsyncReplyRouteInitContainsExactV2Init ==
  AsyncReplyRouteInit => AsyncReplyRoute!ReplyRouteV2Init
BY DEF AsyncReplyRouteInit

THEOREM AsyncProductionStepIsDisjointProduct ==
  AsyncProductionNext <=>
    \/ /\ AsyncNext
       /\ UNCHANGED AsyncReplyRouteVars
       /\ AsyncReplyRouteBaseAttemptCoupling'
    \/ /\ AsyncReplyRouteNext
       /\ UNCHANGED AsyncAllVars
       /\ AsyncReplyRouteBaseAttemptCoupling'
BY DEF AsyncProductionNext

THEOREM AsyncConsensusProductBranchStuttersReplyLifecycle ==
  /\ AsyncNext
  /\ UNCHANGED AsyncReplyRouteVars
  => UNCHANGED AsyncReplyRouteVars
OBVIOUS

THEOREM AsyncReplyProductBranchStuttersConsensus ==
  /\ AsyncReplyRouteNext
  /\ UNCHANGED AsyncAllVars
  => UNCHANGED AsyncAllVars
OBVIOUS

THEOREM AsyncReplyRouteFairnessIsExactV2Fairness ==
  AsyncReplyRouteFairness <=>
    AsyncReplyRoute!ReplyRouteV2Fairness
BY DEF AsyncReplyRouteFairness

(***************************************************************************
The inherited route fairness names bare ticket/service actions.  These
current-state guards prove that their kernel prerequisites already retain the
exact positive, nonempty base output required by the production wrappers.
The primed product coupling then prevents base-only revocation while either
route occurrence remains live; no additional fairness premise is introduced.
***************************************************************************)
THEOREM AsyncReplyServiceReadyPositiveOutputGuardObligation ==
  AsyncReplyServiceReadyPositiveOutputGuard
BY DEF AsyncReplyServiceReadyPositiveOutputGuard,
       AsyncReplySemanticServiceReady

THEOREM AsyncReplyBareAcquirePositiveBaseGuardObligation ==
  AsyncReplyBareAcquirePositiveBaseGuard
BY Isa
   DEF AsyncReplyBareAcquirePositiveBaseGuard,
       AsyncReplyRouteBaseAttemptCoupling,
       AsyncReplyRouteToBaseAttemptCoupling,
       AsyncReplyRoute!ReplyAttemptLifecycleIdentityOwned,
       AsyncReplyRoute!ReplyAttemptLifecycleIdentitiesFor

THEOREM AsyncReplyBareServicePositiveBaseGuardObligation ==
  AsyncReplyBareServicePositiveBaseGuard
BY Isa
   DEF AsyncReplyBareServicePositiveBaseGuard,
       AsyncReplyRouteBaseAttemptCoupling,
       AsyncReplyRouteToBaseAttemptCoupling,
       AsyncReplyRoute!ReplyAttemptOwned,
       AsyncReplyRoute!ReplyAttemptsForSource,
       AsyncReplyRoute!ReplyAttemptsFor

THEOREM AsyncReplyBareFairnessGuardsRequirePositiveBase ==
  /\ AsyncReplyServiceReadyPositiveOutputGuard
  /\ AsyncReplyBareAcquirePositiveBaseGuard
  /\ AsyncReplyBareServicePositiveBaseGuard
BY AsyncReplyServiceReadyPositiveOutputGuardObligation,
   AsyncReplyBareAcquirePositiveBaseGuardObligation,
   AsyncReplyBareServicePositiveBaseGuardObligation

(***************************************************************************
Checked product boundaries.  These theorems prove exact V2 action projection,
both bracket projections, and both spec projections without importing any
reply-route safety or progress result.
***************************************************************************)
THEOREM AsyncReplyRouteNextProjectionObligation ==
  AsyncReplyRouteNext => AsyncReplyRoute!ReplyRouteV2Next
BY SMT
   DEF AsyncReplyRouteNext,
       AsyncObserveNewReplySource,
       AsyncObserveLaterReplyDelivery,
       AsyncReconnectReplySource,
       AsyncExactReplyCapabilityRetry,
       AsyncReplyRoute!ReplyRouteV2Next,
       AsyncReplyRoute!RetireReplySourceV2,
       AsyncReplyRoute!RecoverReplyRouteState

THEOREM AsyncProductionBracketProjectsAsyncBracketObligation ==
  [AsyncProductionNext]_AsyncProductionVars
    => [AsyncNext]_AsyncAllVars
BY SMT DEF AsyncProductionNext, AsyncProductionVars

THEOREM AsyncProductionBracketProjectsReplyV2BracketObligation ==
  [AsyncProductionNext]_AsyncProductionVars
    => [AsyncReplyRoute!ReplyRouteV2Next]_AsyncReplyRouteVars
BY AsyncReplyRouteNextProjectionObligation, SMT
   DEF AsyncProductionNext, AsyncProductionVars

THEOREM AsyncProductionSpecAtProjectsAsyncSpecAtObligation ==
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
      BY <2>2,
         AsyncProductionBracketProjectsAsyncBracketObligation, PTL
    <2>4. AsyncFairnessAt(initialContext)
      BY <1>1 DEF AsyncProductionSpecAt
    <2> QED BY <2>1, <2>3, <2>4 DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncProductionSpecAtProjectsReplyRouteV2SpecObligation ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext)
      => AsyncReplyRoute!ReplyRouteV2Spec
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncProductionSpecAt(initialContext)
         PROVE AsyncReplyRoute!ReplyRouteV2Spec
    <2>1. AsyncReplyRouteInit
      BY <1>1 DEF AsyncProductionSpecAt
    <2>2. [][AsyncProductionNext]_AsyncProductionVars
      BY <1>1 DEF AsyncProductionSpecAt
    <2>3. [][AsyncReplyRoute!ReplyRouteV2Next]_AsyncReplyRouteVars
      BY <2>2,
         AsyncProductionBracketProjectsReplyV2BracketObligation, PTL
    <2>4. AsyncReplyRouteFairness
      BY <1>1 DEF AsyncProductionSpecAt
    <2>5. AsyncReplyRoute!ReplyRouteV2Init
      BY <2>1, AsyncReplyRouteInitContainsExactV2Init
    <2>6. AsyncReplyRoute!ReplyRouteV2Fairness
      BY <2>4, AsyncReplyRouteFairnessIsExactV2Fairness
    <2> QED BY <2>3, <2>5, <2>6
         DEF AsyncReplyRoute!ReplyRouteV2Spec,
             AsyncReplyRouteVars
  <1> QED BY <1>1

(***************************************************************************
Specified-unproved temporal boundaries.  They remain operators, not THEOREMs,
and therefore provide no machine-checked completion evidence.
***************************************************************************)
AsyncReplyRouteV2InductiveSafetyObligation ==
  AsyncReplyRouteProofs!ReplyRouteV2InductiveSafetyObligation

AsyncReplyRouteV2SuccessorIsolationObligation ==
  AsyncReplyRouteProofs!ReplyRouteV2SuccessorIsolationObligation

AsyncNetworkReplyRouteTemporalProductObligation ==
  \A initialContext:
    AsyncProductionSpecAt(initialContext) =>
      /\ []AsyncReplyRouteV2SafetyInvariant
      /\ AsyncReplyTenureAwareReplay
      /\ AsyncReplySourceIsolation

THEOREM AsyncNetworkReplyRouteActionProjectionObligation ==
  /\ AsyncReplyRouteNextProjectionObligation
  /\ AsyncProductionBracketProjectsAsyncBracketObligation
  /\ AsyncProductionBracketProjectsReplyV2BracketObligation
BY AsyncReplyRouteNextProjectionObligation,
   AsyncProductionBracketProjectsAsyncBracketObligation,
   AsyncProductionBracketProjectsReplyV2BracketObligation

AsyncNetworkReplyRouteModelObligation ==
  /\ AsyncProductionSpecAtProjectsAsyncSpecAtObligation
  /\ AsyncProductionSpecAtProjectsReplyRouteV2SpecObligation
  /\ AsyncReplyRouteV2InductiveSafetyObligation
  /\ AsyncReplyRouteV2SuccessorIsolationObligation
  /\ AsyncNetworkReplyRouteTemporalProductObligation

=============================================================================
