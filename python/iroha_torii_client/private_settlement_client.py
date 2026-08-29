"""Exact-route Python client surface for atomic private settlement V1.

The native Rust wallet/coordinator owns witnesses and audit plaintext.  This
module accepts only bounded, already-prepared Norito JSON objects, authenticates
the exact Torii route, and returns an opaque bounded response for native
decoding.  Secret-bearing bodies are never rendered in exceptions or reprs.
"""

from __future__ import annotations

import json
import re
from enum import Enum
from typing import Any, Callable, Optional, Sequence, Union

from .canonical_transport import OperatorRequestHeaderPlan

__all__ = [
    "AtomicPrivateSettlementAuthV1",
    "AtomicPrivateSettlementIdentifierV1",
    "AtomicPrivateSettlementJsonResponseV1",
    "AtomicPrivateSettlementOperationV1",
    "AtomicPrivateSettlementPreparedRequestV1",
    "AtomicPrivateSettlementToriiErrorV1",
]

_JSON_MEDIA_TYPE = "application/json"
_RESPONSE_SMALL_MAX_BYTES = 1024 * 1024
_RESPONSE_PUBLIC_BUNDLE_MAX_BYTES = 8 * 1024 * 1024
_RESPONSE_RESTRICTED_MAX_BYTES = 32 * 1024 * 1024
_JSON_MAX_DEPTH = 128
_JSON_MAX_NODES = 1_000_000
_U256_MAX = (1 << 256) - 1


class AtomicPrivateSettlementToriiErrorV1(RuntimeError):
    """Redacted settlement transport or response-validation failure."""


class AtomicPrivateSettlementAuthV1(Enum):
    """Authentication class required by a settlement operation."""

    SPONSOR = "sponsor"
    ROLE_IDENTITY = "role_identity"
    PUBLIC = "public"


class AtomicPrivateSettlementOperationV1(Enum):
    """Closed catalog of atomic-private-settlement mutation routes."""

    AVAILABILITY_SHARE = (
        "/v1/nexus/private-settlements/legs/availability-shares",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"material"}),
        32 * 1024 * 1024,
    )
    PREPARE_VOTE = (
        "/v1/nexus/private-settlements/phases/prepare-votes",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"manifest", "payload_digest"}),
        8 * 1024 * 1024,
    )
    COMMIT_VOTE = (
        "/v1/nexus/private-settlements/phases/commit-votes",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"payload_digest", "barrier"}),
        8 * 1024 * 1024,
    )
    PHASE_CERTIFICATE = (
        "/v1/nexus/private-settlements/phases/certificates",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"manifest", "payload_digest", "certificate"}),
        8 * 1024 * 1024,
    )
    LEG_UPLOAD = (
        "/v1/nexus/private-settlements/legs",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"manifest", "audit_policy", "committee_authority", "payload"}),
        32 * 1024 * 1024,
    )
    AUDIT_APPROVAL = (
        "/v1/nexus/private-settlements/legs/{payload_digest}/audit-approvals",
        AtomicPrivateSettlementAuthV1.ROLE_IDENTITY,
        frozenset({"approval"}),
        2 * 1024 * 1024,
    )
    BUNDLE_SUBMIT = (
        "/v1/nexus/private-settlements/bundles",
        AtomicPrivateSettlementAuthV1.SPONSOR,
        frozenset({"transaction"}),
        8 * 1024 * 1024,
    )

    def __init__(
        self,
        path: str,
        auth: AtomicPrivateSettlementAuthV1,
        top_level_fields: frozenset[str],
        maximum_request_bytes: int,
    ) -> None:
        self.path = path
        self.auth = auth
        self.top_level_fields = top_level_fields
        self.maximum_request_bytes = maximum_request_bytes


def _crc16_ccitt_false(value: bytes) -> int:
    crc = 0xFFFF
    for byte in value:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


class AtomicPrivateSettlementIdentifierV1:
    """Immutable exact marked Iroha hash used in settlement paths."""

    _RAW_HEX = re.compile(r"[0-9A-Fa-f]{64}")
    _LITERAL = re.compile(r"hash:([0-9A-F]{64})#([0-9A-F]{4})")

    def __init__(self, value: str) -> None:
        if not isinstance(value, str):
            raise TypeError("settlement identifier must be a string")
        if not value or value != value.strip():
            raise ValueError("settlement identifier must be exact and non-empty")
        if self._RAW_HEX.fullmatch(value):
            body = value.upper()
            if int(body[-2:], 16) & 1 == 0:
                raise ValueError("settlement identifier must set the Iroha hash marker bit")
            checksum = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
            literal = f"hash:{body}#{checksum:04X}"
        else:
            match = self._LITERAL.fullmatch(value)
            if match is None:
                raise ValueError(
                    "settlement identifier must be raw hex or a canonical hash literal"
                )
            body, checksum_hex = match.groups()
            expected = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
            if int(checksum_hex, 16) != expected or int(body[-2:], 16) & 1 == 0:
                raise ValueError("settlement identifier checksum or marker is invalid")
            literal = value
        self._path_component = body.lower()
        self._json_literal = literal

    @property
    def path_component(self) -> str:
        """Return the exact lowercase raw-hex Torii path component."""

        return self._path_component

    @property
    def json_literal(self) -> str:
        """Return the canonical Norito JSON hash literal."""

        return self._json_literal

    def __eq__(self, other: object) -> bool:
        return (
            isinstance(other, AtomicPrivateSettlementIdentifierV1)
            and self._path_component == other._path_component
        )

    def __hash__(self) -> int:
        return hash(self._path_component)

    def __str__(self) -> str:
        return self._path_component

    def __repr__(self) -> str:
        return f"AtomicPrivateSettlementIdentifierV1({self._path_component!r})"


def _reject_json_float(_value: str) -> Any:
    raise ValueError("atomic private settlement JSON forbids floating-point numbers")


def _parse_json_integer(value: str) -> int:
    parsed = int(value, 10)
    if abs(parsed) > _U256_MAX:
        raise ValueError("atomic private settlement JSON contains an oversized integer")
    return parsed


def _reject_json_constant(_value: str) -> Any:
    raise ValueError("atomic private settlement JSON contains a non-finite number")


def _unique_object(pairs: Sequence[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("atomic private settlement JSON contains a duplicate object field")
        result[key] = value
    return result


def _parse_exact_json_object(data: bytes, label: str) -> dict[str, Any]:
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise ValueError(f"{label} must be exact UTF-8") from error
    try:
        parsed = json.loads(
            text,
            object_pairs_hook=_unique_object,
            parse_int=_parse_json_integer,
            parse_float=_reject_json_float,
            parse_constant=_reject_json_constant,
        )
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        raise ValueError(f"{label} must be one strict JSON object") from error
    if not isinstance(parsed, dict):
        raise ValueError(f"{label} must be one strict JSON object")

    nodes = 0
    stack: list[tuple[Any, int]] = [(parsed, 1)]
    while stack:
        value, depth = stack.pop()
        nodes += 1
        if nodes > _JSON_MAX_NODES or depth > _JSON_MAX_DEPTH:
            raise ValueError(f"{label} exceeds the JSON structure bounds")
        if isinstance(value, dict):
            stack.extend((child, depth + 1) for child in value.values())
        elif isinstance(value, list):
            stack.extend((child, depth + 1) for child in value)
    return parsed


class AtomicPrivateSettlementPreparedRequestV1:
    """Bounded operation-tagged JSON produced by the native coordinator."""

    def __init__(
        self,
        operation: AtomicPrivateSettlementOperationV1,
        body: bytes,
    ) -> None:
        self.operation = operation
        self._body = bytearray(body)
        self._closed = False

    @classmethod
    def from_native_prepared_json(
        cls,
        operation: AtomicPrivateSettlementOperationV1,
        body: Union[bytes, bytearray, memoryview],
    ) -> "AtomicPrivateSettlementPreparedRequestV1":
        """Validate and retain one exact native-prepared request body."""

        if not isinstance(operation, AtomicPrivateSettlementOperationV1):
            raise TypeError("operation must be AtomicPrivateSettlementOperationV1")
        if not isinstance(body, (bytes, bytearray, memoryview)):
            raise TypeError("prepared settlement request must be bytes-like")
        exact = bytes(body)
        if not exact or len(exact) > operation.maximum_request_bytes:
            raise ValueError("prepared settlement request is empty or exceeds its route bound")
        parsed = _parse_exact_json_object(exact, "prepared settlement request")
        if frozenset(parsed) != operation.top_level_fields:
            raise ValueError("prepared settlement request has the wrong top-level fields")
        return cls(operation, exact)

    def bytes(self) -> bytes:
        """Return a defensive copy for one exact transport operation."""

        if self._closed:
            raise RuntimeError("prepared settlement request is closed")
        return bytes(self._body)

    def close(self) -> None:
        """Erase the retained SDK copy."""

        if not self._closed:
            self._body[:] = b"\0" * len(self._body)
            self._closed = True

    def __enter__(self) -> "AtomicPrivateSettlementPreparedRequestV1":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def __repr__(self) -> str:
        return (
            "AtomicPrivateSettlementPreparedRequestV1("
            f"operation={self.operation.name}, body=[REDACTED])"
        )


class AtomicPrivateSettlementJsonResponseV1:
    """Opaque bounded exact response suitable for native decoding."""

    def __init__(self, route: str, body: bytes) -> None:
        self.route = route
        self._body = bytearray(body)
        self._closed = False

    def bytes(self) -> bytes:
        """Return a defensive response copy."""

        if self._closed:
            raise RuntimeError("settlement response is closed")
        return bytes(self._body)

    def close(self) -> None:
        """Erase the retained SDK copy."""

        if not self._closed:
            self._body[:] = b"\0" * len(self._body)
            self._closed = True

    def __enter__(self) -> "AtomicPrivateSettlementJsonResponseV1":
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def __repr__(self) -> str:
        return f"AtomicPrivateSettlementJsonResponseV1(route={self.route!r}, body=[REDACTED])"


_RESPONSE_FIELDS: tuple[tuple[str, frozenset[str]], ...] = (
    (
        "/availability-shares",
        frozenset({"bundle_id", "payload_digest", "leg_ordinal", "disposition", "share"}),
    ),
    (
        "/prepare-votes",
        frozenset({"bundle_id", "payload_digest", "leg_ordinal", "vote"}),
    ),
    (
        "/commit-votes",
        frozenset({"bundle_id", "payload_digest", "leg_ordinal", "vote"}),
    ),
    (
        "/certificates",
        frozenset({"bundle_id", "payload_digest", "leg_ordinal", "phase", "lifecycle"}),
    ),
    (
        "/audit-approvals",
        frozenset(
            {
                "bundle_id",
                "payload_digest",
                "leg_ordinal",
                "collected",
                "required",
                "newly_recorded",
                "lifecycle",
            }
        ),
    ),
    (
        "/status",
        frozenset(
            {
                "bundle_id",
                "payload_digest",
                "leg_ordinal",
                "route",
                "stored_at_height",
                "lifecycle_height",
                "expiry_height",
                "lifecycle",
            }
        ),
    ),
    (
        "/committee-proof",
        frozenset(
            {
                "manifest",
                "audit_policy",
                "committee_authority",
                "statement",
                "proof",
                "delta",
                "audit_approvals",
                "audit_capsule_digest",
                "availability",
                "lifecycle",
            }
        ),
    ),
    (
        "/audit-capsule",
        frozenset(
            {
                "manifest",
                "audit_policy",
                "committee_authority",
                "statement",
                "delta",
                "audit_capsule",
                "availability",
                "lifecycle",
            }
        ),
    ),
    ("/receipt", frozenset({"status", "value"})),
)


def _response_fields(route: str) -> frozenset[str]:
    for suffix, fields in _RESPONSE_FIELDS:
        if route.endswith(suffix):
            return fields
    if route.endswith("/legs"):
        return frozenset(
            {"bundle_id", "payload_digest", "leg_ordinal", "disposition", "lifecycle"}
        )
    if route.endswith("/bundles"):
        return frozenset({"bundle_id", "accepted_at_height", "carrier_id", "lifecycle"})
    if "/bundles/" in route:
        return frozenset({"manifest", "lifecycle", "finalized_height"})
    raise ValueError("unknown atomic private settlement route")


def _read_bounded_response(response: Any, maximum_bytes: int) -> bytes:
    try:
        content_length = response.headers.get("Content-Length")
        if content_length is not None:
            if re.fullmatch(r"(?:0|[1-9][0-9]*)", content_length) is None:
                raise ValueError("settlement response Content-Length is not canonical")
            if int(content_length, 10) > maximum_bytes:
                raise ValueError("settlement response exceeds its byte bound")
        body = bytearray()
        for chunk in response.iter_content(chunk_size=8192, decode_unicode=False):
            if not chunk:
                continue
            if not isinstance(chunk, (bytes, bytearray)):
                raise TypeError("settlement response yielded a non-byte chunk")
            if len(chunk) > maximum_bytes - len(body):
                raise ValueError("settlement response exceeds its byte bound")
            body.extend(chunk)
        if not body:
            raise ValueError("settlement response must not be empty")
        return bytes(body)
    finally:
        response.close()


def create_atomic_private_settlement_client_mixin(
    *,
    canonical_auth_type: type[Any],
    operator_context_type: type[Any],
) -> type[Any]:
    """Build a mixin after the client's concrete signing types are defined."""

    class AtomicPrivateSettlementClientMixin:
        _base_url: str
        _canonical_request_headers: Callable[..., Any]
        _request: Callable[..., Any]
        _require_canonical_auth: Callable[..., Any]
        _require_exact_i105_account_id: Callable[..., str]

        @staticmethod
        def _private_settlement_request_headers(has_body: bool) -> dict[str, str]:
            headers = {
                "Accept": _JSON_MEDIA_TYPE,
                "Accept-Encoding": "identity",
                "Cache-Control": "no-store",
                "Pragma": "no-cache",
            }
            if has_body:
                headers["Content-Type"] = _JSON_MEDIA_TYPE
            return headers

        def _private_settlement_sponsor_request(
            self,
            method: str,
            path: str,
            body: bytes,
            canonical_auth: Any,
        ) -> Any:
            auth = self._require_canonical_auth(canonical_auth, "atomic private settlement")
            if not isinstance(auth, canonical_auth_type):
                raise TypeError("settlement sponsor auth has the wrong type")
            self._require_exact_i105_account_id(
                auth.account_id,
                "atomic private settlement sponsor account",
            )
            headers = self._canonical_request_headers(
                method,
                path,
                body,
                canonical_auth=auth,
                headers=self._private_settlement_request_headers(bool(body)),
                has_body=bool(body),
            )
            return self._request(
                method,
                path,
                headers=headers,
                data=body or None,
                stream=True,
                allow_retry=False,
                allow_redirects=False,
            )

        def _private_settlement_role_request(
            self,
            method: str,
            path: str,
            body: bytes,
            signing_context: Any,
        ) -> Any:
            if not isinstance(signing_context, operator_context_type):
                raise TypeError("settlement role identity has the wrong signing-context type")
            headers = OperatorRequestHeaderPlan(
                self._private_settlement_request_headers(bool(body)),
                signing_context,
            )
            return self._request(
                method,
                path,
                headers=headers,
                data=body or None,
                stream=True,
                allow_retry=False,
                allow_redirects=False,
            )

        def _private_settlement_public_request(self, path: str) -> Any:
            return self._request(
                "GET",
                path,
                headers=self._private_settlement_request_headers(False),
                stream=True,
                allow_retry=False,
                allow_redirects=False,
            )

        def _private_settlement_validate_response(
            self,
            response: Any,
            route: str,
            maximum_bytes: int,
            expected_identifier: Optional[AtomicPrivateSettlementIdentifierV1] = None,
            expected_identifier_field: Optional[str] = None,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            expected_url = f"{self._base_url}{route}"
            actual_url = getattr(response, "url", None)
            history = getattr(response, "history", ())
            if actual_url != expected_url or history:
                response.close()
                raise AtomicPrivateSettlementToriiErrorV1(
                    "atomic private settlement response provenance is invalid"
                )
            if response.status_code < 200 or response.status_code > 299:
                reject_code = response.headers.get("X-Iroha-Reject-Code")
                response.close()
                suffix = (
                    f"; reject_code={reject_code}"
                    if isinstance(reject_code, str)
                    and re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", reject_code)
                    else ""
                )
                raise AtomicPrivateSettlementToriiErrorV1(
                    "atomic private settlement request failed with HTTP "
                    f"{response.status_code}{suffix}"
                )
            if response.headers.get("Content-Type") != _JSON_MEDIA_TYPE:
                response.close()
                raise AtomicPrivateSettlementToriiErrorV1(
                    "atomic private settlement response content type is invalid"
                )
            content_encoding = response.headers.get("Content-Encoding")
            if content_encoding not in (None, "identity"):
                response.close()
                raise AtomicPrivateSettlementToriiErrorV1(
                    "atomic private settlement response encoding is invalid"
                )
            try:
                body = _read_bounded_response(response, maximum_bytes)
                parsed = _parse_exact_json_object(body, "atomic private settlement response")
                if frozenset(parsed) != _response_fields(route):
                    raise ValueError("settlement response has unexpected public fields")
                if expected_identifier is not None and expected_identifier_field is not None:
                    if parsed.get(expected_identifier_field) != expected_identifier.json_literal:
                        raise ValueError("settlement response identifier is substituted")
                if route.endswith("/receipt"):
                    if expected_identifier is None:
                        raise ValueError("settlement receipt request identity is missing")
                    if parsed.get("status") not in {"pending", "finalized", "aborted"}:
                        raise ValueError("settlement receipt has an unknown tag")
                    value = parsed.get("value")
                    if not isinstance(value, dict) or value.get(
                        "bundle_id"
                    ) != expected_identifier.json_literal:
                        raise ValueError("settlement receipt identifier is substituted")
                elif "/bundles/" in route:
                    if expected_identifier is None:
                        raise ValueError("settlement bundle request identity is missing")
                    manifest = parsed.get("manifest")
                    if manifest is not None and (
                        not isinstance(manifest, dict)
                        or manifest.get("bundle_id") != expected_identifier.json_literal
                    ):
                        raise ValueError("settlement bundle status is substituted")
                canonical = json.dumps(
                    parsed,
                    ensure_ascii=False,
                    separators=(",", ":"),
                ).encode("utf-8")
                return AtomicPrivateSettlementJsonResponseV1(route, canonical)
            except AtomicPrivateSettlementToriiErrorV1:
                raise
            except Exception as error:
                raise AtomicPrivateSettlementToriiErrorV1(
                    "atomic private settlement response is invalid"
                ) from error

        def _private_settlement_sponsor_mutation(
            self,
            request: AtomicPrivateSettlementPreparedRequestV1,
            expected_operation: AtomicPrivateSettlementOperationV1,
            canonical_auth: Any,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            if request.operation is not expected_operation:
                raise ValueError("prepared settlement request is bound to another operation")
            body = request.bytes()
            response = self._private_settlement_sponsor_request(
                "POST", expected_operation.path, body, canonical_auth
            )
            return self._private_settlement_validate_response(
                response,
                expected_operation.path,
                _RESPONSE_RESTRICTED_MAX_BYTES,
            )

        def request_private_settlement_availability_share_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Persist provisional material and obtain one validator share."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.AVAILABILITY_SHARE, canonical_auth
            )

        def request_private_settlement_prepare_vote_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Request one independently verified and durably staged Prepare vote."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.PREPARE_VOTE, canonical_auth
            )

        def request_private_settlement_commit_vote_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Request one Commit vote bound to the complete Prepare barrier."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.COMMIT_VOTE, canonical_auth
            )

        def persist_private_settlement_phase_certificate_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Persist one exact Prepare or Commit certificate on a participant."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.PHASE_CERTIFICATE, canonical_auth
            )

        def upload_private_settlement_leg_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Promote one availability-certified encrypted leg."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.LEG_UPLOAD, canonical_auth
            )

        def submit_private_settlement_bundle_v1(
            self, request: AtomicPrivateSettlementPreparedRequestV1, *, canonical_auth: Any
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Submit one exact sponsor-signed global finalization carrier."""

            return self._private_settlement_sponsor_mutation(
                request, AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT, canonical_auth
            )

        def submit_private_settlement_audit_approval_v1(
            self,
            payload_digest: Union[str, AtomicPrivateSettlementIdentifierV1],
            request: AtomicPrivateSettlementPreparedRequestV1,
            *,
            auditor_signing_context: Any,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Submit one purpose-separated approval as an exact auditor identity."""

            if request.operation is not AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL:
                raise ValueError("prepared settlement request is bound to another operation")
            identifier = (
                payload_digest
                if isinstance(payload_digest, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(payload_digest)
            )
            path = request.operation.path.replace(
                "{payload_digest}", identifier.path_component
            )
            response = self._private_settlement_role_request(
                "POST", path, request.bytes(), auditor_signing_context
            )
            return self._private_settlement_validate_response(
                response,
                path,
                _RESPONSE_RESTRICTED_MAX_BYTES,
                identifier,
                "payload_digest",
            )

        def private_settlement_leg_status_v1(
            self,
            payload_digest: Union[str, AtomicPrivateSettlementIdentifierV1],
            *,
            canonical_auth: Any,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Read one sponsor-authenticated redacted leg status."""

            identifier = (
                payload_digest
                if isinstance(payload_digest, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(payload_digest)
            )
            path = f"/v1/nexus/private-settlements/legs/{identifier.path_component}/status"
            response = self._private_settlement_sponsor_request(
                "GET", path, b"", canonical_auth
            )
            return self._private_settlement_validate_response(
                response, path, _RESPONSE_SMALL_MAX_BYTES, identifier, "payload_digest"
            )

        def private_settlement_committee_proof_v1(
            self,
            payload_digest: Union[str, AtomicPrivateSettlementIdentifierV1],
            *,
            validator_signing_context: Any,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Fetch proof and opaque delta material as one committee validator."""

            identifier = (
                payload_digest
                if isinstance(payload_digest, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(payload_digest)
            )
            path = (
                "/v1/nexus/private-settlements/legs/"
                f"{identifier.path_component}/committee-proof"
            )
            response = self._private_settlement_role_request(
                "GET", path, b"", validator_signing_context
            )
            return self._private_settlement_validate_response(
                response, path, _RESPONSE_RESTRICTED_MAX_BYTES
            )

        def private_settlement_auditor_capsule_v1(
            self,
            payload_digest: Union[str, AtomicPrivateSettlementIdentifierV1],
            *,
            auditor_signing_context: Any,
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Fetch one padded encrypted capsule as a governed auditor."""

            identifier = (
                payload_digest
                if isinstance(payload_digest, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(payload_digest)
            )
            path = f"/v1/nexus/private-settlements/legs/{identifier.path_component}/audit-capsule"
            response = self._private_settlement_role_request(
                "GET", path, b"", auditor_signing_context
            )
            return self._private_settlement_validate_response(
                response, path, _RESPONSE_RESTRICTED_MAX_BYTES
            )

        def private_settlement_bundle_status_v1(
            self, bundle_id: Union[str, AtomicPrivateSettlementIdentifierV1]
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Read the public allowlisted lifecycle for one bundle."""

            identifier = (
                bundle_id
                if isinstance(bundle_id, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(bundle_id)
            )
            path = f"/v1/nexus/private-settlements/bundles/{identifier.path_component}"
            response = self._private_settlement_public_request(path)
            return self._private_settlement_validate_response(
                response, path, _RESPONSE_PUBLIC_BUNDLE_MAX_BYTES, identifier
            )

        def private_settlement_bundle_receipt_v1(
            self, bundle_id: Union[str, AtomicPrivateSettlementIdentifierV1]
        ) -> AtomicPrivateSettlementJsonResponseV1:
            """Read the public terminal receipt or pending marker."""

            identifier = (
                bundle_id
                if isinstance(bundle_id, AtomicPrivateSettlementIdentifierV1)
                else AtomicPrivateSettlementIdentifierV1(bundle_id)
            )
            path = f"/v1/nexus/private-settlements/bundles/{identifier.path_component}/receipt"
            response = self._private_settlement_public_request(path)
            return self._private_settlement_validate_response(
                response, path, _RESPONSE_RESTRICTED_MAX_BYTES, identifier
            )

    AtomicPrivateSettlementClientMixin.__name__ = "AtomicPrivateSettlementClientMixin"
    return AtomicPrivateSettlementClientMixin
