"""Process-contract doubles for receipt parsing; these do not qualify hardware custody.

Actual cryptographic receipt/observer/custody validation belongs to the native
suite and separately recorded native-process replay with public simulated data.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

import pytest

from sorafs_reference_sdk_signed_manifest_test import (
    source_context, _digest, _repin_context, _options,
)
import sorafs_reference_sdk_signed_manifest as sources
import sorafs_reference_sdk_receipt_verifier as process
import build_sorafs_reference_sdk_release_canary as builder
import check_sorafs_reference_sdk_release_evidence as checker

NOW = 1_800_700_000


def _native_result(context):
    return {
        "schema": sources.NATIVE_RECEIPT_SCHEMA, "status": "verified",
        **{field: context["sources"][name]["sha256"] for field, name in sources.NATIVE_SOURCE_FIELDS.items()},
        "manifest_size": Path(context["sources"]["manifest"]["path"]).stat().st_size,
        "operation_id": "22" * 32, "custody_record_digest": "33" * 32,
        "policy_digest": "44" * 32, "key_revision": 7, "policy_revision": 9,
        "service_id": "release-signer", "administrator_id": "release-administrator",
        "role": "release_manifest", "backend": "hardware",
        "deployment_id": "release-sdk-primary", "chain_id": "release-chain",
        "network_id": "55" * 32, "finalized_height": 101,
        "finalized_block_hash": "66" * 32, "verified_at_unix_ms": NOW * 1000,
    }


def _install_receipt_double(path, context, output, *, body=None):
    verifier = Path(context["sources"]["native_verifier"]["path"])
    original = verifier.read_text()
    expected = {name: row["sha256"] for name, row in context["sources"].items() if name != "native_verifier"}
    flags = {"--" + name.replace("_", "-") for name in expected} | {
        "--public-key-fingerprint", "--signer-policy-sha256", "--custody-trust-sha256", "--now-unix-ms", "--format",
    }
    block = f'''
import hashlib, json, os, pathlib, sys
if sys.argv[1:2] == ["release-manifest-receipt"]:
    values = sys.argv[2:]
    assert len(values) % 2 == 0
    args = dict(zip(values[::2], values[1::2]))
    assert len(args) * 2 == len(values) and set(args) == {flags!r}
    expected = {expected!r}
    for name, digest in expected.items():
        candidate = pathlib.Path(args["--" + name.replace("_", "-")])
        assert candidate.parent == pathlib.Path(sys.argv[0]).parent
        assert hashlib.sha256(candidate.read_bytes()).hexdigest() == digest
        assert candidate.stat().st_mode & 0o222 == 0
    assert args["--public-key-fingerprint"] == expected["public_key"]
    assert args["--signer-policy-sha256"] == expected["signer_policy"]
    assert args["--custody-trust-sha256"] == expected["custody_trust"]
    assert args["--now-unix-ms"] == {str(NOW * 1000)!r}
    assert args["--format"] == "json"
    # The launch-boundary test asserts the complete supplied environment.
    # Python/macOS can add interpreter and locale metadata after exec.
    assert "SIGNER_PRIVATE_KEY" not in os.environ
    assert "SORAFS_RELEASE_VERIFIER_BYPASS" not in os.environ
    assert os.environ.get("LC_CTYPE") != "caller-locale-must-not-be-forwarded"
    {body if body is not None else 'sys.stdout.buffer.write(' + repr(output) + ')'}
    raise SystemExit(0)
'''
    verifier.write_text(original.split("\n", 1)[0] + "\n" + block + original.split("\n", 1)[1])
    verifier.chmod(0o700)
    context["sources"]["native_verifier"]["sha256"] = _digest(verifier.read_bytes())
    return _repin_context(path, context)


@pytest.fixture
def receipt_context(source_context):
    path, context, _ = source_context
    result = _native_result(context)
    pin = _install_receipt_double(path, context, json.dumps(result).encode())
    return path, context, result, pin


def _claims(verified):
    return {
        "schema": "sorafs.reference_sdk.signed_manifest_canary.v1",
        "status": "passed", "generated_at_unix": NOW - 1,
        "environment": "production", "deployment_context_reviewed": True,
        **verified.canary_fields(),
    }


def test_exact_native_contract_derives_canary_and_checker_reauthenticates(receipt_context, monkeypatch):
    path, _, result, pin = receipt_context
    monkeypatch.setenv("SIGNER_PRIVATE_KEY", "must-not-be-forwarded")
    verified = sources.authenticate_signed_manifest_sources(path, pin, NOW)
    assert isinstance(verified, sources.VerifiedSignedManifestSources)
    assert dict(verified.native_fields) == result
    payload = _claims(verified)
    assert "verified_at_unix_ms" not in payload  # trusted clock is fresh on each recheck
    assert checker.validate_evidence_payload(payload, _options(path, pin)) == ("signed_manifest", [])
    assert not list(path.parent.glob(".sf11-*-*"))


def test_receipt_launcher_supplies_only_the_fixed_native_environment(receipt_context, monkeypatch):
    path, _, _, pin = receipt_context
    monkeypatch.setenv("SIGNER_PRIVATE_KEY", "must-not-be-forwarded")
    monkeypatch.setenv("LC_CTYPE", "caller-locale-must-not-be-forwarded")
    original = process.subprocess.Popen
    seen = []

    def inspect_launch(*args, **kwargs):
        assert kwargs["env"] == {"PATH": os.defpath}
        assert kwargs["close_fds"] is True
        seen.append(args[0][1])
        return original(*args, **kwargs)

    monkeypatch.setattr(process.subprocess, "Popen", inspect_launch)
    sources.authenticate_signed_manifest_sources(path, pin, NOW)
    assert "release-manifest-receipt" in seen


@pytest.mark.parametrize("field", sorted(sources.NATIVE_RECEIPT_FIELDS))
def test_native_result_requires_every_exact_field(source_context, field):
    path, context, _ = source_context
    result = _native_result(context)
    del result[field]
    pin = _install_receipt_double(path, context, json.dumps(result).encode())
    with pytest.raises(sources.SignedManifestSourceError, match="closed schema"):
        sources.authenticate_signed_manifest_sources(path, pin, NOW)


@pytest.mark.parametrize("field", sorted(sources.NATIVE_SOURCE_FIELDS))
def test_native_result_cannot_retarget_any_pinned_source(source_context, field):
    path, context, _ = source_context
    result = _native_result(context)
    result[field] = "aa" * 32
    pin = _install_receipt_double(path, context, json.dumps(result).encode())
    with pytest.raises(sources.SignedManifestSourceError, match="pinned sources"):
        sources.authenticate_signed_manifest_sources(path, pin, NOW)


@pytest.mark.parametrize("field,value", (
    ("schema", "sorafs.external_software_signer.signature_receipt_validation.v1"),
    ("status", "passed"), ("role", "promotion"), ("backend", "software"),
    ("manifest_size", 1), ("verified_at_unix_ms", NOW * 1000 - 1),
    ("key_revision", True), ("policy_revision", 0), ("finalized_height", 1.5),
    ("manifest_size", 1 << 64), ("operation_id", "00" * 32),
    ("custody_record_digest", "AB" * 32), ("policy_digest", None),
    ("service_id", "release\nservice"), ("administrator_id", "release-signer"),
    ("deployment_id", "production-test"), ("chain_id", "-release-chain"),
    ("chain_id", "a" * 129), ("network_id", "00" * 32),
))
def test_native_result_rejects_downgrade_invalid_identity_and_clock(source_context, field, value):
    path, context, _ = source_context
    result = _native_result(context)
    result[field] = value
    pin = _install_receipt_double(path, context, json.dumps(result).encode())
    with pytest.raises(sources.SignedManifestSourceError):
        sources.authenticate_signed_manifest_sources(path, pin, NOW)


@pytest.mark.parametrize("encoding", ("duplicate", "unknown", "array", "utf16", "trailing", "oversized", "exit_failure"))
def test_native_result_is_bounded_closed_utf8_json(source_context, encoding):
    path, context, _ = source_context
    result = _native_result(context)
    output = json.dumps(result).encode()
    body = None
    if encoding == "duplicate": output = b'{"status":"verified",' + output[1:]
    elif encoding == "unknown": output = b'{"hardware_claim":true,' + output[1:]
    elif encoding == "array": output = b"[]"
    elif encoding == "utf16": output = json.dumps(result).encode("utf-16")
    elif encoding == "trailing": output += b"{}"
    elif encoding == "oversized": output = b"x" * (process.MAX_RESULT_BYTES + 1)
    else: body = "raise SystemExit(2)"
    pin = _install_receipt_double(path, context, output, body=body)
    with pytest.raises(sources.SignedManifestSourceError):
        sources.authenticate_signed_manifest_sources(path, pin, NOW)


def test_native_process_timeout_is_a_failure_and_its_child_is_reaped(source_context, monkeypatch):
    path, context, _ = source_context
    pin = _install_receipt_double(path, context, b"", body="__import__('time').sleep(5)")
    monkeypatch.setattr(process, "VERIFIER_TIMEOUT_SECONDS", 0.05)
    with pytest.raises(sources.SignedManifestSourceError):
        sources.authenticate_signed_manifest_sources(path, pin, NOW)
    assert not list(path.parent.glob(".sf11-*-*"))


@pytest.mark.parametrize("now", (None, True, 0, -1, 1.5, ((1 << 64) - 1) // 1000 + 1))
def test_native_receipt_requires_an_independent_bounded_integer_clock(source_context, now):
    path, _, log = source_context
    with pytest.raises(sources.SignedManifestSourceError, match="clock"):
        sources.authenticate_signed_manifest_sources(path, _digest(path.read_bytes()), now)
    assert not log.exists()


@pytest.mark.parametrize("field", sorted(sources.SIGNED_MANIFEST_CANARY_FIELDS) + ["deployment_id"])
def test_checker_compares_every_claim_to_authenticated_native_result(receipt_context, field):
    path, _, _, pin = receipt_context
    verified = sources.authenticate_signed_manifest_sources(path, pin, NOW)
    payload = _claims(verified)
    old = payload[field]
    payload[field] = not old if isinstance(old, bool) else old + 1 if isinstance(old, int) else "retargeted"
    assert checker.validate_evidence_payload(payload, _options(path, pin))[1]


def test_builder_uses_native_deployment_and_does_not_emit_unverified_publication_claims(receipt_context):
    path, _, _, pin = receipt_context
    output = path.parent / "canary.json"
    args = ["--kind", "signed_manifest", "--out", str(output), "--deployment-id", "release-sdk-primary",
            "--environment", "production", "--generated-at-unix", str(NOW - 1), "--now-unix", str(NOW),
            "--signed-manifest-source-context", str(path), "--signed-manifest-source-context-sha256", pin]
    assert builder.main(args) == 0
    payload = json.loads(output.read_text())
    assert set(payload) == set(checker.EVIDENCE_REQUIRED_FIELDS["signed_manifest"])
    assert not {"manifest_sha256_published", "signing_provider", "signer_response_verified", "private_key_absent"} & payload.keys()
    output.unlink()
    args[args.index("--deployment-id") + 1] = "different-deployment"
    assert builder.main(args) == 2
    assert not output.exists()
