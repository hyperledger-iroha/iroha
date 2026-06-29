---- MODULE SumeragiRbcPayloadHydrationGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for RBC payload hydration.

This slice pins `maybe_hydrate_rbc_session_from_local_payload(...)` and
`apply_hydrated_payload(...)` around malformed live chunk counters:
- invalid sessions and already complete positive-shape sessions do not hydrate;
- incomplete, zero-total, and over-counted sessions do try authoritative local
  payload hydration;
- zero-total metadata adopts the deterministic positive chunk count from the
  exact local payload instead of staying terminal; and
- empty payloads, payload-hash mismatches, zero-total digest/root mismatches,
  and nonzero count mismatches still fail closed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InvalidSession == "invalid_session"
AlreadyComplete == "already_complete"
IncompletePositive == "incomplete_positive"
ZeroTotalLocalPayload == "zero_total_local_payload"
OvercountLocalPayload == "overcount_local_payload"
ZeroTotalDigestMismatch == "zero_total_digest_mismatch"
ZeroTotalRootMismatch == "zero_total_root_mismatch"
EmptyPayload == "empty_payload"
HashMismatch == "hash_mismatch"
NonzeroCountMismatch == "nonzero_count_mismatch"

Cases == {
  InvalidSession,
  AlreadyComplete,
  IncompletePositive,
  ZeroTotalLocalPayload,
  OvercountLocalPayload,
  ZeroTotalDigestMismatch,
  ZeroTotalRootMismatch,
  EmptyPayload,
  HashMismatch,
  NonzeroCountMismatch
}

HydrateAttempt == 1
Skipped == 2
Updated == 3
Invalidated == 4
LayoutMismatch == 5
PayloadHashMismatch == 6
AdoptObservedTotal == 7
RecountReceived == 8
AllChunksPresent == 9
PositiveShape == 10
ChunkDigestMismatch == 11
ChunkRootMismatch == 12

ActionUniverse == 1..12

SpecActions(c) ==
  CASE c = InvalidSession ->
      {Skipped}
    [] c = AlreadyComplete ->
      {Skipped, AllChunksPresent, PositiveShape}
    [] c = IncompletePositive ->
      {HydrateAttempt, Updated, RecountReceived, AllChunksPresent,
       PositiveShape}
    [] c = ZeroTotalLocalPayload ->
      {HydrateAttempt, Updated, AdoptObservedTotal, RecountReceived,
       AllChunksPresent, PositiveShape}
    [] c = OvercountLocalPayload ->
      {HydrateAttempt, Updated, RecountReceived, AllChunksPresent,
       PositiveShape}
    [] c = ZeroTotalDigestMismatch ->
      {HydrateAttempt, Updated, AdoptObservedTotal, Invalidated,
       ChunkDigestMismatch}
    [] c = ZeroTotalRootMismatch ->
      {HydrateAttempt, Updated, AdoptObservedTotal, RecountReceived,
       AllChunksPresent, PositiveShape, Invalidated, ChunkRootMismatch}
    [] c = EmptyPayload ->
      {HydrateAttempt, Updated, Invalidated, LayoutMismatch}
    [] c = HashMismatch ->
      {HydrateAttempt, Updated, Invalidated, PayloadHashMismatch}
    [] c = NonzeroCountMismatch ->
      {HydrateAttempt, Updated, Invalidated, LayoutMismatch}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "empty_payload_accepted" /\ c = EmptyPayload ->
      (spec \ {Invalidated, LayoutMismatch}) \cup
        {AllChunksPresent, PositiveShape}
    [] Bug = "hash_mismatch_accepted" /\ c = HashMismatch ->
      (spec \ {Invalidated, PayloadHashMismatch}) \cup
        {AllChunksPresent, PositiveShape}
    [] Bug = "zero_total_rejected" /\ c = ZeroTotalLocalPayload ->
      (spec \ {AdoptObservedTotal, RecountReceived, AllChunksPresent,
               PositiveShape}) \cup {Invalidated, LayoutMismatch}
    [] Bug = "zero_total_helper_skips" /\ c = ZeroTotalLocalPayload ->
      {Skipped}
    [] Bug = "overcount_helper_skips" /\ c = OvercountLocalPayload ->
      {Skipped}
    [] Bug = "overcount_keeps_received" /\ c = OvercountLocalPayload ->
      spec \ {RecountReceived, AllChunksPresent, PositiveShape}
    [] Bug = "zero_total_digest_mismatch_accepted" /\
       c = ZeroTotalDigestMismatch ->
      (spec \ {Invalidated, ChunkDigestMismatch}) \cup
        {RecountReceived, AllChunksPresent, PositiveShape}
    [] Bug = "zero_total_root_mismatch_accepted" /\
       c = ZeroTotalRootMismatch ->
      spec \ {Invalidated, ChunkRootMismatch}
    [] Bug = "count_mismatch_accepted" /\ c = NonzeroCountMismatch ->
      (spec \ {Invalidated, LayoutMismatch}) \cup
        {AllChunksPresent, PositiveShape}
    [] OTHER -> spec

Bugs == {
  "none",
  "empty_payload_accepted",
  "hash_mismatch_accepted",
  "zero_total_rejected",
  "zero_total_helper_skips",
  "overcount_helper_skips",
  "overcount_keeps_received",
  "zero_total_digest_mismatch_accepted",
  "zero_total_root_mismatch_accepted",
  "count_mismatch_accepted"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

RbcPayloadHydrationMatchesSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

HydrationSafetyAnchors ==
  /\ ImplementationActions(InvalidSession) = {Skipped}
  /\ ImplementationActions(AlreadyComplete) =
       {Skipped, AllChunksPresent, PositiveShape}
  /\ HydrateAttempt \in ImplementationActions(IncompletePositive)
  /\ HydrateAttempt \in ImplementationActions(ZeroTotalLocalPayload)
  /\ AdoptObservedTotal \in ImplementationActions(ZeroTotalLocalPayload)
  /\ PositiveShape \in ImplementationActions(ZeroTotalLocalPayload)
  /\ HydrateAttempt \in ImplementationActions(OvercountLocalPayload)
  /\ RecountReceived \in ImplementationActions(OvercountLocalPayload)
  /\ Invalidated \in ImplementationActions(ZeroTotalDigestMismatch)
  /\ ChunkDigestMismatch \in ImplementationActions(ZeroTotalDigestMismatch)
  /\ Invalidated \in ImplementationActions(ZeroTotalRootMismatch)
  /\ ChunkRootMismatch \in ImplementationActions(ZeroTotalRootMismatch)
  /\ Invalidated \in ImplementationActions(EmptyPayload)
  /\ Invalidated \in ImplementationActions(HashMismatch)
  /\ Invalidated \in ImplementationActions(NonzeroCountMismatch)

RbcPayloadHydrationExactness ==
  /\ RbcPayloadHydrationMatchesSpec
  /\ HydrationSafetyAnchors

RbcPayloadHydrationCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcPayloadHydrationExactness

SafetyFast ==
  RbcPayloadHydrationExactness

====
