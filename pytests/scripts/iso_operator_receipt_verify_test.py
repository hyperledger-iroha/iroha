import importlib.util
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path

from pytests.scripts import iso_audit_notary_adapter_test as audit_test
from pytests.scripts import iso_rail_gateway_adapter_test as rail_test


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_operator_receipt_verify.py"
SPEC = importlib.util.spec_from_file_location("iso_operator_receipt_verify", SCRIPT_PATH)
VERIFIER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = VERIFIER
SPEC.loader.exec_module(VERIFIER)


def run_verify(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = VERIFIER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def rewrite_receipt(path, mutate):
    receipt = json.loads(path.read_text(encoding="utf-8"))
    mutate(receipt)
    receipt.pop(VERIFIER.RECEIPT_DIGEST_FIELD, None)
    receipt[VERIFIER.RECEIPT_DIGEST_FIELD] = VERIFIER.sha256_hex(
        VERIFIER._canonical_json_bytes(receipt)
    )
    path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return receipt


class IsoOperatorReceiptVerifyTest(unittest.TestCase):
    def test_verifies_successful_notary_and_rail_receipts_with_source_files(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )

            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )

            rc, stdout, stderr = run_verify(
                [
                    "--receipt-dir",
                    str(export_dir / "receipts"),
                    "--receipt-dir",
                    str(inbox / "receipts"),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["verified_receipts"], 2)
            self.assertEqual(
                summary["receipt_kind"],
                ["iso-audit-notary", "iso-rail-gateway"],
            )
            self.assertFalse(summary["allow_failed"])
            self.assertTrue(summary["allow_insecure_http"])
            self.assertFalse(summary["allow_legacy_colr007"])
            self.assertTrue(summary["require_source_files"])
            self.assertEqual(len(summary["receipts"]), 2)
            for receipt in summary["receipts"]:
                self.assertIn(receipt["receipt_kind"], VERIFIER.SUPPORTED_KINDS)
                self.assertTrue(VERIFIER._is_lower_hex_sha256(receipt["receipt_sha256"]))
            body = dict(summary)
            digest = body.pop("summary_sha256")
            self.assertEqual(
                digest,
                VERIFIER.sha256_hex(VERIFIER._canonical_summary_json_bytes(body)),
            )

    def test_tampered_receipt_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            raw = json.loads(receipt.read_text(encoding="utf-8"))
            raw["status_code"] = 500
            receipt.write_text(json.dumps(raw, indent=2), encoding="utf-8")

            rc, _stdout, _stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)

    def test_failed_receipt_requires_explicit_allow_failed(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server(status=409, body=b"duplicate") as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    1,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            self.assertEqual(
                run_verify(["--receipt", str(receipt), "--allow-insecure-http"])[0],
                2,
            )
            self.assertEqual(
                run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--allow-failed",
                    ]
                )[0],
                0,
            )

    def test_source_payload_mismatch_is_rejected_when_required(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            xml_path.write_bytes(rail_test.SAMPLE_XML + b" changed")

            rc, _stdout, _stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--require-source-files",
                ]
            )

            self.assertEqual(rc, 2)

    def test_legacy_colr007_rail_receipts_require_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(
                inbox,
                message_type="colr.007",
                profile="securities-csd",
                payload=b"<Document><CollSbstitnConf/></Document>",
            )
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            "--allow-legacy-colr007",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )
            self.assertEqual(rc, 2)
            self.assertIn("legacy rail message_type", stderr)

            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                    "--allow-legacy-colr007",
                    "--require-source-files",
                ]
            )
            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertTrue(summary["allow_legacy_colr007"])
            self.assertEqual(summary["receipts"][0]["message_type"], "colr.007")

    def test_secret_material_in_receipt_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            receipt = next((inbox / "receipts").glob("*.receipt.json"))
            rewrite_receipt(receipt, lambda body: body.update({"authorization": "Bearer secret"}))

            rc, _stdout, _stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)

    def test_smuggled_receipt_endpoint_urls_are_rejected(self):
        cases = [
            ("notary", "endpoint", "https://user:pass@notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example/anchor;debug", False),
            ("notary", "endpoint", "https://notary.example/anchor?debug=true", False),
            ("notary", "endpoint", "https://notary.example/anchor#fragment", False),
            ("notary", "endpoint", "https:///anchor", False),
            ("notary", "endpoint", "https://[::1", False),
            ("notary", "endpoint", "https://notary.example/anc\nhor", False),
            ("rail", "endpoint_url", "http://user:pass@127.0.0.1/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022?debug=true", True),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(export_dir)
            with audit_test.capture_server() as (endpoint, _requests):
                self.assertEqual(
                    audit_test.run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            notary_receipt = next((export_dir / "receipts").glob("*.receipt.json"))

            inbox = root / "inbox"
            inbox.mkdir()
            rail_test.write_message(inbox)
            with rail_test.capture_server() as (base_url, _requests):
                self.assertEqual(
                    rail_test.run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                        ]
                    )[0],
                    0,
                )
            rail_receipt = next((inbox / "receipts").glob("*.receipt.json"))

            for kind, field, url, allow_insecure in cases:
                with self.subTest(kind=kind, url=url, allow_insecure=allow_insecure):
                    receipt = notary_receipt if kind == "notary" else rail_receipt
                    rewrite_receipt(receipt, lambda body, field=field, url=url: body.update({field: url}))
                    argv = ["--receipt", str(receipt)]
                    if allow_insecure:
                        argv.append("--allow-insecure-http")
                    rc, _stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("error:", stderr)


if __name__ == "__main__":
    unittest.main()
