"""Server-side Play Integrity verification for an authenticated enrollment policy.

The public verifier submits the opaque token to Google's fixed HTTPS decoder.
It never accepts a mobile-supplied decoded verdict. Hardware key attestation
remains an independent check; these verdicts confer no offline state guarantee.

https://developer.android.com/google/play/integrity/standard
https://developer.android.com/google/play/integrity/verdicts
"""
from __future__ import annotations

import base64
import hashlib
import json
import re
import ssl
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Callable

from .attestation import AttestationRejected, VerificationUnavailable, fixed32, require

MAX_TOKEN_BYTES = 64 * 1024
MAX_RESPONSE_BYTES = 128 * 1024
_PACKAGE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)+\Z")
_DECIMAL = re.compile(r"(?:0|[1-9][0-9]{0,19})\Z")
_LABELS = frozenset({"MEETS_BASIC_INTEGRITY", "MEETS_DEVICE_INTEGRITY",
                     "MEETS_STRONG_INTEGRITY", "MEETS_VIRTUAL_INTEGRITY"})


class PlayIntegrityUnavailable(VerificationUnavailable):
    """Google's decoder or its OAuth token could not answer; retry later."""


@dataclass(frozen=True)
class PlayIntegrityEnrollmentPolicy:
    """Current E1 app/verdict/freshness policy with no periodic refresh or lease field."""
    policy_digest: bytes
    package_name: str
    package_version: int
    app_signing_certificate_sha256: bytes
    maximum_evidence_age_ms: int
    require_play_recognized: bool
    require_licensed: bool
    minimum_device_integrity: str

    def validate(self) -> None:
        require(any(fixed32(self.policy_digest, "Play Integrity policy")), "empty Play Integrity policy")
        require(type(self.package_name) is str and len(self.package_name) <= 255
                and _PACKAGE.fullmatch(self.package_name) is not None,
                "invalid Play Integrity package")
        require(type(self.package_version) is int and 0 <= self.package_version < (1 << 64),
                "invalid Play Integrity version")
        require(any(fixed32(self.app_signing_certificate_sha256, "Play app-signing certificate")),
                "empty Play app-signing certificate")
        require(type(self.maximum_evidence_age_ms) is int
                and 0 < self.maximum_evidence_age_ms < (1 << 64)
                and type(self.require_play_recognized) is bool
                and type(self.require_licensed) is bool
                and type(self.minimum_device_integrity) is str
                and self.minimum_device_integrity in ("MEETS_DEVICE_INTEGRITY", "MEETS_STRONG_INTEGRITY"),
                "invalid Play Integrity trust policy")


@dataclass(frozen=True)
class PlayIntegrityPolicy:
    """Retained primitive input for historical consumers; current E1 uses EnrollmentPolicy.

    This data type creates no lease. Removed ordinary issuer/refresh owners are not restored.
    """
    policy_digest: bytes
    package_name: str
    package_version: int
    app_signing_certificate_sha256: bytes
    maximum_evidence_age_ms: int
    maximum_refresh_interval_ms: int
    require_play_recognized: bool
    require_licensed: bool
    minimum_device_integrity: str

    def validate(self) -> None:
        PlayIntegrityEnrollmentPolicy(
            self.policy_digest, self.package_name, self.package_version,
            self.app_signing_certificate_sha256, self.maximum_evidence_age_ms,
            self.require_play_recognized, self.require_licensed,
            self.minimum_device_integrity).validate()
        require(type(self.maximum_refresh_interval_ms) is int
                and 0 < self.maximum_refresh_interval_ms < (1 << 64),
                "invalid historical Play Integrity refresh interval")


@dataclass(frozen=True)
class PlayIntegrityProof:
    """Projection of a server-decoded verdict, not an enrollment credential."""
    policy_digest: bytes
    request_hash: bytes
    timestamp_ms: int
    token_sha256: bytes
    google_response_sha256: bytes
    app_recognition_verdict: str
    app_licensing_verdict: str
    device_integrity: tuple[str, ...]


@dataclass(frozen=True)
class DecodedPlayIntegrityEvidence:
    """Original server TLS response retained with its checked projection.

    A caller that issues a credential from it must retain these bytes first.
    They are never an accepted mobile request field or a caller verdict grant.
    """
    google_response: bytes
    proof: PlayIntegrityProof


def request_hash_text(digest: bytes) -> str:
    """Use one unpadded base64url representation in SDK and Google requestHash."""
    return base64.urlsafe_b64encode(fixed32(digest, "Play enrollment request hash")).rstrip(b"=").decode("ascii")


def _unique(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        require(key not in result, "duplicate Play Integrity response member")
        result[key] = value
    return result


def _number(value: object, label: str) -> int:
    require(type(value) is str and _DECIMAL.fullmatch(value) is not None,
            f"invalid Play Integrity {label}")
    number = int(value)
    require(number < (1 << 64), f"invalid Play Integrity {label}")
    return number


def _verify_google_payload(body: bytes, policy: PlayIntegrityEnrollmentPolicy | PlayIntegrityPolicy,
                           expected_request_hash: bytes, trusted_time_ms: int,
                           token_sha256: bytes) -> PlayIntegrityProof:
    """Internal parser; its caller must obtain bytes directly from Google TLS."""
    policy.validate()
    expected_text = request_hash_text(expected_request_hash)
    require(type(trusted_time_ms) is int and 0 < trusted_time_ms < (1 << 64),
            "invalid Play Integrity trusted time")
    require(type(body) is bytes and 0 < len(body) <= MAX_RESPONSE_BYTES,
            "Play Integrity response outside bound")
    try:
        value = json.loads(body.decode("utf-8"), object_pairs_hook=_unique,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError()))
    except (UnicodeDecodeError, ValueError, RecursionError) as error:
        raise AttestationRejected("invalid Play Integrity response") from error
    require(type(value) is dict and set(value) == {"tokenPayloadExternal"},
            "invalid Play Integrity decoder envelope")
    payload = value["tokenPayloadExternal"]
    require(type(payload) is dict and {"requestDetails", "appIntegrity", "deviceIntegrity", "accountDetails"} <= set(payload),
            "missing Play Integrity verdicts")
    # Google Console can deliberately override verdicts for configured testing
    # accounts. Those responses cannot establish genuine release admission.
    require("testingDetails" not in payload, "Play Integrity testing response rejected")
    details, app, device, account = (payload[name] for name in
        ("requestDetails", "appIntegrity", "deviceIntegrity", "accountDetails"))
    require(all(type(item) is dict for item in (details, app, device, account)),
            "invalid Play Integrity verdict object")
    require(set(details) == {"requestPackageName", "requestHash", "timestampMillis"}
            and details["requestPackageName"] == policy.package_name
            and details["requestHash"] == expected_text,
            "Play Integrity enrollment request mismatch")
    timestamp = _number(details["timestampMillis"], "timestamp")
    require(0 < timestamp <= trusted_time_ms
            and trusted_time_ms - timestamp <= policy.maximum_evidence_age_ms,
            "Play Integrity evidence is stale or from the future")
    verdict = app.get("appRecognitionVerdict")
    require(type(verdict) is str and verdict in ("PLAY_RECOGNIZED", "UNRECOGNIZED_VERSION")
            and (not policy.require_play_recognized or verdict == "PLAY_RECOGNIZED"),
            "Play Integrity app is not recognized under selected policy")
    require(app.get("packageName") == policy.package_name
            and _number(app.get("versionCode"), "app version") == policy.package_version,
            "Play Integrity app identity differs")
    certificates = app.get("certificateSha256Digest")
    require(type(certificates) is list and certificates == [request_hash_text(policy.app_signing_certificate_sha256)],
            "Play Integrity app-signing certificate differs")
    licensing = account.get("appLicensingVerdict")
    require(type(licensing) is str and licensing in ("LICENSED", "UNLICENSED", "UNEVALUATED")
            and (not policy.require_licensed or licensing == "LICENSED"),
            "Play Integrity license differs from selected policy")
    labels = device.get("deviceRecognitionVerdict")
    require(type(labels) is list and 0 < len(labels) <= len(_LABELS)
            and all(type(label) is str and label in _LABELS for label in labels)
            and len(set(labels)) == len(labels)
            and "MEETS_VIRTUAL_INTEGRITY" not in labels
            and policy.minimum_device_integrity in labels,
            "Play Integrity device verdict differs from selected policy")
    return PlayIntegrityProof(policy.policy_digest, expected_request_hash, timestamp,
                             fixed32(token_sha256, "opaque integrity token digest"),
                             hashlib.sha256(body).digest(), verdict, licensing,
                             tuple(sorted(labels)))


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


class GooglePlayIntegrityVerifier:
    """Fixed Google decoder with deployment-owned OAuth service-account custody.

    The callback supplies a playintegrity-scoped OAuth access token from the
    authenticated server deployment. No credential or decoder URL comes from
    mobile input, and no caller-provided verdict callback is accepted.
    """
    def __init__(self, access_token: Callable[[], str]) -> None:
        require(callable(access_token), "Play Integrity service-account custody absent")
        self._access_token = access_token

    def verify(self, opaque_token: str, policy: PlayIntegrityEnrollmentPolicy | PlayIntegrityPolicy,
               expected_request_hash: bytes, trusted_time_ms: int) -> PlayIntegrityProof:
        return self.decode(opaque_token, policy, expected_request_hash, trusted_time_ms).proof

    def decode(self, opaque_token: str, policy: PlayIntegrityEnrollmentPolicy | PlayIntegrityPolicy,
               expected_request_hash: bytes, trusted_time_ms: int) -> DecodedPlayIntegrityEvidence:
        policy.validate()
        request_hash_text(expected_request_hash)
        require(type(opaque_token) is str and 0 < len(opaque_token) <= MAX_TOKEN_BYTES
                and all(33 <= ord(char) <= 126 for char in opaque_token),
                "invalid opaque Play Integrity token")
        # Only HTTP 400 (a malformed or foreign token) judges the token. A
        # transport, OAuth or other HTTP failure, or a changed decoder source
        # or response type, is retryable unavailability. Exceptions are not
        # chained: they may carry the bearer token.
        try:
            access = self._access_token()
        except Exception:
            raise PlayIntegrityUnavailable("Play Integrity OAuth token unavailable") from None
        if not (type(access) is str and 0 < len(access) <= 8192
                and all(33 <= ord(char) <= 126 for char in access)):
            raise PlayIntegrityUnavailable("Play Integrity service-account token unavailable")
        url = f"https://playintegrity.googleapis.com/v1/{policy.package_name}:decodeIntegrityToken"
        request = urllib.request.Request(url,
            data=json.dumps({"integrity_token": opaque_token}, separators=(",", ":")).encode("ascii"),
            headers={"Authorization": f"Bearer {access}", "Content-Type": "application/json", "Accept": "application/json"},
            method="POST")
        try:
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}),
                urllib.request.HTTPSHandler(context=ssl.create_default_context()), _NoRedirect())
            with opener.open(request, timeout=5) as response:
                if not (response.status == 200 and response.geturl() == url):
                    raise PlayIntegrityUnavailable("Play Integrity decoder source changed")
                if not (response.headers.get("Content-Type", "").split(";", 1)[0].strip().lower() == "application/json"
                        and response.headers.get("Content-Encoding", "identity").lower() == "identity"):
                    raise PlayIntegrityUnavailable("invalid Play Integrity response type")
                body = response.read(MAX_RESPONSE_BYTES + 1)
        except urllib.error.HTTPError as error:
            code = error.code
            error.close()
            if code == 400:
                raise AttestationRejected("Play Integrity decoder rejected the token") from None
            raise PlayIntegrityUnavailable("Play Integrity decoder unavailable") from None
        except VerificationUnavailable:
            raise
        except Exception:
            raise PlayIntegrityUnavailable("Play Integrity decoder unavailable") from None
        proof = _verify_google_payload(body, policy, expected_request_hash, trusted_time_ms,
                                      hashlib.sha256(opaque_token.encode("ascii")).digest())
        return DecodedPlayIntegrityEvidence(body, proof)
