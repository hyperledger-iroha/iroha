---- MODULE SumeragiCommitQcLookupGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for commit-QC cache/history lookup.

This slice models `commit_qc_from_cache_or_history(...)` and
`commit_qc_from_history(...)`. Cached commit QCs have priority when the cache
contains an exact commit-phase certificate for the requested block/round/epoch.
If the cache misses, history fallback may return only a commit-phase QC whose
subject hash, height, view, epoch, mode tag, nonempty aggregate signature, and
validator set all match the lookup context.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "cache_only",
  "cache_over_history",
  "history_valid",
  "history_prepare_phase",
  "history_hash_mismatch",
  "history_height_mismatch",
  "history_view_mismatch",
  "history_epoch_mismatch",
  "history_mode_mismatch",
  "history_empty_aggregate",
  "history_topology_mismatch",
  "history_absent"
}

CacheCases == {"cache_only", "cache_over_history"}

HistoryPositiveCases == {"history_valid"}

HistoryIdentityCases == {
  "history_prepare_phase",
  "history_hash_mismatch",
  "history_height_mismatch",
  "history_view_mismatch"
}

HistoryContextCases == {
  "history_epoch_mismatch",
  "history_mode_mismatch",
  "history_topology_mismatch"
}

HistoryAggregateCases == {"history_empty_aggregate"}

HistoryAbsentCases == {"history_absent"}

CacheHit(c) ==
  c \in {"cache_only", "cache_over_history"}

HistoryPresent(c) ==
  c # "cache_only" /\ c # "history_absent"

HistoryPhaseCommit(c) ==
  c # "history_prepare_phase"

HistorySubjectMatches(c) ==
  c # "history_hash_mismatch"

HistoryHeightMatches(c) ==
  c # "history_height_mismatch"

HistoryViewMatches(c) ==
  c # "history_view_mismatch"

HistoryEpochMatches(c) ==
  c # "history_epoch_mismatch"

HistoryModeTagMatches(c) ==
  c # "history_mode_mismatch"

HistoryAggregateNonEmpty(c) ==
  c # "history_empty_aggregate"

HistoryTopologyMatches(c) ==
  c # "history_topology_mismatch"

HistoryRawMatches(c) ==
  HistoryPresent(c)
    /\ HistoryPhaseCommit(c)
    /\ HistorySubjectMatches(c)
    /\ HistoryHeightMatches(c)
    /\ HistoryViewMatches(c)
    /\ HistoryEpochMatches(c)
    /\ HistoryModeTagMatches(c)
    /\ HistoryAggregateNonEmpty(c)

SpecSource(c) ==
  IF CacheHit(c) THEN
    "cache"
  ELSE IF HistoryRawMatches(c) /\ HistoryTopologyMatches(c) THEN
    "history"
  ELSE
    "none"

ActualSource(c) ==
  CASE Bug = "ignore_cache_priority"
       /\ c = "cache_over_history" -> "history"
    [] Bug = "drop_cache_hit"
       /\ c = "cache_only" -> "none"
    [] Bug = "ignore_valid_history"
       /\ c = "history_valid" -> "none"
    [] Bug = "accept_prepare_history"
       /\ c = "history_prepare_phase" -> "history"
    [] Bug = "accept_hash_mismatch"
       /\ c = "history_hash_mismatch" -> "history"
    [] Bug = "accept_height_mismatch"
       /\ c = "history_height_mismatch" -> "history"
    [] Bug = "accept_view_mismatch"
       /\ c = "history_view_mismatch" -> "history"
    [] Bug = "accept_epoch_mismatch"
       /\ c = "history_epoch_mismatch" -> "history"
    [] Bug = "accept_mode_mismatch"
       /\ c = "history_mode_mismatch" -> "history"
    [] Bug = "accept_empty_aggregate"
       /\ c = "history_empty_aggregate" -> "history"
    [] Bug = "accept_topology_mismatch"
       /\ c = "history_topology_mismatch" -> "history"
    [] Bug = "accept_absent_history"
       /\ c = "history_absent" -> "history"
    [] OTHER -> SpecSource(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in {
       "none",
       "ignore_cache_priority",
       "drop_cache_hit",
       "ignore_valid_history",
       "accept_prepare_history",
       "accept_hash_mismatch",
       "accept_height_mismatch",
       "accept_view_mismatch",
       "accept_epoch_mismatch",
       "accept_mode_mismatch",
       "accept_empty_aggregate",
       "accept_topology_mismatch",
       "accept_absent_history"
     }

SpecSourceAnchors ==
  /\ SpecSource("cache_only") = "cache"
  /\ SpecSource("cache_over_history") = "cache"
  /\ SpecSource("history_valid") = "history"
  /\ SpecSource("history_prepare_phase") = "none"
  /\ SpecSource("history_hash_mismatch") = "none"
  /\ SpecSource("history_height_mismatch") = "none"
  /\ SpecSource("history_view_mismatch") = "none"
  /\ SpecSource("history_epoch_mismatch") = "none"
  /\ SpecSource("history_mode_mismatch") = "none"
  /\ SpecSource("history_empty_aggregate") = "none"
  /\ SpecSource("history_topology_mismatch") = "none"
  /\ SpecSource("history_absent") = "none"

CachePriority ==
  \A c \in Cases: CacheHit(c) => SpecSource(c) = "cache"

HistoryAdmissionRequiresAllPredicates ==
  \A c \in Cases:
    SpecSource(c) = "history" =>
      /\ ~CacheHit(c)
      /\ HistoryPresent(c)
      /\ HistoryRawMatches(c)
      /\ HistoryTopologyMatches(c)

NoHistoryFromInvalidCases ==
  \A c \in Cases:
    (~CacheHit(c)
      /\ (~HistoryPresent(c)
        \/ ~HistoryRawMatches(c)
        \/ ~HistoryTopologyMatches(c))) =>
      SpecSource(c) = "none"

CommitQcLookupSourceMatchesSpec ==
  \A c \in Cases: ActualSource(c) = SpecSource(c)

SafetyFast == CommitQcLookupSourceMatchesSpec

CommitQcLookupCachePriorityExact ==
  \A c \in CacheCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupHistoryPositiveExact ==
  \A c \in HistoryPositiveCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupHistoryIdentityExact ==
  \A c \in HistoryIdentityCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupHistoryContextExact ==
  \A c \in HistoryContextCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupHistoryAggregateExact ==
  \A c \in HistoryAggregateCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupAbsentHistoryExact ==
  \A c \in HistoryAbsentCases:
    ActualSource(c) = SpecSource(c)

CommitQcLookupExactness ==
  /\ CommitQcLookupCachePriorityExact
  /\ CommitQcLookupHistoryPositiveExact
  /\ CommitQcLookupHistoryIdentityExact
  /\ CommitQcLookupHistoryContextExact
  /\ CommitQcLookupHistoryAggregateExact
  /\ CommitQcLookupAbsentHistoryExact

BugIgnoreCachePriority ==
  ActualSource("cache_over_history") = SpecSource("cache_over_history")

BugDropCacheHit ==
  ActualSource("cache_only") = SpecSource("cache_only")

BugIgnoreValidHistory ==
  ActualSource("history_valid") = SpecSource("history_valid")

BugAcceptPrepareHistory ==
  ActualSource("history_prepare_phase") = SpecSource("history_prepare_phase")

BugAcceptHashMismatch ==
  ActualSource("history_hash_mismatch") = SpecSource("history_hash_mismatch")

BugAcceptHeightMismatch ==
  ActualSource("history_height_mismatch") = SpecSource("history_height_mismatch")

BugAcceptViewMismatch ==
  ActualSource("history_view_mismatch") = SpecSource("history_view_mismatch")

BugAcceptEpochMismatch ==
  ActualSource("history_epoch_mismatch") = SpecSource("history_epoch_mismatch")

BugAcceptModeMismatch ==
  ActualSource("history_mode_mismatch") = SpecSource("history_mode_mismatch")

BugAcceptEmptyAggregate ==
  ActualSource("history_empty_aggregate") = SpecSource("history_empty_aggregate")

BugAcceptTopologyMismatch ==
  ActualSource("history_topology_mismatch") =
    SpecSource("history_topology_mismatch")

BugAcceptAbsentHistory ==
  ActualSource("history_absent") = SpecSource("history_absent")

====
