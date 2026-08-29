"""Tests for AtomicPrivateSettlementV1 benchmark evidence reporting."""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_benchmark_report.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_benchmark_report", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def sample(
    profile: str,
    participants: int,
    seed: int,
    run: int,
    warmup: bool,
    scale: float = 1.0,
):
    private_stages = {
        stage: scale * (index + 1)
        for index, stage in enumerate(MODULE.REQUIRED_PRIVATE_STAGES)
    }
    stages = (
        private_stages
        if profile == "private"
        else {
            stage: private_stages[stage] for stage in ("global_finality", "end_to_end")
        }
    )
    return MODULE.Sample(
        commit="a" * 40,
        hardware_sha256="b" * 64,
        configuration_sha256=f"{participants:064x}",
        profile=profile,
        participants=participants,
        seed=seed,
        run=run,
        warmup=warmup,
        stages_ms=stages,
        resources={field: scale * 10 for field in MODULE.RESOURCE_FIELDS},
    )


def complete_matrix(scale: float = 1.0):
    samples: list[Any] = []
    for profile in MODULE.PROFILES:
        for participants in MODULE.REQUIRED_PARTICIPANTS:
            samples.extend(
                sample(profile, participants, 0, run, True, scale)
                for run in range(MODULE.MIN_WARMUPS)
            )
            samples.extend(
                sample(profile, participants, run % 2, run, False, scale)
                for run in range(MODULE.MIN_MEASURED)
            )
    return samples


class PrivateSettlementBenchmarkReportTests(unittest.TestCase):
    """Validate matrix enforcement, statistics, and regression policy."""

    def test_complete_matrix_reports_every_profile_and_participant(self) -> None:
        report = MODULE.build_report(complete_matrix(), 100)
        self.assertEqual(set(report["profiles"]), set(MODULE.PROFILES))
        self.assertEqual(
            set(report["profiles"]["private"]),
            {str(value) for value in MODULE.REQUIRED_PARTICIPANTS},
        )
        self.assertEqual(
            report["profiles"]["private"]["3"]["stages_ms"]["end_to_end"]["count"],
            MODULE.MIN_MEASURED,
        )

    def test_missing_real_network_participant_bucket_is_rejected(self) -> None:
        incomplete = [
            value
            for value in complete_matrix()
            if not (value.profile == "private" and value.participants == 16)
        ]
        with self.assertRaises(MODULE.EvidenceError):
            MODULE.build_report(incomplete, 100)

    def test_baseline_policy_allows_small_shift_and_rejects_large_shift(self) -> None:
        baseline = MODULE.build_report(complete_matrix(1.0), 100)
        small = MODULE.build_report(complete_matrix(1.05), 100)
        large = MODULE.build_report(complete_matrix(1.25), 100)
        self.assertEqual(MODULE.compare_baseline(small, baseline), [])
        regressions = MODULE.compare_baseline(large, baseline)
        self.assertTrue(regressions)
        self.assertTrue({item["quantile"] for item in regressions} >= {"p95", "p99"})

    def test_baseline_from_different_hardware_is_rejected(self) -> None:
        baseline = MODULE.build_report(complete_matrix(1.0), 100)
        candidate = MODULE.build_report(complete_matrix(1.0), 100)
        baseline["environment"]["hardware_sha256"] = "e" * 64
        with self.assertRaisesRegex(
            MODULE.EvidenceError, "identical hardware and configurations"
        ):
            MODULE.compare_baseline(candidate, baseline)

    def test_percentile_and_mad_are_deterministic(self) -> None:
        first = MODULE.summarize_values(
            [1.0, 2.0, 3.0, 4.0], binding=b"fixed", bootstrap_iterations=100
        )
        second = MODULE.summarize_values(
            [1.0, 2.0, 3.0, 4.0], binding=b"fixed", bootstrap_iterations=100
        )
        self.assertEqual(first, second)
        self.assertEqual(first["p50"], 2.5)
        self.assertEqual(first["mad"], 1.0)

    def test_mixed_source_commits_are_rejected(self) -> None:
        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            commit="b" * 40,
            hardware_sha256=original.hardware_sha256,
            configuration_sha256=original.configuration_sha256,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(MODULE.EvidenceError, "one exact source commit"):
            MODULE.build_report(samples, 100)

    def test_mixed_hardware_or_n_configuration_is_rejected(self) -> None:
        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            commit=original.commit,
            hardware_sha256="c" * 64,
            configuration_sha256=original.configuration_sha256,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(
            MODULE.EvidenceError, "one pinned hardware description"
        ):
            MODULE.build_report(samples, 100)

        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            commit=original.commit,
            hardware_sha256=original.hardware_sha256,
            configuration_sha256="d" * 64,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(MODULE.EvidenceError, "one pinned configuration"):
            MODULE.build_report(samples, 100)


if __name__ == "__main__":
    unittest.main()
