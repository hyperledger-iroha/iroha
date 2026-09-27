"""Native receipt identity grammar regressions; doubles grant no custody evidence."""

from __future__ import annotations

import json
from contextlib import contextmanager
from pathlib import Path

import pytest

import sorafs_reference_sdk_signed_manifest as sources


NOW = 1_800_700_000
IDENTITY_FIELDS = ("service_id", "administrator_id", "deployment_id")
RESERVED = ("null", "mock", "test", "dev", "demo", "fake", "dummy", "placeholder")


@pytest.fixture
def native_receipt(monkeypatch):
    """Exercise the complete result parser independently of native cryptography."""
    digests = {source: f"{index:02x}" * 32 for index, source in enumerate(sources.NATIVE_SOURCE_FIELDS.values(), 1)}
    raw = sources.VerifiedManifestSignatureSources(
        "11" * 32, digests["manifest"], 42, digests["signature"],
        digests["public_key"], "12" * 32,
    )
    result = {
        "schema": sources.NATIVE_RECEIPT_SCHEMA, "status": "verified",
        **{field: digests[source] for field, source in sources.NATIVE_SOURCE_FIELDS.items()},
        "manifest_size": raw.manifest_size,
        "operation_id": "22" * 32, "custody_record_digest": "33" * 32,
        "policy_digest": "44" * 32, "key_revision": 7, "policy_revision": 9,
        "service_id": "release-signer", "administrator_id": "release-administrator",
        "role": "release_manifest", "deployment_id": "release-sdk-primary",
        "chain_id": "release-chain", "network_id": "55" * 32,
        "finalized_height": 101, "finalized_block_hash": "66" * 32,
        "verified_at_unix_ms": NOW * 1000,
    }

    @contextmanager
    def snapshot(*_args):
        yield raw, {}, Path("unused-native-verifier"), digests

    monkeypatch.setattr(sources, "_verified_manifest_source_snapshot", snapshot)
    monkeypatch.setattr(sources, "verify_receipt_snapshots", lambda *_args: json.dumps(result).encode())
    return result


@pytest.mark.parametrize("field", IDENTITY_FIELDS)
@pytest.mark.parametrize("identity", (
    "account-attester", "attestation", "latest", "contest",
    "Account-Attester", "ATTESTATION", "LATEST", "CONTEST",
    "production:primary_1.alpha", "a" * 128,
) + tuple(identity for reserved in RESERVED for identity in (f"x{reserved}", f"{reserved}1")))
def test_native_receipt_preserves_canonical_identity_words(native_receipt, field, identity):
    native_receipt[field] = identity
    result = sources.authenticate_signed_manifest_sources(None, None, NOW)
    assert dict(result.native_fields)[field] == identity
    assert result.canary_fields()[field] == identity


@pytest.mark.parametrize("field", IDENTITY_FIELDS)
@pytest.mark.parametrize("reserved", RESERVED)
@pytest.mark.parametrize("delimiter", (".", "_", "-", ":"))
def test_native_receipt_rejects_reserved_identity_components(native_receipt, field, reserved, delimiter):
    native_receipt[field] = f"production{delimiter}{reserved.upper()}{delimiter}primary"
    with pytest.raises(sources.SignedManifestSourceError, match="invalid signer or deployment identity"):
        sources.authenticate_signed_manifest_sources(None, None, NOW)


@pytest.mark.parametrize("field", IDENTITY_FIELDS)
@pytest.mark.parametrize("reserved", tuple(word for reserved in RESERVED for word in (reserved, reserved.upper())))
@pytest.mark.parametrize("template", ("{}", "{}-primary", "production-{}"))
def test_native_receipt_rejects_bare_and_edge_reserved_components(native_receipt, field, reserved, template):
    native_receipt[field] = template.format(reserved)
    with pytest.raises(sources.SignedManifestSourceError, match="invalid signer or deployment identity"):
        sources.authenticate_signed_manifest_sources(None, None, NOW)


@pytest.mark.parametrize("identity", (
    "", "a" * 129, " a", "a ", "a/b", "a@b", "a?b", "a#b",
    "a%2fb", "a;b", "a=b", "a\\b", "a\0b", "a\nb", "é", "テスト", None, True,
))
def test_native_receipt_rejects_malformed_identity_bytes(native_receipt, identity):
    native_receipt["service_id"] = identity
    with pytest.raises(sources.SignedManifestSourceError, match="invalid signer or deployment identity"):
        sources.authenticate_signed_manifest_sources(None, None, NOW)


def test_native_receipt_still_requires_independent_administrator(native_receipt):
    native_receipt["service_id"] = native_receipt["administrator_id"] = "account-attester"
    with pytest.raises(sources.SignedManifestSourceError, match="must be independent"):
        sources.authenticate_signed_manifest_sources(None, None, NOW)
