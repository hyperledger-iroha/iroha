"""Unit tests for the pinned cosign boundary; mocked outcomes are not cryptographic evidence."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import sys

import pytest

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))
import sorafs_final_promotion_cosign as module


def bundle_payload():
    """Load public upstream proof bytes; mocked unit calls perform no cryptography."""
    fixture = SCRIPT_DIR.parent / "fixtures/sorafs/final_promotion_cosign/bundle.sigstore.json"
    return json.loads(fixture.read_bytes())


@pytest.fixture
def inputs(tmp_path):
    root = tmp_path.resolve(strict=True)
    executable, trust = root / "reviewed-cosign", root / "reviewed-root.json"
    executable.write_bytes(b"synthetic executable never executed")
    executable.chmod(0o500)
    trust.write_bytes(b"public unit trust material")
    trust.chmod(0o400)
    args = argparse.Namespace(
        provenance_cosign_verifier=executable,
        provenance_cosign_verifier_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
        provenance_cosign_trusted_root=trust,
        provenance_cosign_trusted_root_sha256=hashlib.sha256(trust.read_bytes()).hexdigest(),
        provenance_certificate_identity="https://github.com/example/repo/.github/workflows/release.yml@refs/heads/main",
        provenance_oidc_issuer="https://token.actions.githubusercontent.com",
    )
    return args, b"unit exact subject bytes", json.dumps(bundle_payload()).encode()


def install_verifier(monkeypatch, result=b""):
    calls = []
    def run(command, root, *, max_stdout_bytes, expected_stderr):
        assert max_stdout_bytes == 0
        assert expected_stderr == b"Verified OK\n"
        calls.append((command, root, {name: (root / name).read_bytes() for name in ("subject", "bundle", "trusted-root")}))
        return result
    monkeypatch.setattr(module.verifier_process, "run_verifier", run)
    return calls


def test_exact_pinned_private_inputs_flags_and_cleanup(inputs, monkeypatch):
    args, subject, bundle = inputs
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == []
    assert len(calls) == 1
    command, root, captured = calls[0]
    assert command == [
        str(root / "cosign"), "verify-blob", "--bundle", str(root / "bundle"),
        "--trusted-root", str(root / "trusted-root"),
        "--certificate-identity", args.provenance_certificate_identity,
        "--certificate-oidc-issuer", args.provenance_oidc_issuer,
        "--use-signed-timestamps", str(root / "subject"),
    ]
    assert captured == {"subject": subject, "bundle": bundle, "trusted-root": args.provenance_cosign_trusted_root.read_bytes()}
    assert not root.exists()


@pytest.mark.parametrize("field", ("provenance_cosign_verifier_sha256", "provenance_cosign_trusted_root_sha256"))
@pytest.mark.parametrize("pin", (None, "", "ab", "00" * 32, "AB" * 32, "ab" * 32, True))
def test_invalid_or_wrong_independent_pin_never_executes(inputs, monkeypatch, field, pin):
    args, subject, bundle = inputs
    setattr(args, field, pin)
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("field", ("provenance_certificate_identity", "provenance_oidc_issuer"))
@pytest.mark.parametrize("identity", (None, "", "http://example.com", "https://user:secret@example.com", "https://localhost", "https://127.0.0.1", "https://example.com/#secret", "https://example.com/\nsecret"))
def test_invalid_independent_identity_never_executes(inputs, monkeypatch, field, identity):
    args, subject, bundle = inputs
    setattr(args, field, identity)
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("subject", (None, "subject", b"", bytearray(b"mutable"), memoryview(b"mutable"), b"x" * (module.MAX_SUBJECT_BYTES + 1)))
def test_subject_must_be_immutable_nonempty_bounded_bytes(inputs, monkeypatch, subject):
    args, _, bundle = inputs
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("raw", (None, bytearray(b"{}"), b"", b"[]", b"null", b"{}", b'{"mediaType":1,"mediaType":2}', b'{"invalid":NaN}', b"x" * (module.MAX_BUNDLE_BYTES + 1)))
def test_invalid_strict_bundle_never_executes(inputs, monkeypatch, raw):
    args, subject, _ = inputs
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, raw) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("mutation", (
    "old_bundle", "old_media_type", "dsse", "extra_root", "missing_signature",
    "managed_key", "certificate_chain", "extra_certificate", "missing_log", "extra_log",
    "old_log", "wrong_kind", "integrated_time", "inclusion_promise", "extra_log_field",
    "missing_timestamp", "empty_timestamp", "wrong_timestamp", "empty_timestamp_list",
    "wrong_timestamp_list", "unknown_timestamp_field", "bad_timestamp", "extra_timestamp_field",
    "extra_material", "extra_signature", "missing_digest", "extra_digest", "wrong_algorithm",
    "short_digest", "bad_digest", "noncanonical_digest", "empty_signature", "bad_signature",
    "bad_certificate", "empty_certificate",
))
def test_alternate_or_malformed_bundle_profiles_rejected_before_execution(inputs, monkeypatch, mutation):
    args, subject, _ = inputs
    payload = bundle_payload()
    material, signature = payload["verificationMaterial"], payload["messageSignature"]
    if mutation == "old_bundle": payload = {"base64Signature": "dW5pdA==", "cert": "unit", "rekorBundle": {}}
    elif mutation == "old_media_type": payload["mediaType"] = "application/vnd.dev.sigstore.bundle+json;version=0.3"
    elif mutation == "dsse": payload["dsseEnvelope"] = payload.pop("messageSignature")
    elif mutation == "extra_root": payload["trustedRoot"] = "candidate-controlled"
    elif mutation == "missing_signature": del payload["messageSignature"]
    elif mutation == "managed_key": material["publicKey"] = material.pop("certificate")
    elif mutation == "certificate_chain": material["x509CertificateChain"] = material.pop("certificate")
    elif mutation == "extra_certificate": material["certificate"]["chain"] = []
    elif mutation == "missing_log": material["tlogEntries"] = []
    elif mutation == "extra_log": material["tlogEntries"] *= 2
    elif mutation == "old_log": material["tlogEntries"][0]["kindVersion"]["version"] = "0.0.1"
    elif mutation == "wrong_kind": material["tlogEntries"][0]["kindVersion"]["kind"] = "intoto"
    elif mutation == "integrated_time": material["tlogEntries"][0]["integratedTime"] = "1"
    elif mutation == "inclusion_promise": material["tlogEntries"][0]["inclusionPromise"] = {}
    elif mutation == "extra_log_field": material["tlogEntries"][0]["unknown"] = "unit"
    elif mutation == "missing_timestamp": del material["timestampVerificationData"]
    elif mutation == "empty_timestamp": material["timestampVerificationData"] = {}
    elif mutation == "wrong_timestamp": material["timestampVerificationData"] = []
    elif mutation == "empty_timestamp_list": material["timestampVerificationData"]["rfc3161Timestamps"] = []
    elif mutation == "wrong_timestamp_list": material["timestampVerificationData"]["rfc3161Timestamps"] = {}
    elif mutation == "unknown_timestamp_field": material["timestampVerificationData"]["unknown"] = "unit"
    elif mutation == "bad_timestamp": material["timestampVerificationData"]["rfc3161Timestamps"][0]["signedTimestamp"] = "!"
    elif mutation == "extra_timestamp_field": material["timestampVerificationData"]["rfc3161Timestamps"][0]["unsignedTime"] = 1
    elif mutation == "extra_material": material["candidateTrust"] = {}
    elif mutation == "extra_signature": signature["algorithm"] = "ed25519"
    elif mutation == "missing_digest": del signature["messageDigest"]
    elif mutation == "extra_digest": signature["messageDigest"]["extra"] = 1
    elif mutation == "wrong_algorithm": signature["messageDigest"]["algorithm"] = "SHA2_512"
    elif mutation == "short_digest": signature["messageDigest"]["digest"] = "dW5pdA=="
    elif mutation == "bad_digest": signature["messageDigest"]["digest"] = "!"
    elif mutation == "noncanonical_digest": signature["messageDigest"]["digest"] = "A" * 42 + "B="
    elif mutation == "empty_signature": signature["signature"] = ""
    elif mutation == "bad_signature": signature["signature"] = "dW5pdA==\n"
    elif mutation == "bad_certificate": material["certificate"]["rawBytes"] = 1
    elif mutation == "empty_certificate": material["certificate"]["rawBytes"] = ""
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, json.dumps(payload).encode()) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("field", ("provenance_cosign_verifier", "provenance_cosign_trusted_root"))
@pytest.mark.parametrize("unsafe", ("symlink", "hardlink", "missing", "oversized"))
def test_unsafe_tool_or_trust_paths_rejected(inputs, monkeypatch, field, unsafe):
    args, subject, bundle = inputs
    path = getattr(args, field)
    if unsafe == "symlink":
        linked = path.with_name(path.name + "-link")
        linked.symlink_to(path)
        setattr(args, field, linked)
    elif unsafe == "hardlink": os.link(path, path.with_name(path.name + "-link"))
    elif unsafe == "missing": path.unlink()
    else:
        path.chmod(0o600)
        maximum = module.MAX_TRUSTED_ROOT_BYTES if field.endswith("trusted_root") else module.verifier_process.MAX_VERIFIER_EXECUTABLE_BYTES
        with path.open("wb") as output: output.truncate(maximum + 1)
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []


def test_source_mutation_after_capture_cannot_replace_private_inputs(inputs, monkeypatch):
    args, subject, bundle = inputs
    original_trust = args.provenance_cosign_trusted_root.read_bytes()
    original_executable = args.provenance_cosign_verifier.read_bytes()
    snapshot = module.verifier_process.snapshot_executable
    def snapshot_then_replace(source, destination, expected):
        snapshot(source, destination, expected)
        for path in (args.provenance_cosign_verifier, args.provenance_cosign_trusted_root):
            path.chmod(0o600)
            path.write_bytes(b"changed original source")
        assert destination.read_bytes() == original_executable
    monkeypatch.setattr(module.verifier_process, "snapshot_executable", snapshot_then_replace)
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == []
    assert calls[0][2] == {"subject": subject, "bundle": bundle, "trusted-root": original_trust}


@pytest.mark.parametrize("result", (b"Verified OK\n", b"secret diagnostic", None))
def test_unexpected_stdout_is_fixed_payload_free_failure(inputs, monkeypatch, result):
    args, subject, bundle = inputs
    calls = install_verifier(monkeypatch, result)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert len(calls) == 1
    assert not calls[0][1].exists()


@pytest.mark.parametrize("failure", (OSError, ValueError, RuntimeError))
def test_verifier_failure_is_payload_free_and_cleans_private_copy(inputs, monkeypatch, failure):
    args, subject, bundle = inputs
    roots = []
    def reject(command, root, *, max_stdout_bytes, expected_stderr):
        roots.append(root)
        raise failure("private path, certificate contents and secret diagnostic")
    monkeypatch.setattr(module.verifier_process, "run_verifier", reject)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert not roots[0].exists()


def test_unresolved_private_directory_never_executes(inputs, monkeypatch):
    args, subject, bundle = inputs
    calls = install_verifier(monkeypatch)
    monkeypatch.setattr(module, "resolve_path_identity", lambda path, errors: None)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("value", (None, 0, True, "", "-1", "+1", "01", " 1", "1 ", "1.0", "١", str(1 << 63), "9" * 100))
def test_log_integer_rejects_noncanonical_or_out_of_range_values(value):
    with pytest.raises(ValueError):
        module._canonical_log_integer(value)


@pytest.mark.parametrize("value", ("0", "1", str((1 << 63) - 1)))
def test_log_integer_accepts_only_canonical_int64_range(value):
    assert module._canonical_log_integer(value) == int(value)


@pytest.mark.parametrize("mutation", (
    "proof_index", "proof_size", "proof_root", "leaf_outside_tree", "empty_tree",
    "extra_proof", "extra_checkpoint", "old_checkpoint_origin", "checkpoint_size",
    "checkpoint_root", "checkpoint_crlf", "short_hash", "too_many_hashes", "wrong_hashes",
    "bad_log_id", "unknown_log_id", "body_digest", "body_signature", "body_certificate",
    "body_key_details", "body_unknown", "body_noncanonical", "body_encoding", "extra_timestamp",
))
def test_duplicate_rekor_fields_must_match_the_consumed_signed_material(inputs, monkeypatch, mutation):
    args, subject, _ = inputs
    payload = bundle_payload()
    material = payload["verificationMaterial"]
    entry = material["tlogEntries"][0]
    proof = entry["inclusionProof"]
    if mutation == "proof_index": proof["logIndex"] = str(int(entry["logIndex"]) + 1)
    elif mutation == "proof_size": proof["treeSize"] = str(int(proof["treeSize"]) + 1)
    elif mutation == "proof_root": proof["rootHash"] = base64.b64encode(bytes(32)).decode()
    elif mutation == "leaf_outside_tree": entry["logIndex"] = proof["logIndex"] = proof["treeSize"]
    elif mutation == "empty_tree": proof["treeSize"] = "0"
    elif mutation == "extra_proof": proof["ignored"] = 1
    elif mutation == "extra_checkpoint": proof["checkpoint"]["ignored"] = 1
    elif mutation == "old_checkpoint_origin":
        lines = proof["checkpoint"]["envelope"].split("\n")
        lines[0] += " - 1234"
        proof["checkpoint"]["envelope"] = "\n".join(lines)
    elif mutation == "checkpoint_size":
        lines = proof["checkpoint"]["envelope"].split("\n")
        lines[1] = str(int(lines[1]) + 1)
        proof["checkpoint"]["envelope"] = "\n".join(lines)
    elif mutation == "checkpoint_root":
        lines = proof["checkpoint"]["envelope"].split("\n")
        lines[2] = base64.b64encode(bytes(32)).decode()
        proof["checkpoint"]["envelope"] = "\n".join(lines)
    elif mutation == "checkpoint_crlf": proof["checkpoint"]["envelope"] = proof["checkpoint"]["envelope"].replace("\n", "\r\n")
    elif mutation == "short_hash": proof["hashes"][0] = "dW5pdA=="
    elif mutation == "too_many_hashes": proof["hashes"] = [proof["rootHash"]] * 64
    elif mutation == "wrong_hashes": proof["hashes"] = {}
    elif mutation == "bad_log_id": entry["logId"]["keyId"] = "dW5pdA=="
    elif mutation == "unknown_log_id": entry["logId"]["ignored"] = 1
    elif mutation == "extra_timestamp": material["timestampVerificationData"]["rfc3161Timestamps"] *= 2
    else:
        raw = base64.b64decode(entry["canonicalizedBody"])
        body = json.loads(raw)
        rekord = body["spec"]["hashedRekordV002"]
        if mutation == "body_digest": rekord["data"]["digest"] = base64.b64encode(bytes(32)).decode()
        elif mutation == "body_signature": rekord["signature"]["content"] = "dW5pdA=="
        elif mutation == "body_certificate": rekord["signature"]["verifier"]["x509Certificate"]["rawBytes"] = "dW5pdA=="
        elif mutation == "body_key_details": rekord["signature"]["verifier"]["keyDetails"] = "PKIX_ECDSA_P384_SHA_384"
        elif mutation == "body_unknown": body["ignored"] = 1
        raw = json.dumps(body, sort_keys=True, separators=(",", ":")).encode()
        if mutation == "body_noncanonical": raw += b"\n"
        entry["canonicalizedBody"] = base64.b64encode(raw).decode()
        if mutation == "body_encoding": entry["canonicalizedBody"] += "\n"
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, json.dumps(payload).encode()) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("version", (3, 127))
def test_unsupported_der_version_is_fixed_preexecution_failure(inputs, monkeypatch, version):
    args, subject, _ = inputs
    payload = bundle_payload()
    certificate = payload["verificationMaterial"]["certificate"]
    raw = base64.b64decode(certificate["rawBytes"])
    original = b"\xa0\x03\x02\x01\x02"
    assert original in raw
    raw = raw.replace(original, original[:-1] + bytes([version]), 1)
    certificate["rawBytes"] = base64.b64encode(raw).decode()
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, json.dumps(payload).encode()) == [module.FAILURE]
    assert calls == []


@pytest.mark.parametrize("profile", ("ed25519", "p384"))
def test_actual_non_p256_leaf_certificate_is_rejected_before_body_or_tool(inputs, monkeypatch, profile):
    from datetime import datetime, timezone
    from cryptography.hazmat.primitives import hashes
    from cryptography.hazmat.primitives.asymmetric import ed25519
    args, subject, _ = inputs
    if profile == "ed25519":
        key = ed25519.Ed25519PrivateKey.from_private_bytes(bytes([1]) * 32)
        algorithm = None
    else:
        key = module.ec.derive_private_key(1, module.ec.SECP384R1())
        algorithm = hashes.SHA256()
    name = module.x509.Name([module.x509.NameAttribute(module.x509.NameOID.COMMON_NAME, "public unit certificate")])
    certificate = (
        module.x509.CertificateBuilder().subject_name(name).issuer_name(name)
        .public_key(key.public_key()).serial_number(1)
        .not_valid_before(datetime(2025, 1, 1, tzinfo=timezone.utc))
        .not_valid_after(datetime(2025, 1, 2, tzinfo=timezone.utc)).sign(key, algorithm)
    )
    payload = bundle_payload()
    payload["verificationMaterial"]["certificate"]["rawBytes"] = base64.b64encode(certificate.public_bytes(module.Encoding.DER)).decode()
    raw = json.dumps(payload).encode()
    with pytest.raises(ValueError, match="invalid cosign P256 leaf certificate"):
        module.validate_cosign_bundle(raw)
    calls = install_verifier(monkeypatch)
    assert module.verify_final_promotion_cosign(args, subject, raw) == [module.FAILURE]
    assert calls == []


def test_unsupported_certificate_algorithm_is_fixed_preexecution_failure(inputs, monkeypatch):
    args, subject, bundle = inputs
    calls = install_verifier(monkeypatch)
    def unsupported(raw):
        raise module.UnsupportedAlgorithm("untrusted curve diagnostics")
    monkeypatch.setattr(module.x509, "load_der_x509_certificate", unsupported)
    assert module.verify_final_promotion_cosign(args, subject, bundle) == [module.FAILURE]
    assert calls == []
