"""Tests for the strict Kotodama Criterion regression gate."""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import textwrap
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
EXPECTED_SOURCE_BOUND_BENCHMARKS = (
    EXPECTED_DECIMAL_BENCHMARKS
    | EXPECTED_INTERFACE_PHASE_BENCHMARKS
    | EXPECTED_RUNTIME_PHASE_BENCHMARKS
)


def write_estimate(path: Path, median: float) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps({"median": {"point_estimate": median, "confidence_interval": {
            "confidence_level": 0.95, "lower_bound": median, "upper_bound": median,
        }}}), encoding="utf-8"
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

    def run_gate(self) -> int:
        """Run sample checks with the provenance boundary mocked as satisfied."""

        baseline = self.root / "baseline"
        with mock.patch.object(
            PERF, "validate_baseline_provenance"
        ) as validate, mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}):
            result = PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--baseline-root",
                    str(baseline),
                ]
            )
        validate.assert_called_once_with(baseline)
        return result

    def test_gate_accepts_five_percent_and_rejects_more(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new", 1.05)
        self.assertEqual(self.run_gate(), 0)

        populate(self.root, "new", 1.051)
        self.assertEqual(self.run_gate(), 1)

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
        self.assertEqual(self.run_gate(), 1)

        write_estimate(missing, float("nan"))
        self.assertEqual(self.run_gate(), 1)

    def test_criterion_confidence_intervals_are_mandatory_and_validated(self) -> None:
        path = self.root / "estimates.json"
        valid = {"confidence_level": 0.95, "lower_bound": 95.0, "upper_bound": 105.0}
        for field, value in (
            ("confidence_level", 0.9), ("confidence_level", 1.0),
            ("confidence_level", True), ("lower_bound", 0),
            ("lower_bound", float("nan")), ("lower_bound", 101.0),
            ("upper_bound", 99.0), ("upper_bound", float("inf")),
            ("upper_bound", "105"),
        ):
            with self.subTest(field=field, value=value):
                path.write_text(json.dumps({"median": {"point_estimate": 100.0,
                    "confidence_interval": {**valid, field: value}}}))
                with self.assertRaises(PERF.GateError):
                    PERF.read_criterion_estimate(path)
        path.write_text(json.dumps({"median": {"point_estimate": 100.0}}))
        with self.assertRaisesRegex(PERF.GateError, "missing median confidence"):
            PERF.read_criterion_median(path)
        path.write_text(json.dumps({"median": {"point_estimate": 100.0,
            "confidence_interval": valid}}))
        self.assertEqual(PERF.read_criterion_estimate(path),
                         PERF.MedianEstimate(100.0, 95.0, 105.0, 0.95))

    def test_uncertain_regression_cannot_qualify_or_write_a_receipt(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        name = PERF.REGRESSION_BENCHMARKS[0]
        path = self.root / name / "base" / "estimates.json"
        payload = json.loads(path.read_text())
        payload["median"]["confidence_interval"]["lower_bound"] *= 0.9
        path.write_text(json.dumps(payload))
        report = self.root / "report.json"
        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}),
            mock.patch.object(PERF, "validate_candidate_provenance") as candidate,
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()) as stderr,
        ):
            result = PERF.main([
                "--criterion-dir", str(self.root), "--baseline-root", "baseline",
                "--candidate-root", "candidate", "--expected-candidate-commit", "c" * 40,
                "--json-output", str(report),
            ])
        self.assertEqual(result, 1)
        self.assertIn("inconclusive confidence bounds: no performance qualification", stderr.getvalue())
        candidate.assert_not_called()
        self.assertFalse(report.exists())

    def test_confidence_extremes_keep_exact_five_percent_boundary(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        path = self.root / PERF.REGRESSION_BENCHMARKS[0] / "new" / "estimates.json"
        payload = json.loads(path.read_text())
        point = payload["median"]["point_estimate"]
        payload["median"]["confidence_interval"]["upper_bound"] = point * 1.05
        path.write_text(json.dumps(payload))
        PERF.enforce_confidence_bounds(self.root, 0.05)
        payload["median"]["confidence_interval"]["upper_bound"] = point * 1.051
        path.write_text(json.dumps(payload))
        with self.assertRaisesRegex(PERF.GateError, "inconclusive"):
            PERF.enforce_confidence_bounds(self.root, 0.05)

    def test_list_sugar_keeps_median_equality_with_reported_interval_overlap(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        for name in (PERF.LIST_SUGAR_BENCHMARK, PERF.LIST_MANUAL_BENCHMARK):
            for sample in ("base", "new"):
                write_estimate(self.root / name / sample / "estimates.json", 100.0)
        PERF.enforce_confidence_bounds(self.root, 0.05)
        path = self.root / PERF.LIST_SUGAR_BENCHMARK / "new" / "estimates.json"
        payload = json.loads(path.read_text())
        payload["median"]["confidence_interval"]["upper_bound"] = 100.001
        path.write_text(json.dumps(payload))
        report = PERF.enforce_confidence_bounds(self.root, 0.05)["list_sugar"]
        self.assertEqual(report["decision_basis"], "point_median_zero_slowdown")
        self.assertEqual(report["interval_relation"], "overlapping_or_touching_intervals")
        self.assertGreater(report["maximum_change"], 0.0)
        self.assertEqual(self.run_gate(), 0)
        payload["median"]["point_estimate"] = 100.001
        path.write_text(json.dumps(payload))
        self.assertEqual(self.run_gate(), 1)

    def test_portable_and_self_baseline_cli_paths_are_rejected(self) -> None:
        common = ["--baseline-root", str(self.root / "baseline")]
        for option in ("--baseline", "--write-baseline"):
            with self.subTest(option=option):
                with (
                    contextlib.redirect_stderr(io.StringIO()),
                    self.assertRaises(SystemExit),
                ):
                    PERF.parse_args([*common, option, str(self.root / "baseline.json")])
        self.assertFalse(hasattr(PERF, "read_baseline"))
        self.assertFalse(hasattr(PERF, "write_baseline"))

    def test_threshold_cannot_be_loosened(self) -> None:
        comparisons = [PERF.Comparison("bench", 100.0, 100.0)]
        with self.assertRaisesRegex(PERF.GateError, "cannot be loosened"):
            PERF.enforce(comparisons, 0.051)

    def test_gate_fails_before_comparison_when_lock_policy_is_unset(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        stdout = io.StringIO()
        stderr = io.StringIO()
        self.assertEqual(
            "1ce796ba4fe8769f4d298a0ce9fe52acff39a012f5ce6cf4323fbd50c2028b55",
            PERF.BASELINE_CARGO_LOCK_SHA256,
        )
        with (
            mock.patch.object(PERF, "BASELINE_CARGO_LOCK_SHA256", None),
            contextlib.redirect_stdout(stdout),
            contextlib.redirect_stderr(stderr),
        ):
            result = PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--baseline-root",
                    str(self.root / "baseline"),
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn(
            "authenticated baseline Cargo.lock provenance is unavailable",
            stderr.getvalue(),
        )
        self.assertNotIn("benchmark | baseline ns", stdout.getvalue())
        self.assertNotIn("within the 5% V1 budget", stdout.getvalue())

    def test_baseline_provenance_rejects_identity_and_source_drift(
        self,
    ) -> None:
        baseline = self.root / "baseline"
        baseline.mkdir()
        lock = baseline / "Cargo.lock"
        lock.write_bytes(b"authenticated baseline lock fixture\n")
        lock_digest = hashlib.sha256(lock.read_bytes()).hexdigest()
        candidates = (self.root / "candidate.rs",)

        with mock.patch.object(
            PERF, "BASELINE_CARGO_LOCK_SHA256", "0" * 63
        ):
            with self.assertRaisesRegex(PERF.GateError, "lowercase 64-hex"):
                PERF.validate_baseline_provenance(baseline, candidates)

        with (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", lock_digest
            ),
            mock.patch.object(PERF, "_git_head", return_value="f" * 40),
        ):
            with self.assertRaisesRegex(PERF.GateError, "revision mismatch"):
                PERF.validate_baseline_provenance(baseline, candidates)

        with (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", lock_digest
            ),
            mock.patch.object(
                PERF, "_git_head", return_value=PERF.BASELINE_SHA
            ),
            mock.patch.object(
                PERF,
                "_require_git_paths_clean",
                side_effect=PERF.GateError("source drift"),
            ),
        ):
            with self.assertRaisesRegex(PERF.GateError, "source drift"):
                PERF.validate_baseline_provenance(baseline, candidates)

        with (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", "1" * 64
            ),
            mock.patch.object(
                PERF, "_git_head", return_value=PERF.BASELINE_SHA
            ),
            mock.patch.object(PERF, "_require_git_paths_clean"),
        ):
            with self.assertRaisesRegex(PERF.GateError, "digest mismatch"):
                PERF.validate_baseline_provenance(baseline, candidates)

        inventory = mock.Mock()
        with (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", lock_digest
            ),
            mock.patch.object(
                PERF, "_git_head", return_value=PERF.BASELINE_SHA
            ),
            mock.patch.object(PERF, "_require_git_paths_clean") as clean,
            mock.patch.object(
                PERF, "validate_revision_inventories", inventory
            ),
        ):
            PERF.validate_baseline_provenance(baseline, candidates)
        clean.assert_called_once_with(baseline.resolve(), ())
        inventory.assert_called_once_with(
            base_sources=tuple(
                baseline.resolve() / relative
                for relative in PERF.BENCHMARK_SOURCE_PATHS
            ),
            candidate_sources=candidates,
        )

    def test_baseline_provenance_rejects_missing_or_symlink_lock(self) -> None:
        baseline = self.root / "baseline"
        baseline.mkdir()
        expected = hashlib.sha256(b"lock").hexdigest()
        shared_patches = (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", expected
            ),
            mock.patch.object(
                PERF, "_git_head", return_value=PERF.BASELINE_SHA
            ),
            mock.patch.object(PERF, "_require_git_paths_clean"),
        )
        with (
            shared_patches[0],
            shared_patches[1],
            shared_patches[2],
        ):
            with self.assertRaisesRegex(PERF.GateError, "regular, non-symlink"):
                PERF.validate_baseline_provenance(baseline)

        target = self.root / "archived.lock"
        target.write_bytes(b"lock")
        try:
            (baseline / "Cargo.lock").symlink_to(target)
        except OSError as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with (
            mock.patch.object(
                PERF, "BASELINE_CARGO_LOCK_SHA256", expected
            ),
            mock.patch.object(
                PERF, "_git_head", return_value=PERF.BASELINE_SHA
            ),
            mock.patch.object(PERF, "_require_git_paths_clean"),
        ):
            with self.assertRaisesRegex(PERF.GateError, "regular, non-symlink"):
                PERF.validate_baseline_provenance(baseline)

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
        self.assertLessEqual(required, set(PERF.REGRESSION_BENCHMARKS))
        self.assertEqual(
            set(PERF.REPRESENTATIVE_BENCHMARKS),
            set(PERF.REGRESSION_BENCHMARKS),
        )
        self.assertEqual(46, len(PERF.REGRESSION_BENCHMARKS))
        self.assertEqual(
            set(PERF.REGRESSION_BENCHMARKS),
            set(PERF.BASELINE_TIMED_BODY_SHA256),
        )
        self.assertEqual(
            "06912ee4f8f9423b458d72039dbd0a1c8fde7b43",
            PERF.BASELINE_SHA,
        )

        populate(self.root, "base")
        populate(self.root, "new")
        for name in required:
            with self.subTest(name=name):
                sample = self.root / name / "new" / "estimates.json"
                median = PERF.read_criterion_median(sample)
                sample.unlink()
                self.assertEqual(self.run_gate(), 1)
                write_estimate(sample, median)

    def test_source_bound_benchmark_source_and_policy_cannot_drift(self) -> None:
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
            EXPECTED_SOURCE_BOUND_BENCHMARKS,
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

    def test_all_comparable_workload_classes_reject_timed_body_drift(self) -> None:
        def mutate_after(
            source: str, anchor: str, old: str, new: str
        ) -> str:
            anchor_index = source.index(anchor)
            old_index = source.find(old, anchor_index)
            self.assertGreaterEqual(old_index, 0, f"{old!r} after {anchor!r}")
            return source[:old_index] + new + source[old_index + len(old) :]

        mutations = (
            (
                "parse",
                0,
                '"kotodama_phase_parse"',
                ".parse()",
                ".resolve()",
            ),
            (
                "semantic",
                0,
                '"kotodama_phase_semantic"',
                ".type_effect()",
                ".lower_ir()",
            ),
            (
                "lowering",
                0,
                '"kotodama_phase_ir_lower"',
                ".lower_ir()",
                ".construct_ssa()",
            ),
            (
                "list-native",
                0,
                '"kotodama_list_get_64"',
                "get_words(&vm, handle, layout, 62)",
                "get_words(&vm, handle, layout, 61)",
            ),
            (
                "list-sugar",
                0,
                '"kotodama_list_comprehension_runtime_64"',
                "sugar_vm.register(10)",
                "sugar_vm.register(11)",
            ),
            (
                "quantity-direct",
                0,
                '"kotodama_quantity_add"',
                ".checked_add(&add_rhs)",
                ".checked_sub(&add_rhs)",
            ),
            (
                "quantity-shared",
                0,
                '"kotodama_quantity_div_round_floor"',
                ".try_div_decimal_round(",
                ".try_div_decimal_exact(",
            ),
            (
                "quantity-mode-binding",
                0,
                '"kotodama_quantity_div_round_floor"',
                "RoundingMode::Floor",
                "RoundingMode::Ceil",
            ),
            (
                "typed-query",
                2,
                PERF.TYPED_QUERY_BENCHMARK_MARKER,
                "host.reset_core_query_page_metrics();",
                "std::hint::black_box(0); host.reset_core_query_page_metrics();",
            ),
            (
                "typed-query-family-binding",
                2,
                '"typed_core_query_accounts_page_64"',
                "CoreQueryEntityTagV1::Account",
                "CoreQueryEntityTagV1::Domain",
            ),
            (
                "ivm-runtime",
                0,
                '"kotodama_runtime_warm_add"',
                "std::hint::black_box(vm.register(10));",
                "std::hint::black_box(vm.register(11));",
            ),
            (
                "core-runtime",
                1,
                '"kotodama_core_runtime_warm_add"',
                "std::hint::black_box(runtime.register(10));",
                "std::hint::black_box(runtime.register(11));",
            ),
        )

        for label, source_index, anchor, old, new in mutations:
            with self.subTest(workload_class=label):
                sources = list(PERF.BENCHMARK_SOURCES)
                original = sources[source_index].read_text(encoding="utf-8")
                mutated = self.root / f"{label}.rs"
                mutated.write_text(
                    mutate_after(original, anchor, old, new),
                    encoding="utf-8",
                )
                sources[source_index] = mutated
                with self.assertRaisesRegex(
                    PERF.GateError, "comparable timed-body drift"
                ):
                    PERF.validate_benchmark_policy(tuple(sources))

    def test_timed_body_policy_rejects_missing_or_unpinned_coverage(self) -> None:
        PERF.validate_benchmark_policy()
        self.assertEqual(
            set(PERF.REGRESSION_BENCHMARKS),
            set(PERF._benchmark_timed_bodies(PERF.BENCHMARK_SOURCES, "candidate")),
        )

        missing = dict(PERF.BASELINE_TIMED_BODY_SHA256)
        missing.pop(PERF.REGRESSION_BENCHMARKS[0])
        with mock.patch.object(PERF, "BASELINE_TIMED_BODY_SHA256", missing):
            with self.assertRaisesRegex(PERF.GateError, "coverage is incomplete"):
                PERF.validate_benchmark_policy()

    def test_typed_query_raw_entity_baseline_drift_is_not_comparable(
        self,
    ) -> None:
        query_source = PERF.BENCHMARK_SOURCES[2]
        original = query_source.read_text(encoding="utf-8")
        suffix = "    });\n    (state, authority, raw_query_response_bytes)"
        self.assertEqual(original.count(suffix), 1)
        mutated = self.root / "inflated-raw-query-baseline.rs"
        mutated.write_text(
            original.replace(
                suffix,
                "    }).map(|bytes| bytes.saturating_add(1_000_000));\n"
                "    (state, authority, raw_query_response_bytes)",
                1,
            ),
            encoding="utf-8",
        )
        candidate_sources = (
            PERF.BENCHMARK_SOURCES[0],
            PERF.BENCHMARK_SOURCES[1],
            mutated,
        )

        # The candidate still has every required benchmark and timed closure;
        # baseline comparison must independently bind the raw-entity size
        # baseline used by the projection assertion.
        PERF.validate_benchmark_policy(candidate_sources)
        with self.assertRaisesRegex(
            PERF.GateError, "raw-entity baseline drift"
        ):
            PERF.validate_typed_query_raw_entity_contract(
                (query_source,), (mutated,)
            )

    def test_core_runtime_explicit_max_heap_preserves_baseline_identity(
        self,
    ) -> None:
        name = PERF.CORE_RUNTIME_WARM_BENCHMARK
        bodies = PERF._benchmark_timed_bodies(
            PERF.BENCHMARK_SOURCES, "candidate"
        )
        body = bodies[name]
        self.assertIn(PERF.CORE_RUNTIME_BASELINE_CHECKOUT, body)
        self.assertNotIn(PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT, body)
        self.assertEqual(
            hashlib.sha256(body.encode("utf-8")).hexdigest(),
            PERF.BASELINE_TIMED_BODY_SHA256[name],
        )

        sources = list(PERF.BENCHMARK_SOURCES)
        original = sources[1].read_text(encoding="utf-8")
        anchor = original.index(f'"{name}"')
        checkout = original.index(
            PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT, anchor
        )
        changed = PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT.replace(
            "ivm::Memory::HEAP_MAX_SIZE",
            "ivm::Memory::HEAP_MAX_SIZE / 2",
        )
        mutated = self.root / "core-runtime-smaller-heap.rs"
        mutated.write_text(
            original[:checkout]
            + changed
            + original[
                checkout + len(PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT) :
            ],
            encoding="utf-8",
        )
        sources[1] = mutated
        with self.assertRaisesRegex(
            PERF.GateError, "baseline-equivalent heap limit"
        ):
            PERF.validate_benchmark_policy(tuple(sources))

    def test_core_runtime_heap_canonicalization_is_strictly_scoped(
        self,
    ) -> None:
        name = PERF.CORE_RUNTIME_WARM_BENCHMARK
        original = PERF.BENCHMARK_SOURCES[1].read_text(encoding="utf-8")
        anchor = original.index(f'"{name}"')
        checkout = original.index(
            PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT, anchor
        )
        prefix = original[:checkout]
        suffix = original[
            checkout + len(PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT) :
        ]
        mutations = {
            "altered-gas": PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT.replace(
                "GAS_LIMIT", "GAS_LIMIT - 1"
            ),
            "duplicate-checkout": (
                PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT
                + "; summary"
                + PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT
            ),
        }
        for label, replacement in mutations.items():
            with self.subTest(case=label):
                mutated = self.root / f"core-runtime-{label}.rs"
                mutated.write_text(
                    prefix + replacement + suffix,
                    encoding="utf-8",
                )
                sources = list(PERF.BENCHMARK_SOURCES)
                sources[1] = mutated
                with self.assertRaises(PERF.GateError):
                    PERF.validate_benchmark_policy(tuple(sources))

        other_original = PERF.BENCHMARK_SOURCES[0].read_text(encoding="utf-8")
        other_anchor = other_original.index('"kotodama_runtime_warm_add"')
        old = "std::hint::black_box(vm.register(10));"
        old_index = other_original.index(old, other_anchor)
        mutated = self.root / "other-runtime-explicit-max.rs"
        mutated.write_text(
            other_original[:old_index]
            + "let _ = summary"
            + PERF.CORE_RUNTIME_EXPLICIT_MAX_CHECKOUT
            + "; "
            + old
            + other_original[old_index + len(old) :],
            encoding="utf-8",
        )
        sources = list(PERF.BENCHMARK_SOURCES)
        sources[0] = mutated
        with self.assertRaisesRegex(
            PERF.GateError, "comparable timed-body drift"
        ):
            PERF.validate_benchmark_policy(tuple(sources))

    def test_revision_inventory_rejects_missing_duplicate_and_misclassified_ids(self) -> None:
        timed_body_policy = mock.patch.object(
            PERF,
            "validate_comparable_timed_bodies",
            return_value={name: name for name in PERF.REGRESSION_BENCHMARKS},
        )
        timed_body_policy.start()
        self.addCleanup(timed_body_policy.stop)
        raw_entity_policy = mock.patch.object(
            PERF, "_typed_query_raw_entity_contract", return_value="raw"
        )
        raw_entity_policy.start()
        self.addCleanup(raw_entity_policy.stop)
        base = self.root / "base.rs"
        candidate = self.root / "candidate.rs"

        typed_body = """
        c.bench_function(family.benchmark_name, |b| {
            b.iter_batched(
                || (),
                |mut vm| {
                    let gas = host
                        .syscall(ivm::syscalls::SYSCALL_CORE_QUERY_PAGE, &mut vm);
                    let items =
                        ivm::list::read_words(&vm, vm.register(10), page_layout);
                    assert_eq!(items.len(), QUERY_PAGE_CAPACITY_V1);
                    let metrics = host.core_query_page_metrics();
                    assert_eq!(metrics.host_queries, 1);
                    assert_eq!(metrics.projection_decodes, 1);
                    assert!(metrics.leaf_tlv_bytes > 0);
                    assert!(metrics.projection_payload_bytes < raw_query_response_bytes);
                    std::hint::black_box((gas, items, vm.register(11), metrics));
                },
                BatchSize::SmallInput,
            )
        });
        """

        def write_inventory(
            path: Path, names: list[str], body: str = typed_body
        ) -> None:
            path.write_text(
                "\n".join(f'const _: &str = "{name}";' for name in names)
                + body,
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
                representative[-1],
                representative[-1],
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

        write_inventory(base, regression)
        drifted_body = typed_body.replace(
            "let metrics = host.core_query_page_metrics();",
            "std::hint::black_box(0);\n"
            "let metrics = host.core_query_page_metrics();",
            1,
        )
        write_inventory(candidate, representative, drifted_body)
        with self.assertRaisesRegex(
            PERF.GateError, "typed-query timed body drift"
        ):
            PERF.validate_revision_inventories((base,), (candidate,))

        missing_contract_body = typed_body.replace(
            "assert_eq!(metrics.host_queries, 1);", "", 1
        )
        write_inventory(candidate, representative, missing_contract_body)
        with self.assertRaisesRegex(
            PERF.GateError, "timed contract is missing or reordered"
        ):
            PERF.validate_revision_inventories((base,), (candidate,))

    def test_every_required_benchmark_requires_base_evidence(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        self.assertEqual(
            set(PERF.REGRESSION_BENCHMARKS),
            set(PERF.REPRESENTATIVE_BENCHMARKS),
        )
        for name in PERF.REGRESSION_BENCHMARKS:
            with self.subTest(name=name):
                sample = self.root / name / "base" / "estimates.json"
                baseline_median = PERF.read_criterion_median(sample)
                sample.unlink()
                self.assertEqual(self.run_gate(), 1)
                write_estimate(sample, baseline_median)

    def test_baseline_root_is_required(self) -> None:
        with (
            contextlib.redirect_stderr(io.StringIO()),
            self.assertRaises(SystemExit),
        ):
            PERF.parse_args([])

    def test_create_only_evidence_report_binds_candidate_and_estimates(
        self,
    ) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        baseline = self.root / "baseline"
        candidate = self.root / "candidate"
        evidence = self.root / "evidence"
        evidence.mkdir()
        report_path = evidence / "report.json"
        candidate_revision = "c" * 40
        candidate_lock = "d" * 64

        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}),
            mock.patch.object(
                PERF,
                "validate_candidate_provenance",
                return_value=(candidate_revision, candidate_lock),
            ) as validate_candidate,
            contextlib.redirect_stdout(io.StringIO()),
        ):
            result = PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--baseline-root",
                    str(baseline),
                    "--candidate-root",
                    str(candidate),
                    "--expected-candidate-commit",
                    candidate_revision,
                    "--json-output",
                    str(report_path),
                ]
            )
        self.assertEqual(result, 0)
        validate_candidate.assert_called_once_with(candidate, candidate_revision)
        report = json.loads(report_path.read_text(encoding="utf-8"))
        self.assertEqual(report["schema"], PERF.EVIDENCE_SCHEMA)
        self.assertEqual(report["threshold"], PERF.MAX_REGRESSION)
        bounds = report["confidence_bounds"]
        self.assertEqual(bounds["minimum_individual_confidence_level"], 0.95)
        self.assertEqual(len(bounds["comparisons"]), len(PERF.REGRESSION_BENCHMARKS))
        self.assertTrue(all(row["maximum_change"] <= 0.05 for row in bounds["comparisons"]))
        self.assertEqual(bounds["list_sugar"]["decision_basis"], "point_median_zero_slowdown")
        self.assertEqual(bounds["list_sugar"]["interval_relation"], "sugar_interval_below_manual")
        self.assertEqual(
            report["benchmark_count"], len(PERF.REGRESSION_BENCHMARKS)
        )
        self.assertEqual(
            report["criterion_estimates_sha256"],
            PERF.criterion_estimates_sha256(self.root),
        )
        self.assertEqual(report["baseline"]["revision"], PERF.BASELINE_SHA)
        self.assertEqual(
            report["baseline"]["cargo_lock_sha256"],
            PERF.BASELINE_CARGO_LOCK_SHA256,
        )
        self.assertEqual(report["derived_query_drivers"], {"base": {}, "candidate": {}})
        self.assertEqual(report["candidate"]["revision"], candidate_revision)
        self.assertEqual(
            report["candidate"]["cargo_lock_sha256"], candidate_lock
        )
        self.assertEqual(
            [row["name"] for row in report["comparisons"]],
            sorted(PERF.REGRESSION_BENCHMARKS),
        )

        original = report_path.read_bytes()
        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}),
            mock.patch.object(
                PERF,
                "validate_candidate_provenance",
                return_value=(candidate_revision, candidate_lock),
            ),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            self.assertEqual(
                PERF.main(
                    [
                        "--criterion-dir",
                        str(self.root),
                        "--baseline-root",
                        str(baseline),
                        "--candidate-root",
                        str(candidate),
                        "--expected-candidate-commit",
                        candidate_revision,
                        "--json-output",
                        str(report_path),
                    ]
                ),
                1,
            )
        self.assertEqual(report_path.read_bytes(), original)

    def test_missing_derived_driver_never_compares_or_qualifies_samples(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        report = self.root / "report.json"
        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "read_current_samples") as samples,
            contextlib.redirect_stdout(io.StringIO()) as stdout,
            contextlib.redirect_stderr(io.StringIO()) as stderr,
        ):
            result = PERF.main([
                "--criterion-dir", str(self.root), "--baseline-root", str(self.root / "absent-base"),
                "--candidate-root", str(ROOT), "--expected-candidate-commit", "c" * 40,
                "--json-output", str(report),
            ])
        self.assertEqual(result, 1)
        self.assertIn("derived query driver evidence is invalid", stderr.getvalue())
        self.assertNotIn("benchmark | baseline ns", stdout.getvalue())
        samples.assert_not_called()
        self.assertFalse(report.exists())

    def test_evidence_arguments_are_all_or_nothing(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()) as stderr,
        ):
            result = PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--baseline-root",
                    str(self.root / "baseline"),
                    "--json-output",
                    str(self.root / "report.json"),
                ]
            )
        self.assertEqual(result, 1)
        self.assertIn("must be supplied together", stderr.getvalue())

    def test_failed_budget_never_writes_or_authenticates_evidence(self) -> None:
        populate(self.root, "base")
        populate(self.root, "new", 1.051)
        evidence = self.root / "evidence"
        evidence.mkdir()
        report = evidence / "report.json"
        with (
            mock.patch.object(PERF, "validate_baseline_provenance"),
            mock.patch.object(PERF, "validate_query_drivers", return_value={"base": {}, "candidate": {}}),
            mock.patch.object(
                PERF, "validate_candidate_provenance"
            ) as validate_candidate,
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            result = PERF.main(
                [
                    "--criterion-dir",
                    str(self.root),
                    "--baseline-root",
                    str(self.root / "baseline"),
                    "--candidate-root",
                    str(self.root / "candidate"),
                    "--expected-candidate-commit",
                    "c" * 40,
                    "--json-output",
                    str(report),
                ]
            )
        self.assertEqual(result, 1)
        self.assertFalse(report.exists())
        validate_candidate.assert_not_called()

    def test_evidence_digest_rejects_intermediate_directory_symlink(
        self,
    ) -> None:
        populate(self.root, "base")
        populate(self.root, "new")
        name = PERF.REGRESSION_BENCHMARKS[0]
        sample = self.root / name / "base"
        outside = self.root / "outside-base"
        sample.rename(outside)
        sample.symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(
            PERF.GateError, "parent must be a regular, non-symlink directory"
        ):
            PERF.criterion_estimates_sha256(self.root)

    def test_candidate_provenance_requires_exact_clean_revision(self) -> None:
        candidate = self.root / "candidate"
        candidate.mkdir()
        subprocess.run(["git", "init", "-q"], cwd=candidate, check=True)
        subprocess.run(
            ["git", "config", "user.name", "Kotodama test"],
            cwd=candidate,
            check=True,
        )
        subprocess.run(
            ["git", "config", "user.email", "kotodama@example.invalid"],
            cwd=candidate,
            check=True,
        )
        lock = candidate / "Cargo.lock"
        lock.write_bytes(b"exact candidate lock\n")
        subprocess.run(["git", "add", "Cargo.lock"], cwd=candidate, check=True)
        subprocess.run(
            ["git", "commit", "-qm", "candidate fixture"],
            cwd=candidate,
            check=True,
        )
        revision = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=candidate,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        ).stdout.strip()
        lock_digest = hashlib.sha256(lock.read_bytes()).hexdigest()
        with self.assertRaisesRegex(PERF.GateError, "Cargo.lock digest mismatch"):
            PERF.validate_candidate_provenance(candidate, revision)
        with mock.patch.object(
            PERF, "CANDIDATE_CARGO_LOCK_SHA256", lock_digest
        ):
            self.assertEqual(
                PERF.validate_candidate_provenance(candidate, revision),
                (revision, lock_digest),
            )

        with self.assertRaisesRegex(PERF.GateError, "revision mismatch"):
            PERF.validate_candidate_provenance(candidate, "f" * 40)
        (candidate / "untracked").write_text("drift\n", encoding="utf-8")
        with (
            mock.patch.object(
                PERF, "CANDIDATE_CARGO_LOCK_SHA256", lock_digest
            ),
            self.assertRaisesRegex(PERF.GateError, "source drift"),
        ):
            PERF.validate_candidate_provenance(candidate, revision)

    def test_measurement_shell_preserves_original_warning_policy_and_commands(self) -> None:
        """Execute both actual steps, preserving diagnostics and every failure status."""
        workflow = (ROOT / ".github/workflows/kotodama_perf.yml").read_text()
        for revision in ("base", "candidate"):
            marker = f"      - name: Measure {revision} revision\n"
            step = workflow.split(marker, 1)[1].split("\n      - name:", 1)[0]
            command = textwrap.dedent(step.split("        run: |\n", 1)[1]).replace("${{ github.sha }}", "c" * 40)
            expected = [
                ["bench", "--locked", "--jobs", "1", "-p", "ivm", "--bench",
                 "bench_kotodama", "--", "--save-baseline", revision],
                ["bench", "--locked", "--jobs", "1", "-p", "iroha_core", "--bench",
                 "kotodama_runtime_cache", "--", "--save-baseline", revision],
                ["../candidate/scripts/kotodama_query_driver.py" if revision == "base" else "scripts/kotodama_query_driver.py",
                 "--revision-root", ".", "--candidate-root", "../candidate" if revision == "base" else ".",
                 "--expected-candidate-commit", "c" * 40, "--label", revision,
                 "--output", str(self.root / "target/kotodama-query-drivers" / revision)],
            ]
            for failure_at in (0, 1, 2, 3):
                with self.subTest(revision=revision, failure_at=failure_at):
                    root = self.root / f"{revision}-{failure_at}"
                    root.mkdir()
                    bin_dir = root / "bin"
                    bin_dir.mkdir()
                    receipt = root / "calls.jsonl"
                    cargo = bin_dir / "cargo"
                    cargo.write_text(
                        f"#!{sys.executable}\n"
                        "import json, os, pathlib, sys\n"
                        "receipt = pathlib.Path(os.environ['TEST_CARGO_RECEIPT'])\n"
                        "count = len(receipt.read_text().splitlines()) if receipt.exists() else 0\n"
                        "keys = ('CARGO_BUILD_WARNINGS', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_JOBS', "
                        "'CARGO_INCREMENTAL', 'CARGO_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')\n"
                        "with receipt.open('a') as out:\n"
                        "    out.write(json.dumps({'argv': sys.argv[1:], 'cwd': os.getcwd(), "
                        "'environment': {key: os.environ[key] for key in keys if key in os.environ}}) + '\\n')\n"
                        "print('cargo output')\n"
                        "print('warning: retained compiler diagnostic', file=sys.stderr)\n"
                        "raise SystemExit(23 if count + 1 == int(os.environ['TEST_FAIL_AT']) else 0)\n"
                    )
                    cargo.chmod(0o700)
                    # The derived driver itself has executable/compiler controls
                    # in kotodama_query_driver_test.py; here its shell invocation
                    # must retain argv, environment and failure propagation.
                    driver_python = bin_dir / "python3"
                    driver_python.write_bytes(cargo.read_bytes())
                    driver_python.chmod(0o700)
                    retained = {
                        "RUSTUP_TOOLCHAIN": "1.99.1", "CARGO_BUILD_JOBS": "1",
                        "CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(self.root / "target"),
                        "RUSTFLAGS": "--cfg retained_warning_policy",
                        "CARGO_ENCODED_RUSTFLAGS": "-C\x1fdebuginfo=1",
                    }
                    environment = {
                        **os.environ, **retained, "CARGO_BUILD_WARNINGS": "deny",
                        "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"],
                        "TEST_CARGO_RECEIPT": str(receipt), "TEST_FAIL_AT": str(failure_at),
                    }
                    result = subprocess.run(["bash", "-e", "-c", command], cwd=root,
                                            env=environment, text=True, capture_output=True)
                    calls = [json.loads(line) for line in receipt.read_text().splitlines()]
                    count = failure_at or 3
                    self.assertEqual(result.returncode, 23 if failure_at else 0)
                    self.assertEqual([call["argv"] for call in calls], expected[:count])
                    self.assertEqual(result.stdout, "cargo output\n" * count)
                    self.assertEqual(result.stderr, "warning: retained compiler diagnostic\n" * count)
                    for call in calls:
                        self.assertEqual(call["cwd"], str(root.resolve()))
                        self.assertNotIn("CARGO_BUILD_WARNINGS", call["environment"])
                        for name, value in retained.items():
                            self.assertEqual(call["environment"][name], value)

    def test_hosted_budget_and_partial_diagnostics_preserve_qualification(self) -> None:
        """Cold builds may finish or fail without reusing or qualifying cached samples."""
        workflow = (ROOT / ".github/workflows/kotodama_perf.yml").read_text()
        job = workflow.split("  representative-regression:\n", 1)[1]
        self.assertIn("    timeout-minutes: 360\n", job)
        self.assertIn('      CARGO_BUILD_JOBS: "1"\n', job)
        cache = job.split("      - name: Restore Rust build cache\n", 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn('          cache-on-failure: "true"\n', cache)
        self.assertIn('          cache-workspace-crates: "true"\n', cache)
        self.assertIn("candidate -> ../target-kotodama-perf", cache)
        self.assertIn("baseline -> ../target-kotodama-perf", cache)
        self.assertLess(job.index("Clear cached benchmark evidence"), job.index("Measure base revision"))
        self.assertIn('run: rm -rf "$CARGO_TARGET_DIR/criterion"', job)
        self.assertLess(job.index("Clear base current samples"), job.index("Measure candidate revision"))

        for name in ("Seal complete performance evidence", "Upload complete performance evidence"):
            complete = job.split(f"      - name: {name}\n", 1)[1].split("\n      - name:", 1)[0]
            self.assertNotIn("        if:", complete, "qualified evidence requires prior success")
            self.assertNotIn("continue-on-error:", complete)
        diagnostic = job.split("      - name: Upload nonqualifying performance diagnostics\n", 1)[1]
        self.assertIn("        if: ${{ !success() }}\n", diagnostic)
        self.assertIn("        timeout-minutes: 5\n", diagnostic)
        self.assertIn("uses: actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9", diagnostic)
        self.assertIn("name: kotodama-performance-nonqualifying-diagnostics-${{ github.sha }}-${{ github.run_id }}-${{ github.run_attempt }}", diagnostic)
        self.assertIn("            target-kotodama-perf/criterion\n", diagnostic)
        self.assertIn("            target-kotodama-perf/kotodama-perf-evidence\n", diagnostic)
        self.assertIn("          if-no-files-found: warn\n", diagnostic)
        self.assertIn("          retention-days: 14\n", diagnostic)
        self.assertNotIn("run:", diagnostic)
        self.assertNotIn("continue-on-error:", job)

    def test_release_workflow_runs_the_complete_artifact_gate(self) -> None:
        workflow = (
            ROOT / ".github" / "workflows" / "kotodama_perf.yml"
        ).read_text(encoding="utf-8")
        representative_job = workflow.split(
            "  representative-regression:\n", 1
        )[1]
        self.assertIn('      CARGO_BUILD_JOBS: "1"', representative_job)
        self.assertIn('      RUSTUP_TOOLCHAIN: "1.99.1"', representative_job)

        comparison_marker = "      - name: Check out comparison base\n"
        self.assertEqual(representative_job.count(comparison_marker), 1)
        comparison_step = representative_job.split(
            comparison_marker, 1
        )[1].split("\n      - name:", 1)[0]
        self.assertIn(
            f"          ref: {PERF.BASELINE_SHA}", comparison_step
        )
        self.assertNotIn("github.event.pull_request.base.sha", comparison_step)
        self.assertNotIn(
            "github.event.repository.default_branch", comparison_step
        )

        lock_marker = "      - name: Require regular revision lockfiles\n"
        self.assertEqual(representative_job.count(lock_marker), 1)
        lock_step = representative_job.split(lock_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            "for lock in candidate/Cargo.lock baseline/Cargo.lock; do",
            lock_step,
        )
        self.assertIn(
            'if [[ ! -f "$lock" || -L "$lock" ]]; then', lock_step
        )
        self.assertIn("exit 1", lock_step)
        self.assertNotIn("|| true", lock_step)
        self.assertNotIn("continue", lock_step)
        self.assertNotRegex(
            lock_step,
            r"(?m)^\s*(?:cargo|cp|curl|install|ln|mv|python|rsync|touch|wget)\b",
        )

        setup_marker = "      - name: Install native benchmark prerequisites\n"
        self.assertEqual(representative_job.count(setup_marker), 1)
        setup_step = representative_job.split(setup_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn("uses: ./candidate/.github/actions/setup-native-ci", setup_step)
        self.assertIn("working-directory: candidate", setup_step)
        self.assertIn("toolchain: 1.99.1", setup_step)
        self.assertIn("runs-on: ubuntu-24.04", representative_job)
        self.assertNotIn("self-hosted", representative_job)
        self.assertIn('".github/actions/setup-native-ci/**"', workflow)
        self.assertLess(
            representative_job.index(setup_marker),
            representative_job.index("      - name: Test regression checker\n"),
        )

        self.assertNotIn("Install the candidate benchmark harness", workflow)
        self.assertNotIn(
            "cp candidate/crates/ivm/benches/bench_kotodama.rs", workflow
        )
        self.assertNotRegex(
            workflow,
            r"(?m)^\s*(?:cp|mv|rsync|install)\b[^\n]*"
            r"candidate/crates/ivm/benches/bench_kotodama\.rs",
        )
        provenance_marker = (
            "      - name: Require authenticated baseline provenance\n"
        )
        self.assertEqual(workflow.count(provenance_marker), 1)
        provenance_step = workflow.split(provenance_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            'policy["validate_baseline_provenance"]', provenance_step
        )
        self.assertIn(
            'policy["validate_revision_inventories"]', provenance_step
        )
        self.assertIn(
            'baseline_root=Path("baseline")', provenance_step
        )
        self.assertIn(
            "baseline/crates/ivm/benches/bench_kotodama.rs",
            provenance_step,
        )
        self.assertIn(
            "candidate/crates/ivm/benches/bench_kotodama.rs",
            provenance_step,
        )

        base_marker = "      - name: Measure base revision\n"
        self.assertEqual(workflow.count(base_marker), 1)
        self.assertLess(
            workflow.index(provenance_marker), workflow.index(base_marker)
        )
        self.assertLess(workflow.index(lock_marker), workflow.index(base_marker))
        base_step = workflow.split(base_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            "cargo bench --locked --jobs 1 -p ivm --bench bench_kotodama",
            base_step,
        )
        self.assertTrue(
            all(
                line.strip().startswith("cargo bench --locked --jobs 1 ")
                for line in base_step.splitlines()
                if line.strip().startswith("cargo bench ")
            )
        )
        self.assertIn("python3 ../candidate/scripts/kotodama_query_driver.py", base_step)
        self.assertIn("--label base", base_step)
        self.assertIn("--revision-root . --candidate-root ../candidate", base_step)

        candidate_marker = "      - name: Measure candidate revision\n"
        self.assertEqual(workflow.count(candidate_marker), 1)
        self.assertLess(
            workflow.index(lock_marker), workflow.index(candidate_marker)
        )
        candidate_step = workflow.split(candidate_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertTrue(
            all(
                line.strip().startswith("cargo bench --locked --jobs 1 ")
                for line in candidate_step.splitlines()
                if line.strip().startswith("cargo bench ")
            )
        )
        self.assertIn("python3 scripts/kotodama_query_driver.py", candidate_step)
        self.assertIn("--label candidate", candidate_step)
        self.assertIn("--revision-root . --candidate-root .", candidate_step)
        self.assertNotIn("KOTODAMA_BASE_FILTER", workflow)

        enforcement_marker = "      - name: Enforce five-percent ceiling\n"
        self.assertEqual(workflow.count(enforcement_marker), 1)
        enforcement_step = workflow.split(enforcement_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn("--baseline-root ../baseline", enforcement_step)
        self.assertIn("--candidate-root .", enforcement_step)
        self.assertIn(
            '--expected-candidate-commit "${{ github.sha }}"',
            enforcement_step,
        )
        self.assertIn(
            '--json-output "$evidence/report.json"', enforcement_step
        )
        self.assertIn('tee "$evidence/checker.log"', enforcement_step)
        self.assertIn("set -euo pipefail", enforcement_step)
        self.assertNotRegex(enforcement_step, r"(^|\s)--baseline(?:\s|$)")
        self.assertNotIn("--write-baseline", enforcement_step)

        seal_marker = "      - name: Seal complete performance evidence\n"
        upload_marker = "      - name: Upload complete performance evidence\n"
        self.assertEqual(workflow.count(seal_marker), 1)
        self.assertEqual(workflow.count(upload_marker), 1)
        seal_step = workflow.split(seal_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            '"schema": "iroha.kotodama.performance.workflow.v1"', seal_step
        )
        self.assertIn('manifest = evidence / "SHA256SUMS"', seal_step)
        self.assertIn('("criterion", criterion)', seal_step)
        self.assertIn('("evidence", evidence)', seal_step)
        self.assertIn("non-regular evidence path", seal_step)
        self.assertLess(
            seal_step.index("if path.is_symlink():"),
            seal_step.index("if path.is_dir():"),
        )
        self.assertNotIn("|| true", seal_step)

        upload_step = workflow.split(upload_marker, 1)[1].split(
            "\n      - name:", 1
        )[0]
        self.assertIn(
            "uses: actions/upload-artifact@"
            "cf430e030ddbb5b0abf93d22962f4752f3646cd9",
            upload_step,
        )
        self.assertIn("target-kotodama-perf/criterion", upload_step)
        self.assertIn(
            "target-kotodama-perf/kotodama-perf-evidence", upload_step
        )
        self.assertIn("if-no-files-found: error", upload_step)
        self.assertIn("include-hidden-files: true", upload_step)
        self.assertIn("retention-days: 90", upload_step)

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
            "--admission-tool ../target-kotodama-perf/debug/ivm_artifact_admit", release_gate
        )
        self.assertNotIn("--skip-runtime-manifest-check", release_gate)
        self.assertNotIn("--skip-contract-tests", release_gate)

        build_marker = "      - name: Build canonical Kotodama release tools\n"
        self.assertEqual(workflow.count(build_marker), 1)
        self.assertLess(
            workflow.index(enforcement_marker), workflow.index(seal_marker)
        )
        self.assertLess(
            workflow.index(seal_marker), workflow.index(upload_marker)
        )
        self.assertLess(
            workflow.index(upload_marker), workflow.index(build_marker)
        )
        build_step = workflow.split(build_marker, 1)[1].split(marker, 1)[0]
        self.assertIn(
            "cargo build --locked -p kotodama_toolchain --bin koto -p ivm_artifact_admission "
            "--features ivm_artifact_admission/dev-tools --bin ivm_artifact_admit",
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
