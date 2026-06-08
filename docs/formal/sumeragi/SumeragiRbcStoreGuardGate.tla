---- MODULE SumeragiRbcStoreGuardGate ----
EXTENDS Integers, FiniteSets

(***************************************************************************
A bounded abstract model for persisted RBC session-store guard behavior.

This slice pins `rbc_store` helpers that sit below restart recovery and chunk
sampling:
- `SoftwareManifest::matches(...)` only accepts identical version/profile and
  matching commit-hash presence/value,
- direct session loads treat absent files as `None`, propagate read errors,
  prefer valid temp snapshots before main snapshots, fall back from invalid
  temp snapshots to valid main snapshots, and do not require temp promotion to
  succeed before returning a valid temp snapshot,
- persisted-session validation rejects and deletes invalid, wrong-key,
  unsupported-version, wrong-chain, wrong-manifest, and chunk-integrity
  failures,
- chunk integrity rejects malformed zero-total sessions, overfull or
  duplicate/out-of-range chunks, malformed READY/DELIVER metadata, and
  mismatched payload/digest/root evidence while deferring payload checks for
  incomplete sessions,
- limit enforcement disables storage when either hard cap is zero, keeps exact
  TTL/soft boundaries, evicts future/stale/oldest excess entries, and reports
  hard vs soft pressure accurately,
- file helpers append `.tmp` without replacing existing extensions and only
  classify three-part `.norito` / `.norito.tmp` session files as RBC sessions.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ManifestCases == {
  "same_with_commit",
  "same_missing_commit",
  "missing_vs_present",
  "present_vs_missing",
  "version_mismatch",
  "profile_mismatch",
  "commit_mismatch"
}

LoadCases == {
  "both_absent",
  "temp_valid_only",
  "main_valid_only",
  "temp_invalid_main_valid",
  "temp_valid_main_valid",
  "temp_read_error_main_valid",
  "main_read_error_temp_valid",
  "temp_valid_promote_fails",
  "inspect_invalid_retained",
  "inspect_main_priority"
}

ValidateCases == {
  "valid",
  "invalid_flag",
  "key_mismatch",
  "version_mismatch",
  "chain_mismatch",
  "manifest_mismatch",
  "chunk_invalid",
  "zero_total_clean",
  "zero_total_with_chunk",
  "zero_total_with_digest",
  "too_many_chunks",
  "duplicate_chunk",
  "chunk_out_of_range",
  "empty_ready_sig",
  "duplicate_ready",
  "ready_sender_oob",
  "deliver_sender_oob",
  "delivered_missing_sender",
  "delivered_empty_sig",
  "payload_hash_mismatch",
  "digest_mismatch",
  "expected_root_mismatch",
  "computed_root_mismatch",
  "incomplete_payload_hash_deferred"
}

LimitCases == {
  "disabled",
  "ttl_disabled_old",
  "ttl_boundary",
  "ttl_stale",
  "ttl_future",
  "capacity_exact",
  "capacity_over_sessions",
  "bytes_exact",
  "bytes_over_oldest",
  "soft_session_boundary",
  "soft_session_over",
  "soft_bytes_boundary",
  "soft_bytes_over"
}

TempPathCases == {
  "path_norito",
  "path_no_ext",
  "path_multi"
}

FileCases == {
  "valid_session",
  "valid_temp",
  "status_snapshot",
  "two_part",
  "four_part",
  "temp_two_part"
}

SpecManifestMatches(c) ==
  c \in {"same_with_commit", "same_missing_commit"}

ActualManifestMatches(c) ==
  CASE Bug = "manifest_accept_version_mismatch"
       /\ c = "version_mismatch" -> TRUE
    [] Bug = "manifest_accept_profile_mismatch"
       /\ c = "profile_mismatch" -> TRUE
    [] Bug = "manifest_accept_commit_mismatch"
       /\ c = "commit_mismatch" -> TRUE
    [] Bug = "manifest_reject_both_missing"
       /\ c = "same_missing_commit" -> FALSE
    [] Bug = "manifest_accept_missing_present"
       /\ c \in {"missing_vs_present", "present_vs_missing"} -> TRUE
    [] OTHER -> SpecManifestMatches(c)

SpecLoadChoice(c) ==
  CASE c = "both_absent" -> "none"
    [] c \in {"temp_read_error_main_valid", "main_read_error_temp_valid"} -> "io_error"
    [] c \in {"temp_valid_only", "temp_valid_main_valid", "temp_valid_promote_fails"} -> "temp"
    [] c \in {"main_valid_only", "temp_invalid_main_valid", "inspect_main_priority"} -> "main"
    [] OTHER -> "none"

ActualLoadChoice(c) ==
  CASE Bug = "load_absent_errors"
       /\ c = "both_absent" -> "io_error"
    [] Bug = "load_temp_invalid_blocks_main"
       /\ c = "temp_invalid_main_valid" -> "none"
    [] Bug = "load_prefers_main_over_temp"
       /\ c = "temp_valid_main_valid" -> "main"
    [] Bug = "load_temp_read_error_falls_back"
       /\ c = "temp_read_error_main_valid" -> "main"
    [] Bug = "load_main_read_error_returns_temp"
       /\ c = "main_read_error_temp_valid" -> "temp"
    [] Bug = "load_requires_promotion_success"
       /\ c = "temp_valid_promote_fails" -> "none"
    [] Bug = "inspect_prefers_temp_over_main"
       /\ c = "inspect_main_priority" -> "temp"
    [] OTHER -> SpecLoadChoice(c)

SpecLoadDeletesTemp(c) ==
  c = "temp_invalid_main_valid"

ActualLoadDeletesTemp(c) ==
  CASE Bug = "load_invalid_temp_kept"
       /\ c = "temp_invalid_main_valid" -> FALSE
    [] Bug = "inspect_deletes_invalid"
       /\ c = "inspect_invalid_retained" -> TRUE
    [] OTHER -> SpecLoadDeletesTemp(c)

SpecValidateAccept(c) ==
  c \in {"valid", "zero_total_clean", "incomplete_payload_hash_deferred"}

ActualValidateAccept(c) ==
  CASE Bug = "validate_accept_invalid_flag"
       /\ c = "invalid_flag" -> TRUE
    [] Bug = "validate_accept_key_mismatch"
       /\ c = "key_mismatch" -> TRUE
    [] Bug = "validate_accept_version_mismatch"
       /\ c = "version_mismatch" -> TRUE
    [] Bug = "validate_accept_chain_mismatch"
       /\ c = "chain_mismatch" -> TRUE
    [] Bug = "validate_accept_manifest_mismatch"
       /\ c = "manifest_mismatch" -> TRUE
    [] Bug = "validate_accept_chunk_invalid"
       /\ c = "chunk_invalid" -> TRUE
    [] Bug = "validate_reject_zero_clean"
       /\ c = "zero_total_clean" -> FALSE
    [] Bug = "validate_accept_zero_with_chunk"
       /\ c = "zero_total_with_chunk" -> TRUE
    [] Bug = "validate_accept_zero_with_digest"
       /\ c = "zero_total_with_digest" -> TRUE
    [] Bug = "validate_accept_too_many_chunks"
       /\ c = "too_many_chunks" -> TRUE
    [] Bug = "validate_accept_duplicate_chunk"
       /\ c = "duplicate_chunk" -> TRUE
    [] Bug = "validate_accept_chunk_out_of_range"
       /\ c = "chunk_out_of_range" -> TRUE
    [] Bug = "validate_accept_empty_ready_sig"
       /\ c = "empty_ready_sig" -> TRUE
    [] Bug = "validate_accept_duplicate_ready"
       /\ c = "duplicate_ready" -> TRUE
    [] Bug = "validate_accept_ready_oob"
       /\ c = "ready_sender_oob" -> TRUE
    [] Bug = "validate_accept_deliver_oob"
       /\ c = "deliver_sender_oob" -> TRUE
    [] Bug = "validate_accept_delivered_missing_sender"
       /\ c = "delivered_missing_sender" -> TRUE
    [] Bug = "validate_accept_delivered_empty_sig"
       /\ c = "delivered_empty_sig" -> TRUE
    [] Bug = "validate_accept_payload_hash_mismatch"
       /\ c = "payload_hash_mismatch" -> TRUE
    [] Bug = "validate_accept_digest_mismatch"
       /\ c = "digest_mismatch" -> TRUE
    [] Bug = "validate_accept_expected_root_mismatch"
       /\ c = "expected_root_mismatch" -> TRUE
    [] Bug = "validate_accept_computed_root_mismatch"
       /\ c = "computed_root_mismatch" -> TRUE
    [] Bug = "validate_reject_incomplete_payload_hash"
       /\ c = "incomplete_payload_hash_deferred" -> FALSE
    [] OTHER -> SpecValidateAccept(c)

SpecValidateDeletes(c) ==
  ~SpecValidateAccept(c)

ActualValidateDeletes(c) ==
  CASE Bug = "validate_keeps_rejected_file"
       /\ c = "invalid_flag" -> FALSE
    [] OTHER -> SpecValidateDeletes(c)

SpecLimitKeys(c) ==
  CASE c = "disabled" -> {}
    [] c = "ttl_disabled_old" -> {"old", "fresh"}
    [] c = "ttl_boundary" -> {"old"}
    [] c = "ttl_stale" -> {}
    [] c = "ttl_future" -> {}
    [] c = "capacity_exact" -> {"old", "new"}
    [] c = "capacity_over_sessions" -> {"mid", "new"}
    [] c = "bytes_exact" -> {"old", "new"}
    [] c = "bytes_over_oldest" -> {"new"}
    [] c = "soft_session_boundary" -> {"a", "b"}
    [] c = "soft_session_over" -> {"a", "b", "c"}
    [] c = "soft_bytes_boundary" -> {"a", "b"}
    [] c = "soft_bytes_over" -> {"a", "b"}
    [] OTHER -> {}

ActualLimitKeys(c) ==
  CASE Bug = "limit_disabled_retains"
       /\ c = "disabled" -> {"old"}
    [] Bug = "limit_ttl_zero_drops"
       /\ c = "ttl_disabled_old" -> {"fresh"}
    [] Bug = "limit_ttl_boundary_drops"
       /\ c = "ttl_boundary" -> {}
    [] Bug = "limit_ttl_stale_kept"
       /\ c = "ttl_stale" -> {"old"}
    [] Bug = "limit_future_kept"
       /\ c = "ttl_future" -> {"future"}
    [] Bug = "limit_max_sessions_keeps_oldest"
       /\ c = "capacity_over_sessions" -> {"old", "mid"}
    [] Bug = "limit_bytes_keeps_oldest"
       /\ c = "bytes_over_oldest" -> {"old"}
    [] OTHER -> SpecLimitKeys(c)

SpecLimitPressure(c) ==
  CASE c \in {"capacity_over_sessions", "bytes_over_oldest"} -> "hard"
    [] c \in {"soft_session_over", "soft_bytes_over"} -> "soft"
    [] OTHER -> "normal"

ActualLimitPressure(c) ==
  CASE Bug = "limit_max_sessions_no_hard"
       /\ c = "capacity_over_sessions" -> "normal"
    [] Bug = "limit_bytes_no_hard"
       /\ c = "bytes_over_oldest" -> "normal"
    [] Bug = "limit_soft_boundary_triggers"
       /\ c = "soft_session_boundary" -> "soft"
    [] Bug = "limit_soft_over_normal"
       /\ c = "soft_session_over" -> "normal"
    [] Bug = "limit_soft_bytes_boundary_triggers"
       /\ c = "soft_bytes_boundary" -> "soft"
    [] Bug = "limit_soft_bytes_over_normal"
       /\ c = "soft_bytes_over" -> "normal"
    [] OTHER -> SpecLimitPressure(c)

SpecTempPath(c) ==
  CASE c = "path_norito" -> "session.norito.tmp"
    [] c = "path_multi" -> "session.a.norito.tmp"
    [] OTHER -> "session.tmp"

ActualTempPath(c) ==
  CASE Bug = "temp_replaces_extension"
       /\ c = "path_norito" -> "session.tmp"
    [] Bug = "temp_drops_multi_extension"
       /\ c = "path_multi" -> "session.tmp"
    [] OTHER -> SpecTempPath(c)

SpecFileClass(c) ==
  CASE c = "valid_session" -> "session"
    [] c = "valid_temp" -> "temp"
    [] OTHER -> "ignore"

ActualFileClass(c) ==
  CASE Bug = "file_accept_status_snapshot"
       /\ c = "status_snapshot" -> "session"
    [] Bug = "file_accept_two_part"
       /\ c = "two_part" -> "session"
    [] Bug = "file_accept_four_part"
       /\ c = "four_part" -> "session"
    [] Bug = "file_reject_valid_temp"
       /\ c = "valid_temp" -> "ignore"
    [] Bug = "file_treat_temp_as_session"
       /\ c = "valid_temp" -> "session"
    [] OTHER -> SpecFileClass(c)

Init ==
  checked = 0

Next ==
  \/ /\ checked < 58
     /\ checked' = checked + 1
  \/ /\ checked = 58
     /\ checked' = checked

TypeInvariant ==
  /\ Bug \in {
       "none",
       "manifest_accept_version_mismatch",
       "manifest_accept_profile_mismatch",
       "manifest_accept_commit_mismatch",
       "manifest_reject_both_missing",
       "manifest_accept_missing_present",
       "load_absent_errors",
       "load_temp_invalid_blocks_main",
       "load_prefers_main_over_temp",
       "load_temp_read_error_falls_back",
       "load_main_read_error_returns_temp",
       "load_requires_promotion_success",
       "load_invalid_temp_kept",
       "inspect_prefers_temp_over_main",
       "inspect_deletes_invalid",
       "validate_accept_invalid_flag",
       "validate_accept_key_mismatch",
       "validate_accept_version_mismatch",
       "validate_accept_chain_mismatch",
       "validate_accept_manifest_mismatch",
       "validate_accept_chunk_invalid",
       "validate_keeps_rejected_file",
       "validate_reject_zero_clean",
       "validate_accept_zero_with_chunk",
       "validate_accept_zero_with_digest",
       "validate_accept_too_many_chunks",
       "validate_accept_duplicate_chunk",
       "validate_accept_chunk_out_of_range",
       "validate_accept_empty_ready_sig",
       "validate_accept_duplicate_ready",
       "validate_accept_ready_oob",
       "validate_accept_deliver_oob",
       "validate_accept_delivered_missing_sender",
       "validate_accept_delivered_empty_sig",
       "validate_accept_payload_hash_mismatch",
       "validate_accept_digest_mismatch",
       "validate_accept_expected_root_mismatch",
       "validate_accept_computed_root_mismatch",
       "validate_reject_incomplete_payload_hash",
       "limit_disabled_retains",
       "limit_ttl_zero_drops",
       "limit_ttl_boundary_drops",
       "limit_ttl_stale_kept",
       "limit_future_kept",
       "limit_max_sessions_keeps_oldest",
       "limit_max_sessions_no_hard",
       "limit_bytes_keeps_oldest",
       "limit_bytes_no_hard",
       "limit_soft_boundary_triggers",
       "limit_soft_over_normal",
       "limit_soft_bytes_boundary_triggers",
       "limit_soft_bytes_over_normal",
       "temp_replaces_extension",
       "temp_drops_multi_extension",
       "file_accept_status_snapshot",
       "file_accept_two_part",
       "file_accept_four_part",
       "file_reject_valid_temp",
       "file_treat_temp_as_session"
     }
  /\ checked \in 0..58

RbcStoreGuardMatchesSpec ==
  /\ \A c \in ManifestCases:
       ActualManifestMatches(c) = SpecManifestMatches(c)
  /\ \A c \in LoadCases:
       /\ ActualLoadChoice(c) = SpecLoadChoice(c)
       /\ ActualLoadDeletesTemp(c) = SpecLoadDeletesTemp(c)
  /\ \A c \in ValidateCases:
       /\ ActualValidateAccept(c) = SpecValidateAccept(c)
       /\ ActualValidateDeletes(c) = SpecValidateDeletes(c)
  /\ \A c \in LimitCases:
       /\ ActualLimitKeys(c) = SpecLimitKeys(c)
       /\ ActualLimitPressure(c) = SpecLimitPressure(c)
       /\ Cardinality(ActualLimitKeys(c)) = Cardinality(SpecLimitKeys(c))
  /\ \A c \in TempPathCases:
       ActualTempPath(c) = SpecTempPath(c)
  /\ \A c \in FileCases:
       ActualFileClass(c) = SpecFileClass(c)

SafetyFast ==
  RbcStoreGuardMatchesSpec

AllManifestMatches ==
  \A c \in ManifestCases:
    ActualManifestMatches(c) = SpecManifestMatches(c)

AllLoadChoicesMatchSpec ==
  \A c \in LoadCases:
    ActualLoadChoice(c) = SpecLoadChoice(c)

AllLoadDeleteFlagsMatchSpec ==
  \A c \in LoadCases:
    ActualLoadDeletesTemp(c) = SpecLoadDeletesTemp(c)

AllValidationResultsMatchSpec ==
  \A c \in ValidateCases:
    ActualValidateAccept(c) = SpecValidateAccept(c)

AllValidationDeleteFlagsMatchSpec ==
  \A c \in ValidateCases:
    ActualValidateDeletes(c) = SpecValidateDeletes(c)

AllLimitKeysMatchSpec ==
  \A c \in LimitCases:
    ActualLimitKeys(c) = SpecLimitKeys(c)

AllLimitPressureMatchesSpec ==
  \A c \in LimitCases:
    ActualLimitPressure(c) = SpecLimitPressure(c)

AllTempPathsMatchSpec ==
  \A c \in TempPathCases:
    ActualTempPath(c) = SpecTempPath(c)

AllFileClassesMatchSpec ==
  \A c \in FileCases:
    ActualFileClass(c) = SpecFileClass(c)

ManifestAnchors ==
  /\ ActualManifestMatches("same_with_commit")
  /\ ActualManifestMatches("same_missing_commit")
  /\ ~ActualManifestMatches("missing_vs_present")
  /\ ~ActualManifestMatches("present_vs_missing")
  /\ ~ActualManifestMatches("version_mismatch")
  /\ ~ActualManifestMatches("profile_mismatch")
  /\ ~ActualManifestMatches("commit_mismatch")

LoadChoiceAnchors ==
  /\ ActualLoadChoice("both_absent") = "none"
  /\ ActualLoadChoice("temp_valid_only") = "temp"
  /\ ActualLoadChoice("main_valid_only") = "main"
  /\ ActualLoadChoice("temp_invalid_main_valid") = "main"
  /\ ActualLoadChoice("temp_valid_main_valid") = "temp"
  /\ ActualLoadChoice("temp_read_error_main_valid") = "io_error"
  /\ ActualLoadChoice("main_read_error_temp_valid") = "io_error"
  /\ ActualLoadChoice("temp_valid_promote_fails") = "temp"
  /\ ActualLoadChoice("inspect_invalid_retained") = "none"
  /\ ActualLoadChoice("inspect_main_priority") = "main"

LoadDeletionAnchors ==
  /\ ActualLoadDeletesTemp("both_absent") = FALSE
  /\ ActualLoadDeletesTemp("temp_invalid_main_valid") = TRUE
  /\ ActualLoadDeletesTemp("inspect_invalid_retained") = FALSE

ValidationAcceptAnchors ==
  /\ ActualValidateAccept("valid")
  /\ ActualValidateAccept("zero_total_clean")
  /\ ActualValidateAccept("incomplete_payload_hash_deferred")
  /\ ~ActualValidateAccept("invalid_flag")
  /\ ~ActualValidateAccept("key_mismatch")
  /\ ~ActualValidateAccept("version_mismatch")
  /\ ~ActualValidateAccept("chain_mismatch")
  /\ ~ActualValidateAccept("manifest_mismatch")
  /\ ~ActualValidateAccept("chunk_invalid")
  /\ ~ActualValidateAccept("zero_total_with_chunk")
  /\ ~ActualValidateAccept("zero_total_with_digest")
  /\ ~ActualValidateAccept("too_many_chunks")
  /\ ~ActualValidateAccept("duplicate_chunk")
  /\ ~ActualValidateAccept("chunk_out_of_range")
  /\ ~ActualValidateAccept("empty_ready_sig")
  /\ ~ActualValidateAccept("duplicate_ready")
  /\ ~ActualValidateAccept("ready_sender_oob")
  /\ ~ActualValidateAccept("deliver_sender_oob")
  /\ ~ActualValidateAccept("delivered_missing_sender")
  /\ ~ActualValidateAccept("delivered_empty_sig")
  /\ ~ActualValidateAccept("payload_hash_mismatch")
  /\ ~ActualValidateAccept("digest_mismatch")
  /\ ~ActualValidateAccept("expected_root_mismatch")
  /\ ~ActualValidateAccept("computed_root_mismatch")

ValidationDeletionAnchors ==
  /\ ActualValidateDeletes("valid") = FALSE
  /\ ActualValidateDeletes("zero_total_clean") = FALSE
  /\ ActualValidateDeletes("incomplete_payload_hash_deferred") = FALSE
  /\ ActualValidateDeletes("invalid_flag") = TRUE
  /\ ActualValidateDeletes("key_mismatch") = TRUE
  /\ ActualValidateDeletes("chunk_invalid") = TRUE
  /\ ActualValidateDeletes("digest_mismatch") = TRUE

LimitKeyAnchors ==
  /\ ActualLimitKeys("disabled") = {}
  /\ ActualLimitKeys("ttl_disabled_old") = {"old", "fresh"}
  /\ ActualLimitKeys("ttl_boundary") = {"old"}
  /\ ActualLimitKeys("ttl_stale") = {}
  /\ ActualLimitKeys("ttl_future") = {}
  /\ ActualLimitKeys("capacity_exact") = {"old", "new"}
  /\ ActualLimitKeys("capacity_over_sessions") = {"mid", "new"}
  /\ ActualLimitKeys("bytes_exact") = {"old", "new"}
  /\ ActualLimitKeys("bytes_over_oldest") = {"new"}
  /\ ActualLimitKeys("soft_session_boundary") = {"a", "b"}
  /\ ActualLimitKeys("soft_session_over") = {"a", "b", "c"}
  /\ ActualLimitKeys("soft_bytes_boundary") = {"a", "b"}
  /\ ActualLimitKeys("soft_bytes_over") = {"a", "b"}

LimitPressureAnchors ==
  /\ ActualLimitPressure("capacity_over_sessions") = "hard"
  /\ ActualLimitPressure("bytes_over_oldest") = "hard"
  /\ ActualLimitPressure("soft_session_over") = "soft"
  /\ ActualLimitPressure("soft_bytes_over") = "soft"
  /\ ActualLimitPressure("disabled") = "normal"
  /\ ActualLimitPressure("ttl_boundary") = "normal"
  /\ ActualLimitPressure("soft_session_boundary") = "normal"
  /\ ActualLimitPressure("soft_bytes_boundary") = "normal"

TempPathAnchors ==
  /\ ActualTempPath("path_norito") = "session.norito.tmp"
  /\ ActualTempPath("path_no_ext") = "session.tmp"
  /\ ActualTempPath("path_multi") = "session.a.norito.tmp"

FileClassAnchors ==
  /\ ActualFileClass("valid_session") = "session"
  /\ ActualFileClass("valid_temp") = "temp"
  /\ ActualFileClass("status_snapshot") = "ignore"
  /\ ActualFileClass("two_part") = "ignore"
  /\ ActualFileClass("four_part") = "ignore"
  /\ ActualFileClass("temp_two_part") = "ignore"

SafetyAnchors ==
  /\ AllManifestMatches
  /\ AllLoadChoicesMatchSpec
  /\ AllLoadDeleteFlagsMatchSpec
  /\ AllValidationResultsMatchSpec
  /\ AllValidationDeleteFlagsMatchSpec
  /\ AllLimitKeysMatchSpec
  /\ AllLimitPressureMatchesSpec
  /\ AllTempPathsMatchSpec
  /\ AllFileClassesMatchSpec
  /\ ManifestAnchors
  /\ LoadChoiceAnchors
  /\ LoadDeletionAnchors
  /\ ValidationAcceptAnchors
  /\ ValidationDeletionAnchors
  /\ LimitKeyAnchors
  /\ LimitPressureAnchors
  /\ TempPathAnchors
  /\ FileClassAnchors

BugManifestAcceptVersionMismatch ==
  ActualManifestMatches("version_mismatch") = SpecManifestMatches("version_mismatch")

BugManifestAcceptProfileMismatch ==
  ActualManifestMatches("profile_mismatch") = SpecManifestMatches("profile_mismatch")

BugManifestAcceptCommitMismatch ==
  ActualManifestMatches("commit_mismatch") = SpecManifestMatches("commit_mismatch")

BugManifestRejectBothMissing ==
  ActualManifestMatches("same_missing_commit") = SpecManifestMatches("same_missing_commit")

BugManifestAcceptMissingPresent ==
  ActualManifestMatches("missing_vs_present") = SpecManifestMatches("missing_vs_present")

BugLoadAbsentErrors ==
  ActualLoadChoice("both_absent") = SpecLoadChoice("both_absent")

BugLoadTempInvalidBlocksMain ==
  ActualLoadChoice("temp_invalid_main_valid") = SpecLoadChoice("temp_invalid_main_valid")

BugLoadPrefersMainOverTemp ==
  ActualLoadChoice("temp_valid_main_valid") = SpecLoadChoice("temp_valid_main_valid")

BugLoadTempReadErrorFallsBack ==
  ActualLoadChoice("temp_read_error_main_valid") = SpecLoadChoice("temp_read_error_main_valid")

BugLoadMainReadErrorReturnsTemp ==
  ActualLoadChoice("main_read_error_temp_valid") = SpecLoadChoice("main_read_error_temp_valid")

BugLoadRequiresPromotionSuccess ==
  ActualLoadChoice("temp_valid_promote_fails") = SpecLoadChoice("temp_valid_promote_fails")

BugLoadInvalidTempKept ==
  ActualLoadDeletesTemp("temp_invalid_main_valid") = SpecLoadDeletesTemp("temp_invalid_main_valid")

BugInspectPrefersTempOverMain ==
  ActualLoadChoice("inspect_main_priority") = SpecLoadChoice("inspect_main_priority")

BugInspectDeletesInvalid ==
  ActualLoadDeletesTemp("inspect_invalid_retained") = SpecLoadDeletesTemp("inspect_invalid_retained")

BugValidateAcceptInvalidFlag ==
  ActualValidateAccept("invalid_flag") = SpecValidateAccept("invalid_flag")

BugValidateAcceptKeyMismatch ==
  ActualValidateAccept("key_mismatch") = SpecValidateAccept("key_mismatch")

BugValidateAcceptVersionMismatch ==
  ActualValidateAccept("version_mismatch") = SpecValidateAccept("version_mismatch")

BugValidateAcceptChainMismatch ==
  ActualValidateAccept("chain_mismatch") = SpecValidateAccept("chain_mismatch")

BugValidateAcceptManifestMismatch ==
  ActualValidateAccept("manifest_mismatch") = SpecValidateAccept("manifest_mismatch")

BugValidateAcceptChunkInvalid ==
  ActualValidateAccept("chunk_invalid") = SpecValidateAccept("chunk_invalid")

BugValidateKeepsRejectedFile ==
  ActualValidateDeletes("invalid_flag") = SpecValidateDeletes("invalid_flag")

BugValidateRejectZeroClean ==
  ActualValidateAccept("zero_total_clean") = SpecValidateAccept("zero_total_clean")

BugValidateAcceptZeroWithChunk ==
  ActualValidateAccept("zero_total_with_chunk") = SpecValidateAccept("zero_total_with_chunk")

BugValidateAcceptZeroWithDigest ==
  ActualValidateAccept("zero_total_with_digest") = SpecValidateAccept("zero_total_with_digest")

BugValidateAcceptTooManyChunks ==
  ActualValidateAccept("too_many_chunks") = SpecValidateAccept("too_many_chunks")

BugValidateAcceptDuplicateChunk ==
  ActualValidateAccept("duplicate_chunk") = SpecValidateAccept("duplicate_chunk")

BugValidateAcceptChunkOutOfRange ==
  ActualValidateAccept("chunk_out_of_range") = SpecValidateAccept("chunk_out_of_range")

BugValidateAcceptEmptyReadySig ==
  ActualValidateAccept("empty_ready_sig") = SpecValidateAccept("empty_ready_sig")

BugValidateAcceptDuplicateReady ==
  ActualValidateAccept("duplicate_ready") = SpecValidateAccept("duplicate_ready")

BugValidateAcceptReadyOob ==
  ActualValidateAccept("ready_sender_oob") = SpecValidateAccept("ready_sender_oob")

BugValidateAcceptDeliverOob ==
  ActualValidateAccept("deliver_sender_oob") = SpecValidateAccept("deliver_sender_oob")

BugValidateAcceptDeliveredMissingSender ==
  ActualValidateAccept("delivered_missing_sender") = SpecValidateAccept("delivered_missing_sender")

BugValidateAcceptDeliveredEmptySig ==
  ActualValidateAccept("delivered_empty_sig") = SpecValidateAccept("delivered_empty_sig")

BugValidateAcceptPayloadHashMismatch ==
  ActualValidateAccept("payload_hash_mismatch") = SpecValidateAccept("payload_hash_mismatch")

BugValidateAcceptDigestMismatch ==
  ActualValidateAccept("digest_mismatch") = SpecValidateAccept("digest_mismatch")

BugValidateAcceptExpectedRootMismatch ==
  ActualValidateAccept("expected_root_mismatch") = SpecValidateAccept("expected_root_mismatch")

BugValidateAcceptComputedRootMismatch ==
  ActualValidateAccept("computed_root_mismatch") = SpecValidateAccept("computed_root_mismatch")

BugValidateRejectIncompletePayloadHash ==
  ActualValidateAccept("incomplete_payload_hash_deferred") =
    SpecValidateAccept("incomplete_payload_hash_deferred")

BugLimitDisabledRetains ==
  ActualLimitKeys("disabled") = SpecLimitKeys("disabled")

BugLimitTtlZeroDrops ==
  ActualLimitKeys("ttl_disabled_old") = SpecLimitKeys("ttl_disabled_old")

BugLimitTtlBoundaryDrops ==
  ActualLimitKeys("ttl_boundary") = SpecLimitKeys("ttl_boundary")

BugLimitTtlStaleKept ==
  ActualLimitKeys("ttl_stale") = SpecLimitKeys("ttl_stale")

BugLimitFutureKept ==
  ActualLimitKeys("ttl_future") = SpecLimitKeys("ttl_future")

BugLimitMaxSessionsKeepsOldest ==
  ActualLimitKeys("capacity_over_sessions") = SpecLimitKeys("capacity_over_sessions")

BugLimitMaxSessionsNoHard ==
  ActualLimitPressure("capacity_over_sessions") = SpecLimitPressure("capacity_over_sessions")

BugLimitBytesKeepsOldest ==
  ActualLimitKeys("bytes_over_oldest") = SpecLimitKeys("bytes_over_oldest")

BugLimitBytesNoHard ==
  ActualLimitPressure("bytes_over_oldest") = SpecLimitPressure("bytes_over_oldest")

BugLimitSoftBoundaryTriggers ==
  ActualLimitPressure("soft_session_boundary") = SpecLimitPressure("soft_session_boundary")

BugLimitSoftOverNormal ==
  ActualLimitPressure("soft_session_over") = SpecLimitPressure("soft_session_over")

BugLimitSoftBytesBoundaryTriggers ==
  ActualLimitPressure("soft_bytes_boundary") = SpecLimitPressure("soft_bytes_boundary")

BugLimitSoftBytesOverNormal ==
  ActualLimitPressure("soft_bytes_over") = SpecLimitPressure("soft_bytes_over")

BugTempReplacesExtension ==
  ActualTempPath("path_norito") = SpecTempPath("path_norito")

BugTempDropsMultiExtension ==
  ActualTempPath("path_multi") = SpecTempPath("path_multi")

BugFileAcceptStatusSnapshot ==
  ActualFileClass("status_snapshot") = SpecFileClass("status_snapshot")

BugFileAcceptTwoPart ==
  ActualFileClass("two_part") = SpecFileClass("two_part")

BugFileAcceptFourPart ==
  ActualFileClass("four_part") = SpecFileClass("four_part")

BugFileRejectValidTemp ==
  ActualFileClass("valid_temp") = SpecFileClass("valid_temp")

BugFileTreatTempAsSession ==
  ActualFileClass("valid_temp") = SpecFileClass("valid_temp")

====
