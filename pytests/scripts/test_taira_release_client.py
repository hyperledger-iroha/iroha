"""Ordinary signed-source Mac client builds; no Cargo, network, or signing keys."""

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release.py"
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("taira_release_client_tests", SCRIPT)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
HOST = "aarch64-apple-darwin"


def executable(cpu=0x100000c):
    return struct.pack("<IIII", 0xfeedfacf, cpu, 0, 2) + b"native client test artifact"


class ClientBuildTests(unittest.TestCase):
    binary = "musubi"

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.target = self.root / "target/taira-macos-client"
        self.target.mkdir(parents=True, mode=0o700)
        self.output = self.root / "target/client-observation"
        self.source = self.target / "captured-source"
        self.source.mkdir(mode=0o700)
        self.args = argparse.Namespace(repo_root=self.root, output_dir=self.output, bin=self.binary,
                                       expected_commit="a" * 40, expected_signer="A" * 40)
        _, self.package, self.manifest, self.entry = release.CLIENT_BINARIES[self.binary]
        manifest = self.source / self.manifest
        manifest.parent.mkdir(parents=True)
        manifest.write_text('[package]\nname = "' + self.package + '"\nversion = "0.1.0"\n')
        self.tool = self.root / "rust-tool"
        self.tool.write_bytes(b"pinned test compiler")
        self.tool.chmod(0o700)
        info = release.stable_hash_path(self.tool)
        self.tools = [{"name": name, "path": str(self.tool), "sha256": info.sha256, "size": info.size}
                      for name in ("cargo", "rustc", "rustdoc")]
        self.emission_edit = lambda value: None
        self.after_build = lambda: None
        self.snapshots = lambda: [{"path": "Cargo.toml", "sha256": "c" * 64}]
        self.binary_bytes = executable()
        self.extra_emissions = []

    def tearDown(self):
        for path in [self.root, *self.root.rglob("*")]:
            if not path.is_symlink():
                path.chmod(0o700 if path.is_dir() else 0o600)
        self.temporary.cleanup()

    def compile(self, source, command, environment, log, **locks):
        self.assertEqual(source, self.source)
        self.assertEqual(command, release.client_build_command(self.source, self.target, str(self.tool), HOST, self.binary))
        self.assertNotIn("CARGO_BUILD_JOBS", environment)
        self.assertNotIn("CARGO_ZIGBUILD_ZIG_PATH", environment)
        self.assertNotIn("PRIVATE_KEY", environment)
        self.assertNotIn("RUSTFLAGS", environment)
        self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
        self.assertEqual(environment["IROHA_GIT_COMMIT_HASH"], self.args.expected_commit)
        self.assertEqual(environment["VERGEN_GIT_SHA"], self.args.expected_commit)
        self.assertEqual(locks["label"], "native " + self.binary + " build")
        os.fstat(locks["lock_fd"])
        os.fstat(locks["mode_lock_fd"])
        with self.assertRaisesRegex(release.PrepareError, "still running"):
            with release.cargo_lane(self.root, self.target, "release"):
                self.fail("client build lost its lane lock")
        binary = self.target / HOST / "debug" / self.binary
        binary.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        binary.write_bytes(self.binary_bytes)
        binary.chmod(0o700)
        manifest = source / self.manifest
        package_id = "path+" + manifest.parent.as_uri() + "#" + (
            "0.1.0" if manifest.parent.name == self.package else self.package + "@0.1.0")
        value = {"reason": "compiler-artifact", "package_id": package_id,
                 "manifest_path": str(manifest),
                 "target": {"name": self.binary, "kind": ["bin"], "src_path": str(source / self.entry)},
                 "profile": {"test": False}, "features": [], "filenames": [str(binary)],
                 "executable": str(binary), "fresh": False}
        self.emission_edit(value)
        log.write_text(''.join(json.dumps(row) + '\n' for row in self.extra_emissions + [value])
                       + '{"reason":"build-finished","success":true}\n')
        self.after_build()

    def run_client(self, build=None):
        linker = {"preference": "system", "platform": "darwin", "tools": {}}
        def isolate(_root, _source, env):
            return dict(env, CARGO=str(self.tool), RUSTC=str(self.tool), CARGO_BUILD_JOBS="6",
                        CARGO_ZIGBUILD_ZIG_PATH="unused"), self.tools
        with contextlib.ExitStack() as stack:
            for owner, name, options in [
                (release, "__file__", {"new": str(self.root / "scripts/taira_release.py")}),
                (release.sys, "platform", {"new": "darwin"}),
                (release, "verify_signed_source", {"return_value": "b" * 40}),
                (release, "commit_entries", {"return_value": b""}),
                (release, "signed_source_size", {"return_value": 0}),
                (release, "capacity_preflight", {"return_value": []}),
                (release, "source_lane", {"side_effect": lambda *_: contextlib.nullcontext((self.source, 88))}),
                (release, "capture_source", {"return_value": self.source}),
                (release, "frozen_snapshot", {"side_effect": lambda *_: self.snapshots()}),
                (release, "isolated_cargo_environment", {"side_effect": isolate}),
                (release, "preparation_native_linker", {"return_value": linker}),
                (release, "preparation_native_environment", {"side_effect": lambda env, _: env}),
                (release.subprocess, "check_output", {"return_value": "rustc fixture\nhost: " + HOST + "\n"}),
                (release, "local_package_names", {"return_value": {self.package}}),
                (release, "admit_source_fingerprints", {"return_value": []}),
                (release, "source_fingerprints", {"side_effect": lambda *_a, **_k: contextlib.nullcontext([])}),
                (release, "run_build", {"side_effect": build or self.compile}),
            ]:
                stack.enter_context(patch.object(owner, name, **options))
            stack.enter_context(patch.dict(os.environ, {"PRIVATE_KEY": "must not reach compiler", "RUSTFLAGS": "untrusted"}))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            return release.prepare_client(self.args)

    def test_client_build_captures_exact_source_tool_and_normal_binary_without_release_claim(self):
        result = self.run_client()
        self.assertEqual(result["commit"], self.args.expected_commit)
        self.assertEqual(result["compiler_tools"], self.tools)
        self.assertEqual(result["jobs"], "cargo-default")
        self.assertTrue(result["source_unchanged"])
        self.assertTrue(result["toolchain_unchanged"])
        self.assertFalse(result["release_qualified"])
        self.assertFalse(result["deployed"])
        self.assertEqual(result["binary"], self.binary)
        self.assertEqual(result["package"], self.package)
        self.assertEqual(result["artifact"]["sha256"], hashlib.sha256(executable()).hexdigest())
        self.assertEqual((self.output / "bin" / self.binary).read_bytes(), executable())
        self.assertEqual(stat.S_IMODE((self.output / "bin" / self.binary).stat().st_mode), 0o500)
        self.assertEqual(stat.S_IMODE((self.output / "result.json").stat().st_mode), 0o400)
        self.assertEqual(release.read_record(self.output / "result.json"), result)
        self.assertEqual(tuple(name for name, _ in release.BINARIES),
                         ("iroha3d_taira", "iroha", "sorafs-node", "kagami"))

    def test_foreign_or_test_harness_emission_never_publishes_success(self):
        for field in ("manifest_path", "executable", "profile"):
            with self.subTest(field=field):
                self.output = self.root / "target" / ("observation-" + field)
                self.args.output_dir = self.output
                self.emission_edit = lambda value, field=field: value.update(
                    {field: {"test": True} if field == "profile" else "/foreign/path"})
                with self.assertRaisesRegex(release.PrepareError, "differs from the captured"):
                    self.run_client()
                self.assertFalse((self.output / "result.json").exists())
                self.assertFalse((self.output / "bin" / self.binary).exists())

    def test_source_drift_after_build_retains_log_without_artifact(self):
        self.after_build = lambda: setattr(self, "snapshots", lambda: [{"changed": True}])
        with self.assertRaisesRegex(release.PrepareError, "source changed"):
            self.run_client()
        self.assertTrue((self.output / "cargo.jsonl").exists())
        self.assertFalse((self.output / "bin" / self.binary).exists())
        self.assertFalse((self.output / "result.json").exists())

    def test_tool_drift_after_build_retains_log_without_success(self):
        self.after_build = lambda: self.tool.write_bytes(b"changed compiler")
        with self.assertRaisesRegex(release.PrepareError, "reviewed executable"):
            self.run_client()
        self.assertFalse((self.output / "result.json").exists())

    def test_wrong_machine_artifact_never_publishes_success(self):
        self.binary_bytes = executable(0x1000007)
        with self.assertRaisesRegex(release.PrepareError, "native macOS executable"):
            self.run_client()
        self.assertFalse((self.output / "result.json").exists())

    def test_compiler_failure_keeps_request_and_log_without_success(self):
        def fail(*args, **kwargs):
            self.compile(*args, **kwargs)
            raise release.PrepareError("native Musubi build failed")
        with self.assertRaisesRegex(release.PrepareError, "build failed"):
            self.run_client(fail)
        self.assertTrue((self.output / "request.json").exists())
        self.assertTrue((self.output / "cargo.jsonl").exists())
        self.assertFalse((self.output / "result.json").exists())

    def test_completed_output_is_not_replayed_or_overwritten(self):
        self.run_client()
        before = (self.output / "result.json").read_bytes()
        with self.assertRaisesRegex(release.PrepareError, "fresh directory"):
            self.run_client(lambda *_a, **_k: self.fail("replayed completed client build"))
        self.assertEqual((self.output / "result.json").read_bytes(), before)

    def test_output_cannot_modify_the_source_or_cargo_lane(self):
        for selected in (self.target, self.source / "output", self.root / "outside-target",
                         self.root / "target/taira-macos-runtime/foreign-output"):
            self.args.output_dir = selected
            with self.subTest(selected=selected), self.assertRaisesRegex(release.PrepareError, "fresh directory"):
                self.run_client()

    def test_command_and_cli_are_narrow_and_do_not_change_shipping_selection(self):
        args = release.parser().parse_args(["prepare-client", "--expected-commit", "a" * 40,
                                          "--expected-signer", "A" * 40, "--bin", self.binary,
                                          "--output-dir", str(self.output)])
        self.assertEqual(args.command, "prepare-client")
        self.assertFalse(hasattr(args, "target_dir"))
        self.assertFalse(hasattr(args, "zig"))
        command = release.client_build_command(self.source, self.target, "/pinned/cargo", HOST, self.binary)
        self.assertEqual(command[-4:], ["-p", self.package, "--bin", self.binary])
        self.assertNotIn("--jobs", command)
        with self.assertRaisesRegex(release.PrepareError, "native macOS"):
            release.client_build_command(self.source, self.target, "/pinned/cargo", release.TARGET, self.binary)

    def test_foreign_package_identity_never_publishes_success(self):
        self.emission_edit = lambda value: value.update(package_id="path+file:///foreign#0.1.0")
        with self.assertRaisesRegex(release.PrepareError, "package identity differs"):
            self.run_client()
        self.assertFalse((self.output / "result.json").exists())

    def test_first_use_creates_only_the_authenticated_fixed_private_client_lane(self):
        moved = self.root / "target/retained-signed-source"
        self.source.rename(moved)
        self.source = moved
        self.target.rmdir()
        result = self.run_client()
        self.assertEqual(result["target_dir"], str(self.target))
        self.assertEqual(stat.S_IMODE(self.target.stat().st_mode), 0o700)

    def test_same_bytes_created_copy_substitution_preserves_foreign_inode(self):
        original = release.exclusive_output_fd
        foreign = {}
        @contextlib.contextmanager
        def substitute(path, **kwargs):
            with original(path, **kwargs) as fd:
                yield fd
            if path.name == self.binary and ".native-bin.pending-" in str(path):
                data = path.read_bytes()
                path.rename(path.with_name("original-created-copy"))
                path.write_bytes(data)
                path.chmod(0o700)
                foreign.update(path=path, identity=release.file_identity(path.lstat()), bytes=data)
        with patch.object(release, "exclusive_output_fd", side_effect=substitute):
            with self.assertRaisesRegex(release.PrepareError, "created copy custody changed"):
                self.run_client()
        self.assertEqual(release.file_identity(foreign["path"].lstat()), foreign["identity"])
        self.assertEqual(stat.S_IMODE(foreign["path"].stat().st_mode), 0o700)
        self.assertEqual(foreign["path"].read_bytes(), foreign["bytes"])
        self.assertFalse((self.output / "bin").exists())
        self.assertFalse((self.output / "result.json").exists())

    def test_same_bytes_cargo_source_substitution_preserves_foreign_inode(self):
        original = release.cargo_hash_path
        foreign = {}
        def substitute(path, **kwargs):
            pin = original(path, **kwargs)
            if path.name == self.binary and not foreign:
                data = path.read_bytes()
                path.rename(path.with_name("original-built-client"))
                path.write_bytes(data)
                path.chmod(0o700)
                foreign.update(path=path, identity=release.file_identity(path.lstat()), bytes=data)
            return pin
        with patch.object(release, "cargo_hash_path", side_effect=substitute):
            with self.assertRaises(release.ReleaseArtifactError):
                self.run_client()
        self.assertEqual(release.file_identity(foreign["path"].lstat()), foreign["identity"])
        self.assertEqual(foreign["path"].read_bytes(), foreign["bytes"])
        self.assertFalse((self.output / "bin").exists())
        self.assertFalse((self.output / "result.json").exists())


class IrohaClientBuildTests(ClientBuildTests):
    binary = "iroha"

    def test_sdk_library_emission_cannot_replace_or_duplicate_the_cli_binary(self):
        self.extra_emissions = [{"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["lib"]},
                                 "executable": None}]
        result = self.run_client()
        self.assertEqual(result["artifact"]["name"], "iroha")
        self.assertEqual(result["cargo_emission"]["target"]["kind"], ["bin"])


if __name__ == "__main__":
    unittest.main()
