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
    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_audit_unknown_secret", "audit_unknown_secret"),
            ("%70assword_audit_unknown_leak", "audit_unknown_leak"),
            ("private-key_audit_unknown_leak", "audit_unknown_leak"),
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

    def test_raw_cli_secret_like_values_rejected_without_echo(self):
        cases = (
            ["--private-key=notary-secret"],
            ["token=notary-secret"],
            ["password=notary-secret"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("secret-looking", stderr)
                self.assertNotIn("token=", stderr)
                self.assertNotIn("password=", stderr)
                self.assertNotIn("notary-secret", stderr)

    def test_url_cli_flags_reject_missing_empty_or_flag_like_values(self):
        cases = (
            ["--endpoint"],
            ["--endpoint", ""],
            ["--endpoint", "--receipt-dir"],
            ["--endpoint="],
            ["--endpoint=--receipt-dir"],
        )
        for argv in cases:
            with self.subTest(argv=argv):
                rc, _stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertIn("--endpoint requires a URL value", stderr)

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
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            token_file = export_dir / "token.txt"
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
            ("non-utf8", b"notary-token\xff", "not UTF-8"),
            (
                "oversized",
                b"a" * (ADAPTER.MAX_BEARER_TOKEN_BYTES + 1),
                "exceeds",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            for name, token_bytes, message in cases:
                with self.subTest(name=name):
                    token_file = export_dir / f"{name}.token"
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
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            token_target = export_dir / "token-target.txt"
            token_target.write_text("notary-token-123", encoding="utf-8")
            symlink_token = export_dir / "symlink-token.txt"
            try:
                symlink_token.symlink_to(token_target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            directory_token = export_dir / "token-dir"
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
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            target_dir = export_dir / "token-target"
            target_dir.mkdir()
            token_target = target_dir / "token.txt"
            token_target.write_text("notary-token-123", encoding="utf-8")
            ancestor = export_dir / "token-ancestor-link"
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

    def test_numeric_cli_limits_reject_nonpositive_and_nonfinite_before_network_delivery(self):
        cases = (
            ("timeout nan", "--timeout-secs", "nan", "positive finite number"),
            ("timeout inf", "--timeout-secs", "inf", "positive finite number"),
            ("timeout zero", "--timeout-secs", "0", "positive finite number"),
            ("response zero", "--response-limit-bytes", "0", "positive integer"),
            ("response negative", "--response-limit-bytes", "-1", "positive integer"),
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
            self.assertIn("must not be a symlink", stderr)

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
            self.assertIn("must not be hard-linked", stderr)
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
            self.assertIn("must not be a symlink", stderr)
            self.assertFalse((target_dir / "nested").exists())

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
            self.assertIn("must not be a symlink", stderr)

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
            self.assertIn("must not be a symlink", stderr)

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
            self.assertIn("non-finite numeric constant NaN", stderr)

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
                    old_limit = ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES
                    try:
                        ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES = 128
                        oversized = '{"version":1,"padding":"' + ("a" * 128) + '"}'
                        if name == "latest-anchor":
                            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                                oversized,
                                encoding="utf-8",
                            )
                        elif name == "audit-index":
                            (export_dir / ADAPTER.INDEX_FILE).write_text(
                                oversized,
                                encoding="utf-8",
                            )
                        else:
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
                        ADAPTER.MAX_AUDIT_EXPORT_JSON_BYTES = old_limit

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
                    self.assertIn("contains unknown keys: operator_note", stderr)

    def test_unknown_or_malformed_audit_index_record_fields_are_rejected(self):
        cases = [
            (
                "unknown-record-field",
                lambda record: record.update({"operator_note": "publish anyway"}),
                "contains unknown keys: operator_note",
            ),
            (
                "padded-state",
                lambda record: record.update({"state": " Accepted"}),
                "state must not have surrounding whitespace",
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
                "bad-payload-hash",
                lambda record: record.update({"payload_hash": "not-a-digest"}),
                "payload_hash must be a canonical SHA-256",
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
            b'{"error":"private_key=notary-secret"}',
            b'{"error":"password=notary-secret"}',
            b'{"error":"%70assword%253Dnotary-secret"}',
            b'{"error":"private-key=notary-secret"}',
            b'{"error":"Set-Cookie: notary-secret"}',
        )
        for body in cases:
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
                    self.assertNotIn("notary-secret", receipt_text)

    def test_secret_looking_url_error_is_redacted(self):
        self.assertEqual(
            ADAPTER._receipt_error("upstream secret=notary-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream password=notary-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream %70assword%253Dnotary-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(
            ADAPTER._receipt_error("upstream private-key=notary-secret"),
            ADAPTER.REDACTED_ERROR,
        )
        self.assertEqual(ADAPTER._receipt_error("connection refused"), "connection refused")


if __name__ == "__main__":
    unittest.main()
