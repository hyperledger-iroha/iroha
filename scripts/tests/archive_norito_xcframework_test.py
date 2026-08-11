#!/usr/bin/env python3
"""Focused tests for the deterministic NoritoBridge archive owner."""

from __future__ import annotations

import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile


ROOT = Path(__file__).resolve().parents[2]
OWNER = ROOT / "scripts/archive_norito_xcframework.py"
VALIDATOR = ROOT / "scripts/validate_norito_bridge_xcframework.py"
SOURCE_DATE_EPOCH = "1700000001"
NORMALIZED_ZIP_TIME = (2023, 11, 14, 22, 13, 20)
KNOWN_FIXTURE_ARCHIVE_SHA256 = (
    "a9c757397511f0d4bcadd4f26a9cdb17a4568d6d596c57507e14289c65ebb66a"
)
SLICE_METADATA = {
    "ios-arm64": ("ios", ["arm64"], None),
    "ios-arm64_x86_64-simulator": (
        "ios",
        ["arm64", "x86_64"],
        "simulator",
    ),
    "macos-arm64_x86_64": ("macos", ["arm64", "x86_64"], None),
}


def digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def load_owner_module():
    spec = importlib.util.spec_from_file_location("norito_bridge_archive_owner", OWNER)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load archive owner")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_validator_module():
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location(
        "norito_bridge_archive_test_validator",
        VALIDATOR,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load XCFramework validator")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class ArchiveNoritoXcframeworkTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="norito-bridge-archive-owner-test."
        )
        self.root = Path(self.temporary.name).resolve(strict=True)
        self.artifact_root = self.root / "artifacts"
        self.output_root = self.root / "packages"
        self.scratch_root = self.root / "scratch"
        self.framework = self.artifact_root / "NoritoBridge.xcframework"
        self.artifact_root.mkdir()
        self.output_root.mkdir()
        self.scratch_root.mkdir()
        self._write_complete_generation()
        self.mechanical_runner = self.root / "mechanical_archive_runner.py"
        self._write_mechanical_runner()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_complete_generation(self) -> None:
        validator = load_validator_module()
        header = (
            ROOT / "crates/connect_norito_bridge/include/connect_norito_bridge.h"
        ).read_bytes()
        wrapper = (
            ROOT / "crates/connect_norito_bridge/include/NoritoBridge.h"
        ).read_bytes()
        module_map = (
            ROOT / "crates/connect_norito_bridge/module.modulemap.template"
        ).read_bytes()
        libraries: list[dict[str, object]] = []
        hashes: dict[str, str] = {}
        self.framework.mkdir()
        for identifier, (platform, architectures, variant) in SLICE_METADATA.items():
            binary = f"native:{identifier}:abi22".encode()
            slice_root = self.framework / identifier
            headers = slice_root / "Headers"
            headers.mkdir(parents=True)
            (slice_root / "libNoritoBridge.a").write_bytes(binary)
            (headers / "connect_norito_bridge.h").write_bytes(header)
            (headers / "NoritoBridge.h").write_bytes(wrapper)
            (headers / "module.modulemap").write_bytes(module_map)
            hashes[identifier] = digest(binary)
            library: dict[str, object] = {
                "BinaryPath": "libNoritoBridge.a",
                "HeadersPath": "Headers",
                "LibraryIdentifier": identifier,
                "LibraryPath": "libNoritoBridge.a",
                "SupportedArchitectures": architectures,
                "SupportedPlatform": platform,
            }
            if variant is not None:
                library["SupportedPlatformVariant"] = variant
            libraries.append(library)

        info = {
            "AvailableLibraries": libraries,
            "CFBundlePackageType": "XFWK",
            "XCFrameworkFormatVersion": "1.0",
        }
        with (self.framework / "Info.plist").open("wb") as handle:
            plistlib.dump(info, handle, sort_keys=True)
        build_environment = {
            "schema": "iroha.mobile-native-build-environment.v1",
            "hermetic_runner_schema": "iroha.mobile-hermetic-command.v1",
            "hermetic_runner_sha256": digest(
                (ROOT / "scripts/run_mobile_hermetic_command.py").read_bytes()
            ),
            "environment_profiles": validator.EXPECTED_ENVIRONMENT_PROFILES,
            "cargo_build_jobs": 1,
            "rust_toolchain_channel": "1.93.1",
            "cargo_release": "1.93.1",
            "cargo_commit_hash": "4" * 40,
            "cargo_binary_sha256": "4" * 64,
            "rustc_release": "1.93.1",
            "rustc_commit_hash": "5" * 40,
            "rustc_binary_sha256": "5" * 64,
            "rustdoc_release": "1.93.1",
            "rustdoc_commit_hash": "5" * 40,
            "rustdoc_binary_sha256": "6" * 64,
            "python_version": "3.12.13",
            "python_binary_sha256": "7" * 64,
            "git_version": "2.51.0",
            "git_binary_sha256": "8" * 64,
            "rustup_version": "1.28.2",
            "rustup_binary_sha256": "9" * 64,
            "xcode_version": "16.2",
            "xcode_build_version": "16C5032a",
            "iphoneos_sdk_version": "18.2",
            "iphonesimulator_sdk_version": "18.2",
            "macosx_sdk_version": "15.2",
            "iphoneos_deployment_target": "15.0",
            "iphonesimulator_deployment_target": "15.0",
            "macosx_deployment_target": "12.0",
        }
        manifest = {
            "version": "0.1.0",
            "native_bridge_abi_version": 22,
            "privacy_production_enabled": False,
            "cargo_features": [],
            "build_environment": build_environment,
            "source_commit": "1" * 40,
            "source_tree_dirty": False,
            "source_fingerprint_sha256": "2" * 64,
            "cargo_lock_sha256": digest((ROOT / "Cargo.lock").read_bytes()),
            "bridge_header_sha256": digest(header),
            "required_symbols": validator.EXPECTED_REQUIRED_SYMBOLS,
            "forbidden_symbols": validator.EXPECTED_FORBIDDEN_SYMBOLS,
            "kagemusha_mobile_artifact_roles": validator.expected_kagemusha_roles(
                False
            ),
            "hashes": hashes,
        }
        (self.framework / "NoritoBridge.artifacts.json").write_text(
            json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )

    def _write_mechanical_runner(self) -> None:
        self.mechanical_runner.write_text(
            """\
import importlib.util
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


owner = load("mechanical_archive_owner", Path(sys.argv[1]))
validator = load("mechanical_archive_validator", Path(sys.argv[2]))
validator._validate_repository_provenance = lambda _root, _payload: None
owner._load_generation_validator = lambda: validator
owner._validate_native_binaries = lambda _snapshot, _validator: None
digest, size = owner.archive_xcframework(sys.argv[3], sys.argv[4], sys.argv[5])
print(f"{digest} {size}")
""",
            encoding="utf-8",
        )

    def _run(
        self,
        output: Path,
        *,
        expect_success: bool = True,
        environment_updates: dict[str, str] | None = None,
        pass_fds: tuple[int, ...] = (),
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["SOURCE_DATE_EPOCH"] = SOURCE_DATE_EPOCH
        environment.pop("NORITO_BRIDGE_OUTPUT_LOCK_FD", None)
        if environment_updates is not None:
            environment.update(environment_updates)
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                "-B",
                str(self.mechanical_runner),
                str(OWNER),
                str(VALIDATOR),
                str(self.framework),
                str(output),
                str(self.scratch_root),
            ],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
            pass_fds=pass_fds,
            timeout=30,
        )
        if expect_success and result.returncode != 0:
            self.fail(f"archive owner failed:\nstdout={result.stdout}\nstderr={result.stderr}")
        if not expect_success and result.returncode == 0:
            self.fail("archive owner unexpectedly succeeded")
        return result

    def test_sorted_archive_is_stable_across_source_mtime_and_mode_changes(self) -> None:
        first = self.output_root / "first.zip"
        second = self.output_root / "second.zip"
        first_result = self._run(first)

        for path in sorted(self.framework.rglob("*")):
            os.chmod(path, 0o700 if path.is_dir() else 0o600)
            os.utime(path, ns=(1_800_000_000_000_000_000,) * 2)
        os.utime(self.framework, ns=(1_800_000_000_000_000_000,) * 2)
        second_result = self._run(second)

        self.assertEqual(first.read_bytes(), second.read_bytes())
        expected_digest = digest(first.read_bytes())
        self.assertEqual(expected_digest, KNOWN_FIXTURE_ARCHIVE_SHA256)
        self.assertEqual(first_result.stdout.split()[0], expected_digest)
        self.assertEqual(second_result.stdout.split()[0], expected_digest)
        with zipfile.ZipFile(first) as archive:
            self.assertIsNone(archive.testzip())
            entries = archive.infolist()
            names = [entry.filename for entry in entries]
            self.assertEqual(names, sorted(names, key=lambda name: name.encode("utf-8")))
            self.assertEqual(names[0], "NoritoBridge.xcframework/")
            for entry in entries:
                self.assertEqual(entry.date_time, NORMALIZED_ZIP_TIME)
                self.assertEqual(entry.compress_type, zipfile.ZIP_STORED)
                archived_mode = entry.external_attr >> 16
                self.assertEqual(
                    stat.S_IMODE(archived_mode),
                    0o755 if entry.is_dir() else 0o644,
                )
                archive.read(entry)

    def test_held_shared_publish_lock_rejects_racing_archive(self) -> None:
        lock_path = self.artifact_root / ".NoritoBridge.publish.lockfile"
        lock_fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
        os.fchmod(lock_fd, 0o600)
        try:
            fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            output = self.output_root / "racing.zip"
            result = self._run(output, expect_success=False)
            self.assertIn("another publisher holds", result.stderr)
            self.assertFalse(output.exists())
        finally:
            os.close(lock_fd)

        self._run(self.output_root / "after-lock-release.zip")

    def test_authenticated_inherited_output_lock_is_accepted(self) -> None:
        lock_path = self.artifact_root / ".NoritoBridge.publish.lockfile"
        lock_fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o644)
        os.fchmod(lock_fd, 0o644)
        try:
            fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self._run(
                self.output_root / "inherited-lock.zip",
                environment_updates={"NORITO_BRIDGE_OUTPUT_LOCK_FD": str(lock_fd)},
                pass_fds=(lock_fd,),
            )
        finally:
            os.close(lock_fd)

    def test_forged_inherited_output_lock_is_rejected(self) -> None:
        expected_lock = self.artifact_root / ".NoritoBridge.publish.lockfile"
        expected_lock.write_bytes(b"")
        expected_lock.chmod(0o600)
        unrelated_path = self.root / "unrelated.lock"
        unrelated_fd = os.open(unrelated_path, os.O_RDWR | os.O_CREAT, 0o600)
        try:
            fcntl.flock(unrelated_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self._run(
                self.output_root / "forged-lock.zip",
                expect_success=False,
                environment_updates={
                    "NORITO_BRIDGE_OUTPUT_LOCK_FD": str(unrelated_fd)
                },
                pass_fds=(unrelated_fd,),
            )
            self.assertIn("not authenticated", result.stderr)
        finally:
            os.close(unrelated_fd)

    def test_preexisting_destination_is_rejected_without_a_lock_artifact(self) -> None:
        output = self.output_root / "existing.zip"
        output.write_bytes(b"authenticated-existing-release")
        result = self._run(output, expect_success=False)
        self.assertIn("must not already exist", result.stderr)
        self.assertEqual(output.read_bytes(), b"authenticated-existing-release")
        self.assertFalse(
            (self.output_root / ".NoritoBridge.archive.lockfile").exists()
        )

    def test_replaced_lock_inode_aborts_before_atomic_publication(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "replaced-lock.zip"
        original_write_archive = owner._write_archive

        def replace_lock_after_archive(*args, **kwargs):
            result = original_write_archive(*args, **kwargs)
            lock_path = self.artifact_root / ".NoritoBridge.publish.lockfile"
            lock_path.unlink()
            lock_path.write_bytes(b"")
            lock_path.chmod(0o600)
            return result

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_write_archive",
                side_effect=replace_lock_after_archive,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "not authenticated"):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertFalse(output.exists())
        self.assertEqual(len(list(self.output_root.glob(".replaced-lock.zip.*.tmp"))), 1)

    def test_interrupted_atomic_publication_retains_owned_residue(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "release.zip"
        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_atomic_publish",
                side_effect=owner.ArchiveInterrupted("simulated interruption"),
            ),
        ):
            with self.assertRaises(owner.ArchiveInterrupted):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertFalse(output.exists())
        self.assertEqual(len(list(self.output_root.glob(".release.zip.*.tmp"))), 1)
        self.assertEqual(
            len(list(self.scratch_root.glob(".NoritoBridge.archive-snapshot.*"))),
            1,
        )

    def test_destination_appearing_during_archive_is_never_overwritten(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "existing-race.zip"
        original_write_archive = owner._write_archive

        def mutate_destination_after_archive(*args, **kwargs):
            result = original_write_archive(*args, **kwargs)
            output.write_bytes(b"competing-destination-update")
            return result

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_write_archive",
                side_effect=mutate_destination_after_archive,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "must not already exist"):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertEqual(output.read_bytes(), b"competing-destination-update")
        self.assertEqual(
            len(list(self.output_root.glob(".existing-race.zip.*.tmp"))),
            1,
        )

    def test_absent_destination_uses_atomic_no_replace_publication(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "absent-race.zip"
        original_assert_absent = owner._assert_destination_absent
        checks = 0

        def introduce_destination_after_check(path):
            nonlocal checks
            original_assert_absent(path)
            checks += 1
            if checks == 2:
                output.write_bytes(b"competing-atomic-publication")

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_assert_destination_absent",
                side_effect=introduce_destination_after_check,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "appeared"):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertEqual(output.read_bytes(), b"competing-atomic-publication")
        self.assertEqual(len(list(self.output_root.glob(".absent-race.zip.*.tmp"))), 1)

    def test_swapped_temporary_path_is_retained_and_never_published(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "swapped-temp.zip"
        original_write_archive = owner._write_archive

        def swap_temporary_after_archive(*args, **kwargs):
            archive = original_write_archive(*args, **kwargs)
            archive.path.unlink()
            archive.path.write_bytes(b"foreign-temporary-inode")
            return archive

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_write_archive",
                side_effect=swap_temporary_after_archive,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "authenticated inode"):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertFalse(output.exists())
        residues = list(self.output_root.glob(".swapped-temp.zip.*.tmp"))
        self.assertEqual(len(residues), 1)
        self.assertEqual(residues[0].read_bytes(), b"foreign-temporary-inode")

    def test_source_swap_inside_no_replace_is_detected_without_cleanup(self) -> None:
        owner = load_owner_module()
        validator = load_validator_module()
        output = self.output_root / "rename-source-race.zip"
        original_rename = owner._rename_no_replace

        def swap_source_then_rename(temporary, destination):
            temporary.unlink()
            temporary.write_bytes(b"foreign-late-source")
            original_rename(temporary, destination)

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                validator,
                "_validate_repository_provenance",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=validator,
            ),
            mock.patch.object(
                owner,
                "_validate_native_binaries",
                return_value=None,
            ),
            mock.patch.object(
                owner,
                "_rename_no_replace",
                side_effect=swap_source_then_rename,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "authenticated inode"):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertEqual(output.read_bytes(), b"foreign-late-source")
        self.assertEqual(list(self.output_root.glob(".rename-source-race.zip.*.tmp")), [])

    def test_repository_contained_archive_paths_are_rejected(self) -> None:
        owner = load_owner_module()
        with self.assertRaisesRegex(owner.ArchiveError, "source tree"):
            owner._canonical_output(
                str(ROOT / "NoritoBridge.xcframework.zip"),
                self.framework,
                ROOT,
            )
        with mock.patch.object(owner, "_repository_root", return_value=self.root):
            with self.assertRaisesRegex(owner.ArchiveError, "artifact root"):
                owner.archive_xcframework(
                    str(self.framework),
                    str(self.output_root / "repository-contained.zip"),
                    str(self.scratch_root),
                )

    def test_manifest_hash_mismatch_fails_before_publication(self) -> None:
        binary = self.framework / "ios-arm64/libNoritoBridge.a"
        binary.write_bytes(binary.read_bytes() + b"tampered")
        output = self.output_root / "tampered.zip"
        result = self._run(output, expect_success=False)
        self.assertIn("slice hash mismatch", result.stderr)
        self.assertFalse(output.exists())

    def test_unexpected_xcframework_entry_is_rejected(self) -> None:
        (self.framework / "legacy-compatibility.txt").write_text(
            "forbidden\n",
            encoding="utf-8",
        )
        output = self.output_root / "unexpected-entry.zip"
        result = self._run(output, expect_success=False)
        self.assertIn("inventory is not exact", result.stderr)
        self.assertFalse(output.exists())

    def test_absent_optional_binary_path_matches_authoritative_validator(self) -> None:
        info_path = self.framework / "Info.plist"
        with info_path.open("rb") as handle:
            info = plistlib.load(handle)
        for library in info["AvailableLibraries"]:
            library.pop("BinaryPath")
        with info_path.open("wb") as handle:
            plistlib.dump(info, handle, sort_keys=True)
        self._run(self.output_root / "without-binary-path.zip")

    def test_owner_explicitly_requests_repository_provenance(self) -> None:
        owner = load_owner_module()
        captured: dict[str, object] = {}

        class FakeValidationError(RuntimeError):
            pass

        class CapturingValidator:
            ValidationError = FakeValidationError

            @staticmethod
            def validate(**arguments):
                captured.update(arguments)

        with mock.patch.object(
            owner,
            "_load_generation_validator",
            return_value=CapturingValidator,
        ):
            owner._validate_generation(self.framework)

        self.assertIs(captured["verify_repository_provenance"], True)
        self.assertFalse((self.artifact_root / "NoritoBridge.artifacts.json").exists())

    def test_repository_provenance_rejection_blocks_publication(self) -> None:
        owner = load_owner_module()
        output = self.output_root / "rejected-provenance.zip"

        class FakeValidationError(RuntimeError):
            pass

        class RejectingValidator:
            ValidationError = FakeValidationError

            @staticmethod
            def validate(**arguments):
                if arguments.get("verify_repository_provenance") is not True:
                    raise AssertionError("owner did not request repository provenance")
                raise FakeValidationError("simulated repository provenance rejection")

        with (
            mock.patch.dict(os.environ, {"SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH}),
            mock.patch.object(
                owner,
                "_load_generation_validator",
                return_value=RejectingValidator,
            ),
        ):
            with self.assertRaisesRegex(
                owner.ArchiveError,
                "repository provenance rejection",
            ):
                owner.archive_xcframework(
                    str(self.framework), str(output), str(self.scratch_root)
                )

        self.assertFalse(output.exists())

    def test_builder_archive_wiring_and_abi_chain_are_exact(self) -> None:
        builder = (ROOT / "scripts/build_norito_xcframework.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("--archive-output)", builder)
        self.assertIn("--archive-output=*)", builder)
        self.assertGreaterEqual(builder.count("--archive-output requires a value"), 2)
        self.assertIn('output.suffix != ".zip"', builder)
        self.assertIn(
            'header_abis != ["22"]',
            builder,
        )
        self.assertIn(
            'bridge_aliases != ["PRIVACY_BRIDGE_ABI_VERSION_V1"]',
            builder,
        )
        self.assertIn("protocol_abis != header_abis", builder)
        self.assertNotIn(
            "CONNECT_NORITO_BRIDGE_ABI_VERSION:[[:space:]]*u32",
            builder,
        )
        for environment_name in (
            "NORITO_BRIDGE_OUTPUT_LOCK_FD",
            "NORITO_BRIDGE_SEAL_HOME",
            "NORITO_BRIDGE_SEAL_CARGO_HOME",
            "NORITO_BRIDGE_SEAL_RUSTUP_HOME",
            "NORITO_BRIDGE_SEAL_TMPDIR",
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR",
            "NORITO_BRIDGE_SEAL_CARGO",
            "NORITO_BRIDGE_SEAL_RUSTC",
            "NORITO_BRIDGE_SEAL_RUSTDOC",
            "NORITO_BRIDGE_SEAL_RUSTUP",
            "NORITO_BRIDGE_SEAL_DEVELOPER_DIR",
        ):
            self.assertIn(f'{environment_name}="$', builder)
        self.assertIn(
            '"$PYTHON_BINARY" -I -S -B "$ARCHIVE_OWNER"',
            builder,
        )
        self.assertIn("must already exist", builder)
        self.assertNotIn("candidate.mkdir(parents=True, exist_ok=True)", builder)
        self.assertIn(
            "live XCFramework requires its canonical public manifest link",
            builder,
        )
        self.assertNotIn("write_embedded_manifest", builder)
        self.assertNotIn("after migration", builder)

    def test_checker_nested_source_seal_is_no_site_and_no_bytecode(self) -> None:
        checker = (ROOT / "scripts/check_mobile_sdk_artifacts.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            'source_seal_python,\n            "-I",\n            "-S",\n            "-B",',
            checker,
        )
        self.assertIn('symbols="$(nm -gUj "$binary"', checker)
        self.assertIn('["nm", "-gUj", binary]', checker)

    def test_native_architecture_and_export_inventory_is_authenticated(self) -> None:
        owner = load_owner_module()

        class NativePolicy:
            LIBRARY_NAME = "libNoritoBridge.a"
            EXPECTED_SLICES = {
                identifier: {"architectures": architectures}
                for identifier, (_, architectures, _) in SLICE_METADATA.items()
            }
            EXPECTED_REQUIRED_SYMBOLS = [
                "connect_norito_bridge_abi_version",
                "connect_norito_kagemusha_required_v4",
            ]
            EXPECTED_FORBIDDEN_SYMBOLS = [
                "connect_norito_kagemusha_retired_v3"
            ]

        def canonical_output(tool: Path, arguments: list[str]) -> str:
            identifier = Path(arguments[-1]).parent.name
            if tool.name == "lipo":
                return " ".join(
                    NativePolicy.EXPECTED_SLICES[identifier]["architectures"]
                ) + "\n"
            self.assertEqual(arguments[:-1], ["-gUj"])
            return (
                "_connect_norito_bridge_abi_version\n"
                "_connect_norito_kagemusha_required_v4\n"
            )

        with (
            mock.patch.object(owner.sys, "platform", "darwin"),
            mock.patch.object(
                owner,
                "_pinned_native_tool",
                side_effect=lambda name: Path("/usr/bin") / name,
            ),
            mock.patch.object(
                owner,
                "_run_native_tool",
                side_effect=canonical_output,
            ),
        ):
            owner._validate_native_binaries(self.framework, NativePolicy)

    def test_reference_only_required_symbol_is_rejected(self) -> None:
        owner = load_owner_module()

        class NativePolicy:
            LIBRARY_NAME = "libNoritoBridge.a"
            EXPECTED_SLICES = {
                identifier: {"architectures": architectures}
                for identifier, (_, architectures, _) in SLICE_METADATA.items()
            }
            EXPECTED_REQUIRED_SYMBOLS = [
                "connect_norito_bridge_abi_version",
                "connect_norito_kagemusha_required_v4",
            ]
            EXPECTED_FORBIDDEN_SYMBOLS = []

        def reference_only_output(tool: Path, arguments: list[str]) -> str:
            identifier = Path(arguments[-1]).parent.name
            if tool.name == "lipo":
                return " ".join(
                    NativePolicy.EXPECTED_SLICES[identifier]["architectures"]
                ) + "\n"
            if arguments[:-1] == ["-gj"]:
                return (
                    "_connect_norito_bridge_abi_version\n"
                    "_connect_norito_kagemusha_required_v4\n"
                )
            self.assertEqual(arguments[:-1], ["-gUj"])
            return "_connect_norito_bridge_abi_version\n"

        with (
            mock.patch.object(owner.sys, "platform", "darwin"),
            mock.patch.object(
                owner,
                "_pinned_native_tool",
                side_effect=lambda name: Path("/usr/bin") / name,
            ),
            mock.patch.object(
                owner,
                "_run_native_tool",
                side_effect=reference_only_output,
            ),
        ):
            with self.assertRaisesRegex(
                owner.ArchiveError,
                "missing required native symbols",
            ):
                owner._validate_native_binaries(self.framework, NativePolicy)

    def test_wrong_architecture_and_missing_export_are_rejected(self) -> None:
        owner = load_owner_module()

        class NativePolicy:
            LIBRARY_NAME = "libNoritoBridge.a"
            EXPECTED_SLICES = {
                identifier: {"architectures": architectures}
                for identifier, (_, architectures, _) in SLICE_METADATA.items()
            }
            EXPECTED_REQUIRED_SYMBOLS = [
                "connect_norito_bridge_abi_version",
                "connect_norito_kagemusha_required_v4",
            ]
            EXPECTED_FORBIDDEN_SYMBOLS = [
                "connect_norito_kagemusha_retired_v3"
            ]

        def wrong_architecture(tool: Path, arguments: list[str]) -> str:
            identifier = Path(arguments[-1]).parent.name
            if tool.name == "lipo":
                return "x86_64\n" if identifier == "ios-arm64" else " ".join(
                    NativePolicy.EXPECTED_SLICES[identifier]["architectures"]
                ) + "\n"
            return (
                "_connect_norito_bridge_abi_version\n"
                "_connect_norito_kagemusha_required_v4\n"
            )

        def missing_export(tool: Path, arguments: list[str]) -> str:
            identifier = Path(arguments[-1]).parent.name
            if tool.name == "lipo":
                return " ".join(
                    NativePolicy.EXPECTED_SLICES[identifier]["architectures"]
                ) + "\n"
            return "_connect_norito_kagemusha_required_v4\n"

        with (
            mock.patch.object(owner.sys, "platform", "darwin"),
            mock.patch.object(
                owner,
                "_pinned_native_tool",
                side_effect=lambda name: Path("/usr/bin") / name,
            ),
            mock.patch.object(
                owner,
                "_run_native_tool",
                side_effect=wrong_architecture,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "architecture mismatch"):
                owner._validate_native_binaries(self.framework, NativePolicy)
        with (
            mock.patch.object(owner.sys, "platform", "darwin"),
            mock.patch.object(
                owner,
                "_pinned_native_tool",
                side_effect=lambda name: Path("/usr/bin") / name,
            ),
            mock.patch.object(
                owner,
                "_run_native_tool",
                side_effect=missing_export,
            ),
        ):
            with self.assertRaisesRegex(owner.ArchiveError, "missing required native symbols"):
                owner._validate_native_binaries(self.framework, NativePolicy)

    def test_missing_source_date_epoch_is_rejected(self) -> None:
        environment = os.environ.copy()
        environment.pop("SOURCE_DATE_EPOCH", None)
        output = self.output_root / "missing-epoch.zip"
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-S",
                "-B",
                str(OWNER),
                "--xcframework",
                str(self.framework),
                "--output",
                str(output),
                "--scratch-dir",
                str(self.scratch_root),
            ],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
            timeout=30,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SOURCE_DATE_EPOCH", result.stderr)
        self.assertFalse(output.exists())

    def test_global_site_initialization_is_rejected(self) -> None:
        output = self.output_root / "site-enabled.zip"
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-B",
                str(OWNER),
                "--xcframework",
                str(self.framework),
                "--output",
                str(output),
                "--scratch-dir",
                str(self.scratch_root),
            ],
            check=False,
            capture_output=True,
            text=True,
            env={**os.environ, "SOURCE_DATE_EPOCH": SOURCE_DATE_EPOCH},
            timeout=30,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("isolated no-site Python 3.12", result.stderr)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
