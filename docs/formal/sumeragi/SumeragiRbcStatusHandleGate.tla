---- MODULE SumeragiRbcStatusHandleGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for RBC status handle lifecycle helpers.

This slice pins `rbc_status::Handle::{configure, remove, clear, update_at}`
and the global active-handle accessors:
- `configure(None)` clears state and disables persistence, while successful
  disk configuration replaces old state with the retained disk snapshot,
  resets the disabled metric, persists the configured store, and publishes the
  post-load active count,
- disk configuration failures clear old state, disable persistence, reset the
  disabled metric, and publish zero active sessions,
- `remove(...)` only deletes the requested key, but disk-backed stores still
  apply retention pruning and persist the resulting map before publishing the
  post-prune active count,
- `clear(...)` always clears the map and active count, persists only when an
  enabled disk store exists, and remains a no-op for disabled persistence,
- `update_at(...)` overwrites the exact test key with Plain/default metadata,
  preserves the recovered-from-disk flag, persists only when disk is enabled,
  and updates the active count from the resulting map,
- `set_active(...)`, `snapshot()`, and `sessions_active()` expose the currently
  active handle store by reference, not by copying stale snapshots.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ConfigureCases == {
  "none_with_state",
  "disk_ok_loaded",
  "disk_ok_empty",
  "disk_error_with_state",
  "disk_ok_replaces_disabled"
}

RemoveCases == {
  "remove_existing_no_disk",
  "remove_absent_no_disk",
  "remove_existing_disk",
  "remove_absent_disk_capacity_zero",
  "remove_absent_disk_ttl_prunes",
  "remove_last_disk"
}

ClearCases == {
  "clear_no_disk",
  "clear_disk",
  "clear_disabled_disk"
}

UpdateAtCases == {
  "insert_no_disk",
  "overwrite_no_disk",
  "insert_disk",
  "recovered_flag_true",
  "recovered_flag_false"
}

ActiveCases == {
  "no_active",
  "set_a",
  "switch_to_b",
  "active_tracks_clear",
  "register_fresh"
}

SpecConfigureMap(c) ==
  CASE c = "disk_ok_loaded" -> {"disk_a", "disk_b"}
    [] c = "disk_ok_replaces_disabled" -> {"disk_a"}
    [] OTHER -> {}

ActualConfigureMap(c) ==
  CASE Bug = "configure_none_keeps_map"
       /\ c = "none_with_state" -> {"old"}
    [] Bug = "configure_disk_ok_skips_load"
       /\ c = "disk_ok_loaded" -> {}
    [] Bug = "configure_disk_ok_keeps_old_map"
       /\ c = "disk_ok_loaded" -> {"old", "disk_a", "disk_b"}
    [] Bug = "configure_disk_error_keeps_map"
       /\ c = "disk_error_with_state" -> {"old"}
    [] OTHER -> SpecConfigureMap(c)

SpecConfigureDiskEnabled(c) ==
  c \in {"disk_ok_loaded", "disk_ok_empty", "disk_ok_replaces_disabled"}

ActualConfigureDiskEnabled(c) ==
  CASE Bug = "configure_none_keeps_disk"
       /\ c = "none_with_state" -> TRUE
    [] Bug = "configure_disk_ok_leaves_disabled"
       /\ c = "disk_ok_replaces_disabled" -> FALSE
    [] Bug = "configure_disk_error_keeps_disk"
       /\ c = "disk_error_with_state" -> TRUE
    [] OTHER -> SpecConfigureDiskEnabled(c)

SpecConfigurePersist(c) == SpecConfigureDiskEnabled(c)

ActualConfigurePersist(c) ==
  CASE Bug = "configure_disk_ok_skips_persist"
       /\ c = "disk_ok_loaded" -> FALSE
    [] OTHER -> SpecConfigurePersist(c)

SpecConfigureMetricReset(c) == TRUE

ActualConfigureMetricReset(c) ==
  CASE Bug = "configure_skips_metric_reset"
       /\ c = "disk_error_with_state" -> FALSE
    [] OTHER -> SpecConfigureMetricReset(c)

SpecConfigureActiveCount(c) == Cardinality(SpecConfigureMap(c))

ActualConfigureActiveCount(c) ==
  CASE Bug = "configure_active_count_old"
       /\ c = "disk_error_with_state" -> 1
    [] OTHER -> Cardinality(ActualConfigureMap(c))

SpecRemoveMap(c) ==
  CASE c \in {"remove_existing_no_disk", "remove_existing_disk"} -> {"other"}
    [] c = "remove_absent_no_disk" -> {"other"}
    [] c = "remove_absent_disk_capacity_zero" -> {}
    [] c = "remove_absent_disk_ttl_prunes" -> {"fresh"}
    [] OTHER -> {}

ActualRemoveMap(c) ==
  CASE Bug = "remove_existing_keeps_key"
       /\ c = "remove_existing_disk" -> {"target", "other"}
    [] Bug = "remove_disk_skips_retention"
       /\ c = "remove_absent_disk_capacity_zero" -> {"other"}
    [] OTHER -> SpecRemoveMap(c)

SpecRemovePersist(c) ==
  c \in {
    "remove_existing_disk",
    "remove_absent_disk_capacity_zero",
    "remove_absent_disk_ttl_prunes",
    "remove_last_disk"
  }

ActualRemovePersist(c) ==
  CASE Bug = "remove_absent_no_disk_persists"
       /\ c = "remove_absent_no_disk" -> TRUE
    [] Bug = "remove_disk_skips_persist"
       /\ c = "remove_existing_disk" -> FALSE
    [] OTHER -> SpecRemovePersist(c)

SpecRemoveActiveCount(c) == Cardinality(SpecRemoveMap(c))

ActualRemoveActiveCount(c) ==
  CASE Bug = "remove_active_count_before_prune"
       /\ c = "remove_absent_disk_capacity_zero" -> 1
    [] OTHER -> Cardinality(ActualRemoveMap(c))

SpecClearMap(c) == {}

ActualClearMap(c) ==
  CASE Bug = "clear_keeps_map"
       /\ c = "clear_disk" -> {"old"}
    [] OTHER -> SpecClearMap(c)

SpecClearPersist(c) == c = "clear_disk"

ActualClearPersist(c) ==
  CASE Bug = "clear_no_disk_persists"
       /\ c = "clear_no_disk" -> TRUE
    [] Bug = "clear_disk_skips_persist"
       /\ c = "clear_disk" -> FALSE
    [] Bug = "clear_disabled_disk_persists"
       /\ c = "clear_disabled_disk" -> TRUE
    [] OTHER -> SpecClearPersist(c)

SpecClearActiveCount(c) == 0

ActualClearActiveCount(c) ==
  CASE Bug = "clear_active_count_stale"
       /\ c = "clear_no_disk" -> 1
    [] OTHER -> Cardinality(ActualClearMap(c))

SpecUpdateAtMap(c) ==
  CASE c \in {"insert_no_disk", "insert_disk"} -> {"other", "target"}
    [] OTHER -> {"target"}

ActualUpdateAtMap(c) ==
  CASE Bug = "update_at_fails_overwrite"
       /\ c = "overwrite_no_disk" -> {"old_target"}
    [] OTHER -> SpecUpdateAtMap(c)

SpecUpdateAtPersist(c) ==
  c = "insert_disk"

ActualUpdateAtPersist(c) ==
  CASE Bug = "update_at_no_disk_persists"
       /\ c = "insert_no_disk" -> TRUE
    [] Bug = "update_at_disk_skips_persist"
       /\ c = "insert_disk" -> FALSE
    [] OTHER -> SpecUpdateAtPersist(c)

SpecUpdateAtDefaultsOk(c) == TRUE

ActualUpdateAtDefaultsOk(c) ==
  CASE Bug = "update_at_wrong_defaults"
       /\ c = "insert_disk" -> FALSE
    [] OTHER -> SpecUpdateAtDefaultsOk(c)

SpecUpdateAtRecovered(c) ==
  c = "recovered_flag_true"

ActualUpdateAtRecovered(c) ==
  CASE Bug = "update_at_drops_recovered_flag"
       /\ c = "recovered_flag_true" -> FALSE
    [] OTHER -> SpecUpdateAtRecovered(c)

SpecUpdateAtActiveCount(c) == Cardinality(SpecUpdateAtMap(c))

ActualUpdateAtActiveCount(c) ==
  CASE Bug = "update_at_active_count_stale"
       /\ c = "insert_no_disk" -> 1
    [] OTHER -> Cardinality(ActualUpdateAtMap(c))

SpecGlobalSnapshot(c) ==
  CASE c = "set_a" -> {"a1", "a2"}
    [] c = "switch_to_b" -> {"b1"}
    [] OTHER -> {}

ActualGlobalSnapshot(c) ==
  CASE Bug = "global_no_active_nonempty"
       /\ c = "no_active" -> {"stale"}
    [] Bug = "global_set_active_ignores_handle"
       /\ c = "set_a" -> {}
    [] Bug = "global_switch_keeps_old"
       /\ c = "switch_to_b" -> {"a1", "a2"}
    [] Bug = "global_active_snapshot_copied_not_shared"
       /\ c = "active_tracks_clear" -> {"a1"}
    [] Bug = "register_reuses_active_store"
       /\ c = "register_fresh" -> {"a1"}
    [] OTHER -> SpecGlobalSnapshot(c)

SpecGlobalActiveCount(c) == Cardinality(SpecGlobalSnapshot(c))

ActualGlobalActiveCount(c) == Cardinality(ActualGlobalSnapshot(c))

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "configure_none_keeps_map",
       "configure_none_keeps_disk",
       "configure_disk_ok_skips_load",
       "configure_disk_ok_keeps_old_map",
       "configure_disk_ok_leaves_disabled",
       "configure_disk_ok_skips_persist",
       "configure_disk_error_keeps_map",
       "configure_disk_error_keeps_disk",
       "configure_active_count_old",
       "configure_skips_metric_reset",
       "remove_existing_keeps_key",
       "remove_absent_no_disk_persists",
       "remove_disk_skips_persist",
       "remove_disk_skips_retention",
       "remove_active_count_before_prune",
       "clear_keeps_map",
       "clear_no_disk_persists",
       "clear_disk_skips_persist",
       "clear_disabled_disk_persists",
       "clear_active_count_stale",
       "update_at_no_disk_persists",
       "update_at_disk_skips_persist",
       "update_at_fails_overwrite",
       "update_at_wrong_defaults",
       "update_at_drops_recovered_flag",
       "update_at_active_count_stale",
       "global_no_active_nonempty",
       "global_set_active_ignores_handle",
       "global_switch_keeps_old",
       "global_active_snapshot_copied_not_shared",
       "register_reuses_active_store"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in ConfigureCases:
       /\ ActualConfigureMap(c) = SpecConfigureMap(c)
       /\ ActualConfigureDiskEnabled(c) = SpecConfigureDiskEnabled(c)
       /\ ActualConfigurePersist(c) = SpecConfigurePersist(c)
       /\ ActualConfigureMetricReset(c) = SpecConfigureMetricReset(c)
       /\ ActualConfigureActiveCount(c) = SpecConfigureActiveCount(c)
  /\ \A c \in RemoveCases:
       /\ ActualRemoveMap(c) = SpecRemoveMap(c)
       /\ ActualRemovePersist(c) = SpecRemovePersist(c)
       /\ ActualRemoveActiveCount(c) = SpecRemoveActiveCount(c)
  /\ \A c \in ClearCases:
       /\ ActualClearMap(c) = SpecClearMap(c)
       /\ ActualClearPersist(c) = SpecClearPersist(c)
       /\ ActualClearActiveCount(c) = SpecClearActiveCount(c)
  /\ \A c \in UpdateAtCases:
       /\ ActualUpdateAtMap(c) = SpecUpdateAtMap(c)
       /\ ActualUpdateAtPersist(c) = SpecUpdateAtPersist(c)
       /\ ActualUpdateAtDefaultsOk(c) = SpecUpdateAtDefaultsOk(c)
       /\ ActualUpdateAtRecovered(c) = SpecUpdateAtRecovered(c)
       /\ ActualUpdateAtActiveCount(c) = SpecUpdateAtActiveCount(c)
  /\ \A c \in ActiveCases:
       /\ ActualGlobalSnapshot(c) = SpecGlobalSnapshot(c)
       /\ ActualGlobalActiveCount(c) = SpecGlobalActiveCount(c)

BugConfigureNoneKeepsMap ==
  ActualConfigureMap("none_with_state") = SpecConfigureMap("none_with_state")

BugConfigureNoneKeepsDisk ==
  ActualConfigureDiskEnabled("none_with_state") =
    SpecConfigureDiskEnabled("none_with_state")

BugConfigureDiskOkSkipsLoad ==
  ActualConfigureMap("disk_ok_loaded") = SpecConfigureMap("disk_ok_loaded")

BugConfigureDiskOkKeepsOldMap ==
  ActualConfigureMap("disk_ok_loaded") = SpecConfigureMap("disk_ok_loaded")

BugConfigureDiskOkLeavesDisabled ==
  ActualConfigureDiskEnabled("disk_ok_replaces_disabled") =
    SpecConfigureDiskEnabled("disk_ok_replaces_disabled")

BugConfigureDiskOkSkipsPersist ==
  ActualConfigurePersist("disk_ok_loaded") = SpecConfigurePersist("disk_ok_loaded")

BugConfigureDiskErrorKeepsMap ==
  ActualConfigureMap("disk_error_with_state") = SpecConfigureMap("disk_error_with_state")

BugConfigureDiskErrorKeepsDisk ==
  ActualConfigureDiskEnabled("disk_error_with_state") =
    SpecConfigureDiskEnabled("disk_error_with_state")

BugConfigureActiveCountOld ==
  ActualConfigureActiveCount("disk_error_with_state") =
    SpecConfigureActiveCount("disk_error_with_state")

BugConfigureSkipsMetricReset ==
  ActualConfigureMetricReset("disk_error_with_state") =
    SpecConfigureMetricReset("disk_error_with_state")

BugRemoveExistingKeepsKey ==
  ActualRemoveMap("remove_existing_disk") = SpecRemoveMap("remove_existing_disk")

BugRemoveAbsentNoDiskPersists ==
  ActualRemovePersist("remove_absent_no_disk") = SpecRemovePersist("remove_absent_no_disk")

BugRemoveDiskSkipsPersist ==
  ActualRemovePersist("remove_existing_disk") = SpecRemovePersist("remove_existing_disk")

BugRemoveDiskSkipsRetention ==
  ActualRemoveMap("remove_absent_disk_capacity_zero") =
    SpecRemoveMap("remove_absent_disk_capacity_zero")

BugRemoveActiveCountBeforePrune ==
  ActualRemoveActiveCount("remove_absent_disk_capacity_zero") =
    SpecRemoveActiveCount("remove_absent_disk_capacity_zero")

BugClearKeepsMap ==
  ActualClearMap("clear_disk") = SpecClearMap("clear_disk")

BugClearNoDiskPersists ==
  ActualClearPersist("clear_no_disk") = SpecClearPersist("clear_no_disk")

BugClearDiskSkipsPersist ==
  ActualClearPersist("clear_disk") = SpecClearPersist("clear_disk")

BugClearDisabledDiskPersists ==
  ActualClearPersist("clear_disabled_disk") = SpecClearPersist("clear_disabled_disk")

BugClearActiveCountStale ==
  ActualClearActiveCount("clear_no_disk") = SpecClearActiveCount("clear_no_disk")

BugUpdateAtNoDiskPersists ==
  ActualUpdateAtPersist("insert_no_disk") = SpecUpdateAtPersist("insert_no_disk")

BugUpdateAtDiskSkipsPersist ==
  ActualUpdateAtPersist("insert_disk") = SpecUpdateAtPersist("insert_disk")

BugUpdateAtFailsOverwrite ==
  ActualUpdateAtMap("overwrite_no_disk") = SpecUpdateAtMap("overwrite_no_disk")

BugUpdateAtWrongDefaults ==
  ActualUpdateAtDefaultsOk("insert_disk") = SpecUpdateAtDefaultsOk("insert_disk")

BugUpdateAtDropsRecoveredFlag ==
  ActualUpdateAtRecovered("recovered_flag_true") =
    SpecUpdateAtRecovered("recovered_flag_true")

BugUpdateAtActiveCountStale ==
  ActualUpdateAtActiveCount("insert_no_disk") = SpecUpdateAtActiveCount("insert_no_disk")

BugGlobalNoActiveNonempty ==
  ActualGlobalSnapshot("no_active") = SpecGlobalSnapshot("no_active")

BugGlobalSetActiveIgnoresHandle ==
  ActualGlobalSnapshot("set_a") = SpecGlobalSnapshot("set_a")

BugGlobalSwitchKeepsOld ==
  ActualGlobalSnapshot("switch_to_b") = SpecGlobalSnapshot("switch_to_b")

BugGlobalActiveSnapshotCopiedNotShared ==
  ActualGlobalSnapshot("active_tracks_clear") = SpecGlobalSnapshot("active_tracks_clear")

BugRegisterReusesActiveStore ==
  ActualGlobalSnapshot("register_fresh") = SpecGlobalSnapshot("register_fresh")

====
