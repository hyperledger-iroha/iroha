#!/usr/bin/env python3
"""Run every source guard and report all failures, instead of stopping at the first one.

Requires Python 3.10+, ripgrep and Git. Read-only. Behind `make guards`.
"""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import time
from typing import Callable, Sequence


ROOT = Path(__file__).resolve().parent.parent
GUARDS: tuple[tuple[str, ...], ...] = (
    ("bash", "scripts/deny_serde_json.sh"),
    ("bash", "scripts/check_no_direct_serde.sh"),
    ("bash", "scripts/deny_handrolled_aos.sh"),
    ("bash", "scripts/check_no_legacy_codec.sh"),
    ("python3", "scripts/check_first_release_history.py"),
    ("python3", "scripts/check_zk_x509_presentation_interval.py"),
    ("python3", "scripts/check_zk_resource_contract.py"),
    ("python3", "scripts/fhe_bgv_cost_model.py"),
    ("python3", "crates/iroha_fhe/tests/vectors/bgv_reference.py", "--check"),
    ("python3", "scripts/check_zk_runtime_validity.py"),
    ("python3", "scripts/check_zk_operation_inventory.py"),
    ("python3", "scripts/check_durable_workflow_contract.py"),
    ("python3", "scripts/check_operation_contract.py"),
    ("python3", "scripts/check_zk_economic_invariants.py"),
    ("python3", "scripts/check_zk_primitive_workloads.py"),
    ("python3", "scripts/check_state_commitment_candidates.py"),
    ("bash", "scripts/check_contract_address_chain_identity.sh"),
)
# A failing guard can print a long inventory; the command reproduces all of it.
TAIL_LINES = 12


def run_guard(command: Sequence[str]) -> tuple[int, str]:
    done = subprocess.run(
        list(command),
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return done.returncode, done.stdout


def tail(output: str, lines: int = TAIL_LINES) -> str:
    kept = output.rstrip().splitlines()
    omitted = len(kept) - lines
    shown = kept[-lines:]
    if omitted > 0:
        shown = [f"... {omitted} earlier lines omitted", *shown]
    return "\n".join(f"    {line}" for line in shown)


def run_all(
    guards: Sequence[Sequence[str]] = GUARDS,
    run: Callable[[Sequence[str]], tuple[int, str]] = run_guard,
) -> list[str]:
    """Run each guard, print its outcome as it finishes, and return the failed commands."""
    failed = []
    for command in guards:
        label = " ".join(command)
        started = time.monotonic()
        code, output = run(command)
        seconds = time.monotonic() - started
        if code == 0:
            print(f"ok   {label} ({seconds:.0f}s)", flush=True)
        else:
            failed.append(label)
            print(f"FAIL {label} ({seconds:.0f}s)\n{tail(output)}", flush=True)
    return failed


def main() -> int:
    failed = run_all()
    if failed:
        print(f"\n{len(failed)} of {len(GUARDS)} guards failed:", file=sys.stderr)
        for label in failed:
            print(f"  {label}", file=sys.stderr)
        return 1
    print(f"\nAll {len(GUARDS)} guards passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
