---- MODULE SumeragiRbcStatusRetentionGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for RBC status retention helpers.

This slice pins `rbc_status::{enforce_limits, enforce_map_limits}` and the
disk-retention branch of `rbc_status::Handle::update(...)`:
- zero TTL disables age pruning,
- positive TTL drops only entries older than the TTL and keeps exact-boundary
  or future-dated summaries,
- capacity pruning keeps the newest retained summaries and capacity zero clears
  the retained set,
- `update(...)` persists changed or inserted summaries when disk is configured,
  refreshes timestamps even when the summary is unchanged, applies map limits
  before publishing the active count, and skips persistence only for unchanged
  summaries when both TTL and capacity retention are disabled.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LimitCases == {
  "ttl_zero_keeps_all",
  "ttl_boundary_keeps",
  "ttl_stale_drops",
  "ttl_future_kept",
  "capacity_zero_clears",
  "capacity_one_keeps_newest",
  "capacity_exact_keeps_all"
}

UpdateCases == {
  "insert_no_disk",
  "insert_disk_no_limits",
  "same_disk_no_limits",
  "same_disk_ttl",
  "same_disk_capacity",
  "changed_disk_no_limits",
  "insert_disk_capacity_zero",
  "same_no_disk_updates_timestamp"
}

SpecLimit(c) ==
  CASE c = "ttl_zero_keeps_all" -> {"old", "fresh"}
    [] c = "ttl_boundary_keeps" -> {"boundary"}
    [] c = "ttl_stale_drops" -> {"fresh"}
    [] c = "ttl_future_kept" -> {"future"}
    [] c = "capacity_zero_clears" -> {}
    [] c = "capacity_one_keeps_newest" -> {"newest"}
    [] OTHER -> {"old", "mid", "newest"}

ActualLimit(c) ==
  CASE Bug = "limit_ttl_zero_clears"
       /\ c = "ttl_zero_keeps_all" -> {}
    [] Bug = "limit_boundary_drops"
       /\ c = "ttl_boundary_keeps" -> {}
    [] Bug = "limit_stale_kept"
       /\ c = "ttl_stale_drops" -> {"stale", "fresh"}
    [] Bug = "limit_future_drops"
       /\ c = "ttl_future_kept" -> {}
    [] Bug = "limit_capacity_zero_keeps_all"
       /\ c = "capacity_zero_clears" -> {"old", "fresh"}
    [] Bug = "limit_capacity_one_keeps_oldest"
       /\ c = "capacity_one_keeps_newest" -> {"old"}
    [] Bug = "limit_capacity_exact_drops_oldest"
       /\ c = "capacity_exact_keeps_all" -> {"mid", "newest"}
    [] OTHER -> SpecLimit(c)

SpecShouldPersist(c) ==
  CASE c \in {"insert_no_disk", "same_no_disk_updates_timestamp"} -> FALSE
    [] c = "same_disk_no_limits" -> FALSE
    [] OTHER -> TRUE

ActualShouldPersist(c) ==
  CASE Bug = "update_insert_no_disk_persists"
       /\ c = "insert_no_disk" -> TRUE
    [] Bug = "update_insert_disk_skips_persist"
       /\ c = "insert_disk_no_limits" -> FALSE
    [] Bug = "update_same_disk_no_limits_persists"
       /\ c = "same_disk_no_limits" -> TRUE
    [] Bug = "update_same_disk_ttl_skips_persist"
       /\ c = "same_disk_ttl" -> FALSE
    [] Bug = "update_same_disk_capacity_skips_persist"
       /\ c = "same_disk_capacity" -> FALSE
    [] Bug = "update_changed_disk_skips_persist"
       /\ c = "changed_disk_no_limits" -> FALSE
    [] OTHER -> SpecShouldPersist(c)

SpecMemoryKeys(c) ==
  CASE c \in {"insert_no_disk", "insert_disk_no_limits"} -> {"existing", "new"}
    [] c = "insert_disk_capacity_zero" -> {}
    [] OTHER -> {"existing"}

ActualMemoryKeys(c) ==
  CASE Bug = "update_insert_capacity_zero_keeps_entry"
       /\ c = "insert_disk_capacity_zero" -> {"new"}
    [] OTHER -> SpecMemoryKeys(c)

SpecActiveCount(c) == Cardinality(SpecMemoryKeys(c))

ActualActiveCount(c) ==
  CASE Bug = "update_active_count_not_after_prune"
       /\ c = "insert_disk_capacity_zero" -> 1
    [] OTHER -> Cardinality(ActualMemoryKeys(c))

SpecUpdatedTimestamp(c) ==
  c /= "insert_disk_capacity_zero"

ActualUpdatedTimestamp(c) ==
  CASE Bug = "update_same_skips_timestamp"
       /\ c \in {
            "same_disk_no_limits",
            "same_disk_ttl",
            "same_disk_capacity",
            "same_no_disk_updates_timestamp"
          } -> FALSE
    [] OTHER -> SpecUpdatedTimestamp(c)

SpecSummaryTag(c) ==
  CASE c = "insert_disk_capacity_zero" -> "none"
    [] c \in {
         "same_disk_no_limits",
         "same_disk_ttl",
         "same_disk_capacity",
         "same_no_disk_updates_timestamp"
       } -> "old"
    [] OTHER -> "new"

ActualSummaryTag(c) ==
  CASE Bug = "update_changed_summary_keeps_old_summary"
       /\ c = "changed_disk_no_limits" -> "old"
    [] OTHER -> SpecSummaryTag(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "limit_ttl_zero_clears",
       "limit_boundary_drops",
       "limit_stale_kept",
       "limit_future_drops",
       "limit_capacity_zero_keeps_all",
       "limit_capacity_one_keeps_oldest",
       "limit_capacity_exact_drops_oldest",
       "update_insert_no_disk_persists",
       "update_insert_disk_skips_persist",
       "update_same_disk_no_limits_persists",
       "update_same_disk_ttl_skips_persist",
       "update_same_disk_capacity_skips_persist",
       "update_changed_disk_skips_persist",
       "update_same_skips_timestamp",
       "update_insert_capacity_zero_keeps_entry",
       "update_active_count_not_after_prune",
       "update_changed_summary_keeps_old_summary"
     }
  /\ checked = 0

RbcStatusRetentionMatchesSpec ==
  /\ \A c \in LimitCases:
       ActualLimit(c) = SpecLimit(c)
  /\ \A c \in UpdateCases:
       /\ ActualShouldPersist(c) = SpecShouldPersist(c)
       /\ ActualMemoryKeys(c) = SpecMemoryKeys(c)
       /\ ActualActiveCount(c) = SpecActiveCount(c)
       /\ ActualUpdatedTimestamp(c) = SpecUpdatedTimestamp(c)
       /\ ActualSummaryTag(c) = SpecSummaryTag(c)

RbcStatusRetentionExactness == RbcStatusRetentionMatchesSpec

RbcStatusRetentionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcStatusRetentionExactness

SafetyFast == RbcStatusRetentionExactness

BugLimitTtlZeroClears ==
  ActualLimit("ttl_zero_keeps_all") = SpecLimit("ttl_zero_keeps_all")

BugLimitBoundaryDrops ==
  ActualLimit("ttl_boundary_keeps") = SpecLimit("ttl_boundary_keeps")

BugLimitStaleKept ==
  ActualLimit("ttl_stale_drops") = SpecLimit("ttl_stale_drops")

BugLimitFutureDrops ==
  ActualLimit("ttl_future_kept") = SpecLimit("ttl_future_kept")

BugLimitCapacityZeroKeepsAll ==
  ActualLimit("capacity_zero_clears") = SpecLimit("capacity_zero_clears")

BugLimitCapacityOneKeepsOldest ==
  ActualLimit("capacity_one_keeps_newest") = SpecLimit("capacity_one_keeps_newest")

BugLimitCapacityExactDropsOldest ==
  ActualLimit("capacity_exact_keeps_all") = SpecLimit("capacity_exact_keeps_all")

BugUpdateInsertNoDiskPersists ==
  ActualShouldPersist("insert_no_disk") = SpecShouldPersist("insert_no_disk")

BugUpdateInsertDiskSkipsPersist ==
  ActualShouldPersist("insert_disk_no_limits") = SpecShouldPersist("insert_disk_no_limits")

BugUpdateSameDiskNoLimitsPersists ==
  ActualShouldPersist("same_disk_no_limits") = SpecShouldPersist("same_disk_no_limits")

BugUpdateSameDiskTtlSkipsPersist ==
  ActualShouldPersist("same_disk_ttl") = SpecShouldPersist("same_disk_ttl")

BugUpdateSameDiskCapacitySkipsPersist ==
  ActualShouldPersist("same_disk_capacity") = SpecShouldPersist("same_disk_capacity")

BugUpdateChangedDiskSkipsPersist ==
  ActualShouldPersist("changed_disk_no_limits") = SpecShouldPersist("changed_disk_no_limits")

BugUpdateSameSkipsTimestamp ==
  ActualUpdatedTimestamp("same_disk_no_limits") = SpecUpdatedTimestamp("same_disk_no_limits")

BugUpdateInsertCapacityZeroKeepsEntry ==
  ActualMemoryKeys("insert_disk_capacity_zero") = SpecMemoryKeys("insert_disk_capacity_zero")

BugUpdateActiveCountNotAfterPrune ==
  ActualActiveCount("insert_disk_capacity_zero") = SpecActiveCount("insert_disk_capacity_zero")

BugUpdateChangedSummaryKeepsOldSummary ==
  ActualSummaryTag("changed_disk_no_limits") = SpecSummaryTag("changed_disk_no_limits")

====
