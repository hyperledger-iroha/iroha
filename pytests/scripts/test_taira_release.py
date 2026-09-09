"""Local preparation regressions; disposable files and a local Git index, no Cargo or network."""

import argparse
import ast
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("taira_release", SCRIPT)
assert SPEC and SPEC.loader
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


def elf(machine=183):
    header = bytearray(64)
    header[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HH", header, 16, 3, machine)
    return bytes(header) + b"disposable binary fixture"


class TairaPrepareTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.target = self.root / "target"
        self.target.mkdir()
        self.target_mode = stat.S_IMODE(self.target.stat().st_mode)
        self.out = self.root / "prepared"
        self.source = self.target / "frozen-source"
        self.zig, self.zigbuild = self.root / "zig", self.root / "cargo-zigbuild"
        for tool in (self.zig, self.zigbuild):
            tool.write_bytes(b"disposable tool fixture")
            tool.chmod(0o755)
        self.args = argparse.Namespace(
            repo_root=SCRIPT.parent.parent, target_dir=self.target,
            output_dir=self.out, expected_commit="a" * 40, expected_signer="A" * 40, zig=self.zig,
            zig_sha256=hashlib.sha256(self.zig.read_bytes()).hexdigest(),
            cargo_zigbuild=self.zigbuild,
            cargo_zigbuild_sha256=hashlib.sha256(self.zigbuild.read_bytes()).hexdigest(),
        )

    def tearDown(self):
        for path in [self.root, *self.root.rglob("*")]:
            if not path.is_symlink():
                path.chmod(0o700 if path.is_dir() else 0o600)
        self.directory.cleanup()

    def binaries(self, machine=183):
        output = self.target / release.TARGET / "release"
        output.mkdir(parents=True, exist_ok=True)
        for name, _ in release.BINARIES:
            path = output / name
            path.write_bytes(elf(machine))
            path.chmod(0o755)

    def prepare(self, *, check=None, build=None, snapshot=None, cache_admission=None):
        def default_build(_root, _command, _env, log):
            self.binaries()
            log.write_bytes(b"fixture compiler output\n")
        def wrapped_build(*args, **kwargs):
            return (build or default_build)(*args)
        with patch.object(release, "source_lane", side_effect=lambda *_: contextlib.nullcontext((self.source, 88))), \
             patch.object(release, "verify_checkout", return_value="b" * 40), \
             patch.object(release, "source_snapshot", return_value=[]), \
             patch.object(release, "verify_signed_source", return_value="b" * 40), \
             patch.object(release, "commit_entries", return_value=b""), \
             patch.object(release, "capture_source", return_value=self.source), \
             patch.object(release, "frozen_snapshot", side_effect=(lambda *_: snapshot(self.source)) if snapshot else (lambda *_: [])), \
             patch.object(release, "isolated_cargo_environment", side_effect=lambda _r, _s, env: (dict(env, CARGO="/fixed/cargo"), [])), \
             patch.object(release.shutil, "which", return_value=str(self.zigbuild)), \
             patch.object(release, "captured_gate", return_value=release.gate), \
             patch.object(release, "local_package_names", return_value=set()), \
             patch.object(release, "admit_source_fingerprints", side_effect=cache_admission or (lambda *_a, **_k: [])), \
             patch.object(release, "source_fingerprints", side_effect=lambda *_a, **_k: contextlib.nullcontext([])), \
             patch.object(release.gate, "run_checks", side_effect=check) as gate, \
             patch.object(release, "run_build", side_effect=wrapped_build) as compile, \
             patch.object(release, "capacity_preflight", return_value=[]), \
             contextlib.redirect_stdout(io.StringIO()):
            result = release.prepare(self.args)
        return result, gate, compile

    def test_prepare_orders_gate_build_capture_and_publishes_read_only_files(self):
        events = []
        def check(_root, *, environment, source_commit, lock_fds):
            events.append("gate")
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(self.target))
            self.assertEqual(len(lock_fds), 3)
            self.assertEqual(lock_fds[1], 88)  # Existing source-custody fixture descriptor.
            os.fstat(lock_fds[0])
            os.fstat(lock_fds[2])
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                with release.cargo_lane(self.args.repo_root, self.target, "release"):
                    self.fail("prepare mode lock must remain held through native checks")
        def build(_root, command, environment, log):
            events.append("build")
            self.assertEqual(environment["IROHA_GIT_COMMIT_HASH"], self.args.expected_commit)
            self.assertEqual(command, release.build_command(self.source, self.target, "/fixed/cargo"))
            self.binaries()
            log.write_bytes(b"fixture build\n")
        result, gate, compile = self.prepare(check=check, build=build)
        self.assertEqual(events, ["gate", "build"])
        self.assertEqual(gate.call_count, 1)
        self.assertEqual(compile.call_count, 1)
        self.assertEqual(len(result["artifacts"]), 4)
        self.assertFalse(result["release_qualified"])
        self.assertFalse(result["deployed"])
        for row in result["artifacts"]:
            path = Path(row["path"])
            self.assertEqual(path.read_bytes(), elf())
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), row["sha256"])
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o500)
        self.assertEqual(stat.S_IMODE((self.out / "result.json").stat().st_mode), 0o400)
        self.assertEqual(stat.S_IMODE(self.out.stat().st_mode), 0o500)
        self.assertEqual(stat.S_IMODE(self.target.stat().st_mode), self.target_mode)

    def test_failed_gate_never_starts_linux_build_or_capture(self):
        with patch.object(release, "run_build") as build:
            with self.assertRaisesRegex(release.PrepareError, "fixture gate failed"):
                self.prepare(check=release.gate.CheckError("fixture gate failed"))
            build.assert_not_called()
        self.assertFalse(list(self.out.glob("attempts/*/bin")))
        self.assertFalse((self.out / "result.json").exists())

    def test_captured_source_drift_stops_before_linux_build(self):
        snapshots = iter([[], [{"path": "changed"}]])
        with self.assertRaisesRegex(release.PrepareError, "source changed"):
            self.prepare(snapshot=lambda _: next(snapshots))
        self.assertFalse(list(self.out.glob("attempts/*/cargo.log")))
        self.assertFalse((self.out / "result.json").exists())

    def test_changed_tool_after_gate_stops_before_build(self):
        def check(*_args, **_kwargs):
            self.zig.write_bytes(b"different tool")
        with self.assertRaisesRegex(release.PrepareError, "reviewed executable"):
            self.prepare(check=check)
        self.assertFalse(list(self.out.glob("attempts/*/cargo.log")))
        self.assertFalse((self.out / "result.json").exists())

    def test_wrong_architecture_cannot_publish_result(self):
        def build(_root, _command, _env, log):
            self.binaries(machine=62)
            log.write_bytes(b"fixture wrong architecture\n")
        with self.assertRaisesRegex(release.PrepareError, "AArch64 Linux ELF"):
            self.prepare(build=build)
        self.assertFalse((self.out / "result.json").exists())

    def test_capture_rejects_symlink(self):
        self.binaries()
        self.out.mkdir()
        original = self.target / release.TARGET / "release" / release.BINARIES[0][0]
        original.rename(original.with_suffix(".retained"))
        original.symlink_to(original.with_suffix(".retained"))
        with self.assertRaises(release.ReleaseArtifactError):
            release.capture_artifacts(self.target, self.out)
        self.assertFalse((self.out / "result.json").exists())

    def test_artifact_replacement_after_hash_is_rejected(self):
        self.binaries()
        self.out.mkdir()
        original = self.target / release.TARGET / "release" / release.BINARIES[0][0]
        real_open = release.stable_open_relative
        def replace_before_open(root, relative, *, expected):
            original.rename(original.with_suffix(".retained"))
            original.write_bytes(elf())
            original.chmod(0o755)
            return real_open(root, relative, expected=expected)
        with patch.object(release, "stable_open_relative", side_effect=replace_before_open):
            with self.assertRaises(release.ReleaseArtifactError):
                release.capture_artifacts(self.target, self.out)

    def test_existing_output_and_symlink_target_fail_before_gate(self):
        self.out.mkdir()
        with patch.object(release.gate, "run_checks") as gate:
            with self.assertRaisesRegex(release.PrepareError, "fresh"):
                release.prepare(self.args)
            gate.assert_not_called()
        alias = self.root / "alias"
        alias.symlink_to(self.target, target_is_directory=True)
        with self.assertRaisesRegex(release.PrepareError, "symlinks"):
            release.real_path(alias)

    def test_same_command_reuses_completed_capture_without_gate_or_build(self):
        first, _, _ = self.prepare()
        second, gate, build = self.prepare()
        self.assertEqual(second, first)
        gate.assert_not_called()
        build.assert_not_called()
        self.assertEqual(len(list((self.out / "attempts").iterdir())), 1)

    def test_failed_build_retry_retains_log_and_reuses_gate_and_warm_lane(self):
        def failed(_root, _command, _environment, log):
            log.write_bytes(b"first build failed")
            raise release.PrepareError("fixture build failed")
        with self.assertRaisesRegex(release.PrepareError, "fixture build failed"):
            self.prepare(build=failed)
        original = self.out / "attempts/000001/cargo.log"
        before = original.read_bytes()
        result, gate, build = self.prepare()
        gate.assert_not_called()
        self.assertEqual(build.call_count, 1)
        self.assertEqual(result["attempt"], "attempts/000002")
        self.assertEqual(original.read_bytes(), before)
        self.assertEqual(build.call_args.args[1], release.build_command(self.source, self.target, "/fixed/cargo"))

    def test_crash_after_request_recovers_attempts_directory_before_work(self):
        real_create = release.create_fresh_directory
        def interrupt_attempts(path, *, mode):
            if path == self.out / "attempts":
                raise OSError("fixture crash after durable request")
            return real_create(path, mode=mode)
        with patch.object(release, "create_fresh_directory", side_effect=interrupt_attempts):
            with self.assertRaisesRegex(OSError, "fixture crash after durable request"):
                self.prepare()
        self.assertTrue((self.out / "request.json").is_file())
        self.assertFalse((self.out / "attempts").exists())
        result, gate, build = self.prepare()
        self.assertEqual(gate.call_count, 1)
        self.assertEqual(build.call_count, 1)
        self.assertEqual(result["attempt"], "attempts/000001")

    def test_cache_retirement_cannot_reuse_pass_after_failed_gate_rerun(self):
        def failed_build(_root, _command, _environment, log):
            log.write_bytes(b"fixture failed build")
            raise release.PrepareError("fixture build failed")
        with self.assertRaisesRegex(release.PrepareError, "fixture build failed"):
            self.prepare(build=failed_build)
        old_checks = (self.out / "checks.json").read_bytes()

        def retire(*_args, before_retire):
            before_retire()
            self.assertFalse((self.out / "checks.json").exists())
            return ["ivm"]

        with self.assertRaisesRegex(release.PrepareError, "fixture native failure"):
            self.prepare(cache_admission=retire, check=release.gate.CheckError("fixture native failure"))
        self.assertFalse((self.out / "checks.json").exists())
        self.assertEqual((self.out / "attempts/000002/retired-checks.json").read_bytes(), old_checks)
        result, gate, _build = self.prepare()
        self.assertEqual(gate.call_count, 1)
        self.assertEqual(result["attempt"], "attempts/000003")

    def test_capture_checkpoint_recovers_missing_final_result_without_build(self):
        real_write = release.write_record
        def fail_final(path, value):
            if path == self.out / "result.json":
                raise OSError("fixture crash before final result")
            real_write(path, value)
        with patch.object(release, "write_record", side_effect=fail_final):
            with self.assertRaisesRegex(OSError, "fixture crash"):
                self.prepare()
        result, gate, build = self.prepare()
        gate.assert_not_called()
        build.assert_not_called()
        self.assertEqual(result["attempt"], "attempts/000001")

    def test_changed_captured_binary_cannot_be_reused_or_trigger_build(self):
        result, _, _ = self.prepare()
        binary = Path(result["artifacts"][0]["path"])
        binary.chmod(0o700)
        binary.write_bytes(elf() + b"changed")
        binary.chmod(0o500)
        with patch.object(release, "run_build") as build:
            with self.assertRaisesRegex(release.PrepareError, "captured artifact changed"):
                self.prepare()
            build.assert_not_called()

    def test_changed_command_cannot_reuse_checkpoint(self):
        self.prepare()
        self.args.expected_commit = "c" * 40
        with self.assertRaisesRegex(release.PrepareError, "different inputs"):
            self.prepare()

    def test_concurrent_prepare_fails_without_waiting_or_breaking_lock(self):
        self.out.mkdir(mode=0o700)
        with release.preparation_lock(self.out):
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                with release.preparation_lock(self.out):
                    self.fail("second writer acquired held lock")

    def test_capacity_groups_same_filesystem_and_exact_boundary(self):
        from types import SimpleNamespace
        with patch.object(release.os, "fstatvfs", return_value=SimpleNamespace(f_bavail=100, f_frsize=1)):
            rows = release.capacity_preflight([(self.target, 60, "build"), (self.out, 40, "capture")])
            self.assertEqual(len(rows), 1)
            self.assertEqual(rows[0]["required_bytes"], 100)
            with self.assertRaisesRegex(release.PrepareError, "need 101 additional bytes"):
                release.capacity_preflight([(self.target, 60, "build"), (self.out, 41, "capture")])

    def test_low_space_stops_before_output_or_native_gate(self):
        with patch.object(release, "source_lane", side_effect=lambda *_: contextlib.nullcontext((self.source, 88))), \
             patch.object(release, "verify_checkout", return_value="b" * 40), \
             patch.object(release, "source_snapshot", return_value=[]), \
             patch.object(release, "verify_signed_source", return_value="b" * 40), \
             patch.object(release, "commit_entries", return_value=b""), \
             patch.object(release, "capture_source", return_value=self.source), \
             patch.object(release, "frozen_snapshot", return_value=[]), \
             patch.object(release, "isolated_cargo_environment", side_effect=lambda _r, _s, env: (dict(env, CARGO="/fixed/cargo"), [])), \
             patch.object(release.shutil, "which", return_value=str(self.zigbuild)), \
             patch.object(release, "capacity_preflight", side_effect=release.PrepareError("insufficient free space")), \
             patch.object(release.gate, "run_checks") as gate:
            with self.assertRaisesRegex(release.PrepareError, "insufficient free space"):
                release.prepare(self.args)
            gate.assert_not_called()
            self.assertFalse(self.out.exists())

    def test_build_progress_only_inspects_elapsed_time_and_log_metadata(self):
        class Child:
            count = 0
            def wait(self, timeout=None):
                self.count += 1
                if self.count == 1:
                    raise subprocess.TimeoutExpired("fixture", timeout)
                return 0
            def poll(self):
                return 0
        log = self.root / "progress.log"
        with patch.object(release.subprocess, "Popen", return_value=Child()) as spawn, \
             patch.object(release, "source_snapshot", side_effect=AssertionError("poll must not hash source")), \
             patch.object(release, "stable_hash_path", side_effect=AssertionError("poll must not hash artifacts")), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            release.run_build(self.root, ["fixture"], {}, log, lock_fd=77)
        self.assertIn("Linux build running", output.getvalue())
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77,))
        self.assertEqual(stat.S_IMODE(log.stat().st_mode), 0o600)

    def test_environment_excludes_secrets_hooks_and_compiler_overrides(self):
        env = release.child_environment({"PATH": "/bin", "HOME": "/fixture",
            "CARGO_TARGET_DIR": "/wrong", "RUSTFLAGS": "bad", "CARGO_PROFILE_RELEASE_LTO": "off",
            "IROHA_GIT_COMMIT_HASH": "stale", "GIT_DIR": "/wrong", "PYTHONPATH": "/untrusted",
            "LD_PRELOAD": "bad", "ONBOARDING_TOKEN": "fixture-private-value",
            "SSH_AUTH_SOCK": "/agent", "SCCACHE_DIR": "/warm/cache"}, self.target)
        self.assertEqual(env["CARGO_TARGET_DIR"], str(self.target))
        self.assertEqual(env["SCCACHE_DIR"], "/warm/cache")
        self.assertEqual(set(env), {"PATH", "HOME", "SCCACHE_DIR", "LC_ALL", "PYTHONNOUSERSITE",
                                    "PYTHONDONTWRITEBYTECODE", "CARGO_TARGET_DIR"})

    def test_native_incremental_admits_only_zero_or_one_without_release_environment_leak(self):
        environment = {"CARGO": "/fixed/cargo", "CARGO_TARGET_DIR": "/warm",
                       "RUSTC_WRAPPER": "/fixed/sccache", "CARGO_BUILD_JOBS": "6"}
        original = dict(environment)
        for inherited, expected in (({}, "1"), ({"CARGO_INCREMENTAL": "1"}, "1"),
                                    ({"CARGO_INCREMENTAL": "0"}, "0")):
            with self.subTest(inherited=inherited):
                native = release.native_check_environment(environment, inherited)
                expected_environment = original | {
                    "CARGO_INCREMENTAL": expected,
                    "CARGO_PROFILE_DEV_SPLIT_DEBUGINFO": "unpacked",
                    "CARGO_PROFILE_TEST_SPLIT_DEBUGINFO": "unpacked",
                }
                if expected == "1":
                    expected_environment.pop("RUSTC_WRAPPER")
                self.assertEqual(native, expected_environment)
                self.assertEqual(environment, original)
        for invalid in ("", "true", "false", "fixture-private-invalid-value"):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(release.PrepareError, "must be 0 or 1") as rejected:
                release.native_check_environment(environment, {"CARGO_INCREMENTAL": invalid})
            if invalid:
                self.assertNotIn(invalid, str(rejected.exception))

    def test_prepare_scopes_incremental_to_native_gate_and_binds_resume_preference(self):
        for preference in ("0", "1"):
            with self.subTest(preference=preference):
                self.out = self.root / ("prepared-incremental-" + preference)
                self.args.output_dir = self.out
                def check(_root, *, environment, source_commit, lock_fds):
                    self.assertEqual(environment["CARGO_INCREMENTAL"], preference)
                    self.assertEqual(environment["CARGO_TARGET_DIR"], str(self.target))
                    self.assertEqual(source_commit, self.args.expected_commit)
                def build(_root, command, environment, log):
                    self.assertNotIn("CARGO_INCREMENTAL", environment)
                    self.assertNotIn("CARGO_PROFILE_TEST_INCREMENTAL", environment)
                    self.assertNotIn("CARGO_PROFILE_DEV_SPLIT_DEBUGINFO", environment)
                    self.assertNotIn("CARGO_PROFILE_TEST_SPLIT_DEBUGINFO", environment)
                    self.assertEqual(command[command.index("--profile") + 1], "release")
                    self.binaries()
                    log.write_bytes(b"fixture compiler output\n")
                with patch.dict(os.environ, {"CARGO_INCREMENTAL": preference}):
                    _, gate, build = self.prepare(check=check, build=build)
                self.assertEqual(gate.call_count, 1)
                self.assertEqual(build.call_count, 1)
                request = release.read_record(self.out / "request.json")
                result = release.read_record(self.out / "result.json")
                self.assertEqual(request["native_incremental"], preference == "1")
                self.assertEqual(result["native_incremental"], preference == "1")
                # The public transfer consumer reconstructs the immutable result
                # identity from every request field except these three wrappers.
                transfer_base = {key: value for key, value in request.items()
                                 if key not in ("schema", "repo_root", "target_dir")}
                self.assertEqual(set(result), set(transfer_base) | {"artifacts", "timings_seconds", "attempt"})
                self.assertTrue(all(result[key] == value for key, value in transfer_base.items()))
                self.assertEqual(release.read_record(self.out / result["attempt"] / "capture.json"), result)
                with patch.dict(os.environ, {"CARGO_INCREMENTAL": "1" if preference == "0" else "0"}):
                    with self.assertRaisesRegex(release.PrepareError, "checkpoint belongs to different inputs"):
                        self.prepare()

    def test_build_command_uses_four_fixed_binaries_six_jobs_and_warm_lane(self):
        command = release.build_command(Path("/frozen"), self.target, "/fixed/cargo")
        self.assertEqual(command[:4], ["/fixed/cargo", "zigbuild", "--config", "/frozen/.cargo/config.toml"])
        self.assertEqual(command[4:6], ["--manifest-path", "/frozen/Cargo.toml"])
        self.assertEqual(command.count("--bin"), 4)
        self.assertEqual(command[command.index("--profile") + 1], "release")
        self.assertNotIn("clean", command)

    def test_failed_build_keeps_diagnostic_log(self):
        class FailedChild:
            pid = 123
            def wait(self, timeout=None):
                return 101
            def poll(self):
                return 101
        def spawn(_command, **kwargs):
            os.write(kwargs["stdout"], b"error: fixture compiler failure\n")
            return FailedChild()
        log = self.root / "cargo.log"
        with patch.object(release.subprocess, "Popen", side_effect=spawn):
            with self.assertRaisesRegex(release.PrepareError, "Linux build failed"):
                release.run_build(self.root, ["fixture"], {}, log)
        self.assertEqual(log.read_bytes(), b"error: fixture compiler failure\n")

    def test_unsigned_dirty_wrong_branch_or_wrong_commit_checkout_is_rejected(self):
        def response(*args):
            if args[1:] == ("rev-parse", "--show-toplevel"):
                return os.fsencode(self.root)
            if args[1:] == ("branch", "--show-current"):
                return b"optimizations"
            if args[1:] == ("rev-parse", "HEAD"):
                return self.args.expected_commit.encode()
            if args[1:] == ("rev-parse", "HEAD^{tree}"):
                return b"b" * 40
            if args[1:] == ("show", "--no-patch", "--format=%GF", self.args.expected_commit):
                return self.args.expected_signer.encode()
            return b""
        failures = [("branch", b"other"), ("status", b" M source.rs"),
                    ("rev-parse", b"c" * 40), ("verify-commit", None), ("show", b"B" * 40)]
        for command, result in failures:
            def failed(*args):
                if args[1] == command and args[2:] != ("--show-toplevel",):
                    if result is None:
                        raise release.PrepareError("signature verification failed")
                    return result
                return response(*args)
            with self.subTest(command=command), patch.object(release, "git", side_effect=failed):
                with self.assertRaises(release.PrepareError):
                    release.verify_checkout(self.root, self.args.expected_commit, self.args.expected_signer)
        with patch.object(release, "git", side_effect=response):
            self.assertEqual(release.verify_checkout(self.root, self.args.expected_commit, self.args.expected_signer), "b" * 40)

    def test_snapshot_supports_empty_tracked_files_without_reading_untracked_inputs(self):
        (self.root / "empty").touch()
        (self.root / "source.rs").write_bytes(b"source fixture\n")
        (self.root / "untracked-private").write_bytes(b"do not inspect fixture")
        def blob(payload):
            return hashlib.sha1(f"blob {len(payload)}\0".encode() + payload).hexdigest()
        listing = (f"100644 {blob(b'')} 0\tempty\0"
                   f"100644 {blob((self.root / 'source.rs').read_bytes())} 0\tsource.rs\0")
        with patch.object(release, "git", return_value=listing.encode()):
            snapshot = release.source_snapshot(self.root)
        self.assertEqual([row["path"] for row in snapshot], ["empty", "source.rs"])
        self.assertEqual(snapshot[0]["sha256"], hashlib.sha256(b"").hexdigest())

    def test_real_gitlink_index_allows_only_uninitialized_worktree(self):
        def git(*args):
            subprocess.run(["git", *args], cwd=self.root, check=True,
                           capture_output=True, env=release.child_environment(dict(os.environ), self.target))
        git("init", "--quiet")
        oid = "c" * 40
        git("update-index", "--add", "--cacheinfo", f"160000,{oid},iroha-docs")
        row, = release.source_snapshot(self.root)
        self.assertEqual((row["kind"], row["index_mode"], row["object"], row["checkout"]),
                         ("gitlink", "160000", oid, "absent"))
        link = self.root / "iroha-docs"
        link.mkdir()
        row, = release.source_snapshot(self.root)
        self.assertTrue(row["uninitialized"])
        self.assertEqual(row["checkout"], "empty")
        (link / "Cargo.toml").write_text("untracked submodule build input")
        with self.assertRaisesRegex(release.PrepareError, "uninitialized"):
            release.source_snapshot(self.root)
        (link / "Cargo.toml").unlink()
        link.rmdir()
        link.symlink_to(self.target, target_is_directory=True)
        with self.assertRaisesRegex(release.PrepareError, "uninitialized"):
            release.source_snapshot(self.root)
        link.unlink()
        git("update-index", "--cacheinfo", f"160000,{'d' * 40},iroha-docs")
        changed, = release.source_snapshot(self.root)
        self.assertNotEqual(changed["object"], row["object"])

    def test_index_merge_stages_are_rejected(self):
        with patch.object(release, "git", return_value=f"100644 {'a' * 40} 2\tsource.rs\0".encode()):
            with self.assertRaisesRegex(release.PrepareError, "merge stage"):
                release.source_snapshot(self.root)

    def fixture_git(self, *args, payload=None):
        return subprocess.run(["git", *args], cwd=self.root, check=True, input=payload,
                              capture_output=True,
                              env=release.child_environment(dict(os.environ), self.target)).stdout.strip()

    def test_git_reads_original_object_even_when_replacement_ref_exists(self):
        self.fixture_git("init", "--quiet")
        original = self.fixture_git("hash-object", "-w", "--stdin", payload=b"original fixture").decode()
        replacement = self.fixture_git("hash-object", "-w", "--stdin", payload=b"replacement fixture").decode()
        self.fixture_git("replace", original, replacement)
        self.assertEqual(self.fixture_git("cat-file", "-p", original), b"replacement fixture")
        self.assertEqual(release.git(self.root, "cat-file", "-p", original), b"original fixture")

    def test_snapshot_rejects_bytes_hidden_by_git_index_flags(self):
        self.fixture_git("init", "--quiet")
        source = self.root / "source.rs"
        original = b"original source fixture"
        source.write_bytes(original)
        self.fixture_git("add", "--", source.name)
        for flag in ("assume-unchanged", "skip-worktree"):
            with self.subTest(flag=flag):
                self.fixture_git("update-index", "--" + flag, "--", source.name)
                source.write_bytes(b"concealed compiler input")
                # Demonstrate Git's ordinary working-tree diff reports no change.
                self.fixture_git("diff-files", "--quiet", "--", source.name)
                with self.assertRaisesRegex(release.PrepareError, "bytes differ from the index"):
                    release.source_snapshot(self.root)
                source.write_bytes(original)
                self.fixture_git("update-index", "--no-" + flag, "--", source.name)
        release.source_snapshot(self.root)

    def test_snapshot_rejects_executable_mode_ignored_by_git(self):
        self.fixture_git("init", "--quiet")
        source = self.root / "source.rs"
        source.write_bytes(b"source fixture")
        source.chmod(0o644)
        self.fixture_git("add", "--", source.name)
        self.fixture_git("config", "core.filemode", "false")
        source.chmod(0o755)
        self.fixture_git("diff", "--quiet", "--", source.name)
        with self.assertRaisesRegex(release.PrepareError, "mode differs from the index"):
            release.source_snapshot(self.root)


    def source_entries(self, files):
        self.fixture_git("init", "--quiet")
        rows = []
        for name, (mode, payload) in sorted(files.items()):
            oid = ("c" * 40 if mode == "160000" else
                   self.fixture_git("hash-object", "-w", "--stdin", payload=payload).decode())
            rows.append(f"{mode} {oid} 0\t{name}".encode())
        return b"\0".join(rows) + b"\0"

    def test_fixed_capture_reads_git_objects_and_survives_working_source_changes(self):
        entries = self.source_entries({"source.rs": ("100644", b"signed source"),
                                       "run.sh": ("100755", b"#!/bin/sh\nexit 0\n"),
                                       "iroha-docs": ("160000", b"")})
        (self.root / "source.rs").write_bytes(b"concurrent unsaved edit")
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            self.assertEqual((source / "source.rs").read_bytes(), b"signed source")
            self.assertEqual(stat.S_IMODE((source / "run.sh").stat().st_mode), 0o500)
            subprocess.run([str(source / "run.sh")], check=True)
            self.assertFalse((source / ".git").exists())
            self.assertEqual(list((source / "iroha-docs").iterdir()), [])
            (self.root / "source.rs").write_bytes(b"another unrelated merge")
            self.assertEqual(release.capture_source(self.root, source, self.target, "a" * 40, entries), source)
            release.frozen_snapshot(source, entries, self.target)

    def test_lane_path_and_unchanged_mtimes_survive_next_commit(self):
        entries = self.source_entries({"same.rs": ("100644", b"same"), "edit.rs": ("100644", b"old")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            unchanged_mtime = (source / "same.rs").stat().st_mtime_ns
            original = source
        updated = self.source_entries({"same.rs": ("100644", b"same"), "edit.rs": ("100644", b"new")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            with patch.object(release, "commit_entries", return_value=entries):
                release.capture_source(self.root, source, self.target, "b" * 40, updated)
            self.assertEqual(source, original)
            self.assertEqual((source / "same.rs").stat().st_mtime_ns, unchanged_mtime)
            self.assertEqual((source / "edit.rs").read_bytes(), b"new")
            self.assertEqual(len(list(source.parent.glob("source.retained-*"))), 1)

    def test_captured_source_tampering_or_extra_files_cannot_resume(self):
        entries = self.source_entries({"source.rs": ("100644", b"signed")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            source.chmod(0o700)
            (source / "injected.rs").write_bytes(b"untracked build input")
            source.chmod(0o500)
            with self.assertRaisesRegex(release.PrepareError, "extra inputs"):
                release.capture_source(self.root, source, self.target, "a" * 40, entries)

    def test_capture_rejects_escaping_symlink_before_publication(self):
        entries = self.source_entries({"escape": ("120000", b"../../outside")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            with self.assertRaisesRegex(release.PrepareError, "symlink escapes"):
                release.capture_source(self.root, source, self.target, "a" * 40, entries)
            self.assertFalse(source.exists())

    def test_source_refresh_recovers_after_old_directory_was_retained(self):
        entries = self.source_entries({"source.rs": ("100644", b"old")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            updated = self.source_entries({"source.rs": ("100644", b"new")})
            rename = release.os.rename
            def fail_publication(src, dst):
                if Path(dst) == source:
                    raise OSError("fixture interrupted source publication")
                return rename(src, dst)
            with patch.object(release.os, "rename", side_effect=fail_publication), \
                 patch.object(release, "commit_entries", return_value=entries):
                with self.assertRaisesRegex(OSError, "interrupted"):
                    release.capture_source(self.root, source, self.target, "b" * 40, updated)
            self.assertFalse(source.exists())
            release.capture_source(self.root, source, self.target, "b" * 40, updated)
            self.assertEqual((source / "source.rs").read_bytes(), b"new")
            self.assertTrue(list(source.parent.glob("source.retained-*")))

    def test_source_lane_blocks_another_output_until_owner_releases_it(self):
        with release.source_lane(self.root, self.target):
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                with release.source_lane(self.root, self.target):
                    self.fail("another preparation replaced a live source lane")

    def test_working_checkout_is_never_rechecked_after_capture(self):
        def check(*_args, **kwargs):
            guard = patch.object(release, "source_snapshot", side_effect=AssertionError("mutable source read"))
            guard.start()
            self.addCleanup(guard.stop)
            self.assertEqual(kwargs["source_commit"], self.args.expected_commit)
            self.assertEqual(kwargs["lock_fds"][1], 88)
        result, _, build = self.prepare(check=check)
        self.assertEqual(build.call_args.args[0], self.source)
        self.assertEqual(result["source_root"], str(self.source))

    def test_frozen_native_fixture_output_uses_only_the_exact_warm_target(self):
        entries = self.source_entries({"crates/iroha_cli/source.rs": ("100644", b"signed")})
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            before = release.frozen_snapshot(source, entries, self.target)
            fixture = source / "crates/iroha_cli/../../target/disposable-native-fixture"
            fixture.mkdir()
            (fixture / "generated").write_bytes(b"fixture output, never inventoried")
            self.assertEqual(before, release.frozen_snapshot(source, entries, self.target))
            self.assertTrue((self.target / "disposable-native-fixture/generated").is_file())
            self.assertNotIn("target", [row["path"] for row in before])
            source.chmod(0o700)
            (source / "target").unlink()
            (source / "target").symlink_to(self.root, target_is_directory=True)
            source.chmod(0o500)
            with self.assertRaisesRegex(release.PrepareError, "output binding differs"):
                release.frozen_snapshot(source, entries, self.target)

    def test_native_gate_selection_is_loaded_from_verified_captured_source(self):
        code = b"SELECTION = ('captured regression',)\n"
        entries = self.source_entries({"scripts/taira_release_check.py": ("100644", code)})
        with release.source_lane(self.root, self.target) as (source, _fd):
            release.capture_source(self.root, source, self.target, "a" * 40, entries)
            before = release.frozen_snapshot(source, entries, self.target)
            (self.root / "scripts").mkdir()
            (self.root / "scripts/taira_release_check.py").write_text("raise RuntimeError('mutable gate must not execute')")
            selected = release.captured_gate(source, before)
            self.assertEqual(selected.SELECTION, ("captured regression",))

    def test_isolated_cargo_ignores_home_and_ancestor_configuration(self):
        self.source.mkdir()
        (self.source / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.93.1"\n')
        home = self.root / "home"
        cache = home / ".cargo"
        cache.mkdir(parents=True)
        for name in ("registry", "git"):
            (cache / name).mkdir()
        (cache / "config.toml").write_text('this fixture config must never be parsed')
        env = release.child_environment({"HOME": str(home), "PATH": "/fixture", "RUSTFLAGS": "injected"}, self.target)
        with patch.object(release.subprocess, "check_output", return_value=str(self.zig) + "\n"), \
             patch.object(release.shutil, "which", return_value=None):
            selected, tools = release.isolated_cargo_environment(self.root, self.source, env)
            reused, _ = release.isolated_cargo_environment(self.root, self.source, env)
        self.assertEqual(selected, reused)
        self.assertEqual(selected["CARGO_BUILD_JOBS"], "6")
        self.assertEqual(selected["CARGO_TARGET_DIR"], str(self.target))
        self.assertNotIn("RUSTFLAGS", selected)
        isolated = Path(selected["CARGO_HOME"])
        self.assertNotEqual(isolated, cache)
        self.assertEqual((isolated / "registry").resolve(), cache / "registry")
        self.assertFalse((isolated / "config.toml").exists())
        command = release.build_command(self.source, self.target, selected["CARGO"])
        self.assertEqual(command[2:4], ["--config", str(self.source / ".cargo/config.toml")])
        class Child:
            def wait(self, timeout=None): return 0
        with patch.object(release.subprocess, "Popen", return_value=Child()) as spawn:
            release.run_build(self.source, command, selected, self.root / "isolated.log", lock_fd=77, lane_lock_fd=88, mode_lock_fd=99)
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88, 99))


    def test_cache_initialization_closes_inheritable_lane_and_session_descriptors(self):
        observations = self.root / "cache-observation.json"
        cache = self.root / "sccache-fixture"
        cache.write_text(
            "#!" + sys.executable + "\n"
            "import json,os,sys\n"
            "seen=[]\n"
            "for fd in range(3,256):\n"
            " try:\n"
            "  info=os.fstat(fd);seen.append([info.st_dev,info.st_ino])\n"
            " except OSError:pass\n"
            "with open(" + repr(str(observations)) + ", 'w') as output:\n"
            " json.dump({'args':sys.argv[1:],'idle':os.environ.get('SCCACHE_IDLE_TIMEOUT'),'seen':seen},output)\n"
        )
        cache.chmod(0o755)
        env = release.child_environment(dict(os.environ), self.target)
        env.update(RUSTC_WRAPPER=str(cache), RUSTC=str(self.zig), SCCACHE_IDLE_TIMEOUT="600")
        with contextlib.ExitStack() as stack:
            held = []
            for name in ("mode", "source", "session"):
                directory = self.root / name
                directory.mkdir(mode=0o700)
                fd = stack.enter_context(release.preparation_lock(directory))
                os.set_inheritable(fd, True)
                held.append((fd, os.fstat(fd)))
            for _ in range(2):
                release.initialize_compiler_cache(env)
                observed = json.loads(observations.read_text())
                self.assertEqual(observed["args"], [str(self.zig), "--version"])
                self.assertEqual(observed["idle"], "0")
                for fd, info in held:
                    self.assertNotIn([info.st_dev, info.st_ino], observed["seen"])
                    self.assertEqual(os.fstat(fd).st_ino, info.st_ino)
                # The cache probe must not unlock the parent's live build custody.
                with self.assertRaisesRegex(release.PrepareError, "still running"):
                    with release.preparation_lock(self.root / "mode"):
                        self.fail("cache startup released the Cargo lane")

    def test_cache_initialization_failure_cannot_silently_continue_without_custody(self):
        env = {"RUSTC_WRAPPER": "/fixture/sccache", "RUSTC": "/fixture/rustc"}
        for failure in (subprocess.CompletedProcess([], 1),
                        subprocess.TimeoutExpired(["sccache"], 30), OSError("fixture failure")):
            with self.subTest(failure=type(failure).__name__), \
                 patch.object(release.subprocess, "run", side_effect=failure if isinstance(failure, Exception) else None,
                              return_value=failure) as run, \
                 self.assertRaisesRegex(release.PrepareError, "before Cargo"):
                release.initialize_compiler_cache(dict(env))
            self.assertEqual(run.call_args.args[0], ["/fixture/sccache", "/fixture/rustc", "--version"])
            self.assertTrue(run.call_args.kwargs["close_fds"])
            self.assertNotIn("pass_fds", run.call_args.kwargs)
            self.assertEqual(run.call_args.kwargs["timeout"], 30)

    def test_no_cache_wrapper_does_not_launch_a_cache_server(self):
        with patch.object(release.subprocess, "run") as run:
            release.initialize_compiler_cache({"RUSTC": "/fixture/rustc"})
        run.assert_not_called()

    def development_paths(self):
        repo = self.root / "repo"
        repo.mkdir()
        (repo / "target").mkdir()
        routine = release.routine_target(repo)
        routine.mkdir(parents=True)
        return repo, routine

    def test_development_target_defaults_and_explicit_selectors_must_agree(self):
        repo, routine = self.development_paths()
        self.assertEqual(release.development_target(repo, None, {"CARGO_TARGET_DIR": str(repo / "target")}), routine)
        selected = {"TAIRA_TESTNET_CARGO_TARGET_DIR": str(self.target)}
        self.assertEqual(release.development_target(repo, None, selected), self.target)
        self.assertEqual(release.development_target(repo, self.target, selected), self.target)
        with self.assertRaisesRegex(release.PrepareError, "conflicts"):
            release.development_target(repo, routine, selected)
        with self.assertRaisesRegex(release.PrepareError, "empty"):
            release.development_target(repo, None, {"TAIRA_TESTNET_CARGO_TARGET_DIR": ""})

    def test_missing_and_symlinked_development_target_never_create_lane(self):
        repo, _ = self.development_paths()
        missing = self.root / "missing"
        linked = self.root / "linked"
        linked.symlink_to(self.target, target_is_directory=True)
        for selected in (missing, linked):
            with self.subTest(selected=selected), self.assertRaises((OSError, release.PrepareError)):
                release.development_check(repo, selected, {})
        self.assertFalse(missing.exists())

    def test_reserved_and_marked_lanes_reject_mode_switches(self):
        repo, routine = self.development_paths()
        for lane, role in ((repo / "target", "development"), (routine, "release")):
            with self.subTest(lane=lane), self.assertRaises(release.PrepareError):
                with release.cargo_lane(repo, lane, role):
                    self.fail("must reject reserved lane")
            self.assertFalse((lane / ".taira-build-lane").exists())
        for original, changed in (("development", "release"), ("release", "development")):
            lane = self.root / original
            lane.mkdir()
            with release.cargo_lane(repo, lane, original):
                pass
            with self.assertRaisesRegex(release.PrepareError, "different build mode"):
                with release.cargo_lane(repo, lane, changed):
                    self.fail("must reject role switch")

    def test_existing_capture_lane_rejects_development_without_new_marker(self):
        repo, _ = self.development_paths()
        capture = self.target / "taira-release-sources"
        capture.mkdir()
        for lane in (self.target, capture):
            with self.assertRaisesRegex(release.PrepareError, "authenticated release lane"):
                with release.cargo_lane(repo, lane, "development"):
                    self.fail("must reject captured source lane")
            self.assertFalse((lane / ".taira-build-lane").exists())

    def test_lane_lock_is_held_through_the_gate_and_target_mode_is_unchanged(self):
        repo, routine = self.development_paths()
        target_mode = stat.S_IMODE(routine.stat().st_mode)
        descriptors = []
        def run(root, *, environment, lock_fds):
            self.assertEqual(root, repo)
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(routine))
            self.assertNotIn("PRIVATE_KEY", environment)
            self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
            self.assertNotIn("RUSTFLAGS", environment)
            self.assertNotIn("CARGO_BUILD_TARGET", environment)
            descriptors.extend(lock_fds)
            self.assertEqual(len(lock_fds), 1)
            os.fstat(lock_fds[0])
            with self.assertRaisesRegex(release.PrepareError, "still running"):
                with release.cargo_lane(repo, routine, "development"):
                    self.fail("must not admit competing check")
        with patch.object(release, "isolated_cargo_environment", side_effect=lambda _r, _s, env: (env, [])), \
             patch.object(release.gate, "run_checks", side_effect=run), contextlib.redirect_stdout(io.StringIO()):
            release.development_check(repo, None, {"PRIVATE_KEY": "fixture must not cross", "RUSTFLAGS": "bad", "CARGO_BUILD_TARGET": "bad", "CARGO_INCREMENTAL": "0"})
        with self.assertRaises(OSError):
            os.fstat(descriptors[0])
        self.assertEqual(stat.S_IMODE(routine.stat().st_mode), target_mode)
        with release.cargo_lane(repo, routine, "development"):
            pass

    def test_both_check_clis_share_the_development_entrypoint(self):
        repo, routine = self.development_paths()
        argv = ["taira_release.py", "check", "--repo-root", str(repo), "--target-dir", str(routine)]
        with patch.object(release.sys, "argv", argv), patch.object(release, "development_check") as check:
            self.assertEqual(release.main(), 0)
        self.assertEqual(check.call_args.args[:2], (repo, routine))
        argv = ["taira_release_check.py", "--repo-root", str(repo), "--target-dir", str(routine)]
        with patch.dict(sys.modules, {"taira_release": release}), patch.object(release.sys, "argv", argv), \
             patch.object(release, "development_check") as check:
            self.assertEqual(release.gate.main(), 0)
        self.assertEqual(check.call_args.args[:2], (repo, routine))

    def test_prepare_default_ignores_the_development_environment_selector(self):
        repo, routine = self.development_paths()
        argv = ["taira_release.py", "prepare", "--repo-root", str(repo), "--expected-commit", "a" * 40,
                "--expected-signer", "A" * 40, "--output-dir", str(self.out), "--zig", str(self.zig),
                "--zig-sha256", "b" * 64, "--cargo-zigbuild", str(self.zigbuild), "--cargo-zigbuild-sha256", "c" * 64]
        with patch.object(release.sys, "argv", argv), patch.dict(os.environ, {"TAIRA_TESTNET_CARGO_TARGET_DIR": str(routine)}), \
             patch.object(release, "prepare", return_value={"commit": "a" * 40}) as prepare, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(release.main(), 0)
        self.assertEqual(prepare.call_args.args[0].target_dir, repo / "target")

    def test_development_and_release_share_exact_isolated_registry_paths(self):
        repo, routine = self.development_paths()
        (repo / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.93.1"\n')
        source = repo / "target" / "captured-fixture"
        source.mkdir()
        (source / "rust-toolchain.toml").write_bytes((repo / "rust-toolchain.toml").read_bytes())
        cache = self.root / "home" / ".cargo"
        (cache / "registry").mkdir(parents=True)
        (cache / "git").mkdir()
        (cache / "config.toml").write_text("fixture must never be parsed")
        inherited = {"HOME": str(cache.parent), "PATH": "/fixture", "RUSTFLAGS": "injected"}
        with patch.object(release.subprocess, "check_output", return_value=str(self.zig) + "\n"), \
             patch.object(release.shutil, "which", return_value=None):
            development, _ = release.isolated_cargo_environment(repo, repo, release.child_environment(inherited, routine))
            authenticated, _ = release.isolated_cargo_environment(repo, source, release.child_environment(inherited, repo / "target"))
        self.assertEqual(development["CARGO_HOME"], authenticated["CARGO_HOME"])
        self.assertEqual(development["CARGO_BUILD_JOBS"], "6")
        self.assertEqual(development["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(development["CARGO"], authenticated["CARGO"])
        self.assertEqual((Path(development["CARGO_HOME"]) / "registry").resolve(), cache / "registry")
        self.assertFalse((Path(development["CARGO_HOME"]) / "config.toml").exists())
        self.assertNotIn("RUSTFLAGS", development)

    def test_development_lane_is_bound_to_the_canonical_repository(self):
        repo, routine = self.development_paths()
        other = self.root / "other-repository"
        other.mkdir()
        with release.cargo_lane(repo, routine, "development"):
            pass
        with self.assertRaisesRegex(release.PrepareError, "different build mode or repository"):
            with release.cargo_lane(other, routine, "development"):
                self.fail("another repository must select its own stable lane")

    def test_invalid_lane_markers_fail_without_blocking_or_reading_aliases(self):
        repo, _ = self.development_paths()
        for kind in ("fifo", "symlink", "hardlink"):
            lane = self.root / kind
            lane.mkdir()
            private = lane / ".taira-build-lane"
            private.mkdir(mode=0o700)
            marker = private / "role.json"
            if kind == "fifo":
                os.mkfifo(marker, 0o600)
            else:
                unrelated = lane / "unrelated"
                unrelated.write_text("fixture contents must not be accepted")
                unrelated.chmod(0o600)
                if kind == "symlink":
                    marker.symlink_to(unrelated)
                else:
                    os.link(unrelated, marker)
            with self.subTest(kind=kind), self.assertRaises((release.PrepareError, OSError)):
                with release.cargo_lane(repo, lane, "development"):
                    self.fail("unsafe marker must fail")

    def test_lane_contention_reports_the_actual_coordination_path(self):
        repo, routine = self.development_paths()
        with release.cargo_lane(repo, routine, "development"):
            with self.assertRaises(release.PrepareError) as raised:
                with release.cargo_lane(repo, routine, "development"):
                    self.fail("must not overlap")
        self.assertIn(str(routine / ".taira-build-lane"), str(raised.exception))
        self.assertNotIn("/attempts", str(raised.exception))

    def test_ci_selects_an_explicit_stable_development_target(self):
        workflow = (SCRIPT.parent.parent / ".github/workflows/workspace_release.yml").read_text()
        self.assertIn('mkdir -p "$GITHUB_WORKSPACE/target/taira-native-checks"', workflow)
        self.assertIn('taira_release_check.py --target-dir "$GITHUB_WORKSPACE/target/taira-native-checks"', workflow)
        self.assertLess(workflow.index('"fetch"'), workflow.index("python3 scripts/taira_release_check.py"))
        fetch_step = workflow.split("- name: Check Taira CLI release boundaries before workspace build", 1)[1].split("- name: Build the full workspace", 1)[0]
        fetch_python = fetch_step.split("python3 - <<'PY'\n", 1)[1].split("          PY", 1)[0]
        ast.parse("\n".join(line.removeprefix("          ") for line in fetch_python.splitlines()), filename="workflow-isolated-fetch")
        self.assertIn('release.isolated_cargo_environment(root, root, env)', fetch_python)
        self.assertIn('env["CARGO_NET_OFFLINE"] = "false"', fetch_python)
        self.assertIn('"--manifest-path", str(root / "Cargo.toml"), "--locked"', fetch_python)
        self.assertIn('pass_fds=(lock_fd,)', fetch_python)
        self.assertNotIn('"--offline"', fetch_python)
        full_build = workflow.split("- name: Build the full workspace", 1)[1].split("\n  doc:", 1)[0]
        for expected in ('target = root / "target/taira-native-checks"',
                         'release.cargo_lane(root, target, "development")',
                         'release.child_environment(dict(os.environ), target)',
                         'release.isolated_cargo_environment(root, root, env)',
                         'env["CARGO_INCREMENTAL"] = "0"',
                         'env["CARGO"], "--config", str(root / ".cargo/config.toml"), "build"',
                         '"--manifest-path", str(root / "Cargo.toml"), "--locked", "--offline", "--workspace"',
                         'subprocess.run(command, cwd="/", env=env', 'pass_fds=(lock_fd,)'):
            self.assertIn(expected, full_build)
        python = full_build.split("python3 - <<'PY'\n", 1)[1].split("          PY", 1)[0]
        ast.parse("\n".join(line.removeprefix("          ") for line in python.splitlines()), filename="workflow-full-build")
        repo, _ = self.development_paths()
        lane = repo / "target" / "taira-native-checks"
        lane.mkdir()
        with release.cargo_lane(repo, lane, "development"):
            pass
        with release.source_lane(repo, repo / "target"):
            pass


if __name__ == "__main__":
    unittest.main()
