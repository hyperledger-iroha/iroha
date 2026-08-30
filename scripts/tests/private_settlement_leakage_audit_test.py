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

    def test_protocol_identifiers_expand_to_norito_binary_encodings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            account = (
                "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP"
            )
            asset = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
            manifest = root / "canaries.json"
            manifest.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "canaries": [
                            {"name": "account_id", "kind": "text", "value": account},
                            {"name": "asset_id", "kind": "text", "value": asset},
                        ],
                    }
                ),
                encoding="utf-8",
            )
            canaries = MODULE.load_canaries(manifest)
            by_encoding = {(item.name, item.encoding): item.value for item in canaries}
            self.assertIn(("account_id", "canonical_account_bytes"), by_encoding)
            self.assertIn(("asset_id", "asset_address_payload"), by_encoding)
            self.assertIn(("asset_id", "asset_uuid_bytes"), by_encoding)
            artifact = root / "state.bin"
            artifact.write_bytes(
                b"prefix"
                + by_encoding[("account_id", "canonical_account_bytes")]
                + by_encoding[("asset_id", "asset_uuid_bytes")]
                + b"suffix"
            )
            hits = MODULE.scan_file(artifact, canaries, chunk_bytes=11)
            self.assertTrue(
                any(
                    hit["canary"] == "account_id"
                    and hit["encoding"] == "canonical_account_bytes"
                    for hit in hits
                )
            )
            self.assertTrue(
                any(
                    hit["canary"] == "asset_id"
                    and hit["encoding"] == "asset_uuid_bytes"
                    for hit in hits
                )
            )

    def test_protocol_identifier_canaries_fail_closed_when_not_typed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, value, message in (
                ("account_id", "alice@canary.invalid", "canonical I105"),
                ("asset_id", "usd#canary.invalid", "canonical Base58"),
                (
                    "asset_id_variant_b",
                    "111111111111111111111",
                    "canonical V1 UUIDv4 address",
                ),
            ):
                manifest = root / f"{name}.json"
                manifest.write_text(
                    json.dumps(
                        {
                            "version": 1,
                            "canaries": [
                                {"name": name, "kind": "text", "value": value}
                            ],
                        }
                    ),
                    encoding="utf-8",
                )
                with self.subTest(name=name, value=value):
                    with self.assertRaisesRegex(MODULE.AuditInputError, message):
                        MODULE.load_canaries(manifest)

    def test_release_canary_identifiers_have_protocol_native_encodings(self) -> None:
        values = (
            (
                "account_id",
                "sorauﾛ1PｺfMﾇﾘｾﾄoﾂﾊﾔH7ZdﾘhﾚmAｸdnｳu1ｱﾄ1ｺﾋuSﾑﾀﾇﾐuHEB5DP",
                "canonical_account_bytes",
            ),
            (
                "account_id_variant_b",
                "sorauﾛ1NﾑﾅpﾐTm5Yfﾕ3ｦSヰﾏBｶA5ｻﾔｽｱｼDkDｸkVZBｳﾈyｽﾜヰ9NA1NP",
                "canonical_account_bytes",
            ),
            ("asset_id", "4Zust3cNxfvUrJRuFjSMmNXho9rF", "asset_uuid_bytes"),
            (
                "asset_id_variant_b",
                "7fnqfbvxnCke21nA2Zy1C3KktDdi",
                "asset_uuid_bytes",
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "release-canaries.json"
            manifest.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "canaries": [
                            {"name": name, "kind": "text", "value": value}
                            for name, value, _encoding in values
                        ],
                    }
                ),
                encoding="utf-8",
            )
            encodings = {
                (canary.name, canary.encoding)
                for canary in MODULE.load_canaries(manifest)
            }
            for name, _value, encoding in values:
                self.assertIn((name, encoding), encodings)

    def test_strict_json_and_nested_symlinks_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            duplicate = root / "duplicate.json"
            duplicate.write_text(
                '{"version":1,"version":1,"canaries":[]}', encoding="utf-8"
            )
            with self.assertRaisesRegex(MODULE.AuditInputError, "duplicate key"):
                MODULE.load_canaries(duplicate)

            count_path = root / "counts.json"
            count_path.write_text(
                '{"version":1,"channels":{"torii_requests":NaN}}',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(MODULE.AuditInputError, "non-JSON constant"):
                MODULE.load_message_counts(count_path)

            manifest = self.write_manifest(root)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            outside = root / "outside"
            outside.mkdir()
            (outside / "capture.bin").write_bytes(b"opaque")
            (artifacts / "linked").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(MODULE.AuditInputError, "symlink"):
                MODULE.run_audit(manifest, [artifacts])


if __name__ == "__main__":
    unittest.main()
