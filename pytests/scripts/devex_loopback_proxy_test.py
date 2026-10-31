"""Exercise only the disposable attachment test controller, never protocol qualification.

Each test uses four fake numeric-loopback HTTP servers. No remote HTTP endpoint
is admitted and no operating-system trust is changed. Small
module constants exercise finite boundaries without writing 256 MiB of captures;
the normal controller CLI is exercised with its actual defaults as well.
"""

from contextlib import contextmanager
import http.client
import http.server
from pathlib import Path
import queue
import socket
import subprocess
import sys
import threading
import time

import pytest


CONTROLLER = (Path(__file__).resolve().parents[2] /
              "crates/iroha_deploy/src/managed/attachment_installed_tests/loopback_proxy.py")


@pytest.fixture
def upstreams():
    received = queue.Queue()
    servers = []
    threads = []

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *_):
            pass

        def serve(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            received.put((self.server.index, self.command, self.path, dict(self.headers), body))
            response = b"x" * 4097 if self.path == "/oversized" else body or b"ready"
            self.send_response(201)
            self.send_header("X-Upstream", str(self.server.index))
            self.send_header("Content-Length", str(len(response)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(response)
            self.close_connection = True

        do_GET = serve
        do_POST = serve

    try:
        for index in range(4):
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            server.daemon_threads = True
            server.index = index
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            servers.append(server)
            threads.append(thread)
        yield servers, received
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        for thread in threads:
            thread.join(timeout=3)
            assert not thread.is_alive()


@pytest.fixture
def controller(tmp_path, upstreams):
    servers, received = upstreams

    @contextmanager
    def start(*, bounded=False):
        root = tmp_path / "ingress"
        root.mkdir(mode=0o700)
        arguments = [str(root), *[f"http://127.0.0.1:{server.server_port}" for server in servers]]
        if bounded:
            script = (
                "import importlib.util,sys; "
                "s=importlib.util.spec_from_file_location('ingress',sys.argv[1]); "
                "m=importlib.util.module_from_spec(s); s.loader.exec_module(m); "
                "m.MAX_BODY=4096; m.MAX_CAPTURE=16384; "
                "sys.argv=sys.argv[1:]; m.run()"
            )
            command = [sys.executable, "-c", script, str(CONTROLLER), *arguments]
        else:
            command = [sys.executable, str(CONTROLLER), *arguments]
        process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        ports = []
        try:
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                assert process.poll() is None, process.communicate()[1].decode()
                if (root / "ports").exists():
                    ports = [int(port) for port in (root / "ports").read_text().splitlines()]
                    break
                threading.Event().wait(0.01)
            assert len(ports) == 4 and len(set(ports)) == 4
            yield root, ports, process, received
        finally:
            process.stdin.close()
            process.wait(timeout=5)
            diagnostics = process.stderr.read().decode()
            process.stdout.close()
            process.stderr.close()
            assert process.returncode == 0, diagnostics
            for port in ports:
                with pytest.raises(OSError):
                    socket.create_connection(("127.0.0.1", port), timeout=0.2)

    return start


def request(port, method="GET", path="/", body=None, headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def test_loopback_preserves_four_upstream_bodies_headers_and_native_discovery(controller):
    with controller() as (root, ports, _, received):
        body = b"\x00\xffexact canonical request\r\n"
        for index, port in enumerate(ports):
            status, headers, response = request(
                port, "POST", "/v1/private?cursor=7", body,
                {"X-Test-Authority": "signed-runtime-value", "Content-Type": "application/x-norito"})
            assert (status, headers["X-Upstream"], response) == (201, str(index), body)
            peer, method, path, forwarded, payload = received.get(timeout=2)
            assert (peer, method, path, payload) == (index, "POST", "/v1/private?cursor=7", body)
            assert forwarded["X-Test-Authority"] == "signed-runtime-value"
            assert forwarded["Content-Type"] == "application/x-norito"
        for path in ["/v1/accounts/capabilities", "/v1/accounts/faucet/policy", "/status",
                     "/v1/bridge/finality/1", "/v1/bridge/finality/interval/2/4"]:
            status, headers, response = request(ports[0], path=path)
            assert (status, headers["X-Upstream"], response) == (201, "0", b"ready")
            assert received.get(timeout=2)[:3] == (0, "GET", path)
        assert received.empty()
        capture = (root / "public-requests.bin").read_bytes()
        assert capture.count(body) == 4
        assert capture.count(b"X-Test-Authority: signed-runtime-value") == 4


@pytest.mark.parametrize("upstream", [
    "http://localhost:8080/", "http://192.0.2.1:8080/", "https://127.0.0.1:8080/",
    "http://127.0.0.1/",
])
def test_upstream_must_be_explicit_numeric_loopback_http(tmp_path, upstream):
    result = subprocess.run([sys.executable, str(CONTROLLER), str(tmp_path), *([upstream] * 4)],
                            input=b"", capture_output=True, timeout=3)
    assert result.returncode != 0
    assert not (tmp_path / "ports").exists()


@pytest.mark.parametrize("framing", [
    b"Content-Length: 0\r\nContent-Length: 0\r\n",
    b"Content-Length: 0\r\nTransfer-Encoding: chunked\r\n",
    b"Transfer-Encoding: chunked\r\n",
    b"Content-Length: +0\r\n",
    b"Content-Length: 1_0\r\n",
    b"Content-Length: -1\r\n",
    b"Content-Length: 67108865\r\n",
])
def test_invalid_framing_never_reaches_upstream(controller, framing):
    with controller() as (root, ports, _, received):
        with socket.create_connection(("127.0.0.1", ports[0]), timeout=3) as channel:
            channel.sendall(b"POST / HTTP/1.1\r\nHost: localhost\r\n" + framing + b"\r\n")
            response = http.client.HTTPResponse(channel)
            response.begin()
            assert response.status == 502
            response.read()
        assert received.empty()
        assert not (root / "public-requests.bin").exists()


def test_finite_request_response_and_capture_limits(controller):
    with controller(bounded=True) as (root, ports, _, received):
        assert request(ports[0], "POST", body=b"x" * 4096)[0] == 201
        assert received.get(timeout=2)[4] == b"x" * 4096
        assert request(ports[0], "POST", body=b"x" * 4097)[0] == 502
        assert received.empty()
        assert request(ports[0], path="/oversized")[0] == 502
        received.get(timeout=2)
        for _ in range(20):
            if request(ports[0], "POST", body=b"x" * 1024)[0] == 502:
                break
        else:
            pytest.fail("capture allowance did not close admission")
        assert (root / "public-requests.bin").stat().st_size <= 16384


def test_idle_client_cannot_block_listener_or_eof_cleanup(controller):
    with controller() as (_, ports, process, _):
        with socket.create_connection(("127.0.0.1", ports[0]), timeout=3):
            # A second request must finish while the first socket sends no headers.
            assert request(ports[0])[0] == 201
            process.stdin.close()
            process.wait(timeout=3)
            assert process.returncode == 0


def test_connection_budget_applies_before_request_headers(controller):
    with controller() as (_, ports, process, _):
        channels = []
        try:
            for _ in range(16):
                raw = socket.create_connection(("127.0.0.1", ports[0]), timeout=3)
                raw.sendall(b"GET / HTTP/1.1\r\n")
                channels.append(raw)
            with socket.create_connection(("127.0.0.1", ports[0]), timeout=3) as raw:
                assert raw.recv(1) == b""
            process.stdin.close()
            process.wait(timeout=3)
            assert process.returncode == 0
        finally:
            for channel in channels:
                channel.close()
