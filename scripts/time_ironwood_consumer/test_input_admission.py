"""Public input-admission controls using temporary stub bytes only.

Requires Python 3.11+ standard library on macOS/Linux (validated with 3.12) and
native_c_driver.py beside this file. No BOI_ACTIVATION_* environment inputs,
generated corpus, native library or signing material are required. Tests create
and remove only temporary test-data directories; stub bytes never enter a native
decoder. Run this file directly or use the unittest discovery command in README.md.
"""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import contextlib
import io

import native_c_driver as d


class InputAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name).resolve()
        self.repo = self.base / "source"
        self.repo.mkdir()
        self.root = self.base / "public-input-unit-stubs"
        self.root.mkdir()
        self.receipt = self.base / "unit-stub-receipt.json"
        names = ["anchor-checkpoint.norito", "promoted-checkpoint.norito", "world.norito"]
        names += [f"proof-{i:04}.norito" for i in range(5)]
        refs = []
        for name in names:
            data = ("PUBLIC ADMISSION UNIT STUB: " + name).encode()
            (self.root / name).write_bytes(data)
            refs.append(dict(name=name, size=str(len(data)), sha256=d.digest(data)))
        self.manifest = dict(schema="iroha.time-ironwood-native-consumer-fixture.v1",
            fixtureKind="certified-synthetic-activation-history", patchBaseCommitIsProducerSource=False,
            patchBaseCommit="1" * 40, predecessorSelection="2" * 64, successorSelection="3" * 64,
            consumerAdmission="requires-external-producer-receipt", networkId="a" * 64, chainId="unit-stub",
            anchorHeight="2", scheduleHeight="3", activationHeight="5", bootstrapHeight="6", finalHeight="6",
            anchorContextId="b" * 64, finalContextId="c" * 64,
            checkpoint=refs[0], promotedCheckpoint=refs[1], world=refs[2], proofs=refs[3:])
        self.producer = dict(schema="boi.time-ironwood-fixture-producer.v1",
            source=dict(commit="d" * 40, tree="e" * 40, signedHeadVerified=True, cleanBefore=True, cleanAfter=True),
            testExecutable=dict(sha256="f" * 64, size=1), buildReceiptSha256="a" * 64,
            run=dict(testName=d.TEST_NAME, exitCode=0, passed=1, failed=0, ignored=0),
            files=[dict(name=r["name"], size=int(r["size"]), sha256=r["sha256"]) for r in refs])
        self.publish()

    def publish(self):
        raw = json.dumps(self.manifest).encode()
        (self.root / "manifest.json").write_bytes(raw)
        self.manifest_pin = d.digest(raw)
        self.producer["fixtureManifestSha256"] = self.manifest_pin
        raw = json.dumps(self.producer).encode()
        self.receipt.write_bytes(raw)
        self.receipt_pin = d.digest(raw)

    def load(self):
        return d.load_fixture(self.repo, str(self.root), self.manifest_pin, str(self.receipt), self.receipt_pin)

    def test_admits_exact_public_stub_inputs_without_native_execution(self):
        self.assertEqual(len(self.load()[2]), 8)

    def test_rejects_changed_manifest_pin(self):
        self.manifest_pin = "0" * 63 + "1"
        with self.assertRaisesRegex(ValueError, "manifest pin"):
            self.load()

    def test_rejects_changed_receipt_pin(self):
        self.receipt_pin = "0" * 63 + "1"
        with self.assertRaisesRegex(ValueError, "receipt pin"):
            self.load()

    def test_rejects_changed_original(self):
        (self.root / "proof-0001.norito").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "identity mismatch"):
            self.load()

    def test_rejects_receipt_file_substitution(self):
        self.producer["files"][0]["sha256"] = "b" * 64
        self.publish()
        with self.assertRaisesRegex(ValueError, "receipt originals"):
            self.load()

    def test_rejects_filename_traversal(self):
        self.manifest["checkpoint"]["name"] = "../outside"
        self.publish()
        with self.assertRaisesRegex(ValueError, "original name"):
            self.load()

    def test_rejects_symlink_original(self):
        path = self.root / "proof-0001.norito"
        moved = self.base / "moved-original"
        path.rename(moved)
        path.symlink_to(moved)
        with self.assertRaises(OSError):
            self.load()

    def test_rejects_generated_files_inside_source(self):
        moved = self.repo / "generated"
        self.root.rename(moved)
        self.root = moved
        with self.assertRaisesRegex(ValueError, "external to source"):
            self.load()

    def test_rejects_extra_original(self):
        (self.root / "unreviewed.norito").write_bytes(b"extra")
        with self.assertRaisesRegex(ValueError, "unexpected fixture"):
            self.load()

    def test_rejects_group_writable_fixture_root(self):
        self.root.chmod(0o770)
        with self.assertRaisesRegex(ValueError, "owner-controlled"):
            self.load()

    def test_rejects_missing_clean_after_gate(self):
        self.producer["source"]["cleanAfter"] = False
        self.publish()
        with self.assertRaisesRegex(ValueError, "signed clean source"):
            self.load()

    def test_rejects_skipped_exporter_and_bool_counter(self):
        for value in (True, 0):
            with self.subTest(value=value):
                self.producer["run"]["passed"] = value
                self.publish()
                with self.assertRaisesRegex(ValueError, "exact exporter run"):
                    self.load()

    def test_rejects_duplicate_json_key(self):
        with self.assertRaisesRegex(ValueError, "duplicate JSON"):
            d.json_original(b'{"source":1,"source":2}')

    def test_rejects_noncanonical_size(self):
        self.manifest["checkpoint"]["size"] = "01"
        self.publish()
        with self.assertRaisesRegex(ValueError, "canonical size"):
            self.load()

    def test_rejects_incomplete_history(self):
        self.manifest["proofs"].pop()
        self.publish()
        with self.assertRaisesRegex(ValueError, "complete proof history"):
            self.load()

    def test_refuses_aggregate_excess_before_original_reads(self):
        self.manifest["proofs"][0]["size"] = str(16 * d.MIB)
        self.publish()
        # The file is deliberately absent: a byte read would fail with FileNotFoundError.
        (self.root / "anchor-checkpoint.norito").unlink()
        with self.assertRaisesRegex(ValueError, "proof page byte bound"):
            self.load()

    def test_refuses_extra_receipt_fields(self):
        self.producer["source"]["unreviewed"] = True
        self.publish()
        with self.assertRaisesRegex(ValueError, "closed JSON"):
            self.load()

    def test_admit_only_never_loads_native_library(self):
        output = self.base / "admission-only.json"
        args = ["native_c_driver.py", "--admit-only", "--source-root", str(self.repo),
                "--source-commit", "d" * 40, "--source-tree", "e" * 40, "--output", str(output)]
        environment = {"BOI_ACTIVATION_FIXTURE_DIR": str(self.root),
                       "BOI_ACTIVATION_MANIFEST_SHA256": self.manifest_pin,
                       "BOI_ACTIVATION_PRODUCER_RECEIPT": str(self.receipt),
                       "BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256": self.receipt_pin}
        with patch("sys.argv", args), patch.dict("os.environ", environment), \
             patch.object(d.C, "CDLL", side_effect=AssertionError("native must not load")) as load, \
             contextlib.redirect_stdout(io.StringIO()):
            d.main()
            load.assert_not_called()
        report = json.loads(output.read_text())
        self.assertEqual(report["status"], "INPUTS_ADMITTED_ONLY")
        self.assertIs(report["nativeLoaded"], False)
        self.assertEqual(report["nativeChecks"], 0)


if __name__ == "__main__":
    unittest.main()
