---- MODULE SumeragiRbcStatusLookupGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for RBC status lookup helpers.

This slice pins `rbc_status::Handle::{is_delivered,
delivered_payload_matches, complete_payload_matches, stale_keys,
next_stale_due}` and the matching `RbcSession` payload predicates.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LookupCases == {
  "absent",
  "other_block",
  "other_height",
  "delivered_complete_match",
  "delivered_incomplete_match",
  "delivered_invalid_match",
  "delivered_missing_payload",
  "delivered_wrong_payload",
  "complete_undelivered_match",
  "wrong_view_complete",
  "complete_invalid",
  "complete_incomplete",
  "two_views_one_delivered",
  "two_views_none_delivered"
}

StaleCases == {
  "ttl_zero",
  "no_entries",
  "fresh_single",
  "boundary_age",
  "stale_single",
  "future_timestamp",
  "two_fresh_entries",
  "mixed_stale_fresh"
}

SpecIsDelivered(c) ==
  c \in {
    "delivered_complete_match",
    "delivered_incomplete_match",
    "delivered_invalid_match",
    "delivered_missing_payload",
    "delivered_wrong_payload",
    "two_views_one_delivered"
  }

ActualIsDelivered(c) ==
  CASE Bug = "is_delivered_requires_complete"
       /\ c = "delivered_incomplete_match" -> FALSE
    [] Bug = "is_delivered_checks_payload"
       /\ c = "delivered_wrong_payload" -> FALSE
    [] Bug = "is_delivered_ignores_other_view"
       /\ c = "two_views_one_delivered" -> FALSE
    [] OTHER -> SpecIsDelivered(c)

SpecDeliveredPayloadMatches(c) ==
  c \in {"delivered_complete_match", "two_views_one_delivered"}

ActualDeliveredPayloadMatches(c) ==
  CASE Bug = "delivered_accepts_incomplete"
       /\ c = "delivered_incomplete_match" -> TRUE
    [] Bug = "delivered_accepts_invalid"
       /\ c = "delivered_invalid_match" -> TRUE
    [] Bug = "delivered_accepts_missing_payload"
       /\ c = "delivered_missing_payload" -> TRUE
    [] Bug = "delivered_accepts_wrong_payload"
       /\ c = "delivered_wrong_payload" -> TRUE
    [] Bug = "delivered_requires_exact_view"
       /\ c = "two_views_one_delivered" -> FALSE
    [] OTHER -> SpecDeliveredPayloadMatches(c)

SpecCompletePayloadMatches(c) ==
  c \in {"delivered_complete_match", "complete_undelivered_match"}

ActualCompletePayloadMatches(c) ==
  CASE Bug = "complete_requires_delivered"
       /\ c = "complete_undelivered_match" -> FALSE
    [] Bug = "complete_accepts_wrong_view"
       /\ c = "wrong_view_complete" -> TRUE
    [] Bug = "complete_accepts_invalid"
       /\ c = "complete_invalid" -> TRUE
    [] Bug = "complete_accepts_incomplete"
       /\ c = "complete_incomplete" -> TRUE
    [] Bug = "complete_accepts_wrong_payload"
       /\ c = "delivered_wrong_payload" -> TRUE
    [] OTHER -> SpecCompletePayloadMatches(c)

SpecStaleKeysNonEmpty(c) ==
  c \in {"stale_single", "mixed_stale_fresh"}

ActualStaleKeysNonEmpty(c) ==
  CASE Bug = "stale_zero_ttl_expires"
       /\ c = "ttl_zero" -> TRUE
    [] Bug = "stale_boundary_expires"
       /\ c = "boundary_age" -> TRUE
    [] Bug = "stale_future_underflows"
       /\ c = "future_timestamp" -> TRUE
    [] OTHER -> SpecStaleKeysNonEmpty(c)

SpecNextDue(c) ==
  CASE c \in {"ttl_zero", "no_entries"} -> "none"
    [] c \in {"boundary_age", "stale_single", "mixed_stale_fresh"} -> "zero"
    [] c = "fresh_single" -> "remaining_five"
    [] c = "future_timestamp" -> "ttl"
    [] OTHER -> "remaining_two"

ActualNextDue(c) ==
  CASE Bug = "due_zero_ttl_reports_zero"
       /\ c = "ttl_zero" -> "zero"
    [] Bug = "due_empty_reports_zero"
       /\ c = "no_entries" -> "zero"
    [] Bug = "due_boundary_reports_ttl"
       /\ c = "boundary_age" -> "ttl"
    [] Bug = "due_uses_latest_remaining"
       /\ c = "two_fresh_entries" -> "remaining_five"
    [] Bug = "due_future_underflows_zero"
       /\ c = "future_timestamp" -> "zero"
    [] OTHER -> SpecNextDue(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "is_delivered_requires_complete",
       "is_delivered_checks_payload",
       "is_delivered_ignores_other_view",
       "delivered_accepts_incomplete",
       "delivered_accepts_invalid",
       "delivered_accepts_missing_payload",
       "delivered_accepts_wrong_payload",
       "delivered_requires_exact_view",
       "complete_requires_delivered",
       "complete_accepts_wrong_view",
       "complete_accepts_invalid",
       "complete_accepts_incomplete",
       "complete_accepts_wrong_payload",
       "stale_zero_ttl_expires",
       "stale_boundary_expires",
       "stale_future_underflows",
       "due_zero_ttl_reports_zero",
       "due_empty_reports_zero",
       "due_boundary_reports_ttl",
       "due_uses_latest_remaining",
       "due_future_underflows_zero"
     }
  /\ checked = 0

RbcStatusLookupMatchesSpec ==
  /\ \A c \in LookupCases:
       /\ ActualIsDelivered(c) = SpecIsDelivered(c)
       /\ ActualDeliveredPayloadMatches(c) = SpecDeliveredPayloadMatches(c)
       /\ ActualCompletePayloadMatches(c) = SpecCompletePayloadMatches(c)
  /\ \A c \in StaleCases:
       /\ ActualStaleKeysNonEmpty(c) = SpecStaleKeysNonEmpty(c)
       /\ ActualNextDue(c) = SpecNextDue(c)

SafetyFast ==
  RbcStatusLookupMatchesSpec

BugIsDeliveredRequiresComplete ==
  ActualIsDelivered("delivered_incomplete_match") =
    SpecIsDelivered("delivered_incomplete_match")

BugIsDeliveredChecksPayload ==
  ActualIsDelivered("delivered_wrong_payload") =
    SpecIsDelivered("delivered_wrong_payload")

BugIsDeliveredIgnoresOtherView ==
  ActualIsDelivered("two_views_one_delivered") =
    SpecIsDelivered("two_views_one_delivered")

BugDeliveredAcceptsIncomplete ==
  ActualDeliveredPayloadMatches("delivered_incomplete_match") =
    SpecDeliveredPayloadMatches("delivered_incomplete_match")

BugDeliveredAcceptsInvalid ==
  ActualDeliveredPayloadMatches("delivered_invalid_match") =
    SpecDeliveredPayloadMatches("delivered_invalid_match")

BugDeliveredAcceptsMissingPayload ==
  ActualDeliveredPayloadMatches("delivered_missing_payload") =
    SpecDeliveredPayloadMatches("delivered_missing_payload")

BugDeliveredAcceptsWrongPayload ==
  ActualDeliveredPayloadMatches("delivered_wrong_payload") =
    SpecDeliveredPayloadMatches("delivered_wrong_payload")

BugDeliveredRequiresExactView ==
  ActualDeliveredPayloadMatches("two_views_one_delivered") =
    SpecDeliveredPayloadMatches("two_views_one_delivered")

BugCompleteRequiresDelivered ==
  ActualCompletePayloadMatches("complete_undelivered_match") =
    SpecCompletePayloadMatches("complete_undelivered_match")

BugCompleteAcceptsWrongView ==
  ActualCompletePayloadMatches("wrong_view_complete") =
    SpecCompletePayloadMatches("wrong_view_complete")

BugCompleteAcceptsInvalid ==
  ActualCompletePayloadMatches("complete_invalid") =
    SpecCompletePayloadMatches("complete_invalid")

BugCompleteAcceptsIncomplete ==
  ActualCompletePayloadMatches("complete_incomplete") =
    SpecCompletePayloadMatches("complete_incomplete")

BugCompleteAcceptsWrongPayload ==
  ActualCompletePayloadMatches("delivered_wrong_payload") =
    SpecCompletePayloadMatches("delivered_wrong_payload")

BugStaleZeroTtlExpires ==
  ActualStaleKeysNonEmpty("ttl_zero") = SpecStaleKeysNonEmpty("ttl_zero")

BugStaleBoundaryExpires ==
  ActualStaleKeysNonEmpty("boundary_age") =
    SpecStaleKeysNonEmpty("boundary_age")

BugStaleFutureUnderflows ==
  ActualStaleKeysNonEmpty("future_timestamp") =
    SpecStaleKeysNonEmpty("future_timestamp")

BugDueZeroTtlReportsZero ==
  ActualNextDue("ttl_zero") = SpecNextDue("ttl_zero")

BugDueEmptyReportsZero ==
  ActualNextDue("no_entries") = SpecNextDue("no_entries")

BugDueBoundaryReportsTtl ==
  ActualNextDue("boundary_age") = SpecNextDue("boundary_age")

BugDueUsesLatestRemaining ==
  ActualNextDue("two_fresh_entries") = SpecNextDue("two_fresh_entries")

BugDueFutureUnderflowsZero ==
  ActualNextDue("future_timestamp") = SpecNextDue("future_timestamp")

====
