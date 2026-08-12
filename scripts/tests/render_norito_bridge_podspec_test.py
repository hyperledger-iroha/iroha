#!/usr/bin/env python3
"""Focused tests for the checksum-pinned NoritoBridge podspec renderer."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
RENDERER = REPOSITORY_ROOT / "scripts/render_norito_bridge_podspec.py"
TEMPLATE = (
    REPOSITORY_ROOT
    / "crates/connect_norito_bridge/NoritoBridge.podspec.template"
)
SPEC = importlib.util.spec_from_file_location("render_norito_bridge_podspec", RENDERER)
assert SPEC is not None and SPEC.loader is not None
renderer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = renderer
SPEC.loader.exec_module(renderer)


class NoritoBridgePodspecRendererTests(unittest.TestCase):
    def setUp(self) -> None:
        if sys.version_info[:2] != (3, 12) or not sys.flags.isolated:
            self.fail("tests require isolated Python 3.12")
        self.temporary = tempfile.TemporaryDirectory(
            prefix="norito-bridge-podspec-renderer-test."
        )
        self.base = Path(self.temporary.name).resolve(strict=True)
        self.root = self.base / "repo"
        self.output_root = self.base / "output"
        self.archive = self.base / "NoritoBridge-v0.1.0.xcframework.zip"
        (self.root / "IrohaSwift").mkdir(parents=True)
        template_parent = self.root / "crates/connect_norito_bridge"
        template_parent.mkdir(parents=True)
        self.output_root.mkdir()
        self.output_root.chmod(0o700)
        (self.root / "IrohaSwift/VERSION").write_text("0.1.0\n", encoding="ascii")
        shutil.copy2(TEMPLATE, template_parent / TEMPLATE.name)
        self.write_archive()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_archive(
        self,
        *,
        unsafe_name: str | None = None,
        symlink: bool = False,
        manifest_version: str = "0.1.0",
    ) -> None:
        with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_STORED) as archive:
            archive.writestr(
                "NoritoBridge.xcframework/NoritoBridge.artifacts.json",
                f'{{"schema_version":1,"version":"{manifest_version}"}}\n'.encode(
                    "ascii"
                ),
            )
            if unsafe_name is not None:
                archive.writestr(unsafe_name, b"unsafe\n")
            if symlink:
                entry = zipfile.ZipInfo("NoritoBridge.xcframework/linked")
                entry.create_system = 3
                entry.external_attr = (stat.S_IFLNK | 0o777) << 16
                archive.writestr(entry, b"target")

    def run_renderer(
        self,
        output: Path,
        *,
        archive: Path | None = None,
        local_source: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        arguments = [
            sys.executable,
            "-I",
            "-S",
            "-B",
            str(RENDERER),
            "--root",
            str(self.root),
            "--archive",
            str(archive or self.archive),
            "--output",
            str(output),
        ]
        if local_source:
            arguments.append("--local-source")
        return subprocess.run(
            arguments,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_renders_canonical_https_source_and_exact_archive_digest(self) -> None:
        output = self.output_root / "NoritoBridge-0.1.0.podspec"
        result = self.run_renderer(output)
        self.assertEqual(result.returncode, 0, result.stderr)
        rendered = output.read_text(encoding="utf-8")
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.assertIn("s.version          = '0.1.0'", rendered)
        self.assertIn(
            "https://github.com/hyperledger-iroha/iroha/releases/download/"
            "v0.1.0/NoritoBridge-v0.1.0.xcframework.zip",
            rendered,
        )
        self.assertIn(f":sha256 => '{digest}'", rendered)
        self.assertIn("s.vendored_frameworks = 'NoritoBridge.xcframework'", rendered)
        for placeholder in ("__VERSION__", "__SOURCE_URL__", "__ARCHIVE_SHA256__"):
            self.assertNotIn(placeholder, rendered)

    def test_explicit_local_mode_uses_exact_archive_file_uri(self) -> None:
        output = self.output_root / "NoritoBridge.local.podspec"
        result = self.run_renderer(output, local_source=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            f":http => '{self.archive.as_uri()}'",
            output.read_text(encoding="utf-8"),
        )

    def test_production_filename_must_match_canonical_version(self) -> None:
        mismatched = self.base / "NoritoBridge-v0.1.1.xcframework.zip"
        self.archive.rename(mismatched)
        result = self.run_renderer(
            self.output_root / "mismatch.podspec",
            archive=mismatched,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "production archive filename must be NoritoBridge-v0.1.0.xcframework.zip",
            result.stderr,
        )
        local_result = self.run_renderer(
            self.output_root / "mismatch.local.podspec",
            archive=mismatched,
            local_source=True,
        )
        self.assertEqual(local_result.returncode, 0, local_result.stderr)

    def test_refuses_overwrite_and_preserves_existing_output(self) -> None:
        output = self.output_root / "NoritoBridge.podspec"
        output.write_bytes(b"preserve\n")
        result = self.run_renderer(output)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("must not already exist", result.stderr)
        self.assertEqual(output.read_bytes(), b"preserve\n")

    def test_rejects_noncanonical_version_and_bad_template_cardinality(self) -> None:
        output = self.output_root / "NoritoBridge.podspec"
        (self.root / "IrohaSwift/VERSION").write_text("01.0.0\n", encoding="ascii")
        version_result = self.run_renderer(output)
        self.assertNotEqual(version_result.returncode, 0)
        self.assertIn("canonical SemVer", version_result.stderr)

        (self.root / "IrohaSwift/VERSION").write_text("0.1.0\n", encoding="ascii")
        template = self.root / "crates/connect_norito_bridge/NoritoBridge.podspec.template"
        template.write_text(
            template.read_text(encoding="utf-8").replace("__ARCHIVE_SHA256__", "missing"),
            encoding="utf-8",
        )
        template_result = self.run_renderer(output)
        self.assertNotEqual(template_result.returncode, 0)
        self.assertIn("__ARCHIVE_SHA256__ exactly once", template_result.stderr)

    def test_rejects_symlinked_or_unsafe_archives(self) -> None:
        linked_archive = self.base / "linked.zip"
        linked_archive.symlink_to(self.archive)
        linked_result = self.run_renderer(
            self.output_root / "linked.podspec", archive=linked_archive
        )
        self.assertNotEqual(linked_result.returncode, 0)
        self.assertIn("non-symbolic canonical", linked_result.stderr)

        self.write_archive(unsafe_name="NoritoBridge.xcframework/../escape")
        unsafe_result = self.run_renderer(self.output_root / "unsafe.podspec")
        self.assertNotEqual(unsafe_result.returncode, 0)
        self.assertIn("unsafe, duplicate, or case-colliding", unsafe_result.stderr)

        self.write_archive(symlink=True)
        symlink_result = self.run_renderer(self.output_root / "symlink.podspec")
        self.assertNotEqual(symlink_result.returncode, 0)
        self.assertIn("symbolic links are forbidden", symlink_result.stderr)

        self.write_archive(
            unsafe_name="NoritoBridge.xcframework/NORITOBRIDGE.ARTIFACTS.JSON"
        )
        collision_result = self.run_renderer(self.output_root / "collision.podspec")
        self.assertNotEqual(collision_result.returncode, 0)
        self.assertIn("case-colliding", collision_result.stderr)

    def test_archive_mutation_changes_the_pinned_digest(self) -> None:
        first = self.output_root / "first.podspec"
        second = self.output_root / "second.podspec"
        self.assertEqual(self.run_renderer(first).returncode, 0)
        first_render = first.read_text(encoding="utf-8")
        self.write_archive(unsafe_name="NoritoBridge.xcframework/extra")
        self.assertEqual(self.run_renderer(second).returncode, 0)
        self.assertNotEqual(first_render, second.read_text(encoding="utf-8"))

    def test_embedded_manifest_version_must_match_pod_version(self) -> None:
        self.write_archive(manifest_version="0.1.1")
        result = self.run_renderer(self.output_root / "manifest-drift.podspec")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "embedded NoritoBridge manifest version must equal IrohaSwift/VERSION",
            result.stderr,
        )

    def test_path_swap_during_read_is_rejected(self) -> None:
        replacement = self.base / "replacement.zip"
        shutil.copy2(self.archive, replacement)
        displaced = self.base / "displaced.zip"
        real_read = renderer.os.read
        swapped = False

        def read_and_swap(descriptor: int, size: int) -> bytes:
            nonlocal swapped
            payload = real_read(descriptor, size)
            if not swapped:
                self.archive.rename(displaced)
                replacement.rename(self.archive)
                swapped = True
            return payload

        with mock.patch.object(renderer.os, "read", side_effect=read_and_swap):
            with self.assertRaisesRegex(renderer.RenderError, "changed while"):
                renderer.read_regular(
                    self.archive,
                    "test archive",
                    max_bytes=renderer.MAX_ARCHIVE_BYTES,
                )

    def test_fifo_leaf_swap_is_nonblocking_and_rejected(self) -> None:
        displaced = self.base / "archive.displaced"
        real_canonical = renderer.canonical_regular_file

        def replace_with_fifo(path: Path, label: str) -> Path:
            canonical = real_canonical(path, label)
            canonical.rename(displaced)
            renderer.os.mkfifo(canonical, 0o600)
            return canonical

        with (
            mock.patch.object(
                renderer,
                "canonical_regular_file",
                side_effect=replace_with_fifo,
            ),
            self.assertRaisesRegex(renderer.RenderError, "single-link regular file"),
        ):
            renderer.read_regular(
                self.archive,
                "test archive",
                max_bytes=renderer.MAX_ARCHIVE_BYTES,
            )

    def test_output_parent_must_be_private_and_current_uid_owned(self) -> None:
        self.output_root.chmod(0o755)
        with self.assertRaisesRegex(renderer.RenderError, "exact mode 0700"):
            renderer.publish_no_replace(
                self.output_root / "public.podspec",
                b"rendered\n",
            )

        self.output_root.chmod(0o700)
        real_lstat = renderer.Path.lstat

        def foreign_owner(path: Path) -> object:
            metadata = real_lstat(path)
            if path == self.output_root:
                values = list(metadata)
                values[4] = metadata.st_uid + 1
                return renderer.os.stat_result(values)
            return metadata

        with (
            mock.patch.object(renderer.Path, "lstat", new=foreign_owner),
            self.assertRaisesRegex(renderer.RenderError, "current-UID-owned"),
        ):
            renderer.publish_no_replace(
                self.output_root / "foreign.podspec",
                b"rendered\n",
            )

    def test_archive_size_entry_count_and_uncompressed_bounds_fail_closed(self) -> None:
        with (
            mock.patch.object(
                renderer,
                "MAX_ARCHIVE_BYTES",
                self.archive.stat().st_size - 1,
            ),
            self.assertRaisesRegex(renderer.RenderError, "byte limit"),
        ):
            renderer.validate_archive(self.archive, "0.1.0")

        with (
            mock.patch.object(renderer, "MAX_ARCHIVE_ENTRIES", 0),
            self.assertRaisesRegex(renderer.RenderError, "entry limit"),
        ):
            renderer.validate_archive(self.archive, "0.1.0")

        with (
            mock.patch.object(renderer, "MAX_ENTRY_BYTES", 1),
            self.assertRaisesRegex(renderer.RenderError, "archive entry exceeds"),
        ):
            renderer.validate_archive(self.archive, "0.1.0")

        with (
            mock.patch.object(renderer, "MAX_TOTAL_UNCOMPRESSED_BYTES", 1),
            self.assertRaisesRegex(renderer.RenderError, "uncompressed limit"),
        ):
            renderer.validate_archive(self.archive, "0.1.0")

        archive_size = self.archive.stat().st_size
        with (
            mock.patch.object(
                renderer.os,
                "read",
                side_effect=(b"x" * archive_size, b"x", b""),
            ),
            self.assertRaisesRegex(renderer.RenderError, "byte limit"),
        ):
            renderer.read_regular(
                self.archive,
                "growing archive",
                max_bytes=archive_size,
            )

    def test_rejects_repository_local_output(self) -> None:
        result = self.run_renderer(self.root / "NoritoBridge.podspec")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("output must be outside the repository", result.stderr)

    def test_publish_final_output_is_exact_single_link_regular_content(self) -> None:
        output = self.output_root / "exact.podspec"
        payload = b"exact rendered podspec\n"
        renderer.publish_no_replace(output, payload)
        metadata = output.lstat()
        self.assertTrue(stat.S_ISREG(metadata.st_mode))
        self.assertEqual(stat.S_IMODE(metadata.st_mode), 0o644)
        self.assertEqual(metadata.st_nlink, 1)
        self.assertEqual(output.read_bytes(), payload)

    def test_post_link_failure_removes_only_the_created_output(self) -> None:
        output = self.output_root / "failed.podspec"
        payload = b"rendered podspec\n"
        with (
            mock.patch.object(renderer.os, "fsync", side_effect=OSError("forced fsync")),
            self.assertRaisesRegex(OSError, "forced fsync"),
        ):
            renderer.publish_no_replace(output, payload)
        self.assertFalse(output.exists())

    def test_first_post_link_identity_failure_removes_owned_output(self) -> None:
        output = self.output_root / "identity-failed.podspec"
        real_identity = renderer.output_identity
        calls = 0

        def fail_first_visible_identity(path: Path) -> tuple[int, int, int, int, int, int]:
            nonlocal calls
            calls += 1
            if calls == 2:
                raise OSError("forced post-link identity failure")
            return real_identity(path)

        with (
            mock.patch.object(
                renderer,
                "output_identity",
                side_effect=fail_first_visible_identity,
            ),
            self.assertRaisesRegex(OSError, "forced post-link identity failure"),
        ):
            renderer.publish_no_replace(output, b"rendered podspec\n")
        self.assertFalse(output.exists())

    def test_repeated_temporary_cleanup_failure_does_not_leak_output(self) -> None:
        output = self.output_root / "temp-cleanup-failed.podspec"
        real_unlink = renderer.Path.unlink

        def fail_temporary_unlink(
            path: Path, *args: object, **kwargs: object
        ) -> None:
            if path.name.startswith(f".{output.name}."):
                raise OSError("forced temporary unlink failure")
            real_unlink(path, *args, **kwargs)

        with (
            mock.patch.object(renderer.Path, "unlink", new=fail_temporary_unlink),
            self.assertRaisesRegex(OSError, "forced temporary unlink failure"),
        ):
            renderer.publish_no_replace(output, b"rendered podspec\n")
        self.assertFalse(output.exists())

    def test_post_link_substitution_is_preserved_on_failure(self) -> None:
        output = self.output_root / "substituted.podspec"
        displaced = self.output_root / "owner.podspec"
        payload = b"rendered podspec\n"
        real_unlink = renderer.Path.unlink
        substituted = False

        def unlink_and_substitute(path: Path, *args: object, **kwargs: object) -> None:
            nonlocal substituted
            if not substituted and path.name.startswith(f".{output.name}."):
                real_unlink(path, *args, **kwargs)
                output.rename(displaced)
                output.write_bytes(b"competitor\n")
                substituted = True
            elif path == output:
                raise OSError("competitor path is outside the owner identity")
            else:
                real_unlink(path, *args, **kwargs)

        with (
            mock.patch.object(renderer.Path, "unlink", new=unlink_and_substitute),
            self.assertRaisesRegex(renderer.RenderError, "final authentication"),
        ):
            renderer.publish_no_replace(output, payload)
        self.assertEqual(output.read_bytes(), b"competitor\n")
        self.assertEqual(displaced.read_bytes(), payload)

    def test_encrypted_or_unsupported_entries_fail_without_traceback(self) -> None:
        class UnsupportedArchive:
            def __enter__(self) -> "UnsupportedArchive":
                return self

            def __exit__(self, *_arguments: object) -> None:
                return None

            def infolist(self) -> list[object]:
                raise NotImplementedError("unsupported compression")

        with (
            mock.patch.object(renderer.zipfile, "ZipFile", return_value=UnsupportedArchive()),
            self.assertRaisesRegex(renderer.RenderError, "unable to authenticate"),
        ):
            renderer.validate_archive(self.archive, "0.1.0")

        encrypted = bytearray(self.archive.read_bytes())
        for signature, flag_offset in ((b"PK\x03\x04", 6), (b"PK\x01\x02", 8)):
            start = 0
            while (position := encrypted.find(signature, start)) >= 0:
                flags = int.from_bytes(
                    encrypted[position + flag_offset : position + flag_offset + 2],
                    "little",
                )
                encrypted[position + flag_offset : position + flag_offset + 2] = (
                    flags | 0x1
                ).to_bytes(2, "little")
                start = position + 1
        self.archive.write_bytes(encrypted)
        with self.assertRaisesRegex(renderer.RenderError, "encrypted"):
            renderer.validate_archive(self.archive, "0.1.0")


if __name__ == "__main__":
    unittest.main()
