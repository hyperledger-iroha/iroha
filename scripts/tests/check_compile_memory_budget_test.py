"""Exercise source-bound RSS budgets without running Cargo or fabricating passes."""

from __future__ import annotations

import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("memory_budget", ROOT / "scripts/check_compile_memory_budget.py")
assert SPEC is not None and SPEC.loader is not None
BUDGET = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUDGET)


def unit(package: str, *, opt_level: str = "0", source: str = "src/lib.rs") -> dict:
    """Describe a small complete Cargo artifact projection."""
    return {
        "crate_types": ["lib"], "features": [], "kind": ["lib"], "name": package,
        "package_id": f"workspace#{package}@1.0.0", "source_path": source,
        "profile": {"debug_assertions": opt_level == "0", "debuginfo": 0,
                    "opt_level": opt_level, "overflow_checks": opt_level == "0", "test": False},
    }


def measured(package: str, peak: int, *, opt_level: str = "0") -> dict:
    """Make a synthetic successful compiler invocation for policy tests only."""
    artifact = {"path": f"target/debug/lib{package}.rlib", "bytes": 25, "sha256": "a" * 64}
    return {
        "unit": unit(package, opt_level=opt_level), "peak_rss_bytes": peak, "returncode": 0,
        "elapsed_ns": 10, "user_cpu_seconds": 1.0, "system_cpu_seconds": 0.1,
        "arguments": ["--edition=2024", "-C", f"opt-level={opt_level}", "-Ccodegen-units=8"],
        "compiler_artifacts": [copy.deepcopy(artifact)], "cargo_artifacts": [copy.deepcopy(artifact)],
        "build_script": None,
    }


def seal(report: dict) -> dict:
    """Recompute synthetic report seals after an intentional test mutation."""
    report["input_sha256"] = BUDGET.digest(report["input"])
    report["input_validation"] = {
        "stable": True, "changed_fields": [], "error": None, "private_state_cleanup_error": None,
        "post_input": copy.deepcopy(report["input"]), "post_input_sha256": report["input_sha256"],
    }
    result = report["result"]
    for field in ("compiler_measurements", "compiler_records", "unit_inventory"):
        result[field + "_sha256"] = BUDGET.digest(result[field])
    return report


def inventory(report: dict) -> dict:
    """Keep synthetic Cargo inventory and all measured invocations aligned."""
    compiled = report["result"]["compiler_measurements"]["compiled"]
    report["result"]["unit_inventory"] = sorted([copy.deepcopy(r["unit"]) for r in compiled],
                                                key=BUDGET.PROFILER.canonical_json_bytes)
    report["result"]["compiled_units"] = len(compiled)
    return seal(report)


def profile(surface: str = "model") -> dict:
    """Create tiny schema-4 test evidence; this is never a real build baseline."""
    fingerprint = {"bytes": 1, "files": 1, "records": 1, "sha256": "a" * 64}
    args = BUDGET.SURFACES[surface]
    package = args[args.index("-p") + 1]
    rows = [measured("iroha_data_model", 1000, opt_level="1" if "--release" in args else "0"),
            measured("foundation", 200)]
    if package != "iroha_data_model":
        rows.append(measured(package, 500, opt_level="3" if "--release" in args else "0"))
    return inventory({
        "schema_version": 4, "valid": True,
        "input": {
            "cargo_args": BUDGET.PROFILER.normalized_cargo_args(args, 1), "jobs": 1,
            "profile_mode": "cold", "label": "synthetic-test", "path": "/pinned/tools",
            "selected_env": {"CARGO_INCREMENTAL": "0"}, "cargo_cache": copy.deepcopy(fingerprint),
            "private_cargo_input": copy.deepcopy(fingerprint), "rustup_tree": copy.deepcopy(fingerprint),
            "private_rustup_input": copy.deepcopy(fingerprint), "source": copy.deepcopy(fingerprint),
            "execution_source": copy.deepcopy(fingerprint), "cargo_lock_sha256": "b" * 64,
            "git_revision": "c" * 40,
            "target_initial": {"bytes": 0, "files": 0, "records": 0, "sha256": BUDGET.PROFILER.sha256_bytes(b"")},
            "compiler_measurement": {
                "method": "wait4-per-compiler", "rss_unit": "bytes", "record_schema": 3,
                "compiler_entrypoint": "RUSTC-and-PATH", "helper_sha256": "e" * 64,
                "profiler": {"sha256": "f" * 64, "bytes": 123},
            },
            "toolchain": {tool: {"sha256": "d" * 64, "launcher_sha256": "e" * 64,
                                 "resolved_path": "/baseline/tools/" + tool, "version": "pinned"}
                          for tool in ("cargo", "rustc", "git")},
        },
        "result": {
            "returncode": 0, "compiler_measurement_error": None, "platform": "pinned-test-platform",
            "compiler_measurements": {"compiled": rows, "fresh": [], "failed": [], "probes": []},
            "compiler_records": [], "fresh_units": 0, "cargo_process_peak_rss_bytes": 999999,
        },
    })


def policy(report: dict, surface: str = "model") -> dict:
    """Pin one synthetic baseline while retaining all required pending surfaces."""
    row = report["result"]["compiler_measurements"]["compiled"][0]
    result = {
        "schema_version": 1, "release_ceiling_bytes": BUDGET.RELEASE_CEILING,
        "model_reduction_percent": 25, "model_packages": sorted(BUDGET.MODEL_PACKAGES),
        "runner": "pinned-test-runner", "surfaces": {},
    }
    for name, args in BUDGET.SURFACES.items():
        result["surfaces"][name] = {"cargo_args": args, "state": "pending", "reason": "test fixture"}
    result["surfaces"][surface] = {
        "cargo_args": BUDGET.SURFACES[surface], "state": "measured", "introduced_units": [],
        "baseline": {"report_sha256": "a" * 64, "input_sha256": report["input_sha256"],
                     "compiler_measurements_sha256": report["result"]["compiler_measurements_sha256"],
                     "model_unit": copy.deepcopy(row["unit"]), "model_peak_bytes": row["peak_rss_bytes"]},
        "model_limit_bytes": row["peak_rss_bytes"] * 3 // 4,
    }
    return result


class CompileMemoryBudgetTests(unittest.TestCase):
    """Keep evidence validity separate from the measured acceptance comparison."""

    def setUp(self) -> None:
        self.baseline = profile()
        self.candidate = copy.deepcopy(self.baseline)
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 750
        self.candidate["input"]["source"]["sha256"] = "1" * 64
        self.candidate["input"]["execution_source"]["sha256"] = "2" * 64
        self.candidate["input"]["cargo_lock_sha256"] = "3" * 64
        seal(self.candidate)
        self.contract = policy(self.baseline)

    def compare(self, **kwargs) -> dict:
        return BUDGET.compare(self.contract, kwargs.get("surface", "model"), self.baseline,
                              self.candidate, kwargs.get("runner", "pinned-test-runner"))

    def test_exact_25_percent_and_source_change_pass(self) -> None:
        result = self.compare()
        self.assertTrue(result["passed"])
        self.assertEqual(result["candidate_source_sha256"], "1" * 64)
        self.assertEqual(result["candidate_lock_sha256"], "3" * 64)
        self.assertEqual(sorted(r["max_peak_bytes"] for r in result["limits"]), [200, 750])

    def test_one_byte_above_reduction_fails(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 751
        seal(self.candidate)
        result = self.compare()
        self.assertFalse(result["passed"])
        self.assertEqual(len(result["failures"]), 1)

    def test_unchanged_unit_cannot_regress(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][1]["peak_rss_bytes"] = 201
        seal(self.candidate)
        self.assertFalse(self.compare()["passed"])

    def test_cargo_parent_rss_is_not_a_unit_limit(self) -> None:
        self.candidate["result"]["cargo_process_peak_rss_bytes"] = BUDGET.RELEASE_CEILING * 100
        self.assertTrue(self.compare()["passed"])

    def test_failed_or_unstable_reports_reject(self) -> None:
        for location, key, value in (
            ([], "valid", False), ([], "schema_version", 3),
            (["result"], "returncode", 101), (["result"], "compiler_measurement_error", "swallowed probe failure"),
            (["input_validation"], "stable", False), (["input_validation"], "changed_fields", ["source"]),
            (["input_validation"], "error", "source drift"),
            (["input_validation"], "private_state_cleanup_error", "cleanup failed"),
            (["input_validation"], "post_input_sha256", "f" * 64),
        ):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.candidate)
                target = changed
                for part in location:
                    target = target[part]
                target[key] = value
                with self.assertRaises(ValueError):
                    BUDGET.validate_report(changed)

    def test_all_report_digests_checked(self) -> None:
        for field in ("input_sha256", "unit_inventory_sha256", "compiler_measurements_sha256", "compiler_records_sha256"):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.candidate)
                (changed if field == "input_sha256" else changed["result"])[field] = "f" * 64
                with self.assertRaisesRegex(ValueError, "digest mismatch"):
                    BUDGET.validate_report(changed)

    def test_nonempty_target_and_warm_profiles_reject(self) -> None:
        for field, value in (("profile_mode", "warm"), ("target_initial", {"bytes": 1})):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.candidate)
                changed["input"][field] = value
                seal(changed)
                with self.assertRaisesRegex(ValueError, "empty cold target"):
                    BUDGET.validate_report(changed)

    def test_fresh_and_failed_units_reject(self) -> None:
        for field in ("fresh", "failed"):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.candidate)
                changed["result"]["compiler_measurements"][field] = [{"unit": "not measured"}]
                seal(changed)
                with self.assertRaisesRegex(ValueError, "cached or failed"):
                    BUDGET.validate_report(changed)

    def test_inventory_count_and_multiplicity_checked(self) -> None:
        changed = copy.deepcopy(self.candidate)
        changed["result"]["unit_inventory"].append(copy.deepcopy(changed["result"]["unit_inventory"][0]))
        seal(changed)
        with self.assertRaisesRegex(ValueError, "differ from Cargo"):
            BUDGET.validate_report(changed)
        changed = copy.deepcopy(self.candidate)
        changed["result"]["compiled_units"] = True
        with self.assertRaisesRegex(ValueError, "count mismatch"):
            BUDGET.validate_report(changed)

    def test_repeated_identity_preserves_highest_measurement(self) -> None:
        original = self.baseline["result"]["compiler_measurements"]["compiled"][1]
        duplicate = copy.deepcopy(original)
        duplicate["peak_rss_bytes"] = 300
        self.baseline["result"]["compiler_measurements"]["compiled"].append(duplicate)
        inventory(self.baseline)
        self.contract = policy(self.baseline)
        self.candidate["result"]["compiler_measurements"]["compiled"][1]["peak_rss_bytes"] = 299
        seal(self.candidate)
        self.assertTrue(self.compare()["passed"])
        self.candidate["result"]["compiler_measurements"]["compiled"].append(copy.deepcopy(duplicate))
        self.candidate["result"]["compiler_measurements"]["compiled"][-1]["peak_rss_bytes"] = 301
        inventory(self.candidate)
        result = self.compare()
        self.assertFalse(result["passed"])
        self.assertEqual(result["failures"][0]["invocations"], 2)

    def test_invalid_rss_and_artifact_mismatch_reject(self) -> None:
        for value in (True, 0, -1, 750.0, None):
            with self.subTest(rss=value):
                changed = copy.deepcopy(self.candidate)
                changed["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = value
                seal(changed)
                with self.assertRaisesRegex(ValueError, "RSS measurement"):
                    BUDGET.validate_report(changed)
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["cargo_artifacts"][0]["sha256"] = "b" * 64
        seal(self.candidate)
        with self.assertRaisesRegex(ValueError, "matching compiler output"):
            self.compare()

    def test_private_toolchain_paths_may_differ_but_bytes_may_not(self) -> None:
        self.candidate["input"]["toolchain"]["rustc"]["resolved_path"] = "/candidate/tools/rustc"
        seal(self.candidate)
        self.assertTrue(self.compare()["passed"])
        self.candidate["input"]["toolchain"]["rustc"]["sha256"] = "f" * 64
        seal(self.candidate)
        with self.assertRaisesRegex(ValueError, "toolchain: rustc"):
            self.compare()

    def test_measurement_inputs_must_match(self) -> None:
        for field in ("jobs", "cargo_args", "path", "selected_env", "cargo_cache", "private_cargo_input",
                      "rustup_tree", "private_rustup_input", "compiler_measurement"):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.candidate)
                if field == "compiler_measurement":
                    changed["input"][field]["helper_sha256"] = "0" * 64
                else:
                    changed["input"][field] = "different"
                seal(changed)
                with self.assertRaisesRegex(ValueError, "incomparable input"):
                    BUDGET.compare(self.contract, "model", self.baseline, changed, "pinned-test-runner")

    def test_host_and_runner_must_match(self) -> None:
        with self.assertRaisesRegex(ValueError, "pinned runner"):
            self.compare(runner="another-runner")
        self.candidate["result"]["platform"] = "different"
        with self.assertRaisesRegex(ValueError, "host platform"):
            self.compare()

    def test_codegen_changes_cannot_hide_in_cargo_profile(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][1]["arguments"].append("-Ccodegen-units=256")
        seal(self.candidate)
        with self.assertRaisesRegex(ValueError, "codegen controls changed"):
            self.compare()

    def test_compiler_profile_must_match_recorded_arguments(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["arguments"].append("-Copt-level=3")
        seal(self.candidate)
        with self.assertRaisesRegex(ValueError, "arguments disagree"):
            self.compare()

    def test_long_and_shorthand_codegen_controls_cannot_bypass_comparison(self) -> None:
        for extra in (["--codegen=opt-level=3"], ["--codegen", "opt-level=3"],
                      ["--codegen=codegen-units=256"], ["--codegen", "codegen-units=256"],
                      ["--codegen=incremental=target/cache"], ["--codegen", "incremental=target/cache"],
                      ["-O"], ["-g"]):
            with self.subTest(arguments=extra):
                changed = copy.deepcopy(self.candidate)
                changed["result"]["compiler_measurements"]["compiled"][0]["arguments"] += extra
                seal(changed)
                with self.assertRaises(ValueError):
                    BUDGET.compare(self.contract, "model", self.baseline, changed, "pinned-test-runner")
        self.assertEqual(BUDGET.compiler_controls(["--codegen", "opt-level=3", "-g"]),
                         BUDGET.compiler_controls(["-Copt-level=3", "--codegen=debuginfo=2"]))
        for extra in (["--codegen"], ["--codegen="], ["--codegen", ""]):
            with self.subTest(arguments=extra), self.assertRaises(ValueError):
                BUDGET.compiler_controls(extra)

    def test_instrumentation_and_artifact_shape_are_mandatory(self) -> None:
        for key, value in (("method", "cargo-parent-rss"), ("rss_unit", "KiB"),
                           ("record_schema", 2), ("compiler_entrypoint", "RUSTC_WRAPPER-only")):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.candidate)
                changed["input"]["compiler_measurement"][key] = value
                seal(changed)
                with self.assertRaisesRegex(ValueError, "unsupported compiler measurement"):
                    BUDGET.validate_report(changed)
        for key, value in (("bytes", True), ("sha256", "not-a-digest"), ("path", "target/../escape")):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.candidate)
                changed["result"]["compiler_measurements"]["compiled"][0]["compiler_artifacts"][0][key] = value
                seal(changed)
                with self.assertRaisesRegex(ValueError, "sealed compiler artifact"):
                    BUDGET.validate_report(changed)

    def test_metadata_disambiguators_can_change(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["arguments"] += [
            "-Cmetadata=new-hash", "-C", "extra-filename=-different",
        ]
        seal(self.candidate)
        self.assertTrue(self.compare()["passed"])

    def test_conditional_compilation_and_sysroot_changes_cannot_hide_reduction(self) -> None:
        for extra in (["--cfg", "skip_expensive_model"], ["--cfg=skip_expensive_model"],
                      ["--sysroot", "other-sysroot"], ["--sysroot=other-sysroot"]):
            with self.subTest(arguments=extra):
                candidate = copy.deepcopy(self.candidate)
                candidate["result"]["compiler_measurements"]["compiled"][0]["arguments"] += extra
                seal(candidate)
                with self.assertRaisesRegex(ValueError, "codegen controls changed"):
                    BUDGET.compare(self.contract, "model", self.baseline, candidate, "pinned-test-runner")
        self.assertEqual(BUDGET.compiler_controls(["--cfg=x", "--cfg=y", "--cfg=x"]),
                         BUDGET.compiler_controls(["--cfg", "y", "--cfg", "x"]))

    def test_compiler_controls_reject_missing_and_incremental_options(self) -> None:
        for arguments in (["-C"], ["-Z"], ["-Cincremental=target/cache"], ["-C", "incremental=target/cache"]):
            with self.subTest(arguments=arguments), self.assertRaises(ValueError):
                BUDGET.compiler_controls(arguments)
        controls = BUDGET.compiler_controls(["-Copt-level=1", "-Z", "unstable-options", "--target", "aarch64-apple-darwin", "--edition=2024"])
        self.assertEqual(controls, {"codegen": ["opt-level=1"], "unstable": ["unstable-options"],
                                    "target": ["aarch64-apple-darwin"], "edition": ["2024"],
                                    "cfg": [], "sysroot": []})

    def test_new_unit_needs_explicit_limit(self) -> None:
        row = measured("iroha_model_base", 800)
        self.candidate["result"]["compiler_measurements"]["compiled"].append(row)
        inventory(self.candidate)
        with self.assertRaisesRegex(ValueError, "lacks reviewed byte limit"):
            self.compare()
        self.contract["surfaces"]["model"]["introduced_units"].append({
            "unit": row["unit"], "max_peak_bytes": 1000, "reason": "new foundation compilation boundary",
            "compiler_controls": [BUDGET.compiler_controls(row["arguments"])],
        })
        result = self.compare()
        self.assertFalse(result["passed"])
        self.assertEqual(result["failures"][0]["max_peak_bytes"], 750)
        row["peak_rss_bytes"] = 750
        inventory(self.candidate)
        self.assertTrue(self.compare()["passed"])

    def test_new_model_cannot_change_optimization(self) -> None:
        row = measured("iroha_privacy_model", 700, opt_level="1")
        self.candidate["result"]["compiler_measurements"]["compiled"].append(row)
        inventory(self.candidate)
        self.contract["surfaces"]["model"]["introduced_units"].append({
            "unit": row["unit"], "max_peak_bytes": 750, "reason": "new privacy model",
            "compiler_controls": [BUDGET.compiler_controls(row["arguments"])],
        })
        with self.assertRaisesRegex(ValueError, "model optimization changed"):
            self.compare()

    def test_model_cap_cannot_be_avoided_with_another_target_kind(self) -> None:
        for kind in ("rlib", "dylib", "cdylib", "staticlib", "bin", "test", "custom-build"):
            with self.subTest(kind=kind):
                candidate, contract = copy.deepcopy(self.candidate), copy.deepcopy(self.contract)
                row = measured("iroha_model_base", 800)
                row["unit"]["kind"] = [kind]
                row["unit"]["crate_types"] = ["bin" if kind in ("test", "custom-build") else kind]
                candidate["result"]["compiler_measurements"]["compiled"].append(row)
                inventory(candidate)
                contract["surfaces"]["model"]["introduced_units"].append({
                    "unit": row["unit"], "max_peak_bytes": 1000, "reason": "new model target",
                    "compiler_controls": [BUDGET.compiler_controls(row["arguments"])],
                })
                result = BUDGET.compare(contract, "model", self.baseline, candidate, "pinned-test-runner")
                self.assertFalse(result["passed"])
                self.assertEqual(result["failures"][0]["max_peak_bytes"], 750)
                row["peak_rss_bytes"] = 750
                inventory(candidate)
                self.assertTrue(BUDGET.compare(contract, "model", self.baseline, candidate, "pinned-test-runner")["passed"])

    def test_selected_root_cannot_change_features_or_disappear(self) -> None:
        self.candidate["result"]["compiler_measurements"]["compiled"][0]["unit"]["features"] = ["reduced"]
        inventory(self.candidate)
        with self.assertRaisesRegex(ValueError, "selected package targets/features/profile"):
            self.compare()

    def test_unused_dependency_removal_is_recorded(self) -> None:
        removed = self.candidate["result"]["compiler_measurements"]["compiled"].pop()
        inventory(self.candidate)
        result = self.compare()
        self.assertTrue(result["passed"])
        self.assertEqual(result["removed_units"], [removed["unit"]])

    def test_no_new_unit_override_of_retained_limits(self) -> None:
        row = self.candidate["result"]["compiler_measurements"]["compiled"][1]
        self.contract["surfaces"]["model"]["introduced_units"].append({
            "unit": row["unit"], "max_peak_bytes": 9999, "reason": "attempt to waive regression",
            "compiler_controls": [BUDGET.compiler_controls(row["arguments"])],
        })
        with self.assertRaisesRegex(ValueError, "cannot override retained-unit"):
            self.compare()

    def test_pending_and_removed_surfaces_reject(self) -> None:
        with self.assertRaisesRegex(ValueError, "pending"):
            self.compare(surface="core-tests")
        del self.contract["surfaces"]["core-tests"]
        with self.assertRaisesRegex(ValueError, "required memory surfaces"):
            BUDGET.validate_policy(self.contract)

    def test_release_ceiling_applies_even_to_unchanged_units(self) -> None:
        for surface in ("native-bridge-release", "javascript-native-release"):
            with self.subTest(surface=surface):
                old = profile(surface)
                old["result"]["compiler_measurements"]["compiled"][1]["peak_rss_bytes"] = BUDGET.RELEASE_CEILING + 1
                seal(old)
                candidate = copy.deepcopy(old)
                candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 750
                seal(candidate)
                result = BUDGET.compare(policy(old, surface), surface, old, candidate, "pinned-test-runner")
                self.assertFalse(result["passed"])
                self.assertIn("13-GiB release ceiling", result["failures"][0]["constraints"])

    def test_release_ceiling_covers_successful_and_negative_compiler_probes(self) -> None:
        surface = "native-bridge-release"
        for source in (None, {"kind": "stdin", "path": None, "bytes": 12, "sha256": "f" * 64}):
            for returncode in (0, 1):
                with self.subTest(source=source, returncode=returncode):
                    old = profile(surface)
                    candidate = copy.deepcopy(old)
                    candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 750
                    probe = {"source": source, "arguments": ["--version"] if source is None else ["-"],
                             "returncode": returncode, "peak_rss_bytes": BUDGET.RELEASE_CEILING + 1}
                    candidate["result"]["compiler_measurements"]["probes"] = [probe]
                    seal(candidate)
                    result = BUDGET.compare(policy(old, surface), surface, old, candidate, "pinned-test-runner")
                    self.assertFalse(result["passed"])
                    self.assertEqual(result["failures"][0]["probe_source"], source)
                    probe["peak_rss_bytes"] = BUDGET.RELEASE_CEILING
                    seal(candidate)
                    self.assertTrue(BUDGET.compare(policy(old, surface), surface, old, candidate, "pinned-test-runner")["passed"])

    def test_probe_measurements_cannot_be_missing_or_invalid(self) -> None:
        probe = {"source": None, "arguments": ["--version"], "returncode": 0, "peak_rss_bytes": 20}
        for key, value in (("peak_rss_bytes", True), ("peak_rss_bytes", 0), ("returncode", False),
                           ("arguments", [None]), ("source", {"kind": "file"})):
            with self.subTest(key=key, value=value):
                candidate = copy.deepcopy(self.candidate)
                changed = copy.deepcopy(probe)
                changed[key] = value
                candidate["result"]["compiler_measurements"]["probes"] = [changed]
                seal(candidate)
                with self.assertRaises(ValueError):
                    BUDGET.validate_report(candidate)

    def test_policy_cannot_weaken_reduction_or_release_ceiling(self) -> None:
        for field, value in (("model_reduction_percent", 24), ("release_ceiling_bytes", BUDGET.RELEASE_CEILING + 1),
                             ("model_packages", ["iroha_data_model"]), ("runner", "")):
            with self.subTest(field=field):
                contract = copy.deepcopy(self.contract)
                contract[field] = value
                with self.assertRaises(ValueError):
                    BUDGET.validate_policy(contract)
        self.contract["surfaces"]["model"]["model_limit_bytes"] = 751
        with self.assertRaisesRegex(ValueError, "25% reduction"):
            BUDGET.validate_policy(self.contract)

    def test_substituted_baseline_peak_is_rejected(self) -> None:
        self.contract["surfaces"]["model"]["baseline"]["model_peak_bytes"] = 1200
        self.contract["surfaces"]["model"]["model_limit_bytes"] = 900
        with self.assertRaisesRegex(ValueError, "model baseline peak differs"):
            self.compare()

    def test_source_span_identity_preserves_package_external_paths(self) -> None:
        external = unit("fixture", source="../shared/lib.rs")
        self.assertIn("../shared/lib.rs", BUDGET.unit_key(external))
        external["source_path"] = "/absolute/lib.rs"
        with self.assertRaisesRegex(ValueError, "package-relative"):
            BUDGET.unit_key(external)

    def test_json_pins_duplicates_and_nonfinite_values_reject(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            for payload in (b'{"x":1,"x":2}', b'{"x":NaN}', b'[]'):
                with self.subTest(payload=payload):
                    path.write_bytes(payload)
                    with self.assertRaises(ValueError):
                        BUDGET.read_json(path)
            path.write_text('{"x":1}')
            with self.assertRaisesRegex(ValueError, "digest mismatch"):
                BUDGET.read_json(path, "f" * 64)
            self.assertEqual(BUDGET.read_json(path, BUDGET.PROFILER.sha256_bytes(path.read_bytes())), {"x": 1})

    def test_cli_pins_reports_and_returns_distinct_budget_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old, new, contract = root / "baseline.json", root / "candidate.json", root / "policy.json"
            old.write_text(json.dumps(self.baseline))
            new.write_text(json.dumps(self.candidate))
            self.contract["surfaces"]["model"]["baseline"]["report_sha256"] = BUDGET.PROFILER.sha256_bytes(old.read_bytes())
            contract.write_text(json.dumps(self.contract))
            argv = ["--policy", str(contract), "--surface", "model", "--baseline", str(old),
                    "--candidate", str(new), "--candidate-sha256", BUDGET.PROFILER.sha256_bytes(new.read_bytes()),
                    "--runner", "pinned-test-runner"]
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(BUDGET.main(argv), 0)
            self.assertTrue(json.loads(output.getvalue())["passed"])
            argv[argv.index("--candidate-sha256") + 1] = "f" * 64
            with contextlib.redirect_stderr(io.StringIO()) as error:
                self.assertEqual(BUDGET.main(argv), 2)
            self.assertIn("digest mismatch", error.getvalue())
            new.write_bytes(old.read_bytes())
            argv[argv.index("--candidate-sha256") + 1] = BUDGET.PROFILER.sha256_bytes(new.read_bytes())
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(BUDGET.main(argv), 1)
            self.assertFalse(json.loads(output.getvalue())["passed"])

    def test_checked_in_policy_keeps_unqualified_surfaces_pending(self) -> None:
        contract = BUDGET.read_json(ROOT / "ci/compile_memory_budgets.json")
        BUDGET.validate_policy(contract)
        self.assertEqual(contract["surfaces"]["model"]["model_limit_bytes"], 9_956_524_032)
        self.assertEqual({name for name, row in contract["surfaces"].items() if row["state"] == "pending"},
                         {"core-tests", "torii-tests", "daemon-release"})


class CompileMemorySuiteTests(unittest.TestCase):
    """A complete qualification cannot mix source revisions or omit failed jobs."""

    def setUp(self) -> None:
        self.reports = {}
        self.contract = policy(profile())
        self.identity = {"source_sha256": "1" * 64, "execution_source_sha256": "2" * 64,
                         "cargo_lock_sha256": "3" * 64, "git_revision": "4" * 40}
        for name in BUDGET.SURFACES:
            old = profile(name)
            candidate = copy.deepcopy(old)
            candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 750
            candidate["input"]["source"]["sha256"] = self.identity["source_sha256"]
            candidate["input"]["execution_source"]["sha256"] = self.identity["execution_source_sha256"]
            candidate["input"]["cargo_lock_sha256"] = self.identity["cargo_lock_sha256"]
            candidate["input"]["git_revision"] = self.identity["git_revision"]
            seal(candidate)
            self.reports[name] = (old, candidate)
            self.contract["surfaces"][name] = policy(old, name)["surfaces"][name]

    def compare(self) -> dict:
        return BUDGET.compare_suite(self.contract, self.reports, "pinned-test-runner", self.identity)

    def test_all_surfaces_share_candidate_identity(self) -> None:
        result = self.compare()
        self.assertTrue(result["passed"])
        self.assertEqual(set(result["surfaces"]), set(BUDGET.SURFACES))
        self.assertEqual(result["candidate"], self.identity)

    def test_one_budget_failure_propagates(self) -> None:
        candidate = self.reports["torii-tests"][1]
        candidate["result"]["compiler_measurements"]["compiled"][0]["peak_rss_bytes"] = 751
        seal(candidate)
        result = self.compare()
        self.assertFalse(result["passed"])
        self.assertFalse(result["surfaces"]["torii-tests"]["passed"])

    def test_missing_surface_or_pending_baseline_cannot_pass(self) -> None:
        del self.reports["core-tests"]
        with self.assertRaisesRegex(ValueError, "every memory surface"):
            self.compare()
        self.setUp()
        self.contract["surfaces"]["core-tests"].update(state="pending", reason="failed test build")
        with self.assertRaisesRegex(ValueError, "pending"):
            self.compare()

    def test_mixed_source_lock_or_revision_reject(self) -> None:
        for kind in self.identity:
            with self.subTest(kind=kind):
                self.setUp()
                candidate = self.reports["native-bridge-release"][1]
                if kind in ("source_sha256", "execution_source_sha256"):
                    candidate["input"][kind.removesuffix("_sha256")]["sha256"] = "9" * 64
                else:
                    candidate["input"][kind] = "9" * (40 if kind == "git_revision" else 64)
                seal(candidate)
                with self.assertRaisesRegex(ValueError, "differs from the suite"):
                    self.compare()

    def test_failed_build_is_an_invalid_suite_not_a_skipped_job(self) -> None:
        self.reports["daemon-release"][1]["result"]["returncode"] = 101
        with self.assertRaisesRegex(ValueError, "failed compiler build"):
            self.compare()

    def test_cli_suite_binds_relative_report_paths_and_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = {"schema_version": 1, "candidate": self.identity,
                        "runner": "pinned-test-runner", "reports": {}}
            for name, (old, candidate) in self.reports.items():
                old_path, candidate_path = root / (name + "-old.json"), root / (name + "-new.json")
                old_path.write_text(json.dumps(old))
                candidate_path.write_text(json.dumps(candidate))
                self.contract["surfaces"][name]["baseline"]["report_sha256"] = BUDGET.PROFILER.sha256_bytes(old_path.read_bytes())
                manifest["reports"][name] = {
                    "baseline": old_path.name, "candidate": candidate_path.name,
                    "candidate_sha256": BUDGET.PROFILER.sha256_bytes(candidate_path.read_bytes()),
                }
            manifest_path, policy_path = root / "suite.json", root / "policy.json"
            manifest_path.write_text(json.dumps(manifest))
            policy_path.write_text(json.dumps(self.contract))
            argv = ["--policy", str(policy_path), "--suite", str(manifest_path)]
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(BUDGET.main(argv), 0)
            self.assertTrue(json.loads(output.getvalue())["passed"])
            with contextlib.redirect_stderr(io.StringIO()) as error:
                self.assertEqual(BUDGET.main(argv + ["--runner", "another-runner"]), 2)
            self.assertIn("only from its manifest", error.getvalue())
            manifest["reports"]["sdk"]["candidate_sha256"] = "f" * 64
            manifest_path.write_text(json.dumps(manifest))
            with contextlib.redirect_stderr(io.StringIO()) as error:
                self.assertEqual(BUDGET.main(argv), 2)
            self.assertIn("digest mismatch", error.getvalue())
            manifest["reports"]["sdk"]["candidate_sha256"] = None
            manifest_path.write_text(json.dumps(manifest))
            with contextlib.redirect_stderr(io.StringIO()) as error:
                self.assertEqual(BUDGET.main(argv), 2)
            self.assertIn("candidate report digest is missing", error.getvalue())


if __name__ == "__main__":
    unittest.main()
