#!/usr/bin/env python3
"""Canonical stdlib-only HTTP server for the Taira Inrou V1 canary."""

import json
import os
from http.server import BaseHTTPRequestHandler, HTTPServer


def require_nonblank_environment(name):
    """Return one exact non-blank environment value or reject startup."""
    value = os.environ.get(name)
    if value is None or not value or value != value.strip():
        raise ValueError(f"{name} must be set to a non-blank value")
    return value


def require_decimal_environment(name, minimum, maximum):
    """Return one canonical base-10 environment integer in the given range."""
    raw = require_nonblank_environment(name)
    if not raw.isascii() or not raw.isdecimal():
        raise ValueError(f"{name} must be a canonical base-10 integer")
    value = int(raw, 10)
    if raw != str(value) or not minimum <= value <= maximum:
        raise ValueError(
            f"{name} must be a canonical base-10 integer from {minimum} to {maximum}"
        )
    return value


class HealthHandler(BaseHTTPRequestHandler):
    """Serve the exact Taira Inrou V1 health identity."""

    service_name = ""
    service_version = ""
    replica_slot = 0

    def do_GET(self):
        """Serve only the canonical health route."""
        if self.path.partition("?")[0] != "/health":
            self.send_error(404)
            return
        payload = json.dumps(
            {
                "service": self.service_name,
                "service_version": self.service_version,
                "runtime": "Inrou",
                "replica_slot": self.replica_slot,
                "identity": f"{self.service_name}:replica:{self.replica_slot}",
            },
            ensure_ascii=True,
            separators=(",", ":"),
        ).encode("ascii")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, format_string, *args):
        """Keep the release canary quiet except for startup failures."""
        del format_string, args


def main():
    """Validate the runtime projection before binding the service port."""
    try:
        port = require_decimal_environment("PORT", 1, 65535)
        HealthHandler.service_name = require_nonblank_environment("HTTP_SERVICE_NAME")
        HealthHandler.replica_slot = require_decimal_environment(
            "SORACLOUD_REPLICA_SLOT", 1, 65535
        )
        HealthHandler.service_version = require_nonblank_environment(
            "SORACLOUD_SERVICE_VERSION"
        )
    except ValueError as error:
        raise SystemExit(f"Taira Inrou configuration error: {error}") from None
    HTTPServer(("0.0.0.0", port), HealthHandler).serve_forever()


if __name__ == "__main__":
    main()
