---- MODULE SumeragiManifestGuardGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi DA manifest guard helpers.

This slice pins `enforce_manifest_available_for_commitment(...)`,
`manifest_guard_outcome(...)`, `manifest_available_for_commitment(...)`,
`manifests_available_for_block(...)`, `CacheOutcome::merge(...)`, and
`validate_da_bundle_caps(...)`.

The model abstracts file-system and hash bytes into finite outcomes while
preserving the consensus-relevant policy: matching manifests pass, hash
mismatches always reject, strict lanes reject any manifest error, audit-only
lanes warn for non-hash errors, block-level scans collect warnings until a
fatal record, cache merges are hit-only when both sides hit, and DA bundles
must stay within caps with present nonzero proof digests.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LookupResults == {
  "lookup_match",
  "lookup_missing",
  "lookup_hash_mismatch",
  "lookup_read_failed",
  "lookup_spool_scan"
}

Policies == {"strict", "audit"}

GuardOutcomes == {"pass", "warn", "reject"}

CacheOutcomes == {"hit", "miss"}

BlockCases == {
  "no_bundle",
  "one_match_strict",
  "audit_missing",
  "audit_read_then_match",
  "strict_missing",
  "audit_hash_mismatch",
  "warn_then_strict_missing"
}

BundleCases == {
  "empty_ok",
  "one_valid_ok",
  "commitment_cap_exceeded",
  "openings_cap_exceeded",
  "missing_digest",
  "zero_digest_all_zero",
  "zero_digest_one",
  "nonzero_digest_two",
  "nonzero_digest_prefix"
}

SpecEnforceResult(r) ==
  CASE r = "lookup_match" -> "ok"
    [] r = "lookup_missing" -> "missing"
    [] r = "lookup_hash_mismatch" -> "hash_mismatch"
    [] r = "lookup_read_failed" -> "read_failed"
    [] OTHER -> "spool_scan"

ActualEnforceResult(r) ==
  CASE Bug = "enforce_missing_allows"
       /\ r = "lookup_missing" -> "ok"
    [] Bug = "enforce_hash_mismatch_allows"
       /\ r = "lookup_hash_mismatch" -> "ok"
    [] Bug = "enforce_read_error_allows"
       /\ r = "lookup_read_failed" -> "ok"
    [] Bug = "enforce_spool_scan_allows"
       /\ r = "lookup_spool_scan" -> "ok"
    [] Bug = "enforce_match_rejects"
       /\ r = "lookup_match" -> "missing"
    [] OTHER -> SpecEnforceResult(r)

OutcomeFromResult(result, policy) ==
  IF result = "ok" THEN
    "pass"
  ELSE IF result = "hash_mismatch" THEN
    "reject"
  ELSE IF policy = "audit" THEN
    "warn"
  ELSE
    "reject"

SpecGuardOutcome(r, policy) ==
  OutcomeFromResult(SpecEnforceResult(r), policy)

ActualGuardOutcome(r, policy) ==
  CASE Bug = "guard_audit_hash_mismatch_warns"
       /\ r = "lookup_hash_mismatch"
       /\ policy = "audit" -> "warn"
    [] Bug = "guard_strict_missing_warns"
       /\ r = "lookup_missing"
       /\ policy = "strict" -> "warn"
    [] Bug = "guard_audit_missing_rejects"
       /\ r = "lookup_missing"
       /\ policy = "audit" -> "reject"
    [] Bug = "guard_pass_rejected"
       /\ r = "lookup_match" -> "reject"
    [] OTHER -> OutcomeFromResult(ActualEnforceResult(r), policy)

SpecAvailable(r, policy) ==
  SpecGuardOutcome(r, policy) # "reject"

ActualAvailable(r, policy) ==
  CASE Bug = "available_warn_rejected"
       /\ r = "lookup_missing"
       /\ policy = "audit" -> FALSE
    [] Bug = "available_reject_allowed"
       /\ r = "lookup_hash_mismatch" -> TRUE
    [] OTHER -> ActualGuardOutcome(r, policy) # "reject"

SpecCacheMerge(left, right) ==
  IF left = "hit" /\ right = "hit" THEN "hit" ELSE "miss"

ActualCacheMerge(left, right) ==
  CASE Bug = "cache_merge_hit_miss_returns_hit"
       /\ left = "hit"
       /\ right = "miss" -> "hit"
    [] Bug = "cache_merge_miss_hit_returns_hit"
       /\ left = "miss"
       /\ right = "hit" -> "hit"
    [] Bug = "cache_merge_miss_miss_returns_hit"
       /\ left = "miss"
       /\ right = "miss" -> "hit"
    [] OTHER -> SpecCacheMerge(left, right)

SpecBlockStatus(c) ==
  CASE c = "no_bundle" -> "ok"
    [] c = "one_match_strict" -> "ok"
    [] c = "audit_missing" -> "ok"
    [] c = "audit_read_then_match" -> "ok"
    [] c = "strict_missing" -> "reject"
    [] c = "audit_hash_mismatch" -> "reject"
    [] OTHER -> "reject"

SpecBlockWarnings(c) ==
  CASE c = "audit_missing" -> 1
    [] c = "audit_read_then_match" -> 1
    [] c = "warn_then_strict_missing" -> 1
    [] OTHER -> 0

ActualBlockStatus(c) ==
  CASE Bug = "block_no_bundle_rejects"
       /\ c = "no_bundle" -> "reject"
    [] Bug = "block_audit_missing_rejects"
       /\ c = "audit_missing" -> "reject"
    [] Bug = "block_strict_missing_warns"
       /\ c = "strict_missing" -> "ok"
    [] Bug = "block_hash_mismatch_warns"
       /\ c = "audit_hash_mismatch" -> "ok"
    [] Bug = "block_skips_later_reject"
       /\ c = "warn_then_strict_missing" -> "ok"
    [] OTHER -> SpecBlockStatus(c)

ActualBlockWarnings(c) ==
  CASE Bug = "block_warning_not_recorded"
       /\ c = "audit_missing" -> 0
    [] Bug = "block_warning_on_pass"
       /\ c = "one_match_strict" -> 1
    [] OTHER -> SpecBlockWarnings(c)

SpecBundleValid(c) ==
  c \in {
    "empty_ok",
    "one_valid_ok",
    "nonzero_digest_two",
    "nonzero_digest_prefix"
  }

ActualBundleValid(c) ==
  CASE Bug = "bundle_commitment_cap_ignored"
       /\ c = "commitment_cap_exceeded" -> TRUE
    [] Bug = "bundle_openings_cap_ignored"
       /\ c = "openings_cap_exceeded" -> TRUE
    [] Bug = "bundle_missing_digest_allowed"
       /\ c = "missing_digest" -> TRUE
    [] Bug = "bundle_zero_digest_allowed"
       /\ c = "zero_digest_all_zero" -> TRUE
    [] Bug = "bundle_zero_one_digest_allowed"
       /\ c = "zero_digest_one" -> TRUE
    [] Bug = "bundle_digest_two_rejected"
       /\ c = "nonzero_digest_two" -> FALSE
    [] Bug = "bundle_empty_rejected"
       /\ c = "empty_ok" -> FALSE
    [] OTHER -> SpecBundleValid(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "enforce_missing_allows",
       "enforce_hash_mismatch_allows",
       "enforce_read_error_allows",
       "enforce_spool_scan_allows",
       "enforce_match_rejects",
       "guard_audit_hash_mismatch_warns",
       "guard_strict_missing_warns",
       "guard_audit_missing_rejects",
       "guard_pass_rejected",
       "available_warn_rejected",
       "available_reject_allowed",
       "cache_merge_hit_miss_returns_hit",
       "cache_merge_miss_hit_returns_hit",
       "cache_merge_miss_miss_returns_hit",
       "block_no_bundle_rejects",
       "block_audit_missing_rejects",
       "block_strict_missing_warns",
       "block_hash_mismatch_warns",
       "block_skips_later_reject",
       "block_warning_not_recorded",
       "block_warning_on_pass",
       "bundle_commitment_cap_ignored",
       "bundle_openings_cap_ignored",
       "bundle_missing_digest_allowed",
       "bundle_zero_digest_allowed",
       "bundle_zero_one_digest_allowed",
       "bundle_digest_two_rejected",
       "bundle_empty_rejected"
     }
  /\ checked = 0

ManifestGuardMatchesSpec ==
  /\ \A result \in LookupResults:
       ActualEnforceResult(result) = SpecEnforceResult(result)
  /\ \A result \in LookupResults:
       \A policy \in Policies:
         ActualGuardOutcome(result, policy) =
           SpecGuardOutcome(result, policy)
  /\ \A result \in LookupResults:
       \A policy \in Policies:
         ActualAvailable(result, policy) =
           SpecAvailable(result, policy)
  /\ \A left \in CacheOutcomes:
       \A right \in CacheOutcomes:
         ActualCacheMerge(left, right) = SpecCacheMerge(left, right)
  /\ \A c \in BlockCases:
       /\ ActualBlockStatus(c) = SpecBlockStatus(c)
       /\ ActualBlockWarnings(c) = SpecBlockWarnings(c)
  /\ \A c \in BundleCases:
       ActualBundleValid(c) = SpecBundleValid(c)

SafetyFast ==
  ManifestGuardMatchesSpec

HashMismatchAlwaysRejects ==
  \A policy \in Policies:
    ActualGuardOutcome("lookup_hash_mismatch", policy) = "reject"

AuditNonHashErrorsWarn ==
  /\ ActualGuardOutcome("lookup_missing", "audit") = "warn"
  /\ ActualGuardOutcome("lookup_read_failed", "audit") = "warn"
  /\ ActualGuardOutcome("lookup_spool_scan", "audit") = "warn"

StrictNonOkErrorsReject ==
  /\ ActualGuardOutcome("lookup_missing", "strict") = "reject"
  /\ ActualGuardOutcome("lookup_read_failed", "strict") = "reject"
  /\ ActualGuardOutcome("lookup_spool_scan", "strict") = "reject"

ManifestGuardExactness ==
  /\ ManifestGuardMatchesSpec
  /\ HashMismatchAlwaysRejects
  /\ AuditNonHashErrorsWarn
  /\ StrictNonOkErrorsReject

ManifestGuardCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ManifestGuardExactness

BugEnforceMissingAllows ==
  ActualEnforceResult("lookup_missing") = SpecEnforceResult("lookup_missing")

BugEnforceHashMismatchAllows ==
  ActualEnforceResult("lookup_hash_mismatch") =
    SpecEnforceResult("lookup_hash_mismatch")

BugEnforceReadErrorAllows ==
  ActualEnforceResult("lookup_read_failed") =
    SpecEnforceResult("lookup_read_failed")

BugEnforceSpoolScanAllows ==
  ActualEnforceResult("lookup_spool_scan") =
    SpecEnforceResult("lookup_spool_scan")

BugEnforceMatchRejects ==
  ActualEnforceResult("lookup_match") = SpecEnforceResult("lookup_match")

BugGuardAuditHashMismatchWarns ==
  /\ HashMismatchAlwaysRejects
  /\ ActualGuardOutcome("lookup_hash_mismatch", "audit") =
       SpecGuardOutcome("lookup_hash_mismatch", "audit")

BugGuardStrictMissingWarns ==
  /\ StrictNonOkErrorsReject
  /\ ActualGuardOutcome("lookup_missing", "strict") =
       SpecGuardOutcome("lookup_missing", "strict")

BugGuardAuditMissingRejects ==
  /\ AuditNonHashErrorsWarn
  /\ ActualGuardOutcome("lookup_missing", "audit") =
       SpecGuardOutcome("lookup_missing", "audit")

BugGuardPassRejected ==
  ActualGuardOutcome("lookup_match", "strict") =
    SpecGuardOutcome("lookup_match", "strict")

BugAvailableWarnRejected ==
  ActualAvailable("lookup_missing", "audit") =
    SpecAvailable("lookup_missing", "audit")

BugAvailableRejectAllowed ==
  ActualAvailable("lookup_hash_mismatch", "strict") =
    SpecAvailable("lookup_hash_mismatch", "strict")

BugCacheMergeHitMissReturnsHit ==
  ActualCacheMerge("hit", "miss") = SpecCacheMerge("hit", "miss")

BugCacheMergeMissHitReturnsHit ==
  ActualCacheMerge("miss", "hit") = SpecCacheMerge("miss", "hit")

BugCacheMergeMissMissReturnsHit ==
  ActualCacheMerge("miss", "miss") = SpecCacheMerge("miss", "miss")

BugBlockNoBundleRejects ==
  ActualBlockStatus("no_bundle") = SpecBlockStatus("no_bundle")

BugBlockAuditMissingRejects ==
  ActualBlockStatus("audit_missing") = SpecBlockStatus("audit_missing")

BugBlockStrictMissingWarns ==
  ActualBlockStatus("strict_missing") = SpecBlockStatus("strict_missing")

BugBlockHashMismatchWarns ==
  ActualBlockStatus("audit_hash_mismatch") =
    SpecBlockStatus("audit_hash_mismatch")

BugBlockSkipsLaterReject ==
  ActualBlockStatus("warn_then_strict_missing") =
    SpecBlockStatus("warn_then_strict_missing")

BugBlockWarningNotRecorded ==
  ActualBlockWarnings("audit_missing") = SpecBlockWarnings("audit_missing")

BugBlockWarningOnPass ==
  ActualBlockWarnings("one_match_strict") =
    SpecBlockWarnings("one_match_strict")

BugBundleCommitmentCapIgnored ==
  ActualBundleValid("commitment_cap_exceeded") =
    SpecBundleValid("commitment_cap_exceeded")

BugBundleOpeningsCapIgnored ==
  ActualBundleValid("openings_cap_exceeded") =
    SpecBundleValid("openings_cap_exceeded")

BugBundleMissingDigestAllowed ==
  ActualBundleValid("missing_digest") = SpecBundleValid("missing_digest")

BugBundleZeroDigestAllowed ==
  ActualBundleValid("zero_digest_all_zero") =
    SpecBundleValid("zero_digest_all_zero")

BugBundleZeroOneDigestAllowed ==
  ActualBundleValid("zero_digest_one") = SpecBundleValid("zero_digest_one")

BugBundleDigestTwoRejected ==
  ActualBundleValid("nonzero_digest_two") =
    SpecBundleValid("nonzero_digest_two")

BugBundleEmptyRejected ==
  ActualBundleValid("empty_ok") = SpecBundleValid("empty_ok")

====
