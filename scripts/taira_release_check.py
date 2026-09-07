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
    )),
    ("network 369 inventory boundaries", (
        "taira_public_reset::executor_model::tests::inventory_wire_roundtrip_scopes_nonempty_placements_before_decode",
        "taira_public_reset::executor_model::tests::inventory_file_boundary_preserves_original_bytes_and_decode_guard",
    )),
    ("aggregate execution budget before custody", (
        "taira_public_reset::inputs::tests::aggregate_timeout_budget_rejects_assembly_and_authorization_before_input_or_custody_reads",
        "taira_public_reset::inputs::tests::aggregate_timeout_policy_accepts_deployment_defaults_and_preserves_individual_bounds",
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


def run_checks(root: Path) -> None:
    if sys.platform not in {"darwin", "linux"}:
        raise CheckError("the Taira descriptor/stage gate requires macOS or Linux")
    started = time.monotonic()
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root,
                                   stdin=subprocess.DEVNULL, text=True).strip()
    env = os.environ.copy()
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
