from __future__ import annotations

from contextlib import contextmanager, nullcontext
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT = Path(__file__).parents[1] / "build_kagemusha_v4_candidate_bundle.py"
SPEC = importlib.util.spec_from_file_location(
    "build_kagemusha_v4_candidate_bundle", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
builder = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = builder
SPEC.loader.exec_module(builder)


class SealedCandidateBuildTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve(strict=True)
        self.external = tempfile.TemporaryDirectory()
        self.external_root = Path(self.external.name).resolve(strict=True)
        self.target_dir = self.external_root / "cargo-target"
        self.cargo = self.root / "admitted-cargo"
        self.cargo.write_bytes(b"cargo fixture\n")
        self.cargo.chmod(0o700)
        self.reviewed_source_closure = self.root / "reviewed-source-closure.json"
        self.reviewed_source_closure.write_bytes(b"{\"fixture\":true}\n")
        self.reviewed_source_closure_sha256 = "c" * 64
        self.reviewed_source_closure_value = {
            "schema": builder.source_seal.REVIEWED_SOURCE_CLOSURE_SCHEMA,
            "fixture": True,
        }
        self.identity = builder.source_seal.SourceIdentity(
            source_commit="a" * 40,
            source_tree_sha256="b" * 64,
            source_repo_dirty=True,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
        )

    def cargo_result(
        self, command: list[str], *, executable: Path | None = None
    ) -> subprocess.CompletedProcess[bytes]:
        if executable is None:
            executable = self.target_dir / "release" / builder.BINARY_NAME
        if not executable.exists() and not executable.is_symlink():
            executable.parent.mkdir(parents=True, exist_ok=True)
            executable.write_bytes(b"sealed candidate fixture\n")
            executable.chmod(0o700)
        message = {
            "reason": "compiler-artifact",
            "target": {"kind": ["bin"], "name": builder.BINARY_NAME},
            "profile": {"debug_assertions": False, "test": False},
            "executable": str(executable),
        }
        return subprocess.CompletedProcess(
            command,
            0,
            stdout=(json.dumps(message) + "\n").encode(),
        )

    def tearDown(self) -> None:
        self.external.cleanup()
        self.temporary.cleanup()

    def build_candidate_bundle(
        self, root: Path, cargo: str, **kwargs: object
    ) -> dict[str, object]:
        """Build with deterministic admission dependencies for unit tests."""

        kwargs.setdefault(
            "physical_memory_reader",
            lambda: builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES,
        )
        kwargs.setdefault("build_lock", lambda: nullcontext())
        kwargs.setdefault(
            "reviewed_source_closure",
            self.reviewed_source_closure,
        )
        kwargs.setdefault(
            "reviewed_source_closure_sha256",
            self.reviewed_source_closure_sha256,
        )
        kwargs.setdefault("target_dir", self.target_dir)
        return builder.build_candidate_bundle(root, cargo, **kwargs)

    def test_build_command_and_report_bind_exact_source_and_release_binary(self) -> None:
        identities: list[Path] = []
        invocations: list[tuple[list[str], dict[str, str]]] = []
        lock_events: list[str] = []
        lock_active = False

        def identity_reader(
            root: Path,
            descriptor_path: str,
            descriptor_sha256: str,
        ) -> builder.source_seal.SourceIdentity:
            identities.append(root)
            self.assertEqual(descriptor_path, str(self.reviewed_source_closure))
            self.assertEqual(
                descriptor_sha256,
                self.reviewed_source_closure_sha256,
            )
            return self.identity

        def command_runner(
            command: list[str], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertTrue(lock_active)
            invocations.append((command, kwargs["env"]))  # type: ignore[arg-type]
            return self.cargo_result(command)

        @contextmanager
        def build_lock():
            nonlocal lock_active
            lock_events.append("enter")
            lock_active = True
            try:
                yield
            finally:
                lock_active = False
                lock_events.append("exit")

        with mock.patch.dict(
            os.environ,
            {
                "CARGO_BUILD_RUSTC": "/hostile/rustc",
                "CARGO_BUILD_TARGET": "ambient-target",
                "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS": "true",
                "CARGO_TARGET_DIR": "elsewhere",
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS": "--cfg hostile",
                "RUSTC_WRAPPER": "/hostile/wrapper",
                "RUSTFLAGS": "--cfg hostile",
            },
        ):
            report = self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=identity_reader,
                command_runner=command_runner,
                physical_memory_reader=lambda: 32 * 1024 * 1024 * 1024,
                build_lock=build_lock,
            )

        self.assertEqual(len(identities), 3)
        command, environment = invocations[0]
        self.assertEqual(command[0], str(self.cargo.resolve()))
        self.assertIn("--release", command)
        self.assertIn("--locked", command)
        self.assertEqual(
            command[command.index("--target-dir") + 1],
            str(self.target_dir),
        )
        self.assertIn(builder.SEALED_FEATURE, command)
        self.assertIn("--message-format=json-render-diagnostics", command)
        self.assertNotIn("CARGO_BUILD_TARGET", environment)
        self.assertNotIn("CARGO_BUILD_RUSTC", environment)
        self.assertNotIn("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", environment)
        self.assertNotIn("CARGO_TARGET_DIR", environment)
        self.assertNotIn("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", environment)
        self.assertEqual(environment["RUSTC_WRAPPER"], "")
        self.assertEqual(environment["RUSTFLAGS"], "")
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_COMMIT"],
            self.identity.source_commit,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_SOURCE_TREE_SHA256"],
            self.identity.source_tree_sha256,
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE"],
            str(self.reviewed_source_closure),
        )
        self.assertEqual(
            environment["KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256"],
            self.reviewed_source_closure_sha256,
        )
        self.assertEqual(report["build_profile"], "release")
        self.assertEqual(report["target_dir"], str(self.target_dir))
        self.assertEqual(report["source_commit"], self.identity.source_commit)
        self.assertIs(report["source_repo_dirty"], True)
        self.assertEqual(
            report["reviewed_source_closure"],
            self.reviewed_source_closure_value,
        )
        self.assertEqual(
            report["reviewed_source_closure_descriptor_sha256"],
            self.reviewed_source_closure_sha256,
        )
        self.assertEqual(lock_events, ["enter", "exit"])
        self.assertEqual(
            report["minimum_build_physical_memory_bytes"],
            builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES,
        )
        self.assertEqual(
            report["physical_memory_bytes_at_admission"],
            32 * 1024 * 1024 * 1024,
        )
        self.assertRegex(str(report["binary_sha256"]), r"^[0-9a-f]{64}$")

    def test_source_change_after_build_fails_closed(self) -> None:
        changed = builder.source_seal.SourceIdentity(
            source_commit="c" * 40,
            source_tree_sha256="d" * 64,
            source_repo_dirty=True,
            reviewed_source_closure=self.reviewed_source_closure_value,
            reviewed_source_closure_descriptor_sha256=(
                self.reviewed_source_closure_sha256
            ),
        )
        identities = iter((self.identity, changed))

        with self.assertRaisesRegex(builder.CandidateBuildError, "changed during"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: next(identities),
                command_runner=lambda command, **_kwargs: self.cargo_result(command),
            )

    def test_exact_release_artifact_symlink_is_rejected_before_resolution(self) -> None:
        linked_binary = self.target_dir / "release" / builder.BINARY_NAME

        def symlink_result(command: list[str]) -> subprocess.CompletedProcess[bytes]:
            linked_binary.parent.mkdir(parents=True)
            real_binary = linked_binary.with_name("real-candidate")
            real_binary.write_bytes(b"replacement candidate\n")
            real_binary.chmod(0o700)
            linked_binary.symlink_to(real_binary)
            return self.cargo_result(command, executable=linked_binary)

        with self.assertRaisesRegex(builder.CandidateBuildError, "symbolic link"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: symlink_result(command),
            )

    def test_cargo_artifact_path_not_default_layout_is_authoritative(self) -> None:
        configured_binary = (
            self.target_dir / "configured-triple" / "release" / builder.BINARY_NAME
        )

        report = self.build_candidate_bundle(
            self.root,
            str(self.cargo),
            identity_reader=lambda _root, _path, _sha256: self.identity,
            command_runner=lambda command, **_kwargs: self.cargo_result(
                command, executable=configured_binary
            ),
        )

        self.assertEqual(report["binary_path"], str(configured_binary.resolve()))

    def test_missing_compiler_artifact_fails_closed(self) -> None:
        with self.assertRaisesRegex(builder.CandidateBuildError, "exactly one"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: subprocess.CompletedProcess(
                    command, 0, stdout=b""
                ),
            )

    def test_debug_compiler_artifact_fails_closed(self) -> None:
        def debug_result(command: list[str]) -> subprocess.CompletedProcess[bytes]:
            message = {
                "reason": "compiler-artifact",
                "target": {"kind": ["bin"], "name": builder.BINARY_NAME},
                "profile": {"debug_assertions": True, "test": False},
                "executable": str(
                    self.target_dir / "release" / builder.BINARY_NAME
                ),
            }
            return subprocess.CompletedProcess(
                command, 0, stdout=(json.dumps(message) + "\n").encode()
            )

        with self.assertRaisesRegex(builder.CandidateBuildError, "debug assertions"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=lambda _root, _path, _sha256: self.identity,
                command_runner=lambda command, **_kwargs: debug_result(command),
            )

    def test_low_memory_refuses_before_source_inspection_lock_or_cargo(self) -> None:
        identity_reader = mock.Mock(return_value=self.identity)
        command_runner = mock.Mock()
        build_lock = mock.Mock(return_value=nullcontext())

        with self.assertRaisesRegex(
            builder.CandidateBuildError,
            "requires at least",
        ):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                identity_reader=identity_reader,
                command_runner=command_runner,
                physical_memory_reader=lambda: (
                    builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES - 1
                ),
                build_lock=build_lock,
            )

        identity_reader.assert_not_called()
        build_lock.assert_not_called()
        command_runner.assert_not_called()

    def test_target_directory_must_be_fresh_absolute_and_external(self) -> None:
        common = {
            "identity_reader": lambda _root, _path, _sha256: self.identity,
            "command_runner": lambda command, **_kwargs: self.cargo_result(command),
        }
        with self.assertRaisesRegex(builder.CandidateBuildError, "absolute and normalized"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                target_dir=Path("relative-target"),
                **common,
            )

        with self.assertRaisesRegex(builder.CandidateBuildError, "outside"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                target_dir=self.root / "target",
                **common,
            )

        self.target_dir.mkdir()
        with self.assertRaisesRegex(builder.CandidateBuildError, "fresh nonexistent"):
            self.build_candidate_bundle(
                self.root,
                str(self.cargo),
                **common,
            )

    def test_detects_macos_physical_memory_with_sysctl(self) -> None:
        expected = 24 * 1024 * 1024 * 1024
        inspection_runner = mock.Mock(
            return_value=subprocess.CompletedProcess(
                ["sysctl"], 0, stdout=f"{expected}\n"
            )
        )

        actual = builder._physical_memory_bytes(
            platform_name="darwin",
            inspection_runner=inspection_runner,
            sysconf_reader=mock.Mock(side_effect=AssertionError("unexpected fallback")),
        )

        self.assertEqual(actual, expected)
        self.assertEqual(
            inspection_runner.call_args.args[0],
            ["/usr/sbin/sysctl", "-n", "hw.memsize"],
        )

    def test_detects_linux_physical_memory_with_sysconf(self) -> None:
        values = {"SC_PHYS_PAGES": 8_388_608, "SC_PAGE_SIZE": 4096}

        actual = builder._physical_memory_bytes(
            platform_name="linux",
            sysconf_reader=lambda name: values[name],
            linux_meminfo_reader=mock.Mock(
                side_effect=AssertionError("unexpected fallback")
            ),
        )

        self.assertEqual(actual, 32 * 1024 * 1024 * 1024)

    def test_linux_meminfo_is_used_when_sysconf_is_unavailable(self) -> None:
        actual = builder._physical_memory_bytes(
            platform_name="linux",
            sysconf_reader=mock.Mock(side_effect=ValueError("unavailable")),
            linux_meminfo_reader=lambda: "MemTotal:       25165824 kB\n",
        )

        self.assertEqual(actual, builder.MINIMUM_BUILD_PHYSICAL_MEMORY_BYTES)

    def test_shared_build_lock_uses_memory_heavy_host_lock(self) -> None:
        host_lock = mock.Mock(return_value=nullcontext())

        with mock.patch.object(builder.resource_guard, "_host_lock", host_lock):
            with builder._shared_memory_heavy_build_lock():
                pass

        host_lock.assert_called_once_with(
            builder.resource_guard.HEAVY_JOB_LOCK_PATH,
            description="memory-heavy job",
        )

    def test_bundle_validates_embedded_seal_before_generation(self) -> None:
        bundle = (
            Path(__file__).parents[2]
            / "crates"
            / "iroha_core"
            / "src"
            / "bin"
            / "kagemusha_recursive_spend_v4_bundle.rs"
        ).read_text(encoding="utf-8")
        build_rs = (
            Path(__file__).parents[2] / "crates" / "iroha_core" / "build.rs"
        ).read_text(encoding="utf-8")

        build_candidate = bundle[bundle.index("fn build_candidate(") :]
        self.assertLess(
            build_candidate.index("validate_embedded_candidate_source("),
            build_candidate.index("prepare_bundle_metadata("),
        )
        self.assertIn('option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT")', bundle)
        self.assertIn("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256", build_rs)
        self.assertIn("kagemusha_source_tree_seal.py", build_rs)


if __name__ == "__main__":
    unittest.main()
