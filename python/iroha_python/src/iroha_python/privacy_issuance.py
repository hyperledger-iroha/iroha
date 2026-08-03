"""Strict first-release client for native Bootle/Lantern blind issuance."""

from __future__ import annotations

import base64
import binascii
import hashlib
import hmac
import json
import re
import struct
import threading
from collections.abc import Mapping, Sequence
from typing import Any
from urllib.parse import urlsplit, urlunsplit

import requests
from requests.adapters import HTTPAdapter

BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1 = "/v1/privacy/bootle-lantern/issuance/authorize"
BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1 = "/v1/privacy/bootle-lantern/issuance/issue"
BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 = "application/x-norito"
BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 = 320
BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 = 71_896
BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 = 3_176
BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 = 4_096
BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1 = 512

_MAX_ENCODED_CREDENTIAL_BYTES = ((BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 + 2) // 3) * 4
_CANONICAL_BASE64URL_RE = re.compile(r"^[A-Za-z0-9_-]+$")
_JSON_MEDIA_TYPE_V1 = "application/json"
_AUTHORIZATION_MAGIC_V1 = b"ILA1"
_BLIND_REQUEST_MAGIC_V1 = b"ILQ1"
_RESPONSE_MAGIC_V1 = b"ILR1"
_WWW_AUTHENTICATE_VALUE_V1 = 'Bearer realm="iroha-bootle-lantern-issuance"'
_ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope"
_ERROR_CONTRACT_V1: dict[int, tuple[str, str, int | None]] = {
    400: ("privacy_issuance_invalid_request", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, None),
    401: ("privacy_issuance_unauthorized", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, None),
    406: ("privacy_issuance_not_acceptable", _JSON_MEDIA_TYPE_V1, None),
    409: ("privacy_issuance_state_conflict", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, None),
    413: ("privacy_issuance_payload_too_large", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, None),
    415: (
        "privacy_issuance_unsupported_media_type",
        BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
        None,
    ),
    429: ("privacy_issuance_capacity_exhausted", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, 1),
    503: ("privacy_issuance_unavailable", BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, None),
}
_CRC64_POLYNOMIAL_V1 = 0xC96C_5795_D787_0F42
_CRC64_MASK_V1 = 0xFFFF_FFFF_FFFF_FFFF


def _crc64_table_v1() -> tuple[int, ...]:
    table = []
    for index in range(256):
        value = index
        for _ in range(8):
            value = value >> 1 if value & 1 == 0 else (value >> 1) ^ _CRC64_POLYNOMIAL_V1
        table.append(value)
    return tuple(table)


_CRC64_TABLE_V1 = _crc64_table_v1()


class BootleLanternIssuanceClientErrorV1(RuntimeError):
    """Fail-closed transport or response validation failure."""

    def __init__(
        self,
        message: str,
        *,
        status_code: int | None = None,
        code: str | None = None,
        retry_after_seconds: int | None = None,
    ) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.code = code
        self.retry_after_seconds = retry_after_seconds


class BootleLanternIssuanceCredentialV1:
    """Opaque, bounded issuer credential with explicit in-memory destruction."""

    __slots__ = ("_bytes", "_destroyed", "_lock")

    def __init__(self, secret: bytes | bytearray | memoryview) -> None:
        if not isinstance(secret, (bytes, bytearray, memoryview)):
            raise TypeError("Bootle/Lantern issuance credential must be bytes-like")
        try:
            copied = bytearray(secret)
        except (TypeError, ValueError) as error:
            raise TypeError("Bootle/Lantern issuance credential must be bytes-like") from error
        if not 1 <= len(copied) <= BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1:
            copied[:] = b"\x00" * len(copied)
            raise ValueError(
                "Bootle/Lantern issuance credential must contain "
                f"1..{BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1} bytes"
            )
        self._bytes = copied
        self._destroyed = False
        self._lock = threading.RLock()

    @classmethod
    def from_opaque_bytes(
        cls, secret: bytes | bytearray | memoryview
    ) -> BootleLanternIssuanceCredentialV1:
        """Defensively copy and validate opaque credential bytes."""

        return cls(secret)

    @classmethod
    def from_canonical_base64url(cls, encoded: str) -> BootleLanternIssuanceCredentialV1:
        """Decode exactly one canonical unpadded base64url credential."""

        if not isinstance(encoded, str):
            raise TypeError("Bootle/Lantern issuance credential must be a string")
        if (
            not encoded
            or len(encoded) > _MAX_ENCODED_CREDENTIAL_BYTES
            or len(encoded) % 4 == 1
            or _CANONICAL_BASE64URL_RE.fullmatch(encoded) is None
        ):
            raise ValueError(
                "Bootle/Lantern issuance credential must be canonical unpadded base64url"
            )
        try:
            ascii_encoded = encoded.encode("ascii")
            decoded = bytearray(
                base64.b64decode(
                    ascii_encoded + b"=" * (-len(ascii_encoded) % 4),
                    altchars=b"-_",
                    validate=True,
                )
            )
        except (UnicodeEncodeError, binascii.Error, ValueError) as error:
            raise ValueError(
                "Bootle/Lantern issuance credential must be canonical unpadded base64url"
            ) from error
        try:
            if (
                not 1 <= len(decoded) <= BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1
                or base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii") != encoded
            ):
                raise ValueError(
                    "Bootle/Lantern issuance credential must be canonical unpadded base64url"
                )
            return cls(decoded)
        finally:
            decoded[:] = b"\x00" * len(decoded)

    def _authorization_header_value(self) -> str:
        with self._lock:
            if self._destroyed:
                raise ValueError("Bootle/Lantern issuance credential has been destroyed")
            encoded = base64.urlsafe_b64encode(self._bytes).rstrip(b"=").decode("ascii")
            return f"Bearer {encoded}"

    def destroy(self) -> None:
        """Overwrite the retained credential byte buffer; idempotent."""

        with self._lock:
            if not self._destroyed:
                self._bytes[:] = b"\x00" * len(self._bytes)
                self._destroyed = True

    close = destroy

    def __enter__(self) -> BootleLanternIssuanceCredentialV1:
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> None:
        self.destroy()

    def __repr__(self) -> str:
        return "BootleLanternIssuanceCredentialV1([REDACTED])"

    __str__ = __repr__

    def __del__(self) -> None:
        try:
            self.destroy()
        except Exception:
            pass


def _validate_base_url(base_url: str) -> str:
    if (
        not isinstance(base_url, str)
        or not base_url
        or "?" in base_url
        or "#" in base_url
        or any(ord(char) < 0x21 for char in base_url)
    ):
        raise ValueError("Bootle/Lantern issuance requires an absolute HTTPS base URL")
    try:
        parsed = urlsplit(base_url)
        port = parsed.port
    except ValueError as error:
        raise ValueError("Bootle/Lantern issuance base URL is invalid") from error
    if (
        parsed.scheme.lower() != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
        or parsed.path not in ("", "/")
    ):
        raise ValueError("Bootle/Lantern issuance requires an origin-only HTTPS base URL")
    host = parsed.hostname
    if ":" in host and not host.startswith("["):
        host = f"[{host}]"
    authority = host if port is None else f"{host}:{port}"
    return urlunsplit(("https", authority, "", "", ""))


def _mapping_header_values(headers: Mapping[str, Any] | None, name: str) -> list[str]:
    if headers is None:
        return []
    values: list[str] = []
    for candidate, value in headers.items():
        if str(candidate).lower() != name.lower():
            continue
        if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
            values.extend(str(item) for item in value)
        else:
            values.append(str(value))
    return values


def _response_header_values(response: Any, name: str) -> list[str]:
    raw_headers = getattr(getattr(response, "raw", None), "headers", None)
    if raw_headers is not None:
        for method_name in ("getlist", "get_all"):
            method = getattr(raw_headers, method_name, None)
            if callable(method):
                values = method(name)
                if values:
                    return [str(value) for value in values]
        if isinstance(raw_headers, Mapping):
            values = _mapping_header_values(raw_headers, name)
            if values:
                return values
    return _mapping_header_values(getattr(response, "headers", None), name)


def _validate_response_headers(response: Any, operation: str, expected_bytes: int) -> None:
    content_types = _response_header_values(response, "Content-Type")
    if content_types != [BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1]:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response Content-Type must be exactly "
            f"{BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1}"
        )
    if _response_header_values(response, "Content-Encoding"):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response must not contain Content-Encoding"
        )
    if _response_header_values(response, "WWW-Authenticate"):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response contains an unexpected WWW-Authenticate"
        )
    lengths = _response_header_values(response, "Content-Length")
    if not lengths:
        return
    if len(lengths) != 1 or lengths[0] != str(expected_bytes):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response Content-Length must be canonical and exact"
        )


def _validate_error_response_headers(
    response: Any,
    operation: str,
    *,
    status: int,
    expected_media_type: str,
    actual_bytes: int,
    retry_after_seconds: int | None,
) -> None:
    if _response_header_values(response, "Content-Type") != [expected_media_type]:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response Content-Type is invalid"
        )
    if _response_header_values(response, "Content-Encoding"):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response must not contain Content-Encoding"
        )
    lengths = _response_header_values(response, "Content-Length")
    if lengths and (len(lengths) != 1 or lengths[0] != str(actual_bytes)):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response Content-Length is invalid"
        )
    retry_after = _response_header_values(response, "Retry-After")
    if retry_after_seconds == 1:
        if retry_after != ["1"]:
            raise BootleLanternIssuanceClientErrorV1(
                f"{operation} error response Retry-After is invalid"
            )
    elif retry_after:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response contains an unexpected Retry-After"
        )
    www_authenticate = _response_header_values(response, "WWW-Authenticate")
    if status == 401:
        if www_authenticate != [_WWW_AUTHENTICATE_VALUE_V1]:
            raise BootleLanternIssuanceClientErrorV1(
                f"{operation} error response WWW-Authenticate is invalid"
            )
    elif www_authenticate:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response contains an unexpected WWW-Authenticate"
        )


def _read_exact_response_body(
    response: Any,
    operation: str,
    expected_bytes: int,
    expected_magic: bytes,
) -> bytes:
    raw = getattr(response, "raw", None)
    reader = getattr(raw, "read", None)
    if not callable(reader):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response body is not a bounded byte stream"
        )
    body = reader(expected_bytes + 1, decode_content=False)
    if not isinstance(body, (bytes, bytearray, memoryview)):
        raise BootleLanternIssuanceClientErrorV1(f"{operation} response body is unavailable")
    copied = bytes(body)
    if len(copied) != expected_bytes:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response must be exactly {expected_bytes} bytes"
        )
    if not hmac.compare_digest(copied[: len(expected_magic)], expected_magic):
        raise BootleLanternIssuanceClientErrorV1(f"{operation} response wire magic is invalid")
    return copied


def _read_bounded_response_body(response: Any, operation: str, maximum_bytes: int) -> bytes:
    raw = getattr(response, "raw", None)
    reader = getattr(raw, "read", None)
    if not callable(reader):
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} response body is not a bounded byte stream"
        )
    body = reader(maximum_bytes + 1, decode_content=False)
    if not isinstance(body, (bytes, bytearray, memoryview)):
        raise BootleLanternIssuanceClientErrorV1(f"{operation} response body is unavailable")
    copied = bytes(body)
    if not copied or len(copied) > maximum_bytes:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} error response body has an invalid length"
        )
    return copied


def _crc64_v1(payload: bytes) -> int:
    value = _CRC64_MASK_V1
    for byte in payload:
        value = _CRC64_TABLE_V1[(value ^ byte) & 0xFF] ^ (value >> 8)
    return (value ^ _CRC64_MASK_V1) & _CRC64_MASK_V1


def _read_compact_length_v1(payload: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    count = 0
    while offset < len(payload) and count < 10:
        byte = payload[offset]
        offset += 1
        count += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if (count > 1 and byte == 0) or value > 0xFFFF_FFFF_FFFF_FFFF:
                raise ValueError("non-canonical compact length")
            return value, offset
        shift += 7
    raise ValueError("invalid compact length")


def _read_norito_string_v1(payload: bytes, offset: int) -> tuple[str, int]:
    length, offset = _read_compact_length_v1(payload, offset)
    end = offset + length
    if end > len(payload):
        raise ValueError("truncated Norito string")
    return payload[offset:end].decode("utf-8", errors="strict"), end


def _read_norito_field_v1(payload: bytes, offset: int) -> tuple[bytes, int]:
    length, offset = _read_compact_length_v1(payload, offset)
    end = offset + length
    if end > len(payload):
        raise ValueError("truncated error-envelope field")
    return payload[offset:end], end


def _read_norito_string_field_v1(payload: bytes, offset: int) -> tuple[str, int]:
    field, offset = _read_norito_field_v1(payload, offset)
    value, field_offset = _read_norito_string_v1(field, 0)
    if field_offset != len(field):
        raise ValueError("trailing bytes in error-envelope string field")
    return value, offset


def _decode_norito_error_envelope_v1(body: bytes) -> tuple[str, str]:
    if len(body) < 40 or body[:6] != b"NRT0\x00\x00":
        raise ValueError("invalid Norito header")
    schema_hash = hashlib.sha256(
        b"norito:v1:type-name\x00" + _ERROR_ENVELOPE_TYPE_NAME_V1.encode("utf-8")
    ).digest()[:16]
    if not hmac.compare_digest(body[6:22], schema_hash):
        raise ValueError("wrong error-envelope schema")
    if body[22] != 0 or body[39] != 0x02:
        raise ValueError("non-canonical error-envelope framing")
    payload_length = struct.unpack_from("<Q", body, 23)[0]
    if payload_length == 0 or len(body) != 40 + payload_length:
        raise ValueError("invalid error-envelope payload length")
    payload = body[40:]
    if struct.unpack_from("<Q", body, 31)[0] != _crc64_v1(payload):
        raise ValueError("error-envelope CRC mismatch")
    code, offset = _read_norito_string_field_v1(payload, 0)
    message, offset = _read_norito_string_field_v1(payload, offset)
    details, offset = _read_norito_field_v1(payload, offset)
    if details != b"\x00":
        raise ValueError("error details must be absent")
    if offset != len(payload):
        raise ValueError("trailing error-envelope payload")
    return code, message


def _decode_error_response_v1(
    response: Any, operation: str, status: int
) -> BootleLanternIssuanceClientErrorV1:
    contract = _ERROR_CONTRACT_V1.get(status)
    if contract is None:
        return BootleLanternIssuanceClientErrorV1(
            f"{operation} returned an unsupported error response"
        )
    expected_code, expected_media_type, retry_after_seconds = contract
    try:
        body = _read_bounded_response_body(
            response, operation, BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1
        )
        _validate_error_response_headers(
            response,
            operation,
            status=status,
            expected_media_type=expected_media_type,
            actual_bytes=len(body),
            retry_after_seconds=retry_after_seconds,
        )
        if status == 406:
            expected_body = f'{{"code":"{expected_code}","message":"{expected_code}"}}'.encode()
            if not hmac.compare_digest(body, expected_body):
                raise ValueError("non-canonical JSON error envelope")
            envelope = json.loads(body.decode("utf-8", errors="strict"))
            code = envelope.get("code")
            message = envelope.get("message")
            if set(envelope) != {"code", "message"}:
                raise ValueError("unexpected JSON error fields")
        else:
            code, message = _decode_norito_error_envelope_v1(body)
        if code != expected_code or message != expected_code:
            raise ValueError("error envelope does not match its HTTP status")
    except BootleLanternIssuanceClientErrorV1:
        raise
    except Exception:
        raise BootleLanternIssuanceClientErrorV1(
            f"{operation} returned an invalid error response"
        ) from None
    return BootleLanternIssuanceClientErrorV1(
        f"{operation} returned HTTP {status}: {expected_code}",
        status_code=status,
        code=expected_code,
        retry_after_seconds=retry_after_seconds,
    )


class BootleLanternIssuanceClientV1:
    """Exact, single-attempt client for native Bootle/Lantern issuance."""

    def __init__(
        self,
        base_url: str,
        *,
        session: Any | None = None,
        timeout: float | tuple[float, float] = 15.0,
    ) -> None:
        self._base_url = _validate_base_url(base_url)
        if isinstance(timeout, tuple):
            if len(timeout) != 2 or any(
                isinstance(value, bool) or not isinstance(value, (int, float)) or value <= 0
                for value in timeout
            ):
                raise ValueError("timeout values must be positive numbers")
        elif isinstance(timeout, bool) or not isinstance(timeout, (int, float)) or timeout <= 0:
            raise ValueError("timeout must be a positive number")
        self._timeout = timeout
        self._owns_session = session is None
        if session is None:
            session = requests.Session()
            no_retry_adapter = HTTPAdapter(max_retries=0)
            session.mount("https://", no_retry_adapter)
        post = getattr(session, "post", None)
        if not callable(post):
            if self._owns_session:
                session.close()
            raise TypeError("session must provide a callable post method")
        self._session = session

    def authorize(self, credential: BootleLanternIssuanceCredentialV1) -> bytes:
        """Request one exact 320-byte ``ILA1`` authorization exactly once."""

        return self._execute_exact(
            "Bootle/Lantern issuance authorization",
            BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
            credential,
            b"",
            BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1,
            _AUTHORIZATION_MAGIC_V1,
        )

    def issue(
        self,
        credential: BootleLanternIssuanceCredentialV1,
        canonical_request: bytes | bytearray | memoryview,
    ) -> bytes:
        """Submit exact ``ILA1 || ILQ1`` and return one exact ``ILR1``."""

        if not isinstance(canonical_request, (bytes, bytearray, memoryview)):
            raise TypeError("Bootle/Lantern issue request must be bytes-like")
        if len(canonical_request) != BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1:
            raise ValueError(
                "Bootle/Lantern issue request must be exactly "
                f"{BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1} bytes"
            )
        if not hmac.compare_digest(
            bytes(canonical_request[:4]), _AUTHORIZATION_MAGIC_V1
        ) or not hmac.compare_digest(
            bytes(
                canonical_request[
                    BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 : BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1
                    + 4
                ]
            ),
            _BLIND_REQUEST_MAGIC_V1,
        ):
            raise ValueError(
                "Bootle/Lantern issue request must contain canonical ILA1 || ILQ1 magics"
            )
        return self._execute_exact(
            "Bootle/Lantern blind issuance",
            BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
            credential,
            bytes(canonical_request),
            BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1,
            _RESPONSE_MAGIC_V1,
        )

    def _execute_exact(
        self,
        operation: str,
        path: str,
        credential: BootleLanternIssuanceCredentialV1,
        body: bytes,
        expected_bytes: int,
        expected_magic: bytes,
    ) -> bytes:
        if not isinstance(credential, BootleLanternIssuanceCredentialV1):
            raise TypeError("credential must be a BootleLanternIssuanceCredentialV1")
        authorization = credential._authorization_header_value()
        headers = {
            "Authorization": authorization,
            "Content-Type": BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
            "Accept": BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
            "Accept-Encoding": "identity",
            "Cache-Control": "no-store",
            "Pragma": "no-cache",
        }
        target = f"{self._base_url}{path}"
        try:
            response = self._session.post(
                target,
                data=body,
                headers=headers,
                timeout=self._timeout,
                allow_redirects=False,
                stream=True,
            )
        except Exception:
            raise BootleLanternIssuanceClientErrorV1(f"{operation} request failed") from None
        try:
            try:
                status = getattr(response, "status_code", None)
                if type(status) is not int:
                    raise BootleLanternIssuanceClientErrorV1(
                        f"{operation} response status is invalid"
                    )
                if status != 200:
                    raise _decode_error_response_v1(response, operation, status)
                response_url = getattr(response, "url", "")
                if response_url and response_url != target:
                    raise BootleLanternIssuanceClientErrorV1(
                        f"{operation} response URL does not match the request"
                    )
                _validate_response_headers(response, operation, expected_bytes)
                return _read_exact_response_body(
                    response,
                    operation,
                    expected_bytes,
                    expected_magic,
                )
            except BootleLanternIssuanceClientErrorV1:
                raise
            except Exception:
                raise BootleLanternIssuanceClientErrorV1(
                    f"{operation} response is invalid"
                ) from None
        finally:
            try:
                close = getattr(response, "close", None)
                if callable(close):
                    close()
            except Exception:
                pass

    def close(self) -> None:
        """Close the internally-created session, if this client owns it."""

        if self._owns_session:
            self._session.close()
            self._owns_session = False

    def __enter__(self) -> BootleLanternIssuanceClientV1:
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> None:
        self.close()


__all__ = [
    "BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1",
    "BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1",
    "BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1",
    "BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1",
    "BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1",
    "BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1",
    "BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1",
    "BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1",
    "BootleLanternIssuanceClientErrorV1",
    "BootleLanternIssuanceCredentialV1",
    "BootleLanternIssuanceClientV1",
]
