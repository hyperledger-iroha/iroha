"""Tests for the strict Kotodama Criterion regression gate."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "check_kotodama_perf", ROOT / "scripts" / "check_kotodama_perf.py"
)
assert SPEC is not None and SPEC.loader is not None
PERF = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = PERF
SPEC.loader.exec_module(PERF)

EXPECTED_DECIMAL_BENCHMARKS = {
    "kotodama_decimal_add",
    "kotodama_decimal_sub",
    "kotodama_decimal_mul",
    "kotodama_decimal_div_exact",
    "kotodama_decimal_div_round_floor",
    "kotodama_decimal_div_round_ceil",
    "kotodama_decimal_div_round_nearest_even",
}
EXPECTED_RUNTIME_PHASE_BENCHMARKS = {
    "kotodama_runtime_phase_prepare_validate_predecode",
    "kotodama_runtime_phase_argument_decode",
    "kotodama_runtime_phase_load_prepared",
    "kotodama_runtime_phase_dirty_reset",
    "kotodama_runtime_phase_execute_prepared",
}
EXPECTED_INTERFACE_PHASE_BENCHMARKS = {
    "kotodama_phase_interface_summary",
}
EXPECTED_CANDIDATE_ONLY_BENCHMARKS = (
    EXPECTED_DECIMAL_BENCHMARKS
    | EXPECTED_INTERFACE_PHASE_BENCHMARKS
    | EXPECTED_RUNTIME_PHASE_BENCHMARKS
)


def write_estimate(path: Path, median: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"median": {"point_estimate": median}}), encoding="utf-8"
    )


def populate(root: Path, sample: str, multiplier: float = 1.0) -> None:
    for index, name in enumerate(PERF.REPRESENTATIVE_BENCHMARKS, start=1):
        write_estimate(
            root / name / sample / "estimates.json", index * 1000.0 * multiplier
        )


class KotodamaPerfGateTests(unittest.TestCase):
    """Exercise baseline capture, strict coverage, and the 5% ceiling."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_gate_accepts_five_percent_and_rejects_more(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new", 1.05)
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 0)

        populate(self.root, "new", 1.051)
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

    def test_gate_fails_closed_on_missing_or_invalid_samples(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        missing = (
            self.root
            / PERF.REPRESENTATIVE_BENCHMARKS[0]
            / "new"
            / "estimates.json"
        )
        missing.unlink()
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

        write_estimate(missing, float("nan"))
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 1)

    def test_checked_in_baseline_roundtrip_and_coverage(self) -> None:
        populate(self.root, "new")
        baseline = self.root / "baseline.json"
        self.assertEqual(
            PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--write-baseline",
                    str(baseline),
                ]
            ),
            0,
        )
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            0,
        )

        payload = json.loads(baseline.read_text(encoding="utf-8"))
        self.assertEqual(
            set(payload["benchmarks"]), set(PERF.REGRESSION_BENCHMARKS)
        )
        self.assertTrue(
            EXPECTED_CANDIDATE_ONLY_BENCHMARKS.isdisjoint(
                payload["benchmarks"]
            )
        )
        del payload["benchmarks"][PERF.REGRESSION_BENCHMARKS[0]]
        baseline.write_text(json.dumps(payload), encoding="utf-8")
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            1,
        )

        payload = {
            "schema": PERF.SCHEMA,
            "unit": "ns",
            "benchmarks": {
                name: 1_000.0 for name in PERF.REGRESSION_BENCHMARKS
            },
        }
        payload["benchmarks"]["unreviewed_benchmark"] = 1_000.0
        baseline.write_text(json.dumps(payload), encoding="utf-8")
        self.assertEqual(
            PERF.main(
                ["--criterion-dir", str(self.root), "--baseline", str(baseline)]
            ),
            1,
        )

    def test_threshold_cannot_be_loosened(self) -> None:
        comparisons = [PERF.Comparison("bench", 100.0, 100.0)]
        with self.assertRaisesRegex(PERF.GateError, "cannot be loosened"):
            PERF.enforce(comparisons, 0.051)

    def test_list_sugar_must_not_be_slower_than_the_manual_loop(self) -> None:
        samples = {
            PERF.LIST_SUGAR_BENCHMARK: 100.0,
            PERF.LIST_MANUAL_BENCHMARK: 100.0,
        }
        PERF.enforce_list_sugar(samples)

        samples[PERF.LIST_SUGAR_BENCHMARK] = 100.1
        with self.assertRaisesRegex(PERF.GateError, "manual-loop baseline"):
            PERF.enforce_list_sugar(samples)

        samples[PERF.LIST_SUGAR_BENCHMARK] = 99.9
        PERF.enforce_list_sugar(samples)

    def test_v1_list_numeric_runtime_phase_and_typed_query_samples_are_required(self) -> None:
        required = {
            PERF.LIST_SUGAR_BENCHMARK,
            PERF.LIST_MANUAL_BENCHMARK,
            "kotodama_list_get_64",
            "kotodama_quantity_add",
            "kotodama_quantity_mul_decimal",
            "kotodama_quantity_div_decimal_exact",
            "kotodama_quantity_div_round_nearest_even",
            *EXPECTED_DECIMAL_BENCHMARKS,
            *EXPECTED_INTERFACE_PHASE_BENCHMARKS,
            "typed_core_query_accounts_page_64",
            "typed_core_query_assets_page_64",
            "typed_core_query_asset_definitions_page_64",
            "typed_core_query_domains_page_64",
            "typed_core_query_nfts_page_64",
            *EXPECTED_RUNTIME_PHASE_BENCHMARKS,
        }
        self.assertLessEqual(required, set(PERF.REPRESENTATIVE_BENCHMARKS))
        self.assertLessEqual(
            required - EXPECTED_CANDIDATE_ONLY_BENCHMARKS,
            set(PERF.REGRESSION_BENCHMARKS),
        )
        self.assertEqual(
            EXPECTED_CANDIDATE_ONLY_BENCHMARKS,
            set(PERF.CANDIDATE_ONLY_BENCHMARKS),
        )
        self.assertTrue(
            EXPECTED_CANDIDATE_ONLY_BENCHMARKS.isdisjoint(
                PERF.REGRESSION_BENCHMARKS
            )
        )

        populate(self.root, "base")
        populate(self.root, "new")
        for name in required:
            with self.subTest(name=name):
                sample = self.root / name / "new" / "estimates.json"
                median = PERF.read_criterion_median(sample)
                sample.unlink()
                self.assertEqual(
                    PERF.main(["--criterion-dir", str(self.root)]), 1
                )
                write_estimate(sample, median)

    def test_candidate_only_benchmark_source_and_policy_cannot_drift(self) -> None:
        self.assertEqual(
            EXPECTED_DECIMAL_BENCHMARKS, set(PERF.DECIMAL_BENCHMARKS)
        )
        self.assertEqual(
            EXPECTED_RUNTIME_PHASE_BENCHMARKS,
            set(PERF.RUNTIME_PHASE_BENCHMARKS),
        )
        self.assertEqual(
            EXPECTED_INTERFACE_PHASE_BENCHMARKS,
            set(PERF.INTERFACE_PHASE_BENCHMARKS),
        )
        self.assertEqual(
            EXPECTED_CANDIDATE_ONLY_BENCHMARKS,
            set(PERF.SOURCE_BOUND_BENCHMARKS),
        )
        PERF.validate_benchmark_policy()

        source = PERF.IVM_BENCHMARK_SOURCE.read_text(encoding="utf-8")
        removed = self.root / "missing_decimal.rs"
        removed.write_text(
            source.replace(
                '"kotodama_decimal_add"', '"untracked_decimal_add"', 1
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            PERF.GateError, "missing: kotodama_decimal_add"
        ):
            PERF.validate_benchmark_policy(
                (removed, *PERF.BENCHMARK_SOURCES[1:])
            )

        added = self.root / "extra_decimal.rs"
        added.write_text(
            source + '\nconst _: &str = "kotodama_decimal_pow";\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            PERF.GateError, "missing from policy: kotodama_decimal_pow"
        ):
            PERF.validate_benchmark_policy((added, *PERF.BENCHMARK_SOURCES[1:]))

    def test_semantic_identity_keeps_its_comparable_workload(self) -> None:
        source = PERF.IVM_BENCHMARK_SOURCE.read_text(encoding="utf-8")

        def benchmark_block(name: str) -> str:
            marker = f'c.bench_function("{name}"'
            start = source.index(marker)
            end = source.find("c.bench_function(", start + len(marker))
            return source[start:] if end < 0 else source[start:end]

        semantic = benchmark_block("kotodama_phase_semantic")
        self.assertIn("resolved.clone()", semantic)
        self.assertIn(".type_effect()", semantic)
        self.assertNotIn("resolve_function_signatures", semantic)

        interface = benchmark_block("kotodama_phase_interface_summary")
        self.assertIn("SemanticContext::new()", interface)
        self.assertIn("resolve_function_signatures", interface)
        self.assertNotIn(".type_effect()", interface)

    def test_revision_inventory_rejects_missing_duplicate_and_misclassified_ids(self) -> None:
        base = self.root / "base.rs"
        candidate = self.root / "candidate.rs"

        def write_inventory(path: Path, names: list[str]) -> None:
            path.write_text(
                "\n".join(f'const _: &str = "{name}";' for name in names),
                encoding="utf-8",
            )

        regression = list(PERF.REGRESSION_BENCHMARKS)
        representative = list(PERF.REPRESENTATIVE_BENCHMARKS)
        write_inventory(base, regression)
        write_inventory(candidate, representative)
        PERF.validate_revision_inventories((base,), (candidate,))

        for revision, path, names, removed, duplicated in (
            ("base", base, regression, regression[0], regression[0]),
            (
                "candidate",
                candidate,
                representative,
                PERF.CANDIDATE_ONLY_BENCHMARKS[0],
                PERF.CANDIDATE_ONLY_BENCHMARKS[0],
            ),
        ):
            with self.subTest(revision=revision, failure="missing"):
                write_inventory(path, [name for name in names if name != removed])
                with self.assertRaisesRegex(PERF.GateError, f"missing: {removed}"):
                    PERF.validate_revision_inventories((base,), (candidate,))
                write_inventory(path, names)

            with self.subTest(revision=revision, failure="duplicate"):
                write_inventory(path, [*names, duplicated])
                with self.assertRaisesRegex(
                    PERF.GateError, f"duplicated: {duplicated}"
                ):
                    PERF.validate_revision_inventories((base,), (candidate,))
                write_inventory(path, names)

        write_inventory(base, [*regression, PERF.CANDIDATE_ONLY_BENCHMARKS[0]])
        with self.assertRaisesRegex(
            PERF.GateError, "candidate-only benchmarks are present in the base"
        ):
            PERF.validate_revision_inventories((base,), (candidate,))

        misclassified = PERF.CANDIDATE_ONLY_BENCHMARKS[0]
        write_inventory(base, regression)
        promoted_candidate_only = tuple(
            name
            for name in PERF.CANDIDATE_ONLY_BENCHMARKS
            if name != misclassified
        )
        promoted_regression = (*PERF.REGRESSION_BENCHMARKS, misclassified)
        with (
            mock.patch.object(
                PERF,
                "CANDIDATE_ONLY_BENCHMARKS",
                promoted_candidate_only,
            ),
            mock.patch.object(
                PERF, "REGRESSION_BENCHMARKS", promoted_regression
            ),
        ):
            with self.assertRaisesRegex(
                PERF.GateError, f"missing: {misclassified}"
            ):
                PERF.validate_revision_inventories((base,), (candidate,))

    def test_only_comparable_benchmarks_require_base_evidence(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        self.assertEqual(
            set(PERF.REGRESSION_BENCHMARKS),
            set(PERF.REPRESENTATIVE_BENCHMARKS)
            - set(PERF.CANDIDATE_ONLY_BENCHMARKS),
        )
        for name in PERF.CANDIDATE_ONLY_BENCHMARKS:
            (self.root / name / "base" / "estimates.json").unlink()
        self.assertEqual(PERF.main(["--criterion-dir", str(self.root)]), 0)

        for name in PERF.REGRESSION_BENCHMARKS:
            with self.subTest(name=name):
                sample = self.root / name / "base" / "estimates.json"
                baseline_median = PERF.read_criterion_median(sample)
                sample.unlink()
                self.assertEqual(
                    PERF.main(["--criterion-dir", str(self.root)]), 1
                )
                write_estimate(sample, baseline_median)

    def test_baseline_capture_and_comparison_are_mutually_exclusive(self) -> None:
        with self.assertRaises(SystemExit):
            PERF.parse_args(
                [
                    "--baseline",
                    "baseline.json",
                    "--write-baseline",
                    "replacement.json",
                ]
            )

    def test_release_workflow_runs_the_complete_artifact_gate(self) -> None:
        workflow = (
            ROOT / ".github" / "workflows" / "kotodama_perf.yml"
        ).read_text(encoding="utf-8")
        self.assertNotIn("Install the candidate benchmark harness", workflow)
        self.assertNotIn(
            "cp candidate/crates/ivm/benches/bench_kotodama.rs", workflow
        )
        self.assertNotRegex(
            workflow,
            r"(?m)^\s*(?:cp|mv|rsync|install)\b[^\n]*"
            r"candidate/crates/ivm/benches/bench_kotodama\.rs",
        )
        inventory_marker = (
            "      - name: Require explicit revision benchmark inventories\n"
        )
        self.assertEqual(workflow.count(inventory_marker), 1)
        inventory_step = workflow.split(inventory_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn('policy["validate_revision_inventories"]', inventory_step)
        self.assertIn(
            "baseline/crates/ivm/benches/bench_kotodama.rs", inventory_step
        )
        self.assertIn(
            "candidate/crates/ivm/benches/bench_kotodama.rs", inventory_step
        )

        base_marker = "      - name: Measure base revision\n"
        self.assertEqual(workflow.count(base_marker), 1)
        self.assertLess(
            workflow.index(inventory_marker), workflow.index(base_marker)
        )
        base_step = workflow.split(base_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn("cargo bench -p ivm --bench bench_kotodama", base_step)
        self.assertIn("--bench queries -- typed_core_query_", base_step)

        candidate_marker = "      - name: Measure candidate revision\n"
        self.assertEqual(workflow.count(candidate_marker), 1)
        candidate_step = workflow.split(candidate_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn("--bench queries -- typed_core_query_", candidate_step)
        self.assertNotIn("KOTODAMA_BASE_FILTER", workflow)

        marker = "      - name: Enforce complete artifact release gate\n"
        self.assertEqual(workflow.count(marker), 1)
        release_gate = workflow.split(marker, 1)[1]
        next_step = release_gate.find("\n      - name:")
        if next_step >= 0:
            release_gate = release_gate[:next_step]

        self.assertIn("scripts/regenerate_kotodama_goldens.py", release_gate)
        self.assertIn("--check", release_gate)
        self.assertIn(
            "--koto ../target-kotodama-perf/debug/koto", release_gate
        )
        self.assertIn(
            "--iroha ../target-kotodama-perf/debug/iroha", release_gate
        )
        self.assertNotIn("--skip-runtime-manifest-check", release_gate)
        self.assertNotIn("--skip-contract-tests", release_gate)

        build_marker = "      - name: Build canonical Kotodama release tools\n"
        self.assertEqual(workflow.count(build_marker), 1)
        build_step = workflow.split(build_marker, 1)[1].split(marker, 1)[0]
        self.assertIn(
            "cargo build -p ivm --bin koto -p iroha_cli --bin iroha",
            build_step,
        )
        self.assertIn("python3 scripts/check_kotodama_docs.py", build_step)
        self.assertIn(
            "--koto ../target-kotodama-perf/debug/koto", build_step
        )
        self.assertIn("bash scripts/check_no_legacy_codec.sh", workflow)
        self.assertIn("npm test --prefix javascript/iroha_js", workflow)


if __name__ == "__main__":
    unittest.main()
