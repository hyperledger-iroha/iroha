#!/usr/bin/env python3
"""Submit a signed transaction envelope (JSON) to an Iroha node.

Usage:
    submit_envelope_json.py --url http://127.0.0.1:8080 --file envelope.json
    submit_envelope_json.py --url http://127.0.0.1:8080 < envelope.json

The JSON payload must be produced by `iroha_python.SignedTransactionEnvelope.to_json()`.
"""

from __future__ import annotations

import argparse
import sys
from typing import Optional

from iroha_python import create_torii_client


def _read_payload(path: Optional[str]) -> str:
    if path:
        with open(path, "r", encoding="utf-8") as handle:
            return handle.read()
    data = sys.stdin.read()
    if not data:
        raise SystemExit("no JSON payload provided via stdin")
    return data


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True, help="Torii base URL, e.g. http://127.0.0.1:8080")
    parser.add_argument(
        "--file",
        help="Path to JSON envelope produced by to_json(); omit to read from stdin",
    )
    parser.add_argument(
        "--wait",
        action="store_true",
        help="Poll transaction status until completion (defaults: interval=1s timeout=30s)",
    )
    parser.add_argument("--interval", type=float, default=1.0, help="Polling interval in seconds")
    parser.add_argument(
        "--timeout",
        type=float,
        default=30.0,
        help="Polling timeout in seconds; set to 0 to disable",
    )
    parser.add_argument(
        "--max-attempts",
        type=int,
        default=None,
        help="Optional max poll attempts before giving up",
    )
    args = parser.parse_args(argv)

    payload = _read_payload(args.file)
    client = create_torii_client(args.url)
    if args.wait:
        timeout = None if args.timeout == 0 else args.timeout
        response = client.submit_transaction_json_and_wait(
            payload,
            interval=max(args.interval, 0.0),
            timeout=timeout,
            max_attempts=args.max_attempts,
        )
    else:
        response = client.submit_transaction_json(payload)
    if response is not None:
        print(response)
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
