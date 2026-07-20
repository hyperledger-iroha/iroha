---- MODULE SumeragiV2ReplyRouteOwnership ----
EXTENDS Naturals, Sequences, FiniteSets

(***************************************************************************
Bounded exact-output reply ownership.

This state machine is deliberately independent of connection lifetime.  A
process-local actor allocates one monotonically increasing delivery ordinal
for every newly observed authenticated delivery.  Connection tenure is a
separate source-scoped value.  Canonical semantic request identity excludes
both values, so a request owns at most one attempt per authenticated source.

Every attempt retains its own message and chunk cursor.  An exact duplicate is
a route-state stutter, while a later delivery from the same source preserves
those cursors.  A reconnect changes connection tenure and invalidates the old
admission ticket, but retries the current message/chunk cursor.  Only a newly
attached alternate source starts at <<0, 0>>.  The payload carrier is a set
keyed only by semantic identity, which models immutable byte sharing without
conflating per-source progress.
***************************************************************************)

CONSTANTS
  ReplyOwners,
  ReplySourceOrder,
  ReplySemantics,
  ReplySourceCapacity,
  ReplyDeliveryOrdinalLimit,
  ReplyMessageCount,
  ReplyChunkCount

ReplySources ==
  {ReplySourceOrder[index]: index \in 1..Len(ReplySourceOrder)}
ReplyDeliveryOrdinals == 1..ReplyDeliveryOrdinalLimit
ReplyConnectionTenures == 1..ReplyDeliveryOrdinalLimit
NoReplyTicketTenure == 0

ReplyRouteConfiguration ==
  /\ IsFiniteSet(ReplyOwners)
  /\ ReplyOwners # {}
  /\ ReplySourceOrder \in Seq(ReplySources)
  /\ Len(ReplySourceOrder) > 0
  /\ Len(ReplySourceOrder) = Cardinality(ReplySources)
  /\ ReplySourceCapacity = Len(ReplySourceOrder)
  /\ IsFiniteSet(ReplySemantics)
  /\ ReplySemantics # {}
  /\ ReplyDeliveryOrdinalLimit \in Nat \ {0}
  /\ ReplyMessageCount \in Nat \ {0}
  /\ ReplyChunkCount \in Nat \ {0}

ReplyCapability(owner, source, target, semantic, deliveryOrdinal,
                connectionTenure) ==
  [owner |-> owner, source |-> source, target |-> target,
   semantic |-> semantic, deliveryOrdinal |-> deliveryOrdinal,
   connectionTenure |-> connectionTenure]

ReplyTicket(owner, source, semantic, connectionTenure) ==
  [owner |-> owner, source |-> source, semantic |-> semantic,
   connectionTenure |-> connectionTenure]

ReplyAttempt(owner, source, semantic, deliveryOrdinal, connectionTenure,
             ticketTenure, messageCursor, chunkCursor) ==
  [owner |-> owner, source |-> source, semantic |-> semantic,
   deliveryOrdinal |-> deliveryOrdinal,
   connectionTenure |-> connectionTenure,
   ticketTenure |-> ticketTenure,
   messageCursor |-> messageCursor, chunkCursor |-> chunkCursor]

ReplyAttemptSet ==
  [owner: ReplyOwners, source: ReplySources, semantic: ReplySemantics,
   deliveryOrdinal: ReplyDeliveryOrdinals,
   connectionTenure: ReplyConnectionTenures,
   ticketTenure: 0..ReplyDeliveryOrdinalLimit,
   messageCursor: 0..ReplyMessageCount,
   chunkCursor: 0..ReplyChunkCount]

VARIABLES
  rrAttempts,
  rrPayloads,
  rrNextDeliveryOrdinal,
  rrConnectionTenure,
  rrSourceActive,
  rrNextServiceIndex

ReplyRouteVars ==
  <<rrAttempts, rrPayloads, rrNextDeliveryOrdinal, rrConnectionTenure,
    rrSourceActive, rrNextServiceIndex>>

ReplyAttemptsFor(owner, semantic) ==
  {attempt \in rrAttempts:
     /\ attempt.owner = owner
     /\ attempt.semantic = semantic}

ReplyAttemptsForSource(owner, semantic, source) ==
  {attempt \in ReplyAttemptsFor(owner, semantic):
     attempt.source = source}

ReplyAttemptSources(owner, semantic) ==
  {attempt.source: attempt \in ReplyAttemptsFor(owner, semantic)}

ReplyAttemptOwned(owner, semantic, source) ==
  ReplyAttemptsForSource(owner, semantic, source) # {}

ReplyAttemptFor(owner, semantic, source) ==
  CHOOSE attempt \in ReplyAttemptsForSource(owner, semantic, source): TRUE

ReplyAttemptCursor(attempt) ==
  <<attempt.messageCursor, attempt.chunkCursor>>

ReplyAttemptRank(attempt) ==
  attempt.messageCursor * (ReplyChunkCount + 1) + attempt.chunkCursor

ReplyAttemptComplete(attempt) ==
  /\ attempt.messageCursor = ReplyMessageCount
  /\ attempt.chunkCursor = ReplyChunkCount

ReplyAttemptCurrent(attempt) ==
  /\ rrSourceActive[attempt.owner][attempt.source]
  /\ attempt.connectionTenure =
       rrConnectionTenure[attempt.owner][attempt.source]

ReplyTicketForAttempt(attempt) ==
  ReplyTicket(attempt.owner, attempt.source, attempt.semantic,
              attempt.ticketTenure)

ReplyTicketValidForAttempt(attempt) ==
  /\ ReplyAttemptCurrent(attempt)
  /\ attempt.ticketTenure = attempt.connectionTenure
  /\ ReplyTicketForAttempt(attempt) =
       ReplyTicket(attempt.owner, attempt.source, attempt.semantic,
                   rrConnectionTenure[attempt.owner][attempt.source])

ReplyCapabilityValidFor(capability, expectedOwner, expectedSource,
                        expectedSemantic) ==
  /\ capability.owner = expectedOwner
  /\ capability.source = expectedSource
  /\ capability.target = expectedSource
  /\ capability.semantic = expectedSemantic
  /\ expectedOwner \in ReplyOwners
  /\ expectedSource \in ReplySources
  /\ expectedSemantic \in ReplySemantics
  /\ rrSourceActive[expectedOwner][expectedSource]
  /\ capability.connectionTenure =
       rrConnectionTenure[expectedOwner][expectedSource]
  /\ capability.deliveryOrdinal \in ReplyDeliveryOrdinals

ReplyCapabilityRejection(capability, expectedOwner, expectedSource,
                         expectedSemantic) ==
  CASE capability.owner # expectedOwner -> "ForeignOwner"
    [] capability.source # expectedSource -> "DifferentSource"
    [] capability.target # expectedSource -> "Retargeted"
    [] capability.semantic # expectedSemantic -> "Retargeted"
    [] ~rrSourceActive[expectedOwner][expectedSource] -> "Inactive"
    [] ReplyAttemptOwned(expectedOwner, expectedSemantic, expectedSource)
         /\ capability.deliveryOrdinal
              < ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).deliveryOrdinal -> "Stale"
    [] ReplyAttemptOwned(expectedOwner, expectedSemantic, expectedSource)
         /\ capability.deliveryOrdinal
              = ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).deliveryOrdinal
         /\ capability.connectionTenure
              # ReplyAttemptFor(expectedOwner, expectedSemantic,
                                expectedSource).connectionTenure ->
           "EqualOrdinalDifferentTenure"
    [] OTHER -> "NotRejected"

ReplaceReplyAttempt(oldAttempt, newAttempt) ==
  (rrAttempts \ {oldAttempt}) \cup {newAttempt}

ReplyAttemptWithRoute(attempt, deliveryOrdinal, connectionTenure) ==
  IF connectionTenure = attempt.connectionTenure
  THEN [attempt EXCEPT !.deliveryOrdinal = deliveryOrdinal]
  ELSE [attempt EXCEPT
          !.deliveryOrdinal = deliveryOrdinal,
          !.connectionTenure = connectionTenure,
          !.ticketTenure = NoReplyTicketTenure]

ReplyAttemptWithTicket(attempt) ==
  [attempt EXCEPT !.ticketTenure = attempt.connectionTenure]

ReplyAttemptAfterService(attempt) ==
  IF attempt.messageCursor < ReplyMessageCount
  THEN [attempt EXCEPT !.messageCursor = @ + 1]
  ELSE [attempt EXCEPT !.chunkCursor = @ + 1]

NextReplySourceIndex(index) ==
  IF index = Len(ReplySourceOrder) THEN 1 ELSE index + 1

ReplySourceCyclicDistance(startIndex, candidateIndex) ==
  IF candidateIndex >= startIndex
  THEN candidateIndex - startIndex
  ELSE Len(ReplySourceOrder) - startIndex + candidateIndex

ReplyPendingSourceIndices(owner, semantic) ==
  {index \in 1..Len(ReplySourceOrder):
     LET source == ReplySourceOrder[index]
     IN /\ ReplyAttemptOwned(owner, semantic, source)
        /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
           IN /\ ReplyTicketValidForAttempt(attempt)
              /\ ~ReplyAttemptComplete(attempt)}

ReplySelectedSourceIndex(owner, semantic) ==
  LET pending == ReplyPendingSourceIndices(owner, semantic)
      start == rrNextServiceIndex[owner][semantic]
  IN CHOOSE index \in pending:
       \A other \in pending:
         ReplySourceCyclicDistance(start, index)
           <= ReplySourceCyclicDistance(start, other)

ReplySelectedSource(owner, semantic) ==
  ReplySourceOrder[ReplySelectedSourceIndex(owner, semantic)]

ReplyRouteInit ==
  /\ ReplyRouteConfiguration
  /\ rrAttempts = {}
  /\ rrPayloads = {}
  /\ rrNextDeliveryOrdinal = [owner \in ReplyOwners |-> 1]
  /\ rrConnectionTenure =
       [owner \in ReplyOwners |-> [source \in ReplySources |-> 1]]
  /\ rrSourceActive =
       [owner \in ReplyOwners |-> [source \in ReplySources |-> TRUE]]
  /\ rrNextServiceIndex =
       [owner \in ReplyOwners |-> [semantic \in ReplySemantics |-> 1]]

(***************************************************************************
Only a new authenticated source allocates a cursor-zero attempt.  Capacity is
the exact configured source geometry, not an eviction policy.
***************************************************************************)
ObserveNewReplySource(owner, semantic, source) ==
  LET deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source]
      capability ==
        ReplyCapability(owner, source, source, semantic, deliveryOrdinal,
                        connectionTenure)
      attempt ==
        ReplyAttempt(owner, source, semantic, deliveryOrdinal,
                     connectionTenure, NoReplyTicketTenure, 0, 0)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ~ReplyAttemptOwned(owner, semantic, source)
     /\ Cardinality(ReplyAttemptSources(owner, semantic))
          < ReplySourceCapacity
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ rrAttempts' = rrAttempts \cup {attempt}
     /\ rrPayloads' = rrPayloads \cup {semantic}
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex>>

(***************************************************************************
A later delivery from one source changes only that attempt.  A same-tenure
delivery retains message/chunk rank and its ticket.  If another semantic
attempt already advanced the source-scoped connection tenure, this delivery
rebinds only the selected attempt and clears its stale ticket.  Neither case
changes the source-owned cursor.
***************************************************************************)
ObserveLaterReplyDelivery(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source]
      capability ==
        ReplyCapability(owner, source, source, semantic, deliveryOrdinal,
                        connectionTenure)
      routed ==
        ReplyAttemptWithRoute(oldAttempt, deliveryOrdinal,
                              connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ rrSourceActive[owner][source]
     /\ oldAttempt.connectionTenure <= connectionTenure
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ deliveryOrdinal > oldAttempt.deliveryOrdinal
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, routed)
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrPayloads, rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex>>

(***************************************************************************
An exact authenticated duplicate observes the already-owned attempt without
changing its route, ticket, service rank, or either cursor.
***************************************************************************)
RetryExactReplySource(owner, semantic, source) ==
  LET attempt == ReplyAttemptFor(owner, semantic, source)
      capability ==
        ReplyCapability(owner, source, source, semantic,
                        attempt.deliveryOrdinal,
                        attempt.connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyCapabilityValidFor(
          capability, owner, source, semantic)
     /\ UNCHANGED ReplyRouteVars

(***************************************************************************
Connection teardown invalidates tenure-bound tickets but does not erase
semantic ownership or either progress cursor.
***************************************************************************)
RetireReplySource(owner, source) ==
  /\ owner \in ReplyOwners
  /\ source \in ReplySources
  /\ rrSourceActive[owner][source]
  /\ rrAttempts' =
       {IF attempt.owner = owner /\ attempt.source = source
        THEN [attempt EXCEPT !.ticketTenure = NoReplyTicketTenure]
        ELSE attempt: attempt \in rrAttempts}
  /\ rrSourceActive' =
       [rrSourceActive EXCEPT ![owner][source] = FALSE]
  /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                 rrConnectionTenure, rrNextServiceIndex>>

(***************************************************************************
Reconnect is a new delivery under a new connection tenure.  The new writer
has no flush continuity, so the selected attempt retries its retained current
message/chunk and must acquire a fresh admission ticket.  Other attempts are
untouched.
***************************************************************************)
ReconnectReplySource(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      deliveryOrdinal == rrNextDeliveryOrdinal[owner]
      connectionTenure == rrConnectionTenure[owner][source] + 1
      capability ==
        ReplyCapability(owner, source, source, semantic, deliveryOrdinal,
                        connectionTenure)
      routed ==
        ReplyAttemptWithRoute(oldAttempt, deliveryOrdinal,
                              connectionTenure)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ~rrSourceActive[owner][source]
     /\ connectionTenure \in ReplyConnectionTenures
     /\ deliveryOrdinal \in ReplyDeliveryOrdinals
     /\ capability.owner = owner
     /\ capability.source = source
     /\ capability.target = source
     /\ capability.semantic = semantic
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, routed)
     /\ rrConnectionTenure' =
          [rrConnectionTenure EXCEPT
             ![owner][source] = connectionTenure]
     /\ rrSourceActive' =
          [rrSourceActive EXCEPT ![owner][source] = TRUE]
     /\ rrNextDeliveryOrdinal' =
          [rrNextDeliveryOrdinal EXCEPT ![owner] = @ + 1]
     /\ UNCHANGED <<rrPayloads, rrNextServiceIndex>>

AcquireReplyTicket(owner, semantic, source) ==
  LET oldAttempt == ReplyAttemptFor(owner, semantic, source)
      ticketed == ReplyAttemptWithTicket(oldAttempt)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ source \in ReplySources
     /\ ReplyAttemptOwned(owner, semantic, source)
     /\ ReplyAttemptCurrent(oldAttempt)
     /\ oldAttempt.ticketTenure = NoReplyTicketTenure
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, ticketed)
     /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive,
                    rrNextServiceIndex>>

ServiceReplyRoute(owner, semantic) ==
  LET selectedIndex == ReplySelectedSourceIndex(owner, semantic)
      source == ReplySourceOrder[selectedIndex]
      oldAttempt == ReplyAttemptFor(owner, semantic, source)
      serviced == ReplyAttemptAfterService(oldAttempt)
  IN /\ owner \in ReplyOwners
     /\ semantic \in ReplySemantics
     /\ ReplyPendingSourceIndices(owner, semantic) # {}
     /\ rrAttempts' = ReplaceReplyAttempt(oldAttempt, serviced)
     /\ rrNextServiceIndex' =
          [rrNextServiceIndex EXCEPT
             ![owner][semantic] = NextReplySourceIndex(selectedIndex)]
     /\ UNCHANGED <<rrPayloads, rrNextDeliveryOrdinal,
                    rrConnectionTenure, rrSourceActive>>

ReplyRouteNext ==
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
  \/ \E owner \in ReplyOwners, semantic \in ReplySemantics:
       ServiceReplyRoute(owner, semantic)

ReplyRouteFairness ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       WF_ReplyRouteVars(AcquireReplyTicket(owner, semantic, source))
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       WF_ReplyRouteVars(ServiceReplyRoute(owner, semantic))

ReplyRouteSpec ==
  ReplyRouteInit
    /\ [][ReplyRouteNext]_ReplyRouteVars
    /\ ReplyRouteFairness

ReplyRouteTypeInvariant ==
  /\ rrAttempts \subseteq ReplyAttemptSet
  /\ rrPayloads \subseteq ReplySemantics
  /\ rrNextDeliveryOrdinal
       \in [ReplyOwners -> 1..(ReplyDeliveryOrdinalLimit + 1)]
  /\ rrConnectionTenure
       \in [ReplyOwners -> [ReplySources -> ReplyConnectionTenures]]
  /\ rrSourceActive \in [ReplyOwners -> [ReplySources -> BOOLEAN]]
  /\ rrNextServiceIndex
       \in [ReplyOwners -> [ReplySemantics -> 1..Len(ReplySourceOrder)]]

ReplyRouteOwnershipInvariant ==
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics,
       source \in ReplySources:
       Cardinality(ReplyAttemptsForSource(owner, semantic, source)) <= 1
  /\ \A owner \in ReplyOwners, semantic \in ReplySemantics:
       /\ Cardinality(ReplyAttemptSources(owner, semantic))
            <= ReplySourceCapacity
       /\ (ReplyAttemptsFor(owner, semantic) # {} =>
             semantic \in rrPayloads)
  /\ \A attempt \in rrAttempts:
       /\ attempt.deliveryOrdinal
            < rrNextDeliveryOrdinal[attempt.owner]
       /\ (attempt.ticketTenure # NoReplyTicketTenure =>
             /\ ReplyAttemptCurrent(attempt)
             /\ ReplyTicketValidForAttempt(attempt))

(***************************************************************************
This transition predicate is the tenure-aware retry contract consumed by both
the asynchronous composition and the mutation matrix.  Matching is semantic
and source-scoped.  A same-tenure update cannot regress its delivery ordinal or
either cursor.  A reconnect must advance both tenure and delivery ordinal,
discard its old admission ticket, and preserve its source-owned cursor.
***************************************************************************)
ReplyTenureAwareReplayStep ==
  \A oldAttempt \in rrAttempts:
    LET afterAttempts ==
          {newAttempt \in rrAttempts':
             /\ newAttempt.owner = oldAttempt.owner
             /\ newAttempt.semantic = oldAttempt.semantic
             /\ newAttempt.source = oldAttempt.source}
    IN afterAttempts # {} =>
         LET newAttempt == CHOOSE attempt \in afterAttempts: TRUE
         IN IF newAttempt.connectionTenure = oldAttempt.connectionTenure
            THEN /\ newAttempt.deliveryOrdinal
                       >= oldAttempt.deliveryOrdinal
                 /\ newAttempt.messageCursor >= oldAttempt.messageCursor
                 /\ newAttempt.chunkCursor >= oldAttempt.chunkCursor
            ELSE /\ newAttempt.connectionTenure
                       > oldAttempt.connectionTenure
                 /\ newAttempt.deliveryOrdinal
                       > oldAttempt.deliveryOrdinal
                 /\ newAttempt.ticketTenure = NoReplyTicketTenure
                 /\ ReplyAttemptCursor(newAttempt) =
                      ReplyAttemptCursor(oldAttempt)

ReplyTenureAwareReplay ==
  [][ReplyTenureAwareReplayStep]_ReplyRouteVars

(***************************************************************************
Every owned attempt survives a route transition.  When one attempt changes,
all other attempts owned by the same actor retain their independent cursors,
including other semantic requests from the same authenticated source.
***************************************************************************)
SameReplyAttemptIdentity(left, right) ==
  /\ left.owner = right.owner
  /\ left.semantic = right.semantic
  /\ left.source = right.source

ReplySourceIsolationStep ==
  /\ \A retainedBefore \in rrAttempts:
       \E retainedAfter \in rrAttempts':
         SameReplyAttemptIdentity(retainedBefore, retainedAfter)
  /\ \A changedBefore \in rrAttempts:
       \A changedAfter \in rrAttempts':
         LET sameAttempt ==
               SameReplyAttemptIdentity(changedBefore, changedAfter)
             attemptChanged == changedAfter # changedBefore
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

ReplySourceIsolation ==
  [][ReplySourceIsolationStep]_ReplyRouteVars

ReplyRouteSafetyInvariant ==
  /\ ReplyRouteTypeInvariant
  /\ ReplyRouteOwnershipInvariant

(***************************************************************************
This is an explicit liveness obligation, not a proved consequence of the
safety invariant.  The premise excludes permanent source loss, ticket loss,
and tenure churn.  Its conclusion requires a strictly larger cursor rank or
completion; merely retiring the route or invalidating its ticket cannot satisfy
the leads-to target.
***************************************************************************)
ReplySourceServiceEligible(owner, semantic, source) ==
  /\ ReplyAttemptOwned(owner, semantic, source)
  /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
     IN ReplyTicketValidForAttempt(attempt)

ReplySourceStableResponsive(owner, semantic, source) ==
  <>[] ReplySourceServiceEligible(owner, semantic, source)

ReplySourceAtCursor(owner, semantic, source, messageCursor, chunkCursor) ==
  /\ ReplySourceServiceEligible(owner, semantic, source)
  /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
     IN /\ ~ReplyAttemptComplete(attempt)
        /\ attempt.messageCursor = messageCursor
        /\ attempt.chunkCursor = chunkCursor

ReplySourceAdvancedFrom(owner, semantic, source,
                        messageCursor, chunkCursor) ==
  /\ ReplySourceServiceEligible(owner, semantic, source)
  /\ LET attempt == ReplyAttemptFor(owner, semantic, source)
         oldRank == messageCursor * (ReplyChunkCount + 1) + chunkCursor
     IN \/ ReplyAttemptComplete(attempt)
        \/ ReplyAttemptRank(attempt) > oldRank

ReplySourceEventuallyProgresses(owner, semantic, source) ==
  ReplySourceStableResponsive(owner, semantic, source) =>
    \A messageCursor \in 0..ReplyMessageCount,
       chunkCursor \in 0..ReplyChunkCount:
      ReplySourceAtCursor(owner, semantic, source,
                          messageCursor, chunkCursor)
        ~> ReplySourceAdvancedFrom(owner, semantic, source,
                                   messageCursor, chunkCursor)

=============================================================================
