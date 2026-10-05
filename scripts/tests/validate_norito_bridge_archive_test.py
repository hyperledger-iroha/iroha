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

    def test_archive_limits_remain_explicit_and_bounded(self) -> None:
        self.assertEqual(archive_owner.MAX_ARCHIVE_BYTES, 1024 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_ENTRY_BYTES, 512 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_TOTAL_UNCOMPRESSED_BYTES, 1024 * 1024 * 1024)
        self.assertEqual(archive_owner.MAX_ARCHIVE_ENTRIES, 4096)
        self.assertEqual(archive_owner.MAX_MANIFEST_BYTES, 64 * 1024)

    def test_valid_stored_archive_returns_original_bytes(self) -> None:
        self.assertEqual(self.bounded(), self.archive.read_bytes())

    def test_archive_byte_budget_accepts_boundary_and_rejects_one_byte_over(self) -> None:
        payload = self.archive.read_bytes()
        with mock.patch.object(archive_owner, "MAX_ARCHIVE_BYTES", len(payload)):
            self.assertEqual(self.bounded(), payload)
        with mock.patch.object(archive_owner, "MAX_ARCHIVE_BYTES", len(payload) - 1):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "byte limit"):
                self.bounded()

    def test_entry_byte_budget_accepts_boundary_and_rejects_one_byte_over(self) -> None:
        self.write_archive([("NoritoBridge.xcframework/member", b"x" * 64)])
        payload = self.archive.read_bytes()
        with mock.patch.object(archive_owner, "MAX_ENTRY_BYTES", 64):
            self.assertEqual(self.bounded(), payload)
        with mock.patch.object(archive_owner, "MAX_ENTRY_BYTES", 63):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "entry exceeds"):
                self.bounded()

    def test_expanded_byte_budget_accepts_boundary_and_rejects_one_byte_over(self) -> None:
        self.write_archive([("NoritoBridge.xcframework/member", b"x" * 64)])
        payload = self.archive.read_bytes()
        with zipfile.ZipFile(self.archive) as bundle:
            expanded_bytes = sum(entry.file_size for entry in bundle.infolist())
        with mock.patch.object(archive_owner, "MAX_TOTAL_UNCOMPRESSED_BYTES", expanded_bytes):
            self.assertEqual(self.bounded(), payload)
        with mock.patch.object(archive_owner, "MAX_TOTAL_UNCOMPRESSED_BYTES", expanded_bytes - 1):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "uncompressed limit"):
                self.bounded()

    def test_actual_over_budget_archive_is_refused_before_payload_read(self) -> None:
        # A sparse inert fixture checks the real cap without reading or allocating it.
        with self.archive.open("r+b") as output:
            output.truncate(archive_owner.MAX_ARCHIVE_BYTES + 1)
        with mock.patch.object(archive_owner.os, "read") as read, \
             mock.patch.object(archive_owner.zipfile, "ZipFile") as unpack:
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "byte limit"):
                self.bounded()
        read.assert_not_called()
        unpack.assert_not_called()

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

    def test_consumer_requires_independent_trust_inputs_before_reading_archive(self) -> None:
        with mock.patch.object(archive_owner, "validate_archive") as read:
            for digest, commit in ((None, "a" * 40), ("A" * 64, "a" * 40),
                                   ("a" * 64, None), ("a" * 64, "HEAD")):
                with self.subTest(digest=digest, commit=commit):
                    with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "requires"):
                        archive_owner.authenticate_archive(
                            self.root, self.archive, self.lockfile, consumer=True,
                            expected_sha256=digest, expected_source_commit=commit,
                        )
            read.assert_not_called()
        with mock.patch.object(archive_owner, "_extract_archive") as extract:
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "SHA-256"):
                archive_owner.authenticate_archive(
                    self.root, self.archive, self.lockfile, consumer=True,
                    expected_sha256="0" * 64, expected_source_commit="a" * 40,
                )
            extract.assert_not_called()

    def test_consumer_installs_exact_archive_and_reauthenticates_final_destination(self) -> None:
        commit = "a" * 40
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        def authenticate(root, lock, directory, *, consumer_source_commit):
            self.assertEqual(consumer_source_commit, commit)
            return self.native_fixture(root, lock, directory)
        with mock.patch.object(archive_owner, "_validate_native_contents", side_effect=authenticate) as check:
            result = archive_owner.authenticate_archive(
                self.root, self.archive, self.lockfile, consumer=True,
                expected_sha256=digest, expected_source_commit=commit,
                install_directory=self.destination,
            )
        self.assertEqual(check.call_count, 2)
        self.assertEqual(result["archive_sha256"], digest)
        self.assertEqual(result["installed"], str(self.destination))
        self.assertEqual(self.marker.read_bytes(), b"retained caller marker\n")
        with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "existing path"):
            archive_owner.authenticate_archive(
                self.root, self.archive, self.lockfile, consumer=True,
                expected_sha256=digest, expected_source_commit=commit,
                install_directory=self.destination,
            )

    def test_consumer_checks_native_policy_with_recipient_tools_and_installer_inspector(self) -> None:
        commit = "a" * 40
        manifest = {"source_commit": commit, "embedded_source_commit": commit,
                    "source_tree_dirty": False}
        validator = SimpleNamespace(validate=mock.Mock(return_value=manifest))
        inspector = SimpleNamespace(_validate_native_binaries=mock.Mock())
        # A frozen older source inspector has no consumer-tool parameter. Only
        # the trusted installed verifier may select local inspection tools.
        def load(root, name):
            if name == "validate_norito_bridge_xcframework.py":
                self.assertEqual(root, self.root)
                return validator
            self.assertEqual(name, "archive_norito_xcframework.py")
            self.assertEqual(root, ROOT)
            return inspector
        with mock.patch.object(archive_owner, "_load_native_owner", side_effect=load), \
             mock.patch.object(archive_owner, "_consumer_source_identity", return_value=(commit, commit)) as source, \
             mock.patch.object(archive_owner, "_consumer_developer_directory", return_value=self.base):
            self.assertEqual(archive_owner._validate_native_contents(
                self.root, self.lockfile, self.destination, consumer_source_commit=commit,
            ), manifest)
        self.assertEqual(source.call_count, 2)
        self.assertFalse(validator.validate.call_args.kwargs["verify_repository_provenance"])
        self.assertEqual(validator.validate.call_args.kwargs["lockfile_path"], self.lockfile)
        self.assertEqual(validator.validate.call_args.kwargs["swift_loader"],
                         self.root / "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift")
        inspector._validate_native_binaries.assert_called_once_with(
            self.destination / archive_owner.ARCHIVE_ROOT, validator, developer_dir=self.base,
        )

    def test_consumer_rejects_artifact_identity_and_native_policy_failures(self) -> None:
        commit = "a" * 40
        for change in ({"source_commit": "b" * 40}, {"embedded_source_commit": "b" * 40},
                       {"source_tree_dirty": True}):
            manifest = {"source_commit": commit, "embedded_source_commit": commit,
                        "source_tree_dirty": False, **change}
            validator = SimpleNamespace(validate=mock.Mock(return_value=manifest))
            with self.subTest(change=change), \
                 mock.patch.object(archive_owner, "_load_native_owner", return_value=validator), \
                 mock.patch.object(archive_owner, "_consumer_source_identity", return_value=(commit, commit)):
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    archive_owner._validate_native_contents(
                        self.root, self.lockfile, self.destination, consumer_source_commit=commit,
                    )
        for failure in ("header mismatch", "slice digest mismatch", "lock mismatch", "Swift pin mismatch"):
            validator = SimpleNamespace(validate=mock.Mock(side_effect=RuntimeError(failure)))
            with self.subTest(failure=failure), \
                 mock.patch.object(archive_owner, "_load_native_owner", return_value=validator), \
                 mock.patch.object(archive_owner, "_consumer_source_identity", return_value=(commit, commit)):
                with self.assertRaisesRegex(archive_owner.ArchiveValidationError, failure):
                    archive_owner._validate_native_contents(
                        self.root, self.lockfile, self.destination, consumer_source_commit=commit,
                    )

    def test_consumer_refuses_source_changes_during_native_inspection(self) -> None:
        commit = "a" * 40
        manifest = {"source_commit": commit, "embedded_source_commit": commit, "source_tree_dirty": False}
        validator = SimpleNamespace(validate=mock.Mock(return_value=manifest))
        inspector = SimpleNamespace(_validate_native_binaries=mock.Mock())
        with mock.patch.object(archive_owner, "_load_native_owner", side_effect=(validator, inspector)), \
             mock.patch.object(archive_owner, "_consumer_developer_directory", return_value=self.base), \
             mock.patch.object(archive_owner, "_consumer_source_identity",
                               side_effect=((commit, commit), ("b" * 40, commit))):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "source changed"):
                archive_owner._validate_native_contents(
                    self.root, self.lockfile, self.destination, consumer_source_commit=commit,
                )

    def test_unrelated_source_cannot_execute_its_own_validator(self) -> None:
        with mock.patch.object(archive_owner, "_consumer_source_identity",
                               side_effect=archive_owner.ArchiveValidationError("unrelated HEAD")), \
             mock.patch.object(archive_owner, "_load_native_owner") as load:
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "unrelated HEAD"):
                archive_owner._validate_native_contents(
                    self.root, self.lockfile, self.destination, consumer_source_commit="a" * 40,
                )
            load.assert_not_called()

    def test_consumer_source_admission_uses_installer_policy_and_rejects_dirty_or_replaced_source(self) -> None:
        commit = "a" * 40
        policy = SimpleNamespace(validate_pin_relationship=mock.Mock(),
                                 embedded_source_commit=mock.Mock(return_value=commit))
        with mock.patch.object(archive_owner, "_consumer_git", side_effect=(commit.encode(), b"", b"", b"H source\0")), \
             mock.patch.object(archive_owner, "_load_native_owner", return_value=policy) as load:
            self.assertEqual(archive_owner._consumer_source_identity(self.root, commit), (commit, commit))
        load.assert_called_once_with(ROOT, "check_mobile_sdk_artifact_pin_commit.py")
        policy.validate_pin_relationship.assert_called_once_with(self.root, commit)
        for status, refs in ((b" M changed\0", b""), (b"", b"refs/replace/aaaa\n")):
            with self.subTest(status=status, refs=refs), \
                 mock.patch.object(archive_owner, "_consumer_git", side_effect=(commit.encode(), status, refs)), \
                 mock.patch.object(archive_owner, "_load_native_owner") as load:
                with self.assertRaises(archive_owner.ArchiveValidationError):
                    archive_owner._consumer_source_identity(self.root, commit)
                load.assert_not_called()
        policy.validate_pin_relationship.side_effect = RuntimeError("unrelated HEAD")
        with mock.patch.object(archive_owner, "_consumer_git", side_effect=(commit.encode(), b"", b"", b"H source\0")), \
             mock.patch.object(archive_owner, "_load_native_owner", return_value=policy):
            with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "unrelated HEAD"):
                archive_owner._consumer_source_identity(self.root, commit)

    def test_consumer_rejects_hidden_index_changes_before_loading_source(self) -> None:
        for entry in (b"h source\0", b"S source\0", b"s source\0"):
            with self.subTest(entry=entry), \
                 mock.patch.object(archive_owner, "_consumer_git", side_effect=(b"a" * 40, b"", b"", entry)), \
                 mock.patch.object(archive_owner, "_load_native_owner") as load:
                with self.assertRaisesRegex(archive_owner.ArchiveValidationError, "index flags"):
                    archive_owner._consumer_source_identity(self.root, "a" * 40)
                load.assert_not_called()

    def test_consumer_git_ignores_inherited_rust_and_git_configuration(self) -> None:
        with mock.patch.dict(os.environ, {"GIT_DIR": "/untrusted", "RUSTUP_TOOLCHAIN": "missing"}), \
             mock.patch.object(archive_owner.subprocess, "run", return_value=SimpleNamespace(stdout=b"ok")) as run:
            self.assertEqual(archive_owner._consumer_git(self.root, "status"), b"ok")
        self.assertEqual(run.call_args.args[0][0], "/usr/bin/git")
        self.assertIn("--no-replace-objects", run.call_args.args[0])
        self.assertNotIn("GIT_DIR", run.call_args.kwargs["env"])
        self.assertNotIn("RUSTUP_TOOLCHAIN", run.call_args.kwargs["env"])

    def test_consumer_developer_directory_uses_local_selection(self) -> None:
        with mock.patch.dict(os.environ, {"DEVELOPER_DIR": str(self.base)}), \
             mock.patch.object(archive_owner.subprocess, "run") as run:
            self.assertEqual(archive_owner._consumer_developer_directory(), self.base)
            run.assert_not_called()
        with mock.patch.dict(os.environ, {}, clear=True), \
             mock.patch.object(archive_owner.subprocess, "run",
                               return_value=SimpleNamespace(stdout=str(self.base) + "\n")) as run:
            self.assertEqual(archive_owner._consumer_developer_directory(), self.base)
            self.assertEqual(run.call_args.args[0], ["/usr/bin/xcode-select", "-p"])

    def test_cli_has_no_native_provenance_or_pin_skip(self) -> None:
        result = subprocess.run([sys.executable, "-I", "-S", "-B", str(SCRIPT), "--skip-provenance"],
                                text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--lockfile-path", result.stderr)


if __name__ == "__main__":
    unittest.main()
