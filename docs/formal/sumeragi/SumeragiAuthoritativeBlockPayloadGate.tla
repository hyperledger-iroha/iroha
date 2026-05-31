---- MODULE SumeragiAuthoritativeBlockPayloadGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for hash-level authoritative payload availability.

This slice captures `authoritative_block_payload_available(...)`. The helper
first asks `with_authoritative_payload_for_progress(hash)`; a valid local
pending owner, valid commit-inflight owner, or committed Kura block for the
requested hash short-circuits. If that local lookup returns missing because the
local owner is rejected or absent, the helper scans RBC sessions and accepts any
session whose key hash matches and whose RBC progress predicate has
authoritative payload material. Wrong-hash and non-authoritative RBC sessions do
not count, but they also do not block later matching sessions in the scan.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LocalPending == "local_pending"
LocalInflight == "local_inflight"
LocalKuraCommitted == "local_kura_committed"
LocalRejectedPending == "local_rejected_pending"
LocalRejectedInflight == "local_rejected_inflight"
LocalRejectedKura == "local_rejected_kura"
LocalRejectedPendingWithRbc == "local_rejected_pending_with_rbc"
LocalRejectedInflightWithRbc == "local_rejected_inflight_with_rbc"
LocalRejectedKuraWithRbc == "local_rejected_kura_with_rbc"
RbcMatchingAuthoritative == "rbc_matching_authoritative"
RbcWrongHashAuthoritative == "rbc_wrong_hash_authoritative"
RbcNonAuthoritative == "rbc_non_authoritative"
RbcWrongThenMatching == "rbc_wrong_then_matching"
RbcNonAuthoritativeThenMatching == "rbc_non_authoritative_then_matching"
AbsentPayload == "absent_payload"

Cases == {
  LocalPending,
  LocalInflight,
  LocalKuraCommitted,
  LocalRejectedPending,
  LocalRejectedInflight,
  LocalRejectedKura,
  LocalRejectedPendingWithRbc,
  LocalRejectedInflightWithRbc,
  LocalRejectedKuraWithRbc,
  RbcMatchingAuthoritative,
  RbcWrongHashAuthoritative,
  RbcNonAuthoritative,
  RbcWrongThenMatching,
  RbcNonAuthoritativeThenMatching,
  AbsentPayload
}

LocalAuthoritativeCases == {
  LocalPending,
  LocalInflight,
  LocalKuraCommitted
}

LocalRejectedCases == {
  LocalRejectedPending,
  LocalRejectedInflight,
  LocalRejectedKura,
  LocalRejectedPendingWithRbc,
  LocalRejectedInflightWithRbc,
  LocalRejectedKuraWithRbc
}

RbcMatchingCases == {
  LocalRejectedPendingWithRbc,
  LocalRejectedInflightWithRbc,
  LocalRejectedKuraWithRbc,
  RbcMatchingAuthoritative,
  RbcWrongThenMatching,
  RbcNonAuthoritativeThenMatching
}

RbcWrongHashCases == {
  RbcWrongHashAuthoritative,
  RbcWrongThenMatching
}

RbcNonAuthoritativeCases == {
  RbcNonAuthoritative,
  RbcNonAuthoritativeThenMatching
}

LocalAuthoritative(c) ==
  c \in LocalAuthoritativeCases

RbcAuthoritative(c) ==
  c \in RbcMatchingCases

SpecResult(c) ==
  LocalAuthoritative(c) \/ RbcAuthoritative(c)

ReturnTrue == 1
ReturnFalse == 2
CheckLocal == 3
CheckRbcSessions == 4
LocalPendingAccepted == 5
LocalInflightAccepted == 6
LocalKuraAccepted == 7
LocalRejected == 8
RbcWrongHashIgnored == 9
RbcNonAuthoritativeRejected == 10
RbcAuthoritativeAccepted == 11
RbcNoMatch == 12

ActionUniverse == 1..12

LocalAction(c) ==
  CASE c = LocalPending -> {LocalPendingAccepted}
    [] c = LocalInflight -> {LocalInflightAccepted}
    [] c = LocalKuraCommitted -> {LocalKuraAccepted}
    [] c \in LocalRejectedCases -> {LocalRejected}
    [] OTHER -> {}

RbcAction(c) ==
  (IF c \in RbcWrongHashCases THEN {RbcWrongHashIgnored} ELSE {})
    \cup (IF c \in RbcNonAuthoritativeCases
          THEN {RbcNonAuthoritativeRejected}
          ELSE {})
    \cup (IF c \in RbcMatchingCases THEN {RbcAuthoritativeAccepted} ELSE {})
    \cup (IF c \notin RbcWrongHashCases \cup RbcNonAuthoritativeCases
              \cup RbcMatchingCases
          THEN {RbcNoMatch}
          ELSE {})

SpecActions(c) ==
  {CheckLocal}
    \cup (IF SpecResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})
    \cup LocalAction(c)
    \cup (IF LocalAuthoritative(c) THEN {} ELSE {CheckRbcSessions})
    \cup (IF LocalAuthoritative(c) THEN {} ELSE RbcAction(c))

ImplementationResult(c) ==
  CASE Bug = "reject_valid_pending"
       /\ c = LocalPending ->
      FALSE
    [] Bug = "reject_valid_inflight"
       /\ c = LocalInflight ->
      FALSE
    [] Bug = "reject_committed_kura"
       /\ c = LocalKuraCommitted ->
      FALSE
    [] Bug = "accept_rejected_pending"
       /\ c = LocalRejectedPending ->
      TRUE
    [] Bug = "accept_rejected_inflight"
       /\ c = LocalRejectedInflight ->
      TRUE
    [] Bug = "accept_rejected_kura"
       /\ c = LocalRejectedKura ->
      TRUE
    [] Bug = "local_rejection_blocks_rbc"
       /\ c \in {LocalRejectedPendingWithRbc,
                 LocalRejectedInflightWithRbc,
                 LocalRejectedKuraWithRbc} ->
      FALSE
    [] Bug = "skip_rbc_scan"
       /\ c = RbcMatchingAuthoritative ->
      FALSE
    [] Bug = "reject_matching_rbc"
       /\ c = RbcMatchingAuthoritative ->
      FALSE
    [] Bug = "first_rbc_miss_blocks_later_match"
       /\ c \in {RbcWrongThenMatching, RbcNonAuthoritativeThenMatching} ->
      FALSE
    [] Bug = "accept_wrong_hash_rbc"
       /\ c = RbcWrongHashAuthoritative ->
      TRUE
    [] Bug = "accept_non_authoritative_rbc"
       /\ c = RbcNonAuthoritative ->
      TRUE
    [] Bug = "accept_absent_payload"
       /\ c = AbsentPayload ->
      TRUE
    [] OTHER -> SpecResult(c)

ImplementationActions(c) ==
  (SpecActions(c) \ {ReturnTrue, ReturnFalse})
    \cup (IF ImplementationResult(c) THEN {ReturnTrue} ELSE {ReturnFalse})

Bugs == {
  "none",
  "reject_valid_pending",
  "reject_valid_inflight",
  "reject_committed_kura",
  "accept_rejected_pending",
  "accept_rejected_inflight",
  "accept_rejected_kura",
  "local_rejection_blocks_rbc",
  "skip_rbc_scan",
  "reject_matching_rbc",
  "first_rbc_miss_blocks_later_match",
  "accept_wrong_hash_rbc",
  "accept_non_authoritative_rbc",
  "accept_absent_payload"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecResult(c) \in BOOLEAN
       /\ ImplementationResult(c) \in BOOLEAN
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ResultMatchesSpec ==
  \A c \in Cases:
    ImplementationResult(c) = SpecResult(c)

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

LocalAuthoritativePayloadsShortCircuit ==
  /\ ImplementationResult(LocalPending) = TRUE
  /\ LocalPendingAccepted \in ImplementationActions(LocalPending)
  /\ ~(CheckRbcSessions \in ImplementationActions(LocalPending))
  /\ ImplementationResult(LocalInflight) = TRUE
  /\ LocalInflightAccepted \in ImplementationActions(LocalInflight)
  /\ ~(CheckRbcSessions \in ImplementationActions(LocalInflight))
  /\ ImplementationResult(LocalKuraCommitted) = TRUE
  /\ LocalKuraAccepted \in ImplementationActions(LocalKuraCommitted)
  /\ ~(CheckRbcSessions \in ImplementationActions(LocalKuraCommitted))

RejectedLocalOwnersNeedRbcFallback ==
  /\ ImplementationResult(LocalRejectedPending) = FALSE
  /\ ImplementationResult(LocalRejectedInflight) = FALSE
  /\ ImplementationResult(LocalRejectedKura) = FALSE
  /\ LocalRejected \in ImplementationActions(LocalRejectedPending)
  /\ LocalRejected \in ImplementationActions(LocalRejectedInflight)
  /\ LocalRejected \in ImplementationActions(LocalRejectedKura)
  /\ CheckRbcSessions \in ImplementationActions(LocalRejectedPending)
  /\ CheckRbcSessions \in ImplementationActions(LocalRejectedInflight)
  /\ CheckRbcSessions \in ImplementationActions(LocalRejectedKura)
  /\ ImplementationResult(LocalRejectedPendingWithRbc) = TRUE
  /\ ImplementationResult(LocalRejectedInflightWithRbc) = TRUE
  /\ ImplementationResult(LocalRejectedKuraWithRbc) = TRUE
  /\ LocalRejected \in ImplementationActions(LocalRejectedPendingWithRbc)
  /\ LocalRejected \in ImplementationActions(LocalRejectedInflightWithRbc)
  /\ LocalRejected \in ImplementationActions(LocalRejectedKuraWithRbc)
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(LocalRejectedPendingWithRbc)
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(LocalRejectedInflightWithRbc)
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(LocalRejectedKuraWithRbc)

RbcHashAndAuthorityFilter ==
  /\ ImplementationResult(RbcMatchingAuthoritative) = TRUE
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(RbcMatchingAuthoritative)
  /\ ImplementationResult(RbcWrongHashAuthoritative) = FALSE
  /\ RbcWrongHashIgnored
       \in ImplementationActions(RbcWrongHashAuthoritative)
  /\ ImplementationResult(RbcNonAuthoritative) = FALSE
  /\ RbcNonAuthoritativeRejected
       \in ImplementationActions(RbcNonAuthoritative)
  /\ ImplementationResult(RbcWrongThenMatching) = TRUE
  /\ RbcWrongHashIgnored \in ImplementationActions(RbcWrongThenMatching)
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(RbcWrongThenMatching)
  /\ ImplementationResult(RbcNonAuthoritativeThenMatching) = TRUE
  /\ RbcNonAuthoritativeRejected
       \in ImplementationActions(RbcNonAuthoritativeThenMatching)
  /\ RbcAuthoritativeAccepted
       \in ImplementationActions(RbcNonAuthoritativeThenMatching)

AbsentPayloadRemainsMissing ==
  /\ ImplementationResult(AbsentPayload) = FALSE
  /\ RbcNoMatch \in ImplementationActions(AbsentPayload)

LookupShapeMatchesShortCircuit ==
  /\ \A c \in Cases:
       CheckLocal \in ImplementationActions(c)
  /\ \A c \in LocalAuthoritativeCases:
       ~(CheckRbcSessions \in ImplementationActions(c))
  /\ \A c \in Cases \ LocalAuthoritativeCases:
       CheckRbcSessions \in ImplementationActions(c)

NoBugInvariant ==
  /\ ResultMatchesSpec
  /\ ActionsMatchSpec
  /\ LocalAuthoritativePayloadsShortCircuit
  /\ RejectedLocalOwnersNeedRbcFallback
  /\ RbcHashAndAuthorityFilter
  /\ AbsentPayloadRemainsMissing
  /\ LookupShapeMatchesShortCircuit

SafetyFast == NoBugInvariant

====
