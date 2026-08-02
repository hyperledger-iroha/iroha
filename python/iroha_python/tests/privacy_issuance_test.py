"""Adversarial tests for exact Bootle/Lantern issuance transport semantics."""

from __future__ import annotations

import base64
import hashlib
import json
import struct
from collections.abc import Callable, Mapping
from pathlib import Path
from typing import Any

import pytest

import iroha_python
from iroha_python.privacy_issuance import (
    BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1,
    BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
    BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1,
    BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1,
    BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
    BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
    BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1,
    BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1,
    BootleLanternIssuanceClientErrorV1,
    BootleLanternIssuanceClientV1,
    BootleLanternIssuanceCredentialV1,
)


def patterned(length: int) -> bytes:
    body = bytearray(index & 0xFF for index in range(length))
    if length == BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1:
        body[:4] = b"ILA1"
    elif length == BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1:
        body[:4] = b"ILA1"
        body[BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 : BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 + 4] = (
            b"ILQ1"
        )
    elif length == BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1:
        body[:4] = b"ILR1"
    return bytes(body)


class RawHeaders:
    def __init__(self, headers: Mapping[str, list[str]]) -> None:
        self.headers = headers

    def getlist(self, name: str) -> list[str]:
        values: list[str] = []
        for candidate, candidate_values in self.headers.items():
            if candidate.lower() == name.lower():
                values.extend(candidate_values)
        return values


class RawBody:
    def __init__(self, body: bytes | bytearray, headers: Mapping[str, list[str]]) -> None:
        self.body = body
        self.headers = RawHeaders(headers)
        self.read_calls: list[tuple[int, bool]] = []

    def read(self, amount: int, *, decode_content: bool) -> bytes | bytearray:
        self.read_calls.append((amount, decode_content))
        return self.body[:amount]


class Response:
    def __init__(
        self,
        body: bytes | bytearray,
        *,
        status: int = 200,
        headers: Mapping[str, list[str]] | None = None,
        url: str = "",
    ) -> None:
        exact_headers = (
            {"Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1]}
            if headers is None
            else headers
        )
        self.status_code = status
        self.url = url
        self.raw = RawBody(body, exact_headers)
        self.headers = {name: ", ".join(values) for name, values in exact_headers.items()}
        self.closed = False

    def close(self) -> None:
        self.closed = True


class Session:
    def __init__(
        self,
        scripted: Response | Exception | Callable[[dict[str, Any]], Response],
    ) -> None:
        self.scripted = scripted
        self.calls: list[dict[str, Any]] = []

    def post(self, url: str, **kwargs: Any) -> Response:
        call = {"url": url, **kwargs}
        self.calls.append(call)
        if isinstance(self.scripted, Exception):
            raise self.scripted
        if callable(self.scripted):
            return self.scripted(call)
        return self.scripted


def success(length: int) -> Response:
    return Response(patterned(length))


def client(session: Session) -> BootleLanternIssuanceClientV1:
    return BootleLanternIssuanceClientV1("https://torii.example", session=session)


def credential() -> BootleLanternIssuanceCredentialV1:
    return BootleLanternIssuanceCredentialV1.from_opaque_bytes(b"\x01\x02\x03")


def client_contract_fixture() -> dict[str, Any]:
    for root in Path(__file__).resolve().parents:
        candidate = root / "fixtures/privacy/bootle_lantern_issuance_client_v1.json"
        if candidate.is_file():
            return json.loads(candidate.read_text(encoding="utf-8"))
    raise AssertionError("shared Bootle/Lantern issuance client fixture was not found")


def error_fixture_body(contract: Mapping[str, Any]) -> bytes:
    if "body_hex" in contract:
        return bytes.fromhex(str(contract["body_hex"]))
    return str(contract["body_utf8"]).encode("utf-8")


def error_fixture_headers(contract: Mapping[str, Any], body_length: int) -> dict[str, list[str]]:
    headers = {
        "Content-Type": [str(contract["media_type"])],
        "Content-Length": [str(body_length)],
    }
    if "retry_after_seconds" in contract:
        headers["Retry-After"] = [str(contract["retry_after_seconds"])]
    if "www_authenticate" in contract:
        headers["WWW-Authenticate"] = [str(contract["www_authenticate"])]
    return headers


def error_fixture_response(
    contract: Mapping[str, Any], *, headers: Mapping[str, list[str]] | None = None
) -> Response:
    body = error_fixture_body(contract)
    canonical_headers = error_fixture_headers(contract, len(body))
    return Response(
        body,
        status=int(contract["status"]),
        headers=canonical_headers if headers is None else headers,
    )


def crc64_ecma(payload: bytes) -> int:
    mask = 0xFFFF_FFFF_FFFF_FFFF
    polynomial = 0xC96C_5795_D787_0F42
    value = mask
    for byte in payload:
        value ^= byte
        for _ in range(8):
            value = (value >> 1) ^ polynomial if value & 1 else value >> 1
    return (value ^ mask) & mask


def norito_frame_with_payload(template: bytes, payload: bytes) -> bytes:
    frame = bytearray(template[:40] + payload)
    struct.pack_into("<Q", frame, 23, len(payload))
    struct.pack_into("<Q", frame, 31, crc64_ecma(payload))
    return bytes(frame)


def malformed_norito_field_frame(body: bytes) -> bytes:
    malformed = bytearray(body)
    assert malformed[:4] == b"NRT0"
    payload_length = struct.unpack_from("<Q", malformed, 23)[0]
    assert 40 + payload_length == len(malformed)
    # Extend the first struct field while keeping a valid frame CRC so this
    # reaches the canonical ErrorEnvelope decoder rather than failing framing.
    assert malformed[40] < 0x7F
    malformed[40] += 1
    struct.pack_into("<Q", malformed, 31, crc64_ecma(bytes(malformed[40:])))
    return bytes(malformed)


def rejected_legacy_norito_error_frame(template: bytes, code: str) -> bytes:
    encoded = code.encode("utf-8")
    assert len(encoded) < 0x80
    # Pre-release hand framing omitted each struct field's authoritative outer
    # compact-length envelope. It must not be accepted as a compatibility form.
    payload = bytes([len(encoded)]) + encoded + bytes([len(encoded)]) + encoded + b"\0"
    return norito_frame_with_payload(template, payload)


def test_shared_client_contract_fixture_binds_exact_wire_bytes() -> None:
    fixture = client_contract_fixture()
    assert fixture["schema"] == "iroha.bootle_lantern.issuance_client_contract"
    assert fixture["version"] == 1
    assert fixture["classification"] == "public-synthetic-test-data"

    transport = fixture["transport"]
    assert transport["method"] == "POST"
    assert transport["authorize_path"] == BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1
    assert transport["issue_path"] == BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1
    assert transport["norito_media_type"] == BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1
    assert (
        transport["unauthorized_www_authenticate"] == 'Bearer realm="iroha-bootle-lantern-issuance"'
    )

    credential_contract = fixture["credential"]
    assert credential_contract["encoding"] == "base64url-unpadded-canonical"
    assert credential_contract["minimum_decoded_bytes"] == 1
    assert (
        credential_contract["maximum_decoded_bytes"]
        == BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1
    )
    assert len(credential_contract["examples"]) == 3
    for example in credential_contract["examples"]:
        decoded = bytes.fromhex(example["decoded_hex"])
        assert base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii") == example["encoded"]
        admitted = BootleLanternIssuanceCredentialV1.from_canonical_base64url(example["encoded"])
        assert admitted._authorization_header_value() == f"Bearer {example['encoded']}"
        admitted.destroy()

    bodies = fixture["bodies"]
    assert bodies["pattern"] == "byte-at-index-equals-index-modulo-256-with-canonical-wire-magics"
    for name, wire, length in (
        ("authorization_response", "ILA1", BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
        ("issue_request", "ILA1+ILQ1", BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1),
        ("issue_response", "ILR1", BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1),
    ):
        body = bodies[name]
        assert body["wire"] == wire
        assert body["length_bytes"] == length
        assert hashlib.sha256(patterned(length)).hexdigest() == body["pattern_sha256_hex"]
    assert patterned(320)[:4] == b"ILA1"
    assert patterned(71_896)[:4] == b"ILA1"
    assert patterned(71_896)[320:324] == b"ILQ1"
    assert patterned(3_176)[:4] == b"ILR1"
    assert bodies["issue_request"]["component_lengths_bytes"] == [320, 71_576]
    assert (
        sum(bodies["issue_request"]["component_lengths_bytes"])
        == BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1
    )

    errors = fixture["errors"]
    assert errors["maximum_body_bytes"] == BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1
    assert errors["norito_envelope"] == {
        "schema_type_name": "iroha_torii_shared::ErrorEnvelope",
        "schema_hash_hex": "793f11768076bfe270a17aeb86752cd9",
        "flags_hex": "02",
    }
    assert len(errors["responses"]) == 8
    for contract in errors["responses"]:
        assert contract.get("www_authenticate") == (
            transport["unauthorized_www_authenticate"] if contract["status"] == 401 else None
        )
        with pytest.raises(BootleLanternIssuanceClientErrorV1) as caught:
            client(Session(error_fixture_response(contract))).authorize(credential())
        assert caught.value.status_code == contract["status"]
        assert caught.value.code == contract["code"]
        assert caught.value.retry_after_seconds == contract.get("retry_after_seconds")


def test_first_release_issuance_surface_is_exported_from_package_root() -> None:
    assert iroha_python.BootleLanternIssuanceClientV1 is BootleLanternIssuanceClientV1
    assert iroha_python.BootleLanternIssuanceCredentialV1 is BootleLanternIssuanceCredentialV1
    assert iroha_python.BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 == 71_896
    assert "BootleLanternIssuanceClientV1" in iroha_python.__all__


def test_authorize_sends_canonical_empty_request_exactly_once() -> None:
    session = Session(success(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1))
    actual = client(session).authorize(BootleLanternIssuanceCredentialV1.from_opaque_bytes(b"a"))

    assert actual == patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)
    assert len(session.calls) == 1
    call = session.calls[0]
    assert call["url"] == f"https://torii.example{BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1}"
    assert call["data"] == b""
    assert call["headers"] == {
        "Authorization": "Bearer YQ",
        "Content-Type": BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
        "Accept": BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
        "Accept-Encoding": "identity",
        "Cache-Control": "no-store",
        "Pragma": "no-cache",
    }
    assert call["allow_redirects"] is False
    assert call["stream"] is True
    assert call["timeout"] == 15.0


def test_issue_defensively_copies_exact_request_and_response() -> None:
    source = bytearray(patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1))
    expected = bytes(source)
    response_bytes = bytearray(patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1))

    def mutate_source(call: dict[str, Any]) -> Response:
        source[:] = b"\x00" * len(source)
        assert call["data"] == expected
        return Response(response_bytes)

    session = Session(mutate_source)
    actual = client(session).issue(credential(), source)
    response_bytes[:] = b"\x00" * len(response_bytes)

    assert actual == patterned(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1)
    assert len(session.calls) == 1
    assert session.calls[0]["url"] == (
        f"https://torii.example{BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1}"
    )


@pytest.mark.parametrize(
    "size",
    [
        0,
        1,
        BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 - 1,
        BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 + 1,
        BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 * 2,
    ],
)
def test_issue_rejects_wrong_lengths_without_transport(size: int) -> None:
    session = Session(success(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1))
    with pytest.raises(ValueError, match="exactly 71896 bytes"):
        client(session).issue(credential(), bytes(size))
    assert session.calls == []


def test_issue_rejects_non_bytes_without_transport() -> None:
    session = Session(success(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1))
    with pytest.raises(TypeError, match="bytes-like"):
        client(session).issue(credential(), [])  # type: ignore[arg-type]
    assert session.calls == []


@pytest.mark.parametrize("prefix", [b"\0\0\0\0", b"ILA0", b"ILA\0", b"XLA1"])
def test_issue_rejects_same_length_noncanonical_ila1_magic_without_transport(
    prefix: bytes,
) -> None:
    session = Session(success(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1))
    request = bytearray(patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1))
    request[:4] = prefix
    with pytest.raises(ValueError, match=r"ILA1 \|\| ILQ1"):
        client(session).issue(credential(), request)
    assert session.calls == []


@pytest.mark.parametrize("prefix", [b"\0\0\0\0", b"ILQ0", b"ILQ\0", b"XLQ1"])
def test_issue_rejects_same_length_noncanonical_ilq1_magic_without_transport(
    prefix: bytes,
) -> None:
    session = Session(success(BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1))
    request = bytearray(patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1))
    offset = BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1
    request[offset : offset + 4] = prefix
    with pytest.raises(ValueError, match=r"ILA1 \|\| ILQ1"):
        client(session).issue(credential(), request)
    assert session.calls == []


def test_credentials_are_canonical_bounded_defensive_destroyable_and_redacted() -> None:
    with pytest.raises(ValueError):
        BootleLanternIssuanceCredentialV1.from_opaque_bytes(b"")
    with pytest.raises(ValueError):
        BootleLanternIssuanceCredentialV1.from_opaque_bytes(
            bytes(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 1)
        )
    with pytest.raises(TypeError):
        BootleLanternIssuanceCredentialV1.from_opaque_bytes([1, 2, 3])  # type: ignore[arg-type]
    malformed = [
        "",
        "A",
        "YQ==",
        "YR",
        "Y Q",
        "YQ\n",
        "Bearer YQ",
        "+w",
        "A" * (((BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 2) // 3) * 4 + 1),
        base64.urlsafe_b64encode(bytes(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 1))
        .rstrip(b"=")
        .decode("ascii"),
    ]
    for encoded in malformed:
        with pytest.raises(ValueError):
            BootleLanternIssuanceCredentialV1.from_canonical_base64url(encoded)

    source = bytearray(b"a")
    secret = BootleLanternIssuanceCredentialV1.from_opaque_bytes(source)
    source[0] = ord("b")
    assert str(secret) == "BootleLanternIssuanceCredentialV1([REDACTED])"
    assert repr(secret) == "BootleLanternIssuanceCredentialV1([REDACTED])"
    session = Session(success(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1))
    client(session).authorize(secret)
    assert session.calls[0]["headers"]["Authorization"] == "Bearer YQ"
    secret.destroy()
    secret.destroy()
    with pytest.raises(ValueError, match="destroyed"):
        client(session).authorize(secret)
    assert len(session.calls) == 1

    maximum = bytes([0xFF]) * BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1
    maximum_encoded = base64.urlsafe_b64encode(maximum).rstrip(b"=").decode("ascii")
    max_secret = BootleLanternIssuanceCredentialV1.from_canonical_base64url(maximum_encoded)
    max_session = Session(success(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1))
    client(max_session).authorize(max_secret)
    assert max_session.calls[0]["headers"]["Authorization"] == f"Bearer {maximum_encoded}"
    max_secret.destroy()


@pytest.mark.parametrize(
    ("expected", "size"),
    [
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, 0),
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 - 1),
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 + 1),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, 0),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 - 1),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 + 1),
    ],
)
def test_response_lengths_are_exact(expected: int, size: int) -> None:
    session = Session(Response(patterned(size)))
    issuance = client(session)
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match=f"exactly {expected} bytes"):
        if expected == BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1:
            issuance.authorize(credential())
        else:
            issuance.issue(credential(), patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1))
    assert len(session.calls) == 1


@pytest.mark.parametrize(
    ("expected", "prefix"),
    [
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, b"\0\0\0\0"),
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, b"ILA0"),
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, b"ILA\0"),
        (BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1, b"XLA1"),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, b"\0\0\0\0"),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, b"ILR0"),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, b"ILR\0"),
        (BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1, b"XLR1"),
    ],
)
def test_success_responses_require_exact_wire_magic(expected: int, prefix: bytes) -> None:
    body = bytearray(patterned(expected))
    body[:4] = prefix
    issuance = client(Session(Response(body)))
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="wire magic"):
        if expected == BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1:
            issuance.authorize(credential())
        else:
            issuance.issue(credential(), patterned(BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1))


@pytest.mark.parametrize("status", [0, 201, 204, 301, 307, 308, 418, 500])
def test_response_requires_exact_200_and_is_not_retried(status: int) -> None:
    session = Session(Response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), status=status))
    with pytest.raises(BootleLanternIssuanceClientErrorV1):
        client(session).authorize(credential())
    assert len(session.calls) == 1


def test_structured_errors_bind_status_media_code_and_retry_hint() -> None:
    for contract in client_contract_fixture()["errors"]["responses"]:
        session = Session(error_fixture_response(contract))
        with pytest.raises(BootleLanternIssuanceClientErrorV1) as caught:
            client(session).authorize(credential())
        assert caught.value.status_code == contract["status"]
        assert caught.value.code == contract["code"]
        assert caught.value.retry_after_seconds == contract.get("retry_after_seconds")
        assert len(session.calls) == 1


def test_all_seven_norito_errors_reject_legacy_malformed_truncated_and_trailing_frames() -> None:
    contracts = [
        contract
        for contract in client_contract_fixture()["errors"]["responses"]
        if contract["media_type"] == BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1
    ]
    assert len(contracts) == 7

    for contract in contracts:
        canonical = error_fixture_body(contract)
        variants = (
            rejected_legacy_norito_error_frame(canonical, str(contract["code"])),
            malformed_norito_field_frame(canonical),
            canonical[:-1],
            canonical + b"\0",
        )
        for body in variants:
            response = Response(
                body,
                status=int(contract["status"]),
                headers=error_fixture_headers(contract, len(body)),
            )
            session = Session(response)
            with pytest.raises(BootleLanternIssuanceClientErrorV1) as caught:
                client(session).authorize(credential())
            assert caught.value.status_code is None
            assert caught.value.code is None
            assert caught.value.retry_after_seconds is None
            assert len(session.calls) == 1


def test_structured_errors_reject_malformed_substituted_and_oversized_envelopes() -> None:
    contracts = {item["status"]: item for item in client_contract_fixture()["errors"]["responses"]}
    corrupted = bytearray(error_fixture_body(contracts[400]))
    corrupted[0] ^= 1
    adversarial = [
        Response(
            corrupted,
            status=400,
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": [str(len(corrupted))],
            },
        ),
        error_fixture_response(
            contracts[400],
            headers={
                "Content-Type": ["application/json"],
                "Content-Length": [str(len(error_fixture_body(contracts[400])))],
            },
        ),
        error_fixture_response(
            contracts[400],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Encoding": ["identity"],
            },
        ),
        error_fixture_response(
            contracts[400],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": ["0107"],
            },
        ),
        Response(
            error_fixture_body(contracts[401]),
            status=400,
            headers={"Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1]},
        ),
        Response(
            f"{contracts[406]['body_utf8']} ".encode(),
            status=406,
            headers={"Content-Type": ["application/json"]},
        ),
        error_fixture_response(
            contracts[429],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Retry-After": ["2"],
            },
        ),
        error_fixture_response(
            contracts[503],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Retry-After": ["1"],
            },
        ),
        error_fixture_response(
            contracts[401],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": [str(len(error_fixture_body(contracts[401])))],
            },
        ),
        error_fixture_response(
            contracts[401],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": [str(len(error_fixture_body(contracts[401])))],
                "WWW-Authenticate": [
                    str(contracts[401]["www_authenticate"]),
                    str(contracts[401]["www_authenticate"]),
                ],
            },
        ),
        error_fixture_response(
            contracts[401],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": [str(len(error_fixture_body(contracts[401])))],
                "WWW-Authenticate": ['Bearer realm="attacker"'],
            },
        ),
        error_fixture_response(
            contracts[400],
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": [str(len(error_fixture_body(contracts[400])))],
                "WWW-Authenticate": [str(contracts[401]["www_authenticate"])],
            },
        ),
        Response(
            bytes(BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1 + 1),
            status=400,
            headers={"Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1]},
        ),
    ]
    for response in adversarial:
        with pytest.raises(BootleLanternIssuanceClientErrorV1) as caught:
            client(Session(response)).authorize(credential())
        assert caught.value.status_code is None
        assert caught.value.code is None
        assert caught.value.retry_after_seconds is None


def test_response_rejects_changed_url_as_redirect_evidence() -> None:
    session = Session(
        Response(
            patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
            url="https://attacker.example/result",
        )
    )
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="response URL"):
        client(session).authorize(credential())
    assert len(session.calls) == 1


@pytest.mark.parametrize(
    "values",
    [
        [],
        ["Application/X-Norito"],
        ["application/octet-stream"],
        ["application/x-norito; charset=binary"],
        ["application/x-norito, application/x-norito"],
        ["application/x-norito", "application/x-norito"],
    ],
)
def test_response_rejects_missing_duplicate_or_parameterized_content_type(
    values: list[str],
) -> None:
    headers = {} if not values else {"Content-Type": values}
    session = Session(Response(patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1), headers=headers))
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="Content-Type"):
        client(session).authorize(credential())


@pytest.mark.parametrize("encoding", [["gzip"], ["identity"], ["br"], ["gzip", "br"]])
def test_response_rejects_any_content_encoding(encoding: list[str]) -> None:
    session = Session(
        Response(
            patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Encoding": encoding,
            },
        )
    )
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="Content-Encoding"):
        client(session).authorize(credential())


@pytest.mark.parametrize(
    "lengths",
    [
        ["0"],
        ["319"],
        ["321"],
        ["0320"],
        ["+320"],
        ["320 "],
        ["320, 320"],
        ["320", "320"],
    ],
)
def test_response_rejects_noncanonical_or_conflicting_content_length(
    lengths: list[str],
) -> None:
    session = Session(
        Response(
            patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
            headers={
                "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
                "Content-Length": lengths,
            },
        )
    )
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="Content-Length"):
        client(session).authorize(credential())


def test_response_accepts_one_canonical_exact_content_length_without_decoding() -> None:
    response = Response(
        patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
        headers={
            "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
            "Content-Length": [str(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)],
        },
    )
    assert client(Session(response)).authorize(credential()) == patterned(
        BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1
    )
    assert response.raw.read_calls == [(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 + 1, False)]


def test_success_response_rejects_www_authenticate_reserved_for_401() -> None:
    response = Response(
        patterned(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1),
        headers={
            "Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1],
            "WWW-Authenticate": ['Bearer realm="iroha-bootle-lantern-issuance"'],
        },
    )
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="unexpected WWW-Authenticate"):
        client(Session(response)).authorize(credential())


def test_transport_and_body_failures_are_sanitized_and_not_retried() -> None:
    leaked = "opaque-secret-must-not-appear"
    failed_session = Session(RuntimeError(leaked))
    with pytest.raises(BootleLanternIssuanceClientErrorV1) as transport_error:
        client(failed_session).authorize(credential())
    assert len(failed_session.calls) == 1
    assert leaked not in str(transport_error.value)
    assert transport_error.value.__cause__ is None

    class FailingRaw(RawBody):
        def read(self, amount: int, *, decode_content: bool) -> bytes:
            raise RuntimeError(leaked)

    body_response = success(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)
    body_response.raw = FailingRaw(
        b"",
        {"Content-Type": [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1]},
    )
    body_session = Session(body_response)
    with pytest.raises(BootleLanternIssuanceClientErrorV1) as body_error:
        client(body_session).authorize(credential())
    assert len(body_session.calls) == 1
    assert leaked not in str(body_error.value)
    assert body_response.closed


def test_response_without_bounded_raw_reader_fails_closed() -> None:
    response = success(BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1)
    response.raw = type("HeadersOnlyRaw", (), {"headers": response.raw.headers})()
    session = Session(response)
    with pytest.raises(BootleLanternIssuanceClientErrorV1, match="bounded byte stream"):
        client(session).authorize(credential())
    assert len(session.calls) == 1
    assert response.closed


@pytest.mark.parametrize(
    "base_url",
    [
        "",
        "torii.example",
        "http://torii.example",
        "https://user:secret@torii.example",
        "https://torii.example/v1",
        "https://torii.example/?",
        "https://torii.example/#",
        "https://torii.example/?query=1",
        "https://torii.example/#fragment",
        "https://torii.example\n",
    ],
)
def test_base_url_admission_is_origin_only_https(base_url: str) -> None:
    with pytest.raises(ValueError):
        BootleLanternIssuanceClientV1(base_url, session=Session(success(320)))
