---- MODULE SumeragiRbcRecoveryHelperGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for RBC recovery helper decisions.

This slice pins `rbc_message_committed(...)` and
`rbc_session_needs_payload(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

CommittedCases == {
  "below_tip_absent",
  "at_tip_absent",
  "above_tip_present",
  "above_tip_absent"
}

PayloadCases == {
  "invalid_incomplete_match",
  "invalid_complete_match",
  "delivered_complete_match",
  "delivered_incomplete_match",
  "delivered_wrong_payload",
  "complete_undelivered_match",
  "complete_undelivered_wrong_payload",
  "incomplete_undelivered_match",
  "missing_payload_complete",
  "missing_payload_incomplete",
  "zero_chunk_complete_match"
}

SpecCommitted(c) ==
  c \in {"below_tip_absent", "at_tip_absent", "above_tip_present"}

ActualCommitted(c) ==
  CASE Bug = "committed_uses_strict_less"
       /\ c = "at_tip_absent" -> FALSE
    [] Bug = "committed_requires_kura_for_stale"
       /\ c = "below_tip_absent" -> FALSE
    [] Bug = "committed_ignores_kura"
       /\ c = "above_tip_present" -> FALSE
    [] Bug = "committed_accepts_future_absent"
       /\ c = "above_tip_absent" -> TRUE
    [] OTHER -> SpecCommitted(c)

SpecNeedsPayload(c) ==
  CASE c \in {"invalid_incomplete_match", "invalid_complete_match"} -> FALSE
    [] c = "delivered_complete_match" -> FALSE
    [] c = "complete_undelivered_match" -> FALSE
    [] OTHER -> TRUE

ActualNeedsPayload(c) ==
  CASE Bug = "needs_invalid_fetches"
       /\ c = "invalid_incomplete_match" -> TRUE
    [] Bug = "needs_complete_invalid_fetches"
       /\ c = "invalid_complete_match" -> TRUE
    [] Bug = "needs_delivered_complete_fetches"
       /\ c = "delivered_complete_match" -> TRUE
    [] Bug = "needs_delivered_incomplete_skips"
       /\ c = "delivered_incomplete_match" -> FALSE
    [] Bug = "needs_delivered_wrong_payload_skips"
       /\ c = "delivered_wrong_payload" -> FALSE
    [] Bug = "needs_complete_undelivered_fetches"
       /\ c = "complete_undelivered_match" -> TRUE
    [] Bug = "needs_wrong_payload_skips"
       /\ c = "complete_undelivered_wrong_payload" -> FALSE
    [] Bug = "needs_incomplete_match_skips"
       /\ c = "incomplete_undelivered_match" -> FALSE
    [] Bug = "needs_missing_payload_skips"
       /\ c = "missing_payload_complete" -> FALSE
    [] Bug = "needs_zero_chunk_skips"
       /\ c = "zero_chunk_complete_match" -> FALSE
    [] OTHER -> SpecNeedsPayload(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 14
     /\ checked' = checked + 1
  \/ /\ checked = 14
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "committed_uses_strict_less",
       "committed_requires_kura_for_stale",
       "committed_ignores_kura",
       "committed_accepts_future_absent",
       "needs_invalid_fetches",
       "needs_complete_invalid_fetches",
       "needs_delivered_complete_fetches",
       "needs_delivered_incomplete_skips",
       "needs_delivered_wrong_payload_skips",
       "needs_complete_undelivered_fetches",
       "needs_wrong_payload_skips",
       "needs_incomplete_match_skips",
       "needs_missing_payload_skips",
       "needs_zero_chunk_skips"
     }
  /\ checked \in 0..14

SafetyFast ==
  /\ \A c \in CommittedCases:
       ActualCommitted(c) = SpecCommitted(c)
  /\ \A c \in PayloadCases:
       ActualNeedsPayload(c) = SpecNeedsPayload(c)

AllCommittedMatches ==
  \A c \in CommittedCases:
    ActualCommitted(c) = SpecCommitted(c)

AllPayloadNeedsMatch ==
  \A c \in PayloadCases:
    ActualNeedsPayload(c) = SpecNeedsPayload(c)

CommittedAnchors ==
  /\ ActualCommitted("below_tip_absent")
  /\ ActualCommitted("at_tip_absent")
  /\ ActualCommitted("above_tip_present")
  /\ ~ActualCommitted("above_tip_absent")

PayloadSkipAnchors ==
  /\ ~ActualNeedsPayload("invalid_incomplete_match")
  /\ ~ActualNeedsPayload("invalid_complete_match")
  /\ ~ActualNeedsPayload("delivered_complete_match")
  /\ ~ActualNeedsPayload("complete_undelivered_match")

PayloadFetchAnchors ==
  /\ ActualNeedsPayload("delivered_incomplete_match")
  /\ ActualNeedsPayload("delivered_wrong_payload")
  /\ ActualNeedsPayload("complete_undelivered_wrong_payload")
  /\ ActualNeedsPayload("incomplete_undelivered_match")
  /\ ActualNeedsPayload("missing_payload_complete")
  /\ ActualNeedsPayload("missing_payload_incomplete")
  /\ ActualNeedsPayload("zero_chunk_complete_match")

RecoveryHelperSafetyAnchors ==
  /\ AllCommittedMatches
  /\ AllPayloadNeedsMatch
  /\ CommittedAnchors
  /\ PayloadSkipAnchors
  /\ PayloadFetchAnchors

BugCommittedUsesStrictLess ==
  ActualCommitted("at_tip_absent") = SpecCommitted("at_tip_absent")

BugCommittedRequiresKuraForStale ==
  ActualCommitted("below_tip_absent") = SpecCommitted("below_tip_absent")

BugCommittedIgnoresKura ==
  ActualCommitted("above_tip_present") = SpecCommitted("above_tip_present")

BugCommittedAcceptsFutureAbsent ==
  ActualCommitted("above_tip_absent") = SpecCommitted("above_tip_absent")

BugNeedsInvalidFetches ==
  ActualNeedsPayload("invalid_incomplete_match") =
    SpecNeedsPayload("invalid_incomplete_match")

BugNeedsCompleteInvalidFetches ==
  ActualNeedsPayload("invalid_complete_match") =
    SpecNeedsPayload("invalid_complete_match")

BugNeedsDeliveredCompleteFetches ==
  ActualNeedsPayload("delivered_complete_match") =
    SpecNeedsPayload("delivered_complete_match")

BugNeedsDeliveredIncompleteSkips ==
  ActualNeedsPayload("delivered_incomplete_match") =
    SpecNeedsPayload("delivered_incomplete_match")

BugNeedsDeliveredWrongPayloadSkips ==
  ActualNeedsPayload("delivered_wrong_payload") =
    SpecNeedsPayload("delivered_wrong_payload")

BugNeedsCompleteUndeliveredFetches ==
  ActualNeedsPayload("complete_undelivered_match") =
    SpecNeedsPayload("complete_undelivered_match")

BugNeedsWrongPayloadSkips ==
  ActualNeedsPayload("complete_undelivered_wrong_payload") =
    SpecNeedsPayload("complete_undelivered_wrong_payload")

BugNeedsIncompleteMatchSkips ==
  ActualNeedsPayload("incomplete_undelivered_match") =
    SpecNeedsPayload("incomplete_undelivered_match")

BugNeedsMissingPayloadSkips ==
  ActualNeedsPayload("missing_payload_complete") =
    SpecNeedsPayload("missing_payload_complete")

BugNeedsZeroChunkFetches ==
  ActualNeedsPayload("zero_chunk_complete_match") =
    SpecNeedsPayload("zero_chunk_complete_match")

====
