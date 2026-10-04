"""Exact request/recovery API for the independent app-certificate issuer.

No production evidence provider is installed by default. A deployment must
authenticate release governance and supply a provider for raw evidence,
Apple receipt/distribution state or Android revocation before signing can be
configured. Later Apple assertions need their own durable counter gate. With
no provider or signer this WSGI endpoint is closed.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
from dataclasses import dataclass
from typing import Callable, Protocol

from .attestation import AttestationRejected, Selection, decode_android_chain, require
from .issuance import DurableCertificateStore, GovernedIssuanceScope
from .revocation import RevocationUnavailable


PATH = "/v1/kagemusha/app-certificates"
MAX_BODY_BYTES = 192 * 1024
REQUIRED_FIELDS = {"operation", "account_canonical", "signed_preparation_base64",
                   "selection", "platform", "platform_evidence_base64"}
SELECTION_FIELDS = {"client_nonce_hex", "server_nonce_hex", "release_id_hex",
                    "hardware_profile_id_hex", "attested_key_id_hex", "lane_id_hex"}


@dataclass(frozen=True)
class CertificateRequest:
    """Untrusted, strictly decoded request; release values are reselected by provider."""

    operation: str
    account_canonical: str
    signed_preparation: bytes
    selection: Selection
    platform: str
    platform_evidence: bytes


class EvidenceProvider(Protocol):
    """Deployment-owned verification boundary; no provider is installed by default."""

    def prepare(self, request: CertificateRequest, *, issue: bool) -> GovernedIssuanceScope:
        """Return authenticated governed scope only after all applicable gates pass."""


def _unique_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        require(key not in result, "duplicate app-certificate request key")
        result[key] = value
    return result


def _decode_base64(value: object, name: str, maximum: int) -> bytes:
    require(isinstance(value, str) and len(value) <= ((maximum + 2) // 3) * 4,
            f"invalid {name}")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError) as error:
        raise AttestationRejected(f"invalid {name}") from error
    require(0 < len(decoded) <= maximum
            and base64.b64encode(decoded).decode("ascii") == value,
            f"invalid {name}")
    return decoded


def _decode_hex32(value: object, name: str) -> bytes:
    require(isinstance(value, str) and len(value) == 64
            and all(character in "0123456789abcdef" for character in value),
            f"invalid {name}")
    return bytes.fromhex(value)


def decode_request(body: bytes) -> CertificateRequest:
    """Reject unknown/duplicate fields and noncanonical byte encodings."""
    require(0 < len(body) <= MAX_BODY_BYTES, "app-certificate request outside bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object)
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError) as error:
        raise AttestationRejected("invalid app-certificate JSON") from error
    require(isinstance(value, dict) and set(value) == REQUIRED_FIELDS,
            "invalid app-certificate request fields")
    operation = value["operation"]
    account = value["account_canonical"]
    platform = value["platform"]
    selected = value["selection"]
    require(isinstance(account, str), "invalid app-certificate account")
    try:
        account_bytes = account.encode("utf-8")
    except UnicodeEncodeError as error:
        raise AttestationRejected("invalid app-certificate account") from error
    require(operation in ("issue", "recover")
            and 0 < len(account_bytes) <= 512
            and platform in ("apple_app_attest", "android_keymint")
            and isinstance(selected, dict) and set(selected) == SELECTION_FIELDS,
            "invalid app-certificate request scope")
    selection = Selection(*(_decode_hex32(selected[name], name) for name in (
        "client_nonce_hex", "server_nonce_hex", "release_id_hex",
        "hardware_profile_id_hex", "attested_key_id_hex", "lane_id_hex",
    )))
    selection.transcript()
    token = _decode_base64(value["signed_preparation_base64"], "signed preparation", 273)
    require(len(token) == 273 and token[0] == 1, "invalid signed preparation frame")
    evidence = _decode_base64(value["platform_evidence_base64"], "platform evidence",
                              64 * 1024 if platform == "apple_app_attest" else 128 * 1024)
    if platform == "android_keymint":
        decode_android_chain(evidence)
    return CertificateRequest(operation, account, token, selection, platform, evidence)


class IssuerService:
    """Callable handler with a closed default configuration."""

    def __init__(
        self, store: DurableCertificateStore | None = None,
        provider: EvidenceProvider | None = None,
        signer: Callable[[bytes], bytes] | None = None,
        caller_authorizer: Callable[[dict], str | None] | None = None,
        expected_caller_identity: str | None = None,
    ) -> None:
        self.store = store
        self.provider = provider
        self.signer = signer
        self.caller_authorizer = caller_authorizer
        self.expected_caller_identity = expected_caller_identity

    def handle(self, method: str, path: str, body: bytes, content_type: str,
               *, caller_identity: str | None = None) -> tuple[int, bytes]:
        """Return an exact JSON response without leaking verifier internals."""
        if method != "POST" or path != PATH:
            return 404, b'{"error":"route_not_found"}'
        if content_type != "application/json":
            return 415, b'{"error":"unsupported_media_type"}'
        try:
            request = decode_request(body)
        except AttestationRejected:
            return 400, b'{"error":"invalid_request"}'
        if (type(self.expected_caller_identity) is not str
                or not self.expected_caller_identity
                or self.expected_caller_identity.strip() != self.expected_caller_identity
                or type(caller_identity) is not str
                or caller_identity != self.expected_caller_identity):
            return 503, b'{"error":"issuer_unavailable"}'
        if self.store is None or self.provider is None or (request.operation == "issue" and self.signer is None):
            return 503, b'{"error":"issuer_unavailable"}'
        try:
            scope = self.provider.prepare(request, issue=request.operation == "issue")
            require(scope.selection == request.selection
                    and scope.account_canonical == request.account_canonical
                    and scope.platform == request.platform,
                    "provider returned a different request scope")
            if request.operation == "issue":
                assert self.signer is not None
                certificate = self.store.issue_once(
                    scope, request.signed_preparation, request.platform_evidence, self.signer,
                    lambda: self.provider.prepare(request, issue=True),
                )
            else:
                certificate = self.store.recover(
                    scope, request.signed_preparation, request.platform_evidence,
                )
        except RevocationUnavailable:
            # Unknown revocation status is not a rejected certificate; retry.
            return 503, b'{"error":"issuer_unavailable"}'
        except AttestationRejected:
            return 409, b'{"error":"certificate_rejected"}'
        except Exception:
            # Provider, signer and storage errors are operational faults; never
            # substitute a software-only certificate or expose secret details.
            return 503, b'{"error":"issuer_unavailable"}'
        response = {
            "certificate_base64": base64.b64encode(certificate).decode("ascii"),
            "certificate_sha256_hex": hashlib.sha256(certificate).hexdigest(),
        }
        return 200, json.dumps(response, separators=(",", ":"), sort_keys=True).encode("ascii")

    def wsgi(self, environment: dict, start_response: Callable) -> list[bytes]:
        """WSGI adapter requiring deployment-owned authenticated caller identity.

        The callback must derive identity from a trusted mTLS/API boundary,
        never from a client-supplied header. TLS and rate limits belong to that
        boundary as well.
        """
        try:
            length = int(environment.get("CONTENT_LENGTH", "0"))
        except ValueError:
            length = -1
        if (self.caller_authorizer is None
                or type(self.expected_caller_identity) is not str
                or not self.expected_caller_identity
                or self.expected_caller_identity.strip() != self.expected_caller_identity):
            code, body = 503, b'{"error":"issuer_unavailable"}'
        else:
            try:
                identity = self.caller_authorizer(environment)
            except Exception:
                code, body = 503, b'{"error":"issuer_unavailable"}'
            else:
                if type(identity) is not str or identity != self.expected_caller_identity:
                    code, body = 401, b'{"error":"caller_unauthorized"}'
                elif length < 0 or length > MAX_BODY_BYTES:
                    code, body = 400, b'{"error":"invalid_request"}'
                else:
                    code, body = self.handle(
                        environment.get("REQUEST_METHOD", ""),
                        environment.get("PATH_INFO", ""),
                        environment["wsgi.input"].read(length),
                        environment.get("CONTENT_TYPE", ""),
                        caller_identity=identity,
                    )
        labels = {200: "OK", 400: "Bad Request", 401: "Unauthorized", 404: "Not Found",
                  409: "Conflict", 415: "Unsupported Media Type", 503: "Service Unavailable"}
        start_response(f"{code} {labels[code]}", [
            ("Content-Type", "application/json"),
            ("Content-Length", str(len(body))),
            ("Cache-Control", "no-store"),
        ])
        return [body]
