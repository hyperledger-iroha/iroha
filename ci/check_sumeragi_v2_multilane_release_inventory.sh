#!/usr/bin/env bash
# Static inventory guard for mandatory four-peer Sumeragi V2 multilane release gates.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

readonly autoscale_file="integration_tests/tests/nexus/autoscale_localnet.rs"
readonly idle_file="integration_tests/tests/sumeragi_localnet_smoke.rs"
readonly idle_impl_file="integration_tests/tests/sumeragi_localnet_smoke/idle_chain.rs"
readonly native_file="integration_tests/tests/native_amx_routing.rs"
readonly phase_cut_file="integration_tests/tests/native_amx_routing/selectable_publication_gate.rs"
readonly launcher="scripts/run_nexus_cross_dataspace_atomic_swap.sh"
readonly release_runner="scripts/run_sumeragi_v2_release_gates.sh"
readonly grouped_parity_harness="ci/run_native_amx_v2_grouped_sdk_parity.sh"
readonly grouped_fixture="fixtures/sumeragi_v2/native_amx_v2_grouped.json"
readonly release_receipt_writer="scripts/write_sumeragi_v2_release_receipt.py"
readonly release_bootstrap="scripts/bootstrap_sumeragi_v2_release.py"
readonly prebuilt_bundle_shell="scripts/sumeragi_v2_prebuilt_bundle.sh"
readonly prebuilt_bundle_helper="scripts/sumeragi_v2_prebuilt_bundle.py"
readonly seed_runner="scripts/run_sumeragi_v2_seed_matrix.sh"
readonly formal_harness="scripts/formal/run_sumeragi_v2_harness.sh"
readonly formal_release_runner="scripts/run_sumeragi_v2_formal_release.sh"
readonly formal_gate="ci/check_sumeragi_formal.sh"
readonly verus_runner="scripts/verify_sumeragi_v2.sh"
readonly replay_runner="scripts/formal/check_sumeragi_v2_replay_trace.sh"
readonly chaos_runner="scripts/run_sumeragi_v2_100k_chaos.sh"
readonly taira_runner="scripts/run_taira_v2_24h_soak.sh"
readonly kura_source="crates/iroha_core/src/kura.rs"
readonly test_network_source="crates/iroha_test_network/src/lib.rs"
readonly autoscale_test="nexus_autoscale_four_peer_release_lifecycle_recreates_lane_and_rejects_stale_artifacts"
readonly autoscale_qualified_test="nexus::autoscale_localnet::${autoscale_test}"
readonly autoscale_restart_test="nexus_autoscale_certified_merge_recovers_missing_sidecar_after_restart"
readonly autoscale_restart_qualified_test="nexus::autoscale_localnet::${autoscale_restart_test}"
readonly autoscale_drain_test="nexus_autoscale_two_phase_drain_closes_certifies_then_retires_after_restart"
readonly autoscale_drain_qualified_test="nexus::autoscale_localnet::${autoscale_drain_test}"
readonly native_test="native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs"
readonly native_amx_release_iterations=10
readonly native_grouped_pruning_marker="[multilane-release-native-evidence] grouped_sources=2 durable_manifest=passed body_eviction_recovery=passed authenticated_remote_recovery=passed exact_once=passed"
readonly ex297_idle_test="permissioned_idle_chain_advances_only_for_external_or_internal_work"
readonly ex297_idle_qualified_test="sumeragi_localnet_smoke::${ex297_idle_test}"
readonly ex297_idle_marker="[ex-297-idle-evidence] clean_idle=passed external_non_empty=passed internal_non_empty=passed"
readonly ex297_phase_cut_test="musubi_selectable_publication_phase_cut_matrix_is_atomic_after_replay"
readonly ex297_phase_cut_marker="[ex-297-phase-cut-evidence] after_prepare_qc=passed after_commit_qc=passed before_world_commit=passed exact_once=passed"
readonly canonical_production_test_count=826

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

require_nonignored_test "$autoscale_file" "$autoscale_test"
require_nonignored_test "$autoscale_file" "$autoscale_restart_test"
require_nonignored_test "$autoscale_file" "$autoscale_drain_test"
require_nonignored_test "$native_file" "$native_test"
require_nonignored_test "$idle_file" "$ex297_idle_test"
require_nonignored_test "$native_file" "$ex297_phase_cut_test"

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
  "readonly NATIVE_AMX_RELEASE_ITERATIONS=${native_amx_release_iterations}"
require_exact_token \
  "$launcher" \
  '    IROHA_TEST_NETWORK_BASE_SEED|IROHA_NEXUS_CROSS_REQUIRE_SEED|IROHA_NEXUS_CROSS_FAULT_SOAK_DURATION_SECS|IROHA_NATIVE_AMX_SOAK_ITERATIONS|IROHA_MULTILANE_RELEASE_MODE|IROHA_RUN_IGNORED|IROHA_RELEASE_PREBUILT_MANIFEST_SHA256)'
require_exact_token \
  "$launcher" \
  '  ENV_VARS+=("IROHA_NATIVE_AMX_SOAK_ITERATIONS=${NATIVE_AMX_RELEASE_ITERATIONS}")'
require_exact_token \
  "$launcher" \
  "readonly NATIVE_AMX_GROUPED_PRUNING_MARKER=\"${native_grouped_pruning_marker}\""
require_exact_token \
  "$launcher" \
  "readonly EX297_IDLE_CHAIN_RELEASE_TEST=\"${ex297_idle_qualified_test}\""
require_exact_token \
  "$launcher" \
  "readonly EX297_IDLE_CHAIN_RELEASE_MARKER=\"${ex297_idle_marker}\""
require_exact_token \
  "$idle_file" \
  'const EX297_IDLE_CHAIN_RELEASE_TEST: &str ='
require_exact_token \
  "$idle_file" \
  '    "sumeragi_localnet_smoke::permissioned_idle_chain_advances_only_for_external_or_internal_work";'
require_exact_token \
  "$idle_impl_file" \
  '    let context = EX297_IDLE_CHAIN_RELEASE_TEST;'
require_exact_token \
  "$launcher" \
  "readonly EX297_PHASE_CUT_RELEASE_TEST=\"${ex297_phase_cut_test}\""
require_exact_token \
  "$launcher" \
  "readonly EX297_PHASE_CUT_RELEASE_MARKER=\"${ex297_phase_cut_marker}\""
require_exact_token \
  "$launcher" \
  'ENV_VARS+=("IROHA_FAIL_ON_SANDBOX_SKIP=1")'
require_exact_token \
  "$launcher" \
  '    "consensus_and_da|${EX297_IDLE_CHAIN_RELEASE_TEST}"'
require_exact_token \
  "$launcher" \
  '    "native_amx_routing|${EX297_PHASE_CUT_RELEASE_TEST}"'
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
  "readonly multilane_ex297_idle_release_test=\"${ex297_idle_qualified_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_ex297_idle_release_marker=\"${ex297_idle_marker}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_ex297_phase_cut_release_test=\"${ex297_phase_cut_test}\""
require_exact_token \
  "$release_runner" \
  "readonly multilane_ex297_phase_cut_release_marker=\"${ex297_phase_cut_marker}\""
require_exact_token \
  "$release_runner" \
  '  "consensus_and_da|${multilane_ex297_idle_release_test}" \'
require_exact_token \
  "$release_runner" \
  '  "native_amx_routing|${multilane_ex297_phase_cut_release_test}"; do'
require_exact_token \
  "$release_runner" \
  "    \$'ex297_phase_cut_evidence\\tpassed'; do"
require_exact_token \
  "$release_runner" \
  "readonly native_amx_grouped_parity_harness=\"${grouped_parity_harness}\""
require_exact_token \
  "$release_runner" \
  "readonly expected_multilane_focus_test_count=313"
require_exact_token \
  "$release_runner" \
  "readonly expected_production_liveness_test_count=${canonical_production_test_count}"
require_exact_token \
  "$release_runner" \
  "  readonly expected_corridor_leg_count=81"
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
  "    native_amx_grouped_negative_control_count 51 \\"
require_exact_token \
  "$grouped_parity_harness" \
  "readonly expected_negative_control_count=51"
for grouped_test_count in 7 58 56 4 6 5; do
  require_exact_token \
    "$grouped_parity_harness" \
    "    observed_test_count=${grouped_test_count}"
done
require_exact_token \
  "$release_receipt_writer" \
  "_NATIVE_AMX_GROUPED_NEGATIVE_CONTROL_COUNT = 51"
require_exact_token \
  "$release_receipt_writer" \
  "_G_UNIT_TEST_COUNT = 313"
require_exact_token \
  "$release_receipt_writer" \
  "_PRODUCTION_TEST_COUNT = ${canonical_production_test_count}"
require_exact_token \
  "$release_receipt_writer" \
  "_G4P_NATIVE_AMX_GROUPED_PRUNING_MARKER = ("
require_exact_token \
  "$release_receipt_writer" \
  "_G4P_EX297_IDLE_MARKER = ("
require_exact_token \
  "$release_receipt_writer" \
  "_G4P_EX297_PHASE_CUT_MARKER = ("
require_exact_token \
  "$release_receipt_writer" \
  '        "native_grouped_pruning_evidence": "passed",'
require_exact_token \
  "$release_receipt_writer" \
  '        "ex297_idle_evidence": "passed",'
require_exact_token \
  "$release_receipt_writer" \
  '        "ex297_phase_cut_evidence": "passed",'
require_exact_token \
  "$release_bootstrap" \
  '        "run-04-consensus_and_da.log",'
require_exact_token \
  "$release_bootstrap" \
  '        "run-05-native_amx_routing.log",'
for grouped_suite in \
  '    ("openapi", 7),' \
  '    ("python", 58),' \
  '    ("javascript", 56),' \
  '    ("swift", 4),' \
  '    ("kotlin", 6),' \
  '    ("java", 5),'; do
  require_exact_token "$release_receipt_writer" "$grouped_suite"
done

python3 -I -S - \
  "$release_runner" \
  "$release_receipt_writer" \
  "$canonical_production_test_count" <<'PY'
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
canonical_production_test_count = int(sys.argv[3])


def reject(message: str) -> None:
    raise SystemExit(f"{runner}: {message}")


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

receipt_tree = ast.parse(receipt_source, filename=str(receipt_writer))
receipt_assignments: dict[str, object] = {}
for node in receipt_tree.body:
    if not isinstance(node, ast.Assign) or len(node.targets) != 1:
        continue
    target = node.targets[0]
    if (
        isinstance(target, ast.Name)
        and target.id in {"_PRODUCTION_TEST_COUNT", "_PRODUCTION_MODULES"}
    ):
        receipt_assignments[target.id] = ast.literal_eval(node.value)
if (
    receipt_assignments.get("_PRODUCTION_TEST_COUNT")
    != canonical_production_test_count
):
    reject(
        "receipt writer production count must equal "
        f"{canonical_production_test_count}"
    )
production_modules = receipt_assignments.get("_PRODUCTION_MODULES")
if not isinstance(production_modules, tuple) or len(production_modules) != 38:
    reject("receipt writer must bind exactly 38 production modules")
module_counts = {
    module: count for _leg_id, module, count in production_modules
}
if (
    len(module_counts) != 38
    or sum(module_counts.values()) != canonical_production_test_count
):
    reject(
        "receipt writer production-module counts must sum exactly to "
        f"{canonical_production_test_count}"
    )
expected_changed_module_counts = {
    "sumeragi::authoritative_runtime_gate_tests": 41,
    "sumeragi::serviced_candidate_store::tests": 1,
    "sumeragi::v2_effects::tests": 71,
    "sumeragi::v2::tests": 46,
    "sumeragi::v2_runtime::tests": 68,
    "merge_sidecar::tests": 118,
    "sumeragi::v2_lane_work::tests": 53,
    "sumeragi::v2_runner::tests": 34,
    "sumeragi::v2_worker::tests": 131,
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
    "4feda6be5196d970ff4a0a114bca7f30"
    "2c96fa967845a965bcdbe11a2978906f"
):
    reject(
        f"canonical {canonical_production_test_count}-test production TSV "
        "digest changed"
    )


wait_definition = exact_line("wait_for_external_cargo() {")
process_snapshot = exact_line("    ps -axo pid,etime,command")
run_cargo_start = exact_line("run_cargo() {")
if not wait_definition < process_snapshot < run_cargo_start:
    reject("Cargo quiescence guard must run the exact required process snapshot")

run_cargo_definition = """\
run_cargo() {
  wait_for_external_cargo
  command cargo "$@"
}"""
if source.count(run_cargo_definition) != 1:
    reject("run_cargo must be defined exactly once and gate every Cargo invocation")

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
    58
    56
    4
    6
    5
  )"""
if source.count(native_amx_parity_inventory) != 1:
    reject(
        "grouped Native AMX V2 release surfaces and exact test counts "
        "must remain paired in canonical order"
    )

# Command descriptions beginning with `cargo` are source-sealed evidence, not
# execution. Reject every direct shell execution form; the only permitted
# `command cargo` is the guarded implementation above.
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
expected_direct_cargo = [(exact_line('  command cargo "$@"') + 1, '  command cargo "$@"')]
if direct_cargo_lines != expected_direct_cargo:
    rendered = ", ".join(f"{number}:{line}" for number, line in direct_cargo_lines)
    reject(f"Cargo execution bypasses run_cargo ({rendered})")

# The two authenticated toolchain-version probes intentionally invoke a
# resolved Cargo binary. They do not build or test, and each remains guarded by
# the same process-quiescence check as run_cargo.
release_probe = exact_line(
    '    || "$("$release_cargo_bin" --version)" != "cargo 1.93.1 (083ac5135 2025-12-15)" \\'
)
if lines[release_probe - 2] != "  wait_for_external_cargo":
    reject("release Cargo version probe is not guarded by wait_for_external_cargo")
corridor_probe = exact_line(
    '  corridor_cargo_version="$("$corridor_cargo_path" --version)"'
)
if lines[corridor_probe - 1] != "  wait_for_external_cargo":
    reject("corridor Cargo version probe is not guarded by wait_for_external_cargo")
resolved_probe_pattern = re.compile(
    r'"\$\("\$[A-Za-z_][A-Za-z0-9_]*cargo[A-Za-z0-9_]*"\s+--version\)"'
)
resolved_probes = [
    (index, line)
    for index, line in enumerate(lines)
    if resolved_probe_pattern.search(line)
]
if [index for index, _ in resolved_probes] != [release_probe, corridor_probe]:
    reject("resolved Cargo may only execute in the two guarded version probes")
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
    (release_probe, "release_cargo_bin", "--version"),
    (corridor_probe, "corridor_cargo_path", "--version"),
]:
    reject(
        "resolved Cargo execution must be limited to the two guarded "
        "version probes"
    )

source_sealed_blocks = (
    """\
  run_corridor_leg \\
    source-sealed-workspace-format command 0 \\
    "cargo fmt --all -- --check" \\
    run_cargo fmt --all -- --check""",
    """\
  run_corridor_leg \\
    source-sealed-legacy-codec-guard command 0 \\
    "bash scripts/check_no_legacy_codec.sh" \\
    bash scripts/check_no_legacy_codec.sh""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-build command 0 \\
    "cargo build --locked --offline --workspace" \\
    run_cargo build --locked --offline --workspace""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-clippy command 0 \\
    "cargo clippy --locked --offline --workspace --all-targets -- -D warnings" \\
    run_cargo clippy --locked --offline --workspace --all-targets -- -D warnings""",
    """\
  run_corridor_leg \\
    source-sealed-workspace-tests command 0 \\
    "cargo test --locked --offline --workspace" \\
    run_cargo test --locked --offline --workspace""",
    """\
  run_corridor_leg \\
    source-sealed-irohad-tests command 0 \\
    "cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control" \\
    run_cargo test --locked --offline -p irohad --bin irohad --features test-network-message-control""",
)
for block in source_sealed_blocks:
    if source.count(block) != 1:
        label = block.splitlines()[1].strip().split()[0]
        reject(f"source-sealed command/evidence block {label} is missing or duplicated")

expected_focus_counts = {
    "required_multilane_core_focus_tests": 117,
    "required_multilane_queue_journal_focus_tests": 137,
    "required_multilane_config_lib_focus_tests": 3,
    "required_multilane_config_runtime_focus_tests": 2,
    "required_multilane_config_fixtures_focus_tests": 2,
    "required_multilane_data_model_focus_tests": 8,
    "required_multilane_torii_focus_tests": 41,
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

if len(all_focus_entries) != 313 or len(set(all_focus_entries)) != 313:
    reject(
        "multilane focus-test arrays must contain 313 globally distinct tests; "
        f"found {len(all_focus_entries)} entries and "
        f"{len(set(all_focus_entries))} distinct entries"
    )

g_unit_groups = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
        117,
        "--lib",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
        137,
        "--lib",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
        3,
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
        41,
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
        reject("G-UNIT expected 313 count is not published exactly once")
    if expected_count <= 0:
        reject(f"G-UNIT leg {leg_id} has an invalid expected count")

fixture_check = """\
run_corridor_leg \\
  native-amx-rust-fixture-check command 0 \\
  "cargo run --locked --offline -p iroha_data_model --features dev-tools --bin sumeragi_v2_wire_fixtures -- --check" \\
  run_cargo run --locked --offline -p iroha_data_model --features dev-tools \\
    --bin sumeragi_v2_wire_fixtures -- --check"""
if source.count(fixture_check) != 1:
    reject(
        "Rust-owned grouped fixture regeneration must be one guarded "
        "source-sealed corridor leg"
    )

scaling_definition = exact_line("run_release_scaling_and_formal_gates() {")
scaling_call = exact_line("  run_release_scaling_and_formal_gates")
g12_soak = exact_line(
    '  verify_release_identity "after G-12P two-hour rotating-validator fault soak"'
)
pr_branch = exact_line('if [[ "$profile" == "--pr" ]]; then')
if not scaling_definition < g12_soak < scaling_call < pr_branch:
    reject("scaling/formal release gates must run after the completed G-12P fault soak")

scaling_validation = exact_line(
    "    scripts/nexus/validate_multilane_scaling_evidence.py \\"
)
formal_release = exact_line("    bash scripts/run_sumeragi_v2_formal_release.sh")
if not scaling_definition < scaling_validation < formal_release < g12_soak:
    reject("scaling/formal gate function must validate scaling before formal evidence")

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
  "$prebuilt_bundle_shell" \
  "$prebuilt_bundle_helper" <<'PY'
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
        "scripts/run_sumeragi_v2_release_gates.sh",
        "scripts/sumeragi_v2_prebuilt_bundle.sh",
        'source "${repo_root}/scripts/sumeragi_v2_prebuilt_bundle.sh"',
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
        '"$IROHA_RELEASE_PYTHON_BIN" -I -S scripts/write_sumeragi_v2_release_receipt.py',
    ),
    (
        "scripts/run_sumeragi_v2_formal_release.sh",
        "ci/check_sumeragi_formal.sh",
        "bash ci/check_sumeragi_formal.sh",
    ),
    (
        "ci/check_sumeragi_formal.sh",
        "scripts/verify_sumeragi_v2.sh",
        "bash scripts/verify_sumeragi_v2.sh",
    ),
    (
        "ci/check_sumeragi_formal.sh",
        "scripts/formal/check_sumeragi_v2_replay_trace.sh",
        "bash scripts/formal/check_sumeragi_v2_replay_trace.sh",
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
)
run_cargo_definition = """\
run_cargo() {
  wait_for_external_cargo
  command cargo "$@"
}"""
for script in guarded_cargo_scripts:
    source = sources[script]
    if source.count("wait_for_external_cargo() {") != 1:
        reject(f"{script} must define exactly one Cargo quiescence guard")
    if source.count("    ps -axo pid,etime,command") != 1:
        reject(f"{script} must execute the exact ps -axo pid,etime,command snapshot")
    if source.count(run_cargo_definition) != 1:
        reject(f"{script} must route Cargo through the exact guarded wrapper")
    if "CARGO_TARGET_DIR" not in source:
        reject(f"{script} does not bind Cargo to an isolated target")

    lines = source.splitlines()
    direct = [
        (index + 1, line)
        for index, line in enumerate(lines)
        if re.search(r"^\s*(?:command\s+)?cargo(?:\s|$)", line)
    ]
    expected_direct = [
        (index + 1, line)
        for index, line in enumerate(lines)
        if line == '  command cargo "$@"'
    ]
    if direct != expected_direct:
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

harness = sources["scripts/formal/run_sumeragi_v2_harness.sh"]
if harness.count("    run_cargo fetch --locked") != 1:
    reject("formal harness must isolate its sole explicitly-online --fetch mode")
for forbidden in ('"${@:2}"', "bash -c", "sh -c", "env cargo"):
    if forbidden in harness:
        reject(f"formal harness retains arbitrary child-command dispatch: {forbidden}")
if harness.count('"$@"') != 1:
    reject("formal harness may pass the argument vector only inside run_cargo")
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
if 'CARGO_TARGET_DIR="$(mktemp -d ' not in verify:
    reject("Verus runner must provide an isolated target to its Cargo child")

for script, source in sources.items():
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
    if source.count(
        "ensure_source_bound_localnet_binaries\n"
        "export_source_bound_localnet_binaries"
    ) != 1:
        reject(
            f"{script} must publish localnet executable overrides immediately "
            "after its source-bound prebuild"
        )

prebuilt_shell = sources["scripts/sumeragi_v2_prebuilt_bundle.sh"]
publish_block = '''\
  export TEST_NETWORK_BIN_IROHAD="${IROHA_TEST_TARGET_DIR}/release/irohad"
  export TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL="${IROHA_TEST_TARGET_DIR}/message-control/release/irohad"
  export TEST_NETWORK_BIN_IROHA="${IROHA_TEST_TARGET_DIR}/release/iroha"
  export KAGAMI_BIN="${IROHA_TEST_TARGET_DIR}/release/kagami"'''
if prebuilt_shell.count(publish_block) != 1:
    reject("shared prebuild helper must publish the four exact manifest paths once")
for command in (
    "run_cargo build --locked --offline --release -p irohad --bin irohad",
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
    "command cargo --version",
    "command rustc -vV",
    '--manifest-sha256 "$IROHA_RELEASE_PREBUILT_MANIFEST_SHA256"',
):
    if prebuilt_shell.count(token) != 1:
        reject(f"shared prebuild helper lacks exact fail-closed token {token!r}")

prebuilt_python = sources["scripts/sumeragi_v2_prebuilt_bundle.py"]
for token in (
    '_SCHEMA_VERSION = "2"',
    "_BINARY_MODE = 0o500",
    "_MANIFEST_MODE = 0o400",
    "_DIRECTORY_MODE = 0o500",
    '"release/irohad"',
    '"message-control/release/irohad"',
    '"release/iroha"',
    '"release/kagami"',
    '"cargo_version_sha256"',
    '"rustc_version_sha256"',
    '"bundle_dir"',
    "_validate_exact_bundle_tree(bundle)",
):
    if token not in prebuilt_python:
        reject(f"prebuilt bundle v2 helper lacks contract token {token!r}")

release = sources["scripts/run_sumeragi_v2_release_gates.sh"]
if "--no-skip-build" in release:
    reject("release runner may not request reentrant localnet builds")
if release.count("--env IROHA_TEST_ALLOW_REENTRANT_BUILD=0") != 2:
    reject("release launcher calls must pin reentrant builds off")
if release.count("  IROHA_TEST_ALLOW_REENTRANT_BUILD=0") != 1:
    reject("array-based release launcher must pin reentrant builds off")

launcher_source = sources["scripts/run_nexus_cross_dataspace_atomic_swap.sh"]
if 'CARGO_TEST_CMD+=("--locked" "--offline")' not in launcher_source:
    reject("Nexus child launcher Cargo commands are not locked/offline")
if 'ENV_VARS+=("IROHA_TEST_SKIP_BUILD=1")' not in launcher_source:
    reject("Nexus child launcher does not propagate skip-build")

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
    'const NATIVE_AMX_EVIDENCE_PRUNE_INTENT_FILE: &str = "native_amx_evidence_prune_intent_v1.norito";',
    '    "native_amx_evidence_prune_intent_v1.norito.tmp";',
    '    "native_amx_participant_receipts.latest_v2.norito";',
    '    "native_amx_participant_receipts.latest_v2.norito.tmp";',
)
for declaration in required_layout:
    if source.count(declaration) != 1:
        reject(f"required current filename declaration is missing or duplicated: {declaration!r}")

obsolete_dense_names = (
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
lines = launcher.read_text(encoding="utf-8").splitlines()
execution = re.compile(
    r'^\s*env .*"\$\{(?:CMD|LIST_CMD)\[@\]\}"(?:\s|$)'
)
execution_lines = [
    index for index, line in enumerate(lines) if execution.search(line)
]
if len(execution_lines) != 10:
    raise SystemExit(
        f"{launcher}: expected 10 Cargo execution sites; "
        f"found {len(execution_lines)}"
    )
for index in execution_lines:
    if index == 0 or lines[index - 1].strip() != "wait_for_cargo_idle":
        raise SystemExit(
            f"{launcher}:{index + 1}: Cargo execution is not immediately "
            "preceded by wait_for_cargo_idle"
        )
PY

for grouped_surface in openapi python javascript swift kotlin java; do
  if [[ "$(grep -Ec -- "^    ${grouped_surface}$" "$release_runner" || true)" != 1 ]]; then
    echo "production release runner must inventory grouped Native AMX V2 ${grouped_surface} parity exactly once" >&2
    exit 1
  fi
  if ! grep -Fq -- "${grouped_surface})" "$grouped_parity_harness"; then
    echo "grouped Native AMX V2 parity harness lacks ${grouped_surface} execution" >&2
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
print(symbols["_native_amx_grouped_suite_source_manifest"](root))
PY
)"
if [[ ! "$grouped_fixture_sha256" =~ ^[0-9a-f]{64}$ \
  || ! "$grouped_suite_source_manifest_sha256" =~ ^[0-9a-f]{64}$ \
  || "$receipt_suite_source_manifest_sha256" \
    != "$grouped_suite_source_manifest_sha256" ]]; then
  echo "grouped Native AMX V2 fixture/suite source binding is invalid" >&2
  exit 1
fi

if [[ "$(grep -Fxc -- '      --multilane-four-peer-release' "$release_runner" || true)" != 1 ]]; then
  echo "production release runner must invoke the mandatory four-peer launcher exactly once" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    skipped_runs 0 \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion contract must record exactly zero skips" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    expected_runs 6 \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion contract must record exactly six runs" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    native_grouped_pruning_evidence passed \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion must record grouped Native AMX pruning evidence" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    ex297_idle_evidence passed \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion must record EX-297 idle evidence" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    ex297_phase_cut_evidence passed \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer completion must record EX-297 phase-cut evidence" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "authenticated_remote_recovery=passed exact_once=passed\";" "$native_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX test must publish the exact grouped/pruning marker" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "clean_idle=passed external_non_empty=passed internal_non_empty=passed\";" "$idle_file" || true)" != 1 ]]; then
  echo "mandatory EX-297 idle test must publish the exact release marker" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "after_prepare_qc=passed after_commit_qc=passed before_world_commit=passed exact_once=passed\";" "$native_file" || true)" != 1 ]]; then
  echo "mandatory EX-297 phase-cut test must publish the exact release marker" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- '        eprintln!("[multilane-release-gate] started: {context}");' "$idle_impl_file" || true)" != 1 \
  || "$(grep -Fxc -- '        eprintln!("[multilane-release-gate] completed: {context}");' "$idle_impl_file" || true)" != 1 \
  || "$(grep -Fxc -- '    eprintln!("[multilane-release-gate] started: {context}");' "$phase_cut_file" || true)" != 1 \
  || "$(grep -Fxc -- '    eprintln!("[multilane-release-gate] completed: {context}");' "$phase_cut_file" || true)" != 1 ]]; then
  echo "mandatory EX-297 tests must publish exact started/completed release markers" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "        .submit_prepared_transaction_payload_batch_async(&payloads)" "$native_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX test must use one exact Torii batch submission" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    kura.remove_evicted_block_sidecar_for_testing(height)?;" "$native_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX pruning test must remove the local evicted-body cache" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    kura.remove_latest_native_amx_participant_manifest_for_testing(" "$native_file" || true)" != 1 ]]; then
  echo "mandatory Native AMX recovery test must stage the exact missing-manifest crash boundary" >&2
  exit 1
fi
if [[ "$(grep -Fxc -- "    env \"\${ENV_VARS[@]}\" IROHA_MULTILANE_RELEASE_MODE=1 \"\${CMD[@]}\" \\" "$launcher" || true)" != 1 ]]; then
  echo "mandatory four-peer launcher must set the exact release-mode environment" >&2
  exit 1
fi

echo "[multilane-release-inventory] 81 corridor legs, exact ${canonical_production_test_count}/${canonical_production_test_count} production tests across 38 modules, exact 313/313 G-UNIT (117 core, 137 queue-journal, 7 config, 8 data-model, 41 Torii, 1 Torii-shared, 2 integration), six mandatory G-4P gates, guarded Cargo execution, and Rust-owned grouped SDK corpus regeneration/parity are source-bound (fixture_sha256=${grouped_fixture_sha256}, suite_source_manifest_sha256=${grouped_suite_source_manifest_sha256})"
