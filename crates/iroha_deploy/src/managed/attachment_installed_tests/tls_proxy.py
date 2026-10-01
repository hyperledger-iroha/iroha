"""Disposable normal-TLS ingress for the ignored native attachment regression.

This controller never supplies protocol authority. It forwards exact HTTP bodies to
four independently running loopback validators and serves one release-signed file.
Its stdin is a lifetime pipe; EOF shuts down the original owned listeners.
"""

import http.client
import http.server
import os
from pathlib import Path
import ssl
import sys
import threading
import urllib.parse


MAX_BODY = 64 * 1024 * 1024
MAX_CAPTURE = 256 * 1024 * 1024


def run():
    root = Path(sys.argv[1])
    upstreams = [urllib.parse.urlsplit(value) for value in sys.argv[2:]]
    if len(upstreams) != 4 or any(
        url.scheme != "http" or url.hostname != "127.0.0.1" or not url.port
        for url in upstreams
    ):
        raise ValueError("exactly four numeric-loopback parent validators are required")
    capture_lock = threading.Lock()
    capture_size = 0
    capacity = threading.BoundedSemaphore(16)

    def capture(data):
        nonlocal capture_size
        with capture_lock:
            if capture_size + len(data) > MAX_CAPTURE:
                raise ValueError("public request capture exceeds its finite allowance")
            with (root / "public-requests.bin").open("ab") as output:
                output.write(data)
            capture_size += len(data)

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *_):
            pass  # Headers and response bodies can contain runtime-only credentials.

        def forward(self):
            try:
                lengths = self.headers.get_all("Content-Length", [])
                if len(lengths) > 1 or self.headers.get("Transfer-Encoding"):
                    raise ValueError("ambiguous request framing")
                raw_length = lengths[0].strip() if lengths else "0"
                if not raw_length.isascii() or not raw_length.isdigit():
                    raise ValueError("invalid content length")
                length = int(raw_length)
                if length < 0 or length > MAX_BODY:
                    raise ValueError("request exceeds finite body bound")
                body = self.rfile.read(length)
                if len(body) != length:
                    raise ValueError("truncated request")
                capture(self.command.encode() + b" " + self.path.encode() + b"\n" +
                        self.headers.as_bytes() + body + b"\n")
                if self.command == "GET" and self.path == "/fixture/checkpoint.nrt":
                    with (root / "checkpoint.nrt").open("rb") as checkpoint:
                        response = checkpoint.read(MAX_BODY + 1)
                    if len(response) > MAX_BODY:
                        raise ValueError("checkpoint exceeds finite body bound")
                    self.send_response(200)
                    self.send_header("Content-Type", "application/x-norito")
                    self.send_header("Content-Length", str(len(response)))
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(response)
                    self.close_connection = True
                    return
                upstream = self.server.upstream
                headers = {name: value for name, value in self.headers.items()
                           if name.lower() not in {"host", "connection", "proxy-connection"}}
                connection = http.client.HTTPConnection(upstream.hostname, upstream.port, timeout=15)
                try:
                    connection.request(self.command, self.path, body=body, headers=headers)
                    response = connection.getresponse()
                    data = response.read(MAX_BODY + 1)
                    if len(data) > MAX_BODY:
                        raise ValueError("upstream response exceeds finite body bound")
                    self.send_response(response.status)
                    for name, value in response.getheaders():
                        if name.lower() not in {"transfer-encoding", "content-length", "connection"}:
                            self.send_header(name, value)
                    self.send_header("Content-Length", str(len(data)))
                    self.send_header("Connection", "close")
                    self.end_headers()
                    self.wfile.write(data)
                    self.close_connection = True
                finally:
                    connection.close()
            except Exception:
                self.close_connection = True
                try:
                    self.send_error(502, "bounded disposable ingress refused request")
                except OSError:
                    pass

        do_GET = forward
        do_POST = forward
        do_DELETE = forward
        do_PUT = forward

    class Server(http.server.ThreadingHTTPServer):
        daemon_threads = True
        block_on_close = False

        def get_request(self):
            connection, address = super().get_request()
            # TLS handshakes and request headers share the finite per-connection timeout.
            connection.settimeout(15)
            return connection, address

        def process_request(self, connection, address):
            # Bound physical connections before allocating a handler thread or reading headers.
            if not capacity.acquire(blocking=False):
                self.shutdown_request(connection)
                return
            try:
                super().process_request(connection, address)
            except BaseException:
                capacity.release()
                raise

        def process_request_thread(self, connection, address):
            try:
                super().process_request_thread(connection, address)
            finally:
                capacity.release()

        def handle_error(self, *_):
            pass  # Aborted TLS/header reads carry no useful public diagnostic.

    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    context.load_cert_chain(root / "certificate.pem", root / "private-key.pem")
    servers = []
    threads = []
    try:
        for upstream in upstreams:
            server = Server(("127.0.0.1", 0), Handler)
            server.upstream = upstream
            # A silent TCP client must not block the listener thread or stdin-EOF shutdown.
            server.socket = context.wrap_socket(
                server.socket, server_side=True, do_handshake_on_connect=False)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            servers.append(server)
            threads.append(thread)
        (root / "ports.pending").write_text(
            "".join(str(server.server_port) + "\n" for server in servers), encoding="ascii")
        os.replace(root / "ports.pending", root / "ports")
        while sys.stdin.buffer.read(1):
            pass
    finally:
        for server in servers:
            server.shutdown()
            server.server_close()
        for thread in threads:
            thread.join(timeout=5)


if __name__ == "__main__":
    run()
