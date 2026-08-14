"""First-release canonical Torii request boundary and wire-parity tests."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

from client_test_support import CANONICAL_OWNER, CANONICAL_OWNER_HEADER, canonical_hash

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client import (
    CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1,
    CANONICAL_REQUEST_MAX_METHOD_BYTES_V1,
    CANONICAL_REQUEST_MAX_PATH_BYTES_V1,
    CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1,
    CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1,
    build_canonical_request_headers,
    canonical_network_request_signature_message,
    canonical_query_string,
    canonical_request_message,
)

NETWORK_ID = canonical_hash(0xA5)


def test_canonical_nonce_v1_boundaries() -> None:
    for nonce in ("nonce\x00", "nonce\x7f"):
        with pytest.raises(ValueError, match="printable ASCII"):
            canonical_network_request_signature_message(
                NETWORK_ID, "POST", "/v1/vpn/quotes", b"{}", timestamp_ms=1, nonce=nonce
            )
    canonical_network_request_signature_message(
        NETWORK_ID, "POST", "/v1/vpn/quotes", b"{}", timestamp_ms=1, nonce="!" * 256
    )


def test_canonical_query_matches_rust_form_safe_set() -> None:
    assert canonical_query_string("tilde=~&star=*&both=*~") == "both=*%7E&star=*&tilde=%7E"
    assert canonical_query_string("&&b=2&&a=1&") == "a=1&b=2"


def test_canonical_query_v1_limits_accept_exact_and_reject_plus_one() -> None:
    exact_pairs = "&".join(
        f"key{index}=value" for index in range(CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1)
    )
    assert canonical_query_string(exact_pairs).count("&") + 1 == CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1
    with pytest.raises(ValueError, match="64 pairs"):
        canonical_query_string(f"{exact_pairs}&overflow=value")

    exact_raw = "q=é" + "x" * (CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 - 4)
    assert len(exact_raw.encode("utf-8")) == CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1
    canonical_query_string(exact_raw)
    with pytest.raises(ValueError, match="65536 raw UTF-8 bytes"):
        canonical_query_string(f"{exact_raw}x")


def test_canonical_request_target_v1_limits_accept_exact_and_reject_plus_one() -> None:
    exact_method = "M" * CANONICAL_REQUEST_MAX_METHOD_BYTES_V1
    canonical_request_message(exact_method, "/", b"")
    with pytest.raises(ValueError, match="32 UTF-8 bytes"):
        canonical_request_message(f"{exact_method}M", "/", b"")

    exact_path = "/" + "p" * (CANONICAL_REQUEST_MAX_PATH_BYTES_V1 - 1)
    canonical_request_message("GET", exact_path, b"")
    with pytest.raises(ValueError, match="65536 UTF-8 bytes"):
        canonical_request_message("GET", f"{exact_path}p", b"")


def test_canonical_account_v1_limit_accepts_exact_and_rejects_plus_one() -> None:
    exact_account = "a" * CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
    headers = build_canonical_request_headers(
        network_id=NETWORK_ID,
        account_id=exact_account,
        signer=lambda _message: b"signature",
        method="GET",
        path="/",
        timestamp_ms=1,
        nonce="nonce",
    )
    assert headers["X-Iroha-Account"] == exact_account

    with pytest.raises(ValueError, match="36864 UTF-8 bytes"):
        build_canonical_request_headers(
            network_id=NETWORK_ID,
            account_id=f"{exact_account}a",
            signer=lambda _message: b"signature",
            method="GET",
            path="/",
            timestamp_ms=1,
            nonce="nonce",
        )


def test_i105_account_header_uses_portable_ascii_canonical_hex() -> None:
    headers = build_canonical_request_headers(
        network_id=NETWORK_ID,
        account_id=CANONICAL_OWNER,
        signer=lambda _message: b"signature",
        method="GET",
        path="/",
        timestamp_ms=1,
        nonce="nonce",
    )
    assert headers["X-Iroha-Account"] == CANONICAL_OWNER_HEADER
    assert headers["X-Iroha-Account"].isascii()
