import argparse
import array
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
SCRIPT_PATH = REPO_ROOT / "scripts" / "iso_audit_notary_adapter.py"
SPEC = importlib.util.spec_from_file_location("iso_audit_notary_adapter", SCRIPT_PATH)
ADAPTER = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = ADAPTER
SPEC.loader.exec_module(ADAPTER)


def with_digest(obj, digest_field):
    obj = dict(obj)
    obj.pop(digest_field, None)
    obj[digest_field] = ADAPTER.sha256_hex(ADAPTER._canonical_json_bytes(obj))
    return obj


CONTEXT_KEYS = [
    "ledger_id",
    "source_account_id",
    "source_account_address",
    "target_account_id",
    "target_account_address",
    "asset_definition_id",
    "asset_id",
    "settlement_amount",
    "settlement_currency",
    "settlement_date",
    "settlement_quantity",
    "settlement_movement_type",
    "settlement_payment_type",
    "security_instrument_id",
    "collateral_obligation_id",
    "collateral_original_amount",
    "collateral_original_currency",
    "collateral_original_instrument_id",
    "collateral_substitute_amount",
    "collateral_substitute_currency",
    "collateral_substitute_instrument_id",
    "collateral_effective_date",
    "collateral_substitution_type",
    "collateral_haircut",
    "collateral_reason_code",
    "plan_execution_order",
    "plan_atomicity",
]


def sample_persisted_record(index_record):
    root = {
        "version": ADAPTER.PERSISTED_RECORD_VERSION,
        "message_id": index_record["message_id"],
        "state": index_record["state"],
        "updated_at_ms": index_record["updated_at_ms"],
        "transaction_hash": index_record["transaction_hash"],
        "detail": None,
        "ledger_tx_queued": index_record["state"] == "Accepted",
        "settled_at_ms": index_record["settled_at_ms"],
        "hold_reason_code": None,
        "change_reason_codes": [],
        "rejection_reason_code": None,
        "context": {key: None for key in CONTEXT_KEYS},
        "metadata": {
            "profile_id": index_record["profile_id"],
            "message_type": index_record["message_type"],
            "business_service": None,
            "business_message_id": index_record["business_message_id"],
            "uetr": index_record["uetr"],
            "payload_hash": index_record["payload_hash"],
            "reference_snapshot_id": index_record["reference_snapshot_id"],
            "embedded_signature_detected": False,
        },
        "status_history": [
            {
                "status": index_record["state"],
                "pacs002_code": index_record["pacs002_code"],
                "updated_at_ms": index_record["updated_at_ms"],
                "detail": None,
                "reason_code": None,
            }
        ],
    }
    return with_digest(root, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)


def sample_record(message_id="msg-1"):
    record = {
        "message_id": message_id,
        "filename": f"{ADAPTER.sha256_hex(message_id.encode('utf-8'))}.json",
        "record_sha256": "",
        "state": "Accepted",
        "pacs002_code": "ACSP",
        "updated_at_ms": 1_717_478_400_000,
        "settled_at_ms": None,
        "transaction_hash": "c" * 64,
        "profile_id": "swift-cbpr-plus",
        "message_type": "pacs.008",
        "business_message_id": f"{message_id}-biz",
        "uetr": None,
        "payload_hash": "b" * 64,
        "reference_snapshot_id": "snapshot",
    }
    record["record_sha256"] = sample_persisted_record(record)[
        ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
    ]
    return record


def sample_index():
    root = {
        "version": 1,
        "record_count": 1,
        "records": [sample_record()],
    }
    return with_digest(root, ADAPTER.INDEX_DIGEST_FIELD)


def sample_anchor(index, store_dir=None):
    root = {
        "version": ADAPTER.ANCHOR_VERSION,
        "index_sha256": index[ADAPTER.INDEX_DIGEST_FIELD],
        "record_count": index["record_count"],
        "store_dir": None if store_dir is None else str(store_dir),
        "audit_index": index,
    }
    return with_digest(root, ADAPTER.ANCHOR_DIGEST_FIELD)


def write_record_sources(store_dir, records):
    messages_dir = store_dir / ADAPTER.RECORDS_DIR
    messages_dir.mkdir(parents=True, exist_ok=True)
    for record in records:
        source = sample_persisted_record(record)
        (messages_dir / record["filename"]).write_text(
            json.dumps(source, indent=2) + "\n",
            encoding="utf-8",
        )


def write_export(
    export_dir,
    index=None,
    anchor=None,
    store_dir=None,
    write_record_sources_flag=True,
):
    index = index or sample_index()
    if write_record_sources_flag:
        store_dir = store_dir or export_dir / "store"
        write_record_sources(store_dir, index["records"])
    anchor = anchor or sample_anchor(index, store_dir=store_dir)
    index_sha256 = index[ADAPTER.INDEX_DIGEST_FIELD]
    anchors_dir = export_dir / ADAPTER.ANCHOR_DIR
    anchors_dir.mkdir(parents=True, exist_ok=True)
    (export_dir / ADAPTER.INDEX_FILE).write_text(
        json.dumps(index, indent=2) + "\n", encoding="utf-8"
    )
    anchor_text = json.dumps(anchor, indent=2) + "\n"
    (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(anchor_text, encoding="utf-8")
    digest_anchor = anchors_dir / f"{index_sha256}.notary.json"
    digest_anchor.write_text(anchor_text, encoding="utf-8")
    return index, anchor, digest_anchor


def run_main(argv):
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        rc = ADAPTER.main(argv)
    return rc, stdout.getvalue(), stderr.getvalue()


@contextlib.contextmanager
def capture_server(status=200, body=b'{"receipt":"ok"}'):
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
        yield f"http://127.0.0.1:{server.server_address[1]}/notary", requests
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
        yield f"http://127.0.0.1:{server.server_address[1]}/notary", requests
    finally:
        server.shutdown()
        thread.join(timeout=5)
        server.server_close()


class IsoAuditNotaryAdapterTest(unittest.TestCase):
    def test_os_error_detail_redacts_unsafe_strerror_without_echo(self):
        self.assertEqual(
            ADAPTER._safe_os_error_detail(OSError(5, "Permission denied")),
            "Permission denied",
        )
        unsafe_values = (
            "token=/tmp/notary-hidden-secret",
            "open /tmp/notary-hidden-path",
            "bad\ncontrol",
            "nonascii-\u2603",
            "x" * 129,
        )
        for value in unsafe_values:
            with self.subTest(value=value):
                detail = ADAPTER._safe_os_error_detail(OSError(5, value))
                self.assertEqual(detail, "I/O error")
                self.assertNotIn("notary-hidden", detail)

    def test_canonical_json_bytes_rejects_non_finite_numbers(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    ADAPTER._canonical_json_bytes({"value": value})

    def test_json_float_parser_rejects_overflow_and_negative_zero(self):
        for value in ("1e9999", "-1e9999", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._parse_canonical_json_float(value)
                self.assertNotIn(value, str(caught.exception))

    def test_text_output_symlink_ancestor_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target = root / "target"
            target.mkdir()
            hidden = "hidden-audit-output-link"
            link = root / hidden
            try:
                link.symlink_to(target, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            with self.assertRaises(ADAPTER.AdapterError) as caught:
                ADAPTER._write_text_output(
                    link / "receipt.json",
                    "{}\n",
                    display_label="receipt output",
                )

            message = str(caught.exception)
            self.assertIn("receipt output", message)
            self.assertIn("must not be a symlink", message)
            self.assertNotIn(str(link), message)
            self.assertNotIn(hidden, message)

    def test_persisted_record_sources_require_records_array(self):
        with self.assertRaisesRegex(
            ADAPTER.AdapterError,
            "anchor.records must be an array",
        ):
            ADAPTER._verify_persisted_record_sources(
                {},
                None,
                "anchor",
                allow_missing_record_sources=False,
            )

    def test_audit_json_arrays_are_count_bounded_without_echo(self):
        items = [None] * (ADAPTER.MAX_JSON_LIST_ITEMS + 1)
        cases = (
            (
                "helper",
                lambda: ADAPTER._require_json_array(items, "audit.records"),
                f"audit.records must contain at most {ADAPTER.MAX_JSON_LIST_ITEMS} items",
            ),
            (
                "record sources",
                lambda: ADAPTER._verify_persisted_record_sources(
                    {"records": items},
                    None,
                    "anchor",
                    allow_missing_record_sources=False,
                ),
                f"anchor.records must contain at most {ADAPTER.MAX_JSON_LIST_ITEMS} items",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    action()

                error = str(caught.exception)
                self.assertIn(expected, error)
                self.assertNotIn(str(len(items)), error)
                self.assertNotIn("[0]", error)

    def test_recursive_json_array_scans_are_count_bounded_without_echo(self):
        items = [None] * (ADAPTER.MAX_JSON_LIST_ITEMS + 1)

        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._reject_json_surrogates(items)

        error = str(caught.exception)
        self.assertIn(
            f"JSON array must contain at most {ADAPTER.MAX_JSON_LIST_ITEMS} items",
            error,
        )
        self.assertNotIn(str(len(items)), error)
        self.assertNotIn("[0]", error)

    def test_recursive_json_object_scans_are_count_bounded_without_echo(self):
        members = {
            f"hidden_key_{offset}": None
            for offset in range(ADAPTER.MAX_JSON_OBJECT_MEMBERS + 1)
        }
        pairs = list(members.items())
        cases = (
            (
                "json hook",
                lambda: ADAPTER._reject_duplicate_json_keys(pairs),
                f"JSON object must contain at most {ADAPTER.MAX_JSON_OBJECT_MEMBERS} members",
            ),
            (
                "surrogates",
                lambda: ADAPTER._reject_json_surrogates(members),
                f"JSON object must contain at most {ADAPTER.MAX_JSON_OBJECT_MEMBERS} members",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    action()

                error = str(caught.exception)
                self.assertIn(expected, error)
                self.assertNotIn(str(len(members)), error)
                self.assertNotIn("hidden_key_0", error)

    def test_recursive_json_depth_scans_are_bounded_without_echo(self):
        nested = "hidden_leaf"
        for _ in range(ADAPTER.MAX_JSON_NESTING_DEPTH + 1):
            nested = [nested]

        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._reject_json_surrogates(nested)

        error = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {ADAPTER.MAX_JSON_NESTING_DEPTH} levels",
            error,
        )
        self.assertNotIn("hidden_leaf", error)
        self.assertNotIn("[0]", error)

    def test_json_parse_recursion_error_is_bounded_without_echo(self):
        hidden = "hidden-audit-recursion"
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / hidden
            path.write_text("[]\n", encoding="utf-8")
            original_loads = ADAPTER.json.loads

            def raising_loads(*_args, **_kwargs):
                raise RecursionError(hidden)

            ADAPTER.json.loads = raising_loads
            try:
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._load_json(path, display_label="audit")
            finally:
                ADAPTER.json.loads = original_loads

        error = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {ADAPTER.MAX_JSON_NESTING_DEPTH} levels",
            error,
        )
        self.assertNotIn(hidden, error)
        self.assertNotIn(str(path), error)

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_audit_unknown_secret", "audit_unknown_secret"),
            ("%70assword_audit_unknown_leak", "audit_unknown_leak"),
            ("private-key_audit_unknown_leak", "audit_unknown_leak"),
            ("private--key_audit_unknown_leak", "audit_unknown_leak"),
            ("private%09key_audit_unknown_leak", "audit_unknown_leak"),
            ("x--iroha--signature_audit_unknown_leak", "audit_unknown_leak"),
            ("unexpected\x1baudit_key", "\x1b"),
            ("unexpected_audit_\uff4bey", "\uff4b"),
            ("operator_note", "operator_note"),
            ("x" * 129, "x" * 129),
        )
        for unknown_key, hidden in cases:
            with self.subTest(unknown_key=unknown_key):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._reject_unknown_keys({unknown_key: "redacted"}, set(), "anchor")

                message = str(caught.exception)
                self.assertIn("contains unknown keys", message)
                self.assertNotIn("password", message)
                self.assertNotIn(unknown_key, message)
                self.assertNotIn(hidden, message)
        many_unknown = {f"field_{offset}": "redacted" for offset in range(9)}
        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._reject_unknown_keys(many_unknown, set(), "anchor")
        message = str(caught.exception)
        self.assertIn("contains unknown keys", message)
        self.assertNotIn("field_0", message)
        self.assertNotIn("field_8", message)

    def test_separator_smuggled_secret_identifiers_are_detected(self):
        cases = (
            "private\tkey audit identifier",
            "private--key audit identifier",
            "private/key audit identifier",
            "private\\key audit identifier",
            "private%2fkey audit identifier",
            "private\u200dkey audit identifier",
            "private\u0301key audit identifier",
            "ｐｒｉｖａｔｅｋｅｙ audit identifier",
            "x--iroha--signature audit identifier",
            "x/iroha/signature audit identifier",
            "x%2firoha%2fsignature audit identifier",
            "x\u200diroha\u200dsignature audit identifier",
            "x\u0301iroha\u0301signature audit identifier",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ audit identifier",
            "token%09secret audit identifier",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(ADAPTER._contains_secret_identifier_material(value))
        for key in (
            "private/key",
            "private%2fkey",
            "private\u0301key",
            "ｐｒｉｖａｔｅｋｅｙ",
            "x/iroha/signature",
            "x%2firoha%2fsignature",
            "x\u0301iroha\u0301signature",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ",
        ):
            with self.subTest(key=key):
                self.assertTrue(ADAPTER._is_secret_looking_key(key))

    def test_separator_smuggled_response_preview_is_secret(self):
        cases = (
            "upstream private\tkey audit leak",
            "upstream private/key audit leak",
            "upstream private%2fkey audit leak",
            "upstream private\u200dkey audit leak",
            "upstream private\u0301key audit leak",
            "upstream ｐｒｉｖａｔｅｋｅｙ audit leak",
            "upstream x--iroha--signature audit leak",
            "upstream x/iroha/signature audit leak",
            "upstream x%2firoha%2fsignature audit leak",
            "upstream x\u200diroha\u200dsignature audit leak",
            "upstream x\u0301iroha\u0301signature audit leak",
            "upstream ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ audit leak",
            "upstream token%09secret audit leak",
        )
        for preview in cases:
            with self.subTest(preview=preview):
                self.assertTrue(ADAPTER._response_preview_looks_secret(preview))
                self.assertEqual(
                    ADAPTER.REDACTED_RESPONSE_PREVIEW,
                    ADAPTER._response_preview(preview.encode("utf-8")),
                )

    def test_unicode_format_response_preview_is_redacted_without_echo(self):
        preview = "upstream audit \u202eaudit-bidi-leak"

        self.assertTrue(ADAPTER._contains_unsafe_preview_control(preview))
        self.assertEqual(
            ADAPTER.REDACTED_RESPONSE_PREVIEW,
            ADAPTER._response_preview(preview.encode("utf-8")),
        )
        self.assertNotIn(
            "audit-bidi-leak",
            ADAPTER._response_preview(preview.encode("utf-8")),
        )

    def test_non_ascii_response_preview_is_redacted_without_echo(self):
        cases = (
            ("unicode", "upstream audit caf\u00e9 hidden-audit-unicode".encode("utf-8")),
            ("invalid-utf8", b"upstream audit \xff hidden-audit-binary"),
        )
        for name, body in cases:
            with self.subTest(name=name):
                self.assertEqual(
                    ADAPTER.REDACTED_RESPONSE_PREVIEW,
                    ADAPTER._response_preview(body),
                )
                self.assertNotIn("hidden-audit", ADAPTER._response_preview(body))

    def test_path_separator_secret_key_values_are_detected(self):
        cases = (
            "private/key=audit-value-secret",
            "api/key:audit-value-secret",
            "client/secret=audit-value-secret",
            "set/cookie:audit-value-secret",
            "x/iroha/signature: audit-value-secret",
            "private%2fkey=audit-value-secret",
            "private\u200dkey=audit-value-secret",
            "private\u0301key=audit-value-secret",
            "ｐｒｉｖａｔｅｋｅｙ=audit-compat-secret",
            "ａｐｉ／ｋｅｙ:audit-compat-secret",
            "x\u200diroha\u200dsignature: audit-value-secret",
            "x\u0301iroha\u0301signature: audit-value-secret",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ: audit-compat-secret",
            "private%E2%80%8Dkey=audit-value-secret",
            "private%CC%81key=audit-value-secret",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(ADAPTER._contains_secret_material(value))

    def test_cli_argument_terminator_is_rejected_without_echo(self):
        hidden = "token=audit-terminator-secret"
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
                    ["--", "--endpoint", hidden],
                    {"--endpoint"},
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
                self.assertNotIn("audit-terminator-secret", message)

    def test_parser_rejects_abbreviated_long_options(self):
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as caught:
                ADAPTER.build_parser().parse_args(
                    ["--export-dir", ".", "--receipt-di", "out"]
                )

        self.assertEqual(caught.exception.code, 2)
        self.assertIn("unrecognized arguments", stderr.getvalue())
        self.assertIn("--receipt-di", stderr.getvalue())

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        cases = ("--unknown-audit\x1bflag", "--unknown-audit\u202eflag")
        for hidden in cases:
            with self.subTest(hidden=hidden):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

                message = str(caught.exception)
                self.assertIn("CLI argument must not contain control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn("\u202e", message)
                self.assertNotIn("unknown-audit", message)

    def test_raw_cli_non_ascii_is_rejected_without_echo(self):
        hidden = "\uff0d\uff0dreceipt-dir"
        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

        message = str(caught.exception)
        self.assertIn("CLI argument must use printable ASCII", message)
        self.assertNotIn(hidden, message)
        self.assertNotIn("receipt-dir", message)

    def test_audit_index_secret_looking_identifiers_are_rejected_without_echo(self):
        cases = (
            (
                "message_id",
                lambda record: record.update({"message_id": "token-notary-index-secret"}),
                "token-notary-index-secret",
            ),
            (
                "profile_id",
                lambda record: record.update({"profile_id": "private_key=notary-index-secret"}),
                "private_key=notary-index-secret",
            ),
            (
                "business_message_id",
                lambda record: record.update(
                    {"business_message_id": "%70assword%253Dnotary-index-secret"}
                ),
                "password=notary-index-secret",
            ),
            (
                "reference_snapshot_id",
                lambda record: record.update(
                    {"reference_snapshot_id": "x_iroha_signature=notary-index-secret"}
                ),
                "x_iroha_signature=notary-index-secret",
            ),
        )
        for name, mutate, hidden in cases:
            with self.subTest(name=name):
                index = sample_index()
                mutate(index["records"][0])
                index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)

                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER.verify_audit_index(index)

                message = str(caught.exception)
                self.assertIn("secret-looking material", message)
                self.assertNotIn("notary-index-secret", message)
                self.assertNotIn(hidden, message)

    def test_persisted_record_secret_values_are_rejected_without_echo(self):
        cases = (
            (
                "detail",
                lambda source: source.update({"detail": "token=notary-source-secret"}),
                "token=notary-source-secret",
            ),
            (
                "context",
                lambda source: source["context"].update(
                    {"source_account_id": "private-key=notary-source-secret"}
                ),
                "private-key=notary-source-secret",
            ),
            (
                "history-detail",
                lambda source: source["status_history"][0].update(
                    {"detail": "%70assword%253Dnotary-source-secret"}
                ),
                "password=notary-source-secret",
            ),
        )
        for name, mutate, hidden in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    record = sample_record()
                    source = sample_persisted_record(record)
                    mutate(source)
                    source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
                    record[ADAPTER.PERSISTED_RECORD_DIGEST_FIELD] = source[
                        ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
                    ]
                    record_path = root / record["filename"]
                    record_path.write_text(
                        json.dumps(source, indent=2) + "\n",
                        encoding="utf-8",
                    )

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._verify_persisted_record_source(
                            record,
                            record_path,
                            str(record_path),
                        )

                    message = str(caught.exception)
                    self.assertIn("secret-looking material", message)
                    self.assertNotIn("notary-source-secret", message)
                    self.assertNotIn(hidden, message)

    def test_persisted_record_unicode_format_controls_are_rejected_without_echo(self):
        cases = (
            (
                "detail",
                lambda source, value: source.update({"detail": value}),
                "record.detail",
            ),
            (
                "context",
                lambda source, value: source["context"].update(
                    {"source_account_id": value}
                ),
                "record.context.source_account_id",
            ),
            (
                "history-detail",
                lambda source, value: source["status_history"][0].update(
                    {"detail": value}
                ),
                "record.status_history[0].detail",
            ),
        )
        for name, mutate, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    hidden = f"notary-{name}-format-leak"
                    value = f"audit \u202e{hidden}"
                    record = sample_record()
                    source = sample_persisted_record(record)
                    mutate(source, value)
                    source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
                    record[ADAPTER.PERSISTED_RECORD_DIGEST_FIELD] = source[
                        ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
                    ]
                    record_path = root / record["filename"]
                    record_path.write_text(
                        json.dumps(source, indent=2) + "\n",
                        encoding="utf-8",
                    )

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._verify_persisted_record_source(
                            record,
                            record_path,
                            "record",
                        )

                    message = str(caught.exception)
                    self.assertIn(f"{label} must not contain control characters", message)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn(value, message)

    def test_clean_string_helpers_reject_unicode_format_controls_without_echo(self):
        hidden = "notary-format-leak"
        value = f"audit \u202e{hidden}"
        cases = (
            (
                "helper",
                lambda: ADAPTER._require_clean_string(value, "record.detail"),
                "record.detail must not contain control characters",
            ),
            (
                "nonsecret-helper",
                lambda: ADAPTER._require_nonsecret_clean_string(
                    value,
                    "record.business_message_id",
                ),
                "record.business_message_id must not contain control characters",
            ),
        )
        for name, run, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(value, message)
                self.assertNotIn("\u202e", message)
                self.assertNotIn(hidden, message)

    def test_overlong_clean_metadata_strings_are_rejected_without_echo(self):
        overlong = "M" * (ADAPTER.MAX_CLEAN_STRING_CHARS + 1)
        cases = (
            (
                "helper",
                lambda: ADAPTER._require_clean_string(overlong, "record.detail"),
                "record.detail must be no longer than 4096 characters",
            ),
            (
                "nonsecret-helper",
                lambda: ADAPTER._require_nonsecret_clean_string(
                    overlong,
                    "record.business_message_id",
                ),
                "record.business_message_id must be no longer than 4096 characters",
            ),
            (
                "audit-index",
                lambda: self._verify_overlong_audit_index_metadata(overlong),
                "audit index records[0].business_message_id must be no longer than 4096 characters",
            ),
            (
                "record-source",
                lambda: self._verify_overlong_record_source_metadata(overlong),
                "record.detail must be no longer than 4096 characters",
            ),
        )
        for name, run, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    run()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(overlong, message)

    def _verify_overlong_audit_index_metadata(self, overlong):
        index = sample_index()
        index["records"][0]["business_message_id"] = overlong
        index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
        ADAPTER.verify_audit_index(index)

    def _verify_overlong_record_source_metadata(self, overlong):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            record = sample_record()
            source = sample_persisted_record(record)
            source["detail"] = overlong
            source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
            record[ADAPTER.PERSISTED_RECORD_DIGEST_FIELD] = source[
                ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            record_path = root / record["filename"]
            record_path.write_text(
                json.dumps(source, indent=2) + "\n",
                encoding="utf-8",
            )

            ADAPTER._verify_persisted_record_source(
                record,
                record_path,
                "record",
            )

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
            ("token=notary-path-leak.receipts", "token=notary-path-leak"),
            ("token%3Dnotary-path-leak.receipts", "token=notary-path-leak"),
            ("private%20key%3Dnotary-path-leak.receipts", "private key=notary-path-leak"),
            ("private%20key-notary-path-secret.receipts", "private key-notary-path-secret"),
            ("private/key-notary-path-secret.receipts", "private/key-notary-path-secret"),
            ("x%2firoha%2fsignature-notary-path-secret.receipts", "x/iroha/signature-notary-path-secret"),
            ("%70assword%253Dnotary-path-leak.receipts", "password=notary-path-leak"),
            ("token-notary-path-secret.receipts", "token-notary-path-secret"),
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
                self.assertNotIn("notary-path-leak", message)

    def test_receipt_outputs_reject_repository_fixture_artifacts(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "fixtures" / "iso20022" / "notary-receipts"
            receipt_path = root / "fixtures" / "iso20022" / "notary-receipt.json"

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

    def test_receipt_dir_rejects_repository_fixture_before_export_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            receipt_dir = root / "fixtures" / "iso20022" / "notary-receipts"

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(root / "missing-export"),
                    "--endpoint",
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

    def test_receipt_dir_cannot_reuse_export_dir_before_export_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "missing-export"

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--receipt-dir",
                    str(export_dir),
                    "--endpoint",
                    "https://notary.example.internal",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt_dir must not reuse export_dir path", stderr)
            self.assertNotIn("does not exist", stderr)

    def test_receipt_dir_cannot_symlink_to_export_dir_before_export_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            receipt_dir = root / "receipt-link"
            try:
                receipt_dir.symlink_to(export_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--receipt-dir",
                    str(receipt_dir),
                    "--endpoint",
                    "https://notary.example.internal",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt_dir must not reuse export_dir path", stderr)
            self.assertNotIn("latest.notary.json", stderr)

    def test_symlinked_receipt_dir_ancestor_is_rejected_before_export_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_dir = root / "receipt-target"
            target_dir.mkdir()
            ancestor = root / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            receipt_dir = ancestor / "nested" / "receipts"

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(root / "missing-export"),
                    "--receipt-dir",
                    str(receipt_dir),
                    "--endpoint",
                    "https://notary.example.internal",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt_dir must not be a symlink", stderr)
            self.assertNotIn("does not exist", stderr)
            self.assertFalse((target_dir / "nested").exists())

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
                "store overlong",
                lambda raw: ADAPTER._require_clean_path_string(raw, "anchor.store_dir"),
                overlong_path,
                f"no longer than {ADAPTER.MAX_LOCAL_PATH_CHARS} characters",
            ),
            (
                "raw encoded dot",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%2e/receipt",
                "encoded dot or separator",
            ),
            (
                "output encoded slash",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                "out/%2f/receipt",
                "encoded dot or separator",
            ),
            (
                "raw format control",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/\u202ereceipt",
                "control characters",
            ),
            (
                "output format control",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                "out/\u202ereceipt",
                "control characters",
            ),
            (
                "store format control",
                lambda raw: ADAPTER._require_clean_path_string(raw, "anchor.store_dir"),
                "/ops/\u202estore",
                "control characters",
            ),
            (
                "raw uri prefix",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "file:out/receipt",
                "URI or drive prefixes",
            ),
            (
                "output drive prefix",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                "C:/out/receipt",
                "URI or drive prefixes",
            ),
            (
                "store encoded semicolon",
                lambda raw: ADAPTER._require_clean_path_string(raw, "anchor.store_dir"),
                "/ops/%3b/store",
                "encoded semicolon",
            ),
            (
                "raw encoded delimiter",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%23/receipt",
                "encoded URL delimiter",
            ),
            (
                "raw encoded percent",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%25/receipt",
                "encoded percent",
            ),
            (
                "raw encoded space",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%20/receipt",
                "percent-encoded control or space",
            ),
            (
                "raw malformed percent",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/%zz/receipt",
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
            "https://notary.local-bank.bank/archive:debug/anchor",
            "https://notary.local-bank.bank/archive@debug/anchor",
            "https://notary.local-bank.bank/archive[debug]/anchor",
        )
        for endpoint in cases:
            with self.subTest(endpoint=endpoint):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._validate_endpoint(endpoint, allow_insecure_http=False)

                message = str(caught.exception)
                self.assertIn("path must not contain URL delimiter characters", message)
                self.assertNotIn(endpoint, message)

    def test_urls_reject_non_ascii_smuggling(self):
        cases = (
            (
                "https://notary\u0661.local-bank.bank/archive/anchor",
                "host must use printable ASCII",
            ),
            (
                "https://notary.local-bank.bank/archive∕debug/anchor",
                "path must use printable ASCII",
            ),
            (
                "https://notary.local-bank.bank/archive\u202edebug/anchor",
                "endpoint must not contain control characters",
            ),
            (
                "https://notary.local-bank.bank/archive%c3%a9/anchor",
                "path must not contain percent-encoded non-ASCII bytes",
            ),
        )
        for endpoint, expected in cases:
            with self.subTest(endpoint=endpoint):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._validate_endpoint(endpoint, allow_insecure_http=False)

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(endpoint, message)

    def test_boolean_cli_flags_reject_values_without_echo(self):
        cases = (
            (["--all=true"], "--all", "--all=true"),
            (
                ["--allow-missing-record-sources", "true"],
                "--allow-missing-record-sources",
                "true",
            ),
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
            ["--response-limit-bytes", "token=notary-secret"],
            ["--response-limit-bytes=token=notary-secret"],
            ["--timeout-secs", "--receipt-dir"],
            ["--timeout-secs="],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("numeric value", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("notary-secret", stderr)

    def test_numeric_cli_flags_reject_unicode_digits_without_echo(self):
        hidden = "\u0661"
        cases = (
            ["--response-limit-bytes", hidden],
            [f"--response-limit-bytes={hidden}"],
            ["--timeout-secs", f"{hidden}.5"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("must use printable ASCII", stderr)
                self.assertNotIn(hidden, stderr)

    def test_numeric_cli_flags_reject_noncanonical_decimal_spellings_before_network(self):
        cases = (
            ["--response-limit-bytes", "000512"],
            ["--response-limit-bytes", "+512"],
            ["--response-limit-bytes=001024"],
            ["--response-limit-bytes", "-0"],
            ["--timeout-secs", ".5"],
            ["--timeout-secs", "01"],
            ["--timeout-secs", "1e01"],
            ["--timeout-secs", "1."],
            ["--timeout-secs", "+1"],
            ["--timeout-secs", "-0"],
            ["--timeout-secs", "-0.0"],
            ["--timeout-secs", "-0e0"],
            ["--timeout-secs", "1e9999"],
            ["--timeout-secs", "-1e9999"],
            ["--timeout-secs=-0e0"],
            ["--timeout-secs=1e9999"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn("numeric value", stderr)

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=notary-secret"],
            ["token=notary-secret"],
            ["password=notary-secret"],
            ["--endpoint", "token=notary-secret"],
            ["--endpoint=%70assword%253Dnotary-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("notary-secret", stderr)
                self.assertNotIn("export_dir", stderr)

    def test_url_cli_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--endpoint"],
            ["--endpoint", ""],
            ["--endpoint", "--receipt-dir"],
            ["--endpoint", "-receipt-dir"],
            ["--endpoint="],
            ["--endpoint=--receipt-dir"],
            ["--endpoint=-receipt-dir"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("--endpoint requires a URL value", stderr)

    def test_url_cli_values_reject_non_ascii_without_echo(self):
        cases = (
            (
                "https://notary.local-bank.bank/source\u2215debug",
                "--endpoint URL must use printable ASCII",
            ),
            (
                "https://notary.local-bank.bank/source\u202edebug",
                "--endpoint URL must not contain control characters",
            ),
        )
        for hidden, expected in cases:
            for argv in (["--endpoint", hidden], [f"--endpoint={hidden}"]):
                with self.subTest(argv=argv):
                    rc, _stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn("\u202e", stderr)
                    self.assertNotIn("export_dir", stderr)

    def test_boolean_and_non_integer_file_read_limits_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / "latest.notary.json"
            path.write_text("{}\n", encoding="utf-8")

            for limit in (True, "64"):
                with self.subTest(limit=limit):
                    with self.assertRaisesRegex(
                        ADAPTER.AdapterError,
                        "max file bytes must be a positive integer",
                    ):
                        ADAPTER._read_regular_file(path, max_bytes=limit)

    def test_publish_posts_verified_anchor_and_writes_receipt(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            index, anchor, _digest_anchor = write_export(export_dir)
            with capture_server() as (endpoint, requests):
                receipt_dir = export_dir / "receipts"
                receipt_dir.mkdir()
                receipt_path = receipt_dir / (
                    f"{index[ADAPTER.INDEX_DIGEST_FIELD]}."
                    f"{ADAPTER._endpoint_sha256(endpoint)}.receipt.json"
                )
                receipt_path.write_text('{"stale": true}\n' + ("x" * 4096), encoding="utf-8")
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 0)
            self.assertEqual(len(requests), 1)
            self.assertEqual(
                requests[0]["headers"]["X-Iroha-Iso-Index-Sha256"],
                index[ADAPTER.INDEX_DIGEST_FIELD],
            )
            self.assertEqual(
                requests[0]["headers"]["X-Iroha-Iso-Anchor-Sha256"],
                anchor[ADAPTER.ANCHOR_DIGEST_FIELD],
            )
            self.assertEqual(
                json.loads(requests[0]["body"].decode("utf-8")),
                anchor,
            )
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertTrue(receipt["ok"])
            self.assertEqual(receipt["status_code"], 200)
            self.assertEqual(
                ADAPTER.require_digest_matches(
                    receipt, ADAPTER.RECEIPT_DIGEST_FIELD, "receipt"
                ),
                receipt[ADAPTER.RECEIPT_DIGEST_FIELD],
            )
            self.assertEqual(receipts[0].stat().st_mode & 0o077, 0)
            self.assertEqual(list(receipt_dir.glob(".iso-*.tmp")), [])

    def test_zero_record_anchor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            empty_index = with_digest(
                {"version": 1, "record_count": 0, "records": []},
                ADAPTER.INDEX_DIGEST_FIELD,
            )
            write_export(
                export_dir,
                index=empty_index,
                store_dir=root / "store",
                write_record_sources_flag=True,
            )

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("record_count must be positive", stderr)

    def test_all_zero_index_digest_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            index = sample_index()
            actual_index_digest = index[ADAPTER.INDEX_DIGEST_FIELD]
            index[ADAPTER.INDEX_DIGEST_FIELD] = "0" * 64
            anchor = sample_anchor(index, store_dir=export_dir / "store")
            write_export(
                export_dir,
                index=index,
                anchor=anchor,
                store_dir=export_dir / "store",
            )

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("index_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_index_digest, stderr)
            self.assertNotIn("mismatch", stderr)
            self.assertFalse((export_dir / "receipts").exists())

    def test_all_zero_anchor_digest_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            index = sample_index()
            anchor = sample_anchor(index, store_dir=export_dir / "store")
            actual_anchor_digest = anchor[ADAPTER.ANCHOR_DIGEST_FIELD]
            anchor[ADAPTER.ANCHOR_DIGEST_FIELD] = "0" * 64
            write_export(
                export_dir,
                index=index,
                anchor=anchor,
                store_dir=export_dir / "store",
            )

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("anchor_sha256 must not be all zero", stderr)
            self.assertNotIn(actual_anchor_digest, stderr)
            self.assertNotIn("mismatch", stderr)
            self.assertFalse((export_dir / "receipts").exists())

    def test_digest_mismatch_diagnostics_do_not_echo_hashes(self):
        index = sample_index()
        expected_digest = index[ADAPTER.INDEX_DIGEST_FIELD]
        index["record_count"] += 1
        actual_digest = ADAPTER.digest_without_field(
            index,
            ADAPTER.INDEX_DIGEST_FIELD,
        )

        with self.assertRaises(ADAPTER.AdapterError) as caught:
            ADAPTER.require_digest_matches(
                index,
                ADAPTER.INDEX_DIGEST_FIELD,
                "audit index",
            )

        message = str(caught.exception)
        self.assertIn("index_sha256 mismatch", message)
        self.assertNotIn(expected_digest, message)
        self.assertNotIn(actual_digest, message)

    def test_checked_in_notary_fixture_paths_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            cases = (
                (
                    "anchor-path",
                    root / "fixtures" / "iso20022" / "notary-export",
                    root / "store",
                    "export_dir must not point to checked-in ISO fixture artifacts",
                ),
                (
                    "store-dir",
                    root / "notary-export",
                    root / "fixtures" / "iso20022" / "notary-store",
                    "store_dir must not point to checked-in ISO fixture artifacts",
                ),
            )
            for name, export_dir, store_dir, message in cases:
                with self.subTest(name=name):
                    receipt_dir = root / f"{name}-receipts"
                    export_dir.mkdir(parents=True, exist_ok=True)
                    write_export(
                        export_dir,
                        store_dir=store_dir,
                        write_record_sources_flag=True,
                    )
                    with capture_server() as (endpoint, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--receipt-dir",
                                str(receipt_dir),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)
                    self.assertFalse((export_dir / "receipts").exists())
                    self.assertFalse(receipt_dir.exists())

    def test_available_persisted_record_sources_are_verified_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            export_dir.mkdir()
            store_dir = root / "store"
            index, _anchor, _digest_anchor = write_export(
                export_dir,
                store_dir=store_dir,
                write_record_sources_flag=True,
            )

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--dry-run",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["record_count"], [index["record_count"]])

    def test_unused_missing_record_sources_override_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--dry-run",
                    "--allow-missing-record-sources",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-missing-record-sources requires at least one anchor with missing record sources",
                stderr,
            )

    def test_unused_missing_record_sources_override_rejects_before_delivery_and_receipts(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--allow-missing-record-sources",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-missing-record-sources requires at least one anchor with missing record sources",
                stderr,
            )
            self.assertEqual(requests, [])
            self.assertFalse((export_dir / "receipts").exists())

    def test_missing_persisted_record_sources_require_explicit_local_override(self):
        cases = [
            (
                "missing-store-dir-field",
                lambda export_dir, _root: write_export(
                    export_dir,
                    write_record_sources_flag=False,
                ),
                "store_dir is required to verify audit records",
            ),
            (
                "missing-store-dir-path",
                lambda export_dir, root: write_export(
                    export_dir,
                    store_dir=root / "missing-store",
                    write_record_sources_flag=False,
                ),
                "store_dir",
            ),
            (
                "missing-messages-dir",
                lambda export_dir, root: (
                    (root / "store").mkdir(),
                    write_export(
                        export_dir,
                        store_dir=root / "store",
                        write_record_sources_flag=False,
                    ),
                )[1],
                "store_dir/messages",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, arrange, expected in cases:
                with self.subTest(name=name):
                    export_dir = root / name / "export"
                    export_dir.mkdir(parents=True)
                    arrange(export_dir, root / name)

                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertEqual(requests, [])

                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--allow-missing-record-sources",
                            ]
                        )

                    self.assertEqual(rc, 0, stderr)
                    self.assertEqual(len(requests), 1)

    def test_missing_record_file_override_is_used_for_digest_addressed_record(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            store_dir = root / "store"
            export_dir.mkdir()
            index, _anchor, _digest_anchor = write_export(
                export_dir,
                store_dir=store_dir,
                write_record_sources_flag=True,
            )
            record_path = store_dir / ADAPTER.RECORDS_DIR / index["records"][0]["filename"]
            record_path.unlink()

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--dry-run",
                    "--allow-missing-record-sources",
                ]
            )

            self.assertEqual(rc, 0, stderr)
            summary = json.loads(stdout)
            self.assertEqual(summary["record_count"], [index["record_count"]])

    def test_latest_anchor_missing_digest_peer_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "hidden-notary-peer-export"
            export_dir = root / hidden
            export_dir.mkdir()
            _index, _anchor, digest_anchor = write_export(export_dir)
            digest_anchor.unlink()

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--dry-run",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("anchor source latest anchor has no digest-addressed peer", stderr)
            self.assertNotIn(str(export_dir), stderr)
            self.assertNotIn(str(digest_anchor), stderr)
            self.assertNotIn(hidden, stderr)

    def test_latest_anchor_digest_peer_mismatch_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "hidden-notary-peer-export"
            export_dir = root / hidden
            export_dir.mkdir()
            _index, _anchor, digest_anchor = write_export(export_dir)
            digest_anchor.write_text("{\"different\": true}\n", encoding="utf-8")

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--dry-run",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("anchor source latest anchor differs from digest-addressed peer", stderr)
            self.assertNotIn(str(export_dir), stderr)
            self.assertNotIn(str(digest_anchor), stderr)
            self.assertNotIn(hidden, stderr)

    def test_malformed_anchor_store_dir_is_rejected_before_network_delivery(self):
        def rewrite_anchor_store_dir(export_dir, store_dir):
            latest = export_dir / ADAPTER.LATEST_ANCHOR_FILE
            anchor = json.loads(latest.read_text(encoding="utf-8"))
            anchor["store_dir"] = store_dir
            anchor = with_digest(anchor, ADAPTER.ANCHOR_DIGEST_FIELD)
            anchor_text = json.dumps(anchor, indent=2) + "\n"
            latest.write_text(anchor_text, encoding="utf-8")
            digest_anchor = (
                export_dir
                / ADAPTER.ANCHOR_DIR
                / f"{anchor[ADAPTER.INDEX_DIGEST_FIELD]}.notary.json"
            )
            digest_anchor.write_text(anchor_text, encoding="utf-8")

        cases = [
            (
                "embedded-whitespace",
                "/ops/iso store",
                "store_dir must not contain whitespace",
            ),
            (
                "leading-dash",
                "--store",
                "store_dir must not start with a dash",
            ),
            (
                "segment-leading-dash",
                "/ops/--store",
                "store_dir must not contain leading-dash path segments",
            ),
            (
                "backslash",
                r"C:\\ops\\iso",
                "store_dir must use forward slashes",
            ),
            (
                "semicolon",
                "/ops/iso;debug",
                "store_dir must not contain semicolon path parameters",
            ),
            (
                "secret-looking",
                "/ops/iso/token=notary-secret",
                "store_dir must not contain secret-looking material",
            ),
            (
                "empty-segment",
                "/ops//iso",
                "store_dir must not contain empty path segments",
            ),
            (
                "parent-segment",
                "/ops/../iso",
                "store_dir must not contain dot or parent segments",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, store_dir, expected in cases:
                with self.subTest(name=name):
                    export_dir = root / name / "export"
                    export_dir.mkdir(parents=True)
                    write_export(export_dir)
                    rewrite_anchor_store_dir(export_dir, store_dir)

                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--allow-missing-record-sources",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    if name == "secret-looking":
                        self.assertNotIn("notary-secret", stderr)
                    self.assertEqual(requests, [])

    def test_tampered_persisted_record_source_is_rejected_before_network_delivery(self):
        def rewrite_export_from_index(export_dir, index, store_dir):
            write_export(
                export_dir,
                index=index,
                anchor=sample_anchor(index, store_dir=store_dir),
                store_dir=store_dir,
                write_record_sources_flag=False,
            )

        def digest_correct_source_mismatch(export_dir, _index, store_dir):
            record_path = next((store_dir / ADAPTER.RECORDS_DIR).glob("*.json"))
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["transaction_hash"] = "d" * 64
            source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")

        def digest_correct_metadata_mismatch(export_dir, index, store_dir):
            record_path = next((store_dir / ADAPTER.RECORDS_DIR).glob("*.json"))
            source = json.loads(record_path.read_text(encoding="utf-8"))
            source["metadata"]["profile_id"] = "forged-profile"
            source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index["records"][0]["record_sha256"] = source[
                ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            rewrite_export_from_index(export_dir, index, store_dir)

        def rewrite_digest_correct_record_source(export_dir, index, store_dir, mutate):
            record_path = next((store_dir / ADAPTER.RECORDS_DIR).glob("*.json"))
            source = json.loads(record_path.read_text(encoding="utf-8"))
            mutate(source)
            source = with_digest(source, ADAPTER.PERSISTED_RECORD_DIGEST_FIELD)
            record_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")
            index["records"][0]["record_sha256"] = source[
                ADAPTER.PERSISTED_RECORD_DIGEST_FIELD
            ]
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            rewrite_export_from_index(export_dir, index, store_dir)

        def status_history_current_timestamp_mismatch(export_dir, index, store_dir):
            def mutate(source):
                source["status_history"][-1]["updated_at_ms"] = (
                    source["updated_at_ms"] - 1
                )

            rewrite_digest_correct_record_source(export_dir, index, store_dir, mutate)

        def status_history_timestamp_moves_backwards(export_dir, index, store_dir):
            def mutate(source):
                earlier_entry = dict(source["status_history"][-1])
                earlier_entry["updated_at_ms"] = source["updated_at_ms"] + 1
                source["status_history"].insert(0, earlier_entry)

            rewrite_digest_correct_record_source(export_dir, index, store_dir, mutate)

        def status_history_state_code_mismatch(export_dir, index, store_dir):
            def mutate(source):
                forged_entry = dict(source["status_history"][-1])
                forged_entry["status"] = "Rejected"
                forged_entry["pacs002_code"] = "ACSP"
                source["status_history"].insert(0, forged_entry)

            rewrite_digest_correct_record_source(export_dir, index, store_dir, mutate)

        def missing_persisted_record_nullable_key(export_dir, index, store_dir):
            rewrite_digest_correct_record_source(
                export_dir,
                index,
                store_dir,
                lambda source: source.pop("detail"),
            )

        def missing_persisted_context_nullable_key(export_dir, index, store_dir):
            rewrite_digest_correct_record_source(
                export_dir,
                index,
                store_dir,
                lambda source: source["context"].pop("ledger_id"),
            )

        def missing_persisted_metadata_nullable_key(export_dir, index, store_dir):
            rewrite_digest_correct_record_source(
                export_dir,
                index,
                store_dir,
                lambda source: source["metadata"].pop("business_service"),
            )

        def missing_persisted_history_nullable_key(export_dir, index, store_dir):
            rewrite_digest_correct_record_source(
                export_dir,
                index,
                store_dir,
                lambda source: source["status_history"][0].pop("reason_code"),
            )

        cases = [
            (
                "digest_correct_source_mismatch",
                digest_correct_source_mismatch,
                "record_sha256 does not match audit index record",
            ),
            (
                "digest_correct_metadata_mismatch",
                digest_correct_metadata_mismatch,
                "metadata.profile_id does not match audit index record",
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
                "status_history_state_code_mismatch",
                status_history_state_code_mismatch,
                "status_history[0].pacs002_code is not valid for Rejected status",
            ),
            (
                "missing_persisted_record_nullable_key",
                missing_persisted_record_nullable_key,
                "is missing required keys: detail",
            ),
            (
                "missing_persisted_context_nullable_key",
                missing_persisted_context_nullable_key,
                "context is missing required keys: ledger_id",
            ),
            (
                "missing_persisted_metadata_nullable_key",
                missing_persisted_metadata_nullable_key,
                "metadata is missing required keys: business_service",
            ),
            (
                "missing_persisted_history_nullable_key",
                missing_persisted_history_nullable_key,
                "status_history[0] is missing required keys: reason_code",
            ),
            (
                "missing_record_source",
                lambda _export_dir, _index, store_dir: next(
                    (store_dir / ADAPTER.RECORDS_DIR).glob("*.json")
                ).unlink(),
                "does not exist",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    export_dir = root / name / "export"
                    store_dir = root / name / "store"
                    export_dir.mkdir(parents=True)
                    index, _anchor, _digest_anchor = write_export(
                        export_dir,
                        store_dir=store_dir,
                        write_record_sources_flag=True,
                    )
                    mutate(export_dir, index, store_dir)

                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertEqual(requests, [])

    def test_bearer_token_file_adds_authorization_without_persisting_token(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            token_dir = root / "runtime"
            token_dir.mkdir()
            token_file = token_dir / "token.txt"
            token_file.write_text("notary-token-123", encoding="utf-8")
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--bearer-token-file",
                        str(token_file),
                    ]
                )

            self.assertEqual(rc, 0, stderr)
            self.assertEqual(len(requests), 1)
            self.assertEqual(
                requests[0]["headers"]["Authorization"], "Bearer notary-token-123"
            )
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            self.assertNotIn(
                "notary-token-123", receipts[0].read_text(encoding="utf-8")
            )

    def test_malformed_bearer_token_file_is_rejected_before_network_delivery(self):
        cases = [
            ("empty", b"", "empty"),
            ("padded", b" notary-token", "surrounding whitespace"),
            ("newline", b"notary-token\n", "surrounding whitespace"),
            ("embedded-space", b"notary token", "must not contain whitespace"),
            ("control", b"notary-token\x7f", "must not contain control characters"),
            (
                "unicode-format",
                "notary-token\u200dnotary-token-hidden".encode("utf-8"),
                "must not contain control characters",
            ),
            ("non-utf8", b"notary-token\xff", "not UTF-8"),
            (
                "oversized",
                b"a" * (ADAPTER.MAX_BEARER_TOKEN_BYTES + 1),
                "exceeds",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            token_dir = root / "runtime"
            token_dir.mkdir()
            for name, token_bytes, message in cases:
                with self.subTest(name=name):
                    token_file = token_dir / f"{name}.token"
                    token_file.write_bytes(token_bytes)
                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--bearer-token-file",
                                str(token_file),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)
                    self.assertNotIn("notary-token-hidden", stderr)

    def test_bearer_token_reader_enforces_configured_file_cap(self):
        with tempfile.TemporaryDirectory() as raw_export:
            token_file = Path(raw_export) / "token.txt"
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
        with tempfile.TemporaryDirectory() as raw_export:
            secret_dir = Path(raw_export) / "private_key=notary-secret"
            secret_dir.mkdir()
            cases = [
                (
                    "missing",
                    secret_dir / "token=notary-secret-missing.txt",
                    None,
                    "does not exist",
                ),
                ("empty", secret_dir / "token=notary-secret-empty.txt", b"", "empty"),
                (
                    "non-utf8",
                    secret_dir / "token=notary-secret-nonutf8.txt",
                    b"notary-token\xff",
                    "not UTF-8",
                ),
                (
                    "oversized",
                    secret_dir / "token=notary-secret-oversized.txt",
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
                    self.assertNotIn("private_key=notary-secret", error)
                    self.assertNotIn("token=notary-secret", error)
                    self.assertNotIn(str(token_file), error)

    def test_non_regular_bearer_token_files_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            token_dir = root / "runtime"
            token_dir.mkdir()
            token_target = token_dir / "token-target.txt"
            token_target.write_text("notary-token-123", encoding="utf-8")
            symlink_token = token_dir / "symlink-token.txt"
            try:
                symlink_token.symlink_to(token_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            directory_token = token_dir / "token-dir"
            directory_token.mkdir()
            cases = [
                (symlink_token, "must not be a symlink"),
                (directory_token, "must be a regular file"),
            ]
            for token_file, message in cases:
                with self.subTest(token_file=token_file.name):
                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--bearer-token-file",
                                str(token_file),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_bearer_token_file_symlinked_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            target_dir = root / "token-target"
            target_dir.mkdir()
            token_target = target_dir / "token.txt"
            token_target.write_text("notary-token-123", encoding="utf-8")
            ancestor = root / "token-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            token_file = ancestor / token_target.name

            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--bearer-token-file",
                        str(token_file),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_bearer_token_file_cannot_overlap_export_dir_before_loading(self):
        cases = (
            (
                "same-as-export",
                lambda root, export_dir: export_dir,
                "notary-token-source-same",
            ),
            (
                "inside-export",
                lambda root, export_dir: export_dir / "runtime-auth.txt",
                "notary-token-source-nested",
            ),
            (
                "ancestor-of-export",
                lambda root, export_dir: export_dir.parent,
                "notary-token-source-ancestor",
            ),
        )
        for name, token_path_for, hidden in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    if name == "ancestor-of-export":
                        source_root = root / "notary-token-source-ancestor"
                        export_dir = source_root / "export"
                    else:
                        export_dir = root / "export"
                    write_export(export_dir)
                    token_file = token_path_for(root, export_dir)
                    if token_file.suffix:
                        token_file.write_text("notary-token-123\n", encoding="utf-8")
                    receipt_dir = root / "receipts"

                    with capture_server() as (endpoint, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--bearer-token-file",
                                str(token_file),
                                "--receipt-dir",
                                str(receipt_dir),
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn(
                        "bearer_token_file must not overlap export_dir path",
                        stderr,
                    )
                    self.assertNotIn("notary-token-123", stderr)
                    self.assertNotIn(hidden, stderr)

    def test_input_cli_paths_reject_raw_smuggling_before_read(self):
        cases = (
            ("export semicolon", "--export-dir", "export;debug", "semicolon path"),
            ("export whitespace", "--export-dir", "export dir", "whitespace"),
            ("export leading-dash", "--export-dir", "nested/-export", "leading-dash"),
            ("export parent", "--export-dir", "nested/../export", "dot or parent"),
            (
                "export dot",
                "--export-dir",
                lambda root: f"{root}/nested/./export",
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
                "token=notary-secret",
                "secret-looking material",
            ),
            (
                "export secret-looking",
                "--export-dir",
                "private_key=notary-secret",
                "secret-looking material",
            ),
            (
                "receipt secret-looking",
                "--receipt-dir",
                "token=notary-secret",
                "secret-looking material",
            ),
        )
        for name, flag, raw_path, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    value = raw_path(root) if callable(raw_path) else str(root / raw_path)
                    export_dir = root / "export"
                    export_dir.mkdir()
                    argv = [
                        "--export-dir",
                        str(export_dir),
                        "--dry-run",
                        flag,
                        value,
                    ]
                    if flag == "--export-dir":
                        argv = ["--export-dir", value, "--dry-run"]

                    rc, stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    if "secret-looking" in name:
                        self.assertNotIn("notary-secret", stderr)

    def test_direct_run_paths_reject_smuggling_before_export_loading(self):
        def args_for(root, **overrides):
            values = {
                "export_dir": root / "missing-export",
                "endpoint": [],
                "receipt_dir": root / "receipts",
                "bearer_token_file": None,
                "timeout_secs": 1.0,
                "response_limit_bytes": 1024,
                "allow_insecure_http": False,
                "allow_missing_record_sources": False,
                "all": False,
                "dry_run": True,
            }
            values.update(overrides)
            return argparse.Namespace(**values)

        cases = (
            (
                "export whitespace",
                lambda root: args_for(root, export_dir=root / "export dir"),
                "export_dir must not contain whitespace",
            ),
            (
                "export repository fixture",
                lambda root: args_for(
                    root,
                    export_dir=root / "fixtures" / "iso20022" / "notary-export",
                ),
                "export_dir must not point to checked-in ISO fixture artifacts",
            ),
            (
                "receipt parent",
                lambda root: args_for(
                    root,
                    receipt_dir=root / "nested" / ".." / "receipts",
                ),
                "receipt_dir must not contain dot or parent segments",
            ),
            (
                "receipt repository fixture",
                lambda root: args_for(
                    root,
                    receipt_dir=root / "fixtures" / "iso20022" / "notary-receipts",
                ),
                "receipt_dir must not point to checked-in ISO fixture artifacts",
            ),
            (
                "token secret",
                lambda root: args_for(
                    root,
                    bearer_token_file=root / "token=notary-secret",
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
                    self.assertNotIn("notary-secret", error)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            with self.assertRaisesRegex(ADAPTER.AdapterError, "provide --export-dir"):
                ADAPTER.run(args_for(root, export_dir=None))
            args = args_for(root)
            delattr(args, "export_dir")
            with self.assertRaisesRegex(ADAPTER.AdapterError, "provide --export-dir"):
                ADAPTER.run(args)

    def test_direct_run_scalar_paths_must_be_paths_before_export_loading(self):
        cases = (
            ("export", "export_dir", object(), "export_dir"),
            ("receipt", "receipt_dir", object(), "receipt_dir"),
            ("token", "bearer_token_file", object(), "bearer_token_file"),
        )
        for name, field, value, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        export_dir=root / "missing-export",
                        endpoint=[],
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        allow_insecure_http=False,
                        allow_missing_record_sources=False,
                        all=False,
                        dry_run=True,
                    )
                    setattr(args, field, value)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a path", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_policy_flags_must_be_booleans_before_export_loading(self):
        cases = (
            ("dry_run", "--dry-run", "true"),
            ("all", "--all", 1),
            ("allow_insecure_http", "--allow-insecure-http", None),
            ("allow_missing_record_sources", "--allow-missing-record-sources", []),
        )
        for attr, label, value in cases:
            with self.subTest(flag=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        export_dir=root / "missing-export",
                        endpoint=[],
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        allow_insecure_http=False,
                        allow_missing_record_sources=False,
                        all=False,
                        dry_run=True,
                    )
                    setattr(args, attr, value)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a boolean", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_numeric_limits_must_exist_before_export_loading(self):
        cases = (
            ("timeout_secs", "--timeout-secs must be a positive finite number"),
            ("response_limit_bytes", "--response-limit-bytes must be a positive integer"),
        )
        for field, expected in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        export_dir=root / "missing-export",
                        endpoint=[],
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        allow_insecure_http=False,
                        allow_missing_record_sources=False,
                        all=False,
                        dry_run=True,
                    )
                    delattr(args, field)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_response_limit_is_capped_before_export_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            args = argparse.Namespace(
                export_dir=root / "missing-export",
                endpoint=[],
                receipt_dir=root / "receipts",
                bearer_token_file=None,
                timeout_secs=1.0,
                response_limit_bytes=ADAPTER.MAX_RESPONSE_LIMIT_BYTES + 1,
                allow_insecure_http=False,
                allow_missing_record_sources=False,
                all=False,
                dry_run=True,
            )

            with self.assertRaises(ADAPTER.AdapterError) as caught:
                ADAPTER.run(args)

            message = str(caught.exception)
            self.assertIn(
                f"--response-limit-bytes must be no more than {ADAPTER.MAX_RESPONSE_LIMIT_BYTES}",
                message,
            )
            self.assertNotIn("does not exist", message)
            self.assertNotIn(str(root), message)

    def test_direct_run_endpoints_must_be_repeatable_string_list_before_export_loading(self):
        cases = (
            ("bare string", "https://notary.example.invalid", "--endpoint"),
            ("bad entry", [object()], "--endpoint[0]"),
        )
        for name, endpoint, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        export_dir=root / "missing-export",
                        endpoint=endpoint,
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        allow_insecure_http=False,
                        allow_missing_record_sources=False,
                        all=False,
                        dry_run=True,
                    )

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    if name == "bare string":
                        self.assertIn(
                            f"{label} must be a repeatable string list",
                            message,
                        )
                    else:
                        self.assertIn(f"{label} must be a string", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_endpoint_lists_are_count_bounded_before_export_loading(self):
        cases = (("direct", False), ("cli", True))
        for name, via_cli in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    endpoints = [
                        f"https://notary-{offset}.example.invalid"
                        for offset in range(ADAPTER.MAX_ENDPOINT_INPUTS + 1)
                    ]
                    if via_cli:
                        argv = [
                            "--export-dir",
                            str(root / "missing-export"),
                            "--dry-run",
                        ]
                        for endpoint in endpoints:
                            argv.extend(["--endpoint", endpoint])

                        rc, stdout, stderr = run_main(argv)

                        self.assertEqual(rc, 2)
                        self.assertEqual(stdout, "")
                        message = stderr
                    else:
                        args = argparse.Namespace(
                            export_dir=root / "missing-export",
                            endpoint=endpoints,
                            receipt_dir=root / "receipts",
                            bearer_token_file=None,
                            timeout_secs=1.0,
                            response_limit_bytes=1024,
                            allow_insecure_http=False,
                            allow_missing_record_sources=False,
                            all=False,
                            dry_run=True,
                        )

                        with self.assertRaises(ADAPTER.AdapterError) as caught:
                            ADAPTER.run(args)
                        message = str(caught.exception)

                    self.assertIn(
                        f"--endpoint accepts at most {ADAPTER.MAX_ENDPOINT_INPUTS} values",
                        message,
                    )
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_numeric_cli_limits_reject_nonpositive_and_nonfinite_before_network_delivery(self):
        cases = (
            ("timeout nan", "--timeout-secs", "nan", "positive finite number"),
            ("timeout inf", "--timeout-secs", "inf", "positive finite number"),
            ("timeout zero", "--timeout-secs", "0", "positive finite number"),
            ("response zero", "--response-limit-bytes", "0", "positive integer"),
            ("response negative", "--response-limit-bytes", "-1", "positive integer"),
            (
                "response too large",
                "--response-limit-bytes",
                str(ADAPTER.MAX_RESPONSE_LIMIT_BYTES + 1),
                f"no more than {ADAPTER.MAX_RESPONSE_LIMIT_BYTES}",
            ),
        )
        for name, flag, value, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    with capture_server() as (endpoint, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                flag,
                                value,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)
                    self.assertFalse((export_dir / "receipts").exists())

    def test_receipt_output_dir_path_diagnostics_do_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            receipt_file = export_dir / "receipt-dir-as-file"
            receipt_file.write_text("not a directory\n", encoding="utf-8")

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_file),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("receipt_dir", stderr)
            self.assertIn("must be a directory", stderr)
            self.assertNotIn(str(receipt_file), stderr)
            self.assertNotIn(receipt_file.name, stderr)

    def test_receipt_dir_cannot_reuse_notary_source_paths_before_loading(self):
        cases = (
            (
                "latest",
                lambda export_dir: export_dir / ADAPTER.LATEST_ANCHOR_FILE,
                [],
                "receipt_dir must not overlap export_dir.latest_anchor",
            ),
            (
                "anchors",
                lambda export_dir: export_dir / ADAPTER.ANCHOR_DIR,
                ["--all"],
                "receipt_dir must not overlap export_dir.anchors",
            ),
            (
                "index",
                lambda export_dir: export_dir / ADAPTER.INDEX_FILE,
                [],
                "receipt_dir must not overlap export_dir.index",
            ),
        )
        for name, receipt_dir_for, extra_args, message in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    if name == "latest":
                        (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                            "{",
                            encoding="utf-8",
                        )
                    elif name == "index":
                        (export_dir / ADAPTER.INDEX_FILE).write_text(
                            "{",
                            encoding="utf-8",
                        )
                    receipt_dir = receipt_dir_for(export_dir)

                    with capture_server() as (endpoint, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--receipt-dir",
                                str(receipt_dir),
                                *extra_args,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)
                    self.assertNotIn("is not valid JSON", stderr)
                    self.assertNotIn(str(receipt_dir), stderr)

    def test_receipt_dir_cannot_reuse_bearer_token_file_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            bearer_token_file = root / "adapter-auth.txt"
            bearer_token_file.write_text("notary-token-123\n", encoding="utf-8")

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--bearer-token-file",
                        str(bearer_token_file),
                        "--receipt-dir",
                        str(bearer_token_file),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("receipt_dir must not overlap bearer_token_file path", stderr)
            self.assertNotIn("notary-token-123", stderr)
            self.assertEqual(
                bearer_token_file.read_text(encoding="utf-8"),
                "notary-token-123\n",
            )

    def test_receipt_dir_cannot_contain_bearer_token_file_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            export_dir = root / "export"
            write_export(export_dir)
            token_dir = root / "runtime-auth"
            token_dir.mkdir()
            bearer_token_file = token_dir / "adapter-auth.txt"
            bearer_token_file.write_text("notary-token-123\n", encoding="utf-8")

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--bearer-token-file",
                        str(bearer_token_file),
                        "--receipt-dir",
                        str(token_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("receipt_dir must not overlap bearer_token_file path", stderr)
            self.assertNotIn("notary-token-123", stderr)
            self.assertEqual(
                bearer_token_file.read_text(encoding="utf-8"),
                "notary-token-123\n",
            )

    def test_symlinked_receipt_output_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            receipt_target_dir = export_dir / "receipt-target"
            receipt_target_dir.mkdir()
            receipt_dir = export_dir / "receipt-link"
            try:
                receipt_dir.symlink_to(receipt_target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("receipt_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(receipt_dir), stderr)
            self.assertNotIn(receipt_dir.name, stderr)

            receipt_dir.unlink()
            receipt_dir.mkdir()
            latest = json.loads(
                (export_dir / ADAPTER.LATEST_ANCHOR_FILE).read_text(encoding="utf-8")
            )
            with capture_server() as (endpoint, requests):
                receipt = receipt_dir / (
                    f"{latest[ADAPTER.INDEX_DIGEST_FIELD]}."
                    f"{ADAPTER._endpoint_sha256(endpoint)}.receipt.json"
                )
                target = export_dir / "receipt-target.json"
                target.write_text("untouched\n", encoding="utf-8")
                try:
                    receipt.symlink_to(target)
                except OSError as error:
                    self.skipTest(f"symlink creation unavailable: {error}")
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("receipt_output[0]", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(receipt), stderr)
            self.assertNotIn(receipt.name, stderr)
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
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    receipt_dir = (
                        receipt_dir_arg(export_dir)
                        if callable(receipt_dir_arg)
                        else str(export_dir / receipt_dir_arg)
                    )

                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--receipt-dir",
                                receipt_dir,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn(message, stderr)

    def test_hardlinked_receipt_output_leaf_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            _index, anchor, _digest_anchor = write_export(export_dir)
            receipt_dir = export_dir / "receipts"
            receipt_dir.mkdir()
            target = export_dir / "receipt-target.json"
            target.write_text("untouched\n", encoding="utf-8")
            with capture_server() as (endpoint, requests):
                receipt_path = receipt_dir / (
                    f"{anchor[ADAPTER.INDEX_DIGEST_FIELD]}."
                    f"{ADAPTER._endpoint_sha256(endpoint)}.receipt.json"
                )
                try:
                    receipt_path.hardlink_to(target)
                except OSError as error:
                    self.skipTest(f"hard link creation unavailable: {error}")
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("receipt_output[0]", stderr)
            self.assertIn("must not be hard-linked", stderr)
            self.assertNotIn(str(receipt_path), stderr)
            self.assertNotIn(receipt_path.name, stderr)
            self.assertEqual(target.read_text(encoding="utf-8"), "untouched\n")

    def test_symlinked_receipt_output_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            target_dir = export_dir / "receipt-target"
            target_dir.mkdir()
            ancestor = export_dir / "receipt-ancestor-link"
            try:
                ancestor.symlink_to(target_dir, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            receipt_dir = ancestor / "nested" / "receipts"

            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                        "--receipt-dir",
                        str(receipt_dir),
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("receipt_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(receipt_dir), stderr)
            self.assertNotIn(ancestor.name, stderr)
            self.assertFalse((target_dir / "nested").exists())

    def test_export_dir_discovery_path_diagnostics_do_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            missing_dir = root / "missing-export"
            export_file = root / "export-as-file"
            export_file.write_text("not a directory\n", encoding="utf-8")
            empty_dir = root / "empty-export"
            empty_dir.mkdir()

            cases = (
                (missing_dir, ["--dry-run"], "does not exist"),
                (export_file, ["--dry-run"], "must be a directory"),
                (empty_dir, ["--all", "--dry-run"], "has no *.notary.json anchors"),
            )
            for path, extra_args, message in cases:
                with self.subTest(path=path.name):
                    rc, stdout, stderr = run_main(
                        ["--export-dir", str(path), *extra_args]
                    )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn("export_dir", stderr)
                    self.assertIn(message, stderr)
                    self.assertNotIn(str(path), stderr)
                    self.assertNotIn(path.name, stderr)

    def test_symlinked_export_dir_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_export = root / "export-target"
            target_export.mkdir()
            write_export(target_export)
            export_dir = root / "export-link"
            try:
                export_dir.symlink_to(target_export, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("export_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(export_dir), stderr)
            self.assertNotIn(export_dir.name, stderr)

    def test_symlinked_export_dir_ancestor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            target_export = root / "export-target"
            target_export.mkdir()
            ancestor = root / "export-ancestor-link"
            try:
                ancestor.symlink_to(target_export, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            export_dir = ancestor / "nested"
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("export_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(export_dir), stderr)
            self.assertNotIn(ancestor.name, stderr)

    def test_plain_http_endpoint_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_server() as (endpoint, requests):
                rc, _stdout, _stderr = run_main(
                    ["--export-dir", str(export_dir), "--endpoint", endpoint]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])

    def test_endpoint_smuggling_variants_are_rejected_before_network_delivery(self):
        cases = [
            (" https://notary.example/anchor", False),
            ("https://notary.example/anchor ", False),
            ("https://user:pass@notary.example/anchor", False),
            ("https://notary.example/anchor;debug", False),
            ("https://notary.example/anchor?token=abc", False),
            ("https://notary.example/anchor#receipt", False),
            ("https://notary.example/anchor", False),
            ("https://notary.example.com/anchor", False),
            ("https://notary.example.net/anchor", False),
            ("https://notary.example.org/anchor", False),
            ("https://notary.example.invalid/anchor", False),
            (
                "https://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                False,
            ),
            ("https:///anchor", False),
            ("https://[::1", False),
            ("https://notary.example/anc\nhor", False),
            ("https://notary.example/iso anchor", False),
            ("https://notary.example:abc/anchor", False),
            ("https://notary.example:/anchor", False),
            ("https://notary.example:0/anchor", False),
            ("https://notary.example:08443/anchor", False),
            ("https://notary.example:99999/anchor", False),
            ("https://notary.example:443/anchor", False),
            ("https://Notary.example/anchor", False),
            ("https://notary.example./anchor", False),
            ("https://notary..example/anchor", False),
            ("https://localhost/anchor", False),
            ("https://10.1.2.3/anchor", False),
            ("https://10.1.2.3.sslip.io/anchor", False),
            ("https://0x7f.0.0.1/anchor", False),
            ("https://[::127.0.0.1]/anchor", False),
            ("https://-notary.example/anchor", False),
            ("https://notary-.example/anchor", False),
            ("https://notary._tcp.example/anchor", False),
            ("https://notary.example%2einvalid/anchor", False),
            ("https://123.000.000.001/anchor", False),
            ("https://notary.example/../anchor", False),
            ("https://notary.example/archive//anchor", False),
            ("https://notary.example/%2e%2e/anchor", False),
            ("https://notary.example/archive%2fanchor", False),
            ("https://notary.example/archive%252fanchor", False),
            ("https://notary.example/archive;debug/anchor", False),
            ("https://notary.example/archive%3bdebug/anchor", False),
            ("https://notary.example/archive%23debug/anchor", False),
            (r"https://notary.example/archive\anchor", False),
            ("https://notary.example/archive%20anchor", False),
            ("https://notary.example/archive%00anchor", False),
            ("https://notary.example/archive%7fanchor", False),
            ("https://notary.example/archive%zzanchor", False),
            ("https://notary.example/" + ("a" * ADAPTER.MAX_HTTP_URL_CHARS), False),
            ("https://" + ".".join(["a" * 63] * 5) + "/anchor", False),
            ("http://127.0.0.1/anchor ", True),
            ("http://user:pass@127.0.0.1/anchor", True),
            ("http://127.0.0.1/anchor?token=abc", True),
            ("http://127.0.0.1:80/anchor", True),
            ("http://127.000.000.001/anchor", True),
            ("http://127.0.0.1/archive//anchor", True),
            ("http://notary.example.invalid/anchor", True),
            (
                "http://notary.swift-cbpr-plus.operator-canary.bank/anchor",
                True,
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            for endpoint, allow_insecure in cases:
                with self.subTest(endpoint=endpoint, allow_insecure=allow_insecure):
                    argv = ["--export-dir", str(export_dir), "--endpoint", endpoint]
                    if allow_insecure:
                        argv.append("--allow-insecure-http")
                    rc, _stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn("error:", stderr)

    def test_unused_insecure_http_override_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)

            rc, stdout, stderr = run_main(
                [
                    "--export-dir",
                    str(export_dir),
                    "--endpoint",
                    "https://notary.bank.internal/archive",
                    "--dry-run",
                    "--allow-insecure-http",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-insecure-http requires at least one http:// or local/private endpoint",
                stderr,
            )

    def test_unused_insecure_http_override_rejects_before_delivery_and_receipts(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            calls = []
            original_publish_anchor = ADAPTER.publish_anchor

            def fake_publish_anchor(anchor, endpoint, **kwargs):
                calls.append((anchor, endpoint, kwargs))
                return ADAPTER.PublishResult(
                    endpoint=endpoint,
                    status_code=200,
                    ok=True,
                    response_body_sha256=ADAPTER.sha256_hex(b"ok"),
                    response_body_preview="ok",
                )

            ADAPTER.publish_anchor = fake_publish_anchor
            try:
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        "https://notary.bank.internal/archive",
                        "--allow-insecure-http",
                    ]
                )
            finally:
                ADAPTER.publish_anchor = original_publish_anchor

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-insecure-http requires at least one http:// or local/private endpoint",
                stderr,
            )
            self.assertEqual(calls, [])
            self.assertFalse((export_dir / "receipts").exists())

    def test_rejected_endpoint_does_not_echo_secret_query(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            secret_endpoint = "https://notary.example/anchor?token=notary-secret"
            rc, _stdout, stderr = run_main(
                ["--export-dir", str(export_dir), "--endpoint", secret_endpoint]
            )

            self.assertEqual(rc, 2)
            self.assertIn("params, query, or fragment", stderr)
            self.assertNotIn(secret_endpoint, stderr)
            self.assertNotIn("token=", stderr)
            self.assertNotIn("notary-secret", stderr)

    def test_rejected_endpoint_does_not_echo_secret_path(self):
        cases = (
            "https://notary.example/archive/token=notary-path-secret",
            "https://notary.example/archive/token-notary-path-secret",
            "https://notary.example/archive/token%3Dnotary-path-secret",
            "https://notary.example/archive/token%253Dnotary-path-secret",
        )
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            for secret_endpoint in cases:
                with self.subTest(secret_endpoint=secret_endpoint):
                    rc, _stdout, stderr = run_main(
                        ["--export-dir", str(export_dir), "--endpoint", secret_endpoint]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("secret-looking material", stderr)
                    self.assertNotIn(secret_endpoint, stderr)
                    self.assertNotIn("token=", stderr)
                    self.assertNotIn("notary-path-secret", stderr)

    def test_rejected_endpoint_does_not_echo_secret_port(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            secret_endpoint = "https://notary.example:token-notary-port-secret/archive"

            rc, _stdout, stderr = run_main(
                ["--export-dir", str(export_dir), "--endpoint", secret_endpoint]
            )

            self.assertEqual(rc, 2)
            self.assertIn("invalid port", stderr)
            self.assertNotIn(secret_endpoint, stderr)
            self.assertNotIn("token-notary-port-secret", stderr)

    def test_rejected_endpoint_does_not_echo_secret_host_or_parser_error(self):
        cases = (
            ("https://token-notary-host-secret.notary.example/archive", "secret-looking material"),
            ("https://[token-notary-host-secret/archive", "is not a valid URL"),
        )
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            for secret_endpoint, message in cases:
                with self.subTest(secret_endpoint=secret_endpoint):
                    rc, _stdout, stderr = run_main(
                        ["--export-dir", str(export_dir), "--endpoint", secret_endpoint]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)
                    self.assertNotIn(secret_endpoint, stderr)
                    self.assertNotIn("token-notary-host-secret", stderr)

    def test_duplicate_endpoint_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("duplicates --endpoint[0]", stderr)
            self.assertNotIn(endpoint, stderr)

    def test_duplicate_anchor_json_keys_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                '{"version":1,"token=notary-duplicate-key-secret":1,"token=notary-duplicate-key-secret":2}\n',
                encoding="utf-8",
            )
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("duplicate key", stderr)
            self.assertNotIn("notary-duplicate-key-secret", stderr)

    def test_non_finite_anchor_json_numbers_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                '{"version": NaN}\n',
                encoding="utf-8",
            )
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("NaN", stderr)

    def test_noncanonical_anchor_json_numbers_are_rejected_before_network_delivery(self):
        for value in ("1e01", "-0", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                        f'{{"version":{value}}}\n',
                        encoding="utf-8",
                    )
                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn("non-canonical numeric value", stderr)
                    self.assertNotIn(value, stderr)

    def test_noncanonical_index_json_numbers_are_rejected_before_network_delivery(self):
        for value in ("1e01", "-0", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    (export_dir / ADAPTER.INDEX_FILE).write_text(
                        f'{{"version":{value}}}\n',
                        encoding="utf-8",
                    )
                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn("non-canonical numeric value", stderr)
                    self.assertNotIn(value, stderr)

    def test_noncanonical_record_source_json_numbers_are_rejected_before_network_delivery(self):
        for value in ("1e01", "-0", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    export_dir = root / "export"
                    store_dir = root / "store"
                    export_dir.mkdir()
                    index, _anchor, _digest_anchor = write_export(
                        export_dir,
                        store_dir=store_dir,
                    )
                    record_path = (
                        store_dir / ADAPTER.RECORDS_DIR / index["records"][0]["filename"]
                    )
                    record_path.write_text(f'{{"version":{value}}}\n', encoding="utf-8")
                    with capture_server() as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn("non-canonical numeric value", stderr)
                    self.assertNotIn(value, stderr)

    def test_anchor_json_surrogate_strings_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                '{"version":"\\ud800"}\n',
                encoding="utf-8",
            )
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("invalid Unicode surrogate", stderr)

    def test_oversized_audit_json_inputs_are_rejected_before_network_delivery(self):
        cases = ("latest-anchor", "audit-index", "record-source")
        for name in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    export_dir = root / "export"
                    store_dir = root / "store"
                    export_dir.mkdir()
                    _index, _anchor, _digest_anchor = write_export(
                        export_dir,
                        store_dir=store_dir,
                        write_record_sources_flag=True,
                    )
                    old_audit_limit = ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES
                    old_record_limit = ADAPTER.MAX_PERSISTED_RECORD_JSON_BYTES
                    try:
                        oversized = '{"version":1,"padding":"' + ("a" * 128) + '"}'
                        if name == "latest-anchor":
                            ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES = 128
                            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                                oversized,
                                encoding="utf-8",
                            )
                        elif name == "audit-index":
                            ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES = 128
                            (export_dir / ADAPTER.INDEX_FILE).write_text(
                                oversized,
                                encoding="utf-8",
                            )
                        else:
                            ADAPTER.MAX_PERSISTED_RECORD_JSON_BYTES = 128
                            record_path = next(
                                (store_dir / ADAPTER.RECORDS_DIR).glob("*.json")
                            )
                            record_path.write_text(oversized, encoding="utf-8")

                        with capture_server() as (endpoint, requests):
                            rc, _stdout, stderr = run_main(
                                [
                                    "--export-dir",
                                    str(export_dir),
                                    "--endpoint",
                                    endpoint,
                                    "--allow-insecure-http",
                                ]
                            )
                    finally:
                        ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES = old_audit_limit
                        ADAPTER.MAX_PERSISTED_RECORD_JSON_BYTES = old_record_limit

                    self.assertEqual(rc, 2)
                    self.assertEqual(requests, [])
                    self.assertIn("exceeds", stderr)

    def test_symlinked_latest_anchor_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            _index, _anchor, digest_anchor = write_export(export_dir)
            latest = export_dir / ADAPTER.LATEST_ANCHOR_FILE
            latest.unlink()
            try:
                latest.symlink_to(digest_anchor)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_symlinked_audit_index_is_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            index_file = export_dir / ADAPTER.INDEX_FILE
            index_copy = export_dir / "messages.index.copy.json"
            index_copy.write_bytes(index_file.read_bytes())
            index_file.unlink()
            try:
                index_file.symlink_to(index_copy)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with capture_server() as (endpoint, requests):
                rc, _stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(requests, [])
            self.assertIn("must not be a symlink", stderr)

    def test_exported_audit_index_mismatch_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "hidden-exported-index-mismatch"
            export_dir = root / hidden / "export"
            export_dir.mkdir(parents=True)
            index, _anchor, _digest_anchor = write_export(export_dir)
            index["records"].append(sample_record("msg-2"))
            index["record_count"] = 2
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            index_file = export_dir / ADAPTER.INDEX_FILE
            index_file.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")

            with capture_server() as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("embedded audit index differs from exported audit index", stderr)
            self.assertNotIn(str(index_file), stderr)
            self.assertNotIn(hidden, stderr)

    def test_malformed_source_paths_do_not_echo_paths(self):
        def malformed_anchor_json(root, hidden):
            export_dir = root / hidden / "export"
            export_dir.mkdir(parents=True)
            write_export(export_dir)
            source_path = export_dir / ADAPTER.LATEST_ANCHOR_FILE
            source_path.write_text("{not-json\n", encoding="utf-8")
            return export_dir, source_path, "is not valid JSON"

        def malformed_exported_index_json(root, hidden):
            export_dir = root / hidden / "export"
            export_dir.mkdir(parents=True)
            write_export(export_dir)
            source_path = export_dir / ADAPTER.INDEX_FILE
            source_path.write_text("{not-json\n", encoding="utf-8")
            return export_dir, source_path, "is not valid JSON"

        def missing_store_dir(root, hidden):
            export_dir = root / "export"
            store_dir = root / hidden / "store"
            export_dir.mkdir()
            index, _anchor, _digest_anchor = write_export(
                export_dir,
                store_dir=store_dir,
                write_record_sources_flag=True,
            )
            record_path = store_dir / ADAPTER.RECORDS_DIR / index["records"][0]["filename"]
            record_path.unlink()
            (store_dir / ADAPTER.RECORDS_DIR).rmdir()
            store_dir.rmdir()
            return export_dir, store_dir, "store_dir does not exist"

        def malformed_record_json(root, hidden):
            export_dir = root / "export"
            store_dir = root / hidden / "store"
            export_dir.mkdir()
            index, _anchor, _digest_anchor = write_export(
                export_dir,
                store_dir=store_dir,
                write_record_sources_flag=True,
            )
            source_path = store_dir / ADAPTER.RECORDS_DIR / index["records"][0]["filename"]
            source_path.write_text("{not-json\n", encoding="utf-8")
            return export_dir, source_path, "is not valid JSON"

        cases = (
            ("malformed-anchor-json", malformed_anchor_json),
            ("malformed-exported-index-json", malformed_exported_index_json),
            ("missing-store-dir", missing_store_dir),
            ("malformed-record-json", malformed_record_json),
        )
        for name, arrange in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                hidden = f"hidden-notary-source-{name}"
                export_dir, source_path, expected = arrange(root, hidden)

                with capture_server() as (endpoint, requests):
                    rc, stdout, stderr = run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertEqual(requests, [])
                self.assertIn(expected, stderr)
                self.assertNotIn("line 1 column", stderr)
                self.assertNotIn("(char ", stderr)
                self.assertNotIn(hidden, stderr)
                self.assertNotIn(str(source_path), stderr)

    def test_tampered_anchor_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            index, anchor, digest_anchor = write_export(export_dir)
            anchor["record_count"] = 2
            tampered = json.dumps(anchor, indent=2) + "\n"
            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(tampered, encoding="utf-8")
            digest_anchor.write_text(tampered, encoding="utf-8")

            rc, _stdout, _stderr = run_main(["--export-dir", str(export_dir), "--dry-run"])

            self.assertEqual(rc, 2)
            self.assertEqual(index["record_count"], 1)

    def test_boolean_export_versions_and_counts_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            cases = []

            def boolean_index(export_dir):
                index = sample_index()
                index["version"] = True
                write_export(export_dir, index=index)

            cases.append(("index", boolean_index, "audit index version must be 1"))

            def boolean_index_record_count(export_dir):
                index = sample_index()
                index["record_count"] = True
                index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
                write_export(export_dir, index=index)

            cases.append(
                (
                    "index-record-count",
                    boolean_index_record_count,
                    "audit index record_count must be a non-negative integer",
                )
            )

            def boolean_anchor(export_dir):
                index = sample_index()
                anchor = sample_anchor(index)
                anchor["version"] = True
                write_export(export_dir, index=index, anchor=anchor)

            cases.append(("anchor", boolean_anchor, "unsupported anchor version"))

            def boolean_anchor_record_count(export_dir):
                index = sample_index()
                anchor = sample_anchor(index)
                anchor["record_count"] = True
                anchor = with_digest(anchor, ADAPTER.ANCHOR_DIGEST_FIELD)
                write_export(export_dir, index=index, anchor=anchor)

            cases.append(
                (
                    "anchor-record-count",
                    boolean_anchor_record_count,
                    "record_count must be a non-negative integer",
                )
            )

            def boolean_record_source(export_dir):
                index, _anchor, _digest_anchor = write_export(export_dir)
                record = index["records"][0]
                source_path = (
                    export_dir
                    / "store"
                    / ADAPTER.RECORDS_DIR
                    / record["filename"]
                )
                source = json.loads(source_path.read_text(encoding="utf-8"))
                source["version"] = True
                source_path.write_text(json.dumps(source, indent=2) + "\n", encoding="utf-8")

            cases.append(
                (
                    "record-source",
                    boolean_record_source,
                    "unsupported persisted record version",
                )
            )

            for name, setup, message in cases:
                with self.subTest(name=name):
                    export_dir = root / name
                    export_dir.mkdir()
                    setup(export_dir)

                    rc, _stdout, stderr = run_main(
                        ["--export-dir", str(export_dir), "--dry-run"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(message, stderr)

    def test_unknown_audit_index_or_anchor_fields_are_rejected(self):
        def unknown_index(index, _anchor):
            index = {**index, "operator_note": "publish anyway"}
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            return index, sample_anchor(index)

        def unknown_anchor(index, anchor):
            anchor = with_digest(
                {
                    **anchor,
                    "operator_note": "publish anyway",
                },
                ADAPTER.ANCHOR_DIGEST_FIELD,
            )
            return index, anchor

        cases = [
            ("index", unknown_index),
            ("anchor", unknown_anchor),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, mutate in cases:
                with self.subTest(name=name):
                    export_dir = root / name
                    export_dir.mkdir()
                    index = sample_index()
                    anchor = sample_anchor(index)
                    index, anchor = mutate(index, anchor)
                    write_export(export_dir, index=index, anchor=anchor)

                    rc, _stdout, stderr = run_main(
                        ["--export-dir", str(export_dir), "--dry-run"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn("contains unknown keys", stderr)
                    self.assertNotIn("operator_note", stderr)

    def test_unknown_or_malformed_audit_index_record_fields_are_rejected(self):
        cases = [
            (
                "unknown-record-field",
                lambda record: record.update({"operator_note": "publish anyway"}),
                "contains unknown keys",
            ),
            (
                "padded-state",
                lambda record: record.update({"state": " Accepted"}),
                "state must not have surrounding whitespace",
            ),
            (
                "unsupported-state",
                lambda record: record.update({"state": "Settled"}),
                "state must be Pending, Accepted, or Rejected",
            ),
            (
                "state-code-mismatch",
                lambda record: record.update({"state": "Rejected", "pacs002_code": "ACSP"}),
                "pacs002_code is not valid for Rejected state",
            ),
            (
                "bad-updated-at",
                lambda record: record.update({"updated_at_ms": -1}),
                "updated_at_ms must be a non-negative integer",
            ),
            (
                "wrong-filename",
                lambda record: record.update({"filename": "msg-1.json"}),
                "filename must be digest-addressed",
            ),
            (
                "all-zero-record-digest",
                lambda record: record.update({"record_sha256": "0" * 64}),
                "record_sha256 must not be all zero",
            ),
            (
                "bad-payload-hash",
                lambda record: record.update({"payload_hash": "not-a-digest"}),
                "payload_hash must be a canonical SHA-256",
            ),
            (
                "all-zero-payload-hash",
                lambda record: record.update({"payload_hash": "0" * 64}),
                "payload_hash must not be all zero",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            for name, mutate, expected in cases:
                with self.subTest(name=name):
                    export_dir = root / name
                    export_dir.mkdir()
                    index = sample_index()
                    mutate(index["records"][0])
                    index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
                    anchor = sample_anchor(index)
                    write_export(export_dir, index=index, anchor=anchor)

                    rc, _stdout, stderr = run_main(
                        ["--export-dir", str(export_dir), "--dry-run"]
                    )

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)

    def test_persisted_record_metadata_rejects_all_zero_payload_hash(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            record = sample_record()
            record["payload_hash"] = "0" * 64
            source = sample_persisted_record(record)
            record["record_sha256"] = source[ADAPTER.PERSISTED_RECORD_DIGEST_FIELD]
            source_path = root / record["filename"]
            source_path.write_text(
                json.dumps(source, indent=2) + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "metadata.payload_hash must not be all zero",
            ):
                ADAPTER._verify_persisted_record_source(
                    record,
                    source_path,
                    f"{source_path}",
                )

    def test_audit_index_records_require_nullable_summary_keys(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            index = sample_index()
            index["records"][0].pop("uetr")
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            anchor = sample_anchor(index)
            write_export(
                export_dir,
                index=index,
                anchor=anchor,
                write_record_sources_flag=False,
            )

            rc, _stdout, stderr = run_main(
                ["--export-dir", str(export_dir), "--dry-run"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("records[0] is missing required keys: uetr", stderr)

    def test_duplicate_audit_index_records_are_rejected(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            record = sample_record("msg-1")
            index = {
                "version": 1,
                "record_count": 2,
                "records": [record, dict(record)],
            }
            index = with_digest(index, ADAPTER.INDEX_DIGEST_FIELD)
            anchor = sample_anchor(index)
            write_export(export_dir, index=index, anchor=anchor)

            rc, _stdout, stderr = run_main(
                ["--export-dir", str(export_dir), "--dry-run"]
            )

            self.assertEqual(rc, 2)
            self.assertIn("records[1].message_id duplicates", stderr)
            self.assertNotIn("msg-1", stderr)

    def test_digest_addressed_filename_must_match_index_digest(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            _index, _anchor, digest_anchor = write_export(export_dir)
            wrong_path = digest_anchor.with_name(f"{'c' * 64}.notary.json")
            digest_anchor.rename(wrong_path)

            rc, _stdout, _stderr = run_main(
                ["--export-dir", str(export_dir), "--all", "--dry-run"]
            )

            self.assertEqual(rc, 2)

    def test_non_successful_remote_response_writes_failed_receipt(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_server(status=503, body=b"not ready") as (endpoint, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 1)
            self.assertEqual(len(requests), 1)
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertFalse(receipt["ok"])
            self.assertEqual(receipt["status_code"], 503)
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(b"not ready"))

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

        result = ADAPTER._invalid_http_status_result(
            "https://notary.example/iso-anchor",
            99,
        )

        self.assertEqual(result.endpoint, "https://notary.example/iso-anchor")
        self.assertIsNone(result.status_code)
        self.assertFalse(result.ok)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status")
        self.assertNotIn("99", result.error)

    def test_http_status_parser_rejects_boolean_and_string_aliases(self):
        cases = (True, False, "202", "099", 202.0)
        for raw in cases:
            with self.subTest(raw=raw):
                self.assertIsNone(ADAPTER._parse_http_status_code(raw))

    def test_invalid_remote_status_writes_transport_failed_receipt(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_server(status=700, body=b"non-standard") as (
                endpoint,
                requests,
            ):
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 1)
            self.assertEqual(len(requests), 1)
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertFalse(receipt["ok"])
            self.assertIsNone(receipt["status_code"])
            self.assertIsNone(receipt["response_body_sha256"])
            self.assertIsNone(receipt["response_body_preview"])
            self.assertEqual(receipt["error"], "invalid HTTP status")
            self.assertNotIn("700", receipt["error"])
            self.assertEqual(
                ADAPTER.require_digest_matches(
                    receipt, ADAPTER.RECEIPT_DIGEST_FIELD, "receipt"
                ),
                receipt[ADAPTER.RECEIPT_DIGEST_FIELD],
            )

    def test_malformed_remote_status_returns_failed_receipt_without_echo(self):
        hidden = "token=notary-status-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class BrokenStatus:
            def __int__(self):
                raise RuntimeError(hidden)

        class FailingResponse:
            status = BrokenStatus()

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                return FailingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status")
        self.assertNotIn(hidden, result.error)

    def test_malformed_remote_error_status_returns_failed_receipt_without_echo(self):
        hidden = "token=notary-error-status-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class BrokenStatus:
            def __int__(self):
                raise RuntimeError(hidden)

        class Body:
            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    BrokenStatus(),
                    "failed",
                    {},
                    Body(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status")
        self.assertNotIn(hidden, result.error)

    def test_huge_remote_status_returns_failed_receipt_without_bloat(self):
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class HugeStatus:
            def __int__(self):
                return 10**1000

        class FailingResponse:
            status = HugeStatus()

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                return FailingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status")

    def test_huge_remote_error_status_returns_failed_receipt_without_bloat(self):
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class HugeStatus:
            def __int__(self):
                return 10**1000

        class Body:
            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    HugeStatus(),
                    "failed",
                    {},
                    Body(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "invalid HTTP status")

    def test_remote_redirect_response_is_not_followed(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_redirect_server() as (endpoint, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 1)
            self.assertEqual([request["method"] for request in requests], ["POST"])
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt = json.loads(receipts[0].read_text(encoding="utf-8"))
            self.assertFalse(receipt["ok"])
            self.assertEqual(receipt["status_code"], 302)
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(b"redirect"))

    def test_oversized_remote_response_error_does_not_echo_endpoint(self):
        body = b"notary response"
        cases = [
            (200, "endpoint response exceeded 4 byte limit"),
            (500, "endpoint error response exceeded 4 byte limit"),
        ]
        for status, expected in cases:
            with self.subTest(status=status):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    with capture_server(status=status, body=body) as (endpoint, requests):
                        rc, _stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                                "--response-limit-bytes",
                                "4",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(len(requests), 1)
                    self.assertIn(expected, stderr)
                    self.assertNotIn(endpoint, stderr)

    def test_secret_looking_remote_response_preview_is_redacted(self):
        cases = (
            (b'{"error":"private_key=notary-secret"}', "notary-secret"),
            (b'{"error":"password=notary-secret"}', "notary-secret"),
            (b'{"error":"%70assword%253Dnotary-secret"}', "notary-secret"),
            (b'{"error":"private-key=notary-secret"}', "notary-secret"),
            (b'{"error":"Set-Cookie: notary-secret"}', "notary-secret"),
            (
                b'{"error":"token-notary-response-secret"}',
                "notary-response-secret",
            ),
        )
        for body, hidden in cases:
            with self.subTest(body=body):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    with capture_server(status=500, body=body) as (
                        endpoint,
                        requests,
                    ):
                        rc, _stdout, _stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 1)
                    self.assertEqual(len(requests), 1)
                    receipts = list((export_dir / "receipts").glob("*.receipt.json"))
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

    def test_multiline_remote_response_preview_is_folded_before_receipt_write(self):
        body = b"rejected\nerror: forged diagnostic\tcontinued"
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            with capture_server(status=500, body=body) as (endpoint, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 1)
            self.assertEqual(len(requests), 1)
            receipts = list((export_dir / "receipts").glob("*.receipt.json"))
            self.assertEqual(len(receipts), 1)
            receipt_text = receipts[0].read_text(encoding="utf-8")
            receipt = json.loads(receipt_text)
            self.assertEqual(receipt["status_code"], 500)
            self.assertEqual(receipt["response_body_sha256"], ADAPTER.sha256_hex(body))
            self.assertEqual(
                receipt["response_body_preview"],
                "rejected error: forged diagnostic continued",
            )
            self.assertNotIn("\\nerror: forged", receipt_text)
            self.assertNotIn("\\tcontinued", receipt_text)

    def test_control_character_remote_response_preview_is_redacted(self):
        cases = (
            (b'{"error":"\x1b[31mnotary-warning"}', "[31mnotary-warning"),
            (b'{"error":"notary\x00warning"}', "notary\\u0000warning"),
        )
        for body, hidden in cases:
            with self.subTest(body=body), tempfile.TemporaryDirectory() as raw_export:
                export_dir = Path(raw_export)
                write_export(export_dir)
                with capture_server(status=500, body=body) as (endpoint, requests):
                    rc, _stdout, _stderr = run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )

                self.assertEqual(rc, 1)
                self.assertEqual(len(requests), 1)
                receipts = list((export_dir / "receipts").glob("*.receipt.json"))
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

    def test_secret_looking_success_response_fails_before_receipt_write(self):
        cases = (
            (b'{"receipt":"private_key=notary-secret"}', "private_key"),
            (b"Bearer\tnotary-secret", "notary-secret"),
            (b'{"receipt":"private key notary-secret"}', "private key"),
            (b'{"receipt":"x iroha signature notary-secret"}', "x iroha signature"),
            (b'{"receipt":"token-notary-response-secret"}', "token-notary"),
        )
        for body, marker in cases:
            with self.subTest(body=body):
                with tempfile.TemporaryDirectory() as raw_export:
                    export_dir = Path(raw_export)
                    write_export(export_dir)
                    with capture_server(status=200, body=body) as (endpoint, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--export-dir",
                                str(export_dir),
                                "--endpoint",
                                endpoint,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(len(requests), 1)
                    self.assertIn(
                        "endpoint response body contains secret-looking material",
                        stderr,
                    )
                    self.assertNotIn(marker, stderr)
                    self.assertEqual(
                        list((export_dir / "receipts").glob("*.receipt.json")),
                        [],
                    )

    def test_control_character_success_response_fails_before_receipt_write(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            body = b'{"receipt":"\x1b[31mnotary-success"}'
            with capture_server(status=200, body=body) as (endpoint, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(len(requests), 1)
            self.assertIn("endpoint response body contains unsafe control characters", stderr)
            self.assertNotIn("[31mnotary-success", stderr)
            self.assertEqual(
                list((export_dir / "receipts").glob("*.receipt.json")),
                [],
            )

    def test_non_ascii_success_response_fails_before_receipt_write(self):
        cases = (
            ("unicode", "notary caf\u00e9 hidden-notary-success".encode("utf-8")),
            ("invalid-utf8", b"notary \xff hidden-notary-success"),
        )
        for name, body in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_export:
                export_dir = Path(raw_export)
                write_export(export_dir)
                with capture_server(status=200, body=body) as (endpoint, requests):
                    rc, stdout, stderr = run_main(
                        [
                            "--export-dir",
                            str(export_dir),
                            "--endpoint",
                            endpoint,
                            "--allow-insecure-http",
                        ]
                    )

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertEqual(len(requests), 1)
                self.assertIn("endpoint response body contains non-ASCII text", stderr)
                self.assertNotIn("hidden-notary-success", stderr)
                self.assertEqual(
                    list((export_dir / "receipts").glob("*.receipt.json")),
                    [],
                )

    def test_url_error_receipt_error_uses_stable_label_without_echo(self):
        class BrokenReason:
            def __str__(self):
                raise RuntimeError("token=notary-url-error-secret")

        cases = (
            ADAPTER.urllib.error.URLError("connection refused"),
            ADAPTER.urllib.error.URLError("token=notary-url-error-secret"),
            ADAPTER.urllib.error.URLError("upstream \x1b[31mnotary-warning"),
            ADAPTER.urllib.error.URLError("upstream r\u00e9seau"),
            ADAPTER.urllib.error.URLError("x" * 4097),
            ADAPTER.urllib.error.URLError(
                FileNotFoundError(2, "No such file or directory", "/tmp/notary.sock")
            ),
            ADAPTER.urllib.error.URLError(BrokenReason()),
        )
        for error in cases:
            with self.subTest(error=type(error.reason).__name__):
                receipt_error = ADAPTER._url_error_receipt_error(error)
                self.assertEqual(receipt_error, ADAPTER.URL_TRANSPORT_ERROR)
                self.assertNotIn("connection refused", receipt_error)
                self.assertNotIn("notary-url-error-secret", receipt_error)
                self.assertNotIn("notary-warning", receipt_error)
                self.assertNotIn("r\u00e9seau", receipt_error)
                self.assertNotIn("/tmp/notary.sock", receipt_error)

    def test_endpoint_transport_open_failure_returns_failed_receipt(self):
        hidden = "token=notary-open-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise OSError(hidden)

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint transport could not be opened")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_transport_open_runtime_failure_returns_failed_receipt(self):
        hidden = "token=notary-open-runtime-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise RuntimeError(hidden)

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint transport could not be opened")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_response_body_read_failure_returns_failed_receipt(self):
        hidden = "token=notary-read-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingResponse:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self, _limit):
                raise OSError(hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                return FailingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_response_body_runtime_read_failure_returns_failed_receipt(self):
        hidden = "token=notary-runtime-read-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingResponse:
            status = 200

            def read(self, _limit):
                raise RuntimeError(hidden)

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                return FailingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_response_body_non_bytes_returns_failed_receipt_without_echo(self):
        hidden = "token=notary-body-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class MalformedResponse:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self, _limit):
                return hidden

        class MalformedOpener:
            def open(self, *_args, **_kwargs):
                return MalformedResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = MalformedOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint response body was not bytes")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_response_body_bytes_like_values_are_capped_by_byte_length(self):
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )
        wide = memoryview(array.array("H", [0x4142] * 4))
        cases = (
            ("bytearray", bytearray(b"accepted"), b"accepted"),
            ("memoryview", memoryview(b"accepted"), b"accepted"),
            ("wide-memoryview", wide, wide.cast("B").tobytes()),
        )
        original_opener = ADAPTER.NO_REDIRECT_OPENER
        try:
            for name, returned, expected in cases:
                with self.subTest(name=name):
                    class BytesLikeResponse:
                        status = 200

                        def read(self, _limit):
                            return returned

                        def close(self):
                            return None

                    class BytesLikeOpener:
                        def open(self, *_args, **_kwargs):
                            return BytesLikeResponse()

                    ADAPTER.NO_REDIRECT_OPENER = BytesLikeOpener()
                    result = ADAPTER.publish_anchor(
                        anchor,
                        "https://notary.example/anchor",
                        timeout_secs=1.0,
                        response_limit_bytes=128,
                        bearer_token=None,
                    )

                    self.assertTrue(result.ok)
                    self.assertEqual(result.status_code, 200)
                    self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(expected))
                    self.assertEqual(result.response_body_preview, expected.decode("utf-8"))
                    self.assertIsNone(result.error)

            too_wide = memoryview(array.array("H", [0x4142] * 4))

            class OversizedResponse:
                status = 200

                def read(self, _limit):
                    return too_wide

                def close(self):
                    return None

            class OversizedOpener:
                def open(self, *_args, **_kwargs):
                    return OversizedResponse()

            ADAPTER.NO_REDIRECT_OPENER = OversizedOpener()
            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "endpoint response exceeded 3 byte limit",
            ):
                ADAPTER.publish_anchor(
                    anchor,
                    "https://notary.example/anchor",
                    timeout_secs=1.0,
                    response_limit_bytes=3,
                    bearer_token=None,
                )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

    def test_endpoint_success_response_close_failure_preserves_receipt(self):
        hidden = "token=notary-close-secret"
        body = b"accepted"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class ClosingResponse:
            status = 200

            def read(self, _limit):
                return body

            def close(self):
                raise OSError(hidden)

        class ClosingOpener:
            def open(self, *_args, **_kwargs):
                return ClosingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = ClosingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertTrue(result.ok)
        self.assertEqual(result.status_code, 200)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertIsNone(result.error)

    def test_endpoint_failed_response_close_failure_preserves_receipt(self):
        hidden = "token=notary-close-secret"
        body = b"rejected"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class ClosingResponse:
            status = 503

            def read(self, _limit):
                return body

            def close(self):
                raise OSError(hidden)

        class ClosingOpener:
            def open(self, *_args, **_kwargs):
                return ClosingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = ClosingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertEqual(result.status_code, 503)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertEqual(result.error, "HTTP 503")
        self.assertNotIn(hidden, result.error)
        self.assertNotIn(hidden, result.response_body_preview)

    def test_endpoint_response_close_lookup_failure_preserves_receipt(self):
        hidden = "token=notary-close-lookup-secret"
        body = b"accepted"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class ClosingResponse:
            status = 200

            def read(self, _limit):
                return body

            @property
            def close(self):
                raise RuntimeError(hidden)

        class ClosingOpener:
            def open(self, *_args, **_kwargs):
                return ClosingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = ClosingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertTrue(result.ok)
        self.assertEqual(result.status_code, 200)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertIsNone(result.error)

    def test_endpoint_error_response_body_read_failure_returns_failed_receipt(self):
        hidden = "token=notary-error-read-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingBody:
            def read(self, _limit):
                raise OSError(hidden)

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint error response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_error_response_body_runtime_read_failure_returns_failed_receipt(self):
        hidden = "token=notary-error-runtime-read-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingBody:
            def read(self, _limit):
                raise RuntimeError(hidden)

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint error response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_error_response_body_non_bytes_returns_failed_receipt_without_echo(self):
        hidden = "token=notary-error-body-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class MalformedBody:
            def read(self, _limit):
                return hidden

            def close(self):
                return None

        class MalformedOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    MalformedBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = MalformedOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint error response body was not bytes")
        self.assertNotIn(hidden, result.error)

    def test_endpoint_error_response_body_bytes_like_values_are_capped_by_byte_length(self):
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )
        wide = memoryview(array.array("H", [0x4142] * 4))
        cases = (
            ("bytearray", bytearray(b"rejected"), b"rejected"),
            ("memoryview", memoryview(b"rejected"), b"rejected"),
            ("wide-memoryview", wide, wide.cast("B").tobytes()),
        )
        original_opener = ADAPTER.NO_REDIRECT_OPENER
        try:
            for name, returned, expected in cases:
                with self.subTest(name=name):
                    class BytesLikeBody:
                        def read(self, _limit):
                            return returned

                        def close(self):
                            return None

                    class BytesLikeOpener:
                        def open(self, *_args, **_kwargs):
                            raise ADAPTER.urllib.error.HTTPError(
                                "https://notary.example/anchor",
                                500,
                                "failed",
                                {},
                                BytesLikeBody(),
                            )

                    ADAPTER.NO_REDIRECT_OPENER = BytesLikeOpener()
                    result = ADAPTER.publish_anchor(
                        anchor,
                        "https://notary.example/anchor",
                        timeout_secs=1.0,
                        response_limit_bytes=128,
                        bearer_token=None,
                    )

                    self.assertFalse(result.ok)
                    self.assertEqual(result.status_code, 500)
                    self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(expected))
                    self.assertEqual(result.response_body_preview, expected.decode("utf-8"))
                    self.assertEqual(result.error, "HTTP 500")

            too_wide = memoryview(array.array("H", [0x4142] * 4))

            class OversizedBody:
                def read(self, _limit):
                    return too_wide

                def close(self):
                    return None

            class OversizedOpener:
                def open(self, *_args, **_kwargs):
                    raise ADAPTER.urllib.error.HTTPError(
                        "https://notary.example/anchor",
                        500,
                        "failed",
                        {},
                        OversizedBody(),
                    )

            ADAPTER.NO_REDIRECT_OPENER = OversizedOpener()
            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "endpoint error response exceeded 3 byte limit",
            ):
                ADAPTER.publish_anchor(
                    anchor,
                    "https://notary.example/anchor",
                    timeout_secs=1.0,
                    response_limit_bytes=3,
                    bearer_token=None,
                )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

    def test_endpoint_error_response_close_failure_preserves_failed_receipt(self):
        hidden = "token=notary-error-close-secret"
        body = b"notary rejected"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingCloseBody:
            def read(self, _limit):
                return body

            def close(self):
                raise OSError(hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    FailingCloseBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertEqual(result.status_code, 500)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertEqual(result.error, "HTTP 500")
        self.assertNotIn(hidden, result.error)
        self.assertNotIn(hidden, result.response_body_preview)

    def test_endpoint_error_response_close_runtime_error_preserves_failed_receipt(self):
        hidden = "token=notary-error-close-secret"
        body = b"notary rejected"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingCloseBody:
            def read(self, _limit):
                return body

            def close(self):
                raise RuntimeError(hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    FailingCloseBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertEqual(result.status_code, 500)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertEqual(result.error, "HTTP 500")
        self.assertNotIn(hidden, result.error)
        self.assertNotIn(hidden, result.response_body_preview)

    def test_endpoint_error_response_read_failure_ignores_close_failure(self):
        read_hidden = "token=notary-error-read-secret"
        close_hidden = "token=notary-error-close-secret"
        index = sample_index()
        anchor_payload = sample_anchor(index)
        anchor = ADAPTER.VerifiedAnchor(
            path=Path("latest.notary.json"),
            payload=anchor_payload,
            raw=json.dumps(anchor_payload).encode("utf-8"),
            index_sha256=index[ADAPTER.INDEX_DIGEST_FIELD],
            anchor_sha256=anchor_payload[ADAPTER.ANCHOR_DIGEST_FIELD],
            record_count=anchor_payload["record_count"],
            missing_record_sources=False,
        )

        class FailingBody:
            def read(self, _limit):
                raise OSError(read_hidden)

            def close(self):
                raise OSError(close_hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://notary.example/anchor",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.publish_anchor(
                anchor,
                "https://notary.example/anchor",
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertFalse(result.ok)
        self.assertIsNone(result.status_code)
        self.assertIsNone(result.response_body_sha256)
        self.assertIsNone(result.response_body_preview)
        self.assertEqual(result.error, "endpoint error response could not be read")
        self.assertNotIn(read_hidden, result.error)
        self.assertNotIn(close_hidden, result.error)


if __name__ == "__main__":
    unittest.main()
