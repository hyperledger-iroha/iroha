"""Hardware-only verifier behind the authenticated Native private worker pipes.

No HTTP listener, offered root, financial enrollment, wallet counter or monetary
capability exists here. Native authenticates C and decodes the sole original
platform archive before lending its exact ordered DERs to this verifier.
"""
from __future__ import annotations
import base64
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from .attestation import (require, verify_android_persistent_app_key_raw)
from .google_oauth import (GoogleServiceAccountTokenProvider, _json, select_google_decoder)
from .provider import GoogleKeyMintPolicy
from .play_integrity import (GooglePlayIntegrityVerifier, PlayIntegrityPolicy,
                             _verify_google_payload)
from .revocation import verify_google_chain_not_revoked
from .service import _decode_base64, _decode_hex32

RAW_POLICY_SCHEMA = "iroha.kagemusha.hardware-evidence-raw-policy.v1"
MAX_BODY = 512 * 1024

@dataclass(frozen=True)
class _OriginalGenerationChallenge:
    original: bytes
    attested_key_id: bytes = bytes(32)
    def transcript(self) -> bytes:
        return self.original

def require_original_window(issued: int, expires: int, now: int) -> None:
    """Data-only bound check; a numeric value cannot construct Native clock custody."""
    require(type(issued) is int and type(expires) is int and type(now) is int
            and 0 < issued <= now < expires < (1 << 64) and expires - issued <= 120000,
            "hardware C expired or future")

class NativeHardwareEvidenceVerifier:
    """Installed public policy and actual inherited Google credential custody.

    Construction belongs only to the Native startup packet path. These data
    projections do not authenticate their own deployment authority.
    """
    def __init__(self, value: dict, channel, *, credential_fd: int):
        require(type(value) is dict and set(value) == {
            "manifest_digest", "package_name", "package_version", "signing_certificate_sha256",
            "raw_policy_original_base64", "raw_policy_sha256", "integrity_policy_original_base64",
            "integrity_policy_sha256", "cloud_project_number", "maximum_evidence_age_ms",
            "maximum_refresh_interval_ms", "require_play_recognized", "require_licensed",
            "minimum_device_integrity", "allowed_security_levels"}, "hardware startup projection differs")
        self._channel = channel
        self._manifest = _decode_hex32(value["manifest_digest"], "hardware manifest")
        levels = value["allowed_security_levels"]
        require(type(levels) is list and levels in ([1], [2], [1, 2]), "hardware levels differ")
        raw = _decode_base64(value["raw_policy_original_base64"], "hardware raw policy", 192*1024)
        require(hashlib.sha256(raw).digest() == _decode_hex32(value["raw_policy_sha256"], "hardware raw pin"), "hardware raw policy pin differs")
        policy = _json(raw, 192*1024, "hardware raw policy")
        require(set(policy) == {"schema", "version", "package_name", "package_version",
            "signing_certificate_sha256", "attestation_roots_der_base64", "allowed_security_levels"}
            and policy["schema"] == RAW_POLICY_SCHEMA and type(policy["version"]) is int and policy["version"] == 1
            and policy["package_name"] == value["package_name"]
            and policy["package_version"] == value["package_version"]
            and policy["signing_certificate_sha256"] == value["signing_certificate_sha256"]
            and policy["allowed_security_levels"] == levels, "hardware raw app policy differs")
        roots = policy["attestation_roots_der_base64"]
        require(type(roots) is list and 1 <= len(roots) <= 3, "hardware roots outside bound")
        roots = tuple(_decode_base64(r, "hardware root", 16*1024) for r in roots)
        self._raw = GoogleKeyMintPolicy(value["package_name"], value["package_version"],
            _decode_hex32(value["signing_certificate_sha256"], "hardware signer"),
            roots[0], hashlib.sha256(roots[0]).digest(), frozenset(levels), roots[1:])
        self._raw.validate()
        self._pi = PlayIntegrityPolicy(_decode_hex32(value["integrity_policy_sha256"], "hardware PI pin"),
            value["package_name"], value["package_version"], self._raw.signing_certificate_sha256,
            value["maximum_evidence_age_ms"], value["maximum_refresh_interval_ms"],
            value["require_play_recognized"], value["require_licensed"],
            {1:"MEETS_DEVICE_INTEGRITY", 2:"MEETS_STRONG_INTEGRITY"}.get(value["minimum_device_integrity"], ""))
        self._pi.validate()
        original = _decode_base64(value["integrity_policy_original_base64"], "hardware PI policy", 16*1024)
        decoder = select_google_decoder(original, self._pi)
        require(decoder.project_number == value["cloud_project_number"], "hardware PI project differs")
        channel.recheck()
        self._oauth = GoogleServiceAccountTokenProvider(public_policy_original=original, native_policy=self._pi,
            credential_fd=credential_fd, trusted_time_ms=channel.trusted_time_ms,
            openssl_path=Path("/usr/bin/openssl"), credential_owner_uid=0)
        self._integrity = GooglePlayIntegrityVerifier(self._oauth)
        channel.recheck()

    def close(self):
        self._oauth.close()

    def _window(self, c: dict):
        self._channel.recheck()
        now = self._channel.trusted_time_ms()
        require_original_window(c["issued_at_ms"], c["expires_at_ms"], now)
        return now

    def handle(self, phase: str, body: bytes) -> bytes:
        value = _json(body, MAX_BODY, "Native hardware evidence request")
        if phase == "hardware_raw":
            require(set(value) == {"signed_challenge_original_base64", "issued_at_ms", "expires_at_ms",
                "certificate_chain_der_base64", "raw_original_sha256"}, "hardware raw private request differs")
            c = _decode_base64(value["signed_challenge_original_base64"], "hardware signed C", 192*1024)
            _decode_hex32(value["raw_original_sha256"], "hardware raw original")
            chain = value["certificate_chain_der_base64"]
            require(type(chain) is list and 2 <= len(chain) <= 8, "hardware chain outside bound")
            chain = [_decode_base64(r, "hardware DER", 16*1024) for r in chain]
            now = self._window(value)
            root = self._raw.root_for_chain(chain[-1])
            proof = verify_android_persistent_app_key_raw(chain, _OriginalGenerationChallenge(c),
                self._raw.package_name, self._raw.package_version, self._raw.signing_certificate_sha256,
                root, hashlib.sha256(root).digest(), now, Path("/usr/bin/openssl"),
                allowed_security_levels=self._raw.allowed_security_levels)
            verify_google_chain_not_revoked(chain)
            checked = self._window(value)
            repeated = verify_android_persistent_app_key_raw(chain, _OriginalGenerationChallenge(c),
                self._raw.package_name, self._raw.package_version, self._raw.signing_certificate_sha256,
                root, hashlib.sha256(root).digest(), checked, Path("/usr/bin/openssl"),
                allowed_security_levels=self._raw.allowed_security_levels)
            require(repeated.attested_public_key_sec1 == proof.attested_public_key_sec1
                    and repeated.android_security_level == proof.android_security_level,
                    "hardware chain changed after original revocation check")
            result = {"app_public_key_sec1_hex": proof.attested_public_key_sec1.hex(),
                "security_level": proof.android_security_level, "checked_at_ms": checked,
                "challenge_original_sha256": hashlib.sha256(c).hexdigest(),
                "raw_original_sha256": value["raw_original_sha256"]}
        else:
            require(phase == "hardware_integrity" and set(value) == {"issued_at_ms", "expires_at_ms",
                "integrity_request_hash", "play_integrity_token"}, "hardware PI private request differs")
            digest = _decode_hex32(value["integrity_request_hash"], "hardware PI request")
            now = self._window(value)
            checked = self._integrity.decode(value["play_integrity_token"], self._pi, digest, now)
            now = self._window(value)
            # Re-evaluate freshness after TLS/OAuth and Native custody checks.
            proof = _verify_google_payload(checked.google_response, self._pi, digest, now,
                hashlib.sha256(value["play_integrity_token"].encode("ascii")).digest())
            result = {"verified_at_ms": now, "token_original_sha256": proof.token_sha256.hex(),
                "integrity_request_hash": proof.request_hash.hex(),
                "google_response_original_base64": base64.b64encode(checked.google_response).decode("ascii")}
        return json.dumps(result, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("ascii")
