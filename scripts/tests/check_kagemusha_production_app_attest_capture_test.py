"""Tests for fail-closed publication of validated App Attest capture outputs."""

from __future__ import annotations

from contextlib import redirect_stderr
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]
if os.fspath(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, os.fspath(SCRIPT_DIRECTORY))

import check_kagemusha_production_app_attest_capture as checker  # noqa: E402


class ProductionAppAttestCaptureCheckerTest(unittest.TestCase):
    def _argv(self, platform: str, summary: str) -> list[str]:
        return [
            "check_kagemusha_production_app_attest_capture.py",
            "--capture",
            "/unused/capture.json",
            "--request",
            "/unused/request.json",
            "--production-policy",
            "/unused/policy.json",
            "--platform-evidence-output",
            platform,
            "--summary-output",
            summary,
        ]

    def test_invalid_second_output_is_rejected_before_first_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], {"kind": "platform"}, {"kind": "summary"}),
                ),
                mock.patch.object(
                    checker.candidate_evidence, "write_new_private_json"
                ) as writer,
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), "relative-summary.json"),
                ),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(checker.main(), 1)
            writer.assert_not_called()
            self.assertFalse(platform.exists())

    def test_duplicate_outputs_are_rejected_before_publication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "capture-output.json"
            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], {"kind": "platform"}, {"kind": "summary"}),
                ),
                mock.patch.object(
                    checker.candidate_evidence, "write_new_private_json"
                ) as writer,
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(output), os.fspath(output)),
                ),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(checker.main(), 1)
            writer.assert_not_called()
            self.assertFalse(output.exists())

    def test_distinct_valid_outputs_are_published_after_joint_preflight(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            summary = root / "summary.json"
            platform_value = {"kind": "platform"}
            summary_value = {"kind": "summary"}
            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], platform_value, summary_value),
                ),
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), os.fspath(summary)),
                ),
            ):
                self.assertEqual(checker.main(), 0)
            self.assertEqual(
                platform.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes(platform_value),
            )
            self.assertEqual(
                summary.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes(summary_value),
            )

    def test_second_output_race_is_uncertain_and_preserves_every_final_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            summary = root / "summary.json"
            raced_payload = b"raced output must survive\n"
            real_link = os.link

            def race_then_link(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                raced = Path(target)
                if raced == summary.resolve():
                    raced.write_bytes(raced_payload)
                    raced.chmod(0o600)
                real_link(source, target, follow_symlinks=follow_symlinks)

            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], {"kind": "platform"}, {"kind": "summary"}),
                ),
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "link",
                    side_effect=race_then_link,
                ),
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), os.fspath(summary)),
                ),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(checker.main(), 75)
            self.assertEqual(
                platform.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes({"kind": "platform"}),
            )
            self.assertEqual(summary.read_bytes(), raced_payload)
            self.assertEqual(set(root.iterdir()), {platform, summary})

    def test_post_link_failure_is_uncertain_and_preserves_final_names(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            summary = root / "summary.json"
            real_fsync = os.fsync
            fsync_calls = 0

            def fail_second_output_directory_fsync(descriptor: int) -> None:
                nonlocal fsync_calls
                fsync_calls += 1
                if fsync_calls == 5:
                    raise OSError("injected post-link fsync failure")
                real_fsync(descriptor)

            stderr = io.StringIO()
            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], {"kind": "platform"}, {"kind": "summary"}),
                ),
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "fsync",
                    side_effect=fail_second_output_directory_fsync,
                ),
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), os.fspath(summary)),
                ),
                redirect_stderr(stderr),
            ):
                self.assertEqual(checker.main(), 75)
            self.assertIn("without a confirmed durable commit", stderr.getvalue())
            self.assertEqual(
                platform.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes({"kind": "platform"}),
            )
            self.assertEqual(
                summary.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes({"kind": "summary"}),
            )
            self.assertEqual(set(root.iterdir()), {platform, summary})

    def test_first_output_mutation_during_second_publish_is_uncertain(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            summary = root / "summary.json"
            platform_value = {"kind": "platform"}
            summary_value = {"kind": "summary"}
            mutated_value = {"kind": "tampered"}
            mutated_payload = checker.candidate_evidence.canonical_json_bytes(
                mutated_value
            )
            real_link = os.link

            def mutate_first_then_link_second(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                if Path(target) == summary.resolve():
                    platform.write_bytes(mutated_payload)
                real_link(source, target, follow_symlinks=follow_symlinks)

            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], platform_value, summary_value),
                ),
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "link",
                    side_effect=mutate_first_then_link_second,
                ),
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), os.fspath(summary)),
                ),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(checker.main(), 75)
            self.assertEqual(platform.read_bytes(), mutated_payload)
            self.assertEqual(
                summary.read_bytes(),
                checker.candidate_evidence.canonical_json_bytes(summary_value),
            )
            self.assertEqual(set(root.iterdir()), {platform, summary})

    def test_writer_close_failure_still_removes_temporary_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "output.json"
            real_close = os.close
            failed = False

            def close_then_fail_once(descriptor: int) -> None:
                nonlocal failed
                real_close(descriptor)
                if not failed:
                    failed = True
                    raise OSError("injected close failure")

            with (
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "close",
                    side_effect=close_then_fail_once,
                ),
                self.assertRaisesRegex(
                    checker.candidate_evidence.EvidenceError,
                    "could not be published",
                ),
            ):
                checker.candidate_evidence.write_new_private_json(
                    output, {"kind": "output"}
                )
            self.assertEqual(list(root.iterdir()), [])

    def test_writer_rejects_same_payload_inode_substitution_as_uncertain(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            output = root / "output.json"
            displaced = root / "displaced-output.json"
            value = {"kind": "output"}
            payload = checker.candidate_evidence.canonical_json_bytes(value)
            real_link = os.link

            def replace_linked_inode(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                real_link(source, target, follow_symlinks=follow_symlinks)
                Path(target).rename(displaced)
                Path(target).write_bytes(payload)
                Path(target).chmod(0o600)

            with (
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "link",
                    side_effect=replace_linked_inode,
                ),
                self.assertRaises(
                    checker.candidate_evidence.NewPrivateJsonPublicationUncertain
                ),
            ):
                checker.candidate_evidence.write_new_private_json(output, value)
            self.assertEqual(output.read_bytes(), payload)
            self.assertEqual(displaced.read_bytes(), payload)
            self.assertNotEqual(output.stat().st_ino, displaced.stat().st_ino)

    def test_changed_first_output_reports_uncertain_without_deleting_replacement(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root.chmod(0o700)
            platform = root / "platform.json"
            displaced = root / "displaced-platform.json"
            summary = root / "summary.json"
            replacement_payload = b"replacement platform output\n"
            summary_racer_payload = b"raced summary output\n"
            real_link = os.link

            def change_first_then_race_second(
                source: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                target: str | bytes | os.PathLike[str] | os.PathLike[bytes],
                *,
                follow_symlinks: bool = True,
            ) -> None:
                target_path = Path(target)
                if target_path == summary.resolve():
                    platform.rename(displaced)
                    platform.write_bytes(replacement_payload)
                    platform.chmod(0o600)
                    summary.write_bytes(summary_racer_payload)
                    summary.chmod(0o600)
                real_link(source, target, follow_symlinks=follow_symlinks)

            with (
                mock.patch.object(
                    checker.capture_evidence,
                    "validate_capture",
                    return_value=([], {"kind": "platform"}, {"kind": "summary"}),
                ),
                mock.patch.object(
                    checker.candidate_evidence.os,
                    "link",
                    side_effect=change_first_then_race_second,
                ),
                mock.patch.object(
                    sys,
                    "argv",
                    self._argv(os.fspath(platform), os.fspath(summary)),
                ),
                redirect_stderr(io.StringIO()),
            ):
                self.assertEqual(checker.main(), 75)
            self.assertEqual(platform.read_bytes(), replacement_payload)
            self.assertEqual(summary.read_bytes(), summary_racer_payload)
            self.assertTrue(displaced.is_file())


if __name__ == "__main__":
    unittest.main()
