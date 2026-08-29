"""Tests for the atomic private-settlement leakage evidence scanner."""

from __future__ import annotations

import base64
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_leakage_audit.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_leakage_audit", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class PrivateSettlementLeakageAuditTests(unittest.TestCase):
    """Exercise canary expansion, chunk boundaries, and differential checks."""

    def write_manifest(self, root: Path) -> Path:
        path = root / "canaries.json"
        path.write_text(
            json.dumps(
                {
                    "version": 1,
                    "canaries": [
                        {
                            "name": "account",
                            "kind": "text",
                            "value": "APS-ACCOUNT-CANARY",
                        },
                        {"name": "amount", "kind": "integer", "value": 987654321},
                        {
                            "name": "memo",
                            "kind": "binary_base64",
                            "value": base64.b64encode(b"APS-MEMO-CANARY").decode(
                                "ascii"
                            ),
                        },
                    ],
                }
            ),
            encoding="utf-8",
        )
        return path

    def write_message_counts(self, path: Path, *, delta: int = 0) -> Path:
        path.write_text(
            json.dumps(
                {
                    "version": 1,
                    "channels": {
                        channel: index + delta
                        for index, channel in enumerate(
                            MODULE.REQUIRED_COUNT_CHANNELS, 1
                        )
                    },
                }
            ),
            encoding="utf-8",
        )
        return path

    def test_finds_canaries_across_chunk_boundaries_without_disclosing_values(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            artifact = root / "capture.bin"
            artifact.write_bytes(b"x" * 13 + b"APS-ACCOUNT-CANARY" + b"y" * 17)
            report = MODULE.run_audit(manifest, [artifact])
            self.assertFalse(report["passed"])
            self.assertEqual(report["findings"][0]["hits"][0]["offset"], 13)
            self.assertNotIn("APS-ACCOUNT-CANARY", json.dumps(report))
            canaries = MODULE.load_canaries(manifest)
            hits = MODULE.scan_file(artifact, canaries, chunk_bytes=16)
            self.assertTrue(any(hit["canary"] == "account" for hit in hits))

    def test_clean_fixed_shape_differential_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            (left / "public.json").write_text(
                json.dumps({"roots": ["a" * 64], "status": "finalized"}),
                encoding="utf-8",
            )
            (right / "public.json").write_text(
                json.dumps({"roots": ["b" * 64], "status": "finalized"}),
                encoding="utf-8",
            )
            left_counts = self.write_message_counts(root / "left-counts.json")
            right_counts = self.write_message_counts(root / "right-counts.json")
            report = MODULE.run_audit(
                manifest,
                [left, right, left_counts, right_counts],
                differential_left=left,
                differential_right=right,
                message_counts_left=left_counts,
                message_counts_right=right_counts,
            )
            self.assertTrue(report["passed"])
            self.assertEqual(report["scanned_files"], 4)
            self.assertEqual(len(report["scanned_artifacts"]), 4)
            self.assertEqual(len(report["message_count_manifests"]), 2)

    def test_differential_rejects_shape_or_size_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            (left / "public.json").write_text(
                json.dumps({"roots": ["a" * 64]}), encoding="utf-8"
            )
            (right / "public.json").write_text(
                json.dumps({"roots": ["b" * 64, "c" * 64]}), encoding="utf-8"
            )
            left_counts = self.write_message_counts(root / "left-counts.json")
            right_counts = self.write_message_counts(root / "right-counts.json")
            report = MODULE.run_audit(
                manifest,
                [left, right, left_counts, right_counts],
                differential_left=left,
                differential_right=right,
                message_counts_left=left_counts,
                message_counts_right=right_counts,
            )
            self.assertFalse(report["passed"])
            self.assertEqual(
                report["differential"]["json_shape_mismatches"], ["public.json"]
            )

    def test_differential_rejects_message_count_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            (left / "capture.bin").write_bytes(b"a" * 32)
            (right / "capture.bin").write_bytes(b"b" * 32)
            left_counts = self.write_message_counts(root / "left-counts.json")
            right_counts = self.write_message_counts(
                root / "right-counts.json", delta=1
            )
            report = MODULE.run_audit(
                manifest,
                [left, right, left_counts, right_counts],
                differential_left=left,
                differential_right=right,
                message_counts_left=left_counts,
                message_counts_right=right_counts,
            )
            self.assertFalse(report["passed"])
            self.assertEqual(
                len(report["message_count_mismatches"]),
                len(MODULE.REQUIRED_COUNT_CHANNELS),
            )

    def test_differential_requires_count_manifests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            with self.assertRaises(MODULE.AuditInputError):
                MODULE.run_audit(
                    manifest,
                    [left, right],
                    differential_left=left,
                    differential_right=right,
                )

    def test_count_manifests_must_be_part_of_scanned_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            (left / "capture.bin").write_bytes(b"a" * 32)
            (right / "capture.bin").write_bytes(b"b" * 32)
            with self.assertRaisesRegex(
                MODULE.AuditInputError, "message-count manifests"
            ):
                MODULE.run_audit(
                    manifest,
                    [left, right],
                    differential_left=left,
                    differential_right=right,
                    message_counts_left=self.write_message_counts(
                        root / "left-counts.json"
                    ),
                    message_counts_right=self.write_message_counts(
                        root / "right-counts.json"
                    ),
                )

    def test_rejects_symlinked_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = self.write_manifest(root)
            target = root / "target.bin"
            target.write_bytes(b"clean")
            link = root / "capture.bin"
            link.symlink_to(target)
            with self.assertRaises(MODULE.AuditInputError):
                MODULE.run_audit(manifest, [link])


if __name__ == "__main__":
    unittest.main()
