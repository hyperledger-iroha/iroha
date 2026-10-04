#!/usr/bin/env python3
"""Bounded ZIP and create-only native installation tests; no replacement native runtime."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock
import warnings
import zipfile


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/validate_norito_bridge_archive.py"
specification = importlib.util.spec_from_file_location("norito_archive_under_test", SCRIPT)
assert specification is not None and specification.loader is not None
archive_owner = importlib.util.module_from_spec(specification)
sys.modules[specification.name] = archive_owner
specification.loader.exec_module(archive_owner)


class NoritoBridgeArchiveTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="norito-archive-test.")
        self.base = Path(self.temporary.name).resolve(strict=True)
        self.root = self.base / "repo"
        (self.root / "IrohaSwift").mkdir(parents=True)
        (self.root / "IrohaSwift/VERSION").write_text("0.1.0\n", encoding="ascii")
        self.lockfile = self.base / "Cargo.lock"
        self.lockfile.write_bytes(b"public unit lock fixture\n")
        self.lockfile.chmod(0o400)
        self.archive = self.base / "NoritoBridge-v0.1.0.xcframework.zip"
        self.destination = self.root / "dist"
        self.destination.mkdir()
        self.marker = self.destination / ".gitkeep"
        self.marker.write_bytes(b"retained caller marker\n")
        self.write_archive()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_archive(self, entries: list[tuple[str | zipfile.ZipInfo, bytes]] | None = None,
                      *, compression: int = zipfile.ZIP_STORED, version: str = "0.1.0") -> None:
        # These are inert filesystem fixtures, never native consumer/build evidence.
        manifest = json.dumps({"version": version}).encode() + b"\n"
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(self.archive, "w", compression=compression) as bundle:
                bundle.writestr(archive_owner.EMBEDDED_MANIFEST, manifest)
                for name, payload in entries or []:
                    bundle.writestr(name, payload)

    def bounded(self) -> bytes:
        return archive_owner.validate_archive(self.archive, "0.1.0")

    def native_fixture(self, root: Path, lock: Path, directory: Path) -> dict[str, object]:
        self.assertEqual(root, self.root)
        self.assertEqual(lock, self.lockfile)
        self.assertEqual(os.readlink(directory / "NoritoBridge.artifacts.json"), archive_owner.EMBEDDED_MANIFEST)
        return json.loads((directory / archive_owner.EMBEDDED_MANIFEST).read_bytes())

    def install(self):
        return archive_owner.authenticate_archive(
            self.root, self.archive, self.lockfile, install_directory=self.destination,
        )

    def test_archive_limits_are_unchanged(self) -> None:
        self.assertEqual(archive_owner.MAX_ARCHIVE_BYTES, 512 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_ENTRY_BYTES, 256 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_TOTAL_UNCOMPRESSED_BYTES, 1024 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_ARCHIVE_ENTRIES, 4096)
        self.assertEqual(archive_owner.MAX_MANIFEST_BYTES, 64 * 1024)

    def test_valid_stored_archive_returns_original_bytes(self) -> None:
        self.assertEqual(self.bounded(), self.archive.read_bytes())

    def test_version_is_exact_canonical_root_owned_semver(self) -> None:
        self.assertEqual(archive_owner.parse_version(self.root), "0.1.0")
        for malformed in (b"0.1.0", b"00.1.0\n", b"0.1.0\n\n", b"0.1.0 \n", b"v0.1.0\n"):
            with self.subTest(value=malformed):
                (self.root / "IrohaSwift/VERSION").write_bytes(malformed)
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    archive_owner.parse_version(self.root)
        self.write_archive(version="0.2.0")
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "version must equal"):
            self.bounded()

    def test_path_escape_collisions_and_duplicates_are_rejected(self) -> None:
        invalid = (
            "../escape", "/absolute", "Other.xcframework/file", "NoritoBridge.xcframework/../escape",
            "NoritoBridge.xcframework//file", "NoritoBridge.xcframework/./file", "NoritoBridge.xcframework\\file",
            archive_owner.EMBEDDED_MANIFEST,
        )
        for name in invalid:
            with self.subTest(name=name):
                self.write_archive([(name, b"unexpected")])
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    self.bounded()
        self.write_archive([("NoritoBridge.xcframework/A", b"a"), ("NoritoBridge.xcframework/a", b"b")])
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "case-colliding"):
            self.bounded()

    def test_symlinks_special_files_and_compression_are_rejected(self) -> None:
        for kind in (stat.S_IFLNK, stat.S_IFIFO, stat.S_IFSOCK, stat.S_IFCHR):
            with self.subTest(kind=kind):
                entry = zipfile.ZipInfo("NoritoBridge.xcframework/linked")
                entry.create_system = 3
                entry.external_attr = (kind | 0o777) << 16
                self.write_archive([(entry, b"target")])
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    self.bounded()
        self.write_archive(compression=zipfile.ZIP_DEFLATED)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "deterministically stored"):
            self.bounded()

    def test_encrypted_members_and_corrupt_crc_are_rejected(self) -> None:
        payload = bytearray(self.archive.read_bytes())
        for signature, offset in ((b"PK\x03\x04", 6), (b"PK\x01\x02", 8)):
            index = payload.index(signature)
            payload[index + offset] |= 1
        self.archive.write_bytes(payload)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "encrypted"):
            self.bounded()
        self.write_archive()
        payload = self.archive.read_bytes().replace(b'"0.1.0"', b'"0.1.1"', 1)
        self.archive.write_bytes(payload)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "corrupt|CRC"):
            self.bounded()

    def test_archive_entry_total_and_manifest_budgets_are_enforced(self) -> None:
        self.write_archive([("NoritoBridge.xcframework/member", b"x" * 64)])
        for name, limit, message in (
            ("MAX_ENTRY_BYTES", 32, "entry exceeds"),
            ("MAX_TOTAL_UNCOMPRESSED_BYTES", 70, "uncompressed limit"),
            ("MAX_ARCHIVE_ENTRIES", 1, "entry limit"),
            ("MAX_MANIFEST_BYTES", 4, "manifest must contain"),
        ):
            with self.subTest(budget=name), mock.patch.object(archive_owner, name, limit):
                with self.assertRaisesRegex(archive_owner.ArchiveValidationError, message):
                    self.bounded()
        with self.archive.open("r+b") as output:
            output.truncate(archive_owner.MAX_ARCHIVE_BYTES + 1)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "byte limit"):
            self.bounded()

    def test_archive_regular_ownership_link_and_mutation_controls_remain(self) -> None:
        alias = self.base / "alias.zip"
        alias.symlink_to(self.archive)
        with self.assertRaises(archive_owner.ArchiveValidationError):
            archive_owner.validate_archive(alias, "0.1.0")
        alias.unlink()
        os.link(self.archive, alias)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "single-link"):
            self.bounded()
        alias.unlink()
        self.archive.chmod(0o666)
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "non-writable-by-others"):
            self.bounded()
        self.archive.chmod(0o600)
        original = archive_owner.os.read
        changed = False
        def mutate(descriptor, size):
            nonlocal changed
            chunk = original(descriptor, size)
            if not changed:
                changed = True
                self.archive.write_bytes(self.archive.read_bytes() + b"changed")
            return chunk
        with mock.patch.object(archive_owner.os, "read", side_effect=mutate):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "changed while"):
                self.bounded()

    def test_optional_archive_checksum_must_match_before_extraction(self) -> None:
        with mock.patch.object(archive_owner, "_extract_archive") as extract:
            for digest in ("0" * 64, "invalid", "A" * 64):
                with self.subTest(digest=digest):
                    with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "SHA-256"):
                        archive_owner.authenticate_archive(self.root, self.archive, self.lockfile, expected_sha256=digest)
            extract.assert_not_called()

    def test_verified_archive_without_install_keeps_no_generated_stage(self) -> None:
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=self.native_fixture):
            result = archive_owner.authenticate_archive(
                self.root, self.archive, self.lockfile,
                expected_sha256=digest, scratch_directory=self.base,
            )
        self.assertEqual(result["archive_sha256"], digest)
        self.assertIsNone(result["installed"])
        self.assertFalse(list(self.base.glob(".NoritoBridge.archive-check.*")))

    def test_changed_stage_identity_or_inventory_is_retained_without_publication(self) -> None:
        for replacement in (False, True):
            with self.subTest(replacement=replacement):
                retained: list[Path] = []
                def interfere(root, lock, directory):
                    manifest = self.native_fixture(root, lock, directory)
                    if replacement:
                        saved = directory.with_name(directory.name + ".retained")
                        directory.rename(saved)
                        retained.append(saved)
                        directory.mkdir(mode=0o700)
                    (directory / "caller.txt").write_bytes(b"caller-owned interference\n")
                    retained.append(directory)
                    return manifest
                with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=interfere):
                    with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "staging.*changed"):
                        self.install()
                self.assertFalse((self.destination / archive_owner.ARCHIVE_ROOT).exists())
                for directory in retained:
                    self.assertTrue(directory.is_dir())
                self.assertEqual((retained[-1] / "caller.txt").read_bytes(), b"caller-owned interference\n")

    def test_default_dist_install_preserves_exact_bytes_and_caller_marker(self) -> None:
        manifest_bytes = b'{"version": "0.1.0"}\n'
        with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=self.native_fixture) as validate:
            result = self.install()
        self.assertEqual(validate.call_count, 2, "installation must reauthenticate the final path")
        self.assertEqual(result["archive_sha256"], hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.assertEqual(result["installed"], str(self.destination))
        self.assertEqual((self.destination / archive_owner.EMBEDDED_MANIFEST).read_bytes(), manifest_bytes)
        self.assertEqual(self.marker.read_bytes(), b"retained caller marker\n")
        self.assertEqual({path.name for path in self.destination.iterdir()},
                         {".gitkeep", archive_owner.ARCHIVE_ROOT, "NoritoBridge.artifacts.json"})
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "existing path"):
            self.install()

    def test_existing_directory_and_dangling_link_are_never_replaced(self) -> None:
        for name in (archive_owner.ARCHIVE_ROOT, "NoritoBridge.artifacts.json"):
            for is_link in (False, True):
                with self.subTest(name=name, symlink=is_link):
                    existing = self.destination / name
                    if is_link:
                        existing.symlink_to("caller-owned-dangling-target")
                    else:
                        existing.mkdir()
                    with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "existing path"):
                        self.install()
                    self.assertTrue(os.path.lexists(existing))
                    existing.unlink() if is_link else existing.rmdir()

    def test_racing_framework_or_manifest_destination_is_preserved(self) -> None:
        original = archive_owner._rename_no_replace
        for raced_name in (archive_owner.ARCHIVE_ROOT, "NoritoBridge.artifacts.json"):
            with self.subTest(name=raced_name):
                output = self.base / raced_name.replace(".", "-")
                output.mkdir()
                def compete(source, destination):
                    if destination.name == raced_name:
                        destination.symlink_to("caller-race")
                    original(source, destination)
                with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=self.native_fixture), \
                     mock.patch.object(archive_owner, "_rename_no_replace", side_effect=compete):
                    with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "retained archive stage"):
                        archive_owner.authenticate_archive(self.root, self.archive, self.lockfile, install_directory=output)
                self.assertEqual(os.readlink(output / raced_name), "caller-race")

    def test_failed_native_authentication_cannot_install_or_delete_caller_paths(self) -> None:
        with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=RuntimeError("native refusal")):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "native refusal"):
                self.install()
        self.assertFalse((self.destination / archive_owner.ARCHIVE_ROOT).exists())
        self.assertEqual(self.marker.read_bytes(), b"retained caller marker\n")

    def test_installation_rejects_source_subdirectories_and_directory_aliases(self) -> None:
        nested = self.root / "wrong-artifacts"
        nested.mkdir()
        alias = self.base / "linked-dist"
        alias.symlink_to(self.destination, target_is_directory=True)
        for directory in (nested, alias):
            with self.subTest(directory=directory):
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    archive_owner.authenticate_archive(self.root, self.archive, self.lockfile, install_directory=directory)

    def test_native_admission_unconditionally_invokes_existing_source_pin_and_physical_owners(self) -> None:
        validate = mock.Mock(return_value={"version": "0.1.0"})
        physical = mock.Mock()
        validator = SimpleNamespace(validate=validate)
        inspector = SimpleNamespace(_validate_native_binaries=physical)
        with mock.patch.object(archive_owner, "_load_native_owner", side_effect=(validator, inspector)):
            archive_owner._validate_native_contents(self.root, self.lockfile, self.destination)
        arguments = validate.call_args.kwargs
        self.assertTrue(arguments["verify_repository_provenance"])
        self.assertEqual(arguments["swift_loader"], self.root / "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        self.assertEqual(arguments["lockfile_path"], self.lockfile)
        self.assertNotIn("allow_dirty_source", arguments)
        physical.assert_called_once_with(self.destination / archive_owner.ARCHIVE_ROOT, validator)

    def test_cli_has_no_native_provenance_or_pin_skip(self) -> None:
        result = subprocess.run([sys.executable, "-I", "-S", "-B", str(SCRIPT), "--skip-provenance"],
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--lockfile-path", result.stderr)


if __name__ == "__main__":
    unittest.main()
