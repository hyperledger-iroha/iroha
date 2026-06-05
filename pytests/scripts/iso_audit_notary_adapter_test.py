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


class IsoAuditNotaryAdapterTest(unittest.TestCase):
    def test_publish_posts_verified_anchor_and_writes_receipt(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            index, anchor, _digest_anchor = write_export(export_dir)
            with capture_server() as (endpoint, requests):
                rc, _stdout, _stderr = run_main(
                    [
                        "--export-dir",
                        str(export_dir),
                        "--endpoint",
                        endpoint,
                        "--allow-insecure-http",
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
            ("https://notary.example:99999/anchor", False),
            ("https://notary.example:443/anchor", False),
            ("https://Notary.example/anchor", False),
            ("https://notary.example./anchor", False),
            ("https://notary..example/anchor", False),
            ("https://-notary.example/anchor", False),
            ("https://notary-.example/anchor", False),
            ("https://notary._tcp.example/anchor", False),
            ("https://notary.example%2einvalid/anchor", False),
            ("https://123.000.000.001/anchor", False),
            ("https://notary.example/../anchor", False),
            ("https://notary.example/%2e%2e/anchor", False),
            ("https://notary.example/archive%2fanchor", False),
            ("https://notary.example/archive%252fanchor", False),
            ("https://notary.example/archive;debug/anchor", False),
            (r"https://notary.example/archive\anchor", False),
            ("https://notary.example/archive%20anchor", False),
            ("https://notary.example/archive%00anchor", False),
            ("https://notary.example/archive%7fanchor", False),
            ("https://notary.example/archive%zzanchor", False),
            ("http://127.0.0.1/anchor ", True),
            ("http://user:pass@127.0.0.1/anchor", True),
            ("http://127.0.0.1/anchor?token=abc", True),
            ("http://127.0.0.1:80/anchor", True),
            ("http://127.000.000.001/anchor", True),
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

    def test_duplicate_anchor_json_keys_are_rejected_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_export:
            export_dir = Path(raw_export)
            write_export(export_dir)
            (export_dir / ADAPTER.LATEST_ANCHOR_FILE).write_text(
                '{"version":1,"version":1}\n',
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


if __name__ == "__main__":
    unittest.main()
