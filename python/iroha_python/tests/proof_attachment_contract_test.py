from __future__ import annotations

import copy
import hashlib
from typing import Any

import pytest

from iroha_python import Instruction
from iroha_python.crypto import _proof_box_max_proof_bytes_v1


def iroha_hash_bytes(payload: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def attachment(
    *,
    backend: str = "halo2/ipa",
    proof_backend: str | None = None,
    proof_bytes: bytes = b"proof",
    vk_backend: str | None = None,
    vk_name: str = "vk_transfer",
) -> dict[str, Any]:
    return {
        "backend": backend,
        "proof": {
            "backend": proof_backend or backend,
            "bytes": proof_bytes,
        },
        "vk_ref": {
            "backend": vk_backend or backend,
            "name": vk_name,
        },
    }


def lane_privacy(
    *,
    commitment_id: object = 7,
    leaf: object = b"l" * 32,
    leaf_index: object = 1,
    audit_path: object = None,
) -> dict[str, Any]:
    if audit_path is None:
        audit_path = [b"s" * 32]
    return {
        "commitment_id": commitment_id,
        "witness": {
            "kind": "merkle",
            "payload": {
                "leaf": leaf,
                "proof": {
                    "leaf_index": leaf_index,
                    "audit_path": audit_path,
                },
            },
        },
    }


def verify(value: object) -> None:
    Instruction.verify_proof(value)


def test_proof_attachment_accepts_exact_first_release_shape_and_optional_fields() -> None:
    proof = attachment(vk_name="halo2/ipa::transfer_v1")
    verify(proof)

    proof["vk_commitment"] = b"v" * 32
    proof["envelope_hash"] = iroha_hash_bytes(b"proof")
    proof["lane_privacy"] = lane_privacy()
    verify(proof)

    proof["vk_commitment"] = None
    proof["envelope_hash"] = None
    proof["lane_privacy"] = None
    verify(proof)


@pytest.mark.parametrize(
    "retired_field",
    (
        "proof_bytes",
        "proof_b64",
        "proofBytes",
        "proofB64",
        "proofBase64",
        "verifying_key_ref",
        "verifyingKeyRef",
        "vkRef",
        "verifying_key",
        "verifying_key_commitment",
        "verifyingKeyCommitment",
        "vkCommitment",
        "envelopeHash",
        "vk_inline",
        "vkInline",
        "verifyingKeyInline",
        "verifying_key_inline",
    ),
)
def test_proof_attachment_rejects_every_retired_alias(retired_field: str) -> None:
    proof = attachment()
    proof[retired_field] = b"retired"
    with pytest.raises(ValueError, match="unknown first-release field"):
        verify(proof)


@pytest.mark.parametrize(
    ("container", "retired_field"),
    (
        ("proof", "bytes_b64"),
        ("proof", "vk_inline"),
        ("proof", "verifyingKeyInline"),
        ("vk_ref", "vk_inline"),
        ("vk_ref", "verifying_key_inline"),
        ("vk_ref", "id"),
        ("vk_ref", "key"),
        ("vk_ref", "backendId"),
    ),
)
def test_proof_attachment_rejects_nested_retired_alias(
    container: str,
    retired_field: str,
) -> None:
    proof = attachment()
    proof[container][retired_field] = b"retired"
    with pytest.raises(ValueError, match="unknown first-release field"):
        verify(proof)


@pytest.mark.parametrize(
    ("mutate", "error_type", "message"),
    (
        (lambda value: value.update({"shadow": 1}), ValueError, "unknown"),
        (
            lambda value: value["proof"].update({"shadow": 1}),
            ValueError,
            "unknown",
        ),
        (
            lambda value: value["vk_ref"].update({"shadow": 1}),
            ValueError,
            "unknown",
        ),
        (lambda value: value.update({1: "shadow"}), TypeError, "field names"),
        (lambda value: value.update({"proof": b"proof"}), TypeError, "mapping"),
        (lambda value: value.update({"vk_ref": "halo2/ipa:vk"}), TypeError, "mapping"),
        (
            lambda value: value["proof"].update({"bytes": bytearray(b"proof")}),
            TypeError,
            "must be bytes",
        ),
        (
            lambda value: value["proof"].update({"bytes": [1, 2, 3]}),
            TypeError,
            "must be bytes",
        ),
    ),
)
def test_proof_attachment_rejects_noncanonical_shape_and_types(
    mutate: Any,
    error_type: type[Exception],
    message: str,
) -> None:
    proof = attachment()
    mutate(proof)
    with pytest.raises(error_type, match=message):
        verify(proof)


@pytest.mark.parametrize(
    "field",
    ("backend", "proof", "vk_ref"),
)
def test_proof_attachment_rejects_missing_required_outer_field(field: str) -> None:
    proof = attachment()
    del proof[field]
    with pytest.raises(ValueError, match=field):
        verify(proof)


@pytest.mark.parametrize(
    "invalid",
    (
        "",
        " ",
        " halo2/ipa",
        "halo2/ipa ",
        "Halo2/ipa",
        "halo2/IPA",
        "halo2/ipa\nforged",
        "halo2/ipa\u200b",
        "halo2/ipa/../vk",
        "halo2/ipa/./vk",
        "halo2//ipa",
        "halo2\\ipa",
        ".halo2",
        "halo2_",
        "halo2/:ipa",
        "a" * 257,
    ),
)
def test_proof_attachment_rejects_nonportable_backend(invalid: str) -> None:
    proof = attachment()
    proof["backend"] = invalid
    with pytest.raises(ValueError, match="portable"):
        verify(proof)


@pytest.mark.parametrize(
    "invalid",
    (
        "",
        " ",
        " vk_transfer",
        "vk_transfer ",
        "VkTransfer",
        "vk\nforged",
        "vk\u200btransfer",
        "vk/../transfer",
        "vk/./transfer",
        "vk\\transfer",
        "-vk_transfer",
        "vk_transfer_",
        "a" * 257,
    ),
)
def test_proof_attachment_rejects_nonportable_verifying_key_name(invalid: str) -> None:
    proof = attachment(vk_name=invalid)
    with pytest.raises(ValueError, match="portable"):
        verify(proof)


def test_proof_attachment_accepts_maximum_portable_identifier_length() -> None:
    verify(attachment(backend="a" * 256, vk_name="v" * 256))


@pytest.mark.parametrize(
    ("proof_backend", "vk_backend", "message"),
    (
        ("stark/fri", None, "proof.backend"),
        (None, "stark/fri", "vk_ref.backend"),
    ),
)
def test_proof_attachment_rejects_backend_mismatch(
    proof_backend: str | None,
    vk_backend: str | None,
    message: str,
) -> None:
    proof = attachment(proof_backend=proof_backend, vk_backend=vk_backend)
    with pytest.raises(ValueError, match=message):
        verify(proof)


@pytest.mark.parametrize(
    ("container", "field"),
    (
        ("proof", "backend"),
        ("proof", "bytes"),
        ("vk_ref", "backend"),
        ("vk_ref", "name"),
    ),
)
def test_proof_attachment_rejects_missing_nested_required_field(
    container: str,
    field: str,
) -> None:
    proof = attachment()
    del proof[container][field]
    with pytest.raises(ValueError, match=field):
        verify(proof)


@pytest.mark.parametrize(
    ("container", "field"),
    (
        ("proof", "backend"),
        ("vk_ref", "backend"),
        ("vk_ref", "name"),
    ),
)
def test_proof_attachment_rejects_nonportable_nested_identifier(
    container: str,
    field: str,
) -> None:
    proof = attachment()
    proof[container][field] = "forged/../selector"
    with pytest.raises(ValueError, match="portable"):
        verify(proof)


def test_proof_attachment_rejects_empty_proof() -> None:
    with pytest.raises(ValueError, match="proof.bytes must be non-empty"):
        verify(attachment(proof_bytes=b""))


def test_proof_attachment_enforces_exact_encoded_proof_box_cap() -> None:
    backend = "a"
    maximum = _proof_box_max_proof_bytes_v1(backend)
    verify(attachment(backend=backend, proof_bytes=b"p" * maximum, vk_name="v"))
    with pytest.raises(ValueError, match=f"{maximum}-byte limit"):
        verify(attachment(backend=backend, proof_bytes=b"p" * (maximum + 1), vk_name="v"))


@pytest.mark.parametrize(
    ("field", "value", "message"),
    (
        ("vk_commitment", b"v" * 31, "must be 32 bytes"),
        ("vk_commitment", b"\x00" * 32, "must be non-zero"),
        ("vk_commitment", "76" * 32, "must be bytes"),
        ("envelope_hash", b"h" * 31, "must be 32 bytes"),
        ("envelope_hash", b"\x00" * 32, "must be non-zero"),
        ("envelope_hash", b"h" * 32, "must match proof bytes"),
        ("envelope_hash", iroha_hash_bytes(b"other"), "must match proof bytes"),
        ("envelope_hash", iroha_hash_bytes(b"proof").hex(), "must be bytes"),
    ),
)
def test_proof_attachment_rejects_invalid_optional_hashes(
    field: str,
    value: object,
    message: str,
) -> None:
    proof = attachment()
    proof[field] = value
    with pytest.raises((TypeError, ValueError), match=message):
        verify(proof)


def test_proof_attachment_accepts_complete_bounded_lane_merkle_witness() -> None:
    proof = attachment()
    proof["lane_privacy"] = lane_privacy(
        leaf_index=0xFFFF_FFFF,
        audit_path=[bytes([index]) * 32 for index in range(1, 33)],
    )
    verify(proof)

    proof["lane_privacy"] = lane_privacy(
        leaf_index=0,
        audit_path=[b"s" * 32] * 255,
    )
    verify(proof)


@pytest.mark.parametrize(
    ("lane", "error_type", "message"),
    (
        (lane_privacy(audit_path=[]), ValueError, "between 1 and 255"),
        (lane_privacy(audit_path=[b"s" * 32] * 256), ValueError, "between 1 and 255"),
        (lane_privacy(audit_path=[None]), ValueError, "must contain a sibling"),
        (lane_privacy(leaf_index=2), ValueError, "not representable"),
        (lane_privacy(leaf=b"l" * 31), ValueError, "must be 32 bytes"),
        (lane_privacy(audit_path=[b"s" * 31]), ValueError, "must be 32 bytes"),
        (lane_privacy(commitment_id=-1), TypeError, "unsigned 16-bit"),
        (lane_privacy(commitment_id=0x1_0000), TypeError, "unsigned 16-bit"),
        (lane_privacy(commitment_id=True), TypeError, "unsigned 16-bit"),
        (lane_privacy(leaf_index=-1), TypeError, "unsigned 32-bit"),
        (lane_privacy(leaf_index=0x1_0000_0000), TypeError, "unsigned 32-bit"),
        (lane_privacy(leaf_index=True), TypeError, "unsigned 32-bit"),
    ),
)
def test_proof_attachment_rejects_malformed_lane_witness(
    lane: dict[str, Any],
    error_type: type[Exception],
    message: str,
) -> None:
    proof = attachment()
    proof["lane_privacy"] = lane
    with pytest.raises(error_type, match=message):
        verify(proof)


@pytest.mark.parametrize(
    "path",
    (
        ("outer",),
        ("witness",),
        ("witness", "payload"),
        ("witness", "payload", "proof"),
    ),
)
def test_proof_attachment_rejects_unknown_lane_fields(path: tuple[str, ...]) -> None:
    lane = lane_privacy()
    target: dict[str, Any] = lane
    for component in path:
        if component != "outer":
            target = target[component]
    target["shadow"] = 1
    proof = attachment()
    proof["lane_privacy"] = lane
    with pytest.raises(ValueError, match="unknown first-release field"):
        verify(proof)


def test_proof_attachment_rejects_unknown_lane_witness_kind() -> None:
    lane = copy.deepcopy(lane_privacy())
    lane["witness"]["kind"] = "sparse-merkle"
    proof = attachment()
    proof["lane_privacy"] = lane
    with pytest.raises(ValueError, match="exactly `merkle`"):
        verify(proof)
