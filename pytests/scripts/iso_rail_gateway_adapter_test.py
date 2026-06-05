import contextlib
import http.server
import importlib.util
import io
import json
import sys
import tempfile
import threading
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_rail_gateway_adapter.py"
SPEC = importlib.util.spec_from_file_location("iso_rail_gateway_adapter", SCRIPT_PATH)
ADAPTER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = ADAPTER
SPEC.loader.exec_module(ADAPTER)


SAMPLE_XML = b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>rail-1</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>"


def run_main(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = ADAPTER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


def write_message(inbox, *, message_type="pacs.002", profile="swift-cbpr-plus", payload=SAMPLE_XML):
    xml_path = inbox / "rail-status.xml"
    xml_path.write_bytes(payload)
    sidecar = {
        "message_type": message_type,
        "profile": profile,
        "payload_sha256": ADAPTER.sha256_hex(payload),
        "rail_message_id": "rail-drop-1",
    }
    (inbox / "rail-status.xml.json").write_text(json.dumps(sidecar), encoding="utf-8")
    return xml_path, sidecar


def receipt_digest_matches(receipt):
    expected = receipt[ADAPTER.RECEIPT_DIGEST_FIELD]
    body = dict(receipt)
    body.pop(ADAPTER.RECEIPT_DIGEST_FIELD)
    return ADAPTER.sha256_hex(ADAPTER._canonical_json_bytes(body)) == expected


@contextlib.contextmanager
def capture_server(status=202, body=b'{"message_id":"rail-1"}'):
    requests = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self):  # noqa: N802 - BaseHTTPRequestHandler API
            length = int(self.headers.get("Content-Length", "0"))
            payload = self.rfile.read(length)
            requests.append(
                {
                    "path": self.path,
                    "headers": dict(self.headers),
                    "body": payload,
                }
            )
            self.send_response(status)
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            return

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_address[1]}", requests
    finally:
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()


class IsoRailGatewayAdapterTest(unittest.TestCase):
    def test_submit_verified_file_drop_to_torii_endpoint(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            with capture_server() as (base_url, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 0)
            self.assertEqual(len(requests), 1)
            self.assertEqual(requests[0]["path"], "/v1/iso20022/pacs002")
            self.assertEqual(requests[0]["body"], SAMPLE_XML)
            self.assertEqual(
                requests[0]["headers"]["X-Iroha-Iso-Profile"], "swift-cbpr-plus"
            )
            self.assertEqual(
                requests[0]["headers"]["X-Iroha-Iso-Gateway-Payload-Sha256"],
                sidecar["payload_sha256"],
            )
            receipts = list((inbox / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertTrue(receipt["ok"])
            self.assertEqual(receipt["status_code"], 202)
            self.assertTrue(receipt_digest_matches(receipt))

    def test_bearer_token_file_adds_authorization_without_persisting_token(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            token_file = inbox / "token.txt"
            token_file.write_text("rail-token-123", encoding="utf-8")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                        "--bearer-token-file",
                        str(token_file),
                    ]
                )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(requests), 1)
            self.assertEqual(
                requests[0]["headers"]["Authorization"], "Bearer rail-token-123"
            )
            receipts = list((inbox / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            self.assertNotIn("rail-token-123", receipts[0].read_text(encoding="utf-8"))

    def test_malformed_bearer_token_file_is_rejected_before_network_delivery(self):
        cases = [
            ("empty", b"", "empty"),
            ("padded", b" rail-token", "surrounding whitespace"),
            ("newline", b"rail-token\n", "surrounding whitespace"),
            ("embedded-space", b"rail token", "must not contain whitespace"),
            ("control", b"rail-token\x7f", "must not contain control characters"),
            ("non-utf8", b"rail-token\xff", "not UTF-8"),
            (
                "oversized",
                b"a" * (ADAPTER.MAX_BEARER_TOKEN_BYTES + 1),
                "exceeds",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            for name, token_bytes, message in cases:
                with self.subTest(name=name):
                    token_file = inbox / f"{name}.token"
                    token_file.write_bytes(token_bytes)
                    with capture_server() as (base_url, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                                "--bearer-token-file",
                                str(token_file),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_non_regular_bearer_token_files_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            token_target = inbox / "token-target.txt"
            token_target.write_text("rail-token-123", encoding="utf-8")
            symlink_token = inbox / "symlink-token.txt"
            try:
                symlink_token.symlink_to(token_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            directory_token = inbox / "token-dir"
            directory_token.mkdir()
            cases = [
                (symlink_token, "must not be a symlink"),
                (directory_token, "must be a regular file"),
            ]
            for token_file, message in cases:
                with self.subTest(token_file=token_file.name):
                    with capture_server() as (base_url, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                                "--bearer-token-file",
                                str(token_file),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_symlinked_receipt_output_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            receipt_target_dir = inbox / "receipt-target"
            receipt_target_dir.mkdir()
            receipt_dir = inbox / "receipt-link"
            try:
                receipt_dir.symlink_to(receipt_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

            receipt_dir.unlink()
            receipt_dir.mkdir()
            receipt = receipt_dir / f"{ADAPTER.sha256_hex(SAMPLE_XML)}.receipt.json"
            target = inbox / "receipt-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            try:
                receipt.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_inbox_dir_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_inbox = root / "inbox-target"
            target_inbox.mkdir()
            write_message(target_inbox)
            inbox = root / "inbox-link"
            try:
                inbox.symlink_to(target_inbox, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_colr012_routes_to_standard_collateral_endpoint(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(
                inbox,
                message_type="colr.012",
                profile="securities-csd",
                payload=b"<Document><CollSbstitnConf/></Document>",
            )
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(requests), 1)
            self.assertEqual(requests[0]["path"], "/v1/iso20022/colr012")
            self.assertEqual(requests[0]["headers"]["X-Iroha-Iso-Profile"], "securities-csd")

    def test_colr007_legacy_submission_requires_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(
                inbox,
                message_type="colr.007",
                profile="securities-csd",
                payload=b"<Document><CollSbstitnConf/></Document>",
            )
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertIn("legacy message_type", stderr)
            self.assertEqual(requests, [])

            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                        "--allow-legacy-colr007",
                    ]
                )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(requests), 1)
            self.assertEqual(requests[0]["path"], "/v1/iso20022/colr007")

    def test_single_message_relative_to_inbox_is_supported(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--message",
                        "rail-status.xml",
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(requests), 1)

    def test_explicit_symlinked_message_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, sidecar = write_message(inbox)
            outside_xml = inbox / "outside.xml"
            outside_xml.write_bytes(xml_path.read_bytes())
            (inbox / "outside.xml.json").write_text(
                json.dumps(sidecar),
                encoding="utf-8",
            )
            xml_path.unlink()
            try:
                xml_path.symlink_to(outside_xml)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--message",
                        xml_path.name,
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_single_message_path_must_stay_under_inbox(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            outside = root / "outside"
            inbox.mkdir()
            outside.mkdir()
            outside_xml, _sidecar = write_message(outside)
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--message",
                        str(outside_xml),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertIn("--message path", stderr)
            self.assertEqual(requests, [])

    def test_symlinked_message_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            outside = root / "outside"
            inbox.mkdir()
            outside.mkdir()
            outside_xml, _outside_sidecar = write_message(outside)
            symlink_xml = inbox / "rail-status.xml"
            try:
                symlink_xml.symlink_to(outside_xml)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            sidecar = {
                "message_type": "pacs.002",
                "profile": "swift-cbpr-plus",
                "payload_sha256": ADAPTER.sha256_hex(SAMPLE_XML),
                "rail_message_id": "rail-drop-1",
            }
            (inbox / "rail-status.xml.json").write_text(
                json.dumps(sidecar),
                encoding="utf-8",
            )
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_symlinked_sidecar_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            outside = root / "outside"
            inbox.mkdir()
            outside.mkdir()
            _xml_path, _sidecar = write_message(inbox)
            outside_xml, _outside_sidecar = write_message(outside)
            sidecar_path = inbox / "rail-status.xml.json"
            sidecar_path.unlink()
            try:
                sidecar_path.symlink_to(outside_xml.with_suffix(outside_xml.suffix + ".json"))
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_directory_sidecar_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            sidecar_path.unlink()
            sidecar_path.mkdir()
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must be a regular file", stderr)

    def test_digest_mismatch_rejects_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, _sidecar = write_message(inbox)
            xml_path.write_bytes(SAMPLE_XML + b"tampered")
            with capture_server() as (base_url, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])

    def test_duplicate_sidecar_json_keys_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            (inbox / "rail-status.xml.json").write_text(
                (
                    '{"message_type":"pacs.002","message_type":"pacs.008",'
                    f'"profile":"swift-cbpr-plus",'
                    f'"payload_sha256":"{sidecar["payload_sha256"]}",'
                    '"rail_message_id":"rail-drop-1"}\n'
                ),
                encoding="utf-8",
            )
            with capture_server() as (base_url, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("duplicate key", stderr)

    def test_sidecar_header_strings_must_not_require_trimming(self):
        cases = [
            (
                "profile whitespace",
                "profile",
                " swift-cbpr-plus",
                "profile must not have surrounding whitespace",
            ),
            (
                "profile control",
                "profile",
                "swift\ncbpr-plus",
                "profile must not contain control characters",
            ),
            (
                "profile embedded whitespace",
                "profile",
                "swift cbpr-plus",
                "profile must not contain whitespace",
            ),
            (
                "rail message whitespace",
                "rail_message_id",
                "rail-drop-1 ",
                "rail_message_id must not have surrounding whitespace",
            ),
            (
                "rail message control",
                "rail_message_id",
                "rail\n1",
                "rail_message_id must not contain control characters",
            ),
            (
                "rail message embedded whitespace",
                "rail_message_id",
                "rail drop 1",
                "rail_message_id must not contain whitespace",
            ),
        ]
        for label, field, value, message in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    _xml_path, sidecar = write_message(inbox)
                    sidecar[field] = value
                    (inbox / "rail-status.xml.json").write_text(
                        json.dumps(sidecar),
                        encoding="utf-8",
                    )
                    with capture_server() as (base_url, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_profile_is_required_for_live_rail_submission_by_default(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox, profile=None)
            sidecar_path = inbox / "rail-status.xml.json"
            sidecar = json.loads(sidecar_path.read_text(encoding="utf-8"))
            sidecar.pop("profile")
            sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
            with capture_server() as (base_url, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])

    def test_plain_http_torii_url_is_rejected_without_test_override(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            with capture_server() as (base_url, requests):
                rc, _stdout, _stderr = run_main(
                    ["--inbox-dir", str(inbox), "--torii-base-url", base_url]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])

    def test_torii_url_smuggling_variants_are_rejected_before_network_delivery(self):
        cases = [
            (" https://torii.example", False),
            ("https://torii.example ", False),
            ("https://user:pass@torii.example", False),
            ("https://torii.example/base;debug", False),
            ("https://torii.example?token=abc", False),
            ("https://torii.example#fragment", False),
            ("https:///v1", False),
            ("https://[::1", False),
            ("https://torii.example/iso\nbridge", False),
            ("https://torii.example/iso bridge", False),
            ("https://torii.example:abc", False),
            ("https://torii.example:99999", False),
            ("https://torii.example:443", False),
            ("https://Torii.example", False),
            ("https://torii.example.", False),
            ("https://torii..example", False),
            ("https://-torii.example", False),
            ("https://torii-.example", False),
            ("https://torii._tcp.example", False),
            ("https://torii.example%2einvalid", False),
            ("https://123.000.000.001", False),
            ("https://torii.example/../base", False),
            ("https://torii.example/%2e%2e/base", False),
            ("https://torii.example/base%2fv1", False),
            ("https://torii.example/base%252fv1", False),
            ("https://torii.example/base;debug/v1", False),
            (r"https://torii.example/base\v1", False),
            ("https://torii.example/base%20v1", False),
            ("https://torii.example/base%00v1", False),
            ("https://torii.example/base%7fv1", False),
            ("https://torii.example/base%zzv1", False),
            ("http://127.0.0.1 ", True),
            ("http://user:pass@127.0.0.1", True),
            ("http://127.0.0.1?token=abc", True),
            ("http://127.0.0.1:80", True),
            ("http://127.000.000.001", True),
        ]
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            for base_url, allow_insecure in cases:
                with self.subTest(base_url=base_url, allow_insecure=allow_insecure):
                    argv = [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                    ]
                    if allow_insecure:
                        argv.append("--allow-insecure-http")
                    rc, _stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("error:", stderr)

    def test_non_successful_torii_response_writes_failed_receipt(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            with capture_server(status=409, body=b"duplicate") as (base_url, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 1)
            self.assertEqual(len(requests), 1)
            receipts = list((inbox / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertFalse(receipt["ok"])
            self.assertEqual(receipt["status_code"], 409)
            self.assertEqual(receipt["payload_sha256"], sidecar["payload_sha256"])
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(b"duplicate"))
            self.assertTrue(receipt_digest_matches(receipt))


if __name__ == "__main__":
    unittest.main()
