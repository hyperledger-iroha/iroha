"""Strict data projection from the live Native parent, never a custody grant.

Only the descriptor-held worker composition consumes these values. A decoded
public JSON copy cannot recreate Native startup, a current release owner, the
protected signer descriptor or authenticated Core caller custody.
"""
from __future__ import annotations

import hashlib
from dataclasses import dataclass

from .attestation import require
from .google_oauth import _json, select_google_decoder
from .ordinary_provider import OrdinaryReleasePolicy
from .play_integrity import PlayIntegrityPolicy
from .provider import AppleAppPolicy, GoogleKeyMintPolicy
from .service import _decode_base64, _decode_hex32

SCHEMA = "iroha.kagemusha.ordinary-app-policy-projection.v1"
PROVIDER_SCHEMA = "iroha.kagemusha.ordinary-app-provider-selection.v1"
PROFILE_FIELDS = {"version", "release_id", "network_id", "hardware_profile_id", "platform_class",
    "suite_id", "policy_epoch", "profile_valid_from_ms", "profile_expires_at_ms", "provider_policy_root",
    "authority_policy_digest", "trust_policy_digest", "issuer_policy_digest", "app_signing_identity_digest",
    "app_release_digest", "authority_public_key", "ordinary_issuer_public_key_sec1_base64",
    "core_preparation_public_key", "maximum_credential_lifetime_ms",
    "allowed_android_security_levels", "play_integrity_policy", "play_integrity_policy_base64",
    "ordinary_trust_policy_base64", "app_authority_policy_base64", "original_sha256"}


def _hex(value, label):
    result = _decode_hex32(value, label)
    require(any(result), f"empty {label}")
    return result


def _original(value, expected, label, bound):
    result = _decode_base64(value, label, bound)
    require(hashlib.sha256(result).digest() == _hex(expected, label + " original pin"),
            f"{label} original changed")
    return result


@dataclass(frozen=True)
class NativePolicyProjection:
    policies: tuple[OrdinaryReleasePolicy, ...]
    google_public_originals: tuple[tuple[PlayIntegrityPolicy, bytes], ...]
    original_sha256: bytes


def decode_native_policy_projection(original: bytes) -> NativePolicyProjection:
    value = _json(original, 3*1024*1024, "Native app policy projection")
    require(set(value) == {"schema", "version", "profiles", "provider_selection_base64", "original_sha256"}
            and value["schema"] == SCHEMA and type(value["version"]) is int and value["version"] == 1
            and type(value["original_sha256"]) is dict and set(value["original_sha256"]) == {"provider_selection"},
            "Native policy projection layout differs")
    provider_original = _original(value["provider_selection_base64"],
        value["original_sha256"]["provider_selection"], "Native provider selection", 256*1024)
    provider = _json(provider_original,256*1024,"Native provider selection")
    require(set(provider) == {"schema", "version", "profiles"} and provider["schema"] == PROVIDER_SCHEMA
            and type(provider["version"]) is int and provider["version"] == 1
            and type(provider["profiles"]) is list and 0 < len(provider["profiles"]) <= 64,
            "Native provider selection layout differs")
    configurations = {}
    for entry in provider["profiles"]:
        require(type(entry) is dict and set(entry) == {"hardware_profile_id", "platform_class", "configuration"},
                "Native provider profile layout differs")
        profile_id = _hex(entry["hardware_profile_id"], "provider profile")
        require(profile_id not in configurations and type(entry["configuration"]) is dict,
                "ambiguous Native provider profile")
        configurations[profile_id] = entry
    require(type(value["profiles"]) is list and 0 < len(value["profiles"]) <= 64,
            "Native policy profiles outside bound")
    policies, google, seen = [], [], set()
    for entry in value["profiles"]:
        require(type(entry) is dict and set(entry) == PROFILE_FIELDS
                and type(entry["version"]) is int and entry["version"] == 1,
                "Native policy profile layout differs")
        profile_id = _hex(entry["hardware_profile_id"], "Native profile")
        configuration = configurations.get(profile_id)
        require(profile_id not in seen and configuration is not None
                and configuration["platform_class"] == entry["platform_class"],
                "Native provider does not select the admitted profile")
        seen.add(profile_id); config = configuration["configuration"]
        pins = entry["original_sha256"]
        require(type(pins) is dict and set(pins) == {"release_manifest", "issuer_policy", "ordinary_trust_policy",
                                                   "app_authority_policy", "play_integrity_policy"},
                "Native profile original pins differ")
        _hex(pins["release_manifest"],"release manifest original")
        _hex(pins["issuer_policy"],"issuer policy original")
        _hex(entry["provider_policy_root"],"provider policy root")
        _original(entry["ordinary_trust_policy_base64"],pins["ordinary_trust_policy"],"ordinary trust policy",16*1024)
        _original(entry["app_authority_policy_base64"],pins["app_authority_policy"],"app authority policy",16*1024)
        integrity = None
        if entry["platform_class"] == "android_keymint":
            require(set(config) == {"package_name", "package_version", "app_signing_certificate_sha256",
                                    "attestation_roots_der_base64"}
                    and type(config["package_version"]) is int and 0 < config["package_version"] < (1 << 64)
                    and type(config["attestation_roots_der_base64"]) is list
                    and 0 < len(config["attestation_roots_der_base64"]) <= 3
                    and type(entry["allowed_android_security_levels"]) is list
                    and all(type(level) is int and level in (1,2) for level in entry["allowed_android_security_levels"])
                    and len(set(entry["allowed_android_security_levels"])) == len(entry["allowed_android_security_levels"]),
                    "Native Android provider fields differ")
            roots = tuple(_decode_base64(root,"Google attestation root",16*1024)
                          for root in config["attestation_roots_der_base64"])
            platform = GoogleKeyMintPolicy(config["package_name"],config["package_version"],
                _hex(config["app_signing_certificate_sha256"],"Play app signing certificate"),roots[0],
                hashlib.sha256(roots[0]).digest(),frozenset(entry["allowed_android_security_levels"]),roots[1:])
            if entry["play_integrity_policy"] is not None:
                p = entry["play_integrity_policy"]
                require(type(p) is dict and set(p) == {"policy_digest", "maximum_evidence_age_ms",
                    "maximum_refresh_interval_ms", "require_play_recognized", "require_licensed", "minimum_device_integrity"}
                    and type(p["minimum_device_integrity"]) is int and p["minimum_device_integrity"] in (1,2),
                    "Native Play Integrity policy layout differs")
                integrity = PlayIntegrityPolicy(_hex(p["policy_digest"],"Play Integrity policy"),platform.package_name,
                    platform.package_version,platform.signing_certificate_sha256,p["maximum_evidence_age_ms"],
                    p["maximum_refresh_interval_ms"],p["require_play_recognized"],p["require_licensed"],
                    {1:"MEETS_DEVICE_INTEGRITY",2:"MEETS_STRONG_INTEGRITY"}[p["minimum_device_integrity"]])
                raw = _original(entry["play_integrity_policy_base64"],pins["play_integrity_policy"],"Play Integrity policy",16*1024)
                select_google_decoder(raw,integrity); google.append((integrity,raw))
        elif entry["platform_class"] == "apple_app_attest":
            require(set(config) == {"app_id", "environment", "expected_validation_category", "expected_bundle_version",
                "attestation_root_der_base64", "receipt_root_der_base64"}
                and entry["allowed_android_security_levels"] == [] and entry["play_integrity_policy"] is None,
                "Native Apple provider fields differ")
            roots = [_decode_base64(config[name],"Apple root",16*1024) for name in
                     ("attestation_root_der_base64","receipt_root_der_base64")]
            platform = AppleAppPolicy(config["app_id"],config["environment"],roots[0],hashlib.sha256(roots[0]).digest(),
                roots[1],hashlib.sha256(roots[1]).digest(),config["expected_validation_category"],config["expected_bundle_version"])
        else:
            require(False,"unsupported Native ordinary provider platform")
        if integrity is None:
            require(entry["play_integrity_policy_base64"] is None and pins["play_integrity_policy"] is None,
                    "unexpected ungoverned Google original")
        policy = OrdinaryReleasePolicy(_hex(entry["release_id"],"release"),_hex(entry["network_id"],"network"),profile_id,
            _hex(entry["suite_id"],"suite"),_hex(entry["trust_policy_digest"],"trust policy"),
            _hex(entry["authority_policy_digest"],"app authority policy"),_hex(entry["issuer_policy_digest"],"issuer policy"),
            entry["policy_epoch"],entry["profile_valid_from_ms"],entry["profile_expires_at_ms"],
            _hex(entry["app_signing_identity_digest"],"app signing identity"),_hex(entry["app_release_digest"],"app release"),
            _hex(entry["core_preparation_public_key"],"Core preparation signer"),_hex(entry["authority_public_key"],"app authority signer"),
            _decode_base64(entry["ordinary_issuer_public_key_sec1_base64"],"governed ordinary circuit issuer",65),
            entry["maximum_credential_lifetime_ms"],platform,integrity)
        policy.validate(); policies.append(policy)
    require(seen == configurations.keys(),"Native provider includes unserved profile")
    if google:
        first = select_google_decoder(google[0][1],google[0][0])
        require(all(select_google_decoder(raw,p) == first for p,raw in google),
                "Native Google profiles require different protected principals")
    return NativePolicyProjection(tuple(policies),tuple(google),hashlib.sha256(original).digest())
