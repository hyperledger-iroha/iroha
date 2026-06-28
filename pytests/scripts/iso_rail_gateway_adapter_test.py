import argparse
import array
import contextlib
import http.server
import importlib.util
import io
import json
import os
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
    def test_os_error_detail_redacts_unsafe_strerror_without_echo(self):
        self.assertEqual(
            ADAPTER._safe_os_error_detail(OSError(5, "Permission denied")),
            "Permission denied",
        )
        unsafe_values = (
            "token=/tmp/rail-hidden-secret",
            "open /tmp/rail-hidden-path",
            "bad\ncontrol",
            "nonascii-\u2603",
            "x" * 129,
        )
        for value in unsafe_values:
            with self.subTest(value=value):
                detail = ADAPTER._safe_os_error_detail(OSError(5, value))
                self.assertEqual(detail, "I/O error")
                self.assertNotIn("rail-hidden", detail)
        hidden = "token=rail-strerror-accessor-secret"

        class HostileOSError(OSError):
            @property
            def strerror(self):
                raise RuntimeError(hidden)

        detail = ADAPTER._safe_os_error_detail(HostileOSError())
        self.assertEqual(detail, "I/O error")
        self.assertNotIn(hidden, detail)

    def test_symlink_ancestor_inspection_failures_do_not_echo_detail(self):
        hidden = "token=rail-ancestor-secret"
        path_type = type(ADAPTER.Path("."))
        original_lstat = path_type.lstat
        cases = (
            ("os_error", OSError(5, hidden)),
            ("runtime", RuntimeError(hidden)),
        )
        for name, failure in cases:
            with self.subTest(name=name):

                def failing_lstat(_self, error=failure):
                    raise error

                path_type.lstat = failing_lstat
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._reject_symlinked_existing_ancestors(
                            ADAPTER.Path("ancestor") / "leaf",
                            display_label="receipt output",
                        )
                finally:
                    path_type.lstat = original_lstat

                message = str(caught.exception)
                self.assertIn("cannot inspect receipt output ancestors", message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

    def test_read_helpers_lstat_failures_do_not_echo_detail(self):
        hidden = "token=rail-reader-inspect-secret"
        path_type = type(ADAPTER.Path("."))
        original_lstat = path_type.lstat
        helper_cases = (
            (
                "_read_regular_file",
                lambda path: ADAPTER._read_regular_file(path, path_label="sidecar"),
                "cannot inspect sidecar",
            ),
            (
                "_bounded_read",
                lambda path: ADAPTER._bounded_read(path, 32, path_label="payload"),
                "cannot inspect payload",
            ),
        )
        failure_cases = (
            ("lstat_os", OSError(5, hidden)),
            ("lstat_runtime", RuntimeError(hidden)),
            ("lstat_type", TypeError(hidden)),
            ("lstat_value", ValueError(hidden)),
        )
        for helper_name, action, expected in helper_cases:
            for failure_name, failure in failure_cases:
                with self.subTest(helper=helper_name, failure=failure_name):
                    with tempfile.TemporaryDirectory() as raw_root:
                        path = ADAPTER.Path(raw_root) / "message.xml"
                        path.write_text("<Document/>", encoding="utf-8")

                        def failing_lstat(self, error=failure):
                            if self == path:
                                raise error
                            return original_lstat(self)

                        path_type.lstat = failing_lstat
                        try:
                            with self.assertRaises(ADAPTER.AdapterError) as caught:
                                action(path)
                        finally:
                            path_type.lstat = original_lstat

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertIn("I/O error", message)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn(str(path), message)
                    if isinstance(failure, OSError):
                        self.assertIs(caught.exception.__cause__, failure)
                    else:
                        self.assertIsNone(caught.exception.__cause__)
                        self.assertTrue(caught.exception.__suppress_context__)

    def test_input_directory_inspection_failures_do_not_echo_detail(self):
        hidden = "token=rail-input-dir-inspect-secret"
        path_type = type(ADAPTER.Path("."))
        original_lstat = path_type.lstat
        cases = (
            ("lstat_os", OSError(5, hidden)),
            ("lstat_runtime", RuntimeError(hidden)),
            ("lstat_type", TypeError(hidden)),
            ("lstat_value", ValueError(hidden)),
        )
        for name, failure in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                path = ADAPTER.Path(raw_root) / "inbox"
                path.mkdir()

                def failing_lstat(self, error=failure):
                    if self == path:
                        raise error
                    return original_lstat(self)

                path_type.lstat = failing_lstat
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._ensure_input_directory(path, "inbox_dir")
                finally:
                    path_type.lstat = original_lstat

                message = str(caught.exception)
                self.assertIn("cannot inspect inbox_dir", message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(str(path), message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

    def test_same_existing_path_stat_failures_return_false(self):
        hidden = "token=rail-alias-stat-secret"
        path_type = type(ADAPTER.Path("."))
        original_stat = path_type.stat
        cases = (
            OSError(5, hidden),
            RuntimeError(hidden),
            TypeError(hidden),
            ValueError(hidden),
        )
        for failure in cases:
            with self.subTest(error=type(failure).__name__):

                def failing_stat(_self, *args, error=failure, **kwargs):
                    raise error

                path_type.stat = failing_stat
                try:
                    self.assertFalse(
                        ADAPTER._same_existing_path(
                            ADAPTER.Path("left"),
                            ADAPTER.Path("right"),
                        )
                    )
                finally:
                    path_type.stat = original_stat

    def test_path_resolve_failures_do_not_echo_detail(self):
        hidden = "token=rail-resolve-secret"
        path_type = type(ADAPTER.Path("."))
        original_resolve = path_type.resolve
        failure_cases = (
            ("resolve_os", OSError(5, hidden)),
            ("resolve_runtime", RuntimeError(hidden)),
            ("resolve_type", TypeError(hidden)),
            ("resolve_value", ValueError(hidden)),
        )

        cases = (
            (
                "receipt-input-overlap",
                lambda root: root / "receipts",
                lambda root: ADAPTER._reject_receipt_dir_input_path_overlap(
                    root / "receipts",
                    root / "inbox" / "message.xml",
                    "message",
                ),
                "cannot resolve receipt_dir",
            ),
            (
                "generic-overlap",
                lambda root: root / "left",
                lambda root: ADAPTER._reject_path_overlap(
                    root / "left",
                    "left path",
                    root / "right",
                    "right path",
                ),
                "cannot resolve left path",
            ),
            (
                "message-inbox-root",
                lambda root: root / "inbox",
                lambda root: ADAPTER.resolve_message_paths(
                    root / "inbox",
                    "message.xml",
                ),
                "cannot resolve inbox_dir",
            ),
            (
                "message-parent",
                lambda root: root / "inbox" / "nested",
                lambda root: ADAPTER.resolve_message_paths(
                    root / "inbox",
                    "nested/message.xml",
                ),
                "cannot resolve --message parent",
            ),
            (
                "message-receipt-dir",
                lambda root: root / "receipts",
                lambda root: ADAPTER._reject_message_receipt_dir_overlap(
                    [root / "inbox" / "message.xml"],
                    root / "receipts",
                ),
                "cannot resolve receipt_dir",
            ),
            (
                "message-source",
                lambda root: root / "inbox" / "message.xml",
                lambda root: ADAPTER._reject_message_receipt_dir_overlap(
                    [root / "inbox" / "message.xml"],
                    root / "receipts",
                ),
                "cannot resolve message[0]",
            ),
        )
        for case_name, setup_target, action, expected in cases:
            for failure_name, failure in failure_cases:
                with self.subTest(case=case_name, failure=failure_name):
                    with tempfile.TemporaryDirectory() as raw_root:
                        root = ADAPTER.Path(raw_root)
                        inbox = root / "inbox"
                        inbox.mkdir()
                        target = setup_target(root)

                        def failing_resolve(self, *args, error=failure, **kwargs):
                            if self == target:
                                raise error
                            return original_resolve(self, *args, **kwargs)

                        path_type.resolve = failing_resolve
                        try:
                            with self.assertRaises(ADAPTER.AdapterError) as caught:
                                action(root)
                        finally:
                            path_type.resolve = original_resolve

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertIn("I/O error", message)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn(str(target), message)
                    if isinstance(failure, OSError):
                        self.assertIs(caught.exception.__cause__, failure)
                    else:
                        self.assertIsNone(caught.exception.__cause__)
                        self.assertTrue(caught.exception.__suppress_context__)

    def test_read_helpers_fdopen_and_close_failures_do_not_echo_os_detail(self):
        hidden = "token=rail-reader-open-secret"
        cleanup_hidden = "token=rail-reader-close-secret"
        cases = (
            (
                "_read_regular_file",
                lambda path: ADAPTER._read_regular_file(
                    path,
                    max_bytes=32,
                    path_label="sidecar",
                ),
                "cannot open sidecar for reading",
            ),
            (
                "_bounded_read",
                lambda path: ADAPTER._bounded_read(
                    path,
                    32,
                    path_label="payload",
                ),
                "cannot open payload for reading",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                path = root / "message.xml"
                path.write_text("<Document/>", encoding="utf-8")
                original_fdopen = ADAPTER.os.fdopen
                original_close = ADAPTER.os.close

                def failing_fdopen(*_args, **_kwargs):
                    raise OSError(5, hidden)

                def failing_close(fd):
                    original_close(fd)
                    raise OSError(5, cleanup_hidden)

                ADAPTER.os.fdopen = failing_fdopen
                ADAPTER.os.close = failing_close
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        action(path)
                finally:
                    ADAPTER.os.fdopen = original_fdopen
                    ADAPTER.os.close = original_close

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(cleanup_hidden, message)
                self.assertNotIn(str(root), message)

                runtime_cleanup_hidden = f"token=rail-reader-close-runtime-secret-{name}"

                def failing_runtime_close(fd):
                    original_close(fd)
                    raise RuntimeError(runtime_cleanup_hidden)

                ADAPTER.os.fdopen = failing_fdopen
                ADAPTER.os.close = failing_runtime_close
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        action(path)
                finally:
                    ADAPTER.os.fdopen = original_fdopen
                    ADAPTER.os.close = original_close

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(runtime_cleanup_hidden, message)
                self.assertNotIn(str(root), message)

                read_hidden = f"token=rail-reader-runtime-secret-{name}"

                class FailingReadHandle:
                    def __init__(self, fd):
                        self.fd = fd

                    def __enter__(self):
                        return self

                    def __exit__(self, exc_type, exc, tb):
                        original_close(self.fd)
                        return False

                    def read(self, _size):
                        raise RuntimeError(read_hidden)

                def failing_read_fdopen(fd, *_args, **_kwargs):
                    return FailingReadHandle(fd)

                ADAPTER.os.fdopen = failing_read_fdopen
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        action(path)
                finally:
                    ADAPTER.os.fdopen = original_fdopen

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertIn("I/O error", message)
                self.assertNotIn(read_hidden, message)
                self.assertIsNone(caught.exception.__cause__)
                self.assertTrue(caught.exception.__suppress_context__)

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
            hidden = "hidden-rail-output-link"
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

    def test_text_output_target_inspection_failures_do_not_echo_detail(self):
        hidden = "token=rail-output-inspect-secret"
        path_type = type(ADAPTER.Path("."))
        original_exists = path_type.exists
        original_is_symlink = path_type.is_symlink
        original_lstat = path_type.lstat
        original_mkdir = path_type.mkdir
        leaf_cases = (
            ("leaf_exists_os", "exists", OSError(5, hidden)),
            ("leaf_exists_runtime", "exists", RuntimeError(hidden)),
            ("leaf_lstat_os", "lstat", OSError(5, hidden)),
            ("leaf_lstat_runtime", "lstat", RuntimeError(hidden)),
        )
        for name, failure_point, failure in leaf_cases:
            with self.subTest(name=name):

                def failing_exists(_self, error=failure):
                    if failure_point == "exists":
                        raise error
                    return True

                def false_is_symlink(_self):
                    return False

                def failing_lstat(_self, error=failure):
                    raise error

                path_type.exists = failing_exists
                path_type.is_symlink = false_is_symlink
                path_type.lstat = failing_lstat
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._ensure_output_file_target(
                            ADAPTER.Path("receipt.json"),
                            display_label="receipt output",
                        )
                finally:
                    path_type.exists = original_exists
                    path_type.is_symlink = original_is_symlink
                    path_type.lstat = original_lstat

                message = str(caught.exception)
                self.assertIn("cannot inspect receipt output leaf", message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

        parent_cases = (
            ("parent_lstat_os", OSError(5, hidden)),
            ("parent_lstat_runtime", RuntimeError(hidden)),
        )
        for name, failure in parent_cases:
            with self.subTest(name=name):

                def failing_lstat(_self, error=failure):
                    raise error

                path_type.lstat = failing_lstat
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._write_text_output(
                            ADAPTER.Path("receipt.json"),
                            "{}\n",
                            display_label="receipt output",
                        )
                finally:
                    path_type.lstat = original_lstat

                message = str(caught.exception)
                self.assertIn("cannot inspect receipt output parent", message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

        parent_create_cases = (
            ("parent_mkdir_os", OSError(5, hidden)),
            ("parent_mkdir_runtime", RuntimeError(hidden)),
            ("parent_mkdir_type", TypeError(hidden)),
            ("parent_mkdir_value", ValueError(hidden)),
        )
        for name, failure in parent_create_cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:

                    def failing_mkdir(_self, *args, error=failure, **kwargs):
                        raise error

                    path_type.mkdir = failing_mkdir
                    try:
                        with self.assertRaises(ADAPTER.AdapterError) as caught:
                            ADAPTER._write_text_output(
                                ADAPTER.Path(raw_root) / "out" / "receipt.json",
                                "{}\n",
                                display_label="receipt output",
                            )
                    finally:
                        path_type.mkdir = original_mkdir

                message = str(caught.exception)
                self.assertIn("cannot create receipt output parent", message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

    def test_text_output_hardlinked_leaf_diagnostic_does_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "hidden-rail-output.json"
            output = root / hidden
            output.write_text("old\n", encoding="utf-8")
            os.link(output, root / "peer.json")

            with self.assertRaises(ADAPTER.AdapterError) as caught:
                ADAPTER._write_text_output(
                    output,
                    "{}\n",
                    display_label="receipt output",
                )

            message = str(caught.exception)
            self.assertIn("receipt output", message)
            self.assertIn("must not be hard-linked", message)
            self.assertNotIn(str(output), message)
            self.assertNotIn(hidden, message)

    def test_output_directory_inspection_failures_do_not_echo_detail(self):
        hidden = "token=rail-output-dir-inspect-secret"
        path_type = type(ADAPTER.Path("."))
        original_exists = path_type.exists
        original_is_symlink = path_type.is_symlink
        original_lstat = path_type.lstat
        original_mkdir = path_type.mkdir
        cases = (
            ("exists_os", "exists", OSError(5, hidden), "cannot inspect receipt directory"),
            ("exists_runtime", "exists", RuntimeError(hidden), "cannot inspect receipt directory"),
            ("lstat_os", "lstat", OSError(5, hidden), "cannot inspect receipt directory"),
            ("lstat_runtime", "lstat", RuntimeError(hidden), "cannot inspect receipt directory"),
            ("mkdir_os", "mkdir", OSError(5, hidden), "cannot create receipt directory"),
            ("mkdir_runtime", "mkdir", RuntimeError(hidden), "cannot create receipt directory"),
            ("post_mkdir_lstat_os", "post_lstat", OSError(5, hidden), "cannot inspect receipt directory"),
            ("post_mkdir_lstat_runtime", "post_lstat", RuntimeError(hidden), "cannot inspect receipt directory"),
        )
        for name, failure_point, failure, expected in cases:
            with self.subTest(name=name):
                lstat_calls = {"count": 0}

                def failing_exists(_self, error=failure):
                    if failure_point == "exists":
                        raise error
                    return failure_point in {"lstat"}

                def false_is_symlink(_self):
                    return False

                def failing_lstat(self, error=failure):
                    lstat_calls["count"] += 1
                    if failure_point in {"lstat", "post_lstat"}:
                        if lstat_calls["count"] == 1:
                            raise FileNotFoundError
                        raise error
                    return original_lstat(self)

                def failing_mkdir(self, *args, error=failure, **kwargs):
                    if failure_point == "mkdir":
                        raise error
                    if failure_point == "post_lstat":
                        return None
                    return original_mkdir(self, *args, **kwargs)

                path_type.exists = failing_exists
                path_type.is_symlink = false_is_symlink
                path_type.lstat = failing_lstat
                path_type.mkdir = failing_mkdir
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._ensure_output_directory(
                            ADAPTER.Path("receipts"),
                            "receipt directory",
                        )
                finally:
                    path_type.exists = original_exists
                    path_type.is_symlink = original_is_symlink
                    path_type.lstat = original_lstat
                    path_type.mkdir = original_mkdir

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                if isinstance(failure, OSError):
                    self.assertIs(caught.exception.__cause__, failure)
                else:
                    self.assertIsNone(caught.exception.__cause__)
                    self.assertTrue(caught.exception.__suppress_context__)

    def test_text_output_write_and_replace_failures_do_not_echo_os_detail(self):
        hidden = "token=rail-output-write-secret"
        cases = (
            ("fsync", None, "cannot write temporary output for receipt output"),
            ("replace", None, "cannot replace receipt output"),
            ("fsync_runtime", None, "cannot write temporary output for receipt output"),
            ("replace_runtime", None, "cannot replace receipt output"),
            ("fsync", "unlink", "cannot write temporary output for receipt output"),
            ("fsync", "close", "cannot write temporary output for receipt output"),
            ("fsync", "unlink_runtime", "cannot write temporary output for receipt output"),
            ("fsync", "close_runtime", "cannot write temporary output for receipt output"),
        )
        for failure, cleanup_failure, expected in cases:
            with self.subTest(
                failure=failure,
                cleanup_failure=cleanup_failure,
            ), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                output = root / "rail.receipt.json"
                cleanup_hidden = f"{hidden}-cleanup"
                original_fsync = ADAPTER.os.fsync
                original_replace = ADAPTER.os.replace
                original_unlink = ADAPTER.os.unlink
                original_close = ADAPTER.os.close

                def failing_fsync(fd):
                    if failure == "fsync":
                        raise OSError(5, hidden)
                    if failure == "fsync_runtime":
                        raise RuntimeError(hidden)
                    return original_fsync(fd)

                def failing_replace(*args, **kwargs):
                    if failure == "replace":
                        raise OSError(5, hidden)
                    if failure == "replace_runtime":
                        raise RuntimeError(hidden)
                    return original_replace(*args, **kwargs)

                def failing_unlink(*args, **kwargs):
                    if cleanup_failure == "unlink":
                        raise OSError(5, cleanup_hidden)
                    if cleanup_failure == "unlink_runtime":
                        raise RuntimeError(cleanup_hidden)
                    return original_unlink(*args, **kwargs)

                def failing_close(fd):
                    if cleanup_failure == "close":
                        raise OSError(5, cleanup_hidden)
                    if cleanup_failure == "close_runtime":
                        raise RuntimeError(cleanup_hidden)
                    return original_close(fd)

                ADAPTER.os.fsync = failing_fsync
                ADAPTER.os.replace = failing_replace
                ADAPTER.os.unlink = failing_unlink
                ADAPTER.os.close = failing_close
                try:
                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER._write_text_output(
                            output,
                            "{}\n",
                            display_label="receipt output",
                        )
                finally:
                    ADAPTER.os.fsync = original_fsync
                    ADAPTER.os.replace = original_replace
                    ADAPTER.os.unlink = original_unlink
                    ADAPTER.os.close = original_close

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertIn("I/O error", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn(cleanup_hidden, message)
                self.assertNotIn(str(root), message)

    def test_secret_looking_unknown_keys_are_rejected_without_echo(self):
        cases = (
            ("password_rail_unknown_secret", "rail_unknown_secret"),
            ("%70assword_rail_unknown_leak", "rail_unknown_leak"),
            ("private-key_rail_unknown_leak", "rail_unknown_leak"),
            ("private--key_rail_unknown_leak", "rail_unknown_leak"),
            ("private%09key_rail_unknown_leak", "rail_unknown_leak"),
            ("x--iroha--signature_rail_unknown_leak", "rail_unknown_leak"),
            ("unexpected\x1brail_key", "\x1b"),
            ("unexpected_rail_\uff4bey", "\uff4b"),
            ("operator_note", "operator_note"),
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

    def test_separator_smuggled_secret_identifiers_are_detected(self):
        cases = (
            "private\tkey rail identifier",
            "private--key rail identifier",
            "private/key rail identifier",
            "private\\key rail identifier",
            "private%2fkey rail identifier",
            "private\u200dkey rail identifier",
            "private\u0301key rail identifier",
            "ｐｒｉｖａｔｅｋｅｙ rail identifier",
            "x--iroha--signature rail identifier",
            "x/iroha/signature rail identifier",
            "x%2firoha%2fsignature rail identifier",
            "x\u200diroha\u200dsignature rail identifier",
            "x\u0301iroha\u0301signature rail identifier",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ rail identifier",
            "token%09secret rail identifier",
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
            "upstream private\tkey rail leak",
            "upstream private/key rail leak",
            "upstream private%2fkey rail leak",
            "upstream private\u200dkey rail leak",
            "upstream private\u0301key rail leak",
            "upstream ｐｒｉｖａｔｅｋｅｙ rail leak",
            "upstream x--iroha--signature rail leak",
            "upstream x/iroha/signature rail leak",
            "upstream x%2firoha%2fsignature rail leak",
            "upstream x\u200diroha\u200dsignature rail leak",
            "upstream x\u0301iroha\u0301signature rail leak",
            "upstream ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ rail leak",
            "upstream token%09secret rail leak",
        )
        for preview in cases:
            with self.subTest(preview=preview):
                self.assertTrue(ADAPTER._response_preview_looks_secret(preview))
                self.assertEqual(
                    ADAPTER.REDACTED_RESPONSE_PREVIEW,
                    ADAPTER._response_preview(preview.encode("utf-8")),
                )

    def test_path_separator_secret_key_values_are_detected(self):
        cases = (
            "private/key=rail-value-secret",
            "api/key:rail-value-secret",
            "client/secret=rail-value-secret",
            "set/cookie:rail-value-secret",
            "x/iroha/signature: rail-value-secret",
            "private%2fkey=rail-value-secret",
            "private\u200dkey=rail-value-secret",
            "private\u0301key=rail-value-secret",
            "ｐｒｉｖａｔｅｋｅｙ=rail-compat-secret",
            "ａｐｉ／ｋｅｙ:rail-compat-secret",
            "x\u200diroha\u200dsignature: rail-value-secret",
            "x\u0301iroha\u0301signature: rail-value-secret",
            "ｘ／ｉｒｏｈａ／ｓｉｇｎａｔｕｒｅ: rail-compat-secret",
            "private%E2%80%8Dkey=rail-value-secret",
            "private%CC%81key=rail-value-secret",
        )
        for value in cases:
            with self.subTest(value=value):
                self.assertTrue(ADAPTER._contains_secret_material(value))

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

    def test_direct_main_argv_inputs_are_normalized_before_preflight(self):
        hidden = "token=rail-argv-secret"

        class HostileArgv(list):
            def __len__(self):
                raise RuntimeError(f"len={hidden}")

            def __iter__(self):
                raise RuntimeError(f"iter={hidden}")

            def __getitem__(self, _key):
                raise RuntimeError(f"item={hidden}")

        class HostileText(str):
            def __str__(self):
                raise RuntimeError(f"str={hidden}")

            def startswith(self, _prefix, *_args):
                raise RuntimeError(f"startswith={hidden}")

            def strip(self, *_args):
                raise RuntimeError(f"strip={hidden}")

        cases = (
            (
                "container",
                HostileArgv(["--receipt-dir"]),
                "argv must be a plain argument list",
            ),
            ("tuple", ("--receipt-dir",), "argv must be a plain argument list"),
            ("non-string", [object()], "argv[0] must be a string"),
            (
                "hostile-string",
                [HostileText("--receipt-dir")],
                "--receipt-dir requires a path value",
            ),
        )
        for name, argv, expected in cases:
            with self.subTest(name=name):
                rc, stdout, stderr = run_main(argv)

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertIn(expected, stderr)
                self.assertNotIn(hidden, stderr)
                self.assertNotIn("rail-argv-secret", stderr)

    def test_raw_cli_control_characters_are_rejected_without_echo(self):
        cases = ("--unknown-rail\x1bflag", "--unknown-rail\u202eflag")
        for hidden in cases:
            with self.subTest(hidden=hidden):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._preflight_raw_cli_secrets([hidden], {"--receipt-dir"})

                message = str(caught.exception)
                self.assertIn("CLI argument must not contain control characters", message)
                self.assertNotIn(hidden, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn("\u202e", message)
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
                {"metadata": {"unexpected\u202erail_key": "redacted"}},
                "forbidden control-bearing field",
                "rail_key",
            ),
            (
                {"metadata": {"note": "warning \x1b[31mred"}},
                "unsafe control characters",
                "[31mred",
            ),
            (
                {"metadata": {"note": "warning \u202erail-bidi-leak"}},
                "unsafe control characters",
                "rail-bidi-leak",
            ),
            (
                {"metadata": {"note": "private%E2%80%8Dkey=rail-field-leak"}},
                "secret-looking material",
                "rail-field-leak",
            ),
            (
                {"metadata": {"note": "private%CC%81key=rail-mark-leak"}},
                "secret-looking material",
                "rail-mark-leak",
            ),
            (
                {"metadata": {"note": "ｐｒｉｖａｔｅｋｅｙ=rail-compat-leak"}},
                "secret-looking material",
                "rail-compat-leak",
            ),
        )
        for body, expected, hidden in cases:
            with self.subTest(body=body):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._check_no_secret_material(body, "sidecar")

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("\x1b", message)
                self.assertNotIn("\u202e", message)
                self.assertNotIn(hidden, message)

    def test_sidecar_string_helpers_normalize_hostile_str_subclasses_without_echo(self):
        hidden = "rail-hostile-string-secret"

        class HostileText(str):
            def __str__(self):
                raise RuntimeError(f"token={hidden}")

            def strip(self, *_args, **_kwargs):
                raise RuntimeError(f"client_secret={hidden}")

            def __iter__(self):
                raise KeyError(f"private_key={hidden}")

        class HostileKey:
            def __str__(self):
                raise RuntimeError(f"key={hidden}")

        class HostileList(list):
            def __len__(self):
                raise RuntimeError(f"list={hidden}")

            def __iter__(self):
                raise RuntimeError(f"list_iter={hidden}")

        class HostileDict(dict):
            def __len__(self):
                raise RuntimeError(f"dict={hidden}")

            def __iter__(self):
                raise RuntimeError(f"dict_iter={hidden}")

            def items(self):
                raise RuntimeError(f"dict_items={hidden}")

        required = ADAPTER._required_cli_string(
            HostileText("message.xml"),
            "--message",
        )
        self.assertEqual(required, "message.xml")
        self.assertIs(type(required), str)

        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            xml_path = root / "message.xml"
            xml_path.write_bytes(SAMPLE_XML)
            sidecar = {
                "message_type": HostileText("pacs.008"),
                "payload_sha256": ADAPTER.sha256_hex(SAMPLE_XML),
                "profile": HostileText("swift-cbpr-plus"),
                "rail_message_id": HostileText("rail-message-001"),
            }
            original_load_json = ADAPTER._load_json

            def patched_load_json(*_args, **_kwargs):
                return sidecar

            ADAPTER._load_json = patched_load_json
            try:
                message = ADAPTER.verify_message_file(
                    xml_path,
                    max_payload_bytes=len(SAMPLE_XML) + 1,
                    allow_default_profile=False,
                    allow_legacy_colr007=False,
                )
            finally:
                ADAPTER._load_json = original_load_json

            self.assertEqual(message.message_type, "pacs.008")
            self.assertIs(type(message.message_type), str)
            self.assertEqual(message.profile, "swift-cbpr-plus")
            self.assertIs(type(message.profile), str)
            self.assertEqual(message.rail_message_id, "rail-message-001")
            self.assertIs(type(message.rail_message_id), str)

        def verify_hostile_sidecar_shape():
            with tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                xml_path = root / "message.xml"
                xml_path.write_bytes(SAMPLE_XML)
                original_load_json = ADAPTER._load_json

                def patched_load_json(*_args, **_kwargs):
                    return HostileDict(
                        {
                            "message_type": "pacs.008",
                            "payload_sha256": ADAPTER.sha256_hex(SAMPLE_XML),
                        }
                    )

                ADAPTER._load_json = patched_load_json
                try:
                    ADAPTER.verify_message_file(
                        xml_path,
                        max_payload_bytes=len(SAMPLE_XML) + 1,
                        allow_default_profile=True,
                        allow_legacy_colr007=False,
                    )
                finally:
                    ADAPTER._load_json = original_load_json

        present = ADAPTER._reject_unknown_keys(
            {HostileText("message_type"): "pacs.008"},
            {"message_type"},
            "sidecar",
        )
        self.assertEqual(present, {"message_type"})
        self.assertTrue(all(type(key) is str for key in present))

        cases = (
            (
                "secret-scan",
                lambda: ADAPTER._check_no_secret_material(
                    {"metadata": HostileText("warning \x1b[31mred")},
                    "sidecar",
                ),
            ),
            (
                "surrogate-string",
                lambda: ADAPTER._reject_json_surrogates(HostileText("ok\ud800")),
            ),
            (
                "surrogate-list-subclass",
                lambda: ADAPTER._reject_json_surrogates(HostileList(["ok"])),
            ),
            (
                "surrogate-dict-subclass",
                lambda: ADAPTER._reject_json_surrogates(HostileDict({"metadata": "ok"})),
            ),
            (
                "loaded-sidecar-dict-subclass",
                verify_hostile_sidecar_shape,
            ),
            (
                "secret-key",
                lambda: ADAPTER._check_no_secret_material(
                    {HostileText("private_key"): "redacted"},
                    "sidecar",
                ),
            ),
            (
                "control-key",
                lambda: ADAPTER._check_no_secret_material(
                    {HostileText("metadata\x1b"): "redacted"},
                    "sidecar",
                ),
            ),
            (
                "unknown-key",
                lambda: ADAPTER._reject_unknown_keys(
                    {HostileText("unknown\x1b"): "redacted"},
                    {"message_type"},
                    "sidecar",
                ),
            ),
            (
                "non-string-key",
                lambda: ADAPTER._check_no_secret_material(
                    {HostileKey(): "redacted"},
                    "sidecar",
                ),
            ),
            (
                "unknown-non-string-key",
                lambda: ADAPTER._reject_unknown_keys(
                    {HostileKey(): "redacted"},
                    {"message_type"},
                    "sidecar",
                ),
            ),
            (
                "unknown-dict-subclass",
                lambda: ADAPTER._reject_unknown_keys(
                    HostileDict({"message_type": "redacted"}),
                    {"message_type"},
                    "sidecar",
                ),
            ),
            (
                "secret-list-subclass",
                lambda: ADAPTER._check_no_secret_material(
                    HostileList(["redacted"]),
                    "sidecar",
                ),
            ),
            (
                "secret-dict-subclass",
                lambda: ADAPTER._check_no_secret_material(
                    HostileDict({"metadata": "redacted"}),
                    "sidecar",
                ),
            ),
        )
        for name, call in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    call()
                message = str(caught.exception)
                self.assertNotIn(hidden, message)
                self.assertIsNone(caught.exception.__cause__)
                self.assertIsNone(caught.exception.__context__)

    def test_recursive_json_array_scans_are_count_bounded_without_echo(self):
        items = [None] * (ADAPTER.MAX_JSON_LIST_ITEMS + 1)
        cases = (
            (
                "surrogates",
                lambda: ADAPTER._reject_json_surrogates(items),
                f"JSON array must contain at most {ADAPTER.MAX_JSON_LIST_ITEMS} items",
            ),
            (
                "secret scan",
                lambda: ADAPTER._check_no_secret_material(items, "sidecar.extra"),
                f"sidecar.extra must contain at most {ADAPTER.MAX_JSON_LIST_ITEMS} items",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    action()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(str(len(items)), message)
                self.assertNotIn("[0]", message)

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
            (
                "secret scan",
                lambda: ADAPTER._check_no_secret_material(members, "sidecar.extra"),
                f"sidecar.extra must contain at most {ADAPTER.MAX_JSON_OBJECT_MEMBERS} object members",
            ),
        )
        for name, action, expected in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    action()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn(str(len(members)), message)
                self.assertNotIn("hidden_key_0", message)

    def test_recursive_json_depth_scans_are_bounded_without_echo(self):
        nested = "hidden_leaf"
        for _ in range(ADAPTER.MAX_JSON_NESTING_DEPTH + 1):
            nested = [nested]
        expected = (
            f"JSON nesting depth must be at most {ADAPTER.MAX_JSON_NESTING_DEPTH} levels"
        )
        cases = (
            ("surrogates", lambda: ADAPTER._reject_json_surrogates(nested)),
            ("secret scan", lambda: ADAPTER._check_no_secret_material(nested, "sidecar.extra")),
        )
        for name, action in cases:
            with self.subTest(name=name):
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    action()

                message = str(caught.exception)
                self.assertIn(expected, message)
                self.assertNotIn("hidden_leaf", message)
                self.assertNotIn("[0]", message)

    def test_json_parse_recursion_error_is_bounded_without_echo(self):
        hidden = "hidden-rail-recursion"
        with tempfile.TemporaryDirectory() as raw_root:
            path = Path(raw_root) / hidden
            path.write_text("[]\n", encoding="utf-8")
            original_loads = ADAPTER.json.loads

            def raising_loads(*_args, **_kwargs):
                raise RecursionError(hidden)

            ADAPTER.json.loads = raising_loads
            try:
                with self.assertRaises(ADAPTER.AdapterError) as caught:
                    ADAPTER._load_json(path, display_label="sidecar")
            finally:
                ADAPTER.json.loads = original_loads

        message = str(caught.exception)
        self.assertIn(
            f"JSON nesting depth must be at most {ADAPTER.MAX_JSON_NESTING_DEPTH} levels",
            message,
        )
        self.assertNotIn(hidden, message)
        self.assertNotIn(str(path), message)

    def test_unicode_format_response_preview_is_redacted_without_echo(self):
        preview = "upstream rail \u202erail-bidi-leak"

        self.assertTrue(ADAPTER._contains_unsafe_preview_control(preview))
        self.assertEqual(
            ADAPTER.REDACTED_RESPONSE_PREVIEW,
            ADAPTER._response_preview(preview.encode("utf-8")),
        )
        self.assertNotIn(
            "rail-bidi-leak",
            ADAPTER._response_preview(preview.encode("utf-8")),
        )

    def test_non_ascii_response_preview_is_redacted_without_echo(self):
        cases = (
            ("unicode", "upstream rail caf\u00e9 hidden-rail-unicode".encode("utf-8")),
            ("invalid-utf8", b"upstream rail \xff hidden-rail-binary"),
        )
        for name, body in cases:
            with self.subTest(name=name):
                self.assertEqual(
                    ADAPTER.REDACTED_RESPONSE_PREVIEW,
                    ADAPTER._response_preview(body),
                )
                self.assertNotIn("hidden-rail", ADAPTER._response_preview(body))

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
            ("private%20key%3Drail-path-leak.receipts", "private key=rail-path-leak"),
            ("private%20key-rail-path-secret.receipts", "private key-rail-path-secret"),
            ("private/key-rail-path-secret.receipts", "private/key-rail-path-secret"),
            ("x%2firoha%2fsignature-rail-path-secret.receipts", "x/iroha/signature-rail-path-secret"),
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

    def test_receipt_dir_cannot_reuse_inbox_dir_before_inbox_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "missing-inbox"

            rc, stdout, stderr = run_main(
                [
                    "--inbox-dir",
                    str(inbox),
                    "--receipt-dir",
                    str(inbox),
                    "--torii-base-url",
                    "https://torii.example.internal",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt_dir must not reuse inbox_dir path", stderr)
            self.assertNotIn("does not exist", stderr)

    def test_receipt_dir_cannot_symlink_to_inbox_dir_before_inbox_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            receipt_dir = root / "receipt-link"
            try:
                receipt_dir.symlink_to(inbox, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")

            rc, stdout, stderr = run_main(
                [
                    "--inbox-dir",
                    str(inbox),
                    "--receipt-dir",
                    str(receipt_dir),
                    "--torii-base-url",
                    "https://torii.example.internal",
                ]
            )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn("receipt_dir must not reuse inbox_dir path", stderr)
            self.assertNotIn("has no XML messages", stderr)

    def test_symlinked_receipt_dir_ancestor_is_rejected_before_inbox_loading(self):
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
                    "--inbox-dir",
                    str(root / "missing-inbox"),
                    "--receipt-dir",
                    str(receipt_dir),
                    "--torii-base-url",
                    "https://torii.example.internal",
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
                "raw format control",
                lambda raw: ADAPTER._reject_raw_output_path_smuggling(raw, "raw path"),
                "out/\u202ereceipts",
                "control characters",
            ),
            (
                "output format control",
                lambda raw: ADAPTER._reject_output_path_smuggling(Path(raw), "output path"),
                "out/\u202ereceipts",
                "control characters",
            ),
            (
                "message format control",
                lambda raw: ADAPTER._validate_path_argument(raw, "--message path"),
                "nested/\u202erail-status.xml",
                "control characters",
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
                "https://torii.local-bank.bank/base\u202edebug/v1",
                "URL must not contain control characters",
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
            ["--torii-base-url", "-receipt-dir"],
            ["--torii-base-url="],
            ["--torii-base-url=--receipt-dir"],
            ["--torii-base-url=-receipt-dir"],
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

    def test_numeric_cli_flags_reject_noncanonical_decimal_spellings_before_network(self):
        cases = (
            ["--max-payload-bytes", "000512"],
            ["--max-payload-bytes", "-0"],
            ["--response-limit-bytes", "+512"],
            ["--response-limit-bytes=001024"],
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
        cases = (
            (
                "https://torii.local-bank.bank/base\u2215debug",
                "--torii-base-url URL must use printable ASCII",
            ),
            (
                "https://torii.local-bank.bank/base\u202edebug",
                "--torii-base-url URL must not contain control characters",
            ),
        )
        for hidden, expected in cases:
            for argv in (["--torii-base-url", hidden], [f"--torii-base-url={hidden}"]):
                with self.subTest(argv=argv):
                    rc, _stdout, stderr = run_main(argv)

                    self.assertEqual(rc, 2)
                    self.assertIn(expected, stderr)
                    self.assertNotIn(hidden, stderr)
                    self.assertNotIn("\u202e", stderr)
                    self.assertNotIn("inbox_dir", stderr)

    def test_sidecar_header_strings_reject_unicode_format_controls_without_echo(self):
        cases = [
            (
                "profile format control",
                "profile",
                "swift\u202ecbpr-plus",
                "profile contains unsafe control characters",
                "cbpr-plus",
            ),
            (
                "rail message format control",
                "rail_message_id",
                "rail\u202edrop-1",
                "rail_message_id contains unsafe control characters",
                "drop-1",
            ),
        ]
        for label, field, value, message, hidden in cases:
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
                self.assertNotIn("inbox_dir", stderr)
                self.assertNotIn(value, stderr)
                self.assertNotIn("\u202e", stderr)
                self.assertNotIn(hidden, stderr)

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

    def test_torii_url_rejects_unsupported_internal_message_type_without_echo(self):
        secret_type = "token=rail-route-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("/ops/rail/message.xml"),
            sidecar_path=Path("/ops/rail/message.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type=secret_type,
            profile="swift-cbpr-plus",
            rail_message_id="rail-drop-1",
        )

        with self.assertRaisesRegex(ADAPTER.AdapterError, "unsupported message_type") as ctx:
            ADAPTER.torii_url("https://torii.example.invalid", message)

        self.assertNotIn(secret_type, str(ctx.exception))

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
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            token_dir = root / "runtime"
            token_dir.mkdir()
            token_file = token_dir / "token.txt"
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

    def test_payload_digest_mismatch_is_rejected_without_echo_before_network_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            xml_path, sidecar = write_message(inbox)
            expected_sha256 = "1" * 64
            actual_sha256 = ADAPTER.sha256_hex(SAMPLE_XML)
            self.assertNotEqual(expected_sha256, actual_sha256)
            sidecar["payload_sha256"] = expected_sha256
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
            self.assertIn("payload_sha256 mismatch", stderr)
            self.assertNotIn(expected_sha256, stderr)
            self.assertNotIn(actual_sha256, stderr)
            self.assertFalse((inbox / "receipts").exists())

    def test_malformed_source_paths_do_not_echo_paths_before_network_delivery(self):
        def malformed_sidecar(inbox):
            xml_path, _sidecar = write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            sidecar_path.write_text("{not-json\n", encoding="utf-8")
            return xml_path, sidecar_path, "is not valid JSON", []

        def oversized_xml(inbox):
            xml_path, _sidecar = write_message(inbox)
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            return (
                xml_path,
                sidecar_path,
                "byte payload limit",
                ["--max-payload-bytes", str(len(SAMPLE_XML) - 1)],
            )

        def payload_mismatch(inbox):
            xml_path, sidecar = write_message(inbox)
            sidecar["payload_sha256"] = "1" * 64
            sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
            sidecar_path.write_text(json.dumps(sidecar), encoding="utf-8")
            return xml_path, sidecar_path, "payload_sha256 mismatch", []

        cases = (
            ("malformed-sidecar", malformed_sidecar),
            ("oversized-xml", oversized_xml),
            ("payload-mismatch", payload_mismatch),
        )
        for name, arrange in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_root:
                root = Path(raw_root)
                hidden = f"hidden-rail-source-{name}"
                inbox = root / hidden / "inbox"
                inbox.mkdir(parents=True)
                xml_path, sidecar_path, expected, extra_args = arrange(inbox)

                with capture_server() as (base_url, requests):
                    rc, stdout, stderr = run_main(
                        [
                            "--inbox-dir",
                            str(inbox),
                            "--torii-base-url",
                            base_url,
                            "--allow-insecure-http",
                            *extra_args,
                        ]
                    )

                self.assertEqual(rc, 2)
                self.assertEqual(stdout, "")
                self.assertEqual(requests, [])
                self.assertIn(expected, stderr)
                self.assertNotIn("line 1 column", stderr)
                self.assertNotIn("(char ", stderr)
                self.assertNotIn(hidden, stderr)
                self.assertNotIn(str(xml_path), stderr)
                self.assertNotIn(str(sidecar_path), stderr)
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

    def test_checked_in_xml_fixture_diagnostic_does_not_echo_message_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            hidden = "hidden-rail-fixture-path"
            xml_path = root / hidden / "fixtures" / "iso20022" / "rail-status.xml"
            xml_path.parent.mkdir(parents=True)

            with self.assertRaises(ADAPTER.AdapterError) as caught:
                ADAPTER.verify_message_file(
                    xml_path,
                    max_payload_bytes=1024,
                    allow_default_profile=False,
                    allow_legacy_colr007=False,
                )

            message = str(caught.exception)
            self.assertIn(
                "message XML payload must not point to checked-in ISO XML fixtures",
                message,
            )
            self.assertNotIn(hidden, message)
            self.assertNotIn(str(xml_path), message)

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

    def test_unsupported_sidecar_message_type_is_rejected_without_echo(self):
        hidden = "zzzz.999"
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
            self.assertIn("unsupported message_type", stderr)
            self.assertNotIn(hidden, stderr)

    def test_malformed_bearer_token_file_is_rejected_before_network_delivery(self):
        cases = [
            ("empty", b"", "empty"),
            ("padded", b" rail-token", "surrounding whitespace"),
            ("newline", b"rail-token\n", "surrounding whitespace"),
            ("embedded-space", b"rail token", "must not contain whitespace"),
            ("control", b"rail-token\x7f", "must not contain control characters"),
            (
                "unicode-format",
                "rail-token\u200drail-token-hidden".encode("utf-8"),
                "must not contain control characters",
            ),
            ("non-utf8", b"rail-token\xff", "not UTF-8"),
            (
                "oversized",
                b"a" * (ADAPTER.MAX_BEARER_TOKEN_BYTES + 1),
                "exceeds",
            ),
        ]
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            token_dir = root / "runtime"
            token_dir.mkdir()
            for name, token_bytes, message in cases:
                with self.subTest(name=name):
                    token_file = token_dir / f"{name}.token"
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
                    self.assertNotIn("rail-token-hidden", stderr)

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
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            token_dir = root / "runtime"
            token_dir.mkdir()
            token_target = token_dir / "token-target.txt"
            token_target.write_text("rail-token-123", encoding="utf-8")
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
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            target_dir = root / "token-target"
            target_dir.mkdir()
            token_target = target_dir / "token.txt"
            token_target.write_text("rail-token-123", encoding="utf-8")
            ancestor = root / "token-ancestor-link"
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

    def test_bearer_token_file_cannot_overlap_inbox_before_loading(self):
        cases = (
            (
                "same-as-inbox",
                lambda root, inbox: inbox,
                "rail-token-source-same",
            ),
            (
                "inside-inbox",
                lambda root, inbox: inbox / "runtime-auth.txt",
                "rail-token-source-nested",
            ),
            (
                "ancestor-of-inbox",
                lambda root, inbox: inbox.parent,
                "rail-token-source-ancestor",
            ),
        )
        for name, token_path_for, hidden in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    if name == "ancestor-of-inbox":
                        source_root = root / "rail-token-source-ancestor"
                        inbox = source_root / "inbox"
                    else:
                        inbox = root / "inbox"
                    inbox.mkdir(parents=True)
                    write_message(inbox)
                    token_file = token_path_for(root, inbox)
                    if token_file.suffix:
                        token_file.write_text("rail-token-123\n", encoding="utf-8")
                    receipt_dir = root / "receipts"

                    with capture_server() as (base_url, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
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
                        "bearer_token_file must not overlap inbox_dir path",
                        stderr,
                    )
                    self.assertNotIn("rail-token-123", stderr)
                    self.assertNotIn(hidden, stderr)

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
            args = args_for(root)
            delattr(args, "inbox_dir")
            with self.assertRaisesRegex(ADAPTER.AdapterError, "provide --inbox-dir"):
                ADAPTER.run(args)

    def test_direct_run_scalar_paths_must_be_paths_before_inbox_loading(self):
        hidden = "rail-hostile-pathlike-secret"

        class HostilePathLike:
            def __fspath__(self):
                raise RuntimeError(f"fspath={hidden}")

        cases = (
            ("inbox", "inbox_dir", object(), "inbox_dir"),
            ("inbox pathlike", "inbox_dir", HostilePathLike(), "inbox_dir"),
            ("receipt", "receipt_dir", object(), "receipt_dir"),
            ("token", "bearer_token_file", object(), "bearer_token_file"),
        )
        for name, field, value, label in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        inbox_dir=root / "missing-inbox",
                        message=None,
                        torii_base_url="https://torii.example.invalid",
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        max_payload_bytes=1024,
                        allow_insecure_http=False,
                        allow_default_profile=False,
                        allow_legacy_colr007=False,
                        dry_run=True,
                    )
                    setattr(args, field, value)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a path", message)
                    self.assertNotIn(hidden, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_message_must_be_safe_path_before_inbox_loading(self):
        pathlike_hidden = "rail-hostile-message-pathlike-secret"
        list_hidden = "rail-hostile-message-list-secret"

        class HostilePathLike:
            def __fspath__(self):
                raise RuntimeError(f"fspath={pathlike_hidden}")

        class HostileList(list):
            def __iter__(self):
                raise RuntimeError(f"iter={list_hidden}")

            def __len__(self):
                raise RuntimeError(f"len={list_hidden}")

        cases = (
            ("pathlike", HostilePathLike(), "message must be a path"),
            ("list subclass", HostileList(["rail-status.xml"]), "message must be a path"),
            ("bytes", b"rail-status.xml", "message must be a path"),
        )
        for name, value, expected in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        inbox_dir=root / "missing-inbox",
                        message=value,
                        torii_base_url="https://torii.example.invalid",
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        max_payload_bytes=1024,
                        allow_insecure_http=False,
                        allow_default_profile=False,
                        allow_legacy_colr007=False,
                        dry_run=True,
                    )

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn(pathlike_hidden, message)
                    self.assertNotIn(list_hidden, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_torii_base_url_must_be_string_before_inbox_loading(self):
        cases = (
            ("missing", None),
            ("object", object()),
            ("message omitted", object()),
        )
        for name, value in cases:
            with self.subTest(name=name):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        inbox_dir=root / "missing-inbox",
                        message=None,
                        torii_base_url=value,
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        max_payload_bytes=1024,
                        allow_insecure_http=False,
                        allow_default_profile=False,
                        allow_legacy_colr007=False,
                        dry_run=True,
                    )
                    if name == "message omitted":
                        delattr(args, "message")

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn("--torii-base-url must be a string", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_policy_flags_must_be_booleans_before_inbox_loading(self):
        cases = (
            ("dry_run", "--dry-run", "true"),
            ("allow_insecure_http", "--allow-insecure-http", 1),
            ("allow_default_profile", "--allow-default-profile", None),
            ("allow_legacy_colr007", "--allow-legacy-colr007", []),
        )
        for attr, label, value in cases:
            with self.subTest(flag=label):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        inbox_dir=root / "missing-inbox",
                        message=None,
                        torii_base_url="https://torii.example.invalid",
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        max_payload_bytes=1024,
                        allow_insecure_http=False,
                        allow_default_profile=False,
                        allow_legacy_colr007=False,
                        dry_run=True,
                    )
                    setattr(args, attr, value)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(f"{label} must be a boolean", message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_numeric_limits_must_exist_before_inbox_loading(self):
        cases = (
            ("timeout_secs", "--timeout-secs must be a positive finite number"),
            ("response_limit_bytes", "--response-limit-bytes must be a positive integer"),
            ("max_payload_bytes", "--max-payload-bytes must be a positive integer"),
        )
        for field, expected in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as raw_root:
                    root = Path(raw_root)
                    args = argparse.Namespace(
                        inbox_dir=root / "missing-inbox",
                        message=None,
                        torii_base_url="https://torii.example.invalid",
                        receipt_dir=root / "receipts",
                        bearer_token_file=None,
                        timeout_secs=1.0,
                        response_limit_bytes=1024,
                        max_payload_bytes=1024,
                        allow_insecure_http=False,
                        allow_default_profile=False,
                        allow_legacy_colr007=False,
                        dry_run=True,
                    )
                    delattr(args, field)

                    with self.assertRaises(ADAPTER.AdapterError) as caught:
                        ADAPTER.run(args)

                    message = str(caught.exception)
                    self.assertIn(expected, message)
                    self.assertNotIn("does not exist", message)
                    self.assertNotIn(str(root), message)

    def test_direct_run_response_limit_is_capped_before_inbox_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            args = argparse.Namespace(
                inbox_dir=root / "missing-inbox",
                message=None,
                torii_base_url="https://torii.example.invalid",
                receipt_dir=root / "receipts",
                bearer_token_file=None,
                timeout_secs=1.0,
                response_limit_bytes=ADAPTER.MAX_RESPONSE_LIMIT_BYTES + 1,
                max_payload_bytes=1024,
                allow_insecure_http=False,
                allow_default_profile=False,
                allow_legacy_colr007=False,
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

    def test_inbox_dir_discovery_path_diagnostics_do_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            missing_dir = root / "missing-inbox"
            inbox_file = root / "inbox-as-file"
            inbox_file.write_text("not a directory\n", encoding="utf-8")
            empty_dir = root / "empty-inbox"
            empty_dir.mkdir()

            cases = (
                (missing_dir, "does not exist"),
                (inbox_file, "must be a directory"),
                (empty_dir, "has no *.xml gateway messages"),
            )
            for path, message in cases:
                with self.subTest(path=path.name):
                    with capture_server() as (base_url, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(path),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertEqual(requests, [])
                    self.assertIn("inbox_dir", stderr)
                    self.assertIn(message, stderr)
                    self.assertNotIn(str(path), stderr)
                    self.assertNotIn(path.name, stderr)

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

    def test_receipt_output_dir_path_diagnostics_do_not_echo_path(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            receipt_file = inbox / "receipt-dir-as-file"
            receipt_file.write_text("not a directory\n", encoding="utf-8")

            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
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
            self.assertIn("receipt_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(receipt_dir), stderr)
            self.assertNotIn(receipt_dir.name, stderr)

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
            self.assertIn("receipt_output[0]", stderr)
            self.assertIn("must not be hard-linked", stderr)
            self.assertNotIn(str(receipt_path), stderr)
            self.assertNotIn(receipt_path.name, stderr)
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
            self.assertIn("receipt_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(receipt_dir), stderr)
            self.assertNotIn(ancestor.name, stderr)
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
            self.assertIn("inbox_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(inbox), stderr)
            self.assertNotIn(inbox.name, stderr)

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
            self.assertIn("inbox_dir", stderr)
            self.assertIn("must not be a symlink", stderr)
            self.assertNotIn(str(inbox), stderr)
            self.assertNotIn(ancestor.name, stderr)

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
            self.assertNotIn("colr.007", stderr)
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

    def test_unused_insecure_http_override_is_rejected_before_delivery_and_receipts(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            calls = []
            original_submit_message = ADAPTER.submit_message

            def fake_submit_message(*args, **kwargs):
                calls.append((args, kwargs))
                return ADAPTER.SubmitResult(
                    status_code=202,
                    ok=True,
                    response_body_sha256=ADAPTER.sha256_hex(b"ok"),
                    response_body_preview="ok",
                )

            ADAPTER.submit_message = fake_submit_message
            try:
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        "https://torii.bank.internal/iso",
                        "--allow-insecure-http",
                    ]
                )
            finally:
                ADAPTER.submit_message = original_submit_message

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertIn(
                "--allow-insecure-http requires an http:// or local/private Torii URL",
                stderr,
            )
            self.assertEqual(calls, [])
            self.assertFalse((inbox / "receipts").exists())

    def test_unused_message_overrides_are_rejected_before_delivery_and_receipts(self):
        cases = (
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
                    with capture_server() as (base_url, requests):
                        rc, stdout, stderr = run_main(
                            [
                                "--inbox-dir",
                                str(inbox),
                                "--torii-base-url",
                                base_url,
                                "--allow-insecure-http",
                                flag,
                            ]
                        )

                    self.assertEqual(rc, 2)
                    self.assertEqual(stdout, "")
                    self.assertIn(message, stderr)
                    self.assertEqual(requests, [])
                    self.assertFalse((inbox / "receipts").exists())

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

    def test_explicit_message_under_receipt_dir_is_rejected_before_delivery(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            receipt_dir = inbox / "receipts"
            receipt_dir.mkdir()
            write_message(receipt_dir)
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--message",
                        "receipts/rail-status.xml",
                        "--torii-base-url",
                        base_url,
                        "--allow-insecure-http",
                    ]
                )

            self.assertEqual(rc, 2)
            self.assertEqual(stdout, "")
            self.assertEqual(requests, [])
            self.assertIn("message[0] must not be read from receipt_dir", stderr)
            self.assertEqual(list(receipt_dir.glob("*.receipt.json")), [])

    def test_receipt_dir_cannot_reuse_message_sidecar_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_inbox:
            inbox = Path(raw_inbox)
            write_message(inbox)
            receipt_dir = inbox / "rail-status.xml.json"
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
            self.assertIn("message[0].sidecar must not be read from receipt_dir", stderr)

    def test_receipt_dir_cannot_reuse_bearer_token_file_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            bearer_token_file = root / "adapter-auth.txt"
            bearer_token_file.write_text("rail-token-123\n", encoding="utf-8")
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
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
            self.assertNotIn("rail-token-123", stderr)
            self.assertEqual(bearer_token_file.read_text(encoding="utf-8"), "rail-token-123\n")

    def test_receipt_dir_cannot_contain_bearer_token_file_before_loading(self):
        with tempfile.TemporaryDirectory() as raw_root:
            root = Path(raw_root)
            inbox = root / "inbox"
            inbox.mkdir()
            write_message(inbox)
            token_dir = root / "runtime-auth"
            token_dir.mkdir()
            bearer_token_file = token_dir / "adapter-auth.txt"
            bearer_token_file.write_text("rail-token-123\n", encoding="utf-8")
            with capture_server() as (base_url, requests):
                rc, stdout, stderr = run_main(
                    [
                        "--inbox-dir",
                        str(inbox),
                        "--torii-base-url",
                        base_url,
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
            self.assertNotIn("rail-token-123", stderr)
            self.assertEqual(bearer_token_file.read_text(encoding="utf-8"), "rail-token-123\n")

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
            self.assertIn("--message path must stay under --inbox-dir", stderr)
            self.assertNotIn(str(outside_xml), stderr)
            self.assertNotIn(str(inbox), stderr)
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
            self.assertIn("non-finite numeric constant", stderr)
            self.assertNotIn("NaN", stderr)

    def test_noncanonical_sidecar_json_numbers_are_rejected_before_network_delivery(self):
        for value in ("1e01", "-0", "-0.0", "-0e0"):
            with self.subTest(value=value):
                with tempfile.TemporaryDirectory() as raw_inbox:
                    inbox = Path(raw_inbox)
                    _xml_path, sidecar = write_message(inbox)
                    (inbox / "rail-status.xml.json").write_text(
                        (
                            f'{{"message_type":"pacs.002","profile":"swift-cbpr-plus",'
                            f'"payload_sha256":"{sidecar["payload_sha256"]}",'
                            f'"rail_message_id":{value}}}\n'
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
                    self.assertIn("non-canonical numeric value", stderr)
                    self.assertNotIn(value, stderr)

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
            self.assertIn("contains unknown keys", stderr)
            self.assertNotIn("operator_note", stderr)

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
            (False, False),
            (True, False),
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
        self.assertEqual(result.error, "invalid HTTP status")
        self.assertNotIn("600", result.error)

    def test_http_status_parser_rejects_boolean_and_string_aliases(self):
        cases = (True, False, "202", "099", 202.0)
        for raw in cases:
            with self.subTest(raw=raw):
                self.assertIsNone(ADAPTER._parse_http_status_code(raw))

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
            self.assertEqual(receipt["error"], "invalid HTTP status")
            self.assertNotIn("700", receipt["error"])
            self.assertTrue(receipt_digest_matches(receipt))

    def test_malformed_torii_status_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-status-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_torii_status_accessor_failure_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-status-accessor-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingResponse:
            @property
            def status(self):
                raise RuntimeError(hidden)

            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                return FailingResponse()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_malformed_torii_error_status_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-error-status-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
                    "https://torii.example",
                    BrokenStatus(),
                    "failed",
                    {},
                    Body(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_torii_error_code_accessor_failure_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-error-code-accessor-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingCodeHttpError(ADAPTER.urllib.error.HTTPError):
            def __init__(self):
                Exception.__init__(self, hidden)

            @property
            def code(self):
                raise RuntimeError(hidden)

            def read(self, _limit):
                raise AssertionError("body must not be read after invalid status")

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise FailingCodeHttpError()

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_huge_torii_status_returns_failed_receipt_without_bloat(self):
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_huge_torii_error_status_returns_failed_receipt_without_bloat(self):
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
                    "https://torii.example",
                    HugeStatus(),
                    "failed",
                    {},
                    Body(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_multiline_torii_response_preview_is_folded_before_receipt_write(self):
        body = b"rejected\nerror: forged diagnostic\tcontinued"
        with tempfile.TemporaryDirectory() as raw_inbox:
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
                "rejected error: forged diagnostic continued",
            )
            self.assertNotIn("\\nerror: forged", receipt_text)
            self.assertNotIn("\\tcontinued", receipt_text)
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
            (b"Bearer\trail-secret", "rail-secret"),
            (b'{"message_id":"private key rail-secret"}', "private key"),
            (b'{"message_id":"x iroha signature rail-secret"}', "x iroha signature"),
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

    def test_non_ascii_success_response_fails_before_receipt_write(self):
        cases = (
            ("unicode", "rail caf\u00e9 hidden-rail-success".encode("utf-8")),
            ("invalid-utf8", b"rail \xff hidden-rail-success"),
        )
        for name, body in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw_inbox:
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
                self.assertIn("Torii response body contains non-ASCII text", stderr)
                self.assertNotIn("hidden-rail-success", stderr)
                self.assertEqual(list((inbox / "receipts").glob("*.receipt.json")), [])

    def test_url_error_receipt_error_uses_stable_label_without_echo(self):
        class BrokenReason:
            def __str__(self):
                raise RuntimeError("token=rail-url-error-secret")

        cases = (
            ADAPTER.urllib.error.URLError("connection refused"),
            ADAPTER.urllib.error.URLError("token=rail-url-error-secret"),
            ADAPTER.urllib.error.URLError("upstream \x1b[31mrail-warning"),
            ADAPTER.urllib.error.URLError("upstream r\u00e9seau"),
            ADAPTER.urllib.error.URLError("x" * 4097),
            ADAPTER.urllib.error.URLError(
                FileNotFoundError(2, "No such file or directory", "/tmp/rail.sock")
            ),
            ADAPTER.urllib.error.URLError(BrokenReason()),
        )
        for error in cases:
            with self.subTest(error=type(error.reason).__name__):
                receipt_error = ADAPTER._url_error_receipt_error(error)
                self.assertEqual(receipt_error, ADAPTER.URL_TRANSPORT_ERROR)
                self.assertNotIn("connection refused", receipt_error)
                self.assertNotIn("rail-url-error-secret", receipt_error)
                self.assertNotIn("rail-warning", receipt_error)
                self.assertNotIn("r\u00e9seau", receipt_error)
                self.assertNotIn("/tmp/rail.sock", receipt_error)

    def test_torii_transport_open_failure_returns_failed_receipt(self):
        hidden = "token=rail-open-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise OSError(hidden)

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii transport could not be opened")
        self.assertNotIn(hidden, result.error)

    def test_torii_transport_open_runtime_failure_returns_failed_receipt(self):
        hidden = "token=rail-open-runtime-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise RuntimeError(hidden)

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii transport could not be opened")
        self.assertNotIn(hidden, result.error)

    def test_torii_response_body_read_failure_returns_failed_receipt(self):
        hidden = "token=rail-read-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingResponse:
            status = 202

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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_torii_response_body_runtime_read_failure_returns_failed_receipt(self):
        hidden = "token=rail-runtime-read-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingResponse:
            status = 202

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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_torii_response_body_non_bytes_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-body-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class MalformedResponse:
            status = 202

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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii response body was not bytes")
        self.assertNotIn(hidden, result.error)

    def test_torii_response_body_bytes_like_values_are_capped_by_byte_length(self):
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
                        status = 202

                        def read(self, _limit):
                            return returned

                        def close(self):
                            return None

                    class BytesLikeOpener:
                        def open(self, *_args, **_kwargs):
                            return BytesLikeResponse()

                    ADAPTER.NO_REDIRECT_OPENER = BytesLikeOpener()
                    result = ADAPTER.submit_message(
                        "https://torii.example",
                        message,
                        timeout_secs=1.0,
                        response_limit_bytes=128,
                        bearer_token=None,
                    )

                    self.assertTrue(result.ok)
                    self.assertEqual(result.status_code, 202)
                    self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(expected))
                    self.assertEqual(result.response_body_preview, expected.decode("utf-8"))
                    self.assertIsNone(result.error)

            too_wide = memoryview(array.array("H", [0x4142] * 4))

            class OversizedResponse:
                status = 202

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
                "Torii response exceeded 3 byte limit",
            ):
                ADAPTER.submit_message(
                    "https://torii.example",
                    message,
                    timeout_secs=1.0,
                    response_limit_bytes=3,
                    bearer_token=None,
                )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

    def test_bounded_response_body_rejects_hostile_bytes_subclasses(self):
        hidden = "rail-response-body-subclass-secret"

        class HostileBytes(bytes):
            def __getitem__(self, _key):
                raise RuntimeError(f"bytes_getitem={hidden}")

            def __len__(self):
                raise RuntimeError(f"bytes_len={hidden}")

        class HostileBytearray(bytearray):
            def __getitem__(self, _key):
                raise RuntimeError(f"bytearray_getitem={hidden}")

            def __len__(self):
                raise RuntimeError(f"bytearray_len={hidden}")

        cases = (
            ("bytes-subclass", HostileBytes(b"accepted")),
            ("bytearray-subclass", HostileBytearray(b"accepted")),
        )
        for name, body in cases:
            with self.subTest(name=name):
                try:
                    result = ADAPTER._bounded_response_body(body, 4)
                except Exception as error:
                    self.fail(
                        "hostile response body method was invoked: "
                        f"{type(error).__name__}"
                    )
                self.assertIsNone(result)

    def test_torii_success_response_close_failure_preserves_receipt(self):
        hidden = "token=rail-close-secret"
        body = b"accepted"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class ClosingResponse:
            status = 202

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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertTrue(result.ok)
        self.assertEqual(result.status_code, 202)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertIsNone(result.error)

    def test_torii_failed_response_close_failure_preserves_receipt(self):
        hidden = "token=rail-close-secret"
        body = b"rejected"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_torii_response_close_lookup_failure_preserves_receipt(self):
        hidden = "token=rail-close-lookup-secret"
        body = b"accepted"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class ClosingResponse:
            status = 202

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
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
                timeout_secs=1.0,
                response_limit_bytes=128,
                bearer_token=None,
            )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

        self.assertTrue(result.ok)
        self.assertEqual(result.status_code, 202)
        self.assertEqual(result.response_body_sha256, ADAPTER.sha256_hex(body))
        self.assertEqual(result.response_body_preview, body.decode("utf-8"))
        self.assertIsNone(result.error)

    def test_torii_error_response_body_read_failure_returns_failed_receipt(self):
        hidden = "token=rail-error-read-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingBody:
            def read(self, _limit):
                raise OSError(hidden)

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii error response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_torii_error_response_body_runtime_read_failure_returns_failed_receipt(self):
        hidden = "token=rail-error-runtime-read-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingBody:
            def read(self, _limit):
                raise RuntimeError(hidden)

            def close(self):
                return None

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii error response could not be read")
        self.assertNotIn(hidden, result.error)

    def test_torii_error_response_body_non_bytes_returns_failed_receipt_without_echo(self):
        hidden = "token=rail-error-body-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class MalformedBody:
            def read(self, _limit):
                return hidden

            def close(self):
                return None

        class MalformedOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    MalformedBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = MalformedOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii error response body was not bytes")
        self.assertNotIn(hidden, result.error)

    def test_torii_error_response_body_bytes_like_values_are_capped_by_byte_length(self):
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
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
                                "https://torii.example",
                                500,
                                "failed",
                                {},
                                BytesLikeBody(),
                            )

                    ADAPTER.NO_REDIRECT_OPENER = BytesLikeOpener()
                    result = ADAPTER.submit_message(
                        "https://torii.example",
                        message,
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
                        "https://torii.example",
                        500,
                        "failed",
                        {},
                        OversizedBody(),
                    )

            ADAPTER.NO_REDIRECT_OPENER = OversizedOpener()
            with self.assertRaisesRegex(
                ADAPTER.AdapterError,
                "Torii error response exceeded 3 byte limit",
            ):
                ADAPTER.submit_message(
                    "https://torii.example",
                    message,
                    timeout_secs=1.0,
                    response_limit_bytes=3,
                    bearer_token=None,
                )
        finally:
            ADAPTER.NO_REDIRECT_OPENER = original_opener

    def test_torii_error_response_close_failure_preserves_failed_receipt(self):
        hidden = "token=rail-error-close-secret"
        body = b"upstream rejected"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingCloseBody:
            def read(self, _limit):
                return body

            def close(self):
                raise OSError(hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    FailingCloseBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_torii_error_response_close_runtime_error_preserves_failed_receipt(self):
        hidden = "token=rail-error-close-secret"
        body = b"upstream rejected"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingCloseBody:
            def read(self, _limit):
                return body

            def close(self):
                raise RuntimeError(hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    FailingCloseBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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

    def test_torii_error_response_read_failure_ignores_close_failure(self):
        read_hidden = "token=rail-error-read-secret"
        close_hidden = "token=rail-error-close-secret"
        message = ADAPTER.GatewayMessage(
            xml_path=Path("rail.xml"),
            sidecar_path=Path("rail.xml.json"),
            payload=SAMPLE_XML,
            payload_sha256=ADAPTER.sha256_hex(SAMPLE_XML),
            message_type="pacs.002",
            profile=None,
            rail_message_id=None,
        )

        class FailingBody:
            def read(self, _limit):
                raise OSError(read_hidden)

            def close(self):
                raise OSError(close_hidden)

        class FailingOpener:
            def open(self, *_args, **_kwargs):
                raise ADAPTER.urllib.error.HTTPError(
                    "https://torii.example",
                    500,
                    "failed",
                    {},
                    FailingBody(),
                )

        original_opener = ADAPTER.NO_REDIRECT_OPENER
        ADAPTER.NO_REDIRECT_OPENER = FailingOpener()
        try:
            result = ADAPTER.submit_message(
                "https://torii.example",
                message,
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
        self.assertEqual(result.error, "Torii error response could not be read")
        self.assertNotIn(read_hidden, result.error)
        self.assertNotIn(close_hidden, result.error)


if __name__ == "__main__":
    unittest.main()
