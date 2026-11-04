#!/usr/bin/env python3
"""Exercise fresh Cargo launch capture and fail-closed prepared benchmark admission."""
from __future__ import annotations

import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("perf_bundle", ROOT / "scripts/kotodama_perf_bundle.py")
assert SPEC and SPEC.loader
BUNDLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUNDLE)


class PreparedBundleTests(unittest.TestCase):
    """Controls exercise mechanism boundaries; they do not qualify performance."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="iroha-prepared-bench-control-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.revision = self.root / "candidate"
        self.revision.mkdir()
        self.target = self.root / "target"
        self.target.mkdir()
        self.directory = self.target / "kotodama-prepared/candidate"
        self.directory.mkdir(parents=True)
        self.environment = mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.target)})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.toolchain = {"cargo": "/toolchain/bin/cargo", "rustc": "/toolchain/bin/rustc"}
        self.identity = {"head": "a" * 40, "lock_sha256": "b" * 64}
        self.run = {"GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "1"}
        self.nonce = "f" * 32
        benches = {}
        for name, (package, bench, source, _) in BUNDLE.RECIPES.items():
            output = self.directory / name
            output.mkdir()
            (output / "executable").write_bytes(f"compiled {name}".encode())
            (output / "executable").chmod(0o755)
            artifact = {"reason": "compiler-artifact", "fresh": False,
                        "executable": str(self.target / "release/deps" / (bench + "-1234567890abcdef")),
                        "target": {"name": bench, "kind": ["bench"], "crate_types": ["bin"],
                                   "src_path": str(self.revision / source)},
                        "manifest_path": str(self.revision / "crates" / package / "Cargo.toml"),
                        "profile": {"opt_level": "3"}}
            environment = {"CARGO": self.toolchain["cargo"], "CARGO_PKG_NAME": package,
                           "CARGO_MANIFEST_DIR": str(self.revision / "crates" / package),
                           "CARGO_MANIFEST_PATH": artifact["manifest_path"],
                           "LD_LIBRARY_PATH": str(self.target / "release/deps") + ":/toolchain/lib"}
            launch = {"argv": [artifact["executable"], *BUNDLE.benchmark_args(name, "candidate")],
                      "cwd": environment["CARGO_MANIFEST_DIR"], "environment": environment}
            BUNDLE.write_json(output / "launch.json", launch)
            benches[name] = {"artifact": artifact, "launch_artifact": {**artifact, "fresh": True},
                             "cargo_argv": [*BUNDLE.cargo_args(name), "--message-format=json", "--",
                                            *BUNDLE.benchmark_args(name, "candidate")[:-1]],
                             "sha256": BUNDLE.sha(output / "executable"), "libraries": {"libc": "identity"}}
        proof = self.directory / "query-driver"
        derived = proof / self.nonce
        derived.mkdir(parents=True)
        BUNDLE.write_json(proof / "preparation.json", {"nonce": self.nonce, "preparation_artifact": {}})
        for name in ("queries.rs", "compiler.json", "plan.json", "measured-dep-info"):
            (derived / name).write_text("{}")
        shutil.copy2(self.directory / "queries/executable", derived / "measured-executable")
        self.manifest = {"schema": BUNDLE.SCHEMA, "label": "candidate", "run": self.run,
                         "root": str(self.revision), "candidate": str(self.revision),
                         "target": str(self.target), "source": self.identity,
                         "toolchain": self.toolchain, "benches": benches,
                         "files": BUNDLE.file_inventory(self.directory)}
        self.save()
        for owner, name, value in ((BUNDLE, "run_identity", self.run),
                                   (BUNDLE, "source_identity", self.identity),
                                   (BUNDLE, "environment_identity", self.toolchain),
                                   (BUNDLE, "library_identity", {"libc": "identity"}),
                                   (BUNDLE.QUERY, "admit_sources", None),
                                   (BUNDLE.QUERY, "validate_preparation", None),
                                   (BUNDLE.QUERY, "validate_compilation", None)):
            patch = mock.patch.object(owner, name, return_value=value)
            patch.start()
            self.addCleanup(patch.stop)

    def save(self) -> None:
        (self.directory / "manifest.json").write_text(json.dumps(self.manifest, sort_keys=True))

    def admit(self) -> dict:
        return BUNDLE.admit(self.directory, self.revision, self.revision, "a" * 40, "candidate")

    def test_complete_admission_stages_and_validates_query_before_timing(self) -> None:
        self.assertEqual(self.admit(), self.manifest)
        BUNDLE.QUERY.validate_preparation.assert_called_once_with(
            self.target / "kotodama-query-drivers/candidate", self.revision,
            self.revision, "a" * 40, "candidate")
        self.assertFalse((self.target / "criterion").exists())

    def test_foreign_run_source_lock_toolchain_layout_and_label_fail(self) -> None:
        original = copy.deepcopy(self.manifest)
        mutations = (("run", {"GITHUB_RUN_ID": "old"}), ("source", {"lock_sha256": "wrong"}),
                     ("toolchain", {"cargo": "wrong"}), ("root", "/foreign"),
                     ("target", "/foreign"), ("label", "base"), ("schema", "retired"))
        for field, value in mutations:
            with self.subTest(field=field):
                self.manifest = copy.deepcopy(original)
                self.manifest[field] = value
                self.save()
                with self.assertRaises(ValueError):
                    self.admit()

    def test_native_header_package_version_drift_fails_before_timing(self) -> None:
        self.manifest["toolchain"] = {**self.toolchain,
            "native_packages": ["libc6-dev\tchanged-version\tii "]}
        self.save()
        with self.assertRaisesRegex(ValueError, "prepared compiler environment changed"):
            self.admit()
        BUNDLE.QUERY.validate_preparation.assert_not_called()
        self.assertFalse((self.target / "criterion").exists())

    def test_missing_or_extra_benchmarks_and_cached_compilation_fail(self) -> None:
        original = copy.deepcopy(self.manifest)
        for alteration in ("missing", "extra", "cached", "rebuilt_launch", "wrong_target", "foreign_output", "wrong_recipe"):
            with self.subTest(alteration=alteration):
                self.manifest = copy.deepcopy(original)
                benches = self.manifest["benches"]
                if alteration == "missing":
                    del benches["queries"]
                elif alteration == "extra":
                    benches["unknown"] = benches["ivm"]
                elif alteration == "cached":
                    benches["ivm"]["artifact"]["fresh"] = True
                elif alteration == "rebuilt_launch":
                    benches["ivm"]["launch_artifact"]["fresh"] = False
                elif alteration == "wrong_target":
                    benches["ivm"]["artifact"]["target"]["src_path"] = "/wrong.rs"
                elif alteration == "foreign_output":
                    benches["ivm"]["artifact"]["executable"] = "/foreign/bench_kotodama-1234567890abcdef"
                    benches["ivm"]["launch_artifact"]["executable"] = benches["ivm"]["artifact"]["executable"]
                else:
                    benches["ivm"]["cargo_argv"].append("--sample-size=1")
                self.save()
                with self.assertRaises(ValueError):
                    self.admit()

    def test_extra_samples_cannot_be_admitted_even_with_updated_inventory(self) -> None:
        for extra in ("criterion/new/estimates.json", "query-driver/measurement.json", "credentials"):
            with self.subTest(extra=extra):
                path = self.directory / extra
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("{}")
                self.manifest["files"] = BUNDLE.file_inventory(self.directory)
                self.save()
                with self.assertRaisesRegex(ValueError, "unexpected bundle files"):
                    self.admit()
                path.unlink()

    def test_modified_executable_fails_independent_hash_binding(self) -> None:
        (self.directory / "runtime/executable").write_bytes(b"replacement")
        self.manifest["files"] = BUNDLE.file_inventory(self.directory)
        self.save()
        with self.assertRaisesRegex(ValueError, "prepared executable changed"):
            self.admit()

    def test_runtime_library_drift_and_invalid_query_proof_fail_before_timing(self) -> None:
        BUNDLE.library_identity.return_value = {"libc": "changed"}
        with self.assertRaisesRegex(ValueError, "runtime library changed"):
            self.admit()
        BUNDLE.library_identity.return_value = {"libc": "identity"}
        BUNDLE.QUERY.validate_preparation.side_effect = ValueError("wrong query compiler nonce")
        with self.assertRaisesRegex(ValueError, "wrong query compiler nonce"):
            self.admit()
        self.assertFalse((self.target / "criterion").exists())

    def test_launch_argument_directory_environment_and_loader_injection_fail(self) -> None:
        path = self.directory / "ivm/launch.json"
        original = BUNDLE.read_json(path)
        mutations = [lambda x: x["argv"].pop(), lambda x: x.update(cwd="/tmp"),
                     lambda x: x["environment"].update(GITHUB_TOKEN="sentinel"),
                     lambda x: x["environment"].update(LD_PRELOAD="/evil.so"),
                     lambda x: x["environment"].update(LD_LIBRARY_PATH="relative"),
                     lambda x: x["environment"].update(LD_LIBRARY_PATH="/foreign"),
                     lambda x: x["environment"].update(LD_LIBRARY_PATH="/toolchain/lib:")]
        for mutate in mutations:
            launch = copy.deepcopy(original)
            mutate(launch)
            path.write_text(json.dumps(launch))
            self.manifest["files"] = BUNDLE.file_inventory(self.directory)
            self.save()
            with self.assertRaises(ValueError):
                self.admit()

    def test_runtime_child_and_recorded_build_env_are_closed(self) -> None:
        secrets = {"GITHUB_TOKEN": "secret-sentinel", "AWS_SECRET_ACCESS_KEY": "other-secret",
                   "LD_PRELOAD": "unapproved-loader", "LD_AUDIT": "unapproved-audit",
                   "CARGO_PROFILE_SECRET_TOKEN": "profile-secret", "IVM_GPU": "runtime-knob"}
        with mock.patch.dict(os.environ, secrets):
            child = BUNDLE.runtime_environment({"CARGO_PKG_NAME": "ivm"})
            build = BUNDLE.QUERY.build_environment()
        for key, value in secrets.items():
            self.assertNotIn(key, child)
            self.assertNotIn(key, build)
            self.assertNotIn(value, json.dumps([child, build]))
        self.assertEqual(child["CARGO_TARGET_DIR"], str(self.target))

    def test_build_children_drop_secrets_and_bind_known_controls(self) -> None:
        base = {"CARGO_BUILD_JOBS": "1", "CARGO_INCREMENTAL": "0", "RUSTUP_TOOLCHAIN": "1.99.1",
                "CARGO_PROFILE_BENCH_BUILD_OVERRIDE_OPT_LEVEL": "2", "GITHUB_TOKEN": "sentinel",
                "LD_PRELOAD": "sentinel-loader", "RUSTFLAGS": "-C opt-level=3"}
        with mock.patch.dict(os.environ, base, clear=True):
            child = BUNDLE.QUERY.compiler_environment()
            recorded = BUNDLE.QUERY.build_environment()
            self.assertEqual(child["CARGO_PROFILE_BENCH_BUILD_OVERRIDE_OPT_LEVEL"], "2")
            self.assertEqual(recorded["CARGO_PROFILE_BENCH_BUILD_OVERRIDE_OPT_LEVEL"], "2")
            self.assertNotIn("GITHUB_TOKEN", child)
            self.assertNotIn("LD_PRELOAD", child)
            for key in ("CARGO_PROFILE_SECRET", "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
                        "CARGO_BUILD_RUSTC", "RUSTC_BOOTSTRAP"):
                with mock.patch.dict(os.environ, {key: "never-record-this-value"}):
                    with self.assertRaisesRegex(ValueError, "unreviewed compiler environment key"):
                        BUNDLE.QUERY.compiler_environment()
            for flags in ("-C target-cpu=native", "-Ctarget-cpu=native", "-march=native", "-mtune=native"):
                with mock.patch.dict(os.environ, {"RUSTFLAGS": flags}):
                    with self.assertRaisesRegex(ValueError, "host-native tuning"):
                        BUNDLE.QUERY.compiler_environment()

    def test_non_executable_preparation_is_rejected_without_mode_mutation(self) -> None:
        executable = self.directory / "ivm/executable"
        executable.chmod(0o644)
        self.manifest["files"] = BUNDLE.file_inventory(self.directory)
        self.save()
        with self.assertRaisesRegex(ValueError, "execute mode"):
            BUNDLE.measure(self.directory, self.revision, self.revision, "a" * 40, "candidate")
        self.assertEqual(executable.stat().st_mode & 0o777, 0o644)

    def test_each_preparation_failure_stops_before_uploadable_bundle(self) -> None:
        original = copy.deepcopy(self.manifest)
        for failure_name in BUNDLE.RECIPES:
            with self.subTest(failure_name=failure_name):
                shutil.rmtree(self.directory)
                calls = []
                def cargo(argv, root, environment, name):
                    row = original["benches"][name]
                    artifact = row["artifact"]
                    if "--no-run" in argv:
                        calls.append(name)
                        if name == failure_name:
                            raise subprocess.CalledProcessError(19, argv)
                        path = Path(artifact["executable"])
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(f"compiled {name}".encode())
                        path.chmod(0o755)
                        return artifact
                    launch = {"argv": [artifact["executable"], *BUNDLE.benchmark_args(name, "candidate")],
                              "cwd": str(root / "crates" / BUNDLE.RECIPES[name][0]),
                              "environment": {"CARGO": self.toolchain["cargo"],
                                  "CARGO_PKG_NAME": BUNDLE.RECIPES[name][0],
                                  "CARGO_MANIFEST_DIR": str(root / "crates" / BUNDLE.RECIPES[name][0]),
                                  "CARGO_MANIFEST_PATH": artifact["manifest_path"]}}
                    BUNDLE.write_json(Path(environment[BUNDLE.CAPTURE_ENV]), launch)
                    return row["launch_artifact"]
                def query(*args):
                    calls.append("queries")
                    raise subprocess.CalledProcessError(19, BUNDLE.QUERY.CARGO_ARGS)
                with mock.patch.object(BUNDLE, "cargo", side_effect=cargo), \
                     mock.patch.object(BUNDLE.QUERY, "prepare", side_effect=query):
                    with self.assertRaises(subprocess.CalledProcessError):
                        BUNDLE.prepare(self.revision, self.revision, "a" * 40, "candidate", self.directory)
                expected = list(BUNDLE.RECIPES)[:list(BUNDLE.RECIPES).index(failure_name) + 1]
                self.assertEqual(calls, expected)
                self.assertFalse((self.directory / "manifest.json").exists())
                self.assertFalse(self.directory.with_suffix(".tar").exists())

    def test_serial_recipe_order_is_independent_of_sorted_manifest_keys(self) -> None:
        calls = []
        with mock.patch.object(BUNDLE.subprocess, "run", side_effect=lambda argv, **kw: calls.append((argv, kw))), \
             mock.patch.object(BUNDLE.QUERY, "record_measurement") as record:
            BUNDLE.measure(self.directory, self.revision, self.revision, "a" * 40, "candidate")
        self.assertEqual([Path(argv[0]).name for argv, _ in calls],
                         [BUNDLE.RECIPES[name][1] + "-1234567890abcdef" for name in BUNDLE.RECIPES])
        for name, (argv, _) in zip(BUNDLE.RECIPES, calls):
            self.assertEqual(argv, BUNDLE.read_json(self.directory / name / "launch.json")["argv"])
            self.assertEqual(BUNDLE.sha(Path(argv[0])), self.manifest["benches"][name]["sha256"])
        self.assertEqual([argv[1:] for argv, _ in calls], [BUNDLE.benchmark_args(name, "candidate") for name in BUNDLE.RECIPES])
        self.assertTrue(all(call[1]["check"] is True for call in calls))
        record.assert_called_once()

    def test_execution_failure_stops_later_benchmarks_and_evidence(self) -> None:
        for failure_at in (1, 2, 3):
            with self.subTest(failure_at=failure_at):
                query_output = self.target / "kotodama-query-drivers"
                if query_output.exists():
                    shutil.rmtree(query_output)
                calls = []
                def run(argv, **kwargs):
                    calls.append(argv)
                    if len(calls) == failure_at:
                        raise subprocess.CalledProcessError(17, argv)
                with mock.patch.object(BUNDLE.subprocess, "run", side_effect=run), \
                     mock.patch.object(BUNDLE.QUERY, "record_measurement") as record:
                    with self.assertRaises(subprocess.CalledProcessError):
                        BUNDLE.measure(self.directory, self.revision, self.revision, "a" * 40, "candidate")
                self.assertEqual(len(calls), failure_at)
                record.assert_not_called()

    def test_executable_tamper_during_timing_never_produces_query_evidence(self) -> None:
        def tamper(argv, **kwargs):
            Path(argv[0]).write_bytes(b"changed while running")
        with mock.patch.object(BUNDLE.subprocess, "run", side_effect=tamper), \
             mock.patch.object(BUNDLE.QUERY, "record_measurement") as record:
            with self.assertRaisesRegex(ValueError, "executable changed during timing"):
                BUNDLE.measure(self.directory, self.revision, self.revision, "a" * 40, "candidate")
        record.assert_not_called()

    def test_stale_measurement_receipt_is_rejected_on_initial_admission(self) -> None:
        self.admit()
        output = self.target / "kotodama-query-drivers/candidate/measurement.json"
        output.write_text("{}")
        with self.assertRaisesRegex(ValueError, "stale or changed query evidence"):
            self.admit()


class ArchiveAndCargoControls(unittest.TestCase):
    """Use actual archive extraction and the actual Cargo launcher for equivalence."""

    def test_native_loader_inventory_rejects_foreign_architecture_missing_and_foreign_libraries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            executable = root / "executable"
            header = bytearray(20)
            header[:7] = b"\x7fELF\x02\x01\x01"
            header[18:20] = b"\x3e\x00"
            executable.write_bytes(header)
            native = root / "native"
            native.mkdir()
            library = native / "libfixture.so"
            library.write_bytes(b"native library")
            toolchain = {"rustc": "/toolchain/bin/rustc", "python_libdir": str(native)}
            with mock.patch.dict(os.environ, {"CARGO_TARGET_DIR": str(root / "target"), "LD_AUDIT": "secret"}), \
                 mock.patch.object(BUNDLE, "command_output", return_value=f"libfixture.so => {library} (0x1234)") as inspect:
                result = BUNDLE.library_identity(executable, {}, toolchain)
                self.assertEqual(result["libfixture.so"]["sha256"], BUNDLE.sha(library))
                self.assertNotIn("LD_AUDIT", inspect.call_args.kwargs["env"])
                inspect.return_value = "libfixture.so => not found"
                with self.assertRaisesRegex(ValueError, "unresolved runtime library"):
                    BUNDLE.library_identity(executable, {}, toolchain)
                outside = root / "foreign.so"
                outside.write_bytes(b"foreign")
                inspect.return_value = f"libfixture.so => {outside} (0x1234)"
                with self.assertRaisesRegex(ValueError, "non-system runtime library"):
                    BUNDLE.library_identity(executable, {}, toolchain)
                header[18:20] = b"\xb7\x00"
                executable.write_bytes(header)
                inspect.reset_mock()
                with self.assertRaisesRegex(ValueError, "architecture"):
                    BUNDLE.library_identity(executable, {}, toolchain)
                inspect.assert_not_called()

    def test_source_inventory_hashes_bytes_modes_and_link_text_without_following(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "Cargo.lock").write_text("locked")
            source = root / "source.rs"
            source.write_text("original")
            (root / "link").symlink_to("missing-target")
            with mock.patch.object(BUNDLE.subprocess, "check_output", return_value=b"Cargo.lock\0source.rs\0link\0"), \
                 mock.patch.object(BUNDLE, "command_output", return_value="tree"), \
                 mock.patch.dict(BUNDLE.QUERY.policy(), {"_git_head": lambda _: "a" * 40}):
                original = BUNDLE.source_identity(root)
                self.assertEqual(original["file_count"], 3)
                source.write_text("changed")
                self.assertNotEqual(original, BUNDLE.source_identity(root))
                source.write_text("original")
                self.assertEqual(original, BUNDLE.source_identity(root))
                source.chmod(0o755)
                self.assertNotEqual(original, BUNDLE.source_identity(root))

    def test_run_identity_requires_exact_commit_and_numeric_run(self) -> None:
        environment = {"GITHUB_REPOSITORY": "owner/repo", "GITHUB_SHA": "a" * 40,
                       "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "1"}
        with mock.patch.dict(os.environ, environment):
            self.assertEqual(BUNDLE.run_identity("a" * 40), environment)
            with self.assertRaisesRegex(ValueError, "run identity"):
                BUNDLE.run_identity("b" * 40)
            for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT"):
                with mock.patch.dict(os.environ, {key: "foreign"}):
                    with self.assertRaisesRegex(ValueError, "run identity"):
                        BUNDLE.run_identity("a" * 40)

    def test_archive_rejects_escapes_links_duplicates_and_special_files(self) -> None:
        for kind in ("parent", "absolute", "symlink", "hardlink", "directory", "duplicate", "mode"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / "bad.tar"
                with tarfile.open(archive, "w") as packed:
                    member = tarfile.TarInfo("../outside" if kind == "parent" else "/outside" if kind == "absolute" else "file")
                    if kind in ("symlink", "hardlink", "directory"):
                        member.type = {"symlink": tarfile.SYMTYPE, "hardlink": tarfile.LNKTYPE,
                                       "directory": tarfile.DIRTYPE}[kind]
                        member.linkname = "/outside"
                    if kind == "mode":
                        member.mode = 0o4755
                    packed.addfile(member)
                    if kind == "duplicate":
                        packed.addfile(member)
                with self.assertRaises(ValueError):
                    BUNDLE.unpack(archive, root / "output")
                self.assertFalse((root / "output").exists())

    def test_archive_roundtrip_preserves_executable_bytes_and_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "good.tar"
            with tarfile.open(archive, "w") as packed:
                member = tarfile.TarInfo("bench/executable")
                member.mode = 0o755
                member.size = 5
                packed.addfile(member, io.BytesIO(b"bytes"))
            BUNDLE.unpack(archive, root / "output")
            executable = root / "output/bench/executable"
            self.assertEqual(executable.read_bytes(), b"bytes")
            self.assertEqual(executable.stat().st_mode & 0o777, 0o755)

    def test_real_cargo_capture_and_replay_match_native_launch(self) -> None:
        """A tiny std-only mechanism control, not an Iroha benchmark or target build."""
        with tempfile.TemporaryDirectory(prefix="iroha-cargo-launch-control-") as temporary:
            root = Path(temporary).resolve()
            (root / "Cargo.toml").write_text('[package]\nname="launch-control"\nversion="0.0.0"\nedition="2024"\n[[bench]]\nname="control"\npath="control.rs"\nharness=false\n')
            (root / "control.rs").write_text('''fn main() {
    for arg in std::env::args().skip(1) { println!("arg={arg}"); }
    println!("cwd={}", std::env::current_dir().unwrap().display());
    for key in ["CARGO_PKG_NAME", "CARGO_MANIFEST_DIR", "CARGO_MANIFEST_PATH"] {
        println!("{key}={}", std::env::var(key).unwrap());
    }
}''')
            env = dict(os.environ)
            env["CARGO_TARGET_DIR"] = str(root / "target")
            for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", BUNDLE.CAPTURE_ENV, BUNDLE.QUERY.PLAN_ENV):
                env.pop(key, None)
            command = ["cargo", "bench", "--bench", "control", "--", "--save-baseline", "base"]
            expected = subprocess.check_output(command, cwd=root, env=env, text=True, stderr=subprocess.PIPE)
            host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-Vv"], text=True).splitlines() if line.startswith("host: "))
            env["CARGO_TARGET_" + host.upper().replace("-", "_") + "_RUNNER"] = str(BUNDLE.SCRIPT)
            env[BUNDLE.CAPTURE_ENV] = str(root / "launch.json")
            env["GITHUB_TOKEN"] = "credential-must-never-be-recorded"
            actual = subprocess.check_output(command, cwd=root, env=env, text=True, stderr=subprocess.PIPE)
            self.assertEqual(actual, "", "preparation must not execute the benchmark")
            launch = BUNDLE.read_json(root / "launch.json")
            self.assertEqual(launch["argv"][1:], ["--save-baseline", "base", "--bench"])
            self.assertNotIn("GITHUB_TOKEN", launch["environment"])
            self.assertNotIn(env["GITHUB_TOKEN"], (root / "launch.json").read_text())
            replay_environment = {"PATH": "/usr/bin:/bin", "CARGO_TARGET_DIR": env["CARGO_TARGET_DIR"], **launch["environment"]}
            actual = subprocess.check_output(launch["argv"], cwd=launch["cwd"], env=replay_environment, text=True)
            self.assertEqual(actual, expected)

    def test_workflow_serial_measurements_require_both_builds_and_artifact_gate(self) -> None:
        workflow = (ROOT / ".github/workflows/kotodama_perf.yml").read_text()
        job = workflow.split("  representative-regression:\n", 1)[1].split("\n  kotodama-artifact-gate:", 1)[0]
        self.assertIn("needs: [prepare-baseline, prepare-candidate, kotodama-artifact-gate]", job)
        self.assertLess(job.index("Require exact same-run preparation artifact IDs"), job.index("Download exact baseline preparation"))
        self.assertLess(job.index("Admit both complete preparations before timing"), job.index("Measure base revision"))
        self.assertLess(job.index("Measure base revision"), job.index("Clear base current samples"))
        self.assertLess(job.index("Clear base current samples"), job.index("Measure candidate revision"))
        self.assertNotIn("cargo bench", job)
        self.assertNotIn("cargo build", job)
        self.assertIn("--threshold 0.05", job)
        self.assertEqual(len(BUNDLE.QUERY.policy()["REPRESENTATIVE_BENCHMARKS"]), 46)
        for prepare in ("prepare-baseline", "prepare-candidate"):
            section = workflow.split(f"  {prepare}:\n", 1)[1].split("\n  prepare-", 1)[0]
            self.assertIn("timeout-minutes: 360", section)


if __name__ == "__main__":
    unittest.main()
