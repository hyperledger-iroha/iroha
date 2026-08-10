#!/usr/bin/env bash
# Run one exact no-skip Sumeragi v2 status/diagnostics SDK contract inventory.

set -euo pipefail

repo_root="$(cd -- "${BASH_SOURCE[0]%/*}/.." && pwd -P)"
readonly repo_root
readonly gradle_init_path="${repo_root}/ci/native_amx_v2_grouped_gradle_init.gradle"
readonly source_closure_resolver="${repo_root}/ci/resolve_sumeragi_v2_sdk_source_closure.py"
readonly source_closure_manifest="${repo_root}/ci/sumeragi_v2_sdk_source_closure.json"

usage() {
  echo "usage: $0 {python|javascript|swift|kotlin|java|--suite-source-manifest-sha256}" >&2
  exit 2
}

require_regular_source() {
  local path="$1"
  if [[ ! -f "$path" || -L "$path" ]]; then
    echo "Sumeragi v2 SDK diagnostics source must be a regular non-symlink file: ${path}" >&2
    exit 1
  fi
}

require_regular_source "$source_closure_resolver"
require_regular_source "$source_closure_manifest"

readonly python_bin="${PYTHON_BIN:-python3}"
if [[ -z "$python_bin" ]] || ! command -v "$python_bin" >/dev/null 2>&1; then
  echo "A valid Python 3 interpreter is required for Sumeragi v2 SDK diagnostics (PYTHON_BIN=${python_bin})" >&2
  exit 1
fi

suite_source_manifest_sha256="$(
  "$python_bin" "$source_closure_resolver" \
    --root "$repo_root" \
    --manifest "$source_closure_manifest" \
    --suite "sumeragi-v2-sdk-diagnostics" \
    --manifest-sha256
)"
if [[ ! "$suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Sumeragi v2 SDK diagnostics suite-source manifest digest is invalid" >&2
  exit 1
fi
readonly suite_source_manifest_sha256

if [[ $# != 1 ]]; then
  usage
fi
readonly surface="$1"
case "$surface" in
  --suite-source-manifest-sha256)
    printf '%s\n' "$suite_source_manifest_sha256"
    exit 0
    ;;
  python|javascript|swift|kotlin|java)
    ;;
  *)
    usage
    ;;
esac

readonly temporary_parent="${TMPDIR:-/tmp}"
temporary_root="$(mktemp -d "${temporary_parent%/}/sumeragi-v2-sdk-diagnostics.XXXXXX")"
readonly temporary_root
cleanup() {
  local status=$?
  if [[ -n "$temporary_root" && -d "$temporary_root" \
    && "${temporary_root##*/}" == sumeragi-v2-sdk-diagnostics.* ]]; then
    rm -rf -- "$temporary_root"
  fi
  trap - EXIT
  exit "$status"
}
trap cleanup EXIT

run_and_capture() {
  local transcript="$1"
  shift
  set +e
  "$@" 2>&1 | tee "$transcript"
  local pipeline_status=("${PIPESTATUS[@]}")
  set -e
  if ((pipeline_status[0] != 0 || pipeline_status[1] != 0)); then
    echo "Sumeragi v2 ${surface} diagnostics command failed (command=${pipeline_status[0]}, tee=${pipeline_status[1]})" >&2
    exit 1
  fi
}

assert_pytest_count() {
  local transcript="$1"
  local expected="$2"
  "$python_bin" - "$transcript" "$expected" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
expected = int(sys.argv[2])
matches = [
    match
    for line in lines
    if (
        match := re.fullmatch(
            r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"
            r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?",
            line,
        )
    )
]
if len(matches) != 1 or int(matches[0].group(1)) != expected:
    raise SystemExit(f"expected one exact no-skip {expected}-test pytest transcript")
PY
}

assert_node_tap() {
  local transcript="$1"
  local expected="$2"
  "$python_bin" - "$transcript" "$expected" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
expected = int(sys.argv[2])
subtests = [line.removeprefix("# Subtest: ") for line in lines if line.startswith("# Subtest: ")]
passing = [
    match.group(2)
    for line in lines
    if (match := re.fullmatch(r"ok ([0-9]+) - (.+)", line))
]
expected_footer = {
    "# tests": expected,
    "# pass": expected,
    "# fail": 0,
    "# cancelled": 0,
    "# skipped": 0,
    "# todo": 0,
}
for label, count in expected_footer.items():
    if lines.count(f"{label} {count}") != 1:
        raise SystemExit(f"Node TAP footer must contain exactly one {label} {count}")
if len(subtests) != expected or passing != subtests or len(set(subtests)) != expected:
    raise SystemExit("Node TAP subtests must be exact, unique, ordered, passing, and no-skip")
PY
}

resolve_java_home() {
  local java_bin
  if [[ -n "${JAVA_BIN:-}" ]]; then
    java_bin="$JAVA_BIN"
  else
    java_bin="$("${repo_root}/scripts/formal/resolve_java.sh")"
  fi
  java_bin="$(
    "$python_bin" - "$java_bin" <<'PY'
from pathlib import Path
import sys
print(Path(sys.argv[1]).resolve(strict=True))
PY
  )"
  if [[ "${java_bin##*/}" != java || ! -x "$java_bin" ]]; then
    echo "Sumeragi v2 JVM diagnostics require an executable JDK java binary" >&2
    exit 1
  fi
  local java_home="${java_bin%/bin/java}"
  if [[ ! -x "${java_home}/bin/javac" ]]; then
    echo "Sumeragi v2 JVM diagnostics require a complete JDK" >&2
    exit 1
  fi
  printf '%s\n' "$java_home"
}

assert_gradle_reports() {
  local report_root="$1"
  local expected="$2"
  shift 2
  "$python_bin" - "$report_root" "$expected" "$@" <<'PY'
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

root = Path(sys.argv[1])
expected = int(sys.argv[2])
expected_classes = tuple(sys.argv[3:])
reports = list(root.rglob("TEST-*.xml"))
observed_classes = tuple(sorted(path.name.removeprefix("TEST-").removesuffix(".xml") for path in reports))
if observed_classes != tuple(sorted(expected_classes)):
    raise SystemExit(
        f"unexpected Gradle diagnostics reports: expected {expected_classes}, found {observed_classes}"
    )
totals = [0, 0, 0, 0]
for report in reports:
    suite = ET.parse(report).getroot()
    totals[0] += int(suite.attrib.get("tests", "-1"))
    totals[1] += int(suite.attrib.get("failures", "-1"))
    totals[2] += int(suite.attrib.get("errors", "-1"))
    totals[3] += int(suite.attrib.get("skipped", "-1"))
if tuple(totals) != (expected, 0, 0, 0):
    raise SystemExit(
        "unexpected Gradle Sumeragi diagnostics result: "
        f"tests={totals[0]} failures={totals[1]} errors={totals[2]} skipped={totals[3]}"
    )
PY
}

observed_test_count=0
case "$surface" in
  python)
    observed_test_count=121
    "$python_bin" -c \
      'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else "Python Sumeragi diagnostics require Python >=3.10")'
    readonly python_path="${repo_root}/python/iroha_python/src:${repo_root}/python/norito_py/src:${repo_root}/python/iroha_torii_client:${repo_root}/python${PYTHONPATH:+:${PYTHONPATH}}"
    readonly python_transcript="${temporary_root}/python.log"
    readonly torii_test="${repo_root}/python/iroha_torii_client/tests/test_client.py"
    python_diagnostics_tests=(
      "${repo_root}/python/iroha_python/tests/client_sumeragi_v2_status_test.py"
      "${torii_test}::test_get_sumeragi_status_parses_authoritative_v2_snapshot"
      "${torii_test}::test_get_sumeragi_status_accepts_nonempty_native_manifest"
      "${torii_test}::test_get_sumeragi_status_rejects_invalid_native_manifest"
      "${torii_test}::test_get_sumeragi_status_requires_exact_merge_carrier_projection"
      "${torii_test}::test_get_sumeragi_status_requires_exact_executed_wire_len"
      "${torii_test}::test_get_sumeragi_status_preserves_exact_proposal_rounds"
      "${torii_test}::test_get_sumeragi_status_enforces_vote_quorum_proposal_geometry"
      "${torii_test}::test_get_sumeragi_status_enforces_outbound_intent_proposal_geometry"
      "${torii_test}::test_get_sumeragi_status_accepts_local_control_pending_liveness_blocker"
      "${torii_test}::test_get_sumeragi_status_accepts_unsafe_proposal_ignore_reason"
      "${torii_test}::test_get_sumeragi_status_accepts_all_twelve_ignore_reasons_at_the_bound"
      "${torii_test}::test_get_sumeragi_status_accepts_all_ten_liveness_queue_kinds"
      "${torii_test}::test_get_sumeragi_status_rejects_operational_diagnostics_fields"
      "${torii_test}::test_sumeragi_endpoint_methods_reject_swapped_payload_contracts"
      "${torii_test}::test_get_sumeragi_diagnostics_parses_exact_nested_fee_and_native_amx_receipts"
      "${torii_test}::test_get_sumeragi_diagnostics_accepts_first_native_amx_participant_block"
      "${torii_test}::test_get_sumeragi_diagnostics_accepts_mixed_role_proposal_without_current_entrypoint"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_native_amx_group_shape_drift"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_same_route_native_identity_drift"
      "${torii_test}::test_get_sumeragi_diagnostics_keeps_global_and_coordinator_views_independent"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_unordered_native_qc_validator_set"
      "${torii_test}::test_get_sumeragi_diagnostics_parses_ordered_native_application_evidence"
      "${torii_test}::test_get_sumeragi_diagnostics_accepts_native_application_state_geometry"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_invalid_native_application_shapes"
      "${torii_test}::test_get_sumeragi_diagnostics_enforces_native_application_bound_and_order"
      "${torii_test}::test_get_sumeragi_diagnostics_parses_autonomous_stage_and_conflict"
      "${torii_test}::test_get_sumeragi_diagnostics_requires_provisional_identity_hashes"
      "${torii_test}::test_get_sumeragi_diagnostics_enforces_reservation_only_geometry"
      "${torii_test}::test_get_sumeragi_diagnostics_enforces_finalized_identity_pair_and_order"
      "${torii_test}::test_get_sumeragi_diagnostics_parses_npos_windows_and_byte_seed"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_native_amx_participant_finality_tampering"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_noncanonical_quantity_json"
      "${torii_test}::test_get_sumeragi_diagnostics_preserves_exact_quantity_boundaries"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_retired_settlement_fields"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_retired_settlement_receipt_fields"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_noncanonical_fixed_hex_and_nested_unknown_fields"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_nested_receipt_coordinate_and_qc_tampering"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_bounded_vector_overflow_before_nested_decode"
      "${torii_test}::test_get_sumeragi_status_rejects_protocol_context_and_commit_tampering"
      "${torii_test}::test_get_sumeragi_status_allows_authenticated_bootstrap_without_commit_details"
      "${torii_test}::test_get_sumeragi_diagnostics_rejects_impossible_queue_bounds"
      "${torii_test}::test_get_sumeragi_diagnostics_requires_all_canonical_lane_arrays"
    )
    run_and_capture "$python_transcript" \
      env PYTHONNOUSERSITE=1 PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 \
      PYTHONPATH="$python_path" \
      "$python_bin" -m pytest -q -p no:cacheprovider \
      --basetemp="${temporary_root}/pytest" \
      "${python_diagnostics_tests[@]}"
    assert_pytest_count "$python_transcript" "$observed_test_count"
    ;;
  javascript)
    observed_test_count=88
    if ! command -v node >/dev/null 2>&1; then
      echo "Node.js is required for Sumeragi v2 JavaScript diagnostics" >&2
      exit 1
    fi
    readonly javascript_sdk_root="${repo_root}/javascript/iroha_js"
    readonly javascript_source_root="${javascript_sdk_root}/src"
    readonly javascript_package_root="${temporary_root}/javascript-package"
    readonly javascript_staged_source_root="${javascript_package_root}/src"
    readonly javascript_dist_root="${javascript_package_root}/dist"
    readonly javascript_dist_diff="${temporary_root}/javascript-dist.diff"
    readonly javascript_test="${javascript_sdk_root}/test/sumeragiDiagnosticsContract.test.js"
    if [[ -n "$(find "$javascript_source_root" -type l -print -quit)" ]]; then
      echo "Sumeragi v2 JavaScript diagnostics source tree must not contain symlinks" >&2
      exit 1
    fi
    if [[ ! -d "${javascript_sdk_root}/node_modules" \
      || -L "${javascript_sdk_root}/node_modules" ]]; then
      echo "Sumeragi v2 JavaScript diagnostics require a regular installed node_modules directory" >&2
      exit 1
    fi
    mkdir -p -- "$javascript_staged_source_root"
    cp "${javascript_sdk_root}/package.json" "${javascript_package_root}/package.json"
    ln -s "${javascript_sdk_root}/node_modules" "${javascript_package_root}/node_modules"
    cp -R "${javascript_source_root}/." "$javascript_staged_source_root/"
    env IROHA_JS_BUILD_DIST_ROOT="$javascript_package_root" \
      node "${javascript_sdk_root}/scripts/build-dist.mjs"
    if [[ -n "$(find "$javascript_dist_root" -type l -print -quit)" ]]; then
      echo "Sumeragi v2 staged JavaScript distribution must not contain symlinks" >&2
      exit 1
    fi
    if ! diff -qr "$javascript_source_root" "$javascript_dist_root" >"$javascript_dist_diff"; then
      echo "Sumeragi v2 staged JavaScript distribution differs from source" >&2
      sed -n '1,40p' "$javascript_dist_diff" >&2
      exit 1
    fi
    for javascript_variant in source dist; do
      if [[ "$javascript_variant" == source ]]; then
        javascript_client="${javascript_source_root}/toriiClient.js"
      else
        javascript_client="${javascript_dist_root}/toriiClient.js"
      fi
      javascript_transcript="${temporary_root}/javascript-${javascript_variant}.log"
      run_and_capture "$javascript_transcript" \
        env IROHA_JS_SUMERAGI_DIAGNOSTICS_TORII_CLIENT="$javascript_client" \
        node --test --test-reporter=tap "$javascript_test"
      assert_node_tap "$javascript_transcript" 44
      printf 'sumeragi-v2-sdk-diagnostics-run surface=javascript variant=%s tests=44 skipped=0\n' \
        "$javascript_variant"
    done
    ;;
  swift)
    observed_test_count=17
    if ! command -v swift >/dev/null 2>&1; then
      echo "Swift is required for Sumeragi v2 Swift diagnostics" >&2
      exit 1
    fi
    readonly swift_transcript="${temporary_root}/swift.log"
    readonly swift_filter='ToriiClientTests/testGetSumeragiStatusParsesAuthoritativeV2SnapshotAsync|ToriiClientTests/testSumeragiExecutionCommitmentRejectsNoncanonicalNativeAmxManifest|ToriiClientTests/testSumeragiExecutionCommitmentRequiresExactMergeCarrierProjection|ToriiClientTests/testSumeragiDiagnosticsPreservesNativeAmxV2AndNexusFeeReceipts|ToriiClientTests/testSumeragiDiagnosticsAutonomousExecutionStagesAndConflict|ToriiClientTests/testSumeragiDiagnosticsReservationsDurableIdentityAndGeometryAreExact|ToriiClientTests/testSumeragiDiagnosticsRejectsUnknownAutonomousLaneExecutionField|ToriiClientTests/testSumeragiDiagnosticsRejectsMalformedNativeAmxApplicationRows|ToriiClientTests/testGetSumeragiDiagnosticsParsesTypedLaneEvidenceAsync|ToriiClientTests/testGetSumeragiDiagnosticsCompletionParsesTypedLaneEvidence|ToriiClientTests/testSumeragiDiagnosticsRejectsWrongNativeAmxPhaseAndDuplicateLegs|ToriiClientTests/testSumeragiDiagnosticsRejectsNativeAmxSignatureAndPopLengthDrift|ToriiClientTests/testSumeragiDiagnosticsRejectsLowercaseSourceAndLegacyReceipt|ToriiClientTests/testSumeragiDiagnosticsRejectsNoncanonicalNexusFeeQuantities|ToriiClientTests/testSumeragiDiagnosticsRejectsNativeAmxContextAndEpochDrift|ToriiClientTests/testSumeragiDiagnosticsRejectsFlattenedNativeAmxPhaseAndSessionTampering|ToriiClientTests/testSumeragiV2StatusRejectsLegacyMissingAndMalformedShapes'
    run_and_capture "$swift_transcript" \
      swift test \
      --package-path "${repo_root}/IrohaSwift" \
      --scratch-path "${temporary_root}/swift-build" \
      --disable-automatic-resolution \
      --only-use-versions-from-resolved-file \
      --disable-swift-testing \
      --filter "$swift_filter"
    "$python_bin" - "$swift_transcript" "$observed_test_count" <<'PY'
from pathlib import Path
import re
import sys

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
expected = int(sys.argv[2])
summaries = [
    int(match.group(1))
    for line in lines
    if (match := re.search(r"Executed ([0-9]+) tests?, with 0 failures(?: \(0 unexpected\))?", line))
]
passed_cases = [line for line in lines if "Test Case " in line and " passed (" in line]
if not summaries or any(count != expected for count in summaries):
    raise SystemExit(f"expected exact {expected}-test Swift XCTest transcript")
if len(passed_cases) != expected or any("skipped" in line.lower() for line in lines):
    raise SystemExit("Swift diagnostics inventory must pass every selected test with zero skips")
PY
    ;;
  kotlin)
    observed_test_count=26
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    readonly kotlin_transcript="${temporary_root}/kotlin.log"
    mkdir -p -- "$gradle_build_root"
    run_and_capture "$kotlin_transcript" \
      env JAVA_HOME="$java_home" PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${repo_root}/kotlin/gradlew" \
      --offline --no-daemon --no-build-cache --rerun-tasks --console=plain \
      --project-dir "${repo_root}/kotlin" \
      --project-cache-dir "${temporary_root}/kotlin-project-cache" \
      --init-script "$gradle_init_path" \
      :core-jvm:test \
      --tests org.hyperledger.iroha.sdk.consensus.SumeragiDiagnosticsModelsTest \
      --tests org.hyperledger.iroha.sdk.consensus.SumeragiStatusModelsTest \
      --tests org.hyperledger.iroha.sdk.client.SumeragiHttpTransportContractTest
    assert_gradle_reports \
      "$gradle_build_root" "$observed_test_count" \
      org.hyperledger.iroha.sdk.consensus.SumeragiDiagnosticsModelsTest \
      org.hyperledger.iroha.sdk.consensus.SumeragiStatusModelsTest \
      org.hyperledger.iroha.sdk.client.SumeragiHttpTransportContractTest
    ;;
  java)
    observed_test_count=24
    java_home="$(resolve_java_home)"
    readonly java_home
    readonly gradle_build_root="${temporary_root}/gradle-build"
    readonly java_transcript="${temporary_root}/java.log"
    mkdir -p -- "$gradle_build_root"
    run_and_capture "$java_transcript" \
      env JAVA_HOME="$java_home" PATH="${java_home}/bin:${PATH}" \
      IROHA_NATIVE_AMX_V2_GROUPED_GRADLE_BUILD_ROOT="$gradle_build_root" \
      "${repo_root}/java/iroha_android/gradlew" \
      --offline --no-daemon --no-build-cache --rerun-tasks --console=plain \
      --project-dir "${repo_root}/java/iroha_android" \
      --project-cache-dir "${temporary_root}/java-project-cache" \
      --init-script "$gradle_init_path" \
      :core:test \
      --tests org.hyperledger.iroha.android.client.SumeragiHttpTransportTests \
      --tests org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModelsTests \
      --tests org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsParsingTests \
      --tests org.hyperledger.iroha.android.consensus.SumeragiStatusModelsTests
    assert_gradle_reports \
      "$gradle_build_root" "$observed_test_count" \
      org.hyperledger.iroha.android.client.SumeragiHttpTransportTests \
      org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModelsTests \
      org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsParsingTests \
      org.hyperledger.iroha.android.consensus.SumeragiStatusModelsTests
    ;;
esac

printf 'sumeragi-v2-sdk-diagnostics surface=%s tests=%s suite_source_manifest_sha256=%s\n' \
  "$surface" \
  "$observed_test_count" \
  "$suite_source_manifest_sha256"
