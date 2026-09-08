"""Local preparation regressions; disposable files and a local Git index, no Cargo or network."""

import argparse
import contextlib
import hashlib
import importlib.util
import io
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

    def prepare(self, *, check=None, build=None, snapshot=None):
        def default_build(_root, _command, _env, log):
            self.binaries()
            log.write_bytes(b"fixture compiler output\n")
        def wrapped_build(*args, **kwargs):
            return (build or default_build)(*args)
        with patch.object(release, "verify_checkout", return_value="b" * 40), \
             patch.object(release, "source_snapshot", side_effect=snapshot or (lambda _: [])), \
             patch.object(release.shutil, "which", return_value=str(self.zigbuild)), \
             patch.object(release.gate, "run_checks", side_effect=check) as gate, \
             patch.object(release, "run_build", side_effect=wrapped_build) as compile, \
             patch.object(release, "capacity_preflight", return_value=[]), \
             contextlib.redirect_stdout(io.StringIO()):
            result = release.prepare(self.args)
        return result, gate, compile

    def test_prepare_orders_gate_build_capture_and_publishes_read_only_files(self):
        events = []
        def check(_root, *, environment):
            events.append("gate")
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(self.target))
        def build(_root, command, environment, log):
            events.append("build")
            self.assertEqual(environment["IROHA_GIT_COMMIT_HASH"], self.args.expected_commit)
            self.assertEqual(command, release.build_command(SCRIPT.parent.parent, self.target))
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
            with self.assertRaisesRegex(release.gate.CheckError, "fixture gate failed"):
                self.prepare(check=release.gate.CheckError("fixture gate failed"))
            build.assert_not_called()
        self.assertFalse(list(self.out.glob("attempts/*/bin")))
        self.assertFalse((self.out / "result.json").exists())

    def test_source_drift_stops_before_linux_build(self):
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
        self.assertEqual(build.call_args.args[1], release.build_command(SCRIPT.parent.parent, self.target))

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
        with patch.object(release, "verify_checkout", return_value="b" * 40), \
             patch.object(release, "source_snapshot", return_value=[]), \
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

    def test_build_command_uses_four_fixed_binaries_six_jobs_and_warm_lane(self):
        command = release.build_command(Path("/repo"), self.target)
        self.assertEqual(command[1:7], ["--target-dir", str(self.target), "--linker", "off", "--jobs", "6"])
        self.assertEqual(command.count("--bin"), 4)
        self.assertEqual(command[command.index("--profile") + 1], "release")
        self.assertNotIn("clean", command)

    def test_failed_build_keeps_diagnostic_log(self):
        class FailedChild:
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
        self.fixture_git("diff-files", "--quiet", "--", source.name)
        with self.assertRaisesRegex(release.PrepareError, "mode differs from the index"):
            release.source_snapshot(self.root)


if __name__ == "__main__":
    unittest.main()
