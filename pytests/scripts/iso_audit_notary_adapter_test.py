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
    obj[digest_field] = ADAPTER.sha256_hex(ADAPTER._canonical_json_bytes(obj))
    return obj


def sample_record(message_id="msg-1"):
    return {
        "message_id": message_id,
        "filename": f"{message_id}.json",
        "record_sha256": "a" * 64,
        "state": "Accepted",
        "message_type": "pacs.008",
        "business_message_id": f"{message_id}-biz",
        "uetr": None,
        "payload_hash": "b" * 64,
        "reference_snapshot_id": "snapshot",
    }


def sample_index():
    root = {
        "version": 1,
        "record_count": 1,
        "records": [sample_record()],
    }
    return with_digest(root, ADAPTER.INDEX_DIGEST_FIELD)


def sample_anchor(index):
    root = {
        "version": ADAPTER.ANCHOR_VERSION,
        "index_sha256": index[ADAPTER.INDEX_DIGEST_FIELD],
        "record_count": index["record_count"],
        "store_dir": None,
        "audit_index": index,
    }
    return with_digest(root, ADAPTER.ANCHOR_DIGEST_FIELD)


def write_export(export_dir, index=None, anchor=None):
    index = index or sample_index()
    anchor = anchor or sample_anchor(index)
    index_sha256 = index[ADAPTER.INDEX_DIGEST_FIELD]
    anchors_dir = export_dir / ADAPTER.ANCHOR_DIR
    anchors_dir.mkdir(parents=True)
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
            ("https://user:pass@notary.example/anchor", False),
            ("https://notary.example/anchor;debug", False),
            ("https://notary.example/anchor?token=abc", False),
            ("https://notary.example/anchor#receipt", False),
            ("https:///anchor", False),
            ("https://[::1", False),
            ("https://notary.example/anc\nhor", False),
            ("http://user:pass@127.0.0.1/anchor", True),
            ("http://127.0.0.1/anchor?token=abc", True),
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
