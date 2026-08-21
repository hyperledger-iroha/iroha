#!/usr/bin/env python3
"""Focused tests for two-pass Kotodama golden ownership and publication."""

from __future__ import annotations

import contextlib
import hashlib
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
            ("--write",),
            ("--write", "--check"),
            ("--write", "--write"),
            ("--check", "--check"),
            ("--output-root",),
            ("--staging-root",),
            ("--write", "--output-root", ""),
            ("--write", "--staging-root", ""),
            ("--write", "--output-root", "."),
            ("--write", "--output-root", "cache/output"),
            ("--check", "--output-root", "cache/output"),
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
    def test_output_root_must_be_outside_the_live_repository(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve(strict=True)
            source = parent / "source"
            source.mkdir()
            with self.assertRaisesRegex(MODULE.GoldenError, "outside"):
                MODULE._require_external_output_path(
                    source,
                    source / "publication",
                )
            publication = parent / "publication"
            self.assertEqual(
                MODULE._require_external_output_path(source, publication),
                publication,
            )

    def test_external_publication_path_is_not_reserved_during_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve(strict=True)
            publication = parent / "publication"
            selected = MODULE._preflight_create_only_output_path(publication)
            self.assertEqual(selected, publication)
            self.assertFalse(selected.exists())

            selected.mkdir()
            with self.assertRaisesRegex(MODULE.GoldenError, "create-only"):
                MODULE._preflight_create_only_output_path(publication)

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
            root = Path(temporary).resolve(strict=True)
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


class SourcePolicyTests(unittest.TestCase):
    @staticmethod
    def _attribute_result(payload: bytes, returncode: int = 0) -> object:
        return MODULE.subprocess.CompletedProcess(
            args=("git", "check-attr"),
            returncode=returncode,
            stdout=payload,
            stderr=b"classification failed" if returncode else b"",
        )

    def test_unset_and_unspecified_sources_are_partitioned_exactly(self) -> None:
        sources = (Path("contracts/live.ko"), Path("fixtures/extracted.ko"))
        output = (
            b"contracts/live.ko\0whitespace\0unspecified\0"
            b"fixtures/extracted.ko\0whitespace\0unset\0"
        )
        with mock.patch.object(
            MODULE.subprocess,
            "run",
            return_value=self._attribute_result(output),
        ) as run:
            checked, byte_exact = MODULE.partition_source_policy(
                Path("/repository"), sources
            )
        self.assertEqual(checked, (sources[0],))
        self.assertEqual(byte_exact, (sources[1],))
        self.assertEqual(
            run.call_args.args[0],
            [
                "git",
                "-C",
                "/repository",
                "check-attr",
                "-z",
                "whitespace",
                "--",
                "contracts/live.ko",
                "fixtures/extracted.ko",
            ],
        )

    def test_malformed_attribute_framing_fails_closed(self) -> None:
        sources = (Path("contracts/live.ko"),)
        malformed = (
            b"contracts/live.ko\0whitespace\0unspecified",
            b"contracts/live.ko\0whitespace\0",
            b"other.ko\0whitespace\0unspecified\0",
            b"contracts/live.ko\0other\0unspecified\0",
            b"contracts/live.ko\0whitespace\0\xff\0",
        )
        for output in malformed:
            with self.subTest(output=output):
                with (
                    mock.patch.object(
                        MODULE.subprocess,
                        "run",
                        return_value=self._attribute_result(output),
                    ),
                    self.assertRaises(MODULE.GoldenError),
                ):
                    MODULE.partition_source_policy(Path("/repository"), sources)

    def test_attributes_file_is_sealed_as_a_generation_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "scripts").mkdir()
            (root / MODULE.ATTRIBUTES_PATH).write_text(
                "fixtures/** -whitespace\n", encoding="utf-8"
            )
            (root / MODULE.MAP_PATH).write_text("map\n", encoding="utf-8")
            source = root / "contract.ko"
            koto = root / "koto"
            iroha = root / "iroha"
            for path, payload in (
                (source, b"contract"),
                (koto, b"koto"),
                (iroha, b"iroha"),
            ):
                path.write_bytes(payload)

            before = MODULE.generation_input_seals(
                root, (Path("contract.ko"),), koto, iroha
            )
            attribute_seal = next(
                seal for seal in before if seal.path == root / MODULE.ATTRIBUTES_PATH
            )
            self.assertEqual(
                attribute_seal.sha256,
                hashlib.sha256(b"fixtures/** -whitespace\n").hexdigest(),
            )

            (root / MODULE.ATTRIBUTES_PATH).write_text(
                "fixtures/** whitespace\n", encoding="utf-8"
            )
            after = MODULE.generation_input_seals(
                root, (Path("contract.ko"),), koto, iroha
            )
            self.assertNotEqual(before, after)

    def test_attribute_value_tamper_fails_closed(self) -> None:
        source = Path("contracts/live.ko")
        for value in (b"set", b"trailing-space", b"whitespace"):
            with self.subTest(value=value):
                output = b"contracts/live.ko\0whitespace\0" + value + b"\0"
                with (
                    mock.patch.object(
                        MODULE.subprocess,
                        "run",
                        return_value=self._attribute_result(output),
                    ),
                    self.assertRaisesRegex(
                        MODULE.GoldenError, "unsupported whitespace attribute"
                    ),
                ):
                    MODULE.partition_source_policy(Path("/repository"), (source,))


class StagedPublicationTests(unittest.TestCase):
    def test_publish_and_check_touch_only_the_distinct_output_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            source_root = root / "source"
            output_root = root / "output"
            stage = root / "stage"
            source_root.mkdir()
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
                mock.patch.object(MODULE, "FORBIDDEN_LEGACY_OUTPUTS", ()),
            ):
                rendered = MODULE.rendered_files(stage, rows)
                self.assertEqual(
                    MODULE.publish_external_create_only(output_root, rendered),
                    1,
                )
                with self.assertRaisesRegex(MODULE.GoldenError, "create-only"):
                    MODULE.publish_external_create_only(output_root, rendered)
                self.assertEqual(
                    MODULE.verify_rendered_tree(output_root, rendered),
                    0,
                )

                destination = output_root / "artifacts/example.to"
                before = b"stale artifact"
                destination.write_bytes(before)
                with self.assertRaises(MODULE.GoldenError):
                    MODULE.verify_rendered_tree(output_root, rendered)
                self.assertEqual(destination.read_bytes(), before)

            self.assertEqual(sentinel.read_bytes(), b"source remains untouched")
            self.assertEqual([entry.name for entry in source_root.iterdir()], ["sentinel"])

    def test_nested_output_symlink_cannot_escape_the_selected_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
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
                mock.patch.object(MODULE, "FORBIDDEN_LEGACY_OUTPUTS", ()),
                self.assertRaises(MODULE.GoldenError),
            ):
                rendered = MODULE.rendered_files(stage, rows)
                MODULE.publish_external_create_only(output_root, rendered)
            self.assertEqual(list(outside.iterdir()), [])

    def test_forbidden_legacy_output_is_rejected_without_removal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output_root = Path(temporary)
            legacy = Path("legacy/retired.to")
            legacy_path = output_root / legacy
            legacy_path.parent.mkdir()
            legacy_path.write_bytes(b"retired")

            with (
                mock.patch.object(MODULE, "FORBIDDEN_LEGACY_OUTPUTS", (legacy,)),
                mock.patch.object(MODULE, "repository_root", return_value=output_root),
                self.assertRaisesRegex(MODULE.GoldenError, "retired Kotodama"),
            ):
                MODULE.verify_rendered_tree(output_root, ())
            self.assertEqual(legacy_path.read_bytes(), b"retired")

    def test_unsealed_or_unexpected_external_tree_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(strict=True)
            publication = root / "publication"
            rendered = (
                MODULE.RenderedFile(Path("artifacts/example.to"), 0o644, b"artifact"),
            )
            publication.mkdir(mode=0o700)
            (publication / "artifacts").mkdir(mode=0o755)
            (publication / "artifacts/example.to").write_bytes(b"artifact")
            with self.assertRaisesRegex(MODULE.GoldenError, "complete"):
                MODULE.verify_rendered_tree(publication, rendered)

            manifest = publication / MODULE.PUBLICATION_MANIFEST_PATH
            manifest.write_bytes(MODULE.owner_manifest(rendered))
            (publication / "unexpected").mkdir(mode=0o755)
            with self.assertRaisesRegex(MODULE.GoldenError, "unexpected"):
                MODULE.verify_rendered_tree(publication, rendered)

    def test_failed_preseal_leaves_rejected_residue_without_completion(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            publication = Path(temporary).resolve(strict=True) / "publication"
            rendered = (
                MODULE.RenderedFile(Path("artifacts/example.to"), 0o644, b"artifact"),
            )

            def reject() -> None:
                raise MODULE.GoldenError("source drift")

            with self.assertRaisesRegex(MODULE.GoldenError, "source drift"):
                MODULE.publish_external_create_only(
                    publication,
                    rendered,
                    preseal=reject,
                )
            self.assertTrue((publication / "artifacts/example.to").is_file())
            self.assertFalse((publication / MODULE.PUBLICATION_MANIFEST_PATH).exists())
            with self.assertRaisesRegex(MODULE.GoldenError, "complete"):
                MODULE.verify_rendered_tree(publication, rendered)


class TwoPassTests(unittest.TestCase):
    def test_renderings_bind_sorted_paths_modes_bytes_and_manifest(self) -> None:
        first = (
            MODULE.RenderedFile(Path("a/one.to"), 0o644, b"one"),
            MODULE.RenderedFile(Path("b/two.to"), 0o644, b"two"),
        )
        MODULE.compare_renderings(first, tuple(first))
        self.assertEqual(
            MODULE.owner_manifest(first),
            MODULE.owner_manifest(tuple(first)),
        )
        manifest = MODULE.json.loads(MODULE.owner_manifest(first))
        self.assertEqual(manifest["root_mode"], "0700")
        self.assertEqual(
            manifest["directories"],
            [
                {"path": "a", "mode": "0755"},
                {"path": "b", "mode": "0755"},
            ],
        )

        for changed in (
            first[::-1],
            (first[0], MODULE.RenderedFile(first[1].relative_path, 0o600, b"two")),
            (first[0], MODULE.RenderedFile(first[1].relative_path, 0o644, b"drift")),
        ):
            with self.subTest(changed=changed):
                with self.assertRaises(MODULE.GoldenError):
                    MODULE.compare_renderings(first, changed)


if __name__ == "__main__":
    unittest.main()
