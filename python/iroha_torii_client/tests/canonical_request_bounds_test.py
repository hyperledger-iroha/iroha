"""First-release canonical Torii request boundary and wire-parity tests."""

from __future__ import annotations

import base64
import sys
from pathlib import Path

import pytest
import requests
from requests.adapters import HTTPAdapter

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
    ToriiCanonicalRequestAuth,
    ToriiClient,
    build_canonical_request_headers,
    canonical_network_request_signature_message,
    canonical_query_string,
    canonical_request_message,
)
from iroha_torii_client.canonical_request_v1 import (
    CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1,
    require_witness_header,
    validate_target,
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

    for method in ("", "GET POST", "GÉT", "GET\n"):
        with pytest.raises(ValueError, match="ASCII HTTP token"):
            canonical_request_message(method, "/", b"")
    for path in (
        "",
        "relative",
        "//evil/x",
        "///evil/x",
        "?x=1",
        "/raw space",
        "/raw\\slash",
        "/café",
        "/x#fragment",
        "/x%GG",
        "/v1/../admin",
        "/v1/%2e%2E/admin",
    ):
        with pytest.raises(ValueError, match="root-relative|fragment-free"):
            canonical_request_message("GET", path, b"")


def test_canonical_request_rejects_path_spellings_requests_would_rewrite() -> None:
    for path in ("/v1/../admin", "/v1/%2e%2E/admin", "/x%GG"):
        prepared = requests.Request("GET", f"https://node.test{path}").prepare()
        assert prepared.path_url != path
        with pytest.raises(ValueError, match="root-relative"):
            canonical_request_message("GET", path, b"")


def test_public_target_validator_rejects_scheme_relative_paths() -> None:
    for path in ("//evil/x", "///evil/x"):
        with pytest.raises(ValueError, match="root-relative"):
            validate_target("GET", path)


def test_canonical_transport_signs_and_sends_the_same_prepared_target() -> None:
    class PreparedRecordingSession(requests.Session):
        prepared: requests.PreparedRequest | None = None

        def send(
            self,
            request: requests.PreparedRequest,
            **_kwargs: object,
        ) -> requests.Response:
            self.prepared = request
            response = requests.Response()
            response.status_code = 200
            response.request = request
            response.url = request.url
            response._content = b"{}"
            return response

    messages: list[bytes] = []
    auth = ToriiCanonicalRequestAuth(
        network_id=NETWORK_ID,
        account_id="alice@universal",
        signer=lambda message: messages.append(message) or b"signature",
        timestamp_ms=1,
        nonce="prepared-target",
    )
    session = PreparedRecordingSession()
    client = ToriiClient("https://node.test", session=session)
    caller_target = "/v1/%2e%2Fasset/%252e"
    headers = client._canonical_request_headers(
        "GET",
        caller_target,
        b"",
        canonical_auth=auth,
        headers=None,
        has_body=False,
    )

    client._request(
        "GET",
        caller_target,
        headers=headers,
        allow_retry=False,
        allow_redirects=False,
    )

    prepared = session.prepared
    assert prepared is not None
    assert prepared.path_url == "/v1/.%2Fasset/%252e"
    assert "%2F" in prepared.path_url
    assert messages == [
        canonical_network_request_signature_message(
            NETWORK_ID,
            "GET",
            prepared.path_url,
            b"",
            timestamp_ms=1,
            nonce="prepared-target",
        )
    ]


def test_public_header_builder_signs_the_requests_prepared_target() -> None:
    caller_target = "/v1/%2e%2Fasset/%252e"
    prepared = requests.Request(
        "GET",
        f"https://node.test{caller_target}",
    ).prepare()
    messages: list[bytes] = []

    build_canonical_request_headers(
        network_id=NETWORK_ID,
        account_id="alice@universal",
        signer=lambda message: messages.append(message) or b"signature",
        method="GET",
        path=caller_target,
        timestamp_ms=1,
        nonce="prepared-header-target",
    )

    assert prepared.path_url == "/v1/.%2Fasset/%252e"
    assert messages == [
        canonical_network_request_signature_message(
            NETWORK_ID,
            "GET",
            prepared.path_url,
            b"",
            timestamp_ms=1,
            nonce="prepared-header-target",
        )
    ]


def test_canonical_account_v1_requires_i105_or_exact_alias() -> None:
    for account in (
        "alice@universal",
        "alice_1@bank_a.paynet",
        "alice@xn--fa-hia",
        "alice@xn--3xa",
        "alice@xn--ll-0ea",
        "alice@xn--mgbh0fb",
        "alice@xn--11b2ezcw70k",
        "alice@xn--mgba3gch31f060k",
        "alice@xn--ngba7iz95i",
        "alice@xn--ab-0ea",
        "alice@xn--a-jib",
        "alice@xn--ab-3n4a",
        "alice@xn--alice",
        "alice@xn--a",
        "alice@xn--ab-uuba211bca8057b",
        "alice@xn--ab-j1t",
        "alice@xn--11b2er09f",
        "alice@xn--4u8c",
        "alice@xn--pq1d",
        "alice@xn--kx7e",
        "alice@xn--5h0f",
        "alice@xn--zo5h",
        "alice@xn--fi3d",
        "alice@xn--d4f",
        CANONICAL_OWNER,
    ):
        build_canonical_request_headers(
            network_id=NETWORK_ID,
            account_id=account,
            signer=lambda _message: b"signature",
            method="GET",
            path="/",
            timestamp_ms=1,
            nonce="nonce",
        )

    for account in (
        "alice",
        "alice.name@universal",
        "alice@a.b.c",
        "0xwallet@universal",
        "alice@ab--invalid",
        "alice@xn--",
        f"{'a' * 64}@universal",
    ):
        with pytest.raises(ValueError, match="exact ASCII account alias"):
            build_canonical_request_headers(
                network_id=NETWORK_ID,
                account_id=account,
                signer=lambda _message: b"signature",
                method="GET",
                path="/",
                timestamp_ms=1,
                nonce="nonce",
            )

    with pytest.raises(ValueError, match="36864 UTF-8 bytes"):
        build_canonical_request_headers(
            network_id=NETWORK_ID,
            account_id="a" * (CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 + 1),
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


def test_canonical_signature_callbacks_enforce_the_v1_payload_cap() -> None:
    common = dict(
        network_id=NETWORK_ID,
        account_id="alice@universal",
        method="GET",
        path="/",
        timestamp_ms=1,
        nonce="nonce",
    )
    accepted = build_canonical_request_headers(
        **common, signer=lambda _message: b"\x01" * 3309
    )
    assert len(base64.b64decode(accepted["X-Iroha-Signature"], validate=True)) == 3309
    for signature in (b"", b"\0" * 64, b"\x01" * 3310, "not-bytes"):
        with pytest.raises((TypeError, ValueError), match="signer|signature|bytes"):
            build_canonical_request_headers(
                **common, signer=lambda _message, value=signature: value
            )


def test_one_shot_requests_reject_unverifiable_or_configured_retries() -> None:
    class CustomSession:
        def request(self, *_args: object, **_kwargs: object) -> object:
            raise AssertionError("unverifiable one-shot session must not dispatch")

    custom_client = ToriiClient("https://node.test", session=CustomSession())
    with pytest.raises(ValueError, match="verifiable retry policy"):
        custom_client._request("GET", "/v1/test", allow_retry=False)

    session = requests.Session()
    session.mount("https://", HTTPAdapter(max_retries=1))
    client = ToriiClient("https://node.test", session=session)
    with pytest.raises(ValueError, match="adapter retries"):
        client._request("GET", "/v1/test", allow_retry=False)


def test_forwarded_witness_is_exact_base64_and_bounded_before_copy() -> None:
    exact = base64.b64encode(b"\x01" * CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1).decode()
    assert require_witness_header(exact, "witness") == exact
    headers = ToriiClient._canonical_request_headers(
        "GET", "/", b"", canonical_auth=None, headers={"X-Iroha-Witness": exact}, has_body=False
    )
    assert headers["X-Iroha-Witness"] == exact

    for witness in (
        base64.b64encode(b"\x01" * (CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 + 1)).decode(),
        "AQ",
        " AQ==",
    ):
        with pytest.raises((TypeError, ValueError), match="witness|padded"):
            ToriiClient._canonical_request_headers(
                "GET",
                "/",
                b"",
                canonical_auth=None,
                headers={"X-Iroha-Witness": witness},
                has_body=False,
            )
