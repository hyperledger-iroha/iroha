"""Exercise only the disposable attachment test controller, never protocol qualification.

Each test uses four fake numeric-loopback HTTP servers and a temporary private CA.
TLS verification remains enabled and no operating-system trust is changed. Small
module constants exercise finite boundaries without writing 256 MiB of captures;
the normal controller CLI is exercised with its actual defaults as well.
"""

from contextlib import contextmanager
import http.client
import http.server
from pathlib import Path
import queue
import shutil
import socket
import ssl
import subprocess
import sys
import threading
import time

import pytest


CONTROLLER = (Path(__file__).resolve().parents[2] /
              "crates/iroha_deploy/src/managed/attachment_installed_tests/tls_proxy.py")


@pytest.fixture(scope="module")
def certificates(tmp_path_factory):
    root = tmp_path_factory.mktemp("devex-tls-authority")
    root.chmod(0o700)
    config = root / "certificate.cnf"
    config.write_text(
        "[req]\ndistinguished_name=dn\nx509_extensions=ca\nprompt=no\n"
        "[dn]\nCN=disposable-controller-test\n"
        "[ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n"
        "[server]\nbasicConstraints=critical,CA:FALSE\n"
        "keyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n"
        "subjectAltName=IP:127.0.0.1,DNS:localhost\n", encoding="ascii")
    openssl = shutil.which("openssl")
    assert openssl, "the TLS controller regression requires openssl"
    commands = [
        ["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
         "-config", str(config), "-keyout", str(root / "authority-key.pem"),
         "-out", str(root / "authority.pem")],
        ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-config", str(config),
         "-subj", "/CN=disposable-controller-server",
         "-keyout", str(root / "private-key.pem"), "-out", str(root / "server.csr")],
        ["x509", "-req", "-days", "1", "-CAcreateserial", "-in", str(root / "server.csr"),
         "-CA", str(root / "authority.pem"), "-CAkey", str(root / "authority-key.pem"),
         "-extfile", str(config), "-extensions", "server", "-out", str(root / "certificate.pem")],
    ]
    for command in commands:
        subprocess.run([openssl, *command], check=True, capture_output=True, timeout=20)
    return root


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
def controller(tmp_path, certificates, upstreams):
    servers, received = upstreams

    @contextmanager
    def start(*, bounded=False):
        root = tmp_path / "ingress"
        root.mkdir(mode=0o700)
        for name in ["certificate.pem", "private-key.pem"]:
            shutil.copyfile(certificates / name, root / name)
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
            context = ssl.create_default_context(cafile=str(certificates / "authority.pem"))
            yield root, ports, context, process, received
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


def request(port, context, method="GET", path="/", body=None, headers=None):
    connection = http.client.HTTPSConnection("127.0.0.1", port, context=context, timeout=3)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def test_verified_tls_preserves_four_upstream_bodies_headers_and_checkpoint(controller):
    with controller() as (root, ports, context, _, received):
        body = b"\x00\xffexact canonical request\r\n"
        for index, port in enumerate(ports):
            status, headers, response = request(
                port, context, "POST", "/v1/private?cursor=7", body,
                {"X-Test-Authority": "signed-runtime-value", "Content-Type": "application/x-norito"})
            assert (status, headers["X-Upstream"], response) == (201, str(index), body)
            peer, method, path, forwarded, payload = received.get(timeout=2)
            assert (peer, method, path, payload) == (index, "POST", "/v1/private?cursor=7", body)
            assert forwarded["X-Test-Authority"] == "signed-runtime-value"
            assert forwarded["Content-Type"] == "application/x-norito"
        checkpoint = b"\x00signed checkpoint\xff"
        (root / "checkpoint.nrt").write_bytes(checkpoint)
        status, headers, response = request(ports[0], context, path="/fixture/checkpoint.nrt")
        assert (status, headers["Content-Type"], response) == (200, "application/x-norito", checkpoint)
        assert received.empty()
        capture = (root / "public-requests.bin").read_bytes()
        assert capture.count(body) == 4
        assert capture.count(b"X-Test-Authority: signed-runtime-value") == 4


def test_tls_rejects_untrusted_authority_and_wrong_hostname(controller):
    with controller() as (_, ports, context, _, _):
        with pytest.raises(ssl.SSLCertVerificationError):
            request(ports[0], ssl.create_default_context())
        with socket.create_connection(("127.0.0.1", ports[1]), timeout=2) as raw:
            with pytest.raises(ssl.SSLCertVerificationError):
                context.wrap_socket(raw, server_hostname="wrong.invalid")


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
    with controller() as (root, ports, context, _, received):
        with socket.create_connection(("127.0.0.1", ports[0]), timeout=3) as raw:
            with context.wrap_socket(raw, server_hostname="127.0.0.1") as channel:
                channel.sendall(b"POST / HTTP/1.1\r\nHost: localhost\r\n" + framing + b"\r\n")
                response = http.client.HTTPResponse(channel)
                response.begin()
                assert response.status == 502
                response.read()
        assert received.empty()
        assert not (root / "public-requests.bin").exists()


def test_finite_request_response_checkpoint_and_capture_limits(controller):
    with controller(bounded=True) as (root, ports, context, _, received):
        assert request(ports[0], context, "POST", body=b"x" * 4096)[0] == 201
        assert received.get(timeout=2)[4] == b"x" * 4096
        assert request(ports[0], context, "POST", body=b"x" * 4097)[0] == 502
        assert received.empty()
        assert request(ports[0], context, path="/oversized")[0] == 502
        received.get(timeout=2)
        (root / "checkpoint.nrt").write_bytes(b"x" * 4097)
        assert request(ports[0], context, path="/fixture/checkpoint.nrt")[0] == 502
        for _ in range(20):
            if request(ports[0], context, "POST", body=b"x" * 1024)[0] == 502:
                break
        else:
            pytest.fail("capture allowance did not close admission")
        assert (root / "public-requests.bin").stat().st_size <= 16384


def test_idle_tls_client_cannot_block_listener_or_eof_cleanup(controller):
    with controller() as (_, ports, context, process, _):
        with socket.create_connection(("127.0.0.1", ports[0]), timeout=3):
            # A second verified handshake must complete while the first socket sends no TLS bytes.
            assert request(ports[0], context)[0] == 201
            process.stdin.close()
            process.wait(timeout=3)
            assert process.returncode == 0


def test_connection_budget_applies_before_request_headers(controller):
    with controller() as (_, ports, context, process, _):
        channels = []
        try:
            for _ in range(16):
                raw = socket.create_connection(("127.0.0.1", ports[0]), timeout=3)
                channels.append(context.wrap_socket(raw, server_hostname="127.0.0.1"))
            with socket.create_connection(("127.0.0.1", ports[0]), timeout=3) as raw:
                with pytest.raises((ssl.SSLError, ConnectionError, OSError)):
                    context.wrap_socket(raw, server_hostname="127.0.0.1")
            process.stdin.close()
            process.wait(timeout=3)
            assert process.returncode == 0
        finally:
            for channel in channels:
                channel.close()
