import os
from http.server import BaseHTTPRequestHandler, HTTPServer


slot = os.environ["SORACLOUD_REPLICA_SLOT"]
root_marker = "/var/lib/soracloud/materialization/root-slot.txt"
os.makedirs("/var/lib/ton-indexer", exist_ok=True)
with open(
    f"/var/lib/ton-indexer/replica-{slot}.txt", "w", encoding="utf-8"
) as handle:
    handle.write(f"{slot}\n")
os.makedirs("/var/lib/soracloud/materialization", exist_ok=True)
with open(root_marker, "w", encoding="utf-8") as handle:
    handle.write(f"{slot}\n")


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/healthz":
            body = b"ok\n"
        elif self.path == "/root-slot":
            with open(root_marker, "rb") as handle:
                body = handle.read()
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


HTTPServer(("0.0.0.0", int(os.environ["PORT"])), Handler).serve_forever()
