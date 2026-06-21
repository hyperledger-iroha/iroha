import argparse
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
    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_rail_unknown_secret", "rail_unknown_secret"),
            ("%70assword_rail_unknown_leak", "rail_unknown_leak"),
            ("private-key_rail_unknown_leak", "rail_unknown_leak"),
            ("unexpected\x1brail_key", "\x1b"),
            ("unexpected_rail_\uff4bey", "\uff4b"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._reject_unknown_keys(
                        {unknown_key: "redacted"}, set(), "sidecar"
                    )

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._reject_unknown_keys(many_unknown, set(), "sidecar")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=rail-terminator-secret"
        cases = (
            (
                "raw",
                lambda: ADAPTER._preflight_raw_cli_secrets(
                    ["--", "--receipt-dir", hidden],
                    {"--receipt-dir"},
                ),
            ),
            (
                "path",
                lambda: ADAPTER._preflight_output_cli_paths(
                    ["--", "--receipt-dir", hidden],
                    {"--receipt-dir"},
                ),
            ),
            (
                "boolean",
                lambda: ADAPTER._preflight_boolean_cli_flags(
                    ["--", "--dry-run", hidden],
                    {"--dry-run"},
                ),
            ),
            (
                "url",
                lambda: ADAPTER._preflight_required_cli_values(
                    ["--", "--torii-base-url", hidden],
                    {"--torii-base-url"},
                    "URL",
                ),
            ),
            (
                "numeric",
                lambda: ADAPTER._preflight_numeric_cli_values(
                    ["--", "--timeout-secs", hidden],
                    integer_flags=set(),
                    number_flags={"--timeout-secs"},
                ),
            ),
        )
        for helper, run in cases:
            with self.subTest(helper=helper):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn("argument terminator is not supported", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("rail-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                ADAPTER.build_parser().parse_args(
                    [
                        "--inbox-dir",
                        ".",
                        "--torii-base-url",
                        "https://bank.example/iso",
                        "--receipt-di",
                        "out",
                    ]
                )

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--receipt-di", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        hidden = "--unknown-rail\x1bflag"
        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

        message = str(caught.exception)
        self.assertIn("CLI argument must not contain control characters", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("\x1b", message)
        self.assertNotIn("unknown-rail", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dreceipt-dir"
        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("receipt-dir", message)

    def test_nested_control_material_in_sidecar_is_rejected_without_echo(self):
        cases = (
            (
                {"metadata": {"unexpected\x1brail_key": "redacted"}},
                "forbidden control-bearing field",
                "rail_key",
            ),
            (
                {"metadata": {"note": "warning \x1b[31mred"}},
                "unsafe control characters",
                "[31mred",
            ),
        )
        for body, expected, hidden in cases:
            with self.subTest(body=body):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._check_no_secret_material(body, "sidecar")

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn(hidden, message)

    def test_output_cli_path_flags_reject_flag_like_values(self):
        cases = (
            ["--receipt-dir"],
            ["--receipt-dir", ""],
            ["--receipt-dir", "--allow-insecure-http"],
            ["--receipt-dir="],
            ["--receipt-dir=--allow-insecure-http"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                with self.assertRaisesRegex(
                    ADAPTER.AdapterError,
                    "--receipt-dir requires a path value",
                ):
                    ADAPTER._preflight_output_cli_paths(argv, {"--receipt-dir"})

    def test_output_cli_paths_reject_encoded_secret_material_without_echo(self):
        cases = (
            ("token=rail-path-leak.receipts", "token=rail-path-leak"),
            ("token%3Drail-path-leak.receipts", "token=rail-path-leak"),
            ("%70assword%253Drail-path-leak.receipts", "password=rail-path-leak"),
            ("token-rail-path-secret.receipts", "token-rail-path-secret"),
        )
        for raw_path, decoded_secret in cases:
            with self.subTest(raw_path=raw_path):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._preflight_output_cli_paths(
                        ["--receipt-dir", raw_path], {"--receipt-dir"}
                    )

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn(raw_path, message)
                self.assertNotIn(decoded_secret, message)
                self.assertNotIn("rail-path-leak", message)

    def test_receipt_outputs_reject_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "fixtures" / "iso20022" / "rail-receipts"
            receipt_path = root / "fixtures" / "iso20022" / "rail-receipt.json"

            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "receipt directory must not point to checked-in ISO fixture artifacts",
            ):
                ADAPTER._ensure_output_directory(receipt_dir, "receipt directory")

            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "output path must not point to checked-in ISO fixture artifacts",
            ):
                ADAPTER._write_text_output(receipt_path, "{}\n")

            self.assertFalse((root / "fixtures").exists())

    def test_receipt_dir_rejects_repository_fixture_before_inbox_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "fixtures" / "iso20022" / "rail-receipts"

            rc, stdout, stderr = run_main(
                [
                    "--inbox-dir",
                    str(root / "missing-inbox"),
                    "--torii-base-url",
                    "http://127.0.0.1:1",
                    "--allow-insecure-http",
                    "--receipt-dir",
                    str(receipt_dir),
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "receipt_dir must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((root / "fixtures").exists())

    def test_local_path_validators_reject_percent_encoded_smuggling(self):
        overlong_path = "out/" + ("a" * (ADAPTER.MAX_LOCAL_PATH_CHARS + 1))
        cases = (
            (
                "raw overlong",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                overlong_path,
                f"no longer than {ADAPTER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "output overlong",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                overlong_path,
                f"no longer than {ADAPTER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "message overlong",
                lambda raw: ADAPTER._validate_path_argument(raw, "--message path"),
                overlong_path,
                f"no longer than {ADAPTER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw encoded dot",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/receipts",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/receipts",
                "encoded dot or separator",
            ),
            (
                "raw uri prefix",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/receipts",
                "URI or drive prefixes",
            ),
            (
                "message drive prefix",
                lambda raw: ADAPTER._validate_path_argument(raw, "--message path"),
                "C:/inbox/rail-status.xml",
                "URI or drive prefixes",
            ),
            (
                "message encoded backslash",
                lambda raw: ADAPTER._validate_path_argument(raw, "--message path"),
                "nested/%5c/rail-status.xml",
                "encoded dot or separator",
            ),
            (
                "message encoded semicolon",
                lambda raw: ADAPTER._validate_path_argument(raw, "--message path"),
                "nested/%3b/rail-status.xml",
                "encoded semicolon",
            ),
            (
                "raw encoded delimiter",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%5b/receipts",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/receipts",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/receipts",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/receipts",
                "malformed percent",
            ),
        )
        for name, call, raw, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    call(raw)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(raw, message)

    def test_url_paths_reject_raw_delimiter_smuggling(self):
        cases = (
            "https://torii.local-bank.bank/base:debug/v1",
            "https://torii.local-bank.bank/base@debug/v1",
            "https://torii.local-bank.bank/base[debug]/v1",
        )
        for base_url in cases:
            with self.subTest(base_url=base_url):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._validate_base_url(base_url, allow_insecure_http=False)

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(base_url, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://torii\u0661.local-bank.bank/base/v1",
                "host must use printable ASCII",
            ),
            (
                "https://torii.local-bank.bank/base∕debug/v1",
                "path must use printable ASCII",
            ),
            (
                "https://torii.local-bank.bank/base%c3%a9/v1",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for base_url, expected in cases:
            with self.subTest(base_url=base_url):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._validate_base_url(base_url, allow_insecure_http=False)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(base_url, message)

    def test_url_cli_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--torii-base-url"],
            ["--torii-base-url", ""],
            ["--torii-base-url", "--receipt-dir"],
            ["--torii-base-url="],
            ["--torii-base-url=--receipt-dir"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("--torii-base-url requires a URL value", stderr)

    def test_message_cli_path_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--message"],
            ["--message", ""],
            ["--message", "--dry-run"],
            ["--message="],
            ["--message=--dry-run"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("--message requires a path value", stderr)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--dry-run=true"], "--dry-run", "--dry-run=true"),
            (["--allow-insecure-http", "true"], "--allow-insecure-http", "true"),
        )
        for argv, flag, rejected in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(f"{flag} does not take a value", stderr)
                self.assertNotIn(rejected, stderr)

    def test_numeric_cli_flags_reject_malformed_values_without_echo(self):
        cases = (
            ["--max-payload-bytes", "token=rail-secret"],
            ["--response-limit-bytes=token=rail-secret"],
            ["--timeout-secs", "--receipt-dir"],
            ["--timeout-secs="],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("rail-secret", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--max-payload-bytes", hidden],
            [f"--response-limit-bytes={hidden}"],
            ["--timeout-secs", f"{hidden}.5"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=rail-secret"],
            ["token=rail-secret"],
            ["password=rail-secret"],
            ["--torii-base-url", "token=rail-secret"],
            ["--torii-base-url=%70assword%253Drail-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("rail-secret", stderr)
                self.assertNotIn("inbox_dir", stderr)

    def test_url_cli_values_reject_non_ascii_without_echo(self):
        hidden = "https://torii.local-bank.bank/base\u2215debug"
        for argv in (["--torii-base-url", hidden], [f"--torii-base-url={hidden}"]):
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("--torii-base-url URL must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)
                self.assertNotIn("inbox_dir", stderr)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "rail-status.xml"
            path.write_bytes(SAMPLE_XML)

            for limit in (True, "64"):
                with self.subTest(helper="_read_regular_file", limit=limit):
                    with self.assertRaisesRegex(
                        ADAPTER.AdapterError,
                        "max file bytes must be a positive integer",
                    ):
                        ADAPTER._read_regular_file(path, max_bytes=limit)
                with self.subTest(helper="_bounded_read", limit=limit):
                    with self.assertRaisesRegex(
                        ADAPTER.AdapterError,
                        "max payload bytes must be a positive integer",
                    ):
                        ADAPTER._bounded_read(path, limit)

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
            self.assertNotIn(ADAPTER.sha256_hex(SAMPLE_XML), stderr)
            self.assertFalse((inbox / "receipts").exists())

    def test_all_zero_payload_digest_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, sidecar = write_message(inbox)
            sidecar["payload_sha256"] = "0" * 64
            xml_path.with_suffix(xml_path.suffix + ".json").write_text(
                json.dumps(sidecar),
                encoding="utf-8",
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
            self.assertIn("payload_sha256 must not be all zero", stderr)
            self.assertNotIn(ADAPTER.sha256_hex(SAMPLE_XML), stderr)
            self.assertFalse((inbox / "receipts").exists())

    def test_checked_in_xml_fixture_path_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "fixtures" / "iso20022" / "rail-inbox"
            receipt_dir = root / "rail-receipts"
            inbox.mkdir(parents=True)
            write_message(inbox)
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
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
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn(
                "inbox_dir must not point to checked-in ISO fixture artifacts",
                stderr,
            )
            self.assertFalse((inbox / "receipts").exists())
            self.assertFalse(receipt_dir.exists())

    def test_duplicate_rail_message_ids_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_named_message(
                inbox,
                "first",
                payload=b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>first</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>",
                rail_message_id="duplicate-rail-id",
            )
            write_named_message(
                inbox,
                "second",
                payload=b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>second</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>",
                rail_message_id="duplicate-rail-id",
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
            self.assertNotIn("duplicate-rail-id", stderr)
            self.assertFalse((inbox / "receipts").exists())

    def test_secret_material_in_sidecar_fields_is_rejected_without_echo(self):
        cases = (
            ("message_type", "token=rail-sidecar-secret"),
            ("message_type", "token%253Drail-sidecar-secret"),
            ("message_type", "token-rail-message-type-secret"),
            ("profile", "private-key=rail-sidecar-secret"),
            ("profile", "token-rail-profile-secret"),
            ("payload_sha256", "client_secret=rail-sidecar-secret"),
            ("payload_sha256", "token-rail-payload-secret"),
            ("rail_message_id", "password=rail-sidecar-secret"),
            ("rail_message_id", "session-key-rail-message-secret"),
        )
        for field, value in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    xml_path, sidecar = write_message(inbox)
                    sidecar[field] = value
                    xml_path.with_suffix(".xml.json").write_text(
                        json.dumps(sidecar),
                        encoding="utf-8",
                    )

                    rc, stdout, stderr = run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            "https://torii.local-bank.bank",
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("secret-looking", stderr)
                    self.assertNotIn(value, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("password=", stderr)
                    self.assertNotIn("private-key=", stderr)
                    self.assertNotIn("client_secret=", stderr)
                    self.assertNotIn("rail-sidecar-secret", stderr)

    def test_non_ascii_sidecar_message_type_is_rejected_without_echo(self):
        hidden = "\u00e9"
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, sidecar = write_message(inbox)
            sidecar["message_type"] = f"pacs.00{hidden}"
            xml_path.with_suffix(".xml.json").write_text(
                json.dumps(sidecar),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_main(
                [
                    "--inbox-dir",
                    str(inbox),
                    "--torii-base-url",
                    "https://torii.local-bank.bank",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("message_type must use printable ASCII", stderr)
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("unsupported message_type", stderr)

    def test_malformed_sidecar_message_type_is_rejected_without_echo(self):
        hidden = "pacs.002" + ("x" * 256)
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, sidecar = write_message(inbox)
            sidecar["message_type"] = hidden
            xml_path.with_suffix(".xml.json").write_text(
                json.dumps(sidecar),
                encoding="utf-8",
            )

            rc, stdout, stderr = run_main(
                [
                    "--inbox-dir",
                    str(inbox),
                    "--torii-base-url",
                    "https://torii.local-bank.bank",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("message_type must be lowercase ISO family id", stderr)
            self.assertNotIn(hidden, stderr)
            self.assertNotIn("unsupported message_type", stderr)

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

    def test_bearer_token_file_errors_do_not_echo_runtime_path(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            secret_dir = Path(raw_inbox) / "private_key=rail-secret"
            secret_dir.mkdir()
            cases = [
                (
                    "missing",
                    secret_dir / "token=rail-secret-missing.txt",
                    None,
                    "does not exist",
                ),
                ("empty", secret_dir / "token=rail-secret-empty.txt", b"", "empty"),
                (
                    "non-utf8",
                    secret_dir / "token=rail-secret-nonutf8.txt",
                    b"rail-token\xff",
                    "not UTF-8",
                ),
                (
                    "oversized",
                    secret_dir / "token=rail-secret-oversized.txt",
                    b"a" * (ADAPTER.MAX_BEARER_TOKEN_BYTES + 1),
                    "exceeds",
                ),
            ]
            for name, token_file, token_bytes, message in cases:
                with self.subTest(name=name):
                    if token_bytes is not None:
                        token_file.write_bytes(token_bytes)
                    with self.assertRaises(ADAPTER.AdapterError) as raised:
                        ADAPTER._load_bearer_token(token_file)

                    error = str(raised.exception)
                    self.assertIn("bearer token file", error)
                    self.assertIn(message, error)
                    self.assertNotIn("private_key=rail-secret", error)
                    self.assertNotIn("token=rail-secret", error)
                    self.assertNotIn(str(token_file), error)

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
            (
                "token secret-looking",
                "--bearer-token-file",
                "token=rail-secret",
                "secret-looking material",
            ),
            (
                "inbox secret-looking",
                "--inbox-dir",
                "token=rail-secret",
                "secret-looking material",
            ),
            (
                "receipt secret-looking",
                "--receipt-dir",
                "private_key=rail-secret",
                "secret-looking material",
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
                    if "secret-looking" in name:
                        self.assertNotIn("rail-secret", stderr)

    def test_direct_run_paths_reject_smuggling_before_inbox_loading(self):
        def args_for(root, **overrides):
            values = {
                "inbox_dir": root / "missing-inbox",
                "message": None,
                "torii_base_url": "https://torii.example.invalid",
                "receipt_dir": root / "receipts",
                "bearer_token_file": None,
                "timeout_secs": 1.0,
                "response_limit_bytes": 1024,
                "max_payload_bytes": 1024,
                "allow_insecure_http": False,
                "allow_default_profile": False,
                "allow_legacy_colr007": False,
                "dry_run": True,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "inbox whitespace",
                lambda root: args_for(root, inbox_dir=root / "inbox dir"),
                "inbox_dir must not contain whitespace",
            ),
            (
                "inbox repository fixture",
                lambda root: args_for(
                    root,
                    inbox_dir=root / "fixtures" / "iso20022" / "rail-inbox",
                ),
                "inbox_dir must not point to checked-in ISO fixture artifacts",
            ),
            (
                "message parent",
                lambda root: args_for(
                    root,
                    message=root / "nested" / ".." / "rail-status.xml",
                ),
                "message must not contain dot or parent segments",
            ),
            (
                "receipt leading dash",
                lambda root: args_for(
                    root,
                    receipt_dir=root / "nested" / "-receipts",
                ),
                "receipt_dir must not contain leading-dash path segments",
            ),
            (
                "receipt repository fixture",
                lambda root: args_for(
                    root,
                    receipt_dir=root / "fixtures" / "iso20022" / "rail-receipts",
                ),
                "receipt_dir must not point to checked-in ISO fixture artifacts",
            ),
            (
                "token secret",
                lambda root: args_for(
                    root,
                    bearer_token_file=root / "token=rail-secret",
                ),
                "bearer_token_file must not contain secret-looking material",
            ),
        )
        for name, make_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(make_args(root))

                    error = str(caught.exception)
                    self.assertIn(message, error)
                    self.assertNotIn("does not exist", error)
                    self.assertNotIn("rail-secret", error)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            with self.assertRaisesRegex(ADAPTER.AdapterError, "provide --inbox-dir"):
                ADAPTER.run(args_for(root, inbox_dir=None))

    def test_secret_looking_message_paths_are_rejected_before_receipt_output(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_named_message(inbox, "token=rail-secret")
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
            self.assertIn("message XML path must not contain secret-looking material", stderr)
            self.assertNotIn("token=rail-secret", stderr)
            self.assertFalse((inbox / "receipts").exists())

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

    def test_unused_local_overrides_are_rejected(self):
        cases = (
            (
                "--allow-insecure-http",
                "--allow-insecure-http requires an http:// or local/private Torii URL",
            ),
            (
                "--allow-default-profile",
                "--allow-default-profile requires at least one sidecar without profile",
            ),
            (
                "--allow-legacy-colr007",
                "--allow-legacy-colr007 requires at least one legacy colr.007 message",
            ),
        )
        for flag, message in cases:
            with self.subTest(flag=flag):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    write_message(inbox)

                    rc, stdout, stderr = run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            "https://torii.bank.internal/iso",
                            "--dry-run",
                            flag,
                        ]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)

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
                    '{"message_type":"pacs.002",'
                    '"token=rail-duplicate-key-secret":1,'
                    '"token=rail-duplicate-key-secret":2,'
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
            self.assertNotIn("rail-duplicate-key-secret", stderr)

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
            ("https://torii.example", False),
            ("https://torii.example.com", False),
            ("https://torii.example.net/base", False),
            ("https://torii.example.org/base", False),
            ("https://torii.example.invalid/base", False),
            ("https://torii.swift-cbpr-plus.operator-canary.bank/base", False),
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
            ("http://torii.example.invalid", True),
            ("http://torii.swift-cbpr-plus.operator-canary.bank", True),
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

    def test_rejected_torii_url_does_not_echo_secret_path(self):
        cases = (
            "https://torii.example/base/token=rail-path-secret",
            "https://torii.example/base/token-rail-path-secret",
            "https://torii.example/base/token%3Drail-path-secret",
            "https://torii.example/base/token%253Drail-path-secret",
        )
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            for secret_url in cases:
                with self.subTest(secret_url=secret_url):
                    rc, _stdout, stderr = run_main(
                        ["--inbox-dir", str(inbox), "--torii-base-url", secret_url]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("rail-path-secret", stderr)

    def test_rejected_torii_url_does_not_echo_secret_port(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            secret_url = "https://torii.example:token-rail-port-secret/base"

            rc, _stdout, stderr = run_main(
                ["--inbox-dir", str(inbox), "--torii-base-url", secret_url]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid port", stderr)
            self.assertNotIn(secret_url, stderr)
            self.assertNotIn("token-rail-port-secret", stderr)

    def test_rejected_torii_url_does_not_echo_secret_host_or_parser_error(self):
        cases = (
            ("https://token-rail-host-secret.torii.example/base", "secret-looking material"),
            ("https://[token-rail-host-secret/base", "is not a valid URL"),
        )
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            for secret_url, message in cases:
                with self.subTest(secret_url=secret_url):
                    rc, _stdout, stderr = run_main(
                        ["--inbox-dir", str(inbox), "--torii-base-url", secret_url]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret_url, stderr)
                    self.assertNotIn("token-rail-host-secret", stderr)

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

    def test_http_status_code_bounds_are_exact(self):
        cases = (
            (99, False),
            (100, True),
            (599, True),
            (600, False),
        )
        for status_code, expected in cases:
            with self.subTest(status_code=status_code):
                self.assertEqual(ADAPTER._is_http_status_code(status_code), expected)

        result = ADAPTER._invalid_http_status_result(600)

        self.assertIsNone(result.status_code)
        self.assertFalse(result.ok)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status 600")

    def test_invalid_torii_status_writes_transport_failed_receipt(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            _xml_path, sidecar = write_message(inbox)
            with capture_server(status=700, body=b"non-standard") as (
                base_url,
                requests,
            ):
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
            self.assertIsNone(receipt["status_code"])
            self.assertEqual(receipt["payload_sha256"], sidecar["payload_sha256"])
            self.assertIsNone(receipt["response_body_sha256"])
            self.assertIsNone(receipt["response_body_preview"])
            self.assertEqual(receipt["error"], "invalid HTTP status 700")
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
        cases = (
            (b'{"error":"token=rail-secret"}', "rail-secret"),
            (b'{"error":"password=rail-secret"}', "rail-secret"),
            (b'{"error":"%70assword%253Drail-secret"}', "rail-secret"),
            (b'{"error":"private-key=rail-secret"}', "rail-secret"),
            (b'{"error":"Set-Cookie: rail-secret"}', "rail-secret"),
            (
                b'{"error":"token-rail-response-secret"}',
                "rail-response-secret",
            ),
        )
        for body, hidden in cases:
            with self.subTest(body=body):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    _xml_path, _sidecar = write_message(inbox)
                    with capture_server(status=500, body=body) as (
                        base_url,
                        requests,
                    ):
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
                    receipt_text = receipts[0].read_text(encoding="utf-8")
                    receipt = json.loads(receipt_text)
                    self.assertEqual(receipt["status_code"], 500)
                    self.assertEqual(
                        receipt["response_body_sha256"], ADAPTER.sha256_hex(body)
                    )
                    self.assertEqual(
                        receipt["response_body_preview"],
                        ADAPTER.REDACTED_RESPONSE_PREVIEW,
                    )
                    self.assertNotIn(hidden, receipt_text)
                    self.assertTrue(receipt_digest_matches(receipt))

    def test_control_character_torii_response_preview_is_redacted(self):
        cases = (
            (b'{"error":"\x1b[31mrail-warning"}', "[31mrail-warning"),
            (b'{"error":"rail\x00warning"}', "rail\\u0000warning"),
        )
        for body, hidden in cases:
            with self.subTest(body=body), tempfile.TemporaryDirectory() as raw_inbox:
                inbox = Path(raw_inbox)
                write_message(inbox)
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
                receipt_text = receipts[0].read_text(encoding="utf-8")
                receipt = json.loads(receipt_text)
                self.assertEqual(receipt["status_code"], 500)
                self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(body))
                self.assertEqual(
                    receipt["response_body_preview"],
                    ADAPTER.REDACTED_RESPONSE_PREVIEW,
                )
                self.assertNotIn(hidden, receipt_text)
                self.assertTrue(receipt_digest_matches(receipt))

    def test_secret_looking_success_response_fails_before_receipt_write(self):
        cases = (
            (b'{"message_id":"private_key=rail-secret"}', "private_key"),
            (b'{"message_id":"token-rail-response-secret"}', "token-rail"),
        )
        for body, marker in cases:
            with self.subTest(body=body):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    write_message(inbox)
                    with capture_server(status=202, body=body) as (base_url, requests):
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
                    self.assertEqual(len(requests), 1)
                    self.assertIn(
                        "Torii response body contains secret-looking material",
                        stderr,
                    )
                    self.assertNotIn(marker, stderr)
                    self.assertEqual(list((inbox / "receipts").glob("*.receipt.json")), [])

    def test_control_character_success_response_fails_before_receipt_write(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            body = b'{"message_id":"\x1b[31mrail-success"}'
            with capture_server(status=202, body=body) as (base_url, requests):
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
            self.assertEqual(len(requests), 1)
            self.assertIn("Torii response body contains unsafe control characters", stderr)
            self.assertNotIn("[31mrail-success", stderr)
            self.assertEqual(list((inbox / "receipts").glob("*.receipt.json")), [])

    def test_secret_looking_url_error_is_redacted(self):
        self.assertEqual(
            ADAPTER._receipt_error("upstream token=rail-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream password=rail-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream %70assword%253Drail-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream private-key=rail-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream token-rail-url-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream \x1b[31mrail-warning"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(ADAPTER._receipt_error("connection refused"), "connection refused")


if __name__ == "__main__":
    unittest.main()
