"""Strict signed SoraFS orderbook submission transport tests."""

from __future__ import annotations

import json
from typing import Any, get_type_hints

import pytest
import requests
from requests.structures import CaseInsensitiveDict
import iroha_torii_client.orderbook_submission as orderbook_submission

from iroha_torii_client import (
    SorafsOrderbookSubmissionAmbiguousError,
    SorafsOrderbookSubmissionIdentity,
    SorafsOrderbookSubmissionReceipt,
    SorafsOrderbookSubmissionReceiptPayload,
    ToriiClient,
)


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

    def inspect_sorafs_orderbook_submission_v1(
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


_TRANSPORTS: list[Transport] = []
_CLOSED_ADAPTERS: list[requests.adapters.HTTPAdapter] = []


@pytest.fixture(autouse=True)
def _patch_stock_adapter(monkeypatch: pytest.MonkeyPatch):
    original_close = orderbook_submission._HTTP_ADAPTER_CLOSE

    def send(adapter: requests.adapters.HTTPAdapter, request: Any, **kwargs: Any):
        if not _TRANSPORTS:
            raise AssertionError("unexpected HTTP adapter send")
        transport = _TRANSPORTS.pop(0)
        transport.calls.append({"request": request, **kwargs})
        if transport.before_return is not None:
            transport.before_return()
        if isinstance(transport.outcome, BaseException):
            raise transport.outcome
        return transport.outcome

    def close(adapter: requests.adapters.HTTPAdapter) -> None:
        _CLOSED_ADAPTERS.append(adapter)
        original_close(adapter)

    monkeypatch.setattr(orderbook_submission, "_HTTP_ADAPTER_SEND", send)
    monkeypatch.setattr(orderbook_submission, "_HTTP_ADAPTER_CLOSE", close)
    yield
    _TRANSPORTS.clear()
    _CLOSED_ADAPTERS.clear()


NETWORK = object()


def test_public_submit_receipt_types_are_precise_and_exported() -> None:
    assert get_type_hints(ToriiClient.submit_sorafs_orderbook_order)["return"] is SorafsOrderbookSubmissionReceipt
    assert SorafsOrderbookSubmissionIdentity.__required_keys__ == frozenset(IDENTITY)
    assert SorafsOrderbookSubmissionReceiptPayload.__required_keys__ == frozenset({
        "entrypoint_hash", "signed_transaction_hash", "submitted_at_ms",
        "submitted_at_height", "signer",
    })


def submit(
    response: Response | BaseException = Response(), *, verifier: Verifier | None = None,
    body: Any = b"\x01", session_headers: dict[str, str] | None = None,
):
    session, transport = stock_session(response)
    if session_headers:
        session.headers.update(session_headers)
    client = ToriiClient(
        "https://torii.example",
        session=session,
        orderbook_native_verifier=verifier or Verifier(),
        orderbook_chain_discriminant=369,
    )
    return client, transport, body


def stock_session(
    response: Response | BaseException,
) -> tuple[requests.Session, Transport]:
    session, transport = requests.Session(), Transport(response)
    session.trust_env = False
    _TRANSPORTS.append(transport)
    return session, transport


def test_submit_snapshots_and_binds_exact_transport_and_receipt() -> None:
    original = bytearray(b"\x01\x02")
    verifier = Verifier(original=original)
    client, transport, _ = submit(verifier=verifier, body=original)
    receipt = client.submit_sorafs_orderbook_order(
        original,
        expected_network_id=NETWORK,
        expected_receipt_signer=SIGNER,
        headers={"X-Trace": "one"},
    )
    assert receipt["payload"]["signer"] == SIGNER
    assert verifier.inspected == b"\x01\x02"
    call = transport.calls[0]
    assert call["request"].body is verifier.inspected
    assert call["stream"] is True and len(transport.calls) == 1
    assert call["timeout"] == 30.0
    assert call["request"].method == "POST"
    assert call["request"].url == "https://torii.example/v1/sorafs/orderbook/orders"
    assert {name: call["request"].headers[name] for name in (
        "X-Trace", "Accept", "Accept-Encoding", "Content-Type",
    )} == {"X-Trace": "one", "Accept": "application/x-norito",
          "Accept-Encoding": "identity", "Content-Type": "application/x-norito"}


def test_submit_snapshots_native_receipt_callable_before_dispatch() -> None:
    verifier = Verifier()
    client, transport, _ = submit(verifier=verifier)
    transport.before_return = lambda: setattr(
        verifier,
        "verify_sorafs_orderbook_submission_receipt_v1",
        lambda *_: (_ for _ in ()).throw(RuntimeError("mutable replacement")),
    )
    receipt = client.submit_sorafs_orderbook_order(
        b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
    )
    assert receipt["payload"]["signer"] == SIGNER


def test_submit_snapshots_target_and_stock_transport_before_native_preflight() -> None:
    verifier = Verifier()
    client, transport, _ = submit(verifier=verifier)
    replacement, replacement_transport = stock_session(AssertionError("must not send"))
    def mutate_client() -> None:
        client._session = replacement
        client._base_url = "https://attacker.example"
    verifier.during_inspect = mutate_client
    client.submit_sorafs_orderbook_order(
        b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
    )
    assert len(transport.calls) == 1
    assert replacement_transport.calls == []


def test_default_client_and_missing_trust_inputs_fail_before_http() -> None:
    session = requests.Session()
    client = ToriiClient("https://torii.example", session=session)
    with pytest.raises(RuntimeError, match="injected native verifier"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    verifier = Verifier()
    client = ToriiClient(
        "https://torii.example", session=session, orderbook_native_verifier=verifier,
        orderbook_chain_discriminant=369,
    )
    with pytest.raises(ValueError, match="expected_network_id"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_receipt_signer=SIGNER
        )
    assert session is client._session


@pytest.mark.parametrize(
    "response",
    [
        ConnectionError("reset"),
        Response(status=500),
        Response(headers={"Content-Type": "application/json"}),
        Response(headers={"x-iroha-entrypoint-hash": "bb" * 32}),
        Response(duplicates={"x-iroha-entrypoint-hash"}),
        Response(headers={"Content-Length": "1048577"}),
        Response(headers={"Content-Length": "2"}),
        Response(body=b""),
        Response(status=500, close_error=RuntimeError("close failed")),
        Response(status=500, close_error=KeyboardInterrupt()),
    ],
)
def test_post_dispatch_failures_are_ambiguous_and_never_retried(response: Any) -> None:
    client, transport, _ = submit(response)
    with pytest.raises(SorafsOrderbookSubmissionAmbiguousError) as caught:
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert caught.value.route == "order"
    assert dict(caught.value.expected_identity) == IDENTITY
    assert caught.value.__cause__ is None
    assert caught.value.__context__ is None
    assert not any(hasattr(caught.value, name) for name in ("request", "response", "body"))
    assert len(transport.calls) == 1


@pytest.mark.parametrize(
    "verified_json",
    [
        '{"payload":{},"payload":{},"signature":"AB"}',
        receipt_json(submitted_at_ms=1.5),
        receipt_json(submitted_at_ms=True),
        receipt_json(submitted_at_ms=1 << 64),
        receipt_json(entrypoint_hash="hash:" + "AA" * 32 + "#0000"),
        receipt_json(tx_hash=canonical_hash(0xAA)),
        json.dumps({"payload": json.loads(receipt_json())["payload"], "signature": "ab"}),
    ],
)
def test_injected_verifier_json_is_strict_and_failure_is_ambiguous(verified_json: str) -> None:
    client, transport, _ = submit(verifier=Verifier(verified_json=verified_json))
    with pytest.raises(SorafsOrderbookSubmissionAmbiguousError):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert len(transport.calls) == 1


def test_effective_prefer_and_caller_fixed_headers_fail_before_http() -> None:
    client, transport, _ = submit(session_headers={"Prefer": "return=minimal"})
    with pytest.raises(ValueError, match="Prefer"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []
    for name in (
        "Accept", "Accept-Encoding", "Connection", "Content-Encoding", "Content-Length",
        "Content-Type", "Expect", "Host", "Keep-Alive", "Prefer", "Proxy-Connection", "TE",
        "Trailer", "Transfer-Encoding", "Upgrade", "X-HTTP-Method-Override", "X-Method-Override",
    ):
        client, transport, _ = submit()
        with pytest.raises(ValueError, match="must not override"):
            client.submit_sorafs_orderbook_order(
                b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER,
                headers={name: "forbidden"},
            )
        assert transport.calls == []


@pytest.mark.parametrize("base_url", [
    "http://torii.example", "ftp://torii.example", "https://user:pass@torii.example",
    "https://torii.example?route=elsewhere", "https://torii.example#fragment",
])
def test_noncanonical_or_insecure_base_url_fails_before_http(base_url: str) -> None:
    client, transport, _ = submit()
    client._base_url = base_url
    with pytest.raises(ValueError, match="canonical HTTPS"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []


@pytest.mark.parametrize("name", [
    "Content-Encoding", "Content-Length", "Content-Type", "Expect", "Host", "Keep-Alive",
    "Prefer", "Proxy-Connection", "TE", "Trailer", "Transfer-Encoding", "Upgrade",
    "X-HTTP-Method-Override", "X-Method-Override",
])
def test_session_routing_and_framing_headers_fail_before_http(name: str) -> None:
    client, transport, _ = submit(session_headers={name: "forbidden"})
    with pytest.raises(ValueError, match="session headers"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []


def test_noncanonical_session_connection_tokens_fail_before_http() -> None:
    client, transport, _ = submit(session_headers={"Connection": "keep-alive, upgrade"})
    with pytest.raises(ValueError, match="Connection"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []


def test_mounted_retrying_adapter_is_never_used() -> None:
    client, transport, _ = submit()
    mounted = requests.adapters.HTTPAdapter(max_retries=1)
    mounted.send = lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("mounted adapter used"))
    client._session.mount("https://", mounted)
    client.submit_sorafs_orderbook_order(
        b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
    )
    assert len(transport.calls) == 1 and len(_CLOSED_ADAPTERS) == 1


def test_redirect_body_is_never_consumed_by_the_one_shot_adapter_path() -> None:
    class Redirect(Response):
        @property
        def content(self):
            raise AssertionError("redirect body must not be read")

    response = Redirect(status=307)
    client, transport, _ = submit(response)
    with pytest.raises(SorafsOrderbookSubmissionAmbiguousError):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert response.closed and len(transport.calls) == 1


def test_strict_transport_ignores_environment_netrc_and_closes_fresh_adapter() -> None:
    client, transport, _ = submit()
    client._session.trust_env = True
    client._session.headers["Authorization"] = "Bearer intended"
    client.submit_sorafs_orderbook_order(
        b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
    )
    assert transport.calls[0]["request"].headers["Authorization"] == "Bearer intended"
    assert transport.calls[0]["proxies"] == {}
    assert len(_CLOSED_ADAPTERS) == 1


@pytest.mark.parametrize("mutation", ["subclass", "auth", "session_send"])
def test_unverifiable_transport_mutations_fail_before_http(mutation: str) -> None:
    client, transport, _ = submit()
    session = client._session
    if mutation == "subclass":
        class SessionSubclass(requests.Session):
            pass
        client._session = SessionSubclass()
    elif mutation == "auth":
        session.auth = lambda request: request
    elif mutation == "session_send":
        session.send = lambda *args, **kwargs: None
    with pytest.raises(ValueError, match="unmodified one-shot"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []


@pytest.mark.parametrize("mutation", ["verify_false", "mutable_cert", "bad_proxy", "cookie"])
def test_unqualified_tls_proxy_or_cookie_state_fails_before_http(mutation: str) -> None:
    client, transport, _ = submit()
    session = client._session
    if mutation == "verify_false":
        session.verify = False
    elif mutation == "mutable_cert":
        session.cert = ["cert.pem", "key.pem"]
    elif mutation == "bad_proxy":
        session.proxies["https"] = object()
    else:
        session.cookies.set("ambient", "cookie")
    with pytest.raises(ValueError, match="TLS|proxy|one-shot"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK, expected_receipt_signer=SIGNER
        )
    assert transport.calls == []


@pytest.mark.parametrize("timeout", [True, 0, -1, float("nan"), float("inf")])
def test_invalid_timeout_fails_before_http(timeout: Any) -> None:
    client, transport, _ = submit()
    with pytest.raises(ValueError, match="positive finite"):
        client.submit_sorafs_orderbook_order(
            b"\x01", expected_network_id=NETWORK,
            expected_receipt_signer=SIGNER, timeout=timeout,
        )
    assert transport.calls == []
