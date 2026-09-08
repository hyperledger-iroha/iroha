from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "norito_bridge_source_seal.py"
APPLE_BUILDER = Path(__file__).parents[1] / "build_norito_xcframework.sh"
HERMETIC_RUNNER = Path(__file__).parents[1] / "run_mobile_hermetic_command.py"
ANDROID_BUILDER = Path(__file__).parents[2] / "kotlin/client-android/build.gradle.kts"
SPEC = importlib.util.spec_from_file_location("norito_bridge_source_seal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
seal = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = seal
SPEC.loader.exec_module(seal)


class NoritoBridgeSourceSealTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.source_seal_target = self.root / "source-seal-cargo-target"
        self.source_seal_target.mkdir()
        self.environment_patch = mock.patch.dict(
            os.environ,
            {
                "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR": str(
                    self.source_seal_target
                )
            },
            clear=False,
        )
        self.environment_patch.start()
        for relative, contents in {
            "Cargo.toml": "[workspace]\n",
            "Cargo.lock": "# locked\n",
            "rust-toolchain.toml": "[toolchain]\nchannel = 'stable'\n",
            "crates/connect_norito_bridge/NoritoBridge.podspec.template": "# podspec\n",
            "crates/connect_norito_bridge/RELEASE_NOTES.md": "# release\n",
            "IrohaSwift/IrohaSwift.podspec": "Pod::Spec.new {}\n",
            "IrohaSwift/Package.swift": "// package\n",
            "IrohaSwift/Package.resolved": '{"pins":[],"version":3}\n',
            "IrohaSwift/VERSION": "0.1.0\n",
            "IrohaSwift/Sources/IrohaSwift/Core.swift": "public struct Core {}\n",
            "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift": (
                "    private static let expectedHashes: [String: String] = [\n"
                '        "macos-arm64_x86_64": "' + ("1" * 64) + '",\n'
                '        "ios-arm64": "' + ("2" * 64) + '",\n'
                '        "ios-arm64_x86_64-simulator": "' + ("3" * 64) + '"\n'
                "    ]\n"
            ),
            "IrohaSwift/Sources/IrohaSwiftMobileTransports/Nfc.swift":
                "public struct Nfc {}\n",
            "scripts/build_norito_xcframework.sh": "#!/bin/sh\n",
            "scripts/archive_norito_xcframework.py": "#!/usr/bin/env python3\n",
            "scripts/check_mobile_sdk_artifact_pin_commit.py": "#!/usr/bin/env python3\n",
            "scripts/check_mobile_sdk_artifacts.sh": "#!/bin/sh\n",
            "scripts/exec_with_file_lock.py": "#!/usr/bin/env python3\n",
            "scripts/norito_bridge_source_seal.py": "# fixture\n",
            "scripts/norito_bridge_apple_slice_handoff.py": "#!/usr/bin/env python3\n",
            "scripts/package_mobile_sdk_artifacts.sh": "#!/bin/sh\n",
            "scripts/render_norito_bridge_podspec.py": "#!/usr/bin/env python3\n",
            "scripts/update_norito_bridge_swift_pins.py": "#!/usr/bin/env python3\n",
            "scripts/validate_norito_bridge_xcframework.py": "#!/usr/bin/env python3\n",
            "kotlin/client-android/build.gradle.kts": "// android\n",
        }.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents, encoding="utf-8")
        self.write_graph_owner((self.root / "Cargo.lock").read_bytes())
        self.git("init", "-q")
        self.git("config", "user.name", "Source Seal Test")
        self.git("config", "user.email", "source-seal@example.invalid")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", "fixture")

    def write_graph_owner(self, graph: bytes) -> None:
        owner = self.root / seal.CANONICAL_CARGO_LOCK_OWNER
        owner.parent.mkdir(parents=True, exist_ok=True)
        owner.write_text(
            "readonly PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256=\\\n"
            + '"' + seal.hashlib.sha256(graph).hexdigest() + '"\n', encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.environment_patch.stop()
        self.temporary.cleanup()

    def git(self, *arguments: str) -> bytes:
        environment = os.environ.copy()
        environment["GIT_CONFIG_GLOBAL"] = os.devnull
        environment["GIT_CONFIG_NOSYSTEM"] = "1"
        return subprocess.run(
            ["git", "-C", str(self.root), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        ).stdout

    def inputs(self, platform: str) -> list[str]:
        with mock.patch.object(seal, "local_dependency_roots", return_value=set()):
            return seal.seal_inputs(self.root, platform, lockfile_path=self.root / "Cargo.lock")

    def test_apple_seal_includes_package_lock_and_mobile_transports(self) -> None:
        apple = self.inputs("apple")
        self.assertIn("crates/connect_norito_bridge/NoritoBridge.podspec.template", apple)
        self.assertIn("crates/connect_norito_bridge/RELEASE_NOTES.md", apple)
        self.assertIn("IrohaSwift/IrohaSwift.podspec", apple)
        self.assertIn("IrohaSwift/Package.swift", apple)
        self.assertIn("IrohaSwift/Package.resolved", apple)
        self.assertIn("IrohaSwift/Sources/IrohaSwift", apple)
        self.assertIn("IrohaSwift/Sources/IrohaSwiftMobileTransports", apple)
        self.assertIn("IrohaSwift/VERSION", apple)
        self.assertIn("scripts/exec_with_file_lock.py", apple)
        self.assertIn("scripts/archive_norito_xcframework.py", apple)
        self.assertIn("scripts/norito_bridge_apple_slice_handoff.py", apple)
        self.assertIn("scripts/package_mobile_sdk_artifacts.sh", apple)
        self.assertIn("scripts/render_norito_bridge_podspec.py", apple)
        self.assertIn("scripts/update_norito_bridge_swift_pins.py", apple)
        self.assertIn("scripts/validate_norito_bridge_xcframework.py", apple)
        self.assertIn("scripts/check_mobile_sdk_artifact_pin_commit.py", apple)

        android = self.inputs("android")
        self.assertNotIn("IrohaSwift/Package.resolved", android)
        self.assertNotIn("IrohaSwift/Sources/IrohaSwiftMobileTransports", android)
        self.assertNotIn("scripts/exec_with_file_lock.py", android)
        self.assertNotIn("scripts/norito_bridge_apple_slice_handoff.py", android)
        self.assertIn("scripts/check_mobile_sdk_artifact_pin_commit.py", android)

    def test_apple_seal_requires_regular_package_resolution_lock(self) -> None:
        resolved = self.root / "IrohaSwift/Package.resolved"
        canonical = resolved.read_bytes()
        resolved.unlink()
        with self.assertRaisesRegex(RuntimeError, "source-seal input is missing"):
            self.inputs("apple")

        target = self.root / "replacement-Package.resolved"
        target.write_bytes(canonical)
        resolved.symlink_to(target)
        with self.assertRaisesRegex(RuntimeError, "source-seal input is not a regular file"):
            self.inputs("apple")

        resolved.unlink()
        resolved.write_bytes(canonical)
        self.assertIn("IrohaSwift/Package.resolved", self.inputs("apple"))

    def test_apple_fingerprint_normalizes_only_native_bridge_hash_pins(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock")
        loader = self.root / "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
        contents = loader.read_text(encoding="utf-8")
        loader.write_text(contents.replace("1" * 64, "a" * 64), encoding="utf-8")
        self.assertEqual(original, seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))

        loader.write_text(
            loader.read_text(encoding="utf-8") + "let changedLogic = true\n",
            encoding="utf-8",
        )
        self.assertNotEqual(original, seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))

    def test_apple_fingerprint_authenticates_a_tracked_source_deletion(self) -> None:
        inputs = self.inputs("apple")
        relative = "IrohaSwift/Sources/IrohaSwift/Core.swift"
        original = seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock")

        (self.root / relative).unlink()

        self.assertNotIn(relative, seal.listed_files(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))
        self.assertNotEqual(original, seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))
        self.assertIn(f" D {relative}", seal.status(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))

    def test_selected_lock_is_root_lock_in_metadata_and_fingerprint(self) -> None:
        root_lock = self.root / "Cargo.lock"
        with mock.patch.object(seal, "local_dependency_roots", return_value=set()):
            inputs = seal.seal_inputs(self.root, "apple", root_lock)

        original = seal.fingerprint(self.root, inputs, root_lock)
        root_lock.write_text("# changed root lock\n", encoding="utf-8")
        self.assertNotEqual(original, seal.fingerprint(self.root, inputs, root_lock))

        cargo = mock.Mock()
        rustc = mock.Mock()
        rustdoc = mock.Mock()
        git = Path("/usr/bin/git")
        with (
            mock.patch.object(
                seal, "source_seal_tools", return_value=(cargo, rustc, rustdoc, git)
            ),
            mock.patch.object(seal, "source_seal_environment", return_value={}),
            mock.patch.object(seal, "run", return_value=b"{}") as run,
        ):
            seal.metadata(self.root, "aarch64-apple-darwin", root_lock)
        arguments = run.call_args.args[2]
        self.assertIn("-Z", arguments)
        self.assertIn("unstable-options", arguments)
        self.assertEqual(
            arguments[arguments.index("--lockfile-path") + 1],
            str(root_lock),
        )

    def test_selected_lock_must_be_explicit_canonical_regular_and_non_symbolic(self) -> None:
        root_lock = self.root / "Cargo.lock"
        with self.assertRaisesRegex(RuntimeError, "explicit --lockfile-path"):
            seal.selected_lockfile_path(self.root)
        self.assertEqual(seal.selected_lockfile_path(self.root, root_lock), root_lock)

        with self.assertRaisesRegex(RuntimeError, "absolute and canonical"):
            seal.selected_lockfile_path(self.root, Path("Cargo.lock"))

        alternate = self.root / "alternate-Cargo.lock"
        alternate.write_text("# alternate lock\n", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "outside the source root"):
            seal.selected_lockfile_path(self.root, alternate)

        root_lock.unlink()
        with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
            seal.selected_lockfile_path(self.root, root_lock)

        root_lock.mkdir()
        with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
            seal.selected_lockfile_path(self.root, root_lock)
        root_lock.rmdir()

        root_lock.symlink_to(alternate)
        with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
            seal.selected_lockfile_path(self.root, root_lock)

    def test_reviewed_external_lock_binds_root_source_and_selected_graph_separately(self) -> None:
        with tempfile.TemporaryDirectory() as external_directory:
            external = Path(external_directory).resolve() / "Cargo.lock"
            root_lock = self.root / "Cargo.lock"
            original = root_lock.read_bytes()
            external.write_bytes(original)
            digest = seal.hashlib.sha256(original).hexdigest()
            self.assertEqual(seal.selected_lockfile_path(self.root, external), external)
            self.assertNotEqual(root_lock.stat().st_ino, external.stat().st_ino)
            with mock.patch.object(seal, "local_dependency_roots", return_value=set()):
                inputs = seal.seal_inputs(self.root, "apple", external)
            baseline = seal.fingerprint(self.root, inputs, external)
            root_lock.write_bytes(original + b"# root source change\n")
            with self.assertRaisesRegex(RuntimeError, "root source Cargo lock"):
                seal.fingerprint(self.root, inputs, external)
            # Ordinary explicit-root source inspection still observes its changed
            # source; it does not authorize an external privacy artifact build.
            changed_root = seal.fingerprint(self.root, inputs, root_lock)
            self.assertNotEqual(baseline, changed_root)
            self.assertIn("Cargo.lock", seal.status(self.root, inputs, root_lock))
            self.assertEqual(seal.lockfile_identity(external)[-1], digest)
            root_lock.write_bytes(original)
            external.write_bytes(b"unreviewed graph\n")
            with self.assertRaisesRegex(RuntimeError, "canonical reviewed graph"):
                seal.fingerprint(self.root, inputs, external)
            # A separately reviewed fixture graph updates the sole fixture owner
            # and both physical locks coherently; no production pin is relaxed.
            root_lock.write_bytes(external.read_bytes())
            self.write_graph_owner(external.read_bytes())
            self.assertNotEqual(changed_root, seal.fingerprint(self.root, inputs, external))

    def test_external_lock_rejects_unreviewed_graph_and_symbolic_ancestors(self) -> None:
        with tempfile.TemporaryDirectory() as external_directory:
            directory = Path(external_directory).resolve()
            external = directory / "Cargo.lock"
            external.write_bytes(b"unreviewed unit-test graph\n")
            with self.assertRaisesRegex(RuntimeError, "canonical reviewed graph"):
                seal.selected_lockfile_path(self.root, external)
            alias = self.root / "lock-alias"
            alias.symlink_to(directory, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
                seal.selected_lockfile_path(self.root, alias / "Cargo.lock")

    def test_external_metadata_receives_exact_selected_path_and_retains_locked_offline(self) -> None:
        with tempfile.TemporaryDirectory() as external_directory:
            external = Path(external_directory).resolve() / "Cargo.lock"
            external.write_bytes((self.root / "Cargo.lock").read_bytes())
            with (
                mock.patch.object(seal, "source_seal_tools", return_value=(mock.Mock(), mock.Mock(), mock.Mock(), Path("/usr/bin/git"))),
                mock.patch.object(seal, "source_seal_environment", return_value={}),
                mock.patch.object(seal, "run", return_value=b"{}") as run,
            ):
                seal.metadata(self.root, "aarch64-apple-darwin", external)
            arguments = run.call_args.args[2]
            self.assertEqual(arguments[arguments.index("--lockfile-path") + 1], str(external))
            self.assertIn("--locked", arguments)
            self.assertIn("--offline", arguments)
            self.assertIn("-Z", arguments)
            self.assertIn("unstable-options", arguments)

    def test_canonical_graph_owner_is_unique_strict_and_part_of_both_platform_seals(self) -> None:
        owner = self.root / seal.CANONICAL_CARGO_LOCK_OWNER
        original = owner.read_text()
        digest = seal.hashlib.sha256((self.root / "Cargo.lock").read_bytes()).hexdigest()
        self.assertEqual(seal.canonical_cargo_lock_sha256(self.root), digest)
        self.assertIn(seal.CANONICAL_CARGO_LOCK_OWNER, seal.COMMON_ROOT_INPUTS)
        for source in (
            original + original,
            original + original.replace("readonly ", "readonly\t"),
            original.replace(digest, digest.upper()),
            original.replace(digest, "0" * 63),
            original.replace("PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256", "OLD_GRAPH_OWNER"),
        ):
            with self.subTest(source=source):
                owner.write_text(source)
                with self.assertRaisesRegex(RuntimeError, "canonical Cargo graph declaration"):
                    seal.canonical_cargo_lock_sha256(self.root)
        owner.write_bytes(b"")
        with self.assertRaisesRegex(RuntimeError, "between 1 byte and 16 MiB"):
            seal.canonical_cargo_lock_sha256(self.root)
        owner.write_text(original)
        with mock.patch.object(seal, "local_dependency_roots", return_value=set()):
            inputs = seal.seal_inputs(self.root, "apple", self.root / "Cargo.lock")
        before = seal.fingerprint(self.root, inputs, self.root / "Cargo.lock")
        owner.write_text(original + "# source owner edit\n")
        self.assertNotEqual(before, seal.fingerprint(self.root, inputs, self.root / "Cargo.lock"))
        replacement = owner.with_suffix(".replacement")
        owner.rename(replacement)
        owner.symlink_to(replacement)
        with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
            seal.canonical_cargo_lock_sha256(self.root)

    def test_external_graph_snapshot_rejects_hardlinks_and_executable_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            external = Path(directory).resolve() / "Cargo.lock"
            os.link(self.root / "Cargo.lock", external)
            with self.assertRaisesRegex(RuntimeError, "singly linked"):
                seal.selected_lockfile_path(self.root, external)
            external.unlink()
            external.write_bytes((self.root / "Cargo.lock").read_bytes())
            external.chmod(0o700)
            with self.assertRaisesRegex(RuntimeError, "non-executable"):
                seal.selected_lockfile_path(self.root, external)

    def test_lock_reader_rejects_fifo_before_open_and_fifo_substitution(self) -> None:
        fifo = self.root / "nonregular-Cargo.lock"
        os.mkfifo(fifo, 0o600)
        with mock.patch.object(seal.os, "open", side_effect=AssertionError("must not open a known FIFO")) as opened:
            with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
                seal.lockfile_identity(fifo)
        opened.assert_not_called()
        root_lock = self.root / "Cargo.lock"
        real_open = os.open
        def substitute_fifo(candidate, flags):
            self.assertEqual(candidate, root_lock)
            self.assertTrue(flags & os.O_NONBLOCK)
            return real_open(fifo, flags)
        with mock.patch.object(seal.os, "open", side_effect=substitute_fifo):
            with self.assertRaisesRegex(RuntimeError, "non-symbolic regular file"):
                seal.lockfile_identity(root_lock)

    def test_canonical_graph_owner_rejects_a_descriptor_from_another_inode(self) -> None:
        owner = self.root / seal.CANONICAL_CARGO_LOCK_OWNER
        replacement = self.root / "same-byte-owner-copy.sh"
        replacement.write_bytes(owner.read_bytes())
        real_open = os.open
        def substituted_open(candidate, flags):
            self.assertEqual(candidate, owner)
            self.assertTrue(flags & getattr(os, "O_NOFOLLOW", 0))
            return real_open(replacement, flags)
        with mock.patch.object(seal.os, "open", side_effect=substituted_open):
            with self.assertRaisesRegex(RuntimeError, "changed while being authenticated"):
                seal.canonical_cargo_lock_sha256(self.root)

    def test_source_seal_cargo_environment_binds_jobs_rustdoc_and_fixed_target(
        self,
    ) -> None:
        tools = self.root / "source-seal-tools"
        target = self.root / "source-seal-target"
        tools.mkdir()
        target.mkdir()
        for name in ("cargo", "rustc", "rustdoc"):
            executable = tools / name
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
        configured = {
            "NORITO_BRIDGE_SEAL_CARGO": str(tools / "cargo"),
            "NORITO_BRIDGE_SEAL_RUSTC": str(tools / "rustc"),
            "NORITO_BRIDGE_SEAL_RUSTDOC": str(tools / "rustdoc"),
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR": str(target),
        }
        with (
            mock.patch.dict(os.environ, configured, clear=False),
            mock.patch.object(seal, "source_seal_home", return_value=self.root),
        ):
            cargo, rustc, rustdoc, git = seal.source_seal_tools()
            environment = seal.source_seal_environment(
                cargo=cargo,
                rustc=rustc,
                rustdoc=rustdoc,
                git=git,
            )
        self.assertEqual(environment["CARGO_BUILD_JOBS"], "1")
        self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
        self.assertEqual(environment["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(target))
        self.assertEqual(environment["RUSTC"], str(tools / "rustc"))
        self.assertEqual(environment["RUSTDOC"], str(tools / "rustdoc"))

        missing_target = dict(configured)
        del missing_target["NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR"]
        with (
            mock.patch.dict(os.environ, missing_target, clear=True),
            mock.patch.object(seal, "source_seal_home", return_value=self.root),
        ):
            cargo, rustc, rustdoc, git = seal.source_seal_tools()
            with self.assertRaisesRegex(
                RuntimeError, "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR is required"
            ):
                seal.source_seal_environment(
                    cargo=cargo,
                    rustc=rustc,
                    rustdoc=rustdoc,
                    git=git,
                )

    def test_apple_fingerprint_and_dirty_state_bind_mobile_transport_bytes(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock")
        transport = self.root / "IrohaSwift/Sources/IrohaSwiftMobileTransports/Nfc.swift"
        transport.write_text("public struct MutatedNfc {}\n", encoding="utf-8")

        self.assertNotEqual(original, seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))
        self.assertIn("Nfc.swift", seal.status(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))

    def test_apple_fingerprint_binds_package_resolution_and_untracked_transport(self) -> None:
        inputs = self.inputs("apple")
        original = seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock")
        resolved = self.root / "IrohaSwift/Package.resolved"
        resolved.write_text('{"pins":[{"identity":"changed"}],"version":3}\n', encoding="utf-8")
        changed_lock = seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock")
        self.assertNotEqual(original, changed_lock)

        extra = self.root / "IrohaSwift/Sources/IrohaSwiftMobileTransports/Extra.swift"
        extra.write_text("public struct Extra {}\n", encoding="utf-8")
        self.assertNotEqual(changed_lock, seal.fingerprint(self.root, inputs, lockfile_path=self.root / "Cargo.lock"))
        dirty = seal.status(self.root, inputs, lockfile_path=self.root / "Cargo.lock")
        self.assertIn("Package.resolved", dirty)
        self.assertIn("Extra.swift", dirty)

    def test_unknown_platform_fails_closed(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "unsupported source-seal platform"):
            self.inputs("windows")

    def test_apple_builder_never_relies_on_the_default_seal_platform(self) -> None:
        builder = APPLE_BUILDER.read_text(encoding="utf-8")
        invocations = re.findall(
            r"run_source_seal (?:fingerprint|status)[^\n]*",
            builder,
        )
        self.assertEqual(len(invocations), 2)
        for invocation in invocations:
            self.assertIn("--platform apple", invocation)
        self.assertGreaterEqual(
            builder.count('--lockfile-path "$CARGO_LOCKFILE"'),
            3,
        )

    def test_apple_builder_uses_one_selected_lock_for_all_five_builds(self) -> None:
        builder = APPLE_BUILDER.read_text(encoding="utf-8")
        self.assertEqual(builder.count("run_hermetic_apple_cargo \\\n"), 5)
        self.assertIn(
            '-Z unstable-options --lockfile-path "$CARGO_LOCKFILE"',
            builder,
        )
        self.assertIn('--set "RUSTC_BOOTSTRAP=1"', builder)
        self.assertIn('--set "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"', builder)
        self.assertIn('--set "RUSTDOC=$RUSTDOC_BINARY"', builder)
        self.assertEqual(
            builder.count('--set "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"'),
            1,
        )
        self.assertNotIn("CARGO_BUILD_DIR_", builder)
        self.assertNotRegex(builder, r"rm -rf[^\n]*CARGO_TARGET_DIR")
        self.assertNotIn("write_static_xcframework_info_plist", builder)
        self.assertNotIn("copy_static_xcframework_slice", builder)
        self.assertNotIn("rebuilding the fallback", builder)
        self.assertIn("xcodebuild_status=$?", builder)
        self.assertIn('exit "$xcodebuild_status"', builder)
        self.assertIn(
            "Cargo target, build, and output directories must be pairwise disjoint",
            builder,
        )
        self.assertIn(
            '"cargo_lock_sha256": "$CARGO_LOCK_SHA256_START"',
            builder,
        )

        runner = HERMETIC_RUNNER.read_text(encoding="utf-8")
        self.assertIn('"CARGO_BUILD_JOBS"', runner)
        self.assertIn('"RUSTDOC"', runner)

    def test_apple_hermetic_runner_rejects_incomplete_or_noncanonical_envelope(
        self,
    ) -> None:
        tools = self.root / "hermetic-tools"
        target = self.root / "cargo-target"
        tools.mkdir()
        target.mkdir()
        for name in ("cargo", "rustc", "rustdoc"):
            executable = tools / name
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
        environment = {
            "CARGO": str(tools / "cargo"),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_HOME": str(self.root / "cargo-home"),
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TARGET_DIR": str(target),
            "CONNECT_NORITO_SOURCE_REVISION": "1" * 40,
            "DEVELOPER_DIR": str(self.root / "developer"),
            "HOME": str(self.root),
            "IPHONEOS_DEPLOYMENT_TARGET": "15.0",
            "IROHA_GIT_COMMIT_HASH": "1" * 40,
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "NORITO_SKIP_BINDINGS_SYNC": "1",
            "PATH": f"{tools}:/usr/bin:/bin",
            "RUSTC": str(tools / "rustc"),
            "RUSTC_BOOTSTRAP": "1",
            "RUSTDOC": str(tools / "rustdoc"),
            "RUSTUP_HOME": str(self.root / "rustup-home"),
            "SDKROOT": str(self.root / "sdk"),
            "TMPDIR": str(self.root),
            "VERGEN_GIT_SHA": "1" * 40,
        }

        def run(assignments: dict[str, str]) -> subprocess.CompletedProcess[str]:
            command = [sys.executable, "-I", "-S", str(HERMETIC_RUNNER)]
            command.extend(("--profile", "apple-ios-device"))
            for name, value in assignments.items():
                command.extend(("--set", f"{name}={value}"))
            command.extend(("--", str(tools / "cargo")))
            return subprocess.run(command, text=True, capture_output=True, check=False)

        self.assertEqual(run(environment).returncode, 0)
        missing_rustdoc = dict(environment)
        del missing_rustdoc["RUSTDOC"]
        rejected = run(missing_rustdoc)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("environment inventory is not exact", rejected.stderr)
        wrong_jobs = dict(environment)
        wrong_jobs["CARGO_BUILD_JOBS"] = "2"
        rejected = run(wrong_jobs)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("CARGO_BUILD_JOBS must be exactly '1'", rejected.stderr)
        mismatched_revision = dict(environment)
        mismatched_revision["VERGEN_GIT_SHA"] = "2" * 40
        rejected = run(mismatched_revision)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("source revision variables must be identical", rejected.stderr)
        noncanonical_revision = dict(environment)
        for name in (
            "CONNECT_NORITO_SOURCE_REVISION",
            "IROHA_GIT_COMMIT_HASH",
            "VERGEN_GIT_SHA",
        ):
            noncanonical_revision[name] = "A" * 40
        rejected = run(noncanonical_revision)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("identical canonical commits", rejected.stderr)
        target_link = self.root / "cargo-target-link"
        target_link.symlink_to(target, target_is_directory=True)
        linked_target = dict(environment)
        linked_target["CARGO_TARGET_DIR"] = str(target_link)
        rejected = run(linked_target)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("non-symbolic canonical directory", rejected.stderr)

    def test_android_builder_binds_exact_root_lock_and_complete_cargo_envelope(
        self,
    ) -> None:
        builder = ANDROID_BUILDER.read_text(encoding="utf-8")
        for required in (
            '"CARGO_BUILD_JOBS"',
            '"RUSTC_BOOTSTRAP"',
            '"RUSTDOC"',
            '"cargo_build_jobs" to 1',
            '"rustdoc_release" to tools.rustdocRelease',
            '"rustdoc_commit_hash" to tools.rustdocCommitHash',
            '"rustdoc_binary_sha256" to sha256Hex(tools.rustdoc)',
            '"CARGO_BUILD_JOBS=1"',
            '"RUSTC_BOOTSTRAP=1"',
            '"RUSTDOC=${tools.rustdoc}"',
            '"--locked"',
            '"--offline"',
            '"--jobs"',
            '"unstable-options"',
            '"--lockfile-path"',
            'tools.cargoLock.toString()',
            '"NORITO_BRIDGE_SEAL_RUSTDOC" to tools.rustdoc.toString()',
            '"NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR" to',
        ):
            self.assertIn(required, builder)
        self.assertIn("rustdocCommitHash == rustcCommitHash", builder)
        self.assertNotIn("MOBILE_SDK_ANDROID_CARGO_LOCK", builder)

    def test_android_hermetic_runner_rejects_inexact_environment_and_command(
        self,
    ) -> None:
        tools = self.root / "android-hermetic-tools"
        target = self.root / "android-cargo-target"
        ndk = self.root / "android-ndk"
        tools.mkdir()
        target.mkdir()
        ndk.mkdir()
        for name in ("cargo", "rustc", "rustdoc"):
            executable = tools / name
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
        environment = {
            "ANDROID_NDK_HOME": str(ndk),
            "ANDROID_NDK_ROOT": str(ndk),
            "CARGO": str(tools / "cargo"),
            "CARGO_BUILD_JOBS": "1",
            "CARGO_HOME": str(self.root / "cargo-home"),
            "CARGO_INCREMENTAL": "0",
            "CARGO_NET_OFFLINE": "true",
            "CARGO_TARGET_DIR": str(target),
            "HOME": str(self.root),
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "NORITO_SKIP_BINDINGS_SYNC": "1",
            "PATH": f"{tools}:/usr/bin:/bin",
            "RUSTC": str(tools / "rustc"),
            "RUSTC_BOOTSTRAP": "1",
            "RUSTDOC": str(tools / "rustdoc"),
            "RUSTUP_HOME": str(self.root / "rustup-home"),
            "TMPDIR": str(self.root),
        }
        cargo_arguments = [
            "ndk",
            "-t",
            "arm64-v8a",
            "-o",
            str(self.root / "staging"),
            "build",
            "--locked",
            "--offline",
            "--jobs",
            "1",
            "-Z",
            "unstable-options",
            "--lockfile-path",
            str(self.root / "Cargo.lock"),
            "--release",
            "-p",
            "connect_norito_bridge",
        ]

        def run(
            assignments: dict[str, str],
            arguments: list[str],
        ) -> subprocess.CompletedProcess[str]:
            command = [sys.executable, "-I", "-S", str(HERMETIC_RUNNER)]
            command.extend(("--profile", "android-cargo"))
            for name, value in assignments.items():
                command.extend(("--set", f"{name}={value}"))
            command.extend(("--", str(tools / "cargo"), *arguments))
            return subprocess.run(
                command,
                cwd=self.root,
                text=True,
                capture_output=True,
                check=False,
            )

        accepted = run(environment, cargo_arguments)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)

        for name, value, expected in (
            ("CARGO_BUILD_JOBS", "2", "must be exactly '1'"),
            ("CARGO_NET_OFFLINE", "false", "must be exactly 'true'"),
            ("RUSTC_BOOTSTRAP", "0", "must be exactly '1'"),
        ):
            with self.subTest(environment=name):
                changed = dict(environment)
                changed[name] = value
                rejected = run(changed, cargo_arguments)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn(expected, rejected.stderr)

        missing_rustdoc = dict(environment)
        del missing_rustdoc["RUSTDOC"]
        rejected = run(missing_rustdoc, cargo_arguments)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("environment inventory is not exact", rejected.stderr)

        for removed, expected in (
            (("--offline",), "exactly one --offline"),
            (("--jobs", "1"), "exactly one --jobs"),
            (("-Z", "unstable-options"), "exactly one -Z"),
        ):
            with self.subTest(command=removed):
                altered = list(cargo_arguments)
                position = altered.index(removed[0])
                del altered[position : position + len(removed)]
                rejected = run(environment, altered)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn(expected, rejected.stderr)

        alternate_lock = self.root / "alternate-Cargo.lock"
        alternate_lock.write_text("# alternate\n", encoding="utf-8")
        altered = list(cargo_arguments)
        altered[altered.index("--lockfile-path") + 1] = str(alternate_lock)
        rejected = run(environment, altered)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("exact sequence --lockfile-path", rejected.stderr)

        (tools / "cargo").write_text(
            """#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--lockfile-path" ]; then
    shift
    printf '# changed during invocation\\n' > "$1"
    exit 0
  fi
  shift
done
exit 65
""",
            encoding="utf-8",
        )
        rejected = run(environment, cargo_arguments)
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("Android root Cargo.lock changed during", rejected.stderr)

    def test_symlinked_rustup_style_proxies_preserve_dispatch_and_fail_on_retarget(
        self,
    ) -> None:
        tools = self.root / "tools"
        tools.mkdir()
        multiplexer = tools / "rustup"
        replacement = tools / "replacement"
        for executable in (multiplexer, replacement):
            executable.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            executable.chmod(0o755)
        cargo_proxy = tools / "cargo"
        rustc_proxy = tools / "rustc"
        rustdoc_proxy = tools / "rustdoc"
        cargo_proxy.symlink_to(multiplexer.name)
        rustc_proxy.symlink_to(multiplexer.name)
        rustdoc_proxy.symlink_to(multiplexer.name)

        with mock.patch.dict(
            os.environ,
            {
                "NORITO_BRIDGE_SEAL_CARGO": str(cargo_proxy),
                "NORITO_BRIDGE_SEAL_RUSTC": str(rustc_proxy),
                "NORITO_BRIDGE_SEAL_RUSTDOC": str(rustdoc_proxy),
            },
            clear=False,
        ):
            cargo = seal.required_tool("NORITO_BRIDGE_SEAL_CARGO", "cargo")
            rustc = seal.required_tool("NORITO_BRIDGE_SEAL_RUSTC", "rustc")
            rustdoc = seal.required_tool("NORITO_BRIDGE_SEAL_RUSTDOC", "rustdoc")

        self.assertEqual(cargo.invocation, cargo_proxy)
        self.assertEqual(rustc.invocation, rustc_proxy)
        self.assertEqual(rustdoc.invocation, rustdoc_proxy)
        canonical_multiplexer = multiplexer.resolve()
        self.assertEqual(cargo.canonical, canonical_multiplexer)
        self.assertEqual(rustc.canonical, canonical_multiplexer)

        completed = subprocess.CompletedProcess([], 0, stdout=b"")
        with mock.patch.object(seal.subprocess, "run", return_value=completed) as run:
            seal.run(self.root, cargo, ["metadata"], {"PATH": str(tools)})
        self.assertEqual(
            run.call_args.args[0],
            [str(cargo_proxy), "metadata"],
        )
        self.assertEqual(
            run.call_args.kwargs["executable"], str(canonical_multiplexer)
        )

        cargo_proxy.unlink()
        cargo_proxy.symlink_to(replacement.name)
        with self.assertRaisesRegex(RuntimeError, "changed after authentication"):
            seal.run(self.root, cargo, ["metadata"], {"PATH": str(tools)})


if __name__ == "__main__":
    unittest.main()
