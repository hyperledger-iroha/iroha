---- MODULE SumeragiFrontierNewViewCatchUpGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for active-frontier NEW_VIEW catch-up voting.

This slice models `should_emit_frontier_new_view_catch_up_vote(...)`.  The
helper lets a validator echo partial same-highest NEW_VIEW support only on the
active committed frontier, when resilience recovery is enabled, a remote group
already supports the canonical committed-tip highest QC, the local validator
has not signed that group, and the candidate view is no more than the immediate
successor of the locally tracked view.  It must fail closed for disabled
resilience, view zero, empty groups, already-signed groups, non-frontier
heights, non-canonical highest QCs, missing local view tracking, and far-future
views.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "valid_current_view",
  "valid_successor_view",
  "valid_canonical_hash_without_payload",
  "resilience_disabled",
  "view_zero",
  "empty_group",
  "local_already_signed",
  "future_height",
  "stale_height",
  "noncanonical_highest",
  "missing_current_view",
  "view_beyond_successor"
}

ResilienceEnabled(c) ==
  c # "resilience_disabled"

View(c) ==
  CASE c = "view_zero" -> 0
    [] c = "valid_current_view" -> 1
    [] c = "view_beyond_successor" -> 3
    [] OTHER -> 1

CurrentViewKnown(c) ==
  c # "missing_current_view"

CurrentView(c) ==
  CASE c = "valid_current_view" -> 1
    [] c = "view_beyond_successor" -> 1
    [] OTHER -> 0

GroupNonempty(c) ==
  c # "empty_group"

LocalAbsentFromGroup(c) ==
  c # "local_already_signed"

HeightIsCommittedFrontier(c) ==
  c # "future_height" /\ c # "stale_height"

HighestQcCanonicalTip(c) ==
  c # "noncanonical_highest"

CanonicalPayloadMissing(c) ==
  c = "valid_canonical_hash_without_payload"

ViewWithinSuccessor(c) ==
  CurrentViewKnown(c) /\ View(c) <= CurrentView(c) + 1

SpecEmit(c) ==
  /\ ResilienceEnabled(c)
  /\ View(c) # 0
  /\ GroupNonempty(c)
  /\ LocalAbsentFromGroup(c)
  /\ HeightIsCommittedFrontier(c)
  /\ HighestQcCanonicalTip(c)
  /\ CurrentViewKnown(c)
  /\ ViewWithinSuccessor(c)

ActualEmit(c) ==
  CASE Bug = "reject_valid_current_view"
       /\ c = "valid_current_view" -> FALSE
    [] Bug = "reject_valid_successor_view"
       /\ c = "valid_successor_view" -> FALSE
    [] Bug = "reject_canonical_hash_without_payload"
       /\ c = "valid_canonical_hash_without_payload" -> FALSE
    [] Bug = "accept_resilience_disabled"
       /\ c = "resilience_disabled" -> TRUE
    [] Bug = "accept_view_zero"
       /\ c = "view_zero" -> TRUE
    [] Bug = "accept_empty_group"
       /\ c = "empty_group" -> TRUE
    [] Bug = "accept_local_already_signed"
       /\ c = "local_already_signed" -> TRUE
    [] Bug = "accept_future_height"
       /\ c = "future_height" -> TRUE
    [] Bug = "accept_stale_height"
       /\ c = "stale_height" -> TRUE
    [] Bug = "accept_noncanonical_highest"
       /\ c = "noncanonical_highest" -> TRUE
    [] Bug = "accept_missing_current_view"
       /\ c = "missing_current_view" -> TRUE
    [] Bug = "accept_view_beyond_successor"
       /\ c = "view_beyond_successor" -> TRUE
    [] Bug = "require_strict_successor"
       /\ c = "valid_current_view" -> FALSE
    [] Bug = "require_payload_for_canonical"
       /\ CanonicalPayloadMissing(c) -> FALSE
    [] OTHER -> SpecEmit(c)

Matches(c) ==
  ActualEmit(c) = SpecEmit(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reject_valid_current_view",
       "reject_valid_successor_view",
       "reject_canonical_hash_without_payload",
       "accept_resilience_disabled",
       "accept_view_zero",
       "accept_empty_group",
       "accept_local_already_signed",
       "accept_future_height",
       "accept_stale_height",
       "accept_noncanonical_highest",
       "accept_missing_current_view",
       "accept_view_beyond_successor",
       "require_strict_successor",
       "require_payload_for_canonical"
     }
  /\ checked = 0

Safety ==
  \A c \in Cases: Matches(c)

CurrentViewAllowed ==
  Matches("valid_current_view")

SuccessorViewAllowed ==
  Matches("valid_successor_view")

CanonicalHashWithoutPayloadAllowed ==
  Matches("valid_canonical_hash_without_payload")

LocalOrEmptySupportRejected ==
  /\ Matches("empty_group")
  /\ Matches("local_already_signed")

FrontierAndCanonicalRequired ==
  /\ Matches("future_height")
  /\ Matches("stale_height")
  /\ Matches("noncanonical_highest")

TrackedViewRequired ==
  /\ Matches("missing_current_view")
  /\ Matches("view_beyond_successor")

=============================================================================
