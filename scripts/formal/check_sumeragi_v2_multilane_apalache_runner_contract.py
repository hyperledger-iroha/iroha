#!/usr/bin/env python3
"""Negative-control checks for the multilane Apalache runner contract."""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from check_sumeragi_v2_multilane_models import (  # noqa: E402
    APALACHE_RUNNER_RELATIVE,
    DEFAULT_ROOT,
    _apalache_runner_source_errors,
)


def _must_reject(source: str, old: str, new: str, label: str) -> None:
    if old not in source:
        raise AssertionError(f"test fixture cannot find {label}: {old!r}")
    mutated = source.replace(old, new, 1)
    errors = _apalache_runner_source_errors(mutated)
    if not errors:
        raise AssertionError(f"runner contract accepted {label}")


def main() -> int:
    runner = DEFAULT_ROOT / APALACHE_RUNNER_RELATIVE
    source = runner.read_text(encoding="utf-8")
    errors = _apalache_runner_source_errors(source)
    if errors:
        raise AssertionError(
            "canonical runner failed its own contract:\n" + "\n".join(errors)
        )

    _must_reject(
        source,
        'APALACHE_VERSION="0.52.2"',
        'APALACHE_VERSION="latest"',
        "version drift",
    )
    _must_reject(
        source,
        "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7",
        "0" * 64,
        "launcher checksum drift",
    )
    _must_reject(
        source,
        "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a",
        "f" * 64,
        "jar checksum drift",
    )
    _must_reject(
        source,
        'python3 -I -S "$CONTRACT_CHECKER"',
        ": # source binding bypass",
        "source-binding bypass",
    )
    _must_reject(
        source,
        "multilane_autoscale_lifecycle_fixed.cfg \\\n  8 \\",
        "multilane_autoscale_lifecycle_fixed.cfg \\\n  7 \\",
        "autoscale bound reduction",
    )
    _must_reject(
        source,
        "multilane_native_application_evidence_fixed.cfg",
        "multilane_native_frontier_before_sidecars_bug.cfg",
        "mutation substitution",
    )
    _must_reject(
        source,
        "multilane_native_application_evidence_fixed.cfg \\\n  8 \\",
        "multilane_native_application_evidence_fixed.cfg \\\n  7 \\",
        "Native application evidence bound reduction",
    )
    _must_reject(
        source,
        "NativeLegacyDenseRejectedInvariant, NativePruneJournalInvariant",
        "NativeLegacyDenseRejectedInvariant",
        "Native prune-journal invariant removal",
    )
    _must_reject(
        source,
        "multilane_queue_plan_admission_registry_fixed.cfg \\\n  8 \\",
        "multilane_queue_plan_admission_registry_fixed.cfg \\\n  7 \\",
        "queue-plan admission bound reduction",
    )
    _must_reject(
        source,
        "kura_replica_retention_fixed.cfg \\\n" "  8 \\",
        "kura_replica_retention_fixed.cfg \\\n" "  7 \\",
        "Kura retention bound reduction",
    )
    _must_reject(
        source,
        "kura_replica_retention_fixed.cfg",
        "kura_replica_relayed_advert_bug.cfg",
        "Kura retention mutation substitution",
    )
    _must_reject(
        source,
        "inflight_first_release_fixed.cfg \\\n  18 \\",
        "inflight_first_release_fixed.cfg \\\n  17 \\",
        "in-flight layout bound reduction",
    )
    _must_reject(
        source,
        "inflight_first_release_fixed.cfg",
        "inflight_first_release_payload_conflict_bug.cfg",
        "in-flight mutation substitution",
    )
    _must_reject(
        source,
        'grep -Fc "The outcome is: NoError"',
        'grep -Fc "The outcome is:"',
        "weakened outcome marker",
    )
    override_mutation = source + "\nAPALACHE_LENGTH=${APALACHE_LENGTH:-1}\n"
    if not _apalache_runner_source_errors(override_mutation):
        raise AssertionError("runner contract accepted a length override")

    print(
        "Sumeragi v2 multilane Apalache runner contract passed 15 "
        "fail-closed negative controls"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
