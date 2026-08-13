#!/usr/bin/env bash
# Static inventory guard for mandatory four-peer Sumeragi V2 multilane release gates.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

readonly autoscale_file="integration_tests/tests/nexus/autoscale_localnet.rs"
readonly native_file="integration_tests/tests/native_amx_routing.rs"
readonly native_recovery_file="$native_file"
readonly launcher="scripts/run_nexus_cross_dataspace_atomic_swap.sh"
readonly release_runner="scripts/run_sumeragi_v2_release_gates.sh"
readonly release_bootstrap="scripts/bootstrap_sumeragi_v2_release.py"
readonly release_bootstrap_test="pytests/scripts/sumeragi_v2_release_bootstrap_test.py"
readonly cargo_cache_copier="scripts/copy_sumeragi_v2_release_cargo_cache.py"
readonly grouped_parity_harness="ci/run_native_amx_v2_grouped_sdk_parity.sh"
readonly sdk_diagnostics_harness="ci/run_sumeragi_v2_sdk_diagnostics.sh"
readonly js_sdk_diagnostics_test="javascript/iroha_js/test/sumeragiDiagnosticsContract.test.js"
readonly grouped_fixture="fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly closure_ledger="specs/sumeragi_v2_multilane_closure_ledger.md"
readonly release_receipt_writer="scripts/write_sumeragi_v2_release_receipt.py"
readonly release_receipt_component="scripts/write_sumeragi_v2_release_receipt_formal_artifacts.py"
readonly release_receipt_corridor_component="scripts/write_sumeragi_v2_release_receipt_corridor_log.py"
readonly prebuilt_bundle_shell="scripts/sumeragi_v2_prebuilt_bundle.sh"
readonly prebuilt_bundle_helper="scripts/sumeragi_v2_prebuilt_bundle.py"
readonly process_policy="scripts/sumeragi_v2_release_process_policy.sh"
readonly cargo_proxy="scripts/sumeragi_v2_release_cargo_proxy.sh"
readonly marker_publisher="scripts/publish_release_marker.py"
readonly pr_workflow=".github/workflows/pr.yml"
readonly nexus_cross_dataspace_pr_helper="ci/check_nexus_cross_dataspace_localnet.sh"
readonly nexus_cross_lane_pr_helper="ci/check_nexus_cross_lane_proofs.sh"
readonly nexus_pr_helper_test="pytests/scripts/nexus_pr_helpers_test.py"
readonly seed_runner="scripts/run_sumeragi_v2_seed_matrix.sh"
readonly formal_harness="scripts/formal/run_sumeragi_v2_harness.sh"
readonly formal_release_runner="scripts/run_sumeragi_v2_formal_release.sh"
readonly formal_gate="ci/check_sumeragi_formal.sh"
readonly verus_runner="scripts/verify_sumeragi_v2.sh"
readonly replay_runner="scripts/formal/check_sumeragi_v2_replay_trace.sh"
readonly chaos_runner="scripts/run_sumeragi_v2_100k_chaos.sh"
readonly taira_runner="scripts/run_taira_v2_24h_soak.sh"
readonly taira_strict_restart_source="integration_tests/tests/taira_public_localnet/strict_restart.rs"
readonly taira_strict_restart_test="taira_localnet_restart_catchup_behavior"
readonly taira_strict_restart_qualified_test="taira_public_localnet::strict_restart::${taira_strict_restart_test}"
readonly kura_source="crates/iroha_core/src/kura.rs"
readonly test_network_source="crates/iroha_test_network/src/lib.rs"
readonly autoscale_test="nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_rejects_stale_artifacts"
readonly autoscale_qualified_test="nexus::autoscale_localnet::${autoscale_test}"
readonly autoscale_restart_test="nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart"
readonly autoscale_restart_qualified_test="nexus::autoscale_localnet::${autoscale_restart_test}"
readonly autoscale_drain_test="nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_restart"
readonly autoscale_drain_qualified_test="nexus::autoscale_localnet::${autoscale_drain_test}"
readonly native_test="native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
readonly native_grouped_pruning_marker="[multilane-release-native-evidence] grouped_sources=2 durable_manifest=passed body_eviction_recovery=passed authenticated_remote_recovery=passed exact_once=passed"
readonly canonical_production_test_count=855

require_nonignored_test() {
  local path="$1"
  local test_name="$2"
  local declaration_count declaration_line window_start
  declaration_count="$(
    grep -Ec -- "^(async )?fn ${test_name}\\(" "$path" || true
  )"
  if [[ "$declaration_count" != 1 ]]; then
    echo "expected one exact mandatory multilane test declaration ${test_name} in ${path}; found ${declaration_count}" >&2
    exit 1
  fi
  declaration_line="$(
    grep -En -- "^(async )?fn ${test_name}\\(" "$path" | cut -d: -f1
  )"
  window_start=$((declaration_line > 12 ? declaration_line - 12 : 1))
  if ! sed -n "${window_start},${declaration_line}p" "$path" \
    | grep -Eq '^#\[(tokio::)?test'; then
    echo "mandatory multilane declaration lacks a test attribute: ${test_name}" >&2
    exit 1
  fi
  if sed -n "${window_start},${declaration_line}p" "$path" \
    | grep -Eq '^#\[ignore([=(]|$)'; then
    echo "mandatory multilane release test must not be ignored: ${test_name}" >&2
    exit 1
  fi
}

require_exact_token() {
  local path="$1"
  local token="$2"
  if [[ "$(grep -Fxc -- "$token" "$path" || true)" != 1 ]]; then
    echo "required multilane release inventory token is missing or duplicated in ${path}: ${token}" >&2
    exit 1
  fi
}

require_exact_digest_occurrences() {
  local path="$1"
  local digest="$2"
  local expected_count="$3"
  local label="$4"
  local observed_count
  if [[ ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
    echo "${label} is not a lowercase SHA-256 digest: ${digest}" >&2
    exit 1
  fi
  observed_count="$(
    awk -v needle="$digest" '
      { count += gsub(needle, "") }
      END { print count + 0 }
    ' "$path"
  )"
  if [[ "$observed_count" != "$expected_count" ]]; then
    echo "${path} must publish the current ${label} exactly ${expected_count} times; found ${observed_count}" >&2
    exit 1
  fi
}

require_nonignored_test "$autoscale_file" "$autoscale_test"
require_nonignored_test "$autoscale_file" "$autoscale_restart_test"
require_nonignored_test "$autoscale_file" "$autoscale_drain_test"
require_nonignored_test "$native_file" "$native_test"
require_nonignored_test "$taira_strict_restart_source" "$taira_strict_restart_test"

require_exact_token \
  "$release_runner" \
  "  ${taira_strict_restart_qualified_test}"
require_exact_token \
  "$release_receipt_writer" \
  "    \"${taira_strict_restart_qualified_test}\","

if [[ ! -f "$release_receipt_component" || -L "$release_receipt_component" ]]; then
  echo "release receipt formal-artifact component must be a regular non-symlink file" >&2
  exit 1
fi
if [[ ! -f "$release_receipt_corridor_component" || -L "$release_receipt_corridor_component" ]]; then
  echo "release receipt corridor-log component must be a regular non-symlink file" >&2
  exit 1
fi
if [[ ! -f "$cargo_proxy" || -L "$cargo_proxy" ]]; then
  echo "release Cargo proxy must be a regular non-symlink file" >&2
  exit 1
fi

require_exact_token "$cargo_proxy" 'source "${PROCESS_POLICY}"'
require_exact_token "$cargo_proxy" 'require_external_cargo_target_dir "${REPO_ROOT}"'
require_exact_token "$cargo_proxy" 'run_cargo "$@"'

require_exact_token \
  "$launcher" \
  "readonly AUTOSCALE_FOUR_PEER_RELEASE_TEST=\"${autoscale_qualified_test}\""
require_exact_token \
  "$launcher" \
  "readonly AUTOSCALE_RESTART_FOUR_PEER_RELEASE_TEST=\"${autoscale_restart_qualified_test}\""
require_exact_token \
  "$launcher" \
  "readonly AUTOSCALE_DRAIN_FOUR_PEER_RELEASE_TEST=\"${autoscale_drain_qualified_test}\""
require_exact_token \
  "$launcher" \
  "readonly NATIVE_AMX_FAULT_SOAK_TEST=\"${native_test}\""
require_exact_token \
  "$launcher" \
  "readonly NATIVE_AMX_GROUPED_PRUNING_MARKER=\"${native_grouped_pruning_marker}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_autoscale_four_peer_release_test=\"${autoscale_qualified_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_autoscale_restart_release_test=\"${autoscale_restart_qualified_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_autoscale_drain_release_test=\"${autoscale_drain_qualified_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_native_amx_rotating_release_test=\"${native_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_native_amx_grouped_pruning_marker=\"${native_grouped_pruning_marker}\""
require_exact_token \
  "$release_runner" \
  "    \$'native_grouped_pruning_evidence\\tpassed'; do"
require_exact_token \
  "$release_runner" \
  "readonly native_amx_grouped_parity_harness=\"${grouped_parity_harness}\""
require_exact_token \
  "$release_runner" \
  "readonly sumeragi_v2_sdk_diagnostics_harness=\"${sdk_diagnostics_harness}\""
require_exact_token \
  "$release_runner" \
  "readonly expected_multilane_focus_test_count=525"
require_exact_token \
  "$release_runner" \
  "readonly expected_multilane_formal_mutation_count=106"
require_exact_token \
  "$release_runner" \
  "  'echo \"[tlc] all 106 multilane mutations produced their exact named counterexamples; no deductive proof status was changed\"' \\"
require_exact_token \
  "$release_runner" \
  "readonly expected_production_liveness_test_count=${canonical_production_test_count}"
require_exact_token \
  "$release_runner" \
  "  readonly expected_corridor_leg_count=88"
require_exact_token \
  "$release_runner" \
  "export CARGO_INCREMENTAL=0"
require_exact_token \
  "$release_runner" \
  "    CARGO_INCREMENTAL=0 \\"
require_exact_token \
  "$release_runner" \
  "    g_unit_expected_test_count \"\$expected_multilane_focus_test_count\" \\"
require_exact_token \
  "$release_runner" \
  "    g_unit_passed_test_count \"\$expected_multilane_focus_test_count\" \\"
require_exact_token \
  "$release_runner" \
  "    g_unit_inventory_sha256 \"\$corridor_g_unit_inventory_sha256\" \\"
require_exact_token \
  "$release_runner" \
  "    native_amx_grouped_fixture_sha256 \"\$native_amx_grouped_fixture_sha256\" \\"
require_exact_token \
  "$release_runner" \
  "    native_amx_grouped_negative_control_count 55 \\"
require_exact_token \
  "$grouped_parity_harness" \
  "readonly expected_negative_control_count=55"
for grouped_test_count in 7 62 60 4 6 5; do
  require_exact_token \
    "$grouped_parity_harness" \
    "    observed_test_count=${grouped_test_count}"
done
require_exact_token \
  "$release_receipt_writer" \
  "_NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT = 55"
require_exact_token \
  "$release_receipt_writer" \
  "_G_UNIT_TEST_COUNT = 525"
require_exact_token \
  "$release_receipt_writer" \
  "_PRODUCTION_TEST_COUNT = ${canonical_production_test_count}"
require_exact_token \
  "$release_receipt_writer" \
  "    \"write_sumeragi_v2_release_receipt_formal_artifacts.py\","
require_exact_token \
  "$release_receipt_writer" \
  "for _release_receipt_component in _RELEASE_RECEIPT_COMPONENT_FILES:"
require_exact_token \
  "$release_receipt_writer" \
  '        "kura-replica-retention",'
require_exact_token \
  "$release_receipt_writer" \
  '        "SumeragiV2KuraReplicaRetention",'
require_exact_token \
  "$release_receipt_writer" \
  '        "kura_replica_retention_fixed.cfg",'
require_exact_token \
  "$release_receipt_writer" \
  "_G4P_NATIVE_AMX_GROUPED_PRUNING_MARKER = ("
require_exact_token \
  "$release_receipt_writer" \
  '        "native_grouped_pruning_evidence": "passed",'
for grouped_suite in \
  '    ("openapi", 7),' \
  '    ("python", 62),' \
  '    ("javascript", 60),' \
  '    ("swift", 4),' \
  '    ("kotlin", 6),' \
  '    ("java", 5),'; do
  require_exact_token "$release_receipt_writer" "$grouped_suite"
done
for sdk_diagnostics_suite in \
  '    ("python", 121),' \
  '    ("javascript", 88),' \
  '    ("swift", 17),' \
  '    ("kotlin", 26),' \
  '    ("java", 24),'; do
  require_exact_token "$release_receipt_writer" "$sdk_diagnostics_suite"
done
for sdk_diagnostics_test_count in 121 88 17 26 24; do
  require_exact_token \
    "$sdk_diagnostics_harness" \
    "    observed_test_count=${sdk_diagnostics_test_count}"
done
require_exact_token \
  "$sdk_diagnostics_harness" \
  '      assert_node_tap "$javascript_transcript" 44'
require_exact_token \
  "$js_sdk_diagnostics_test" \
  '  "typed Sumeragi endpoints reject swapped status and diagnostics payloads",'

python3 -I -S - \
  "$release_runner" \
  "$release_receipt_writer" \
  "$release_receipt_component" \
  "$release_receipt_corridor_component" \
  "$canonical_production_test_count" \
  "$process_policy" \
  "$cargo_cache_copier" \
  "$release_bootstrap" \
  "$release_bootstrap_test" <<'PY'
from __future__ import annotations

import ast
import hashlib
from pathlib import Path
import re
import sys


runner = Path(sys.argv[1])
source = runner.read_text(encoding="utf-8")
lines = source.splitlines()
receipt_writer = Path(sys.argv[2])
receipt_source = receipt_writer.read_text(encoding="utf-8")
receipt_component = Path(sys.argv[3])
receipt_component_source = receipt_component.read_text(encoding="utf-8")
receipt_corridor_component = Path(sys.argv[4])
receipt_corridor_component_source = receipt_corridor_component.read_text(encoding="utf-8")
canonical_production_test_count = int(sys.argv[5])
process_policy = Path(sys.argv[6])
process_policy_source = process_policy.read_text(encoding="utf-8")
cargo_cache_copier = Path(sys.argv[7])
cargo_cache_source = cargo_cache_copier.read_text(encoding="utf-8")
release_bootstrap = Path(sys.argv[8])
release_bootstrap_source = release_bootstrap.read_text(encoding="utf-8")
release_bootstrap_test = Path(sys.argv[9])
release_bootstrap_test_source = release_bootstrap_test.read_text(encoding="utf-8")


def reject(message: str) -> None:
    raise SystemExit(f"{runner}: {message}")


expected_shell_utilities = (
    "awk", "basename", "cat", "chmod", "cmp", "cp", "cut", "diff",
    "dirname", "env", "find", "grep", "ln", "ls", "mkdir", "mkfifo",
    "mktemp", "mv", "openssl", "rm", "rmdir", "sed", "sh", "sleep",
    "tail", "tee", "tr", "uname", "wc", "xargs",
    "shasum" if sys.platform == "darwin" else "sha256sum",
)
expected_language_tools = (
    "cargo", "cargo-verus", "git-index-pack", "git-upload-pack", "java",
    "node", "rustc", "swift", "tlapm", "verus",
)


def assigned_string_collection(source_text: str, name: str) -> tuple[str, ...]:
    tree = ast.parse(source_text)
    assignments = [
        node for node in tree.body
        if isinstance(node, ast.Assign)
        and any(isinstance(target, ast.Name) and target.id == name for target in node.targets)
    ]
    if len(assignments) != 1:
        reject(f"{name} must have one assignment")
    value = assignments[0].value
    if isinstance(value, ast.Call):
        if (
            not isinstance(value.func, ast.Name)
            or value.func.id != "frozenset"
            or len(value.args) != 1
            or value.keywords
            or not isinstance(value.args[0], ast.Set)
        ):
            reject(f"{name} must be one literal tuple or frozenset")
        elements = value.args[0].elts
    elif isinstance(value, ast.Tuple):
        elements = value.elts
    else:
        reject(f"{name} must be one literal tuple or frozenset")
    values: list[str] = []
    for element in elements:
        if isinstance(element, ast.Constant) and isinstance(element.value, str):
            values.append(element.value)
        elif (
            isinstance(element, ast.IfExp)
            and isinstance(element.test, ast.Compare)
            and isinstance(element.test.left, ast.Attribute)
            and isinstance(element.test.left.value, ast.Name)
            and element.test.left.value.id == "sys"
            and element.test.left.attr == "platform"
            and isinstance(element.body, ast.Constant)
            and isinstance(element.body.value, str)
            and isinstance(element.orelse, ast.Constant)
            and isinstance(element.orelse.value, str)
        ):
            values.append(element.body.value if sys.platform == "darwin" else element.orelse.value)
        else:
            reject(f"{name} contains a non-literal command selector")
    return tuple(values)


if set(assigned_string_collection(release_bootstrap_source, "_RELEASE_SHELL_UTILITY_NAMES")) != set(expected_shell_utilities):
    reject("bootstrap shell-utility command closure is not exact")
if set(assigned_string_collection(release_bootstrap_source, "_RELEASE_LANGUAGE_TOOL_NAMES")) != set(expected_language_tools):
    reject("bootstrap release language-tool closure is not exact")
if assigned_string_collection(cargo_cache_source, "_RELEASE_SHELL_UTILITY_NAMES") != expected_shell_utilities:
    reject("private-runtime shell-utility command closure is not exact")
for token, count in (
    ("*_RELEASE_SHELL_UTILITY_NAMES,", 2),
    ("set(tools) != _REQUIRED_RUNNER_TOOL_NAMES", 1),
):
    observed = (
        cargo_cache_source.count(token)
        if token.startswith("*_RELEASE")
        else release_bootstrap_source.count(token)
    )
    if observed != count:
        reject(f"release command-closure guard changed: {token}")


def runner_array(name: str) -> tuple[str, ...]:
    match = re.search(
        rf"^  {re.escape(name)}=\(\n(?P<body>.*?)^  \)$",
        source,
        flags=re.MULTILINE | re.DOTALL,
    )
    if match is None:
        reject(f"release runner lacks exact {name} array")
    return tuple(match.group("body").split())


for array_name in ("pr_shell_utility_names", "release_shell_utility_names"):
    if runner_array(array_name) != expected_shell_utilities[:-1]:
        reject(f"release runner {array_name} fixed command closure is not exact")
if source.count("/usr/bin:/bin") != 0 or source.count("/usr/bin/env -i") != 0:
    reject("private release children retain an ambient execution fallback")
for token, count in (
    ('"$pr_bin/env" -i \\', 1),
    ('"$release_child_bin/env" -i \\', 1),
    ('export PATH="$IROHA_RELEASE_PR_BIN" GIT_EXEC_PATH="$IROHA_RELEASE_PR_BIN"', 1),
    ('export PATH="${IROHA_RELEASE_INVOCATION_ROOT}/runtime/bin"', 1),
):
    if source.count(token) != count:
        reject(f"private child command-closure binding changed: {token}")
for token, count in (
    ('"unlisted-command": "iroha-unlisted-release-command"', 1),
    ("def test_undeclared_runner_tool_has_no_ambient_path_fallback(", 1),
    ("assert result.returncode == 127", 1),
):
    if release_bootstrap_test_source.count(token) != count:
        reject(f"unknown-command rejection contract changed: {token}")

for token, count in (
    ("# RELEASE_CARGO_CACHE_COPY_HELPER_V1", 1),
    ("MAXIMUM_RECORDS = 250_000", 1),
    ("MAXIMUM_FILE_BYTES = 4 * 1024 * 1024 * 1024", 1),
    ("MAXIMUM_TOTAL_BYTES = 64 * 1024 * 1024 * 1024", 1),
    ("MAXIMUM_DEPTH = 128", 1),
    ("MAXIMUM_PATH_BYTES = 4096", 1),
    ('FINAL_FORMAT = "iroha-sumeragi-v2-cargo-cache-final"', 1),
    ('"source_read_semantics": "read-only; host filesystem may update access time"', 1),
    ("or (copied.st_dev, copied.st_ino) == (opened.st_dev, opened.st_ino)", 1),
    ("cache symlink escapes its cache root", 1),
    ("cache entry is a forbidden special file", 2),
    ("source, private Cargo home, and inventory must be disjoint", 1),
    ("def snapshot_cache(", 1),
    ("_rename_noreplace_at(destination_parent_fd, stage_name, destination_name)", 1),
    ("_rename_noreplace_at(parent_fd, temporary, inventory_path.name)", 1),
    ("inventory parent must be owner-owned with mode 0700", 1),
    ('"source_cargo_home_disclosure": "withheld"', 1),
    ("def copy_runtime(", 1),
    ("def seal_release_result(", 1),
    ("receipt-validation-ack.json", 3),
    ("receipt validation acknowledgment contract is not exact", 1),
):
    if cargo_cache_source.count(token) != count:
        raise SystemExit(
            f"{cargo_cache_copier}: private cache-copy contract token is not exact: {token!r}"
        )

if "_prebuilt_workspace_target" in receipt_source:
    raise SystemExit(
        f"{receipt_writer}: receipt must not derive prebuilt evidence from a "
        "repository target path"
    )
if receipt_corridor_component_source.count("def _prebuilt_artifact_root(") != 1:
    raise SystemExit(
        f"{receipt_corridor_component}: prebuilt artifact-root binding is not exact"
    )
if 'Path(fields["artifact_root_path"])' in receipt_corridor_component_source:
    raise SystemExit(
        f"{receipt_corridor_component}: corridor fields must not authorize an artifact root"
    )

def exact_line(line: str) -> int:
    matches = [index for index, candidate in enumerate(lines) if candidate == line]
    if len(matches) != 1:
        reject(f"expected one exact line {line!r}; found {len(matches)}")
    return matches[0]


production_marker = "required_production_liveness_tests=(\n"
if source.count(production_marker) != 1:
    reject("release runner must contain one canonical production inventory")
production_body = source.split(production_marker, 1)[1].split("\n)", 1)[0]
production_tests = [
    line.strip() for line in production_body.splitlines() if line.strip()
]
if (
    len(production_tests) != canonical_production_test_count
    or len(set(production_tests)) != canonical_production_test_count
):
    reject(
        "production inventory must contain exactly "
        f"{canonical_production_test_count} unique tests"
    )

if any(
    test.startswith("sumeragi::v2_core::network_simulation::")
    for test in production_tests
):
    reject("retired dormant network simulations must stay out of production inventory")

receipt_tree = ast.parse(receipt_source, filename=str(receipt_writer))
receipt_assignments: dict[str, object] = {}
for node in receipt_tree.body:
    if not isinstance(node, ast.Assign) or len(node.targets) != 1:
        continue
    target = node.targets[0]
    if (
        isinstance(target, ast.Name)
        and target.id
        in {
            "_RELEASE_RECEIPT_COMPONENT_FILES",
            "_PRODUCTION_TEST_COUNT",
            "_PRODUCTION_MODULES",
            "_APALACHE_REFINEMENT_RESULTS",
            "_APALACHE_LAYOUT_ONLY_RESULTS",
        }
    ):
        receipt_assignments[target.id] = ast.literal_eval(node.value)
expected_receipt_components = (
    "write_sumeragi_v2_release_receipt_formal_artifacts.py",
    "write_sumeragi_v2_release_receipt_corridor_log.py",
)
if (
    receipt_assignments.get("_RELEASE_RECEIPT_COMPONENT_FILES")
    != expected_receipt_components
):
    reject("receipt writer component manifest is not exact")
expected_receipt_component_symbols = (
    "_validate_multilane_apalache_evidence",
    "_validate_formal_snapshot_replays",
    "_formal_artifacts",
)
expected_receipt_corridor_component_symbols = (
    "_receipt_validation_invocation_value_sha256",
    "_receipt_validation_invocation_binding",
    "_cargo_cache_relative_path",
    "_cargo_cache_final_relative_path",
    "_cargo_cache_octal_mode",
    "_cargo_cache_integer",
    "_cargo_cache_unchanged",
    "_cargo_cache_names",
    "_cargo_cache_stat",
    "_cargo_cache_open_regular",
    "_cargo_cache_tree",
    "_validate_cargo_cache_input",
    "_sdk_suite_source_manifest",
    "_test_count_from_log",
    "_prebuilt_artifact_root",
    "_prebuilt_release_roots",
    "_prebuilt_directory",
    "_publish_receipt_validation_ack",
    "_receipt_validation_ack_arguments",
    "_receipt_validation_ack",
    "_owned_unlink_name",
    "_corridor_legs",
)
parent_component_symbols = tuple(
    node.name
    for node in receipt_tree.body
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    and node.name
    in expected_receipt_component_symbols
    + expected_receipt_corridor_component_symbols
)
if parent_component_symbols:
    reject("receipt writer formal-artifact functions are not source-isolated")
receipt_component_tree = ast.parse(
    receipt_component_source, filename=str(receipt_component)
)
component_symbols = tuple(
    node.name
    for node in receipt_component_tree.body
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
)
if component_symbols != expected_receipt_component_symbols:
    reject("receipt writer formal-artifact component symbol inventory is not exact")
receipt_corridor_component_tree = ast.parse(
    receipt_corridor_component_source,
    filename=str(receipt_corridor_component),
)
corridor_component_symbols = tuple(
    node.name
    for node in receipt_corridor_component_tree.body
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
)
if corridor_component_symbols != expected_receipt_corridor_component_symbols:
    reject("receipt writer corridor-log component symbol inventory is not exact")
if (
    receipt_assignments.get("_PRODUCTION_TEST_COUNT")
    != canonical_production_test_count
):
    reject(
        "receipt writer production count must equal "
        f"{canonical_production_test_count}"
    )
production_modules = receipt_assignments.get("_PRODUCTION_MODULES")
if not isinstance(production_modules, tuple) or len(production_modules) != 40:
    reject("receipt writer must bind exactly 40 production modules")
module_counts = {
    module: count for _leg_id, module, count in production_modules
}
if (
    len(module_counts) != 40
    or sum(module_counts.values()) != canonical_production_test_count
):
    reject(
        "receipt writer production-module counts must sum exactly to "
        f"{canonical_production_test_count}"
    )
if "sumeragi::v2_core::network_simulation" in module_counts:
    reject("retired dormant network-simulation module must stay out of the receipt")


def shell_array(name: str) -> tuple[str, ...]:
    declaration = f"{name}=("
    if lines.count(declaration) != 1:
        reject(f"{name} must be declared exactly once")
    start = lines.index(declaration)
    try:
        end = lines.index(")", start + 1)
    except ValueError:
        reject(f"{name} is unterminated")
    return tuple(
        line.strip()
        for line in lines[start + 1 : end]
        if line.strip() and not line.lstrip().startswith("#")
    )


runner_modules = shell_array("production_liveness_modules")
runner_leg_ids = shell_array("production_liveness_leg_ids")
receipt_modules = tuple(module for _leg_id, module, _count in production_modules)
receipt_leg_ids = tuple(leg_id for leg_id, _module, _count in production_modules)
if runner_modules != receipt_modules or len(set(runner_modules)) != 40:
    reject("release runner must bind the exact 40 receipt production modules")
if runner_leg_ids != receipt_leg_ids or len(set(runner_leg_ids)) != 40:
    reject("release runner must bind the exact 40 receipt production leg IDs")

expected_apalache_refinement_results = (
    (
        "autoscale-lifecycle",
        "SumeragiV2AutoscaleLifecycle",
        "multilane_autoscale_lifecycle_fixed.cfg",
        "8",
    ),
    (
        "native-application-evidence",
        "SumeragiV2NativeApplicationEvidence",
        "multilane_native_application_evidence_fixed.cfg",
        "8",
    ),
    (
        "autonomous-reservation-carrier",
        "SumeragiV2AutonomousReservationCarrier",
        "multilane_autonomous_reservation_carrier_fixed.cfg",
        "10",
    ),
    (
        "queue-plan-admission-registry",
        "SumeragiV2QueuePlanAdmissionRegistry",
        "multilane_queue_plan_admission_registry_fixed.cfg",
        "8",
    ),
    (
        "kura-replica-retention",
        "SumeragiV2KuraReplicaRetention",
        "kura_replica_retention_fixed.cfg",
        "8",
    ),
)
expected_apalache_layout_results = (
    (
        "inflight-first-release-layout",
        "SumeragiV2InFlightFirstRelease",
        "inflight_first_release_fixed.cfg",
        "18",
    ),
)
if (
    receipt_assignments.get("_APALACHE_REFINEMENT_RESULTS")
    != expected_apalache_refinement_results
    or receipt_assignments.get("_APALACHE_LAYOUT_ONLY_RESULTS")
    != expected_apalache_layout_results
):
    reject(
        "receipt writer must bind exactly five Apalache refinement results "
        "plus the one layout-only result"
    )
expected_changed_module_counts = {
    "kura::tests": 17,
    "sumeragi::authoritative_runtime_gate_tests": 43,
    "sumeragi::serviced_candidate_store::tests": 1,
    "sumeragi::v2_effects::tests": 72,
    "sumeragi::v2::tests": 47,
    "sumeragi::v2_runtime::tests": 68,
    "merge_sidecar::tests": 118,
    "state::tests": 1,
    "sumeragi::v2_lane_work::tests": 61,
    "sumeragi::v2_lifecycle_recovery::tests": 5,
    "sumeragi::v2_runner::tests": 37,
    "sumeragi::v2_worker::tests": 133,
    "network::tests": 84,
    "network::inbound_source_memory_bound_tests": 2,
    "network::handle_update_tests": 4,
    "network_relay_tests": 4,
}
if any(
    module_counts.get(module) != expected
    for module, expected in expected_changed_module_counts.items()
):
    reject("receipt writer changed-module production counts are not canonical")

canonical_rows = ["module\ttest"]
observed_counts = {module: 0 for module in module_counts}
for test in production_tests:
    matches = [
        module for module in module_counts if test.startswith(f"{module}::")
    ]
    if len(matches) != 1:
        reject(f"production test has no unique module owner: {test}")
    module = matches[0]
    observed_counts[module] += 1
    canonical_rows.append(f"{module}\t{test}")
if observed_counts != module_counts:
    reject("release runner inventory does not match receipt module counts")
canonical_inventory = ("\n".join(canonical_rows) + "\n").encode()
if hashlib.sha256(canonical_inventory).hexdigest() != (
    "a40a9d7ef0dafcad2a6e3eb710d550a7"
    "f80f905c378117ef9a52b39a86d77b1e"
):
    reject(
        f"canonical {canonical_production_test_count}-test production TSV "
        "digest changed"
    )


expected_policy_source = (
    'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"'
)
if source.count(expected_policy_source) != 1:
    reject("release runner must source the one reviewed process policy")
if any(
    definition in source
    for definition in (
        "acquire_invocation_cargo_lock() {",
        "release_invocation_cargo_lock() {",
        "run_cargo() {",
    )
):
    reject("release runner must not shadow the shared process policy")
if process_policy_source.count("acquire_invocation_cargo_lock() {") != 1:
    reject("shared process policy must define one invocation-local Cargo lock")
if process_policy_source.count("release_invocation_cargo_lock() {") != 1:
    reject("shared process policy must define one Cargo lock release")
if process_policy_source.count("run_cargo() {") != 1:
    reject("shared process policy must define one Cargo wrapper")
for token in (
    "lock.mkdir(mode=0o700)",
    'pinned_arguments=("$subcommand" -j1)',
    'pinned_arguments+=("$@")',
    'local RUSTUP_AUTO_INSTALL=0',
    'export RUSTUP_AUTO_INSTALL',
    'run_cargo forbids caller-owned rustup auto-install policy',
    'run_cargo requires IROHA_RELEASE_CARGO_BIN',
    'if "$IROHA_RELEASE_CARGO_BIN" "$@"; then',
    '_release_scoped_invocation_cargo_lock() {',
    'trap _release_scoped_invocation_cargo_lock RETURN EXIT',
    'os.rmdir(lock.name, dir_fd=root_fd)',
    'if ((cargo_prefix)) && [[ "$argument" == "--" ]]; then',
    '--target-dir|--target-dir=*|--manifest-path|--manifest-path=*|--config|--config=*',
    "require_disjoint_release_roots() {",
    "build|test|run|clippy|verus)",
    'b\'{"reason":"operator-request","schema_version":1}\\n\'',
):
    if process_policy_source.count(token) != 1:
        reject(f"shared process policy lacks exact required token: {token}")
if process_policy_source.count("lock.rmdir()") != 1:
    reject("shared process policy must clean only a partially-acquired private lock")
if process_policy_source.count(
    'lock_path="${artifact_root}/.sumeragi-v2-cargo.lock"'
) != 2:
    reject("shared process policy must bind acquire and release to the exact lock path")
if process_policy_source.count("release_invocation_cargo_lock || return $?") != 1:
    reject("shared process policy must release its lock after natural completion")
for forbidden in (
    "SIGSTOP",
    "SIGTERM",
    "SIGKILL",
    "killpg",
    "pkill",
    "renice",
    "start_new_session",
    ".terminate(",
    ".kill(",
    "wait_for_external_cargo",
    "ps -",
    "pgrep",
    "/proc/",
    "process_snapshot",
    "sleep ",
):
    if forbidden in process_policy_source:
        reject(
            "shared process policy contains forbidden process control or "
            f"observation: {forbidden}"
        )

native_amx_parity_inventory = """\
  native_amx_grouped_parity_surfaces=(
    openapi
    python
    javascript
    swift
    kotlin
    java
  )
  native_amx_grouped_parity_test_counts=(
    7
    62
    60
    4
    6
    5
  )"""
if source.count(native_amx_parity_inventory) != 1:
    reject(
        "grouped Native AMX V2 release surfaces and exact test counts "
        "must remain paired in canonical order"
    )

# Command descriptions containing the authenticated Cargo path are
# source-sealed evidence, not execution. Reject every direct shell execution
# form in the runner; the sole Cargo execution lives in the shared policy.
direct_cargo_patterns = (
    re.compile(r"^\s*cargo(?:\s|$)"),
    re.compile(r"^\s*command\s+cargo(?:\s|$)"),
    re.compile(
        r"^\s*(?:[A-Za-z_][A-Za-z0-9_]*=[^\s]+\s+)+"
        r"(?:command\s+)?cargo(?:\s|$)"
    ),
    re.compile(r"^\s*env(?:\s+[^\s]+)*\s+(?:command\s+)?cargo(?:\s|$)"),
    re.compile(r"(?:&&|\|\||;)\s*(?:command\s+)?cargo(?:\s|$)"),
    re.compile(r"\$\(\s*(?:command\s+)?cargo(?:\s|$)"),
    re.compile(r"`\s*(?:command\s+)?cargo(?:\s|$)"),
)
direct_cargo_lines: list[tuple[int, str]] = []
for line_number, line in enumerate(lines, start=1):
    if any(pattern.search(line) for pattern in direct_cargo_patterns):
        direct_cargo_lines.append((line_number, line))
if direct_cargo_lines:
    rendered = ", ".join(f"{number}:{line}" for number, line in direct_cargo_lines)
    reject(f"Cargo execution bypasses run_cargo ({rendered})")

version_functions = [
    node
    for node in receipt_tree.body
    if isinstance(node, ast.FunctionDef)
    and node.name == "_prebuilt_version_transcripts"
]
if len(version_functions) != 1:
    reject("receipt writer must define one prebuilt version transcript validator")
version_function = version_functions[0]
version_source = ast.get_source_segment(receipt_source, version_function) or ""
subprocess_calls = [
    node
    for node in ast.walk(receipt_tree)
    if isinstance(node, ast.Call)
    and isinstance(node.func, ast.Attribute)
    and isinstance(node.func.value, ast.Name)
    and node.func.value.id == "subprocess"
]
if len(subprocess_calls) != 1 or subprocess_calls[0].func.attr != "Popen":
    reject("receipt writer must use only its one naturally supervised subprocess")
replay_calls = [
    node
    for node in ast.walk(version_function)
    if isinstance(node, ast.Call)
    and isinstance(node.func, ast.Name)
    and node.func.id == "_run_bounded_replay"
]
cargo_branches = []
for node in ast.walk(version_function):
    if not isinstance(node, ast.If):
        continue
    if not (
        isinstance(node.test, ast.Compare)
        and isinstance(node.test.left, ast.Name)
        and node.test.left.id == "tool"
        and len(node.test.ops) == 1
        and isinstance(node.test.ops[0], ast.Eq)
        and len(node.test.comparators) == 1
        and isinstance(node.test.comparators[0], ast.Constant)
        and node.test.comparators[0].value == "cargo"
    ):
        continue
    body_replays = [
        call
        for statement in node.body
        for call in ast.walk(statement)
        if isinstance(call, ast.Call)
        and isinstance(call.func, ast.Name)
        and call.func.id == "_run_bounded_replay"
    ]
    else_replays = [
        call
        for statement in node.orelse
        for call in ast.walk(statement)
        if isinstance(call, ast.Call)
        and isinstance(call.func, ast.Name)
        and call.func.id == "_run_bounded_replay"
    ]
    if else_replays:
        cargo_branches.append((body_replays, else_replays))
if (
    len(replay_calls) != 1
    or len(cargo_branches) != 1
    or cargo_branches[0][0]
    or len(cargo_branches[0][1]) != 1
    or version_source.count(
        '("cargo", Path(corridor_fields["cargo_path"]), (), '
        'fields["cargo_version_sha256"])'
    )
    != 1
    or "subprocess.Popen" in version_source
):
    reject(
        "receipt writer may validate Cargo only from the policy-captured transcript"
    )

# The outer release probes the authenticated bootstrap alias directly before
# policy loading; the sealed corridor uses the pinned cooperative wrapper.
release_cargo_binding = exact_line(
    '  export IROHA_RELEASE_CARGO_BIN="$release_cargo_bin"'
)
release_probe = exact_line(
    '  release_cargo_version="$("$release_cargo_bin" --version)" || {'
)
corridor_probe = exact_line('  corridor_cargo_version="$(run_cargo --version)"')
if not (
    release_cargo_binding
    < release_probe
    < corridor_probe
):
    reject("authenticated Cargo binding/version probes are missing or reordered")
if source.count(
    '  corridor_cargo_path="$(canonical_path "$IROHA_RELEASE_CARGO_BIN")"'
) != 1:
    reject("corridor receipt does not bind the exact Cargo executable used")
resolved_probe_pattern = re.compile(
    r'"\$\("\$[A-Za-z_][A-Za-z0-9_]*cargo[A-Za-z0-9_]*"\s+--version\)"'
)
resolved_probes = [
    (index, line)
    for index, line in enumerate(lines)
    if resolved_probe_pattern.search(line)
]
if resolved_probes != [(release_probe, lines[release_probe])]:
    reject("only the authenticated outer Cargo alias may bypass run_cargo for --version")
variable_cargo_execution_pattern = re.compile(
    r'(?:^|[;&|]|\$\()\s*(?:command\s+)?'
    r'(?:"\$\{?([A-Za-z_][A-Za-z0-9_]*cargo[A-Za-z0-9_]*)\}?"|'
    r'\$\{?([A-Za-z_][A-Za-z0-9_]*cargo[A-Za-z0-9_]*)\}?)'
    r"\s+([A-Za-z0-9_-]+)",
    re.IGNORECASE,
)
variable_cargo_executions = []
for index, line in enumerate(lines):
    match = variable_cargo_execution_pattern.search(line)
    if match is not None:
        variable_cargo_executions.append(
            (index, match.group(1) or match.group(2), match.group(3))
        )
if variable_cargo_executions != [
    (release_probe, "release_cargo_bin", "--version")
]:
    reject("resolved Cargo execution bypasses the pinned shared wrapper")

policy_source_line = exact_line(
    'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"'
)
inventory_before = exact_line(
    'release_gate_boundary "release-inventory:before" || exit $?'
)
inventory_run = exact_line("bash ci/check_sumeragi_v2_multilane_release_inventory.sh")
inventory_after = exact_line(
    'release_gate_boundary "release-inventory:after-natural-completion" || exit $?'
)
if not policy_source_line < inventory_before < inventory_run < inventory_after:
    reject("initial inventory must be policy-sourced and cooperatively bracketed")

prebuilt_before = exact_line(
    'release_gate_boundary "release-prebuilt-publication:before" || exit $?'
)
prebuilt_ensure = exact_line(
    "ensure_source_bound_localnet_binaries || release_prebuilt_status=$?"
)
prebuilt_export = exact_line(
    "  export_source_bound_localnet_binaries || release_prebuilt_status=$?"
)
prebuilt_after = exact_line(
    'release_gate_boundary "release-prebuilt-publication:after-natural-completion" \\'
)
if not prebuilt_before < prebuilt_ensure < prebuilt_export < prebuilt_after:
    reject("main prebuild/publication is not cooperatively bracketed")

preflight_labels = (
    "source-seal",
    "seed-launcher",
    "chaos-launcher",
    "release-identity",
    "release-bootstrap",
    "release-bootstrap-validator",
    "release-receipt",
    "multilane-scaling",
    "proof-fidelity",
    "formal-launcher",
    "taira-soak",
)
for label in preflight_labels:
    before_token = f'release_gate_boundary "preflight-{label}:before"'
    after_token = (
        f'"preflight-{label}:after-natural-completion"'
        if label in {
            "release-identity",
            "release-bootstrap",
            "release-bootstrap-validator",
            "multilane-scaling",
        }
        else f'release_gate_boundary "preflight-{label}:after-natural-completion"'
    )
    if source.count(before_token) != 1 or source.count(after_token) != 1:
        reject(f"pytest preflight {label} is not bounded before/after completion")
    if source.index(before_token) >= source.index(after_token):
        reject(f"pytest preflight {label} completion boundary is reordered")

for token in (
    "run_cooperative_gate source-bound-g-scale-validation",
    "run_cooperative_gate pr-proof-ledger",
    "run_cooperative_gate pr-formal-unit",
    "run_cooperative_gate pr-formal-fast-network",
    "run_cooperative_gate pr-formal-model-replay",
):
    if source.count(token) != 1:
        reject(f"release runner lacks one cooperative gate {token!r}")
final_proof_before = exact_line(
    'release_gate_boundary "final-proof-validation:before" || exit $?'
)
final_proof_run = exact_line(
    "run_final_proof_validation || final_proof_validation_status=$?"
)
final_proof_after = exact_line(
    'release_gate_boundary "final-proof-validation:after-natural-completion" || exit $?'
)
if not final_proof_before < final_proof_run < final_proof_after:
    reject("final proof validation is not cooperatively bracketed")

source_sealed_blocks = (
    """\
  run_corridor_leg \\
    source-sealed-workspace-build command 0 \\
    "${IROHA_RELEASE_CARGO_BIN} build -j1 --locked --offline --workspace" \\
    run_cargo build --locked --offline --workspace""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-tests command 0 \\
    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline --workspace" \\
    run_cargo test --locked --offline --workspace""",
    """\
  run_corridor_leg \\
    source-sealed-irohad-tests command 0 \\
    "${IROHA_RELEASE_CARGO_BIN} test -j1 --locked --offline -p irohad --bin irohad --features test-network-message-control" \\
    run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-clippy command 0 \\
    "${IROHA_RELEASE_CARGO_BIN} clippy -j1 --locked --offline --workspace --all-targets -- -D warnings" \\
    run_cargo clippy --locked --offline --workspace --all-targets -- -D warnings""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-format command 0 \\
    "${IROHA_RELEASE_CARGO_BIN} fmt --all -- --check" \\
    run_cargo fmt --all -- --check""",
    """\
  run_corridor_leg \\
    source-sealed-legacy-codec-guard command 0 \\
    "bash scripts/check_no_legacy_codec.sh" \\
    bash scripts/check_no_legacy_codec.sh""",
)
for block in source_sealed_blocks:
    if source.count(block) != 1:
        label = block.splitlines()[1].strip().split()[0]
        reject(f"source-sealed command/evidence block {label} is missing or duplicated")
source_sealed_positions = [source.index(block) for block in source_sealed_blocks]
if source_sealed_positions != sorted(source_sealed_positions):
    reject(
        "final workspace gates are not "
        "build/test/irohad/clippy/fmt/legacy in exact order"
    )

expected_focus_counts = {
    "required_multilane_core_focus_tests": 319,
    "required_multilane_queue_journal_focus_tests": 143,
    "required_multilane_config_lib_focus_tests": 9,
    "required_multilane_config_runtime_focus_tests": 2,
    "required_multilane_config_fixtures_focus_tests": 2,
    "required_multilane_data_model_focus_tests": 8,
    "required_multilane_torii_focus_tests": 39,
    "required_multilane_torii_shared_focus_tests": 1,
    "required_multilane_integration_lib_focus_tests": 2,
}
all_focus_entries = []
for array_name, expected_count in expected_focus_counts.items():
    declaration = f"{array_name}=("
    if lines.count(declaration) != 1:
        reject(f"{array_name} must be declared exactly once")
    array_start = lines.index(declaration)
    try:
        array_end = lines.index(")", array_start + 1)
    except ValueError:
        reject(f"{array_name} is unterminated")
    entries = []
    for line in lines[array_start + 1 : array_end]:
        entry = line.strip()
        if not entry or entry.startswith("#"):
            continue
        pattern = (
            r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*"
            if array_name
            in {
                "required_multilane_config_runtime_focus_tests",
                "required_multilane_config_fixtures_focus_tests",
            }
            else r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)+"
        )
        if not re.fullmatch(pattern, entry):
            reject(f"unexpected {array_name} entry: {entry!r}")
        entries.append(entry)
    if len(entries) != expected_count or len(set(entries)) != expected_count:
        reject(
            f"{array_name} must contain {expected_count} distinct tests; "
            f"found {len(entries)} entries and {len(set(entries))} distinct entries"
        )
    all_focus_entries.extend(entries)

if len(all_focus_entries) != 525 or len(set(all_focus_entries)) != 525:
    reject(
        "multilane focus-test arrays must contain 525 globally distinct tests; "
        f"found {len(all_focus_entries)} entries and "
        f"{len(set(all_focus_entries))} distinct entries"
    )

g_unit_groups = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
        319,
        "--lib",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
        143,
        "--lib",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
        9,
        "--lib",
    ),
    (
        "required_multilane_config_runtime_focus_tests",
        "g-unit-iroha-config-runtime",
        "iroha_config",
        2,
        "--test sumeragi_v2_merge_runtime_config",
    ),
    (
        "required_multilane_config_fixtures_focus_tests",
        "g-unit-iroha-config-fixtures",
        "iroha_config",
        2,
        "--test fixtures",
    ),
    (
        "required_multilane_data_model_focus_tests",
        "g-unit-iroha-data-model",
        "iroha_data_model",
        8,
        "--lib",
    ),
    (
        "required_multilane_torii_focus_tests",
        "g-unit-iroha-torii",
        "iroha_torii",
        39,
        "--lib",
    ),
    (
        "required_multilane_torii_shared_focus_tests",
        "g-unit-iroha-torii-shared",
        "iroha_torii_shared",
        1,
        "--lib",
    ),
    (
        "required_multilane_integration_lib_focus_tests",
        "g-unit-integration-tests",
        "integration_tests",
        2,
        "--lib",
    ),
)
for array_name, leg_id, package, expected_count, cargo_target in g_unit_groups:
    command = (
        f"'for test in {array_name}; do cargo test --locked --offline "
        f'-p {package} {cargo_target} "$test" -- --exact --test-threads=1; done\''
    )
    if source.count(command) != 1:
        reject(f"G-UNIT leg {leg_id} lacks its exact crate-bound Cargo command")
    if source.count(f"    {leg_id} cargo-focus") != 1:
        reject(f"G-UNIT leg {leg_id} is missing or duplicated")
    if source.count(
        f'    g_unit_expected_test_count "$expected_multilane_focus_test_count" \\'
    ) != 1:
        reject("G-UNIT expected 525 count is not published exactly once")
    if expected_count <= 0:
        reject(f"G-UNIT leg {leg_id} has an invalid expected count")

fixture_check = """\
run_corridor_leg \\
  native-amx-rust-fixture-check command 0 \\
  "regenerate Native AMX Rust fixture authority twice into disjoint private roots and byte-authenticate both outputs" \\
  regenerate_native_amx_rust_fixtures_twice"""
if source.count(fixture_check) != 1:
    reject(
        "Rust-owned grouped fixture regeneration must be one guarded "
        "source-sealed corridor leg"
    )
fixture_helper = """\
regenerate_native_amx_rust_fixtures_twice() {
  run_cargo run --locked --offline -p iroha_data_model --features dev-tools \\
    --bin sumeragi_v2_wire_fixtures -- \\
    --out-dir "$native_amx_fixture_regeneration_first"
  run_cargo run --locked --offline -p iroha_data_model --features dev-tools \\
    --bin sumeragi_v2_wire_fixtures -- \\
    --out-dir "$native_amx_fixture_regeneration_second"
  python3 -I -S ci/resolve_sumeragi_v2_sdk_source_closure.py \\
    --root "$repo_root" \\
    --manifest ci/sumeragi_v2_sdk_source_closure.json \\
    --suite native-amx-v2-grouped \\
    --check-regeneration rust-fixtures \\
    --first-output-root "$native_amx_fixture_regeneration_first" \\
    --second-output-root "$native_amx_fixture_regeneration_second"
}"""
if source.count(fixture_helper) != 1:
    reject(
        "Rust-owned grouped fixture corridor leg must generate twice and "
        "authenticate both complete private inventories"
    )

post_preflight_identity = exact_line(
    'verify_release_identity "after release contract preflights"'
)
formal_definition = exact_line("run_release_formal_gate() {")
formal_release = exact_line("    bash scripts/run_sumeragi_v2_formal_release.sh")
formal_call = exact_line("  run_release_formal_gate")
seed_matrix = exact_line('  bash scripts/run_sumeragi_v2_seed_matrix.sh "$profile"')
if not (
    post_preflight_identity
    < formal_definition
    < formal_release
    < formal_call
    < seed_matrix
):
    reject(
        "formal release evidence must complete after source preflights and "
        "before the seed matrix"
    )
if (
    lines[formal_call - 1] != 'if [[ "$profile" == "--release" ]]; then'
    or lines[formal_call + 1] != "fi"
):
    reject("formal release evidence must execute only in the release profile")

scaling_definition = exact_line("run_release_scaling_gate() {")
scaling_validation = exact_line(
    "    scripts/nexus/validate_multilane_scaling_evidence.py \\"
)
scaling_call = exact_line("  run_release_scaling_gate")
g12_soak = exact_line(
    '  verify_release_identity "after G-12P two-hour rotating-validator fault soak"'
)
pr_branch = exact_line('if [[ "$profile" == "--pr" ]]; then')
if not scaling_definition < scaling_validation < g12_soak < scaling_call < pr_branch:
    reject("scaling release evidence must run after the completed G-12P fault soak")
if "run_release_scaling_and_formal_gates" in source:
    reject("formal and scaling release gates must remain independent")

final_proof_validation = exact_line(
    'verify_release_identity "after final proof-evidence validation"'
)
final_workspace_calls = [
    index
    for index, line in enumerate(lines)
    if line.strip() == "run_final_workspace_verification"
]
if len(final_workspace_calls) != 2:
    reject(
        "run_final_workspace_verification must execute exactly once in each "
        "PR and release corridor"
    )
publish_completion = exact_line("publish_corridor_completion")
if not final_proof_validation < final_workspace_calls[-1] < publish_completion:
    reject(
        "release workspace verification must remain after final proof validation "
        "and before corridor publication"
    )

for token in (
    'nexus_cross_completion_path_file="${IROHA_RELEASE_ARTIFACT_ROOT}/nexus-cross-dataspace-completion-path"',
    'release_gate_boundary "corridor-completion:before-publication"',
    'release_gate_boundary "corridor-completion:after-publication"',
    'release_gate_boundary "child-result:before-publication"',
    'release_gate_boundary "child-result:after-publication"',
    'verify_release_identity "after protected child-result publication"',
):
    if source.count(token) != 1:
        reject(f"release publication contract token is missing or duplicated: {token!r}")
if '${IROHA_RELEASE_HOST_ROOT:-${repo_root}/target}' in source:
    reject("PR completion-pointer authority must not fall back to repository target")

corridor_before = exact_line(
    '  release_gate_boundary "corridor-completion:before-publication" || return $?'
)
corridor_move = exact_line(
    '  mv -- "$corridor_completion_tmp" "$corridor_completion_path"'
)
corridor_after = exact_line(
    '  release_gate_boundary "corridor-completion:after-publication" \\'
)
if not corridor_before < corridor_move < corridor_after:
    reject("corridor completion publication is not bracketed by gate boundaries")

child_result_before = exact_line(
    'release_gate_boundary "child-result:before-publication" || exit $?'
)
child_result_after = exact_line(
    'release_gate_boundary "child-result:after-publication" || {'
)
if not publish_completion < child_result_before < child_result_after:
    reject("bounded child-result publication is not bracketed by gate boundaries")
outer_child_status = exact_line("  sealed_status=$?")
aggregate_writer = exact_line(
    '      "$sealed_repo_root/scripts/write_sumeragi_v2_release_receipt.py" \\'
)
protected_validator = exact_line(
    '    "$release_python_bin" -I -S "$release_bootstrap_evidence_dir/validate-receipt.py" \\'
)
result_seal = exact_line('      --seal-release-result \\')
if not outer_child_status < aggregate_writer < protected_validator < result_seal:
    reject("protected outer receipt/ack/seal validation is reordered")
if '"$sealed_repo_root/scripts/validate_sumeragi_v2_release_bootstrap.py"' in source:
    reject("candidate sealed validator must not be terminal receipt authority")
PY

python3 -I -S - \
  "$release_runner" \
  "$seed_runner" \
  "$formal_harness" \
  "$formal_release_runner" \
  "$formal_gate" \
  "$verus_runner" \
  "$replay_runner" \
  "$chaos_runner" \
  "$taira_runner" \
  "$launcher" \
  "$release_receipt_writer" \
  "$cargo_cache_copier" \
  "$prebuilt_bundle_shell" \
  "$prebuilt_bundle_helper" \
  "$process_policy" \
  "$marker_publisher" \
  "$pr_workflow" \
  "$nexus_cross_dataspace_pr_helper" \
  "$nexus_cross_lane_pr_helper" \
  "$nexus_pr_helper_test" <<'PY'
from __future__ import annotations

from pathlib import Path
import re
import sys


paths = [Path(raw) for raw in sys.argv[1:]]
sources = {path.as_posix(): path.read_text(encoding="utf-8") for path in paths}


def reject(message: str) -> None:
    raise SystemExit(f"transitive Cargo release inventory: {message}")


# This is the complete Cargo-bearing/delegating child closure reachable from
# the release runner. Every edge is source-explicit so adding a new child
# requires reviewing its Cargo policy here.
expected_edges = (
    (
        ".github/workflows/pr.yml",
        "ci/check_nexus_cross_dataspace_localnet.sh",
        "run: bash ci/check_nexus_cross_dataspace_localnet.sh",
    ),
    (
        ".github/workflows/pr.yml",
        "ci/check_nexus_cross_lane_proofs.sh",
        "run: bash ci/check_nexus_cross_lane_proofs.sh",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/copy_sumeragi_v2_release_cargo_cache.py",
        'pr_clone_helper="$pr_source_root/scripts/copy_sumeragi_v2_release_cargo_cache.py"',
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/run_sumeragi_v2_seed_matrix.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/run_sumeragi_v2_100k_chaos.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "ci/check_nexus_cross_dataspace_localnet.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "ci/check_nexus_cross_lane_proofs.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/run_taira_v2_24h_soak.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "ci/check_nexus_cross_dataspace_localnet.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
    ),
    (
        "ci/check_nexus_cross_lane_proofs.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
    ),
    (
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/run_sumeragi_v2_formal_release.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "ci/check_sumeragi_formal.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/verify_sumeragi_v2.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/formal/run_sumeragi_v2_harness.sh",
        "scripts/sumeragi_v2_release_process_policy.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${repo_root}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
    ),
    (
        "ci/check_nexus_cross_dataspace_localnet.sh",
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "bash scripts/run_nexus_cross_dataspace_atomic_swap.sh",
    ),
    (
        "scripts/run_sumeragi_v2_seed_matrix.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${repo_root}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
    ),
    (
        "scripts/run_taira_v2_24h_soak.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
    ),
    (
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.py",
        'local prebuilt_helper="${prebuilt_repo_root}/scripts/sumeragi_v2_prebuilt_bundle.py"',
    ),
    (
        "scripts/run_sumeragi_v2_seed_matrix.sh",
        "scripts/publish_release_marker.py",
        '"${repo_root}/scripts/publish_release_marker.py"',
    ),
    (
        "scripts/run_sumeragi_v2_100k_chaos.sh",
        "scripts/publish_release_marker.py",
        '"${repo_root}/scripts/publish_release_marker.py"',
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/run_sumeragi_v2_seed_matrix.sh",
        'bash scripts/run_sumeragi_v2_seed_matrix.sh "$profile"',
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
        "bash scripts/run_nexus_cross_dataspace_atomic_swap.sh",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/run_sumeragi_v2_formal_release.sh",
        "bash scripts/run_sumeragi_v2_formal_release.sh",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/formal/run_sumeragi_v2_harness.sh",
        "bash scripts/formal/run_sumeragi_v2_harness.sh --unit",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/run_sumeragi_v2_100k_chaos.sh",
        "bash scripts/run_sumeragi_v2_100k_chaos.sh",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/run_taira_v2_24h_soak.sh",
        "bash scripts/run_taira_v2_24h_soak.sh",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/write_sumeragi_v2_release_receipt.py",
        '"$sealed_repo_root/scripts/write_sumeragi_v2_release_receipt.py"',
    ),
    (
        "scripts/run_sumeragi_v2_formal_release.sh",
        "ci/check_sumeragi_formal.sh",
        "bash ci/check_sumeragi_formal.sh",
    ),
    (
        "ci/check_sumeragi_formal.sh",
        "scripts/verify_sumeragi_v2.sh",
        "run_formal_script scripts/verify_sumeragi_v2.sh",
    ),
    (
        "ci/check_sumeragi_formal.sh",
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
        "run_formal_script scripts/formal/check_sumeragi_v2_replay_trace.sh",
    ),
    (
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
        "scripts/formal/run_sumeragi_v2_harness.sh",
        'bash "$REPO_ROOT/scripts/formal/run_sumeragi_v2_harness.sh" --model-replay',
    ),
    (
        "scripts/verify_sumeragi_v2.sh",
        "scripts/formal/run_sumeragi_v2_harness.sh",
        'bash "$REPO_ROOT/scripts/formal/run_sumeragi_v2_harness.sh" --unit',
    ),
    (
        "scripts/run_sumeragi_v2_100k_chaos.sh",
        "scripts/formal/run_sumeragi_v2_harness.sh",
        "bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k",
    ),
)
for parent, child, token in expected_edges:
    if parent not in sources or child not in sources:
        reject(f"reachable script closure omits {parent} -> {child}")
    if token not in sources[parent]:
        reject(f"reachable child edge is missing: {parent} -> {child}")

guarded_cargo_scripts = (
    "scripts/run_sumeragi_v2_release_gates.sh",
    "scripts/run_sumeragi_v2_seed_matrix.sh",
    "scripts/formal/run_sumeragi_v2_harness.sh",
    "scripts/run_taira_v2_24h_soak.sh",
    "scripts/run_nexus_cross_dataspace_atomic_swap.sh",
    "ci/check_nexus_cross_dataspace_localnet.sh",
    "ci/check_nexus_cross_lane_proofs.sh",
)
policy = sources["scripts/sumeragi_v2_release_process_policy.sh"]
if policy.count("acquire_invocation_cargo_lock() {") != 1:
    reject("shared process policy lacks its one invocation-local Cargo lock")
if policy.count("release_invocation_cargo_lock() {") != 1:
    reject("shared process policy lacks its one Cargo lock release")
if policy.count("run_cargo() {") != 1:
    reject("shared process policy lacks its one pinned Cargo wrapper")
for token, count in (
    ('lock_path="${artifact_root}/.sumeragi-v2-cargo.lock"', 2),
    ("lock.mkdir(mode=0o700)", 1),
    ("lock.rmdir()", 1),
    ("os.rmdir(lock.name, dir_fd=root_fd)", 1),
    ("acquire_invocation_cargo_lock || return $?", 1),
    ("release_invocation_cargo_lock || return $?", 1),
    ("trap _release_scoped_invocation_cargo_lock RETURN EXIT", 1),
):
    if policy.count(token) != count:
        reject(
            "shared process policy lacks exact invocation-local lock contract "
            f"{token!r} x{count}"
        )
for forbidden in (
    "wait_for_external_cargo",
    "ps -",
    "pgrep",
    "/proc/",
    "process_snapshot",
    "sleep ",
):
    if forbidden in policy:
        reject(
            "shared process policy observes or polls ambient processes via "
            f"{forbidden!r}"
        )
if policy.count('if "$IROHA_RELEASE_CARGO_BIN" "$@"; then') != 1:
    reject("shared process policy does not own the sole pinned Cargo execution")
if 'pinned_arguments=("$subcommand" -j1)' not in policy or 'pinned_arguments+=("$@")' not in policy:
    reject("shared process policy does not impose the one global -j1 bound")
if "local status" in policy:
    reject("shared process policy retains the zsh-reserved local status name")
for token in (
    'local RUSTUP_AUTO_INSTALL=0',
    'export RUSTUP_AUTO_INSTALL',
    'run_cargo forbids caller-owned rustup auto-install policy',
):
    if policy.count(token) != 1:
        reject(f"shared process policy lacks exact offline rustup token {token!r}")
if policy.count("require_release_artifact_path() {") != 1:
    reject("shared process policy lacks its pre-creation artifact containment guard")
if policy.count("require_disjoint_release_roots() {") != 1:
    reject("shared process policy lacks its one disjoint target/artifact guard")
if policy.count("build|test|run|clippy|verus)") != 1:
    reject("shared process policy accepted-subcommand inventory is not exact")
if "build|test|run|clippy|verus|fetch)" in policy:
    reject("shared process policy still accepts network-fetch Cargo execution")
predicate_start = policy.index("require_disjoint_release_roots() {")
predicate_end = policy.index("\n}\n", predicate_start) + 3
predicate = policy[predicate_start:predicate_end]
for token in (
    'cancel_parent="${cancel_path%/*}"',
    '"$source_root" "$cancel_parent" "release cancellation parent"',
    "release source/output/cancellation paths must be absolute",
    "os.path.abspath(cancel_path) != cancel_path",
    "os.path.realpath(cancel_path) != cancel_path",
    "Cargo target and release artifact roots must be disjoint",
    "release cancellation marker must be outside source, Cargo target, and",
):
    if predicate.count(token) != 1:
        reject(f"shared root predicate lacks exact cancellation/root contract {token!r}")
if re.search(r"\b(?:rm|unlink)\b[^\n]*cancel", predicate):
    reject("shared root predicate must never delete an operator marker")
for script in guarded_cargo_scripts:
    source = sources[script]
    if any(
        definition in source
        for definition in (
            "acquire_invocation_cargo_lock() {",
            "release_invocation_cargo_lock() {",
            "run_cargo() {",
        )
    ):
        reject(f"{script} shadows the shared process policy")
    if "sumeragi_v2_release_process_policy.sh" not in source:
        reject(f"{script} does not source the shared process policy")
    if "CARGO_TARGET_DIR" not in source:
        reject(f"{script} does not bind Cargo to an isolated target")
    if "run_cargo fetch" in source:
        reject(f"{script} retains forbidden Cargo fetch execution")

    lines = source.splitlines()
    direct = [
        (index + 1, line)
        for index, line in enumerate(lines)
        if re.search(r"^\s*(?:command\s+)?cargo(?:\s|$)", line)
    ]
    if direct:
        reject(f"{script} contains unguarded direct Cargo execution: {direct!r}")

    for index, line in enumerate(lines):
        match = re.search(r"\brun_cargo\s+(build|test|run|clippy|fmt)\b", line)
        if match is None:
            continue
        logical = line
        cursor = index
        while logical.rstrip().endswith("\\") and cursor + 1 < len(lines):
            cursor += 1
            logical += " " + lines[cursor]
        if match.group(1) == "fmt":
            if script != "scripts/run_sumeragi_v2_release_gates.sh":
                reject(f"{script}:{index + 1} has an unreviewed Cargo fmt command")
            continue
        if "--locked" not in logical or "--offline" not in logical:
            reject(
                f"{script}:{index + 1} Cargo command lacks --locked --offline"
            )

entry_root_contracts = (
    (
        "scripts/run_taira_v2_24h_soak.sh",
        'require_disjoint_release_roots "$REPO_ROOT"',
        'release_gate_boundary "taira:entry"',
    ),
    (
        "scripts/run_sumeragi_v2_formal_release.sh",
        'require_disjoint_release_roots "$repo_root"',
        'release_gate_boundary "formal-release:entry"',
    ),
    (
        "ci/check_sumeragi_formal.sh",
        'require_disjoint_release_roots "$repo_root"',
        'release_gate_boundary "formal:entry"',
    ),
    (
        "scripts/verify_sumeragi_v2.sh",
        'require_disjoint_release_roots "$REPO_ROOT"',
        'release_gate_boundary "verus:entry"',
    ),
    (
        "scripts/formal/run_sumeragi_v2_harness.sh",
        'require_disjoint_release_roots "$REPO_ROOT"',
        'release_gate_boundary "formal-harness:entry"',
    ),
)
for script, root_token, entry_token in entry_root_contracts:
    entry_source = sources[script]
    if entry_source.count(root_token) != 1 or entry_source.count(entry_token) != 1:
        reject(f"{script} lacks one exact root-validated entry")
    if entry_source.index(root_token) >= entry_source.index(entry_token):
        reject(f"{script} validates roots after its entry boundary")

harness = sources["scripts/formal/run_sumeragi_v2_harness.sh"]
for forbidden_fetch in ("--fetch", "CARGO_NET_OFFLINE=false", "run_cargo fetch"):
    if forbidden_fetch in harness:
        reject(f"formal harness retains forbidden network-fetch mode: {forbidden_fetch}")
for forbidden in ('"${@:2}"', "bash -c", "sh -c", "env cargo"):
    if forbidden in harness:
        reject(f"formal harness retains arbitrary child-command dispatch: {forbidden}")
expected_verus_branch = """\
  --verus)
    if (($# != 1)); then
      echo "--verus accepts no additional arguments" >&2
      exit 2
    fi
    run_cargo verus verify --locked --offline -p iroha_sumeragi_core --features verus \\
      --fwd-verus-args-to roots -- \\
      --rlimit 60 \\
      --expand-errors \\
      --no-cheating
    ;;"""
expected_clippy_branch = """\
  --clippy)
    if (($# != 1)); then
      echo "--clippy accepts no additional arguments" >&2
      exit 2
    fi
    run_cargo clippy --locked --offline -p iroha_sumeragi_core --lib -- -D warnings
    ;;"""
if harness.count(expected_verus_branch) != 1 or harness.count(expected_clippy_branch) != 1:
    reject("formal harness must expose only the reviewed fixed Verus and Clippy Cargo modes")
if (
    harness.count(
        'echo "positional harness commands are unsupported; select one fixed mode" >&2'
    )
    != 1
):
    reject("formal harness positional-command fallback must fail closed")

verify = sources["scripts/verify_sumeragi_v2.sh"]
delegated_verus = """\
bash scripts/formal/run_sumeragi_v2_harness.sh --verus \\"""
if verify.count(delegated_verus) != 1:
    reject("Verus execution must use the one fixed guarded-harness mode")
for token in (
    'require_external_cargo_target_dir "$REPO_ROOT"',
    'require_external_release_artifact_root "$REPO_ROOT"',
    'require_release_artifact_directory "$FORMAL_EVIDENCE_DIR"',
):
    if verify.count(token) != 1:
        reject("Verus runner must inherit the authenticated external target and evidence roots")

for script, source in sources.items():
    if script.startswith("pytests/"):
        continue
    if "IROHA_TEST_ALLOW_REENTRANT_BUILD=1" in source:
        reject(f"{script} re-enables nested Cargo builds")

for script in (
    "scripts/run_sumeragi_v2_release_gates.sh",
    "scripts/run_sumeragi_v2_seed_matrix.sh",
    "scripts/run_taira_v2_24h_soak.sh",
):
    source = sources[script]
    for token in (
        "export IROHA_TEST_SKIP_BUILD=1",
        "export IROHA_TEST_ALLOW_REENTRANT_BUILD=0",
        "ensure_source_bound_localnet_binaries",
        "IROHA_RELEASE_PREBUILT_MANIFEST_SHA256",
        "export_source_bound_localnet_binaries",
    ):
        if token not in source:
            reject(f"{script} lacks source-bound prebuild contract token {token!r}")
    publication_block = (
        "ensure_source_bound_localnet_binaries || release_prebuilt_status=$?\n"
        "if ((release_prebuilt_status == 0)); then\n"
        "  export_source_bound_localnet_binaries || release_prebuilt_status=$?"
        if script == "scripts/run_sumeragi_v2_release_gates.sh"
        else "ensure_source_bound_localnet_binaries\n"
        "export_source_bound_localnet_binaries"
    )
    if source.count(publication_block) != 1:
        reject(
            f"{script} must publish localnet executable overrides immediately "
            "after its source-bound prebuild"
        )

prebuilt_shell = sources["scripts/sumeragi_v2_prebuilt_bundle.sh"]
prebuilt_root_guard = prebuilt_shell.index(
    'require_disjoint_release_roots "$prebuilt_repo_root"'
)
prebuilt_first_cargo = prebuilt_shell.index("run_cargo ")
if prebuilt_shell.count('require_disjoint_release_roots "$prebuilt_repo_root"') != 1:
    reject("shared prebuild helper must validate the complete root triplet once")
if prebuilt_root_guard >= prebuilt_first_cargo:
    reject("shared prebuild helper validates roots after Cargo execution")
publish_block = '''\
  export TEST_NETWORK_BIN_IROHAD="${IROHA_TEST_TARGET_DIR}/release/iroha3d"
  export TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="${IROHA_TEST_TARGET_DIR}/message-control/release/iroha3d"
  export TEST_NETWORK_BIN_IROHA="${IROHA_TEST_TARGET_DIR}/release/iroha"
  export KAGAMI_BIN="${IROHA_TEST_TARGET_DIR}/release/kagami"'''
if prebuilt_shell.count(publish_block) != 1:
    reject("shared prebuild helper must publish the four exact manifest paths once")
for command in (
    "run_cargo build --locked --offline --release -p irohad --bin iroha3d",
    "run_cargo build --locked --offline --release -p iroha_cli --bin iroha",
    "run_cargo build --locked --offline --release -p iroha_kagami --bin kagami",
):
    if prebuilt_shell.splitlines().count(f"      {command} || exit $?") != 1:
        reject(f"shared prebuild helper must execute exactly one {command!r}")
if prebuilt_shell.splitlines().count(
    "        --features test-network-message-control || exit $?"
) != 1:
    reject("shared prebuild helper must build exactly one message-control irohad")
for token in (
    "unset IROHA_TEST_TARGET_DIR",
    "run_cargo --version",
    "command rustc -vV",
    '--manifest-sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"',
):
    if prebuilt_shell.count(token) != 1:
        reject(f"shared prebuild helper lacks exact fail-closed token {token!r}")
for token, expected_count in (
    ('--cargo-target-dir "$CARGO_TARGET_DIR"', 3),
    ('--artifact-root "$IROHA_RELEASE_ARTIFACT_ROOT"', 2),
    (
        'local prebuilt_build_root="${CARGO_TARGET_DIR}/sumeragi-v2-release/${prebuilt_source_manifest_sha256}/program-build-cache"',
        1,
    ),
    (
        'local prebuilt_programs_root="${IROHA_RELEASE_ARTIFACT_ROOT}/sumeragi-v2-release/${prebuilt_source_manifest_sha256}/programs"',
        1,
    ),
):
    if prebuilt_shell.count(token) != expected_count:
        reject(
            "shared prebuild helper does not bind authenticated target/artifact "
            f"roots exactly: {token!r}"
        )
for forbidden in (
    '${prebuilt_repo_root}/target',
    'readlink "${prebuilt_repo_root}/target"',
):
    if forbidden in prebuilt_shell:
        reject(f"shared prebuild helper retains repository target authority: {forbidden!r}")

prebuilt_python = sources["scripts/sumeragi_v2_prebuilt_bundle.py"]
for token in (
    '_SCHEMA_VERSION = "2"',
    "_BINARY_MODE = 0o500",
    "_MANIFEST_MODE = 0o400",
    "_DIRECTORY_MODE = 0o500",
    '"release/iroha3d"',
    '"message-control/release/iroha3d"',
    '"release/iroha"',
    '"release/kagami"',
    '"cargo_version_sha256"',
    '"rustc_version_sha256"',
    '"bundle_dir"',
    "_validate_exact_bundle_tree(bundle)",
    "def _external_roots(",
    'cargo_target_dir\n        / "sumeragi-v2-release"',
    'artifact_root / "sumeragi-v2-release" / source_manifest_sha256 / "programs"',
    '"Cargo target and release artifact roots must be disjoint"',
):
    if token not in prebuilt_python:
        reject(f"prebuilt bundle v2 helper lacks contract token {token!r}")
for forbidden in ("_workspace_target", 'repo_root / "target"'):
    if forbidden in prebuilt_python:
        reject(f"prebuilt bundle helper retains repository target authority: {forbidden!r}")

release = sources["scripts/run_sumeragi_v2_release_gates.sh"]
main_entry = release.index('release_gate_boundary "release-runner:entry"')
main_root_guards = [
    match.start()
    for match in re.finditer(
        r'require_disjoint_release_roots "\$repo_root"', release
    )
]
if len(main_root_guards) != 3 or main_root_guards[-1] >= main_entry:
    reject("every main-runner path must validate roots before its entry boundary")
if "--no-skip-build" in release:
    reject("release runner may not request reentrant localnet builds")
if "--env IROHA_TEST_ALLOW_REENTRANT_BUILD=" in release:
    reject("release callers must not override launcher-owned reentrant policy")

launcher_source = sources["scripts/run_nexus_cross_dataspace_atomic_swap.sh"]
if 'CARGO_TEST_CMD+=("--locked" "--offline")' not in launcher_source:
    reject("Nexus child launcher Cargo commands are not locked/offline")
if 'ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")' not in launcher_source:
    reject("Nexus child launcher does not propagate skip-build")
for token in (
    'ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")',
    'ENV_VARS+=("IROHA_TEST_ALLOW_REENTRANT_BUILD=0")',
    "IROHA_TEST_SKIP_BUILD|IROHA_TEST_ALLOW_REENTRANT_BUILD|",
):
    if launcher_source.count(token) != 1:
        reject(f"Nexus child launcher lacks exact nested-build policy {token!r}")
for forbidden in ("--no-skip-build", "SKIP_BUILD=true", "SKIP_BUILD=false"):
    if forbidden in launcher_source:
        reject(f"Nexus child launcher retains obsolete build mode {forbidden!r}")
extras_end = launcher_source.index("done\n# Test processes must consume")
for token in (
    'ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")',
    'ENV_VARS+=("IROHA_TEST_ALLOW_REENTRANT_BUILD=0")',
):
    if launcher_source.index(token) <= extras_end:
        reject("Nexus child launcher must pin nested-build policy after extras")

seed = sources["scripts/run_sumeragi_v2_seed_matrix.sh"]
replay_lines = [line for line in seed.splitlines() if line.strip().startswith("command=")]
if len(replay_lines) != 1:
    reject("seed runner must publish one canonical replay-command template")
replay = replay_lines[0]
for token in (
    "CARGO_TARGET_DIR=",
    "IROHA_TEST_TARGET_DIR=",
    "TEST_NETWORK_BIN_IROHAD=",
    "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL=",
    "TEST_NETWORK_BIN_IROHA=",
    "KAGAMI_BIN=",
    "CARGO_NET_OFFLINE=true",
    "IROHA_TEST_SKIP_BUILD=1",
    "IROHA_TEST_ALLOW_REENTRANT_BUILD=0",
    "IROHA_TEST_BUILD_PROFILE=release",
    "PROFILE=release",
    "cargo test --locked --offline",
):
    if token not in replay:
        reject(f"seed replay command lacks {token!r}")

chaos = sources["scripts/run_sumeragi_v2_100k_chaos.sh"]
triplet_error = (
    "CARGO_TARGET_DIR, IROHA_RELEASE_ARTIFACT_ROOT, and "
    "IROHA_RELEASE_CANCEL_REQUEST_PATH must be supplied all-or-none"
)
for runner_name, runner_source, evidence_token in (
    (
        "seed",
        seed,
        'require_release_artifact_path "$evidence_root"',
    ),
    (
        "chaos-100k",
        chaos,
        'require_release_artifact_path "$evidence_root"',
    ),
):
    for token in (
        'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
        triplet_error,
        evidence_token,
        'require_release_artifact_directory "$evidence_root"',
        "require_disjoint_release_roots",
        "--no-writable-paths",
    ):
        if runner_source.count(token) != 1:
            reject(
                f"{runner_name} runner lacks one exact external-root token {token!r}"
            )
    if "--writable target" in runner_source:
        reject(f"{runner_name} runner permits writable repository source")

for token in (
    'release_gate_boundary "seed-matrix:entry"',
    'release_gate_boundary "seed-matrix:prebuilt-publication:before"',
    'release_gate_boundary "seed-matrix:prebuilt-publication:after"',
    'release_gate_boundary "seed-matrix:inventory-harness:before"',
    'release_gate_boundary "seed-matrix:inventory-harness:after"',
    'release_gate_boundary "seed-matrix:test-harness-${run_index}:before"',
    'release_gate_boundary "seed-matrix:test-harness-${run_index}:after"',
    'release_gate_boundary "seed-matrix:completion-publication:before"',
    'release_gate_boundary "seed-matrix:completion-publication:after"',
    'require_release_artifact_path "$completion_pointer_parent"',
):
    if seed.count(token) != 1:
        reject(f"seed runner lacks one exact boundary/containment token {token!r}")

for token in (
    'release_gate_boundary "chaos-100k:entry"',
    'release_gate_boundary "chaos-100k:harness:before"',
    'release_gate_boundary "chaos-100k:harness:after"',
    'release_gate_boundary "chaos-100k:completion-publication:before"',
    'release_gate_boundary "chaos-100k:completion-publication:after"',
    'require_release_artifact_path "$completion_pointer_parent"',
):
    if chaos.count(token) != 1:
        reject(f"chaos runner lacks one exact boundary/containment token {token!r}")
if '${repo_root}/target' in chaos:
    reject("chaos runner retains repository target evidence authority")

pr_workflow = sources[".github/workflows/pr.yml"]
localnet_pr = sources["ci/check_nexus_cross_dataspace_localnet.sh"]
lane_pr = sources["ci/check_nexus_cross_lane_proofs.sh"]
nexus_pr_tests = sources["pytests/scripts/nexus_pr_helpers_test.py"]
for helper_name, helper_source in (
    ("cross-dataspace", localnet_pr),
    ("cross-lane", lane_pr),
):
    for token in (
        'source "${REPO_ROOT}/scripts/sumeragi_v2_release_process_policy.sh"',
        'source "${REPO_ROOT}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
        triplet_error,
        'require_external_cargo_target_dir "$REPO_ROOT"',
        'require_external_release_artifact_root "$REPO_ROOT"',
        "require_disjoint_release_roots",
        "export IROHA_TEST_SKIP_BUILD=1",
        "export IROHA_TEST_ALLOW_REENTRANT_BUILD=0",
        "sumeragi_v2_ensure_source_bound_localnet_binaries",
        "sumeragi_v2_export_source_bound_localnet_binaries",
    ):
        if helper_source.count(token) != 1:
            reject(
                f"Nexus {helper_name} PR helper lacks one exact policy token {token!r}"
            )
    for forbidden in (
        "--no-skip-build",
        "IROHA_TEST_ALLOW_REENTRANT_BUILD=1",
        "IROHA_TEST_SKIP_BUILD=0",
    ):
        if forbidden in helper_source:
            reject(
                f"Nexus {helper_name} PR helper retains obsolete build mode {forbidden!r}"
            )

if localnet_pr.count(
    "bash scripts/run_nexus_cross_dataspace_atomic_swap.sh"
) != 1:
    reject("Nexus cross-dataspace PR helper must delegate once to the guarded launcher")
if localnet_pr.count('require_release_artifact_path "$evidence_dir"') != 1:
    reject("Nexus cross-dataspace PR evidence is not pre-contained")
if lane_pr.count("run_cargo test --locked --offline") != 4:
    reject("Nexus cross-lane PR helper must expose four exact guarded Cargo filters")

def workflow_job(name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        pr_workflow,
    )
    if match is None:
        reject(f"PR workflow lacks Nexus job {name}")
    return match.group("body")


for job_name, command in (
    (
        "nexus_cross_dataspace_localnet",
        "run: bash ci/check_nexus_cross_dataspace_localnet.sh",
    ),
    (
        "nexus_cross_lane_proofs",
        "run: bash ci/check_nexus_cross_lane_proofs.sh",
    ),
):
    body = workflow_job(job_name)
    if body.count(command) != 1:
        reject(f"PR Nexus job {job_name} lacks its one reviewed helper command")
    if "timeout-minutes:" in body:
        reject(f"PR Nexus job {job_name} retains an outer timeout")

for test_name in (
    "test_nexus_pr_helpers_and_jobs_are_pinned_to_shared_policy",
    "test_cross_dataspace_helper_reuses_attested_prebuilt_bundle",
    "test_cross_lane_helper_routes_all_filters_through_pinned_cargo",
):
    if nexus_pr_tests.count(f"def {test_name}(") != 1:
        reject(f"Nexus PR helper focused contract is missing: {test_name}")

publisher = sources["scripts/publish_release_marker.py"]
for token in (
    "os.O_EXCL",
    '"O_NOFOLLOW"',
    "os.fsync(parent.descriptor)",
    "def publish_release_marker(",
):
    if token not in publisher:
        reject(f"release marker publisher lacks fail-closed token {token!r}")

receipt_writer = sources["scripts/write_sumeragi_v2_release_receipt.py"]
for token in (
    'f"CARGO_TARGET_DIR={cargo_target_dir} "',
    'f"IROHA_TEST_TARGET_DIR={program_target_dir} "',
    'f"TEST_NETWORK_BIN_IROHAD={irohad} "',
    'f"TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL={message_control_irohad} "',
    'f"TEST_NETWORK_BIN_IROHA={iroha} "',
    'f"KAGAMI_BIN={kagami} "',
    '"CARGO_NET_OFFLINE=true "',
    '"IROHA_TEST_SKIP_BUILD=1 "',
    '"IROHA_TEST_ALLOW_REENTRANT_BUILD=0 "',
    '"cargo test --locked --offline -p integration_tests --test "',
):
    if token not in receipt_writer:
        reject(f"release receipt writer lacks exact seed replay token {token!r}")
for obsolete in (
    '"IROHA_TEST_SKIP_BUILD=0 "',
    '"IROHA_TEST_ALLOW_REENTRANT_BUILD=1 "',
    '"cargo test --locked -p integration_tests --test "',
):
    if obsolete in receipt_writer:
        reject(f"release receipt writer accepts obsolete seed replay token {obsolete!r}")
PY

python3 -I -S - "$kura_source" <<'PY'
from pathlib import Path
import sys


kura = Path(sys.argv[1])
source = kura.read_text(encoding="utf-8")


def reject(message: str) -> None:
    raise SystemExit(f"{kura}: Native standalone evidence inventory: {message}")


required_layout = (
    'const NATIVE_AMX_APPLICATION_MANIFEST_FILE_PREFIX: &str = "native_amx_manifest_v1_";',
    'const NATIVE_AMX_PARTICIPANT_RECEIPT_FILE_PREFIX: &str = "native_amx_receipt_v1_";',
    'const NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE: &str = "native_amx_evidence_prune_intent_v2.norito";',
    '    "native_amx_evidence_prune_intent_v2.norito.tmp";',
    '    "native_amx_participant_receipts.latest_v2.norito";',
    '    "native_amx_participant_receipts.latest_v2.norito.tmp";',
)
for declaration in required_layout:
    if source.count(declaration) != 1:
        reject(f"required current filename declaration is missing or duplicated: {declaration!r}")

obsolete_dense_names = (
    "native_amx_evidence_prune_intent_v1.norito",
    "native_amx_evidence_prune_intent_v1.norito.tmp",
    "native_amx_participant_receipts.latest_v1.norito",
    "native_amx_participant_receipts.latest_v1.norito.tmp",
    "native_amx_participant_receipts.norito",
    "native_amx_participant_receipts.index",
    "native_amx_application_manifests.norito",
    "native_amx_application_manifests.index",
)
for obsolete in obsolete_dense_names:
    if obsolete in source:
        reject(f"obsolete or legacy Native evidence filename remains reachable: {obsolete}")
PY

python3 -I -S - "$test_network_source" <<'PY'
from pathlib import Path
import sys


test_network = Path(sys.argv[1])
source = test_network.read_text(encoding="utf-8")


def reject(message: str) -> None:
    raise SystemExit(f"{test_network}: release program inventory: {message}")


required_contract = (
    'const IROHA_TEST_SKIP_BUILD_ENV: &str = "IROHA_TEST_SKIP_BUILD";',
    'const IROHA_TEST_ALLOW_REENTRANT_BUILD_ENV: &str = "IROHA_TEST_ALLOW_REENTRANT_BUILD";',
    'const IROHA_RELEASE_SOURCE_MANIFEST_SHA256_ENV: &str = "IROHA_RELEASE_SOURCE_MANIFEST_SHA256";',
    'const SUMERAGI_V2_RELEASE_TARGET_SUBDIR: &str = "sumeragi-v2-release";',
    'const SUMERAGI_V2_RELEASE_PROGRAMS_SUBDIR: &str = "programs";',
    "fn release_program_contract(repo: &Path)",
    "fn validate_release_program_candidate(",
    "if release_corridor {\n        return false;\n    }",
    '.arg("--locked")\n                .arg("--offline")',
    'std::process::Command::new("ps")\n        .args(["-axo", "pid,etime,command"])',
    "ensure_child_cargo_quiescent(&cargo_program)?;",
    "release_contract.is_none() && isolated_target_subdir.is_none()",
    "release binary resolution cannot override {IROHA_TEST_SKIP_BUILD_ENV}=1",
)
for token in required_contract:
    if token not in source:
        reject(f"source-manifest/reentrant-build contract token is missing: {token!r}")

if source.count("let release_contract = release_program_contract(&repo)?;") != 1:
    reject("program resolution must activate the release contract exactly once")
resolve_start = source.index("fn resolve_internal(")
resolve_end = source.index("\n    pub fn resolve(", resolve_start)
resolve_body = source[resolve_start:resolve_end]
if resolve_body.count("validate_release_program_candidate(") != 3:
    reject("not every explicit, cached, and fallback release candidate is contained")
PY

python3 -I -S - "$launcher" <<'PY'
from pathlib import Path
import re
import sys

launcher = Path(sys.argv[1])
source = launcher.read_text(encoding="utf-8")
lines = source.splitlines()
execution = re.compile(r'^\s*run_cargo "\$\{(?:CMD|LIST_CMD)\[@\]\}"')
execution_lines = [index for index, line in enumerate(lines) if execution.search(line)]
if len(execution_lines) != 10:
    raise SystemExit(
        f"{launcher}: expected 10 Cargo execution sites; "
        f"found {len(execution_lines)} guarded sites"
    )
for token in (
    'source "${repo_root}/scripts/sumeragi_v2_release_process_policy.sh"',
    'require_external_cargo_target_dir "$repo_root"',
    'require_external_release_artifact_root "$repo_root"',
    "require_disjoint_release_roots",
    'require_release_artifact_directory "$EVIDENCE_DIR"',
    "CARGO_TEST_CMD=(test)",
):
    if source.count(token) != 1:
        raise SystemExit(f"{launcher}: missing exact pinned launcher token {token!r}")
for forbidden in (
    "wait_for_cargo_idle",
    "scripts/cargo_fast.sh",
    "cargo_runner=(cargo)",
):
    if forbidden in source:
        raise SystemExit(f"{launcher}: forbidden Cargo bypass remains: {forbidden}")
PY

for grouped_surface in openapi python javascript swift kotlin java; do
  grouped_surface_occurrences=2
  [[ "$grouped_surface" == openapi ]] && grouped_surface_occurrences=1
  if [[ "$(grep -Ec -- "^    ${grouped_surface}$" "$release_runner" || true)" != "$grouped_surface_occurrences" ]]; then
    echo "production release runner has an invalid grouped/diagnostics ${grouped_surface} surface inventory" >&2
    exit 1
  fi
  if ! grep -Fq -- "${grouped_surface})" "$grouped_parity_harness"; then
    echo "grouped Native AMX V2 parity harness lacks ${grouped_surface} execution" >&2
    exit 1
  fi
done

for sdk_diagnostics_surface in python javascript swift kotlin java; do
  if [[ "$(grep -Ec -- "^    ${sdk_diagnostics_surface}$" "$release_runner" || true)" != 2 ]]; then
    echo "production release runner must inventory grouped and diagnostics ${sdk_diagnostics_surface} SDK surfaces exactly once each" >&2
    exit 1
  fi
  if ! grep -Fq -- "${sdk_diagnostics_surface})" "$sdk_diagnostics_harness"; then
    echo "Sumeragi v2 SDK diagnostics harness lacks ${sdk_diagnostics_surface} execution" >&2
    exit 1
  fi
done

if [[ ! -f "$grouped_fixture" || -L "$grouped_fixture" ]]; then
  echo "Rust-owned grouped Native AMX V2 corpus must be a regular non-symlink fixture" >&2
  exit 1
fi
grouped_fixture_sha256="$(bash "$grouped_parity_harness" --fixture-sha256)"
grouped_suite_source_manifest_sha256="$(
  bash "$grouped_parity_harness" --suite-source-manifest-sha256
)"
receipt_suite_source_manifest_sha256="$(
  python3 -I -S - "$repo_root" <<'PY'
from pathlib import Path
import runpy
import sys

root = Path(sys.argv[1]).resolve(strict=True)
sys.path.insert(0, str(root))
symbols = runpy.run_path(str(root / "scripts/write_sumeragi_v2_release_receipt.py"))
print(
    symbols["_sdk_suite_source_manifest"](
        root, symbols["_NATIVE_AMX_GROUPED_SOURCE_CLOSURE_SUITE"]
    )
)
PY
)"
if [[ ! "$grouped_fixture_sha256" =~ ^[0-9a-f]{64}$ \
  || ! "$grouped_suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
  || "$receipt_suite_source_manifest_sha256" \
    != "$grouped_suite_source_manifest_sha256" ]]; then
  echo "grouped Native AMX V2 fixture/suite source binding is invalid" >&2
  exit 1
fi
require_exact_digest_occurrences \
  "$closure_ledger" \
  "$grouped_fixture_sha256" \
  2 \
  "grouped Native AMX V2 fixture SHA-256"
require_exact_digest_occurrences \
  "$closure_ledger" \
  "$grouped_suite_source_manifest_sha256" \
  2 \
  "grouped Native AMX V2 suite-source manifest SHA-256"

sdk_diagnostics_suite_source_manifest_sha256="$(
  bash "$sdk_diagnostics_harness" --suite-source-manifest-sha256
)"
receipt_sdk_diagnostics_suite_source_manifest_sha256="$(
  python3 -I -S - "$repo_root" <<'PY'
from pathlib import Path
import runpy
import sys

root = Path(sys.argv[1]).resolve(strict=True)
sys.path.insert(0, str(root))
symbols = runpy.run_path(str(root / "scripts/write_sumeragi_v2_release_receipt.py"))
print(
    symbols["_sdk_suite_source_manifest"](
        root, symbols["_SUMERAGI_SDK_DIAGNOSTICS_SOURCE_CLOSURE_SUITE"]
    )
)
PY
)"
if [[ ! "$sdk_diagnostics_suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
  || "$receipt_sdk_diagnostics_suite_source_manifest_sha256" \
    != "$sdk_diagnostics_suite_source_manifest_sha256" ]]; then
  echo "Sumeragi v2 SDK diagnostics suite source binding is invalid" >&2
  exit 1
fi
require_exact_digest_occurrences \
  "$closure_ledger" \
  "$sdk_diagnostics_suite_source_manifest_sha256" \
  2 \
  "Sumeragi v2 SDK diagnostics suite-source manifest SHA-256"

if [[ "$(grep -Fxc -- '      --multilane-four-peer-release' "$release_runner" || true)" != 1 ]]; then
  echo "production release runner must invoke the mandatory four-peer launcher exactly once" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    skipped_runs 0 \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion contract must record exactly zero skips" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    expected_runs 4 \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion contract must record exactly four runs" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    native_grouped_pruning_evidence passed \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion must record grouped Native AMX pruning evidence" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "authenticated_remote_recovery=passed exact_once=passed\";" "$native_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX test must publish the exact grouped/pruning marker" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "        .submit_prepared_transaction_payload_batch_async(&payloads)" "$native_recovery_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX test must use one exact Torii batch submission" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    kura.remove_evicted_block_sidecar_for_testing(height)?;" "$native_recovery_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX pruning test must remove the local evicted-body cache" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    kura.remove_latest_native_amx_participant_manifest_for_testing(" "$native_recovery_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX recovery test must stage the exact missing-manifest crash boundary" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "      export IROHA_MULTILANE_RELEASE_MODE=1" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer launcher must set the exact release-mode environment" >&2
  exit 1
fi

echo "[multilane-release-inventory] 88 corridor legs, exact ${canonical_production_test_count}/${canonical_production_test_count} production tests across 40 modules, exact 525/525 G-UNIT (319 core, 143 queue-journal, 13 config, 8 data-model, 39 Torii, 1 Torii-shared, 2 integration), four mandatory G-4P gates, guarded Cargo execution, Rust-owned grouped SDK corpus parity, and exact no-skip Sumeragi diagnostics SDK inventories are source-bound (fixture_sha256=${grouped_fixture_sha256}, grouped_suite_source_manifest_sha256=${grouped_suite_source_manifest_sha256}, sdk_diagnostics_suite_source_manifest_sha256=${sdk_diagnostics_suite_source_manifest_sha256})"
