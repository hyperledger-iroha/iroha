"""Strict native-backed SoraFS orderbook submission helpers."""

from __future__ import annotations

import json
import math
import re
from types import MappingProxyType
from typing import Any, Dict, Mapping, NoReturn, Optional, Sequence, Tuple, TypedDict, cast
from urllib.parse import urlsplit

import requests
from requests.cookies import RequestsCookieJar
from requests.structures import CaseInsensitiveDict

ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024
ORDERBOOK_RECEIPT_MAX_BYTES_V1 = 1024 * 1024
ORDERBOOK_SUBMISSION_TIMEOUT_SECONDS_V1 = 30.0
_HASH_HEX = re.compile(r"[0-9a-f]{64}")
_IDENTITY_KEYS = frozenset({"entrypoint_hash", "signed_transaction_hash"})
_RECEIPT_KEYS = frozenset({"payload", "signature"})
_PAYLOAD_KEYS = frozenset(
    {
        "entrypoint_hash",
        "signed_transaction_hash",
        "submitted_at_ms",
        "submitted_at_height",
        "signer",
    }
)
_FIXED_HEADERS = frozenset({
    "accept", "accept-encoding", "connection", "content-encoding", "content-length",
    "content-type", "expect", "host", "keep-alive", "prefer", "proxy-connection",
    "te", "trailer", "transfer-encoding", "upgrade", "x-http-method-override",
    "x-method-override",
})
_MAX_SIGNATURE_HEX_LENGTH = 2 * 3_309
_HTTP_ADAPTER = requests.adapters.HTTPAdapter
_HTTP_ADAPTER_SEND = _HTTP_ADAPTER.send
_HTTP_ADAPTER_CLOSE = _HTTP_ADAPTER.close


class SorafsOrderbookSubmissionIdentity(TypedDict):
    """Exact identities derived from the authenticated signed transaction."""

    entrypoint_hash: str
    signed_transaction_hash: str


class SorafsOrderbookSubmissionReceiptPayload(TypedDict):
    """Authenticated canonical receipt payload."""

    entrypoint_hash: str
    signed_transaction_hash: str
    submitted_at_ms: int
    submitted_at_height: int
    signer: str


class SorafsOrderbookSubmissionReceipt(TypedDict):
    """Pinned-signer Torii admission receipt."""

    payload: SorafsOrderbookSubmissionReceiptPayload
    signature: str


class SorafsOrderbookSubmissionAmbiguousError(RuntimeError):
    """Dispatch began, so callers must reconcile identity and never resubmit blindly."""

    def __init__(
        self, route: str, expected_identity: SorafsOrderbookSubmissionIdentity
    ) -> None:
        self.route = route
        self.expected_identity = MappingProxyType(dict(expected_identity))
        super().__init__(
            "SoraFS orderbook submission outcome is ambiguous after dispatch; "
            "do not resubmit automatically, reconcile the expected transaction identity"
        )


class SorafsOrderbookSubmissionMixin:
    """Fail-closed signed orderbook submission transport."""

    _base_url: str
    _default_headers: Mapping[str, str]
    _session: requests.Session

    def _configure_sorafs_orderbook_native_verifier(self, verifier: Any) -> None:
        if verifier is not None:
            _require_native_function(
                verifier,
                "inspect_sorafs_orderbook_submission_for_discriminant_v1",
            )
            _require_native_function(
                verifier, "verify_sorafs_orderbook_submission_receipt_v1"
            )
        self.__sorafs_orderbook_native_verifier = verifier

    def _sorafs_orderbook_native_verifier(self) -> Any:
        verifier = getattr(
            self,
            "_SorafsOrderbookSubmissionMixin__sorafs_orderbook_native_verifier",
            None,
        )
        if verifier is None:
            raise RuntimeError(
                "SoraFS orderbook submission requires an injected native verifier; "
                "use iroha_python.client.ToriiClient or pass orderbook_native_verifier="
            )
        return verifier

    def _sorafs_orderbook_expected_network_id(self, value: Any, context: str) -> Any:
        del context
        return value

    def _sorafs_orderbook_expected_chain_discriminant(self, context: str) -> int:
        del context
        raise RuntimeError(
            "SoraFS orderbook submission requires an expected chain discriminant provider"
        )

    def _sorafs_orderbook_submission_timeout(self) -> Any:
        return getattr(self, "_timeout", ORDERBOOK_SUBMISSION_TIMEOUT_SECONDS_V1)

    def submit_sorafs_orderbook_order(
        self, signed_transaction: Any, *, headers: Optional[Mapping[str, str]] = None,
        expected_receipt_signer: str, expected_network_id: Any = None,
        timeout: Optional[float] = None,
    ) -> SorafsOrderbookSubmissionReceipt:
        """Submit exactly one authenticated order instruction."""
        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/orders", signed_transaction, route="order", headers=headers,
            expected_network_id=expected_network_id,
            expected_receipt_signer=expected_receipt_signer, timeout=timeout,
            context="submit_sorafs_orderbook_order",
        )

    def submit_sorafs_orderbook_cancel(
        self, signed_transaction: Any, *, headers: Optional[Mapping[str, str]] = None,
        expected_receipt_signer: str, expected_network_id: Any = None,
        timeout: Optional[float] = None,
    ) -> SorafsOrderbookSubmissionReceipt:
        """Submit exactly one authenticated order cancellation instruction."""
        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/cancel", signed_transaction, route="cancel", headers=headers,
            expected_network_id=expected_network_id,
            expected_receipt_signer=expected_receipt_signer, timeout=timeout,
            context="submit_sorafs_orderbook_cancel",
        )

    def submit_sorafs_orderbook_receipt(
        self, signed_transaction: Any, *, headers: Optional[Mapping[str, str]] = None,
        expected_receipt_signer: str, expected_network_id: Any = None,
        timeout: Optional[float] = None,
    ) -> SorafsOrderbookSubmissionReceipt:
        """Submit exactly one authenticated settlement-receipt instruction."""
        return self._submit_sorafs_orderbook_transaction(
            "/v1/sorafs/orderbook/receipts", signed_transaction, route="receipt", headers=headers,
            expected_network_id=expected_network_id,
            expected_receipt_signer=expected_receipt_signer, timeout=timeout,
            context="submit_sorafs_orderbook_receipt",
        )

    def _submit_sorafs_orderbook_transaction(
        self, path: str, signed_transaction: Any, *, route: str,
        headers: Optional[Mapping[str, str]], expected_network_id: Any,
        expected_receipt_signer: str, context: str,
        timeout: Optional[float] = None,
    ) -> SorafsOrderbookSubmissionReceipt:
        try:
            session, base_url = self._session, self._base_url
        except AttributeError as transport_error:
            raise ValueError(
                f"{context} requires a verifiable one-shot HTTP transport"
            ) from transport_error
        require_orderbook_https_base_url(base_url, context)
        transport_state = snapshot_one_shot_transport(session, context)
        if timeout is None:
            timeout = self._sorafs_orderbook_submission_timeout()
        if isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or not math.isfinite(timeout) or timeout <= 0:
            raise ValueError(f"{context}.timeout must be a positive finite number")
        request_headers = validate_fixed_request_headers(
            getattr(self, "_default_headers", None), context=f"{context} default headers",
            allow_default_json_accept=True,
        )
        request_headers.update(validate_fixed_request_headers(headers, context=context))
        native = self._sorafs_orderbook_native_verifier()
        expected_network_id = self._sorafs_orderbook_expected_network_id(
            expected_network_id, context
        )
        expected_chain_discriminant = self._sorafs_orderbook_expected_chain_discriminant(context)
        body, identity, verify_native_receipt = prepare_orderbook_submission(
            native=native, route=route, signed_transaction=signed_transaction,
            expected_network_id=expected_network_id,
            expected_chain_discriminant=expected_chain_discriminant,
            expected_receipt_signer=expected_receipt_signer, context=context,
        )
        request_headers.update({"Accept": "application/x-norito", "Accept-Encoding": "identity",
                                "Content-Type": "application/x-norito"})
        adapter, prepared_request, transport = prepare_one_shot_request(
            transport_state, base_url, path, request_headers, body, context
        )
        response = None
        try:
            response = _HTTP_ADAPTER_SEND(
                adapter, prepared_request, stream=True, timeout=float(timeout), **transport
            )
            if response.status_code != 202:
                raise RuntimeError(f"{context} expected status 202")
            validate_response_headers(response, identity, context)
            receipt = read_bounded_receipt(response, context)
            close_response_best_effort(response)
            response = None
            return verify_receipt(
                verify_native_receipt=verify_native_receipt,
                receipt_norito=receipt, identity=identity,
                expected_receipt_signer=expected_receipt_signer, context=context,
            )
        except BaseException:
            if response is not None:
                close_response_best_effort(response)
        finally:
            close_adapter_best_effort(adapter)
        ambiguous_error = SorafsOrderbookSubmissionAmbiguousError(route, identity)
        ambiguous_error.__context__ = None
        raise ambiguous_error


def _require_native_function(native: Any, name: str) -> Any:
    function = getattr(native, name, None)
    if not callable(function):
        raise RuntimeError(
            f"native verifier is missing {name}; install/rebuild iroha_python for this SDK version"
        )
    return function


def require_orderbook_chain_discriminant(value: Any, context: str) -> int:
    """Validate the deployment discriminant supplied to native preflight."""
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 0xFFFF:
        raise ValueError(f"{context} requires orderbook_chain_discriminant within 0..=65535")
    return value


def _require_exact_mapping(value: Any, keys: frozenset[str], context: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping) or set(value) != keys:
        raise RuntimeError(f"{context} must contain exactly {', '.join(sorted(keys))}")
    return value


def _require_hash_hex(value: Any, context: str) -> str:
    if not isinstance(value, str) or _HASH_HEX.fullmatch(value) is None:
        raise RuntimeError(f"{context} must be exactly 32 lowercase hexadecimal bytes")
    return value


def _canonical_hash_literal(value: Any, context: str) -> str:
    if not isinstance(value, str) or re.fullmatch(
        r"hash:([0-9A-F]{64})#([0-9A-F]{4})", value
    ) is None:
        raise RuntimeError(f"{context} is invalid")
    body, supplied = value[5:69], value[70:]
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    if supplied != f"{crc:04X}":
        raise RuntimeError(f"{context} has an invalid checksum")
    return value


def validate_fixed_request_headers(
    headers: Mapping[str, str] | None,
    *,
    context: str,
    allow_default_json_accept: bool = False,
) -> Dict[str, str]:
    result: Dict[str, str] = {}
    if headers is None:
        return result
    if not isinstance(headers, Mapping):
        raise TypeError(f"{context}.headers must be a mapping")
    for raw_name, raw_value in headers.items():
        name, value = str(raw_name), str(raw_value)
        lowered = name.lower()
        if lowered in _FIXED_HEADERS:
            if allow_default_json_accept and lowered == "accept" and value == "application/json":
                continue
            raise ValueError(f"{context}.headers must not override {name}")
        result[name] = value
    return result


def prepare_one_shot_request(
    transport: Mapping[str, Any], base_url: Any, path: str,
    headers: Mapping[str, str], body: bytes, context: str,
) -> tuple[requests.adapters.HTTPAdapter, requests.PreparedRequest, Dict[str, Any]]:
    """Prepare and audit the exact stock Requests message sent once."""
    try:
        url = f"{base_url}{path}"
    except (AttributeError, LookupError, TypeError, ValueError) as error:
        raise ValueError(f"{context} requires a verifiable one-shot HTTP transport") from error
    merged_headers = dict(transport["headers"])
    if any(name.lower() == "prefer" for name in merged_headers):
        raise ValueError(f"{context} forbids an effective Prefer header")
    merged_headers.update(headers)
    prepared = requests.Request("POST", url, headers=merged_headers, data=body).prepare()
    if prepared.method != "POST" or prepared.url != url or prepared.body != body:
        raise ValueError(f"{context} transport changed the signed request target or body")
    for name, value in {
        "Accept": "application/x-norito", "Accept-Encoding": "identity",
        "Content-Type": "application/x-norito",
    }.items():
        if prepared.headers.get(name) != value:
            raise ValueError(f"{context} transport changed fixed {name}")
    if any(name.lower() == "prefer" for name in prepared.headers):
        raise ValueError(f"{context} forbids an effective Prefer header")
    expected_host = urlsplit(url).netloc
    if prepared.headers.get("Host") not in (None, expected_host):
        raise ValueError(f"{context} transport changed the request Host")
    if prepared.headers.get("Content-Length") != str(len(body)):
        raise ValueError(f"{context} transport changed the request Content-Length")
    for name in (
        "Connection", "Transfer-Encoding", "Content-Encoding", "Trailer", "Expect", "TE",
        "Keep-Alive", "Proxy-Connection", "Upgrade", "X-HTTP-Method-Override", "X-Method-Override",
    ):
        if name in prepared.headers:
            raise ValueError(f"{context} transport introduced forbidden {name}")
    for name in ("Authorization", "X-API-Token"):
        if prepared.headers.get(name) != CaseInsensitiveDict(merged_headers).get(name):
            raise ValueError(f"{context} transport changed intended {name}")
    adapter = _HTTP_ADAPTER(max_retries=0)
    return adapter, prepared, {
        "verify": transport["verify"], "cert": transport["cert"],
        "proxies": transport["proxies"],
    }


def snapshot_one_shot_transport(session: Any, context: str) -> Dict[str, Any]:
    if (
        type(session) is not requests.Session
        or type(session.headers) is not CaseInsensitiveDict
        or type(session.cookies) is not RequestsCookieJar
        or type(session.proxies) is not dict
        or any(name in session.__dict__ for name in ("send", "prepare_request", "get_adapter"))
        or session.auth is not None
        or len(session.cookies) != 0
        or any(session.hooks.values())
    ):
        raise ValueError(f"{context} requires an unmodified one-shot Requests transport")
    verify, cert = session.verify, session.cert
    if not (verify is True or type(verify) is str and verify):
        raise ValueError(f"{context} requires TLS certificate verification")
    if not (
        cert is None or type(cert) is str and cert
        or type(cert) is tuple and len(cert) == 2 and all(type(value) is str and value for value in cert)
    ):
        raise ValueError(f"{context} requires immutable TLS certificate configuration")
    if not all(type(key) is str and type(value) is str for key, value in session.proxies.items()):
        raise ValueError(f"{context} requires exact string proxy configuration")
    for name in session.headers:
        if name.lower() in _FIXED_HEADERS - {"accept", "accept-encoding", "connection"}:
            raise ValueError(f"{context} session headers must not set {name}")
    if session.headers.get("Connection") not in (None, "keep-alive"):
        raise ValueError(f"{context} session headers must not set Connection")
    return {
        "headers": {key: value for key, value in session.headers.items() if key.lower() != "connection"},
        "verify": verify, "cert": cert, "proxies": dict(session.proxies),
    }


def require_orderbook_https_base_url(base_url: Any, context: str) -> None:
    if type(base_url) is not str:
        raise ValueError(f"{context} requires a canonical HTTPS Torii base URL")
    parsed = urlsplit(base_url)
    if (
        parsed.scheme != "https" or not parsed.hostname or parsed.username is not None
        or parsed.password is not None or parsed.query or parsed.fragment
    ):
        raise ValueError(f"{context} requires a canonical HTTPS Torii base URL without userinfo, query, or fragment")


def read_bounded_receipt(response: Any, context: str) -> bytes:
    """Read and close one receipt without buffering beyond the V1 bound."""
    length = response_header(response, "Content-Length", context)
    if length is not None:
        if re.fullmatch(r"(?:0|[1-9][0-9]*)", length) is None:
            raise RuntimeError(f"{context} response Content-Length is not canonical")
        if int(length) > ORDERBOOK_RECEIPT_MAX_BYTES_V1:
            raise RuntimeError(f"{context} response receipt exceeds its byte bound")
    body = bytearray()
    for chunk in response.iter_content(chunk_size=8192, decode_unicode=False):
        if not isinstance(chunk, (bytes, bytearray)):
            raise RuntimeError(f"{context} response yielded a non-byte chunk")
        if len(body) + len(chunk) > ORDERBOOK_RECEIPT_MAX_BYTES_V1:
            raise RuntimeError(f"{context} response receipt exceeds its byte bound")
        body.extend(chunk)
    if not body:
        raise RuntimeError(f"{context} response returned an empty receipt")
    if length is not None and int(length) != len(body):
        raise RuntimeError(f"{context} response Content-Length does not match its body")
    return bytes(body)


def close_response_best_effort(response: Any) -> None:
    try:
        response.close()
    except BaseException:
        pass


def close_adapter_best_effort(adapter: Any) -> None:
    try:
        _HTTP_ADAPTER_CLOSE(adapter)
    except BaseException:
        pass


def prepare_orderbook_submission(
    *,
    native: Any,
    route: str,
    signed_transaction: Any,
    expected_network_id: Any,
    expected_chain_discriminant: Any,
    expected_receipt_signer: Any,
    context: str,
) -> tuple[bytes, SorafsOrderbookSubmissionIdentity, Any]:
    inspect = _require_native_function(
        native,
        "inspect_sorafs_orderbook_submission_for_discriminant_v1",
    )
    verify_native_receipt = _require_native_function(
        native, "verify_sorafs_orderbook_submission_receipt_v1"
    )
    if not isinstance(signed_transaction, (bytes, bytearray, memoryview)):
        raise TypeError(f"{context}.signed_transaction must be bytes-like")
    body = bytes(signed_transaction)
    if not body or len(body) > ORDERBOOK_TRANSACTION_MAX_BYTES_V1:
        raise ValueError(
            f"{context}.signed_transaction must contain 1..{ORDERBOOK_TRANSACTION_MAX_BYTES_V1} bytes"
        )
    if expected_network_id is None:
        raise ValueError(f"{context}.expected_network_id is required")
    if isinstance(expected_chain_discriminant, bool) or not isinstance(expected_chain_discriminant, int) or not 0 <= expected_chain_discriminant <= 0xFFFF:
        raise ValueError(f"{context}.expected_chain_discriminant must fit in u16")
    if not isinstance(expected_receipt_signer, str) or not expected_receipt_signer:
        raise ValueError(f"{context}.expected_receipt_signer is required")
    identity = _require_exact_mapping(
        inspect(
            route, expected_network_id, expected_chain_discriminant,
            expected_receipt_signer, body,
        ),
        _IDENTITY_KEYS,
        "native orderbook submission identity",
    )
    return body, cast(SorafsOrderbookSubmissionIdentity, {
        key: _require_hash_hex(identity[key], f"native orderbook submission identity.{key}")
        for key in _IDENTITY_KEYS
    }), verify_native_receipt


def response_header(response: Any, name: str, context: str) -> str | None:
    raw = getattr(response, "raw", None)
    raw_headers = getattr(raw, "headers", None)
    getlist = getattr(raw_headers, "getlist", None)
    if callable(getlist):
        values = getlist(name)
        if len(values) > 1:
            raise RuntimeError(f"{context} response contains duplicate {name} headers")
    value = response.headers.get(name)
    if isinstance(value, str) and "," in value:
        raise RuntimeError(f"{context} response contains a coalesced {name} header")
    return value


def validate_response_headers(
    response: Any,
    identity: SorafsOrderbookSubmissionIdentity,
    context: str,
) -> None:
    content_type = response_header(response, "Content-Type", context)
    if content_type != "application/x-norito":
        raise RuntimeError(f"{context} response Content-Type must be exactly application/x-norito")
    content_encoding = response_header(response, "Content-Encoding", context)
    if content_encoding not in (None, "identity"):
        raise RuntimeError(f"{context} response Content-Encoding must be absent or identity")
    for header, expected in (
        ("x-iroha-entrypoint-hash", identity["entrypoint_hash"]),
        (
            "x-iroha-signed-transaction-hash",
            identity["signed_transaction_hash"],
        ),
    ):
        value = response_header(response, header, context)
        if value is None or _HASH_HEX.fullmatch(value) is None:
            raise RuntimeError(f"{context} response {header} must be one lowercase 32-byte hash")
        if value != expected:
            raise RuntimeError(f"{context} response {header} does not match the submitted transaction")


def verify_receipt(
    *,
    verify_native_receipt: Any,
    receipt_norito: bytes,
    identity: SorafsOrderbookSubmissionIdentity,
    expected_receipt_signer: str,
    context: str,
) -> SorafsOrderbookSubmissionReceipt:
    if not receipt_norito:
        raise RuntimeError(f"{context} response returned an empty receipt")
    raw = verify_native_receipt(
        receipt_norito,
        identity["entrypoint_hash"],
        identity["signed_transaction_hash"],
        expected_receipt_signer,
    )
    if not isinstance(raw, str):
        raise RuntimeError("native orderbook receipt verifier returned non-text JSON")
    def unique_object(pairs: Sequence[Tuple[str, Any]]) -> Dict[str, Any]:
        result: Dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate field {key}")
            result[key] = value
        return result

    def canonical_uint(token: str) -> int:
        if re.fullmatch(r"(?:0|[1-9][0-9]*)", token) is None:
            raise ValueError("noncanonical unsigned integer")
        value = int(token)
        if value > (1 << 64) - 1:
            raise ValueError("unsigned integer exceeds u64")
        return value

    def reject_noninteger(token: str) -> NoReturn:
        raise ValueError(f"noncanonical numeric value {token}")

    try:
        receipt = json.loads(
            raw,
            object_pairs_hook=unique_object,
            parse_int=canonical_uint,
            parse_float=reject_noninteger,
            parse_constant=reject_noninteger,
        )
    except (TypeError, ValueError, RecursionError) as error:
        raise RuntimeError("native orderbook receipt verifier returned invalid JSON") from error
    receipt = _require_exact_mapping(receipt, _RECEIPT_KEYS, "verified orderbook receipt")
    payload = _require_exact_mapping(
        receipt["payload"], _PAYLOAD_KEYS, "verified orderbook receipt.payload"
    )
    for key in ("entrypoint_hash", "signed_transaction_hash"):
        literal = _canonical_hash_literal(
            payload[key], f"verified orderbook receipt.payload.{key}"
        )
        if literal[5:69].lower() != identity[key]:
            raise RuntimeError(
                f"verified orderbook receipt.payload.{key} changed at the native boundary"
            )
    for key in ("submitted_at_ms", "submitted_at_height"):
        if isinstance(payload[key], bool) or not isinstance(payload[key], int):
            raise RuntimeError(f"verified orderbook receipt.payload.{key} must be u64")
    if payload["signer"] != expected_receipt_signer:
        raise RuntimeError("verified orderbook receipt signer changed at the native boundary")
    signature = receipt["signature"]
    if (
        not isinstance(signature, str)
        or len(signature) > _MAX_SIGNATURE_HEX_LENGTH
        or re.fullmatch(r"(?:[0-9A-F]{2})+", signature) is None
    ):
        raise RuntimeError("verified orderbook receipt signature is invalid")
    return cast(SorafsOrderbookSubmissionReceipt, dict(receipt))
