#!/usr/bin/env python3
"""Focused tests for cache-staged Kotodama golden publication."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "regenerate_kotodama_goldens.py"
SPEC = importlib.util.spec_from_file_location("regenerate_kotodama_goldens", SCRIPT)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import machinery guard
    raise RuntimeError(f"could not import {SCRIPT}")
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class CommandLineTests(unittest.TestCase):
    def test_explicit_output_and_staging_roots_are_parsed(self) -> None:
        defaults = MODULE.parse_args(())
        self.assertFalse(defaults.write)
        self.assertFalse(defaults.check)
        self.assertEqual(defaults.koto, Path("target/debug/koto"))
        self.assertEqual(defaults.iroha, Path("target/debug/iroha"))
        self.assertIsNone(defaults.output_root)
        self.assertIsNone(defaults.staging_root)

        options = MODULE.parse_args(
            (
                "--write",
                "--output-root",
                "/cache/output",
                "--staging-root",
                "/cache/work",
            )
        )
        self.assertTrue(options.write)
        self.assertEqual(options.output_root, Path("/cache/output"))
        self.assertEqual(options.staging_root, Path("/cache/work"))

        for arguments in (
            ("--write", "--check"),
            ("--write", "--write"),
            ("--check", "--check"),
            ("--output-root",),
            ("--staging-root",),
            ("--write", "--output-root", ""),
            ("--write", "--staging-root", ""),
            ("--write", "--output-root", "."),
            ("--write", "--staging-root", ".."),
            ("--write", "--output-root", "/"),
            ("--output-root", "first", "--output-root", "second"),
            ("--staging-root", "first", "--staging-root", "second"),
            ("--koto", ""),
            ("--iroha", ""),
            ("--koto", "first", "--koto", "second"),
            ("--iroha", "first", "--iroha", "second"),
            ("--koto=-h",),
            ("--iroha=--write",),
            ("--output-root=-h",),
            ("--staging-root=--write",),
            ("--skip-runtime-manifest-check",),
            ("--skip-contract-tests",),
            ("--unknown",),
        ):
            with self.subTest(arguments=arguments):
                with contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit):
                        MODULE.parse_args(arguments)

    def test_relative_locations_are_rooted_at_the_live_repository(self) -> None:
        root = Path("/live/repository")
        self.assertEqual(
            MODULE._rooted_path(root, Path("cache/output"), root),
            root / "cache/output",
        )
        self.assertEqual(
            MODULE._rooted_path(root, Path("/cache/output"), root),
            Path("/cache/output"),
        )
        self.assertEqual(MODULE._rooted_path(root, None, root), root)


class DirectorySafetyTests(unittest.TestCase):
    def test_check_does_not_create_a_missing_output_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            missing = Path(temporary) / "missing"
            with self.assertRaises(MODULE.GoldenError):
                MODULE._prepare_real_directory(
                    missing,
                    context="output root",
                    create=False,
                )
            self.assertFalse(missing.exists())

            MODULE._prepare_real_directory(
                missing,
                context="output root",
                create=True,
            )
            self.assertTrue(missing.is_dir())

    def test_symlink_root_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "target"
            link = root / "link"
            target.mkdir()
            try:
                link.symlink_to(target, target_is_directory=True)
            except (NotImplementedError, OSError):
                self.skipTest("symlinks are unavailable on this platform")
            with self.assertRaises(MODULE.GoldenError):
                MODULE._prepare_real_directory(
                    link,
                    context="output root",
                    create=False,
                )

    def test_symlink_above_the_selected_root_is_canonicalized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "target"
            link = root / "link"
            target.mkdir()
            try:
                link.symlink_to(target, target_is_directory=True)
            except (NotImplementedError, OSError):
                self.skipTest("symlinks are unavailable on this platform")
            selected = MODULE._prepare_real_directory(
                link / "nested",
                context="output root",
                create=True,
            )
            self.assertEqual(selected, (target / "nested").resolve())
            self.assertTrue(selected.is_dir())


class StagedPublicationTests(unittest.TestCase):
    def test_write_and_check_touch_only_the_distinct_output_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source_root = root / "source"
            output_root = root / "output"
            stage = root / "stage"
            source_root.mkdir()
            output_root.mkdir()
            (stage / "release").mkdir(parents=True)
            sentinel = source_root / "sentinel"
            sentinel.write_bytes(b"source remains untouched")
            (stage / "release/example.to").write_bytes(b"canonical artifact")
            rows = [
                MODULE.Golden(
                    mode="standard",
                    source=Path("contracts/example.ko"),
                    destination=Path("artifacts/example.to"),
                )
            ]

            with (
                mock.patch.object(MODULE, "COMPILER_MANIFESTS", {}),
                mock.patch.object(MODULE, "RETIRED_OUTPUTS", ()),
            ):
                self.assertEqual(
                    MODULE.publish_or_check(output_root, stage, rows, True),
                    1,
                )
                self.assertEqual(
                    MODULE.publish_or_check(output_root, stage, rows, True),
                    0,
                )
                self.assertEqual(
                    MODULE.publish_or_check(output_root, stage, rows, False),
                    0,
                )

                destination = output_root / "artifacts/example.to"
                before = b"stale artifact"
                destination.write_bytes(before)
                with self.assertRaises(MODULE.GoldenError):
                    MODULE.publish_or_check(output_root, stage, rows, False)
                self.assertEqual(destination.read_bytes(), before)

            self.assertEqual(sentinel.read_bytes(), b"source remains untouched")
            self.assertEqual([entry.name for entry in source_root.iterdir()], ["sentinel"])

    def test_nested_output_symlink_cannot_escape_the_selected_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output_root = root / "output"
            outside = root / "outside"
            stage = root / "stage"
            output_root.mkdir()
            outside.mkdir()
            (stage / "release").mkdir(parents=True)
            (stage / "release/example.to").write_bytes(b"canonical artifact")
            try:
                (output_root / "artifacts").symlink_to(
                    outside,
                    target_is_directory=True,
                )
            except (NotImplementedError, OSError):
                self.skipTest("symlinks are unavailable on this platform")
            rows = [
                MODULE.Golden(
                    mode="standard",
                    source=Path("contracts/example.ko"),
                    destination=Path("artifacts/example.to"),
                )
            ]

            with (
                mock.patch.object(MODULE, "COMPILER_MANIFESTS", {}),
                mock.patch.object(MODULE, "RETIRED_OUTPUTS", ()),
                self.assertRaises(MODULE.GoldenError),
            ):
                MODULE.publish_or_check(output_root, stage, rows, True)
            self.assertEqual(list(outside.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
