---- MODULE SumeragiDeferredBlockSyncCacheGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `cache_deferred_block_sync_update(...)` and
`defer_block_sync_update(...)`.

The lower-level helper gate checks merge and cap ordering rules in isolation.
This gate pins the integration obligations that make deferred BlockSyncUpdate
replay deterministic and observable:

- incoming commit votes are always cleared before an update is cached,
- the deferred-map key is the full `(height, view, block_hash)` tuple,
- matching keys merge while distinct height/view/hash tuples insert,
- sender replacement follows the merge helper's Some-only rule,
- the bounded cap is enforced after both insert and merge paths,
- `defer_block_sync_update(...)` invokes the cache path first, and
- the deferred consensus handling outcome is recorded as
  BlockSyncUpdate/Deferred/CommitPipelineActive.
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
  "cache_new_entry",
  "cache_new_sender_none",
  "cache_same_key_fills_missing_qc",
  "cache_same_key_preserves_existing_qc",
  "cache_same_key_sender_none_preserves",
  "cache_same_key_sender_some_replaces",
  "cache_distinct_height_inserts",
  "cache_distinct_view_inserts",
  "cache_distinct_hash_inserts",
  "cache_cap_after_insert",
  "cache_cap_after_merge",
  "defer_invokes_cache",
  "defer_records_deferred_outcome"
}

IsDefer(c) ==
  c \in {"defer_invokes_cache", "defer_records_deferred_outcome"}

InitialLen(c) ==
  CASE c = "cache_cap_after_merge" -> 2
    [] c \in {
       "cache_same_key_fills_missing_qc",
       "cache_same_key_preserves_existing_qc",
       "cache_same_key_sender_none_preserves",
       "cache_same_key_sender_some_replaces",
       "cache_distinct_height_inserts",
       "cache_distinct_view_inserts",
       "cache_distinct_hash_inserts",
       "cache_cap_after_insert"
     } -> 1
    [] OTHER -> 0

ExistingSameFullKey(c) ==
  c \in {
    "cache_same_key_fills_missing_qc",
    "cache_same_key_preserves_existing_qc",
    "cache_same_key_sender_none_preserves",
    "cache_same_key_sender_some_replaces",
    "cache_cap_after_merge"
  }

IncomingCommit(c) ==
  IF c \in {
       "cache_new_entry",
       "cache_same_key_fills_missing_qc",
       "cache_same_key_preserves_existing_qc",
       "defer_invokes_cache",
       "defer_records_deferred_outcome"
     }
  THEN "incoming"
  ELSE "none"

ExistingCommit(c) ==
  IF c = "cache_same_key_preserves_existing_qc" THEN "existing" ELSE "none"

IncomingSender(c) ==
  CASE c \in {
       "cache_new_entry",
       "cache_same_key_sender_some_replaces",
       "defer_invokes_cache",
       "defer_records_deferred_outcome"
     } -> "incoming"
    [] OTHER -> "none"

ExistingSender(c) ==
  IF c \in {
       "cache_same_key_sender_none_preserves",
       "cache_same_key_sender_some_replaces"
     }
  THEN "existing"
  ELSE "none"

Cap(c) ==
  IF c \in {"cache_cap_after_insert", "cache_cap_after_merge"} THEN 1 ELSE 0

SpecCacheCalled(c) ==
  TRUE

SpecKeyMatched(c) ==
  ExistingSameFullKey(c)

SpecInserted(c) ==
  ~SpecKeyMatched(c)

SpecLenBeforeCap(c) ==
  InitialLen(c) + (IF SpecInserted(c) THEN 1 ELSE 0)

SpecCapCalled(c) ==
  TRUE

SpecEvictionCount(c) ==
  IF Cap(c) = 0 \/ SpecLenBeforeCap(c) <= Cap(c)
  THEN 0
  ELSE SpecLenBeforeCap(c) - Cap(c)

SpecFinalLen(c) ==
  SpecLenBeforeCap(c) - SpecEvictionCount(c)

SpecCommitVotesCleared(c) ==
  SpecCacheCalled(c)

SpecFinalCommit(c) ==
  IF ExistingCommit(c) # "none"
  THEN ExistingCommit(c)
  ELSE IncomingCommit(c)

SpecFinalSender(c) ==
  IF IncomingSender(c) # "none"
  THEN IncomingSender(c)
  ELSE ExistingSender(c)

SpecCacheReasonForwarded(c) ==
  TRUE

SpecRecordCalled(c) ==
  IsDefer(c)

SpecRecordAfterCache(c) ==
  TRUE

SpecRecordedKind(c) ==
  IF IsDefer(c) THEN "BlockSyncUpdate" ELSE "none"

SpecRecordedOutcome(c) ==
  IF IsDefer(c) THEN "Deferred" ELSE "none"

SpecRecordedReason(c) ==
  IF IsDefer(c) THEN "CommitPipelineActive" ELSE "none"

ActualCacheCalled(c) ==
  CASE Bug = "defer_skips_cache"
       /\ IsDefer(c) -> FALSE
    [] OTHER -> TRUE

ActualKeyMatched(c) ==
  IF ~ActualCacheCalled(c) THEN FALSE
  ELSE CASE Bug = "key_ignores_height"
            /\ c = "cache_distinct_height_inserts" -> TRUE
         [] Bug = "key_ignores_view"
            /\ c = "cache_distinct_view_inserts" -> TRUE
         [] Bug = "key_ignores_hash"
            /\ c = "cache_distinct_hash_inserts" -> TRUE
         [] Bug = "same_key_inserts_duplicate"
            /\ c = "cache_same_key_fills_missing_qc" -> FALSE
         [] OTHER -> ExistingSameFullKey(c)

ActualInserted(c) ==
  ActualCacheCalled(c) /\ ~ActualKeyMatched(c)

ActualLenBeforeCap(c) ==
  InitialLen(c) + (IF ActualInserted(c) THEN 1 ELSE 0)

ActualCapCalled(c) ==
  IF ~ActualCacheCalled(c) THEN FALSE
  ELSE CASE Bug = "skip_cap_after_insert"
            /\ c = "cache_cap_after_insert" -> FALSE
         [] Bug = "skip_cap_after_merge"
            /\ c = "cache_cap_after_merge" -> FALSE
         [] OTHER -> TRUE

ActualEvictionCount(c) ==
  CASE Bug = "cap_before_insert"
       /\ c = "cache_cap_after_insert" -> 0
    [] ~ActualCapCalled(c) -> 0
    [] Cap(c) = 0 \/ ActualLenBeforeCap(c) <= Cap(c) -> 0
    [] OTHER -> ActualLenBeforeCap(c) - Cap(c)

ActualFinalLen(c) ==
  ActualLenBeforeCap(c) - ActualEvictionCount(c)

ActualCommitVotesCleared(c) ==
  IF ~ActualCacheCalled(c) THEN FALSE
  ELSE CASE Bug = "new_keeps_commit_votes"
            /\ c = "cache_new_entry" -> FALSE
         [] Bug = "merge_keeps_commit_votes"
            /\ c = "cache_same_key_fills_missing_qc" -> FALSE
         [] OTHER -> TRUE

ActualFinalCommit(c) ==
  IF ~ActualCacheCalled(c) THEN "none"
  ELSE IF ActualInserted(c) THEN IncomingCommit(c)
  ELSE CASE Bug = "same_key_drops_incoming_commit_qc"
            /\ c = "cache_same_key_fills_missing_qc" -> "none"
         [] Bug = "same_key_overwrites_commit_qc"
            /\ c = "cache_same_key_preserves_existing_qc" -> "incoming"
         [] ExistingCommit(c) # "none" -> ExistingCommit(c)
         [] OTHER -> IncomingCommit(c)

ActualFinalSender(c) ==
  IF ~ActualCacheCalled(c) THEN "none"
  ELSE IF ActualInserted(c) THEN IncomingSender(c)
  ELSE CASE Bug = "sender_none_clears_existing"
            /\ c = "cache_same_key_sender_none_preserves" -> "none"
         [] Bug = "sender_some_ignored"
            /\ c = "cache_same_key_sender_some_replaces" -> "existing"
         [] IncomingSender(c) # "none" -> IncomingSender(c)
         [] OTHER -> ExistingSender(c)

ActualCacheReasonForwarded(c) ==
  CASE Bug = "cache_reason_not_forwarded"
       /\ c = "defer_records_deferred_outcome" -> FALSE
    [] OTHER -> ActualCacheCalled(c)

ActualRecordCalled(c) ==
  CASE Bug = "defer_missing_record"
       /\ c = "defer_records_deferred_outcome" -> FALSE
    [] OTHER -> IsDefer(c)

ActualRecordAfterCache(c) ==
  CASE Bug = "defer_records_before_cache"
       /\ c = "defer_records_deferred_outcome" -> FALSE
    [] OTHER -> TRUE

ActualRecordedKind(c) ==
  IF ~ActualRecordCalled(c) THEN "none"
  ELSE CASE Bug = "defer_wrong_kind"
            /\ c = "defer_records_deferred_outcome" -> "Vote"
         [] OTHER -> "BlockSyncUpdate"

ActualRecordedOutcome(c) ==
  IF ~ActualRecordCalled(c) THEN "none"
  ELSE CASE Bug = "defer_wrong_outcome"
            /\ c = "defer_records_deferred_outcome" -> "Accepted"
         [] OTHER -> "Deferred"

ActualRecordedReason(c) ==
  IF ~ActualRecordCalled(c) THEN "none"
  ELSE CASE Bug = "defer_wrong_reason"
            /\ c = "defer_records_deferred_outcome" -> "None"
         [] OTHER -> "CommitPipelineActive"

Matches(c) ==
  /\ ActualCacheCalled(c) = SpecCacheCalled(c)
  /\ ActualKeyMatched(c) = SpecKeyMatched(c)
  /\ ActualInserted(c) = SpecInserted(c)
  /\ ActualCapCalled(c) = SpecCapCalled(c)
  /\ ActualEvictionCount(c) = SpecEvictionCount(c)
  /\ ActualFinalLen(c) = SpecFinalLen(c)
  /\ ActualCommitVotesCleared(c) = SpecCommitVotesCleared(c)
  /\ ActualFinalCommit(c) = SpecFinalCommit(c)
  /\ ActualFinalSender(c) = SpecFinalSender(c)
  /\ ActualCacheReasonForwarded(c) = SpecCacheReasonForwarded(c)
  /\ ActualRecordCalled(c) = SpecRecordCalled(c)
  /\ ActualRecordAfterCache(c) = SpecRecordAfterCache(c)
  /\ ActualRecordedKind(c) = SpecRecordedKind(c)
  /\ ActualRecordedOutcome(c) = SpecRecordedOutcome(c)
  /\ ActualRecordedReason(c) = SpecRecordedReason(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "new_keeps_commit_votes",
       "merge_keeps_commit_votes",
       "same_key_inserts_duplicate",
       "same_key_drops_incoming_commit_qc",
       "same_key_overwrites_commit_qc",
       "sender_none_clears_existing",
       "sender_some_ignored",
       "key_ignores_height",
       "key_ignores_view",
       "key_ignores_hash",
       "skip_cap_after_insert",
       "cap_before_insert",
       "skip_cap_after_merge",
       "defer_skips_cache",
       "defer_missing_record",
       "defer_wrong_kind",
       "defer_wrong_outcome",
       "defer_wrong_reason",
       "defer_records_before_cache",
       "cache_reason_not_forwarded"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

CacheNewClearsCommitVotes ==
  Matches("cache_new_entry")

CacheNewStoresSenderNone ==
  Matches("cache_new_sender_none")

CacheSameKeyMerges ==
  Matches("cache_same_key_fills_missing_qc")

CacheMergeFillsMissingCommitQc ==
  Matches("cache_same_key_fills_missing_qc")

CacheMergePreservesExistingCommitQc ==
  Matches("cache_same_key_preserves_existing_qc")

CacheSenderNonePreserves ==
  Matches("cache_same_key_sender_none_preserves")

CacheSenderSomeReplaces ==
  Matches("cache_same_key_sender_some_replaces")

CacheDistinctHeightInserts ==
  Matches("cache_distinct_height_inserts")

CacheDistinctViewInserts ==
  Matches("cache_distinct_view_inserts")

CacheDistinctHashInserts ==
  Matches("cache_distinct_hash_inserts")

CacheCapAfterInsert ==
  Matches("cache_cap_after_insert")

CacheCapAfterMerge ==
  Matches("cache_cap_after_merge")

DeferInvokesCache ==
  Matches("defer_invokes_cache")

DeferRecordsOutcome ==
  Matches("defer_records_deferred_outcome")

DeferRecordsAfterCache ==
  Matches("defer_records_deferred_outcome")

CacheReasonForwarded ==
  Matches("defer_records_deferred_outcome")

=============================================================================
====
