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


def write_named_message(
    inbox,
    name,
    *,
    message_type="pacs.002",
    profile="swift-cbpr-plus",
    payload=SAMPLE_XML,
    rail_message_id=None,
):
    xml_path = inbox / f"{name}.xml"
    xml_path.write_bytes(payload)
    sidecar = {
        "message_type": message_type,
        "profile": profile,
        "payload_sha256": ADAPTER.sha256_hex(payload),
        "rail_message_id": rail_message_id or f"{name}-rail-id",
    }
    (inbox / f"{name}.xml.json").write_text(json.dumps(sidecar), encoding="utf-8")
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


@contextlib.contextmanager
def capture_redirect_server(body=b"redirect"):
    requests = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self):  # noqa: N802 - BaseHTTPRequestHandler API
            length = int(self.headers.get("Content-Length", "0"))
            payload = self.rfile.read(length)
            requests.append(
                {
                    "method": "POST",
                    "path": self.path,
                    "headers": dict(self.headers),
                    "body": payload,
                }
            )
            location = f"http://127.0.0.1:{self.server.server_address[1]}/redirected"
            self.send_response(302)
            self.send_header("Location", location)
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):  # noqa: N802 - BaseHTTPRequestHandler API
            requests.append(
                {
                    "method": "GET",
                    "path": self.path,
                    "headers": dict(self.headers),
                    "body": b"",
                }
            )
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"followed")

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
            receipt_dir = inbox / "receipts"
            receipt_dir.mkdir()
            receipt_path = receipt_dir / f"{sidecar['payload_sha256']}.receipt.json"
            receipt_path.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")
            with capture_server() as (base_url, requests):
                rc, _stdout, _stderr = run_main(
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
            receipts = list(receipt_dir.glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertTrue(receipt["ok"])
            self.assertEqual(receipt["status_code"], 202)
            self.assertTrue(receipt_digest_matches(receipt))
            self.assertEqual(receipts[0].stat().st_mode & 0o077, 0)
            self.assertEqual(list(receipt_dir.glob(".iso-*.tmp")), [])

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

    def test_duplicate_payloads_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_named_message(inbox, "first", payload=SAMPLE_XML)
            write_named_message(inbox, "second", payload=SAMPLE_XML)
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("payload_sha256 duplicates", stderr)
            self.assertFalse((inbox / "receipts").exists())

    def test_duplicate_rail_message_ids_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_named_message(
                inbox,
                "first",
                payload=b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>first</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>",
                rail_message_id="rail-duplicate",
            )
            write_named_message(
                inbox,
                "second",
                payload=b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>second</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>",
                rail_message_id="rail-duplicate",
            )
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("rail_message_id duplicates", stderr)
            self.assertFalse((inbox / "receipts").exists())

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

    def test_bearer_token_reader_enforces_configured_file_cap(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            token_file = Path(raw_inbox) / "token.txt"
            token_file.write_bytes(b"a" * 9)
            original_limit = ADAPTER.MAX_BEARER_TOKEN_BYTES
            ADAPTER.MAX_BEARER_TOKEN_BYTES = 8
            try:
                with self.assertRaises(ADAPTER.AdapterError) as raised:
                    ADAPTER._load_bearer_token(token_file)
            finally:
                ADAPTER.MAX_BEARER_TOKEN_BYTES = original_limit

            self.assertIn("exceeds 8 byte bearer token limit", str(raised.exception))

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

    def test_bearer_token_file_symlinked_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            target_dir = inbox / "token-target"
            target_dir.mkdir()
            token_target = target_dir / "token.txt"
            token_target.write_text("rail-token-123", encoding="utf-8")
            ancestor = inbox / "token-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            token_file = ancestor / token_target.name

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
            self.assertIn("must not be a symlink", stderr)

    def test_input_cli_paths_reject_raw_smuggling_before_read(self):
        cases = (
            ("inbox semicolon", "--inbox-dir", "inbox;debug", "semicolon path"),
            ("inbox whitespace", "--inbox-dir", "inbox dir", "whitespace"),
            ("inbox leading-dash", "--inbox-dir", "nested/-inbox", "leading-dash"),
            ("inbox parent", "--inbox-dir", "nested/../inbox", "dot or parent"),
            (
                "inbox dot",
                "--inbox-dir",
                lambda root: f"{root}/nested/./inbox",
                "dot or parent",
            ),
            (
                "token empty",
                "--bearer-token-file",
                lambda root: f"{root}//token.txt",
                "empty path",
            ),
            (
                "token backslash",
                "--bearer-token-file",
                r"nested\token.txt",
                "forward slashes",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)
                    argv = [
                        "--inbox-dir",
                        str(root),
                        "--torii-base-url",
                        "https://torii.example.invalid",
                        flag,
                        value,
                    ]
                    if flag == "--inbox-dir":
                        argv = [
                            "--inbox-dir",
                            value,
                            "--torii-base-url",
                            "https://torii.example.invalid",
                        ]

                    rc, stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

    def test_numeric_cli_limits_reject_nonpositive_and_nonfinite_before_network_delivery(self):
        cases = (
            ("timeout nan", "--timeout-secs", "nan", "positive finite number"),
            ("timeout inf", "--timeout-secs", "inf", "positive finite number"),
            ("timeout zero", "--timeout-secs", "0", "positive finite number"),
            ("response zero", "--response-limit-bytes", "0", "positive integer"),
            ("response negative", "--response-limit-bytes", "-1", "positive integer"),
            ("payload zero", "--max-payload-bytes", "0", "positive integer"),
            ("payload negative", "--max-payload-bytes", "-1", "positive integer"),
        )
        for name, flag, value, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    write_message(inbox)
                    with capture_server() as (base_url, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                                flag,
                                value,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)
                    self.assertFalse((inbox / "receipts").exists())

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

    def test_receipt_output_paths_reject_smuggled_segments_before_network_delivery(self):
        cases = (
            ("semicolon", "receipts;debug", "must not contain semicolon path parameters"),
            ("whitespace", "receipt dir", "must not contain whitespace"),
            ("leading-dash", "nested/-receipts", "must not contain leading-dash path segments"),
            ("parent", "nested/../receipts", "must not contain dot or parent segments"),
            ("dot", lambda root: f"{root}/nested/./receipts", "dot or parent segments"),
            ("empty", lambda root: f"{root}//receipts", "empty path segments"),
        )
        for name, receipt_dir_arg, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    write_message(inbox)
                    receipt_dir = (
                        receipt_dir_arg(inbox)
                        if callable(receipt_dir_arg)
                        else str(inbox / receipt_dir_arg)
                    )

                    with capture_server() as (base_url, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                                "--receipt-dir",
                                receipt_dir,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_hardlinked_receipt_output_leaf_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            receipt_dir = inbox / "receipts"
            receipt_dir.mkdir()
            target = inbox / "receipt-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            receipt_path = receipt_dir / f"{sidecar['payload_sha256']}.receipt.json"
            try:
                receipt_path.hardlink_to(target)
            except OSError as error:
                self.skipTest(f"hard link creation unavailable: {error}")

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
            self.assertIn("must not be hard-linked", stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_receipt_output_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            target_dir = inbox / "receipt-target"
            target_dir.mkdir()
            ancestor = inbox / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            receipt_dir = ancestor / "nested" / "receipts"

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
            self.assertFalse((target_dir / "nested").exists())

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

    def test_symlinked_inbox_dir_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_inbox = root / "inbox-target"
            target_inbox.mkdir()
            ancestor = root / "inbox-ancestor-link"
            try:
                ancestor.symlink_to(target_inbox, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            inbox = ancestor / "nested"
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

    def test_explicit_message_paths_reject_smuggling_before_network_delivery(self):
        cases = [
            ("whitespace", "rail status.xml", "must not contain whitespace"),
            ("leading dash", "--rail-status.xml", "must not start with a dash"),
            (
                "segment leading dash",
                "nested/--rail-status.xml",
                "must not contain leading-dash path segments",
            ),
            ("backslash", r"nested\rail-status.xml", "must use forward slashes"),
            (
                "semicolon",
                "rail-status.xml;v=1",
                "must not contain semicolon path parameters",
            ),
            ("empty segment", "nested//rail-status.xml", "must not contain empty path segments"),
            ("dot segment", "nested/./rail-status.xml", "must not contain dot or parent segments"),
            ("parent segment", "nested/../rail-status.xml", "must not contain dot or parent segments"),
        ]
        for label, message, expected in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    write_message(inbox)
                    with capture_server() as (base_url, requests):
                        message_args = (
                            [f"--message={message}"]
                            if message.startswith("-")
                            else ["--message", message]
                        )
                        rc, _stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                *message_args,
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(expected, stderr)

    def test_discovered_message_leaf_rejects_smuggling_before_network_delivery(self):
        cases = [
            ("whitespace", "rail status.xml", "filename must not contain whitespace"),
            ("leading dash", "--rail-status.xml", "filename must not start with a dash"),
        ]
        for label, filename, expected in cases:
            with self.subTest(label=label):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    xml_path = inbox / filename
                    xml_path.write_bytes(SAMPLE_XML)
                    sidecar = {
                        "message_type": "pacs.002",
                        "profile": "swift-cbpr-plus",
                        "payload_sha256": ADAPTER.sha256_hex(SAMPLE_XML),
                        "rail_message_id": "rail-drop-1",
                    }
                    (inbox / f"{filename}.json").write_text(
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
                    self.assertIn(expected, stderr)

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

    def test_non_finite_sidecar_json_numbers_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            (inbox / "rail-status.xml.json").write_text(
                (
                    f'{{"message_type":"pacs.002","profile":"swift-cbpr-plus",'
                    f'"payload_sha256":"{sidecar["payload_sha256"]}",'
                    '"rail_message_id":NaN}\n'
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
            self.assertIn("non-finite numeric constant NaN", stderr)

    def test_sidecar_json_surrogate_strings_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            (inbox / "rail-status.xml.json").write_text(
                (
                    f'{{"message_type":"pacs.002","profile":"swift-cbpr-plus",'
                    f'"payload_sha256":"{sidecar["payload_sha256"]}",'
                    '"rail_message_id":"\\ud800"}\n'
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
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_unknown_sidecar_fields_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            sidecar["operator_note"] = "looks valid"
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
            self.assertIn("contains unknown keys: operator_note", stderr)

    def test_oversized_sidecar_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            sidecar["rail_message_id"] = "a" * ADAPTER.MAX_SIDECAR_JSON_BYTES
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
            self.assertIn("exceeds", stderr)

    def test_sidecar_header_strings_must_not_require_trimming(self):
        cases = [
            (
                "profile null",
                "profile",
                None,
                "profile must be a non-empty string",
            ),
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
                "profile uppercase",
                "profile",
                "Swift-CBPR-Plus",
                "profile must be a canonical lowercase profile id",
            ),
            (
                "profile underscore",
                "profile",
                "swift_cbpr_plus",
                "profile must be a canonical lowercase profile id",
            ),
            (
                "profile trailing hyphen",
                "profile",
                "swift-cbpr-plus-",
                "profile must be a canonical lowercase profile id",
            ),
            (
                "rail message null",
                "rail_message_id",
                None,
                "rail_message_id must be a non-empty string",
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
            (
                "rail message unicode",
                "rail_message_id",
                "rail-drop-\U0001f69a",
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail message path separator",
                "rail_message_id",
                "rail/drop/1",
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail message leading punctuation",
                "rail_message_id",
                "-rail-drop-1",
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail message trailing punctuation",
                "rail_message_id",
                "rail-drop-1-",
                "rail_message_id must be a canonical ASCII rail message id",
            ),
            (
                "rail message oversized",
                "rail_message_id",
                "a" * (ADAPTER.MAX_RAIL_MESSAGE_ID_CHARS + 1),
                "rail_message_id must be at most",
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
            ("https://torii.example:", False),
            ("https://torii.example:0", False),
            ("https://torii.example:08443", False),
            ("https://torii.example:99999", False),
            ("https://torii.example:443", False),
            ("https://Torii.example", False),
            ("https://torii.example.", False),
            ("https://torii..example", False),
            ("https://localhost/base", False),
            ("https://127.0.0.1/base", False),
            ("https://127.0.0.1.nip.io/base", False),
            ("https://0x7f000001/base", False),
            ("https://[64:ff9b::7f00:1]/base", False),
            ("https://-torii.example", False),
            ("https://torii-.example", False),
            ("https://torii._tcp.example", False),
            ("https://torii.example%2einvalid", False),
            ("https://123.000.000.001", False),
            ("https://torii.example/../base", False),
            ("https://torii.example/base//v1", False),
            ("https://torii.example/%2e%2e/base", False),
            ("https://torii.example/base%2fv1", False),
            ("https://torii.example/base%252fv1", False),
            ("https://torii.example/base;debug/v1", False),
            ("https://torii.example/base%3bdebug/v1", False),
            ("https://torii.example/base%3fdebug/v1", False),
            (r"https://torii.example/base\v1", False),
            ("https://torii.example/base%20v1", False),
            ("https://torii.example/base%00v1", False),
            ("https://torii.example/base%7fv1", False),
            ("https://torii.example/base%zzv1", False),
            ("https://torii.example/" + ("a" * ADAPTER.MAX_HTTP_URL_CHARS), False),
            ("https://" + ".".join(["a" * 63] * 5), False),
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

    def test_rejected_torii_url_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            secret_url = "https://torii.example?token=rail-secret"
            rc, _stdout, stderr = run_main(
                ["--inbox-dir", str(inbox), "--torii-base-url", secret_url]
            )

            self.assertEqual(rc, 2)
            self.assertIn("params, query, or fragment", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("rail-secret", stderr)

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

    def test_torii_redirect_response_is_not_followed(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            with capture_redirect_server() as (base_url, requests):
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
            self.assertEqual([request["method"] for request in requests], ["POST"])
            receipts = list((inbox / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertFalse(receipt["ok"])
            self.assertEqual(receipt["status_code"], 302)
            self.assertEqual(receipt["payload_sha256"], sidecar["payload_sha256"])
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(b"redirect"))
            self.assertTrue(receipt_digest_matches(receipt))

    def test_secret_looking_torii_response_preview_is_redacted(self):
        body = b'{"error":"token=rail-secret"}'
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, _sidecar = write_message(inbox)
            with capture_server(status=500, body=body) as (base_url, requests):
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
            self.assertEqual(receipt["status_code"], 500)
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(body))
            self.assertEqual(
                receipt["response_body_preview"],
                ADAPTER.REDACTED_RESPONSE_PREVIEW,
            )
            self.assertNotIn("rail-secret", receipts[0].read_text(encoding="utf-8"))
            self.assertTrue(receipt_digest_matches(receipt))

    def test_secret_looking_url_error_is_redacted(self):
        self.assertEqual(
            ADAPTER._receipt_error("upstream token=rail-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(ADAPTER._receipt_error("connection refused"), "connection refused")


if __name__ == "__main__":
    unittest.main()
