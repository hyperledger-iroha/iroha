#!/usr/bin/env python3
"""Catch Taira CLI release regressions before expensive cross-compilation.

Requires Python 3.11+, the repository Rust toolchain and Cargo on PATH. Compile
one native CLI test harness through scripts/cargo_fast.sh, then run existing
fixture-only tests. Reuse the current warm Cargo target and native jobserver.
No live configuration, runtime credentials, SSH, deployment or signing inputs
are accepted. No report files are written; Cargo and test fixtures use their
usual local scratch files. This focused check does not qualify release artifacts.

Pass --repo-root to check another Iroha checkout. CARGO_TARGET_DIR may name an existing
stable native build lane; this script never creates a per-run lane or cleans it.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time


STAGES = (
    ("private config descriptors", (
        "client_config::tests::inherited_config_loads_exact_descriptor_without_reopening_provenance",
        "client_config::tests::inherited_private_descriptor_rejects_writable_unsafe_and_nonregular_inputs",
        "client_config::tests::inherited_private_descriptor_rejects_pipe_socket_and_closed_fd",
        "client_config::tests::inherited_config_errors_never_include_source_values",
        "tests::inherited_config_cli_requires_explicit_provenance_and_rejects_mixed_sources",
        "taira_public_reset::inputs::tests::inherited_owner_key_is_bounded_private_and_matches_the_independent_public_key",
        "taira_public_reset::inputs::tests::signing_key_rejects_hardlinks_and_nonregular_descriptors_without_reading_them",
    )),
    ("native validator config preparation", (
        "taira_public_reset::config::tests::config_rebase_changes_only_genesis_file_and_keeps_errors_secret_free",
        "taira_public_reset::config::tests::config_rebase_inherited_fd_publishes_private_file_without_stdout_or_overwrite",
        "taira_public_reset::config::tests::config_rebase_rejects_drift_malformed_input_and_unsafe_descriptors_before_output",
        "taira_public_reset::config::tests::config_rebase_preserves_caller_offset_and_rejects_writable_or_unlinked_descriptors",
        "taira_public_reset::host::tests::config_upload_admission_requires_owner_private_artifact_mode",
        "taira_public_reset::executor_model::tests::shared_validator_artifacts_are_hashed_once_per_local_source",
        "taira_public_reset::executor_model::tests::shared_artifact_declarations_must_agree_on_content_size_and_mode",
        "taira_public_reset::executor_model::tests::shared_artifact_descriptor_clone_rejects_path_identity_drift",
        "taira_public_reset::inputs::tests::fresh_output_is_private_durable_and_never_overwrites_or_follows_a_symlink",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_accepts_exact_artifact_and_checked_identity",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_rejects_unbound_manifest_identity_and_inheritance",
        "taira_public_reset::signed_genesis_startup_tests::signed_genesis_startup_rejects_foreign_path_and_network",
    )),
    ("network 369 inventory boundaries", (
        "taira_public_reset::executor_model::tests::inventory_wire_roundtrip_scopes_nonempty_placements_before_decode",
        "taira_public_reset::executor_model::tests::inventory_file_boundary_preserves_original_bytes_and_decode_guard",
    )),
    ("aggregate execution budget before custody", (
        "taira_public_reset::inputs::tests::aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads",
        "taira_public_reset::inputs::tests::aggregate_timeout_policy_accepts_deployment_defaults_and_preserves_individual_bounds",
    )),
    ("physical preseed and start budgets", (
        "taira_public_reset::inputs::tests::preseed_timeout_is_required_and_has_its_own_physical_work_bound",
        "taira_public_reset::host::tests::preseed_and_start_deadlines_charge_only_each_physical_host_carrier",
        "taira_public_reset::host::tests::host_admission_accepts_combined_carrier_verification_but_rejects_borrowed_time",
        "taira_public_reset::executor_model::tests::execution_lifetime_uses_the_exact_first_release_action_ledger",
    )),
    ("generated stage through frozen consumer", (
        "soracloud::tests::taira_inrou_workspace_generator_emits_exact_private_deploy_layout",
        "soracloud::tests::taira_stage_reads_require_private_custody_for_prepared_and_frozen_files",
    )),
    ("preseed receipt ordering", (
        "taira_public_reset::host::tests::preseed_receipt_targets_follow_receipt_order_for_reversed_stores",
    )),
    ("KVM ioctl error handling", (
        "taira_public_reset::host::tests::kvm_api_query_preserves_notty_for_regular_files",
    )),
    ("bounded duplex process streaming", (
        "taira_public_reset::host::tests::process_runner_streams_large_closure_with_bidirectional_backpressure",
        "taira_public_reset::host::tests::process_runner_output_budget_bounds_continuous_and_interrupted_readers",
        "taira_public_reset::host::tests::process_runner_deadline_kills_descendant_holding_output_pipes",
        "taira_public_reset::host::tests::process_runner_enforces_one_absolute_timeout",
        "taira_public_reset::host::tests::process_runner_handles_child_that_closes_stdin_early",
        "taira_public_reset::host::tests::process_runner_preserves_rejection_after_child_closes_stdin",
        "taira_public_reset::host::tests::process_runner_deadline_reaps_child_after_stdin_closure",
    )),
    ("canonical receipt namespace", (
        "taira_public_reset::host::tests::host_receipt_names_cover_every_action_and_artifact_role",
        "taira_public_reset::host::tests::receipt_names_reject_path_control_and_unicode_escape",
    )),
    ("systemd operation and validator lifecycle", (
        "taira_public_reset::host::tests::manager_evidence_stays_pending_until_exact_terminal_job",
        "taira_public_reset::host::tests::manager_recovery_uses_immutable_mutation_deadline_but_observes_terminal_state",
        "taira_public_reset::host::tests::manager_evidence_rejects_wrong_or_duplicate_exec_identity",
        "taira_public_reset::host::tests::manager_evidence_accepts_captured_systemd_numeric_exit_after_deadline",
        "taira_public_reset::host::tests::manager_evidence_requires_exact_numeric_exit_code_and_status",
        "taira_public_reset::host::tests::manager_evidence_keeps_unexecuted_and_running_operations_pending",
        "taira_public_reset::host::tests::validator_restart_evidence_requires_running_service_and_settled_job",
        "taira_public_reset::host::tests::validator_process_readiness_waits_for_launcher_then_daemon",
        "taira_public_reset::host::tests::validator_process_readiness_preserves_original_deadline",
        "taira_public_reset::host::tests::validator_process_readiness_rejects_changed_launcher_immediately",
    )),
    ("read-only host preflight", (
        "taira_public_reset::host::tests::preflight_dispatches_five_read_only_hosts_without_runtime_custody",
    )),
)

if sys.platform == "linux":
    STAGES += (("OpenSSH parent descriptor custody", (
        "taira_public_reset::host::tests::openssh_parent_pinned_inputs_survive_descriptor_sweep_without_network",
    )),)


class CheckError(Exception):
    """A build or selected regression did not pass."""


def compile_command(root: Path) -> list[str]:
    return [str(root / "scripts/cargo_fast.sh"), "--", "test", "--locked",
            "-p", "iroha_cli", "--bin", "iroha", "--no-run",
            "--message-format=json-render-diagnostics"]


def test_artifact(line: str) -> str | None:
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        return None  # The accelerator wrapper also emits ordinary progress.
    if not isinstance(event, dict) or event.get("reason") != "compiler-artifact":
        return None
    target = event.get("target", {})
    if (target.get("name") == "iroha" and "bin" in target.get("kind", [])
            and event.get("profile", {}).get("test") is True):
        executable = event.get("executable")
        if isinstance(executable, str) and executable:
            return executable
    return None


def show_build_diagnostic(line: str) -> None:
    """Keep Cargo's rendered compiler errors visible while consuming JSON events."""
    try:
        event = json.loads(line)
    except json.JSONDecodeError:
        sys.stdout.write(line)  # Preserve accelerator progress, not raw JSON events.
        sys.stdout.flush()
        return
    if isinstance(event, dict) and event.get("reason") == "compiler-message":
        message = event.get("message")
        rendered = message.get("rendered") if isinstance(message, dict) else None
        if isinstance(rendered, str):
            sys.stderr.write(rendered)
            sys.stderr.flush()


def compile_harness(root: Path, env: dict[str, str]) -> str:
    command = compile_command(root)
    print("[taira-check] build native CLI test harness", flush=True)
    started = time.monotonic()
    artifacts: set[str] = set()
    with subprocess.Popen(command, cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8", errors="replace") as child:
        assert child.stdout is not None
        for line in child.stdout:
            show_build_diagnostic(line)
            artifact = test_artifact(line)
            if artifact is not None:
                artifacts.add(artifact)
        code = child.wait()
    elapsed = time.monotonic() - started
    if code:
        raise CheckError(f"native CLI build failed (exit {code}, {elapsed:.1f}s)")
    if len(artifacts) != 1:
        raise CheckError(f"native CLI build reported {len(artifacts)} test executables; expected one")
    print(f"[taira-check] native CLI build passed in {elapsed:.1f}s", flush=True)
    return artifacts.pop()


def require_tests(listing: str) -> None:
    available = {line.removesuffix(": test") for line in listing.splitlines()
                 if line.endswith(": test")}
    missing = [name for _, names in STAGES for name in names if name not in available]
    if missing:
        raise CheckError("required regressions missing from native harness: " + ", ".join(missing))


def require_one_pass(name: str, result: subprocess.CompletedProcess[str]) -> None:
    if (result.returncode != 0
            or f"test {name} ... ok" not in result.stdout.splitlines()
            or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in result.stdout):
        # These tests use disposable fixtures, never operator runtime inputs.
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)
        raise CheckError(f"regression did not execute and pass: {name} (exit {result.returncode})")


def run_checks(root: Path, *, environment: dict[str, str] | None = None) -> None:
    if sys.platform not in {"darwin", "linux"}:
        raise CheckError("the Taira descriptor/stage gate requires macOS or Linux")
    started = time.monotonic()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root,
                                   stdin=subprocess.DEVNULL, text=True).strip()
    env = dict(os.environ if environment is None else environment)
    env.pop("CARGO_BUILD_TARGET", None)  # This check executes a host-native harness.
    env["VERGEN_GIT_SHA"] = head
    env["IROHA_GIT_COMMIT_HASH"] = head
    print(f"[taira-check] source HEAD {head}; current worktree inputs", flush=True)
    harness = compile_harness(root, env)
    listing = subprocess.run([harness, "--list", "--format", "terse"], cwd=root,
                             env=env, stdin=subprocess.DEVNULL, text=True, capture_output=True, check=False)
    if listing.returncode:
        raise CheckError(f"cannot list native harness tests (exit {listing.returncode})")
    require_tests(listing.stdout)
    for label, names in STAGES:
        stage_start = time.monotonic()
        print(f"[taira-check] start {label} ({len(names)} tests)", flush=True)
        for name in names:
            test_start = time.monotonic()
            print(f"[taira-check] start {name}", flush=True)
            result = subprocess.run([harness, name, "--exact", "--color", "never"],
                                    cwd=root, env=env, stdin=subprocess.DEVNULL,
                                    text=True, capture_output=True, check=False)
            require_one_pass(name, result)
            print(f"[taira-check] passed {name} ({time.monotonic() - test_start:.1f}s)", flush=True)
        print(f"[taira-check] passed {label} ({time.monotonic() - stage_start:.1f}s)", flush=True)
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root,
                               stdin=subprocess.DEVNULL, text=True).strip() != head:
        raise CheckError("HEAD changed during checks; rerun against the intended source")
    count = sum(len(names) for _, names in STAGES)
    print(f"[taira-check] PASS: {count} regressions in {time.monotonic() - started:.1f}s", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1],
                        help="repository root (default: this maintained script's parent repository)")
    args = parser.parse_args()
    try:
        run_checks(args.repo_root.resolve(strict=True))
    except (CheckError, OSError, subprocess.SubprocessError) as error:
        print(f"[taira-check] FAIL: {error}", file=sys.stderr, flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
