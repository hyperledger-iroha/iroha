"""Bounded source authentication and fail-closed hardware dependency regressions."""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path

import pytest

SCRIPT_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import sorafs_reference_sdk_signed_manifest as sources
import build_sorafs_reference_sdk_release_canary as builder
import check_sorafs_reference_sdk_release_evidence as checker
from release_manifest_signing_test import (
    TEST_FINGERPRINT, TEST_MANIFEST, TEST_PUBLIC_KEY, TEST_SIGNATURE, _native_verifier,
)
from sccp_release_common import verify_ed25519


def _digest(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


@pytest.fixture
def source_context(tmp_path: Path):
    inputs = {
        "manifest": TEST_MANIFEST,
        "signature": TEST_SIGNATURE,
        "public_key": TEST_PUBLIC_KEY,
        # Pinned candidate bytes are deliberately not treated as authenticated custody.
        "signer_policy": b'{"candidate":"policy"}',
        "custody_trust": b'{"candidate":"trust"}',
        "completed_operation_state": b'{"candidate":"completed"}',
        "operation_receipt": b'{"candidate":"hardware","verified":true}',
    }
    references = {}
    for name, payload in inputs.items():
        path = tmp_path / name
        path.write_bytes(payload)
        path.chmod(0o600)
        references[name] = {"path": str(path), "sha256": _digest(payload)}
    verifier, verifier_digest, log = _native_verifier(
        tmp_path, require_sanitized_environment=True,
        forbidden_input_paths={name: Path(references[name]["path"]) for name in ("manifest", "signature", "public_key")},
    )
    references["native_verifier"] = {"path": str(verifier), "sha256": verifier_digest}
    context = {"schema": sources.SOURCE_CONTEXT_SCHEMA, "sources": references}
    path = tmp_path / "source-context.json"
    path.write_text(json.dumps(context), encoding="utf-8")
    path.chmod(0o600)
    return path, context, log


def _repin_context(path: Path, context: dict) -> str:
    path.write_text(json.dumps(context), encoding="utf-8")
    return _digest(path.read_bytes())


def _options(path: Path | None = None, digest: str | None = None) -> checker.ValidationOptions:
    return checker.ValidationOptions(
        now_unix=1_800_700_000,
        max_evidence_age_secs=86_400,
        min_release_targets=5,
        min_downstream_packages=6,
        max_smoke_duration_secs=1800,
        signed_manifest_source_context=path,
        signed_manifest_source_context_sha256=digest,
    )


def _forged_canary() -> dict:
    return {
        "schema": "sorafs.reference_sdk.signed_manifest_canary.v1",
        "status": "passed", "generated_at_unix": 1_800_699_999,
        "deployment_id": "release-sdk-primary", "environment": "production",
        "deployment_context_reviewed": True, "manifest_signed": True,
        "manifest_signature_verified": True, "manifest_sha256_published": True,
        "governed_release_key_used": True, "public_key_fingerprint_recorded": True,
        "private_key_absent": True, "signature_algorithm": "ed25519",
        "signing_provider": "authenticated_external_signer", "signing_backend": "hardware",
        "signing_provider_revision": 1, "signer_response_verified": True,
        "manifest_digest_hex": _digest(TEST_MANIFEST), "policy_digest_hex": "11" * 32,
        "public_key_fingerprint_hex": TEST_FINGERPRINT, "raw_manifest_included": False,
    }


def test_raw_signature_sources_are_verified_without_claiming_hardware(source_context, monkeypatch):
    path, context, log = source_context
    assert verify_ed25519(TEST_PUBLIC_KEY, TEST_SIGNATURE, TEST_MANIFEST)
    monkeypatch.setenv("SORAFS_RELEASE_VERIFIER_BYPASS", "must-not-reach-native")
    result = sources.verify_signed_manifest_source_files(path, _digest(path.read_bytes()))
    assert result.manifest_sha256 == _digest(TEST_MANIFEST)
    assert result.manifest_size == len(TEST_MANIFEST)
    assert result.signature_sha256 == _digest(TEST_SIGNATURE)
    assert result.public_key_sha256 == TEST_FINGERPRINT
    assert result.native_verifier_sha256 == context["sources"]["native_verifier"]["sha256"]
    assert not hasattr(result, "hardware_custody_verified")
    assert log.read_text() == "release-manifest\n"
    assert not list(path.parent.glob(".sf11-manifest-sources-*"))


def test_pinned_raw_signature_and_receipt_claims_cannot_qualify_hardware(source_context):
    path, _, log = source_context
    with pytest.raises(sources.SignedManifestSourceError, match="ReleaseManifest receipt"):
        sources.authenticate_signed_manifest_sources(path, _digest(path.read_bytes()), 1_800_700_000)
    assert log.read_text() == "release-manifest\n"


@pytest.mark.parametrize("name", sorted(sources.SOURCE_FIELDS))
def test_every_independent_source_is_required(source_context, name):
    path, context, log = source_context
    del context["sources"][name]
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, _repin_context(path, context))
    assert not log.exists()


@pytest.mark.parametrize("name", sorted(sources.SOURCE_FIELDS))
def test_every_source_digest_is_independently_pinned(source_context, name):
    path, context, _ = source_context
    context["sources"][name]["sha256"] = "12" * 32
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, _repin_context(path, context))


@pytest.mark.parametrize("name", sorted(sources.SOURCE_LIMITS))
@pytest.mark.parametrize("mutation", ("symlink", "hardlink", "writable", "oversized"))
def test_unsafe_or_oversized_sources_are_rejected(source_context, name, mutation):
    path, context, log = source_context
    leaf = Path(context["sources"][name]["path"])
    if mutation == "symlink":
        renamed = leaf.with_suffix(".original")
        leaf.rename(renamed)
        leaf.symlink_to(renamed)
    elif mutation == "hardlink":
        os.link(leaf, leaf.with_suffix(".alias"))
    elif mutation == "writable":
        leaf.chmod(0o666)
    else:
        payload = b"x" * (sources.SOURCE_LIMITS[name] + 1)
        leaf.write_bytes(payload)
        context["sources"][name]["sha256"] = _digest(payload)
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, _repin_context(path, context))
    assert not log.exists()


@pytest.mark.parametrize("mutation", ("missing_pin", "wrong_pin", "duplicate", "unknown", "relative", "alias"))
def test_source_context_has_one_closed_independent_trust_path(source_context, mutation):
    path, context, log = source_context
    pin = _digest(path.read_bytes())
    if mutation == "missing_pin":
        pin = None
    elif mutation == "wrong_pin":
        pin = "ab" * 32
    elif mutation == "duplicate":
        path.write_text('{"schema":"duplicate",' + json.dumps(context)[1:])
        pin = _digest(path.read_bytes())
    elif mutation == "unknown":
        context["verified"] = True
        pin = _repin_context(path, context)
    elif mutation == "relative":
        context["sources"]["signer_policy"]["path"] = "signer-policy.json"
        pin = _repin_context(path, context)
    else:
        context["sources"]["signer_policy"] = context["sources"]["custody_trust"].copy()
        pin = _repin_context(path, context)
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, pin)
    assert not log.exists()


@pytest.mark.parametrize("name", sorted(sources.SOURCE_LIMITS))
def test_original_source_replacement_is_detected_across_native_verification(source_context, monkeypatch, name):
    path, context, _ = source_context
    native = sources.verify_release_manifest
    def replace_source(*args):
        result = native(*args)
        leaf = Path(context["sources"][name]["path"])
        payload = leaf.read_bytes()
        leaf.unlink()
        leaf.write_bytes(payload)
        leaf.chmod(0o600)
        return result
    monkeypatch.setattr(sources, "verify_release_manifest", replace_source)
    with pytest.raises(sources.SignedManifestSourceError, match="changed"):
        sources.verify_signed_manifest_source_files(path, _digest(path.read_bytes()))


@pytest.mark.parametrize("name", sorted(sources.SOURCE_LIMITS))
def test_private_snapshot_replacement_is_detected_across_native_verification(source_context, monkeypatch, name):
    path, _, _ = source_context
    native = sources.verify_release_manifest
    def replace_snapshot(*args):
        result = native(*args)
        leaf = args[0].parent / name
        payload = leaf.read_bytes()
        leaf.unlink()
        leaf.write_bytes(payload)
        leaf.chmod(0o400)
        return result
    monkeypatch.setattr(sources, "verify_release_manifest", replace_snapshot)
    with pytest.raises(sources.SignedManifestSourceError, match="changed"):
        sources.verify_signed_manifest_source_files(path, _digest(path.read_bytes()))


def test_context_replacement_is_detected_across_native_verification(source_context, monkeypatch):
    path, _, _ = source_context
    native = sources.verify_release_manifest
    def replace_context(*args):
        result = native(*args)
        payload = path.read_bytes()
        path.unlink()
        path.write_bytes(payload)
        path.chmod(0o600)
        return result
    monkeypatch.setattr(sources, "verify_release_manifest", replace_context)
    with pytest.raises(sources.SignedManifestSourceError, match="changed"):
        sources.verify_signed_manifest_source_files(path, _digest(path.read_bytes()))


def test_ancestor_replacement_cannot_preserve_trust_by_moving_the_same_lower_subtree(source_context, monkeypatch):
    path, context, _ = source_context
    outer = path.parent / "authority"
    inner = outer / "inputs"
    inner.mkdir(parents=True)
    for reference in context["sources"].values():
        original = Path(reference["path"])
        relocated = inner / original.name
        original.rename(relocated)
        reference["path"] = str(relocated)
    moved_context = inner / path.name
    path.rename(moved_context)
    path = moved_context
    pin = _repin_context(path, context)
    context_identity = path.stat().st_ino
    inner_identity = inner.stat().st_ino
    native = sources.verify_release_manifest
    def replace_ancestor(*args):
        result = native(*args)
        retired = outer.with_name("retired-authority")
        outer.rename(retired)
        outer.mkdir()
        (retired / "inputs").rename(inner)
        assert path.stat().st_ino == context_identity
        assert inner.stat().st_ino == inner_identity
        return result
    monkeypatch.setattr(sources, "verify_release_manifest", replace_ancestor)
    with pytest.raises(sources.SignedManifestSourceError, match="ancestor changed"):
        sources.verify_signed_manifest_source_files(path, pin)


@pytest.mark.parametrize("name", ("manifest", "signature", "public_key"))
def test_repinning_mutated_inputs_cannot_replace_native_signature_verification(source_context, name):
    path, context, log = source_context
    leaf = Path(context["sources"][name]["path"])
    payload = bytearray(leaf.read_bytes())
    payload[0] ^= 1
    leaf.write_bytes(payload)
    context["sources"][name]["sha256"] = _digest(payload)
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, _repin_context(path, context))
    # The exact pinned test process rejects the changed vector (or native input
    # validation rejects its key material); candidate digests cannot grant trust.
    if log.exists():
        assert log.read_text() == "release-manifest\n"


def test_source_context_rejects_non_utf8_encoding(source_context):
    path, context, log = source_context
    path.write_bytes(json.dumps(context).encode("utf-16"))
    with pytest.raises(sources.SignedManifestSourceError):
        sources.verify_signed_manifest_source_files(path, _digest(path.read_bytes()))
    assert not log.exists()


@pytest.mark.parametrize("field", ("manifest_digest_hex", "policy_digest_hex", "public_key_fingerprint_hex", "signing_backend"))
def test_artifact_only_claims_and_retargeting_never_pass(field):
    payload = _forged_canary()
    kind, errors = checker.validate_evidence_payload(payload, _options())
    assert kind == "signed_manifest"
    assert any("independently pinned source context" in error for error in errors)
    payload[field] = "software" if field == "signing_backend" else "44" * 32
    assert checker.validate_evidence_payload(payload, _options())[1]


def test_checker_rejects_receipt_metadata_even_after_exact_raw_signature_verification(source_context):
    path, _, _ = source_context
    _, errors = checker.validate_evidence_payload(_forged_canary(), _options(path, _digest(path.read_bytes())))
    assert any("ReleaseManifest receipt" in error for error in errors)


def test_builder_cannot_emit_a_canary_with_only_raw_signature_sources(source_context, capsys):
    path, _, _ = source_context
    output = path.parent / "canary.json"
    assert builder.main([
        "--kind", "signed_manifest", "--out", str(output),
        "--deployment-id", "release-sdk-primary", "--environment", "production",
        "--generated-at-unix", "1800699999", "--now-unix", "1800700000",
        "--signed-manifest-source-context", str(path),
        "--signed-manifest-source-context-sha256", _digest(path.read_bytes()),
    ]) == 2
    assert "ReleaseManifest receipt" in capsys.readouterr().err
    assert not output.exists()


def test_checker_rejects_an_authenticator_return_without_a_typed_hardware_result(monkeypatch):
    monkeypatch.setattr(checker, "authenticate_signed_manifest_sources", lambda *_args: None)
    _, errors = checker.validate_evidence_payload(_forged_canary(), _options())
    assert "signed-manifest source authenticator returned without a verified hardware receipt" in errors


def test_builder_rejects_an_authenticator_return_without_a_typed_hardware_result(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(builder, "authenticate_signed_manifest_sources", lambda *_args: None)
    output = tmp_path / "canary.json"
    assert builder.main([
        "--kind", "signed_manifest", "--out", str(output),
        "--deployment-id", "release-sdk-primary", "--environment", "production",
        "--generated-at-unix", "1800699999", "--now-unix", "1800700000",
    ]) == 2
    assert "without a verified hardware receipt" in capsys.readouterr().err
    assert not output.exists()


@pytest.mark.parametrize("flag", ("--manifest-digest-hex", "--signing-provider", "--signing-backend", "--signing-provider-revision", "--signature-algorithm"))
def test_builder_retires_caller_asserted_signing_inputs(tmp_path, flag, capsys):
    output = tmp_path / "canary.json"
    assert builder.main([
        "--kind", "signed_manifest", "--out", str(output),
        "--deployment-id", "release-sdk-primary", "--environment", "production",
        "--generated-at-unix", "1800699999", "--now-unix", "1800700000", flag, "claim",
    ]) == 2
    assert "unrecognized arguments" in capsys.readouterr().err
    assert not output.exists()
