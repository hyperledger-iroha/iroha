---- MODULE SumeragiV2ReplyRouteOwnershipProofs ----
EXTENDS SumeragiV2ReplyRouteOwnership, FiniteSetTheorems,
        SumeragiV2TemporalLemmas, NaturalsInduction, TLAPS

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

THEOREM ReplyNaturalStrictMultiplierGap ==
  \A factor, lower, upper \in Nat:
    factor > 0 /\ lower < upper
      => factor * lower + factor <= factor * upper
PROOF
  <1>1. ASSUME NEW lower \in Nat,
                NEW upper \in Nat,
                lower < upper
         PROVE \A factor \in Nat:
                 factor * lower + factor <= factor * upper
    <2>1. 0 * lower + 0 <= 0 * upper
      BY <1>1, SMT
    <2>2. ASSUME NEW factor \in Nat,
                  factor * lower + factor <= factor * upper
           PROVE (factor + 1) * lower + (factor + 1)
                   <= (factor + 1) * upper
      <3>1. /\ (factor + 1) * lower =
                   factor * lower + lower
             /\ (factor + 1) * upper =
                   factor * upper + upper
        BY <1>1, <2>2, SMT
      <3>2. lower + 1 <= upper
        BY <1>1, SMT
      <3> QED BY <1>1, <2>2, <3>1, <3>2, SMT
    <2> QED BY <2>1, <2>2, NatInduction
  <1> QED BY <1>1

THEOREM ReplyNaturalMultiplicationCommutes ==
  \A left, right \in Nat:
    left * right = right * left
PROOF
  <1>1. ASSUME NEW right \in Nat
         PROVE \A left \in Nat:
                 left * right = right * left
    <2>1. 0 * right = right * 0
      BY <1>1, SMT
    <2>2. ASSUME NEW left \in Nat,
                  left * right = right * left
           PROVE (left + 1) * right =
                   right * (left + 1)
      BY <1>1, <2>2, SMT
    <2> QED BY <2>1, <2>2, NatInduction
  <1> QED BY <1>1

THEOREM ReplyNaturalStrictThenWeakTransitive ==
  \A lower, middle, upper \in Nat:
    lower < middle /\ middle <= upper
      => lower < upper
BY SMT

THEOREM ReplyNaturalStrictTransitive ==
  \A lower, middle, upper \in Nat:
    lower < middle /\ middle < upper
      => lower < upper
BY SMT

THEOREM ReplyNaturalRankTermTyped ==
  \A messageCursor, chunkCursor, chunkCount \in Nat:
    messageCursor * (chunkCount + 1) + chunkCursor \in Nat
BY SMT

THEOREM ReplyNaturalProductSuccessorTyped ==
  \A left, right \in Nat:
    left * (right + 1) \in Nat
BY SMT

THEOREM ReplyNaturalStrictAdditiveMonotone ==
  \A base, lower, upper \in Nat:
    lower < upper => base + lower < base + upper
BY SMT

THEOREM ReplyBoundedNaturalFacts ==
  \A upper \in Nat:
    \A value \in 0..upper:
      /\ value \in Nat
      /\ value <= upper
BY SMT

THEOREM ReplyStableDisjunctionMeetsResponsive ==
  ASSUME STATE P, STATE Q, STATE S,
         [](P \/ Q),
         <>[]S
  PROVE P ~> (Q \/ (P /\ S))
BY PTL

ReplyAttemptFieldDomain ==
  {"owner", "source", "semantic", "deliveryOrdinal",
   "connectionTenure", "retiredDeliveryOrdinal",
   "retiredConnectionTenure", "ticketTenure", "ticketSemantic",
   "ticketTarget", "ticketMessageCursor", "ticketChunkCursor",
   "messageCursor", "chunkCursor"}

ReplyAttemptAfterRetire(owner, source, attempt) ==
  IF attempt.owner = owner /\ attempt.source = source
  THEN ReplyAttemptWithoutTicket(attempt)
  ELSE attempt

ReplyAttemptAfterReconnectTransform(
    oldAttempt, routedAttempt, attempt) ==
  IF attempt = oldAttempt
  THEN routedAttempt
  ELSE IF attempt.owner = oldAttempt.owner
            /\ attempt.source = oldAttempt.source
       THEN ReplyAttemptWithoutTicket(attempt)
       ELSE attempt

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

THEOREM ReplySourceServiceRankPrimeIntroduction ==
  \A owner, semantic, source, messageCursor, chunkCursor, distance:
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
    /\ distance \in ReplyDistanceCarrier
    /\ ReplySourceRoundRobinRank(owner, semantic, source)' = distance
    => ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, distance)'
BY Isa DEF ReplySourceServiceRank

ReplySourceLowerServiceRank(owner, semantic, source,
                            messageCursor, chunkCursor, distance) ==
  \E lower \in SetLessThan(
                 distance, ReplyDistanceOrdering, ReplyDistanceCarrier):
    ReplySourceServiceRank(
      owner, semantic, source, messageCursor, chunkCursor, lower)

THEOREM ReplySourceLowerServiceRankPrimeIntroduction ==
  \A owner, semantic, source, messageCursor, chunkCursor, distance:
    (\E lower \in SetLessThan(
       distance, ReplyDistanceOrdering, ReplyDistanceCarrier):
       /\ ReplySourceReadyAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
       /\ lower \in ReplyDistanceCarrier
       /\ ReplySourceRoundRobinRank(
            owner, semantic, source)' = lower)
      => ReplySourceLowerServiceRank(
           owner, semantic, source,
           messageCursor, chunkCursor, distance)'
BY Isa
   DEF ReplySourceLowerServiceRank, ReplySourceServiceRank

THEOREM ReplyAttemptSetHasCanonicalDomain ==
  \A attempt \in ReplyAttemptSet:
    DOMAIN attempt = ReplyAttemptFieldDomain
BY Zenon
   DEF ReplyAttemptSet, ReplyAttemptFieldDomain

THEOREM ReplyAttemptSetMembersAreFunctions ==
  \A attempt \in ReplyAttemptSet:
    attempt =
      [field \in DOMAIN attempt |-> attempt[field]]
BY SMTT(30) DEF ReplyAttemptSet

THEOREM ReplyAttemptConstructorTyped ==
  \A owner \in ReplyOwners,
     source \in ReplySources,
     semantic \in ReplySemantics,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures,
     retiredDeliveryOrdinal \in 0..ReplyDeliveryOrdinalLimit,
     retiredConnectionTenure \in 0..ReplyDeliveryOrdinalLimit,
     ticketTenure \in 0..ReplyDeliveryOrdinalLimit,
     ticketSemantic \in SUBSET ReplySemantics,
     ticketTarget \in SUBSET ReplyTargets,
     ticketMessageCursor \in SUBSET (0..ReplyMessageCount),
     ticketChunkCursor \in SUBSET (0..ReplyChunkCount),
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyAttempt(
      owner, source, semantic, deliveryOrdinal, connectionTenure,
      retiredDeliveryOrdinal, retiredConnectionTenure,
      ticketTenure, ticketSemantic, ticketTarget,
      ticketMessageCursor, ticketChunkCursor,
      messageCursor, chunkCursor) \in ReplyAttemptSet
BY SMTT(30) DEF ReplyAttempt, ReplyAttemptSet

THEOREM ReplyZeroCursorAttemptTyped ==
  \A owner \in ReplyOwners,
     source \in ReplySources,
     semantic \in ReplySemantics,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    ReplyRouteConfiguration =>
      ReplyAttempt(
        owner, source, semantic, deliveryOrdinal, connectionTenure,
        0, 0, NoReplyTicketTenure, {}, {}, {}, {}, 0, 0)
        \in ReplyAttemptSet
BY ReplyAttemptConstructorTyped, SMTT(10)
   DEF ReplyRouteConfiguration, NoReplyTicketTenure

THEOREM ReplyCanonicalFunctionReconstruction ==
  \A attempt:
    /\ DOMAIN attempt = ReplyAttemptFieldDomain
    /\ attempt = [field \in DOMAIN attempt |-> attempt[field]]
    => attempt =
      ReplyAttempt(
        attempt.owner, attempt.source, attempt.semantic,
        attempt.deliveryOrdinal, attempt.connectionTenure,
        attempt.retiredDeliveryOrdinal,
        attempt.retiredConnectionTenure,
        attempt.ticketTenure, attempt.ticketSemantic,
        attempt.ticketTarget, attempt.ticketMessageCursor,
        attempt.ticketChunkCursor,
        attempt.messageCursor, attempt.chunkCursor)
PROOF
  <1>1. ASSUME NEW attempt,
                DOMAIN attempt = ReplyAttemptFieldDomain,
                attempt =
                  [field \in DOMAIN attempt |-> attempt[field]]
         PROVE attempt =
                 ReplyAttempt(
                   attempt.owner, attempt.source, attempt.semantic,
                   attempt.deliveryOrdinal, attempt.connectionTenure,
                   attempt.retiredDeliveryOrdinal,
                   attempt.retiredConnectionTenure,
                   attempt.ticketTenure, attempt.ticketSemantic,
                   attempt.ticketTarget, attempt.ticketMessageCursor,
                   attempt.ticketChunkCursor,
                   attempt.messageCursor, attempt.chunkCursor)
    <2>0. DOMAIN attempt = ReplyAttemptFieldDomain
      BY <1>1
    <2>1. DOMAIN ReplyAttempt(
                  attempt.owner, attempt.source, attempt.semantic,
                  attempt.deliveryOrdinal, attempt.connectionTenure,
                  attempt.retiredDeliveryOrdinal,
                  attempt.retiredConnectionTenure,
                  attempt.ticketTenure, attempt.ticketSemantic,
                  attempt.ticketTarget, attempt.ticketMessageCursor,
                  attempt.ticketChunkCursor,
                  attempt.messageCursor, attempt.chunkCursor) =
                DOMAIN attempt
      BY <2>0, Zenon
         DEF ReplyAttempt, ReplyAttemptFieldDomain
    <2>2. \A field \in DOMAIN attempt:
             attempt[field] =
               ReplyAttempt(
                 attempt.owner, attempt.source, attempt.semantic,
                 attempt.deliveryOrdinal,
                 attempt.connectionTenure,
                 attempt.retiredDeliveryOrdinal,
                 attempt.retiredConnectionTenure,
                 attempt.ticketTenure, attempt.ticketSemantic,
                 attempt.ticketTarget,
                 attempt.ticketMessageCursor,
                 attempt.ticketChunkCursor,
                 attempt.messageCursor,
                 attempt.chunkCursor)[field]
      <3>1. ASSUME NEW field \in DOMAIN attempt
             PROVE attempt[field] =
                     ReplyAttempt(
                       attempt.owner, attempt.source, attempt.semantic,
                       attempt.deliveryOrdinal,
                       attempt.connectionTenure,
                       attempt.retiredDeliveryOrdinal,
                       attempt.retiredConnectionTenure,
                       attempt.ticketTenure, attempt.ticketSemantic,
                       attempt.ticketTarget,
                       attempt.ticketMessageCursor,
                       attempt.ticketChunkCursor,
                       attempt.messageCursor,
                       attempt.chunkCursor)[field]
        <4>1. field \in ReplyAttemptFieldDomain
          BY <2>0, <3>1
        <4>2. CASE field = "owner"
          BY <4>2 DEF ReplyAttempt
        <4>3. CASE field = "source"
          BY <4>3 DEF ReplyAttempt
        <4>4. CASE field = "semantic"
          BY <4>4 DEF ReplyAttempt
        <4>5. CASE field = "deliveryOrdinal"
          BY <4>5 DEF ReplyAttempt
        <4>6. CASE field = "connectionTenure"
          BY <4>6 DEF ReplyAttempt
        <4>7. CASE field = "retiredDeliveryOrdinal"
          BY <4>7 DEF ReplyAttempt
        <4>8. CASE field = "retiredConnectionTenure"
          BY <4>8 DEF ReplyAttempt
        <4>9. CASE field = "ticketTenure"
          BY <4>9 DEF ReplyAttempt
        <4>10. CASE field = "ticketSemantic"
          BY <4>10 DEF ReplyAttempt
        <4>11. CASE field = "ticketTarget"
          BY <4>11 DEF ReplyAttempt
        <4>12. CASE field = "ticketMessageCursor"
          BY <4>12 DEF ReplyAttempt
        <4>13. CASE field = "ticketChunkCursor"
          BY <4>13 DEF ReplyAttempt
        <4>14. CASE field = "messageCursor"
          BY <4>14 DEF ReplyAttempt
        <4>15. CASE field = "chunkCursor"
          BY <4>15 DEF ReplyAttempt
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5,
                     <4>6, <4>7, <4>8, <4>9, <4>10,
                     <4>11, <4>12, <4>13, <4>14, <4>15,
                     Zenon
             DEF ReplyAttemptFieldDomain
      <3> QED BY <3>1
    <2>3. attempt =
             [field \in DOMAIN attempt |-> attempt[field]]
      BY <1>1
    <2>4. ReplyAttempt(
              attempt.owner, attempt.source, attempt.semantic,
              attempt.deliveryOrdinal, attempt.connectionTenure,
              attempt.retiredDeliveryOrdinal,
              attempt.retiredConnectionTenure,
              attempt.ticketTenure, attempt.ticketSemantic,
              attempt.ticketTarget, attempt.ticketMessageCursor,
              attempt.ticketChunkCursor,
              attempt.messageCursor, attempt.chunkCursor) =
            [field \in DOMAIN attempt |-> attempt[field]]
      BY <2>1, <2>2, SMTT(30) DEF ReplyAttempt
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyAttemptCanonicalReconstruction ==
  \A attempt \in ReplyAttemptSet:
    attempt =
      ReplyAttempt(
        attempt.owner, attempt.source, attempt.semantic,
        attempt.deliveryOrdinal, attempt.connectionTenure,
        attempt.retiredDeliveryOrdinal,
        attempt.retiredConnectionTenure,
        attempt.ticketTenure, attempt.ticketSemantic,
        attempt.ticketTarget, attempt.ticketMessageCursor,
        attempt.ticketChunkCursor,
        attempt.messageCursor, attempt.chunkCursor)
BY ReplyAttemptSetHasCanonicalDomain,
   ReplyAttemptSetMembersAreFunctions,
   ReplyCanonicalFunctionReconstruction

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
PROOF
  <1>1. ASSUME ReplyRouteConfiguration
         PROVE \A source \in ReplySources:
                 \A left, right \in ReplySourceIndices(source):
                   left = right
    <2>1. ASSUME NEW source \in ReplySources
           PROVE \A left, right \in ReplySourceIndices(source):
                   left = right
      <3>1. ASSUME NEW left \in ReplySourceIndices(source),
                    NEW right \in ReplySourceIndices(source)
             PROVE left = right
        <4> QED BY <1>1, <3>1, Isa
             DEF ReplyRouteConfiguration, ReplySourceIndices
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1

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
        BY <3>1 DEF ReplySourceIndex
      <3> QED BY <3>2 DEF ReplySourceIndices
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplySourceIndexInjective ==
  ReplyRouteConfiguration =>
    \A left, right \in ReplySources:
      ReplySourceIndex(left) = ReplySourceIndex(right) => left = right
PROOF
  <1>1. ASSUME ReplyRouteConfiguration
         PROVE \A left, right \in ReplySources:
                 ReplySourceIndex(left) = ReplySourceIndex(right)
                   => left = right
    <2>1. ASSUME NEW left \in ReplySources,
                  NEW right \in ReplySources,
                  ReplySourceIndex(left) = ReplySourceIndex(right)
           PROVE left = right
      <3>1. /\ ReplySourceOrder[ReplySourceIndex(left)] = left
              /\ ReplySourceOrder[ReplySourceIndex(right)] = right
        BY <1>1, <2>1, ReplySourceIndexTyped
      <3> QED BY <2>1, <3>1
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplySourceCyclicDistanceBounded ==
  ReplyRouteConfiguration =>
    \A start, candidate \in 1..Len(ReplySourceOrder):
      ReplySourceCyclicDistance(start, candidate)
        \in ReplyDistanceCarrier
BY SMTT(30)
   DEF ReplyRouteConfiguration, ReplySourceCyclicDistance,
       ReplyDistanceCarrier

THEOREM ReplyNextSourceIndexTyped ==
  ReplyRouteConfiguration =>
    \A index \in 1..Len(ReplySourceOrder):
      NextReplySourceIndex(index) \in 1..Len(ReplySourceOrder)
BY SMTT(20)
   DEF ReplyRouteConfiguration, NextReplySourceIndex

THEOREM ReplyAdvancePastEarlierSourceLowersDistance ==
  ReplyRouteConfiguration =>
    \A start, earlier, later \in 1..Len(ReplySourceOrder):
      /\ earlier # later
      /\ ReplySourceCyclicDistance(start, earlier)
           <= ReplySourceCyclicDistance(start, later)
      => ReplySourceCyclicDistance(
           NextReplySourceIndex(earlier), later)
           < ReplySourceCyclicDistance(start, later)
BY SMTT(30)
   DEF ReplyRouteConfiguration, ReplySourceCyclicDistance,
       NextReplySourceIndex

THEOREM ReplySourceRoundRobinRankTyped ==
  ReplyRouteInductiveInvariant =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
      ReplySourceRoundRobinRank(owner, semantic, source)
        \in ReplyDistanceCarrier
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant
         PROVE \A owner \in ReplyOwners, semantic \in ReplySemantics,
                  source \in ReplySources:
                 ReplySourceRoundRobinRank(owner, semantic, source)
                   \in ReplyDistanceCarrier
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE ReplySourceRoundRobinRank(owner, semantic, source)
                   \in ReplyDistanceCarrier
      <3>1. rrNextServiceIndex[owner][semantic]
               \in 1..Len(ReplySourceOrder)
        BY <1>1, <2>1, SMTT(30)
           DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3>2. ReplySourceIndex(source) \in 1..Len(ReplySourceOrder)
        BY <1>1, <2>1, ReplySourceIndexTyped
           DEF ReplyRouteInductiveInvariant
      <3> QED BY <1>1, <2>1, <3>1, <3>2,
                    ReplySourceCyclicDistanceBounded
           DEF ReplyRouteInductiveInvariant, ReplySourceRoundRobinRank
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplySourceRoundRobinRankTypedPrime ==
  ReplyRouteInductiveInvariant' =>
    \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
      ReplySourceRoundRobinRank(owner, semantic, source)'
        \in ReplyDistanceCarrier
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant'
         PROVE \A owner \in ReplyOwners, semantic \in ReplySemantics,
                  source \in ReplySources:
                 ReplySourceRoundRobinRank(owner, semantic, source)'
                   \in ReplyDistanceCarrier
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE ReplySourceRoundRobinRank(
                   owner, semantic, source)'
                   \in ReplyDistanceCarrier
      <3>1. rrNextServiceIndex'[owner][semantic]
               \in 1..Len(ReplySourceOrder)
        BY <1>1, <2>1, SMTT(30)
           DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3>2. ReplySourceIndex(source)
               \in 1..Len(ReplySourceOrder)
        BY <1>1, <2>1, ReplySourceIndexTyped
           DEF ReplyRouteInductiveInvariant
      <3> QED BY <1>1, <2>1, <3>1, <3>2,
                    ReplySourceCyclicDistanceBounded
           DEF ReplyRouteInductiveInvariant,
               ReplySourceRoundRobinRank
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyDistanceOrderingWellFounded ==
  IsWellFoundedOn(ReplyDistanceOrdering, ReplyDistanceCarrier)
PROOF
  <1>1. ReplyDistanceCarrier \subseteq Nat
    BY Isa DEF ReplyDistanceCarrier
  <1>2. IsWellFoundedOn(OpToRel(<, Nat), ReplyDistanceCarrier)
    BY <1>1, NatLessThanWellFounded, IsWellFoundedOnSubset
  <1> QED BY <1>2 DEF ReplyDistanceOrdering

THEOREM ReplySelectedSourceIndexExists ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteInductiveInvariant
    /\ ReplyPendingSourceIndices(owner, semantic) # {}
    => \E index \in ReplyPendingSourceIndices(owner, semantic):
         \A other \in ReplyPendingSourceIndices(owner, semantic):
           ReplySourceCyclicDistance(
             rrNextServiceIndex[owner][semantic], index)
             <= ReplySourceCyclicDistance(
                  rrNextServiceIndex[owner][semantic], other)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                ReplyRouteInductiveInvariant,
                ReplyPendingSourceIndices(owner, semantic) # {}
         PROVE \E index \in
                    ReplyPendingSourceIndices(owner, semantic):
                 \A other \in
                      ReplyPendingSourceIndices(owner, semantic):
                   ReplySourceCyclicDistance(
                     rrNextServiceIndex[owner][semantic], index)
                     <= ReplySourceCyclicDistance(
                          rrNextServiceIndex[owner][semantic], other)
    <2>1. /\ ReplyRouteConfiguration
           /\ rrNextServiceIndex[owner][semantic]
                \in 1..Len(ReplySourceOrder)
      BY <1>1, SMTT(10)
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>2. ReplyPendingSourceIndices(owner, semantic)
               \subseteq 1..Len(ReplySourceOrder)
      BY SMTT(10) DEF ReplyPendingSourceIndices
    <2>3. \A index \in
                 ReplyPendingSourceIndices(owner, semantic):
               ReplySourceCyclicDistance(
                 rrNextServiceIndex[owner][semantic], index)
                 \in ReplyDistanceCarrier
      BY <2>1, <2>2, ReplySourceCyclicDistanceBounded
    <2>4. LET pending ==
                 ReplyPendingSourceIndices(owner, semantic)
               start == rrNextServiceIndex[owner][semantic]
               distances ==
                 {ReplySourceCyclicDistance(start, index):
                    index \in pending}
           IN /\ distances \subseteq ReplyDistanceCarrier
              /\ distances # {}
      BY <1>1, <2>3, SMTT(20)
    <2>5. LET pending ==
                 ReplyPendingSourceIndices(owner, semantic)
               start == rrNextServiceIndex[owner][semantic]
               distances ==
                 {ReplySourceCyclicDistance(start, index):
                    index \in pending}
           IN \E minimum \in distances:
                \A other \in distances:
                  ~<<other, minimum>> \in ReplyDistanceOrdering
      BY <2>4, ReplyDistanceOrderingWellFounded, WFMin
    <2> QED BY <2>3, <2>5, SMTT(30)
         DEF ReplyDistanceOrdering, OpToRel,
             ReplyDistanceCarrier
  <1> QED BY <1>1

THEOREM ReplySelectedSourceDistanceMinimal ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteInductiveInvariant
    /\ ReplyPendingSourceIndices(owner, semantic) # {}
    => \A other \in ReplyPendingSourceIndices(owner, semantic):
         ReplySourceCyclicDistance(
           rrNextServiceIndex[owner][semantic],
           ReplySelectedSourceIndex(owner, semantic))
           <= ReplySourceCyclicDistance(
                rrNextServiceIndex[owner][semantic], other)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                ReplyRouteInductiveInvariant,
                ReplyPendingSourceIndices(owner, semantic) # {}
         PROVE \A other \in
                    ReplyPendingSourceIndices(owner, semantic):
                 ReplySourceCyclicDistance(
                   rrNextServiceIndex[owner][semantic],
                   ReplySelectedSourceIndex(owner, semantic))
                   <= ReplySourceCyclicDistance(
                        rrNextServiceIndex[owner][semantic], other)
    <2>1. \E index \in
                 ReplyPendingSourceIndices(owner, semantic):
               \A other \in
                    ReplyPendingSourceIndices(owner, semantic):
                 ReplySourceCyclicDistance(
                   rrNextServiceIndex[owner][semantic], index)
                   <= ReplySourceCyclicDistance(
                        rrNextServiceIndex[owner][semantic], other)
      BY <1>1, ReplySelectedSourceIndexExists
    <2> QED BY <2>1, Zenon DEF ReplySelectedSourceIndex
  <1> QED BY <1>1

THEOREM ReplyConfiguredSourcesFinite ==
  ReplyRouteConfiguration => IsFiniteSet(ReplySources)
PROOF
  <1>1. ASSUME ReplyRouteConfiguration
         PROVE IsFiniteSet(ReplySources)
    <2>1. Len(ReplySourceOrder) \in Nat
      BY <1>1, Isa DEF ReplyRouteConfiguration
    <2>2. IsFiniteSet(1..Len(ReplySourceOrder))
      BY <2>1, FS_Interval, SMTT(5)
    <2>3. IsFiniteSet(
             {ReplySourceOrder[index]:
                index \in 1..Len(ReplySourceOrder)})
      BY <2>2, FS_Image
    <2> QED BY <2>3 DEF ReplySources
  <1> QED BY <1>1

THEOREM ReplyTypeBoundsSourceGeometry ==
  /\ ReplyRouteConfiguration
  /\ ReplyRouteTypeInvariant
  => \A owner \in ReplyOwners, semantic \in ReplySemantics:
       /\ Cardinality(ReplyAttemptSources(owner, semantic))
            <= ReplySourceCapacity
       /\ Cardinality(
            ReplyRetiredDeliverySources(owner, semantic))
            <= ReplySourceCapacity
PROOF
  <1>1. ASSUME ReplyRouteConfiguration,
                ReplyRouteTypeInvariant
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics:
                 /\ Cardinality(
                      ReplyAttemptSources(owner, semantic))
                      <= ReplySourceCapacity
                 /\ Cardinality(
                      ReplyRetiredDeliverySources(owner, semantic))
                      <= ReplySourceCapacity
    <2>1. IsFiniteSet(ReplySources)
      BY <1>1, ReplyConfiguredSourcesFinite
    <2>2. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics
           PROVE /\ Cardinality(
                        ReplyAttemptSources(owner, semantic))
                        <= ReplySourceCapacity
                 /\ Cardinality(
                        ReplyRetiredDeliverySources(owner, semantic))
                        <= ReplySourceCapacity
      <3>1. ReplyAttemptSources(owner, semantic)
                 \subseteq ReplySources
        BY <1>1, SMTT(20)
           DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyAttemptSources, ReplyAttemptsFor
      <3>2. ReplyRetiredDeliverySources(owner, semantic)
                 \subseteq ReplySources
        BY <1>1, SMTT(20)
           DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyRetiredDeliverySources, ReplyAttemptsFor
      <3>3. /\ Cardinality(
                   ReplyAttemptSources(owner, semantic))
                   <= Cardinality(ReplySources)
             /\ Cardinality(
                   ReplyRetiredDeliverySources(owner, semantic))
                   <= Cardinality(ReplySources)
        BY <2>1, <3>1, <3>2, FS_Subset
      <3> QED BY <1>1, <3>3
           DEF ReplyRouteConfiguration
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM ReplyNextTypeBoundsSourceGeometry ==
  /\ ReplyRouteConfiguration'
  /\ ReplyRouteTypeInvariant'
  => \A owner \in ReplyOwners, semantic \in ReplySemantics:
       /\ Cardinality(ReplyAttemptSources(owner, semantic))'
            <= ReplySourceCapacity
       /\ Cardinality(
            ReplyRetiredDeliverySources(owner, semantic))'
            <= ReplySourceCapacity
PROOF
  <1>1. ASSUME ReplyRouteConfiguration',
                ReplyRouteTypeInvariant'
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics:
                 /\ Cardinality(
                      ReplyAttemptSources(owner, semantic))'
                      <= ReplySourceCapacity
                 /\ Cardinality(
                      ReplyRetiredDeliverySources(owner, semantic))'
                      <= ReplySourceCapacity
    <2>1. ReplyRouteConfiguration
      BY <1>1 DEF ReplyRouteConfiguration
    <2>2. IsFiniteSet(ReplySources)
      BY <2>1, ReplyConfiguredSourcesFinite
    <2>3. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics
           PROVE /\ Cardinality(
                        ReplyAttemptSources(owner, semantic))'
                        <= ReplySourceCapacity
                 /\ Cardinality(
                        ReplyRetiredDeliverySources(owner, semantic))'
                        <= ReplySourceCapacity
      <3>1. ReplyAttemptSources(owner, semantic)'
                 \subseteq ReplySources
        BY <1>1, SMTT(20)
           DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyAttemptSources, ReplyAttemptsFor
      <3>2. ReplyRetiredDeliverySources(owner, semantic)'
                 \subseteq ReplySources
        BY <1>1, SMTT(20)
           DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyRetiredDeliverySources, ReplyAttemptsFor
      <3>3. /\ Cardinality(
                   ReplyAttemptSources(owner, semantic))'
                   <= Cardinality(ReplySources)
             /\ Cardinality(
                   ReplyRetiredDeliverySources(owner, semantic))'
                   <= Cardinality(ReplySources)
        BY <2>2, <3>1, <3>2, FS_Subset
      <3> QED BY <2>1, <3>3
           DEF ReplyRouteConfiguration
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ReplyFunctionalUpdateAtKey ==
  \A domain, codomain, mapping, key, value:
    /\ mapping \in [domain -> codomain]
    /\ key \in domain
    => [mapping EXCEPT ![key] = value][key] = value
BY Isa

THEOREM ReplyFunctionalUpdateAwayFromKey ==
  \A domain, codomain, mapping, key, value, other:
    /\ mapping \in [domain -> codomain]
    /\ key \in domain
    /\ other \in domain
    /\ other # key
    => [mapping EXCEPT ![key] = value][other] = mapping[other]
BY Isa

THEOREM ReplyFunctionalUpdatePreservesType ==
  \A domain, codomain, mapping, key, value:
    /\ mapping \in [domain -> codomain]
    /\ key \in domain
    /\ value \in codomain
    => [mapping EXCEPT ![key] = value] \in [domain -> codomain]
BY Isa

THEOREM ReplyNestedFunctionalUpdatePreservesType ==
  ASSUME NEW outerDomain,
         NEW innerDomain,
         NEW codomain,
         NEW mapping,
         NEW outerKey,
         NEW innerKey,
         NEW value,
         mapping \in [outerDomain -> [innerDomain -> codomain]],
         outerKey \in outerDomain,
         innerKey \in innerDomain,
         value \in codomain
  PROVE [mapping EXCEPT ![outerKey][innerKey] = value]
          \in [outerDomain -> [innerDomain -> codomain]]
PROOF
  <1>1. mapping[outerKey] \in [innerDomain -> codomain]
    BY Isa
  <1>2. [mapping[outerKey] EXCEPT ![innerKey] = value]
           \in [innerDomain -> codomain]
    BY <1>1, ReplyFunctionalUpdatePreservesType
  <1>3. [mapping EXCEPT
           ![outerKey] =
             [mapping[outerKey] EXCEPT ![innerKey] = value]]
           \in [outerDomain -> [innerDomain -> codomain]]
    BY <1>2, ReplyFunctionalUpdatePreservesType
  <1> QED BY <1>3

THEOREM ReplyNestedFunctionalUpdateAtKey ==
  \A outerDomain, innerDomain, codomain, mapping,
     outerKey, innerKey, value:
    /\ mapping \in [outerDomain -> [innerDomain -> codomain]]
    /\ outerKey \in outerDomain
    /\ innerKey \in innerDomain
    => [mapping EXCEPT ![outerKey][innerKey] = value]
         [outerKey][innerKey] = value
PROOF
  <1>1. ASSUME NEW outerDomain,
                NEW innerDomain,
                NEW codomain,
                NEW mapping,
                NEW outerKey,
                NEW innerKey,
                NEW value,
                mapping \in
                  [outerDomain -> [innerDomain -> codomain]],
                outerKey \in outerDomain,
                innerKey \in innerDomain
         PROVE [mapping EXCEPT
                  ![outerKey][innerKey] = value]
                 [outerKey][innerKey] = value
    <2>1. [mapping EXCEPT
             ![outerKey] =
               [mapping[outerKey] EXCEPT ![innerKey] = value]]
             [outerKey] =
               [mapping[outerKey] EXCEPT ![innerKey] = value]
      BY <1>1, ReplyFunctionalUpdateAtKey
    <2>2. mapping[outerKey] \in [innerDomain -> codomain]
      BY <1>1, Isa
    <2>3. [mapping[outerKey] EXCEPT ![innerKey] = value]
             [innerKey] = value
      BY <1>1, <2>2, ReplyFunctionalUpdateAtKey
    <2> QED BY <2>1, <2>3
  <1> QED BY <1>1

THEOREM ReplyNestedFunctionalUpdateAwayFromKey ==
  \A outerDomain, innerDomain, codomain, mapping,
     outerKey, innerKey, value, queryOuter, queryInner:
    /\ mapping \in [outerDomain -> [innerDomain -> codomain]]
    /\ outerKey \in outerDomain
    /\ innerKey \in innerDomain
    /\ queryOuter \in outerDomain
    /\ queryInner \in innerDomain
    /\ (queryOuter # outerKey \/ queryInner # innerKey)
    => [mapping EXCEPT ![outerKey][innerKey] = value]
         [queryOuter][queryInner] =
         mapping[queryOuter][queryInner]
PROOF
  <1>1. ASSUME NEW outerDomain,
                NEW innerDomain,
                NEW codomain,
                NEW mapping,
                NEW outerKey,
                NEW innerKey,
                NEW value,
                NEW queryOuter,
                NEW queryInner,
                mapping \in
                  [outerDomain -> [innerDomain -> codomain]],
                outerKey \in outerDomain,
                innerKey \in innerDomain,
                queryOuter \in outerDomain,
                queryInner \in innerDomain,
                queryOuter # outerKey \/ queryInner # innerKey
         PROVE [mapping EXCEPT
                  ![outerKey][innerKey] = value]
                 [queryOuter][queryInner] =
                 mapping[queryOuter][queryInner]
    <2>1. CASE queryOuter # outerKey
      BY <1>1, ReplyFunctionalUpdateAwayFromKey
    <2>2. CASE queryOuter = outerKey
      <3>1. queryInner # innerKey
        BY <1>1, <2>2
      <3> QED BY <1>1, <2>2, <3>1,
           ReplyFunctionalUpdateAtKey,
           ReplyFunctionalUpdateAwayFromKey
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyNestedFunctionalUpdateIdentity ==
  \A outerDomain, innerDomain, codomain, mapping,
     outerKey, innerKey:
    /\ mapping \in [outerDomain -> [innerDomain -> codomain]]
    /\ outerKey \in outerDomain
    /\ innerKey \in innerDomain
    => [mapping EXCEPT
          ![outerKey][innerKey] = mapping[outerKey][innerKey]]
         = mapping
BY Isa

THEOREM ReplyDeliveryOrdinalBumpPreservesMetadata ==
  \A bumpOwner \in ReplyOwners, attempt \in rrAttempts:
    /\ ReplyRouteSafetyInvariant
    /\ rrNextDeliveryOrdinal' =
         [rrNextDeliveryOrdinal EXCEPT ![bumpOwner] = @ + 1]
    /\ rrConnectionTenure' = rrConnectionTenure
    /\ rrSourceActive' = rrSourceActive
    => /\ attempt.deliveryOrdinal <
             rrNextDeliveryOrdinal'[attempt.owner]
       /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
       /\ IF attempt.ticketTenure = NoReplyTicketTenure
          THEN ReplyAttemptHasNoTicket(attempt)
          ELSE ReplyTicketValidForAttempt(attempt)'
PROOF
  <1>1. ASSUME NEW bumpOwner \in ReplyOwners,
                NEW attempt \in rrAttempts,
                ReplyRouteSafetyInvariant,
                rrNextDeliveryOrdinal' =
                  [rrNextDeliveryOrdinal EXCEPT
                     ![bumpOwner] = @ + 1],
                rrConnectionTenure' = rrConnectionTenure,
                rrSourceActive' = rrSourceActive
         PROVE /\ attempt.deliveryOrdinal <
                    rrNextDeliveryOrdinal'[attempt.owner]
               /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
               /\ IF attempt.ticketTenure = NoReplyTicketTenure
                  THEN ReplyAttemptHasNoTicket(attempt)
                  ELSE ReplyTicketValidForAttempt(attempt)'
    <2>1. /\ attempt.owner \in ReplyOwners
           /\ attempt.deliveryOrdinal \in Nat
           /\ rrNextDeliveryOrdinal[attempt.owner] \in Nat
           /\ rrNextDeliveryOrdinal
                \in [ReplyOwners ->
                      1..(ReplyDeliveryOrdinalLimit + 1)]
      BY <1>1, SMTT(10)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant, ReplyAttemptSet,
             ReplyDeliveryOrdinals
    <2>2. /\ attempt.deliveryOrdinal <
                 rrNextDeliveryOrdinal[attempt.owner]
           /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
           /\ IF attempt.ticketTenure = NoReplyTicketTenure
              THEN ReplyAttemptHasNoTicket(attempt)
              ELSE ReplyTicketValidForAttempt(attempt)
      BY <1>1 DEF ReplyRouteSafetyInvariant,
           ReplyRouteOwnershipInvariant
    <2>3. /\ rrNextDeliveryOrdinal'[attempt.owner] \in Nat
           /\ rrNextDeliveryOrdinal'[attempt.owner] >=
                rrNextDeliveryOrdinal[attempt.owner]
      <3>1. CASE attempt.owner = bumpOwner
        BY <1>1, <2>1, <3>1,
           ReplyFunctionalUpdateAtKey, SMTT(5)
      <3>2. CASE attempt.owner # bumpOwner
        BY <1>1, <2>1, <3>2,
           ReplyFunctionalUpdateAwayFromKey
      <3> QED BY <3>1, <3>2
    <2>4. attempt.deliveryOrdinal <
             rrNextDeliveryOrdinal'[attempt.owner]
      BY <2>1, <2>2, <2>3, SMTT(5)
    <2>5. ReplyTicketValidForAttempt(attempt) =>
             ReplyTicketValidForAttempt(attempt)'
      BY <1>1, SMTT(10)
         DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent
    <2>6. CASE attempt.ticketTenure = NoReplyTicketTenure
      <3>1. ReplyAttemptHasNoTicket(attempt)
        BY <2>2, <2>6
      <3> QED BY <2>2, <2>4, <2>6, <3>1
    <2>7. CASE attempt.ticketTenure # NoReplyTicketTenure
      <3>1. ReplyTicketValidForAttempt(attempt)
        BY <2>2, <2>7
      <3>2. ReplyTicketValidForAttempt(attempt)'
        BY <2>5, <3>1
      <3> QED BY <2>2, <2>4, <2>7, <3>2
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

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
      /\ capability.sourceCapacity = ReplySourceCapacity
      /\ rrSourceActive[owner][source]
      /\ ReplyCapabilityHasKnownOrdinalCollision(capability)
      => ReplyCapabilityRejection(capability, owner, source, semantic) =
           "EqualOrdinalDifferentTenure"
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                NEW semantic \in ReplySemantics,
                NEW capability,
                ReplyCapabilityIntrinsicBindingValid(capability),
                capability.owner = owner,
                capability.source = source,
                capability.target = ReplySemanticTarget(semantic),
                capability.semantic = semantic,
                capability.sourceCapacity = ReplySourceCapacity,
                rrSourceActive[owner][source],
                ReplyCapabilityHasKnownOrdinalCollision(capability)
         PROVE ReplyCapabilityRejection(
                 capability, owner, source, semantic) =
                 "EqualOrdinalDifferentTenure"
    <2> QED BY <1>1, SMTT(30) DEF ReplyCapabilityRejection
  <1> QED BY <1>1

THEOREM ReplyRouteUpdateRecordsLatestRetiredDelivery ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    deliveryOrdinal > attempt.deliveryOrdinal =>
      LET routed ==
            ReplyAttemptWithRoute(
              attempt, deliveryOrdinal, connectionTenure)
      IN /\ routed.deliveryOrdinal = deliveryOrdinal
         /\ routed.retiredDeliveryOrdinal = attempt.deliveryOrdinal
         /\ routed.retiredConnectionTenure = attempt.connectionTenure
         /\ ReplyAttemptCursor(routed) = ReplyAttemptCursor(attempt)
BY SMTT(60)
   DEF ReplyAttemptWithRoute, ReplyAttemptCursor,
       ReplyAttemptSet

THEOREM ReplySameTenureRefreshPreservesTicketState ==
  \A attempt \in ReplyAttemptSet:
    \A deliveryOrdinal:
      LET routed ==
            ReplyAttemptWithRoute(
              attempt, deliveryOrdinal, attempt.connectionTenure)
      IN /\ routed.owner = attempt.owner
         /\ routed.source = attempt.source
         /\ routed.semantic = attempt.semantic
         /\ routed.ticketTenure = attempt.ticketTenure
         /\ routed.ticketSemantic = attempt.ticketSemantic
         /\ routed.ticketTarget = attempt.ticketTarget
         /\ routed.ticketMessageCursor = attempt.ticketMessageCursor
         /\ routed.ticketChunkCursor = attempt.ticketChunkCursor
         /\ routed.connectionTenure = attempt.connectionTenure
         /\ routed.messageCursor = attempt.messageCursor
         /\ routed.chunkCursor = attempt.chunkCursor
         /\ ReplyAttemptCursor(routed) = ReplyAttemptCursor(attempt)
BY SMTT(60)
   DEF ReplyAttemptWithRoute, ReplyAttemptCursor, ReplyAttemptSet

THEOREM ReplySameTenureRefreshPreservesValidTicket ==
  \A attempt \in ReplyAttemptSet:
    \A deliveryOrdinal:
      /\ ReplyTicketValidForAttempt(attempt)
      /\ rrConnectionTenure' = rrConnectionTenure
      /\ rrSourceActive' = rrSourceActive
      => ReplyTicketValidForAttempt(
           ReplyAttemptWithRoute(
             attempt, deliveryOrdinal, attempt.connectionTenure))'
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                NEW deliveryOrdinal,
                ReplyTicketValidForAttempt(attempt),
                rrConnectionTenure' = rrConnectionTenure,
                rrSourceActive' = rrSourceActive
         PROVE ReplyTicketValidForAttempt(
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal,
                   attempt.connectionTenure))'
    <2>1. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal,
                   attempt.connectionTenure)
           IN /\ routed.owner = attempt.owner
              /\ routed.source = attempt.source
              /\ routed.semantic = attempt.semantic
              /\ routed.ticketTenure = attempt.ticketTenure
              /\ routed.ticketSemantic = attempt.ticketSemantic
              /\ routed.ticketTarget = attempt.ticketTarget
              /\ routed.ticketMessageCursor =
                   attempt.ticketMessageCursor
              /\ routed.ticketChunkCursor =
                   attempt.ticketChunkCursor
              /\ routed.connectionTenure =
                   attempt.connectionTenure
              /\ routed.messageCursor = attempt.messageCursor
              /\ routed.chunkCursor = attempt.chunkCursor
      BY <1>1, ReplySameTenureRefreshPreservesTicketState
    <2>2. ReplyAttemptCurrent(
             ReplyAttemptWithRoute(
               attempt, deliveryOrdinal,
               attempt.connectionTenure))'
      BY <1>1, <2>1, SMTT(10)
         DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent
    <2>3. ReplyAttemptWithRoute(
             attempt, deliveryOrdinal,
             attempt.connectionTenure).ticketTenure =
             ReplyAttemptWithRoute(
               attempt, deliveryOrdinal,
               attempt.connectionTenure).connectionTenure
      BY <1>1, <2>1
         DEF ReplyTicketValidForAttempt
    <2>4. ReplyTicketForAttempt(
             ReplyAttemptWithRoute(
               attempt, deliveryOrdinal,
               attempt.connectionTenure)) =
             ReplyTicket(
               ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal,
                 attempt.connectionTenure).owner,
               ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal,
                 attempt.connectionTenure).source,
               ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal,
                 attempt.connectionTenure).semantic,
               ReplySemanticTarget(
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal,
                   attempt.connectionTenure).semantic),
               rrConnectionTenure'[
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal,
                   attempt.connectionTenure).owner][
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal,
                   attempt.connectionTenure).source],
               ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal,
                 attempt.connectionTenure).messageCursor,
               ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal,
                 attempt.connectionTenure).chunkCursor)
      BY <1>1, <2>1, SMTT(20)
         DEF ReplyTicketValidForAttempt,
             ReplyTicketForAttempt, ReplyTicket
    <2> QED BY <2>2, <2>3, <2>4
         DEF ReplyTicketValidForAttempt
  <1> QED BY <1>1

THEOREM ReplyDifferentTenureRefreshClearsTicket ==
  \A attempt \in ReplyAttemptSet:
    \A deliveryOrdinal, connectionTenure:
      connectionTenure # attempt.connectionTenure =>
        LET routed ==
              ReplyAttemptWithRoute(
                attempt, deliveryOrdinal, connectionTenure)
        IN /\ routed.ticketTenure = NoReplyTicketTenure
           /\ ReplyAttemptHasNoTicket(routed)
BY SMTT(60)
   DEF ReplyAttemptWithRoute, ReplyAttemptHasNoTicket,
       ReplyAttemptSet, NoReplyTicketTenure

THEOREM ReplyRouteRefreshProducesFunction ==
  \A attempt \in ReplyAttemptSet:
    \A deliveryOrdinal, connectionTenure:
      LET routed ==
            ReplyAttemptWithRoute(
              attempt, deliveryOrdinal, connectionTenure)
      IN routed =
           [field \in DOMAIN routed |-> routed[field]]
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                NEW deliveryOrdinal,
                NEW connectionTenure
         PROVE LET routed ==
                     ReplyAttemptWithRoute(
                       attempt, deliveryOrdinal, connectionTenure)
               IN routed =
                    [field \in DOMAIN routed |-> routed[field]]
    <2>1. attempt =
             [field \in DOMAIN attempt |-> attempt[field]]
      BY <1>1, ReplyAttemptSetMembersAreFunctions
    <2>2. CASE connectionTenure = attempt.connectionTenure
      BY <2>1, <2>2, SMTT(15) DEF ReplyAttemptWithRoute
    <2>3. CASE connectionTenure # attempt.connectionTenure
      BY <2>1, <2>3, SMTT(15) DEF ReplyAttemptWithRoute
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyTicketRemovalProducesFunction ==
  \A attempt \in ReplyAttemptSet:
    LET cleared == ReplyAttemptWithoutTicket(attempt)
    IN cleared =
         [field \in DOMAIN cleared |-> cleared[field]]
BY ReplyAttemptSetMembersAreFunctions, SMTT(15)
   DEF ReplyAttemptWithoutTicket

THEOREM ReplyTicketAcquisitionProducesFunction ==
  \A attempt \in ReplyAttemptSet:
    LET ticketed == ReplyAttemptWithTicket(attempt)
    IN ticketed =
         [field \in DOMAIN ticketed |-> ticketed[field]]
BY ReplyAttemptSetMembersAreFunctions, SMTT(15)
   DEF ReplyAttemptWithTicket

THEOREM ReplyServiceProducesFunction ==
  \A attempt \in ReplyAttemptSet:
    LET serviced == ReplyAttemptAfterService(attempt)
    IN serviced =
         [field \in DOMAIN serviced |-> serviced[field]]
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet
         PROVE LET serviced == ReplyAttemptAfterService(attempt)
               IN serviced =
                    [field \in DOMAIN serviced |-> serviced[field]]
    <2>1. attempt =
             [field \in DOMAIN attempt |-> attempt[field]]
      BY <1>1, ReplyAttemptSetMembersAreFunctions
    <2>2. CASE attempt.messageCursor < ReplyMessageCount
      BY <2>1, <2>2, SMTT(15) DEF ReplyAttemptAfterService
    <2>3. CASE ~(attempt.messageCursor < ReplyMessageCount)
      BY <2>1, <2>3, SMTT(15) DEF ReplyAttemptAfterService
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyRouteRefreshPreservesAttemptType ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    ReplyRouteConfiguration =>
      ReplyAttemptWithRoute(
        attempt, deliveryOrdinal, connectionTenure) \in ReplyAttemptSet
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                NEW deliveryOrdinal \in ReplyDeliveryOrdinals,
                NEW connectionTenure \in ReplyConnectionTenures,
                ReplyRouteConfiguration
         PROVE ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure)
                 \in ReplyAttemptSet
    <2>1. /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).owner
                 \in ReplyOwners
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).source
                 \in ReplySources
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).semantic
                 \in ReplySemantics
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).messageCursor
                 \in 0..ReplyMessageCount
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).chunkCursor
                 \in 0..ReplyChunkCount
      BY <1>1, SMTT(15)
         DEF ReplyAttemptWithRoute, ReplyAttemptSet
    <2>2. /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).deliveryOrdinal
                 \in ReplyDeliveryOrdinals
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).connectionTenure
                 \in ReplyConnectionTenures
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure)
                   .retiredDeliveryOrdinal
                 \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure)
                   .retiredConnectionTenure
                 \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(20)
         DEF ReplyAttemptWithRoute, ReplyAttemptSet,
             ReplyDeliveryOrdinals, ReplyConnectionTenures
    <2>3. /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).ticketTenure
                 \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).ticketSemantic
                 \in SUBSET ReplySemantics
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure).ticketTarget
                 \in SUBSET ReplyTargets
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure)
                   .ticketMessageCursor
                 \in SUBSET (0..ReplyMessageCount)
           /\ ReplyAttemptWithRoute(
                 attempt, deliveryOrdinal, connectionTenure)
                   .ticketChunkCursor
                 \in SUBSET (0..ReplyChunkCount)
      <3>1. CASE connectionTenure = attempt.connectionTenure
        BY <1>1, <3>1, SMTT(10)
           DEF ReplyAttemptWithRoute, ReplyAttemptSet
      <3>2. CASE connectionTenure # attempt.connectionTenure
        <4>1. 0 \in 0..ReplyDeliveryOrdinalLimit
          BY <1>1, SMTT(5) DEF ReplyRouteConfiguration
        <4> QED BY <1>1, <3>2, <4>1, SMTT(10)
             DEF ReplyAttemptWithRoute, ReplyAttemptSet,
                 NoReplyTicketTenure
      <3> QED BY <3>1, <3>2
    <2>4. DOMAIN ReplyAttemptWithRoute(
                  attempt, deliveryOrdinal, connectionTenure) =
                ReplyAttemptFieldDomain
      <3>1. CASE connectionTenure = attempt.connectionTenure
        BY <1>1, <3>1, ReplyAttemptSetHasCanonicalDomain,
           Isa DEF ReplyAttemptWithRoute
      <3>2. CASE connectionTenure # attempt.connectionTenure
        BY <1>1, <3>2, ReplyAttemptSetHasCanonicalDomain,
           Isa DEF ReplyAttemptWithRoute
      <3> QED BY <3>1, <3>2
    <2>5. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal, connectionTenure)
           IN routed =
                [field \in DOMAIN routed |-> routed[field]]
      BY <1>1, ReplyRouteRefreshProducesFunction
    <2>6. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal, connectionTenure)
           IN routed =
                ReplyAttempt(
                  routed.owner, routed.source, routed.semantic,
                  routed.deliveryOrdinal, routed.connectionTenure,
                  routed.retiredDeliveryOrdinal,
                  routed.retiredConnectionTenure,
                  routed.ticketTenure, routed.ticketSemantic,
                  routed.ticketTarget, routed.ticketMessageCursor,
                  routed.ticketChunkCursor,
                  routed.messageCursor, routed.chunkCursor)
      BY <2>4, <2>5, ReplyCanonicalFunctionReconstruction
    <2>7. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal, connectionTenure)
           IN ReplyAttempt(
                routed.owner, routed.source, routed.semantic,
                routed.deliveryOrdinal, routed.connectionTenure,
                routed.retiredDeliveryOrdinal,
                routed.retiredConnectionTenure,
                routed.ticketTenure, routed.ticketSemantic,
                routed.ticketTarget, routed.ticketMessageCursor,
                routed.ticketChunkCursor,
                routed.messageCursor, routed.chunkCursor)
                \in ReplyAttemptSet
      BY <2>1, <2>2, <2>3, ReplyAttemptConstructorTyped
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM ReplyRouteRefreshPreservesIdentityAndCursor ==
  \A attempt \in ReplyAttemptSet,
     deliveryOrdinal \in ReplyDeliveryOrdinals,
     connectionTenure \in ReplyConnectionTenures:
    deliveryOrdinal > attempt.deliveryOrdinal =>
      LET routed ==
            ReplyAttemptWithRoute(
              attempt, deliveryOrdinal, connectionTenure)
      IN /\ SameReplyAttemptIdentity(attempt, routed)
         /\ ReplyAttemptCursor(routed) = ReplyAttemptCursor(attempt)
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                NEW deliveryOrdinal \in ReplyDeliveryOrdinals,
                NEW connectionTenure \in ReplyConnectionTenures,
                deliveryOrdinal > attempt.deliveryOrdinal
         PROVE LET routed ==
                     ReplyAttemptWithRoute(
                       attempt, deliveryOrdinal, connectionTenure)
               IN /\ SameReplyAttemptIdentity(attempt, routed)
                  /\ ReplyAttemptCursor(routed) =
                       ReplyAttemptCursor(attempt)
    <2>1. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal, connectionTenure)
           IN /\ routed.owner = attempt.owner
              /\ routed.semantic = attempt.semantic
              /\ routed.source = attempt.source
      BY <1>1, SMTT(10)
         DEF ReplyAttemptWithRoute, ReplyAttemptSet
    <2>2. LET routed ==
                 ReplyAttemptWithRoute(
                   attempt, deliveryOrdinal, connectionTenure)
           IN ReplyAttemptCursor(routed) =
                ReplyAttemptCursor(attempt)
      BY <1>1, ReplyRouteUpdateRecordsLatestRetiredDelivery
    <2> QED BY <2>1, <2>2 DEF SameReplyAttemptIdentity
  <1> QED BY <1>1

THEOREM ReplyTicketRemovalPreservesAttemptType ==
  \A attempt:
    /\ ReplyRouteConfiguration
    /\ attempt \in ReplyAttemptSet
    => ReplyAttemptWithoutTicket(attempt) \in ReplyAttemptSet
PROOF
  <1>1. ASSUME NEW attempt,
                ReplyRouteConfiguration,
                attempt \in ReplyAttemptSet
         PROVE ReplyAttemptWithoutTicket(attempt) \in ReplyAttemptSet
    <2>1. /\ ReplyAttemptWithoutTicket(attempt).owner \in ReplyOwners
           /\ ReplyAttemptWithoutTicket(attempt).source \in ReplySources
           /\ ReplyAttemptWithoutTicket(attempt).semantic \in ReplySemantics
           /\ ReplyAttemptWithoutTicket(attempt).deliveryOrdinal
                \in ReplyDeliveryOrdinals
           /\ ReplyAttemptWithoutTicket(attempt).connectionTenure
                \in ReplyConnectionTenures
           /\ ReplyAttemptWithoutTicket(attempt).retiredDeliveryOrdinal
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithoutTicket(attempt).retiredConnectionTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithoutTicket(attempt).messageCursor
                \in 0..ReplyMessageCount
           /\ ReplyAttemptWithoutTicket(attempt).chunkCursor
                \in 0..ReplyChunkCount
      BY <1>1, SMTT(20)
         DEF ReplyAttemptWithoutTicket, ReplyAttemptSet
    <2>2. 0 \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(5) DEF ReplyRouteConfiguration
    <2>3. /\ ReplyAttemptWithoutTicket(attempt).ticketTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithoutTicket(attempt).ticketSemantic
                \in SUBSET ReplySemantics
           /\ ReplyAttemptWithoutTicket(attempt).ticketTarget
                \in SUBSET ReplyTargets
           /\ ReplyAttemptWithoutTicket(attempt).ticketMessageCursor
                \in SUBSET (0..ReplyMessageCount)
           /\ ReplyAttemptWithoutTicket(attempt).ticketChunkCursor
                \in SUBSET (0..ReplyChunkCount)
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyAttemptWithoutTicket, ReplyAttemptSet,
             NoReplyTicketTenure
    <2>4. DOMAIN ReplyAttemptWithoutTicket(attempt) =
                ReplyAttemptFieldDomain
      BY <1>1, ReplyAttemptSetHasCanonicalDomain,
         Isa DEF ReplyAttemptWithoutTicket
    <2>5. LET cleared == ReplyAttemptWithoutTicket(attempt)
           IN cleared =
                [field \in DOMAIN cleared |-> cleared[field]]
      BY <1>1, ReplyTicketRemovalProducesFunction
    <2>6. LET cleared == ReplyAttemptWithoutTicket(attempt)
           IN cleared =
                ReplyAttempt(
                  cleared.owner, cleared.source, cleared.semantic,
                  cleared.deliveryOrdinal, cleared.connectionTenure,
                  cleared.retiredDeliveryOrdinal,
                  cleared.retiredConnectionTenure,
                  cleared.ticketTenure, cleared.ticketSemantic,
                  cleared.ticketTarget, cleared.ticketMessageCursor,
                  cleared.ticketChunkCursor,
                  cleared.messageCursor, cleared.chunkCursor)
      BY <2>4, <2>5, ReplyCanonicalFunctionReconstruction
    <2>7. LET cleared == ReplyAttemptWithoutTicket(attempt)
           IN ReplyAttempt(
                cleared.owner, cleared.source, cleared.semantic,
                cleared.deliveryOrdinal, cleared.connectionTenure,
                cleared.retiredDeliveryOrdinal,
                cleared.retiredConnectionTenure,
                cleared.ticketTenure, cleared.ticketSemantic,
                cleared.ticketTarget, cleared.ticketMessageCursor,
                cleared.ticketChunkCursor,
                cleared.messageCursor, cleared.chunkCursor)
                \in ReplyAttemptSet
      BY <2>1, <2>3, ReplyAttemptConstructorTyped
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM ReplyTicketRemovalPreservesIdentityAndCursor ==
  \A attempt \in ReplyAttemptSet:
    /\ SameReplyAttemptIdentity(
         attempt, ReplyAttemptWithoutTicket(attempt))
    /\ ReplyAttemptCursor(ReplyAttemptWithoutTicket(attempt)) =
         ReplyAttemptCursor(attempt)
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet
         PROVE /\ SameReplyAttemptIdentity(
                    attempt, ReplyAttemptWithoutTicket(attempt))
               /\ ReplyAttemptCursor(
                    ReplyAttemptWithoutTicket(attempt)) =
                    ReplyAttemptCursor(attempt)
    <2>1. /\ ReplyAttemptWithoutTicket(attempt).owner =
                attempt.owner
           /\ ReplyAttemptWithoutTicket(attempt).semantic =
                attempt.semantic
           /\ ReplyAttemptWithoutTicket(attempt).source =
                attempt.source
      BY <1>1, SMTT(10)
         DEF ReplyAttemptWithoutTicket, ReplyAttemptSet
    <2>2. /\ ReplyAttemptWithoutTicket(attempt).messageCursor =
                attempt.messageCursor
           /\ ReplyAttemptWithoutTicket(attempt).chunkCursor =
                attempt.chunkCursor
      BY <1>1, SMTT(10)
         DEF ReplyAttemptWithoutTicket, ReplyAttemptSet
    <2> QED BY <2>1, <2>2
         DEF SameReplyAttemptIdentity, ReplyAttemptCursor
  <1> QED BY <1>1

THEOREM ReplyRetireTransformTypedAndIdentity ==
  \A owner \in ReplyOwners, source \in ReplySources,
     attempt \in ReplyAttemptSet:
    /\ ReplyRouteConfiguration
    => LET retired ==
             ReplyAttemptAfterRetire(owner, source, attempt)
       IN /\ retired \in ReplyAttemptSet
          /\ SameReplyAttemptIdentity(attempt, retired)
          /\ ReplyAttemptCursor(retired) =
               ReplyAttemptCursor(attempt)
          /\ retired.deliveryOrdinal = attempt.deliveryOrdinal
          /\ retired.connectionTenure = attempt.connectionTenure
          /\ retired.retiredDeliveryOrdinal =
               attempt.retiredDeliveryOrdinal
          /\ retired.retiredConnectionTenure =
               attempt.retiredConnectionTenure
          /\ IF attempt.owner = owner /\ attempt.source = source
             THEN ReplyAttemptHasNoTicket(retired)
             ELSE retired = attempt
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                NEW attempt \in ReplyAttemptSet,
                ReplyRouteConfiguration
         PROVE LET retired ==
                     ReplyAttemptAfterRetire(owner, source, attempt)
               IN /\ retired \in ReplyAttemptSet
                  /\ SameReplyAttemptIdentity(attempt, retired)
                  /\ ReplyAttemptCursor(retired) =
                       ReplyAttemptCursor(attempt)
                  /\ retired.deliveryOrdinal =
                       attempt.deliveryOrdinal
                  /\ retired.connectionTenure =
                       attempt.connectionTenure
                  /\ retired.retiredDeliveryOrdinal =
                       attempt.retiredDeliveryOrdinal
                  /\ retired.retiredConnectionTenure =
                       attempt.retiredConnectionTenure
                  /\ IF attempt.owner = owner
                           /\ attempt.source = source
                     THEN ReplyAttemptHasNoTicket(retired)
                     ELSE retired = attempt
    <2>1. CASE attempt.owner = owner /\ attempt.source = source
      <3>1. LET retired ==
                   ReplyAttemptAfterRetire(owner, source, attempt)
             IN /\ retired \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(attempt, retired)
                /\ ReplyAttemptCursor(retired) =
                     ReplyAttemptCursor(attempt)
        BY <1>1, <2>1,
           ReplyTicketRemovalPreservesAttemptType,
           ReplyTicketRemovalPreservesIdentityAndCursor
           DEF ReplyAttemptAfterRetire
      <3>2. LET retired ==
                   ReplyAttemptAfterRetire(owner, source, attempt)
             IN /\ retired.deliveryOrdinal =
                       attempt.deliveryOrdinal
                /\ retired.connectionTenure =
                     attempt.connectionTenure
                /\ retired.retiredDeliveryOrdinal =
                     attempt.retiredDeliveryOrdinal
                /\ retired.retiredConnectionTenure =
                     attempt.retiredConnectionTenure
                /\ ReplyAttemptHasNoTicket(retired)
        BY <1>1, <2>1, SMTT(15)
           DEF ReplyAttemptAfterRetire,
               ReplyAttemptWithoutTicket,
               ReplyAttemptHasNoTicket,
               ReplyAttemptSet, NoReplyTicketTenure
      <3> QED BY <2>1, <3>1, <3>2
    <2>2. CASE ~(attempt.owner = owner /\ attempt.source = source)
      BY <1>1, <2>2 DEF ReplyAttemptAfterRetire,
           SameReplyAttemptIdentity
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyTicketAcquisitionPreservesAttemptType ==
  \A attempt:
    /\ ReplyRouteConfiguration
    /\ attempt \in ReplyAttemptSet
    => ReplyAttemptWithTicket(attempt) \in ReplyAttemptSet
PROOF
  <1>1. ASSUME NEW attempt,
                ReplyRouteConfiguration,
                attempt \in ReplyAttemptSet
         PROVE ReplyAttemptWithTicket(attempt) \in ReplyAttemptSet
    <2>1. /\ ReplyAttemptWithTicket(attempt).owner \in ReplyOwners
           /\ ReplyAttemptWithTicket(attempt).source \in ReplySources
           /\ ReplyAttemptWithTicket(attempt).semantic \in ReplySemantics
           /\ ReplyAttemptWithTicket(attempt).deliveryOrdinal
                \in ReplyDeliveryOrdinals
           /\ ReplyAttemptWithTicket(attempt).connectionTenure
                \in ReplyConnectionTenures
           /\ ReplyAttemptWithTicket(attempt).retiredDeliveryOrdinal
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithTicket(attempt).retiredConnectionTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithTicket(attempt).messageCursor
                \in 0..ReplyMessageCount
           /\ ReplyAttemptWithTicket(attempt).chunkCursor
                \in 0..ReplyChunkCount
      BY <1>1, SMTT(20)
         DEF ReplyAttemptWithTicket, ReplyAttemptSet
    <2>2. /\ ReplySemanticTarget(attempt.semantic) \in ReplyTargets
           /\ attempt.connectionTenure
                \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(10)
         DEF ReplyRouteConfiguration, ReplyAttemptSet,
             ReplyConnectionTenures
    <2>3. /\ ReplyAttemptWithTicket(attempt).ticketTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptWithTicket(attempt).ticketSemantic
                \in SUBSET ReplySemantics
           /\ ReplyAttemptWithTicket(attempt).ticketTarget
                \in SUBSET ReplyTargets
           /\ ReplyAttemptWithTicket(attempt).ticketMessageCursor
                \in SUBSET (0..ReplyMessageCount)
           /\ ReplyAttemptWithTicket(attempt).ticketChunkCursor
                \in SUBSET (0..ReplyChunkCount)
      BY <1>1, <2>2, SMTT(15)
         DEF ReplyAttemptWithTicket,
             ReplyAttemptSet
    <2>4. DOMAIN ReplyAttemptWithTicket(attempt) =
                ReplyAttemptFieldDomain
      BY <1>1, ReplyAttemptSetHasCanonicalDomain,
         Isa DEF ReplyAttemptWithTicket
    <2>5. LET ticketed == ReplyAttemptWithTicket(attempt)
           IN ticketed =
                [field \in DOMAIN ticketed |-> ticketed[field]]
      BY <1>1, ReplyTicketAcquisitionProducesFunction
    <2>6. LET ticketed == ReplyAttemptWithTicket(attempt)
           IN ticketed =
                ReplyAttempt(
                  ticketed.owner, ticketed.source, ticketed.semantic,
                  ticketed.deliveryOrdinal, ticketed.connectionTenure,
                  ticketed.retiredDeliveryOrdinal,
                  ticketed.retiredConnectionTenure,
                  ticketed.ticketTenure, ticketed.ticketSemantic,
                  ticketed.ticketTarget,
                  ticketed.ticketMessageCursor,
                  ticketed.ticketChunkCursor,
                  ticketed.messageCursor, ticketed.chunkCursor)
      BY <2>4, <2>5, ReplyCanonicalFunctionReconstruction
    <2>7. LET ticketed == ReplyAttemptWithTicket(attempt)
           IN ReplyAttempt(
                ticketed.owner, ticketed.source, ticketed.semantic,
                ticketed.deliveryOrdinal, ticketed.connectionTenure,
                ticketed.retiredDeliveryOrdinal,
                ticketed.retiredConnectionTenure,
                ticketed.ticketTenure, ticketed.ticketSemantic,
                ticketed.ticketTarget, ticketed.ticketMessageCursor,
                ticketed.ticketChunkCursor,
                ticketed.messageCursor, ticketed.chunkCursor)
                \in ReplyAttemptSet
      BY <2>1, <2>3, ReplyAttemptConstructorTyped
    <2> QED BY <2>6, <2>7
  <1> QED BY <1>1

THEOREM ReplyTicketAcquisitionPreservesIdentityAndCursor ==
  \A attempt \in ReplyAttemptSet:
    /\ SameReplyAttemptIdentity(
         attempt, ReplyAttemptWithTicket(attempt))
    /\ ReplyAttemptCursor(ReplyAttemptWithTicket(attempt)) =
         ReplyAttemptCursor(attempt)
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet
         PROVE /\ SameReplyAttemptIdentity(
                    attempt, ReplyAttemptWithTicket(attempt))
               /\ ReplyAttemptCursor(
                    ReplyAttemptWithTicket(attempt)) =
                    ReplyAttemptCursor(attempt)
    <2>1. /\ ReplyAttemptWithTicket(attempt).owner =
                attempt.owner
           /\ ReplyAttemptWithTicket(attempt).semantic =
                attempt.semantic
           /\ ReplyAttemptWithTicket(attempt).source =
                attempt.source
      BY <1>1, SMTT(10)
         DEF ReplyAttemptWithTicket, ReplyAttemptSet
    <2>2. /\ ReplyAttemptWithTicket(attempt).messageCursor =
                attempt.messageCursor
           /\ ReplyAttemptWithTicket(attempt).chunkCursor =
                attempt.chunkCursor
      BY <1>1, SMTT(10)
         DEF ReplyAttemptWithTicket, ReplyAttemptSet
    <2> QED BY <2>1, <2>2
         DEF SameReplyAttemptIdentity, ReplyAttemptCursor
  <1> QED BY <1>1

THEOREM ReplyServicePreservesAttemptType ==
  \A attempt:
    /\ ReplyRouteConfiguration
    /\ attempt \in ReplyAttemptSet
    /\ ~ReplyAttemptComplete(attempt)
    => ReplyAttemptAfterService(attempt) \in ReplyAttemptSet
PROOF
  <1>1. ASSUME NEW attempt,
                ReplyRouteConfiguration,
                attempt \in ReplyAttemptSet,
                ~ReplyAttemptComplete(attempt)
         PROVE ReplyAttemptAfterService(attempt) \in ReplyAttemptSet
    <2>1. /\ ReplyAttemptAfterService(attempt).owner \in ReplyOwners
           /\ ReplyAttemptAfterService(attempt).source \in ReplySources
           /\ ReplyAttemptAfterService(attempt).semantic
                \in ReplySemantics
           /\ ReplyAttemptAfterService(attempt).deliveryOrdinal
                \in ReplyDeliveryOrdinals
           /\ ReplyAttemptAfterService(attempt).connectionTenure
                \in ReplyConnectionTenures
           /\ ReplyAttemptAfterService(attempt).retiredDeliveryOrdinal
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptAfterService(attempt).retiredConnectionTenure
                \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(20)
         DEF ReplyAttemptAfterService, ReplyAttemptSet
    <2>2. /\ ReplyAttemptAfterService(attempt).messageCursor
                \in 0..ReplyMessageCount
           /\ ReplyAttemptAfterService(attempt).chunkCursor
                \in 0..ReplyChunkCount
      BY <1>1, SMTT(30)
         DEF ReplyRouteConfiguration, ReplyAttemptAfterService,
             ReplyAttemptComplete, ReplyAttemptSet
    <2>3. 0 \in 0..ReplyDeliveryOrdinalLimit
      BY <1>1, SMTT(5) DEF ReplyRouteConfiguration
    <2>4. /\ ReplyAttemptAfterService(attempt).ticketTenure
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyAttemptAfterService(attempt).ticketSemantic
                \in SUBSET ReplySemantics
           /\ ReplyAttemptAfterService(attempt).ticketTarget
                \in SUBSET ReplyTargets
           /\ ReplyAttemptAfterService(attempt).ticketMessageCursor
                \in SUBSET (0..ReplyMessageCount)
           /\ ReplyAttemptAfterService(attempt).ticketChunkCursor
                \in SUBSET (0..ReplyChunkCount)
      BY <1>1, <2>3, SMTT(10)
         DEF ReplyAttemptAfterService, ReplyAttemptSet,
             NoReplyTicketTenure
    <2>5. DOMAIN ReplyAttemptAfterService(attempt) =
                ReplyAttemptFieldDomain
      <3>1. CASE attempt.messageCursor < ReplyMessageCount
        BY <1>1, <3>1, ReplyAttemptSetHasCanonicalDomain,
           Zenon DEF ReplyAttemptAfterService
      <3>2. CASE ~(attempt.messageCursor < ReplyMessageCount)
        BY <1>1, <3>2, ReplyAttemptSetHasCanonicalDomain,
           Zenon DEF ReplyAttemptAfterService
      <3> QED BY <3>1, <3>2
    <2>6. LET serviced == ReplyAttemptAfterService(attempt)
           IN serviced =
                [field \in DOMAIN serviced |-> serviced[field]]
      BY <1>1, ReplyServiceProducesFunction
    <2>7. LET serviced == ReplyAttemptAfterService(attempt)
           IN serviced =
                ReplyAttempt(
                  serviced.owner, serviced.source, serviced.semantic,
                  serviced.deliveryOrdinal, serviced.connectionTenure,
                  serviced.retiredDeliveryOrdinal,
                  serviced.retiredConnectionTenure,
                  serviced.ticketTenure, serviced.ticketSemantic,
                  serviced.ticketTarget,
                  serviced.ticketMessageCursor,
                  serviced.ticketChunkCursor,
                  serviced.messageCursor, serviced.chunkCursor)
      BY <2>5, <2>6, ReplyCanonicalFunctionReconstruction
    <2>8. LET serviced == ReplyAttemptAfterService(attempt)
           IN ReplyAttempt(
                serviced.owner, serviced.source, serviced.semantic,
                serviced.deliveryOrdinal, serviced.connectionTenure,
                serviced.retiredDeliveryOrdinal,
                serviced.retiredConnectionTenure,
                serviced.ticketTenure, serviced.ticketSemantic,
                serviced.ticketTarget, serviced.ticketMessageCursor,
                serviced.ticketChunkCursor,
                serviced.messageCursor, serviced.chunkCursor)
                \in ReplyAttemptSet
      BY <2>1, <2>2, <2>4, ReplyAttemptConstructorTyped
    <2> QED BY <2>7, <2>8
  <1> QED BY <1>1

THEOREM ReplyServicePreservesIdentity ==
  \A attempt \in ReplyAttemptSet:
    SameReplyAttemptIdentity(
      attempt, ReplyAttemptAfterService(attempt))
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet
         PROVE SameReplyAttemptIdentity(
                 attempt, ReplyAttemptAfterService(attempt))
    <2>1. /\ ReplyAttemptAfterService(attempt).owner =
                attempt.owner
           /\ ReplyAttemptAfterService(attempt).semantic =
                attempt.semantic
           /\ ReplyAttemptAfterService(attempt).source =
                attempt.source
      BY <1>1, SMTT(15)
         DEF ReplyAttemptAfterService, ReplyAttemptSet
    <2> QED BY <2>1 DEF SameReplyAttemptIdentity
  <1> QED BY <1>1

THEOREM ReplyServiceProducesReplayValid ==
  \A attempt \in ReplyAttemptSet:
    ~ReplyAttemptComplete(attempt) =>
      ReplyAttemptReplayValid(
        attempt, ReplyAttemptAfterService(attempt))
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                ~ReplyAttemptComplete(attempt)
         PROVE ReplyAttemptReplayValid(
                 attempt, ReplyAttemptAfterService(attempt))
    <2>1. SameReplyAttemptIdentity(
             attempt, ReplyAttemptAfterService(attempt))
      BY <1>1, ReplyServicePreservesIdentity
    <2>2. /\ attempt.deliveryOrdinal \in Nat
           /\ attempt.messageCursor \in Nat
           /\ attempt.chunkCursor \in Nat
      BY <1>1, SMTT(10)
         DEF ReplyAttemptSet, ReplyDeliveryOrdinals
    <2>3. CASE attempt.messageCursor < ReplyMessageCount
      <3>1. /\ ReplyAttemptAfterService(attempt).deliveryOrdinal =
                     attempt.deliveryOrdinal
             /\ ReplyAttemptAfterService(attempt).connectionTenure =
                     attempt.connectionTenure
             /\ ReplyAttemptAfterService(attempt).messageCursor =
                     attempt.messageCursor + 1
             /\ ReplyAttemptAfterService(attempt).chunkCursor =
                     attempt.chunkCursor
        BY <1>1, <2>3, SMTT(15)
           DEF ReplyAttemptAfterService, ReplyAttemptSet
      <3> QED BY <2>1, <2>2, <3>1, SMTT(5)
           DEF ReplyAttemptReplayValid,
               ReplyAttemptCursor,
               SameReplyAttemptIdentity
    <2>4. CASE ~(attempt.messageCursor < ReplyMessageCount)
      <3>1. /\ ReplyAttemptAfterService(attempt).deliveryOrdinal =
                     attempt.deliveryOrdinal
             /\ ReplyAttemptAfterService(attempt).connectionTenure =
                     attempt.connectionTenure
             /\ ReplyAttemptAfterService(attempt).messageCursor =
                     attempt.messageCursor
             /\ ReplyAttemptAfterService(attempt).chunkCursor =
                     attempt.chunkCursor + 1
        BY <1>1, <2>4, SMTT(15)
           DEF ReplyAttemptAfterService, ReplyAttemptSet
      <3> QED BY <2>1, <2>2, <3>1, SMTT(5)
           DEF ReplyAttemptReplayValid,
               ReplyAttemptCursor,
               SameReplyAttemptIdentity
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyOwnedAttemptSelected ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyAttemptOwned(owner, semantic, source) =>
      ReplyAttemptFor(owner, semantic, source)
        \in ReplyAttemptsForSource(owner, semantic, source)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyAttemptOwned(owner, semantic, source)
         PROVE ReplyAttemptFor(owner, semantic, source)
                 \in ReplyAttemptsForSource(
                      owner, semantic, source)
    <2>1. ReplyAttemptsForSource(
             owner, semantic, source) # {}
      BY <1>1 DEF ReplyAttemptOwned
    <2> QED BY <2>1 DEF ReplyAttemptFor
  <1> QED BY <1>1

THEOREM ReplyOwnedAttemptIdentity ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyAttemptOwned(owner, semantic, source) =>
      LET attempt == ReplyAttemptFor(owner, semantic, source)
      IN /\ attempt \in rrAttempts
         /\ attempt.owner = owner
         /\ attempt.semantic = semantic
         /\ attempt.source = source
BY ReplyOwnedAttemptSelected
   DEF ReplyAttemptsForSource, ReplyAttemptsFor

THEOREM ReplyOwnedAttemptUnique ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyRouteOwnershipInvariant =>
      \A left, right \in ReplyAttemptsForSource(
                            owner, semantic, source):
        left = right
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteOwnershipInvariant
         PROVE \A left, right \in ReplyAttemptsForSource(
                                      owner, semantic, source):
                 left = right
    <2>1. /\ IsFiniteSet(
                 ReplyAttemptsForSource(owner, semantic, source))
           /\ Cardinality(
                 ReplyAttemptsForSource(owner, semantic, source)) <= 1
      BY <1>1 DEF ReplyRouteOwnershipInvariant
    <2>2. ASSUME NEW left \in ReplyAttemptsForSource(
                              owner, semantic, source),
                  NEW right \in ReplyAttemptsForSource(
                               owner, semantic, source)
           PROVE left = right
      <3>1. ReplyAttemptsForSource(
               owner, semantic, source) # {}
        BY <2>2
      <3>2. Cardinality(
               ReplyAttemptsForSource(
                 owner, semantic, source)) = 1
        BY <2>1, <3>1, FS_EmptySet, FS_CardinalityType,
           SMTT(10)
      <3>3. \E only:
               ReplyAttemptsForSource(
                 owner, semantic, source) = {only}
        BY <2>1, <3>2, FS_Singleton
      <3> QED BY <2>2, <3>3
    <2> QED BY <2>2
  <1> QED BY <1>1

THEOREM ReplySameOwnedAttemptIdentityUnique ==
  \A left, right:
    /\ ReplyRouteSafetyInvariant
    /\ left \in rrAttempts
    /\ right \in rrAttempts
    /\ SameReplyAttemptIdentity(left, right)
    => left = right
PROOF
  <1>1. ASSUME NEW left,
                NEW right,
                ReplyRouteSafetyInvariant,
                left \in rrAttempts,
                right \in rrAttempts,
                SameReplyAttemptIdentity(left, right)
         PROVE left = right
    <2>1. /\ left.owner \in ReplyOwners
           /\ left.semantic \in ReplySemantics
           /\ left.source \in ReplySources
      BY <1>1, SMTT(30)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant, ReplyAttemptSet
    <2>2. /\ left \in ReplyAttemptsForSource(
                    left.owner, left.semantic, left.source)
           /\ right \in ReplyAttemptsForSource(
                    left.owner, left.semantic, left.source)
      BY <1>1 DEF SameReplyAttemptIdentity,
           ReplyAttemptsForSource, ReplyAttemptsFor
    <2> QED BY <1>1, <2>1, <2>2, ReplyOwnedAttemptUnique
         DEF ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ReplySameOwnedAttemptIdentityUniquePrime ==
  \A left, right:
    /\ ReplyRouteSafetyInvariant'
    /\ left \in rrAttempts'
    /\ right \in rrAttempts'
    /\ SameReplyAttemptIdentity(left, right)
    => left = right
PROOF
  <1>1. ASSUME NEW left,
                NEW right,
                ReplyRouteSafetyInvariant',
                left \in rrAttempts',
                right \in rrAttempts',
                SameReplyAttemptIdentity(left, right)
         PROVE left = right
    <2>1. /\ left.owner \in ReplyOwners
           /\ left.semantic \in ReplySemantics
           /\ left.source \in ReplySources
      BY <1>1, SMTT(30)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant, ReplyAttemptSet
    <2>2. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
           IN /\ IsFiniteSet(attempts)
              /\ Cardinality(attempts) <= 1
              /\ left \in attempts
              /\ right \in attempts
      BY <1>1, <2>1
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant,
             ReplyAttemptsForSource, ReplyAttemptsFor,
             SameReplyAttemptIdentity
    <2>3. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
           IN Cardinality(attempts) = 1
      BY <2>2, FS_CardinalityType, FS_EmptySet, SMTT(10)
    <2>4. LET attempts ==
                    ReplyAttemptsForSource(
                      left.owner, left.semantic, left.source)'
           IN \E only: attempts = {only}
      BY <2>2, <2>3, FS_Singleton
    <2> QED BY <2>2, <2>4
  <1> QED BY <1>1

THEOREM ReplyCursorPreservingIdentityReplacementProvidesSourceIsolation ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteSafetyInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ ReplyAttemptCursor(newAttempt) =
         ReplyAttemptCursor(oldAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    => ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteSafetyInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                ReplyAttemptCursor(newAttempt) =
                  ReplyAttemptCursor(oldAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt)
         PROVE ReplySourceIsolationStep
    <2>1. ReplyAttemptSurvivalStep
      <3>1. ASSUME NEW retainedBefore \in rrAttempts
             PROVE \E retainedAfter \in rrAttempts':
                     SameReplyAttemptIdentity(
                       retainedBefore, retainedAfter)
        <4>1. CASE retainedBefore = oldAttempt
          <5>1. newAttempt \in rrAttempts'
            BY <1>1 DEF ReplaceReplyAttempt
          <5> QED BY <1>1, <4>1, <5>1
        <4>2. CASE retainedBefore # oldAttempt
          <5>1. retainedBefore \in rrAttempts'
            BY <1>1, <3>1, <4>2 DEF ReplaceReplyAttempt
          <5>2. SameReplyAttemptIdentity(
                   retainedBefore, retainedBefore)
            BY DEF SameReplyAttemptIdentity
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF ReplyAttemptSurvivalStep
    <2>2. ReplyOtherCursorIsolationStep
      <3>1. ASSUME NEW changedBefore \in rrAttempts,
                    NEW changedAfter \in rrAttempts'
             PROVE LET sameAttempt ==
                         SameReplyAttemptIdentity(
                           changedBefore, changedAfter)
                       attemptChanged ==
                         ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
                   IN (sameAttempt /\ attemptChanged) =>
                        \A otherBefore \in rrAttempts:
                          (otherBefore.owner = changedBefore.owner
                            /\ ~SameReplyAttemptIdentity(
                                 otherBefore, changedBefore))
                          => \E otherAfter \in rrAttempts':
                               /\ SameReplyAttemptIdentity(
                                    otherBefore, otherAfter)
                               /\ ReplyAttemptCursor(otherAfter) =
                                    ReplyAttemptCursor(otherBefore)
        <4>1. ASSUME SameReplyAttemptIdentity(
                        changedBefore, changedAfter)
                      /\ ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
               PROVE \A otherBefore \in rrAttempts:
                       (otherBefore.owner = changedBefore.owner
                         /\ ~SameReplyAttemptIdentity(
                              otherBefore, changedBefore))
                       => \E otherAfter \in rrAttempts':
                            /\ SameReplyAttemptIdentity(
                                 otherBefore, otherAfter)
                            /\ ReplyAttemptCursor(otherAfter) =
                                 ReplyAttemptCursor(otherBefore)
          <5>1. CASE changedAfter = newAttempt
            <6>1. SameReplyAttemptIdentity(
                     changedBefore, oldAttempt)
              BY <1>1, <4>1, <5>1, Isa
                 DEF SameReplyAttemptIdentity
            <6>2. changedBefore = oldAttempt
              BY <1>1, <3>1, <6>1,
                 ReplySameOwnedAttemptIdentityUnique
            <6>3. FALSE
              BY <1>1, <4>1, <5>1, <6>2
            <6> QED BY <6>3
          <5>2. CASE changedAfter # newAttempt
            <6>1. changedAfter \in rrAttempts
              BY <1>1, <3>1, <5>2
                 DEF ReplaceReplyAttempt
            <6>2. changedBefore = changedAfter
              BY <1>1, <3>1, <4>1, <6>1,
                 ReplySameOwnedAttemptIdentityUnique
            <6>3. FALSE
              BY <4>1, <6>2
            <6> QED BY <6>3
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplyOtherCursorIsolationStep
    <2> QED BY <2>1, <2>2 DEF ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyIdentityReplacementProvidesSourceIsolation ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteSafetyInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    => ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteSafetyInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt)
         PROVE ReplySourceIsolationStep
    <2>1. ReplyAttemptSurvivalStep
      <3>1. ASSUME NEW retainedBefore \in rrAttempts
             PROVE \E retainedAfter \in rrAttempts':
                     SameReplyAttemptIdentity(
                       retainedBefore, retainedAfter)
        <4>1. CASE retainedBefore = oldAttempt
          <5>1. newAttempt \in rrAttempts'
            BY <1>1 DEF ReplaceReplyAttempt
          <5> QED BY <1>1, <4>1, <5>1
        <4>2. CASE retainedBefore # oldAttempt
          <5>1. retainedBefore \in rrAttempts'
            BY <1>1, <3>1, <4>2 DEF ReplaceReplyAttempt
          <5>2. SameReplyAttemptIdentity(
                   retainedBefore, retainedBefore)
            BY DEF SameReplyAttemptIdentity
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF ReplyAttemptSurvivalStep
    <2>2. ReplyOtherCursorIsolationStep
      <3>1. ASSUME NEW changedBefore \in rrAttempts,
                    NEW changedAfter \in rrAttempts'
             PROVE LET sameAttempt ==
                         SameReplyAttemptIdentity(
                           changedBefore, changedAfter)
                       attemptChanged ==
                         ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
                   IN (sameAttempt /\ attemptChanged) =>
                        \A otherBefore \in rrAttempts:
                          (otherBefore.owner = changedBefore.owner
                            /\ ~SameReplyAttemptIdentity(
                                 otherBefore, changedBefore))
                          => \E otherAfter \in rrAttempts':
                               /\ SameReplyAttemptIdentity(
                                    otherBefore, otherAfter)
                               /\ ReplyAttemptCursor(otherAfter) =
                                    ReplyAttemptCursor(otherBefore)
        <4>1. ASSUME SameReplyAttemptIdentity(
                        changedBefore, changedAfter)
                      /\ ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
               PROVE \A otherBefore \in rrAttempts:
                       (otherBefore.owner = changedBefore.owner
                         /\ ~SameReplyAttemptIdentity(
                              otherBefore, changedBefore))
                       => \E otherAfter \in rrAttempts':
                            /\ SameReplyAttemptIdentity(
                                 otherBefore, otherAfter)
                            /\ ReplyAttemptCursor(otherAfter) =
                                 ReplyAttemptCursor(otherBefore)
          <5>1. CASE changedAfter = newAttempt
            <6>1. SameReplyAttemptIdentity(
                     changedBefore, oldAttempt)
              BY <1>1, <4>1, <5>1, Isa
                 DEF SameReplyAttemptIdentity
            <6>2. changedBefore = oldAttempt
              BY <1>1, <3>1, <6>1,
                 ReplySameOwnedAttemptIdentityUnique
            <6>3. ASSUME NEW otherBefore \in rrAttempts,
                          otherBefore.owner = changedBefore.owner
                            /\ ~SameReplyAttemptIdentity(
                                 otherBefore, changedBefore)
                   PROVE \E otherAfter \in rrAttempts':
                           /\ SameReplyAttemptIdentity(
                                otherBefore, otherAfter)
                           /\ ReplyAttemptCursor(otherAfter) =
                                ReplyAttemptCursor(otherBefore)
              <7>1. otherBefore # oldAttempt
                BY <6>2, <6>3
                   DEF SameReplyAttemptIdentity
              <7>2. otherBefore \in rrAttempts'
                BY <1>1, <6>3, <7>1 DEF ReplaceReplyAttempt
              <7>3. /\ SameReplyAttemptIdentity(
                           otherBefore, otherBefore)
                      /\ ReplyAttemptCursor(otherBefore) =
                           ReplyAttemptCursor(otherBefore)
                BY DEF SameReplyAttemptIdentity
              <7> QED BY <7>2, <7>3
            <6> QED BY <6>3
          <5>2. CASE changedAfter # newAttempt
            <6>1. changedAfter \in rrAttempts
              BY <1>1, <3>1, <5>2 DEF ReplaceReplyAttempt
            <6>2. changedBefore = changedAfter
              BY <1>1, <3>1, <4>1, <6>1,
                 ReplySameOwnedAttemptIdentityUnique
            <6>3. FALSE
              BY <4>1, <6>2
            <6> QED BY <6>3
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplyOtherCursorIsolationStep
    <2> QED BY <2>1, <2>2 DEF ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyReplayValidIdentityReplacementProvidesReplayStep ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteTypeInvariant
    /\ oldAttempt \in rrAttempts
    /\ ReplyAttemptReplayValid(oldAttempt, newAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    => ReplyAttemptReplayStep
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteTypeInvariant,
                oldAttempt \in rrAttempts,
                ReplyAttemptReplayValid(oldAttempt, newAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt)
         PROVE ReplyAttemptReplayStep
    <2>1. ASSUME NEW retainedBefore \in rrAttempts
           PROVE \E retainedAfter \in rrAttempts':
                   ReplyAttemptReplayValid(
                     retainedBefore, retainedAfter)
      <3>1. CASE retainedBefore = oldAttempt
        <4>1. newAttempt \in rrAttempts'
          BY <1>1 DEF ReplaceReplyAttempt
        <4> QED BY <1>1, <3>1, <4>1
      <3>2. CASE retainedBefore # oldAttempt
        <4>1. retainedBefore \in rrAttempts'
          BY <1>1, <2>1, <3>2 DEF ReplaceReplyAttempt
        <4>2. /\ retainedBefore.deliveryOrdinal \in Nat
               /\ retainedBefore.messageCursor \in Nat
               /\ retainedBefore.chunkCursor \in Nat
          BY <1>1, <2>1, SMTT(30)
             DEF ReplyRouteTypeInvariant,
                 ReplyAttemptSet, ReplyDeliveryOrdinals
        <4>3. ReplyAttemptReplayValid(
                 retainedBefore, retainedBefore)
          BY <4>2, SMTT(5)
             DEF ReplyAttemptReplayValid, ReplyAttemptCursor
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1 DEF ReplyAttemptReplayStep
  <1> QED BY <1>1

THEOREM ReplySelectedPendingAttemptFacts ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteInductiveInvariant
    /\ ReplyPendingSourceIndices(owner, semantic) # {}
    => LET selectedIndex ==
             ReplySelectedSourceIndex(owner, semantic)
           source == ReplySourceOrder[selectedIndex]
           oldAttempt == ReplyAttemptFor(owner, semantic, source)
       IN /\ selectedIndex \in
                ReplyPendingSourceIndices(owner, semantic)
          /\ selectedIndex \in 1..Len(ReplySourceOrder)
          /\ source \in ReplySources
          /\ ReplyAttemptOwned(owner, semantic, source)
          /\ oldAttempt \in rrAttempts
          /\ oldAttempt \in ReplyAttemptSet
          /\ ReplyAttemptCurrent(oldAttempt)
          /\ ReplyTicketValidForAttempt(oldAttempt)
          /\ ~ReplyAttemptComplete(oldAttempt)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                ReplyRouteInductiveInvariant,
                ReplyPendingSourceIndices(owner, semantic) # {}
         PROVE LET selectedIndex ==
                     ReplySelectedSourceIndex(owner, semantic)
                   source == ReplySourceOrder[selectedIndex]
                   oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN /\ selectedIndex \in
                        ReplyPendingSourceIndices(owner, semantic)
                  /\ selectedIndex \in 1..Len(ReplySourceOrder)
                  /\ source \in ReplySources
                  /\ ReplyAttemptOwned(owner, semantic, source)
                  /\ oldAttempt \in rrAttempts
                  /\ oldAttempt \in ReplyAttemptSet
                  /\ ReplyAttemptCurrent(oldAttempt)
                  /\ ReplyTicketValidForAttempt(oldAttempt)
                  /\ ~ReplyAttemptComplete(oldAttempt)
    <2>1. ReplySelectedSourceIndex(owner, semantic)
               \in ReplyPendingSourceIndices(owner, semantic)
      <3>1. \E index \in
                   ReplyPendingSourceIndices(owner, semantic):
                \A other \in
                     ReplyPendingSourceIndices(owner, semantic):
                  ReplySourceCyclicDistance(
                    rrNextServiceIndex[owner][semantic], index)
                    <= ReplySourceCyclicDistance(
                         rrNextServiceIndex[owner][semantic], other)
        BY <1>1, ReplySelectedSourceIndexExists
      <3> QED BY <3>1, Zenon DEF ReplySelectedSourceIndex
    <2>2. LET selectedIndex ==
                 ReplySelectedSourceIndex(owner, semantic)
               source == ReplySourceOrder[selectedIndex]
               oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ selectedIndex \in 1..Len(ReplySourceOrder)
              /\ ReplyAttemptOwned(owner, semantic, source)
              /\ ReplyTicketValidForAttempt(oldAttempt)
              /\ ~ReplyAttemptComplete(oldAttempt)
      BY <2>1 DEF ReplyPendingSourceIndices
    <2>3. LET selectedIndex ==
                 ReplySelectedSourceIndex(owner, semantic)
               source == ReplySourceOrder[selectedIndex]
           IN source \in ReplySources
      BY <2>2 DEF ReplySources
    <2>4. LET selectedIndex ==
                 ReplySelectedSourceIndex(owner, semantic)
               source == ReplySourceOrder[selectedIndex]
               oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ ReplyAttemptCurrent(oldAttempt)
      BY <1>1, <2>2, <2>3, ReplyOwnedAttemptIdentity
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             ReplyTicketValidForAttempt
    <2> QED BY <2>1, <2>2, <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyAttemptExtensionPreservesSourceOwnership ==
  \A newAttempt:
    /\ ReplyRouteSafetyInvariant
    /\ newAttempt \in ReplyAttemptSet
    /\ ~ReplyAttemptOwned(
         newAttempt.owner, newAttempt.semantic, newAttempt.source)
    /\ rrAttempts' = rrAttempts \cup {newAttempt}
    => \A owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
         /\ IsFiniteSet(
              ReplyAttemptsForSource(owner, semantic, source))'
         /\ Cardinality(
              ReplyAttemptsForSource(owner, semantic, source))' <= 1
PROOF
  <1>1. ASSUME NEW newAttempt,
                ReplyRouteSafetyInvariant,
                newAttempt \in ReplyAttemptSet,
                ~ReplyAttemptOwned(
                  newAttempt.owner, newAttempt.semantic,
                  newAttempt.source),
                rrAttempts' = rrAttempts \cup {newAttempt}
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics,
                  source \in ReplySources:
                 /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))'
                 /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source))' <= 1
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE /\ IsFiniteSet(
                        ReplyAttemptsForSource(
                          owner, semantic, source))'
                 /\ Cardinality(
                        ReplyAttemptsForSource(
                          owner, semantic, source))' <= 1
      <3>1. CASE /\ newAttempt.owner = owner
                  /\ newAttempt.semantic = semantic
                  /\ newAttempt.source = source
        <4>1. ReplyAttemptsForSource(
                 owner, semantic, source) = {}
          BY <1>1, <3>1
             DEF ReplyAttemptOwned
        <4>2. ReplyAttemptsForSource(
                 owner, semantic, source)' = {newAttempt}
          BY <1>1, <3>1, <4>1, SMTT(20)
             DEF ReplyAttemptsForSource, ReplyAttemptsFor
        <4> QED BY <4>2, FS_Singleton, SMTT(5)
      <3>2. CASE ~(/\ newAttempt.owner = owner
                    /\ newAttempt.semantic = semantic
                    /\ newAttempt.source = source)
        <4>1. ReplyAttemptsForSource(
                 owner, semantic, source)' =
                   ReplyAttemptsForSource(owner, semantic, source)
          BY <1>1, <3>2, SMTT(20)
             DEF ReplyAttemptsForSource, ReplyAttemptsFor
        <4>2. /\ IsFiniteSet(
                    ReplyAttemptsForSource(owner, semantic, source))
               /\ Cardinality(
                    ReplyAttemptsForSource(
                      owner, semantic, source)) <= 1
          BY <1>1, <2>1
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyIdentityReplacementPreservesSourceOwnership ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteSafetyInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    => \A owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
         /\ IsFiniteSet(
              ReplyAttemptsForSource(owner, semantic, source))'
         /\ Cardinality(
              ReplyAttemptsForSource(owner, semantic, source))' <= 1
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteSafetyInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt)
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics,
                  source \in ReplySources:
                 /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))'
                 /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source))' <= 1
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE /\ IsFiniteSet(
                        ReplyAttemptsForSource(
                          owner, semantic, source))'
                 /\ Cardinality(
                        ReplyAttemptsForSource(
                          owner, semantic, source))' <= 1
      <3>1. CASE /\ oldAttempt.owner = owner
                  /\ oldAttempt.semantic = semantic
                  /\ oldAttempt.source = source
        <4>1. oldAttempt \in
                 ReplyAttemptsForSource(owner, semantic, source)
          BY <1>1, <3>1
             DEF ReplyAttemptsForSource, ReplyAttemptsFor
        <4>2. \A attempt \in
                   ReplyAttemptsForSource(owner, semantic, source):
                   attempt = oldAttempt
          BY <1>1, <2>1, <4>1, ReplyOwnedAttemptUnique
             DEF ReplyRouteSafetyInvariant
        <4>3. ReplyAttemptsForSource(
                 owner, semantic, source) = {oldAttempt}
          BY <4>1, <4>2, SMTT(10)
        <4>4. /\ newAttempt.owner = owner
               /\ newAttempt.semantic = semantic
               /\ newAttempt.source = source
          BY <1>1, <3>1 DEF SameReplyAttemptIdentity
        <4>5. ReplyAttemptsForSource(
                 owner, semantic, source)' = {newAttempt}
          BY <1>1, <3>1, <4>3, <4>4, SMTT(20)
             DEF ReplaceReplyAttempt,
                 ReplyAttemptsForSource, ReplyAttemptsFor
        <4> QED BY <4>5, FS_Singleton, SMTT(5)
      <3>2. CASE ~(/\ oldAttempt.owner = owner
                    /\ oldAttempt.semantic = semantic
                    /\ oldAttempt.source = source)
        <4>1. ~(/\ newAttempt.owner = owner
                  /\ newAttempt.semantic = semantic
                  /\ newAttempt.source = source)
          BY <1>1, <3>2 DEF SameReplyAttemptIdentity
        <4>2. ReplyAttemptsForSource(
                 owner, semantic, source)' =
                   ReplyAttemptsForSource(owner, semantic, source)
          BY <1>1, <3>2, <4>1, SMTT(20)
             DEF ReplaceReplyAttempt,
                 ReplyAttemptsForSource, ReplyAttemptsFor
        <4>3. /\ IsFiniteSet(
                    ReplyAttemptsForSource(owner, semantic, source))
               /\ Cardinality(
                    ReplyAttemptsForSource(
                      owner, semantic, source)) <= 1
          BY <1>1, <2>1
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyIdentityReplacementPreservesPayloadOwnership ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteOwnershipInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    /\ rrPayloads' = rrPayloads
    => \A owner \in ReplyOwners, semantic \in ReplySemantics:
         ReplyAttemptsFor(owner, semantic)' # {} =>
           semantic \in rrPayloads'
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteOwnershipInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt),
                rrPayloads' = rrPayloads
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics:
                 ReplyAttemptsFor(owner, semantic)' # {} =>
                   semantic \in rrPayloads'
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  ReplyAttemptsFor(owner, semantic)' # {}
           PROVE semantic \in rrPayloads'
      <3>1. ReplyAttemptsFor(owner, semantic) # {}
        BY <1>1, <2>1, SMTT(30)
           DEF ReplaceReplyAttempt, SameReplyAttemptIdentity,
               ReplyAttemptsFor
      <3>2. semantic \in rrPayloads
        BY <1>1, <3>1 DEF ReplyRouteOwnershipInvariant
      <3> QED BY <1>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyIdentityImagePreservesSourceOwnership ==
  ASSUME NEW Transform(_),
         ReplyRouteSafetyInvariant,
         \A attempt \in rrAttempts:
           SameReplyAttemptIdentity(attempt, Transform(attempt)),
         rrAttempts' = {Transform(attempt): attempt \in rrAttempts}
  PROVE \A owner \in ReplyOwners, semantic \in ReplySemantics,
           source \in ReplySources:
          /\ IsFiniteSet(
               ReplyAttemptsForSource(owner, semantic, source))'
          /\ Cardinality(
               ReplyAttemptsForSource(owner, semantic, source))' <= 1
PROOF
  <1>1. ASSUME NEW ImageTransform(_),
                ReplyRouteSafetyInvariant,
                \A attempt \in rrAttempts:
                  SameReplyAttemptIdentity(
                    attempt, ImageTransform(attempt)),
                rrAttempts' =
                  {ImageTransform(attempt): attempt \in rrAttempts}
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics,
                  source \in ReplySources:
                 /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))'
                 /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source))' <= 1
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  NEW source \in ReplySources
           PROVE /\ IsFiniteSet(
                        ReplyAttemptsForSource(
                          owner, semantic, source))'
                 /\ Cardinality(
                        ReplyAttemptsForSource(
                          owner, semantic, source))' <= 1
      <3>1. ReplyAttemptsForSource(
               owner, semantic, source)' =
                 {ImageTransform(attempt):
                    attempt \in ReplyAttemptsForSource(
                      owner, semantic, source)}
        BY <1>1, SMTT(30)
           DEF SameReplyAttemptIdentity,
               ReplyAttemptsForSource, ReplyAttemptsFor
      <3>2. /\ IsFiniteSet(
                    ReplyAttemptsForSource(
                      owner, semantic, source))
             /\ Cardinality(
                    ReplyAttemptsForSource(
                      owner, semantic, source)) <= 1
        BY <1>1, <2>1
           DEF ReplyRouteSafetyInvariant,
               ReplyRouteOwnershipInvariant
      <3>3. LET image ==
                   {ImageTransform(attempt):
                      attempt \in ReplyAttemptsForSource(
                        owner, semantic, source)}
             IN /\ IsFiniteSet(image)
                /\ Cardinality(image) <=
                     Cardinality(
                       ReplyAttemptsForSource(
                         owner, semantic, source))
        BY <3>2, FS_Image
      <3>4. IsFiniteSet(
               ReplyAttemptsForSource(
                 owner, semantic, source))'
        BY <3>1, <3>3
      <3>5. Cardinality(
               ReplyAttemptsForSource(
                 owner, semantic, source))' <=
                 Cardinality(
                   ReplyAttemptsForSource(
                     owner, semantic, source))
        BY <3>1, <3>3
      <3>6. /\ Cardinality(
                    ReplyAttemptsForSource(
                      owner, semantic, source)) \in Nat
             /\ Cardinality(
                    ReplyAttemptsForSource(
                      owner, semantic, source))' \in Nat
        BY <3>2, <3>4, FS_CardinalityType
      <3>7. Cardinality(
               ReplyAttemptsForSource(
                 owner, semantic, source))' <= 1
        BY <3>2, <3>5, <3>6, SMTT(10)
      <3> QED BY <3>4, <3>7
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyIdentityImagePreservesPayloadOwnership ==
  ASSUME NEW Transform(_),
         ReplyRouteOwnershipInvariant,
         \A attempt \in rrAttempts:
           SameReplyAttemptIdentity(attempt, Transform(attempt)),
         rrAttempts' = {Transform(attempt): attempt \in rrAttempts},
         rrPayloads' = rrPayloads
  PROVE \A owner \in ReplyOwners, semantic \in ReplySemantics:
          ReplyAttemptsFor(owner, semantic)' # {} =>
            semantic \in rrPayloads'
PROOF
  <1>1. ASSUME NEW ImageTransform(_),
                ReplyRouteOwnershipInvariant,
                \A attempt \in rrAttempts:
                  SameReplyAttemptIdentity(
                    attempt, ImageTransform(attempt)),
                rrAttempts' =
                  {ImageTransform(attempt): attempt \in rrAttempts},
                rrPayloads' = rrPayloads
         PROVE \A owner \in ReplyOwners,
                  semantic \in ReplySemantics:
                 ReplyAttemptsFor(owner, semantic)' # {} =>
                   semantic \in rrPayloads'
    <2>1. ASSUME NEW owner \in ReplyOwners,
                  NEW semantic \in ReplySemantics,
                  ReplyAttemptsFor(owner, semantic)' # {}
           PROVE semantic \in rrPayloads'
      <3>1. ReplyAttemptsFor(owner, semantic) # {}
        BY <1>1, <2>1, SMTT(30)
           DEF SameReplyAttemptIdentity, ReplyAttemptsFor
      <3>2. semantic \in rrPayloads
        BY <1>1, <3>1 DEF ReplyRouteOwnershipInvariant
      <3> QED BY <1>1, <3>2
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyReplacementRetainsOtherAttempt ==
  \A oldAttempt, newAttempt, nextAttempt:
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    /\ nextAttempt \in rrAttempts'
    /\ nextAttempt # newAttempt
    => nextAttempt \in rrAttempts
BY SMTT(20) DEF ReplaceReplyAttempt

THEOREM ReplyUnchangedRouteMapsPreserveAttemptMetadata ==
  \A attempt \in rrAttempts:
    /\ ReplyRouteOwnershipInvariant
    /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
    /\ rrConnectionTenure' = rrConnectionTenure
    /\ rrSourceActive' = rrSourceActive
    => /\ attempt.deliveryOrdinal <
            rrNextDeliveryOrdinal'[attempt.owner]
       /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
       /\ IF attempt.ticketTenure = NoReplyTicketTenure
          THEN ReplyAttemptHasNoTicket(attempt)
          ELSE ReplyTicketValidForAttempt(attempt)'
BY SMTT(30)
   DEF ReplyRouteOwnershipInvariant,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent

THEOREM ReplyPointwiseRouteStatePreservesAttemptMetadata ==
  \A attempt \in rrAttempts:
    /\ ReplyRouteOwnershipInvariant
    /\ rrNextDeliveryOrdinal'[attempt.owner] =
         rrNextDeliveryOrdinal[attempt.owner]
    /\ rrConnectionTenure'[attempt.owner][attempt.source] =
         rrConnectionTenure[attempt.owner][attempt.source]
    /\ rrSourceActive'[attempt.owner][attempt.source] =
         rrSourceActive[attempt.owner][attempt.source]
    => /\ attempt.deliveryOrdinal <
            rrNextDeliveryOrdinal'[attempt.owner]
       /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
       /\ IF attempt.ticketTenure = NoReplyTicketTenure
          THEN ReplyAttemptHasNoTicket(attempt)
          ELSE ReplyTicketValidForAttempt(attempt)'
BY SMTT(30)
   DEF ReplyRouteOwnershipInvariant,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent

THEOREM ReplyNonregressingPointwiseRouteStatePreservesAttemptMetadata ==
  \A attempt \in rrAttempts:
    /\ ReplyRouteSafetyInvariant
    /\ rrNextDeliveryOrdinal'[attempt.owner] \in Nat
    /\ rrNextDeliveryOrdinal'[attempt.owner] >=
         rrNextDeliveryOrdinal[attempt.owner]
    /\ rrConnectionTenure'[attempt.owner][attempt.source] =
         rrConnectionTenure[attempt.owner][attempt.source]
    /\ rrSourceActive'[attempt.owner][attempt.source] =
         rrSourceActive[attempt.owner][attempt.source]
    => /\ attempt.deliveryOrdinal <
            rrNextDeliveryOrdinal'[attempt.owner]
       /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
       /\ IF attempt.ticketTenure = NoReplyTicketTenure
          THEN ReplyAttemptHasNoTicket(attempt)
          ELSE ReplyTicketValidForAttempt(attempt)'
PROOF
  <1>1. ASSUME NEW attempt \in rrAttempts,
                ReplyRouteSafetyInvariant,
                rrNextDeliveryOrdinal'[attempt.owner] \in Nat,
                rrNextDeliveryOrdinal'[attempt.owner] >=
                  rrNextDeliveryOrdinal[attempt.owner],
                rrConnectionTenure'[attempt.owner][attempt.source] =
                  rrConnectionTenure[attempt.owner][attempt.source],
                rrSourceActive'[attempt.owner][attempt.source] =
                  rrSourceActive[attempt.owner][attempt.source]
         PROVE /\ attempt.deliveryOrdinal <
                    rrNextDeliveryOrdinal'[attempt.owner]
               /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
               /\ IF attempt.ticketTenure = NoReplyTicketTenure
                  THEN ReplyAttemptHasNoTicket(attempt)
                  ELSE ReplyTicketValidForAttempt(attempt)'
    <2>1. /\ attempt.deliveryOrdinal <
                 rrNextDeliveryOrdinal[attempt.owner]
           /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
           /\ IF attempt.ticketTenure = NoReplyTicketTenure
              THEN ReplyAttemptHasNoTicket(attempt)
              ELSE ReplyTicketValidForAttempt(attempt)
      BY <1>1 DEF ReplyRouteSafetyInvariant,
           ReplyRouteOwnershipInvariant
    <2>2. /\ attempt.deliveryOrdinal \in Nat
           /\ rrNextDeliveryOrdinal[attempt.owner] \in Nat
      BY <1>1, SMTT(10)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplyAttemptSet, ReplyDeliveryOrdinals
    <2>3. attempt.deliveryOrdinal <
             rrNextDeliveryOrdinal'[attempt.owner]
      BY <1>1, <2>1, <2>2, SMTT(5)
    <2>4. ReplyTicketValidForAttempt(attempt) =>
             ReplyTicketValidForAttempt(attempt)'
      BY <1>1, SMTT(10)
         DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent
    <2>5. CASE attempt.ticketTenure = NoReplyTicketTenure
      BY <2>1, <2>3, <2>5
    <2>6. CASE attempt.ticketTenure # NoReplyTicketTenure
      BY <2>1, <2>3, <2>4, <2>6
    <2> QED BY <2>5, <2>6
  <1> QED BY <1>1

(***************************************************************************
Inductive safety and source-isolated replay.
***************************************************************************)

THEOREM ReplyRouteInitEstablishesInductiveInvariant ==
  ReplyRouteInit => ReplyRouteInductiveInvariant
PROOF
  <1>1. ASSUME ReplyRouteInit
         PROVE ReplyRouteInductiveInvariant
    <2>1. ReplyRouteConfiguration
      BY <1>1 DEF ReplyRouteInit
    <2>2. ReplyRouteTypeInvariant
      <3>1. rrAttempts \subseteq ReplyAttemptSet
        BY <1>1 DEF ReplyRouteInit
      <3>2. rrPayloads \subseteq ReplySemantics
        BY <1>1 DEF ReplyRouteInit
      <3>3. rrNextDeliveryOrdinal
                 \in [ReplyOwners ->
                       1..(ReplyDeliveryOrdinalLimit + 1)]
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteInit, ReplyRouteConfiguration
      <3>4. rrConnectionTenure
                 \in [ReplyOwners ->
                       [ReplySources -> ReplyConnectionTenures]]
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteInit, ReplyRouteConfiguration,
               ReplyConnectionTenures
      <3>5. rrSourceActive
                 \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
        BY <1>1, Isa DEF ReplyRouteInit
      <3>6. rrNextServiceIndex
                 \in [ReplyOwners ->
                       [ReplySemantics ->
                         1..Len(ReplySourceOrder)]]
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyRouteInit, ReplyRouteConfiguration
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
           DEF ReplyRouteTypeInvariant
    <2>3. ReplyRouteOwnershipInvariant
      <3>1. \A owner \in ReplyOwners,
                   semantic \in ReplySemantics,
                   source \in ReplySources:
                 /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))
                 /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source)) <= 1
        <4>1. ASSUME NEW owner \in ReplyOwners,
                      NEW semantic \in ReplySemantics,
                      NEW source \in ReplySources
               PROVE /\ IsFiniteSet(
                            ReplyAttemptsForSource(
                              owner, semantic, source))
                     /\ Cardinality(
                            ReplyAttemptsForSource(
                              owner, semantic, source)) <= 1
          <5>1. ReplyAttemptsForSource(
                   owner, semantic, source) = {}
            BY <1>1, Isa
               DEF ReplyRouteInit, ReplyAttemptsFor,
                   ReplyAttemptsForSource
          <5> QED BY <5>1, FS_EmptySet, SMTT(5)
        <4> QED BY <4>1
      <3>2. \A owner \in ReplyOwners,
                   semantic \in ReplySemantics:
                 /\ Cardinality(
                      ReplyAttemptSources(owner, semantic))
                      <= ReplySourceCapacity
                 /\ Cardinality(
                      ReplyRetiredDeliverySources(owner, semantic))
                      <= ReplySourceCapacity
                 /\ ReplyAttemptsFor(owner, semantic) # {} =>
                      semantic \in rrPayloads
        <4>1. ASSUME NEW owner \in ReplyOwners,
                      NEW semantic \in ReplySemantics
               PROVE /\ Cardinality(
                            ReplyAttemptSources(owner, semantic))
                            <= ReplySourceCapacity
                     /\ Cardinality(
                            ReplyRetiredDeliverySources(
                              owner, semantic))
                            <= ReplySourceCapacity
                     /\ ReplyAttemptsFor(owner, semantic) # {} =>
                            semantic \in rrPayloads
          <5>1. ReplyAttemptsFor(owner, semantic) = {}
            BY <1>1, Isa DEF ReplyRouteInit, ReplyAttemptsFor
          <5>2. ReplyAttemptSources(owner, semantic) = {}
            BY <5>1, Isa DEF ReplyAttemptSources
          <5>3. ReplyRetiredDeliverySources(
                   owner, semantic) = {}
            BY <5>1, Isa DEF ReplyRetiredDeliverySources
          <5>4. ReplySourceCapacity \in Nat \ {0}
            BY <2>1, SMTT(5) DEF ReplyRouteConfiguration
          <5> QED BY <5>1, <5>2, <5>3, <5>4,
                       FS_EmptySet, SMTT(5)
        <4> QED BY <4>1
      <3>3. \A attempt \in rrAttempts:
                 /\ attempt.deliveryOrdinal <
                      rrNextDeliveryOrdinal[attempt.owner]
                 /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
                 /\ IF attempt.ticketTenure = NoReplyTicketTenure
                    THEN ReplyAttemptHasNoTicket(attempt)
                    ELSE ReplyTicketValidForAttempt(attempt)
        BY <1>1 DEF ReplyRouteInit
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>3
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ReplyRouteStateIdentityPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ rrAttempts' = rrAttempts
  /\ rrPayloads' = rrPayloads
  /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
  /\ rrConnectionTenure' = rrConnectionTenure
  /\ rrSourceActive' = rrSourceActive
  /\ rrNextServiceIndex' = rrNextServiceIndex
  => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant,
                rrAttempts' = rrAttempts,
                rrPayloads' = rrPayloads,
                rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal,
                rrConnectionTenure' = rrConnectionTenure,
                rrSourceActive' = rrSourceActive,
                rrNextServiceIndex' = rrNextServiceIndex
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. ReplyRouteTypeInvariant'
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>3. ReplyRouteOwnershipInvariant'
      BY <1>1, Isa
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant,
             ReplyAttemptsFor, ReplyAttemptsForSource,
             ReplyAttemptSources, ReplyRetiredDeliverySources,
             ReplyAttemptCurrent, ReplyTicketValidForAttempt
    <2> QED BY <2>1, <2>2, <2>3
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ObserveNewReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ObserveNewReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                ObserveNewReplySource(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. ReplyRouteTypeInvariant'
      <3>1. rrAttempts \subseteq ReplyAttemptSet
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>2. rrAttempts' \subseteq ReplyAttemptSet
        <4>1. ReplyRouteConfiguration
          BY <1>1 DEF ReplyRouteInductiveInvariant
        <4>2. /\ rrNextDeliveryOrdinal[owner]
                     \in ReplyDeliveryOrdinals
               /\ rrConnectionTenure[owner][source]
                     \in ReplyConnectionTenures
          BY <1>1, SMTT(20)
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
                 ObserveNewReplySource, ReplyCapabilityValidFor,
                 ReplyCapability, ReplyDeliveryOrdinals
        <4>3. ReplyAttempt(
                 owner, source, semantic,
                 rrNextDeliveryOrdinal[owner],
                 rrConnectionTenure[owner][source],
                 0, 0, NoReplyTicketTenure,
                 {}, {}, {}, {}, 0, 0) \in ReplyAttemptSet
          BY <1>1, <4>1, <4>2, ReplyZeroCursorAttemptTyped
        <4>4. rrAttempts' =
                 rrAttempts \cup
                   {ReplyAttempt(
                      owner, source, semantic,
                      rrNextDeliveryOrdinal[owner],
                      rrConnectionTenure[owner][source],
                      0, 0, NoReplyTicketTenure,
                      {}, {}, {}, {}, 0, 0)}
          BY <1>1 DEF ObserveNewReplySource
        <4> QED BY <3>1, <4>3, <4>4
      <3>3. rrPayloads' \subseteq ReplySemantics
        BY <1>1, SMTT(15)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveNewReplySource
      <3>4. rrNextDeliveryOrdinal'
                 \in [ReplyOwners ->
                       1..(ReplyDeliveryOrdinalLimit + 1)]
        BY <1>1, SMTT(30)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyRouteConfiguration, ObserveNewReplySource,
               ReplyCapabilityValidFor, ReplyCapability,
               ReplyDeliveryOrdinals
      <3>5. rrConnectionTenure'
                 \in [ReplyOwners ->
                       [ReplySources -> ReplyConnectionTenures]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveNewReplySource
      <3>6. rrSourceActive'
                 \in [ReplyOwners ->
                       [ReplySources -> BOOLEAN]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveNewReplySource
      <3>7. rrNextServiceIndex'
                 \in [ReplyOwners ->
                       [ReplySemantics ->
                         1..Len(ReplySourceOrder)]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveNewReplySource
      <3> QED BY <3>2, <3>3, <3>4, <3>5, <3>6, <3>7
           DEF ReplyRouteTypeInvariant
    <2>3. ReplyRouteOwnershipInvariant'
      <3>1. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics,
                 nextSource \in ReplySources:
               /\ IsFiniteSet(
                    ReplyAttemptsForSource(
                      nextOwner, nextSemantic, nextSource))'
               /\ Cardinality(
                    ReplyAttemptsForSource(
                      nextOwner, nextSemantic, nextSource))' <= 1
        <4>1. ReplyRouteSafetyInvariant
          BY <1>1 DEF ReplyRouteInductiveInvariant
        <4>2. LET newAttempt ==
                     ReplyAttempt(
                       owner, source, semantic,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source],
                       0, 0, NoReplyTicketTenure,
                       {}, {}, {}, {}, 0, 0)
               IN newAttempt \in rrAttempts'
          BY <1>1 DEF ObserveNewReplySource
        <4>3. LET newAttempt ==
                     ReplyAttempt(
                       owner, source, semantic,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source],
                       0, 0, NoReplyTicketTenure,
                       {}, {}, {}, {}, 0, 0)
               IN newAttempt \in ReplyAttemptSet
          BY <2>2, <4>2 DEF ReplyRouteTypeInvariant
        <4>4. LET newAttempt ==
                     ReplyAttempt(
                       owner, source, semantic,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source],
                       0, 0, NoReplyTicketTenure,
                       {}, {}, {}, {}, 0, 0)
               IN /\ ~ReplyAttemptOwned(
                       newAttempt.owner, newAttempt.semantic,
                       newAttempt.source)
                  /\ rrAttempts' =
                       rrAttempts \cup {newAttempt}
          BY <1>1, SMTT(10)
             DEF ObserveNewReplySource, ReplyAttempt,
                 ReplyAttemptOwned
        <4>5. LET newAttempt ==
                     ReplyAttempt(
                       owner, source, semantic,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source],
                       0, 0, NoReplyTicketTenure,
                       {}, {}, {}, {}, 0, 0)
               IN /\ ReplyRouteSafetyInvariant
                  /\ newAttempt \in ReplyAttemptSet
                  /\ ~ReplyAttemptOwned(
                       newAttempt.owner, newAttempt.semantic,
                       newAttempt.source)
                  /\ rrAttempts' =
                       rrAttempts \cup {newAttempt}
          BY <4>1, <4>3, <4>4
        <4> QED BY <4>5,
             ReplyAttemptExtensionPreservesSourceOwnership
      <3>2. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics:
               /\ Cardinality(
                    ReplyAttemptSources(
                      nextOwner, nextSemantic))' <=
                    ReplySourceCapacity
               /\ Cardinality(
                    ReplyRetiredDeliverySources(
                      nextOwner, nextSemantic))' <=
                    ReplySourceCapacity
        BY <2>1, <2>2, ReplyNextTypeBoundsSourceGeometry
      <3>3. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics:
               ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
                 nextSemantic \in rrPayloads'
        <4>1. ASSUME NEW nextOwner \in ReplyOwners,
                      NEW nextSemantic \in ReplySemantics,
                      ReplyAttemptsFor(
                        nextOwner, nextSemantic)' # {}
               PROVE nextSemantic \in rrPayloads'
          <5>1. /\ rrAttempts' =
                       rrAttempts \cup
                         {ReplyAttempt(
                            owner, source, semantic,
                            rrNextDeliveryOrdinal[owner],
                            rrConnectionTenure[owner][source],
                            0, 0, NoReplyTicketTenure,
                            {}, {}, {}, {}, 0, 0)}
                 /\ rrPayloads' = rrPayloads \cup {semantic}
            BY <1>1 DEF ObserveNewReplySource
          <5>2. CASE nextSemantic = semantic
            <6> QED BY <5>1, <5>2
          <5>3. CASE nextSemantic # semantic
            <6>1. ReplyAttemptsFor(
                     nextOwner, nextSemantic) # {}
              BY <4>1, <5>1, <5>3, SMTT(20)
                 DEF ReplyAttempt, ReplyAttemptsFor
            <6>2. nextSemantic \in rrPayloads
              BY <1>1, <6>1
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteOwnershipInvariant
            <6> QED BY <5>1, <6>2
          <5> QED BY <5>2, <5>3
        <4> QED BY <4>1
      <3>4. \A nextAttempt \in rrAttempts':
               /\ nextAttempt.deliveryOrdinal <
                    rrNextDeliveryOrdinal'[nextAttempt.owner]
               /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
               /\ IF nextAttempt.ticketTenure =
                         NoReplyTicketTenure
                  THEN ReplyAttemptHasNoTicket(nextAttempt)
                  ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>1. ASSUME NEW nextAttempt \in rrAttempts'
               PROVE /\ nextAttempt.deliveryOrdinal <
                          rrNextDeliveryOrdinal'[
                            nextAttempt.owner]
                     /\ ReplyAttemptRetiredDeliveryWellFormed(
                          nextAttempt)
                     /\ IF nextAttempt.ticketTenure =
                               NoReplyTicketTenure
                        THEN ReplyAttemptHasNoTicket(nextAttempt)
                        ELSE ReplyTicketValidForAttempt(nextAttempt)'
          <5>1. CASE nextAttempt \in rrAttempts
            <6>1. /\ nextAttempt.deliveryOrdinal <
                         rrNextDeliveryOrdinal[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)
              BY <1>1, <5>1
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteOwnershipInvariant
            <6>2. /\ nextAttempt.owner \in ReplyOwners
                   /\ nextAttempt.deliveryOrdinal \in Nat
                   /\ rrNextDeliveryOrdinal[
                        nextAttempt.owner] \in Nat
                   /\ rrNextDeliveryOrdinal
                        \in [ReplyOwners ->
                              1..(ReplyDeliveryOrdinalLimit + 1)]
              BY <1>1, <5>1, SMTT(10)
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteTypeInvariant, ReplyAttemptSet,
                     ReplyDeliveryOrdinals
            <6>3. rrNextDeliveryOrdinal' =
                     [rrNextDeliveryOrdinal EXCEPT
                        ![owner] = @ + 1]
              BY <1>1 DEF ObserveNewReplySource
            <6>4. /\ rrNextDeliveryOrdinal'[
                         nextAttempt.owner] \in Nat
                   /\ rrNextDeliveryOrdinal'[
                         nextAttempt.owner] >=
                         rrNextDeliveryOrdinal[nextAttempt.owner]
              <7>1. CASE nextAttempt.owner = owner
                BY <1>1, <6>2, <6>3, <7>1,
                   ReplyFunctionalUpdateAtKey, SMTT(5)
              <7>2. CASE nextAttempt.owner # owner
                BY <1>1, <6>2, <6>3, <7>2,
                   ReplyFunctionalUpdateAwayFromKey
              <7> QED BY <7>1, <7>2
            <6>5. ReplyTicketValidForAttempt(nextAttempt) =>
                     ReplyTicketValidForAttempt(nextAttempt)'
              BY <1>1, SMTT(10)
                 DEF ObserveNewReplySource,
                     ReplyTicketValidForAttempt,
                     ReplyAttemptCurrent
            <6>6. CASE nextAttempt.ticketTenure =
                          NoReplyTicketTenure
              <7>1. nextAttempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal'[
                         nextAttempt.owner]
                BY <6>1, <6>2, <6>4, SMTT(5)
              <7>2. ReplyAttemptRetiredDeliveryWellFormed(
                       nextAttempt)
                BY <6>1
              <7>3. ReplyAttemptHasNoTicket(nextAttempt)
                BY <6>1, <6>6
              <7> QED BY <6>6, <7>1, <7>2, <7>3
            <6>7. CASE nextAttempt.ticketTenure #
                          NoReplyTicketTenure
              <7>1. nextAttempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal'[
                         nextAttempt.owner]
                BY <6>1, <6>2, <6>4, SMTT(5)
              <7>2. ReplyAttemptRetiredDeliveryWellFormed(
                       nextAttempt)
                BY <6>1
              <7>3. ReplyTicketValidForAttempt(nextAttempt)
                BY <6>1, <6>7
              <7>4. ReplyTicketValidForAttempt(nextAttempt)'
                BY <6>5, <7>3
              <7> QED BY <6>7, <7>1, <7>2, <7>4
            <6> QED BY <6>6, <6>7
          <5>2. CASE nextAttempt \notin rrAttempts
            <6>1. nextAttempt =
                     ReplyAttempt(
                       owner, source, semantic,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source],
                       0, 0, NoReplyTicketTenure,
                       {}, {}, {}, {}, 0, 0)
              BY <1>1, <4>1, <5>2
                 DEF ObserveNewReplySource
            <6>2. rrNextDeliveryOrdinal[owner] \in Nat
              BY <1>1, Isa
                 DEF ObserveNewReplySource,
                     ReplyDeliveryOrdinals
            <6>3. rrNextDeliveryOrdinal' =
                     [rrNextDeliveryOrdinal EXCEPT
                        ![owner] = @ + 1]
              BY <1>1 DEF ObserveNewReplySource
            <6>4. rrNextDeliveryOrdinal'[owner] =
                     rrNextDeliveryOrdinal[owner] + 1
              BY <1>1, <6>3, ReplyFunctionalUpdateAtKey
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteTypeInvariant
            <6>5. /\ nextAttempt.deliveryOrdinal <
                         rrNextDeliveryOrdinal'[
                           nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ nextAttempt.ticketTenure =
                        NoReplyTicketTenure
                   /\ ReplyAttemptHasNoTicket(nextAttempt)
              BY <6>1, <6>2, <6>4, SMTT(10)
                 DEF ReplyAttempt,
                     ReplyAttemptRetiredDeliveryWellFormed,
                     ReplyAttemptHasNoRetiredDelivery,
                     ReplyAttemptHasNoTicket,
                     NoReplyTicketTenure
            <6> QED BY <6>5
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>1, <3>2, <3>3, <3>4
           DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>3
         DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ObserveLaterReplyDeliveryPreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                ObserveLaterReplyDelivery(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. ReplyRouteTypeInvariant'
      <3>1. rrAttempts \subseteq ReplyAttemptSet
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt,
                     rrNextDeliveryOrdinal[owner],
                     rrConnectionTenure[owner][source])
             IN /\ oldAttempt \in ReplyAttemptSet
                /\ routed \in ReplyAttemptSet
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN oldAttempt \in rrAttempts
          BY <1>1, ReplyOwnedAttemptIdentity
             DEF ObserveLaterReplyDelivery
        <4>2. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN oldAttempt \in ReplyAttemptSet
          BY <3>1, <4>1
        <4>3. /\ rrNextDeliveryOrdinal[owner]
                     \in ReplyDeliveryOrdinals
               /\ rrConnectionTenure[owner][source]
                     \in ReplyConnectionTenures
          BY <1>1, SMTT(20)
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
                 ObserveLaterReplyDelivery,
                 ReplyCapabilityValidFor, ReplyCapability,
                 ReplyDeliveryOrdinals
        <4>4. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source])
               IN routed \in ReplyAttemptSet
          BY <1>1, <4>2, <4>3,
             ReplyRouteRefreshPreservesAttemptType
             DEF ReplyRouteInductiveInvariant
        <4> QED BY <4>2, <4>4
      <3>3. rrAttempts' \subseteq ReplyAttemptSet
        BY <1>1, <3>1, <3>2, SMTT(20)
           DEF ObserveLaterReplyDelivery, ReplaceReplyAttempt
      <3>4. rrPayloads' \subseteq ReplySemantics
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveLaterReplyDelivery
      <3>5. rrNextDeliveryOrdinal'
                 \in [ReplyOwners ->
                       1..(ReplyDeliveryOrdinalLimit + 1)]
        BY <1>1, SMTT(30)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyRouteConfiguration,
               ObserveLaterReplyDelivery,
               ReplyCapabilityValidFor, ReplyCapability,
               ReplyDeliveryOrdinals
      <3>6. rrConnectionTenure'
                 \in [ReplyOwners ->
                       [ReplySources -> ReplyConnectionTenures]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveLaterReplyDelivery
      <3>7. rrSourceActive'
                 \in [ReplyOwners ->
                       [ReplySources -> BOOLEAN]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveLaterReplyDelivery
      <3>8. rrNextServiceIndex'
                 \in [ReplyOwners ->
                       [ReplySemantics ->
                         1..Len(ReplySourceOrder)]]
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ObserveLaterReplyDelivery
      <3> QED BY <3>3, <3>4, <3>5, <3>6, <3>7, <3>8
           DEF ReplyRouteTypeInvariant
    <2>3. ReplyRouteOwnershipInvariant'
      <3>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt,
                     rrNextDeliveryOrdinal[owner],
                     rrConnectionTenure[owner][source])
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
                /\ SameReplyAttemptIdentity(oldAttempt, routed)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, routed)
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN oldAttempt \in rrAttempts
          BY <1>1, ReplyOwnedAttemptIdentity
             DEF ObserveLaterReplyDelivery
        <4>2. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN oldAttempt \in ReplyAttemptSet
          BY <1>1, <4>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>3. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN /\ rrNextDeliveryOrdinal[owner]
                       \in ReplyDeliveryOrdinals
                  /\ rrConnectionTenure[owner][source]
                       \in ReplyConnectionTenures
                  /\ rrNextDeliveryOrdinal[owner] >
                       oldAttempt.deliveryOrdinal
          <5>1. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                 IN /\ rrNextDeliveryOrdinal[owner]
                         \in ReplyDeliveryOrdinals
                    /\ rrNextDeliveryOrdinal[owner] >
                         oldAttempt.deliveryOrdinal
            BY <1>1 DEF ObserveLaterReplyDelivery
          <5>2. rrConnectionTenure[owner][source]
                   \in ReplyConnectionTenures
            BY <1>1
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant
          <5> QED BY <5>1, <5>2
        <4>4. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source])
               IN SameReplyAttemptIdentity(oldAttempt, routed)
          BY <4>2, <4>3,
             ReplyRouteRefreshPreservesIdentityAndCursor
        <4>5. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source])
               IN rrAttempts' =
                    ReplaceReplyAttempt(oldAttempt, routed)
          BY <1>1 DEF ObserveLaterReplyDelivery
        <4> QED BY <1>1, <4>1, <4>4, <4>5
             DEF ReplyRouteInductiveInvariant
      <3>2. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics,
                 nextSource \in ReplySources:
               /\ IsFiniteSet(
                    ReplyAttemptsForSource(
                      nextOwner, nextSemantic, nextSource))'
               /\ Cardinality(
                    ReplyAttemptsForSource(
                      nextOwner, nextSemantic, nextSource))' <= 1
        BY <3>1, ReplyIdentityReplacementPreservesSourceOwnership
      <3>3. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics:
               /\ Cardinality(
                    ReplyAttemptSources(
                      nextOwner, nextSemantic))' <=
                    ReplySourceCapacity
               /\ Cardinality(
                    ReplyRetiredDeliverySources(
                      nextOwner, nextSemantic))' <=
                    ReplySourceCapacity
        BY <2>1, <2>2, ReplyNextTypeBoundsSourceGeometry
      <3>4. \A nextOwner \in ReplyOwners,
                 nextSemantic \in ReplySemantics:
               ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
                 nextSemantic \in rrPayloads'
        BY <1>1, <3>1, SMTT(20)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteOwnershipInvariant,
               ObserveLaterReplyDelivery,
               ReplaceReplyAttempt, ReplyAttemptsFor,
               SameReplyAttemptIdentity
      <3>5. \A nextAttempt \in rrAttempts':
               /\ nextAttempt.deliveryOrdinal <
                    rrNextDeliveryOrdinal'[nextAttempt.owner]
               /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
               /\ IF nextAttempt.ticketTenure =
                         NoReplyTicketTenure
                  THEN ReplyAttemptHasNoTicket(nextAttempt)
                  ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>1. ASSUME NEW nextAttempt \in rrAttempts'
               PROVE /\ nextAttempt.deliveryOrdinal <
                          rrNextDeliveryOrdinal'[
                            nextAttempt.owner]
                     /\ ReplyAttemptRetiredDeliveryWellFormed(
                          nextAttempt)
                     /\ IF nextAttempt.ticketTenure =
                               NoReplyTicketTenure
                        THEN ReplyAttemptHasNoTicket(nextAttempt)
                        ELSE ReplyTicketValidForAttempt(nextAttempt)'
          <5>1. CASE LET oldAttempt ==
                            ReplyAttemptFor(
                              owner, semantic, source)
                          routed ==
                            ReplyAttemptWithRoute(
                              oldAttempt,
                              rrNextDeliveryOrdinal[owner],
                              rrConnectionTenure[owner][source])
                      IN nextAttempt = routed
            <6>1. LET oldAttempt ==
                         ReplyAttemptFor(owner, semantic, source)
                   IN /\ oldAttempt \in rrAttempts
                      /\ oldAttempt \in ReplyAttemptSet
                      /\ oldAttempt.owner = owner
                      /\ oldAttempt.semantic = semantic
                      /\ oldAttempt.source = source
                      /\ oldAttempt.deliveryOrdinal
                           \in ReplyDeliveryOrdinals
                      /\ oldAttempt.connectionTenure
                           \in ReplyConnectionTenures
                      /\ rrNextDeliveryOrdinal[owner]
                           \in ReplyDeliveryOrdinals
                      /\ rrConnectionTenure[owner][source]
                           \in ReplyConnectionTenures
                      /\ rrNextDeliveryOrdinal[owner] >
                           oldAttempt.deliveryOrdinal
              <7>1. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                     IN /\ oldAttempt \in rrAttempts
                        /\ oldAttempt.owner = owner
                        /\ oldAttempt.semantic = semantic
                        /\ oldAttempt.source = source
                BY <1>1, ReplyOwnedAttemptIdentity
                   DEF ObserveLaterReplyDelivery
              <7>2. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                     IN /\ oldAttempt \in ReplyAttemptSet
                        /\ oldAttempt.deliveryOrdinal
                             \in ReplyDeliveryOrdinals
                        /\ oldAttempt.connectionTenure
                             \in ReplyConnectionTenures
                BY <1>1, <7>1, SMTT(10)
                   DEF ReplyRouteInductiveInvariant,
                       ReplyRouteSafetyInvariant,
                       ReplyRouteTypeInvariant,
                       ReplyAttemptSet,
                       ReplyDeliveryOrdinals,
                       ReplyConnectionTenures
              <7>3. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                     IN /\ rrNextDeliveryOrdinal[owner]
                             \in ReplyDeliveryOrdinals
                        /\ rrNextDeliveryOrdinal[owner] >
                             oldAttempt.deliveryOrdinal
                BY <1>1 DEF ObserveLaterReplyDelivery
              <7>4. rrConnectionTenure[owner][source]
                       \in ReplyConnectionTenures
                BY <1>1
                   DEF ReplyRouteInductiveInvariant,
                       ReplyRouteSafetyInvariant,
                       ReplyRouteTypeInvariant
              <7> QED BY <7>1, <7>2, <7>3, <7>4
            <6>2. rrNextDeliveryOrdinal' =
                     [rrNextDeliveryOrdinal EXCEPT
                        ![owner] = @ + 1]
              BY <1>1 DEF ObserveLaterReplyDelivery
            <6>3. rrNextDeliveryOrdinal'[owner] =
                     rrNextDeliveryOrdinal[owner] + 1
              BY <1>1, <6>2, ReplyFunctionalUpdateAtKey
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteTypeInvariant
            <6>4. LET oldAttempt ==
                         ReplyAttemptFor(owner, semantic, source)
                       routed ==
                         ReplyAttemptWithRoute(
                           oldAttempt,
                           rrNextDeliveryOrdinal[owner],
                           rrConnectionTenure[owner][source])
                   IN /\ SameReplyAttemptIdentity(
                           oldAttempt, routed)
                      /\ routed.deliveryOrdinal =
                           rrNextDeliveryOrdinal[owner]
                      /\ routed.retiredDeliveryOrdinal =
                           oldAttempt.deliveryOrdinal
                      /\ routed.retiredConnectionTenure =
                           oldAttempt.connectionTenure
                      /\ ReplyAttemptCursor(routed) =
                           ReplyAttemptCursor(oldAttempt)
              <7>1. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                         routed ==
                           ReplyAttemptWithRoute(
                             oldAttempt,
                             rrNextDeliveryOrdinal[owner],
                             rrConnectionTenure[owner][source])
                     IN /\ SameReplyAttemptIdentity(
                             oldAttempt, routed)
                        /\ routed.deliveryOrdinal =
                             rrNextDeliveryOrdinal[owner]
                        /\ routed.retiredDeliveryOrdinal =
                             oldAttempt.deliveryOrdinal
                        /\ routed.retiredConnectionTenure =
                             oldAttempt.connectionTenure
                        /\ ReplyAttemptCursor(routed) =
                             ReplyAttemptCursor(oldAttempt)
                BY <6>1,
                   ReplyRouteRefreshPreservesIdentityAndCursor,
                   ReplyRouteUpdateRecordsLatestRetiredDelivery
              <7> QED BY <7>1
            <6>5. /\ nextAttempt.deliveryOrdinal <
                         rrNextDeliveryOrdinal'[
                           nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
              <7>1. nextAttempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal'[
                         nextAttempt.owner]
                <8>1. /\ nextAttempt.owner = owner
                       /\ nextAttempt.deliveryOrdinal =
                            rrNextDeliveryOrdinal[owner]
                  BY <5>1, <6>1, <6>4
                     DEF SameReplyAttemptIdentity
                <8>2. rrNextDeliveryOrdinal[owner] \in Nat
                  BY <6>1, Isa DEF ReplyDeliveryOrdinals
                <8> QED BY <6>3, <8>1, <8>2, SMTT(5)
              <7>2. ReplyAttemptRetiredDeliveryWellFormed(
                       nextAttempt)
                BY <5>1, <6>1, <6>4, SMTT(10)
                   DEF ReplyAttemptRetiredDeliveryWellFormed,
                       ReplyAttemptHasNoRetiredDelivery
              <7> QED BY <7>1, <7>2
            <6>6. CASE rrConnectionTenure[owner][source] =
                          ReplyAttemptFor(
                            owner, semantic, source)
                            .connectionTenure
              <7>1. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                     IN IF oldAttempt.ticketTenure =
                              NoReplyTicketTenure
                        THEN ReplyAttemptHasNoTicket(oldAttempt)
                        ELSE ReplyTicketValidForAttempt(oldAttempt)
                BY <3>1
                   DEF ReplyRouteSafetyInvariant,
                       ReplyRouteOwnershipInvariant
              <7>2. LET oldAttempt ==
                           ReplyAttemptFor(
                             owner, semantic, source)
                         routed ==
                           ReplyAttemptWithRoute(
                             oldAttempt,
                             rrNextDeliveryOrdinal[owner],
                             rrConnectionTenure[owner][source])
                     IN /\ routed.ticketTenure =
                              oldAttempt.ticketTenure
                        /\ routed.ticketSemantic =
                              oldAttempt.ticketSemantic
                        /\ routed.ticketTarget =
                              oldAttempt.ticketTarget
                        /\ routed.ticketMessageCursor =
                              oldAttempt.ticketMessageCursor
                        /\ routed.ticketChunkCursor =
                              oldAttempt.ticketChunkCursor
                        /\ routed.connectionTenure =
                              oldAttempt.connectionTenure
                        /\ ReplyAttemptCursor(routed) =
                              ReplyAttemptCursor(oldAttempt)
                BY <6>1, <6>4, <6>6,
                   ReplySameTenureRefreshPreservesTicketState
              <7>3. /\ rrConnectionTenure' =
                           rrConnectionTenure
                     /\ rrSourceActive' = rrSourceActive
                BY <1>1 DEF ObserveLaterReplyDelivery
              <7>4. CASE ReplyAttemptFor(
                            owner, semantic, source)
                            .ticketTenure =
                          NoReplyTicketTenure
                <8>1. /\ nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                       /\ ReplyAttemptHasNoTicket(nextAttempt)
                  BY <5>1, <7>1, <7>2, <7>4, SMTT(10)
                     DEF ReplyAttemptHasNoTicket
                <8> QED BY <6>5, <8>1
              <7>5. CASE ReplyAttemptFor(
                            owner, semantic, source)
                            .ticketTenure #
                          NoReplyTicketTenure
                <8>1. LET oldAttempt ==
                             ReplyAttemptFor(
                               owner, semantic, source)
                       IN ReplyTicketValidForAttempt(oldAttempt)
                  BY <7>1, <7>5
                <8>2. nextAttempt.ticketTenure #
                         NoReplyTicketTenure
                  BY <5>1, <7>2, <7>5
                <8>3. ReplyTicketValidForAttempt(nextAttempt)'
                  BY <5>1, <6>1, <6>4, <7>2, <7>3, <8>1,
                     SMTT(30)
                     DEF ReplyTicketValidForAttempt,
                         ReplyAttemptCurrent,
                         ReplyTicketForAttempt, ReplyTicket,
                         ReplyAttemptCursor,
                         SameReplyAttemptIdentity
                <8> QED BY <6>5, <8>2, <8>3
              <7> QED BY <7>4, <7>5
            <6>7. CASE rrConnectionTenure[owner][source] #
                          ReplyAttemptFor(
                            owner, semantic, source)
                            .connectionTenure
              <7>1. /\ nextAttempt.ticketTenure =
                           NoReplyTicketTenure
                     /\ ReplyAttemptHasNoTicket(nextAttempt)
                BY <5>1, <6>1, <6>7,
                   ReplyDifferentTenureRefreshClearsTicket
              <7> QED BY <6>5, <7>1
            <6> QED BY <6>5, <6>6, <6>7
          <5>2. CASE LET oldAttempt ==
                            ReplyAttemptFor(
                              owner, semantic, source)
                          routed ==
                            ReplyAttemptWithRoute(
                              oldAttempt,
                              rrNextDeliveryOrdinal[owner],
                              rrConnectionTenure[owner][source])
                      IN nextAttempt # routed
            <6>1. nextAttempt \in rrAttempts
              BY <3>1, <4>1, <5>2, SMTT(10)
                 DEF ReplaceReplyAttempt
            <6>2. /\ ReplyRouteSafetyInvariant
                   /\ rrNextDeliveryOrdinal' =
                        [rrNextDeliveryOrdinal EXCEPT
                           ![owner] = @ + 1]
                   /\ rrConnectionTenure' =
                        rrConnectionTenure
                   /\ rrSourceActive' = rrSourceActive
              BY <1>1
                 DEF ReplyRouteInductiveInvariant,
                     ObserveLaterReplyDelivery
            <6> QED BY <1>1, <6>1, <6>2,
                 ReplyDeliveryOrdinalBumpPreservesMetadata
          <5> QED BY <5>1, <5>2
        <4> QED BY <4>1
      <3> QED BY <3>2, <3>3, <3>4, <3>5
           DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>3
         DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM RetryExactReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RetryExactReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                RetryExactReplySource(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. /\ rrAttempts' = rrAttempts
           /\ rrPayloads' = rrPayloads
           /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
           /\ rrConnectionTenure' = rrConnectionTenure
           /\ rrSourceActive' = rrSourceActive
           /\ rrNextServiceIndex' = rrNextServiceIndex
      BY <1>1 DEF RetryExactReplySource, ReplyRouteVars
    <2> QED BY <1>1, <2>1,
         ReplyRouteStateIdentityPreservesInductiveInvariant
  <1> QED BY <1>1

THEOREM RetireReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RetireReplySource(owner, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                RetireReplySource(owner, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. /\ ReplyRouteSafetyInvariant
           /\ (\A attempt \in rrAttempts:
                 LET retired ==
                       ReplyAttemptAfterRetire(
                         owner, source, attempt)
                 IN /\ attempt \in ReplyAttemptSet
                    /\ retired \in ReplyAttemptSet
                    /\ SameReplyAttemptIdentity(attempt, retired)
                    /\ ReplyAttemptCursor(retired) =
                         ReplyAttemptCursor(attempt)
                    /\ retired.deliveryOrdinal =
                         attempt.deliveryOrdinal
                    /\ retired.connectionTenure =
                         attempt.connectionTenure
                    /\ retired.retiredDeliveryOrdinal =
                         attempt.retiredDeliveryOrdinal
                    /\ retired.retiredConnectionTenure =
                         attempt.retiredConnectionTenure
                    /\ IF attempt.owner = owner
                             /\ attempt.source = source
                       THEN ReplyAttemptHasNoTicket(retired)
                       ELSE retired = attempt)
           /\ rrAttempts' =
                {ReplyAttemptAfterRetire(owner, source, attempt):
                   attempt \in rrAttempts}
           /\ rrPayloads' = rrPayloads
           /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
           /\ rrConnectionTenure' = rrConnectionTenure
           /\ rrSourceActive' =
                [rrSourceActive EXCEPT
                   ![owner][source] = FALSE]
           /\ rrNextServiceIndex' = rrNextServiceIndex
      <3>1. /\ ReplyRouteConfiguration
             /\ ReplyRouteSafetyInvariant
        BY <1>1 DEF ReplyRouteInductiveInvariant
      <3>2. \A attempt \in rrAttempts:
               LET retired ==
                     ReplyAttemptAfterRetire(
                       owner, source, attempt)
               IN /\ attempt \in ReplyAttemptSet
                  /\ retired \in ReplyAttemptSet
                  /\ SameReplyAttemptIdentity(attempt, retired)
                  /\ ReplyAttemptCursor(retired) =
                       ReplyAttemptCursor(attempt)
                  /\ retired.deliveryOrdinal =
                       attempt.deliveryOrdinal
                  /\ retired.connectionTenure =
                       attempt.connectionTenure
                  /\ retired.retiredDeliveryOrdinal =
                       attempt.retiredDeliveryOrdinal
                  /\ retired.retiredConnectionTenure =
                       attempt.retiredConnectionTenure
                  /\ IF attempt.owner = owner
                           /\ attempt.source = source
                     THEN ReplyAttemptHasNoTicket(retired)
                     ELSE retired = attempt
        <4>1. ASSUME NEW attempt \in rrAttempts
               PROVE LET retired ==
                           ReplyAttemptAfterRetire(
                             owner, source, attempt)
                     IN /\ attempt \in ReplyAttemptSet
                        /\ retired \in ReplyAttemptSet
                        /\ SameReplyAttemptIdentity(
                             attempt, retired)
                        /\ ReplyAttemptCursor(retired) =
                             ReplyAttemptCursor(attempt)
                        /\ retired.deliveryOrdinal =
                             attempt.deliveryOrdinal
                        /\ retired.connectionTenure =
                             attempt.connectionTenure
                        /\ retired.retiredDeliveryOrdinal =
                             attempt.retiredDeliveryOrdinal
                        /\ retired.retiredConnectionTenure =
                             attempt.retiredConnectionTenure
                        /\ IF attempt.owner = owner
                                 /\ attempt.source = source
                           THEN ReplyAttemptHasNoTicket(retired)
                           ELSE retired = attempt
          <5>1. attempt \in ReplyAttemptSet
            BY <3>1, <4>1
               DEF ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant
          <5> QED BY <3>1, <5>1,
               ReplyRetireTransformTypedAndIdentity
        <4> QED BY <4>1
      <3>3. /\ rrAttempts' =
                  {ReplyAttemptAfterRetire(
                     owner, source, attempt):
                     attempt \in rrAttempts}
             /\ rrPayloads' = rrPayloads
             /\ rrNextDeliveryOrdinal' =
                  rrNextDeliveryOrdinal
             /\ rrConnectionTenure' = rrConnectionTenure
             /\ rrSourceActive' =
                  [rrSourceActive EXCEPT
                     ![owner][source] = FALSE]
             /\ rrNextServiceIndex' = rrNextServiceIndex
        BY <1>1
           DEF RetireReplySource,
               ReplyAttemptAfterRetire
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. ReplyRouteTypeInvariant'
      <3>1. rrAttempts' \subseteq ReplyAttemptSet
        <4>1. ASSUME NEW nextAttempt \in rrAttempts'
               PROVE nextAttempt \in ReplyAttemptSet
          <5>1. PICK oldAttempt \in rrAttempts:
                   nextAttempt =
                     ReplyAttemptAfterRetire(
                       owner, source, oldAttempt)
            BY <2>2, <4>1
          <5> QED BY <2>2, <5>1
        <4> QED BY <4>1
      <3>2. /\ rrPayloads' \subseteq ReplySemantics
             /\ rrNextDeliveryOrdinal'
                  \in [ReplyOwners ->
                        1..(ReplyDeliveryOrdinalLimit + 1)]
             /\ rrConnectionTenure'
                  \in [ReplyOwners ->
                        [ReplySources -> ReplyConnectionTenures]]
             /\ rrNextServiceIndex'
                  \in [ReplyOwners ->
                        [ReplySemantics ->
                          1..Len(ReplySourceOrder)]]
        <4>1. /\ rrPayloads \subseteq ReplySemantics
               /\ rrNextDeliveryOrdinal
                    \in [ReplyOwners ->
                          1..(ReplyDeliveryOrdinalLimit + 1)]
               /\ rrConnectionTenure
                    \in [ReplyOwners ->
                          [ReplySources -> ReplyConnectionTenures]]
               /\ rrNextServiceIndex
                    \in [ReplyOwners ->
                          [ReplySemantics ->
                            1..Len(ReplySourceOrder)]]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4> QED BY <2>2, <4>1
      <3>3. rrSourceActive'
                 \in [ReplyOwners ->
                       [ReplySources -> BOOLEAN]]
        <4>1. rrSourceActive
                   \in [ReplyOwners ->
                         [ReplySources -> BOOLEAN]]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>2. [rrSourceActive EXCEPT
                  ![owner][source] = FALSE]
                   \in [ReplyOwners ->
                         [ReplySources -> BOOLEAN]]
          BY <4>1, ReplyNestedFunctionalUpdatePreservesType
        <4> QED BY <2>2, <4>2
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyRouteTypeInvariant
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))'
             /\ Cardinality(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))' <= 1
      <3>1. ASSUME NEW nextOwner \in ReplyOwners,
                    NEW nextSemantic \in ReplySemantics,
                    NEW nextSource \in ReplySources
             PROVE /\ IsFiniteSet(
                          ReplyAttemptsForSource(
                            nextOwner, nextSemantic, nextSource))'
                   /\ Cardinality(
                          ReplyAttemptsForSource(
                            nextOwner, nextSemantic, nextSource))' <= 1
        <4>1. ReplyAttemptsForSource(
                 nextOwner, nextSemantic, nextSource)' =
                   {ReplyAttemptAfterRetire(
                      owner, source, attempt):
                      attempt \in ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource)}
          BY <2>2, SMTT(30)
             DEF SameReplyAttemptIdentity,
                 ReplyAttemptsForSource, ReplyAttemptsFor
        <4>2. /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource))
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource)) <= 1
          BY <2>2, <3>1
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4>3. LET image ==
                     {ReplyAttemptAfterRetire(
                        owner, source, attempt):
                        attempt \in ReplyAttemptsForSource(
                          nextOwner, nextSemantic, nextSource)}
               IN /\ IsFiniteSet(image)
                  /\ Cardinality(image) <=
                       Cardinality(
                         ReplyAttemptsForSource(
                           nextOwner, nextSemantic, nextSource))
          BY <4>2, FS_Image
        <4>4. IsFiniteSet(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))'
          BY <4>1, <4>3
        <4>5. Cardinality(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))' <=
                   Cardinality(
                     ReplyAttemptsForSource(
                       nextOwner, nextSemantic, nextSource))
          BY <4>1, <4>3
        <4>6. /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource)) \in Nat
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource))' \in Nat
          BY <4>2, <4>4, FS_CardinalityType
        <4>7. Cardinality(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))' <= 1
          BY <4>2, <4>5, <4>6, SMTT(10)
        <4> QED BY <4>4, <4>7
      <3> QED BY <3>1
    <2>5. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
      BY <2>1, <2>3, ReplyNextTypeBoundsSourceGeometry
    <2>6. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
               nextSemantic \in rrPayloads'
      <3>1. ASSUME NEW nextOwner \in ReplyOwners,
                    NEW nextSemantic \in ReplySemantics,
                    ReplyAttemptsFor(
                      nextOwner, nextSemantic)' # {}
             PROVE nextSemantic \in rrPayloads'
        <4>1. ReplyAttemptsFor(
                 nextOwner, nextSemantic) # {}
          BY <2>2, <3>1, SMTT(30)
             DEF SameReplyAttemptIdentity,
                 ReplyAttemptsFor
        <4>2. nextSemantic \in rrPayloads
          BY <1>1, <4>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4> QED BY <2>2, <4>2
      <3> QED BY <3>1
    <2>7. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      <3>1. ASSUME NEW nextAttempt \in rrAttempts'
             PROVE /\ nextAttempt.deliveryOrdinal <
                        rrNextDeliveryOrdinal'[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>0. rrAttempts' =
                 {ReplyAttemptAfterRetire(
                    owner, source, attempt):
                    attempt \in rrAttempts}
          BY <2>2
        <4>1. PICK oldAttempt \in rrAttempts:
                 nextAttempt =
                   ReplyAttemptAfterRetire(
                     owner, source, oldAttempt)
          BY <3>1, <4>0, SMTT(10)
        <4>2. /\ oldAttempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal[oldAttempt.owner]
                 /\ ReplyAttemptRetiredDeliveryWellFormed(oldAttempt)
                 /\ IF oldAttempt.ticketTenure =
                           NoReplyTicketTenure
                    THEN ReplyAttemptHasNoTicket(oldAttempt)
                    ELSE ReplyTicketValidForAttempt(oldAttempt)
          BY <1>1, <4>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4>3. LET retired ==
                   ReplyAttemptAfterRetire(
                     owner, source, oldAttempt)
               IN /\ SameReplyAttemptIdentity(oldAttempt, retired)
                  /\ retired.deliveryOrdinal =
                       oldAttempt.deliveryOrdinal
                  /\ retired.connectionTenure =
                       oldAttempt.connectionTenure
                  /\ retired.retiredDeliveryOrdinal =
                       oldAttempt.retiredDeliveryOrdinal
                  /\ retired.retiredConnectionTenure =
                       oldAttempt.retiredConnectionTenure
                  /\ IF oldAttempt.owner = owner
                           /\ oldAttempt.source = source
                     THEN ReplyAttemptHasNoTicket(retired)
                     ELSE retired = oldAttempt
          BY <1>1, <2>2, <4>1
        <4>4. CASE oldAttempt.owner = owner
                    /\ oldAttempt.source = source
          <5>1. nextAttempt.deliveryOrdinal <
                   rrNextDeliveryOrdinal'[nextAttempt.owner]
            BY <2>2, <4>1, <4>2, <4>3, <4>4
               DEF SameReplyAttemptIdentity
          <5>2. ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <4>1, <4>2, <4>3, <4>4, SMTT(10)
               DEF ReplyAttemptRetiredDeliveryWellFormed,
                   ReplyAttemptHasNoRetiredDelivery
          <5>3. /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <4>1, <4>3, <4>4
               DEF ReplyAttemptHasNoTicket
          <5> QED BY <5>1, <5>2, <5>3
        <4>5. CASE ~(oldAttempt.owner = owner
                     /\ oldAttempt.source = source)
          <5>1. nextAttempt = oldAttempt
            BY <4>1, <4>3, <4>5
          <5>2. /\ rrNextDeliveryOrdinal'[
                         oldAttempt.owner] =
                       rrNextDeliveryOrdinal[oldAttempt.owner]
                 /\ rrConnectionTenure'[
                         oldAttempt.owner][oldAttempt.source] =
                       rrConnectionTenure[
                         oldAttempt.owner][oldAttempt.source]
            BY <2>2
          <5>3. /\ rrSourceActive
                       \in [ReplyOwners ->
                             [ReplySources -> BOOLEAN]]
                 /\ oldAttempt.owner \in ReplyOwners
                 /\ oldAttempt.source \in ReplySources
                 /\ (oldAttempt.owner # owner
                      \/ oldAttempt.source # source)
            BY <1>1, <4>1, <4>5, SMTT(10)
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant,
                   ReplyAttemptSet
          <5>4. [rrSourceActive EXCEPT
                    ![owner][source] = FALSE]
                    [oldAttempt.owner][oldAttempt.source] =
                  rrSourceActive[
                    oldAttempt.owner][oldAttempt.source]
            BY <5>3, ReplyNestedFunctionalUpdateAwayFromKey
          <5>5. rrSourceActive'[
                     oldAttempt.owner][oldAttempt.source] =
                   rrSourceActive[
                     oldAttempt.owner][oldAttempt.source]
            BY <2>2, <5>4
          <5> QED BY <1>1, <4>1, <5>1, <5>2,
               <5>5,
               ReplyPointwiseRouteStatePreservesAttemptMetadata
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
        <4> QED BY <4>4, <4>5
      <3> QED BY <3>1
    <2>8. ReplyRouteOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>3, <2>8
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ReconnectReplySourcePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ ReconnectReplySource(owner, semantic, source)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                ReconnectReplySource(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt, deliveryOrdinal, connectionTenure)
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ oldAttempt.owner = owner
                /\ oldAttempt.semantic = semantic
                /\ oldAttempt.source = source
                /\ deliveryOrdinal \in ReplyDeliveryOrdinals
                /\ connectionTenure \in ReplyConnectionTenures
                /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
                /\ connectionTenure # oldAttempt.connectionTenure
                /\ routed \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, routed)
                /\ ReplyAttemptCursor(routed) =
                     ReplyAttemptCursor(oldAttempt)
                /\ routed.deliveryOrdinal = deliveryOrdinal
                /\ routed.connectionTenure = connectionTenure
                /\ routed.retiredDeliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ routed.retiredConnectionTenure =
                     oldAttempt.connectionTenure
                /\ ReplyAttemptHasNoTicket(routed)
                /\ (\A attempt \in rrAttempts:
                      LET transformed ==
                            ReplyAttemptAfterReconnectTransform(
                              oldAttempt, routed, attempt)
                      IN /\ attempt \in ReplyAttemptSet
                         /\ transformed \in ReplyAttemptSet
                         /\ SameReplyAttemptIdentity(
                              attempt, transformed)
                         /\ ReplyAttemptCursor(transformed) =
                              ReplyAttemptCursor(attempt)
                         /\ IF attempt = oldAttempt
                            THEN /\ transformed = routed
                                 /\ ReplyAttemptHasNoTicket(transformed)
                            ELSE IF attempt.owner = oldAttempt.owner
                                      /\ attempt.source =
                                           oldAttempt.source
                                 THEN
                                   /\ transformed =
                                        ReplyAttemptWithoutTicket(
                                          attempt)
                                   /\ transformed.deliveryOrdinal =
                                        attempt.deliveryOrdinal
                                   /\ transformed.connectionTenure =
                                        attempt.connectionTenure
                                   /\ transformed.retiredDeliveryOrdinal =
                                        attempt.retiredDeliveryOrdinal
                                   /\ transformed.retiredConnectionTenure =
                                        attempt.retiredConnectionTenure
                                   /\ ReplyAttemptHasNoTicket(transformed)
                                 ELSE transformed = attempt)
                /\ rrAttempts' =
                     {ReplyAttemptAfterReconnectTransform(
                        oldAttempt, routed, attempt):
                        attempt \in rrAttempts}
                /\ rrPayloads' = rrPayloads
                /\ rrConnectionTenure' =
                     [rrConnectionTenure EXCEPT
                        ![owner][source] = connectionTenure]
                /\ rrSourceActive' =
                     [rrSourceActive EXCEPT
                        ![owner][source] = TRUE]
                /\ rrNextDeliveryOrdinal' =
                     [rrNextDeliveryOrdinal EXCEPT
                        ![owner] = @ + 1]
                /\ rrNextServiceIndex' = rrNextServiceIndex
      <3>1. /\ ReplyRouteConfiguration
             /\ ReplyRouteSafetyInvariant
        BY <1>1 DEF ReplyRouteInductiveInvariant
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
               IN /\ oldAttempt \in rrAttempts
                  /\ oldAttempt \in ReplyAttemptSet
                  /\ oldAttempt.owner = owner
                  /\ oldAttempt.semantic = semantic
                  /\ oldAttempt.source = source
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN /\ oldAttempt \in rrAttempts
                  /\ oldAttempt.owner = owner
                  /\ oldAttempt.semantic = semantic
                  /\ oldAttempt.source = source
          BY <1>1, ReplyOwnedAttemptIdentity
             DEF ReconnectReplySource
        <4> QED BY <3>1, <4>1
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
      <3>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
               IN /\ deliveryOrdinal \in ReplyDeliveryOrdinals
                  /\ connectionTenure \in ReplyConnectionTenures
                  /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
                  /\ connectionTenure # oldAttempt.connectionTenure
        BY <1>1, SMTT(10)
           DEF ReconnectReplySource
      <3>4. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt, deliveryOrdinal, connectionTenure)
               IN /\ routed \in ReplyAttemptSet
                  /\ SameReplyAttemptIdentity(oldAttempt, routed)
                  /\ ReplyAttemptCursor(routed) =
                       ReplyAttemptCursor(oldAttempt)
                  /\ routed.deliveryOrdinal = deliveryOrdinal
                  /\ routed.connectionTenure = connectionTenure
                  /\ routed.retiredDeliveryOrdinal =
                       oldAttempt.deliveryOrdinal
                  /\ routed.retiredConnectionTenure =
                       oldAttempt.connectionTenure
                  /\ ReplyAttemptHasNoTicket(routed)
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
               IN /\ routed \in ReplyAttemptSet
                  /\ SameReplyAttemptIdentity(oldAttempt, routed)
                  /\ ReplyAttemptCursor(routed) =
                       ReplyAttemptCursor(oldAttempt)
          BY <3>1, <3>2, <3>3,
             ReplyRouteRefreshPreservesAttemptType,
             ReplyRouteRefreshPreservesIdentityAndCursor
        <4>2. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
               IN /\ routed.deliveryOrdinal = deliveryOrdinal
                  /\ routed.connectionTenure = connectionTenure
                  /\ routed.retiredDeliveryOrdinal =
                       oldAttempt.deliveryOrdinal
                  /\ routed.retiredConnectionTenure =
                       oldAttempt.connectionTenure
                  /\ ReplyAttemptHasNoTicket(routed)
          BY <3>2, <3>3,
             ReplyRouteUpdateRecordsLatestRetiredDelivery,
             ReplyDifferentTenureRefreshClearsTicket,
             SMTT(10)
             DEF ReplyAttemptWithRoute, ReplyAttemptSet
        <4> QED BY <4>1, <4>2
      <3>5. \A attempt \in rrAttempts:
               LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
                   transformed ==
                     ReplyAttemptAfterReconnectTransform(
                       oldAttempt, routed, attempt)
               IN /\ attempt \in ReplyAttemptSet
                  /\ transformed \in ReplyAttemptSet
                  /\ SameReplyAttemptIdentity(attempt, transformed)
                  /\ ReplyAttemptCursor(transformed) =
                       ReplyAttemptCursor(attempt)
                  /\ IF attempt = oldAttempt
                     THEN /\ transformed = routed
                          /\ ReplyAttemptHasNoTicket(transformed)
                     ELSE IF attempt.owner = oldAttempt.owner
                               /\ attempt.source = oldAttempt.source
                          THEN
                            /\ transformed =
                                 ReplyAttemptWithoutTicket(attempt)
                            /\ transformed.deliveryOrdinal =
                                 attempt.deliveryOrdinal
                            /\ transformed.connectionTenure =
                                 attempt.connectionTenure
                            /\ transformed.retiredDeliveryOrdinal =
                                 attempt.retiredDeliveryOrdinal
                            /\ transformed.retiredConnectionTenure =
                                 attempt.retiredConnectionTenure
                            /\ ReplyAttemptHasNoTicket(transformed)
                          ELSE transformed = attempt
        <4>1. ASSUME NEW attempt \in rrAttempts
               PROVE LET oldAttempt ==
                           ReplyAttemptFor(owner, semantic, source)
                         deliveryOrdinal ==
                           rrNextDeliveryOrdinal[owner]
                         connectionTenure ==
                           rrConnectionTenure[owner][source] + 1
                         routed ==
                           ReplyAttemptWithRoute(
                             oldAttempt, deliveryOrdinal,
                             connectionTenure)
                         transformed ==
                           ReplyAttemptAfterReconnectTransform(
                             oldAttempt, routed, attempt)
                     IN /\ attempt \in ReplyAttemptSet
                        /\ transformed \in ReplyAttemptSet
                        /\ SameReplyAttemptIdentity(
                             attempt, transformed)
                        /\ ReplyAttemptCursor(transformed) =
                             ReplyAttemptCursor(attempt)
                        /\ IF attempt = oldAttempt
                           THEN /\ transformed = routed
                                /\ ReplyAttemptHasNoTicket(transformed)
                           ELSE IF
                             attempt.owner = oldAttempt.owner
                               /\ attempt.source = oldAttempt.source
                           THEN
                             /\ transformed =
                                  ReplyAttemptWithoutTicket(attempt)
                             /\ transformed.deliveryOrdinal =
                                  attempt.deliveryOrdinal
                             /\ transformed.connectionTenure =
                                  attempt.connectionTenure
                             /\ transformed.retiredDeliveryOrdinal =
                                  attempt.retiredDeliveryOrdinal
                             /\ transformed.retiredConnectionTenure =
                                  attempt.retiredConnectionTenure
                             /\ ReplyAttemptHasNoTicket(transformed)
                           ELSE transformed = attempt
          <5>1. attempt \in ReplyAttemptSet
            BY <3>1, <4>1
               DEF ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant
          <5>2. CASE LET oldAttempt ==
                            ReplyAttemptFor(
                              owner, semantic, source)
                        IN attempt = oldAttempt
            BY <3>4, <4>1, <5>1, <5>2, SMTT(10)
               DEF ReplyAttemptAfterReconnectTransform,
                   SameReplyAttemptIdentity
          <5>3. CASE LET oldAttempt ==
                            ReplyAttemptFor(
                              owner, semantic, source)
                        IN /\ attempt # oldAttempt
                           /\ attempt.owner = oldAttempt.owner
                           /\ attempt.source = oldAttempt.source
            <6>1. /\ ReplyAttemptWithoutTicket(attempt)
                         \in ReplyAttemptSet
                   /\ SameReplyAttemptIdentity(
                        attempt,
                        ReplyAttemptWithoutTicket(attempt))
                   /\ ReplyAttemptCursor(
                        ReplyAttemptWithoutTicket(attempt)) =
                        ReplyAttemptCursor(attempt)
              BY <3>1, <5>1,
                 ReplyTicketRemovalPreservesAttemptType,
                 ReplyTicketRemovalPreservesIdentityAndCursor
            <6>2. /\ ReplyAttemptWithoutTicket(attempt)
                         .deliveryOrdinal =
                       attempt.deliveryOrdinal
                   /\ ReplyAttemptWithoutTicket(attempt)
                         .connectionTenure =
                       attempt.connectionTenure
                   /\ ReplyAttemptWithoutTicket(attempt)
                         .retiredDeliveryOrdinal =
                       attempt.retiredDeliveryOrdinal
                   /\ ReplyAttemptWithoutTicket(attempt)
                         .retiredConnectionTenure =
                       attempt.retiredConnectionTenure
                   /\ ReplyAttemptHasNoTicket(
                        ReplyAttemptWithoutTicket(attempt))
              BY <5>1, SMTT(15)
                 DEF ReplyAttemptWithoutTicket,
                     ReplyAttemptHasNoTicket,
                     ReplyAttemptSet,
                     NoReplyTicketTenure
            <6> QED BY <5>1, <5>3, <6>1, <6>2, SMTT(10)
                 DEF ReplyAttemptAfterReconnectTransform
          <5>4. CASE LET oldAttempt ==
                            ReplyAttemptFor(
                              owner, semantic, source)
                        IN /\ attempt # oldAttempt
                           /\ ~(attempt.owner = oldAttempt.owner
                                /\ attempt.source =
                                     oldAttempt.source)
            BY <5>1, <5>4
               DEF ReplyAttemptAfterReconnectTransform,
                   SameReplyAttemptIdentity
          <5> QED BY <5>2, <5>3, <5>4
        <4> QED BY <4>1
      <3>6. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt, deliveryOrdinal, connectionTenure)
             IN /\ rrAttempts' =
                    {ReplyAttemptAfterReconnectTransform(
                       oldAttempt, routed, attempt):
                       attempt \in rrAttempts}
                /\ rrPayloads' = rrPayloads
                /\ rrConnectionTenure' =
                     [rrConnectionTenure EXCEPT
                        ![owner][source] = connectionTenure]
                /\ rrSourceActive' =
                     [rrSourceActive EXCEPT
                        ![owner][source] = TRUE]
                /\ rrNextDeliveryOrdinal' =
                     [rrNextDeliveryOrdinal EXCEPT
                        ![owner] = @ + 1]
                /\ rrNextServiceIndex' = rrNextServiceIndex
        BY <1>1
           DEF ReconnectReplySource,
               ReplyAttemptsAfterReconnect,
               ReplyAttemptAfterReconnectTransform
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5, <3>6
    <2>3. ReplyRouteTypeInvariant'
      <3>1. rrAttempts' \subseteq ReplyAttemptSet
        <4>1. ASSUME NEW nextAttempt \in rrAttempts'
               PROVE nextAttempt \in ReplyAttemptSet
          <5>1. PICK attempt \in rrAttempts:
                   LET oldAttempt ==
                         ReplyAttemptFor(owner, semantic, source)
                       deliveryOrdinal ==
                         rrNextDeliveryOrdinal[owner]
                       connectionTenure ==
                         rrConnectionTenure[owner][source] + 1
                       routed ==
                         ReplyAttemptWithRoute(
                           oldAttempt, deliveryOrdinal,
                           connectionTenure)
                   IN nextAttempt =
                        ReplyAttemptAfterReconnectTransform(
                          oldAttempt, routed, attempt)
            BY <2>2, <4>1
          <5> QED BY <2>2, <5>1
        <4> QED BY <4>1
      <3>2. /\ rrPayloads' \subseteq ReplySemantics
             /\ rrNextServiceIndex'
                  \in [ReplyOwners ->
                        [ReplySemantics ->
                          1..Len(ReplySourceOrder)]]
        BY <1>1, <2>2
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3>3. rrNextDeliveryOrdinal'
                 \in [ReplyOwners ->
                       1..(ReplyDeliveryOrdinalLimit + 1)]
        <4>1. rrNextDeliveryOrdinal
                   \in [ReplyOwners ->
                         1..(ReplyDeliveryOrdinalLimit + 1)]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>2. /\ rrNextDeliveryOrdinal[owner]
                    \in ReplyDeliveryOrdinals
               /\ ReplyDeliveryOrdinalLimit \in Nat
               /\ ReplyDeliveryOrdinalLimit >= 1
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteConfiguration,
                 ReconnectReplySource
        <4>3. /\ rrNextDeliveryOrdinal[owner] \in Nat
               /\ rrNextDeliveryOrdinal[owner] + 1
                    \in 1..(ReplyDeliveryOrdinalLimit + 1)
          BY <4>2, SMTT(10) DEF ReplyDeliveryOrdinals
        <4> QED BY <2>2, <4>1, <4>3, Isa
      <3>4. rrConnectionTenure'
                 \in [ReplyOwners ->
                       [ReplySources -> ReplyConnectionTenures]]
        <4>1. rrConnectionTenure
                   \in [ReplyOwners ->
                         [ReplySources -> ReplyConnectionTenures]]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>2. LET connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
               IN connectionTenure \in ReplyConnectionTenures
          BY <1>1 DEF ReconnectReplySource
        <4> QED BY <2>2, <4>1, <4>2,
             ReplyNestedFunctionalUpdatePreservesType
      <3>5. rrSourceActive'
                 \in [ReplyOwners ->
                       [ReplySources -> BOOLEAN]]
        <4>1. rrSourceActive
                   \in [ReplyOwners ->
                         [ReplySources -> BOOLEAN]]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4> QED BY <2>2, <4>1,
             ReplyNestedFunctionalUpdatePreservesType
      <3> QED BY <3>1, <3>2, <3>3, <3>4, <3>5
           DEF ReplyRouteTypeInvariant
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))'
             /\ Cardinality(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))' <= 1
      <3>1. ASSUME NEW nextOwner \in ReplyOwners,
                    NEW nextSemantic \in ReplySemantics,
                    NEW nextSource \in ReplySources
             PROVE /\ IsFiniteSet(
                          ReplyAttemptsForSource(
                            nextOwner, nextSemantic, nextSource))'
                   /\ Cardinality(
                          ReplyAttemptsForSource(
                            nextOwner, nextSemantic, nextSource))' <= 1
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
               IN ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource)' =
                    {ReplyAttemptAfterReconnectTransform(
                       oldAttempt, routed, attempt):
                       attempt \in ReplyAttemptsForSource(
                         nextOwner, nextSemantic, nextSource)}
          BY <2>2, SMTT(30)
             DEF SameReplyAttemptIdentity,
                 ReplyAttemptsForSource, ReplyAttemptsFor
        <4>2. /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource))
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource)) <= 1
          BY <2>2, <3>1
             DEF ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4>3. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
                   image ==
                     {ReplyAttemptAfterReconnectTransform(
                        oldAttempt, routed, attempt):
                        attempt \in ReplyAttemptsForSource(
                          nextOwner, nextSemantic, nextSource)}
               IN /\ IsFiniteSet(image)
                  /\ Cardinality(image) <=
                       Cardinality(
                         ReplyAttemptsForSource(
                           nextOwner, nextSemantic, nextSource))
          BY <4>2, FS_Image
        <4>4. IsFiniteSet(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))'
          BY <4>1, <4>3
        <4>5. Cardinality(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))' <=
                   Cardinality(
                     ReplyAttemptsForSource(
                       nextOwner, nextSemantic, nextSource))
          BY <4>1, <4>3
        <4>6. /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource)) \in Nat
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        nextOwner, nextSemantic, nextSource))' \in Nat
          BY <4>2, <4>4, FS_CardinalityType
        <4>7. Cardinality(
                 ReplyAttemptsForSource(
                   nextOwner, nextSemantic, nextSource))' <= 1
          BY <4>2, <4>5, <4>6, SMTT(10)
        <4> QED BY <4>4, <4>7
      <3> QED BY <3>1
    <2>5. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
      BY <2>1, <2>3, ReplyNextTypeBoundsSourceGeometry
    <2>6. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
               nextSemantic \in rrPayloads'
      <3>1. ASSUME NEW nextOwner \in ReplyOwners,
                    NEW nextSemantic \in ReplySemantics,
                    ReplyAttemptsFor(
                      nextOwner, nextSemantic)' # {}
             PROVE nextSemantic \in rrPayloads'
        <4>1. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   deliveryOrdinal ==
                     rrNextDeliveryOrdinal[owner]
                   connectionTenure ==
                     rrConnectionTenure[owner][source] + 1
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt, deliveryOrdinal, connectionTenure)
               IN ReplyAttemptsFor(nextOwner, nextSemantic) # {}
          BY <2>2, <3>1, SMTT(30)
             DEF SameReplyAttemptIdentity,
                 ReplyAttemptsFor
        <4>2. nextSemantic \in rrPayloads
          BY <1>1, <4>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4> QED BY <2>2, <4>2
      <3> QED BY <3>1
    <2>7. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      <3>1. ASSUME NEW nextAttempt \in rrAttempts'
             PROVE /\ nextAttempt.deliveryOrdinal <
                        rrNextDeliveryOrdinal'[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>0. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt, deliveryOrdinal, connectionTenure)
             IN rrAttempts' =
                  {ReplyAttemptAfterReconnectTransform(
                     oldAttempt, routed, attempt):
                     attempt \in rrAttempts}
          BY <2>2
        <4>1. PICK attempt \in rrAttempts:
                 LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                     deliveryOrdinal ==
                       rrNextDeliveryOrdinal[owner]
                     connectionTenure ==
                       rrConnectionTenure[owner][source] + 1
                     routed ==
                       ReplyAttemptWithRoute(
                         oldAttempt, deliveryOrdinal, connectionTenure)
                 IN nextAttempt =
                      ReplyAttemptAfterReconnectTransform(
                        oldAttempt, routed, attempt)
          BY <3>1, <4>0, SMTT(10)
        <4>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 deliveryOrdinal ==
                   rrNextDeliveryOrdinal[owner]
                 connectionTenure ==
                   rrConnectionTenure[owner][source] + 1
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt, deliveryOrdinal, connectionTenure)
             IN /\ SameReplyAttemptIdentity(oldAttempt, routed)
                /\ routed.deliveryOrdinal = deliveryOrdinal
                /\ routed.connectionTenure = connectionTenure
                /\ routed.retiredDeliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ routed.retiredConnectionTenure =
                     oldAttempt.connectionTenure
                /\ ReplyAttemptHasNoTicket(routed)
          BY <2>2
        <4>3. rrNextDeliveryOrdinal'[owner] =
                 rrNextDeliveryOrdinal[owner] + 1
          BY <1>1, <2>2,
             ReplyFunctionalUpdateAtKey
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>4. CASE attempt =
                      ReplyAttemptFor(owner, semantic, source)
          <5>1. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                     routed ==
                       ReplyAttemptWithRoute(
                         oldAttempt,
                         rrNextDeliveryOrdinal[owner],
                         rrConnectionTenure[owner][source] + 1)
                 IN nextAttempt = routed
            BY <4>1, <4>4
               DEF ReplyAttemptAfterReconnectTransform
          <5>2. rrNextDeliveryOrdinal[owner] \in Nat
            BY <1>1, Isa DEF ReconnectReplySource,
                 ReplyDeliveryOrdinals
          <5>3. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                 IN /\ nextAttempt.owner = owner
                    /\ nextAttempt.deliveryOrdinal =
                         rrNextDeliveryOrdinal[owner]
                    /\ nextAttempt.retiredDeliveryOrdinal =
                         oldAttempt.deliveryOrdinal
                    /\ nextAttempt.retiredConnectionTenure =
                         oldAttempt.connectionTenure
                    /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <2>2, <4>2, <4>4, <5>1
               DEF SameReplyAttemptIdentity
          <5>4. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                 IN /\ oldAttempt.deliveryOrdinal
                          \in ReplyDeliveryOrdinals
                    /\ oldAttempt.connectionTenure
                          \in ReplyConnectionTenures
                    /\ oldAttempt.deliveryOrdinal <
                         rrNextDeliveryOrdinal[owner]
            BY <2>2, SMTT(10)
               DEF ReplyAttemptSet
          <5>5. nextAttempt.deliveryOrdinal <
                   rrNextDeliveryOrdinal'[nextAttempt.owner]
            BY <4>3, <5>2, <5>3, SMTT(5)
          <5>6. ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <5>3, <5>4, SMTT(10)
               DEF ReplyAttemptRetiredDeliveryWellFormed
          <5>7. /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <5>3 DEF ReplyAttemptHasNoTicket
          <5> QED BY <5>5, <5>6, <5>7
        <4>5. CASE /\ attempt #
                          ReplyAttemptFor(owner, semantic, source)
                     /\ attempt.owner =
                          ReplyAttemptFor(
                            owner, semantic, source).owner
                     /\ attempt.source =
                          ReplyAttemptFor(
                            owner, semantic, source).source
          <5>1. /\ nextAttempt =
                       ReplyAttemptWithoutTicket(attempt)
                 /\ nextAttempt.deliveryOrdinal =
                       attempt.deliveryOrdinal
                 /\ nextAttempt.connectionTenure =
                       attempt.connectionTenure
                 /\ nextAttempt.retiredDeliveryOrdinal =
                       attempt.retiredDeliveryOrdinal
                 /\ nextAttempt.retiredConnectionTenure =
                       attempt.retiredConnectionTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <2>2, <4>1, <4>5
          <5>2. /\ attempt \in rrAttempts
                 /\ attempt.owner = owner
                 /\ attempt.source = source
            BY <2>2, <4>1, <4>5
               DEF SameReplyAttemptIdentity
          <5>3. /\ attempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal[attempt.owner]
                 /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
            BY <1>1, <5>2
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteOwnershipInvariant
          <5>4. /\ SameReplyAttemptIdentity(attempt, nextAttempt)
                 /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
            BY <2>2, <4>1, <5>1
               DEF ReplyAttemptHasNoTicket
          <5>5. /\ attempt.deliveryOrdinal \in Nat
                 /\ rrNextDeliveryOrdinal[attempt.owner] \in Nat
            BY <1>1, <5>2, SMTT(10)
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant,
                   ReplyAttemptSet,
                   ReplyDeliveryOrdinals
          <5>6. nextAttempt.deliveryOrdinal <
                   rrNextDeliveryOrdinal'[nextAttempt.owner]
            BY <4>3, <5>1, <5>2, <5>3, <5>4, <5>5,
               SMTT(5)
               DEF SameReplyAttemptIdentity
          <5>7. ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <5>1, <5>3, <5>4, SMTT(10)
               DEF ReplyAttemptRetiredDeliveryWellFormed,
                   ReplyAttemptHasNoRetiredDelivery,
                   SameReplyAttemptIdentity
          <5>8. /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <5>1, <5>4
          <5> QED BY <5>6, <5>7, <5>8
        <4>6. CASE /\ attempt #
                          ReplyAttemptFor(owner, semantic, source)
                     /\ ~(attempt.owner =
                            ReplyAttemptFor(
                              owner, semantic, source).owner
                          /\ attempt.source =
                            ReplyAttemptFor(
                              owner, semantic, source).source)
          <5>1. /\ attempt \in rrAttempts
                 /\ nextAttempt = attempt
                 /\ attempt.owner \in ReplyOwners
                 /\ attempt.source \in ReplySources
                 /\ (attempt.owner # owner
                      \/ attempt.source # source)
            BY <2>2, <4>1, <4>6
               DEF SameReplyAttemptIdentity,
                   ReplyAttemptSet
          <5>2. /\ rrConnectionTenure'[
                       attempt.owner][attempt.source] =
                    rrConnectionTenure[
                       attempt.owner][attempt.source]
                 /\ rrSourceActive'[
                       attempt.owner][attempt.source] =
                    rrSourceActive[
                       attempt.owner][attempt.source]
            <6>1. /\ rrConnectionTenure
                         \in [ReplyOwners ->
                               [ReplySources ->
                                 ReplyConnectionTenures]]
                   /\ rrSourceActive
                         \in [ReplyOwners ->
                               [ReplySources -> BOOLEAN]]
              BY <1>1
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteTypeInvariant
            <6>2. /\ [rrConnectionTenure EXCEPT
                         ![owner][source] =
                           rrConnectionTenure[owner][source] + 1]
                         [attempt.owner][attempt.source] =
                       rrConnectionTenure[
                         attempt.owner][attempt.source]
                   /\ [rrSourceActive EXCEPT
                         ![owner][source] = TRUE]
                         [attempt.owner][attempt.source] =
                       rrSourceActive[
                         attempt.owner][attempt.source]
              BY <5>1, <6>1,
                 ReplyNestedFunctionalUpdateAwayFromKey
            <6> QED BY <2>2, <6>2
          <5>3. /\ rrNextDeliveryOrdinal'[attempt.owner] \in Nat
                 /\ rrNextDeliveryOrdinal'[attempt.owner] >=
                      rrNextDeliveryOrdinal[attempt.owner]
            <6>1. /\ rrNextDeliveryOrdinal
                        \in [ReplyOwners ->
                              1..(ReplyDeliveryOrdinalLimit + 1)]
                   /\ attempt.owner \in ReplyOwners
              BY <1>1, <5>1
                 DEF ReplyRouteInductiveInvariant,
                     ReplyRouteSafetyInvariant,
                     ReplyRouteTypeInvariant
            <6>2. CASE attempt.owner = owner
              BY <1>1, <2>2, <5>1, <6>1, <6>2,
                 ReplyFunctionalUpdateAtKey, SMTT(5)
            <6>3. CASE attempt.owner # owner
              BY <2>2, <6>1, <6>3,
                 ReplyFunctionalUpdateAwayFromKey
            <6> QED BY <6>2, <6>3
          <5> QED BY <1>1, <5>1, <5>2, <5>3,
               ReplyNonregressingPointwiseRouteStatePreservesAttemptMetadata
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
        <4> QED BY <4>4, <4>5, <4>6
      <3> QED BY <3>1
    <2>8. ReplyRouteOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>3, <2>8
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

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
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                AcquireReplyTicket(owner, semantic, source)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ ReplyAttemptCurrent(oldAttempt)
                /\ ticketed \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, ticketed)
                /\ ReplyAttemptCursor(ticketed) =
                     ReplyAttemptCursor(oldAttempt)
                /\ ticketed.deliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ ticketed.connectionTenure =
                     oldAttempt.connectionTenure
                /\ ticketed.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ ticketed.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ticketed.ticketTenure =
                     oldAttempt.connectionTenure
                /\ ticketed.ticketSemantic = {oldAttempt.semantic}
                /\ ticketed.ticketTarget =
                     {ReplySemanticTarget(oldAttempt.semantic)}
                /\ ticketed.ticketMessageCursor =
                     {oldAttempt.messageCursor}
                /\ ticketed.ticketChunkCursor =
                     {oldAttempt.chunkCursor}
                /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, ticketed)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
                /\ rrNextServiceIndex' = rrNextServiceIndex
      <3>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ ReplyAttemptCurrent(oldAttempt)
        BY <1>1, ReplyOwnedAttemptIdentity
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant,
               AcquireReplyTicket
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
             IN /\ ticketed \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, ticketed)
                /\ ReplyAttemptCursor(ticketed) =
                     ReplyAttemptCursor(oldAttempt)
        BY <1>1, <3>1,
           ReplyTicketAcquisitionPreservesAttemptType,
           ReplyTicketAcquisitionPreservesIdentityAndCursor
           DEF ReplyRouteInductiveInvariant
      <3>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
             IN /\ ticketed.deliveryOrdinal =
                       oldAttempt.deliveryOrdinal
                /\ ticketed.connectionTenure =
                     oldAttempt.connectionTenure
                /\ ticketed.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ ticketed.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ticketed.ticketTenure =
                     oldAttempt.connectionTenure
                /\ ticketed.ticketSemantic = {oldAttempt.semantic}
                /\ ticketed.ticketTarget =
                     {ReplySemanticTarget(oldAttempt.semantic)}
                /\ ticketed.ticketMessageCursor =
                     {oldAttempt.messageCursor}
                /\ ticketed.ticketChunkCursor =
                     {oldAttempt.chunkCursor}
        BY <3>1, SMTT(15)
           DEF ReplyAttemptWithTicket, ReplyAttemptSet
      <3>4. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
             IN /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, ticketed)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
                /\ rrNextServiceIndex' = rrNextServiceIndex
        BY <1>1 DEF AcquireReplyTicket
      <3> QED BY <3>1, <3>2, <3>3, <3>4
    <2>3. ReplyRouteTypeInvariant'
      <3>1. rrAttempts' \subseteq ReplyAttemptSet
        BY <1>1, <2>2, SMTT(20)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant,
               ReplaceReplyAttempt
      <3>2. /\ rrPayloads' \subseteq ReplySemantics
             /\ rrNextDeliveryOrdinal'
                  \in [ReplyOwners ->
                        1..(ReplyDeliveryOrdinalLimit + 1)]
             /\ rrConnectionTenure'
                  \in [ReplyOwners ->
                        [ReplySources -> ReplyConnectionTenures]]
             /\ rrSourceActive'
                  \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
             /\ rrNextServiceIndex'
                  \in [ReplyOwners ->
                        [ReplySemantics ->
                          1..Len(ReplySourceOrder)]]
        BY <1>1, <2>2
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3> QED BY <3>1, <3>2 DEF ReplyRouteTypeInvariant
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))'
             /\ Cardinality(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))' <= 1
      BY <2>2, ReplyIdentityReplacementPreservesSourceOwnership
    <2>5. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
      BY <2>1, <2>3, ReplyNextTypeBoundsSourceGeometry
    <2>6. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
               nextSemantic \in rrPayloads'
      BY <1>1, <2>2,
         ReplyIdentityReplacementPreservesPayloadOwnership
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
    <2>7. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      <3>1. ASSUME NEW nextAttempt \in rrAttempts'
             PROVE /\ nextAttempt.deliveryOrdinal <
                        rrNextDeliveryOrdinal'[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>1. CASE nextAttempt =
                    ReplyAttemptWithTicket(
                      ReplyAttemptFor(owner, semantic, source))
          <5>1. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                 IN /\ oldAttempt.deliveryOrdinal <
                          rrNextDeliveryOrdinal[oldAttempt.owner]
                    /\ ReplyAttemptRetiredDeliveryWellFormed(
                         oldAttempt)
            BY <1>1, <2>2
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteOwnershipInvariant
          <5>2. /\ nextAttempt.deliveryOrdinal <
                       rrNextDeliveryOrdinal'[nextAttempt.owner]
                 /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <2>2, <4>1, <5>1, SMTT(10)
               DEF SameReplyAttemptIdentity,
                   ReplyAttemptRetiredDeliveryWellFormed,
                   ReplyAttemptHasNoRetiredDelivery
          <5>3. nextAttempt.ticketTenure #
                   NoReplyTicketTenure
            BY <1>1, <2>2, <4>1, SMTT(10)
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant,
                   ReplyAttemptSet, ReplyConnectionTenures,
                   NoReplyTicketTenure
          <5>4. ReplyAttemptCurrent(nextAttempt)'
            BY <2>2, <4>1, SMTT(10)
               DEF ReplyAttemptCurrent,
                   SameReplyAttemptIdentity
          <5>5. nextAttempt.ticketTenure =
                   nextAttempt.connectionTenure
            BY <2>2, <4>1
          <5>6. /\ nextAttempt.ticketSemantic =
                       {nextAttempt.semantic}
                 /\ nextAttempt.ticketTarget =
                      {ReplySemanticTarget(nextAttempt.semantic)}
                 /\ nextAttempt.ticketTenure =
                      rrConnectionTenure'[
                        nextAttempt.owner][nextAttempt.source]
                 /\ nextAttempt.ticketMessageCursor =
                      {nextAttempt.messageCursor}
                 /\ nextAttempt.ticketChunkCursor =
                      {nextAttempt.chunkCursor}
            BY <2>2, <4>1, <5>4, <5>5, SMTT(10)
               DEF ReplyAttemptCurrent,
                   ReplyAttemptCursor,
                   SameReplyAttemptIdentity
          <5>7. ReplyTicketForAttempt(nextAttempt) =
                   ReplyTicket(
                     nextAttempt.owner, nextAttempt.source,
                     nextAttempt.semantic,
                     ReplySemanticTarget(nextAttempt.semantic),
                     rrConnectionTenure'[
                       nextAttempt.owner][nextAttempt.source],
                     nextAttempt.messageCursor,
                     nextAttempt.chunkCursor)
            BY <5>6, Isa
               DEF ReplyTicketForAttempt, ReplyTicket
          <5>8. ReplyTicketValidForAttempt(nextAttempt)'
            BY <5>4, <5>5, <5>7
               DEF ReplyTicketValidForAttempt
          <5> QED BY <5>2, <5>3, <5>8
        <4>2. CASE nextAttempt #
                    ReplyAttemptWithTicket(
                      ReplyAttemptFor(owner, semantic, source))
          <5>1. nextAttempt \in rrAttempts
            BY <2>2, <3>1, <4>2,
               ReplyReplacementRetainsOtherAttempt
          <5> QED BY <1>1, <2>2, <5>1,
               ReplyUnchangedRouteMapsPreserveAttemptMetadata
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>8. ReplyRouteOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>3, <2>8
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

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
    <2>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
                /\ serviced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, serviced)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
                /\ serviced.deliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ serviced.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ serviced.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ReplyAttemptHasNoTicket(serviced)
      <3>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
             IN /\ ReplyRouteSafetyInvariant
                /\ oldAttempt \in rrAttempts
        BY <1>1, ReplyOwnedAttemptIdentity
           DEF ReplyRouteInductiveInvariant,
               AdvanceCurrentReplyAttempt
      <3>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ serviced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ serviced.deliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ serviced.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ serviced.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ReplyAttemptHasNoTicket(serviced)
        BY <1>1
           DEF AdvanceCurrentReplyAttempt,
               ReplyAttemptServiceKernelValid,
               SameReplyAttemptIdentity
      <3>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, serviced)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
        BY <1>1 DEF AdvanceCurrentReplyAttempt
      <3> QED BY <3>1, <3>2, <3>3
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))'
             /\ Cardinality(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))' <= 1
      BY <2>3, ReplyIdentityReplacementPreservesSourceOwnership
    <2>5. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
      BY <2>1, <2>2, ReplyNextTypeBoundsSourceGeometry
    <2>6. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
               nextSemantic \in rrPayloads'
      BY <1>1, <2>3,
         ReplyIdentityReplacementPreservesPayloadOwnership
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
    <2>7. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      <3>1. ASSUME NEW nextAttempt \in rrAttempts'
             PROVE /\ nextAttempt.deliveryOrdinal <
                        rrNextDeliveryOrdinal'[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>1. CASE nextAttempt =
                    ReplyAttemptAfterService(
                      ReplyAttemptFor(owner, semantic, source))
          <5>1. LET oldAttempt ==
                       ReplyAttemptFor(owner, semantic, source)
                 IN /\ oldAttempt.deliveryOrdinal <
                          rrNextDeliveryOrdinal[oldAttempt.owner]
                    /\ ReplyAttemptRetiredDeliveryWellFormed(
                         oldAttempt)
            BY <1>1, <2>3
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteOwnershipInvariant
          <5>2. nextAttempt.deliveryOrdinal <
                   rrNextDeliveryOrdinal'[nextAttempt.owner]
            BY <2>3, <4>1, <5>1
               DEF SameReplyAttemptIdentity
          <5>3. ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <2>3, <4>1, <5>1, SMTT(10)
               DEF ReplyAttemptRetiredDeliveryWellFormed,
                   ReplyAttemptHasNoRetiredDelivery
          <5>4. /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <2>3, <4>1 DEF ReplyAttemptHasNoTicket
          <5> QED BY <5>2, <5>3, <5>4
        <4>2. CASE nextAttempt #
                    ReplyAttemptAfterService(
                      ReplyAttemptFor(owner, semantic, source))
          <5>1. nextAttempt \in rrAttempts
            BY <2>3, <3>1, <4>2,
               ReplyReplacementRetainsOtherAttempt
          <5> QED BY <1>1, <2>3, <5>1,
               ReplyUnchangedRouteMapsPreserveAttemptMetadata
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>8. ReplyRouteOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>8
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ServiceReplyRoutePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteInductiveInvariant
    /\ ServiceReplyRoute(owner, semantic)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                ReplyRouteInductiveInvariant,
                ServiceReplyRoute(owner, semantic)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 source == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ ReplyRouteSafetyInvariant
                /\ selectedIndex \in 1..Len(ReplySourceOrder)
                /\ source \in ReplySources
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ ReplyAttemptCurrent(oldAttempt)
                /\ ReplyTicketValidForAttempt(oldAttempt)
                /\ ~ReplyAttemptComplete(oldAttempt)
                /\ serviced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ serviced.deliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ serviced.connectionTenure =
                     oldAttempt.connectionTenure
                /\ serviced.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ serviced.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ReplyAttemptHasNoTicket(serviced)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, serviced)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
                /\ rrNextServiceIndex' =
                     [rrNextServiceIndex EXCEPT
                        ![owner][semantic] =
                          NextReplySourceIndex(selectedIndex)]
      <3>1. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 source == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
             IN /\ ReplyRouteSafetyInvariant
                /\ selectedIndex \in 1..Len(ReplySourceOrder)
                /\ source \in ReplySources
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ ReplyAttemptCurrent(oldAttempt)
                /\ ReplyTicketValidForAttempt(oldAttempt)
                /\ ~ReplyAttemptComplete(oldAttempt)
        BY <1>1, ReplySelectedPendingAttemptFacts
           DEF ReplyRouteInductiveInvariant,
               ServiceReplyRoute
      <3>2. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 source == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ serviced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ serviced.deliveryOrdinal =
                     oldAttempt.deliveryOrdinal
                /\ serviced.connectionTenure =
                     oldAttempt.connectionTenure
                /\ serviced.retiredDeliveryOrdinal =
                     oldAttempt.retiredDeliveryOrdinal
                /\ serviced.retiredConnectionTenure =
                     oldAttempt.retiredConnectionTenure
                /\ ReplyAttemptHasNoTicket(serviced)
        BY <1>1, <3>1,
           ReplyServicePreservesAttemptType,
           ReplyServicePreservesIdentity,
           SMTT(20)
           DEF ReplyRouteInductiveInvariant,
               ReplyAttemptAfterService,
               ReplyAttemptSet,
               ReplyAttemptHasNoTicket,
               NoReplyTicketTenure
      <3>3. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 source == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, serviced)
                /\ rrPayloads' = rrPayloads
                /\ rrNextDeliveryOrdinal' =
                     rrNextDeliveryOrdinal
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
                /\ rrNextServiceIndex' =
                     [rrNextServiceIndex EXCEPT
                        ![owner][semantic] =
                          NextReplySourceIndex(selectedIndex)]
        BY <1>1 DEF ServiceReplyRoute
      <3> QED BY <3>1, <3>2, <3>3
    <2>3. ReplyRouteTypeInvariant'
      <3>1. rrAttempts' \subseteq ReplyAttemptSet
        BY <1>1, <2>2, SMTT(20)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant,
               ReplaceReplyAttempt
      <3>2. /\ rrPayloads' \subseteq ReplySemantics
             /\ rrNextDeliveryOrdinal'
                  \in [ReplyOwners ->
                        1..(ReplyDeliveryOrdinalLimit + 1)]
             /\ rrConnectionTenure'
                  \in [ReplyOwners ->
                        [ReplySources -> ReplyConnectionTenures]]
             /\ rrSourceActive'
                  \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
        BY <1>1, <2>2
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3>3. rrNextServiceIndex'
                 \in [ReplyOwners ->
                       [ReplySemantics ->
                         1..Len(ReplySourceOrder)]]
        <4>1. NextReplySourceIndex(
                 ReplySelectedSourceIndex(owner, semantic))
                 \in 1..Len(ReplySourceOrder)
          BY <1>1, <2>2, ReplyNextSourceIndexTyped
             DEF ReplyRouteInductiveInvariant
        <4>2. rrNextServiceIndex
                   \in [ReplyOwners ->
                         [ReplySemantics ->
                           1..Len(ReplySourceOrder)]]
          BY <1>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4> QED BY <2>2, <4>1, <4>2,
             ReplyNestedFunctionalUpdatePreservesType
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyRouteTypeInvariant
    <2>4. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics,
               nextSource \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))'
             /\ Cardinality(
                  ReplyAttemptsForSource(
                    nextOwner, nextSemantic, nextSource))' <= 1
      BY <2>2, ReplyIdentityReplacementPreservesSourceOwnership
    <2>5. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             /\ Cardinality(
                  ReplyAttemptSources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(
                    nextOwner, nextSemantic))' <=
                  ReplySourceCapacity
      BY <2>1, <2>3, ReplyNextTypeBoundsSourceGeometry
    <2>6. \A nextOwner \in ReplyOwners,
               nextSemantic \in ReplySemantics:
             ReplyAttemptsFor(nextOwner, nextSemantic)' # {} =>
               nextSemantic \in rrPayloads'
      BY <1>1, <2>2,
         ReplyIdentityReplacementPreservesPayloadOwnership
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
    <2>7. \A nextAttempt \in rrAttempts':
             /\ nextAttempt.deliveryOrdinal <
                  rrNextDeliveryOrdinal'[nextAttempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
             /\ IF nextAttempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(nextAttempt)
                ELSE ReplyTicketValidForAttempt(nextAttempt)'
      <3>1. ASSUME NEW nextAttempt \in rrAttempts'
             PROVE /\ nextAttempt.deliveryOrdinal <
                        rrNextDeliveryOrdinal'[nextAttempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(
                        nextAttempt)
                   /\ IF nextAttempt.ticketTenure =
                             NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(nextAttempt)
                      ELSE ReplyTicketValidForAttempt(nextAttempt)'
        <4>1. CASE nextAttempt =
                    ReplyAttemptAfterService(
                      ReplyAttemptFor(
                        owner, semantic,
                        ReplySourceOrder[
                          ReplySelectedSourceIndex(
                            owner, semantic)]))
          <5>1. LET oldAttempt ==
                       ReplyAttemptFor(
                         owner, semantic,
                         ReplySourceOrder[
                           ReplySelectedSourceIndex(
                             owner, semantic)])
                 IN /\ oldAttempt.deliveryOrdinal <
                          rrNextDeliveryOrdinal[oldAttempt.owner]
                    /\ ReplyAttemptRetiredDeliveryWellFormed(
                         oldAttempt)
            BY <1>1, <2>2
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteOwnershipInvariant
          <5>2. nextAttempt.deliveryOrdinal <
                   rrNextDeliveryOrdinal'[nextAttempt.owner]
            BY <2>2, <4>1, <5>1
               DEF SameReplyAttemptIdentity
          <5>3. ReplyAttemptRetiredDeliveryWellFormed(nextAttempt)
            BY <2>2, <4>1, <5>1, SMTT(10)
               DEF ReplyAttemptRetiredDeliveryWellFormed,
                   ReplyAttemptHasNoRetiredDelivery
          <5>4. /\ nextAttempt.ticketTenure =
                       NoReplyTicketTenure
                 /\ ReplyAttemptHasNoTicket(nextAttempt)
            BY <2>2, <4>1 DEF ReplyAttemptHasNoTicket
          <5> QED BY <5>2, <5>3, <5>4
        <4>2. CASE nextAttempt #
                    ReplyAttemptAfterService(
                      ReplyAttemptFor(
                        owner, semantic,
                        ReplySourceOrder[
                          ReplySelectedSourceIndex(
                            owner, semantic)]))
          <5>1. nextAttempt \in rrAttempts
            BY <2>2, <3>1, <4>2,
               ReplyReplacementRetainsOtherAttempt
          <5> QED BY <1>1, <2>2, <5>1,
               ReplyUnchangedRouteMapsPreserveAttemptMetadata
               DEF ReplyRouteInductiveInvariant,
                   ReplyRouteSafetyInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>8. ReplyRouteOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>3, <2>8
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM ReplyAttemptSubsetPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ rrAttempts' \subseteq rrAttempts
  /\ rrPayloads' = ReplyPayloadsForAttempts(rrAttempts')
  /\ UNCHANGED <<rrNextDeliveryOrdinal, rrConnectionTenure,
                 rrSourceActive, rrNextServiceIndex>>
  => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant,
                rrAttempts' \subseteq rrAttempts,
                rrPayloads' = ReplyPayloadsForAttempts(rrAttempts'),
                UNCHANGED <<rrNextDeliveryOrdinal, rrConnectionTenure,
                            rrSourceActive, rrNextServiceIndex>>
         PROVE ReplyRouteInductiveInvariant'
    <2>1. ReplyRouteConfiguration'
      BY <1>1 DEF ReplyRouteInductiveInvariant
    <2>2. ReplyRouteTypeInvariant'
      <3>1. rrAttempts' \subseteq ReplyAttemptSet
        BY <1>1
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>2. rrPayloads' \subseteq ReplySemantics
        BY <1>1, SMTT(30)
           DEF ReplyPayloadsForAttempts,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
               ReplyAttemptSet
      <3>3. /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
             /\ rrConnectionTenure' = rrConnectionTenure
             /\ rrSourceActive' = rrSourceActive
             /\ rrNextServiceIndex' = rrNextServiceIndex
        BY <1>1
      <3> QED BY <1>1, <3>1, <3>2, <3>3
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>3. \A owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
             /\ IsFiniteSet(
                  ReplyAttemptsForSource(owner, semantic, source))'
             /\ Cardinality(
                  ReplyAttemptsForSource(owner, semantic, source))' <= 1
      <3>1. ASSUME NEW owner \in ReplyOwners,
                    NEW semantic \in ReplySemantics,
                    NEW source \in ReplySources
             PROVE /\ IsFiniteSet(
                          ReplyAttemptsForSource(
                            owner, semantic, source))'
                   /\ Cardinality(
                          ReplyAttemptsForSource(
                            owner, semantic, source))' <= 1
        <4>1. ReplyAttemptsForSource(owner, semantic, source)'
                   \subseteq
                 ReplyAttemptsForSource(owner, semantic, source)
          BY <1>1, SMTT(20)
             DEF ReplyAttemptsForSource, ReplyAttemptsFor
        <4>2. /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source)) <= 1
          BY <1>1, <3>1
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteOwnershipInvariant
        <4>3. /\ IsFiniteSet(
                      ReplyAttemptsForSource(
                        owner, semantic, source))'
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source))'
                    <= Cardinality(
                         ReplyAttemptsForSource(
                           owner, semantic, source))
          BY <4>1, <4>2, FS_Subset
        <4>4. /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source)) \in Nat
               /\ Cardinality(
                      ReplyAttemptsForSource(
                        owner, semantic, source))' \in Nat
          BY <4>2, <4>3, FS_CardinalityType
        <4> QED BY <4>2, <4>3, <4>4, SMTT(5)
      <3> QED BY <3>1
    <2>4. \A owner \in ReplyOwners, semantic \in ReplySemantics:
             /\ Cardinality(ReplyAttemptSources(owner, semantic))'
                  <= ReplySourceCapacity
             /\ Cardinality(
                  ReplyRetiredDeliverySources(owner, semantic))'
                  <= ReplySourceCapacity
      BY <2>1, <2>2, ReplyNextTypeBoundsSourceGeometry
    <2>5. \A owner \in ReplyOwners, semantic \in ReplySemantics:
             ReplyAttemptsFor(owner, semantic)' # {} =>
               semantic \in rrPayloads'
      BY <1>1, SMTT(30)
         DEF ReplyPayloadsForAttempts, ReplyAttemptsFor
    <2>6. \A attempt \in rrAttempts':
             /\ attempt.deliveryOrdinal
                  < rrNextDeliveryOrdinal'[attempt.owner]
             /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
             /\ IF attempt.ticketTenure = NoReplyTicketTenure
                THEN ReplyAttemptHasNoTicket(attempt)
                ELSE ReplyTicketValidForAttempt(attempt)'
      <3>1. ASSUME NEW attempt \in rrAttempts'
             PROVE /\ attempt.deliveryOrdinal
                          < rrNextDeliveryOrdinal'[attempt.owner]
                   /\ ReplyAttemptRetiredDeliveryWellFormed(attempt)
                   /\ IF attempt.ticketTenure = NoReplyTicketTenure
                      THEN ReplyAttemptHasNoTicket(attempt)
                      ELSE ReplyTicketValidForAttempt(attempt)'
        <4>1. attempt \in rrAttempts
          BY <1>1, <3>1
        <4>2. /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
               /\ rrConnectionTenure' = rrConnectionTenure
               /\ rrSourceActive' = rrSourceActive
          BY <1>1
        <4> QED BY <1>1, <4>1, <4>2,
             ReplyUnchangedRouteMapsPreserveAttemptMetadata
             DEF ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant
      <3> QED BY <3>1
    <2>7. ReplyRouteOwnershipInvariant'
      BY <2>3, <2>4, <2>5, <2>6
         DEF ReplyRouteOwnershipInvariant
    <2> QED BY <2>1, <2>2, <2>7
         DEF ReplyRouteInductiveInvariant, ReplyRouteSafetyInvariant
  <1> QED BY <1>1

THEOREM CloseSemanticRequestPreservesInductiveInvariant ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteInductiveInvariant
    /\ CloseSemanticRequest(witness)
    => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteInductiveInvariant,
                CloseSemanticRequest(witness)
         PROVE ReplyRouteInductiveInvariant'
    <2>1. /\ rrAttempts' \subseteq rrAttempts
           /\ rrPayloads' = ReplyPayloadsForAttempts(rrAttempts')
           /\ UNCHANGED <<rrNextDeliveryOrdinal,
                          rrConnectionTenure, rrSourceActive,
                          rrNextServiceIndex>>
      BY <1>1, SMTT(30)
         DEF CloseSemanticRequest, ReplyAttemptsAfterClose
    <2> QED BY <1>1, <2>1,
         ReplyAttemptSubsetPreservesInductiveInvariant
  <1> QED BY <1>1

THEOREM PiggybackCloseSemanticRequestPreservesInductiveInvariant ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteInductiveInvariant
    /\ PiggybackCloseSemanticRequest(witness)
    => ReplyRouteInductiveInvariant'
BY CloseSemanticRequestPreservesInductiveInvariant
   DEF PiggybackCloseSemanticRequest

THEOREM RecoverReplyRouteStatePreservesInductiveInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RecoverReplyRouteState(owner, source)
    => ReplyRouteInductiveInvariant'
BY RetireReplySourcePreservesInductiveInvariant
   DEF RecoverReplyRouteState

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
  <1>8. CASE \E witness \in ReplyCloseWitnessSet:
               CloseSemanticRequest(witness)
    BY <1>8, CloseSemanticRequestPreservesInductiveInvariant
  <1>9. CASE \E witness \in ReplyCloseWitnessSet:
               PiggybackCloseSemanticRequest(witness)
    BY <1>9, PiggybackCloseSemanticRequestPreservesInductiveInvariant
  <1>10. CASE \E witness \in ReplyCloseWitnessSet:
                RetryCloseSemanticRequest(witness)
    BY <1>10, ReplyRouteStateIdentityPreservesInductiveInvariant
       DEF RetryCloseSemanticRequest, ReplyRouteVars
  <1>11. CASE \E acknowledgement \in ReplyCloseAcknowledgementSet:
                AcknowledgeCloseSemanticRequest(acknowledgement)
    BY <1>11, ReplyRouteStateIdentityPreservesInductiveInvariant
       DEF AcknowledgeCloseSemanticRequest
  <1>12. CASE \E owner \in ReplyOwners, source \in ReplySources:
                RecoverReplyRouteState(owner, source)
    BY <1>12, RecoverReplyRouteStatePreservesInductiveInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
       <1>8, <1>9, <1>10, <1>11, <1>12
       DEF ReplyRouteNext

THEOREM ReplyRouteStutterPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ UNCHANGED ReplyRouteVars
  => ReplyRouteInductiveInvariant'
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant,
                UNCHANGED ReplyRouteVars
         PROVE ReplyRouteInductiveInvariant'
    <2>1. /\ rrAttempts' = rrAttempts
           /\ rrPayloads' = rrPayloads
           /\ rrNextDeliveryOrdinal' = rrNextDeliveryOrdinal
           /\ rrConnectionTenure' = rrConnectionTenure
           /\ rrSourceActive' = rrSourceActive
           /\ rrNextServiceIndex' = rrNextServiceIndex
      BY <1>1 DEF ReplyRouteVars
    <2> QED BY <1>1, <2>1,
         ReplyRouteStateIdentityPreservesInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplyRouteBracketPreservesInductiveInvariant ==
  /\ ReplyRouteInductiveInvariant
  /\ [ReplyRouteNext]_ReplyRouteVars
  => ReplyRouteInductiveInvariant'
BY ReplyRouteNextPreservesInductiveInvariant,
   ReplyRouteStutterPreservesInductiveInvariant

THEOREM ReplyRouteSpecAlwaysInductiveInvariant ==
  ReplyRouteSpec => []ReplyRouteInductiveInvariant
PROOF
  <1>1. ReplyRouteInit => ReplyRouteInductiveInvariant
    BY ReplyRouteInitEstablishesInductiveInvariant
  <1>2. /\ ReplyRouteInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => ReplyRouteInductiveInvariant'
    BY ReplyRouteBracketPreservesInductiveInvariant
  <1>3. ReplyRouteSpec => []ReplyRouteInductiveInvariant
    BY <1>1, <1>2, PTL DEF ReplyRouteSpec
  <1> QED BY <1>3

THEOREM ReplyRouteSpecAlwaysSafetyInvariant ==
  ReplyRouteSpec => []ReplyRouteSafetyInvariant
BY ReplyRouteSpecAlwaysInductiveInvariant, PTL
   DEF ReplyRouteInductiveInvariant

(***************************************************************************
Durable semantic lifecycle and close-channel safety.  This invariant is kept
separate from the historical route-only invariant above so refinements which
consume only route typing retain their narrow boundary.
***************************************************************************)
ReplyRouteLifecycleInductiveInvariant ==
  /\ ReplyRouteConfiguration
  /\ ReplyRouteFullSafetyInvariant

THEOREM ReplyNextCloseRetryGenerationTyped ==
  \A generation \in 0..ReplyDeliveryOrdinalLimit:
    ReplyRouteConfiguration =>
      NextReplyCloseRetryGeneration(generation)
        \in 0..ReplyDeliveryOrdinalLimit
BY SMTT(10)
   DEF ReplyRouteConfiguration, NextReplyCloseRetryGeneration

THEOREM ReplyRouteInitEstablishesLifecycleInvariant ==
  ReplyRouteInit => ReplyRouteLifecycleInductiveInvariant
PROOF
  <1>1. ASSUME ReplyRouteInit
         PROVE ReplyRouteLifecycleInductiveInvariant
    <2>1. ReplyRouteInductiveInvariant
      BY <1>1, ReplyRouteInitEstablishesInductiveInvariant
    <2>2. ReplyLifecycleTypeInvariant
      BY <1>1, SMTT(30)
         DEF ReplyRouteInit, ReplyRouteConfiguration,
             ReplyLifecycleTypeInvariant
    <2>3. ReplyLifecycleOwnershipInvariant
      BY <1>1, FS_EmptySet, SMTT(45)
         DEF ReplyRouteInit, ReplyRouteConfiguration,
             ReplyLifecycleOwnershipInvariant,
             ReplySemanticActive, ReplySemanticBound,
             ReplyPayloadsForAttempts,
             ReplyCanonicalSemanticHash
    <2> QED BY <2>1, <2>2, <2>3
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyRouteInductiveInvariant
  <1> QED BY <1>1

THEOREM ReplySemanticCarrierPreservationPreservesLifecycle ==
  /\ ReplyRouteLifecycleInvariant
  /\ UNCHANGED <<rrSemanticSequence, rrSemanticHash,
                 rrRequesterNextSequence, rrRequesterClosedThrough,
                 rrClosePendingThrough, rrCloseSentThrough,
                 rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>
  /\ \A nextAttempt \in rrAttempts':
       \E oldAttempt \in rrAttempts:
         /\ nextAttempt.owner = oldAttempt.owner
         /\ nextAttempt.semantic = oldAttempt.semantic
  /\ ReplyPayloadsForAttempts(rrAttempts') =
       ReplyPayloadsForAttempts(rrAttempts)
  /\ rrPayloads' = rrPayloads
  => ReplyRouteLifecycleInvariant'
BY SMTT(90)
   DEF ReplyRouteLifecycleInvariant, ReplyLifecycleTypeInvariant,
       ReplyLifecycleOwnershipInvariant, ReplySemanticActive,
       ReplySemanticBound

THEOREM ReplyIdentityReplacementPreservesLifecycle ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteLifecycleInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, newAttempt)
    /\ rrPayloads' = rrPayloads
    /\ UNCHANGED <<rrSemanticSequence, rrSemanticHash,
                   rrRequesterNextSequence, rrRequesterClosedThrough,
                   rrClosePendingThrough, rrCloseSentThrough,
                   rrCloseAcknowledgedThrough, rrCloseRetryGeneration>>
    => ReplyRouteLifecycleInvariant'
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteLifecycleInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                rrAttempts' =
                  ReplaceReplyAttempt(oldAttempt, newAttempt),
                rrPayloads' = rrPayloads,
                UNCHANGED <<rrSemanticSequence, rrSemanticHash,
                            rrRequesterNextSequence,
                            rrRequesterClosedThrough,
                            rrClosePendingThrough, rrCloseSentThrough,
                            rrCloseAcknowledgedThrough,
                            rrCloseRetryGeneration>>
         PROVE ReplyRouteLifecycleInvariant'
    <2>1. \A nextAttempt \in rrAttempts':
             \E priorAttempt \in rrAttempts:
               /\ nextAttempt.owner = priorAttempt.owner
               /\ nextAttempt.semantic = priorAttempt.semantic
      BY <1>1, SMTT(20)
         DEF ReplaceReplyAttempt, SameReplyAttemptIdentity
    <2>2. ReplyPayloadsForAttempts(rrAttempts') =
             ReplyPayloadsForAttempts(rrAttempts)
      BY <1>1, SMTT(20)
         DEF ReplyPayloadsForAttempts, ReplaceReplyAttempt,
             SameReplyAttemptIdentity
    <2> QED BY <1>1, <2>1, <2>2,
         ReplySemanticCarrierPreservationPreservesLifecycle
  <1> QED BY <1>1

THEOREM ReplyIdentityImagePreservesLifecycle ==
  ASSUME NEW Transform(_),
         ReplyRouteLifecycleInvariant,
         \A attempt \in rrAttempts:
           SameReplyAttemptIdentity(attempt, Transform(attempt)),
         rrAttempts' = {Transform(attempt): attempt \in rrAttempts},
         rrPayloads' = rrPayloads,
         UNCHANGED <<rrSemanticSequence, rrSemanticHash,
                      rrRequesterNextSequence,
                      rrRequesterClosedThrough,
                      rrClosePendingThrough, rrCloseSentThrough,
                      rrCloseAcknowledgedThrough,
                      rrCloseRetryGeneration>>
  PROVE ReplyRouteLifecycleInvariant'
PROOF
  <1>1. \A nextAttempt \in rrAttempts':
           \E priorAttempt \in rrAttempts:
             /\ nextAttempt.owner = priorAttempt.owner
             /\ nextAttempt.semantic = priorAttempt.semantic
    BY SMTT(20) DEF SameReplyAttemptIdentity
  <1>2. ReplyPayloadsForAttempts(rrAttempts') =
           ReplyPayloadsForAttempts(rrAttempts)
    BY SMTT(20)
       DEF ReplyPayloadsForAttempts, SameReplyAttemptIdentity
  <1> QED BY <1>1, <1>2,
       ReplySemanticCarrierPreservationPreservesLifecycle

THEOREM ReplyClosedSemanticHasNoAttempts ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteLifecycleInvariant
    /\ ReplySemanticClosed(owner, semantic)
    => ReplyAttemptsFor(owner, semantic) = {}
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                ReplyRouteLifecycleInvariant,
                ReplySemanticClosed(owner, semantic)
         PROVE ReplyAttemptsFor(owner, semantic) = {}
    <2>1. \A attempt:
             attempt \notin ReplyAttemptsFor(owner, semantic)
      <3>1. ASSUME NEW attempt,
                    attempt \in ReplyAttemptsFor(owner, semantic)
             PROVE FALSE
        <4>1. /\ attempt \in rrAttempts
               /\ attempt.owner = owner
               /\ attempt.semantic = semantic
          BY <3>1 DEF ReplyAttemptsFor
        <4>2. ReplySemanticActive(
                 attempt.owner, attempt.semantic)
          BY <1>1, <4>1
             DEF ReplyRouteLifecycleInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4>3. /\ rrSemanticSequence[owner][semantic] \in Nat
               /\ rrRequesterClosedThrough[owner] \in Nat
          BY <1>1
             DEF ReplyRouteLifecycleInvariant,
                 ReplyLifecycleTypeInvariant
        <4>4. /\ rrSemanticSequence[owner][semantic]
                    > rrRequesterClosedThrough[owner]
               /\ rrSemanticSequence[owner][semantic]
                    <= rrRequesterClosedThrough[owner]
          BY <1>1, <4>1, <4>2
             DEF ReplySemanticActive, ReplySemanticClosed
        <4> QED BY <4>3, <4>4, SMT
      <3> QED BY <3>1
    <2> QED BY <2>1, SMT
  <1> QED BY <1>1

THEOREM ObserveNewReplySourcePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ ObserveNewReplySource(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteLifecycleInductiveInvariant,
                ObserveNewReplySource(owner, semantic, source)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ObserveNewReplySourcePreservesInductiveInvariant
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
    <2>2. CASE ReplySemanticBound(owner, semantic)
      <3>1. ReplyRouteLifecycleInvariant'
        BY <1>1, <2>2, SMTT(120)
           DEF ObserveNewReplySource, ReplyAttempt,
               ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant,
               ReplyLifecycleOwnershipInvariant,
               ReplySemanticActive, ReplySemanticBound,
               ReplyPayloadsForAttempts
      <3> QED BY <2>1, <3>1
           DEF ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteInductiveInvariant
    <2>3. CASE ~ReplySemanticBound(owner, semantic)
      <3>1. /\ ReplyLifecycleTypeInvariant
             /\ ReplyLifecycleOwnershipInvariant
             /\ rrRequesterNextSequence[owner]
                  \in ReplySemanticSequences
             /\ rrRequesterNextSequence[owner]
                  > rrRequesterClosedThrough[owner]
             /\ rrRequesterNextSequence[owner]
                  <= rrRequesterClosedThrough[owner]
                       + ReplyActiveWindowCapacity
             /\ rrSemanticSequence[owner][semantic] = 0
             /\ rrSemanticHash[owner][semantic] = {}
        BY <1>1, <2>3, SMTT(30)
           DEF ObserveNewReplySource,
               ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant,
               ReplyLifecycleOwnershipInvariant,
               ReplySemanticBound
      <3>15. /\ rrSemanticSequence' =
                    [rrSemanticSequence EXCEPT
                       ![owner][semantic] =
                         rrRequesterNextSequence[owner]]
                /\ rrSemanticHash' =
                    [rrSemanticHash EXCEPT
                       ![owner][semantic] =
                         ReplyCanonicalSemanticHash(semantic)]
                /\ rrRequesterNextSequence' =
                    [rrRequesterNextSequence EXCEPT
                       ![owner] =
                         rrRequesterNextSequence[owner] + 1]
                /\ rrRequesterClosedThrough' =
                     rrRequesterClosedThrough
        BY <1>1, <2>3
           DEF ObserveNewReplySource
      <3>2. rrSemanticSequence'
               \in [ReplyOwners ->
                     [ReplySemantics ->
                       0..ReplyDeliveryOrdinalLimit]]
        BY <3>1, <3>15,
           ReplyNestedFunctionalUpdatePreservesType,
           SMTT(5)
           DEF ReplyLifecycleTypeInvariant,
               ReplySemanticSequences
      <3>3. rrSemanticHash'
               \in [ReplyOwners ->
                     [ReplySemantics -> SUBSET ReplySemantics]]
        BY <1>1, <3>1, <3>15,
           ReplyNestedFunctionalUpdatePreservesType,
           SMTT(5)
           DEF ReplyLifecycleTypeInvariant,
               ReplyCanonicalSemanticHash
      <3>4. rrRequesterNextSequence'
               \in [ReplyOwners ->
                     1..(ReplyDeliveryOrdinalLimit + 1)]
        <4>1. rrRequesterNextSequence[owner] + 1
                 \in 1..(ReplyDeliveryOrdinalLimit + 1)
          BY <3>1, SMT
             DEF ReplySemanticSequences
        <4> QED BY <3>1, <3>15, <4>1,
             ReplyFunctionalUpdatePreservesType
             DEF ReplyLifecycleTypeInvariant
      <3>5. /\ rrRequesterClosedThrough'
                    \in [ReplyOwners ->
                          0..ReplyDeliveryOrdinalLimit]
               /\ rrClosePendingThrough'
                    \in [ReplyOwners ->
                          [ReplySources ->
                            0..ReplyDeliveryOrdinalLimit]]
               /\ rrCloseSentThrough'
                    \in [ReplyOwners ->
                          [ReplySources ->
                            0..ReplyDeliveryOrdinalLimit]]
               /\ rrCloseAcknowledgedThrough'
                    \in [ReplyOwners ->
                          [ReplySources ->
                            0..ReplyDeliveryOrdinalLimit]]
               /\ rrCloseRetryGeneration'
                    \in [ReplyOwners ->
                          [ReplySources ->
                            0..ReplyDeliveryOrdinalLimit]]
        BY <1>1, <3>1
           DEF ObserveNewReplySource,
               ReplyLifecycleTypeInvariant
      <3>6. ReplyLifecycleTypeInvariant'
        BY <3>2, <3>3, <3>4, <3>5
           DEF ReplyLifecycleTypeInvariant
      <3>7. \A nextOwner \in ReplyOwners:
               /\ rrRequesterClosedThrough'[nextOwner]
                    < rrRequesterNextSequence'[nextOwner]
               /\ \A nextSemantic \in ReplySemantics:
                    /\ (rrSemanticSequence'[
                          nextOwner][nextSemantic] = 0)
                         <=>
                         (rrSemanticHash'[
                            nextOwner][nextSemantic] = {})
                    /\ rrSemanticSequence'[
                         nextOwner][nextSemantic] # 0 =>
                         /\ rrSemanticHash'[
                              nextOwner][nextSemantic] =
                              ReplyCanonicalSemanticHash(
                                nextSemantic)
                         /\ rrSemanticSequence'[
                              nextOwner][nextSemantic] <
                              rrRequesterNextSequence'[nextOwner]
                    /\ ReplySemanticActive(
                         nextOwner, nextSemantic)' =>
                         rrSemanticSequence'[
                           nextOwner][nextSemantic] <=
                           rrRequesterClosedThrough'[nextOwner]
                             + ReplyActiveWindowCapacity
               /\ \A left, right \in ReplySemantics:
                    /\ rrSemanticSequence'[nextOwner][left] # 0
                    /\ rrSemanticSequence'[nextOwner][left] =
                         rrSemanticSequence'[nextOwner][right]
                    => left = right
        BY <1>1, <2>3, <3>1,
           ReplyNestedFunctionalUpdateAtKey,
           ReplyNestedFunctionalUpdateAwayFromKey,
           ReplyFunctionalUpdateAtKey,
           ReplyFunctionalUpdateAwayFromKey,
           SMTT(120)
           DEF ObserveNewReplySource,
               ReplyLifecycleTypeInvariant,
               ReplyLifecycleOwnershipInvariant,
               ReplySemanticActive, ReplySemanticBound,
               ReplyCanonicalSemanticHash
      <3>8. \A attempt \in rrAttempts:
               ReplySemanticActive(
                 attempt.owner, attempt.semantic)'
        <4>1. ASSUME NEW attempt \in rrAttempts
               PROVE ReplySemanticActive(
                       attempt.owner, attempt.semantic)'
          <5>1. ReplySemanticActive(
                   attempt.owner, attempt.semantic)
            BY <1>1, <4>1
               DEF ReplyRouteLifecycleInductiveInvariant,
                   ReplyRouteFullSafetyInvariant,
                   ReplyRouteLifecycleInvariant,
                   ReplyLifecycleOwnershipInvariant
          <5>2. ~(/\ attempt.owner = owner
                    /\ attempt.semantic = semantic)
            BY <2>3, <3>1, <5>1, SMT
               DEF ReplySemanticActive, ReplySemanticBound
          <5>3. /\ attempt.owner \in ReplyOwners
                 /\ attempt.semantic \in ReplySemantics
            BY <1>1, <4>1
               DEF ReplyRouteLifecycleInductiveInvariant,
                   ReplyRouteFullSafetyInvariant,
                   ReplyRouteSafetyInvariant,
                   ReplyRouteTypeInvariant, ReplyAttemptSet
          <5>4. /\ rrSemanticSequence'[
                        attempt.owner][attempt.semantic] =
                       rrSemanticSequence[
                         attempt.owner][attempt.semantic]
                 /\ rrSemanticHash'[
                        attempt.owner][attempt.semantic] =
                       rrSemanticHash[
                         attempt.owner][attempt.semantic]
                 /\ rrRequesterClosedThrough'[attempt.owner] =
                       rrRequesterClosedThrough[attempt.owner]
            BY <3>1, <3>15, <5>2, <5>3,
               ReplyNestedFunctionalUpdateAwayFromKey,
               ReplyFunctionalUpdateAwayFromKey,
               SMTT(5)
               DEF ReplyLifecycleTypeInvariant
          <5> QED BY <5>1, <5>4
               DEF ReplySemanticActive, ReplySemanticBound
        <4> QED BY <4>1
      <3>16. /\ rrSemanticSequence'[owner][semantic] =
                    rrRequesterNextSequence[owner]
                /\ rrSemanticHash'[owner][semantic] =
                    ReplyCanonicalSemanticHash(semantic)
                /\ rrRequesterClosedThrough'[owner] =
                    rrRequesterClosedThrough[owner]
        BY <1>1, <3>1, <3>15,
           ReplyNestedFunctionalUpdateAtKey,
           ReplyFunctionalUpdateAtKey,
           SMTT(5)
           DEF ReplyLifecycleTypeInvariant
      <3>9. ReplySemanticActive(owner, semantic)'
        BY <3>1, <3>16
           DEF ReplySemanticActive, ReplySemanticBound
      <3>10. \A attempt \in rrAttempts':
                ReplySemanticActive(
                  attempt.owner, attempt.semantic)'
        BY <1>1, <3>8, <3>9, SMTT(30)
           DEF ObserveNewReplySource, ReplyAttempt
      <3>11. rrPayloads' =
               ReplyPayloadsForAttempts(rrAttempts')
        BY <1>1, <2>3, <3>1, SMTT(30)
           DEF ObserveNewReplySource, ReplyAttempt,
               ReplyLifecycleOwnershipInvariant,
               ReplyPayloadsForAttempts
      <3>12. \A nextOwner \in ReplyOwners,
                 responder \in ReplySources:
               /\ rrCloseSentThrough'[
                    nextOwner][responder] =
                    rrClosePendingThrough'[
                      nextOwner][responder]
               /\ rrCloseAcknowledgedThrough'[
                    nextOwner][responder] <=
                    rrClosePendingThrough'[
                      nextOwner][responder]
               /\ rrClosePendingThrough'[
                    nextOwner][responder] <=
                    rrRequesterClosedThrough'[nextOwner]
        BY <1>1, <2>3, <3>1
           DEF ObserveNewReplySource,
               ReplyLifecycleOwnershipInvariant
      <3>13. ReplyLifecycleOwnershipInvariant'
        BY <3>7, <3>10, <3>11, <3>12
           DEF ReplyLifecycleOwnershipInvariant
      <3>14. ReplyRouteLifecycleInvariant'
        BY <3>6, <3>13 DEF ReplyRouteLifecycleInvariant
      <3> QED BY <2>1, <3>14
           DEF ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteInductiveInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ObserveLaterReplyDeliveryPreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ ObserveLaterReplyDelivery(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
BY ObserveLaterReplyDeliveryPreservesInductiveInvariant,
   ReplyOwnedAttemptIdentity,
   ReplyRouteRefreshPreservesIdentityAndCursor,
   ReplyIdentityReplacementPreservesLifecycle,
   SMTT(60)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, ObserveLaterReplyDelivery,
       ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant

THEOREM RetryExactReplySourcePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ RetryExactReplySource(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
BY RetryExactReplySourcePreservesInductiveInvariant,
   ReplySemanticCarrierPreservationPreservesLifecycle,
   SMTT(30)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, RetryExactReplySource,
       ReplyRouteVars

THEOREM RetireReplySourcePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ RetireReplySource(owner, source)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyRouteLifecycleInductiveInvariant,
                RetireReplySource(owner, source)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, RetireReplySourcePreservesInductiveInvariant
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
    <2>2. \A attempt \in rrAttempts:
             SameReplyAttemptIdentity(
               attempt,
               ReplyAttemptAfterRetire(owner, source, attempt))
      BY <1>1, ReplyRetireTransformTypedAndIdentity
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>3. /\ rrAttempts' =
                 {ReplyAttemptAfterRetire(owner, source, attempt):
                    attempt \in rrAttempts}
           /\ rrPayloads' = rrPayloads
           /\ UNCHANGED <<rrSemanticSequence, rrSemanticHash,
                          rrRequesterNextSequence,
                          rrRequesterClosedThrough,
                          rrClosePendingThrough, rrCloseSentThrough,
                          rrCloseAcknowledgedThrough,
                          rrCloseRetryGeneration>>
      BY <1>1 DEF RetireReplySource, ReplyAttemptAfterRetire
    <2>4. ReplyRouteLifecycleInvariant'
      BY <1>1, <2>2, <2>3,
         ReplyIdentityImagePreservesLifecycle
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant
    <2> QED BY <2>1, <2>4
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
  <1> QED BY <1>1

THEOREM ReconnectReplySourcePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ ReconnectReplySource(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteLifecycleInductiveInvariant,
                ReconnectReplySource(owner, semantic, source)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReconnectReplySourcePreservesInductiveInvariant
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
    <2>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt,
                     rrNextDeliveryOrdinal[owner],
                     rrConnectionTenure[owner][source] + 1)
             IN \A attempt \in rrAttempts:
                  SameReplyAttemptIdentity(
                    attempt,
                    ReplyAttemptAfterReconnectTransform(
                      oldAttempt, routed, attempt))
      <3>1. ASSUME NEW attempt \in rrAttempts
             PROVE LET oldAttempt ==
                         ReplyAttemptFor(owner, semantic, source)
                       routed ==
                         ReplyAttemptWithRoute(
                           oldAttempt,
                           rrNextDeliveryOrdinal[owner],
                           rrConnectionTenure[owner][source] + 1)
                   IN SameReplyAttemptIdentity(
                        attempt,
                        ReplyAttemptAfterReconnectTransform(
                          oldAttempt, routed, attempt))
        <4>1. attempt \in ReplyAttemptSet
          BY <1>1, <3>1
             DEF ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
        <4>2. LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   routed ==
                     ReplyAttemptWithRoute(
                       oldAttempt,
                       rrNextDeliveryOrdinal[owner],
                       rrConnectionTenure[owner][source] + 1)
               IN SameReplyAttemptIdentity(oldAttempt, routed)
          BY <1>1, ReplyOwnedAttemptIdentity,
             ReplyRouteRefreshPreservesIdentityAndCursor
             DEF ReconnectReplySource,
                 ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
        <4>3. SameReplyAttemptIdentity(
                 attempt, ReplyAttemptWithoutTicket(attempt))
          BY <4>1, ReplyTicketRemovalPreservesIdentityAndCursor
        <4>4. CASE attempt =
                    ReplyAttemptFor(owner, semantic, source)
          BY <4>2, <4>4
             DEF ReplyAttemptAfterReconnectTransform
        <4>5. CASE /\ attempt #
                         ReplyAttemptFor(owner, semantic, source)
                    /\ attempt.owner =
                         ReplyAttemptFor(
                           owner, semantic, source).owner
                    /\ attempt.source =
                         ReplyAttemptFor(
                           owner, semantic, source).source
          BY <4>3, <4>5
             DEF ReplyAttemptAfterReconnectTransform
        <4>6. CASE /\ attempt #
                         ReplyAttemptFor(owner, semantic, source)
                    /\ ~(attempt.owner =
                           ReplyAttemptFor(
                             owner, semantic, source).owner
                         /\ attempt.source =
                           ReplyAttemptFor(
                             owner, semantic, source).source)
          BY <4>6
             DEF ReplyAttemptAfterReconnectTransform,
                 SameReplyAttemptIdentity
        <4> QED BY <4>4, <4>5, <4>6
      <3> QED BY <3>1
    <2>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 routed ==
                   ReplyAttemptWithRoute(
                     oldAttempt,
                     rrNextDeliveryOrdinal[owner],
                     rrConnectionTenure[owner][source] + 1)
             IN /\ rrAttempts' =
                       {ReplyAttemptAfterReconnectTransform(
                          oldAttempt, routed, attempt):
                          attempt \in rrAttempts}
                /\ rrPayloads' = rrPayloads
                /\ UNCHANGED
                     <<rrSemanticSequence, rrSemanticHash,
                       rrRequesterNextSequence,
                       rrRequesterClosedThrough,
                       rrClosePendingThrough, rrCloseSentThrough,
                       rrCloseAcknowledgedThrough,
                       rrCloseRetryGeneration>>
      BY <1>1
         DEF ReconnectReplySource, ReplyAttemptsAfterReconnect,
             ReplyAttemptAfterReconnectTransform
    <2>4. ReplyRouteLifecycleInvariant'
      BY <1>1, <2>2, <2>3,
         ReplyIdentityImagePreservesLifecycle
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant
    <2> QED BY <2>1, <2>4
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
  <1> QED BY <1>1

THEOREM AcquireReplyTicketPreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ AcquireReplyTicket(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
BY AcquireReplyTicketPreservesInductiveInvariant,
   ReplyOwnedAttemptIdentity,
   ReplyTicketAcquisitionPreservesIdentityAndCursor,
   ReplyIdentityReplacementPreservesLifecycle,
   SMTT(45)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, AcquireReplyTicket,
       ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant

THEOREM AdvanceCurrentReplyAttemptPreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ AdvanceCurrentReplyAttempt(owner, semantic, source)
    => ReplyRouteLifecycleInductiveInvariant'
BY AdvanceCurrentReplyAttemptPreservesInductiveInvariant,
   ReplyOwnedAttemptIdentity, ReplyServicePreservesIdentity,
   ReplyIdentityReplacementPreservesLifecycle,
   SMTT(45)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, AdvanceCurrentReplyAttempt,
       ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant

THEOREM ServiceReplyRoutePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ ServiceReplyRoute(owner, semantic)
    => ReplyRouteLifecycleInductiveInvariant'
BY ServiceReplyRoutePreservesInductiveInvariant,
   ReplySelectedPendingAttemptFacts,
   ReplyServicePreservesIdentity,
   ReplyIdentityReplacementPreservesLifecycle,
   SMTT(60)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, ServiceReplyRoute,
       ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant

THEOREM CloseSemanticRequestPreservesLifecycleInvariant ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ CloseSemanticRequest(witness)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteLifecycleInductiveInvariant,
                CloseSemanticRequest(witness)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, CloseSemanticRequestPreservesInductiveInvariant
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
    <2>2. /\ witness.requester \in ReplyOwners
           /\ witness.responder \in ReplySources
           /\ witness.closedThrough
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyLifecycleTypeInvariant
           /\ ReplyLifecycleOwnershipInvariant
      BY <1>1
         DEF ReplyCloseWitnessSet,
             ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant
    <2>10. /\ rrAttempts' =
                   ReplyAttemptsAfterClose(
                     witness.requester, witness.closedThrough)
             /\ rrSemanticSequence' = rrSemanticSequence
             /\ rrSemanticHash' = rrSemanticHash
             /\ rrRequesterNextSequence' =
                  rrRequesterNextSequence
             /\ rrRequesterClosedThrough' =
                  [rrRequesterClosedThrough EXCEPT
                     ![witness.requester] =
                       witness.closedThrough]
             /\ rrClosePendingThrough' =
                  [rrClosePendingThrough EXCEPT
                     ![witness.requester][witness.responder] =
                       witness.closedThrough]
             /\ rrCloseSentThrough' =
                  [rrCloseSentThrough EXCEPT
                     ![witness.requester][witness.responder] =
                       witness.closedThrough]
             /\ rrCloseAcknowledgedThrough' =
                  rrCloseAcknowledgedThrough
      BY <1>1 DEF CloseSemanticRequest
    <2>3. ReplyLifecycleTypeInvariant'
      BY <1>1, <2>2,
         ReplyFunctionalUpdatePreservesType,
         ReplyNestedFunctionalUpdatePreservesType,
         SMTT(45)
         DEF CloseSemanticRequest,
             ReplyLifecycleTypeInvariant
    <2>4. \A owner \in ReplyOwners:
             /\ rrRequesterClosedThrough'[owner]
                  < rrRequesterNextSequence'[owner]
             /\ \A semantic \in ReplySemantics:
                  /\ (rrSemanticSequence'[owner][semantic] = 0)
                       <=>
                       (rrSemanticHash'[owner][semantic] = {})
                  /\ rrSemanticSequence'[owner][semantic] # 0 =>
                       /\ rrSemanticHash'[owner][semantic] =
                            ReplyCanonicalSemanticHash(semantic)
                       /\ rrSemanticSequence'[owner][semantic] <
                            rrRequesterNextSequence'[owner]
                  /\ ReplySemanticActive(owner, semantic)' =>
                       rrSemanticSequence'[owner][semantic] <=
                         rrRequesterClosedThrough'[owner]
                           + ReplyActiveWindowCapacity
             /\ \A left, right \in ReplySemantics:
                  /\ rrSemanticSequence'[owner][left] # 0
                  /\ rrSemanticSequence'[owner][left] =
                       rrSemanticSequence'[owner][right]
                  => left = right
      <3>1. ASSUME NEW owner \in ReplyOwners
             PROVE /\ rrRequesterClosedThrough'[owner]
                        < rrRequesterNextSequence'[owner]
                   /\ \A semantic \in ReplySemantics:
                        /\ (rrSemanticSequence'[
                              owner][semantic] = 0)
                             <=>
                             (rrSemanticHash'[
                                owner][semantic] = {})
                        /\ rrSemanticSequence'[
                             owner][semantic] # 0 =>
                             /\ rrSemanticHash'[
                                  owner][semantic] =
                                  ReplyCanonicalSemanticHash(
                                    semantic)
                             /\ rrSemanticSequence'[
                                  owner][semantic] <
                                  rrRequesterNextSequence'[owner]
                        /\ ReplySemanticActive(
                             owner, semantic)' =>
                             rrSemanticSequence'[
                               owner][semantic] <=
                               rrRequesterClosedThrough'[owner]
                                 + ReplyActiveWindowCapacity
                   /\ \A left, right \in ReplySemantics:
                        /\ rrSemanticSequence'[owner][left] # 0
                        /\ rrSemanticSequence'[owner][left] =
                             rrSemanticSequence'[owner][right]
                        => left = right
        <4>1. CASE owner = witness.requester
          <5>1. /\ rrRequesterClosedThrough'[owner] =
                       witness.closedThrough
                 /\ rrSemanticSequence' = rrSemanticSequence
                 /\ rrSemanticHash' = rrSemanticHash
                 /\ rrRequesterNextSequence' =
                      rrRequesterNextSequence
            BY <2>2, <2>10, <3>1, <4>1,
               ReplyFunctionalUpdateAtKey
               DEF ReplyLifecycleTypeInvariant
          <5>2. rrRequesterClosedThrough'[owner] <
                   rrRequesterNextSequence'[owner]
            BY <1>1, <4>1, <5>1
               DEF CloseSemanticRequest
          <5>3. \A semantic \in ReplySemantics:
                   /\ (rrSemanticSequence'[
                         owner][semantic] = 0)
                        <=>
                        (rrSemanticHash'[
                           owner][semantic] = {})
                   /\ rrSemanticSequence'[
                        owner][semantic] # 0 =>
                        /\ rrSemanticHash'[
                             owner][semantic] =
                             ReplyCanonicalSemanticHash(semantic)
                        /\ rrSemanticSequence'[
                             owner][semantic] <
                             rrRequesterNextSequence'[owner]
            BY <1>1, <4>1, <5>1
               DEF ReplyRouteLifecycleInductiveInvariant,
                   ReplyRouteFullSafetyInvariant,
                   ReplyRouteLifecycleInvariant,
                   ReplyLifecycleOwnershipInvariant
          <5>4. \A semantic \in ReplySemantics:
                   ReplySemanticActive(owner, semantic)' =>
                     rrSemanticSequence'[owner][semantic] <=
                       rrRequesterClosedThrough'[owner]
                         + ReplyActiveWindowCapacity
            <6>1. ASSUME NEW semantic \in ReplySemantics,
                          ReplySemanticActive(owner, semantic)'
                   PROVE rrSemanticSequence'[owner][semantic] <=
                           rrRequesterClosedThrough'[owner]
                             + ReplyActiveWindowCapacity
              <7>1. /\ ReplySemanticBound(owner, semantic)
                     /\ rrSemanticSequence[owner][semantic] >
                          witness.closedThrough
                BY <5>1, <6>1
                   DEF ReplySemanticActive, ReplySemanticBound
              <7>2. rrRequesterClosedThrough[owner] <
                       witness.closedThrough
                BY <1>1, <4>1 DEF CloseSemanticRequest
              <7>3. /\ rrSemanticSequence[owner][semantic] \in Nat
                     /\ rrRequesterClosedThrough[owner] \in Nat
                     /\ witness.closedThrough \in Nat
                BY <1>1, <2>2, <7>1
                   DEF ReplyRouteLifecycleInductiveInvariant,
                       ReplyRouteFullSafetyInvariant,
                       ReplyRouteLifecycleInvariant,
                       ReplyLifecycleTypeInvariant,
                       ReplySemanticBound, ReplySemanticSequences
              <7>4. rrSemanticSequence[owner][semantic] >
                       rrRequesterClosedThrough[owner]
                BY <7>1, <7>2, <7>3,
                   ReplyNaturalStrictTransitive
              <7>5. ReplySemanticActive(owner, semantic)
                BY <7>1, <7>4
                   DEF ReplySemanticActive, ReplySemanticBound
              <7>6. rrSemanticSequence[owner][semantic] <=
                       rrRequesterClosedThrough[owner] +
                         ReplyActiveWindowCapacity
                BY <1>1, <6>1, <7>5
                   DEF ReplyRouteLifecycleInductiveInvariant,
                       ReplyRouteFullSafetyInvariant,
                       ReplyRouteLifecycleInvariant,
                       ReplyLifecycleOwnershipInvariant
              <7>7. /\ rrRequesterClosedThrough[owner] \in Nat
                     /\ witness.closedThrough \in Nat
                     /\ ReplyActiveWindowCapacity \in Nat
                BY <1>1, <2>2
                   DEF ReplyRouteLifecycleInductiveInvariant,
                       ReplyRouteFullSafetyInvariant,
                       ReplyRouteLifecycleInvariant,
                       ReplyLifecycleTypeInvariant,
                       ReplyRouteConfiguration,
                       ReplyActiveWindowCapacity
              <7>8. rrRequesterClosedThrough[owner] +
                       ReplyActiveWindowCapacity <=
                     witness.closedThrough +
                       ReplyActiveWindowCapacity
                BY <7>2, <7>7, SMT
              <7>9. rrSemanticSequence[owner][semantic] <=
                     witness.closedThrough +
                       ReplyActiveWindowCapacity
                BY <7>3, <7>6, <7>7, <7>8, SMT
              <7> QED BY <5>1, <7>9
            <6> QED BY <6>1
          <5>5. \A left, right \in ReplySemantics:
                   /\ rrSemanticSequence'[owner][left] # 0
                   /\ rrSemanticSequence'[owner][left] =
                        rrSemanticSequence'[owner][right]
                   => left = right
            BY <1>1, <4>1, <5>1
               DEF ReplyRouteLifecycleInductiveInvariant,
                   ReplyRouteFullSafetyInvariant,
                   ReplyRouteLifecycleInvariant,
                   ReplyLifecycleOwnershipInvariant
          <5> QED BY <5>2, <5>3, <5>4, <5>5
        <4>2. CASE owner # witness.requester
          BY <1>1, <2>2, <2>10, <3>1, <4>2,
             ReplyFunctionalUpdateAwayFromKey,
             SMTT(20)
             DEF ReplyLifecycleTypeInvariant,
                 ReplyLifecycleOwnershipInvariant,
                 ReplySemanticActive, ReplySemanticBound
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>5. \A attempt \in rrAttempts':
             ReplySemanticActive(
               attempt.owner, attempt.semantic)'
      <3>1. ASSUME NEW attempt \in rrAttempts'
             PROVE ReplySemanticActive(
                     attempt.owner, attempt.semantic)'
        <4>1. /\ attempt \in rrAttempts
               /\ (\/ attempt.owner # witness.requester
                    \/ rrSemanticSequence[
                         witness.requester][attempt.semantic] >
                         witness.closedThrough)
          BY <2>10, <3>1 DEF ReplyAttemptsAfterClose
        <4>2. /\ attempt.owner \in ReplyOwners
               /\ attempt.semantic \in ReplySemantics
               /\ ReplySemanticActive(
                    attempt.owner, attempt.semantic)
          BY <1>1, <4>1
             DEF ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
                 ReplyRouteLifecycleInvariant,
                 ReplyLifecycleOwnershipInvariant,
                 ReplyAttemptSet
        <4>3. CASE attempt.owner = witness.requester
          <5>1. rrSemanticSequence' = rrSemanticSequence
            BY <2>10
          <5>2. rrSemanticHash' = rrSemanticHash
            BY <2>10
          <5>3. rrRequesterClosedThrough'[attempt.owner] =
                   witness.closedThrough
            BY <2>2, <2>10, <4>2, <4>3,
               ReplyFunctionalUpdateAtKey
               DEF ReplyLifecycleTypeInvariant
          <5>4. rrSemanticSequence[
                   attempt.owner][attempt.semantic] >
                   witness.closedThrough
            BY <4>1, <4>3
          <5> QED BY <4>2, <5>1, <5>2, <5>3, <5>4
               DEF ReplySemanticActive, ReplySemanticBound
        <4>4. CASE attempt.owner # witness.requester
          <5>1. rrSemanticSequence' = rrSemanticSequence
            BY <2>10
          <5>2. rrSemanticHash' = rrSemanticHash
            BY <2>10
          <5>3. rrRequesterClosedThrough'[attempt.owner] =
                   rrRequesterClosedThrough[attempt.owner]
            BY <2>2, <2>10, <4>2, <4>4,
               ReplyFunctionalUpdateAwayFromKey
               DEF ReplyLifecycleTypeInvariant
          <5> QED BY <4>2, <5>1, <5>2, <5>3
               DEF ReplySemanticActive, ReplySemanticBound
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>1
    <2>6. rrPayloads' =
             ReplyPayloadsForAttempts(rrAttempts')
      BY <1>1 DEF CloseSemanticRequest
    <2>7. \A owner \in ReplyOwners, responder \in ReplySources:
             /\ rrCloseSentThrough'[owner][responder] =
                  rrClosePendingThrough'[owner][responder]
             /\ rrCloseAcknowledgedThrough'[owner][responder]
                  <= rrClosePendingThrough'[owner][responder]
             /\ rrClosePendingThrough'[owner][responder]
                  <= rrRequesterClosedThrough'[owner]
      <3>1. ASSUME NEW owner \in ReplyOwners,
                    NEW responder \in ReplySources
             PROVE /\ rrCloseSentThrough'[
                          owner][responder] =
                          rrClosePendingThrough'[
                            owner][responder]
                   /\ rrCloseAcknowledgedThrough'[
                          owner][responder] <=
                          rrClosePendingThrough'[
                            owner][responder]
                   /\ rrClosePendingThrough'[
                          owner][responder] <=
                          rrRequesterClosedThrough'[owner]
        <4>1. CASE /\ owner = witness.requester
                    /\ responder = witness.responder
          BY <1>1, <2>2, <2>10, <3>1, <4>1,
             ReplyFunctionalUpdateAtKey,
             ReplyNestedFunctionalUpdateAtKey,
             SMTT(20)
             DEF CloseSemanticRequest, ReplyCloseWorkPending,
                 ReplyLifecycleTypeInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4>2. CASE /\ owner = witness.requester
                    /\ responder # witness.responder
          BY <1>1, <2>2, <2>10, <3>1, <4>2,
             ReplyFunctionalUpdateAtKey,
             ReplyNestedFunctionalUpdateAwayFromKey,
             SMTT(20)
             DEF CloseSemanticRequest,
                 ReplyLifecycleTypeInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4>3. CASE owner # witness.requester
          BY <1>1, <2>2, <2>10, <3>1, <4>3,
             ReplyFunctionalUpdateAwayFromKey,
             ReplyNestedFunctionalUpdateAwayFromKey,
             SMTT(20)
             DEF ReplyLifecycleTypeInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1
    <2>8. ReplyLifecycleOwnershipInvariant'
      BY <2>4, <2>5, <2>6, <2>7
         DEF ReplyLifecycleOwnershipInvariant
    <2>9. ReplyRouteLifecycleInvariant'
      BY <2>3, <2>8 DEF ReplyRouteLifecycleInvariant
    <2> QED BY <2>1, <2>9
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
  <1> QED BY <1>1

THEOREM PiggybackCloseSemanticRequestPreservesLifecycleInvariant ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ PiggybackCloseSemanticRequest(witness)
    => ReplyRouteLifecycleInductiveInvariant'
BY CloseSemanticRequestPreservesLifecycleInvariant
   DEF PiggybackCloseSemanticRequest

THEOREM RetryCloseSemanticRequestPreservesLifecycleInvariant ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ RetryCloseSemanticRequest(witness)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteLifecycleInductiveInvariant,
                RetryCloseSemanticRequest(witness)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStateIdentityPreservesInductiveInvariant,
         SMTT(20)
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant,
             RetryCloseSemanticRequest, ReplyRouteVars
    <2>2. CASE ReplyCloseWorkPending(
                 witness.requester, witness.responder)
      <3>1. /\ witness.requester \in ReplyOwners
             /\ witness.responder \in ReplySources
             /\ ReplyRouteConfiguration
             /\ ReplyRouteLifecycleInvariant
             /\ rrCloseRetryGeneration[
                  witness.requester][witness.responder]
                  \in 0..ReplyDeliveryOrdinalLimit
        BY <1>1
           DEF ReplyCloseWitnessSet,
               ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant
      <3>2. /\ rrCloseRetryGeneration' =
                  [rrCloseRetryGeneration EXCEPT
                     ![witness.requester][witness.responder] =
                       NextReplyCloseRetryGeneration(
                         rrCloseRetryGeneration[
                           witness.requester][witness.responder])]
             /\ UNCHANGED <<rrAttempts, rrPayloads,
                            rrSemanticSequence, rrSemanticHash,
                            rrRequesterNextSequence,
                            rrRequesterClosedThrough,
                            rrClosePendingThrough, rrCloseSentThrough,
                            rrCloseAcknowledgedThrough>>
        BY <1>1, <2>2
           DEF RetryCloseSemanticRequest
      <3>3. NextReplyCloseRetryGeneration(
               rrCloseRetryGeneration[
                 witness.requester][witness.responder])
               \in 0..ReplyDeliveryOrdinalLimit
        BY <3>1, ReplyNextCloseRetryGenerationTyped
      <3>4. rrCloseRetryGeneration'
               \in [ReplyOwners ->
                     [ReplySources ->
                       0..ReplyDeliveryOrdinalLimit]]
        BY <3>1, <3>2, <3>3,
           ReplyNestedFunctionalUpdatePreservesType
           DEF ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant
      <3>5. ReplyLifecycleTypeInvariant'
        BY <3>1, <3>2, <3>4
           DEF ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant
      <3>6. ReplyLifecycleOwnershipInvariant'
        BY <3>1, <3>2
           DEF ReplyRouteLifecycleInvariant,
               ReplyLifecycleOwnershipInvariant,
               ReplySemanticActive, ReplySemanticBound
      <3>7. ReplyRouteLifecycleInvariant'
        BY <3>5, <3>6 DEF ReplyRouteLifecycleInvariant
      <3> QED BY <2>1, <3>7
           DEF ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteInductiveInvariant
    <2>3. CASE ~ReplyCloseWorkPending(
                 witness.requester, witness.responder)
      <3>1. UNCHANGED ReplyRouteVars
        BY <1>1, <2>3
           DEF RetryCloseSemanticRequest
      <3>2. ReplyRouteLifecycleInvariant'
        <4>1. ReplyRouteLifecycleInvariant
          BY <1>1
             DEF ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant
        <4>2. /\ rrAttempts' = rrAttempts
               /\ rrPayloads' = rrPayloads
               /\ rrSemanticSequence' = rrSemanticSequence
               /\ rrSemanticHash' = rrSemanticHash
               /\ rrRequesterNextSequence' =
                    rrRequesterNextSequence
               /\ rrRequesterClosedThrough' =
                    rrRequesterClosedThrough
               /\ rrClosePendingThrough' =
                    rrClosePendingThrough
               /\ rrCloseSentThrough' = rrCloseSentThrough
               /\ rrCloseAcknowledgedThrough' =
                    rrCloseAcknowledgedThrough
               /\ rrCloseRetryGeneration' =
                    rrCloseRetryGeneration
          BY <3>1 DEF ReplyRouteVars
        <4>3. ReplyLifecycleTypeInvariant'
          BY <4>1, <4>2
             DEF ReplyRouteLifecycleInvariant,
                 ReplyLifecycleTypeInvariant
        <4>4. ReplyLifecycleOwnershipInvariant'
          BY <4>1, <4>2
             DEF ReplyRouteLifecycleInvariant,
                 ReplyLifecycleOwnershipInvariant,
                 ReplySemanticActive, ReplySemanticBound
        <4> QED BY <4>3, <4>4
             DEF ReplyRouteLifecycleInvariant
      <3> QED BY <2>1, <3>2
           DEF ReplyRouteLifecycleInductiveInvariant,
               ReplyRouteFullSafetyInvariant,
               ReplyRouteInductiveInvariant
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM AcknowledgeCloseSemanticRequestPreservesLifecycleInvariant ==
  \A acknowledgement \in ReplyCloseAcknowledgementSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ AcknowledgeCloseSemanticRequest(acknowledgement)
    => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. ASSUME NEW acknowledgement
                  \in ReplyCloseAcknowledgementSet,
                ReplyRouteLifecycleInductiveInvariant,
                AcknowledgeCloseSemanticRequest(acknowledgement)
         PROVE ReplyRouteLifecycleInductiveInvariant'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteStateIdentityPreservesInductiveInvariant,
         SMTT(20)
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant,
             AcknowledgeCloseSemanticRequest
    <2>2. /\ acknowledgement.requester \in ReplyOwners
           /\ acknowledgement.responder \in ReplySources
           /\ acknowledgement.closedThrough
                \in 0..ReplyDeliveryOrdinalLimit
           /\ ReplyRouteLifecycleInvariant
      BY <1>1
         DEF ReplyCloseAcknowledgementSet,
             ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant
    <2>3. /\ rrCloseAcknowledgedThrough' =
                [rrCloseAcknowledgedThrough EXCEPT
                   ![acknowledgement.requester][
                     acknowledgement.responder] =
                       acknowledgement.closedThrough]
           /\ UNCHANGED <<rrAttempts, rrPayloads,
                          rrSemanticSequence, rrSemanticHash,
                          rrRequesterNextSequence,
                          rrRequesterClosedThrough,
                          rrClosePendingThrough, rrCloseSentThrough,
                          rrCloseRetryGeneration>>
      BY <1>1 DEF AcknowledgeCloseSemanticRequest
    <2>4. rrCloseAcknowledgedThrough'
             \in [ReplyOwners ->
                   [ReplySources ->
                     0..ReplyDeliveryOrdinalLimit]]
      BY <2>2, <2>3,
         ReplyNestedFunctionalUpdatePreservesType
         DEF ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>5. ReplyLifecycleTypeInvariant'
      BY <2>2, <2>3, <2>4
         DEF ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>6. \A owner \in ReplyOwners,
               responder \in ReplySources:
             /\ rrCloseSentThrough'[owner][responder] =
                  rrClosePendingThrough'[owner][responder]
             /\ rrCloseAcknowledgedThrough'[owner][responder] <=
                  rrClosePendingThrough'[owner][responder]
             /\ rrClosePendingThrough'[owner][responder] <=
                  rrRequesterClosedThrough'[owner]
      <3>1. ASSUME NEW owner \in ReplyOwners,
                    NEW responder \in ReplySources
             PROVE /\ rrCloseSentThrough'[owner][responder] =
                          rrClosePendingThrough'[owner][responder]
                   /\ rrCloseAcknowledgedThrough'[owner][responder] <=
                          rrClosePendingThrough'[owner][responder]
                   /\ rrClosePendingThrough'[owner][responder] <=
                          rrRequesterClosedThrough'[owner]
        <4>1. CASE /\ owner = acknowledgement.requester
                    /\ responder = acknowledgement.responder
          <5>1. rrCloseSentThrough'[owner][responder] =
                   rrCloseSentThrough[owner][responder]
            BY <2>3
          <5>2. rrClosePendingThrough'[owner][responder] =
                   rrClosePendingThrough[owner][responder]
            BY <2>3
          <5>3. rrRequesterClosedThrough'[owner] =
                   rrRequesterClosedThrough[owner]
            BY <2>3
          <5>4. rrCloseAcknowledgedThrough'[
                   owner][responder] =
                   acknowledgement.closedThrough
            BY <2>2, <2>3, <3>1, <4>1,
               ReplyNestedFunctionalUpdateAtKey
               DEF ReplyRouteLifecycleInvariant,
                   ReplyLifecycleTypeInvariant
          <5>5. rrCloseSentThrough'[owner][responder] =
                   rrClosePendingThrough'[owner][responder]
            BY <1>1, <4>1, <5>1, <5>2
               DEF AcknowledgeCloseSemanticRequest
          <5>6. rrCloseAcknowledgedThrough'[
                   owner][responder] <=
                   rrClosePendingThrough'[owner][responder]
            <6>1. /\ acknowledgement.closedThrough =
                     rrClosePendingThrough[owner][responder]
                   /\ rrClosePendingThrough[owner][responder] \in Nat
              BY <1>1, <4>1
                 DEF AcknowledgeCloseSemanticRequest,
                     ReplyRouteLifecycleInductiveInvariant,
                     ReplyRouteFullSafetyInvariant,
                     ReplyRouteLifecycleInvariant,
                     ReplyLifecycleTypeInvariant
            <6> QED BY <5>2, <5>4, <6>1, SMT
          <5>7. rrClosePendingThrough'[owner][responder] <=
                   rrRequesterClosedThrough'[owner]
            BY <1>1, <4>1, <5>2, <5>3
               DEF AcknowledgeCloseSemanticRequest
          <5> QED BY <5>5, <5>6, <5>7
        <4>2. CASE \/ owner # acknowledgement.requester
                    \/ responder # acknowledgement.responder
          BY <1>1, <2>2, <2>3, <3>1, <4>2,
             ReplyNestedFunctionalUpdateAwayFromKey,
             SMTT(10)
             DEF ReplyRouteLifecycleInvariant,
                 ReplyLifecycleTypeInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>7. ReplyLifecycleOwnershipInvariant'
      BY <2>2, <2>3, <2>6
         DEF ReplyRouteLifecycleInvariant,
             ReplyLifecycleOwnershipInvariant,
             ReplySemanticActive, ReplySemanticBound
    <2>8. ReplyRouteLifecycleInvariant'
      BY <2>5, <2>7 DEF ReplyRouteLifecycleInvariant
    <2> QED BY <2>1, <2>8
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteInductiveInvariant
  <1> QED BY <1>1

THEOREM RecoverReplyRouteStatePreservesLifecycleInvariant ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ RecoverReplyRouteState(owner, source)
    => ReplyRouteLifecycleInductiveInvariant'
BY RetireReplySourcePreservesLifecycleInvariant
   DEF RecoverReplyRouteState

THEOREM ReplyRouteNextPreservesLifecycleInvariant ==
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ ReplyRouteNext
  => ReplyRouteLifecycleInductiveInvariant'
PROOF
  <1>1. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveNewReplySource(owner, semantic, source)
    BY <1>1, ObserveNewReplySourcePreservesLifecycleInvariant
  <1>2. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveLaterReplyDelivery(owner, semantic, source)
    BY <1>2, ObserveLaterReplyDeliveryPreservesLifecycleInvariant
  <1>3. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               RetryExactReplySource(owner, semantic, source)
    BY <1>3, RetryExactReplySourcePreservesLifecycleInvariant
  <1>4. CASE \E owner \in ReplyOwners, source \in ReplySources:
               RetireReplySource(owner, source)
    BY <1>4, RetireReplySourcePreservesLifecycleInvariant
  <1>5. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ReconnectReplySource(owner, semantic, source)
    BY <1>5, ReconnectReplySourcePreservesLifecycleInvariant
  <1>6. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               AcquireReplyTicket(owner, semantic, source)
    BY <1>6, AcquireReplyTicketPreservesLifecycleInvariant
  <1>7. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics:
               ServiceReplyRoute(owner, semantic)
    BY <1>7, ServiceReplyRoutePreservesLifecycleInvariant
  <1>8. CASE \E witness \in ReplyCloseWitnessSet:
               CloseSemanticRequest(witness)
    BY <1>8, CloseSemanticRequestPreservesLifecycleInvariant
  <1>9. CASE \E witness \in ReplyCloseWitnessSet:
               PiggybackCloseSemanticRequest(witness)
    BY <1>9, PiggybackCloseSemanticRequestPreservesLifecycleInvariant
  <1>10. CASE \E witness \in ReplyCloseWitnessSet:
                RetryCloseSemanticRequest(witness)
    BY <1>10, RetryCloseSemanticRequestPreservesLifecycleInvariant
  <1>11. CASE \E acknowledgement \in ReplyCloseAcknowledgementSet:
                AcknowledgeCloseSemanticRequest(acknowledgement)
    BY <1>11,
       AcknowledgeCloseSemanticRequestPreservesLifecycleInvariant
  <1>12. CASE \E owner \in ReplyOwners, source \in ReplySources:
                RecoverReplyRouteState(owner, source)
    BY <1>12, RecoverReplyRouteStatePreservesLifecycleInvariant
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
       <1>8, <1>9, <1>10, <1>11, <1>12
       DEF ReplyRouteNext

THEOREM ReplyRouteLifecycleStutterPreservesInvariant ==
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ UNCHANGED ReplyRouteVars
  => ReplyRouteLifecycleInductiveInvariant'
BY ReplyRouteStateIdentityPreservesInductiveInvariant,
   ReplySemanticCarrierPreservationPreservesLifecycle,
   SMTT(30)
   DEF ReplyRouteLifecycleInductiveInvariant,
       ReplyRouteFullSafetyInvariant, ReplyRouteLifecycleInvariant,
       ReplyRouteInductiveInvariant, ReplyRouteVars

THEOREM ReplyRouteLifecycleBracketPreservesInvariant ==
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ [ReplyRouteNext]_ReplyRouteVars
  => ReplyRouteLifecycleInductiveInvariant'
BY ReplyRouteNextPreservesLifecycleInvariant,
   ReplyRouteLifecycleStutterPreservesInvariant

THEOREM ReplyRouteSpecAlwaysFullSafetyInvariant ==
  ReplyRouteSpec => []ReplyRouteFullSafetyInvariant
PROOF
  <1>1. ReplyRouteInit => ReplyRouteLifecycleInductiveInvariant
    BY ReplyRouteInitEstablishesLifecycleInvariant
  <1>2. /\ ReplyRouteLifecycleInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => ReplyRouteLifecycleInductiveInvariant'
    BY ReplyRouteLifecycleBracketPreservesInvariant
  <1>3. ReplyRouteSpec => []ReplyRouteLifecycleInductiveInvariant
    BY <1>1, <1>2, PTL DEF ReplyRouteSpec
  <1> QED BY <1>3, PTL
       DEF ReplyRouteLifecycleInductiveInvariant

THEOREM ReplyRouteSpecAlwaysLifecycleInductiveInvariant ==
  ReplyRouteSpec => []ReplyRouteLifecycleInductiveInvariant
PROOF
  <1>1. ReplyRouteInit => ReplyRouteLifecycleInductiveInvariant
    BY ReplyRouteInitEstablishesLifecycleInvariant
  <1>2. /\ ReplyRouteLifecycleInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => ReplyRouteLifecycleInductiveInvariant'
    BY ReplyRouteLifecycleBracketPreservesInvariant
  <1> QED BY <1>1, <1>2, PTL DEF ReplyRouteSpec

(***************************************************************************
The durable sequence/hash journal is append-only, and the requester close
floor is monotone.  Only first observation may bind an empty slot; the close
transition never changes an existing binding.
***************************************************************************)
THEOREM ReplyLifecycleVarsStutterProvidesJournalStep ==
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ UNCHANGED ReplyLifecycleVars
  => ReplyLifecycleJournalStep
PROOF
  <1>1. ASSUME ReplyRouteLifecycleInductiveInvariant,
                UNCHANGED ReplyLifecycleVars
         PROVE ReplyLifecycleJournalStep
    <2>1. /\ rrSemanticSequence' = rrSemanticSequence
           /\ rrSemanticHash' = rrSemanticHash
           /\ rrRequesterClosedThrough' = rrRequesterClosedThrough
      BY <1>1 DEF ReplyLifecycleVars
    <2>2. \A owner \in ReplyOwners:
             rrRequesterClosedThrough'[owner] >=
               rrRequesterClosedThrough[owner]
      BY <1>1, <2>1, SMT
         DEF ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2> QED BY <2>1, <2>2 DEF ReplyLifecycleJournalStep
  <1> QED BY <1>1

THEOREM ObserveNewReplySourceProvidesLifecycleJournalStep ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ ObserveNewReplySource(owner, semantic, source)
    => ReplyLifecycleJournalStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyRouteLifecycleInductiveInvariant,
                ObserveNewReplySource(owner, semantic, source)
         PROVE ReplyLifecycleJournalStep
    <2>1. \A journalOwner \in ReplyOwners:
             rrRequesterClosedThrough'[journalOwner] >=
               rrRequesterClosedThrough[journalOwner]
      BY <1>1, SMT
         DEF ObserveNewReplySource,
             ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>2. \A journalOwner \in ReplyOwners,
               journalSemantic \in ReplySemantics:
             ReplySemanticBound(journalOwner, journalSemantic) =>
               /\ rrSemanticSequence'[
                    journalOwner][journalSemantic] =
                    rrSemanticSequence[
                      journalOwner][journalSemantic]
               /\ rrSemanticHash'[
                    journalOwner][journalSemantic] =
                    rrSemanticHash[
                      journalOwner][journalSemantic]
      <3>1. ASSUME NEW journalOwner \in ReplyOwners,
                    NEW journalSemantic \in ReplySemantics,
                    ReplySemanticBound(
                      journalOwner, journalSemantic)
             PROVE /\ rrSemanticSequence'[
                          journalOwner][journalSemantic] =
                          rrSemanticSequence[
                            journalOwner][journalSemantic]
                   /\ rrSemanticHash'[
                          journalOwner][journalSemantic] =
                          rrSemanticHash[
                            journalOwner][journalSemantic]
        <4>1. CASE /\ journalOwner = owner
                    /\ journalSemantic = semantic
          BY <1>1, <3>1, <4>1
             DEF ObserveNewReplySource
        <4>2. CASE \/ journalOwner # owner
                    \/ journalSemantic # semantic
          BY <1>1, <3>1, <4>2,
             ReplyNestedFunctionalUpdateAwayFromKey,
             SMTT(15)
             DEF ObserveNewReplySource,
                 ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteLifecycleInvariant,
                 ReplyLifecycleTypeInvariant
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2> QED BY <2>1, <2>2
         DEF ReplyLifecycleJournalStep
  <1> QED BY <1>1

THEOREM CloseSemanticRequestProvidesLifecycleJournalStep ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ CloseSemanticRequest(witness)
    => ReplyLifecycleJournalStep
PROOF
  <1>1. ASSUME NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteLifecycleInductiveInvariant,
                CloseSemanticRequest(witness)
         PROVE ReplyLifecycleJournalStep
    <2>1. /\ witness.requester \in ReplyOwners
           /\ rrRequesterClosedThrough
                \in [ReplyOwners ->
                      0..ReplyDeliveryOrdinalLimit]
      BY <1>1
         DEF ReplyCloseWitnessSet,
             ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>2. \A owner \in ReplyOwners:
             rrRequesterClosedThrough'[owner] >=
               rrRequesterClosedThrough[owner]
      <3>1. ASSUME NEW owner \in ReplyOwners
             PROVE rrRequesterClosedThrough'[owner] >=
                     rrRequesterClosedThrough[owner]
        <4>1. CASE owner = witness.requester
          BY <1>1, <2>1, <3>1, <4>1,
             ReplyFunctionalUpdateAtKey, SMT
             DEF CloseSemanticRequest
        <4>2. CASE owner # witness.requester
          BY <1>1, <2>1, <3>1, <4>2,
             ReplyFunctionalUpdateAwayFromKey
             DEF CloseSemanticRequest
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1
    <2>3. \A owner \in ReplyOwners,
               semantic \in ReplySemantics:
             ReplySemanticBound(owner, semantic) =>
               /\ rrSemanticSequence'[owner][semantic] =
                    rrSemanticSequence[owner][semantic]
               /\ rrSemanticHash'[owner][semantic] =
                    rrSemanticHash[owner][semantic]
      BY <1>1 DEF CloseSemanticRequest
    <2> QED BY <2>2, <2>3
         DEF ReplyLifecycleJournalStep
  <1> QED BY <1>1

THEOREM ReplyRouteNextProvidesLifecycleJournalStep ==
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ ReplyRouteNext
  => ReplyLifecycleJournalStep
PROOF
  <1>1. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveNewReplySource(owner, semantic, source)
    BY <1>1, ObserveNewReplySourceProvidesLifecycleJournalStep
  <1>2. CASE \E witness \in ReplyCloseWitnessSet:
               CloseSemanticRequest(witness)
    BY <1>2, CloseSemanticRequestProvidesLifecycleJournalStep
  <1>3. CASE \E witness \in ReplyCloseWitnessSet:
               PiggybackCloseSemanticRequest(witness)
    BY <1>3, CloseSemanticRequestProvidesLifecycleJournalStep
       DEF PiggybackCloseSemanticRequest
  <1>4. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ObserveLaterReplyDelivery(owner, semantic, source)
    BY <1>4, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF ObserveLaterReplyDelivery, ReplyLifecycleVars
  <1>5. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               RetryExactReplySource(owner, semantic, source)
    BY <1>5, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF RetryExactReplySource, ReplyRouteVars,
           ReplyLifecycleVars
  <1>6. CASE \E owner \in ReplyOwners, source \in ReplySources:
               RetireReplySource(owner, source)
    BY <1>6, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF RetireReplySource, ReplyLifecycleVars
  <1>7. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               ReconnectReplySource(owner, semantic, source)
    BY <1>7, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF ReconnectReplySource, ReplyLifecycleVars
  <1>8. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
               source \in ReplySources:
               AcquireReplyTicket(owner, semantic, source)
    BY <1>8, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF AcquireReplyTicket, ReplyLifecycleVars
  <1>9. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics:
               ServiceReplyRoute(owner, semantic)
    BY <1>9, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF ServiceReplyRoute, ReplyLifecycleVars
  <1>10. CASE \E witness \in ReplyCloseWitnessSet:
                RetryCloseSemanticRequest(witness)
    BY <1>10, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF RetryCloseSemanticRequest, ReplyRouteVars,
           ReplyLifecycleVars
  <1>11. CASE \E acknowledgement \in ReplyCloseAcknowledgementSet:
                AcknowledgeCloseSemanticRequest(acknowledgement)
    BY <1>11, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF AcknowledgeCloseSemanticRequest, ReplyLifecycleVars
  <1>12. CASE \E owner \in ReplyOwners, source \in ReplySources:
                RecoverReplyRouteState(owner, source)
    BY <1>12, ReplyLifecycleVarsStutterProvidesJournalStep
       DEF RecoverReplyRouteState, RetireReplySource,
           ReplyLifecycleVars
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6, <1>7,
       <1>8, <1>9, <1>10, <1>11, <1>12
       DEF ReplyRouteNext

THEOREM ReplyRouteSpecProvidesLifecycleJournal ==
  ReplyRouteSpec => ReplyLifecycleJournal
PROOF
  <1>1. ReplyRouteSpec => []ReplyRouteLifecycleInductiveInvariant
    BY ReplyRouteSpecAlwaysFullSafetyInvariant, PTL
       DEF ReplyRouteLifecycleInductiveInvariant
  <1>2. /\ ReplyRouteLifecycleInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => [ReplyLifecycleJournalStep]_ReplyRouteVars
    BY ReplyRouteNextProvidesLifecycleJournalStep
  <1> QED BY <1>1, <1>2, PTL
       DEF ReplyRouteSpec, ReplyLifecycleJournal

(***************************************************************************
Rehydration is source-scoped ticket invalidation.  Every retained attempt
keeps the same semantic/source identity and exact message/chunk cursor.
***************************************************************************)
THEOREM RecoverReplyRouteStatePreservesCursors ==
  \A owner \in ReplyOwners, source \in ReplySources:
    /\ ReplyRouteInductiveInvariant
    /\ RecoverReplyRouteState(owner, source)
    => ReplyRecoveryCursorPreservationStep
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW source \in ReplySources,
                ReplyRouteInductiveInvariant,
                RecoverReplyRouteState(owner, source)
         PROVE ReplyRecoveryCursorPreservationStep
    <2>1. /\ rrAttempts' =
                 {ReplyAttemptAfterRetire(owner, source, attempt):
                    attempt \in rrAttempts}
           /\ \A attempt \in rrAttempts:
                /\ attempt \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(
                     attempt,
                     ReplyAttemptAfterRetire(owner, source, attempt))
                /\ ReplyAttemptCursor(
                     ReplyAttemptAfterRetire(
                       owner, source, attempt)) =
                     ReplyAttemptCursor(attempt)
      BY <1>1, ReplyRetireTransformTypedAndIdentity
         DEF RecoverReplyRouteState, RetireReplySource,
             ReplyAttemptAfterRetire,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2> QED BY <2>1
         DEF ReplyRecoveryCursorPreservationStep
  <1> QED BY <1>1

THEOREM ReplyAttemptExtensionProvidesReplayAndIsolation ==
  \A newAttempt:
    /\ ReplyRouteTypeInvariant
    /\ rrAttempts' = rrAttempts \cup {newAttempt}
    /\ UNCHANGED rrConnectionTenure
    => /\ ReplyTenureAwareReplayStep
       /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW newAttempt,
                ReplyRouteTypeInvariant,
                rrAttempts' = rrAttempts \cup {newAttempt},
                UNCHANGED rrConnectionTenure
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. ReplyAttemptReplayStep
      <3>1. ASSUME NEW oldAttempt \in rrAttempts
             PROVE \E retained \in rrAttempts':
                     ReplyAttemptReplayValid(oldAttempt, retained)
        <4>1. oldAttempt \in rrAttempts'
          BY <1>1, <3>1
        <4>2. /\ oldAttempt.deliveryOrdinal \in Nat
               /\ oldAttempt.messageCursor \in Nat
               /\ oldAttempt.chunkCursor \in Nat
          BY <1>1, <3>1, SMTT(30)
             DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
                 ReplyDeliveryOrdinals
        <4>3. ReplyAttemptReplayValid(oldAttempt, oldAttempt)
          BY <4>2, SMTT(5)
             DEF ReplyAttemptReplayValid, ReplyAttemptCursor
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1 DEF ReplyAttemptReplayStep
    <2>2. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(30)
         DEF ReplySourceTenureInvalidationStep
    <2>3. ReplyAttemptSurvivalStep
      <3>1. ASSUME NEW retainedBefore \in rrAttempts
             PROVE \E retainedAfter \in rrAttempts':
                     SameReplyAttemptIdentity(
                       retainedBefore, retainedAfter)
        <4>1. retainedBefore \in rrAttempts'
          BY <1>1, <3>1
        <4>2. SameReplyAttemptIdentity(
                 retainedBefore, retainedBefore)
          BY DEF SameReplyAttemptIdentity
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF ReplyAttemptSurvivalStep
    <2>4. ReplyOtherCursorIsolationStep
      <3>1. ASSUME NEW changedBefore \in rrAttempts,
                    NEW changedAfter \in rrAttempts'
             PROVE LET sameAttempt ==
                         SameReplyAttemptIdentity(
                           changedBefore, changedAfter)
                       attemptChanged ==
                         ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
                   IN (sameAttempt /\ attemptChanged) =>
                        \A otherBefore \in rrAttempts:
                          (otherBefore.owner = changedBefore.owner
                            /\ ~SameReplyAttemptIdentity(
                                 otherBefore, changedBefore))
                          => \E otherAfter \in rrAttempts':
                               /\ SameReplyAttemptIdentity(
                                    otherBefore, otherAfter)
                               /\ ReplyAttemptCursor(otherAfter) =
                                    ReplyAttemptCursor(otherBefore)
        <4>1. ASSUME SameReplyAttemptIdentity(
                        changedBefore, changedAfter)
                      /\ ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
               PROVE \A otherBefore \in rrAttempts:
                       (otherBefore.owner = changedBefore.owner
                         /\ ~SameReplyAttemptIdentity(
                              otherBefore, changedBefore))
                       => \E otherAfter \in rrAttempts':
                            /\ SameReplyAttemptIdentity(
                                 otherBefore, otherAfter)
                            /\ ReplyAttemptCursor(otherAfter) =
                                 ReplyAttemptCursor(otherBefore)
          <5>1. ASSUME NEW otherBefore \in rrAttempts,
                        otherBefore.owner = changedBefore.owner
                          /\ ~SameReplyAttemptIdentity(
                               otherBefore, changedBefore)
                 PROVE \E otherAfter \in rrAttempts':
                         /\ SameReplyAttemptIdentity(
                              otherBefore, otherAfter)
                         /\ ReplyAttemptCursor(otherAfter) =
                              ReplyAttemptCursor(otherBefore)
            <6>1. otherBefore \in rrAttempts'
              BY <1>1, <5>1
            <6>2. /\ SameReplyAttemptIdentity(
                         otherBefore, otherBefore)
                    /\ ReplyAttemptCursor(otherBefore) =
                         ReplyAttemptCursor(otherBefore)
              BY DEF SameReplyAttemptIdentity
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplyOtherCursorIsolationStep
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF ReplyTenureAwareReplayStep, ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyAttemptStutterProvidesReplayAndIsolation ==
  /\ ReplyRouteTypeInvariant
  /\ UNCHANGED rrAttempts
  /\ UNCHANGED rrConnectionTenure
  => /\ ReplyTenureAwareReplayStep
     /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME ReplyRouteTypeInvariant,
                UNCHANGED rrAttempts,
                UNCHANGED rrConnectionTenure
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. rrAttempts' = rrAttempts
      BY <1>1
    <2>2. ReplyAttemptReplayStep
      <3>1. ASSUME NEW oldAttempt \in rrAttempts
             PROVE \E retained \in rrAttempts':
                     ReplyAttemptReplayValid(oldAttempt, retained)
        <4>1. oldAttempt \in rrAttempts'
          BY <2>1, <3>1
        <4>2. /\ oldAttempt.deliveryOrdinal \in Nat
               /\ oldAttempt.messageCursor \in Nat
               /\ oldAttempt.chunkCursor \in Nat
          BY <1>1, <3>1, SMTT(30)
             DEF ReplyRouteTypeInvariant, ReplyAttemptSet,
                 ReplyDeliveryOrdinals
        <4>3. ReplyAttemptReplayValid(oldAttempt, oldAttempt)
          BY <4>2, SMTT(5)
             DEF ReplyAttemptReplayValid, ReplyAttemptCursor
        <4> QED BY <4>1, <4>3
      <3> QED BY <3>1 DEF ReplyAttemptReplayStep
    <2>3. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(30)
         DEF ReplySourceTenureInvalidationStep
    <2>4. ReplyAttemptSurvivalStep
      <3>1. ASSUME NEW retainedBefore \in rrAttempts
             PROVE \E retainedAfter \in rrAttempts':
                     SameReplyAttemptIdentity(
                       retainedBefore, retainedAfter)
        <4>1. retainedBefore \in rrAttempts'
          BY <2>1, <3>1
        <4>2. SameReplyAttemptIdentity(
                 retainedBefore, retainedBefore)
          BY DEF SameReplyAttemptIdentity
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF ReplyAttemptSurvivalStep
    <2>5. ReplyOtherCursorIsolationStep
      <3>1. ASSUME NEW changedBefore \in rrAttempts,
                    NEW changedAfter \in rrAttempts'
             PROVE LET sameAttempt ==
                         SameReplyAttemptIdentity(
                           changedBefore, changedAfter)
                       attemptChanged ==
                         ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
                   IN (sameAttempt /\ attemptChanged) =>
                        \A otherBefore \in rrAttempts:
                          (otherBefore.owner = changedBefore.owner
                            /\ ~SameReplyAttemptIdentity(
                                 otherBefore, changedBefore))
                          => \E otherAfter \in rrAttempts':
                               /\ SameReplyAttemptIdentity(
                                    otherBefore, otherAfter)
                               /\ ReplyAttemptCursor(otherAfter) =
                                    ReplyAttemptCursor(otherBefore)
        <4>1. ASSUME SameReplyAttemptIdentity(
                        changedBefore, changedAfter)
                      /\ ReplyAttemptCursor(changedAfter) #
                           ReplyAttemptCursor(changedBefore)
               PROVE \A otherBefore \in rrAttempts:
                       (otherBefore.owner = changedBefore.owner
                         /\ ~SameReplyAttemptIdentity(
                              otherBefore, changedBefore))
                       => \E otherAfter \in rrAttempts':
                            /\ SameReplyAttemptIdentity(
                                 otherBefore, otherAfter)
                            /\ ReplyAttemptCursor(otherAfter) =
                                 ReplyAttemptCursor(otherBefore)
          <5>1. ASSUME NEW otherBefore \in rrAttempts,
                        otherBefore.owner = changedBefore.owner
                          /\ ~SameReplyAttemptIdentity(
                               otherBefore, changedBefore)
                 PROVE \E otherAfter \in rrAttempts':
                         /\ SameReplyAttemptIdentity(
                              otherBefore, otherAfter)
                         /\ ReplyAttemptCursor(otherAfter) =
                              ReplyAttemptCursor(otherBefore)
            <6>1. otherBefore \in rrAttempts'
              BY <2>1, <5>1
            <6>2. /\ SameReplyAttemptIdentity(
                         otherBefore, otherBefore)
                    /\ ReplyAttemptCursor(otherBefore) =
                         ReplyAttemptCursor(otherBefore)
              BY DEF SameReplyAttemptIdentity
            <6> QED BY <6>1, <6>2
          <5> QED BY <5>1
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplyOtherCursorIsolationStep
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF ReplyTenureAwareReplayStep, ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM CloseSemanticRequestProvidesReplayAndIsolation ==
  \A witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteLifecycleInductiveInvariant
    /\ CloseSemanticRequest(witness)
    => /\ ReplyTenureAwareReplayStep
       /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteLifecycleInductiveInvariant,
                CloseSemanticRequest(witness)
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. \A oldAttempt \in rrAttempts:
             \/ ReplyAttemptCoveredByCloseStep(oldAttempt)
             \/ oldAttempt \in rrAttempts'
      <3>1. ASSUME NEW oldAttempt \in rrAttempts
             PROVE \/ ReplyAttemptCoveredByCloseStep(oldAttempt)
                   \/ oldAttempt \in rrAttempts'
        <4>1. ReplySemanticActive(
                 oldAttempt.owner, oldAttempt.semantic)
          BY <1>1, <3>1
             DEF ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteLifecycleInvariant,
                 ReplyLifecycleOwnershipInvariant
        <4>2. CASE oldAttempt.owner # witness.requester
                     \/ rrSemanticSequence[
                          witness.requester][oldAttempt.semantic]
                          > witness.closedThrough
          <5> QED BY <1>1, <3>1, <4>2
               DEF CloseSemanticRequest,
                   ReplyAttemptsAfterClose
        <4>3. CASE ~(oldAttempt.owner # witness.requester
                     \/ rrSemanticSequence[
                          witness.requester][oldAttempt.semantic]
                          > witness.closedThrough)
          <5>1. /\ oldAttempt.owner = witness.requester
                 /\ rrSemanticSequence[
                      oldAttempt.owner][oldAttempt.semantic]
                      <= witness.closedThrough
            BY <4>3, SMTT(5)
          <5>2. ReplyAttemptCoveredByCloseStep(oldAttempt)
            BY <1>1, <3>1, <4>1, <5>1, SMTT(30)
               DEF CloseSemanticRequest,
                   ReplyAttemptCoveredByCloseStep,
                   ReplySemanticActive, ReplySemanticBound
          <5> QED BY <5>2
        <4> QED BY <4>2, <4>3
      <3> QED BY <3>1
    <2>2. ReplyAttemptReplayStep
      <3>1. ASSUME NEW oldAttempt \in rrAttempts
             PROVE \/ ReplyAttemptCoveredByCloseStep(oldAttempt)
                   \/ \E newAttempt \in rrAttempts':
                        ReplyAttemptReplayValid(
                          oldAttempt, newAttempt)
        <4>1. \/ ReplyAttemptCoveredByCloseStep(oldAttempt)
               \/ oldAttempt \in rrAttempts'
          BY <2>1, <3>1
        <4>2. ReplyAttemptReplayValid(oldAttempt, oldAttempt)
          BY <1>1, <3>1, SMTT(15)
             DEF ReplyRouteLifecycleInductiveInvariant,
                 ReplyRouteFullSafetyInvariant,
                 ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
                 ReplyAttemptSet, ReplyDeliveryOrdinals,
                 ReplyAttemptReplayValid, ReplyAttemptCursor
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1 DEF ReplyAttemptReplayStep
    <2>3. ReplySourceTenureInvalidationStep
      BY <1>1, SMTT(20)
         DEF CloseSemanticRequest,
             ReplySourceTenureInvalidationStep
    <2>4. ReplyAttemptSurvivalStep
      BY <2>1, SMTT(10)
         DEF ReplyAttemptSurvivalStep,
             SameReplyAttemptIdentity
    <2>5. ReplyOtherCursorIsolationStep
      BY <1>1, SMTT(90)
         DEF CloseSemanticRequest, ReplyAttemptsAfterClose,
             ReplyOtherCursorIsolationStep,
             ReplyAttemptCursor, SameReplyAttemptIdentity,
             ReplyRouteLifecycleInductiveInvariant,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant
    <2> QED BY <2>2, <2>3, <2>4, <2>5
         DEF ReplyTenureAwareReplayStep,
             ReplySourceIsolationStep
  <1> QED BY <1>1

THEOREM ReplyRouteNextProvidesReplayAndIsolation ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplyRouteLifecycleInductiveInvariant
  /\ ReplyRouteNext
  => /\ ReplyTenureAwareReplayStep
     /\ ReplySourceIsolationStep
PROOF
  <1>1. ASSUME ReplyRouteInductiveInvariant,
                ReplyRouteLifecycleInductiveInvariant,
                ReplyRouteNext
         PROVE /\ ReplyTenureAwareReplayStep
               /\ ReplySourceIsolationStep
    <2>1. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
                 source \in ReplySources:
                 ObserveNewReplySource(owner, semantic, source)
      BY <1>1, <2>1,
         ReplyAttemptExtensionProvidesReplayAndIsolation,
         SMTT(30)
         DEF ObserveNewReplySource,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
    <2>2. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
                 source \in ReplySources:
                 ObserveLaterReplyDelivery(owner, semantic, source)
      <3>1. ReplyAttemptReplayStep
        BY <1>1, <2>2, SMTT(90)
           DEF ObserveLaterReplyDelivery,
               ReplyAttemptReplayStep, ReplyAttemptReplayValid,
               ReplyAttemptCursor, ReplyAttemptWithRoute,
               ReplaceReplyAttempt,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyDeliveryOrdinals,
               ReplyAttemptHasNoTicket,
               NoReplyTicketTenure
      <3>2. ReplySourceTenureInvalidationStep
        BY <2>2, SMTT(30)
           DEF ObserveLaterReplyDelivery,
               ReplySourceTenureInvalidationStep
      <3>3. ReplySourceIsolationStep
        BY <1>1, <2>2,
           ReplyOwnedAttemptIdentity,
           ReplyRouteRefreshPreservesIdentityAndCursor,
           ReplyCursorPreservingIdentityReplacementProvidesSourceIsolation,
           SMTT(60)
           DEF ObserveLaterReplyDelivery,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyTenureAwareReplayStep,
               ReplySourceIsolationStep
    <2>3. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
                 source \in ReplySources:
                 RetryExactReplySource(owner, semantic, source)
      <3>1. /\ ReplyRouteTypeInvariant
             /\ UNCHANGED rrAttempts
             /\ UNCHANGED rrConnectionTenure
        BY <1>1, <2>3, SMTT(15)
           DEF RetryExactReplySource, ReplyRouteVars,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant
      <3> QED BY <3>1,
           ReplyAttemptStutterProvidesReplayAndIsolation
    <2>4. CASE \E owner \in ReplyOwners, source \in ReplySources:
                 RetireReplySource(owner, source)
      <3>1. ReplyAttemptReplayStep
        BY <1>1, <2>4, SMTT(60)
           DEF RetireReplySource,
               ReplyAttemptAfterRetire,
               ReplyAttemptReplayStep, ReplyAttemptReplayValid,
               ReplyAttemptCursor, ReplyAttemptWithoutTicket,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyDeliveryOrdinals,
               ReplyAttemptHasNoTicket,
               NoReplyTicketTenure
      <3>2. ReplySourceTenureInvalidationStep
        BY <2>4, SMTT(30)
           DEF RetireReplySource,
               ReplySourceTenureInvalidationStep
      <3>3. ReplySourceIsolationStep
        BY <1>1, <2>4,
           ReplyRetireTransformTypedAndIdentity,
           ReplySameOwnedAttemptIdentityUnique,
           SMTT(90)
           DEF RetireReplySource,
               ReplyAttemptAfterRetire,
               ReplySourceIsolationStep,
               ReplyAttemptSurvivalStep,
               ReplyOtherCursorIsolationStep,
               SameReplyAttemptIdentity, ReplyAttemptCursor,
               ReplyAttemptWithoutTicket,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyTenureAwareReplayStep
    <2>5. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
                 source \in ReplySources:
                 ReconnectReplySource(owner, semantic, source)
      <3>1. ReplyAttemptReplayStep
        BY <1>1, <2>5, SMTT(90)
           DEF ReconnectReplySource,
               ReplyAttemptAfterReconnectTransform,
               ReplyAttemptsAfterReconnect,
               ReplyAttemptReplayStep, ReplyAttemptReplayValid,
               ReplyAttemptCursor, ReplyAttemptWithRoute,
               ReplyAttemptWithoutTicket,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyDeliveryOrdinals,
               ReplyAttemptHasNoTicket,
               NoReplyTicketTenure
      <3>2. ReplySourceTenureInvalidationStep
        BY <1>1, <2>5,
           ReplyNestedFunctionalUpdateAwayFromKey,
           SMTT(60)
           DEF ReconnectReplySource,
               ReplySourceTenureInvalidationStep,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3>3. ReplySourceIsolationStep
        BY <1>1, <2>5,
           ReplyOwnedAttemptIdentity,
           ReplyRouteRefreshPreservesIdentityAndCursor,
           ReplyTicketRemovalPreservesIdentityAndCursor,
           ReplySameOwnedAttemptIdentityUnique,
           SMTT(90)
           DEF ReconnectReplySource,
               ReplyAttemptAfterReconnectTransform,
               ReplyAttemptsAfterReconnect,
               ReplySourceIsolationStep,
               ReplyAttemptSurvivalStep,
               ReplyOtherCursorIsolationStep,
               SameReplyAttemptIdentity, ReplyAttemptCursor,
               ReplyAttemptWithRoute,
               ReplyAttemptWithoutTicket,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyTenureAwareReplayStep
    <2>6. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics,
                 source \in ReplySources:
                 AcquireReplyTicket(owner, semantic, source)
      <3>1. ReplyAttemptReplayStep
        BY <1>1, <2>6, SMTT(60)
           DEF AcquireReplyTicket,
               ReplyAttemptReplayStep, ReplyAttemptReplayValid,
               ReplyAttemptCursor, ReplyAttemptWithTicket,
               ReplaceReplyAttempt,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant, ReplyAttemptSet,
               ReplyDeliveryOrdinals
      <3>2. ReplySourceTenureInvalidationStep
        BY <2>6, SMTT(30)
           DEF AcquireReplyTicket,
               ReplySourceTenureInvalidationStep
      <3>3. ReplySourceIsolationStep
        BY <1>1, <2>6,
           ReplyOwnedAttemptIdentity,
           ReplyTicketAcquisitionPreservesIdentityAndCursor,
           ReplyCursorPreservingIdentityReplacementProvidesSourceIsolation,
           SMTT(45)
           DEF AcquireReplyTicket,
               ReplyAttemptOwned, ReplyAttemptFor,
               ReplyAttemptsFor, ReplyAttemptsForSource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyTenureAwareReplayStep
    <2>7. CASE \E owner \in ReplyOwners, semantic \in ReplySemantics:
                 ServiceReplyRoute(owner, semantic)
      <3>1. ReplyAttemptReplayStep
        <4>1. PICK serviceOwner \in ReplyOwners,
                    serviceSemantic \in ReplySemantics:
                    ServiceReplyRoute(
                      serviceOwner, serviceSemantic)
          BY <2>7
        <4>2. LET selectedIndex ==
                       ReplySelectedSourceIndex(
                         serviceOwner, serviceSemantic)
                     source == ReplySourceOrder[selectedIndex]
                     oldAttempt ==
                       ReplyAttemptFor(
                         serviceOwner, serviceSemantic, source)
                     serviced ==
                       ReplyAttemptAfterService(oldAttempt)
                 IN /\ ReplyRouteTypeInvariant
                    /\ oldAttempt \in rrAttempts
                    /\ oldAttempt \in ReplyAttemptSet
                    /\ ~ReplyAttemptComplete(oldAttempt)
                    /\ SameReplyAttemptIdentity(
                         oldAttempt, serviced)
                    /\ rrAttempts' =
                         ReplaceReplyAttempt(oldAttempt, serviced)
          BY <1>1, <4>1,
             ReplySelectedPendingAttemptFacts,
             ReplyServicePreservesIdentity,
             SMTT(30)
             DEF ServiceReplyRoute,
                 ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4>3. LET selectedIndex ==
                       ReplySelectedSourceIndex(
                         serviceOwner, serviceSemantic)
                     source == ReplySourceOrder[selectedIndex]
                     oldAttempt ==
                       ReplyAttemptFor(
                         serviceOwner, serviceSemantic, source)
                     serviced ==
                       ReplyAttemptAfterService(oldAttempt)
                 IN ReplyAttemptReplayValid(
                      oldAttempt, serviced)
          BY <4>2, ReplyServiceProducesReplayValid
        <4>4. LET selectedIndex ==
                       ReplySelectedSourceIndex(
                         serviceOwner, serviceSemantic)
                     source == ReplySourceOrder[selectedIndex]
                     oldAttempt ==
                       ReplyAttemptFor(
                         serviceOwner, serviceSemantic, source)
                     serviced ==
                       ReplyAttemptAfterService(oldAttempt)
                 IN /\ ReplyRouteTypeInvariant
                    /\ oldAttempt \in rrAttempts
                    /\ ReplyAttemptReplayValid(
                         oldAttempt, serviced)
                    /\ rrAttempts' =
                         ReplaceReplyAttempt(oldAttempt, serviced)
          BY <4>2, <4>3
        <4> QED BY <4>4,
             ReplyReplayValidIdentityReplacementProvidesReplayStep
      <3>2. ReplySourceTenureInvalidationStep
        BY <2>7, SMTT(30)
           DEF ServiceReplyRoute,
               ReplySourceTenureInvalidationStep
      <3>3. ReplySourceIsolationStep
        <4>1. PICK serviceOwner \in ReplyOwners,
                    serviceSemantic \in ReplySemantics:
                    ServiceReplyRoute(
                      serviceOwner, serviceSemantic)
          BY <2>7
        <4>2. LET selectedIndex ==
                       ReplySelectedSourceIndex(
                         serviceOwner, serviceSemantic)
                     source == ReplySourceOrder[selectedIndex]
                     oldAttempt ==
                       ReplyAttemptFor(
                         serviceOwner, serviceSemantic, source)
                     serviced ==
                       ReplyAttemptAfterService(oldAttempt)
                 IN /\ ReplyRouteSafetyInvariant
                    /\ oldAttempt \in rrAttempts
                    /\ SameReplyAttemptIdentity(
                         oldAttempt, serviced)
                    /\ rrAttempts' =
                         ReplaceReplyAttempt(oldAttempt, serviced)
          BY <1>1, <4>1,
             ReplySelectedPendingAttemptFacts,
             ReplyServicePreservesIdentity,
             SMTT(30)
             DEF ServiceReplyRoute,
                 ReplyRouteInductiveInvariant,
                 ReplyRouteSafetyInvariant,
                 ReplyRouteTypeInvariant
        <4> QED BY <4>2,
             ReplyIdentityReplacementProvidesSourceIsolation
      <3> QED BY <3>1, <3>2, <3>3
           DEF ReplyTenureAwareReplayStep
    <2>8. CASE \E witness \in ReplyCloseWitnessSet:
                 CloseSemanticRequest(witness)
      BY <1>1, <2>8,
         CloseSemanticRequestProvidesReplayAndIsolation
    <2>9. CASE \E witness \in ReplyCloseWitnessSet:
                 PiggybackCloseSemanticRequest(witness)
      BY <1>1, <2>9,
         CloseSemanticRequestProvidesReplayAndIsolation
         DEF PiggybackCloseSemanticRequest
    <2>10. CASE \E witness \in ReplyCloseWitnessSet:
                  RetryCloseSemanticRequest(witness)
      BY <1>1, <2>10,
         ReplyAttemptStutterProvidesReplayAndIsolation,
         SMTT(30)
         DEF RetryCloseSemanticRequest,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteVars
    <2>11. CASE \E acknowledgement \in
                    ReplyCloseAcknowledgementSet:
                  AcknowledgeCloseSemanticRequest(acknowledgement)
      BY <1>1, <2>11,
         ReplyAttemptStutterProvidesReplayAndIsolation,
         SMTT(30)
         DEF AcknowledgeCloseSemanticRequest,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant
    <2>12. CASE \E owner \in ReplyOwners, source \in ReplySources:
                  RecoverReplyRouteState(owner, source)
      BY <1>1, <2>12, SMTT(30)
         DEF RecoverReplyRouteState, ReplyRouteNext
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6,
         <2>7, <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ReplyRouteNext
  <1> QED BY <1>1

THEOREM ReplyRouteBracketProvidesReplayAndIsolation ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplyRouteLifecycleInductiveInvariant
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
    BY ReplyRouteSpecAlwaysInductiveInvariant
  <1>2. ReplyRouteSpec => []ReplyRouteLifecycleInductiveInvariant
    BY ReplyRouteSpecAlwaysFullSafetyInvariant, PTL
       DEF ReplyRouteLifecycleInductiveInvariant
  <1>3. /\ ReplyRouteInductiveInvariant
           /\ ReplyRouteLifecycleInductiveInvariant
           /\ [ReplyRouteNext]_ReplyRouteVars
          => /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
             /\ [ReplySourceIsolationStep]_ReplyRouteVars
    BY ReplyRouteBracketProvidesReplayAndIsolation
  <1> QED BY <1>1, <1>2, <1>3, PTL
       DEF ReplyRouteSpec, ReplyTenureAwareReplay,
           ReplySourceIsolation

(***************************************************************************
Cursor persistence before the stable suffix.
***************************************************************************)

THEOREM ReplyAttemptRankTyped ==
  \A attempt \in ReplyAttemptSet:
    ReplyRouteConfiguration =>
      ReplyAttemptRank(attempt) \in Nat
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                ReplyRouteConfiguration
         PROVE ReplyAttemptRank(attempt) \in Nat
    <2>1. /\ attempt.messageCursor \in 0..ReplyMessageCount
           /\ attempt.chunkCursor \in 0..ReplyChunkCount
      BY <1>1 DEF ReplyAttemptSet
    <2>2. /\ attempt.messageCursor \in Nat
           /\ attempt.chunkCursor \in Nat
           /\ ReplyChunkCount \in Nat
      BY <1>1, <2>1, ReplyBoundedNaturalFacts
         DEF ReplyRouteConfiguration
    <2> QED BY <2>2, ReplyNaturalRankTermTyped
         DEF ReplyAttemptRank
  <1> QED BY <1>1

THEOREM ReplyCursorRankTyped ==
  \A messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyRouteConfiguration =>
      messageCursor * (ReplyChunkCount + 1)
        + chunkCursor \in Nat
PROOF
  <1>1. ASSUME NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration
         PROVE messageCursor * (ReplyChunkCount + 1)
                 + chunkCursor \in Nat
    <2>1. /\ messageCursor \in Nat
           /\ chunkCursor \in Nat
           /\ ReplyChunkCount \in Nat
      BY <1>1, ReplyBoundedNaturalFacts
         DEF ReplyRouteConfiguration
    <2> QED BY <2>1, ReplyNaturalRankTermTyped
  <1> QED BY <1>1

THEOREM ReplyReplayValidCursorUnchangedOrRankAdvances ==
  \A oldAttempt, newAttempt \in ReplyAttemptSet:
    /\ ReplyRouteConfiguration
    /\ ReplyAttemptReplayValid(oldAttempt, newAttempt)
    => \/ ReplyAttemptCursor(newAttempt) =
            ReplyAttemptCursor(oldAttempt)
       \/ ReplyAttemptRank(newAttempt) >
            ReplyAttemptRank(oldAttempt)
PROOF
  <1>1. ASSUME NEW oldAttempt \in ReplyAttemptSet,
                NEW newAttempt \in ReplyAttemptSet,
                ReplyRouteConfiguration,
                ReplyAttemptReplayValid(oldAttempt, newAttempt)
         PROVE \/ ReplyAttemptCursor(newAttempt) =
                     ReplyAttemptCursor(oldAttempt)
               \/ ReplyAttemptRank(newAttempt) >
                     ReplyAttemptRank(oldAttempt)
    <2>1. CASE newAttempt.connectionTenure =
                  oldAttempt.connectionTenure
      <3>1. /\ newAttempt.messageCursor >=
                    oldAttempt.messageCursor
             /\ newAttempt.chunkCursor >=
                    oldAttempt.chunkCursor
        BY <1>1, <2>1
           DEF ReplyAttemptReplayValid
      <3>2. CASE /\ newAttempt.messageCursor =
                       oldAttempt.messageCursor
                  /\ newAttempt.chunkCursor =
                       oldAttempt.chunkCursor
        <4> QED BY <3>2 DEF ReplyAttemptCursor
      <3>3. CASE ~(/\ newAttempt.messageCursor =
                         oldAttempt.messageCursor
                    /\ newAttempt.chunkCursor =
                         oldAttempt.chunkCursor)
        <4>1. /\ oldAttempt.messageCursor \in
                       0..ReplyMessageCount
               /\ newAttempt.messageCursor \in
                       0..ReplyMessageCount
               /\ oldAttempt.chunkCursor \in
                       0..ReplyChunkCount
               /\ newAttempt.chunkCursor \in
                       0..ReplyChunkCount
          BY <1>1 DEF ReplyAttemptSet
        <4>2. /\ ReplyChunkCount \in Nat
               /\ oldAttempt.messageCursor \in Nat
               /\ newAttempt.messageCursor \in Nat
               /\ oldAttempt.chunkCursor \in Nat
               /\ newAttempt.chunkCursor \in Nat
               /\ oldAttempt.chunkCursor <= ReplyChunkCount
          BY <1>1, <4>1, ReplyBoundedNaturalFacts
             DEF ReplyRouteConfiguration
        <4>3. CASE newAttempt.messageCursor =
                    oldAttempt.messageCursor
          <5>1. newAttempt.chunkCursor >
                   oldAttempt.chunkCursor
            BY <3>1, <3>3, <4>3, <4>2, SMTT(10)
          <5>2. newAttempt.messageCursor *
                    (ReplyChunkCount + 1) =
                  oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
            BY <4>3
          <5>3. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1) \in Nat
            BY <4>2, ReplyNaturalProductSuccessorTyped
          <5>4. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                    + oldAttempt.chunkCursor
                  < oldAttempt.messageCursor *
                      (ReplyChunkCount + 1)
                      + newAttempt.chunkCursor
            BY <4>2, <5>1, <5>3,
               ReplyNaturalStrictAdditiveMonotone
          <5> QED BY <5>2, <5>4
               DEF ReplyAttemptRank
        <4>4. CASE newAttempt.messageCursor #
                    oldAttempt.messageCursor
          <5>1. oldAttempt.messageCursor <
                   newAttempt.messageCursor
            BY <3>1, <4>2, <4>4, SMTT(10)
          <5>2. (ReplyChunkCount + 1) *
                    oldAttempt.messageCursor
                    + (ReplyChunkCount + 1)
                  <= (ReplyChunkCount + 1) *
                       newAttempt.messageCursor
            BY <4>2, <5>1, ReplyNaturalStrictMultiplierGap
          <5>3. /\ (ReplyChunkCount + 1) *
                         oldAttempt.messageCursor =
                       oldAttempt.messageCursor *
                         (ReplyChunkCount + 1)
                 /\ (ReplyChunkCount + 1) *
                         newAttempt.messageCursor =
                       newAttempt.messageCursor *
                         (ReplyChunkCount + 1)
            BY <4>2, ReplyNaturalMultiplicationCommutes
          <5>4. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                    + oldAttempt.chunkCursor
                  < oldAttempt.messageCursor *
                      (ReplyChunkCount + 1)
                      + (ReplyChunkCount + 1)
            BY <4>2, SMTT(10)
          <5>5. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                    + (ReplyChunkCount + 1)
                  <= newAttempt.messageCursor *
                       (ReplyChunkCount + 1)
            BY <5>2, <5>3, SMTT(10)
          <5>6. newAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                  <= newAttempt.messageCursor *
                       (ReplyChunkCount + 1)
                       + newAttempt.chunkCursor
            BY <4>2, SMTT(10)
          <5>7. /\ oldAttempt.messageCursor *
                         (ReplyChunkCount + 1)
                         + oldAttempt.chunkCursor \in Nat
                 /\ oldAttempt.messageCursor *
                         (ReplyChunkCount + 1)
                         + (ReplyChunkCount + 1) \in Nat
                 /\ newAttempt.messageCursor *
                         (ReplyChunkCount + 1) \in Nat
                 /\ newAttempt.messageCursor *
                         (ReplyChunkCount + 1)
                         + newAttempt.chunkCursor \in Nat
            BY <4>2, SMTT(10)
          <5>8. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                    + oldAttempt.chunkCursor
                  < newAttempt.messageCursor *
                      (ReplyChunkCount + 1)
            BY <5>4, <5>5, <5>7,
               ReplyNaturalStrictThenWeakTransitive
          <5>9. oldAttempt.messageCursor *
                    (ReplyChunkCount + 1)
                    + oldAttempt.chunkCursor
                  < newAttempt.messageCursor *
                      (ReplyChunkCount + 1)
                      + newAttempt.chunkCursor
            BY <5>6, <5>7, <5>8,
               ReplyNaturalStrictThenWeakTransitive
          <5> QED BY <5>9
               DEF ReplyAttemptRank
        <4> QED BY <4>3, <4>4
      <3> QED BY <3>2, <3>3
    <2>2. CASE newAttempt.connectionTenure #
                  oldAttempt.connectionTenure
      <3> QED BY <1>1, <2>2
           DEF ReplyAttemptReplayValid
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyEqualCursorPreservesRankAndCompletion ==
  \A oldAttempt, newAttempt:
    ReplyAttemptCursor(newAttempt) =
      ReplyAttemptCursor(oldAttempt)
    => /\ ReplyAttemptRank(newAttempt) =
            ReplyAttemptRank(oldAttempt)
       /\ (ReplyAttemptComplete(newAttempt)
             <=> ReplyAttemptComplete(oldAttempt))
BY SMTT(10)
   DEF ReplyAttemptCursor, ReplyAttemptRank,
       ReplyAttemptComplete

THEOREM ReplyReplayValidPreservesCompletion ==
  \A oldAttempt, newAttempt \in ReplyAttemptSet:
    /\ ReplyAttemptComplete(oldAttempt)
    /\ ReplyAttemptReplayValid(oldAttempt, newAttempt)
    => ReplyAttemptComplete(newAttempt)
PROOF
  <1>1. ASSUME NEW oldAttempt \in ReplyAttemptSet,
                NEW newAttempt \in ReplyAttemptSet,
                ReplyAttemptComplete(oldAttempt),
                ReplyAttemptReplayValid(oldAttempt, newAttempt)
         PROVE ReplyAttemptComplete(newAttempt)
    <2>1. /\ oldAttempt.messageCursor \in
                    0..ReplyMessageCount
           /\ newAttempt.messageCursor \in
                    0..ReplyMessageCount
           /\ oldAttempt.chunkCursor \in
                    0..ReplyChunkCount
           /\ newAttempt.chunkCursor \in
                    0..ReplyChunkCount
      BY <1>1 DEF ReplyAttemptSet
    <2>2. CASE newAttempt.connectionTenure =
                  oldAttempt.connectionTenure
      <3> QED BY <1>1, <2>1, <2>2, SMTT(20)
           DEF ReplyAttemptComplete,
               ReplyAttemptReplayValid
    <2>3. CASE newAttempt.connectionTenure #
                  oldAttempt.connectionTenure
      <3> QED BY <1>1, <2>3, SMTT(10)
           DEF ReplyAttemptComplete,
               ReplyAttemptReplayValid,
               ReplyAttemptCursor
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyCursorStepPersistsOrAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
    chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
    => \/ ReplySourceAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplyTenureAwareReplayStep,
                ReplySourceIsolationStep
         PROVE \/ ReplySourceAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
               \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
    <2>1. \E oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ~ReplyAttemptComplete(oldAttempt)
             /\ oldAttempt.messageCursor = messageCursor
             /\ oldAttempt.chunkCursor = chunkCursor
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(30)
         DEF ReplySourceAtCursor,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>2. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ~ReplyAttemptComplete(oldAttempt)
             /\ oldAttempt.messageCursor = messageCursor
             /\ oldAttempt.chunkCursor = chunkCursor
      BY <2>1
    <2>3. \E newAttempt \in rrAttempts':
             ReplyAttemptReplayValid(oldAttempt, newAttempt)
      BY <1>1, <2>2
         DEF ReplyTenureAwareReplayStep,
             ReplyAttemptReplayStep
    <2>4. PICK newAttempt \in rrAttempts':
             ReplyAttemptReplayValid(oldAttempt, newAttempt)
      BY <2>3
    <2>5. /\ newAttempt \in ReplyAttemptSet
           /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
      BY <1>1, <2>2, <2>4, SMTT(20)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             ReplyAttemptReplayValid,
             SameReplyAttemptIdentity
    <2>6. newAttempt =
             ReplyAttemptFor(owner, semantic, source)'
      BY <1>1, <2>2, <2>4, <2>5,
         ReplySameOwnedAttemptIdentityUniquePrime, SMTT(30)
         DEF ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsForSource, ReplyAttemptsFor,
             SameReplyAttemptIdentity
    <2>7. \/ ReplyAttemptCursor(newAttempt) =
                  ReplyAttemptCursor(oldAttempt)
           \/ ReplyAttemptRank(newAttempt) >
                  ReplyAttemptRank(oldAttempt)
      BY <1>1, <2>2, <2>4, <2>5,
         ReplyReplayValidCursorUnchangedOrRankAdvances
    <2>8. CASE ReplyAttemptCursor(newAttempt) =
                  ReplyAttemptCursor(oldAttempt)
      <3>1. /\ newAttempt.messageCursor = messageCursor
             /\ newAttempt.chunkCursor = chunkCursor
             /\ ~ReplyAttemptComplete(newAttempt)
        BY <2>2, <2>8, SMTT(10)
           DEF ReplyAttemptCursor, ReplyAttemptComplete
      <3>2. ReplyAttemptOwned(owner, semantic, source)'
        BY <2>2, <2>4, <2>5
           DEF ReplyAttemptOwned, ReplyAttemptsForSource,
               ReplyAttemptsFor, SameReplyAttemptIdentity
      <3>3. ReplySourceAtCursor(
               owner, semantic, source,
               messageCursor, chunkCursor)'
        BY <2>6, <3>1, <3>2
           DEF ReplySourceAtCursor
      <3> QED BY <3>3
    <2>9. CASE ReplyAttemptRank(newAttempt) >
                  ReplyAttemptRank(oldAttempt)
      <3>1. ReplyAttemptOwned(owner, semantic, source)'
        BY <2>2, <2>4, <2>5
           DEF ReplyAttemptOwned, ReplyAttemptsForSource,
               ReplyAttemptsFor, SameReplyAttemptIdentity
      <3>2. ReplyAttemptRank(oldAttempt) =
               messageCursor * (ReplyChunkCount + 1)
                 + chunkCursor
        BY <2>2 DEF ReplyAttemptRank
      <3>3. ReplyAttemptRank(newAttempt) >
               messageCursor * (ReplyChunkCount + 1)
                 + chunkCursor
        BY <2>9, <3>2
      <3>4. ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor)'
        BY <2>6, <3>1, <3>3
           DEF ReplySourceAdvancedFrom
      <3> QED BY <3>4
    <2> QED BY <2>7, <2>8, <2>9
  <1> QED BY <1>1

THEOREM ReplyAdvancedStepIsStable ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
    chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplyTenureAwareReplayStep
    /\ ReplySourceIsolationStep
    => ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplyTenureAwareReplayStep,
                ReplySourceIsolationStep
         PROVE ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
    <2>1. \E oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ \/ ReplyAttemptComplete(oldAttempt)
                \/ ReplyAttemptRank(oldAttempt) >
                     messageCursor * (ReplyChunkCount + 1)
                       + chunkCursor
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(30)
         DEF ReplySourceAdvancedFrom,
             ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant
    <2>2. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ \/ ReplyAttemptComplete(oldAttempt)
                \/ ReplyAttemptRank(oldAttempt) >
                     messageCursor * (ReplyChunkCount + 1)
                       + chunkCursor
      BY <2>1
    <2>3. \E newAttempt \in rrAttempts':
             ReplyAttemptReplayValid(oldAttempt, newAttempt)
      BY <1>1, <2>2
         DEF ReplyTenureAwareReplayStep,
             ReplyAttemptReplayStep
    <2>4. PICK newAttempt \in rrAttempts':
             ReplyAttemptReplayValid(oldAttempt, newAttempt)
      BY <2>3
    <2>5. /\ newAttempt \in ReplyAttemptSet
           /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
      BY <1>1, <2>2, <2>4, SMTT(20)
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteTypeInvariant,
             ReplyAttemptReplayValid,
             SameReplyAttemptIdentity
    <2>6. newAttempt =
             ReplyAttemptFor(owner, semantic, source)'
      BY <1>1, <2>2, <2>4, <2>5,
         ReplySameOwnedAttemptIdentityUniquePrime, SMTT(30)
         DEF ReplyAttemptOwned, ReplyAttemptFor,
             ReplyAttemptsForSource, ReplyAttemptsFor,
             SameReplyAttemptIdentity
    <2>7. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>2, <2>4, <2>5
         DEF ReplyAttemptOwned, ReplyAttemptsForSource,
             ReplyAttemptsFor, SameReplyAttemptIdentity
    <2>8. \/ ReplyAttemptCursor(newAttempt) =
                  ReplyAttemptCursor(oldAttempt)
           \/ ReplyAttemptRank(newAttempt) >
                  ReplyAttemptRank(oldAttempt)
      BY <1>1, <2>2, <2>4, <2>5,
         ReplyReplayValidCursorUnchangedOrRankAdvances
    <2>9. CASE ReplyAttemptComplete(oldAttempt)
      <3>1. ReplyAttemptComplete(newAttempt)
        BY <2>2, <2>4, <2>5, <2>9,
           ReplyReplayValidPreservesCompletion
      <3> QED BY <2>6, <2>7, <3>1
           DEF ReplySourceAdvancedFrom
    <2>10. CASE ReplyAttemptRank(oldAttempt) >
                   messageCursor * (ReplyChunkCount + 1)
                     + chunkCursor
      <3>1. CASE ReplyAttemptCursor(newAttempt) =
                    ReplyAttemptCursor(oldAttempt)
        <4>1. ReplyAttemptRank(newAttempt) =
                 ReplyAttemptRank(oldAttempt)
          BY <3>1, ReplyEqualCursorPreservesRankAndCompletion
        <4> QED BY <2>6, <2>7, <2>10, <4>1
             DEF ReplySourceAdvancedFrom
      <3>2. CASE ReplyAttemptRank(newAttempt) >
                    ReplyAttemptRank(oldAttempt)
        <4>1. /\ ReplyAttemptRank(newAttempt) \in Nat
               /\ ReplyAttemptRank(oldAttempt) \in Nat
               /\ messageCursor * (ReplyChunkCount + 1)
                    + chunkCursor \in Nat
          BY <1>1, <2>2, <2>5,
             ReplyAttemptRankTyped, ReplyCursorRankTyped
        <4>2. ReplyAttemptRank(newAttempt) >
                 messageCursor * (ReplyChunkCount + 1)
                   + chunkCursor
          BY <2>10, <3>2, <4>1,
             ReplyNaturalStrictTransitive
        <4> QED BY <2>6, <2>7, <4>2
             DEF ReplySourceAdvancedFrom
      <3> QED BY <2>8, <3>1, <3>2
    <2> QED BY <2>2, <2>9, <2>10
  <1> QED BY <1>1

THEOREM ReplyAttemptLookupStutters ==
  \A owner, semantic, source:
    rrAttempts' = rrAttempts
    => /\ (ReplyAttemptOwned(owner, semantic, source)'
             <=> ReplyAttemptOwned(owner, semantic, source))
       /\ ReplyAttemptFor(owner, semantic, source)' =
            ReplyAttemptFor(owner, semantic, source)
PROOF
  <1>1. ASSUME NEW owner,
                NEW semantic,
                NEW source,
                rrAttempts' = rrAttempts
         PROVE /\ (ReplyAttemptOwned(owner, semantic, source)'
                      <=> ReplyAttemptOwned(
                            owner, semantic, source))
               /\ ReplyAttemptFor(owner, semantic, source)' =
                    ReplyAttemptFor(owner, semantic, source)
    <2>1. ReplyAttemptsFor(owner, semantic)' =
             ReplyAttemptsFor(owner, semantic)
      BY <1>1 DEF ReplyAttemptsFor
    <2>2. ReplyAttemptsForSource(owner, semantic, source)' =
             ReplyAttemptsForSource(owner, semantic, source)
      BY <2>1 DEF ReplyAttemptsForSource
    <2> QED BY <2>2
         DEF ReplyAttemptOwned, ReplyAttemptFor
  <1> QED BY <1>1

THEOREM ReplyCursorBracketPersistsOrAdvances ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
    chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
    /\ [ReplySourceIsolationStep]_ReplyRouteVars
    => \/ ReplySourceAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceAdvancedFrom(
            owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                [ReplyTenureAwareReplayStep]_ReplyRouteVars,
                [ReplySourceIsolationStep]_ReplyRouteVars
         PROVE \/ ReplySourceAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
               \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
    <2>1. \/ /\ ReplyTenureAwareReplayStep
                  /\ ReplySourceIsolationStep
           \/ UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(10) DEF ReplyRouteVars
    <2>2. CASE /\ ReplyTenureAwareReplayStep
                   /\ ReplySourceIsolationStep
      <3> QED BY <1>1, <2>2,
           ReplyCursorStepPersistsOrAdvances
    <2>3. CASE UNCHANGED ReplyRouteVars
      <3>1. rrAttempts' = rrAttempts
        BY <2>3 DEF ReplyRouteVars
      <3>2. /\ (ReplyAttemptOwned(
                        owner, semantic, source)'
                    <=> ReplyAttemptOwned(
                          owner, semantic, source))
               /\ ReplyAttemptFor(owner, semantic, source)' =
                    ReplyAttemptFor(owner, semantic, source)
        BY <3>1, ReplyAttemptLookupStutters
      <3>3. ReplySourceAtCursor(
               owner, semantic, source,
               messageCursor, chunkCursor)'
        BY <1>1, <3>2 DEF ReplySourceAtCursor
      <3> QED BY <3>3
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyAdvancedBracketIsStable ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
    chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
    /\ [ReplySourceIsolationStep]_ReplyRouteVars
    => ReplySourceAdvancedFrom(
         owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                [ReplyTenureAwareReplayStep]_ReplyRouteVars,
                [ReplySourceIsolationStep]_ReplyRouteVars
         PROVE ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
    <2>1. \/ /\ ReplyTenureAwareReplayStep
                  /\ ReplySourceIsolationStep
           \/ UNCHANGED ReplyRouteVars
      BY <1>1, SMTT(10) DEF ReplyRouteVars
    <2>2. CASE /\ ReplyTenureAwareReplayStep
                   /\ ReplySourceIsolationStep
      <3> QED BY <1>1, <2>2, ReplyAdvancedStepIsStable
    <2>3. CASE UNCHANGED ReplyRouteVars
      <3>1. rrAttempts' = rrAttempts
        BY <2>3 DEF ReplyRouteVars
      <3>2. /\ (ReplyAttemptOwned(
                        owner, semantic, source)'
                    <=> ReplyAttemptOwned(
                          owner, semantic, source))
               /\ ReplyAttemptFor(owner, semantic, source)' =
                    ReplyAttemptFor(owner, semantic, source)
        BY <3>1, ReplyAttemptLookupStutters
      <3> QED BY <1>1, <3>2
           DEF ReplySourceAdvancedFrom
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

ReplyCursorBracketObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyRouteConfiguration
  /\ ReplyRouteSafetyInvariant
  /\ ReplyRouteSafetyInvariant'
  /\ ReplySourceAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
  /\ [ReplySourceIsolationStep]_ReplyRouteVars
  => \/ ReplySourceAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)'
     \/ ReplySourceAdvancedFrom(
          owner, semantic, source, messageCursor, chunkCursor)'

ReplyAdvancedBracketObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyRouteConfiguration
  /\ ReplyRouteSafetyInvariant
  /\ ReplyRouteSafetyInvariant'
  /\ ReplySourceAdvancedFrom(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
  /\ [ReplySourceIsolationStep]_ReplyRouteVars
  => ReplySourceAdvancedFrom(
       owner, semantic, source, messageCursor, chunkCursor)'

THEOREM ReplyCursorBracketObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyCursorBracketObligation(
      owner, semantic, source, messageCursor, chunkCursor)
BY ReplyCursorBracketPersistsOrAdvances
   DEF ReplyCursorBracketObligation

THEOREM ReplyAdvancedBracketObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyAdvancedBracketObligation(
      owner, semantic, source, messageCursor, chunkCursor)
BY ReplyAdvancedBracketIsStable
   DEF ReplyAdvancedBracketObligation

THEOREM ReplyCursorOrAdvancedPersists ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyRouteSpec =>
      [][(\/ ReplySourceAtCursor(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
              \/ ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor))
           => (\/ ReplySourceAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
                 \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor))']_ReplyRouteVars
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteSpec
         PROVE [][(\/ ReplySourceAtCursor(
                          owner, semantic, source,
                          messageCursor, chunkCursor)
                       \/ ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor))
                    => (\/ ReplySourceAtCursor(
                             owner, semantic, source,
                             messageCursor, chunkCursor)
                          \/ ReplySourceAdvancedFrom(
                             owner, semantic, source,
                             messageCursor, chunkCursor))']_ReplyRouteVars
    <2>1. /\ []ReplyRouteConfiguration
           /\ []ReplyRouteSafetyInvariant
      BY <1>1, ReplyRouteSpecAlwaysInductiveInvariant, PTL
         DEF ReplyRouteInductiveInvariant
    <2>2. /\ ReplyTenureAwareReplay
             /\ ReplySourceIsolation
      BY <1>1, ReplyRouteSpecAlwaysReplayAndIsolation
    <2>3. [][ReplyCursorBracketObligationsHold]_ReplyRouteVars
      BY ReplyCursorBracketObligationsHold, PTL
    <2>4. [][ReplyAdvancedBracketObligationsHold]_ReplyRouteVars
      BY ReplyAdvancedBracketObligationsHold, PTL
    <2>5. [][ReplyCursorBracketObligation(
                owner, semantic, source,
                messageCursor, chunkCursor)]_ReplyRouteVars
      BY <2>3, IsaM("blast")
    <2>6. [][ReplyAdvancedBracketObligation(
                owner, semantic, source,
                messageCursor, chunkCursor)]_ReplyRouteVars
      BY <2>4, IsaM("blast")
    <2> QED BY <2>1, <2>2, <2>5, <2>6, PTL
         DEF ReplyTenureAwareReplay, ReplySourceIsolation,
             ReplyCursorBracketObligation,
             ReplyAdvancedBracketObligation
  <1> QED BY <1>1

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
    <2>1. [][(\/ ReplySourceAtCursor(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
                   \/ ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor))
                => (\/ ReplySourceAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                      \/ ReplySourceAdvancedFrom(
                         owner, semantic, source,
                         messageCursor, chunkCursor))']_ReplyRouteVars
      BY <1>1, ReplyCursorOrAdvancedPersists
    <2> QED BY <1>1, <2>1, PTL
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
PROOF
  <1>1. ASSUME NEW attempt \in rrAttempts,
                ReplyTicketValidForAttempt(attempt)
         PROVE /\ attempt.ticketSemantic = {attempt.semantic}
               /\ attempt.ticketTarget =
                    {ReplySemanticTarget(attempt.semantic)}
               /\ attempt.ticketMessageCursor =
                    {attempt.messageCursor}
               /\ attempt.ticketChunkCursor = {attempt.chunkCursor}
    <2>1. ReplyTicketForAttempt(attempt) =
             ReplyTicket(
               attempt.owner, attempt.source, attempt.semantic,
               ReplySemanticTarget(attempt.semantic),
               rrConnectionTenure[attempt.owner][attempt.source],
               attempt.messageCursor, attempt.chunkCursor)
      BY <1>1 DEF ReplyTicketValidForAttempt
    <2> QED BY <2>1, SMTT(10)
         DEF ReplyTicketForAttempt, ReplyTicket
  <1> QED BY <1>1

THEOREM ReplyOwnedAttemptSelectedPrime ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyAttemptOwned(owner, semantic, source)' =>
      ReplyAttemptFor(owner, semantic, source)'
        \in ReplyAttemptsForSource(owner, semantic, source)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                ReplyAttemptOwned(owner, semantic, source)'
         PROVE ReplyAttemptFor(owner, semantic, source)'
                 \in ReplyAttemptsForSource(
                      owner, semantic, source)'
    <2>1. ReplyAttemptsForSource(
             owner, semantic, source)' # {}
      BY <1>1 DEF ReplyAttemptOwned
    <2> QED BY <2>1 DEF ReplyAttemptFor
  <1> QED BY <1>1

THEOREM ReplyOwnedAttemptIdentityPrime ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    ReplyAttemptOwned(owner, semantic, source)' =>
      LET attempt == ReplyAttemptFor(owner, semantic, source)'
      IN /\ attempt \in rrAttempts'
         /\ attempt.owner = owner
         /\ attempt.semantic = semantic
         /\ attempt.source = source
BY ReplyOwnedAttemptSelectedPrime
   DEF ReplyAttemptsForSource, ReplyAttemptsFor

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

THEOREM ReplyTicketPendingProvidesAcquirePreconditions ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A messageCursor, chunkCursor:
      ReplySourceTicketPending(
        owner, semantic, source, messageCursor, chunkCursor)
        => LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyRouteSafetyInvariant
              /\ ReplyAttemptOwned(owner, semantic, source)
              /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ ReplyAttemptCurrent(oldAttempt)
              /\ ReplyAttemptHasNoTicket(oldAttempt)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor,
                NEW chunkCursor,
                ReplySourceTicketPending(
                  owner, semantic, source, messageCursor, chunkCursor)
         PROVE LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
               IN /\ ReplyRouteSafetyInvariant
                  /\ ReplyAttemptOwned(owner, semantic, source)
                  /\ oldAttempt \in rrAttempts
                  /\ oldAttempt \in ReplyAttemptSet
                  /\ ReplyAttemptCurrent(oldAttempt)
                  /\ ReplyAttemptHasNoTicket(oldAttempt)
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyRouteSafetyInvariant
              /\ ReplyAttemptOwned(owner, semantic, source)
              /\ ReplyAttemptCurrent(oldAttempt)
              /\ ~ReplyTicketValidForAttempt(oldAttempt)
      BY <1>1, SMTT(20)
         DEF ReplySourceTicketPending, ReplySourceRouteStable,
             ReplySourceServiceEligible
    <2>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN oldAttempt \in rrAttempts
      BY <2>1, ReplyOwnedAttemptIdentity
    <2>3. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN oldAttempt \in ReplyAttemptSet
      BY <2>1, <2>2, SMTT(10)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>4. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN IF oldAttempt.ticketTenure = NoReplyTicketTenure
              THEN ReplyAttemptHasNoTicket(oldAttempt)
              ELSE ReplyTicketValidForAttempt(oldAttempt)
      BY <2>1, <2>2
         DEF ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant
    <2>5. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN ReplyAttemptHasNoTicket(oldAttempt)
      BY <2>1, <2>4, SMTT(10)
    <2> QED BY <2>1, <2>2, <2>3, <2>5
  <1> QED BY <1>1

THEOREM ReplyTicketAcquisitionChangesAttempt ==
  \A attempt \in ReplyAttemptSet:
    ReplyAttemptHasNoTicket(attempt) =>
      ReplyAttemptWithTicket(attempt) # attempt
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet,
                ReplyAttemptHasNoTicket(attempt)
         PROVE ReplyAttemptWithTicket(attempt) # attempt
    <2>1. /\ attempt.connectionTenure \in ReplyConnectionTenures
           /\ attempt.connectionTenure # NoReplyTicketTenure
      BY <1>1, SMTT(10)
         DEF ReplyAttemptSet, ReplyConnectionTenures,
             NoReplyTicketTenure
    <2>2. /\ ReplyAttemptWithTicket(attempt).ticketTenure =
                  attempt.connectionTenure
           /\ attempt.ticketTenure = NoReplyTicketTenure
      BY <1>1, SMTT(15)
         DEF ReplyAttemptWithTicket, ReplyAttemptHasNoTicket,
             ReplyAttemptSet
    <2> QED BY <2>1, <2>2, SMTT(10)
  <1> QED BY <1>1

THEOREM ReplyTicketAcquisitionHasLiveTenure ==
  \A attempt \in ReplyAttemptSet:
    ReplyAttemptWithTicket(attempt).ticketTenure #
      NoReplyTicketTenure
PROOF
  <1>1. ASSUME NEW attempt \in ReplyAttemptSet
         PROVE ReplyAttemptWithTicket(attempt).ticketTenure #
                 NoReplyTicketTenure
    <2>1. attempt.connectionTenure # NoReplyTicketTenure
      BY <1>1, SMTT(10)
         DEF ReplyAttemptSet, ReplyConnectionTenures,
             NoReplyTicketTenure
    <2>2. ReplyAttemptWithTicket(attempt).ticketTenure =
             attempt.connectionTenure
      BY <1>1, SMTT(15)
         DEF ReplyAttemptWithTicket, ReplyAttemptSet
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyDistinctSameIdentityReplacementChangesSet ==
  \A oldAttempt, newAttempt:
    /\ ReplyRouteSafetyInvariant
    /\ oldAttempt \in rrAttempts
    /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
    /\ newAttempt # oldAttempt
    => ReplaceReplyAttempt(oldAttempt, newAttempt) # rrAttempts
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW newAttempt,
                ReplyRouteSafetyInvariant,
                oldAttempt \in rrAttempts,
                SameReplyAttemptIdentity(oldAttempt, newAttempt),
                newAttempt # oldAttempt
         PROVE ReplaceReplyAttempt(oldAttempt, newAttempt) # rrAttempts
    <2>1. newAttempt \notin rrAttempts
      <3>1. ASSUME newAttempt \in rrAttempts
             PROVE FALSE
        <4>1. oldAttempt = newAttempt
          BY <1>1, <3>1, ReplySameOwnedAttemptIdentityUnique
        <4> QED BY <1>1, <4>1
      <3> QED BY <3>1
    <2>2. newAttempt \in
             ReplaceReplyAttempt(oldAttempt, newAttempt)
      BY SMT DEF ReplaceReplyAttempt
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyPendingAcquireReplacementChangesAttemptSet ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A messageCursor, chunkCursor:
      ReplySourceTicketPending(
        owner, semantic, source, messageCursor, chunkCursor)
        => LET oldAttempt ==
                 ReplyAttemptFor(owner, semantic, source)
               ticketed == ReplyAttemptWithTicket(oldAttempt)
           IN ReplaceReplyAttempt(oldAttempt, ticketed) # rrAttempts
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor,
                NEW chunkCursor,
                ReplySourceTicketPending(
                  owner, semantic, source, messageCursor, chunkCursor)
         PROVE LET oldAttempt ==
                     ReplyAttemptFor(owner, semantic, source)
                   ticketed == ReplyAttemptWithTicket(oldAttempt)
               IN ReplaceReplyAttempt(oldAttempt, ticketed) # rrAttempts
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyRouteSafetyInvariant
              /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ ReplyAttemptHasNoTicket(oldAttempt)
      BY <1>1, ReplyTicketPendingProvidesAcquirePreconditions
    <2>2. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
           IN /\ SameReplyAttemptIdentity(oldAttempt, ticketed)
              /\ ticketed # oldAttempt
      BY <2>1, ReplyTicketAcquisitionPreservesIdentityAndCursor,
         ReplyTicketAcquisitionChangesAttempt
    <2> QED BY <2>1, <2>2,
         ReplyDistinctSameIdentityReplacementChangesSet
  <1> QED BY <1>1

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
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
                 ticketed == ReplyAttemptWithTicket(oldAttempt)
           IN /\ ReplyAttemptOwned(owner, semantic, source)
              /\ ReplyAttemptCurrent(oldAttempt)
              /\ ReplyAttemptHasNoTicket(oldAttempt)
              /\ ReplaceReplyAttempt(oldAttempt, ticketed) # rrAttempts
      BY <1>1, ReplyTicketPendingProvidesAcquirePreconditions,
         ReplyPendingAcquireReplacementChangesAttemptSet
    <2> QED BY <1>1, <2>1, ExpandENABLED, Isa
         DEF AcquireReplyTicket, ReplyRouteVars
  <1> QED BY <1>1

THEOREM ReplyAcquireMakesSourceReady ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplySourceTicketPending(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ AcquireReplyTicket(owner, semantic, source)
    => ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteConfiguration,
                ReplySourceTicketPending(
                  owner, semantic, source, messageCursor, chunkCursor),
                AcquireReplyTicket(owner, semantic, source)
         PROVE ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
    <2>1. ReplyRouteInductiveInvariant
      BY <1>1
         DEF ReplyRouteInductiveInvariant,
             ReplySourceTicketPending
    <2>2. ReplyRouteInductiveInvariant'
      BY <1>1, <2>1,
         AcquireReplyTicketPreservesInductiveInvariant
    <2>3. \E oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ReplyAttemptCurrent(oldAttempt)
             /\ ReplyAttemptHasNoTicket(oldAttempt)
             /\ ~ReplyAttemptComplete(oldAttempt)
             /\ oldAttempt.messageCursor = messageCursor
             /\ oldAttempt.chunkCursor = chunkCursor
      BY <1>1, ReplyTicketPendingProvidesAcquirePreconditions,
         ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplySourceTicketPending, ReplySourceAtCursor
    <2>4. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ReplyAttemptCurrent(oldAttempt)
             /\ ReplyAttemptHasNoTicket(oldAttempt)
             /\ ~ReplyAttemptComplete(oldAttempt)
             /\ oldAttempt.messageCursor = messageCursor
             /\ oldAttempt.chunkCursor = chunkCursor
      BY <2>3
    <2>5. LET newAttempt == ReplyAttemptWithTicket(oldAttempt)
           IN /\ newAttempt \in ReplyAttemptSet
              /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
              /\ ReplyAttemptCursor(newAttempt) =
                   ReplyAttemptCursor(oldAttempt)
              /\ newAttempt.ticketTenure # NoReplyTicketTenure
              /\ rrAttempts' =
                   ReplaceReplyAttempt(oldAttempt, newAttempt)
      BY <1>1, <2>4,
         ReplyTicketAcquisitionPreservesAttemptType,
         ReplyTicketAcquisitionPreservesIdentityAndCursor,
         ReplyTicketAcquisitionHasLiveTenure
         DEF AcquireReplyTicket
    <2>6. ReplyAttemptWithTicket(oldAttempt) \in rrAttempts'
      BY <2>5, SMTT(10) DEF ReplaceReplyAttempt
    <2>7. PICK newAttempt \in rrAttempts':
             newAttempt = ReplyAttemptWithTicket(oldAttempt)
      BY <2>6
    <2>8. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>4, <2>5, <2>7
         DEF ReplyAttemptOwned,
             ReplyAttemptsForSource, ReplyAttemptsFor,
             SameReplyAttemptIdentity
    <2>9. LET selected ==
                   ReplyAttemptFor(owner, semantic, source)'
           IN /\ selected \in rrAttempts'
              /\ selected.owner = owner
              /\ selected.semantic = semantic
              /\ selected.source = source
      BY <2>8, ReplyOwnedAttemptIdentityPrime
    <2>10. SameReplyAttemptIdentity(
               newAttempt,
               ReplyAttemptFor(owner, semantic, source)')
      BY <2>4, <2>5, <2>7, <2>9, SMTT(10)
         DEF SameReplyAttemptIdentity
    <2>11. ReplyRouteSafetyInvariant'
      BY <2>2 DEF ReplyRouteInductiveInvariant
    <2>12. newAttempt =
              ReplyAttemptFor(owner, semantic, source)'
      BY <2>7, <2>9, <2>10, <2>11,
         ReplySameOwnedAttemptIdentityUniquePrime
    <2>13. ReplyTicketValidForAttempt(newAttempt)'
      BY <2>2, <2>5, <2>7, SMTT(20)
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant,
             ReplyRouteOwnershipInvariant
    <2>14. /\ ~ReplyAttemptComplete(newAttempt)
            /\ newAttempt.messageCursor = messageCursor
            /\ newAttempt.chunkCursor = chunkCursor
      BY <2>4, <2>5, <2>7, SMTT(10)
         DEF ReplyAttemptCursor, ReplyAttemptComplete
    <2>15. ReplyAttemptCurrent(newAttempt)'
      BY <2>13 DEF ReplyTicketValidForAttempt
    <2>16. ReplySourceRouteStable(owner, semantic, source)'
      BY <2>8, <2>12, <2>15
         DEF ReplySourceRouteStable
    <2>17. ReplySourceServiceEligible(owner, semantic, source)'
      BY <2>8, <2>12, <2>13
         DEF ReplySourceServiceEligible
    <2>18. ReplySourceAtCursor(
              owner, semantic, source, messageCursor, chunkCursor)'
      BY <2>8, <2>12, <2>14
         DEF ReplySourceAtCursor
    <2> QED BY <2>11, <2>16, <2>17, <2>18
         DEF ReplySourceReadyAtCursor
  <1> QED BY <1>1

ReplySourceCursorRetained(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
  IN \E newAttempt \in rrAttempts':
       /\ SameReplyAttemptIdentity(oldAttempt, newAttempt)
       /\ ReplyAttemptCursor(newAttempt) =
            ReplyAttemptCursor(oldAttempt)

THEOREM ReplyCursorPreservingReplacementRetainsCursor ==
  \A replacedBefore, replacedAfter, retainedBefore:
    /\ retainedBefore \in rrAttempts
    /\ SameReplyAttemptIdentity(replacedBefore, replacedAfter)
    /\ ReplyAttemptCursor(replacedAfter) =
         ReplyAttemptCursor(replacedBefore)
    /\ rrAttempts' =
         ReplaceReplyAttempt(replacedBefore, replacedAfter)
    => \E retainedAfter \in rrAttempts':
         /\ SameReplyAttemptIdentity(retainedBefore, retainedAfter)
         /\ ReplyAttemptCursor(retainedAfter) =
              ReplyAttemptCursor(retainedBefore)
PROOF
  <1>1. ASSUME NEW replacedBefore,
                NEW replacedAfter,
                NEW retainedBefore,
                retainedBefore \in rrAttempts,
                SameReplyAttemptIdentity(
                  replacedBefore, replacedAfter),
                ReplyAttemptCursor(replacedAfter) =
                  ReplyAttemptCursor(replacedBefore),
                rrAttempts' =
                  ReplaceReplyAttempt(replacedBefore, replacedAfter)
         PROVE \E retainedAfter \in rrAttempts':
                 /\ SameReplyAttemptIdentity(
                      retainedBefore, retainedAfter)
                 /\ ReplyAttemptCursor(retainedAfter) =
                      ReplyAttemptCursor(retainedBefore)
    <2>1. CASE retainedBefore = replacedBefore
      <3>1. replacedAfter \in rrAttempts'
        BY <1>1 DEF ReplaceReplyAttempt
      <3> QED BY <1>1, <2>1, <3>1
    <2>2. CASE retainedBefore # replacedBefore
      <3>1. retainedBefore \in rrAttempts'
        BY <1>1, <2>2 DEF ReplaceReplyAttempt
      <3>2. /\ SameReplyAttemptIdentity(
                       retainedBefore, retainedBefore)
              /\ ReplyAttemptCursor(retainedBefore) =
                   ReplyAttemptCursor(retainedBefore)
        BY DEF SameReplyAttemptIdentity
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyReconnectTransformPreservesIdentityAndCursor ==
  \A oldAttempt, routedAttempt, attempt:
    /\ attempt \in ReplyAttemptSet
    /\ SameReplyAttemptIdentity(oldAttempt, routedAttempt)
    /\ ReplyAttemptCursor(routedAttempt) =
         ReplyAttemptCursor(oldAttempt)
    => LET transformed ==
             ReplyAttemptAfterReconnectTransform(
               oldAttempt, routedAttempt, attempt)
       IN /\ SameReplyAttemptIdentity(attempt, transformed)
          /\ ReplyAttemptCursor(transformed) =
               ReplyAttemptCursor(attempt)
PROOF
  <1>1. ASSUME NEW oldAttempt,
                NEW routedAttempt,
                NEW attempt,
                attempt \in ReplyAttemptSet,
                SameReplyAttemptIdentity(oldAttempt, routedAttempt),
                ReplyAttemptCursor(routedAttempt) =
                  ReplyAttemptCursor(oldAttempt)
         PROVE LET transformed ==
                     ReplyAttemptAfterReconnectTransform(
                       oldAttempt, routedAttempt, attempt)
               IN /\ SameReplyAttemptIdentity(attempt, transformed)
                  /\ ReplyAttemptCursor(transformed) =
                       ReplyAttemptCursor(attempt)
    <2>1. CASE attempt = oldAttempt
      BY <1>1, <2>1
         DEF ReplyAttemptAfterReconnectTransform
    <2>2. CASE /\ attempt # oldAttempt
                 /\ attempt.owner = oldAttempt.owner
                 /\ attempt.source = oldAttempt.source
      BY <1>1, <2>2,
         ReplyTicketRemovalPreservesIdentityAndCursor
         DEF ReplyAttemptAfterReconnectTransform
    <2>3. CASE /\ attempt # oldAttempt
                 /\ ~(attempt.owner = oldAttempt.owner
                       /\ attempt.source = oldAttempt.source)
      BY <2>3
         DEF ReplyAttemptAfterReconnectTransform,
             SameReplyAttemptIdentity
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyRetainedCursorProvidesPostCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplySourceCursorRetained(owner, semantic, source)
    => ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteSafetyInvariant,
                ReplyRouteSafetyInvariant',
                ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplySourceCursorRetained(owner, semantic, source)
         PROVE ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ ~ReplyAttemptComplete(oldAttempt)
              /\ oldAttempt.messageCursor = messageCursor
              /\ oldAttempt.chunkCursor = chunkCursor
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplySourceAtCursor,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>2. PICK newAttempt \in rrAttempts':
             /\ SameReplyAttemptIdentity(
                  ReplyAttemptFor(owner, semantic, source), newAttempt)
             /\ ReplyAttemptCursor(newAttempt) =
                  ReplyAttemptCursor(
                    ReplyAttemptFor(owner, semantic, source))
      BY <1>1 DEF ReplySourceCursorRetained
    <2>3. newAttempt \in ReplyAttemptSet
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>4. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>1, <2>2
         DEF ReplyAttemptOwned,
             ReplyAttemptsForSource, ReplyAttemptsFor,
             SameReplyAttemptIdentity
    <2>5. LET selected ==
                   ReplyAttemptFor(owner, semantic, source)'
           IN /\ selected \in rrAttempts'
              /\ selected.owner = owner
              /\ selected.semantic = semantic
              /\ selected.source = source
      BY <2>4, ReplyOwnedAttemptIdentityPrime
    <2>6. SameReplyAttemptIdentity(
             newAttempt, ReplyAttemptFor(owner, semantic, source)')
      BY <2>1, <2>2, <2>5, SMTT(10)
         DEF SameReplyAttemptIdentity
    <2>7. newAttempt =
             ReplyAttemptFor(owner, semantic, source)'
      BY <1>1, <2>2, <2>5, <2>6,
         ReplySameOwnedAttemptIdentityUniquePrime
    <2>8. /\ ~ReplyAttemptComplete(newAttempt)
            /\ newAttempt.messageCursor = messageCursor
            /\ newAttempt.chunkCursor = chunkCursor
      BY <2>1, <2>2, SMTT(10)
         DEF ReplyAttemptCursor, ReplyAttemptComplete
    <2> QED BY <2>4, <2>7, <2>8
         DEF ReplySourceAtCursor
  <1> QED BY <1>1

THEOREM ReplyStableCursorClassifiesTicketPrime ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceRouteStable(owner, semantic, source)'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
    => \/ ReplySourceTicketPending(
            owner, semantic, source, messageCursor, chunkCursor)'
       \/ ReplySourceReadyAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)'
BY SMTT(30)
   DEF ReplySourceTicketPending, ReplySourceReadyAtCursor,
       ReplySourceServiceEligible, ReplySourceRouteStable,
       ReplyRouteSafetyInvariant, ReplyRouteOwnershipInvariant,
       ReplyTicketValidForAttempt, ReplyAttemptCurrent,
       ReplyAttemptOwned, ReplyAttemptFor, NoReplyTicketTenure

THEOREM ReplyRouteNextRetainsPendingCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceTicketPending(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplyRouteNext
    => ReplySourceCursorRetained(owner, semantic, source)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteInductiveInvariant,
                ReplySourceTicketPending(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplyRouteNext
         PROVE ReplySourceCursorRetained(owner, semantic, source)
    <2>1. LET retained ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ retained \in rrAttempts
              /\ retained \in ReplyAttemptSet
              /\ ReplyAttemptHasNoTicket(retained)
      BY <1>1, ReplyTicketPendingProvidesAcquirePreconditions
    <2>2. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveNewReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveNewReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>2
      <3>2. ReplyAttemptFor(owner, semantic, source) \in rrAttempts'
        BY <2>1, <3>1, SMTT(10)
           DEF ObserveNewReplySource
      <3> QED BY <3>2
           DEF ReplySourceCursorRetained,
               SameReplyAttemptIdentity
    <2>3. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveLaterReplyDelivery(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveLaterReplyDelivery(
                     actionOwner, actionSemantic, actionSource)
        BY <2>3
      <3>2. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
                 routed ==
                     ReplyAttemptWithRoute(
                       replaced,
                       rrNextDeliveryOrdinal[actionOwner],
                       rrConnectionTenure[actionOwner][actionSource])
             IN /\ replaced \in rrAttempts
                /\ replaced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(replaced, routed)
                /\ ReplyAttemptCursor(routed) =
                     ReplyAttemptCursor(replaced)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(replaced, routed)
        BY <1>1, <3>1, ReplyOwnedAttemptIdentity,
           ReplyRouteRefreshPreservesIdentityAndCursor
           DEF ObserveLaterReplyDelivery,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3> QED BY <2>1, <3>2,
           ReplyCursorPreservingReplacementRetainsCursor
           DEF ReplySourceCursorRetained
    <2>4. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   RetryExactReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   RetryExactReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>4
      <3>2. rrAttempts' = rrAttempts
        BY <3>1 DEF RetryExactReplySource, ReplyRouteVars
      <3> QED BY <2>1, <3>2
           DEF ReplySourceCursorRetained,
               SameReplyAttemptIdentity
    <2>5. CASE \E actionOwner \in ReplyOwners,
                   actionSource \in ReplySources:
                   RetireReplySource(actionOwner, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSource \in ReplySources:
                   RetireReplySource(actionOwner, actionSource)
        BY <2>5
      <3>2. LET retained ==
                     ReplyAttemptFor(owner, semantic, source)
                 retired ==
                     ReplyAttemptAfterRetire(
                       actionOwner, actionSource, retained)
             IN /\ SameReplyAttemptIdentity(retained, retired)
                /\ ReplyAttemptCursor(retired) =
                     ReplyAttemptCursor(retained)
        BY <1>1, <2>1, <3>1,
           ReplyRetireTransformTypedAndIdentity
           DEF ReplyRouteInductiveInvariant
      <3>3. LET retained ==
                     ReplyAttemptFor(owner, semantic, source)
                 retired ==
                     ReplyAttemptAfterRetire(
                       actionOwner, actionSource, retained)
             IN retired \in rrAttempts'
        BY <2>1, <3>1, SMTT(15)
           DEF RetireReplySource, ReplyAttemptAfterRetire
      <3> QED BY <3>2, <3>3
           DEF ReplySourceCursorRetained
    <2>6. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ReconnectReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ReconnectReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>6
      <3>2. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
                 routed ==
                     ReplyAttemptWithRoute(
                       replaced,
                       rrNextDeliveryOrdinal[actionOwner],
                       rrConnectionTenure[actionOwner][actionSource] + 1)
             IN /\ replaced \in rrAttempts
                /\ replaced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(replaced, routed)
                /\ ReplyAttemptCursor(routed) =
                     ReplyAttemptCursor(replaced)
        BY <1>1, <3>1, ReplyOwnedAttemptIdentity,
           ReplyRouteRefreshPreservesIdentityAndCursor
           DEF ReconnectReplySource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>3. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
                 routed ==
                     ReplyAttemptWithRoute(
                       replaced,
                       rrNextDeliveryOrdinal[actionOwner],
                       rrConnectionTenure[actionOwner][actionSource] + 1)
                 retained ==
                     ReplyAttemptFor(owner, semantic, source)
                 transformed ==
                     ReplyAttemptAfterReconnectTransform(
                       replaced, routed, retained)
             IN /\ SameReplyAttemptIdentity(retained, transformed)
                /\ ReplyAttemptCursor(transformed) =
                     ReplyAttemptCursor(retained)
        BY <2>1, <3>2,
           ReplyReconnectTransformPreservesIdentityAndCursor
      <3>4. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
                 routed ==
                     ReplyAttemptWithRoute(
                       replaced,
                       rrNextDeliveryOrdinal[actionOwner],
                       rrConnectionTenure[actionOwner][actionSource] + 1)
                 retained ==
                     ReplyAttemptFor(owner, semantic, source)
                 transformed ==
                     ReplyAttemptAfterReconnectTransform(
                       replaced, routed, retained)
             IN transformed \in rrAttempts'
        BY <2>1, <3>1, SMTT(20)
           DEF ReconnectReplySource, ReplyAttemptsAfterReconnect,
               ReplyAttemptAfterReconnectTransform
      <3> QED BY <3>3, <3>4
           DEF ReplySourceCursorRetained
    <2>7. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   AcquireReplyTicket(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   AcquireReplyTicket(
                     actionOwner, actionSemantic, actionSource)
        BY <2>7
      <3>2. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
                 ticketed == ReplyAttemptWithTicket(replaced)
             IN /\ replaced \in rrAttempts
                /\ replaced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(replaced, ticketed)
                /\ ReplyAttemptCursor(ticketed) =
                     ReplyAttemptCursor(replaced)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(replaced, ticketed)
        BY <1>1, <3>1, ReplyOwnedAttemptIdentity,
           ReplyTicketAcquisitionPreservesIdentityAndCursor
           DEF AcquireReplyTicket,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3> QED BY <2>1, <3>2,
           ReplyCursorPreservingReplacementRetainsCursor
           DEF ReplySourceCursorRetained
    <2>8. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics:
                   ServiceReplyRoute(actionOwner, actionSemantic)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics:
                   ServiceReplyRoute(actionOwner, actionSemantic)
        BY <2>8
      <3>2. LET selectedIndex ==
                     ReplySelectedSourceIndex(
                       actionOwner, actionSemantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 selected ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, selectedSource)
             IN /\ selected \in rrAttempts
                /\ selected \in ReplyAttemptSet
                /\ ReplyTicketValidForAttempt(selected)
        BY <1>1, <3>1, ReplySelectedPendingAttemptFacts
           DEF ServiceReplyRoute
      <3>3. LET selectedIndex ==
                     ReplySelectedSourceIndex(
                       actionOwner, actionSemantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 selected ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, selectedSource)
             IN selected.ticketTenure # NoReplyTicketTenure
        BY <3>2, SMTT(10)
           DEF ReplyTicketValidForAttempt,
               ReplyAttemptSet, ReplyConnectionTenures,
               NoReplyTicketTenure
      <3>4. LET retained ==
                     ReplyAttemptFor(owner, semantic, source)
                 selectedIndex ==
                     ReplySelectedSourceIndex(
                       actionOwner, actionSemantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 selected ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, selectedSource)
             IN retained # selected
        BY <2>1, <3>3
           DEF ReplyAttemptHasNoTicket
      <3>5. ReplyAttemptFor(owner, semantic, source) \in rrAttempts'
        BY <2>1, <3>1, <3>4, SMTT(10)
           DEF ServiceReplyRoute, ReplaceReplyAttempt
      <3> QED BY <3>5
           DEF ReplySourceCursorRetained,
               SameReplyAttemptIdentity
    <2> QED BY <1>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
         <2>8
         DEF ReplyRouteNext
  <1> QED BY <1>1

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
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteInductiveInvariant,
                ReplySourceRouteStable(owner, semantic, source)',
                ReplySourceTicketPending(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                [ReplyRouteNext]_ReplyRouteVars
         PROVE \/ ReplySourceTicketPending(
                      owner, semantic, source,
                      messageCursor, chunkCursor)'
               \/ ReplySourceReadyAtCursor(
                      owner, semantic, source,
                      messageCursor, chunkCursor)'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteBracketPreservesInductiveInvariant
    <2>2. /\ ReplyRouteSafetyInvariant
           /\ ReplyRouteSafetyInvariant'
      BY <1>1, <2>1 DEF ReplyRouteInductiveInvariant
    <2>3. ReplySourceCursorRetained(owner, semantic, source)
      <3>1. CASE ReplyRouteNext
        <4> QED BY <1>1, <3>1,
             ReplyRouteNextRetainsPendingCursor
      <3>2. CASE UNCHANGED ReplyRouteVars
        <4>1. rrAttempts' = rrAttempts
          BY <3>2 DEF ReplyRouteVars
        <4>2. ReplyAttemptFor(owner, semantic, source) \in rrAttempts
          BY <1>1, ReplyTicketPendingProvidesAcquirePreconditions
        <4> QED BY <4>1, <4>2
             DEF ReplySourceCursorRetained,
                 SameReplyAttemptIdentity
      <3> QED BY <1>1, <3>1, <3>2
    <2>4. ReplySourceAtCursor(
             owner, semantic, source, messageCursor, chunkCursor)'
      BY <1>1, <2>2, <2>3,
         ReplyRetainedCursorProvidesPostCursor
         DEF ReplySourceTicketPending
    <2> QED BY <1>1, <2>2, <2>4,
         ReplyStableCursorClassifiesTicketPrime
  <1> QED BY <1>1

ReplyStableCursorClassificationObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyRouteSafetyInvariant
  /\ ReplySourceRouteStable(owner, semantic, source)
  /\ ReplySourceAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)
  => \/ ReplySourceTicketPending(
          owner, semantic, source, messageCursor, chunkCursor)
     \/ ReplySourceReadyAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)

ReplyTicketCursorKeys ==
  ReplyOwners \X ReplySemantics \X ReplySources
    \X (0..ReplyMessageCount) \X (0..ReplyChunkCount)

ReplyStableCursorClassificationObligationsHold ==
  \A key \in ReplyTicketCursorKeys:
    ReplyStableCursorClassificationObligation(
      key[1], key[2], key[3], key[4], key[5])

THEOREM ReplyStableCursorClassificationObligationsHoldProof ==
  ReplyStableCursorClassificationObligationsHold
BY ReplyStableCursorClassifiesTicket
   DEF ReplyStableCursorClassificationObligationsHold,
       ReplyStableCursorClassificationObligation,
       ReplyTicketCursorKeys

ReplyTicketPendingPersistenceObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplySourceRouteStable(owner, semantic, source)'
  /\ ReplySourceTicketPending(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ [ReplyRouteNext]_ReplyRouteVars
  => \/ ReplySourceTicketPending(
          owner, semantic, source, messageCursor, chunkCursor)'
     \/ ReplySourceReadyAtCursor(
          owner, semantic, source, messageCursor, chunkCursor)'

ReplyTicketPendingPersistenceObligationsHold ==
  \A key \in ReplyTicketCursorKeys:
    ReplyTicketPendingPersistenceObligation(
      key[1], key[2], key[3], key[4], key[5])

THEOREM ReplyTicketPendingPersistenceObligationsHoldProof ==
  ReplyTicketPendingPersistenceObligationsHold
BY ReplyTicketPendingPersistsOrBecomesReady
   DEF ReplyTicketPendingPersistenceObligationsHold,
       ReplyTicketPendingPersistenceObligation,
       ReplyTicketCursorKeys

ReplyTicketPendingEnablementObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  ReplySourceTicketPending(
    owner, semantic, source, messageCursor, chunkCursor)
    => ENABLED
         <<AcquireReplyTicket(owner, semantic, source)>>_ReplyRouteVars

ReplyTicketPendingEnablementObligationsHold ==
  \A key \in ReplyTicketCursorKeys:
    ReplyTicketPendingEnablementObligation(
      key[1], key[2], key[3], key[4], key[5])

THEOREM ReplyTicketPendingEnablementObligationsHoldProof ==
  ReplyTicketPendingEnablementObligationsHold
PROOF
  <1>1. ASSUME NEW key \in ReplyTicketCursorKeys
         PROVE ReplyTicketPendingEnablementObligation(
                 key[1], key[2], key[3], key[4], key[5])
    <2>1. /\ key[1] \in ReplyOwners
           /\ key[2] \in ReplySemantics
           /\ key[3] \in ReplySources
           /\ key[4] \in 0..ReplyMessageCount
           /\ key[5] \in 0..ReplyChunkCount
      BY <1>1, SMT DEF ReplyTicketCursorKeys
    <2>2. ASSUME ReplySourceTicketPending(
                    key[1], key[2], key[3], key[4], key[5])
           PROVE ENABLED
                   <<AcquireReplyTicket(
                       key[1], key[2], key[3])>>_ReplyRouteVars
      <3>1. LET oldAttempt ==
                     ReplyAttemptFor(key[1], key[2], key[3])
                   ticketed == ReplyAttemptWithTicket(oldAttempt)
             IN /\ ReplyAttemptOwned(key[1], key[2], key[3])
                /\ ReplyAttemptCurrent(oldAttempt)
                /\ ReplyAttemptHasNoTicket(oldAttempt)
                /\ ReplaceReplyAttempt(oldAttempt, ticketed) #
                     rrAttempts
        BY <2>1, <2>2,
           ReplyTicketPendingProvidesAcquirePreconditions,
           ReplyPendingAcquireReplacementChangesAttemptSet
      <3> QED BY <2>1, <2>2, <3>1, ExpandENABLED, Isa
           DEF AcquireReplyTicket, ReplyRouteVars
    <2> QED BY <2>2
         DEF ReplyTicketPendingEnablementObligation
  <1> QED BY <1>1
       DEF ReplyTicketPendingEnablementObligationsHold

ReplyTicketAcquireReadinessObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplyRouteConfiguration
  /\ ReplySourceTicketPending(
       owner, semantic, source, messageCursor, chunkCursor)
  /\ <<AcquireReplyTicket(
            owner, semantic, source)>>_ReplyRouteVars
  => ReplySourceReadyAtCursor(
       owner, semantic, source, messageCursor, chunkCursor)'

ReplyTicketAcquireReadinessObligationsHold ==
  \A key \in ReplyTicketCursorKeys:
    ReplyTicketAcquireReadinessObligation(
      key[1], key[2], key[3], key[4], key[5])

THEOREM ReplyTicketAcquireReadinessObligationsHoldProof ==
  ReplyTicketAcquireReadinessObligationsHold
PROOF
  <1>1. ASSUME NEW key \in ReplyTicketCursorKeys
         PROVE ReplyTicketAcquireReadinessObligation(
                 key[1], key[2], key[3], key[4], key[5])
    <2>1. /\ key[1] \in ReplyOwners
           /\ key[2] \in ReplySemantics
           /\ key[3] \in ReplySources
           /\ key[4] \in 0..ReplyMessageCount
           /\ key[5] \in 0..ReplyChunkCount
      BY <1>1, SMT DEF ReplyTicketCursorKeys
    <2> QED BY <2>1, ReplyAcquireMakesSourceReady
         DEF ReplyTicketAcquireReadinessObligation
  <1> QED BY <1>1
       DEF ReplyTicketAcquireReadinessObligationsHold

ReplyStableCursorLiveSuffixObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  (/\ []ReplyRouteInductiveInvariant
   /\ [][ReplyRouteNext]_ReplyRouteVars
   /\ WF_ReplyRouteVars(
        AcquireReplyTicket(owner, semantic, source))
   /\ []ReplySourceRouteStable(owner, semantic, source))
  => ((/\ ReplySourceAtCursor(
            owner, semantic, source, messageCursor, chunkCursor)
           /\ ReplySourceRouteStable(owner, semantic, source))
        ~> ReplySourceReadyAtCursor(
             owner, semantic, source, messageCursor, chunkCursor))

THEOREM ReplyStableCursorLiveSuffixObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyStableCursorLiveSuffixObligation(
      owner, semantic, source, messageCursor, chunkCursor)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount
         PROVE ReplyStableCursorLiveSuffixObligation(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
    <2>1. ReplyStableCursorClassificationObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <1>1, ReplyStableCursorClassifiesTicket
         DEF ReplyStableCursorClassificationObligation
    <2>2. []ReplyStableCursorClassificationObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <2>1, PTL
    <2>3. ReplyTicketPendingPersistenceObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <1>1, ReplyTicketPendingPersistsOrBecomesReady
         DEF ReplyTicketPendingPersistenceObligation
    <2>4. [][ReplyTicketPendingPersistenceObligation(
               owner, semantic, source,
               messageCursor, chunkCursor)]_ReplyRouteVars
      BY <2>3, PTL
    <2>5. ReplyTicketPendingEnablementObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <1>1, ReplyTicketPendingEnablesAcquire
         DEF ReplyTicketPendingEnablementObligation
    <2>6. []ReplyTicketPendingEnablementObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <2>5, PTL
    <2>7. ReplyTicketAcquireReadinessObligation(
             owner, semantic, source, messageCursor, chunkCursor)
      BY <1>1, ReplyAcquireMakesSourceReady
         DEF ReplyTicketAcquireReadinessObligation
    <2>8. [][ReplyTicketAcquireReadinessObligation(
               owner, semantic, source,
               messageCursor, chunkCursor)]_ReplyRouteVars
      BY <2>7, PTL
    <2>9. ASSUME []ReplyRouteInductiveInvariant,
                  [][ReplyRouteNext]_ReplyRouteVars,
                  WF_ReplyRouteVars(
                    AcquireReplyTicket(owner, semantic, source)),
                  []ReplySourceRouteStable(owner, semantic, source)
           PROVE (/\ ReplySourceAtCursor(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
                     /\ ReplySourceRouteStable(
                          owner, semantic, source))
                   ~> ReplySourceReadyAtCursor(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
      <3>1. []ReplyRouteConfiguration
        BY <2>9, PTL DEF ReplyRouteInductiveInvariant
      <3>2. ReplySourceTicketPending(
               owner, semantic, source, messageCursor, chunkCursor)
               ~> ReplySourceReadyAtCursor(
                    owner, semantic, source, messageCursor, chunkCursor)
        BY <2>4, <2>6, <2>8, <2>9, <3>1, PTL
           DEF ReplyTicketPendingPersistenceObligation,
               ReplyTicketPendingEnablementObligation,
               ReplyTicketAcquireReadinessObligation
      <3> QED BY <2>2, <2>9, <3>2, PTL
           DEF ReplyRouteInductiveInvariant,
               ReplyStableCursorClassificationObligation
    <2> QED BY <2>9
         DEF ReplyStableCursorLiveSuffixObligation
  <1> QED BY <1>1

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
                NEW chunkCursor \in 0..ReplyChunkCount
         PROVE /\ ReplyRouteSpec
               /\ []ReplySourceRouteStable(owner, semantic, source)
               => ((/\ ReplySourceAtCursor(
                          owner, semantic, source,
                          messageCursor, chunkCursor)
                       /\ ReplySourceRouteStable(
                            owner, semantic, source))
                     ~> ReplySourceReadyAtCursor(
                          owner, semantic, source,
                          messageCursor, chunkCursor))
    <2>1. ReplyStableCursorLiveSuffixObligation(
             owner, semantic, source,
             messageCursor, chunkCursor)
      BY <1>1, ReplyStableCursorLiveSuffixObligationsHold,
         IsaM("blast")
    <2>2. ASSUME ReplyRouteSpec,
                  []ReplySourceRouteStable(owner, semantic, source)
           PROVE (/\ ReplySourceAtCursor(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
                     /\ ReplySourceRouteStable(
                          owner, semantic, source))
                   ~> ReplySourceReadyAtCursor(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
      <3>1. []ReplyRouteInductiveInvariant
        BY <2>2, ReplyRouteSpecAlwaysInductiveInvariant
      <3>2. [][ReplyRouteNext]_ReplyRouteVars
        BY <2>2, PTL DEF ReplyRouteSpec
      <3>3. WF_ReplyRouteVars(
               AcquireReplyTicket(owner, semantic, source))
        BY <2>2 DEF ReplyRouteSpec, ReplyRouteFairness
      <3> QED BY <2>1, <2>2, <3>1, <3>2, <3>3,
                   IsaM("blast")
           DEF ReplyStableCursorLiveSuffixObligation
    <2> QED BY <2>2
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

THEOREM ReplyReadySourceIndexPending ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    => ReplySourceIndex(source)
         \in ReplyPendingSourceIndices(owner, semantic)
BY ReplySourceIndexTyped, SMTT(20)
   DEF ReplySourceReadyAtCursor, ReplySourceServiceEligible,
       ReplySourceAtCursor, ReplyPendingSourceIndices

THEOREM ReplyIncompleteServiceChangesAttempt ==
  \A attempt:
    /\ ReplyRouteConfiguration
    /\ attempt \in ReplyAttemptSet
    /\ ~ReplyAttemptComplete(attempt)
    => ReplyAttemptAfterService(attempt) # attempt
PROOF
  <1>1. ASSUME NEW attempt,
                ReplyRouteConfiguration,
                attempt \in ReplyAttemptSet,
                ~ReplyAttemptComplete(attempt)
         PROVE ReplyAttemptAfterService(attempt) # attempt
    <2>1. CASE attempt.messageCursor < ReplyMessageCount
      <3>1. /\ attempt.messageCursor \in Nat
             /\ ReplyAttemptAfterService(attempt).messageCursor =
                  attempt.messageCursor + 1
        BY <1>1, <2>1, SMTT(10)
           DEF ReplyAttemptSet, ReplyAttemptAfterService
      <3> QED BY <3>1, SMT
    <2>2. CASE ~(attempt.messageCursor < ReplyMessageCount)
      <3>1. /\ attempt.chunkCursor \in Nat
             /\ ReplyAttemptAfterService(attempt).chunkCursor =
                  attempt.chunkCursor + 1
        BY <1>1, <2>2, SMTT(10)
           DEF ReplyAttemptSet, ReplyAttemptAfterService
      <3> QED BY <3>1, SMT
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyIncompleteServiceChangesCursor ==
  \A attempt:
    /\ attempt \in ReplyAttemptSet
    /\ ~ReplyAttemptComplete(attempt)
    => ReplyAttemptCursor(ReplyAttemptAfterService(attempt)) #
         ReplyAttemptCursor(attempt)
PROOF
  <1>1. ASSUME NEW attempt,
                attempt \in ReplyAttemptSet,
                ~ReplyAttemptComplete(attempt)
         PROVE ReplyAttemptCursor(
                 ReplyAttemptAfterService(attempt)) #
                   ReplyAttemptCursor(attempt)
    <2>1. CASE attempt.messageCursor < ReplyMessageCount
      BY <1>1, <2>1, SMTT(10)
         DEF ReplyAttemptCursor, ReplyAttemptSet,
             ReplyAttemptAfterService
    <2>2. CASE ~(attempt.messageCursor < ReplyMessageCount)
      BY <1>1, <2>2, SMTT(10)
         DEF ReplyAttemptCursor, ReplyAttemptSet,
             ReplyAttemptAfterService
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyIncompleteServiceAdvancesRank ==
  \A attempt:
    /\ ReplyRouteConfiguration
    /\ attempt \in ReplyAttemptSet
    /\ ~ReplyAttemptComplete(attempt)
    => ReplyAttemptRank(ReplyAttemptAfterService(attempt)) >
         ReplyAttemptRank(attempt)
PROOF
  <1>1. ASSUME NEW attempt,
                ReplyRouteConfiguration,
                attempt \in ReplyAttemptSet,
                ~ReplyAttemptComplete(attempt)
         PROVE ReplyAttemptRank(ReplyAttemptAfterService(attempt)) >
                 ReplyAttemptRank(attempt)
    <2>1. ReplyAttemptAfterService(attempt) \in ReplyAttemptSet
      BY <1>1, ReplyServicePreservesAttemptType
    <2>2. ReplyAttemptReplayValid(
             attempt, ReplyAttemptAfterService(attempt))
      BY <1>1, ReplyServiceProducesReplayValid
    <2>3. ReplyAttemptCursor(ReplyAttemptAfterService(attempt)) #
             ReplyAttemptCursor(attempt)
      BY <1>1, ReplyIncompleteServiceChangesCursor
    <2>4. \/ ReplyAttemptCursor(
                  ReplyAttemptAfterService(attempt)) =
                    ReplyAttemptCursor(attempt)
           \/ ReplyAttemptRank(ReplyAttemptAfterService(attempt)) >
                ReplyAttemptRank(attempt)
      BY <1>1, <2>1, <2>2,
         ReplyReplayValidCursorUnchangedOrRankAdvances
    <2> QED BY <2>3, <2>4
  <1> QED BY <1>1

THEOREM ReplyReadyCursorEnablesService ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteConfiguration
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    => ENABLED <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor, NEW chunkCursor,
                ReplyRouteConfiguration,
                ReplySourceReadyAtCursor(
                  owner, semantic, source, messageCursor, chunkCursor)
         PROVE ENABLED
                 <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
    <2>1. ReplyRouteInductiveInvariant
      BY <1>1
         DEF ReplyRouteInductiveInvariant,
             ReplySourceReadyAtCursor
    <2>2. ReplySourceIndex(source)
             \in ReplyPendingSourceIndices(owner, semantic)
      BY <1>1, ReplySourceIndexTyped, SMTT(20)
         DEF ReplySourceReadyAtCursor,
             ReplySourceServiceEligible, ReplySourceAtCursor,
             ReplyPendingSourceIndices
    <2>3. ReplyPendingSourceIndices(owner, semantic) # {}
      BY <2>2
    <2>4. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ ~ReplyAttemptComplete(oldAttempt)
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ serviced # oldAttempt
      BY <1>1, <2>1, <2>3,
         ReplySelectedPendingAttemptFacts,
         ReplyServicePreservesIdentity,
         ReplyIncompleteServiceChangesAttempt
    <2>5. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN oldAttempt \notin
                  ReplaceReplyAttempt(oldAttempt, serviced)
      BY <2>4, SMT DEF ReplaceReplyAttempt
    <2>6. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN ReplaceReplyAttempt(oldAttempt, serviced) # rrAttempts
      BY <2>4, <2>5
    <2>7. ServiceReplyRoute(owner, semantic) =>
             rrAttempts' # rrAttempts
      BY <2>6, SMTT(10) DEF ServiceReplyRoute
    <2>8. ServiceReplyRoute(owner, semantic)
             => <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
      BY <2>7, SMTT(10) DEF ReplyRouteVars
    <2>9. ENABLED ServiceReplyRoute(owner, semantic)
      BY <1>1, <2>3, ExpandENABLED, Isa
         DEF ServiceReplyRoute
    <2>10. ServiceReplyRoute(owner, semantic) \in BOOLEAN
      BY Isa DEF ServiceReplyRoute
    <2>11. <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
             \in BOOLEAN
      BY Isa
    <2>12. ENABLED ServiceReplyRoute(owner, semantic)
             => ENABLED
                  <<ServiceReplyRoute(
                      owner, semantic)>>_ReplyRouteVars
      BY <2>8, <2>10, <2>11, ENABLEDaxioms
    <2> QED BY <2>9, <2>12
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
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteInductiveInvariant,
                ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplySelectedSource(owner, semantic) = source,
                ServiceReplyRoute(owner, semantic)
         PROVE /\ ReplyAttemptHasNoTicket(
                      ReplyAttemptFor(owner, semantic, source))'
               /\ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ServiceReplyRoutePreservesInductiveInvariant
    <2>2. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ selectedSource = source
                /\ oldAttempt =
                     ReplyAttemptFor(owner, semantic, source)
                /\ oldAttempt \in rrAttempts
                /\ oldAttempt \in ReplyAttemptSet
                /\ oldAttempt.owner = owner
                /\ oldAttempt.semantic = semantic
                /\ oldAttempt.source = source
                /\ ~ReplyAttemptComplete(oldAttempt)
                /\ oldAttempt.messageCursor = messageCursor
                /\ oldAttempt.chunkCursor = chunkCursor
                /\ rrAttempts' =
                     ReplaceReplyAttempt(oldAttempt, serviced)
      BY <1>1, ReplySelectedPendingAttemptFacts,
         ReplyOwnedAttemptIdentity, SMTT(30)
         DEF ReplySelectedSource, ServiceReplyRoute,
             ReplySourceReadyAtCursor, ReplySourceAtCursor,
             ReplySourceServiceEligible
    <2>3. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ serviced \in ReplyAttemptSet
                /\ SameReplyAttemptIdentity(oldAttempt, serviced)
                /\ ReplyAttemptRank(serviced) >
                     ReplyAttemptRank(oldAttempt)
                /\ ReplyAttemptHasNoTicket(serviced)
      BY <1>1, <2>2,
         ReplyServicePreservesAttemptType,
         ReplyServicePreservesIdentity,
         ReplyIncompleteServiceAdvancesRank,
         SMTT(15)
         DEF ReplyRouteInductiveInvariant,
             ReplyAttemptAfterService, ReplyAttemptHasNoTicket,
             ReplyAttemptSet, NoReplyTicketTenure
    <2>4. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN serviced \in rrAttempts'
      BY <2>2, SMT DEF ReplaceReplyAttempt
    <2>5. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 oldAttempt ==
                   ReplyAttemptFor(
                     owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(oldAttempt)
             IN /\ serviced.owner = owner
                /\ serviced.semantic = semantic
                /\ serviced.source = source
                /\ ReplyAttemptRank(serviced) >
                     messageCursor * (ReplyChunkCount + 1) + chunkCursor
                /\ ReplyAttemptHasNoTicket(serviced)
      BY <2>2, <2>3, Zenon
         DEF SameReplyAttemptIdentity, ReplyAttemptRank
    <2>6. \E nextAttempt \in rrAttempts':
             /\ nextAttempt.owner = owner
             /\ nextAttempt.semantic = semantic
             /\ nextAttempt.source = source
             /\ ReplyAttemptRank(nextAttempt) >
                  messageCursor * (ReplyChunkCount + 1) + chunkCursor
             /\ ReplyAttemptHasNoTicket(nextAttempt)
      BY <2>4, <2>5, Zenon
    <2>7. PICK nextAttempt \in rrAttempts':
             /\ nextAttempt.owner = owner
             /\ nextAttempt.semantic = semantic
             /\ nextAttempt.source = source
             /\ ReplyAttemptRank(nextAttempt) >
                  messageCursor * (ReplyChunkCount + 1) + chunkCursor
             /\ ReplyAttemptHasNoTicket(nextAttempt)
      BY <2>6
    <2>8. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>7
         DEF ReplyAttemptOwned, ReplyAttemptsForSource,
             ReplyAttemptsFor
    <2>9. LET chosen ==
                   ReplyAttemptFor(owner, semantic, source)'
             IN /\ chosen \in rrAttempts'
                /\ chosen.owner = owner
                /\ chosen.semantic = semantic
                /\ chosen.source = source
      BY <1>1, <2>8, ReplyOwnedAttemptIdentityPrime
    <2>10. SameReplyAttemptIdentity(
             ReplyAttemptFor(owner, semantic, source)', nextAttempt)
      BY <2>7, <2>9 DEF SameReplyAttemptIdentity
    <2>11. ReplyAttemptFor(owner, semantic, source)' = nextAttempt
      BY <2>1, <2>7, <2>9, <2>10,
         ReplySameOwnedAttemptIdentityUniquePrime
         DEF ReplyRouteInductiveInvariant
    <2>12. ReplyAttemptHasNoTicket(
              ReplyAttemptFor(owner, semantic, source))'
      BY <2>7, <2>11
    <2>13. ReplySourceAdvancedFrom(
              owner, semantic, source, messageCursor, chunkCursor)'
      BY <2>7, <2>8, <2>11
         DEF ReplySourceAdvancedFrom
    <2> QED BY <2>12, <2>13
  <1> QED BY <1>1

THEOREM ReplyNonSelectedServiceRetainsReadyCursor ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplySelectedSource(owner, semantic) # source
    /\ ServiceReplyRoute(owner, semantic)
    => ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteInductiveInvariant,
                ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplySelectedSource(owner, semantic) # source,
                ServiceReplyRoute(owner, semantic)
         PROVE ReplySourceReadyAtCursor(
                 owner, semantic, source,
                 messageCursor, chunkCursor)'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ServiceReplyRoutePreservesInductiveInvariant
    <2>2. LET retained ==
                   ReplyAttemptFor(owner, semantic, source)
             IN /\ retained \in rrAttempts
                /\ retained \in ReplyAttemptSet
                /\ retained.owner = owner
                /\ retained.semantic = semantic
                /\ retained.source = source
                /\ ReplyTicketValidForAttempt(retained)
                /\ ~ReplyAttemptComplete(retained)
                /\ retained.messageCursor = messageCursor
                /\ retained.chunkCursor = chunkCursor
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplySourceReadyAtCursor, ReplySourceServiceEligible,
             ReplySourceAtCursor
    <2>3. LET selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 selected ==
                   ReplyAttemptFor(owner, semantic, selectedSource)
                 serviced == ReplyAttemptAfterService(selected)
             IN /\ selectedSource =
                     ReplySelectedSource(owner, semantic)
                /\ selected \in rrAttempts
                /\ selected \in ReplyAttemptSet
                /\ selected.owner = owner
                /\ selected.semantic = semantic
                /\ selected.source = selectedSource
                /\ SameReplyAttemptIdentity(selected, serviced)
                /\ rrAttempts' =
                     ReplaceReplyAttempt(selected, serviced)
                /\ rrConnectionTenure' = rrConnectionTenure
                /\ rrSourceActive' = rrSourceActive
      BY <1>1, ReplySelectedPendingAttemptFacts,
         ReplyOwnedAttemptIdentity, ReplyServicePreservesIdentity,
         SMTT(20)
         DEF ReplySelectedSource, ServiceReplyRoute
    <2>4. LET retained ==
                   ReplyAttemptFor(owner, semantic, source)
                 selectedIndex ==
                   ReplySelectedSourceIndex(owner, semantic)
                 selectedSource == ReplySourceOrder[selectedIndex]
                 selected ==
                   ReplyAttemptFor(owner, semantic, selectedSource)
             IN retained # selected
      BY <1>1, <2>2, <2>3
    <2>5. ReplyAttemptFor(owner, semantic, source) \in rrAttempts'
      BY <2>2, <2>3, <2>4, SMTT(10)
         DEF ReplaceReplyAttempt
    <2>6. ReplyAttemptOwned(owner, semantic, source)'
      BY <2>2, <2>5
         DEF ReplyAttemptOwned, ReplyAttemptsForSource,
             ReplyAttemptsFor
    <2>7. LET chosen ==
                   ReplyAttemptFor(owner, semantic, source)'
             IN /\ chosen \in rrAttempts'
                /\ chosen.owner = owner
                /\ chosen.semantic = semantic
                /\ chosen.source = source
      BY <1>1, <2>6, ReplyOwnedAttemptIdentityPrime
    <2>8. SameReplyAttemptIdentity(
             ReplyAttemptFor(owner, semantic, source)',
             ReplyAttemptFor(owner, semantic, source))
      BY <2>2, <2>7 DEF SameReplyAttemptIdentity
    <2>9. ReplyAttemptFor(owner, semantic, source)' =
             ReplyAttemptFor(owner, semantic, source)
      BY <2>1, <2>2, <2>5, <2>7, <2>8,
         ReplySameOwnedAttemptIdentityUniquePrime
         DEF ReplyRouteInductiveInvariant
    <2>10. ReplyTicketValidForAttempt(
               ReplyAttemptFor(owner, semantic, source))'
      BY <1>1, <2>2, <2>3, <2>9, SMTT(20)
         DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent,
             ReplyTicketForAttempt, ReplyTicket
    <2>11. ReplySourceRouteStable(owner, semantic, source)'
      BY <2>6, <2>9, <2>10
         DEF ReplySourceRouteStable, ReplyTicketValidForAttempt
    <2>12. ReplySourceServiceEligible(owner, semantic, source)'
      BY <2>6, <2>9, <2>10
         DEF ReplySourceServiceEligible
    <2>13. ReplySourceAtCursor(
               owner, semantic, source, messageCursor, chunkCursor)'
      BY <2>2, <2>6, <2>9
         DEF ReplySourceAtCursor
    <2> QED BY <2>1, <2>11, <2>12, <2>13
         DEF ReplyRouteInductiveInvariant,
             ReplySourceReadyAtCursor
  <1> QED BY <1>1

THEOREM ReplyPostTicketAndPointerPreserveServiceRank ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    /\ ReplyRouteSafetyInvariant'
    /\ ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, distance)
    /\ ReplySourceRouteStable(owner, semantic, source)'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
    /\ ReplyTicketValidForAttempt(
         ReplyAttemptFor(owner, semantic, source))'
    /\ rrNextServiceIndex'[owner][semantic] =
         rrNextServiceIndex[owner][semantic]
    => ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, distance)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier,
                ReplyRouteSafetyInvariant',
                ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance),
                ReplySourceRouteStable(owner, semantic, source)',
                ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)',
                ReplyTicketValidForAttempt(
                  ReplyAttemptFor(owner, semantic, source))',
                rrNextServiceIndex'[owner][semantic] =
                  rrNextServiceIndex[owner][semantic]
         PROVE ReplySourceServiceRank(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)'
    <2>1. ReplySourceReadyAtCursor(
             owner, semantic, source,
             messageCursor, chunkCursor)'
      BY <1>1, SMTT(10)
         DEF ReplySourceReadyAtCursor,
             ReplySourceServiceEligible,
             ReplySourceRouteStable
    <2>2. ReplySourceRoundRobinRank(
             owner, semantic, source) = distance
      BY <1>1 DEF ReplySourceServiceRank
    <2>3. ReplySourceRoundRobinRank(
             owner, semantic, source)' =
             ReplySourceRoundRobinRank(owner, semantic, source)
      BY <1>1, SMTT(10)
         DEF ReplySourceRoundRobinRank,
             ReplySourceCyclicDistance
    <2>4. ReplySourceRoundRobinRank(
             owner, semantic, source)' = distance
      BY <2>2, <2>3
    <2> QED BY <2>1, <2>4,
         ReplySourceServiceRankPrimeIntroduction
  <1> QED BY <1>1

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
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier,
                ReplyRouteInductiveInvariant,
                ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance),
                ServiceReplyRoute(owner, semantic)
         PROVE \/ ReplySourceAdvancedFrom(
                     owner, semantic, source,
                     messageCursor, chunkCursor)'
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ServiceReplyRoutePreservesInductiveInvariant
    <2>2. CASE ReplySelectedSource(owner, semantic) = source
      <3>1. ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor)'
        BY <1>1, <2>2,
           ReplySelectedServiceConsumesTicketAndAdvances
           DEF ReplySourceServiceRank
      <3> QED BY <3>1
    <2>3. CASE ReplySelectedSource(owner, semantic) # source
      <3>1. ReplySourceReadyAtCursor(
               owner, semantic, source,
               messageCursor, chunkCursor)'
        BY <1>1, <2>3,
           ReplyNonSelectedServiceRetainsReadyCursor
           DEF ReplySourceServiceRank
      <3>2. ReplySourceIndex(source)
                 \in ReplyPendingSourceIndices(owner, semantic)
        BY <1>1, ReplyReadySourceIndexPending
           DEF ReplyRouteInductiveInvariant,
               ReplySourceServiceRank
      <3>3. ReplyPendingSourceIndices(owner, semantic) # {}
        BY <3>2
      <3>4. LET selectedIndex ==
                     ReplySelectedSourceIndex(owner, semantic)
                   targetIndex == ReplySourceIndex(source)
               IN /\ selectedIndex \in
                        1..Len(ReplySourceOrder)
                  /\ targetIndex \in 1..Len(ReplySourceOrder)
                  /\ rrNextServiceIndex[owner][semantic]
                       \in 1..Len(ReplySourceOrder)
                  /\ selectedIndex # targetIndex
                  /\ ReplySourceCyclicDistance(
                       rrNextServiceIndex[owner][semantic],
                       selectedIndex)
                       <= ReplySourceCyclicDistance(
                            rrNextServiceIndex[owner][semantic],
                            targetIndex)
        BY <1>1, <2>3, <3>2, <3>3,
           ReplySelectedPendingAttemptFacts,
           ReplySourceIndexTyped,
           ReplySelectedSourceDistanceMinimal,
           SMTT(20)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant,
               ReplyRouteTypeInvariant,
               ReplySelectedSource
      <3>5. rrNextServiceIndex'[owner][semantic] =
               NextReplySourceIndex(
                 ReplySelectedSourceIndex(owner, semantic))
        BY <1>1, ReplyNestedFunctionalUpdateAtKey
           DEF ServiceReplyRoute, ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>6. ReplySourceCyclicDistance(
               NextReplySourceIndex(
                 ReplySelectedSourceIndex(owner, semantic)),
               ReplySourceIndex(source))
               < ReplySourceCyclicDistance(
                    rrNextServiceIndex[owner][semantic],
                    ReplySourceIndex(source))
        BY <1>1, <3>4, SMTT(30)
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteConfiguration,
               ReplySourceCyclicDistance,
               NextReplySourceIndex
      <3>7. ReplySourceRoundRobinRank(
               owner, semantic, source)' =
               ReplySourceCyclicDistance(
                 NextReplySourceIndex(
                   ReplySelectedSourceIndex(owner, semantic)),
                 ReplySourceIndex(source))
        BY <3>5, SMTT(10)
           DEF ReplySourceRoundRobinRank,
               ReplySourceCyclicDistance
      <3>8. ReplySourceRoundRobinRank(
               owner, semantic, source)'
               < ReplySourceRoundRobinRank(
                    owner, semantic, source)
        BY <3>6, <3>7 DEF ReplySourceRoundRobinRank
      <3>9. ReplySourceRoundRobinRank(
               owner, semantic, source)'
                 \in ReplyDistanceCarrier
        BY <2>1, ReplySourceRoundRobinRankTypedPrime
      <3>10. ReplySourceRoundRobinRank(
                owner, semantic, source) = distance
        BY <1>1 DEF ReplySourceServiceRank
      <3>11. \E lower \in SetLessThan(
                         distance, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
                  /\ ReplySourceReadyAtCursor(
                       owner, semantic, source,
                       messageCursor, chunkCursor)'
                  /\ lower \in ReplyDistanceCarrier
                  /\ ReplySourceRoundRobinRank(
                       owner, semantic, source)' = lower
        <4>1. ReplySourceRoundRobinRank(
                 owner, semantic, source)'
                 \in SetLessThan(
                      distance, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)
          BY <3>8, <3>9, <3>10, Isa
             DEF SetLessThan,
                 ReplyDistanceOrdering, ReplyDistanceCarrier,
                 OpToRel
        <4>2. /\ ReplySourceReadyAtCursor(
                     owner, semantic, source,
                     messageCursor, chunkCursor)'
               /\ ReplySourceRoundRobinRank(
                    owner, semantic, source)'
                    \in ReplyDistanceCarrier
               /\ ReplySourceRoundRobinRank(
                    owner, semantic, source)' =
                    ReplySourceRoundRobinRank(
                      owner, semantic, source)'
          BY <3>1, <3>9
        <4> QED BY <4>1, <4>2, IsaM("blast")
             DEF ReplySourceRoundRobinRank,
                 ReplySourceCyclicDistance
      <3>12. ReplySourceLowerServiceRank(
                owner, semantic, source,
                messageCursor, chunkCursor, distance)'
        BY <3>11, ReplySourceLowerServiceRankPrimeIntroduction,
           SMTT(10)
      <3> QED BY <3>12
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM ReplyPointwiseRouteStatePreservesValidTicket ==
  \A attempt:
    /\ ReplyTicketValidForAttempt(attempt)
    /\ rrConnectionTenure'[attempt.owner][attempt.source] =
         rrConnectionTenure[attempt.owner][attempt.source]
    /\ rrSourceActive'[attempt.owner][attempt.source] =
         rrSourceActive[attempt.owner][attempt.source]
    => ReplyTicketValidForAttempt(attempt)'
BY SMTT(20)
   DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent

THEOREM ReplyValidTicketIsNotNoTicket ==
  \A attempt \in ReplyAttemptSet:
    ReplyTicketValidForAttempt(attempt) =>
      ~ReplyAttemptHasNoTicket(attempt)
BY SMTT(20)
   DEF ReplyTicketValidForAttempt, ReplyAttemptHasNoTicket,
       ReplyAttemptSet, ReplyConnectionTenures,
       NoReplyTicketTenure

THEOREM ReplyReconnectRetainsOtherSourceAttempt ==
  \A replaced, routed, retained:
    /\ rrAttempts' =
         ReplyAttemptsAfterReconnect(replaced, routed)
    /\ retained \in rrAttempts
    /\ ~(retained.owner = replaced.owner
          /\ retained.source = replaced.source)
    => retained \in rrAttempts'
BY SMTT(20) DEF ReplyAttemptsAfterReconnect

THEOREM ReplyPostOwnedAttemptIsCandidate ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources:
    \A candidate:
      /\ ReplyRouteSafetyInvariant'
      /\ candidate \in rrAttempts'
      /\ candidate.owner = owner
      /\ candidate.semantic = semantic
      /\ candidate.source = source
      => candidate = ReplyAttemptFor(owner, semantic, source)'
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW candidate,
                ReplyRouteSafetyInvariant',
                candidate \in rrAttempts',
                candidate.owner = owner,
                candidate.semantic = semantic,
                candidate.source = source
         PROVE candidate =
                 ReplyAttemptFor(owner, semantic, source)'
    <2>1. ReplyAttemptOwned(owner, semantic, source)'
      BY <1>1
         DEF ReplyAttemptOwned,
             ReplyAttemptsForSource, ReplyAttemptsFor
    <2>2. LET selected ==
                   ReplyAttemptFor(owner, semantic, source)'
           IN /\ selected \in rrAttempts'
              /\ selected.owner = owner
              /\ selected.semantic = semantic
              /\ selected.source = source
      BY <2>1, ReplyOwnedAttemptIdentityPrime
    <2>3. SameReplyAttemptIdentity(
             candidate,
             ReplyAttemptFor(owner, semantic, source)')
      BY <1>1, <2>2 DEF SameReplyAttemptIdentity
    <2> QED BY <1>1, <2>2, <2>3,
         ReplySameOwnedAttemptIdentityUniquePrime
  <1> QED BY <1>1

ReplyRouteNonServiceNext ==
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveNewReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ObserveLaterReplyDelivery(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       RetryExactReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, source \in ReplySources:
       RetireReplySource(owner, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       ReconnectReplySource(owner, semantic, source)
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       AcquireReplyTicket(owner, semantic, source)

THEOREM ReplyNonServiceRetainsServicePointer ==
  ReplyRouteNonServiceNext =>
    rrNextServiceIndex' = rrNextServiceIndex
BY SMTT(20)
   DEF ReplyRouteNonServiceNext,
       ObserveNewReplySource, ObserveLaterReplyDelivery,
       RetryExactReplySource, RetireReplySource,
       ReconnectReplySource, AcquireReplyTicket,
       ReplyRouteVars

THEOREM ReplyRouteStutterRetainsReadyTicketAndPointer ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ UNCHANGED ReplyRouteVars
    => /\ ReplyTicketValidForAttempt(
              ReplyAttemptFor(owner, semantic, source))'
       /\ rrNextServiceIndex'[owner][semantic] =
            rrNextServiceIndex[owner][semantic]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                UNCHANGED ReplyRouteVars
         PROVE /\ ReplyTicketValidForAttempt(
                      ReplyAttemptFor(owner, semantic, source))'
               /\ rrNextServiceIndex'[owner][semantic] =
                    rrNextServiceIndex[owner][semantic]
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplySourceReadyAtCursor,
             ReplySourceServiceEligible,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2>2. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <2>1
    <2>3. /\ rrAttempts' = rrAttempts
            /\ rrConnectionTenure' = rrConnectionTenure
            /\ rrSourceActive' = rrSourceActive
            /\ rrNextServiceIndex' = rrNextServiceIndex
      BY <1>1 DEF ReplyRouteVars
    <2>4. ReplyAttemptFor(owner, semantic, source)' =
             ReplyAttemptFor(owner, semantic, source)
      BY <2>3, ReplyAttemptLookupStutters
    <2>5. ReplyTicketValidForAttempt(oldAttempt)'
      BY <2>2, <2>3,
         ReplyPointwiseRouteStatePreservesValidTicket
    <2> QED BY <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

THEOREM ReplyOtherRequestServiceRetainsReadyTicketAndPointer ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    \A actionOwner \in ReplyOwners,
       actionSemantic \in ReplySemantics:
      /\ ReplyRouteInductiveInvariant
      /\ ReplySourceReadyAtCursor(
           owner, semantic, source, messageCursor, chunkCursor)
      /\ ServiceReplyRoute(actionOwner, actionSemantic)
      /\ (actionOwner # owner \/ actionSemantic # semantic)
      => /\ ReplyTicketValidForAttempt(
                ReplyAttemptFor(owner, semantic, source))'
         /\ rrNextServiceIndex'[owner][semantic] =
              rrNextServiceIndex[owner][semantic]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW actionOwner \in ReplyOwners,
                NEW actionSemantic \in ReplySemantics,
                ReplyRouteInductiveInvariant,
                ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ServiceReplyRoute(actionOwner, actionSemantic),
                actionOwner # owner \/ actionSemantic # semantic
         PROVE /\ ReplyTicketValidForAttempt(
                      ReplyAttemptFor(owner, semantic, source))'
               /\ rrNextServiceIndex'[owner][semantic] =
                    rrNextServiceIndex[owner][semantic]
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplySourceReadyAtCursor, ReplySourceServiceEligible
    <2>2. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <2>1
    <2>3. ReplyRouteInductiveInvariant'
      BY <1>1, ServiceReplyRoutePreservesInductiveInvariant
    <2>4. LET selectedIndex ==
                   ReplySelectedSourceIndex(
                     actionOwner, actionSemantic)
                 selectedSource ==
                   ReplySourceOrder[selectedIndex]
                 selected ==
                   ReplyAttemptFor(
                     actionOwner, actionSemantic, selectedSource)
                 serviced == ReplyAttemptAfterService(selected)
           IN /\ selected \in rrAttempts
              /\ selected \in ReplyAttemptSet
              /\ selected.owner = actionOwner
              /\ selected.semantic = actionSemantic
              /\ selected.source = selectedSource
              /\ SameReplyAttemptIdentity(selected, serviced)
              /\ rrAttempts' =
                   ReplaceReplyAttempt(selected, serviced)
              /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
      BY <1>1, ReplySelectedPendingAttemptFacts,
         ReplyOwnedAttemptIdentity, ReplyServicePreservesIdentity,
         SMTT(20)
         DEF ServiceReplyRoute
    <2>5. LET selectedIndex ==
                   ReplySelectedSourceIndex(
                     actionOwner, actionSemantic)
                 selectedSource ==
                   ReplySourceOrder[selectedIndex]
                 selected ==
                   ReplyAttemptFor(
                     actionOwner, actionSemantic, selectedSource)
           IN oldAttempt # selected
      BY <1>1, <2>2, <2>4
    <2>6. oldAttempt \in rrAttempts'
      BY <2>2, <2>4, <2>5, SMTT(10)
         DEF ReplaceReplyAttempt
    <2>7. oldAttempt =
             ReplyAttemptFor(owner, semantic, source)'
      BY <2>2, <2>3, <2>6,
         ReplyPostOwnedAttemptIsCandidate
         DEF ReplyRouteInductiveInvariant
    <2>8. ReplyTicketValidForAttempt(oldAttempt)'
      BY <2>2, <2>4,
         ReplyPointwiseRouteStatePreservesValidTicket
    <2>9. rrNextServiceIndex'[owner][semantic] =
             rrNextServiceIndex[owner][semantic]
      BY <1>1, ReplyNestedFunctionalUpdateAwayFromKey
         DEF ServiceReplyRoute,
             ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
    <2> QED BY <2>7, <2>8, <2>9
  <1> QED BY <1>1

THEOREM ReplyNonServiceRetainsReadyTicketAndPointer ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    /\ ReplyRouteInductiveInvariant
    /\ ReplySourceReadyAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)
    /\ ReplySourceRouteStable(owner, semantic, source)'
    /\ ReplySourceAtCursor(
         owner, semantic, source, messageCursor, chunkCursor)'
    /\ ReplyRouteNonServiceNext
    => /\ ReplyTicketValidForAttempt(
              ReplyAttemptFor(owner, semantic, source))'
       /\ rrNextServiceIndex'[owner][semantic] =
            rrNextServiceIndex[owner][semantic]
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                ReplyRouteInductiveInvariant,
                ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor),
                ReplySourceRouteStable(owner, semantic, source)',
                ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)',
                ReplyRouteNonServiceNext
         PROVE /\ ReplyTicketValidForAttempt(
                      ReplyAttemptFor(owner, semantic, source))'
               /\ rrNextServiceIndex'[owner][semantic] =
                    rrNextServiceIndex[owner][semantic]
    <2>1. LET oldAttempt ==
                   ReplyAttemptFor(owner, semantic, source)
           IN /\ oldAttempt \in rrAttempts
              /\ oldAttempt \in ReplyAttemptSet
              /\ oldAttempt.owner = owner
              /\ oldAttempt.semantic = semantic
              /\ oldAttempt.source = source
              /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <1>1, ReplyOwnedAttemptIdentity, SMTT(20)
         DEF ReplyRouteInductiveInvariant,
             ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant,
             ReplySourceReadyAtCursor, ReplySourceServiceEligible
    <2>2. PICK oldAttempt \in rrAttempts:
             /\ oldAttempt =
                  ReplyAttemptFor(owner, semantic, source)
             /\ oldAttempt \in ReplyAttemptSet
             /\ oldAttempt.owner = owner
             /\ oldAttempt.semantic = semantic
             /\ oldAttempt.source = source
             /\ ReplyTicketValidForAttempt(oldAttempt)
      BY <2>1
    <2>3. rrNextServiceIndex'[owner][semantic] =
             rrNextServiceIndex[owner][semantic]
      BY <1>1, ReplyNonServiceRetainsServicePointer
    <2>4. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveNewReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveNewReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>4
      <3>2. ReplyRouteInductiveInvariant'
        BY <1>1, <3>1,
           ObserveNewReplySourcePreservesInductiveInvariant
      <3>3. oldAttempt \in rrAttempts'
        BY <2>2, <3>1 DEF ObserveNewReplySource
      <3>4. ReplyAttemptOwned(owner, semantic, source)'
        BY <2>2, <3>3
           DEF ReplyAttemptOwned,
               ReplyAttemptsForSource, ReplyAttemptsFor
      <3>5. LET postAttempt ==
                     ReplyAttemptFor(owner, semantic, source)'
             IN /\ postAttempt \in rrAttempts'
                /\ postAttempt.owner = owner
                /\ postAttempt.semantic = semantic
                /\ postAttempt.source = source
        BY <3>4, ReplyOwnedAttemptIdentityPrime
      <3>6. SameReplyAttemptIdentity(
               oldAttempt,
               ReplyAttemptFor(owner, semantic, source)')
        BY <2>2, <3>5 DEF SameReplyAttemptIdentity
      <3>7. oldAttempt =
               ReplyAttemptFor(owner, semantic, source)'
        BY <2>2, <3>2, <3>3, <3>5, <3>6,
           ReplySameOwnedAttemptIdentityUniquePrime
           DEF ReplyRouteInductiveInvariant
      <3>8. rrConnectionTenure' = rrConnectionTenure
        BY <3>1 DEF ObserveNewReplySource
      <3>9. rrSourceActive' = rrSourceActive
        BY <3>1 DEF ObserveNewReplySource
      <3>10. ReplyTicketValidForAttempt(oldAttempt)'
        BY <2>2, <3>8, <3>9,
           ReplyPointwiseRouteStatePreservesValidTicket
      <3> QED BY <2>3, <3>7, <3>10
    <2>5. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveLaterReplyDelivery(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ObserveLaterReplyDelivery(
                     actionOwner, actionSemantic, actionSource)
        BY <2>5
      <3>2. ReplyRouteInductiveInvariant'
        BY <1>1, <3>1,
           ObserveLaterReplyDeliveryPreservesInductiveInvariant
      <3>3. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
             IN /\ replaced \in rrAttempts
                /\ replaced \in ReplyAttemptSet
                /\ replaced.owner = actionOwner
                /\ replaced.semantic = actionSemantic
                /\ replaced.source = actionSource
                /\ rrNextDeliveryOrdinal[actionOwner]
                     \in ReplyDeliveryOrdinals
                /\ rrConnectionTenure[actionOwner][actionSource]
                     \in ReplyConnectionTenures
                /\ rrNextDeliveryOrdinal[actionOwner] >
                     replaced.deliveryOrdinal
        BY <1>1, <3>1, ReplyOwnedAttemptIdentity, SMTT(20)
           DEF ObserveLaterReplyDelivery,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>4. PICK deliveryOrdinal \in ReplyDeliveryOrdinals:
               deliveryOrdinal =
                 rrNextDeliveryOrdinal[actionOwner]
        BY <3>3
      <3>5. PICK replaced \in rrAttempts:
               /\ replaced =
                    ReplyAttemptFor(
                      actionOwner, actionSemantic, actionSource)
               /\ replaced \in ReplyAttemptSet
               /\ replaced.owner = actionOwner
               /\ replaced.semantic = actionSemantic
               /\ replaced.source = actionSource
               /\ deliveryOrdinal > replaced.deliveryOrdinal
        BY <3>3, <3>4
      <3>6. LET routed ==
                     ReplyAttemptWithRoute(
                       replaced, deliveryOrdinal,
                       rrConnectionTenure[actionOwner][actionSource])
             IN SameReplyAttemptIdentity(replaced, routed)
        BY <3>3, <3>4, <3>5,
           ReplyRouteRefreshPreservesIdentityAndCursor
      <3>7. ReplyAttemptWithRoute(
               replaced, deliveryOrdinal,
               rrConnectionTenure[actionOwner][actionSource])
                 \in rrAttempts'
        BY <3>1, <3>4, <3>5
           DEF ObserveLaterReplyDelivery, ReplaceReplyAttempt
      <3>8. PICK routed \in rrAttempts':
               /\ routed =
                    ReplyAttemptWithRoute(
                      replaced, deliveryOrdinal,
                      rrConnectionTenure[
                        actionOwner][actionSource])
               /\ SameReplyAttemptIdentity(replaced, routed)
        BY <3>6, <3>7
      <3>9. /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
        BY <3>1 DEF ObserveLaterReplyDelivery
      <3>10. CASE oldAttempt = replaced
        <4>1. rrConnectionTenure[actionOwner][actionSource] =
                 replaced.connectionTenure
          BY <2>2, <3>5, <3>10, SMTT(10)
             DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent
        <4>2. routed =
                 ReplyAttemptWithRoute(
                   replaced, deliveryOrdinal,
                   replaced.connectionTenure)
          BY <3>8, <4>1
        <4>3. ReplyTicketValidForAttempt(
                 ReplyAttemptWithRoute(
                   replaced, deliveryOrdinal,
                   replaced.connectionTenure))'
          BY <2>2, <3>5, <3>9, <3>10,
             ReplySameTenureRefreshPreservesValidTicket
        <4>4. ReplyTicketValidForAttempt(routed)'
          BY <4>2, <4>3
        <4>5. /\ routed.owner = owner
                /\ routed.semantic = semantic
                /\ routed.source = source
          BY <2>2, <3>8, <3>10
             DEF SameReplyAttemptIdentity
        <4>6. routed =
                 ReplyAttemptFor(owner, semantic, source)'
          BY <3>2, <3>8, <4>5,
             ReplyPostOwnedAttemptIsCandidate
             DEF ReplyRouteInductiveInvariant
        <4> QED BY <2>3, <4>4, <4>6
      <3>11. CASE oldAttempt # replaced
        <4>1. oldAttempt \in rrAttempts'
          BY <2>2, <3>1, <3>4, <3>5, <3>11
             DEF ObserveLaterReplyDelivery,
                 ReplaceReplyAttempt
        <4>2. ReplyTicketValidForAttempt(oldAttempt)'
          BY <2>2, <3>9,
             ReplyPointwiseRouteStatePreservesValidTicket
        <4>3. oldAttempt =
                 ReplyAttemptFor(owner, semantic, source)'
          BY <2>2, <3>2, <4>1,
             ReplyPostOwnedAttemptIsCandidate
             DEF ReplyRouteInductiveInvariant
        <4> QED BY <2>3, <4>2, <4>3
      <3> QED BY <3>10, <3>11
    <2>6. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   RetryExactReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   RetryExactReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>6
      <3>2. /\ rrAttempts' = rrAttempts
              /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
        BY <3>1 DEF RetryExactReplySource, ReplyRouteVars
      <3>3. ReplyAttemptFor(owner, semantic, source)' =
               ReplyAttemptFor(owner, semantic, source)
        BY <3>2, ReplyAttemptLookupStutters
      <3>4. ReplyTicketValidForAttempt(oldAttempt)'
        BY <2>2, <3>2,
           ReplyPointwiseRouteStatePreservesValidTicket
      <3> QED BY <2>2, <2>3, <3>3, <3>4
    <2>7. CASE \E actionOwner \in ReplyOwners,
                   actionSource \in ReplySources:
                   RetireReplySource(actionOwner, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSource \in ReplySources:
                   RetireReplySource(actionOwner, actionSource)
        BY <2>7
      <3>2. ReplyRouteInductiveInvariant'
        BY <1>1, <3>1,
           RetireReplySourcePreservesInductiveInvariant
      <3>3. rrSourceActive'[owner][source]
        BY <1>1, ReplyOwnedAttemptIdentityPrime, SMTT(10)
           DEF ReplySourceRouteStable, ReplyAttemptCurrent
      <3>4. [rrSourceActive EXCEPT
                ![actionOwner][actionSource] = FALSE]
                [actionOwner][actionSource] = FALSE
        BY <1>1, <3>1, ReplyNestedFunctionalUpdateAtKey
           DEF ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>5. ~(owner = actionOwner /\ source = actionSource)
        BY <3>1, <3>3, <3>4
           DEF RetireReplySource
      <3>6. oldAttempt \in rrAttempts'
        BY <2>2, <3>1, <3>5, SMTT(10)
           DEF RetireReplySource, ReplyAttemptAfterRetire
      <3>7. rrSourceActive'[owner][source] =
               rrSourceActive[owner][source]
        BY <1>1, <3>1, <3>5,
           ReplyNestedFunctionalUpdateAwayFromKey
           DEF RetireReplySource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>8. rrConnectionTenure' = rrConnectionTenure
        BY <3>1 DEF RetireReplySource
      <3>9. ReplyTicketValidForAttempt(oldAttempt)'
        BY <2>2, <3>7, <3>8,
           ReplyPointwiseRouteStatePreservesValidTicket
      <3>10. oldAttempt =
                ReplyAttemptFor(owner, semantic, source)'
        BY <2>2, <3>2, <3>6,
           ReplyPostOwnedAttemptIsCandidate
           DEF ReplyRouteInductiveInvariant
      <3> QED BY <2>3, <3>9, <3>10
    <2>8. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ReconnectReplySource(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   ReconnectReplySource(
                     actionOwner, actionSemantic, actionSource)
        BY <2>8
      <3>2. ReplyRouteInductiveInvariant'
        BY <1>1, <3>1,
           ReconnectReplySourcePreservesInductiveInvariant
      <3>3. rrSourceActive[owner][source]
        BY <2>2
           DEF ReplyTicketValidForAttempt, ReplyAttemptCurrent
      <3>4. ~rrSourceActive[actionOwner][actionSource]
        BY <3>1 DEF ReconnectReplySource
      <3>5. ~(owner = actionOwner /\ source = actionSource)
        BY <3>3, <3>4
      <3>6. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
             IN /\ replaced \in rrAttempts
                /\ replaced.owner = actionOwner
                /\ replaced.semantic = actionSemantic
                /\ replaced.source = actionSource
        BY <3>1, ReplyOwnedAttemptIdentity
           DEF ReconnectReplySource
      <3>7. PICK replaced \in rrAttempts:
               /\ replaced =
                    ReplyAttemptFor(
                      actionOwner, actionSemantic, actionSource)
               /\ replaced.owner = actionOwner
               /\ replaced.semantic = actionSemantic
               /\ replaced.source = actionSource
        BY <3>6
      <3>8. ~(oldAttempt.owner = replaced.owner
                /\ oldAttempt.source = replaced.source)
        BY <2>2, <3>5, <3>7
      <3>9. rrAttempts' =
               ReplyAttemptsAfterReconnect(
                 replaced,
                 ReplyAttemptWithRoute(
                   replaced,
                   rrNextDeliveryOrdinal[actionOwner],
                   rrConnectionTenure[actionOwner][actionSource] + 1))
        BY <3>1, <3>7 DEF ReconnectReplySource
      <3>10. oldAttempt \in rrAttempts'
        BY <2>2, <3>8, <3>9,
           ReplyReconnectRetainsOtherSourceAttempt
      <3>11. rrConnectionTenure'[owner][source] =
               rrConnectionTenure[owner][source]
        BY <1>1, <3>1, <3>5,
           ReplyNestedFunctionalUpdateAwayFromKey
           DEF ReconnectReplySource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>12. rrSourceActive'[owner][source] =
               rrSourceActive[owner][source]
        BY <1>1, <3>1, <3>5,
           ReplyNestedFunctionalUpdateAwayFromKey
           DEF ReconnectReplySource,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>13. ReplyTicketValidForAttempt(oldAttempt)'
        BY <2>2, <3>11, <3>12,
           ReplyPointwiseRouteStatePreservesValidTicket
      <3>14. oldAttempt =
                ReplyAttemptFor(owner, semantic, source)'
        BY <2>2, <3>2, <3>10,
           ReplyPostOwnedAttemptIsCandidate
           DEF ReplyRouteInductiveInvariant
      <3> QED BY <2>3, <3>13, <3>14
    <2>9. CASE \E actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   AcquireReplyTicket(
                     actionOwner, actionSemantic, actionSource)
      <3>1. PICK actionOwner \in ReplyOwners,
                   actionSemantic \in ReplySemantics,
                   actionSource \in ReplySources:
                   AcquireReplyTicket(
                     actionOwner, actionSemantic, actionSource)
        BY <2>9
      <3>2. ReplyRouteInductiveInvariant'
        BY <1>1, <3>1,
           AcquireReplyTicketPreservesInductiveInvariant
      <3>3. LET replaced ==
                     ReplyAttemptFor(
                       actionOwner, actionSemantic, actionSource)
             IN /\ replaced \in rrAttempts
                /\ replaced \in ReplyAttemptSet
                /\ replaced.owner = actionOwner
                /\ replaced.semantic = actionSemantic
                /\ replaced.source = actionSource
                /\ ReplyAttemptHasNoTicket(replaced)
        BY <1>1, <3>1, ReplyOwnedAttemptIdentity, SMTT(10)
           DEF AcquireReplyTicket,
               ReplyRouteInductiveInvariant,
               ReplyRouteSafetyInvariant, ReplyRouteTypeInvariant
      <3>4. PICK replaced \in rrAttempts:
               /\ replaced =
                    ReplyAttemptFor(
                      actionOwner, actionSemantic, actionSource)
               /\ replaced \in ReplyAttemptSet
               /\ replaced.owner = actionOwner
               /\ replaced.semantic = actionSemantic
               /\ replaced.source = actionSource
               /\ ReplyAttemptHasNoTicket(replaced)
        BY <3>3
      <3>5. ~ReplyAttemptHasNoTicket(oldAttempt)
        BY <2>2, ReplyValidTicketIsNotNoTicket
      <3>6. oldAttempt # replaced
        BY <3>4, <3>5
      <3>7. oldAttempt \in rrAttempts'
        BY <2>2, <3>1, <3>4, <3>6
           DEF AcquireReplyTicket, ReplaceReplyAttempt
      <3>8. /\ rrConnectionTenure' = rrConnectionTenure
              /\ rrSourceActive' = rrSourceActive
        BY <3>1 DEF AcquireReplyTicket
      <3>9. ReplyTicketValidForAttempt(oldAttempt)'
        BY <2>2, <3>8,
           ReplyPointwiseRouteStatePreservesValidTicket
      <3>10. oldAttempt =
                ReplyAttemptFor(owner, semantic, source)'
        BY <2>2, <3>2, <3>7,
           ReplyPostOwnedAttemptIsCandidate
           DEF ReplyRouteInductiveInvariant
      <3> QED BY <2>3, <3>9, <3>10
    <2> QED BY <1>1, <2>3, <2>4, <2>5, <2>6, <2>7, <2>8, <2>9
         DEF ReplyRouteNonServiceNext
  <1> QED BY <1>1

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
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier,
                ReplyRouteInductiveInvariant,
                ReplySourceRouteStable(owner, semantic, source)',
                ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance),
                [ReplyRouteNext]_ReplyRouteVars
         PROVE \/ ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, distance)'
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
               \/ ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)'
    <2>1. ReplyRouteInductiveInvariant'
      BY <1>1, ReplyRouteBracketPreservesInductiveInvariant
    <2>2. /\ [ReplyTenureAwareReplayStep]_ReplyRouteVars
            /\ [ReplySourceIsolationStep]_ReplyRouteVars
      BY <1>1, ReplyRouteBracketProvidesReplayAndIsolation
    <2>3. /\ ReplyRouteConfiguration
            /\ ReplyRouteSafetyInvariant
            /\ ReplyRouteSafetyInvariant'
            /\ ReplySourceReadyAtCursor(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
            /\ ReplySourceAtCursor(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
      BY <1>1, <2>1
         DEF ReplyRouteInductiveInvariant,
             ReplySourceServiceRank,
             ReplySourceReadyAtCursor
    <2>4. \/ ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
            \/ ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor)'
      BY <2>2, <2>3,
         ReplyCursorBracketPersistsOrAdvances
    <2>5. CASE ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
      <3> QED BY <2>5
    <2>6. CASE ReplySourceAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)'
      <3>1. \/ UNCHANGED ReplyRouteVars
              \/ ReplyRouteNext
        BY <1>1
      <3>2. CASE UNCHANGED ReplyRouteVars
        <4>1. /\ ReplyTicketValidForAttempt(
                      ReplyAttemptFor(
                        owner, semantic, source))'
                /\ rrNextServiceIndex'[owner][semantic] =
                     rrNextServiceIndex[owner][semantic]
          BY <2>3, <3>2,
             ReplyRouteStutterRetainsReadyTicketAndPointer
        <4>2. ReplySourceServiceRank(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)'
          BY <1>1, <2>3, <2>6, <4>1,
             ReplyPostTicketAndPointerPreserveServiceRank
        <4> QED BY <4>2
      <3>3. CASE ReplyRouteNext
        <4>1. \/ ReplyRouteNonServiceNext
                \/ \E actionOwner \in ReplyOwners,
                      actionSemantic \in ReplySemantics:
                      ServiceReplyRoute(
                        actionOwner, actionSemantic)
          BY <3>3
             DEF ReplyRouteNext, ReplyRouteNonServiceNext
        <4>2. CASE ReplyRouteNonServiceNext
          <5>1. /\ ReplyTicketValidForAttempt(
                        ReplyAttemptFor(
                          owner, semantic, source))'
                  /\ rrNextServiceIndex'[owner][semantic] =
                       rrNextServiceIndex[owner][semantic]
            BY <1>1, <2>3, <2>6, <4>2,
               ReplyNonServiceRetainsReadyTicketAndPointer
          <5>2. ReplySourceServiceRank(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)'
            BY <1>1, <2>3, <2>6, <5>1,
               ReplyPostTicketAndPointerPreserveServiceRank
          <5> QED BY <5>2
        <4>3. CASE \E actionOwner \in ReplyOwners,
                     actionSemantic \in ReplySemantics:
                     ServiceReplyRoute(
                       actionOwner, actionSemantic)
          <5>1. PICK actionOwner \in ReplyOwners,
                       actionSemantic \in ReplySemantics:
                       ServiceReplyRoute(
                         actionOwner, actionSemantic)
            BY <4>3
          <5>2. CASE /\ actionOwner = owner
                         /\ actionSemantic = semantic
            <6>1. \/ ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor)'
                    \/ ReplySourceLowerServiceRank(
                         owner, semantic, source,
                         messageCursor, chunkCursor, distance)'
              BY <1>1, <5>1, <5>2,
                 ReplyServiceLowersRankOrAdvancesTarget
            <6> QED BY <6>1
          <5>3. CASE actionOwner # owner
                       \/ actionSemantic # semantic
            <6>1. /\ ReplyTicketValidForAttempt(
                          ReplyAttemptFor(
                            owner, semantic, source))'
                    /\ rrNextServiceIndex'[owner][semantic] =
                         rrNextServiceIndex[owner][semantic]
              BY <1>1, <2>3, <5>1, <5>3,
                 ReplyOtherRequestServiceRetainsReadyTicketAndPointer
            <6>2. ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, distance)'
              BY <1>1, <2>3, <2>6, <6>1,
                 ReplyPostTicketAndPointerPreserveServiceRank
            <6> QED BY <6>2
          <5> QED BY <5>2, <5>3
        <4> QED BY <4>1, <4>2, <4>3
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>4, <2>5, <2>6
  <1> QED BY <1>1

ReplyServiceRankPersistenceObligation(
    owner, semantic, source, messageCursor, chunkCursor, distance) ==
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

THEOREM ReplyServiceRankPersistenceObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    ReplyServiceRankPersistenceObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, distance)
BY ReplyServiceRankPersistsOrExits
   DEF ReplyServiceRankPersistenceObligation

ReplyServiceRankEnablementObligation(
    owner, semantic, source, messageCursor, chunkCursor, distance) ==
  (/\ ReplyRouteConfiguration
   /\ ReplySourceServiceRank(
        owner, semantic, source,
        messageCursor, chunkCursor, distance))
  => ENABLED
       <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars

THEOREM ReplyServiceRankEnablementObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    ReplyServiceRankEnablementObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, distance)
BY ReplyReadyCursorEnablesService
   DEF ReplyServiceRankEnablementObligation,
       ReplySourceServiceRank

ReplyServiceRankOutcomeObligation(
    owner, semantic, source, messageCursor, chunkCursor, distance) ==
  /\ ReplyRouteInductiveInvariant
  /\ ReplySourceServiceRank(
       owner, semantic, source,
       messageCursor, chunkCursor, distance)
  /\ <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
  => \/ ReplySourceAdvancedFrom(
          owner, semantic, source, messageCursor, chunkCursor)'
     \/ ReplySourceLowerServiceRank(
          owner, semantic, source,
          messageCursor, chunkCursor, distance)'

THEOREM ReplyServiceRankOutcomeObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    ReplyServiceRankOutcomeObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, distance)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier,
                ReplyRouteInductiveInvariant,
                ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance),
                <<ServiceReplyRoute(
                    owner, semantic)>>_ReplyRouteVars
         PROVE \/ ReplySourceAdvancedFrom(
                     owner, semantic, source,
                     messageCursor, chunkCursor)'
               \/ ReplySourceLowerServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)'
    <2>1. ServiceReplyRoute(owner, semantic)
      BY <1>1, PTL
    <2> QED BY <1>1, <2>1,
         ReplyServiceLowersRankOrAdvancesTarget
  <1> QED BY <1>1
       DEF ReplyServiceRankOutcomeObligation

ReplyServiceRankLiveSuffixObligation(
    owner, semantic, source,
    messageCursor, chunkCursor, distance) ==
  (/\ []ReplyRouteInductiveInvariant
   /\ [][ReplyRouteNext]_ReplyRouteVars
   /\ WF_ReplyRouteVars(ServiceReplyRoute(owner, semantic))
   /\ []ReplySourceRouteStable(owner, semantic, source))
  => (ReplySourceServiceRank(
        owner, semantic, source,
        messageCursor, chunkCursor, distance)
        ~> (\/ ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
             \/ ReplySourceLowerServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance)))

THEOREM ReplyServiceRankLiveSuffixObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distance \in ReplyDistanceCarrier:
    ReplyServiceRankLiveSuffixObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, distance)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW distance \in ReplyDistanceCarrier
         PROVE ReplyServiceRankLiveSuffixObligation(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)
    <2>1. [][ReplyServiceRankPersistenceObligationsHold]_ReplyRouteVars
      BY ReplyServiceRankPersistenceObligationsHold, PTL
    <2>2. [][ReplyServiceRankPersistenceObligation(
                owner, semantic, source,
                messageCursor, chunkCursor, distance)]_ReplyRouteVars
      BY <2>1, IsaM("blast")
    <2>3. []ReplyServiceRankEnablementObligationsHold
      BY ReplyServiceRankEnablementObligationsHold, PTL
    <2>4. []ReplyServiceRankEnablementObligation(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
      BY <2>3, IsaM("blast")
    <2>5. [][ReplyServiceRankOutcomeObligationsHold]_ReplyRouteVars
      BY ReplyServiceRankOutcomeObligationsHold, PTL
    <2>6. [][ReplyServiceRankOutcomeObligation(
                owner, semantic, source,
                messageCursor, chunkCursor, distance)]_ReplyRouteVars
      BY <2>5, IsaM("blast")
    <2>7. ASSUME []ReplyRouteInductiveInvariant,
                  [][ReplyRouteNext]_ReplyRouteVars,
                  WF_ReplyRouteVars(
                    ServiceReplyRoute(owner, semantic)),
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
      <3>1. []ReplyRouteConfiguration
        BY <2>7, PTL DEF ReplyRouteInductiveInvariant
      <3>2. [][ReplySourceServiceRank(
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
                          messageCursor, chunkCursor)']_ReplyRouteVars
        BY <2>2, <2>7, PTL
           DEF ReplyServiceRankPersistenceObligation
      <3>3. [](ReplySourceServiceRank(
                    owner, semantic, source,
                    messageCursor, chunkCursor, distance)
                  => ENABLED
                       <<ServiceReplyRoute(
                           owner, semantic)>>_ReplyRouteVars)
        BY <2>4, <2>7, <3>1, PTL
           DEF ReplyRouteInductiveInvariant,
               ReplyServiceRankEnablementObligation
      <3>4. [][/\ ReplySourceServiceRank(
                          owner, semantic, source,
                          messageCursor, chunkCursor, distance)
                     /\ <<ServiceReplyRoute(
                            owner, semantic)>>_ReplyRouteVars
                    => \/ ReplySourceAdvancedFrom(
                            owner, semantic, source,
                            messageCursor, chunkCursor)'
                       \/ ReplySourceLowerServiceRank(
                            owner, semantic, source,
                            messageCursor, chunkCursor, distance)']_ReplyRouteVars
        BY <2>6, <2>7, PTL
           DEF ReplyServiceRankOutcomeObligation
      <3> QED BY <2>7, <3>2, <3>3, <3>4, PTL
    <2> QED BY <2>7
         DEF ReplyServiceRankLiveSuffixObligation
  <1> QED BY <1>1

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
      BY <1>1, ReplyRouteSpecAlwaysInductiveInvariant
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
      <3>1. ReplyRouteConfiguration
        BY <2>1, PTL DEF ReplyRouteInductiveInvariant
      <3> QED BY <3>1, ReplyReadyCursorEnablesService
           DEF ReplySourceServiceRank
    <2>6. <<ServiceReplyRoute(owner, semantic)>>_ReplyRouteVars
             => ServiceReplyRoute(owner, semantic)
      BY PTL
    <2>7. /\ ReplyRouteInductiveInvariant
             /\ ReplySourceServiceRank(
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
      BY <1>1, <2>6, ReplyServiceLowersRankOrAdvancesTarget
    <2>8. [][ReplyServiceRankPersistenceObligationsHold]_ReplyRouteVars
      BY ReplyServiceRankPersistenceObligationsHold, PTL
    <2>9. [][ReplyServiceRankPersistenceObligation(
                owner, semantic, source,
                messageCursor, chunkCursor, distance)]_ReplyRouteVars
      BY <2>8, IsaM("blast")
    <2>10. [][ReplySourceServiceRank(
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
                         messageCursor, chunkCursor)']_ReplyRouteVars
      BY <1>1, <2>1, <2>2, <2>9, PTL
         DEF ReplyServiceRankPersistenceObligation
    <2>11. []ReplyServiceRankEnablementObligationsHold
      BY ReplyServiceRankEnablementObligationsHold, PTL
    <2>12. []ReplyServiceRankEnablementObligation(
                owner, semantic, source,
                messageCursor, chunkCursor, distance)
      BY <2>11, IsaM("blast")
    <2>13. [](ReplySourceServiceRank(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)
                => ENABLED
                     <<ServiceReplyRoute(
                         owner, semantic)>>_ReplyRouteVars)
      BY <2>1, <2>12, PTL
         DEF ReplyRouteInductiveInvariant,
             ReplyServiceRankEnablementObligation
    <2>14. [][ReplyServiceRankOutcomeObligationsHold]_ReplyRouteVars
      BY ReplyServiceRankOutcomeObligationsHold, PTL
    <2>15. [][ReplyServiceRankOutcomeObligation(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)]_ReplyRouteVars
      BY <2>14, IsaM("blast")
    <2>16. [][/\ ReplySourceServiceRank(
                        owner, semantic, source,
                        messageCursor, chunkCursor, distance)
                   /\ <<ServiceReplyRoute(
                          owner, semantic)>>_ReplyRouteVars
                  => \/ ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor)'
                     \/ ReplySourceLowerServiceRank(
                          owner, semantic, source,
                          messageCursor, chunkCursor, distance)']_ReplyRouteVars
      BY <2>1, <2>15, PTL
         DEF ReplyServiceRankOutcomeObligation
    <2> QED BY <2>2, <2>3, <2>10, <2>13, <2>16, PTL
  <1> QED BY <1>1

ReplyServiceRankExistentialEquivalenceObligation(
    owner, semantic, source, messageCursor, chunkCursor, distances) ==
  (\A distance \in distances:
     ReplySourceServiceRank(
       owner, semantic, source,
       messageCursor, chunkCursor, distance)
       => <>ReplySourceAdvancedFrom(
            owner, semantic, source,
            messageCursor, chunkCursor))
    <=> ((\E distance \in distances:
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, distance))
          => <>ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor))

THEOREM ReplyServiceRankExistentialEquivalenceObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     distances \in SUBSET ReplyDistanceCarrier:
    ReplyServiceRankExistentialEquivalenceObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, distances)
BY IsaM("blast")
   DEF ReplyServiceRankExistentialEquivalenceObligation

THEOREM ReplyServiceRankCarrierExistentialEquivalenceObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyServiceRankExistentialEquivalenceObligation(
      owner, semantic, source,
      messageCursor, chunkCursor, ReplyDistanceCarrier)
BY IsaM("blast")
   DEF ReplyServiceRankExistentialEquivalenceObligation

THEOREM ReplyServiceRankLowerExistentialEquivalenceObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     current \in ReplyDistanceCarrier:
    ReplyServiceRankExistentialEquivalenceObligation(
      owner, semantic, source, messageCursor, chunkCursor,
      SetLessThan(
        current, ReplyDistanceOrdering, ReplyDistanceCarrier))
BY IsaM("blast")
   DEF ReplyServiceRankExistentialEquivalenceObligation

ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
    owner, semantic, source, messageCursor, chunkCursor, distances) ==
  (\A lower \in distances:
     ReplySourceServiceRank(
       owner, semantic, source,
       messageCursor, chunkCursor, lower)
       => <>ReplySourceAdvancedFrom(
            owner, semantic, source,
            messageCursor, chunkCursor))
    <=> ((\E lower \in distances:
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, lower))
          => <>ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor))

THEOREM ReplyServiceRankLowerNamedExistentialEquivalenceObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     current \in ReplyDistanceCarrier:
    ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
      owner, semantic, source, messageCursor, chunkCursor,
      SetLessThan(
        current, ReplyDistanceOrdering, ReplyDistanceCarrier))
BY IsaM("blast")
   DEF ReplyServiceRankLowerNamedExistentialEquivalenceObligation

THEOREM ReplyServiceRankExistentialLift ==
  ASSUME NEW owner \in ReplyOwners,
         NEW semantic \in ReplySemantics,
         NEW source \in ReplySources,
         NEW messageCursor \in 0..ReplyMessageCount,
         NEW chunkCursor \in 0..ReplyChunkCount,
         \A distance \in ReplyDistanceCarrier:
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, distance)
             ~> ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
  PROVE (\E distance \in ReplyDistanceCarrier:
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, distance))
          ~> ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor)
PROOF
  <1>1. (\A distance \in ReplyDistanceCarrier:
            [](ReplySourceServiceRank(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)
                 => <>ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor)))
          <=> [](\A distance \in ReplyDistanceCarrier:
                   ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, distance)
                     => <>ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor))
    OBVIOUS
  <1>2. [](\A distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
               => <>ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor))
          <=> []((\E distance \in ReplyDistanceCarrier:
                    ReplySourceServiceRank(
                      owner, semantic, source,
                      messageCursor, chunkCursor, distance))
                   => <>ReplySourceAdvancedFrom(
                        owner, semantic, source,
                        messageCursor, chunkCursor))
    <2>1. []ReplyServiceRankCarrierExistentialEquivalenceObligationsHold
      BY ReplyServiceRankCarrierExistentialEquivalenceObligationsHold,
         PTL
    <2>2. [](\A allSemantic \in ReplySemantics,
                    allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplyServiceRankExistentialEquivalenceObligation(
                    owner, allSemantic, allSource,
                    allMessageCursor, allChunkCursor,
                    ReplyDistanceCarrier))
      BY <2>1, IsaM("blast")
    <2>3. [](\A allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplyServiceRankExistentialEquivalenceObligation(
                    owner, semantic, allSource,
                    allMessageCursor, allChunkCursor,
                    ReplyDistanceCarrier))
      BY <2>2, IsaM("blast")
    <2>4. [](\A allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplyServiceRankExistentialEquivalenceObligation(
                    owner, semantic, source,
                    allMessageCursor, allChunkCursor,
                    ReplyDistanceCarrier))
      BY <2>3, IsaM("blast")
    <2>5. [](\A allChunkCursor \in 0..ReplyChunkCount:
                  ReplyServiceRankExistentialEquivalenceObligation(
                    owner, semantic, source,
                    messageCursor, allChunkCursor,
                    ReplyDistanceCarrier))
      BY <2>4, IsaM("blast")
    <2>6. []ReplyServiceRankExistentialEquivalenceObligation(
               owner, semantic, source,
               messageCursor, chunkCursor, ReplyDistanceCarrier)
      BY <2>5, IsaM("blast")
    <2> QED BY <2>6, PTL
         DEF ReplyServiceRankExistentialEquivalenceObligation
  <1> QED BY <1>1, <1>2, PTL

THEOREM ReplyServiceRankLowerSetExistentialLift ==
  ASSUME NEW owner \in ReplyOwners,
         NEW semantic \in ReplySemantics,
         NEW source \in ReplySources,
         NEW messageCursor \in 0..ReplyMessageCount,
         NEW chunkCursor \in 0..ReplyChunkCount,
         NEW current \in ReplyDistanceCarrier,
         \A lower \in SetLessThan(
              current, ReplyDistanceOrdering, ReplyDistanceCarrier):
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, lower)
             ~> ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
  PROVE (\E lower \in SetLessThan(
                         current, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
           ReplySourceServiceRank(
             owner, semantic, source,
             messageCursor, chunkCursor, lower))
          ~> ReplySourceAdvancedFrom(
               owner, semantic, source,
               messageCursor, chunkCursor)
PROOF
  <1>1. (\A lower \in SetLessThan(
                         current, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
            [](ReplySourceServiceRank(
                 owner, semantic, source,
                 messageCursor, chunkCursor, lower)
                 => <>ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor)))
          <=> [](\A lower \in SetLessThan(
                                  current, ReplyDistanceOrdering,
                                  ReplyDistanceCarrier):
                   ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, lower)
                     => <>ReplySourceAdvancedFrom(
                          owner, semantic, source,
                          messageCursor, chunkCursor))
    OBVIOUS
  <1>2. [](\A lower \in SetLessThan(
                            current, ReplyDistanceOrdering,
                            ReplyDistanceCarrier):
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, lower)
               => <>ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor))
          <=> []((\E lower \in SetLessThan(
                                  current, ReplyDistanceOrdering,
                                  ReplyDistanceCarrier):
                    ReplySourceServiceRank(
                      owner, semantic, source,
                      messageCursor, chunkCursor, lower))
                   => <>ReplySourceAdvancedFrom(
                        owner, semantic, source,
                        messageCursor, chunkCursor))
    <2>1. []ReplyServiceRankLowerNamedExistentialEquivalenceObligationsHold
      BY ReplyServiceRankLowerNamedExistentialEquivalenceObligationsHold,
         PTL
    <2>2. [](\A allSemantic \in ReplySemantics,
                    allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount,
                    allCurrent \in ReplyDistanceCarrier:
                  ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
                    owner, allSemantic, allSource,
                    allMessageCursor, allChunkCursor,
                    SetLessThan(
                      allCurrent, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)))
      BY <2>1, IsaM("blast")
    <2>3. [](\A allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount,
                    allCurrent \in ReplyDistanceCarrier:
                  ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
                    owner, semantic, allSource,
                    allMessageCursor, allChunkCursor,
                    SetLessThan(
                      allCurrent, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)))
      BY <2>2, IsaM("blast")
    <2>4. [](\A allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount,
                    allCurrent \in ReplyDistanceCarrier:
                  ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
                    owner, semantic, source,
                    allMessageCursor, allChunkCursor,
                    SetLessThan(
                      allCurrent, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)))
      BY <2>3, IsaM("blast")
    <2>5. [](\A allChunkCursor \in 0..ReplyChunkCount,
                    allCurrent \in ReplyDistanceCarrier:
                  ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
                    owner, semantic, source,
                    messageCursor, allChunkCursor,
                    SetLessThan(
                      allCurrent, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)))
      BY <2>4, IsaM("blast")
    <2>6. [](\A allCurrent \in ReplyDistanceCarrier:
                  ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
                    owner, semantic, source, messageCursor, chunkCursor,
                    SetLessThan(
                      allCurrent, ReplyDistanceOrdering,
                      ReplyDistanceCarrier)))
      BY <2>5, IsaM("blast")
    <2>7. []ReplyServiceRankLowerNamedExistentialEquivalenceObligation(
               owner, semantic, source, messageCursor, chunkCursor,
               SetLessThan(
                 current, ReplyDistanceOrdering, ReplyDistanceCarrier))
      BY <2>6, IsaM("blast")
    <2> QED BY <2>7, PTL
         DEF ReplyServiceRankLowerNamedExistentialEquivalenceObligation
  <1> QED BY <1>1, <1>2, PTL

ReplyServiceRankProgress(
    owner, semantic, source, messageCursor, chunkCursor, distance) ==
  [](ReplySourceServiceRank(
       owner, semantic, source,
       messageCursor, chunkCursor, distance)
     => <>ReplySourceAdvancedFrom(
          owner, semantic, source,
          messageCursor, chunkCursor))

ReplyServiceRankLowerSetAllProgress(
    owner, semantic, source, messageCursor, chunkCursor, current) ==
  \A lower \in SetLessThan(
       current, ReplyDistanceOrdering, ReplyDistanceCarrier):
    ReplyServiceRankProgress(
      owner, semantic, source,
      messageCursor, chunkCursor, lower)

ReplyServiceRankLowerSetSomeProgress(
    owner, semantic, source, messageCursor, chunkCursor, current) ==
  (\E lower \in SetLessThan(
                current, ReplyDistanceOrdering,
                ReplyDistanceCarrier):
     ReplySourceServiceRank(
       owner, semantic, source,
       messageCursor, chunkCursor, lower))
    ~> ReplySourceAdvancedFrom(
         owner, semantic, source,
         messageCursor, chunkCursor)

ReplyServiceRankLowerSetExistentialLiftObligation(
    owner, semantic, source, messageCursor, chunkCursor, current) ==
  ReplyServiceRankLowerSetAllProgress(
    owner, semantic, source,
    messageCursor, chunkCursor, current)
    => ReplyServiceRankLowerSetSomeProgress(
         owner, semantic, source,
         messageCursor, chunkCursor, current)

THEOREM ReplyServiceRankLowerSetExistentialLiftsHold ==
  ASSUME NEW owner \in ReplyOwners,
         NEW semantic \in ReplySemantics,
         NEW source \in ReplySources,
         NEW messageCursor \in 0..ReplyMessageCount,
         NEW chunkCursor \in 0..ReplyChunkCount,
         NEW current \in ReplyDistanceCarrier
  PROVE ReplyServiceRankLowerSetExistentialLiftObligation(
          owner, semantic, source,
          messageCursor, chunkCursor, current)
PROOF
  <1>1. ASSUME ReplyServiceRankLowerSetAllProgress(
                  owner, semantic, source,
                  messageCursor, chunkCursor, current)
         PROVE ReplyServiceRankLowerSetSomeProgress(
                 owner, semantic, source,
                 messageCursor, chunkCursor, current)
    <2>1. \A lower \in SetLessThan(
                         current, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, lower)
               ~> ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
      BY <1>1, PTL
         DEF ReplyServiceRankLowerSetAllProgress,
             ReplyServiceRankProgress
    <2> QED BY <2>1, ReplyServiceRankLowerSetExistentialLift
         DEF ReplyServiceRankLowerSetSomeProgress
  <1> QED BY <1>1
       DEF ReplyServiceRankLowerSetExistentialLiftObligation

THEOREM ReplyServiceRankComposition ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount,
     current \in ReplyDistanceCarrier:
    /\ ReplySourceServiceRank(
         owner, semantic, source,
         messageCursor, chunkCursor, current)
         ~> (\/ ReplySourceAdvancedFrom(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
              \/ \E lower \in SetLessThan(
                   current, ReplyDistanceOrdering,
                   ReplyDistanceCarrier):
                   ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, lower))
    /\ ReplyServiceRankLowerSetAllProgress(
         owner, semantic, source,
         messageCursor, chunkCursor, current)
    => ReplyServiceRankProgress(
         owner, semantic, source,
         messageCursor, chunkCursor, current)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount,
                NEW current \in ReplyDistanceCarrier
         PROVE
           /\ ReplySourceServiceRank(
                owner, semantic, source,
                messageCursor, chunkCursor, current)
                ~> (\/ ReplySourceAdvancedFrom(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                     \/ \E lower \in SetLessThan(
                          current, ReplyDistanceOrdering,
                          ReplyDistanceCarrier):
                          ReplySourceServiceRank(
                            owner, semantic, source,
                            messageCursor, chunkCursor, lower))
           /\ ReplyServiceRankLowerSetAllProgress(
                owner, semantic, source,
                messageCursor, chunkCursor, current)
           => ReplyServiceRankProgress(
                owner, semantic, source,
                messageCursor, chunkCursor, current)
    <2>1. (\A lower \in SetLessThan(
                         current, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
              ReplyServiceRankProgress(
                owner, semantic, source,
                messageCursor, chunkCursor, lower))
            <=> [](\A lower \in SetLessThan(
                               current, ReplyDistanceOrdering,
                               ReplyDistanceCarrier):
                     ReplySourceServiceRank(
                       owner, semantic, source,
                       messageCursor, chunkCursor, lower)
                       => <>ReplySourceAdvancedFrom(
                            owner, semantic, source,
                            messageCursor, chunkCursor))
      BY DEF ReplyServiceRankProgress
    <2>2. []ReplyServiceRankExistentialEquivalenceObligation(
               owner, semantic, source, messageCursor, chunkCursor,
               SetLessThan(
                 current, ReplyDistanceOrdering, ReplyDistanceCarrier))
      <3>1. []ReplyServiceRankLowerExistentialEquivalenceObligationsHold
        BY ReplyServiceRankLowerExistentialEquivalenceObligationsHold,
           PTL
      <3>2. [](\A allSemantic \in ReplySemantics,
                      allSource \in ReplySources,
                      allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount,
                      allCurrent \in ReplyDistanceCarrier:
                    ReplyServiceRankExistentialEquivalenceObligation(
                      owner, allSemantic, allSource,
                      allMessageCursor, allChunkCursor,
                      SetLessThan(
                        allCurrent, ReplyDistanceOrdering,
                        ReplyDistanceCarrier)))
        BY <3>1, IsaM("blast")
      <3>3. [](\A allSource \in ReplySources,
                      allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount,
                      allCurrent \in ReplyDistanceCarrier:
                    ReplyServiceRankExistentialEquivalenceObligation(
                      owner, semantic, allSource,
                      allMessageCursor, allChunkCursor,
                      SetLessThan(
                        allCurrent, ReplyDistanceOrdering,
                        ReplyDistanceCarrier)))
        BY <3>2, IsaM("blast")
      <3>4. [](\A allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount,
                      allCurrent \in ReplyDistanceCarrier:
                    ReplyServiceRankExistentialEquivalenceObligation(
                      owner, semantic, source,
                      allMessageCursor, allChunkCursor,
                      SetLessThan(
                        allCurrent, ReplyDistanceOrdering,
                        ReplyDistanceCarrier)))
        BY <3>3, IsaM("blast")
      <3>5. [](\A allChunkCursor \in 0..ReplyChunkCount,
                      allCurrent \in ReplyDistanceCarrier:
                    ReplyServiceRankExistentialEquivalenceObligation(
                      owner, semantic, source,
                      messageCursor, allChunkCursor,
                      SetLessThan(
                        allCurrent, ReplyDistanceOrdering,
                        ReplyDistanceCarrier)))
        BY <3>4, IsaM("blast")
      <3>6. [](\A allCurrent \in ReplyDistanceCarrier:
                    ReplyServiceRankExistentialEquivalenceObligation(
                      owner, semantic, source, messageCursor, chunkCursor,
                      SetLessThan(
                        allCurrent, ReplyDistanceOrdering,
                        ReplyDistanceCarrier)))
        BY <3>5, IsaM("blast")
      <3>7. []ReplyServiceRankExistentialEquivalenceObligation(
                 owner, semantic, source, messageCursor, chunkCursor,
                 SetLessThan(
                   current, ReplyDistanceOrdering, ReplyDistanceCarrier))
        BY <3>6, IsaM("blast")
      <3> QED BY <3>7
    <2>3. ASSUME
             /\ ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, current)
                  ~> (\/ ReplySourceAdvancedFrom(
                           owner, semantic, source,
                           messageCursor, chunkCursor)
                       \/ \E lower \in SetLessThan(
                            current, ReplyDistanceOrdering,
                            ReplyDistanceCarrier):
                            ReplySourceServiceRank(
                              owner, semantic, source,
                              messageCursor, chunkCursor, lower))
             /\ ReplyServiceRankLowerSetAllProgress(
                  owner, semantic, source,
                  messageCursor, chunkCursor, current)
            PROVE ReplyServiceRankProgress(
                    owner, semantic, source,
                    messageCursor, chunkCursor, current)
      <3> DEFINE LowerServiceRankExists ==
                     \E lower \in SetLessThan(
                          current, ReplyDistanceOrdering,
                          ReplyDistanceCarrier):
                       ReplySourceServiceRank(
                         owner, semantic, source,
                         messageCursor, chunkCursor, lower)
                 RankAdvanced ==
                   ReplySourceAdvancedFrom(
                     owner, semantic, source,
                     messageCursor, chunkCursor)
      <3>1. ReplyServiceRankLowerSetSomeProgress(
               owner, semantic, source,
               messageCursor, chunkCursor, current)
        BY <2>3, ReplyServiceRankLowerSetExistentialLiftsHold
           DEF ReplyServiceRankLowerSetExistentialLiftObligation
      <3>2. LowerServiceRankExists ~> RankAdvanced
        BY <3>1, PTL
           DEF ReplyServiceRankLowerSetSomeProgress,
               LowerServiceRankExists, RankAdvanced
      <3>3. (RankAdvanced \/ LowerServiceRankExists)
               ~> RankAdvanced
        BY <3>2, PTL
      <3> QED BY <2>3, <3>3, PTL
           DEF ReplyServiceRankProgress,
               LowerServiceRankExists, RankAdvanced
    <2> QED BY <2>3
  <1> QED BY <1>1

THEOREM ReplyServiceRankWellFoundedLeadsTo ==
  ASSUME NEW owner \in ReplyOwners,
         NEW semantic \in ReplySemantics,
         NEW source \in ReplySources,
         NEW messageCursor \in 0..ReplyMessageCount,
         NEW chunkCursor \in 0..ReplyChunkCount,
         \A distance \in ReplyDistanceCarrier:
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
  PROVE \A distance \in ReplyDistanceCarrier:
          ReplySourceServiceRank(
            owner, semantic, source,
            messageCursor, chunkCursor, distance)
            ~> ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
PROOF
  <1> DEFINE RankProgress(distance) ==
                 ReplyServiceRankProgress(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)
             RankStep(distance) ==
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
  <1>1. ASSUME NEW current \in ReplyDistanceCarrier
         PROVE (\A distance \in ReplyDistanceCarrier:
                  RankStep(distance))
                 => ((\A lower \in SetLessThan(
                            current, ReplyDistanceOrdering,
                            ReplyDistanceCarrier):
                        RankProgress(lower))
                       => RankProgress(current))
    <2>1. ASSUME \A distance \in ReplyDistanceCarrier:
                    RankStep(distance)
            PROVE (\A lower \in SetLessThan(
                              current, ReplyDistanceOrdering,
                              ReplyDistanceCarrier):
                     RankProgress(lower))
                    => RankProgress(current)
      <3>1. ASSUME \A lower \in SetLessThan(
                             current, ReplyDistanceOrdering,
                             ReplyDistanceCarrier):
                      RankProgress(lower)
             PROVE RankProgress(current)
        <4>0. ReplyServiceRankLowerSetAllProgress(
                 owner, semantic, source,
                 messageCursor, chunkCursor, current)
          BY <3>1
             DEF RankProgress,
                 ReplyServiceRankLowerSetAllProgress
        <4>1. RankStep(current)
          <5> HIDE DEF RankStep
          <5> QED BY <2>1, IsaM("blast")
        <4> QED BY ReplyServiceRankComposition,
                     <4>1, <4>0, IsaM("blast")
             DEF RankProgress, RankStep
      <3> QED BY <3>1
    <2> QED BY <2>1
  <1>2. QED
    <2> HIDE DEF RankProgress
    <2>1. (\A distance \in ReplyDistanceCarrier:
             RankStep(distance))
            => \A distance \in ReplyDistanceCarrier:
                 RankProgress(distance)
      BY <1>1, ReplyDistanceOrderingWellFounded,
         WFInduction, IsaM("blast")
    <2>2. \A distance \in ReplyDistanceCarrier:
             RankProgress(distance)
      BY <2>1, IsaM("blast")
         DEF RankStep
    <2>3. ASSUME NEW distance \in ReplyDistanceCarrier
           PROVE ReplySourceServiceRank(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)
                   ~> ReplySourceAdvancedFrom(
                        owner, semantic, source,
                        messageCursor, chunkCursor)
      <3>1. ReplyServiceRankProgress(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
        BY <2>2, IsaM("blast")
           DEF RankProgress
      <3> QED BY <3>1, PTL
           DEF ReplyServiceRankProgress
    <2> QED BY <2>3

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
    <2> DEFINE ServiceRankAt(distance) ==
                   ReplySourceServiceRank(
                     owner, semantic, source,
                     messageCursor, chunkCursor, distance)
               TargetAdvanced ==
                   ReplySourceAdvancedFrom(
                     owner, semantic, source,
                     messageCursor, chunkCursor)
    <2>1. []ReplyRouteInductiveInvariant
      BY <1>1, ReplyRouteSpecAlwaysInductiveInvariant
    <2>2. \A distance \in ReplyDistanceCarrier:
             ServiceRankAt(distance)
               ~> (\/ TargetAdvanced
                    \/ \E lower \in SetLessThan(
                         distance, ReplyDistanceOrdering,
                         ReplyDistanceCarrier):
                         ServiceRankAt(lower))
      BY <1>1, ReplyServiceRankLeadsToLowerOrAdvance
         DEF ReplySourceLowerServiceRank,
             ServiceRankAt, TargetAdvanced
    <2>3. \A distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
               ~> ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
      BY <2>2, ReplyServiceRankWellFoundedLeadsTo
         DEF ServiceRankAt, TargetAdvanced
    <2>4. []ReplyReadyCursorHasServiceRank
      BY ReplyReadyCursorHasServiceRank, PTL
    <2>5. [](\A allOwner \in ReplyOwners,
                    allSemantic \in ReplySemantics,
                    allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplySourceReadyAtCursor(
                    allOwner, allSemantic, allSource,
                    allMessageCursor, allChunkCursor)
                  => \E distance \in ReplyDistanceCarrier:
                       ReplySourceServiceRank(
                         allOwner, allSemantic, allSource,
                         allMessageCursor, allChunkCursor, distance))
      BY <2>1, <2>4, PTL
    <2>6. [](\A allSemantic \in ReplySemantics,
                    allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplySourceReadyAtCursor(
                    owner, allSemantic, allSource,
                    allMessageCursor, allChunkCursor)
                  => \E distance \in ReplyDistanceCarrier:
                       ReplySourceServiceRank(
                         owner, allSemantic, allSource,
                         allMessageCursor, allChunkCursor, distance))
      BY <2>5, IsaM("blast")
    <2>7. [](\A allSource \in ReplySources,
                    allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplySourceReadyAtCursor(
                    owner, semantic, allSource,
                    allMessageCursor, allChunkCursor)
                  => \E distance \in ReplyDistanceCarrier:
                       ReplySourceServiceRank(
                         owner, semantic, allSource,
                         allMessageCursor, allChunkCursor, distance))
      BY <2>6, IsaM("blast")
    <2>8. [](\A allMessageCursor \in 0..ReplyMessageCount,
                    allChunkCursor \in 0..ReplyChunkCount:
                  ReplySourceReadyAtCursor(
                    owner, semantic, source,
                    allMessageCursor, allChunkCursor)
                  => \E distance \in ReplyDistanceCarrier:
                       ReplySourceServiceRank(
                         owner, semantic, source,
                         allMessageCursor, allChunkCursor, distance))
      BY <2>7, IsaM("blast")
    <2>9. [](\A allChunkCursor \in 0..ReplyChunkCount:
                  ReplySourceReadyAtCursor(
                    owner, semantic, source,
                    messageCursor, allChunkCursor)
                  => \E distance \in ReplyDistanceCarrier:
                       ReplySourceServiceRank(
                         owner, semantic, source,
                         messageCursor, allChunkCursor, distance))
      BY <2>8, IsaM("blast")
    <2>10. [](ReplySourceReadyAtCursor(
                owner, semantic, source, messageCursor, chunkCursor)
               => \E distance \in ReplyDistanceCarrier:
                    ReplySourceServiceRank(
                      owner, semantic, source,
                      messageCursor, chunkCursor, distance))
      BY <2>9, IsaM("blast")
    <2>11. ReplySourceReadyAtCursor(
              owner, semantic, source, messageCursor, chunkCursor)
              ~> (\E distance \in ReplyDistanceCarrier:
                    ReplySourceServiceRank(
                      owner, semantic, source,
                      messageCursor, chunkCursor, distance))
      BY <2>10, PTL
    <2>12. (\E distance \in ReplyDistanceCarrier:
             ReplySourceServiceRank(
               owner, semantic, source,
               messageCursor, chunkCursor, distance))
            ~> ReplySourceAdvancedFrom(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
      BY <2>3, ReplyServiceRankExistentialLift
    <2> QED BY <2>11, <2>12, PTL
  <1> QED BY <1>1

ReplyReadyCursorLiveSuffixObligation(
    owner, semantic, source, messageCursor, chunkCursor) ==
  (/\ []ReplyRouteInductiveInvariant
   /\ [][ReplyRouteNext]_ReplyRouteVars
   /\ WF_ReplyRouteVars(ServiceReplyRoute(owner, semantic))
   /\ []ReplySourceRouteStable(owner, semantic, source))
  => (ReplySourceReadyAtCursor(
        owner, semantic, source, messageCursor, chunkCursor)
        ~> ReplySourceAdvancedFrom(
             owner, semantic, source, messageCursor, chunkCursor))

THEOREM ReplyReadyCursorLiveSuffixObligationsHold ==
  \A owner \in ReplyOwners, semantic \in ReplySemantics,
     source \in ReplySources,
     messageCursor \in 0..ReplyMessageCount,
     chunkCursor \in 0..ReplyChunkCount:
    ReplyReadyCursorLiveSuffixObligation(
      owner, semantic, source, messageCursor, chunkCursor)
PROOF
  <1>1. ASSUME NEW owner \in ReplyOwners,
                NEW semantic \in ReplySemantics,
                NEW source \in ReplySources,
                NEW messageCursor \in 0..ReplyMessageCount,
                NEW chunkCursor \in 0..ReplyChunkCount
         PROVE ReplyReadyCursorLiveSuffixObligation(
                 owner, semantic, source,
                 messageCursor, chunkCursor)
    <2>1. \A distance \in ReplyDistanceCarrier:
             ReplyServiceRankLiveSuffixObligation(
               owner, semantic, source,
               messageCursor, chunkCursor, distance)
      BY ReplyServiceRankLiveSuffixObligationsHold,
         IsaM("blast")
    <2>2. ASSUME []ReplyRouteInductiveInvariant,
                  [][ReplyRouteNext]_ReplyRouteVars,
                  WF_ReplyRouteVars(
                    ServiceReplyRoute(owner, semantic)),
                  []ReplySourceRouteStable(owner, semantic, source)
           PROVE ReplySourceReadyAtCursor(
                   owner, semantic, source,
                   messageCursor, chunkCursor)
                 ~> ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
      <3>1. \A distance \in ReplyDistanceCarrier:
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
        <4>1. ASSUME NEW distance \in ReplyDistanceCarrier
               PROVE ReplySourceServiceRank(
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
          <5>1. ReplyServiceRankLiveSuffixObligation(
                   owner, semantic, source,
                   messageCursor, chunkCursor, distance)
            BY <2>1, <4>1, IsaM("blast")
          <5> QED BY <2>2, <5>1, IsaM("blast")
               DEF ReplyServiceRankLiveSuffixObligation,
                   ReplySourceLowerServiceRank
        <4> QED BY <4>1
      <3>2. \A distance \in ReplyDistanceCarrier:
               ReplySourceServiceRank(
                 owner, semantic, source,
                 messageCursor, chunkCursor, distance)
                 ~> ReplySourceAdvancedFrom(
                      owner, semantic, source,
                      messageCursor, chunkCursor)
        BY <3>1, ReplyServiceRankWellFoundedLeadsTo
      <3>3. []ReplyReadyCursorHasServiceRank
        BY ReplyReadyCursorHasServiceRank, PTL
      <3>4. [](\A allOwner \in ReplyOwners,
                      allSemantic \in ReplySemantics,
                      allSource \in ReplySources,
                      allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount:
                    ReplySourceReadyAtCursor(
                      allOwner, allSemantic, allSource,
                      allMessageCursor, allChunkCursor)
                    => \E distance \in ReplyDistanceCarrier:
                         ReplySourceServiceRank(
                           allOwner, allSemantic, allSource,
                           allMessageCursor, allChunkCursor, distance))
        BY <2>2, <3>3, PTL
      <3>5. [](\A allSemantic \in ReplySemantics,
                      allSource \in ReplySources,
                      allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount:
                    ReplySourceReadyAtCursor(
                      owner, allSemantic, allSource,
                      allMessageCursor, allChunkCursor)
                    => \E distance \in ReplyDistanceCarrier:
                         ReplySourceServiceRank(
                           owner, allSemantic, allSource,
                           allMessageCursor, allChunkCursor, distance))
        BY <3>4, IsaM("blast")
      <3>6. [](\A allSource \in ReplySources,
                      allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount:
                    ReplySourceReadyAtCursor(
                      owner, semantic, allSource,
                      allMessageCursor, allChunkCursor)
                    => \E distance \in ReplyDistanceCarrier:
                         ReplySourceServiceRank(
                           owner, semantic, allSource,
                           allMessageCursor, allChunkCursor, distance))
        BY <3>5, IsaM("blast")
      <3>7. [](\A allMessageCursor \in 0..ReplyMessageCount,
                      allChunkCursor \in 0..ReplyChunkCount:
                    ReplySourceReadyAtCursor(
                      owner, semantic, source,
                      allMessageCursor, allChunkCursor)
                    => \E distance \in ReplyDistanceCarrier:
                         ReplySourceServiceRank(
                           owner, semantic, source,
                           allMessageCursor, allChunkCursor, distance))
        BY <3>6, IsaM("blast")
      <3>8. [](\A allChunkCursor \in 0..ReplyChunkCount:
                    ReplySourceReadyAtCursor(
                      owner, semantic, source,
                      messageCursor, allChunkCursor)
                    => \E distance \in ReplyDistanceCarrier:
                         ReplySourceServiceRank(
                           owner, semantic, source,
                           messageCursor, allChunkCursor, distance))
        BY <3>7, IsaM("blast")
      <3>9. [](ReplySourceReadyAtCursor(
                  owner, semantic, source,
                  messageCursor, chunkCursor)
                 => \E distance \in ReplyDistanceCarrier:
                      ReplySourceServiceRank(
                        owner, semantic, source,
                        messageCursor, chunkCursor, distance))
        BY <3>8, IsaM("blast")
      <3>10. ReplySourceReadyAtCursor(
                owner, semantic, source,
                messageCursor, chunkCursor)
                ~> (\E distance \in ReplyDistanceCarrier:
                      ReplySourceServiceRank(
                        owner, semantic, source,
                        messageCursor, chunkCursor, distance))
        BY <3>9, PTL
      <3>11. (\E distance \in ReplyDistanceCarrier:
                ReplySourceServiceRank(
                  owner, semantic, source,
                  messageCursor, chunkCursor, distance))
               ~> ReplySourceAdvancedFrom(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
        BY <3>2, ReplyServiceRankExistentialLift
      <3> QED BY <3>10, <3>11, PTL
    <2> QED BY <2>2
         DEF ReplyReadyCursorLiveSuffixObligation
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
          <5>1. []ReplyRouteInductiveInvariant
            BY <1>1, ReplyRouteSpecAlwaysInductiveInvariant
          <5>2. []([]ReplyRouteInductiveInvariant)
            BY <5>1, PTL
          <5>3. [][ReplyRouteNext]_ReplyRouteVars
            BY <1>1, PTL DEF ReplyRouteSpec
          <5>4. []([][ReplyRouteNext]_ReplyRouteVars)
            BY <5>3, PTL
          <5>5. WF_ReplyRouteVars(
                   AcquireReplyTicket(owner, semantic, source))
            BY <1>1 DEF ReplyRouteSpec, ReplyRouteFairness
          <5>6. [](WF_ReplyRouteVars(
                    AcquireReplyTicket(owner, semantic, source)))
            BY <5>5, PTL
          <5>7. WF_ReplyRouteVars(
                   ServiceReplyRoute(owner, semantic))
            BY <1>1 DEF ReplyRouteSpec, ReplyRouteFairness
          <5>8. [](WF_ReplyRouteVars(
                    ServiceReplyRoute(owner, semantic)))
            BY <5>7, PTL
          <5>9. []ReplyStableCursorLiveSuffixObligationsHold
            BY ReplyStableCursorLiveSuffixObligationsHold, PTL
          <5>10. []ReplyStableCursorLiveSuffixObligation(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
            BY <5>9, IsaM("blast")
          <5>11. []ReplyReadyCursorLiveSuffixObligationsHold
            BY ReplyReadyCursorLiveSuffixObligationsHold, PTL
          <5>12. []ReplyReadyCursorLiveSuffixObligation(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
            BY <5>11, IsaM("blast")
          <5>13. []([]ReplySourceRouteStable(
                       owner, semantic, source)
                    => ((/\ ReplySourceAtCursor(
                               owner, semantic, source,
                               messageCursor, chunkCursor)
                             /\ ReplySourceRouteStable(
                                  owner, semantic, source))
                          ~> ReplySourceReadyAtCursor(
                               owner, semantic, source,
                               messageCursor, chunkCursor)))
            BY <5>2, <5>4, <5>6, <5>10, PTL
               DEF ReplyStableCursorLiveSuffixObligation
          <5>14. []([]ReplySourceRouteStable(
                       owner, semantic, source)
                    => (ReplySourceReadyAtCursor(
                          owner, semantic, source,
                          messageCursor, chunkCursor)
                          ~> ReplySourceAdvancedFrom(
                               owner, semantic, source,
                               messageCursor, chunkCursor)))
            BY <5>2, <5>4, <5>8, <5>12, PTL
               DEF ReplyReadyCursorLiveSuffixObligation
          <5>15. [][(\/ ReplySourceAtCursor(
                             owner, semantic, source,
                             messageCursor, chunkCursor)
                          \/ ReplySourceAdvancedFrom(
                             owner, semantic, source,
                             messageCursor, chunkCursor))
                       => (\/ ReplySourceAtCursor(
                                owner, semantic, source,
                                messageCursor, chunkCursor)
                             \/ ReplySourceAdvancedFrom(
                                owner, semantic, source,
                                messageCursor, chunkCursor))']_ReplyRouteVars
            BY <1>1, <4>1, ReplyCursorOrAdvancedPersists
          <5>16. ReplySourceAtCursor(
                    owner, semantic, source,
                    messageCursor, chunkCursor)
                    ~> (\/ ReplySourceAdvancedFrom(
                             owner, semantic, source,
                             messageCursor, chunkCursor)
                         \/ /\ ReplySourceAtCursor(
                                  owner, semantic, source,
                                  messageCursor, chunkCursor)
                            /\ []ReplySourceRouteStable(
                                 owner, semantic, source))
            BY <3>1, <5>15, PTL
               DEF ReplySourceStableResponsive
          <5>17. (/\ ReplySourceAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                      /\ []ReplySourceRouteStable(
                           owner, semantic, source))
                    ~> (/\ ReplySourceReadyAtCursor(
                              owner, semantic, source,
                              messageCursor, chunkCursor)
                           /\ []ReplySourceRouteStable(
                                owner, semantic, source))
            BY <5>13, PTL
          <5>18. (/\ ReplySourceReadyAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                      /\ []ReplySourceRouteStable(
                           owner, semantic, source))
                    ~> ReplySourceAdvancedFrom(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
            BY <5>14, PTL
          <5>19. (/\ ReplySourceAtCursor(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
                      /\ []ReplySourceRouteStable(
                           owner, semantic, source))
                    ~> ReplySourceAdvancedFrom(
                         owner, semantic, source,
                         messageCursor, chunkCursor)
            BY <5>17, <5>18, PTL
          <5> QED BY <5>16, <5>19, PTL
        <4> QED BY <4>1
      <3> QED BY <3>1 DEF ReplySourceEventuallyProgresses
    <2> QED BY <2>1
  <1> QED BY <1>1

(***************************************************************************
Authenticated cumulative close acknowledgement.

The pending floor is fixed until the exact responder acknowledgement changes
the acknowledged floor.  Weak fairness for that exact acknowledgement then
terminates the requester-side retry work.  Retry generation remains separate
bookkeeping and never appears in a reply capability.
***************************************************************************)
ReplyCloseWorkAtFloor(requester, responder, closedThrough) ==
  /\ ReplyCloseWorkPending(requester, responder)
  /\ rrClosePendingThrough[requester][responder] = closedThrough
  /\ rrCloseSentThrough[requester][responder] = closedThrough

ReplyCloseAcknowledgementFieldDomain ==
  {"requester", "responder", "authenticatedResponder", "closedThrough",
   "bindingRequester", "bindingResponder", "bindingClosedThrough"}

ReplyRouteCapabilityAuthorityFields ==
  {"owner", "source", "target", "semantic", "deliveryOrdinal",
   "connectionTenure", "sourceCapacity", "ticketTenure",
   "ticketSemantic", "ticketTarget", "ticketMessageCursor",
   "ticketChunkCursor"}

THEOREM ReplyCloseAcknowledgementSetHasCanonicalDomain ==
  \A acknowledgement \in ReplyCloseAcknowledgementSet:
    DOMAIN acknowledgement = ReplyCloseAcknowledgementFieldDomain
BY Zenon
   DEF ReplyCloseAcknowledgementSet,
       ReplyCloseAcknowledgementFieldDomain

THEOREM ReplyCloseAcknowledgementCarriesNoRouteCapability ==
  \A acknowledgement \in ReplyCloseAcknowledgementSet:
    /\ DOMAIN acknowledgement = ReplyCloseAcknowledgementFieldDomain
    /\ DOMAIN acknowledgement \cap ReplyRouteCapabilityAuthorityFields = {}
PROOF
  <1>1. ASSUME NEW acknowledgement \in ReplyCloseAcknowledgementSet
         PROVE /\ DOMAIN acknowledgement =
                    ReplyCloseAcknowledgementFieldDomain
               /\ DOMAIN acknowledgement \cap
                    ReplyRouteCapabilityAuthorityFields = {}
    <2>1. DOMAIN acknowledgement =
             ReplyCloseAcknowledgementFieldDomain
      BY <1>1, ReplyCloseAcknowledgementSetHasCanonicalDomain
    <2>2. ReplyCloseAcknowledgementFieldDomain \cap
             ReplyRouteCapabilityAuthorityFields = {}
      BY SMT
         DEF ReplyCloseAcknowledgementFieldDomain,
             ReplyRouteCapabilityAuthorityFields
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyAcknowledgedCloseRetryIsIdempotent ==
  \A witness \in ReplyCloseWitnessSet:
    /\ RetryCloseSemanticRequest(witness)
    /\ ~ReplyCloseWorkPending(witness.requester, witness.responder)
    => UNCHANGED ReplyRouteVars
BY DEF RetryCloseSemanticRequest

THEOREM ReplyEqualCloseAcknowledgementIsIdempotent ==
  \A acknowledgement \in ReplyCloseAcknowledgementSet:
    /\ ReplyRouteFullSafetyInvariant
    /\ AcknowledgeCloseSemanticRequest(acknowledgement)
    /\ rrCloseAcknowledgedThrough[
         acknowledgement.requester][acknowledgement.responder] =
         acknowledgement.closedThrough
    => UNCHANGED ReplyRouteVars
PROOF
  <1>1. ASSUME NEW acknowledgement
                  \in ReplyCloseAcknowledgementSet,
                ReplyRouteFullSafetyInvariant,
                AcknowledgeCloseSemanticRequest(acknowledgement),
                rrCloseAcknowledgedThrough[
                  acknowledgement.requester][
                    acknowledgement.responder] =
                  acknowledgement.closedThrough
         PROVE UNCHANGED ReplyRouteVars
    <2>1. /\ acknowledgement.requester \in ReplyOwners
           /\ acknowledgement.responder \in ReplySources
           /\ rrCloseAcknowledgedThrough
                \in [ReplyOwners ->
                      [ReplySources ->
                        0..ReplyDeliveryOrdinalLimit]]
      BY <1>1
         DEF ReplyCloseAcknowledgementSet,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>2. rrCloseAcknowledgedThrough' =
             rrCloseAcknowledgedThrough
      BY <1>1, <2>1, ReplyNestedFunctionalUpdateIdentity
         DEF AcknowledgeCloseSemanticRequest
    <2> QED BY <1>1, <2>2
         DEF AcknowledgeCloseSemanticRequest, ReplyRouteVars
  <1> QED BY <1>1

THEOREM ReplyCanonicalCloseAcknowledgementIsValid ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    /\ ReplyCanonicalCloseAcknowledgement(
         requester, responder, closedThrough)
         \in ReplyCloseAcknowledgementSet
    /\ ReplyCloseAcknowledgementValid(
         ReplyCanonicalCloseAcknowledgement(
           requester, responder, closedThrough))
BY SMTT(10)
   DEF ReplyCanonicalCloseAcknowledgement,
       ReplyCloseAcknowledgement,
       ReplyCloseAcknowledgementSet,
       ReplyCloseAcknowledgementValid

THEOREM ReplyCanonicalCloseAcknowledgementFields ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    LET acknowledgement ==
          ReplyCanonicalCloseAcknowledgement(
            requester, responder, closedThrough)
    IN /\ acknowledgement.requester = requester
       /\ acknowledgement.responder = responder
       /\ acknowledgement.authenticatedResponder = responder
       /\ acknowledgement.closedThrough = closedThrough
       /\ acknowledgement.bindingRequester = requester
       /\ acknowledgement.bindingResponder = responder
       /\ acknowledgement.bindingClosedThrough = closedThrough
BY DEF ReplyCanonicalCloseAcknowledgement,
       ReplyCloseAcknowledgement

THEOREM ReplyCloseWorkAtFloorEnablesExactAcknowledgement ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    => ENABLED
         <<AcknowledgeCloseSemanticRequest(
             ReplyCanonicalCloseAcknowledgement(
               requester, responder, closedThrough))>>_ReplyRouteVars
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough)
         PROVE ENABLED
                 <<AcknowledgeCloseSemanticRequest(
                     ReplyCanonicalCloseAcknowledgement(
                       requester, responder,
                       closedThrough))>>_ReplyRouteVars
    <2>1. LET acknowledgement ==
                   ReplyCanonicalCloseAcknowledgement(
                     requester, responder, closedThrough)
           IN /\ ReplyCloseAcknowledgementValid(acknowledgement)
              /\ closedThrough # 0
              /\ closedThrough =
                   rrClosePendingThrough[requester][responder]
              /\ closedThrough =
                   rrCloseSentThrough[requester][responder]
              /\ closedThrough <=
                   rrRequesterClosedThrough[requester]
              /\ rrCloseAcknowledgedThrough[
                   requester][responder] < closedThrough
      BY <1>1, ReplyCanonicalCloseAcknowledgementIsValid,
         SMTT(15)
         DEF ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant,
             ReplyLifecycleOwnershipInvariant,
             ReplyCloseWorkAtFloor, ReplyCloseWorkPending
    <2>2. [rrCloseAcknowledgedThrough EXCEPT
             ![requester][responder] = closedThrough] #
             rrCloseAcknowledgedThrough
      <3>1. [rrCloseAcknowledgedThrough EXCEPT
               ![requester][responder] = closedThrough][
                 requester][responder] = closedThrough
        BY <1>1, ReplyNestedFunctionalUpdateAtKey
           DEF ReplyRouteFullSafetyInvariant,
               ReplyRouteLifecycleInvariant,
               ReplyLifecycleTypeInvariant
      <3> QED BY <2>1, <3>1
    <2>3. LET acknowledgement ==
                   ReplyCanonicalCloseAcknowledgement(
                     requester, responder, closedThrough)
           IN /\ acknowledgement.requester = requester
              /\ acknowledgement.responder = responder
              /\ acknowledgement.closedThrough = closedThrough
      BY <1>1, ReplyCanonicalCloseAcknowledgementFields
    <2> QED BY <1>1, <2>1, <2>2, <2>3, ExpandENABLED, Isa
         DEF AcknowledgeCloseSemanticRequest, ReplyRouteVars
  <1> QED BY <1>1

THEOREM ReplyUnchangedCloseChannelPreservesFloorWork ==
  \A requester, responder, closedThrough:
    /\ ReplyCloseWorkAtFloor(requester, responder, closedThrough)
    /\ UNCHANGED <<rrClosePendingThrough, rrCloseSentThrough,
                   rrCloseAcknowledgedThrough>>
    => ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)'
BY DEF ReplyCloseWorkAtFloor, ReplyCloseWorkPending

THEOREM ReplyCloseStepPreservesOtherPendingFloor ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit,
     witness \in ReplyCloseWitnessSet:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    /\ CloseSemanticRequest(witness)
    => ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)'
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                NEW witness \in ReplyCloseWitnessSet,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough),
                CloseSemanticRequest(witness)
         PROVE ReplyCloseWorkAtFloor(
                 requester, responder, closedThrough)'
    <2>1. ~(/\ requester = witness.requester
             /\ responder = witness.responder)
      BY <1>1
         DEF CloseSemanticRequest,
             ReplyCloseWorkAtFloor, ReplyCloseWorkPending
    <2>2. /\ rrClosePendingThrough'
                  [requester][responder] =
                    rrClosePendingThrough[requester][responder]
           /\ rrCloseSentThrough'
                  [requester][responder] =
                    rrCloseSentThrough[requester][responder]
           /\ rrCloseAcknowledgedThrough'
                  [requester][responder] =
                    rrCloseAcknowledgedThrough[requester][responder]
      BY <1>1, <2>1,
         ReplyNestedFunctionalUpdateAwayFromKey,
         SMTT(15)
         DEF CloseSemanticRequest,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2> QED BY <1>1, <2>2
         DEF ReplyCloseWorkAtFloor, ReplyCloseWorkPending
  <1> QED BY <1>1

THEOREM ReplyAcknowledgementPersistsOrTerminatesFloorWork ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit,
     acknowledgement \in ReplyCloseAcknowledgementSet:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    /\ AcknowledgeCloseSemanticRequest(acknowledgement)
    => \/ ReplyCloseWorkAtFloor(
            requester, responder, closedThrough)'
       \/ ~ReplyCloseWorkPending(requester, responder)'
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                NEW acknowledgement
                  \in ReplyCloseAcknowledgementSet,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough),
                AcknowledgeCloseSemanticRequest(acknowledgement)
         PROVE \/ ReplyCloseWorkAtFloor(
                     requester, responder, closedThrough)'
               \/ ~ReplyCloseWorkPending(requester, responder)'
    <2>1. CASE /\ requester = acknowledgement.requester
                /\ responder = acknowledgement.responder
      BY <1>1, <2>1,
         ReplyNestedFunctionalUpdateAtKey,
         SMTT(15)
         DEF AcknowledgeCloseSemanticRequest,
             ReplyCloseWorkAtFloor, ReplyCloseWorkPending,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>2. CASE \/ requester # acknowledgement.requester
                \/ responder # acknowledgement.responder
      BY <1>1, <2>2,
         ReplyNestedFunctionalUpdateAwayFromKey,
         SMTT(15)
         DEF AcknowledgeCloseSemanticRequest,
             ReplyCloseWorkAtFloor, ReplyCloseWorkPending,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyRouteNextPersistsOrTerminatesFloorWork ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    /\ ReplyRouteNext
    => \/ ReplyCloseWorkAtFloor(
            requester, responder, closedThrough)'
       \/ ~ReplyCloseWorkPending(requester, responder)'
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough),
                ReplyRouteNext
         PROVE \/ ReplyCloseWorkAtFloor(
                     requester, responder, closedThrough)'
               \/ ~ReplyCloseWorkPending(requester, responder)'
    <2>1. CASE \E witness \in ReplyCloseWitnessSet:
                 CloseSemanticRequest(witness)
      BY <1>1, <2>1, ReplyCloseStepPreservesOtherPendingFloor
    <2>2. CASE \E witness \in ReplyCloseWitnessSet:
                 PiggybackCloseSemanticRequest(witness)
      BY <1>1, <2>2, ReplyCloseStepPreservesOtherPendingFloor
         DEF PiggybackCloseSemanticRequest
    <2>3. CASE \E acknowledgement
                   \in ReplyCloseAcknowledgementSet:
                 AcknowledgeCloseSemanticRequest(acknowledgement)
      BY <1>1, <2>3,
         ReplyAcknowledgementPersistsOrTerminatesFloorWork
    <2>4. CASE \E owner \in ReplyOwners,
                       semantic \in ReplySemantics,
                       source \in ReplySources:
                 ObserveNewReplySource(owner, semantic, source)
      BY <1>1, <2>4, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF ObserveNewReplySource
    <2>5. CASE \E owner \in ReplyOwners,
                       semantic \in ReplySemantics,
                       source \in ReplySources:
                 ObserveLaterReplyDelivery(owner, semantic, source)
      BY <1>1, <2>5, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF ObserveLaterReplyDelivery
    <2>6. CASE \E owner \in ReplyOwners,
                       semantic \in ReplySemantics,
                       source \in ReplySources:
                 RetryExactReplySource(owner, semantic, source)
      BY <1>1, <2>6, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF RetryExactReplySource, ReplyRouteVars
    <2>7. CASE \E owner \in ReplyOwners, source \in ReplySources:
                 RetireReplySource(owner, source)
      BY <1>1, <2>7, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF RetireReplySource
    <2>8. CASE \E owner \in ReplyOwners,
                       semantic \in ReplySemantics,
                       source \in ReplySources:
                 ReconnectReplySource(owner, semantic, source)
      BY <1>1, <2>8, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF ReconnectReplySource
    <2>9. CASE \E owner \in ReplyOwners,
                       semantic \in ReplySemantics,
                       source \in ReplySources:
                 AcquireReplyTicket(owner, semantic, source)
      BY <1>1, <2>9, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF AcquireReplyTicket
    <2>10. CASE \E owner \in ReplyOwners,
                        semantic \in ReplySemantics:
                  ServiceReplyRoute(owner, semantic)
      BY <1>1, <2>10, ReplyUnchangedCloseChannelPreservesFloorWork
         DEF ServiceReplyRoute
    <2>11. CASE \E witness \in ReplyCloseWitnessSet:
                  RetryCloseSemanticRequest(witness)
      BY <1>1, <2>11,
         ReplyUnchangedCloseChannelPreservesFloorWork
         DEF RetryCloseSemanticRequest, ReplyRouteVars
    <2>12. CASE \E owner \in ReplyOwners, source \in ReplySources:
                  RecoverReplyRouteState(owner, source)
      BY <1>1, <2>12,
         ReplyUnchangedCloseChannelPreservesFloorWork
         DEF RecoverReplyRouteState, RetireReplySource
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
         <2>8, <2>9, <2>10, <2>11, <2>12
         DEF ReplyRouteNext
  <1> QED BY <1>1

THEOREM ReplyRouteBracketPersistsOrTerminatesFloorWork ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    /\ [ReplyRouteNext]_ReplyRouteVars
    => \/ ReplyCloseWorkAtFloor(
            requester, responder, closedThrough)'
       \/ ~ReplyCloseWorkPending(requester, responder)'
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough),
                [ReplyRouteNext]_ReplyRouteVars
         PROVE \/ ReplyCloseWorkAtFloor(
                     requester, responder, closedThrough)'
               \/ ~ReplyCloseWorkPending(requester, responder)'
    <2>1. CASE ReplyRouteNext
      BY <1>1, <2>1,
         ReplyRouteNextPersistsOrTerminatesFloorWork
    <2>2. CASE UNCHANGED ReplyRouteVars
      BY <1>1, <2>2,
         ReplyUnchangedCloseChannelPreservesFloorWork
         DEF ReplyRouteVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyExactAcknowledgementTerminatesFloorWork ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
    /\ <<AcknowledgeCloseSemanticRequest(
            ReplyCanonicalCloseAcknowledgement(
              requester, responder,
              closedThrough))>>_ReplyRouteVars
    => ~ReplyCloseWorkPending(requester, responder)'
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkAtFloor(
                  requester, responder, closedThrough),
                <<AcknowledgeCloseSemanticRequest(
                    ReplyCanonicalCloseAcknowledgement(
                      requester, responder,
                      closedThrough))>>_ReplyRouteVars
         PROVE ~ReplyCloseWorkPending(requester, responder)'
    <2>1. AcknowledgeCloseSemanticRequest(
             ReplyCanonicalCloseAcknowledgement(
               requester, responder, closedThrough))
      BY <1>1, PTL
    <2>2. LET acknowledgement ==
                   ReplyCanonicalCloseAcknowledgement(
                     requester, responder, closedThrough)
           IN /\ acknowledgement.requester = requester
              /\ acknowledgement.responder = responder
              /\ acknowledgement.closedThrough = closedThrough
      BY <1>1, ReplyCanonicalCloseAcknowledgementFields
    <2>3. /\ rrCloseAcknowledgedThrough
                  \in [ReplyOwners ->
                        [ReplySources ->
                          0..ReplyDeliveryOrdinalLimit]]
           /\ rrCloseAcknowledgedThrough' =
                [rrCloseAcknowledgedThrough EXCEPT
                   ![requester][responder] = closedThrough]
           /\ rrClosePendingThrough' = rrClosePendingThrough
      BY <1>1, <2>1, <2>2
         DEF AcknowledgeCloseSemanticRequest,
             ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant
    <2>4. rrCloseAcknowledgedThrough'[
             requester][responder] = closedThrough
      BY <1>1, <2>3, ReplyNestedFunctionalUpdateAtKey
    <2>5. rrClosePendingThrough'[requester][responder] =
             closedThrough
      BY <1>1, <2>3 DEF ReplyCloseWorkAtFloor
    <2> QED BY <2>4, <2>5, SMT
         DEF ReplyCloseWorkPending
  <1> QED BY <1>1

ReplyCloseFloorPersistenceObligation(
    requester, responder, closedThrough) ==
  /\ ReplyRouteFullSafetyInvariant
  /\ ReplyCloseWorkAtFloor(requester, responder, closedThrough)
  /\ [ReplyRouteNext]_ReplyRouteVars
  => \/ ReplyCloseWorkAtFloor(requester, responder, closedThrough)'
     \/ ~ReplyCloseWorkPending(requester, responder)'

THEOREM ReplyCloseFloorPersistenceObligationsHold ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    ReplyCloseFloorPersistenceObligation(
      requester, responder, closedThrough)
BY ReplyRouteBracketPersistsOrTerminatesFloorWork
   DEF ReplyCloseFloorPersistenceObligation

THEOREM ReplyCloseFloorPersistenceObligationHoldsAt ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         NEW closedThrough \in 0..ReplyDeliveryOrdinalLimit
  PROVE ReplyCloseFloorPersistenceObligation(
          requester, responder, closedThrough)
BY ReplyCloseFloorPersistenceObligationsHold

ReplyCloseFloorEnablementObligation(
    requester, responder, closedThrough) ==
  /\ ReplyRouteFullSafetyInvariant
  /\ ReplyCloseWorkAtFloor(requester, responder, closedThrough)
  => ENABLED
       <<AcknowledgeCloseSemanticRequest(
           ReplyCanonicalCloseAcknowledgement(
             requester, responder, closedThrough))>>_ReplyRouteVars

THEOREM ReplyCloseFloorEnablementObligationsHold ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    ReplyCloseFloorEnablementObligation(
      requester, responder, closedThrough)
BY ReplyCloseWorkAtFloorEnablesExactAcknowledgement
   DEF ReplyCloseFloorEnablementObligation

THEOREM ReplyCloseFloorEnablementObligationHoldsAt ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         NEW closedThrough \in 0..ReplyDeliveryOrdinalLimit
  PROVE ReplyCloseFloorEnablementObligation(
          requester, responder, closedThrough)
BY ReplyCloseFloorEnablementObligationsHold

ReplyCloseFloorOutcomeObligation(
    requester, responder, closedThrough) ==
  /\ ReplyRouteFullSafetyInvariant
  /\ ReplyCloseWorkAtFloor(requester, responder, closedThrough)
  /\ <<AcknowledgeCloseSemanticRequest(
          ReplyCanonicalCloseAcknowledgement(
            requester, responder, closedThrough))>>_ReplyRouteVars
  => ~ReplyCloseWorkPending(requester, responder)'

THEOREM ReplyCloseFloorOutcomeObligationsHold ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    ReplyCloseFloorOutcomeObligation(
      requester, responder, closedThrough)
BY ReplyExactAcknowledgementTerminatesFloorWork
   DEF ReplyCloseFloorOutcomeObligation

THEOREM ReplyCloseFloorOutcomeObligationHoldsAt ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         NEW closedThrough \in 0..ReplyDeliveryOrdinalLimit
  PROVE ReplyCloseFloorOutcomeObligation(
          requester, responder, closedThrough)
BY ReplyCloseFloorOutcomeObligationsHold

THEOREM ReplyCloseFloorPersistenceBracketProjects ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    [ReplyCloseFloorPersistenceObligationsHold]_ReplyRouteVars
      => [ReplyCloseFloorPersistenceObligation(
            requester, responder, closedThrough)]_ReplyRouteVars
BY Isa

THEOREM ReplyCloseFloorOutcomeBracketProjects ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    [ReplyCloseFloorOutcomeObligationsHold]_ReplyRouteVars
      => [ReplyCloseFloorOutcomeObligation(
            requester, responder, closedThrough)]_ReplyRouteVars
BY Isa

THEOREM ReplyCloseAcknowledgementWeakFairnessDefinition ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         NEW closedThrough \in 0..ReplyDeliveryOrdinalLimit
  PROVE WF_ReplyRouteVars(
          AcknowledgeCloseSemanticRequest(
            ReplyCanonicalCloseAcknowledgement(
              requester, responder, closedThrough)))
        <=> (<>[]ENABLED
                   <<AcknowledgeCloseSemanticRequest(
                       ReplyCanonicalCloseAcknowledgement(
                         requester, responder,
                         closedThrough))>>_ReplyRouteVars
               => []<>
                    <<AcknowledgeCloseSemanticRequest(
                        ReplyCanonicalCloseAcknowledgement(
                          requester, responder,
                          closedThrough))>>_ReplyRouteVars)
BY PTL

THEOREM ReplyCloseFloorLifecycleWorkLeadsToTermination ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         NEW closedThrough \in 0..ReplyDeliveryOrdinalLimit
  PROVE /\ [][ReplyRouteNext]_ReplyRouteVars
         /\ WF_ReplyRouteVars(
              AcknowledgeCloseSemanticRequest(
                ReplyCanonicalCloseAcknowledgement(
                  requester, responder, closedThrough)))
         => ((/\ ReplyRouteLifecycleInductiveInvariant
                 /\ ReplyCloseWorkAtFloor(
                      requester, responder, closedThrough))
               ~> ~ReplyCloseWorkPending(requester, responder))
PROOF
  <1>1. /\ ReplyRouteLifecycleInductiveInvariant
          /\ ReplyCloseWorkAtFloor(
               requester, responder, closedThrough)
          /\ [ReplyRouteNext]_ReplyRouteVars
         => \/ (/\ ReplyRouteLifecycleInductiveInvariant
                    /\ ReplyCloseWorkAtFloor(
                         requester, responder, closedThrough))'
            \/ ~ReplyCloseWorkPending(requester, responder)'
    BY ReplyRouteLifecycleBracketPreservesInvariant,
       ReplyRouteBracketPersistsOrTerminatesFloorWork,
       IsaM("blast")
       DEF ReplyRouteLifecycleInductiveInvariant
  <1>2. /\ ReplyRouteLifecycleInductiveInvariant
          /\ ReplyCloseWorkAtFloor(
               requester, responder, closedThrough)
          /\ [ReplyRouteNext]_ReplyRouteVars
          /\ <<AcknowledgeCloseSemanticRequest(
                  ReplyCanonicalCloseAcknowledgement(
                    requester, responder,
                    closedThrough))>>_ReplyRouteVars
         => ~ReplyCloseWorkPending(requester, responder)'
    BY ReplyExactAcknowledgementTerminatesFloorWork
       DEF ReplyRouteLifecycleInductiveInvariant
  <1>3. /\ ReplyRouteLifecycleInductiveInvariant
          /\ ReplyCloseWorkAtFloor(
               requester, responder, closedThrough)
          /\ [ReplyRouteNext]_ReplyRouteVars
         => ENABLED
              <<AcknowledgeCloseSemanticRequest(
                  ReplyCanonicalCloseAcknowledgement(
                    requester, responder,
                    closedThrough))>>_ReplyRouteVars
    BY ReplyCloseWorkAtFloorEnablesExactAcknowledgement
       DEF ReplyRouteLifecycleInductiveInvariant
  <1> QED BY <1>1, <1>2, <1>3,
       ReplyCloseAcknowledgementWeakFairnessDefinition, PTL

THEOREM ReplyRouteSpecTerminatesCloseWorkAtFloor ==
  \A requester \in ReplyOwners, responder \in ReplySources,
     closedThrough \in 0..ReplyDeliveryOrdinalLimit:
    ReplyRouteSpec =>
      (ReplyCloseWorkAtFloor(
         requester, responder, closedThrough)
        ~> ~ReplyCloseWorkPending(requester, responder))
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                NEW closedThrough
                  \in 0..ReplyDeliveryOrdinalLimit,
                ReplyRouteSpec
         PROVE ReplyCloseWorkAtFloor(
                 requester, responder, closedThrough)
                 ~> ~ReplyCloseWorkPending(requester, responder)
    <2>1. []ReplyRouteLifecycleInductiveInvariant
      BY <1>1, ReplyRouteSpecAlwaysLifecycleInductiveInvariant
    <2>2. [][ReplyRouteNext]_ReplyRouteVars
      BY <1>1, PTL DEF ReplyRouteSpec
    <2>3. WF_ReplyRouteVars(
             AcknowledgeCloseSemanticRequest(
               ReplyCanonicalCloseAcknowledgement(
                 requester, responder, closedThrough)))
      <3>1. ReplyCanonicalCloseAcknowledgement(
               requester, responder, closedThrough)
               \in ReplyCloseAcknowledgementSet
        BY <1>1, ReplyCanonicalCloseAcknowledgementIsValid
      <3> QED BY <1>1, <3>1, IsaM("blast")
           DEF ReplyRouteSpec, ReplyRouteFairness
    <2>4. (/\ ReplyRouteLifecycleInductiveInvariant
              /\ ReplyCloseWorkAtFloor(
                   requester, responder, closedThrough))
             ~> ~ReplyCloseWorkPending(requester, responder)
      BY <1>1, <2>2, <2>3,
         ReplyCloseFloorLifecycleWorkLeadsToTermination
    <2>5. ReplyCloseWorkAtFloor(
             requester, responder, closedThrough)
             ~> (/\ ReplyRouteLifecycleInductiveInvariant
                    /\ ReplyCloseWorkAtFloor(
                         requester, responder, closedThrough))
      BY <2>1, PTL
    <2> QED BY <2>4, <2>5, PTL
  <1> QED BY <1>1

THEOREM ReplyPendingCloseHasTrackedFloor ==
  \A requester \in ReplyOwners, responder \in ReplySources:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkPending(requester, responder)
    => \E closedThrough \in 0..ReplyDeliveryOrdinalLimit:
         ReplyCloseWorkAtFloor(
           requester, responder, closedThrough)
PROOF
  <1>1. ASSUME NEW requester \in ReplyOwners,
                NEW responder \in ReplySources,
                ReplyRouteFullSafetyInvariant,
                ReplyCloseWorkPending(requester, responder)
         PROVE \E closedThrough
                   \in 0..ReplyDeliveryOrdinalLimit:
                 ReplyCloseWorkAtFloor(
                   requester, responder, closedThrough)
    <2>1. /\ rrClosePendingThrough[requester][responder]
                  \in 0..ReplyDeliveryOrdinalLimit
           /\ rrCloseSentThrough[requester][responder] =
                rrClosePendingThrough[requester][responder]
      BY <1>1
         DEF ReplyRouteFullSafetyInvariant,
             ReplyRouteLifecycleInvariant,
             ReplyLifecycleTypeInvariant,
             ReplyLifecycleOwnershipInvariant
    <2>2. ReplyCloseWorkAtFloor(
             requester, responder,
             rrClosePendingThrough[requester][responder])
      BY <1>1, <2>1 DEF ReplyCloseWorkAtFloor
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM ReplyPendingCloseTrackedFloorObligationsHold ==
  \A requester \in ReplyOwners, responder \in ReplySources:
    /\ ReplyRouteFullSafetyInvariant
    /\ ReplyCloseWorkPending(requester, responder)
    => \E closedThrough \in 0..ReplyDeliveryOrdinalLimit:
         ReplyCloseWorkAtFloor(
           requester, responder, closedThrough)
BY ReplyPendingCloseHasTrackedFloor

THEOREM ReplyPendingCloseHasTrackedFloorAt ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources
  PROVE /\ ReplyRouteFullSafetyInvariant
          /\ ReplyCloseWorkPending(requester, responder)
         => \E closedThrough \in 0..ReplyDeliveryOrdinalLimit:
              ReplyCloseWorkAtFloor(
                requester, responder, closedThrough)
BY ReplyPendingCloseHasTrackedFloor

ReplyCloseFloorExistentialEquivalenceObligation(
    requester, responder, floors) ==
  (\A closedThrough \in floors:
     ReplyCloseWorkAtFloor(requester, responder, closedThrough)
       => <>~ReplyCloseWorkPending(requester, responder))
    <=> ((\E closedThrough \in floors:
            ReplyCloseWorkAtFloor(requester, responder, closedThrough))
          => <>~ReplyCloseWorkPending(requester, responder))

ReplyCloseFloorCarrierExistentialEquivalenceObligationsHold ==
  \A requester \in ReplyOwners, responder \in ReplySources:
    ReplyCloseFloorExistentialEquivalenceObligation(
      requester, responder, 0..ReplyDeliveryOrdinalLimit)

THEOREM ReplyCloseFloorCarrierExistentialEquivalenceObligationsHoldProof ==
  ReplyCloseFloorCarrierExistentialEquivalenceObligationsHold
BY IsaM("blast")
   DEF ReplyCloseFloorCarrierExistentialEquivalenceObligationsHold,
       ReplyCloseFloorExistentialEquivalenceObligation

THEOREM ReplyCloseFloorExistentialEquivalenceHoldsAt ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources
  PROVE ReplyCloseFloorExistentialEquivalenceObligation(
          requester, responder, 0..ReplyDeliveryOrdinalLimit)
BY IsaM("blast")
   DEF ReplyCloseFloorExistentialEquivalenceObligation

THEOREM ReplyCloseFloorExistentialLift ==
  ASSUME NEW requester \in ReplyOwners,
         NEW responder \in ReplySources,
         \A closedThrough \in 0..ReplyDeliveryOrdinalLimit:
           ReplyCloseWorkAtFloor(
             requester, responder, closedThrough)
             ~> ~ReplyCloseWorkPending(requester, responder)
  PROVE (\E closedThrough \in 0..ReplyDeliveryOrdinalLimit:
           ReplyCloseWorkAtFloor(
             requester, responder, closedThrough))
          ~> ~ReplyCloseWorkPending(requester, responder)
PROOF
  <1>1. (\A closedThrough \in 0..ReplyDeliveryOrdinalLimit:
            [](ReplyCloseWorkAtFloor(
                 requester, responder, closedThrough)
                 => <>~ReplyCloseWorkPending(requester, responder)))
          <=> [](\A closedThrough
                        \in 0..ReplyDeliveryOrdinalLimit:
                   ReplyCloseWorkAtFloor(
                     requester, responder, closedThrough)
                     => <>~ReplyCloseWorkPending(
                          requester, responder))
    OBVIOUS
  <1>2. [](\A closedThrough \in 0..ReplyDeliveryOrdinalLimit:
             ReplyCloseWorkAtFloor(
               requester, responder, closedThrough)
               => <>~ReplyCloseWorkPending(requester, responder))
          <=> []((\E closedThrough
                       \in 0..ReplyDeliveryOrdinalLimit:
                    ReplyCloseWorkAtFloor(
                      requester, responder, closedThrough))
                 => <>~ReplyCloseWorkPending(requester, responder))
    <2>1. []ReplyCloseFloorExistentialEquivalenceObligation(
               requester, responder, 0..ReplyDeliveryOrdinalLimit)
      BY ReplyCloseFloorExistentialEquivalenceHoldsAt, PTL
    <2> QED BY <2>1, PTL
         DEF ReplyCloseFloorExistentialEquivalenceObligation
  <1> QED BY <1>1, <1>2, PTL

THEOREM ReplyRouteSpecTerminatesCloseWork ==
  ReplyRouteSpec =>
    \A requester \in ReplyOwners, responder \in ReplySources:
      ReplyCloseWorkEventuallyTerminates(requester, responder)
PROOF
  <1>1. ASSUME ReplyRouteSpec
         PROVE \A requester \in ReplyOwners,
                    responder \in ReplySources:
                   ReplyCloseWorkEventuallyTerminates(
                     requester, responder)
    <2>1. ASSUME NEW requester \in ReplyOwners,
                  NEW responder \in ReplySources
           PROVE ReplyCloseWorkEventuallyTerminates(
                   requester, responder)
      <3>1. []ReplyRouteFullSafetyInvariant
        BY <1>1, ReplyRouteSpecAlwaysFullSafetyInvariant
      <3>2. [](/\ ReplyRouteFullSafetyInvariant
                 /\ ReplyCloseWorkPending(requester, responder)
                => \E closedThrough
                     \in 0..ReplyDeliveryOrdinalLimit:
                     ReplyCloseWorkAtFloor(
                       requester, responder, closedThrough))
        BY <2>1, ReplyPendingCloseHasTrackedFloorAt, PTL
      <3>3. [](ReplyCloseWorkPending(requester, responder) =>
                 \E closedThrough
                      \in 0..ReplyDeliveryOrdinalLimit:
                   ReplyCloseWorkAtFloor(
                     requester, responder, closedThrough))
        BY <3>1, <3>2, PTL
      <3>4. ReplyCloseWorkPending(requester, responder)
                 ~> (\E closedThrough
                          \in 0..ReplyDeliveryOrdinalLimit:
                       ReplyCloseWorkAtFloor(
                         requester, responder, closedThrough))
        BY <3>3, PTL
      <3>5. \A closedThrough
                    \in 0..ReplyDeliveryOrdinalLimit:
                 ReplyCloseWorkAtFloor(
                   requester, responder, closedThrough)
                   ~> ~ReplyCloseWorkPending(
                        requester, responder)
        BY <1>1, <2>1,
           ReplyRouteSpecTerminatesCloseWorkAtFloor
      <3>6. (\E closedThrough
                    \in 0..ReplyDeliveryOrdinalLimit:
                  ReplyCloseWorkAtFloor(
                    requester, responder, closedThrough))
                  ~> ~ReplyCloseWorkPending(requester, responder)
        BY <2>1, <3>5, ReplyCloseFloorExistentialLift
      <3> QED BY <3>4, <3>6, PTL
           DEF ReplyCloseWorkEventuallyTerminates
    <2> QED BY <2>1
  <1> QED BY <1>1

THEOREM ReplyRouteOwnershipModelObligation ==
  ReplyRouteSpec =>
    /\ []ReplyRouteFullSafetyInvariant
    /\ ReplyTenureAwareReplay
    /\ ReplySourceIsolation
    /\ ReplyLifecycleJournal
    /\ \A requester \in ReplyOwners, responder \in ReplySources:
         ReplyCloseWorkEventuallyTerminates(requester, responder)
    /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
          source \in ReplySources:
         ReplySourceEventuallyProgresses(owner, semantic, source)
BY ReplyRouteSpecAlwaysFullSafetyInvariant,
   ReplyRouteSpecAlwaysReplayAndIsolation,
   ReplyRouteSpecProvidesLifecycleJournal,
   ReplyRouteSpecTerminatesCloseWork,
   ReplyRouteSpecProvidesSourceProgress

=============================================================================
