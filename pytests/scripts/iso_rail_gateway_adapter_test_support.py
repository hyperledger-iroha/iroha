"""Shared fixtures for ISO rail gateway adapter tests."""

import contextlib
import http.server
import io
import json
import threading

import iso_rail_gateway_adapter as ADAPTER

SAMPLE_XML = b"<Document><FIToFIPmtStsRpt><GrpHdr><MsgId>rail-1</MsgId></GrpHdr></FIToFIPmtStsRpt></Document>"
TEST_NETWORK_ID = "hash:" + ("A5" * 32) + "#95D7"


class _TestOperatorContext:
    def __init__(self):
        self.calls = []

    def headers(self, method, path, body):
        self.calls.append((method, path, body))
        nonce = f"test-nonce-{len(self.calls)}"
        return {
            "X-Iroha-Operator-Public-Key": "ed0120" + ("11" * 32),
            "X-Iroha-Operator-Timestamp-Ms": "123456",
            "X-Iroha-Operator-Nonce": nonce,
            "X-Iroha-Operator-Signature": "c2lnbmF0dXJl",
        }


TEST_OPERATOR_CONTEXT = _TestOperatorContext()


def run_main(argv):
    can_inject = type(argv) is list and all(type(item) is str for item in argv)
    effective_argv = list(argv) if can_inject else argv
    if (
        can_inject
        and "--operator-private-key-file" in effective_argv
        and "--network-id" not in effective_argv
    ):
        effective_argv.extend(["--network-id", TEST_NETWORK_ID])
    inject_operator = (
        can_inject
        and "--dry-run" not in effective_argv
        and "--operator-private-key-file" not in effective_argv
    )
    if inject_operator:
        effective_argv.extend(
            [
                "--network-id",
                TEST_NETWORK_ID,
                "--operator-private-key-file",
                "runtime-operator-key.txt",
            ]
        )
    stdout = io.StringIO()
    stderr = io.StringIO()
    original_loader = ADAPTER._load_operator_signing_context
    if inject_operator:
        ADAPTER._load_operator_signing_context = lambda _network_id, _path: (
            TEST_OPERATOR_CONTEXT
        )
    try:
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            rc = ADAPTER.main(effective_argv)
    finally:
        ADAPTER._load_operator_signing_context = original_loader
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
