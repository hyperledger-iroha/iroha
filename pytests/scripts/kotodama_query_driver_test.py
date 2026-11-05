#!/usr/bin/env python3
"""Exercise exact driver derivation, compiler admission and fresh artifact evidence."""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "kotodama_query_driver", ROOT / "scripts/kotodama_query_driver.py"
)
assert SPEC and SPEC.loader
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


class DerivedQueryDriverTests(unittest.TestCase):
    """Driver evidence never substitutes for the complete hosted comparison."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="iroha-query-driver-control-"
        )
        self.root = Path(self.temporary.name).resolve()
        self.revision = self.root / "revision"
        for relative in DRIVER.policy()["BENCHMARK_SOURCE_PATHS"]:
            destination = self.revision / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative, destination)
        (self.revision / "Cargo.lock").write_bytes(b"original lock bytes\n")
        self.target = self.root / "compiler-output"
        (self.target / "release/deps").mkdir(parents=True)
        self.directory = self.target / "kotodama-query-drivers/base" / ("a" * 32)
        self.directory.mkdir(parents=True)
        self.derived = self.directory / "queries.rs"
        self.original = (self.revision / DRIVER.SOURCE).read_bytes()
        self.derived.write_bytes(DRIVER.derive(self.original))
        self.rustc = Path(
            subprocess.check_output(["rustup", "which", "rustc"], text=True).strip()
        ).resolve()
        self.plan = {
            "schema": DRIVER.SCHEMA,
            "nonce": "a" * 32,
            "label": "base",
            "revision_root": str(self.revision),
            "revision": "b" * 40,
            "lock_sha256": DRIVER.sha(self.revision / "Cargo.lock"),
            "target": str(self.target),
            "rustc": str(self.rustc),
            "rustc_sha256": DRIVER.sha(self.rustc),
            "driver_sha256": DRIVER.sha(DRIVER.SCRIPT),
            "contract": DRIVER.source_contract(self.revision, self.derived),
        }
        self.args = [
            "--crate-name",
            "queries",
            "--edition=2024",
            str(self.revision / DRIVER.SOURCE),
            "--crate-type",
            "bin",
            "--emit=dep-info,link",
            "-C",
            "opt-level=3",
            "-C",
            "extra-filename=-0123456789abcdef",
            "--cfg",
            'feature="bench"',
            "--cfg",
            'feature="iroha-core-tests"',
            "--out-dir",
            str(self.target / "release/deps"),
        ]
        self.executable = self.target / "release/deps/queries-0123456789abcdef"
        self.depinfo = self.executable.with_suffix(".d")
        self.artifact = {
            "reason": "compiler-artifact",
            "fresh": False,
            "executable": str(self.executable),
            "manifest_path": str(self.revision / "crates/iroha_core/Cargo.toml"),
            "target": {
                "name": "queries",
                "kind": ["bench"],
                "crate_types": ["bin"],
                "src_path": str(self.revision / DRIVER.SOURCE),
            },
            "profile": {"opt_level": "3"},
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def compiler_context(self):
        return mock.patch.dict(
            os.environ,
            {
                DRIVER.PLAN_ENV: str(self.directory / "plan.json"),
                "CARGO_MANIFEST_DIR": str(self.revision / "crates/iroha_core"),
                "CARGO_PKG_NAME": "iroha_core",
            },
        )

    def make_receipt(self) -> dict:
        self.executable.write_bytes(b"compiled artifact fixture")
        self.depinfo.write_text(
            f"{self.executable}: {self.derived}\n\n{self.derived}:\n"
        )
        changed = self.args.copy()
        changed[3] = str(self.derived)
        receipt = {
            "nonce": self.plan["nonce"],
            "driver_sha256": DRIVER.sha(DRIVER.SCRIPT),
            "rustc": str(self.rustc),
            "cwd": str(self.revision),
            "manifest_dir": str(self.revision / "crates/iroha_core"),
            "package": "iroha_core",
            "original_args": self.args,
            "derived_args": changed,
            "returncode": 0,
            "contract": self.plan["contract"],
            "executable": str(self.executable),
            "executable_sha256": DRIVER.sha(self.executable),
            "depinfo": str(self.depinfo),
            "depinfo_sha256": DRIVER.sha(self.depinfo),
        }
        (self.directory / "compiler.json").write_text(json.dumps(receipt))
        return receipt

    def test_exact_derivation_preserves_all_46_bodies_and_authorizes_reader(
        self,
    ) -> None:
        changed = self.derived.read_bytes()
        self.assertEqual(
            changed,
            self.original.replace(
                DRIVER.DISPATCH.encode(),
                b"    bench_typed_core_query_pages(&mut c);\n",
                1,
            ).replace(
                DRIVER.WORLD_ARGUMENT.encode(),
                DRIVER.AUTHORIZED_WORLD_ARGUMENT.encode(),
                1,
            ),
        )
        contract = DRIVER.source_contract(self.revision, self.derived)
        self.assertEqual(len(contract["timed_body_sha256"]), 46)
        self.assertEqual(
            contract["timed_body_sha256"], DRIVER.policy()["BASELINE_TIMED_BODY_SHA256"]
        )
        self.assertEqual((self.revision / DRIVER.SOURCE).read_bytes(), self.original)
        self.assertEqual(
            (self.revision / "Cargo.lock").read_bytes(), b"original lock bytes\n"
        )
        self.assertEqual(contract["original_setup_sha256"], DRIVER.SETUP_SHA256)
        self.assertNotEqual(
            contract["derived_setup_sha256"], contract["original_setup_sha256"]
        )
        self.assertEqual(
            contract["fixture_permission"],
            {
                "authority_seed": "typed-query-authority",
                "permission": "CanReadAllLedgerData",
                "grant_count": 1,
                "phase": "before raw-response setup and timed queries",
            },
        )
        setup = DRIVER.typed_setup(changed.decode())
        self.assertEqual(setup.count("world.account_permissions_mut_for_testing().insert("), 1)
        self.assertLess(
            setup.index("world.account_permissions_mut_for_testing().insert("),
            setup.index("let raw_query_response_bytes ="),
        )
        self.assertEqual(
            setup.replace(DRIVER.AUTHORIZED_WORLD_ARGUMENT, DRIVER.WORLD_ARGUMENT, 1),
            DRIVER.typed_setup(self.original.decode()),
        )
        for marker in (
            b"set_banner_enabled(false);",
            b"Criterion::default().configure_from_args()",
            b"c.final_summary();",
            b"fn build_state_for_typed_core_query_pages()",
        ):
            self.assertIn(marker, changed)

    def test_original_setup_drift_is_refused_before_derivation(self) -> None:
        for old, new in (
            (b'bench_account("typed-query-authority")', b'bench_account("other")'),
            (b"for i in 0..QUERY_PAGE_CAPACITY_V1", b"for i in 0..1"),
            (b"if i + 1 < QUERY_PAGE_CAPACITY_V1", b"if i < QUERY_PAGE_CAPACITY_V1"),
            (
                b"AssetId::of(asset_definition_id, authority.clone())",
                b"AssetId::of(asset_definition_id, account_id.clone())",
            ),
            (
                b'Quantity::from(u64::try_from(i).expect("fixture index fits u64") + 1)',
                b"Quantity::from(1_u64)",
            ),
            (
                b"&authority,\n                family.tag,",
                b'&bench_account("other"),\n                family.tag,',
            ),
        ):
            with self.subTest(old=old):
                changed = self.original.replace(old, new)
                self.assertNotEqual(changed, self.original)
                with self.assertRaisesRegex(ValueError, "setup differs"):
                    DRIVER.derive(changed)

    def test_setup_boundaries_must_be_unique_and_ordered(self) -> None:
        for text in (
            "missing setup",
            DRIVER.SETUP_END + DRIVER.SETUP_START,
            DRIVER.SETUP_START * 2 + DRIVER.SETUP_END,
        ):
            with self.subTest(text=text), self.assertRaisesRegex(
                ValueError, "setup boundaries"
            ):
                DRIVER.typed_setup(text)

    def test_changed_main_is_refused(self) -> None:
        for old, new in (
            (b"set_banner_enabled(false)", b"set_banner_enabled(true)"),
            (b"c.final_summary();", b""),
            (b"bench_find_triggers_iter(&mut c);", b"other(&mut c);"),
        ):
            with self.subTest(old=old), self.assertRaisesRegex(
                ValueError, "main differs"
            ):
                DRIVER.derive(self.original.replace(old, new))

    def test_altered_derived_body_or_extra_edit_is_refused(self) -> None:
        for old, new in (
            (b"metrics.host_queries, 1", b"metrics.host_queries, 2"),
            (b"QUERY_PAGE_CAPACITY_V1 as u64", b"1_u64"),
            (b"bench_typed_core_query_pages(&mut c);", b"c.final_summary();"),
            (b"CanReadAllLedgerData.into()", b"CanManagePeers.into()"),
            (
                DRIVER.AUTHORIZED_WORLD_ARGUMENT.encode(),
                DRIVER.WORLD_ARGUMENT.encode(),
            ),
            (
                b"world.account_permissions_mut_for_testing().insert(\n                authority.clone(),",
                b'world.account_permissions_mut_for_testing().insert(\n                bench_account("other"),',
            ),
        ):
            with self.subTest(old=old):
                self.derived.write_bytes(DRIVER.derive(self.original).replace(old, new))
                with self.assertRaisesRegex(
                    ValueError, "dispatch-and-permission derivation"
                ):
                    DRIVER.source_contract(self.revision, self.derived)

    def test_altered_original_timed_body_is_refused_even_if_derivation_matches(
        self,
    ) -> None:
        changed = self.original.replace(
            b"metrics.host_queries, 1", b"metrics.host_queries, 2"
        )
        (self.revision / DRIVER.SOURCE).write_bytes(changed)
        self.derived.write_bytes(DRIVER.derive(changed))
        with self.assertRaises(RuntimeError):
            DRIVER.source_contract(self.revision, self.derived)

    def test_foreign_compiler_targets_are_refused(self) -> None:
        variants = []
        for old, new in (
            ("queries", "other"),
            ("bin", "lib"),
            ("opt-level=3", "opt-level=0"),
            (str(self.revision / DRIVER.SOURCE), str(self.root / "foreign.rs")),
            ("--emit=dep-info,link", "--emit=link"),
        ):
            variants.append([new if value == old else value for value in self.args])
        variants += [
            self.args + ["--target", "x86_64-unknown-linux-gnu"],
            self.args + ["--test"],
        ]
        for args in variants:
            with self.subTest(args=args), self.assertRaises(ValueError):
                DRIVER.admitted_compiler(
                    self.plan,
                    str(self.rustc),
                    args,
                    self.revision,
                    str(self.revision / "crates/iroha_core"),
                    "iroha_core",
                )

    def test_cargo_default_binary_crate_type_is_admitted(self) -> None:
        # Cargo harness=false benches omit --crate-type and rustc defaults to bin.
        args = self.args.copy()
        index = args.index("--crate-type")
        del args[index : index + 2]
        DRIVER.admitted_compiler(
            self.plan,
            str(self.rustc),
            args,
            self.revision,
            str(self.revision / "crates/iroha_core"),
            "iroha_core",
        )

    def test_foreign_package_and_compiler_are_refused(self) -> None:
        for rustc, manifest, package in (
            (
                str(self.root / "rustc"),
                str(self.revision / "crates/iroha_core"),
                "iroha_core",
            ),
            (str(self.rustc), str(self.revision), "iroha_core"),
            (str(self.rustc), str(self.revision / "crates/iroha_core"), "other"),
        ):
            with self.subTest(package=package), self.assertRaises(ValueError):
                DRIVER.admitted_compiler(
                    self.plan, rustc, self.args, self.revision, manifest, package
                )

    def test_unrelated_compiler_invocation_delegates_exact_argv_and_status(
        self,
    ) -> None:
        DRIVER.write_json(self.directory / "plan.json", self.plan)
        args = ["--crate-name", "unrelated", "untouched.rs", "-C", "opt-level=1"]
        with self.compiler_context(), mock.patch.object(
            DRIVER.subprocess, "call", return_value=27
        ) as call:
            self.assertEqual(DRIVER.wrapper(str(self.rustc), args), 27)
        call.assert_called_once_with([str(self.rustc), *args])
        self.assertFalse((self.directory / "compiler.json").exists())

    def test_missing_or_faked_wrapper_receipts_fail_closed(self) -> None:
        with self.assertRaises(ValueError):
            DRIVER.validate_compilation(self.directory, self.plan, self.artifact)
        receipt = self.make_receipt()
        mutations = [
            {**receipt, "nonce": "f" * 32},
            {**receipt, "returncode": 1},
            {**receipt, "derived_args": self.args},
            {**receipt, "driver_sha256": "0" * 64},
            {**receipt, "executable_sha256": "0" * 64},
        ]
        for changed in mutations:
            (self.directory / "compiler.json").write_text(json.dumps(changed))
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                DRIVER.validate_compilation(self.directory, self.plan, self.artifact)

    def test_original_source_depinfo_cannot_fake_derived_invocation(self) -> None:
        receipt = self.make_receipt()
        self.depinfo.write_text(str(self.revision / DRIVER.SOURCE) + ":\n")
        receipt["depinfo_sha256"] = DRIVER.sha(self.depinfo)
        (self.directory / "compiler.json").write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, "fresh derived source"):
            DRIVER.validate_compilation(self.directory, self.plan, self.artifact)

    def test_cached_or_foreign_cargo_artifact_is_rejected(self) -> None:
        self.make_receipt()
        for artifact in (
            {**self.artifact, "fresh": True},
            {**self.artifact, "executable": "foreign"},
        ):
            with self.assertRaises(ValueError):
                DRIVER.validate_compilation(self.directory, self.plan, artifact)
        for artifact in (
            {**self.artifact, "manifest_path": "foreign"},
            {**self.artifact, "profile": {"opt_level": "0"}},
        ):
            with self.assertRaises(ValueError):
                DRIVER.selected_artifact([artifact], self.revision)
        with self.assertRaises(ValueError):
            DRIVER.selected_artifact([self.artifact, self.artifact], self.revision)

    def test_binary_and_depinfo_tampering_after_admission_fail(self) -> None:
        self.make_receipt()
        DRIVER.validate_compilation(self.directory, self.plan, self.artifact)
        self.executable.write_bytes(b"replacement")
        with self.assertRaisesRegex(ValueError, "executable hash"):
            DRIVER.validate_compilation(self.directory, self.plan, self.artifact)
        self.make_receipt()
        self.depinfo.write_text("replacement")
        with self.assertRaisesRegex(ValueError, "dep-info hash"):
            DRIVER.validate_compilation(self.directory, self.plan, self.artifact)

    def test_archived_bytes_survive_reuse_of_cargo_output_path(self) -> None:
        self.make_receipt()
        shutil.copyfile(self.executable, self.directory / "measured-executable")
        shutil.copyfile(self.depinfo, self.directory / "measured-dep-info")
        self.executable.write_bytes(b"next revision's artifact")
        DRIVER.validate_compilation(
            self.directory, self.plan, self.artifact, archived=True
        )
        (self.directory / "measured-executable").write_bytes(b"tampered archive")
        with self.assertRaisesRegex(ValueError, "executable hash"):
            DRIVER.validate_compilation(
                self.directory, self.plan, self.artifact, archived=True
            )

    def test_real_compiler_source_substitution_and_depinfo(self) -> None:
        """Compile a tiny mechanism control; this does not run the Core query bench."""
        DRIVER.write_json(self.directory / "plan.json", self.plan)
        self.derived.write_text('fn main() { println!("derived compiler control"); }\n')
        with self.compiler_context(), mock.patch.object(
            DRIVER, "source_contract", return_value=self.plan["contract"]
        ):
            self.assertEqual(DRIVER.wrapper(str(self.rustc), self.args), 0)
            receipt = DRIVER.validate_compilation(
                self.directory, self.plan, self.artifact
            )
        self.assertNotEqual(receipt["original_args"], receipt["derived_args"])
        result = subprocess.check_output([str(self.executable)], text=True)
        self.assertEqual(result, "derived compiler control\n")
        self.assertEqual((self.revision / DRIVER.SOURCE).read_bytes(), self.original)
        self.assertIn(str(self.derived) + ":", self.depinfo.read_text().splitlines())

    def test_real_compiler_delegation(self) -> None:
        DRIVER.write_json(self.directory / "plan.json", self.plan)
        source = self.root / "delegated.rs"
        source.write_text('fn main() { println!("delegated compiler control"); }\n')
        executable = self.root / "delegated"
        with self.compiler_context():
            self.assertEqual(
                DRIVER.wrapper(
                    str(self.rustc),
                    ["--crate-name", "delegated", str(source), "-o", str(executable)],
                ),
                0,
            )
        self.assertEqual(
            subprocess.check_output([str(executable)], text=True),
            "delegated compiler control\n",
        )
        self.assertFalse((self.directory / "compiler.json").exists())

    def exercise_measurement(
        self,
        *,
        cached: bool = True,
        tamper: bool = False,
        missing_receipt: bool = False,
        label: str = "candidate",
    ) -> tuple[list, Path]:
        """Run the real orchestration with controlled compiler/Cargo boundaries."""
        output = self.target / "kotodama-query-drivers" / label
        self.executable.write_bytes(b"old unwrapped cached executable")
        neighbour = self.executable.with_name("unrelated-warm-artifact")
        neighbour.write_bytes(b"preserve dependency")
        calls = []

        def cargo(args, root, environment):
            calls.append(args)
            plan_path = Path(environment[DRIVER.PLAN_ENV])
            self.directory = plan_path.parent
            self.derived = self.directory / "queries.rs"
            self.plan = DRIVER.read_json(plan_path)
            if len(calls) == 1 and cached:
                return {**self.artifact, "fresh": True}
            if "--no-run" in args:
                if cached:
                    self.assertFalse(
                        self.executable.exists(),
                        "only admitted cached output is removed",
                    )
                if not missing_receipt:
                    self.make_receipt()
                return self.artifact
            if tamper:
                self.executable.write_bytes(b"tampered during timed Cargo invocation")
            return {**self.artifact, "fresh": True}

        env = {
            "CARGO_TARGET_DIR": str(self.target),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_INCREMENTAL": "0",
            "RUSTUP_TOOLCHAIN": "1.99.1",
        }
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(DRIVER, "admit_sources"),
            mock.patch.dict(DRIVER.policy(), {"_git_head": lambda _: "b" * 40}),
            mock.patch.object(DRIVER, "cargo", side_effect=cargo),
        ):
            DRIVER.prepare(self.revision, self.revision, "b" * 40, label, output)
            DRIVER.validate_preparation(output, self.revision, self.revision, "b" * 40, label)
            if tamper:
                (self.directory / "measured-executable").write_bytes(b"tampered during timing")
            bundle = self.root / "bundle-manifest.json"
            bundle.write_text("{}")
            DRIVER.record_measurement(output, [str(self.executable),
                "typed_core_query_", "--save-baseline", label, "--bench"], 0, bundle)
        self.assertEqual(neighbour.read_bytes(), b"preserve dependency")
        self.assertEqual((self.revision / DRIVER.SOURCE).read_bytes(), self.original)
        return calls, output

    def test_cached_unwrapped_binary_is_rebuilt_before_measurement(self) -> None:
        calls, output = self.exercise_measurement()
        self.assertEqual(
            calls[:2], [[*DRIVER.CARGO_ARGS, "--no-run", "--message-format=json"]] * 2
        )
        self.assertEqual(len(calls), 2, "measurement must not start another Cargo build")
        self.assertTrue((output / "measurement.json").is_file())
        self.assertTrue((self.directory / "measured-executable").is_file())
        self.assertTrue((self.directory / "measured-dep-info").is_file())

    def test_fresh_compiler_receipt_avoids_unnecessary_recompilation(self) -> None:
        calls, _ = self.exercise_measurement(cached=False)
        self.assertEqual(len(calls), 1)

    def test_missing_wrapper_invocation_never_reaches_timing(self) -> None:
        with self.assertRaisesRegex(ValueError, "not a regular file"):
            self.exercise_measurement(cached=False, missing_receipt=True)
        self.assertFalse(
            (self.target / "kotodama-query-drivers/candidate/measurement.json").exists()
        )

    def test_executable_replacement_during_timing_cannot_complete_evidence(
        self,
    ) -> None:
        with self.assertRaisesRegex(ValueError, "executable hash"):
            self.exercise_measurement(tamper=True)
        self.assertFalse(
            (self.target / "kotodama-query-drivers/candidate/measurement.json").exists()
        )

    def test_failed_direct_launch_cannot_produce_measurement_receipt(self) -> None:
        _, output = self.exercise_measurement(cached=False)
        (output / "measurement.json").unlink()
        argv = [str(self.executable), "typed_core_query_",
                "--save-baseline", "candidate", "--bench"]
        with self.assertRaisesRegex(ValueError, "execution failed"):
            DRIVER.record_measurement(output, argv, 17, self.root / "bundle-manifest.json")
        self.assertFalse((output / "measurement.json").exists())

    def test_paired_evidence_requires_both_revisions_and_unchanged_environment(
        self,
    ) -> None:
        shutil.rmtree(
            self.directory.parent
        )  # Remove only this unit test's placeholder evidence.
        self.exercise_measurement(label="base")
        _, output = self.exercise_measurement(label="candidate")
        pair = output.parent
        with (
            mock.patch.object(DRIVER, "admit_sources"),
            mock.patch.dict(DRIVER.policy(), {"_git_head": lambda _: "b" * 40}),
        ):
            report = DRIVER.validate_pair(pair, self.revision, self.revision, "b" * 40)
            self.assertEqual(set(report), {"base", "candidate"})
            self.assertTrue(
                all(
                    len(row["contract"]["timed_body_sha256"]) == 46
                    for row in report.values()
                )
            )
            self.assertEqual(
                report["base"]["contract"]["fixture_permission"],
                report["candidate"]["contract"]["fixture_permission"],
            )
            self.assertEqual(
                report["base"]["contract"]["derived_setup_sha256"],
                report["candidate"]["contract"]["derived_setup_sha256"],
            )
            plan_path = self.directory / "plan.json"
            measured_path = output / "measurement.json"
            original_plan = plan_path.read_bytes()
            original_measurement = measured_path.read_bytes()
            for field in ("schema", "fixture_permission"):
                with self.subTest(field=field):
                    plan = json.loads(original_plan)
                    measured = json.loads(original_measurement)
                    if field == "schema":
                        plan["schema"] = measured["schema"] = (
                            "iroha.kotodama.derived-query-driver.v1"
                        )
                        reason = "unknown driver evidence schema"
                    else:
                        plan["contract"]["fixture_permission"]["grant_count"] = 2
                        reason = "driver source drift"
                    plan_path.write_text(json.dumps(plan))
                    measured["plan_sha256"] = DRIVER.sha(plan_path)
                    measured_path.write_text(json.dumps(measured))
                    with self.assertRaisesRegex(ValueError, reason):
                        DRIVER.validate_pair(
                            pair, self.revision, self.revision, "b" * 40
                        )
                    plan_path.write_bytes(original_plan)
                    measured_path.write_bytes(original_measurement)
            # Recompute the local plan hash to show that the independent paired
            # environment check, rather than a stale outer hash, catches drift.
            plan = DRIVER.read_json(plan_path)
            plan["build_environment"]["RUSTFLAGS"] = "-C target-cpu=native"
            plan_path.write_text(json.dumps(plan))
            measured = DRIVER.read_json(measured_path)
            measured["plan_sha256"] = DRIVER.sha(plan_path)
            measured_path.write_text(json.dumps(measured))
            with self.assertRaisesRegex(
                ValueError, "paired compiler environment differs"
            ):
                DRIVER.validate_pair(pair, self.revision, self.revision, "b" * 40)
            (pair / "base/measurement.json").unlink()
            with self.assertRaisesRegex(ValueError, "not a regular file"):
                DRIVER.validate_pair(pair, self.revision, self.revision, "b" * 40)

    def test_cargo_command_inventory_and_complete_workflow_evidence(self) -> None:
        self.assertEqual(
            DRIVER.CARGO_ARGS,
            [
                "bench",
                "--locked",
                "--jobs",
                "1",
                "-p",
                "iroha_core",
                "--features",
                "bench,iroha-core-tests",
                "--bench",
                "queries",
            ],
        )
        workflow = (ROOT / ".github/workflows/kotodama_perf.yml").read_text()
        self.assertEqual(workflow.count("--label base"), 2)
        self.assertEqual(workflow.count("--label candidate"), 2)
        self.assertIn(
            "--query-driver-root ../target-kotodama-perf/kotodama-query-drivers",
            workflow,
        )
        self.assertEqual(
            workflow.count("            target-kotodama-perf/kotodama-query-drivers\n"),
            2,
        )
        self.assertIn('("derived-query-drivers", drivers)', workflow)
        self.assertIn("timeout-minutes: 360", workflow)
        self.assertEqual(len(DRIVER.policy()["REPRESENTATIVE_BENCHMARKS"]), 46)


if __name__ == "__main__":
    unittest.main()
