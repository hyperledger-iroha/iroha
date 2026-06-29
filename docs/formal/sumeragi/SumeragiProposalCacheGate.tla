---- MODULE SumeragiProposalCacheGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `ProposalCache`.

The cache stores proposal hints and proposal metadata under the same
`(height, view)` key space, but enforces the configured limit independently for
each map. Eviction removes the lowest key from the overflowing map, then keeps
`observed_at` only for keys that still have either a hint or a proposal.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  limit,
  \* @type: Int;
  final_hint_count,
  \* @type: Int;
  final_proposal_count,
  \* @type: Int;
  observed_count,
  \* @type: Int;
  evicted_count,
  \* @type: Bool;
  inserted_present,
  \* @type: Bool;
  inserted_observed,
  \* @type: Bool;
  lowest_hint_present,
  \* @type: Bool;
  lowest_proposal_present,
  \* @type: Bool;
  lowest_observed,
  \* @type: Bool;
  target_hint_present,
  \* @type: Bool;
  target_proposal_present,
  \* @type: Bool;
  target_observed,
  \* @type: Bool;
  observed_retained_for_other_kind,
  \* @type: Bool;
  prune_removed_committed,
  \* @type: Bool;
  prune_kept_future

\* @type: <<Str, Int, Int, Int, Int, Int, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate, limit, final_hint_count, final_proposal_count, observed_count,
    evicted_count, inserted_present, inserted_observed, lowest_hint_present,
    lowest_proposal_present, lowest_observed, target_hint_present,
    target_proposal_present, target_observed, observed_retained_for_other_kind,
    prune_removed_committed, prune_kept_future>>

Cases == {
  "new_empty",
  "insert_hint_under_limit",
  "insert_hint_over_limit",
  "insert_hint_limit_zero",
  "replace_existing_hint",
  "insert_proposal_under_limit",
  "insert_proposal_over_limit",
  "insert_proposal_limit_zero",
  "pop_hint_only",
  "pop_hint_keeps_observed_for_proposal",
  "pop_proposal_only",
  "pop_missing",
  "evict_hint_keeps_observed_for_proposal",
  "prune_height_leq",
  "prune_noop"
}

CountValues == 0..8

LimitFor(c) ==
  CASE c \in {"insert_hint_limit_zero", "insert_proposal_limit_zero"} -> 0
    [] c \in {"insert_proposal_under_limit", "insert_proposal_over_limit",
              "insert_proposal_limit_zero"} -> 1
    [] c = "new_empty" -> 2
    [] OTHER -> 2

SpecFinalHintCount(c) ==
  CASE c = "new_empty" -> 0
    [] c = "insert_hint_under_limit" -> 2
    [] c = "insert_hint_over_limit" -> 2
    [] c = "insert_hint_limit_zero" -> 0
    [] c = "replace_existing_hint" -> 1
    [] c = "insert_proposal_under_limit" -> 0
    [] c = "insert_proposal_over_limit" -> 0
    [] c = "insert_proposal_limit_zero" -> 0
    [] c = "pop_hint_only" -> 0
    [] c = "pop_hint_keeps_observed_for_proposal" -> 0
    [] c = "pop_proposal_only" -> 0
    [] c = "pop_missing" -> 1
    [] c = "evict_hint_keeps_observed_for_proposal" -> 2
    [] c = "prune_height_leq" -> 1
    [] c = "prune_noop" -> 1

SpecFinalProposalCount(c) ==
  CASE c = "new_empty" -> 0
    [] c = "insert_hint_under_limit" -> 0
    [] c = "insert_hint_over_limit" -> 0
    [] c = "insert_hint_limit_zero" -> 0
    [] c = "replace_existing_hint" -> 0
    [] c = "insert_proposal_under_limit" -> 1
    [] c = "insert_proposal_over_limit" -> 1
    [] c = "insert_proposal_limit_zero" -> 0
    [] c = "pop_hint_only" -> 0
    [] c = "pop_hint_keeps_observed_for_proposal" -> 1
    [] c = "pop_proposal_only" -> 0
    [] c = "pop_missing" -> 0
    [] c = "evict_hint_keeps_observed_for_proposal" -> 1
    [] c = "prune_height_leq" -> 1
    [] c = "prune_noop" -> 1

SpecObservedCount(c) ==
  CASE c = "new_empty" -> 0
    [] c = "insert_hint_under_limit" -> 2
    [] c = "insert_hint_over_limit" -> 2
    [] c = "insert_hint_limit_zero" -> 0
    [] c = "replace_existing_hint" -> 1
    [] c = "insert_proposal_under_limit" -> 1
    [] c = "insert_proposal_over_limit" -> 1
    [] c = "insert_proposal_limit_zero" -> 0
    [] c = "pop_hint_only" -> 0
    [] c = "pop_hint_keeps_observed_for_proposal" -> 1
    [] c = "pop_proposal_only" -> 0
    [] c = "pop_missing" -> 1
    [] c = "evict_hint_keeps_observed_for_proposal" -> 3
    [] c = "prune_height_leq" -> 1
    [] c = "prune_noop" -> 1

SpecEvictedCount(c) ==
  CASE c \in {"insert_hint_over_limit", "insert_hint_limit_zero",
              "insert_proposal_over_limit", "insert_proposal_limit_zero",
              "evict_hint_keeps_observed_for_proposal"} -> 1
    [] OTHER -> 0

SpecInsertedPresent(c) ==
  c \in {
    "insert_hint_under_limit",
    "insert_hint_over_limit",
    "replace_existing_hint",
    "insert_proposal_under_limit",
    "insert_proposal_over_limit"
  }

SpecInsertedObserved(c) ==
  SpecInsertedPresent(c)

SpecLowestHintPresent(c) ==
  CASE c \in {"insert_hint_over_limit", "insert_hint_limit_zero",
              "evict_hint_keeps_observed_for_proposal"} -> FALSE
    [] c \in {"replace_existing_hint", "pop_missing", "prune_height_leq",
              "prune_noop"} -> TRUE
    [] OTHER -> FALSE

SpecLowestProposalPresent(c) ==
  CASE c = "insert_proposal_over_limit" -> FALSE
    [] c \in {"pop_hint_keeps_observed_for_proposal",
              "evict_hint_keeps_observed_for_proposal", "prune_height_leq",
              "prune_noop"} -> TRUE
    [] OTHER -> FALSE

SpecLowestObserved(c) ==
  CASE c = "evict_hint_keeps_observed_for_proposal" -> TRUE
    [] c = "pop_hint_keeps_observed_for_proposal" -> TRUE
    [] c = "insert_hint_over_limit" -> FALSE
    [] c = "insert_proposal_over_limit" -> FALSE
    [] c = "insert_hint_limit_zero" -> FALSE
    [] c = "insert_proposal_limit_zero" -> FALSE
    [] c = "pop_hint_only" -> FALSE
    [] c = "pop_proposal_only" -> FALSE
    [] OTHER -> SpecLowestHintPresent(c) \/ SpecLowestProposalPresent(c)

SpecTargetHintPresent(c) ==
  c # "pop_hint_only" /\ c # "pop_hint_keeps_observed_for_proposal"

SpecTargetProposalPresent(c) ==
  c = "pop_hint_keeps_observed_for_proposal"

SpecTargetObserved(c) ==
  CASE c = "pop_hint_only" -> FALSE
    [] c = "pop_hint_keeps_observed_for_proposal" -> TRUE
    [] c = "pop_proposal_only" -> FALSE
    [] OTHER -> TRUE

SpecObservedRetainedForOtherKind(c) ==
  c = "pop_hint_keeps_observed_for_proposal" \/
  c = "evict_hint_keeps_observed_for_proposal"

SpecPruneRemovedCommitted(c) ==
  c = "prune_height_leq"

SpecPruneKeptFuture(c) ==
  c \in {"prune_height_leq", "prune_noop"}

ActualFinalHintCount(c) ==
  CASE Bug = "hint_limit_overflow" /\ c = "insert_hint_over_limit" -> 3
    [] Bug = "hint_limit_zero_keeps_insert" /\ c = "insert_hint_limit_zero" -> 1
    [] Bug = "replace_existing_duplicates_hint" /\ c = "replace_existing_hint" -> 2
    [] Bug = "pop_hint_does_not_remove" /\ c = "pop_hint_only" -> 1
    [] Bug = "prune_keeps_committed_hint" /\ c = "prune_height_leq" -> 2
    [] OTHER -> SpecFinalHintCount(c)

ActualFinalProposalCount(c) ==
  CASE Bug = "proposal_limit_overflow" /\ c = "insert_proposal_over_limit" -> 2
    [] Bug = "proposal_limit_zero_keeps_insert" /\
          c = "insert_proposal_limit_zero" -> 1
    [] Bug = "pop_proposal_does_not_remove" /\ c = "pop_proposal_only" -> 1
    [] Bug = "pop_hint_removes_proposal" /\
          c = "pop_hint_keeps_observed_for_proposal" -> 0
    [] Bug = "hint_evict_removes_other_kind" /\
          c = "evict_hint_keeps_observed_for_proposal" -> 0
    [] Bug = "prune_keeps_committed_proposal" /\ c = "prune_height_leq" -> 2
    [] OTHER -> SpecFinalProposalCount(c)

ActualObservedCount(c) ==
  CASE Bug = "hint_evict_leaves_stale_observed" /\ c = "insert_hint_over_limit" -> 3
    [] Bug = "proposal_evict_leaves_stale_observed" /\
          c = "insert_proposal_over_limit" -> 2
    [] Bug = "hint_limit_zero_keeps_insert" /\ c = "insert_hint_limit_zero" -> 1
    [] Bug = "proposal_limit_zero_keeps_insert" /\
          c = "insert_proposal_limit_zero" -> 1
    [] Bug = "pop_hint_leaves_stale_observed" /\ c = "pop_hint_only" -> 1
    [] Bug = "pop_proposal_leaves_stale_observed" /\ c = "pop_proposal_only" -> 1
    [] Bug = "pop_hint_drops_shared_observed" /\
          c = "pop_hint_keeps_observed_for_proposal" -> 0
    [] Bug = "hint_evict_drops_shared_observed" /\
          c = "evict_hint_keeps_observed_for_proposal" -> 2
    [] Bug = "prune_keeps_committed_observed" /\ c = "prune_height_leq" -> 2
    [] Bug = "prune_drops_future_observed" /\ c = "prune_height_leq" -> 0
    [] OTHER -> SpecObservedCount(c)

ActualEvictedCount(c) ==
  CASE Bug = "skip_eviction_metric" /\ c = "insert_hint_over_limit" -> 0
    [] Bug = "spurious_eviction_metric" /\ c = "insert_hint_under_limit" -> 1
    [] OTHER -> SpecEvictedCount(c)

ActualInsertedPresent(c) ==
  CASE Bug = "drop_inserted_hint" /\ c = "insert_hint_under_limit" -> FALSE
    [] Bug = "hint_limit_zero_keeps_insert" /\ c = "insert_hint_limit_zero" -> TRUE
    [] Bug = "proposal_limit_zero_keeps_insert" /\
          c = "insert_proposal_limit_zero" -> TRUE
    [] OTHER -> SpecInsertedPresent(c)

ActualInsertedObserved(c) ==
  CASE Bug = "skip_observed_on_insert" /\ c = "insert_hint_under_limit" -> FALSE
    [] Bug = "hint_limit_zero_keeps_insert" /\ c = "insert_hint_limit_zero" -> TRUE
    [] Bug = "proposal_limit_zero_keeps_insert" /\
          c = "insert_proposal_limit_zero" -> TRUE
    [] OTHER -> SpecInsertedObserved(c)

ActualLowestHintPresent(c) ==
  CASE Bug = "evict_newest_hint" /\ c = "insert_hint_over_limit" -> TRUE
    [] Bug = "hint_limit_zero_keeps_insert" /\ c = "insert_hint_limit_zero" -> TRUE
    [] OTHER -> SpecLowestHintPresent(c)

ActualLowestProposalPresent(c) ==
  CASE Bug = "evict_newest_proposal" /\ c = "insert_proposal_over_limit" -> TRUE
    [] Bug = "proposal_limit_zero_keeps_insert" /\
          c = "insert_proposal_limit_zero" -> TRUE
    [] Bug = "pop_hint_removes_proposal" /\
          c = "pop_hint_keeps_observed_for_proposal" -> FALSE
    [] Bug = "hint_evict_removes_other_kind" /\
          c = "evict_hint_keeps_observed_for_proposal" -> FALSE
    [] OTHER -> SpecLowestProposalPresent(c)

ActualLowestObserved(c) ==
  CASE Bug = "hint_evict_leaves_stale_observed" /\ c = "insert_hint_over_limit" -> TRUE
    [] Bug = "proposal_evict_leaves_stale_observed" /\
          c = "insert_proposal_over_limit" -> TRUE
    [] Bug = "hint_evict_drops_shared_observed" /\
          c = "evict_hint_keeps_observed_for_proposal" -> FALSE
    [] OTHER -> SpecLowestObserved(c)

ActualTargetHintPresent(c) ==
  CASE Bug = "pop_hint_does_not_remove" /\ c = "pop_hint_only" -> TRUE
    [] OTHER -> SpecTargetHintPresent(c)

ActualTargetProposalPresent(c) ==
  CASE Bug = "pop_hint_removes_proposal" /\
          c = "pop_hint_keeps_observed_for_proposal" -> FALSE
    [] Bug = "pop_proposal_does_not_remove" /\ c = "pop_proposal_only" -> TRUE
    [] OTHER -> SpecTargetProposalPresent(c)

ActualTargetObserved(c) ==
  CASE Bug = "pop_hint_leaves_stale_observed" /\ c = "pop_hint_only" -> TRUE
    [] Bug = "pop_proposal_leaves_stale_observed" /\ c = "pop_proposal_only" -> TRUE
    [] Bug = "pop_hint_drops_shared_observed" /\
          c = "pop_hint_keeps_observed_for_proposal" -> FALSE
    [] OTHER -> SpecTargetObserved(c)

ActualObservedRetainedForOtherKind(c) ==
  CASE Bug = "pop_hint_drops_shared_observed" /\
          c = "pop_hint_keeps_observed_for_proposal" -> FALSE
    [] Bug = "hint_evict_drops_shared_observed" /\
          c = "evict_hint_keeps_observed_for_proposal" -> FALSE
    [] OTHER -> SpecObservedRetainedForOtherKind(c)

ActualPruneRemovedCommitted(c) ==
  CASE Bug = "prune_keeps_committed_hint" /\ c = "prune_height_leq" -> FALSE
    [] Bug = "prune_keeps_committed_proposal" /\ c = "prune_height_leq" -> FALSE
    [] Bug = "prune_keeps_committed_observed" /\ c = "prune_height_leq" -> FALSE
    [] OTHER -> SpecPruneRemovedCommitted(c)

ActualPruneKeptFuture(c) ==
  CASE Bug = "prune_drops_future_observed" /\ c = "prune_height_leq" -> FALSE
    [] OTHER -> SpecPruneKeptFuture(c)

TypeInvariant ==
  /\ Bug \in {
       "none",
       "hint_limit_overflow",
       "proposal_limit_overflow",
       "hint_limit_zero_keeps_insert",
       "proposal_limit_zero_keeps_insert",
       "evict_newest_hint",
       "evict_newest_proposal",
       "hint_evict_leaves_stale_observed",
       "proposal_evict_leaves_stale_observed",
       "skip_eviction_metric",
       "spurious_eviction_metric",
       "drop_inserted_hint",
       "skip_observed_on_insert",
       "replace_existing_duplicates_hint",
       "pop_hint_does_not_remove",
       "pop_hint_leaves_stale_observed",
       "pop_hint_drops_shared_observed",
       "pop_hint_removes_proposal",
       "pop_proposal_does_not_remove",
       "pop_proposal_leaves_stale_observed",
       "hint_evict_drops_shared_observed",
       "hint_evict_removes_other_kind",
       "prune_keeps_committed_hint",
       "prune_keeps_committed_proposal",
       "prune_keeps_committed_observed",
       "prune_drops_future_observed"
     }
  /\ candidate \in Cases
  /\ limit \in CountValues
  /\ final_hint_count \in CountValues
  /\ final_proposal_count \in CountValues
  /\ observed_count \in CountValues
  /\ evicted_count \in CountValues
  /\ inserted_present \in BOOLEAN
  /\ inserted_observed \in BOOLEAN
  /\ lowest_hint_present \in BOOLEAN
  /\ lowest_proposal_present \in BOOLEAN
  /\ lowest_observed \in BOOLEAN
  /\ target_hint_present \in BOOLEAN
  /\ target_proposal_present \in BOOLEAN
  /\ target_observed \in BOOLEAN
  /\ observed_retained_for_other_kind \in BOOLEAN
  /\ prune_removed_committed \in BOOLEAN
  /\ prune_kept_future \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ limit = LimitFor(candidate)
  /\ final_hint_count = ActualFinalHintCount(candidate)
  /\ final_proposal_count = ActualFinalProposalCount(candidate)
  /\ observed_count = ActualObservedCount(candidate)
  /\ evicted_count = ActualEvictedCount(candidate)
  /\ inserted_present = ActualInsertedPresent(candidate)
  /\ inserted_observed = ActualInsertedObserved(candidate)
  /\ lowest_hint_present = ActualLowestHintPresent(candidate)
  /\ lowest_proposal_present = ActualLowestProposalPresent(candidate)
  /\ lowest_observed = ActualLowestObserved(candidate)
  /\ target_hint_present = ActualTargetHintPresent(candidate)
  /\ target_proposal_present = ActualTargetProposalPresent(candidate)
  /\ target_observed = ActualTargetObserved(candidate)
  /\ observed_retained_for_other_kind = ActualObservedRetainedForOtherKind(candidate)
  /\ prune_removed_committed = ActualPruneRemovedCommitted(candidate)
  /\ prune_kept_future = ActualPruneKeptFuture(candidate)

Next ==
  UNCHANGED vars

HintCountMatchesSpec ==
  final_hint_count = SpecFinalHintCount(candidate)

ProposalCountMatchesSpec ==
  final_proposal_count = SpecFinalProposalCount(candidate)

ObservedCountMatchesSpec ==
  observed_count = SpecObservedCount(candidate)

EvictedCountMatchesSpec ==
  evicted_count = SpecEvictedCount(candidate)

HintLimitEnforced ==
  final_hint_count <= limit

ProposalLimitEnforced ==
  final_proposal_count <= limit

InsertedEntryRetainedWhenWithinLimit ==
  SpecInsertedPresent(candidate) => inserted_present

InsertedEntryObservedWhenRetained ==
  inserted_present => inserted_observed

ZeroLimitKeepsNothing ==
  limit = 0 =>
    /\ final_hint_count = 0
    /\ final_proposal_count = 0
    /\ observed_count = 0
    /\ ~inserted_present

HintOverflowEvictsLowestKey ==
  candidate = "insert_hint_over_limit" =>
    /\ ~lowest_hint_present
    /\ ~lowest_observed
    /\ inserted_present

ProposalOverflowEvictsLowestKey ==
  candidate = "insert_proposal_over_limit" =>
    /\ ~lowest_proposal_present
    /\ ~lowest_observed
    /\ inserted_present

EvictionMetricMatchesOverflow ==
  evicted_count = SpecEvictedCount(candidate)

NoEvictionMetricWithoutEviction ==
  SpecEvictedCount(candidate) = 0 => evicted_count = 0

ReplaceExistingDoesNotDuplicate ==
  candidate = "replace_existing_hint" =>
    /\ final_hint_count = 1
    /\ evicted_count = 0
    /\ inserted_present

PopHintRemovesHint ==
  candidate = "pop_hint_only" =>
    /\ ~target_hint_present
    /\ ~target_observed

PopHintKeepsObservedForProposal ==
  candidate = "pop_hint_keeps_observed_for_proposal" =>
    /\ ~target_hint_present
    /\ target_proposal_present
    /\ target_observed
    /\ observed_retained_for_other_kind

PopProposalRemovesProposal ==
  candidate = "pop_proposal_only" =>
    /\ ~target_proposal_present
    /\ ~target_observed

PopMissingNoChange ==
  candidate = "pop_missing" =>
    /\ final_hint_count = 1
    /\ observed_count = 1
    /\ evicted_count = 0

HintEvictionKeepsObservedForProposal ==
  candidate = "evict_hint_keeps_observed_for_proposal" =>
    /\ ~lowest_hint_present
    /\ lowest_proposal_present
    /\ lowest_observed
    /\ observed_retained_for_other_kind

ObservedOnlyForLiveEntries ==
  observed_count <= final_hint_count + final_proposal_count

PruneRemovesCommittedEntries ==
  candidate = "prune_height_leq" => prune_removed_committed

PruneKeepsFutureEntries ==
  candidate = "prune_height_leq" => prune_kept_future

PruneNoopLeavesFutureEntries ==
  candidate = "prune_noop" =>
    /\ prune_kept_future
    /\ final_hint_count = 1
    /\ final_proposal_count = 1
    /\ observed_count = 1

ProposalCacheCoreSafety ==
  /\ HintCountMatchesSpec
  /\ ProposalCountMatchesSpec
  /\ ObservedCountMatchesSpec
  /\ EvictedCountMatchesSpec
  /\ HintLimitEnforced
  /\ ProposalLimitEnforced
  /\ InsertedEntryRetainedWhenWithinLimit
  /\ InsertedEntryObservedWhenRetained
  /\ ZeroLimitKeepsNothing
  /\ HintOverflowEvictsLowestKey
  /\ ProposalOverflowEvictsLowestKey
  /\ EvictionMetricMatchesOverflow
  /\ NoEvictionMetricWithoutEviction
  /\ ReplaceExistingDoesNotDuplicate
  /\ PopHintRemovesHint
  /\ PopHintKeepsObservedForProposal
  /\ PopProposalRemovesProposal
  /\ PopMissingNoChange
  /\ HintEvictionKeepsObservedForProposal
  /\ ObservedOnlyForLiveEntries
  /\ PruneRemovesCommittedEntries
  /\ PruneKeepsFutureEntries
  /\ PruneNoopLeavesFutureEntries

ProposalCacheExactness ==
  /\ HintCountMatchesSpec
  /\ ProposalCountMatchesSpec
  /\ ObservedCountMatchesSpec
  /\ EvictedCountMatchesSpec
  /\ HintLimitEnforced
  /\ ProposalLimitEnforced
  /\ InsertedEntryRetainedWhenWithinLimit
  /\ InsertedEntryObservedWhenRetained
  /\ ZeroLimitKeepsNothing
  /\ HintOverflowEvictsLowestKey
  /\ ProposalOverflowEvictsLowestKey
  /\ EvictionMetricMatchesOverflow
  /\ NoEvictionMetricWithoutEviction
  /\ ReplaceExistingDoesNotDuplicate
  /\ PopHintRemovesHint
  /\ PopHintKeepsObservedForProposal
  /\ PopProposalRemovesProposal
  /\ PopMissingNoChange
  /\ HintEvictionKeepsObservedForProposal
  /\ ObservedOnlyForLiveEntries
  /\ PruneRemovesCommittedEntries
  /\ PruneKeepsFutureEntries
  /\ PruneNoopLeavesFutureEntries
ProposalCacheCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ProposalCacheExactness

Safety == ProposalCacheExactness

====
