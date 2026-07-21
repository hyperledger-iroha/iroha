---- MODULE SumeragiV2ReplyRouteOwnershipProofs ----
EXTENDS SumeragiV2ReplyRouteOwnership, FiniteSetTheorems,
        SumeragiV2TemporalLemmas, TLAPS

(***************************************************************************
Deductive ownership and liveness boundary for bounded reply routes.

The executable module keeps semantic request identity independent of both
delivery ordinal and connection tenure.  This proof module establishes the
inductive ownership/replay invariants and then discharges progress in two
fair phases.  A stable current route first acquires its own tenure-bound
ticket.  The shared service action then lowers the finite cyclic distance to
that source until its retained cursor strictly advances.
***************************************************************************)

ReplyRouteInductiveInvariant ==
  /\ ReplyRouteConfiguration
  /\ ReplyRouteSafetyInvariant

ReplyDistanceCarrier == 0..(Len(ReplySourceOrder) - 1)
ReplyDistanceOrdering == OpToRel(<, Nat)

ReplySourceTicketPending(owner, semantic, source,
                         messageCursor, chunkCursor) ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplySourceRouteStable(owner, semantic, source)
  /\ ReplySourceAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ ~ReplySourceServiceEligible(owner, semantic, source)

ReplySourceReadyAtCursor(owner, semantic, source,
                         messageCursor, chunkCursor) ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplySourceRouteStable(owner, semantic, source)
  /\ ReplySourceServiceEligible(owner, semantic, source)
  /\ ReplySourceAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)

ReplySourceServiceRank(owner, semantic, source,
                       messageCursor, chunkCursor, distance) ==
  /\ ReplySourceReadyAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ distance \in ReplyDistanceCarrier
  /\ ReplySourceRoundRobinRank(owner, semantic, source) = distance

ReplySourceLowerServiceRank(owner, semantic, source,
                            messageCursor, chunkCursor, distance) ==
  \E lower \in SetLessThan(
                 distance, ReplyDistanceOrdering, ReplyDistanceCarrier):
    ReplySourceServiceRank(
      owner, semantic, source, messageCursor, chunkCursor, lower)

(***************************************************************************
Finite source geometry.
***************************************************************************)

THEOREM ReplySourceIndexExists ==
  ReplyRouteConfiguration =>
    \A source \in ReplySources:
      ReplySourceIndices(source) # {}
BY Isa DEF ReplyRouteConfiguration, ReplySources, ReplySourceIndices

THEOREM ReplySourceIndexUnique ==
  ReplyRouteConfiguration =>
    \A source \in ReplySources:
      \A left, right \in ReplySourceIndices(source):
        left = right
BY Isa DEF ReplyRouteConfiguration, ReplySourceIndices

THEOREM ReplySourceIndexTyped ==
  ReplyRouteConfiguration =>
    \A source \in ReplySources:
      /\ ReplySourceIndex(source) \in 1..Len(ReplySourceOrder)
      /\ ReplySourceOrder[ReplySourceIndex(source)] = source
PROOF
  <1>1. ASSUME ReplyRouteConfiguration
         PROVE \A source \in ReplySources:
                 /\ ReplySourceIndex(source)
                      \in 1..Len(ReplySourceOrder)
                 /\ ReplySourceOrder[ReplySourceIndex(source)] = source
    <2>1. ASSUME NEW source \in ReplySources
           PROVE /\ ReplySourceIndex(source)
                        \in 1..Len(ReplySourceOrder)
                 /\ ReplySourceOrder[ReplySourceIndex(source)] = source
      <3>1. ReplySourceIndices(source) # {}
        BY <1>1, <2>1, ReplySourceIndexExists
      <3>2. ReplySourceIndex(source) \in ReplySourceIndices(source)
        BY <3>1, Isa DEF ReplySourceIndex
      <3> QED BY <3>2 DEF ReplySourceIndices
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplySourceIndexInjective ==
  ReplyRouteConfiguration =>
    \A left, right \in ReplySources:
      ReplySourceIndex(left) = ReplySourceIndex(right) => left = right
BY ReplySourceIndexTyped, Isa

THEOREM ReplySourceCyclicDistanceBounded ==
  ReplyRouteConfiguration =>
    \A start, candidate \in 1..Len(ReplySourceOrder):
      ReplySourceCyclicDistance(start, candidate)
        \in ReplyDistanceCarrier
BY SMTT(30)
   DEF ReplyRouteConfiguration, ReplySourceCyclicDistance,
       ReplyDistanceCarrier

THEOREM ReplySourceRoundRobinRankTyped ==
  ReplyRouteInductiveInvariant =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
      ReplySourceRoundRobinRank(owner, semantic, source)
        \in ReplyDistanceCarrier
BY ReplySourceIndexTyped, ReplySourceCyclicDistanceBounded, Isa
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplySourceRoundRobinRank

THEOREM ReplyDistanceOrderingWellFounded ==
  IsWellFoundedOn(ReplyDistanceOrdering, ReplyDistanceCarrier)
PROOF
  <1>1. ReplyDistanceCarrier \subseteq Nat
    BY Isa DEF ReplyDistanceCarrier
  <1>2. IsWellFoundedOn(OpToRel(<, Nat), ReplyDistanceCarrier)
    BY <1>1, NatLessThanWellFounded, IsWellFoundedOnSubset
  <1> QED BY <1>2 DEF ReplyDistanceOrdering

(***************************************************************************
Opaque delivery binding and bounded retired-history leaves.
***************************************************************************)

THEOREM ReplyIntrinsicBindingSubstitutionRejected ==
  \A owner \in ReplyOwners, source \in ReplySources,
     semantic \in ReplySemantics:
    \A capability:
      ~ReplyCapabilityIntrinsicBindingValid(capability) =>
        ReplyCapabilityRejection(capability, owner, source, semantic) =
          "EqualOrdinalDifferentTenure"
BY SMTT(30)
   DEF ReplyCapabilityRejection,
       ReplyCapabilityIntrinsicBindingValid

THEOREM ReplyKnownOrdinalCollisionRejected ==
  \A owner \in ReplyOwners, source \in ReplySources,
     semantic \in ReplySemantics:
    \A capability:
      /\ ReplyCapabilityIntrinsicBindingValid(capability)
      /\ capability.owner = owner
      /\ capability.source = source
      /\ capability.target = ReplySemanticTarget(semantic)
      /\ capability.semantic = semantic
      /\ rrSourceActive[owner][source]
      /\ ReplyCapabilityHasKnownOrdinalCollision(capability)
      => ReplyCapabilityRejection(capability, owner, source, semantic) =
           "EqualOrdinalDifferentTenure"
BY SMTT(60)
   DEF ReplyCapabilityRejection,
       ReplyCapabilityIntrinsicBindingValid,
       ReplyCapabilityHasKnownOrdinalCollision

THEOREM ReplyRouteUpdateRecordsLatestRetiredDelivery ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    deliveryOrdinal > attempt.deliveryOrdinal =>
      LET routed ==
            ReplyAttemptWithRoute(
              attempt, deliveryOrdinal, connectionTenure)
      IN /\ routed.retiredDeliveryOrdinal = attempt.deliveryOrdinal
         /\ routed.retiredConnectionTenure = attempt.connectionTenure
         /\ ReplyAttemptCursor(routed) = ReplyAttemptCursor(attempt)
BY SMTT(60)
   DEF ReplyAttemptWithRoute, ReplyAttemptCursor,
       ReplyAttemptSet

(***************************************************************************
Inductive safety and source-isolated replay.
***************************************************************************)

THEOREM ReplyRouteInitEstablishesInductiveInvariant ==
  ReplyRouteInit => ReplyRouteInductiveInvariant
BY FS_EmptySet, SMTT(60)
   DEF ReplyRouteInit, ReplyRouteInductiveInvariant,
       ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
       ReplyRouteOwnershipInvariant, ReplyAttemptsFor,
       ReplyAttemptsForSource, ReplyAttemptSources,
       ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor, ReplyAttemptSet,
       ReplySources, ReplyDeliveryOrdinals, ReplyConnectionTenures,
       NoReplyTicketTenure

THEOREM ObserveNewReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ObserveNewReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
BY FS_AddElement, SMTT(90)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       ObserveNewReplySource, ReplyCapabilityValidFor,
       ReplyCapability, ReplyCapabilityIntrinsicBindingValid,
       ReplyCapabilityHasKnownOrdinalCollision,
       ReplyCapabilityIdentityMatchesAttempt,
       ReplyAttempt, ReplyAttemptSet,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptCurrent, ReplyTicketValidForAttempt,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyTicketForAttempt, ReplyTicket, ReplySources,
       ReplyDeliveryOrdinals, ReplyConnectionTenures,
       NoReplyTicketTenure

THEOREM ObserveLaterReplyDeliveryPreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
BY SMTT(120)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       ObserveLaterReplyDelivery, ReplyCapabilityValidFor,
       ReplyCapability, ReplyCapabilityIntrinsicBindingValid,
       ReplyCapabilityHasKnownOrdinalCollision,
       ReplyCapabilityIdentityMatchesAttempt,
       ReplyAttemptWithRoute, ReplaceReplyAttempt,
       ReplyAttemptSet, ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptCurrent, ReplyTicketValidForAttempt,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyTicketForAttempt, ReplyTicket, ReplySources,
       ReplyDeliveryOrdinals, ReplyConnectionTenures,
       NoReplyTicketTenure

THEOREM RetryExactReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RetryExactReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
BY SMTT(30)
   DEF RetryExactReplySource, ReplyRouteInductiveInvariant,
       ReplyRouteVars

THEOREM RetireReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RetireReplySource(owner, source)
    => ReplyRouteInductiveInvariant'
BY SMTT(120)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       RetireReplySource, ReplyAttemptWithoutTicket,
       ReplySourceHasNoTickets, ReplyAttemptSet,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyAttemptCurrent, ReplyTicketValidForAttempt,
       ReplyTicketForAttempt, ReplyTicket,
       NoReplyTicketTenure, ReplyRouteConfiguration

THEOREM ReconnectReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ReconnectReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
BY SMTT(120)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       ReconnectReplySource, ReplyCapability,
       ReplyAttemptWithRoute, ReplyAttemptsAfterReconnect,
       ReplyAttemptWithoutTicket, ReplySourceHasNoTickets,
       ReplyAttemptSet, ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyAttemptCurrent, ReplyTicketValidForAttempt,
       ReplyTicketForAttempt, ReplyTicket, ReplySources,
       ReplyDeliveryOrdinals, ReplyConnectionTenures,
       NoReplyTicketTenure

THEOREM ReconnectReplySourceInvalidatesAllSourceTickets ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReconnectReplySource(owner, semantic, source) =>
      ReplySourceHasNoTickets(owner, source)'
BY SMTT(90)
   DEF ReconnectReplySource, ReplyAttemptsAfterReconnect,
       ReplyAttemptWithoutTicket, ReplySourceHasNoTickets,
       ReplyAttemptHasNoTicket, ReplyAttemptWithRoute,
       ReplyAttemptFor, ReplyAttemptsForSource,
       NoReplyTicketTenure

THEOREM AcquireReplyTicketPreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ AcquireReplyTicket(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
BY SMTT(90)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       AcquireReplyTicket, ReplyAttemptWithTicket,
       ReplaceReplyAttempt, ReplyAttemptSet,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyAttemptCurrent, ReplyTicketValidForAttempt,
       ReplyTicketForAttempt, ReplyTicket, NoReplyTicketTenure

THEOREM AdvanceCurrentReplyAttemptPreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ AdvanceCurrentReplyAttempt(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                AdvanceCurrentReplyAttempt(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. ReplyRouteTypeInvariant'
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             AdvanceCurrentReplyAttempt,
             ReplyAttemptServiceKernelValid,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>3. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             Cardinality(
               ReplyAttemptsForSource(
                 nextOwner, nextSemantic, nextSource))' <= 1
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant,
             AdvanceCurrentReplyAttempt,
             ReplyAttemptServiceKernelValid,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
                  nextSemantic \in rrPayloads'
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant,
             AdvanceCurrentReplyAttempt,
             ReplyAttemptServiceKernelValid,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptSources, ReplyRetiredDeliverySources
    <2>5. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant,
             AdvanceCurrentReplyAttempt,
             ReplyAttemptServiceKernelValid,
             ReplaceReplyAttempt,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptRetiredDeliveryWellFormed,
             ReplyAttemptCurrent,
             ReplyTicketValidForAttempt,
             ReplyTicketForAttempt, ReplyTicket,
             ReplyAttemptHasNoTicket,
             NoReplyTicketTenure
    <2>6. ReplyRouteOwnershipInvariant'
      BY <2>3, <2>4, <2>5
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>6
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ServiceReplyRoutePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteInductiveInvariant
    /\ ServiceReplyRoute(owner, semantic)
    => ReplyRouteInductiveInvariant'
BY SMTT(120)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       ServiceReplyRoute, ReplySelectedSourceIndex,
       ReplyPendingSourceIndices, ReplyAttemptAfterService,
       ReplaceReplyAttempt, NextReplySourceIndex,
       ReplyAttemptSet, ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyAttemptSources, ReplyRetiredDeliverySources,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       ReplyAttemptComplete, ReplyAttemptCurrent,
       ReplyTicketValidForAttempt, ReplyTicketForAttempt, ReplyTicket,
       ReplySourceCyclicDistance, ReplyRouteConfiguration

THEOREM ReplyRouteNextPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplyRouteNext
  => ReplyRouteInductiveInvariant'
PROOF
  <1>1. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveNewReplySource(owner, semantic, source)
    BY <1>1, ObserveNewReplySourcePreservesInductiveInvariant
  <1>2. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveLaterReplyDelivery(owner, semantic, source)
    BY <1>2, ObserveLaterReplyDeliveryPreservesInductiveInvariant
  <1>3. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               RetryExactReplySource(owner, semantic, source)
    BY <1>3, RetryExactReplySourcePreservesInductiveInvariant
  <1>4. CASE \E owner \in ReplyOwners, source \in ReplySources:
               RetireReplySource(owner, source)
    BY <1>4, RetireReplySourcePreservesInductiveInvariant
  <1>5. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ReconnectReplySource(owner, semantic, source)
    BY <1>5, ReconnectReplySourcePreservesInductiveInvariant
  <1>6. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               AcquireReplyTicket(owner, semantic, source)
    BY <1>6, AcquireReplyTicketPreservesInductiveInvariant
  <1>7. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics:
               ServiceReplyRoute(owner, semantic)
    BY <1>7, ServiceReplyRoutePreservesInductiveInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7
       DEF ReplyRouteNext

THEOREM ReplyRouteStutterPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ UNCHANGED ReplyRouteVars
  => ReplyRouteInductiveInvariant'
BY SMTT(30)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteTypeInvariant, ReplyRouteOwnershipInvariant,
       ReplyRouteVars

THEOREM ReplyRouteBracketPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ [ReplyRouteNext]_ReplyRouteVars
  => ReplyRouteInductiveInvariant'
BY ReplyRouteNextPreservesInductiveInvariant,
   ReplyRouteStutterPreservesInductiveInvariant

THEOREM ReplyRouteSpecAlwaysSafetyInvariant ==
  ReplyRouteSpec => []ReplyRouteSafetyInvariant
PROOF
  <1>1. ReplyRouteInit => ReplyRouteInductiveInvariant
    BY ReplyRouteInitEstablishesInductiveInvariant
  <1>2. /\ ReplyRouteInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => ReplyRouteInductiveInvariant'
    BY ReplyRouteBracketPreservesInductiveInvariant
  <1>3. ReplyRouteSpec => []ReplyRouteInductiveInvariant
    BY <1>1, <1>2, PTL DEF ReplyRouteSpec
  <1> QED BY <1>3, PTL DEF ReplyRouteInductiveInvariant

THEOREM ReplyRouteNextProvidesReplayAndIsolation ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplyRouteNext
  => /\ ReplyTenureAwareReplayStep
     /\ ReplySourceIsolationStep
BY SMTT(180)
   DEF ReplyRouteNext, ObserveNewReplySource,
       ObserveLaterReplyDelivery, RetryExactReplySource,
       RetireReplySource, ReconnectReplySource,
       AcquireReplyTicket, ServiceReplyRoute,
       ReplyTenureAwareReplayStep, ReplySourceTenureInvalidationStep,
       ReplySourceIsolationStep, ReplySourceHasNoTickets,
       SameReplyAttemptIdentity, ReplyAttemptCursor,
       ReplyAttemptAfterService, ReplyAttemptWithRoute,
       ReplyAttemptWithTicket, ReplyAttemptWithoutTicket,
       ReplyAttemptsAfterReconnect, ReplaceReplyAttempt,
       ReplyCapabilityValidFor, ReplyCapability,
       ReplyCapabilityIntrinsicBindingValid,
       ReplyCapabilityHasKnownOrdinalCollision,
       ReplyCapabilityIdentityMatchesAttempt,
       ReplySelectedSourceIndex, ReplyPendingSourceIndices,
       ReplyAttemptOwned, ReplyAttemptFor, ReplyAttemptsFor,
       ReplyAttemptsForSource, ReplyRouteInductiveInvariant,
       ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplyRetiredDeliverySources,
       ReplyAttemptRetiredDeliveryWellFormed,
       ReplyAttemptHasNoRetiredDelivery,
       NoReplyTicketTenure

THEOREM ReplyRouteBracketProvidesReplayAndIsolation ==
  /\ ReplyRouteInductiveInvariant
  /\ [ReplyRouteNext]_ReplyRouteVars
  => /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
     /\ [ReplySourceIsolationStep]_ReplyRouteVars
BY ReplyRouteNextProvidesReplayAndIsolation, PTL

THEOREM ReplyRouteSpecAlwaysReplayAndIsolation ==
  ReplyRouteSpec =>
    /\ ReplyTenureAwareReplay
    /\ ReplySourceIsolation
PROOF
  <1>1. ReplyRouteSpec => []ReplyRouteInductiveInvariant
    BY ReplyRouteSpecAlwaysSafetyInvariant, PTL
       DEF ReplyRouteInductiveInvariant, ReplyRouteSpec, ReplyRouteInit
  <1>2. /\ ReplyRouteInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
             /\ [ReplySourceIsolationStep]_ReplyRouteVars
    BY ReplyRouteBracketProvidesReplayAndIsolation
  <1> QED BY <1>1, <1>2, PTL
       DEF ReplyRouteSpec, ReplyTenureAwareReplay,
           ReplySourceIsolation

(***************************************************************************
Cursor persistence before the stable suffix.
***************************************************************************)

THEOREM ReplyCursorStepPersistsOrAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
    => \/ ReplySourceAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(90)
   DEF ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplySourceAtCursor, ReplySourceAdvancedFrom,
       ReplyTenureAwareReplayStep, ReplySourceTenureInvalidationStep,
       ReplySourceHasNoTickets, ReplySourceIsolationStep,
       SameReplyAttemptIdentity, ReplyAttemptCursor,
       ReplyAttemptRank, ReplyAttemptComplete,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

THEOREM ReplyAdvancedStepIsStable ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
    => ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(90)
   DEF ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplySourceAdvancedFrom, ReplyTenureAwareReplayStep,
       ReplySourceTenureInvalidationStep, ReplySourceHasNoTickets,
       ReplySourceIsolationStep, SameReplyAttemptIdentity,
       ReplyAttemptCursor, ReplyAttemptRank, ReplyAttemptComplete,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource

THEOREM ReplyCursorBracketPersistsOrAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
    /\ [ReplySourceIsolationStep]_ReplyRouteVars
    => \/ ReplySourceAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
BY ReplyCursorStepPersistsOrAdvances, SMTT(30)
   DEF ReplySourceAtCursor, ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource, ReplyRouteVars

THEOREM ReplyAdvancedBracketIsStable ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
    /\ [ReplySourceIsolationStep]_ReplyRouteVars
    => ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)'
BY ReplyAdvancedStepIsStable, SMTT(30)
   DEF ReplySourceAdvancedFrom, ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource, ReplyRouteVars

THEOREM ReplyCursorReachesStableSuffixOrAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSpec
    /\ ReplySourceStableResponsive(owner, semantic, source)
    => (ReplySourceAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)
          ~> (\/ ReplySourceAdvancedFrom(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
               \/ /\ ReplySourceAtCursor(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
                  /\ ReplySourceRouteStable(
                       owner, semantic, source)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteSpec,
                ReplySourceStableResponsive(owner, semantic, source)
         PROVE ReplySourceAtCursor(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
                 ~> (\/ ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor)
                      \/ /\ ReplySourceAtCursor(
                               owner, semantic, source,
                               messageCursor, chunkCursor)
                         /\ ReplySourceRouteStable(
                              owner, semantic, source))
    <2>1. []ReplyRouteSafetyInvariant
      BY <1>1, ReplyRouteSpecAlwaysSafetyInvariant
    <2>2. /\ ReplyTenureAwareReplay
             /\ ReplySourceIsolation
      BY <1>1, ReplyRouteSpecAlwaysReplayAndIsolation
    <2>3. /\ ReplyRouteSafetyInvariant
             /\ ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
             /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
             /\ [ReplySourceIsolationStep]_ReplyRouteVars
            => \/ ReplySourceAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
               \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
      BY ReplyCursorBracketPersistsOrAdvances
    <2>4. /\ ReplyRouteSafetyInvariant
             /\ ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
             /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
             /\ [ReplySourceIsolationStep]_ReplyRouteVars
            => ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor)'
      BY ReplyAdvancedBracketIsStable
    <2> QED BY <1>1, <2>3, <2>4, PTL
         DEF ReplySourceStableResponsive
  <1> QED BY <1>1

(***************************************************************************
Ticket acquisition under a stable current route.
***************************************************************************)

THEOREM ReplyValidTicketBindsCurrentPayload ==
  \A attempt \in rrAttempts:
    ReplyTicketValidForAttempt(attempt) =>
      /\ attempt.ticketSemantic = {attempt.semantic}
      /\ attempt.ticketTarget =
           {ReplySemanticTarget(attempt.semantic)}
      /\ attempt.ticketMessageCursor = {attempt.messageCursor}
      /\ attempt.ticketChunkCursor = {attempt.chunkCursor}
BY Isa DEF ReplyTicketValidForAttempt, ReplyTicketForAttempt,
           ReplyTicket

THEOREM ReplyStableCursorClassifiesTicket ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplySourceRouteStable(owner, semantic, source)
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    => \/ ReplySourceTicketPending(
            owner, semantic, source, messageCursor, chunkCursor)
       \/ ReplySourceReadyAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)
BY SMTT(30)
   DEF ReplySourceTicketPending, ReplySourceReadyAtCursor,
       ReplySourceServiceEligible, ReplySourceRouteStable,
       ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptOwned, ReplyAttemptFor, NoReplyTicketTenure

THEOREM ReplyTicketPendingEnablesAcquire ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplySourceTicketPending(
      owner, semantic, source, messageCursor, chunkCursor)
      => ENABLED
           <<AcquireReplyTicket(owner, semantic, source)>>_ReplyRouteVars
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor, NEW chunkCursor,
                ReplySourceTicketPending(
                  owner, semantic, source, messageCursor, chunkCursor)
         PROVE ENABLED
                 <<AcquireReplyTicket(owner, semantic, source)>>_(
                   ReplyRouteVars)
    <2>1. ENABLED AcquireReplyTicket(owner, semantic, source)
      BY <1>1, ExpandENABLED, Isa
         DEF ReplySourceTicketPending, ReplySourceServiceEligible,
             ReplySourceRouteStable, ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant, AcquireReplyTicket,
             ReplyAttemptWithTicket, ReplaceReplyAttempt,
             ReplyTicketValidForAttempt, ReplyAttemptCurrent,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             NoReplyTicketTenure
    <2>2. AcquireReplyTicket(owner, semantic, source)
             => <<AcquireReplyTicket(owner, semantic, source)>>_(
                   ReplyRouteVars)
      BY <1>1, SMTT(30)
         DEF ReplySourceTicketPending, ReplySourceServiceEligible,
             ReplySourceRouteStable, AcquireReplyTicket,
             ReplyAttemptWithTicket, ReplaceReplyAttempt,
             ReplyAttemptCurrent, ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             NoReplyTicketTenure, ReplyRouteVars
    <2> QED BY <2>1, <2>2, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM ReplyAcquireMakesSourceReady ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplySourceTicketPending(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ AcquireReplyTicket(owner, semantic, source)
    => ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
BY ReplyValidTicketBindsCurrentPayload, SMTT(60)
   DEF ReplySourceTicketPending, ReplySourceReadyAtCursor,
       ReplySourceServiceEligible, ReplySourceRouteStable,
       ReplySourceAtCursor, ReplyRouteSafetyInvariant,
       ReplyRouteOwnershipInvariant, AcquireReplyTicket,
       ReplyAttemptWithTicket, ReplaceReplyAttempt,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptComplete, ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource, ReplyAttemptSet,
       NoReplyTicketTenure

THEOREM ReplyTicketPendingPersistsOrBecomesReady ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceRouteStable(owner, semantic, source)'
    /\ ReplySourceTicketPending(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ [ReplyRouteNext]_ReplyRouteVars
    => \/ ReplySourceTicketPending(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceReadyAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(180)
   DEF ReplyRouteNext, ObserveNewReplySource,
       ObserveLaterReplyDelivery, RetryExactReplySource,
       RetireReplySource, ReconnectReplySource,
       AcquireReplyTicket, ServiceReplyRoute,
       ReplySourceTicketPending, ReplySourceReadyAtCursor,
       ReplySourceServiceEligible, ReplySourceRouteStable,
       ReplySourceAtCursor, ReplyRouteInductiveInvariant,
       ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplyAttemptWithRoute, ReplyAttemptWithTicket,
       ReplyAttemptWithoutTicket, ReplyAttemptsAfterReconnect,
       ReplySourceHasNoTickets, ReplyAttemptAfterService,
       ReplaceReplyAttempt,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptComplete, ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplySelectedSourceIndex, ReplyPendingSourceIndices,
       NoReplyTicketTenure, ReplyRouteVars

THEOREM ReplyStableCursorEventuallyBecomesReady ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSpec
    /\ []ReplySourceRouteStable(owner, semantic, source)
    => ((/\ ReplySourceAtCursor(
              owner, semantic, source, messageCursor, chunkCursor)
             /\ ReplySourceRouteStable(owner, semantic, source))
          ~> ReplySourceReadyAtCursor(
               owner, semantic, source, messageCursor, chunkCursor))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteSpec,
                []ReplySourceRouteStable(owner, semantic, source)
         PROVE (/\ ReplySourceAtCursor(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
                   /\ ReplySourceRouteStable(
                        owner, semantic, source))
                 ~> ReplySourceReadyAtCursor(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
    <2>1. []ReplyRouteInductiveInvariant
      BY <1>1, ReplyRouteSpecAlwaysSafetyInvariant, PTL
         DEF ReplyRouteInductiveInvariant, ReplyRouteSpec, ReplyRouteInit
    <2>2. [][ReplyRouteNext]_ReplyRouteVars
      BY <1>1, PTL DEF ReplyRouteSpec
    <2>3. WF_ReplyRouteVars(
             AcquireReplyTicket(owner, semantic, source))
      BY <1>1 DEF ReplyRouteSpec, ReplyRouteFairness
    <2>4. /\ ReplyRouteSafetyInvariant
             /\ ReplySourceRouteStable(owner, semantic, source)
             /\ ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
            => \/ ReplySourceTicketPending(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
               \/ ReplySourceReadyAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
      BY ReplyStableCursorClassifiesTicket
    <2>5. /\ ReplyRouteInductiveInvariant
             /\ ReplySourceRouteStable(owner, semantic, source)'
             /\ ReplySourceTicketPending(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
             /\ [ReplyRouteNext]_ReplyRouteVars
            => \/ ReplySourceTicketPending(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
               \/ ReplySourceReadyAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
      BY ReplyTicketPendingPersistsOrBecomesReady
    <2>6. ReplySourceTicketPending(
             owner, semantic, source, messageCursor, chunkCursor)
             => ENABLED
                  <<AcquireReplyTicket(
                      owner, semantic, source)>>_ReplyRouteVars
      BY ReplyTicketPendingEnablesAcquire
    <2>7. /\ ReplySourceTicketPending(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
             /\ <<AcquireReplyTicket(
                       owner, semantic, source)>>_ReplyRouteVars
            => ReplySourceReadyAtCursor(
                 owner, semantic, source,
                 messageCursor, chunkCursor)'
      BY ReplyAcquireMakesSourceReady
    <2>8. ReplySourceTicketPending(
             owner, semantic, source, messageCursor, chunkCursor)
             ~> ReplySourceReadyAtCursor(
                  owner, semantic, source, messageCursor, chunkCursor)
      BY <1>1, <2>1, <2>2, <2>3, <2>5, <2>6, <2>7, PTL
    <2> QED BY <1>1, <2>1, <2>4, <2>8, PTL
         DEF ReplyRouteInductiveInvariant
  <1> QED BY <1>1

(***************************************************************************
Finite round-robin service rank.
***************************************************************************)

THEOREM ReplyReadyCursorHasServiceRank ==
  ReplyRouteInductiveInvariant =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources,
       messageCursor \in 0..ReplyMessageCount,
       chunkCursor \in 0..ReplyChunkCount:
      ReplySourceReadyAtCursor(
        owner, semantic, source, messageCursor, chunkCursor)
        => \E distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
BY ReplySourceRoundRobinRankTyped, Isa
   DEF ReplySourceServiceRank, ReplySourceReadyAtCursor

THEOREM ReplyReadyCursorEnablesService ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplySourceReadyAtCursor(
      owner, semantic, source, messageCursor, chunkCursor)
      => ENABLED <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor, NEW chunkCursor,
                ReplySourceReadyAtCursor(
                  owner, semantic, source, messageCursor, chunkCursor)
         PROVE ENABLED
                 <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
    <2>1. ENABLED ServiceReplyRoute(owner, semantic)
      BY <1>1, ReplySourceIndexTyped, ExpandENABLED, Isa
         DEF ReplySourceReadyAtCursor,
             ReplySourceServiceEligible, ReplySourceRouteStable,
             ReplySourceAtCursor, ServiceReplyRoute,
             ReplyPendingSourceIndices, ReplySelectedSourceIndex,
             ReplyAttemptAfterService, ReplaceReplyAttempt,
             ReplyTicketValidForAttempt, ReplyAttemptComplete,
             ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplySourceIndices, ReplySourceIndex,
             ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
             ReplyRouteConfiguration
    <2>2. ServiceReplyRoute(owner, semantic)
             => <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
      BY <1>1, SMTT(30)
         DEF ReplySourceReadyAtCursor, ReplySourceAtCursor,
             ServiceReplyRoute, ReplyAttemptAfterService,
             ReplaceReplyAttempt, ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyRouteVars
    <2> QED BY <2>1, <2>2, ENABLEDaxioms
  <1> QED BY <1>1

THEOREM ReplySelectedServiceConsumesTicketAndAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplySelectedSource(owner, semantic) = source
    /\ ServiceReplyRoute(owner, semantic)
    => /\ ReplyAttemptHasNoTicket(
              ReplyAttemptFor(owner, semantic, source))'
       /\ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(120)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteOwnershipInvariant,
       ReplySourceReadyAtCursor, ReplySourceAdvancedFrom,
       ReplySourceAtCursor, ReplySourceServiceEligible,
       ReplySourceRouteStable, ServiceReplyRoute,
       ReplySelectedSource, ReplySelectedSourceIndex,
       ReplyPendingSourceIndices, ReplyAttemptAfterService,
       ReplyAttemptHasNoTicket, ReplyAttemptRank,
       ReplyAttemptComplete, ReplaceReplyAttempt,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       NoReplyTicketTenure

THEOREM ReplyServiceLowersRankOrAdvancesTarget ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, distance)
    /\ ServiceReplyRoute(owner, semantic)
    => \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceLowerServiceRank(
            owner, semantic, source,
            messageCursor, chunkCursor, distance)'
BY ReplySelectedServiceConsumesTicketAndAdvances, SMTT(180)
   DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteOwnershipInvariant, ReplyRouteTypeInvariant,
       ReplySourceServiceRank, ReplySourceLowerServiceRank,
       ReplySourceReadyAtCursor, ReplySourceAdvancedFrom,
       ReplySourceAtCursor, ReplySourceServiceEligible,
       ReplySourceRouteStable, ReplySourceRoundRobinRank,
       ReplySourceIndex, ReplySourceIndices,
       ReplyDistanceOrdering, ReplyDistanceCarrier,
       ServiceReplyRoute, ReplySelectedSourceIndex,
       ReplySelectedSource, ReplyPendingSourceIndices,
       ReplySourceCyclicDistance, NextReplySourceIndex,
       ReplyAttemptAfterService, ReplyAttemptRank,
       ReplyAttemptComplete, ReplaceReplyAttempt,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyRouteConfiguration

THEOREM ReplyServiceRankPersistsOrExits ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceRouteStable(owner, semantic, source)'
    /\ ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, distance)
    /\ [ReplyRouteNext]_ReplyRouteVars
    => \/ ReplySourceServiceRank(
            owner, semantic, source,
            messageCursor, chunkCursor, distance)'
       \/ ReplySourceLowerServiceRank(
            owner, semantic, source,
            messageCursor, chunkCursor, distance)'
       \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(240)
   DEF ReplyRouteNext, ObserveNewReplySource,
       ObserveLaterReplyDelivery, RetryExactReplySource,
       RetireReplySource, ReconnectReplySource,
       AcquireReplyTicket, ServiceReplyRoute,
       ReplySourceServiceRank, ReplySourceLowerServiceRank,
       ReplySourceReadyAtCursor, ReplySourceAdvancedFrom,
       ReplySourceAtCursor, ReplySourceServiceEligible,
       ReplySourceRouteStable, ReplySourceRoundRobinRank,
       ReplySourceIndex, ReplySourceIndices,
       ReplyDistanceOrdering, ReplyDistanceCarrier,
       ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
       ReplyRouteOwnershipInvariant, ReplyRouteTypeInvariant,
       ReplyAttemptWithRoute, ReplyAttemptWithTicket,
       ReplyAttemptWithoutTicket, ReplyAttemptsAfterReconnect,
       ReplySourceHasNoTickets, ReplyAttemptAfterService,
       ReplaceReplyAttempt,
       ReplySelectedSourceIndex, ReplySelectedSource,
       ReplyPendingSourceIndices, ReplySourceCyclicDistance,
       NextReplySourceIndex, ReplyAttemptRank,
       ReplyAttemptComplete, ReplyTicketValidForAttempt,
       ReplyAttemptCurrent, ReplyAttemptOwned, ReplyAttemptFor,
       ReplyAttemptsFor, ReplyAttemptsForSource,
       ReplyRouteConfiguration, ReplyRouteVars,
       NoReplyTicketTenure

THEOREM ReplyServiceRankLeadsToLowerOrAdvance ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    /\ ReplyRouteSpec
    /\ []ReplySourceRouteStable(owner, semantic, source)
    => (ReplySourceServiceRank(
          owner, semantic, source,
          messageCursor, chunkCursor, distance)
          ~> (\/ ReplySourceAdvancedFrom(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier,
                ReplyRouteSpec,
                []ReplySourceRouteStable(owner, semantic, source)
         PROVE ReplySourceServiceRank(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)
                 ~> (\/ ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor)
                      \/ ReplySourceLowerServiceRank(
                           owner, semantic, source,
                           messageCursor, chunkCursor, distance))
    <2>1. []ReplyRouteInductiveInvariant
      BY <1>1, ReplyRouteSpecAlwaysSafetyInvariant, PTL
         DEF ReplyRouteInductiveInvariant, ReplyRouteSpec, ReplyRouteInit
    <2>2. [][ReplyRouteNext]_ReplyRouteVars
      BY <1>1, PTL DEF ReplyRouteSpec
    <2>3. WF_ReplyRouteVars(ServiceReplyRoute(owner, semantic))
      BY <1>1 DEF ReplyRouteSpec, ReplyRouteFairness
    <2>4. /\ ReplyRouteInductiveInvariant
             /\ ReplySourceRouteStable(owner, semantic, source)'
             /\ ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance)
             /\ [ReplyRouteNext]_ReplyRouteVars
            => \/ ReplySourceServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
               \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
      BY ReplyServiceRankPersistsOrExits
    <2>5. ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, distance)
             => ENABLED
                  <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
      BY ReplyReadyCursorEnablesService
         DEF ReplySourceServiceRank
    <2>6. /\ ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance)
             /\ <<ServiceReplyRoute(
                       owner, semantic)>>_ReplyRouteVars
            => \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
      BY <1>1, <2>1, ReplyServiceLowersRankOrAdvancesTarget
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM ReplyReadyCursorEventuallyAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSpec
    /\ []ReplySourceRouteStable(owner, semantic, source)
    => (ReplySourceReadyAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)
          ~> ReplySourceAdvancedFrom(
               owner, semantic, source, messageCursor, chunkCursor))
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteSpec,
                []ReplySourceRouteStable(owner, semantic, source)
         PROVE ReplySourceReadyAtCursor(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
                 ~> ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
    <2>1. ReplyRouteSpec => []ReplyRouteInductiveInvariant
      BY ReplyRouteSpecAlwaysSafetyInvariant, PTL
         DEF ReplyRouteInductiveInvariant, ReplyRouteSpec, ReplyRouteInit
    <2>2. \A distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
               ~> (\/ ReplySourceAdvancedFrom(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
                    \/ \E lower \in SetLessThan(
                         distance, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
                         ReplySourceServiceRank(
                           owner, semantic, source,
                           messageCursor, chunkCursor, lower))
      BY <1>1, ReplyServiceRankLeadsToLowerOrAdvance
         DEF ReplySourceLowerServiceRank
    <2>3. \A distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
               ~> ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
      BY <2>2, ReplyDistanceOrderingWellFounded,
         WellFoundedLeadsTo
    <2>4. ReplySourceReadyAtCursor(
             owner, semantic, source, messageCursor, chunkCursor)
             => \E distance \in ReplyDistanceCarrier:
                  ReplySourceServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)
      BY <2>1, ReplyReadyCursorHasServiceRank, PTL
    <2> QED BY <2>3, <2>4, PTL
  <1> QED BY <1>1

(***************************************************************************
The stable premise does not grant a ticket.  The theorem composes retained
cursor monotonicity, fair ticket acquisition, and finite round-robin service.
***************************************************************************)

THEOREM ReplyRouteSpecProvidesSourceProgress ==
  ReplyRouteSpec =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
      ReplySourceEventuallyProgresses(owner, semantic, source)
PROOF
  <1>1. ASSUME ReplyRouteSpec
         PROVE \A owner \in ReplyOwners, semantic \in ReplySemantics,
                  source \in ReplySources:
                 ReplySourceEventuallyProgresses(
                   owner, semantic, source)
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE ReplySourceEventuallyProgresses(
                   owner, semantic, source)
      <3>1. ASSUME ReplySourceStableResponsive(
                    owner, semantic, source)
             PROVE \A messageCursor \in 0..ReplyMessageCount,
                        chunkCursor \in 0..ReplyChunkCount:
                     ReplySourceAtCursor(
                       owner, semantic, source,
                       messageCursor, chunkCursor)
                       ~> ReplySourceAdvancedFrom(
                            owner, semantic, source,
                            messageCursor, chunkCursor)
        <4>1. ASSUME NEW messageCursor \in 0..ReplyMessageCount,
                      NEW chunkCursor \in 0..ReplyChunkCount
               PROVE ReplySourceAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                       ~> ReplySourceAdvancedFrom(
                            owner, semantic, source,
                            messageCursor, chunkCursor)
          <5>1. ReplySourceAtCursor(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
                   ~> (\/ ReplySourceAdvancedFrom(
                            owner, semantic, source,
                            messageCursor, chunkCursor)
                        \/ /\ ReplySourceAtCursor(
                                 owner, semantic, source,
                                 messageCursor, chunkCursor)
                           /\ ReplySourceRouteStable(
                                owner, semantic, source))
            BY <1>1, <3>1, <4>1,
               ReplyCursorReachesStableSuffixOrAdvances
          <5>2. []ReplySourceRouteStable(owner, semantic, source)
                   => ((/\ ReplySourceAtCursor(
                              owner, semantic, source,
                              messageCursor, chunkCursor)
                            /\ ReplySourceRouteStable(
                                 owner, semantic, source))
                         ~> ReplySourceReadyAtCursor(
                              owner, semantic, source,
                              messageCursor, chunkCursor))
            BY <1>1, <4>1, ReplyStableCursorEventuallyBecomesReady
          <5>3. []ReplySourceRouteStable(owner, semantic, source)
                   => (ReplySourceReadyAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                         ~> ReplySourceAdvancedFrom(
                              owner, semantic, source,
                              messageCursor, chunkCursor))
            BY <1>1, <4>1, ReplyReadyCursorEventuallyAdvances
          <5> QED BY <3>1, <5>1, <5>2, <5>3, PTL
               DEF ReplySourceStableResponsive
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplySourceEventuallyProgresses
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyRouteOwnershipModelObligation ==
  ReplyRouteSpec =>
    /\ []ReplyRouteSafetyInvariant
    /\ ReplyTenureAwareReplay
    /\ ReplySourceIsolation
    /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
         ReplySourceEventuallyProgresses(owner, semantic, source)
BY ReplyRouteSpecAlwaysSafetyInvariant,
   ReplyRouteSpecAlwaysReplayAndIsolation,
   ReplyRouteSpecProvidesSourceProgress

=============================================================================
