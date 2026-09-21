"""Actual pinned cosign verification against public upstream conformance evidence.

This exercises cryptography and the adapter process boundary. It does not create
SoraFS promotion evidence; canonical statement construction is tested separately.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import sys
from pathlib import Path

import pytest

SCRIPT_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPT_DIR))
import sorafs_final_promotion_cosign as module

FIXTURES = SCRIPT_DIR.parent / "fixtures/sorafs/final_promotion_cosign"
IDENTITY = (
    "https://github.com/sigstore-conformance/extremely-dangerous-public-oidc-beacon/"
    ".github/workflows/extremely-dangerous-oidc-beacon.yml@refs/heads/main"
)
ISSUER = "https://token.actions.githubusercontent.com"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(document: dict) -> bytes:
    return json.dumps(document, sort_keys=True, separators=(",", ":")).encode("ascii")


def flip(encoded: str) -> str:
    raw = bytearray(base64.b64decode(encoded, validate=True))
    raw[-1] ^= 1
    return base64.b64encode(raw).decode("ascii")


def test_public_upstream_crypto_fixture_integrity():
    sources = json.loads((FIXTURES / "sources.json").read_bytes())
    assert sources["scope"] == "public_upstream_conformance_only"
    assert set(sources["files"]) == {
        "artifact.txt", "bundle.sigstore.json", "trusted_root.json", "timestamp_mismatch.bundle.json",
    }
    for name, expected in sources["files"].items():
        data = (FIXTURES / name).read_bytes()
        assert len(data) == expected["size"]
        assert digest(data) == expected["sha256"]


@pytest.fixture
def crypto_inputs(tmp_path, pytestconfig):
    executable = pytestconfig.getoption("sorafs_cosign_verifier")
    pin = pytestconfig.getoption("sorafs_cosign_verifier_sha256")
    if executable is None and pin is None:
        pytest.skip("requires an explicitly reviewed cosign executable and SHA-256")
    if not executable or not pin:
        pytest.fail("cosign cryptographic qualification requires both independent options")
    trust = tmp_path / "trusted_root.json"
    trust.write_bytes((FIXTURES / "trusted_root.json").read_bytes())
    args = argparse.Namespace(
        provenance_cosign_verifier=Path(executable),
        provenance_cosign_verifier_sha256=pin,
        provenance_cosign_trusted_root=trust,
        provenance_cosign_trusted_root_sha256=digest(trust.read_bytes()),
        provenance_certificate_identity=IDENTITY,
        provenance_oidc_issuer=ISSUER,
    )
    return args, (FIXTURES / "artifact.txt").read_bytes(), (FIXTURES / "bundle.sigstore.json").read_bytes()


def test_real_cosign_accepts_exact_subject_and_independent_trust(crypto_inputs):
    args, subject, bundle = crypto_inputs
    assert module.verify_final_promotion_cosign(args, subject, bundle) == []


def test_real_cosign_rejects_different_subject(crypto_inputs):
    args, subject, bundle = crypto_inputs
    assert module.verify_final_promotion_cosign(args, subject + b"changed", bundle)


@pytest.mark.parametrize("field,value", [
    ("provenance_certificate_identity", IDENTITY.replace("@refs/heads/main", "@refs/heads/untrusted")),
    ("provenance_oidc_issuer", "https://accounts.google.com"),
])
def test_real_cosign_rejects_wrong_independent_identity(crypto_inputs, field, value):
    args, subject, bundle = crypto_inputs
    setattr(args, field, value)
    assert module.verify_final_promotion_cosign(args, subject, bundle)


@pytest.mark.parametrize("mutation", [
    "signature", "certificate", "artifact_digest", "checkpoint", "inclusion_hash",
    "missing_inclusion_proof", "missing_transparency_log", "signed_timestamp",
    "missing_signed_timestamp", "timestamp_payload_mismatch",
    "proof_tree_size", "proof_log_index", "entry_log_index", "proof_hash",
    "checkpoint_tree_size", "checkpoint_root_hash", "body_digest", "body_signature",
    "body_certificate", "body_key_details", "body_noncanonical",
])
def test_real_cosign_rejects_corrupted_cryptographic_evidence(crypto_inputs, mutation):
    args, subject, raw = crypto_inputs
    bundle = json.loads(raw)
    entry = bundle["verificationMaterial"]["tlogEntries"][0]
    if mutation == "signature":
        bundle["messageSignature"]["signature"] = flip(bundle["messageSignature"]["signature"])
    elif mutation == "certificate":
        cert = bundle["verificationMaterial"]["certificate"]
        cert["rawBytes"] = flip(cert["rawBytes"])
    elif mutation == "artifact_digest":
        item = bundle["messageSignature"]["messageDigest"]
        item["digest"] = flip(item["digest"])
    elif mutation == "checkpoint":
        entry["inclusionProof"]["checkpoint"]["envelope"] += "corrupted\n"
    elif mutation == "inclusion_hash":
        proof = entry["inclusionProof"]
        proof["rootHash"] = flip(proof["rootHash"])
    elif mutation == "missing_inclusion_proof":
        del entry["inclusionProof"]
    elif mutation == "missing_transparency_log":
        bundle["verificationMaterial"]["tlogEntries"] = []
    elif mutation == "signed_timestamp":
        timestamp = bundle["verificationMaterial"]["timestampVerificationData"]["rfc3161Timestamps"][0]
        timestamp["signedTimestamp"] = flip(timestamp["signedTimestamp"])
    elif mutation == "missing_signed_timestamp":
        del bundle["verificationMaterial"]["timestampVerificationData"]
    elif mutation == "timestamp_payload_mismatch":
        # The public upstream negative carries a timestamp over different signature bytes.
        other = json.loads((FIXTURES / "timestamp_mismatch.bundle.json").read_bytes())
        bundle["verificationMaterial"]["timestampVerificationData"] = other["verificationMaterial"]["timestampVerificationData"]
    elif mutation in {"proof_tree_size", "proof_log_index"}:
        key = "treeSize" if mutation == "proof_tree_size" else "logIndex"
        entry["inclusionProof"][key] = str(int(entry["inclusionProof"][key]) + 1)
    elif mutation == "entry_log_index":
        entry["logIndex"] = str(int(entry["logIndex"]) + 1)
    elif mutation == "proof_hash":
        entry["inclusionProof"]["hashes"][0] = flip(entry["inclusionProof"]["hashes"][0])
    elif mutation in {"checkpoint_tree_size", "checkpoint_root_hash"}:
        lines = entry["inclusionProof"]["checkpoint"]["envelope"].split("\n")
        if mutation == "checkpoint_tree_size":
            lines[1] = str(int(lines[1]) + 1)
        else:
            lines[2] = flip(lines[2])
        entry["inclusionProof"]["checkpoint"]["envelope"] = "\n".join(lines)
    elif mutation.startswith("body_"):
        body = json.loads(base64.b64decode(entry["canonicalizedBody"], validate=True))
        record = body["spec"]["hashedRekordV002"]
        if mutation == "body_digest":
            record["data"]["digest"] = flip(record["data"]["digest"])
        elif mutation == "body_signature":
            record["signature"]["content"] = flip(record["signature"]["content"])
        elif mutation == "body_certificate":
            cert = record["signature"]["verifier"]["x509Certificate"]
            cert["rawBytes"] = flip(cert["rawBytes"])
        elif mutation == "body_key_details":
            record["signature"]["verifier"]["keyDetails"] = "PKIX_ED25519"
        raw_body = json.dumps(body, indent=2).encode() if mutation == "body_noncanonical" else encode(body)
        entry["canonicalizedBody"] = base64.b64encode(raw_body).decode("ascii")
    assert module.verify_final_promotion_cosign(args, subject, encode(bundle))


@pytest.mark.parametrize("authority", ["certificateAuthorities", "ctlogs", "tlogs", "timestampAuthorities"])
def test_real_cosign_rejects_missing_independent_authority(crypto_inputs, authority):
    args, subject, bundle = crypto_inputs
    root = json.loads(args.provenance_cosign_trusted_root.read_bytes())
    assert root[authority]
    root[authority] = []
    raw = encode(root)
    args.provenance_cosign_trusted_root.write_bytes(raw)
    args.provenance_cosign_trusted_root_sha256 = digest(raw)
    assert module.verify_final_promotion_cosign(args, subject, bundle)
