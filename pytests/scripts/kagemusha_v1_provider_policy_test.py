"""Provider-policy derivation and provenance tests; synthetic data grants no hardware authority."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
from pathlib import Path

import pytest

from kagemusha_v1_release_evidence_test import PHYSICAL_TEST, VERIFIER, _fixture, _provider_policy_signature, _verify_direct


SIGNING_PREIMAGE_HEX = (
    "4e5254300000779107008bd31fb01e66d644a082ad5e0081000000000000003f5f967f6566f08a02"
    "38300000000000000069726f68613a6b6167656d757368613a76313a70726f76696465722d706f6c6963792d617574686f72697a6174696f6e"
    "020100204141414141414141414141414141414141414141414141414141414141414141"
    "0231a520d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1"
)


def test_provider_signing_preimage_matches_rust_golden_and_binds_all_fields() -> None:
    message = VERIFIER.rust_provider_policy_signing_bytes("41" * 32, 0xA531, "d1" * 32)
    assert message == bytes.fromhex(SIGNING_PREIMAGE_HEX)
    for profile, position, authority in [("42" * 32, 0xA531, "d1" * 32),
                                          ("41" * 32, 0xA530, "d1" * 32),
                                          ("41" * 32, 0xA531, "d2" * 32)]:
        assert VERIFIER.rust_provider_policy_signing_bytes(profile, position, authority) != message


def _der_signature(r: int, s: int) -> bytes:
    values = []
    for scalar in (r, s):
        encoded = scalar.to_bytes(33, "big").lstrip(b"\0")
        if encoded[0] & 0x80:
            encoded = b"\0" + encoded
        values.append(b"\x02" + bytes([len(encoded)]) + encoded)
    body = b"".join(values)
    return b"\x30" + bytes([len(body)]) + body


def test_provider_p256_verification_and_test_signatures_match_openssl(tmp_path: Path) -> None:
    """OpenSSL is an independent test oracle; the projector imports only stdlib."""
    message = bytes.fromhex(SIGNING_PREIMAGE_HEX)
    scalar = (1).to_bytes(32, "big")  # Publicly known, synthetic fixture issuer only.
    key = tmp_path / "synthetic-issuer.der"
    key.write_bytes(b"\x30\x31\x02\x01\x01\x04\x20" + scalar + bytes.fromhex("a00a06082a8648ce3d030107"))
    key.chmod(0o600)
    public = tmp_path / "issuer.pem"
    subprocess.run(["openssl", "pkey", "-inform", "DER", "-in", str(key), "-pubout", "-out", str(public)],
                   capture_output=True, check=True)
    signed = subprocess.run(["openssl", "dgst", "-sha256", "-sign", str(key), "-keyform", "DER"],
                            input=message, capture_output=True, check=True).stdout
    assert signed[0] == 0x30 and signed[1] == len(signed) - 2
    offset, integers = 2, []
    for _ in range(2):
        assert signed[offset] == 2
        length = signed[offset + 1]
        integers.append(int.from_bytes(signed[offset + 2:offset + 2 + length], "big"))
        offset += 2 + length
    assert offset == len(signed)
    r, s = integers
    raw = r.to_bytes(32, "big") + min(s, VERIFIER._P256_ORDER - s).to_bytes(32, "big")
    public_bytes = bytes.fromhex(golden_profile()["governance_credential_public_key"])
    assert VERIFIER._p256_verify(public_bytes, message, raw)
    synthetic = bytes.fromhex(_provider_policy_signature("41" * 32, 0xA531, "d1" * 32))
    signature_path = tmp_path / "signature.der"
    signature_path.write_bytes(_der_signature(int.from_bytes(synthetic[:32], "big"), int.from_bytes(synthetic[32:], "big")))
    subprocess.run(["openssl", "dgst", "-sha256", "-verify", str(public), "-signature", str(signature_path)],
                   input=message, capture_output=True, check=True)


def test_provider_p256_rejects_scalar_point_message_and_infinity_attacks() -> None:
    message = bytes.fromhex(SIGNING_PREIMAGE_HEX)
    public = bytes.fromhex(golden_profile()["governance_credential_public_key"])
    signature = bytes.fromhex(_provider_policy_signature("41" * 32, 0xA531, "d1" * 32))
    assert VERIFIER._p256_verify(public, message, signature)
    r, s = int.from_bytes(signature[:32], "big"), int.from_bytes(signature[32:], "big")
    order = VERIFIER._P256_ORDER
    for bad_r, bad_s in [(0, s), (order, s), (order + 1, s), (r, 0), (r, order), (r, order - s)]:
        assert not VERIFIER._p256_verify(public, message, bad_r.to_bytes(32, "big") + bad_s.to_bytes(32, "big"))
    for invalid in [b"", signature[:-1], signature + b"\0"]:
        assert not VERIFIER._p256_verify(public, message, invalid)
    for invalid in [b"", public[:-1], b"\x00" + public[1:], b"\x04" + b"\0" * 64,
                    b"\x04" + VERIFIER._P256_PRIME.to_bytes(32, "big") + public[33:]]:
        assert not VERIFIER._p256_verify(invalid, message, signature)
    assert not VERIFIER._p256_verify(public, message + b"\0", signature)
    wrong_point = VERIFIER._p256_multiply(2, VERIFIER._P256_GENERATOR)
    wrong_key = b"\x04" + b"".join(value.to_bytes(32, "big") for value in wrong_point)
    assert not VERIFIER._p256_verify(wrong_key, message, signature)
    # With r=s=1 and Q=-SHA256(message)*G, the verifier's sum is infinity.
    inverse_point = VERIFIER._p256_multiply((-int.from_bytes(hashlib.sha256(message).digest(), "big")) % order,
                                           VERIFIER._P256_GENERATOR)
    infinity_key = b"\x04" + b"".join(value.to_bytes(32, "big") for value in inverse_point)
    assert not VERIFIER._p256_verify(infinity_key, message, (1).to_bytes(32, "big") * 2)


def golden_profile() -> dict:
    profile = {
        "version": 1, "protocol_version": 1, "hardware_profile_id": "0" * 64,
        "provider_id": "41" * 32, "platform_class": "dedicated_secure_element",
        "product_class_digest": "42" * 32, "firmware_policy_digest": "43" * 32,
        "enrollment_attestation_verifier_digest": "44" * 32,
        "attestation_trust_roots_digest": "45" * 32,
        "allowed_suite_commitment": VERIFIER._suite_commitment("51" * 32),
        "policy_epoch": 65,
        "governance_credential_public_key": (
            "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
            "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        ),
        "capability_mask": 65535, "qualification_report_digest": "61" * 32,
        "valid_from_ms": 1, "expires_at_ms": 100000,
    }
    profile["hardware_profile_id"] = VERIFIER.rust_hardware_profile_id(profile)
    return profile


def entry(profile: dict, position: int = 0xA531) -> dict:
    return {
        "hardware_profile_id": profile["hardware_profile_id"],
        "provider_authority_commitment": "d1" * 32,
        "provider_profile_index": position,
        "issuer_signature": _provider_policy_signature(profile["hardware_profile_id"], position, "d1" * 32),
    }


def test_provider_root_matches_rust_golden_and_dense_reference() -> None:
    profile = golden_profile()
    row = entry(profile)
    root = VERIFIER.rust_provider_policy_root([profile], [row])
    # Deliberately use a full reference tree to check sparse index/padding logic.
    nodes = [hashlib.sha256(b"iroha:kagemusha:v1:hardware-policy-empty\0").digest()] * 65536
    nodes[row["provider_profile_index"]] = hashlib.sha256(
        b"iroha:kagemusha:v1:hardware-policy-leaf\0"
        + bytes.fromhex(profile["hardware_profile_id"]) + b"\x02\xff\xff"
        + bytes.fromhex(row["provider_authority_commitment"])
    ).digest()
    for _ in range(16):
        nodes = [hashlib.sha256(b"iroha:kagemusha:v1:hardware-policy-node\0" + left + right).digest()
                 for left, right in zip(nodes[::2], nodes[1::2])]
    assert root == nodes[0].hex()
    assert root == "01e5b53f36db41dcd2f9db725171d6405005ca6ad5374b23b3cbce6e1efdc100"


@pytest.mark.parametrize("field,value", [
    ("hardware_profile_id", "ef" * 32),
    ("provider_authority_commitment", "0" * 64),
    ("provider_authority_commitment", True),
    ("provider_profile_index", -1), ("provider_profile_index", 65536),
    ("provider_profile_index", True), ("provider_profile_index", 1.0),
    ("issuer_signature", "01" * 64), ("issuer_signature", "00" * 64),
    ("issuer_signature", True), ("issuer_signature", "01" * 63),
    ("provider_authority_secret", "bb" * 32), ("policy_siblings", ["cc" * 32] * 16),
])
def test_provider_policy_rejects_invalid_or_private_inputs(field: str, value: object) -> None:
    profile = golden_profile()
    row = entry(profile)
    row[field] = value
    with pytest.raises(VERIFIER.KagemushaEvidenceError):
        VERIFIER.rust_provider_policy_root([profile], [row])


def test_provider_policy_requires_complete_sorted_unique_inventory() -> None:
    first = golden_profile()
    second = copy.deepcopy(first)
    second["provider_id"] = "42" * 32
    second["hardware_profile_id"] = VERIFIER.rust_hardware_profile_id(second)
    profiles = sorted([first, second], key=lambda item: item["hardware_profile_id"])
    rows = [entry(profiles[0], 0), entry(profiles[1], 65535)]
    root = VERIFIER.rust_provider_policy_root(profiles, rows)
    assert len(root) == 64
    invalid = [[], rows[:1], rows + rows[:1], list(reversed(rows)), [rows[0], rows[0]]]
    repeated_position = copy.deepcopy(rows)
    repeated_position[1]["provider_profile_index"] = 0
    invalid.append(repeated_position)
    for candidate in invalid:
        with pytest.raises(VERIFIER.KagemushaEvidenceError):
            VERIFIER.rust_provider_policy_root(profiles, candidate)
    with pytest.raises(VERIFIER.KagemushaEvidenceError):
        VERIFIER.rust_provider_policy_root(profiles * 33, rows * 33)


@pytest.mark.parametrize("platform", VERIFIER._PLATFORM_CLASSES)
def test_provider_policy_admits_each_governed_platform_and_binds_class(platform: str) -> None:
    profile = golden_profile()
    original = VERIFIER.rust_provider_policy_root([profile], [entry(profile)])
    profile["platform_class"] = platform
    profile["hardware_profile_id"] = VERIFIER.rust_hardware_profile_id(profile)
    changed = VERIFIER.rust_provider_policy_root([profile], [entry(profile)])
    assert (changed == original) == (platform == "dedicated_secure_element")


def test_provider_commitment_and_index_bind_candidate_context(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    before = fixture.candidate_context_digest()
    row = fixture.manifest["profiles"][0]["provider_policy"]
    row["provider_authority_commitment"] = "ab" * 32
    assert fixture.candidate_context_digest() != before
    row["provider_authority_commitment"] = "d1" * 32
    row["provider_profile_index"] ^= 1
    assert fixture.candidate_context_digest() != before


@pytest.mark.parametrize("identity", [[], {}])
def test_release_rejects_non_scalar_provider_id_before_indexing(
    tmp_path: Path, identity: object
) -> None:
    fixture = _fixture(tmp_path)
    fixture.manifest["profiles"][0]["provider_policy"]["hardware_profile_id"] = identity
    fixture.write_manifest()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="hexadecimal"):
        _verify_direct(fixture)


def test_signed_physical_transcript_cannot_choose_an_unapproved_policy_root(tmp_path: Path) -> None:
    fixture = _fixture(tmp_path)
    paths = fixture.manifest["profiles"][0]["physical_evidence"]
    path = paths["transcript"]
    document = json.loads(fixture.path(path).read_text())
    sender_context = next(event["data"] for event in document["events"]
                          if event["kind"] == "sender_validity_context")
    assert sender_context["hardware_policy_id"] == document["profile"]["hardware_policy_id"]
    assert sender_context["hardware_policy_id"] != "ae" * 32
    document["profile"]["hardware_policy_id"] = "ae" * 32
    document["endpoint"]["hardware_policy_id"] = "ae" * 32
    policy = VERIFIER._load_observer_policy(fixture.observer_policy_path, fixture.observer_policy_sha256)
    PHYSICAL_TEST._TranscriptBuilder(
        policy, {fixture.observer_authority_id: fixture.observer_seed}
    ).approve(document)
    fixture.write(path, VERIFIER.canonical_json_bytes(document), fixture.kinds[path])
    fixture.resign_commands_for_file(path)
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError,
                       match="physical transcript rejected: sender context substitutes the authenticated device or policy"):
        _verify_direct(fixture)


@pytest.mark.parametrize("field,value", [
    ("provider_authority_commitment", "ab" * 32), ("provider_profile_index", 0),
    ("provider_profile_index", True),
])
def test_signed_oem_report_cannot_substitute_provider_authority(
    tmp_path: Path, field: str, value: object
) -> None:
    fixture = _fixture(tmp_path)
    path = fixture.manifest["profiles"][0]["physical_evidence"]["oem_report"]
    report = json.loads(fixture.path(path).read_text())
    report[field] = value
    fixture.write(path, VERIFIER.canonical_json_bytes(report), fixture.kinds[path])
    fixture.resign_commands_for_file(path)
    fixture.refresh_files()
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="OEM attestation report substitutes"):
        _verify_direct(fixture)


def _refresh_signed_physical_candidate(fixture) -> None:
    paths = fixture.manifest["profiles"][0]["physical_evidence"]
    document = json.loads(fixture.path(paths["transcript"]).read_text())
    document["run"]["candidate_context_digest"] = fixture.candidate_context_digest()
    policy = VERIFIER._load_observer_policy(fixture.observer_policy_path, fixture.observer_policy_sha256)
    sender_context = next(event["data"] for event in document["events"] if event["kind"] == "sender_validity_context")
    sender_context["candidate_context_digest"] = document["run"]["candidate_context_digest"]
    PHYSICAL_TEST._TranscriptBuilder(policy, {fixture.observer_authority_id: fixture.observer_seed}).rechain_sender(document)
    fixture.write(paths["transcript"], VERIFIER.canonical_json_bytes(document), "physical_transcript")
    report = json.loads(fixture.path(paths["oem_report"]).read_text())
    report["candidate_context_digest"] = document["run"]["candidate_context_digest"]
    report["challenge_sha256"] = VERIFIER.physical_oem_challenge(document["profile"]["hardware_profile_id"],
                                                                document["endpoint"], document["run"])
    payload = fixture.path(paths["transcript"]).read_bytes()
    report["transcript"] = {"sha256": hashlib.sha256(payload).hexdigest(), "byte_len": len(payload)}
    fixture.write(paths["oem_report"], VERIFIER.canonical_json_bytes(report), "report")
    fixture.resign_all_for_candidate_context()
    fixture.refresh_files()


def test_consistent_provider_substitution_requires_issuer_approval_despite_fresh_observers(tmp_path: Path) -> None:
    # The changed row, derived root, physical transcript and OEM report already
    # agree. Only the issuer's signature is replaced with approval for old D1.
    fixture = _fixture(tmp_path, provider_commitment="ab" * 32)
    _verify_direct(fixture)
    row = fixture.manifest["profiles"][0]["provider_policy"]
    approved = row["issuer_signature"]
    row["issuer_signature"] = _provider_policy_signature(row["hardware_profile_id"], row["provider_profile_index"], "d1" * 32)
    _refresh_signed_physical_candidate(fixture)
    with pytest.raises(VERIFIER.KagemushaEvidenceError, match="governed issuer authorization"):
        _verify_direct(fixture)
    row["issuer_signature"] = approved
    _refresh_signed_physical_candidate(fixture)
    _verify_direct(fixture)
