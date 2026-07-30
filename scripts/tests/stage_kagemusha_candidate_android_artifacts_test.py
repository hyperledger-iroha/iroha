"""Focused tests for the app-private Kagemusha Android artifact stager."""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, os.fspath(SCRIPTS))
import stage_kagemusha_candidate_android_artifacts as stage  # noqa: E402


class CapturingPopen:
    """Minimal synchronous stdin sink for exercising the streaming loop."""

    def __init__(self) -> None:
        self.argv: list[str] | None = None
        self.stdin = self
        self.payload = bytearray()
        self.closed = False

    def write(self, chunk: bytes) -> int:
        self.payload.extend(chunk)
        return len(chunk)

    def close(self) -> None:
        self.closed = True

    def wait(self) -> int:
        return 0


class AndroidArtifactStagerTests(unittest.TestCase):
    def test_inventory_uses_exact_canonical_names_sizes_and_hashes(self) -> None:
        root = Path("/tmp/unused-stage-root")
        entries = []
        for index, name in enumerate(stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4):
            entries.append(
                {
                    "path": f"evidence/candidate/artifacts/{name}",
                    "mode": "0600",
                    "size_bytes": index + 1,
                    "sha256": hashlib.sha256(name.encode()).hexdigest(),
                }
            )
        inventory = stage._artifact_inventory(root, {"entries": entries})
        self.assertEqual(
            tuple(entry.name for entry in inventory),
            stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
        )
        self.assertEqual(inventory[0].size_bytes, 1)
        self.assertEqual(inventory[-1].sha256, entries[-1]["sha256"])

    def test_inventory_accepts_exact_v4_limit_and_rejects_next_byte(self) -> None:
        entries = [
            {
                "path": f"evidence/candidate/artifacts/{name}",
                "mode": "0600",
                "size_bytes": 1,
                "sha256": hashlib.sha256(name.encode()).hexdigest(),
            }
            for name in stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
        ]
        entries[0]["size_bytes"] = stage.MAX_ARTIFACT_BYTES
        stage._artifact_inventory(Path("/tmp/unused-stage-root"), {"entries": entries})
        entries[0]["size_bytes"] += 1
        with self.assertRaisesRegex(stage.StageError, "outside the V4 corridor"):
            stage._artifact_inventory(Path("/tmp/unused-stage-root"), {"entries": entries})

    def test_inventory_rejects_an_extra_or_repeated_artifact_catalog_path(self) -> None:
        entries = [
            {
                "path": f"evidence/candidate/artifacts/{name}",
                "mode": "0600",
                "size_bytes": 1,
                "sha256": hashlib.sha256(name.encode()).hexdigest(),
            }
            for name in stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
        ]
        entries.append(
            {
                "path": "evidence/candidate/artifacts/unbound.krv4",
                "mode": "0600",
                "size_bytes": 1,
                "sha256": hashlib.sha256(b"extra").hexdigest(),
            }
        )
        with self.assertRaisesRegex(stage.StageError, "missing or extra"):
            stage._artifact_inventory(Path("/tmp/unused-stage-root"), {"entries": entries})
        entries[-1] = dict(entries[0])
        with self.assertRaisesRegex(stage.StageError, "repeats"):
            stage._artifact_inventory(Path("/tmp/unused-stage-root"), {"entries": entries})

    def test_source_parent_chain_rejects_symlink_redirection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            redirected = root / "redirected"
            (redirected / "candidate/artifacts").mkdir(parents=True)
            (root / "evidence").symlink_to(redirected, target_is_directory=True)
            with self.assertRaisesRegex(stage.StageError, "real directories"):
                stage._validate_source_parent_chain(root)

    def test_stream_authenticates_source_while_using_adb_shell_without_a_pty(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "step-eq.proving-key.krv4"
            payload = (b"authenticated-krv4\x00" * 8192) + b"tail"
            source.write_bytes(payload)
            source.chmod(0o600)
            entry = stage.ArtifactEntry(
                name=source.name,
                path=source,
                size_bytes=len(payload),
                sha256=hashlib.sha256(payload).hexdigest(),
            )
            process = CapturingPopen()

            def popen(argv: list[str], **_kwargs: object) -> CapturingPopen:
                process.argv = argv
                return process

            with mock.patch.object(stage.subprocess, "Popen", side_effect=popen):
                stage._stream_artifact(
                    ["/absolute/adb"],
                    stage.PACKAGE,
                    entry,
                    "no_backup/kagemusha-candidate-artifacts-v1/incoming",
                )
            self.assertEqual(bytes(process.payload), payload)
            self.assertTrue(process.closed)
            self.assertIsNotNone(process.argv)
            assert process.argv is not None
            self.assertEqual(process.argv[1:3], ["shell", "-T"])
            remote = process.argv[3]
            self.assertIn(f"run-as {stage.PACKAGE} sh -c", remote)
            self.assertIn(f".{source.name}.tmp", remote)
            self.assertIn("mv", remote)

    def test_stream_rejects_bytes_that_do_not_match_the_catalog(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "step-ep.proving-key.krv4"
            source.write_bytes(b"substituted")
            source.chmod(0o600)
            entry = stage.ArtifactEntry(
                name=source.name,
                path=source,
                size_bytes=source.stat().st_size,
                sha256=hashlib.sha256(b"different").hexdigest(),
            )
            process = CapturingPopen()
            with (
                mock.patch.object(stage.subprocess, "Popen", return_value=process),
                self.assertRaisesRegex(stage.StageError, "catalog binding"),
            ):
                stage._stream_artifact(
                    ["/absolute/adb"],
                    stage.PACKAGE,
                    entry,
                    "no_backup/kagemusha-candidate-artifacts-v1/incoming",
                )

    def test_binding_is_exactly_candidate_stage_and_count_bound(self) -> None:
        candidate = "1" * 64
        stage_sha = "2" * 64
        self.assertEqual(
            stage._binding_bytes(candidate, stage_sha),
            (
                f"{stage.REMOTE_BINDING_SCHEMA}\n{candidate}\n{stage_sha}\n8\n"
            ).encode("ascii"),
        )

    def test_free_space_reserves_published_copy_native_spool_and_one_gib(self) -> None:
        inventory = tuple(
            stage.ArtifactEntry(name, Path("/unused") / name, index + 1, "1" * 64)
            for index, name in enumerate(stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4)
        )
        artifact_bytes = sum(entry.size_bytes for entry in inventory)
        self.assertEqual(
            stage._required_device_free_bytes(inventory),
            artifact_bytes * 2 + 1024 * 1024 * 1024,
        )

    def test_remote_directory_guard_rejects_unsafe_existing_parents(self) -> None:
        script = stage._remote_directory_guard(
            (
                stage.REMOTE_BASE,
                stage.REMOTE_ROOT,
                f"{stage.REMOTE_ROOT}/{'1' * 64}",
            ),
            create_missing=True,
        )
        self.assertNotIn("mkdir -p", script)
        self.assertEqual(script.count("test -L"), 3)
        self.assertEqual(script.count("stat -c %u"), 3)
        self.assertEqual(script.count("stat -c %a"), 3)
        self.assertIn('= "$uid"', script)
        self.assertIn("= 700", script)

    def test_cleanup_is_non_recursive_and_names_only_stager_outputs(self) -> None:
        candidate_parent = f"{stage.REMOTE_ROOT}/{'1' * 64}"
        incoming = f"{candidate_parent}/.incoming-{'2' * 64}"
        script = stage._constrained_cleanup_script(candidate_parent, incoming)
        self.assertNotIn("rm -r", script)
        self.assertIn(f"rmdir {incoming}", script)
        for name in stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4:
            self.assertIn(f"{incoming}/{name}", script)
            self.assertIn(f"{incoming}/.{name}.tmp", script)

    def test_remote_paths_reject_absolute_traversal_and_empty_components(self) -> None:
        self.assertEqual(
            stage._safe_remote_path("no_backup/.incoming-stage/artifact.krv4"),
            "no_backup/.incoming-stage/artifact.krv4",
        )
        for path in (
            "/no_backup/artifact",
            "no_backup/../artifact",
            "no_backup/./artifact",
            "no_backup//artifact",
            "no_backup/",
        ):
            with self.subTest(path=path), self.assertRaises(stage.StageError):
                stage._safe_remote_path(path)

    def test_device_free_space_uses_posix_available_blocks(self) -> None:
        with mock.patch.object(
            stage,
            "_capture_remote",
            return_value=(
                "Filesystem 1024-blocks Used Available Capacity Mounted on\n"
                "/dev/block/dm-1 999999 1 24576 1% /data\n"
            ),
        ):
            self.assertEqual(
                stage._device_available_bytes(["/absolute/adb"], stage.PACKAGE),
                24576 * 1024,
            )

    def test_remote_measurement_requires_exact_size_and_sha256_shape(self) -> None:
        digest = hashlib.sha256(b"device").hexdigest()
        with mock.patch.object(
            stage,
            "_capture_remote",
            return_value=(
                f"123:10234:600:1\n10234\n"
                f"{digest}  private/artifact.krv4\n"
            ),
        ):
            self.assertEqual(
                stage._remote_artifact_measurement(
                    ["/absolute/adb"],
                    stage.PACKAGE,
                    "private/artifact.krv4",
                ),
                (123, digest),
            )
        with mock.patch.object(
            stage,
            "_capture_remote",
            return_value=f"123:10234:600:1\n10234\n{digest}\n",
        ):
            with self.assertRaisesRegex(stage.StageError, "SHA-256 output"):
                stage._remote_artifact_measurement(
                    ["/absolute/adb"],
                    stage.PACKAGE,
                    "private/artifact.krv4",
                )

    def test_remote_measurement_rejects_wrong_uid_mode_or_link_count(self) -> None:
        digest = hashlib.sha256(b"device").hexdigest()
        for metadata in ("123:9:600:1", "123:10:640:1", "123:10:600:2"):
            with self.subTest(metadata=metadata), mock.patch.object(
                stage,
                "_capture_remote",
                return_value=(
                    f"{metadata}\n10\n{digest}  private/artifact.krv4\n"
                ),
            ):
                with self.assertRaisesRegex(stage.StageError, "ownership, mode, or link"):
                    stage._remote_artifact_measurement(
                        ["/absolute/adb"],
                        stage.PACKAGE,
                        "private/artifact.krv4",
                    )

    def test_stage_measures_exact_set_once_immediately_before_atomic_publish(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            stage_root = root / "stage"
            (stage_root / "evidence/candidate/artifacts").mkdir(parents=True)
            fake_adb = root / "adb"
            fake_adb.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            fake_adb.chmod(0o700)
            inventory = tuple(
                stage.ArtifactEntry(
                    name,
                    stage_root / "evidence/candidate/artifacts" / name,
                    index + 1,
                    hashlib.sha256(name.encode()).hexdigest(),
                )
                for index, name in enumerate(
                    stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4
                )
            )
            candidate = "1" * 64
            stage_sha = "2" * 64
            events: list[tuple[str, str]] = []

            def run_remote(
                _adb_prefix: list[str],
                _package: str,
                script: str,
                *,
                input_bytes: bytes | None = None,
            ) -> None:
                del input_bytes
                events.append(("run", script))

            def measure(
                _adb_prefix: list[str],
                _package: str,
                remote_path: str,
            ) -> tuple[int, str]:
                events.append(("measure", remote_path))
                if remote_path.endswith(stage.REMOTE_BINDING_FILE):
                    binding = stage._binding_bytes(candidate, stage_sha)
                    return len(binding), hashlib.sha256(binding).hexdigest()
                entry = next(item for item in inventory if remote_path.endswith(item.name))
                return entry.size_bytes, entry.sha256

            def available_bytes(_adb_prefix: list[str], _package: str) -> int:
                events.append(("free-space", "checked"))
                return stage._required_device_free_bytes(inventory)

            exact_names = (
                *stage.KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
                stage.REMOTE_BINDING_FILE,
            )
            with (
                mock.patch.object(
                    stage,
                    "validate_kagemusha_candidate_stage_manifest_v1",
                    return_value={"entries": []},
                ),
                mock.patch.object(stage, "_artifact_inventory", return_value=inventory),
                mock.patch.object(
                    stage,
                    "_device_available_bytes",
                    side_effect=available_bytes,
                ),
                mock.patch.object(
                    stage,
                    "_stream_artifact",
                    side_effect=lambda _a, _p, entry, _i: events.append(
                        ("stream", entry.name)
                    ),
                ),
                mock.patch.object(stage, "_run_remote", side_effect=run_remote),
                mock.patch.object(
                    stage,
                    "_capture_remote",
                    return_value="\n".join(exact_names) + "\n",
                ),
                mock.patch.object(
                    stage,
                    "_remote_artifact_measurement",
                    side_effect=measure,
                ),
            ):
                stage.stage_artifacts(
                    adb=fake_adb,
                    serial=None,
                    stage_root=stage_root,
                    candidate_sha256=candidate,
                    stage_sha256=stage_sha,
                    source_commit="3" * 40,
                    source_tree_sha256="4" * 64,
                )

            measured = [value for kind, value in events if kind == "measure"]
            self.assertEqual(len(measured), 9)
            self.assertEqual(len(set(measured)), 9)
            free_space_index = events.index(("free-space", "checked"))
            self.assertTrue(
                all(
                    free_space_index < index
                    for index, (kind, _value) in enumerate(events)
                    if kind == "stream"
                )
            )
            publish_index = next(
                index
                for index, (kind, value) in enumerate(events)
                if kind == "run" and f"mv {stage.REMOTE_ROOT}/{candidate}/.incoming-" in value
                and f" {stage.REMOTE_ROOT}/{candidate}/{stage_sha}" in value
            )
            self.assertTrue(
                all(
                    index < publish_index
                    for index, (kind, _value) in enumerate(events)
                    if kind == "measure"
                )
            )


if __name__ == "__main__":
    unittest.main()
