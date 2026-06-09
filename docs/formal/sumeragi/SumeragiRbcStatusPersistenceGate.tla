---- MODULE SumeragiRbcStatusPersistenceGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for RBC status disk persistence helpers.

This slice pins `rbc_status::{read_entries_with_fallback,
promote_temp_store, temp_store_path, persist_if_needed}`:
- persisted snapshots prefer a valid main store over any temp store,
- invalid decoded stores are removed, but unreadable stores are only ignored,
- a valid temp store is selected and promoted only when no valid main store is
  available,
- temp promotion handles the AlreadyExists retry by removing the main store
  before retrying the rename, and syncs the parent directory only after a
  successful promotion with a nonempty parent,
- fatal persistence errors disable disk persistence and record the disabled
  metric/failure counter, while nonfatal errors leave disk persistence enabled,
- temp paths append `.tmp` without replacing the existing extension.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ReadCases == {
  "no_files",
  "main_valid_tmp_absent",
  "main_valid_tmp_valid",
  "main_valid_tmp_invalid",
  "main_invalid_tmp_valid",
  "main_invalid_tmp_invalid",
  "main_invalid_tmp_absent",
  "main_absent_tmp_valid",
  "main_absent_tmp_invalid",
  "main_read_error_tmp_valid",
  "tmp_read_error_main_valid"
}

PromoteCases == {
  "rename_ok",
  "already_exists_remove_ok_rename_ok",
  "already_exists_remove_fails",
  "already_exists_rename_fails",
  "other_error",
  "parent_empty"
}

PersistCases == {
  "no_disk",
  "disabled",
  "success",
  "nonfatal",
  "fatal_storage_full",
  "fatal_write_zero",
  "fatal_out_of_memory",
  "fatal_file_too_large",
  "fatal_quota"
}

TempPathCases == {
  "with_ext",
  "no_ext",
  "multi_ext"
}

FatalPersistCase(c) ==
  c \in {
    "fatal_storage_full",
    "fatal_write_zero",
    "fatal_out_of_memory",
    "fatal_file_too_large",
    "fatal_quota"
  }

SpecReadResult(c) ==
  CASE c \in {
         "main_valid_tmp_absent",
         "main_valid_tmp_valid",
         "main_valid_tmp_invalid",
         "tmp_read_error_main_valid"
       } -> "main"
    [] c \in {
         "main_invalid_tmp_valid",
         "main_absent_tmp_valid",
         "main_read_error_tmp_valid"
       } -> "tmp"
    [] OTHER -> "empty"

ActualReadResult(c) ==
  CASE Bug = "read_prefers_tmp_over_main"
       /\ c = "main_valid_tmp_valid" -> "tmp"
    [] Bug = "read_rejects_valid_tmp_after_invalid_main"
       /\ c = "main_invalid_tmp_valid" -> "empty"
    [] Bug = "read_accepts_invalid_main"
       /\ c = "main_invalid_tmp_valid" -> "main"
    [] Bug = "read_errors_block_tmp_fallback"
       /\ c = "main_read_error_tmp_valid" -> "empty"
    [] Bug = "read_tmp_error_blocks_valid_main"
       /\ c = "tmp_read_error_main_valid" -> "empty"
    [] OTHER -> SpecReadResult(c)

SpecPromoteAfterRead(c) ==
  SpecReadResult(c) = "tmp"

ActualPromoteAfterRead(c) ==
  CASE Bug = "read_skips_tmp_promotion"
       /\ c = "main_absent_tmp_valid" -> FALSE
    [] Bug = "read_promotes_when_main_selected"
       /\ c = "main_valid_tmp_valid" -> TRUE
    [] OTHER -> SpecPromoteAfterRead(c)

SpecRemoveMainAfterRead(c) ==
  c \in {
    "main_invalid_tmp_valid",
    "main_invalid_tmp_invalid",
    "main_invalid_tmp_absent"
  }

ActualRemoveMainAfterRead(c) ==
  CASE Bug = "read_keeps_invalid_main"
       /\ c = "main_invalid_tmp_invalid" -> FALSE
    [] Bug = "read_removes_main_on_read_error"
       /\ c = "main_read_error_tmp_valid" -> TRUE
    [] OTHER -> SpecRemoveMainAfterRead(c)

SpecRemoveTmpAfterRead(c) ==
  c \in {"main_invalid_tmp_invalid", "main_absent_tmp_invalid"}

ActualRemoveTmpAfterRead(c) ==
  CASE Bug = "read_keeps_invalid_tmp"
       /\ c = "main_absent_tmp_invalid" -> FALSE
    [] Bug = "read_decodes_tmp_despite_valid_main"
       /\ c = "main_valid_tmp_invalid" -> TRUE
    [] OTHER -> SpecRemoveTmpAfterRead(c)

SpecPromoted(c) ==
  c \in {"rename_ok", "already_exists_remove_ok_rename_ok", "parent_empty"}

ActualPromoted(c) ==
  CASE Bug = "promote_rejects_direct_rename"
       /\ c = "rename_ok" -> FALSE
    [] Bug = "promote_accepts_remove_failure"
       /\ c = "already_exists_remove_fails" -> TRUE
    [] Bug = "promote_accepts_retry_rename_failure"
       /\ c = "already_exists_rename_fails" -> TRUE
    [] Bug = "promote_accepts_other_error"
       /\ c = "other_error" -> TRUE
    [] OTHER -> SpecPromoted(c)

SpecRemoveBeforeRetry(c) ==
  c \in {
    "already_exists_remove_ok_rename_ok",
    "already_exists_remove_fails",
    "already_exists_rename_fails"
  }

ActualRemoveBeforeRetry(c) ==
  CASE Bug = "promote_skips_remove_before_retry"
       /\ c = "already_exists_remove_ok_rename_ok" -> FALSE
    [] Bug = "promote_removes_on_other_error"
       /\ c = "other_error" -> TRUE
    [] OTHER -> SpecRemoveBeforeRetry(c)

SpecRetryRename(c) ==
  c \in {"already_exists_remove_ok_rename_ok", "already_exists_rename_fails"}

ActualRetryRename(c) ==
  CASE Bug = "promote_skips_retry_rename"
       /\ c = "already_exists_remove_ok_rename_ok" -> FALSE
    [] Bug = "promote_retries_after_remove_failure"
       /\ c = "already_exists_remove_fails" -> TRUE
    [] OTHER -> SpecRetryRename(c)

SpecSyncParent(c) ==
  c \in {"rename_ok", "already_exists_remove_ok_rename_ok"}

ActualSyncParent(c) ==
  CASE Bug = "promote_skips_parent_sync"
       /\ c = "rename_ok" -> FALSE
    [] Bug = "promote_syncs_empty_parent"
       /\ c = "parent_empty" -> TRUE
    [] Bug = "promote_syncs_failed_promotion"
       /\ c = "other_error" -> TRUE
    [] OTHER -> SpecSyncParent(c)

SpecPersistAttempt(c) ==
  c \in {
    "success",
    "nonfatal",
    "fatal_storage_full",
    "fatal_write_zero",
    "fatal_out_of_memory",
    "fatal_file_too_large",
    "fatal_quota"
  }

ActualPersistAttempt(c) ==
  CASE Bug = "persist_attempts_without_disk"
       /\ c = "no_disk" -> TRUE
    [] Bug = "persist_attempts_when_disabled"
       /\ c = "disabled" -> TRUE
    [] Bug = "persist_skips_success"
       /\ c = "success" -> FALSE
    [] OTHER -> SpecPersistAttempt(c)

SpecDiskDisabledAfter(c) ==
  c = "disabled" \/ FatalPersistCase(c)

ActualDiskDisabledAfter(c) ==
  CASE Bug = "persist_disables_on_nonfatal"
       /\ c = "nonfatal" -> TRUE
    [] Bug = "persist_keeps_enabled_on_fatal"
       /\ c = "fatal_storage_full" -> FALSE
    [] Bug = "persist_treats_write_zero_nonfatal"
       /\ c = "fatal_write_zero" -> FALSE
    [] Bug = "persist_treats_quota_nonfatal"
       /\ c = "fatal_quota" -> FALSE
    [] OTHER -> SpecDiskDisabledAfter(c)

SpecFatalMetric(c) ==
  FatalPersistCase(c)

ActualFatalMetric(c) ==
  CASE Bug = "persist_skips_fatal_metric"
       /\ c = "fatal_storage_full" -> FALSE
    [] Bug = "persist_records_metric_on_nonfatal"
       /\ c = "nonfatal" -> TRUE
    [] OTHER -> SpecFatalMetric(c)

SpecTempPath(c) ==
  CASE c = "with_ext" -> "sessions.norito.tmp"
    [] c = "no_ext" -> "sessions.tmp"
    [] OTHER -> "sessions.norito.v1.tmp"

ActualTempPath(c) ==
  CASE Bug = "temp_replaces_extension"
       /\ c = "with_ext" -> "sessions.tmp"
    [] Bug = "temp_drops_multi_extension"
       /\ c = "multi_ext" -> "sessions.tmp"
    [] OTHER -> SpecTempPath(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "read_prefers_tmp_over_main",
       "read_rejects_valid_tmp_after_invalid_main",
       "read_accepts_invalid_main",
       "read_errors_block_tmp_fallback",
       "read_tmp_error_blocks_valid_main",
       "read_skips_tmp_promotion",
       "read_promotes_when_main_selected",
       "read_keeps_invalid_main",
       "read_removes_main_on_read_error",
       "read_keeps_invalid_tmp",
       "read_decodes_tmp_despite_valid_main",
       "promote_rejects_direct_rename",
       "promote_accepts_remove_failure",
       "promote_accepts_retry_rename_failure",
       "promote_accepts_other_error",
       "promote_skips_remove_before_retry",
       "promote_removes_on_other_error",
       "promote_skips_retry_rename",
       "promote_retries_after_remove_failure",
       "promote_skips_parent_sync",
       "promote_syncs_empty_parent",
       "promote_syncs_failed_promotion",
       "persist_attempts_without_disk",
       "persist_attempts_when_disabled",
       "persist_skips_success",
       "persist_disables_on_nonfatal",
       "persist_keeps_enabled_on_fatal",
       "persist_treats_write_zero_nonfatal",
       "persist_treats_quota_nonfatal",
       "persist_skips_fatal_metric",
       "persist_records_metric_on_nonfatal",
       "temp_replaces_extension",
       "temp_drops_multi_extension"
     }
  /\ checked = 0

RbcStatusPersistenceMatchesSpec ==
  /\ \A c \in ReadCases:
       /\ ActualReadResult(c) = SpecReadResult(c)
       /\ ActualPromoteAfterRead(c) = SpecPromoteAfterRead(c)
       /\ ActualRemoveMainAfterRead(c) = SpecRemoveMainAfterRead(c)
       /\ ActualRemoveTmpAfterRead(c) = SpecRemoveTmpAfterRead(c)
  /\ \A c \in PromoteCases:
       /\ ActualPromoted(c) = SpecPromoted(c)
       /\ ActualRemoveBeforeRetry(c) = SpecRemoveBeforeRetry(c)
       /\ ActualRetryRename(c) = SpecRetryRename(c)
       /\ ActualSyncParent(c) = SpecSyncParent(c)
  /\ \A c \in PersistCases:
       /\ ActualPersistAttempt(c) = SpecPersistAttempt(c)
       /\ ActualDiskDisabledAfter(c) = SpecDiskDisabledAfter(c)
       /\ ActualFatalMetric(c) = SpecFatalMetric(c)
  /\ \A c \in TempPathCases:
       ActualTempPath(c) = SpecTempPath(c)

SafetyFast ==
  RbcStatusPersistenceMatchesSpec

BugReadPrefersTmpOverMain ==
  ActualReadResult("main_valid_tmp_valid") = SpecReadResult("main_valid_tmp_valid")

BugReadRejectsValidTmpAfterInvalidMain ==
  ActualReadResult("main_invalid_tmp_valid") = SpecReadResult("main_invalid_tmp_valid")

BugReadAcceptsInvalidMain ==
  ActualReadResult("main_invalid_tmp_valid") = SpecReadResult("main_invalid_tmp_valid")

BugReadErrorsBlockTmpFallback ==
  ActualReadResult("main_read_error_tmp_valid") = SpecReadResult("main_read_error_tmp_valid")

BugReadTmpErrorBlocksValidMain ==
  ActualReadResult("tmp_read_error_main_valid") = SpecReadResult("tmp_read_error_main_valid")

BugReadSkipsTmpPromotion ==
  ActualPromoteAfterRead("main_absent_tmp_valid") = SpecPromoteAfterRead("main_absent_tmp_valid")

BugReadPromotesWhenMainSelected ==
  ActualPromoteAfterRead("main_valid_tmp_valid") = SpecPromoteAfterRead("main_valid_tmp_valid")

BugReadKeepsInvalidMain ==
  ActualRemoveMainAfterRead("main_invalid_tmp_invalid") =
    SpecRemoveMainAfterRead("main_invalid_tmp_invalid")

BugReadRemovesMainOnReadError ==
  ActualRemoveMainAfterRead("main_read_error_tmp_valid") =
    SpecRemoveMainAfterRead("main_read_error_tmp_valid")

BugReadKeepsInvalidTmp ==
  ActualRemoveTmpAfterRead("main_absent_tmp_invalid") =
    SpecRemoveTmpAfterRead("main_absent_tmp_invalid")

BugReadDecodesTmpDespiteValidMain ==
  ActualRemoveTmpAfterRead("main_valid_tmp_invalid") =
    SpecRemoveTmpAfterRead("main_valid_tmp_invalid")

BugPromoteRejectsDirectRename ==
  ActualPromoted("rename_ok") = SpecPromoted("rename_ok")

BugPromoteAcceptsRemoveFailure ==
  ActualPromoted("already_exists_remove_fails") =
    SpecPromoted("already_exists_remove_fails")

BugPromoteAcceptsRetryRenameFailure ==
  ActualPromoted("already_exists_rename_fails") =
    SpecPromoted("already_exists_rename_fails")

BugPromoteAcceptsOtherError ==
  ActualPromoted("other_error") = SpecPromoted("other_error")

BugPromoteSkipsRemoveBeforeRetry ==
  ActualRemoveBeforeRetry("already_exists_remove_ok_rename_ok") =
    SpecRemoveBeforeRetry("already_exists_remove_ok_rename_ok")

BugPromoteRemovesOnOtherError ==
  ActualRemoveBeforeRetry("other_error") = SpecRemoveBeforeRetry("other_error")

BugPromoteSkipsRetryRename ==
  ActualRetryRename("already_exists_remove_ok_rename_ok") =
    SpecRetryRename("already_exists_remove_ok_rename_ok")

BugPromoteRetriesAfterRemoveFailure ==
  ActualRetryRename("already_exists_remove_fails") =
    SpecRetryRename("already_exists_remove_fails")

BugPromoteSkipsParentSync ==
  ActualSyncParent("rename_ok") = SpecSyncParent("rename_ok")

BugPromoteSyncsEmptyParent ==
  ActualSyncParent("parent_empty") = SpecSyncParent("parent_empty")

BugPromoteSyncsFailedPromotion ==
  ActualSyncParent("other_error") = SpecSyncParent("other_error")

BugPersistAttemptsWithoutDisk ==
  ActualPersistAttempt("no_disk") = SpecPersistAttempt("no_disk")

BugPersistAttemptsWhenDisabled ==
  ActualPersistAttempt("disabled") = SpecPersistAttempt("disabled")

BugPersistSkipsSuccess ==
  ActualPersistAttempt("success") = SpecPersistAttempt("success")

BugPersistDisablesOnNonfatal ==
  ActualDiskDisabledAfter("nonfatal") = SpecDiskDisabledAfter("nonfatal")

BugPersistKeepsEnabledOnFatal ==
  ActualDiskDisabledAfter("fatal_storage_full") = SpecDiskDisabledAfter("fatal_storage_full")

BugPersistTreatsWriteZeroNonfatal ==
  ActualDiskDisabledAfter("fatal_write_zero") = SpecDiskDisabledAfter("fatal_write_zero")

BugPersistTreatsQuotaNonfatal ==
  ActualDiskDisabledAfter("fatal_quota") = SpecDiskDisabledAfter("fatal_quota")

BugPersistSkipsFatalMetric ==
  ActualFatalMetric("fatal_storage_full") = SpecFatalMetric("fatal_storage_full")

BugPersistRecordsMetricOnNonfatal ==
  ActualFatalMetric("nonfatal") = SpecFatalMetric("nonfatal")

BugTempReplacesExtension ==
  ActualTempPath("with_ext") = SpecTempPath("with_ext")

BugTempDropsMultiExtension ==
  ActualTempPath("multi_ext") = SpecTempPath("multi_ext")

====
