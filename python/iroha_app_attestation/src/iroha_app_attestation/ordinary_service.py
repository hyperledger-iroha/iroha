"""Closed first-release ordinary credential DTO and deployment-owned handler.

The authentic Core facade uses this exact envelope; release/root/policy and
decoded Google verdict fields are forbidden. Production caller admission comes
from authenticated deployment startup, not a supplied HTTP identity string.
"""
from __future__ import annotations

import base64
import hashlib
import json
from typing import Callable

from .attestation import AttestationRejected, require
from .ordinary_issuance import DurableOrdinaryCredentialIssuer
from .ordinary_provider import OrdinaryCredentialRequest, OrdinaryRawAttestationRequest
from .service import _decode_base64, _decode_hex32, _unique_object
from .ordinary_refresh_service import PATH as REFRESH_PATH, handle_refresh
from .ordinary_refresh_issuance import DurableOrdinaryIntegrityRefreshIssuer

SCHEMA = "iroha.kagemusha.ordinary-app-credential-request.v1"
PATH = "/v1/kagemusha/ordinary-app-credentials"
RAW_SCHEMA = "iroha.kagemusha.ordinary-app-raw-admission-request.v1"
RAW_PATH = "/v1/kagemusha/ordinary-app-raw-attestations"
RAW_FIELDS = {"schema", "operation", "operation_id", "signed_preparation_base64",
              "attested_public_key_sec1_base64", "raw_attestation_base64"}
MAX_BODY_BYTES = 256 * 1024
FIELDS = {"schema", "operation", "operation_id", "signed_preparation_base64",
          "attested_public_key_sec1_base64", "raw_attestation_base64", "app_possession",
          "play_integrity_token"}


def decode_request(body: bytes) -> OrdinaryCredentialRequest:
    require(type(body) is bytes and 0 < len(body) <= MAX_BODY_BYTES,
            "ordinary credential request outside bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError, RecursionError):
        raise AttestationRejected("invalid ordinary credential JSON") from None
    require(type(value) is dict and set(value) == FIELDS and value["schema"] == SCHEMA,
            "invalid ordinary credential fields")
    possession = value["app_possession"]
    require(type(possession) is dict, "invalid ordinary possession envelope")
    if possession.get("platform") == "android_keystore":
        require(set(possession) == {"platform", "signature_der_base64"}, "invalid Android possession fields")
        original = _decode_base64(possession["signature_der_base64"], "Android possession", 72)
    else:
        require(set(possession) == {"platform", "raw_assertion_base64"}
                and possession["platform"] == "apple_app_attest", "invalid Apple possession fields")
        original = _decode_base64(possession["raw_assertion_base64"], "Apple possession", 4096)
    request = OrdinaryCredentialRequest(value["operation"], _decode_hex32(value["operation_id"], "ordinary operation ID"),
        _decode_base64(value["signed_preparation_base64"], "ordinary signed preparation", 515),
        _decode_base64(value["attested_public_key_sec1_base64"], "ordinary app key", 65),
        _decode_base64(value["raw_attestation_base64"], "ordinary raw attestation", 128*1024),
        original, possession["platform"], value["play_integrity_token"])
    request.validate()
    return request


def decode_raw_request(body: bytes) -> OrdinaryRawAttestationRequest:
    """Decode the one raw-only envelope; policy/verdict/possession fields denied."""
    require(type(body) is bytes and 0 < len(body) <= MAX_BODY_BYTES,
            "raw admission request outside bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique_object,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (ValueError, UnicodeError, RecursionError):
        raise AttestationRejected("invalid raw admission JSON") from None
    require(type(value) is dict and set(value) == RAW_FIELDS and value["schema"] == RAW_SCHEMA,
            "invalid raw admission fields")
    request = OrdinaryRawAttestationRequest(value["operation"],
        _decode_hex32(value["operation_id"], "raw operation ID"),
        _decode_base64(value["signed_preparation_base64"], "raw signed C preparation", 515),
        _decode_base64(value["attested_public_key_sec1_base64"], "raw attested key", 65),
        _decode_base64(value["raw_attestation_base64"], "raw attestation", 128*1024))
    request.validate()
    return request


class OrdinaryCredentialService:
    """Handler connected only through an authenticated deployment caller owner.

    ``authorize_core_call`` consumes server-owned transport context (for example
    a TLS peer checked by the actual serving owner). It must not inspect a
    mobile/header-supplied identity as authority. An absent owner closes the
    route. The returned public credential is authenticated again by Native Core.
    """
    def __init__(self, *, issuer: DurableOrdinaryCredentialIssuer | None = None,
                 refresh_issuer: DurableOrdinaryIntegrityRefreshIssuer | None = None,
                 authorize_core_call: Callable[[object], bool] | None = None) -> None:
        require(issuer is None or type(issuer) is DurableOrdinaryCredentialIssuer,
                "actual ordinary issuer owner required")
        require(authorize_core_call is None or callable(authorize_core_call),
                "ordinary Core caller owner invalid")
        self._issuer = issuer
        require(refresh_issuer is None or type(refresh_issuer) is DurableOrdinaryIntegrityRefreshIssuer,
                "actual refresh issuer owner required")
        self._refresh = refresh_issuer
        self._authorize = authorize_core_call

    def handle(self, *, method: str, path: str, body: bytes,
               content_type: str, transport_context: object) -> tuple[int, bytes]:
        if method != "POST" or path not in (PATH, RAW_PATH, REFRESH_PATH):
            return 404, b'{"error":"route_not_found"}'
        if content_type != "application/json":
            return 415, b'{"error":"unsupported_media_type"}'
        if self._issuer is None or self._authorize is None:
            return 503, b'{"error":"issuer_unavailable"}'
        try:
            if self._authorize(transport_context) is not True:
                return 401, b'{"error":"caller_unauthorized"}'
        except Exception:
            return 503, b'{"error":"issuer_unavailable"}'
        if path == REFRESH_PATH:
            return handle_refresh(self._refresh,body)
        try:
            request = decode_raw_request(body) if path == RAW_PATH else decode_request(body)
        except AttestationRejected:
            return 400, b'{"error":"invalid_request"}'
        try:
            certificate = (self._issuer.accept_raw(request) if path == RAW_PATH
                           else self._issuer.issue(request))
        except AttestationRejected:
            return 409, b'{"error":"credential_rejected"}'
        except Exception:
            return 503, b'{"error":"issuer_unavailable"}'
        value = ({"raw_admission_base64": base64.b64encode(certificate).decode("ascii"),
                  "raw_admission_sha256_hex": hashlib.sha256(certificate).hexdigest()}
                 if path == RAW_PATH else
                 {"certificate_base64": base64.b64encode(certificate).decode("ascii"),
                  "certificate_sha256_hex": hashlib.sha256(certificate).hexdigest()})
        return 200, json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")
