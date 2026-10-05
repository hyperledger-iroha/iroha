"""Shared test-only fixtures for the installed SDK orderbook suites.

This module has no production package namespace or suite-local transport queue.
"""

from __future__ import annotations

import json
from typing import Any

import requests
from requests.structures import CaseInsensitiveDict

IDENTITY = {
    "entrypoint_hash": "aa" * 32,
    "signed_transaction_hash": "aa" * 32,
}


SIGNER = "ed0120ABCDEF"


def canonical_hash(seed: int) -> str:
    body = f"{seed:02X}" * 32
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return f"hash:{body}#{crc:04X}"


def receipt_json(**payload_overrides: Any) -> str:
    payload = {
        "entrypoint_hash": canonical_hash(0xAA),
        "signed_transaction_hash": canonical_hash(0xAA),
        "submitted_at_ms": 1,
        "submitted_at_height": 2,
        "signer": SIGNER,
        **payload_overrides,
    }
    return json.dumps({"payload": payload, "signature": "AB"}, separators=(",", ":"))


class Verifier:
    def __init__(
        self,
        *,
        verified_json: str | None = None,
        original: bytearray | None = None,
        expected_network: object | None = None,
    ) -> None:
        self.verified_json = verified_json or receipt_json()
        self.original = original
        self.expected_network = expected_network
        self.inspected: bytes | None = None
        self.during_inspect = None

    def inspect_sorafs_orderbook_submission_for_discriminant_v1(
        self, route: str, network: object, discriminant: int, signer: str, body: bytes
    ) -> dict[str, str]:
        assert route in {"order", "cancel", "receipt"}
        assert network is (NETWORK if self.expected_network is None else self.expected_network)
        assert discriminant == 369
        assert signer == SIGNER
        assert type(body) is bytes
        self.inspected = body
        if self.during_inspect is not None:
            self.during_inspect()
        if self.original is not None:
            self.original[:] = b"\xff" * len(self.original)
        return dict(IDENTITY)

    def verify_sorafs_orderbook_submission_receipt_v1(self, *args: Any) -> str:
        assert args[1:3] == tuple(IDENTITY.values())
        assert args[3] == SIGNER
        return self.verified_json


class RawHeaders:
    def __init__(self, duplicates: set[str] | None = None) -> None:
        self.duplicates = {name.lower() for name in duplicates or ()}

    def getlist(self, name: str) -> list[str]:
        return ["x", "y"] if name.lower() in self.duplicates else []


class Response:
    def __init__(
        self,
        *,
        status: int = 202,
        body: bytes = b"\x09",
        headers: dict[str, str] | None = None,
        duplicates: set[str] | None = None,
        chunks: list[bytes] | None = None,
        close_error: BaseException | None = None,
    ) -> None:
        self.status_code = status
        self.body = body
        self.chunks = chunks
        self.closed = False
        self.close_error = close_error
        self.headers = CaseInsensitiveDict(
            {
                "Content-Type": "application/x-norito",
                "Content-Length": str(len(body)),
                "x-iroha-entrypoint-hash": IDENTITY["entrypoint_hash"],
                "x-iroha-signed-transaction-hash": IDENTITY["signed_transaction_hash"],
                **(headers or {}),
            }
        )
        self.raw = type("Raw", (), {"headers": RawHeaders(duplicates)})()

    def iter_content(self, **_: Any):
        yield from self.chunks if self.chunks is not None else [self.body]

    def close(self) -> None:
        self.closed = True
        if self.close_error is not None:
            raise self.close_error


class Transport:
    def __init__(self, outcome: Response | BaseException) -> None:
        self.outcome = outcome
        self.calls: list[dict[str, Any]] = []
        self.before_return = None


NETWORK = object()


def stock_session(
    response: Response | BaseException,
) -> tuple[requests.Session, Transport]:
    session, transport = requests.Session(), Transport(response)
    session.trust_env = False
    return session, transport
