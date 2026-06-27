---- MODULE SumeragiDaGateHelperGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for data-availability gate helpers.

This slice pins `da::evaluate(...)`, `da::gate_satisfaction(...)`, and
`ManifestGateKind::as_str()`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EvalCases == {
  "disabled_available",
  "disabled_missing",
  "enabled_available",
  "enabled_missing"
}

ManifestKinds == {
  "missing",
  "hash_mismatch",
  "read_failed",
  "spool_scan"
}

ManifestReasons == {
  "manifest_missing",
  "manifest_hash_mismatch",
  "manifest_read_failed",
  "manifest_spool_scan"
}

Reasons == {"none", "missing_local_data"} \cup ManifestReasons

SpecEvaluate(c) ==
  CASE c = "enabled_missing" -> "missing_local_data"
    [] OTHER -> "none"

ActualEvaluate(c) ==
  CASE Bug = "eval_disabled_available_blocks"
       /\ c = "disabled_available" -> "missing_local_data"
    [] Bug = "eval_disabled_missing_blocks"
       /\ c = "disabled_missing" -> "missing_local_data"
    [] Bug = "eval_enabled_available_blocks"
       /\ c = "enabled_available" -> "missing_local_data"
    [] Bug = "eval_enabled_available_manifest"
       /\ c = "enabled_available" -> "manifest_missing"
    [] Bug = "eval_enabled_missing_allows"
       /\ c = "enabled_missing" -> "none"
    [] Bug = "eval_enabled_missing_manifest"
       /\ c = "enabled_missing" -> "manifest_missing"
    [] OTHER -> SpecEvaluate(c)

SpecSatisfaction(previous, current) ==
  IF previous = "missing_local_data" /\ current = "none"
  THEN "missing_data_recovered"
  ELSE "none"

ActualSatisfaction(previous, current) ==
  CASE Bug = "satisfaction_missing_to_none_ignored"
       /\ previous = "missing_local_data"
       /\ current = "none" -> "none"
    [] Bug = "satisfaction_missing_to_missing_recovers"
       /\ previous = "missing_local_data"
       /\ current = "missing_local_data" -> "missing_data_recovered"
    [] Bug = "satisfaction_none_to_none_recovers"
       /\ previous = "none"
       /\ current = "none" -> "missing_data_recovered"
    [] Bug = "satisfaction_none_to_missing_recovers"
       /\ previous = "none"
       /\ current = "missing_local_data" -> "missing_data_recovered"
    [] Bug = "satisfaction_missing_to_manifest_recovers"
       /\ previous = "missing_local_data"
       /\ current \in ManifestReasons -> "missing_data_recovered"
    [] Bug = "satisfaction_manifest_to_none_recovers"
       /\ previous \in ManifestReasons
       /\ current = "none" -> "missing_data_recovered"
    [] Bug = "satisfaction_manifest_to_manifest_recovers"
       /\ previous \in ManifestReasons
       /\ current \in ManifestReasons -> "missing_data_recovered"
    [] Bug = "satisfaction_manifest_to_missing_recovers"
       /\ previous \in ManifestReasons
       /\ current = "missing_local_data" -> "missing_data_recovered"
    [] OTHER -> SpecSatisfaction(previous, current)

SpecManifestLabel(kind) ==
  CASE kind = "missing" -> "manifest_missing"
    [] kind = "hash_mismatch" -> "manifest_hash_mismatch"
    [] kind = "read_failed" -> "manifest_read_failed"
    [] OTHER -> "manifest_spool_scan"

ActualManifestLabel(kind) ==
  CASE Bug = "manifest_missing_wrong_label"
       /\ kind = "missing" -> "manifest_hash_mismatch"
    [] Bug = "manifest_hash_mismatch_wrong_label"
       /\ kind = "hash_mismatch" -> "manifest_missing"
    [] Bug = "manifest_read_failed_wrong_label"
       /\ kind = "read_failed" -> "manifest_missing"
    [] Bug = "manifest_spool_scan_wrong_label"
       /\ kind = "spool_scan" -> "manifest_missing"
    [] OTHER -> SpecManifestLabel(kind)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "eval_disabled_available_blocks",
       "eval_disabled_missing_blocks",
       "eval_enabled_available_blocks",
       "eval_enabled_available_manifest",
       "eval_enabled_missing_allows",
       "eval_enabled_missing_manifest",
       "satisfaction_missing_to_none_ignored",
       "satisfaction_missing_to_missing_recovers",
       "satisfaction_none_to_none_recovers",
       "satisfaction_none_to_missing_recovers",
       "satisfaction_missing_to_manifest_recovers",
       "satisfaction_manifest_to_none_recovers",
       "satisfaction_manifest_to_manifest_recovers",
       "satisfaction_manifest_to_missing_recovers",
       "manifest_missing_wrong_label",
       "manifest_hash_mismatch_wrong_label",
       "manifest_read_failed_wrong_label",
       "manifest_spool_scan_wrong_label"
     }
  /\ checked = 0

DaGateHelperMatchesSpec ==
  /\ \A c \in EvalCases:
       ActualEvaluate(c) = SpecEvaluate(c)
  /\ \A previous \in Reasons:
       \A current \in Reasons:
         ActualSatisfaction(previous, current) =
           SpecSatisfaction(previous, current)
  /\ \A kind \in ManifestKinds:
       ActualManifestLabel(kind) = SpecManifestLabel(kind)

DaGateHelperExactness ==
  DaGateHelperMatchesSpec

DaGateHelperCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DaGateHelperExactness

SafetyFast == DaGateHelperExactness

Safety ==
  DaGateHelperCorrectnessEnvelope

BugEvalDisabledAvailableBlocks ==
  ActualEvaluate("disabled_available") = SpecEvaluate("disabled_available")

BugEvalDisabledMissingBlocks ==
  ActualEvaluate("disabled_missing") = SpecEvaluate("disabled_missing")

BugEvalEnabledAvailableBlocks ==
  ActualEvaluate("enabled_available") = SpecEvaluate("enabled_available")

BugEvalEnabledAvailableManifest ==
  ActualEvaluate("enabled_available") = SpecEvaluate("enabled_available")

BugEvalEnabledMissingAllows ==
  ActualEvaluate("enabled_missing") = SpecEvaluate("enabled_missing")

BugEvalEnabledMissingManifest ==
  ActualEvaluate("enabled_missing") = SpecEvaluate("enabled_missing")

BugSatisfactionMissingToNoneIgnored ==
  ActualSatisfaction("missing_local_data", "none") =
    SpecSatisfaction("missing_local_data", "none")

BugSatisfactionMissingToMissingRecovers ==
  ActualSatisfaction("missing_local_data", "missing_local_data") =
    SpecSatisfaction("missing_local_data", "missing_local_data")

BugSatisfactionNoneToNoneRecovers ==
  ActualSatisfaction("none", "none") =
    SpecSatisfaction("none", "none")

BugSatisfactionNoneToMissingRecovers ==
  ActualSatisfaction("none", "missing_local_data") =
    SpecSatisfaction("none", "missing_local_data")

BugSatisfactionMissingToManifestRecovers ==
  \A reason \in ManifestReasons:
    ActualSatisfaction("missing_local_data", reason) =
      SpecSatisfaction("missing_local_data", reason)

BugSatisfactionManifestToNoneRecovers ==
  \A reason \in ManifestReasons:
    ActualSatisfaction(reason, "none") =
      SpecSatisfaction(reason, "none")

BugSatisfactionManifestToManifestRecovers ==
  \A previous \in ManifestReasons:
    \A current \in ManifestReasons:
      ActualSatisfaction(previous, current) =
        SpecSatisfaction(previous, current)

BugSatisfactionManifestToMissingRecovers ==
  \A reason \in ManifestReasons:
    ActualSatisfaction(reason, "missing_local_data") =
      SpecSatisfaction(reason, "missing_local_data")

BugManifestMissingWrongLabel ==
  ActualManifestLabel("missing") = SpecManifestLabel("missing")

BugManifestHashMismatchWrongLabel ==
  ActualManifestLabel("hash_mismatch") = SpecManifestLabel("hash_mismatch")

BugManifestReadFailedWrongLabel ==
  ActualManifestLabel("read_failed") = SpecManifestLabel("read_failed")

BugManifestSpoolScanWrongLabel ==
  ActualManifestLabel("spool_scan") = SpecManifestLabel("spool_scan")

====
