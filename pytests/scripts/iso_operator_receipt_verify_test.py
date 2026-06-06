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


@contextlib.contextmanager
def patched_verifier_constant(name, value):
    original = getattr(VERIFIER, name)
    setattr(VERIFIER, name, value)
    try:
        yield
    finally:
        setattr(VERIFIER, name, original)


def oversized_json_bytes(limit):
    return b'{"padding":"' + (b"a" * (limit + 1)) + b'"}\n'


class IsoOperatorReceiptVerifyTest(unittest.TestCase):
    def test_verifies_successful_notary_and_rail_receipts_with_source_files(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
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

    def test_duplicate_receipt_paths_and_digests_are_rejected(self):
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

            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--receipt",
                    str(receipt),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("duplicates receipt[0]", stderr)

            copied = receipt.with_name("copied.receipt.json")
            copied.write_bytes(receipt.read_bytes())
            rc, _stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(receipt),
                    "--receipt",
                    str(copied),
                    "--allow-insecure-http",
                ]
            )
            self.assertEqual(rc, 2)
            self.assertIn("receipt_sha256 duplicates", stderr)

    def test_symlinked_receipt_file_ancestor_is_rejected_before_read(self):
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
            target_dir = inbox / "receipt-target"
            target_dir.mkdir()
            target = target_dir / receipt.name
            target.write_bytes(receipt.read_bytes())
            ancestor = inbox / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, stdout, stderr = run_verify(
                [
                    "--receipt",
                    str(ancestor / target.name),
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_non_regular_receipt_dirs_are_rejected_before_discovery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "receipts"
            receipt_dir.mkdir()
            receipt_file = root / "receipt-dir-as-file"
            receipt_file.write_text("not a directory\n", encoding="utf-8")
            receipt_link = root / "receipt-dir-link"
            try:
                receipt_link.symlink_to(receipt_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            cases = [
                (receipt_link, "must not be a symlink"),
                (receipt_file, "is not a directory"),
            ]
            for path, message in cases:
                with self.subTest(path=path.name):
                    rc, _stdout, stderr = run_verify(
                        ["--receipt-dir", str(path), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_symlinked_receipt_dir_ancestor_is_rejected_before_discovery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "receipt-target"
            target_dir.mkdir()
            ancestor = root / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            receipt_dir = ancestor / "receipts"

            rc, stdout, stderr = run_verify(
                ["--receipt-dir", str(receipt_dir), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("must not be a symlink", stderr)

    def test_receipt_cli_paths_reject_raw_smuggling_before_discovery(self):
        cases = (
            ("receipt semicolon", "--receipt", "bad;debug.receipt.json", "semicolon path"),
            ("receipt whitespace", "--receipt", "bad receipt.json", "whitespace"),
            (
                "receipt leading dash",
                "--receipt",
                "-bad.receipt.json",
                "leading-dash path segments",
            ),
            (
                "receipt segment dash",
                "--receipt",
                "nested/-bad.receipt.json",
                "leading-dash path segments",
            ),
            (
                "receipt dot",
                "--receipt",
                lambda root: f"{root}/nested/./bad.receipt.json",
                "dot or parent",
            ),
            (
                "receipt empty",
                "--receipt",
                lambda root: f"{root}//bad.receipt.json",
                "empty path",
            ),
            ("dir semicolon", "--receipt-dir", "receipts;debug", "semicolon path"),
            ("dir whitespace", "--receipt-dir", "receipt dir", "whitespace"),
            (
                "dir segment dash",
                "--receipt-dir",
                "nested/-receipts",
                "leading-dash path segments",
            ),
            (
                "dir parent",
                "--receipt-dir",
                "nested/../receipts",
                "dot or parent",
            ),
            (
                "dir empty equals",
                "--receipt-dir",
                lambda root: f"{root}//receipts",
                "empty path",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)
                    argv = (
                        [f"{flag}={value}", "--allow-insecure-http"]
                        if "equals" in name
                        else [flag, value, "--allow-insecure-http"]
                    )

                    rc, stdout, stderr = run_verify(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_duplicate_receipt_json_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":1,"version":1}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("duplicate key", stderr)

    def test_non_finite_receipt_json_numbers_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":NaN}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_receipt_json_surrogate_strings_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_text('{"version":"\\ud800"}\n', encoding="utf-8")

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_oversized_receipt_json_is_rejected_before_parsing(self):
        with tempfile.TemporaryDirectory() as raw_root:
            receipt = Path(raw_root) / "receipt.json"
            receipt.write_bytes(oversized_json_bytes(64))

            with patched_verifier_constant("MAX_RECEIPT_JSON_BYTES", 64):
                rc, _stdout, stderr = run_verify(
                    ["--receipt", str(receipt), "--allow-insecure-http"]
                )

            self.assertEqual(rc, 2)
            self.assertIn("exceeds 64 byte JSON limit", stderr)

    def test_unknown_receipt_fields_are_rejected_even_with_valid_digest(self):
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
            rewrite_receipt(
                receipt,
                lambda body: body.update({"operator_comment": "looks fine"}),
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("contains unknown keys: operator_comment", stderr)

    def test_endpoint_digest_mismatch_is_rejected(self):
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
            rewrite_receipt(
                receipt,
                lambda body: body.update({"endpoint_sha256": "0" * 64}),
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("endpoint_sha256 does not match endpoint", stderr)

    def test_rail_receipt_required_strings_must_not_require_trimming(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
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
            original_receipt = receipt.read_bytes()
            cases = [
                (
                    "message type whitespace",
                    lambda body: body.update({"message_type": " pacs.002"}),
                    "message_type must not have surrounding whitespace",
                ),
                (
                    "message type control",
                    lambda body: body.update({"message_type": "pacs\n002"}),
                    "message_type must not contain control characters",
                ),
                (
                    "xml path whitespace",
                    lambda body: body.update({"xml_path": str(xml_path) + " "}),
                    "xml_path must not have surrounding whitespace",
                ),
                (
                    "xml path control",
                    lambda body: body.update({"xml_path": str(xml_path) + "\n"}),
                    "xml_path must not contain control characters",
                ),
                (
                    "xml path embedded whitespace",
                    lambda body: body.update(
                        {
                            "xml_path": str(xml_path).replace(
                                "rail-status.xml",
                                "rail status.xml",
                            )
                        }
                    ),
                    "xml_path must not contain whitespace",
                ),
                (
                    "xml path dash",
                    lambda body: body.update({"xml_path": "--rail-status.xml"}),
                    "xml_path must not start with a dash",
                ),
                (
                    "xml path segment dash",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}/--{xml_path.name}"}
                    ),
                    "xml_path must not contain leading-dash path segments",
                ),
                (
                    "xml path non xml",
                    lambda body: body.update(
                        {
                            "xml_path": str(xml_path.with_suffix(".txt")),
                            "sidecar_path": str(
                                xml_path.with_suffix(".txt").with_suffix(".txt.json")
                            ),
                        }
                    ),
                    "xml_path must point to a .xml file",
                ),
                (
                    "xml path backslash",
                    lambda body: body.update(
                        {"xml_path": str(xml_path).replace("/", "\\", 1)}
                    ),
                    "xml_path must use forward slashes",
                ),
                (
                    "xml path semicolon",
                    lambda body: body.update({"xml_path": str(xml_path) + ";debug"}),
                    "xml_path must not contain semicolon path parameters",
                ),
                (
                    "xml path empty segment",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}//{xml_path.name}"}
                    ),
                    "xml_path must not contain empty path segments",
                ),
                (
                    "xml path parent segment",
                    lambda body: body.update(
                        {"xml_path": f"{xml_path.parent}/../{xml_path.name}"}
                    ),
                    "xml_path must not contain dot or parent segments",
                ),
                (
                    "sidecar path whitespace",
                    lambda body: body.update({"sidecar_path": " " + str(sidecar_path)}),
                    "sidecar_path must not have surrounding whitespace",
                ),
                (
                    "sidecar path control",
                    lambda body: body.update({"sidecar_path": str(sidecar_path) + "\n"}),
                    "sidecar_path must not contain control characters",
                ),
                (
                    "sidecar path embedded whitespace",
                    lambda body: body.update(
                        {
                            "sidecar_path": str(sidecar_path).replace(
                                "rail-status.xml.json",
                                "rail status.xml.json",
                            )
                        }
                    ),
                    "sidecar_path must not contain whitespace",
                ),
                (
                    "sidecar path dash",
                    lambda body: body.update({"sidecar_path": "--rail-status.xml.json"}),
                    "sidecar_path must not start with a dash",
                ),
                (
                    "sidecar path segment dash",
                    lambda body: body.update(
                        {"sidecar_path": f"{sidecar_path.parent}/--{sidecar_path.name}"}
                    ),
                    "sidecar_path must not contain leading-dash path segments",
                ),
                (
                    "sidecar path backslash",
                    lambda body: body.update(
                        {"sidecar_path": str(sidecar_path).replace("/", "\\", 1)}
                    ),
                    "sidecar_path must use forward slashes",
                ),
                (
                    "sidecar path semicolon",
                    lambda body: body.update(
                        {"sidecar_path": str(sidecar_path) + ";debug"}
                    ),
                    "sidecar_path must not contain semicolon path parameters",
                ),
                (
                    "sidecar path empty segment",
                    lambda body: body.update(
                        {
                            "sidecar_path": (
                                f"{sidecar_path.parent}//{sidecar_path.name}"
                            )
                        }
                    ),
                    "sidecar_path must not contain empty path segments",
                ),
                (
                    "sidecar path dot segment",
                    lambda body: body.update(
                        {"sidecar_path": f"{sidecar_path.parent}/./{sidecar_path.name}"}
                    ),
                    "sidecar_path must not contain dot or parent segments",
                ),
            ]

            for label, mutate, expected in cases:
                with self.subTest(label=label):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_status_timestamp_and_response_metadata_are_consistent(self):
        cases = [
            (
                "ok_status_mismatch",
                lambda body: body.update({"ok": True, "status_code": 500}),
                "ok does not match status_code",
            ),
            (
                "success_error",
                lambda body: body.update({"error": "HTTP 202"}),
                "successful receipt must not record error",
            ),
            (
                "success_error_whitespace",
                lambda body: body.update({"error": " HTTP 202"}),
                "error must not have surrounding whitespace",
            ),
            (
                "success_error_control",
                lambda body: body.update({"error": "HTTP\n202"}),
                "error must not contain control characters",
            ),
            (
                "preview_without_digest",
                lambda body: body.update(
                    {"response_body_sha256": None, "response_body_preview": "accepted"}
                ),
                "response_body_preview requires response_body_sha256",
            ),
            (
                "bad_response_digest",
                lambda body: body.update({"response_body_sha256": "not-a-digest"}),
                "invalid response_body_sha256",
            ),
            (
                "oversized_preview",
                lambda body: body.update({"response_body_preview": "x" * 4097}),
                "response_body_preview exceeds 4096 characters",
            ),
            (
                "bearer_preview",
                lambda body: body.update(
                    {"response_body_preview": "Authorization: Bearer abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "token_preview",
                lambda body: body.update(
                    {"response_body_preview": "upstream token=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "private_key_preview",
                lambda body: body.update(
                    {"response_body_preview": "private_key=abc"}
                ),
                "response_body_preview contains secret-looking material",
            ),
            (
                "secret_error",
                lambda body: body.update(
                    {"error": "upstream token=abc"}
                ),
                "error contains secret-looking material",
            ),
            (
                "naive_timestamp",
                lambda body: body.update({"submitted_at": "2026-06-05T00:00:00"}),
                "submitted_at must include a timezone offset",
            ),
            (
                "timestamp_whitespace",
                lambda body: body.update({"submitted_at": body["submitted_at"] + " "}),
                "submitted_at must not have surrounding whitespace",
            ),
            (
                "timestamp_control",
                lambda body: body.update({"submitted_at": body["submitted_at"] + "\n"}),
                "submitted_at must not contain control characters",
            ),
        ]
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
            original = receipt.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    receipt.write_bytes(original)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_redacted_failed_response_preview_is_verifier_acceptable(self):
        body = b'{"error":"token=rail-secret"}'
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            rail_test.write_message(inbox)
            with rail_test.capture_server(status=500, body=body) as (base_url, _requests):
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
            receipt_body = json.loads(receipt.read_text(encoding="utf-8"))
            self.assertEqual(
                receipt_body["response_body_preview"],
                rail_test.ADAPTER.REDACTED_RESPONSE_PREVIEW,
            )

            rc, _stdout, stderr = run_verify(
                ["--receipt", str(receipt), "--allow-insecure-http", "--allow-failed"]
            )

            self.assertEqual(rc, 0, stderr)

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

    def test_oversized_rail_source_xml_is_rejected_when_required(self):
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

            with patched_verifier_constant(
                "MAX_RAIL_XML_BYTES",
                len(rail_test.SAMPLE_XML) - 1,
            ):
                rc, _stdout, stderr = run_verify(
                    [
                        "--receipt",
                        str(receipt),
                        "--allow-insecure-http",
                        "--require-source-files",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertIn("byte payload limit", stderr)

    def test_source_sidecar_mismatches_are_rejected_when_required(self):
        def replace_with_symlink(path, target):
            if path.exists() or path.is_symlink():
                path.unlink()
            try:
                path.symlink_to(target)
            except OSError as error:
                raise unittest.SkipTest(f"symlink creation unavailable: {error}") from error

        def symlinked_xml(_receipt, sidecar_path):
            xml_path = sidecar_path.with_suffix("")
            copy = xml_path.with_name("rail-status.copy.xml")
            copy.write_bytes(xml_path.read_bytes())
            replace_with_symlink(xml_path, copy)

        def symlinked_sidecar(_receipt, sidecar_path):
            copy = sidecar_path.with_name("rail-status.copy.xml.json")
            copy.write_bytes(sidecar_path.read_bytes())
            replace_with_symlink(sidecar_path, copy)

        cases = [
            (
                "missing_sidecar",
                lambda receipt, sidecar_path: sidecar_path.unlink(),
                "references missing sidecar_path",
            ),
            (
                "message_type_mismatch",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "message_type": "pacs.008",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "message_type does not match source sidecar",
            ),
            (
                "unknown_sidecar_key",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "operator_note": "accepted",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "contains unknown keys: operator_note",
            ),
            (
                "profile_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": " swift-cbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not have surrounding whitespace",
            ),
            (
                "profile_control",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift\ncbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not contain control characters",
            ),
            (
                "profile_embedded_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift cbpr-plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must not contain whitespace",
            ),
            (
                "profile_uppercase",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "Swift-CBPR-Plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "profile_underscore",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "profile": "swift_cbpr_plus",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "rail_message_id_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail-drop-1 ",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not have surrounding whitespace",
            ),
            (
                "rail_message_id_control",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail\n1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not contain control characters",
            ),
            (
                "rail_message_id_embedded_whitespace",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail drop 1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must not contain whitespace",
            ),
            (
                "rail_message_id_unicode",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail-drop-\U0001f69a",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail_message_id_path_separator",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "rail/drop/1",
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail_message_id_oversized",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "a"
                            * (VERIFIER.MAX_RAIL_MESSAGE_ID_CHARS + 1),
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "rail_message_id must be at most",
            ),
            (
                "oversized_sidecar",
                lambda receipt, sidecar_path: sidecar_path.write_text(
                    json.dumps(
                        {
                            **json.loads(sidecar_path.read_text(encoding="utf-8")),
                            "rail_message_id": "a"
                            * VERIFIER.MAX_RAIL_SIDECAR_JSON_BYTES,
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                ),
                "exceeds",
            ),
            (
                "symlinked_xml",
                symlinked_xml,
                "must not be a symlink",
            ),
            (
                "symlinked_sidecar",
                symlinked_sidecar,
                "must not be a symlink",
            ),
            (
                "swapped_sidecar_path",
                lambda receipt, sidecar_path: (
                    sidecar_path.with_name("copied.xml.json").write_bytes(
                        sidecar_path.read_bytes()
                    ),
                    rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "sidecar_path": str(
                                    sidecar_path.with_name("copied.xml.json")
                                )
                            }
                        ),
                    ),
                ),
                "sidecar_path must match xml_path sidecar",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = rail_test.write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
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
            original_receipt = receipt.read_bytes()
            original_sidecar = sidecar_path.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    copied_sidecar = sidecar_path.with_name("copied.xml.json")
                    if copied_sidecar.exists():
                        copied_sidecar.unlink()
                    if xml_path.is_symlink():
                        xml_path.unlink()
                    if sidecar_path.is_symlink():
                        sidecar_path.unlink()
                    receipt.write_bytes(original_receipt)
                    xml_path.write_bytes(rail_test.SAMPLE_XML)
                    sidecar_path.write_bytes(original_sidecar)
                    mutate(receipt, sidecar_path)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_rail_receipt_metadata_strings_must_not_require_trimming(self):
        cases = [
            (
                "receipt profile whitespace",
                lambda body: body.update({"profile": " swift-cbpr-plus"}),
                "profile must not have surrounding whitespace",
            ),
            (
                "receipt profile control",
                lambda body: body.update({"profile": "swift\ncbpr-plus"}),
                "profile must not contain control characters",
            ),
            (
                "receipt profile embedded whitespace",
                lambda body: body.update({"profile": "swift cbpr-plus"}),
                "profile must not contain whitespace",
            ),
            (
                "receipt profile uppercase",
                lambda body: body.update({"profile": "Swift-CBPR-Plus"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt profile underscore",
                lambda body: body.update({"profile": "swift_cbpr_plus"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt profile trailing hyphen",
                lambda body: body.update({"profile": "swift-cbpr-plus-"}),
                "profile must be a canonical lowercase profile id",
            ),
            (
                "receipt rail message whitespace",
                lambda body: body.update({"rail_message_id": "rail-drop-1 "}),
                "rail_message_id must not have surrounding whitespace",
            ),
            (
                "receipt rail message control",
                lambda body: body.update({"rail_message_id": "rail\n1"}),
                "rail_message_id must not contain control characters",
            ),
            (
                "receipt rail message embedded whitespace",
                lambda body: body.update({"rail_message_id": "rail drop 1"}),
                "rail_message_id must not contain whitespace",
            ),
            (
                "receipt rail message unicode",
                lambda body: body.update({"rail_message_id": "rail-drop-\U0001f69a"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message path separator",
                lambda body: body.update({"rail_message_id": "rail/drop/1"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message leading punctuation",
                lambda body: body.update({"rail_message_id": "-rail-drop-1"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message trailing punctuation",
                lambda body: body.update({"rail_message_id": "rail-drop-1-"}),
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "receipt rail message oversized",
                lambda body: body.update(
                    {
                        "rail_message_id": "a"
                        * (VERIFIER.MAX_RAIL_MESSAGE_ID_CHARS + 1)
                    }
                ),
                "rail_message_id must be at most",
            ),
        ]
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
            original_receipt = receipt.read_bytes()

            for label, mutate, expected in cases:
                with self.subTest(label=label):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(receipt, mutate)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_oversized_notary_source_json_files_are_rejected_when_required(self):
        def make_fixture(root):
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
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
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            latest = export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE
            index_file = export_dir / audit_test.ADAPTER.INDEX_FILE
            record_source = next(
                (root / "store" / audit_test.ADAPTER.RECORDS_DIR).glob("*.json")
            )
            return receipt, latest, digest_anchor, index_file, record_source

        cases = [
            "latest_anchor",
            "digest_peer",
            "exported_index",
            "record_source",
        ]
        for name in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                receipt, latest, digest_anchor, index_file, record_source = make_fixture(
                    root
                )
                latest_size = len(latest.read_bytes())
                digest_anchor_size = len(digest_anchor.read_bytes())
                index_size = len(index_file.read_bytes())
                record_size = len(record_source.read_bytes())

                if name == "latest_anchor":
                    limit = latest_size - 1
                elif name == "digest_peer":
                    limit = max(latest_size, index_size, record_size) + 64
                    digest_anchor.write_bytes(oversized_json_bytes(limit))
                elif name == "exported_index":
                    limit = max(latest_size, digest_anchor_size, record_size) + 64
                    index_file.write_bytes(oversized_json_bytes(limit))
                elif name == "record_source":
                    limit = max(latest_size, digest_anchor_size, index_size) + 64
                    record_source.write_bytes(oversized_json_bytes(limit))
                else:  # pragma: no cover - guarded by the cases table.
                    raise AssertionError(name)

                with patched_verifier_constant("MAX_AUDIT_EXPORT_JSON_BYTES", limit):
                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                self.assertEqual(rc, 2)
                self.assertIn("byte JSON limit", stderr)

    def test_notary_anchor_source_mismatches_are_rejected_when_required(self):
        def mismatched_index():
            index = {
                "version": 1,
                "record_count": 1,
                "records": [audit_test.sample_record("msg-2")],
            }
            return audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)

        def replace_with_symlink(path, target):
            if path.exists() or path.is_symlink():
                path.unlink()
            try:
                path.symlink_to(target)
            except OSError as error:
                raise unittest.SkipTest(f"symlink creation unavailable: {error}") from error

        def symlinked_latest(_receipt, latest, _digest_anchor, _index_file):
            copy = latest.with_name("latest.copy.notary.json")
            copy.write_bytes(latest.read_bytes())
            replace_with_symlink(latest, copy)

        def symlinked_digest_peer(_receipt, _latest, digest_anchor, _index_file):
            copy = digest_anchor.with_name("digest.copy.notary.json")
            copy.write_bytes(digest_anchor.read_bytes())
            replace_with_symlink(digest_anchor, copy)

        def symlinked_index(_receipt, _latest, _digest_anchor, index_file):
            copy = index_file.with_name("messages.index.copy.json")
            copy.write_bytes(index_file.read_bytes())
            replace_with_symlink(index_file, copy)

        def unknown_anchor_field(receipt, latest, digest_anchor, _index_file):
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["operator_note"] = "accepted by phone"
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor.write_text(anchor_text, encoding="utf-8")
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ]
                    }
                ),
            )

        def unknown_exported_index_field(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["operator_note"] = "accepted by phone"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def unknown_embedded_record_field(receipt, latest, digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["operator_note"] = "accepted by phone"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def malformed_exported_record_field(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["updated_at_ms"] = -1
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def wrong_exported_record_filename(_receipt, _latest, _digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["filename"] = "msg-1.json"
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

        def duplicate_embedded_record(receipt, latest, digest_anchor, index_file):
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"].append(dict(index["records"][0]))
            index["record_count"] = 2
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor["record_count"] = 2
            anchor = audit_test.with_digest(
                anchor,
                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
            )
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                        "record_count": 2,
                    }
                ),
            )

        def record_source_path(latest):
            store_dir = latest.parent.parent / "store"
            return next((store_dir / audit_test.ADAPTER.RECORDS_DIR).glob("*.json"))

        def rewrite_digest_correct_record_source(
            receipt,
            latest,
            digest_anchor,
            index_file,
            mutate_source,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            mutate_source(source)
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["record_sha256"] = source[
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        def status_history_current_timestamp_mismatch(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            def mutate(source):
                source["status_history"][-1]["updated_at_ms"] = (
                    source["updated_at_ms"] - 1
                )

            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                mutate,
            )

        def status_history_timestamp_moves_backwards(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            def mutate(source):
                earlier_entry = dict(source["status_history"][-1])
                earlier_entry["updated_at_ms"] = source["updated_at_ms"] + 1
                source["status_history"].insert(0, earlier_entry)

            rewrite_digest_correct_record_source(
                receipt,
                latest,
                digest_anchor,
                index_file,
                mutate,
            )

        def missing_anchor_store_dir(receipt, latest, digest_anchor, _index_file):
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["store_dir"] = None
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor.write_text(anchor_text, encoding="utf-8")
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ]
                    }
                ),
            )

        def malformed_anchor_store_dir(value):
            def mutate(receipt, latest, digest_anchor, _index_file):
                anchor = json.loads(latest.read_text(encoding="utf-8"))
                anchor["store_dir"] = value
                anchor = audit_test.with_digest(
                    anchor,
                    audit_test.ADAPTER.ANCHOR_DIGEST_FIELD,
                )
                anchor_text = json.dumps(anchor, indent=2) + "\n"
                latest.write_text(anchor_text, encoding="utf-8")
                digest_anchor.write_text(anchor_text, encoding="utf-8")
                rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_sha256": anchor[
                                audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                            ]
                        }
                    ),
                )

            return mutate

        def missing_persisted_record_source(_receipt, latest, _digest_anchor, _index_file):
            record_source_path(latest).unlink()

        def digest_correct_record_source_mismatch(
            _receipt,
            latest,
            _digest_anchor,
            _index_file,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["transaction_hash"] = "d" * 64
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")

        def digest_correct_record_metadata_mismatch(
            receipt,
            latest,
            digest_anchor,
            index_file,
        ):
            record_path = record_source_path(latest)
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["metadata"]["message_type"] = "pacs.009"
            source = audit_test.with_digest(
                source,
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD,
            )
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index = json.loads(index_file.read_text(encoding="utf-8"))
            index["records"][0]["record_sha256"] = source[
                audit_test.ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = audit_test.with_digest(index, audit_test.ADAPTER.INDEX_DIGEST_FIELD)
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["audit_index"] = index
            anchor["index_sha256"] = index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]
            anchor = audit_test.with_digest(anchor, audit_test.ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            new_digest_anchor = digest_anchor.with_name(
                f"{index[audit_test.ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            new_digest_anchor.write_text(anchor_text, encoding="utf-8")
            if new_digest_anchor != digest_anchor:
                digest_anchor.unlink()
            rewrite_receipt(
                receipt,
                lambda body: body.update(
                    {
                        "anchor_sha256": anchor[
                            audit_test.ADAPTER.ANCHOR_DIGEST_FIELD
                        ],
                        "index_sha256": index[
                            audit_test.ADAPTER.INDEX_DIGEST_FIELD
                        ],
                    }
                ),
            )

        cases = [
            (
                "missing_digest_peer",
                lambda receipt, latest, digest_anchor, index_file: digest_anchor.unlink(),
                "latest anchor has no digest-addressed peer",
            ),
            (
                "different_digest_peer",
                lambda receipt, latest, digest_anchor, index_file: digest_anchor.write_text(
                    "{}\n",
                    encoding="utf-8",
                ),
                "latest anchor differs from digest-addressed peer",
            ),
            (
                "symlinked_latest_anchor",
                symlinked_latest,
                "must not be a symlink",
            ),
            (
                "symlinked_digest_peer",
                symlinked_digest_peer,
                "must not be a symlink",
            ),
            (
                "symlinked_index",
                symlinked_index,
                "must not be a symlink",
            ),
            (
                "exported_index_mismatch",
                lambda receipt, latest, digest_anchor, index_file: index_file.write_text(
                    json.dumps(mismatched_index(), indent=2) + "\n",
                    encoding="utf-8",
                ),
                "embedded audit index differs",
            ),
            (
                "unknown_anchor_field",
                unknown_anchor_field,
                "contains unknown keys: operator_note",
            ),
            (
                "unknown_exported_index_field",
                unknown_exported_index_field,
                "contains unknown keys: operator_note",
            ),
            (
                "unknown_embedded_record_field",
                unknown_embedded_record_field,
                "contains unknown keys: operator_note",
            ),
            (
                "malformed_exported_record_field",
                malformed_exported_record_field,
                "updated_at_ms must be a non-negative integer",
            ),
            (
                "wrong_exported_record_filename",
                wrong_exported_record_filename,
                "filename must be digest-addressed",
            ),
            (
                "duplicate_embedded_record",
                duplicate_embedded_record,
                "records[1].message_id duplicates",
            ),
            (
                "missing_anchor_store_dir",
                missing_anchor_store_dir,
                "store_dir is required to verify audit records",
            ),
            (
                "anchor_store_dir_whitespace",
                malformed_anchor_store_dir(str(Path("/ops/iso store"))),
                "store_dir must not contain whitespace",
            ),
            (
                "anchor_store_dir_dash",
                malformed_anchor_store_dir("--store"),
                "store_dir must not start with a dash",
            ),
            (
                "anchor_store_dir_segment_dash",
                malformed_anchor_store_dir("/ops/--store"),
                "store_dir must not contain leading-dash path segments",
            ),
            (
                "anchor_store_dir_parent_segment",
                malformed_anchor_store_dir("/ops/iso/../store"),
                "store_dir must not contain dot or parent segments",
            ),
            (
                "missing_persisted_record_source",
                missing_persisted_record_source,
                "references missing audit record",
            ),
            (
                "digest_correct_record_source_mismatch",
                digest_correct_record_source_mismatch,
                "record_sha256 does not match audit index record",
            ),
            (
                "digest_correct_record_metadata_mismatch",
                digest_correct_record_metadata_mismatch,
                "metadata.message_type does not match audit index record",
            ),
            (
                "status_history_current_timestamp_mismatch",
                status_history_current_timestamp_mismatch,
                "status_history does not end at current updated_at_ms",
            ),
            (
                "status_history_timestamp_moves_backwards",
                status_history_timestamp_moves_backwards,
                "status_history[1].updated_at_ms must not move backwards",
            ),
            (
                "wrong_anchor_filename",
                lambda receipt, latest, digest_anchor, index_file: (
                    digest_anchor.with_name("wrong.notary.json").write_bytes(
                        latest.read_bytes()
                    ),
                    rewrite_receipt(
                        receipt,
                        lambda body: body.update(
                            {
                                "anchor_path": str(
                                    digest_anchor.with_name("wrong.notary.json")
                                )
                            }
                        ),
                    ),
                ),
                "anchor_path filename must be digest-addressed",
            ),
            (
                "anchor_path_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + " "}),
                ),
                "anchor_path must not have surrounding whitespace",
            ),
            (
                "anchor_path_control",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + "\n"}),
                ),
                "anchor_path must not contain control characters",
            ),
            (
                "anchor_path_embedded_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {
                            "anchor_path": str(latest).replace(
                                "latest.notary.json",
                                "latest notary.json",
                            )
                        }
                    ),
                ),
                "anchor_path must not contain whitespace",
            ),
            (
                "anchor_path_dash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": "--latest.notary.json"}),
                ),
                "anchor_path must not start with a dash",
            ),
            (
                "anchor_path_segment_dash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": f"{latest.parent}/--{latest.name}"}),
                ),
                "anchor_path must not contain leading-dash path segments",
            ),
            (
                "anchor_path_backslash",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": str(latest).replace("/", "\\", 1)}
                    ),
                ),
                "anchor_path must use forward slashes",
            ),
            (
                "anchor_path_semicolon",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"anchor_path": str(latest) + ";debug"}),
                ),
                "anchor_path must not contain semicolon path parameters",
            ),
            (
                "anchor_path_empty_segment",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": f"{latest.parent}//{latest.name}"}
                    ),
                ),
                "anchor_path must not contain empty path segments",
            ),
            (
                "anchor_path_parent_segment",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update(
                        {"anchor_path": f"{latest.parent}/../{latest.name}"}
                    ),
                ),
                "anchor_path must not contain dot or parent segments",
            ),
            (
                "published_at_whitespace",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"published_at": body["published_at"] + " "}),
                ),
                "published_at must not have surrounding whitespace",
            ),
            (
                "published_at_control",
                lambda receipt, latest, digest_anchor, index_file: rewrite_receipt(
                    receipt,
                    lambda body: body.update({"published_at": body["published_at"] + "\n"}),
                ),
                "published_at must not contain control characters",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(
                export_dir,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )
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
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            latest = export_dir / audit_test.ADAPTER.LATEST_ANCHOR_FILE
            index_file = export_dir / audit_test.ADAPTER.INDEX_FILE
            original_receipt = receipt.read_bytes()
            original_latest = latest.read_bytes()
            original_digest_anchor = digest_anchor.read_bytes()
            original_index = index_file.read_bytes()
            store_dir = root / "store"
            record_source = next((store_dir / audit_test.ADAPTER.RECORDS_DIR).glob("*.json"))
            original_record_source = record_source.read_bytes()

            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    wrong_anchor = digest_anchor.with_name("wrong.notary.json")
                    if wrong_anchor.exists():
                        wrong_anchor.unlink()
                    for path in (latest, digest_anchor, index_file):
                        if path.is_symlink():
                            path.unlink()
                    if record_source.is_symlink():
                        record_source.unlink()
                    receipt.write_bytes(original_receipt)
                    latest.write_bytes(original_latest)
                    digest_anchor.write_bytes(original_digest_anchor)
                    index_file.write_bytes(original_index)
                    record_source.write_bytes(original_record_source)
                    mutate(receipt, latest, digest_anchor, index_file)

                    rc, _stdout, stderr = run_verify(
                        [
                            "--receipt",
                            str(receipt),
                            "--allow-insecure-http",
                            "--require-source-files",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_missing_notary_anchor_path_must_keep_digest_addressed_shape(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            _index, _anchor, digest_anchor = audit_test.write_export(export_dir)
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
            receipt = next((export_dir / "receipts").glob("*.receipt.json"))
            original_receipt = receipt.read_bytes()
            index_sha256 = digest_anchor.name.removesuffix(".notary.json")
            cases = [
                (
                    export_dir / "missing.notary.json",
                    "anchor_path must be latest.notary.json or anchors/<index_sha256>.notary.json",
                ),
                (
                    export_dir / "anchors" / "wrong.notary.json",
                    "anchor_path filename must be digest-addressed",
                ),
                (
                    export_dir / "anchors" / f"{index_sha256}.json",
                    "anchor_path filename must be digest-addressed",
                ),
            ]
            for anchor_path, expected in cases:
                with self.subTest(anchor_path=anchor_path.name):
                    receipt.write_bytes(original_receipt)
                    rewrite_receipt(
                        receipt,
                        lambda body, anchor_path=anchor_path: body.update(
                            {"anchor_path": str(anchor_path)}
                        ),
                    )
                    if anchor_path.exists():
                        anchor_path.unlink()

                    rc, _stdout, stderr = run_verify(
                        ["--receipt", str(receipt), "--allow-insecure-http"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

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
            ("notary", "endpoint", " https://notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example/anchor ", False),
            ("notary", "endpoint", "https://user:pass@notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example/anchor;debug", False),
            ("notary", "endpoint", "https://notary.example/anchor?debug=true", False),
            ("notary", "endpoint", "https://notary.example/anchor#fragment", False),
            ("notary", "endpoint", "https:///anchor", False),
            ("notary", "endpoint", "https://[::1", False),
            ("notary", "endpoint", "https://notary.example/anc\nhor", False),
            ("notary", "endpoint", "https://notary.example/iso anchor", False),
            ("notary", "endpoint", "https://notary.example:abc/anchor", False),
            ("notary", "endpoint", "https://notary.example:/anchor", False),
            ("notary", "endpoint", "https://notary.example:0/anchor", False),
            ("notary", "endpoint", "https://notary.example:08443/anchor", False),
            ("notary", "endpoint", "https://notary.example:99999/anchor", False),
            ("notary", "endpoint", "https://notary.example:443/anchor", False),
            ("notary", "endpoint", "https://Notary.example/anchor", False),
            ("notary", "endpoint", "https://notary.example./anchor", False),
            ("notary", "endpoint", "https://notary..example/anchor", False),
            ("notary", "endpoint", "https://localhost/anchor", False),
            ("notary", "endpoint", "https://10.1.2.3/anchor", False),
            ("notary", "endpoint", "https://10.1.2.3.sslip.io/anchor", False),
            ("notary", "endpoint", "https://0x7f.0.0.1/anchor", False),
            ("notary", "endpoint", "https://[::127.0.0.1]/anchor", False),
            ("notary", "endpoint", "https://-notary.example/anchor", False),
            ("notary", "endpoint", "https://notary-.example/anchor", False),
            ("notary", "endpoint", "https://notary._tcp.example/anchor", False),
            ("notary", "endpoint", "https://notary.example%2einvalid/anchor", False),
            ("notary", "endpoint", "https://123.000.000.001/anchor", False),
            ("notary", "endpoint", "https://notary.example/../anchor", False),
            ("notary", "endpoint", "https://notary.example/archive//anchor", False),
            ("notary", "endpoint", "https://notary.example/%2e%2e/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%2fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive%252fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive;debug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%3bdebug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%23debug/anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%20anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%00anchor", False),
            ("notary", "endpoint", "https://notary.example/archive%7fanchor", False),
            ("notary", "endpoint", "https://notary.example/archive%zzanchor", False),
            (
                "notary",
                "endpoint",
                "https://notary.example/" + ("a" * VERIFIER.MAX_HTTP_URL_CHARS),
                False,
            ),
            (
                "notary",
                "endpoint",
                "https://" + ".".join(["a" * 63] * 5) + "/anchor",
                False,
            ),
            ("rail", "endpoint_url", "https://localhost/v1/iso20022", False),
            ("rail", "endpoint_url", "https://127.0.0.1/v1/iso20022", False),
            ("rail", "endpoint_url", "https://127.0.0.1.nip.io/v1/iso20022", False),
            ("rail", "endpoint_url", "https://0x7f000001/v1/iso20022", False),
            ("rail", "endpoint_url", "https://[64:ff9b::7f00:1]/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example:/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example:0/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example:08443/v1/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example/v1%3bdebug/iso20022", False),
            ("rail", "endpoint_url", "https://rail.example/v1%3fdebug/iso20022", False),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022 ", True),
            ("rail", "endpoint_url", "http://user:pass@127.0.0.1/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022?debug=true", True),
            ("rail", "endpoint_url", "http://127.0.0.1/v1/iso20022 pacs008", True),
            ("rail", "endpoint_url", "http://127.0.0.1:abc/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:80/v1/iso20022", True),
            ("rail", "endpoint_url", "http://LocalHost:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://localhost.:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://local_host:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.000.000.001:8080/v1/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1/../iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1//iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1/%2e%2e/iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%2fiso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%252fiso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%20iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%00iso20022", True),
            ("rail", "endpoint_url", "http://127.0.0.1:8080/v1%zziso20022", True),
            ("rail", "endpoint_url", r"http://127.0.0.1:8080/v1\iso20022", True),
            (
                "rail",
                "endpoint_url",
                "http://127.0.0.1:8080/" + ("a" * VERIFIER.MAX_HTTP_URL_CHARS),
                True,
            ),
            (
                "rail",
                "endpoint_url",
                "http://" + ".".join(["a" * 63] * 5) + ":8080/v1/iso20022",
                True,
            ),
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

    def test_rejected_receipt_endpoint_url_does_not_echo_secret_query(self):
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

            cases = [
                (
                    notary_receipt,
                    "endpoint",
                    "https://notary.example/anchor?token=notary-secret",
                    "notary-secret",
                ),
                (
                    rail_receipt,
                    "endpoint_url",
                    "https://rail.example/v1/iso20022?token=rail-secret",
                    "rail-secret",
                ),
            ]
            for receipt, field, secret_url, secret in cases:
                with self.subTest(field=field):
                    rewrite_receipt(
                        receipt,
                        lambda body, field=field, secret_url=secret_url: body.update(
                            {field: secret_url}
                        ),
                    )

                    rc, _stdout, stderr = run_verify(["--receipt", str(receipt)])

                    self.assertEqual(rc, 2)
                    self.assertIn("params, query, or fragment", stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn(secret, stderr)


if __name__ == "__main__":
    unittest.main()
