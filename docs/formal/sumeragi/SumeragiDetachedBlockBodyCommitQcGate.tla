---- MODULE SumeragiDetachedBlockBodyCommitQcGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `handle_detached_block_body_commit_qc(...)`.

The helper processes a commit QC detached from an ignored or already
materialized BlockBodyResponse. It has no effect when the response carries no
QC. If the target commit QC is already cached, it clears the missing commit-QC
request as obsolete and returns without re-handling the QC. Otherwise it
attempts `handle_qc`, then clears the missing commit-QC request only if the QC
is cached after that attempt, regardless of whether `handle_qc` returned `Ok`
or `Err`.
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
  "no_qc",
  "no_qc_cached",
  "cached_before",
  "handle_success_caches",
  "handle_success_no_cache",
  "handle_error_caches",
  "handle_error_no_cache"
}

HasQc(c) ==
  c \notin {"no_qc", "no_qc_cached"}

CachedBefore(c) ==
  c \in {"no_qc_cached", "cached_before"}

CachedAfterHandle(c) ==
  c \in {"handle_success_caches", "handle_error_caches"}

SpecHandled(c) ==
  HasQc(c) /\ ~CachedBefore(c)

SpecCleared(c) ==
  HasQc(c) /\ (CachedBefore(c) \/ (~CachedBefore(c) /\ CachedAfterHandle(c)))

ActualHandled(c) ==
  CASE Bug = "handle_missing_qc"
       /\ c = "no_qc" -> TRUE
    [] Bug = "handle_after_cached_before"
       /\ c = "cached_before" -> TRUE
    [] Bug = "skip_handle_success"
       /\ c \in {"handle_success_caches", "handle_success_no_cache"} -> FALSE
    [] Bug = "skip_handle_error"
       /\ c \in {"handle_error_caches", "handle_error_no_cache"} -> FALSE
    [] OTHER -> SpecHandled(c)

ActualCleared(c) ==
  CASE Bug = "clear_missing_qc"
       /\ c = "no_qc_cached" -> TRUE
    [] Bug = "skip_cached_before_clear"
       /\ c = "cached_before" -> FALSE
    [] Bug = "clear_without_post_cache"
       /\ c = "handle_success_no_cache" -> TRUE
    [] Bug = "skip_post_cache_clear"
       /\ c = "handle_success_caches" -> FALSE
    [] Bug = "post_clear_requires_success"
       /\ c = "handle_error_caches" -> FALSE
    [] Bug = "clear_after_error_no_cache"
       /\ c = "handle_error_no_cache" -> TRUE
    [] OTHER -> SpecCleared(c)

Matches(c) ==
  /\ ActualHandled(c) = SpecHandled(c)
  /\ ActualCleared(c) = SpecCleared(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "handle_missing_qc",
       "clear_missing_qc",
       "skip_cached_before_clear",
       "handle_after_cached_before",
       "skip_handle_success",
       "clear_without_post_cache",
       "skip_post_cache_clear",
       "post_clear_requires_success",
       "skip_handle_error",
       "clear_after_error_no_cache"
     }
  /\ checked = 0

DetachedBlockBodyCommitQcMatchesSpec ==
  \A c \in Cases: Matches(c)

DetachedBlockBodyCommitQcExactness ==
  /\ DetachedBlockBodyCommitQcMatchesSpec

DetachedBlockBodyCommitQcCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DetachedBlockBodyCommitQcExactness

SafetyFast == DetachedBlockBodyCommitQcExactness

NoQcNoHandle ==
  Matches("no_qc")

NoQcCachedNoClear ==
  Matches("no_qc_cached")

CachedBeforeClearedWithoutHandle ==
  Matches("cached_before")

SuccessNoCacheHandled ==
  Matches("handle_success_no_cache")

SuccessNoCacheNotCleared ==
  Matches("handle_success_no_cache")

SuccessCacheCleared ==
  Matches("handle_success_caches")

ErrorCacheCleared ==
  Matches("handle_error_caches")

ErrorNoCacheHandled ==
  Matches("handle_error_no_cache")

ErrorNoCacheNotCleared ==
  Matches("handle_error_no_cache")

====
