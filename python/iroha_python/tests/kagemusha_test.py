from __future__ import annotations

import base64
import hashlib
import inspect
import json
from dataclasses import FrozenInstanceError
from pathlib import Path

import pytest

import iroha_python
from iroha_python import kagemusha

RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)
PALLAS_OPEN_ENVELOPE_BUILDER_METHOD = "kagemusha_build_pallas_open_envelopes_archive"
PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD = (
    "kagemusha_build_previous_proof_open_envelopes_archive"
)
RECURSIVE_COMPACT_METHOD = (
    "kagemusha_prove_verified_recursive_compact_payment_token"
    "_with_records_and_pallas_open_envelopes"
)
RECURSIVE_COMPACT_VERIFY_METHOD = "kagemusha_verify_recursive_compact_payment_token"
RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD = (
    "kagemusha_recursive_spend_compact_payment_token_from_bundle"
)
RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection"
)
RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
)
RECURSIVE_SPEND_METHODS = (
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
)
MALFORMED_PROBE_ARCHIVE = b"\x00"
UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID = (
    "kagemusha-recursive-spend-lineage-badhop-v1"
)
UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND = "halo2/kzg"


def _shared_recursive_spend_manifest() -> dict[str, object]:
    return _shared_recursive_spend_fixture("manifest.json")


def _shared_recursive_spend_archives() -> dict[str, object]:
    return _shared_recursive_spend_fixture("archives.json")


def _shared_recursive_spend_fixture(file_name: str) -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "kagemusha_recursive_spend_abi6"
        / file_name
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _shared_recursive_spend_abi7_fixture(file_name: str) -> dict[str, object]:
    path = (
        Path(__file__).resolve().parents[3]
        / "fixtures"
        / "kagemusha_recursive_spend_abi7"
        / file_name
    )
    return json.loads(path.read_text(encoding="utf-8"))


def _shared_recursive_spend_abi7_manifest() -> dict[str, object]:
    return _shared_recursive_spend_abi7_fixture("manifest.json")


def _shared_recursive_spend_archive(name: str) -> bytes:
    archives = _shared_recursive_spend_archives()["archives"]
    assert isinstance(archives, list)
    for entry in archives:
        assert isinstance(entry, dict)
        if entry.get("name") == name:
            encoded = entry.get("bytes_base64")
            assert isinstance(encoded, str)
            return base64.b64decode(encoded)
    raise AssertionError(f"missing shared recursive spend archive: {name}")


def _shared_recursive_spend_abi7_archive(name: str) -> bytes:
    archives = _shared_recursive_spend_abi7_fixture("archives.json")["archives"]
    assert isinstance(archives, list)
    for entry in archives:
        assert isinstance(entry, dict)
        if entry.get("name") == name:
            encoded = entry.get("bytes_base64")
            assert isinstance(encoded, str)
            return base64.b64decode(encoded)
    raise AssertionError(f"missing shared recursive spend ABI-7 archive: {name}")


def _synthetic_kagemusha_archive(schema: str, seed: int = 0x41) -> bytes:
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(schema),
        bytes([seed, seed ^ 0x5A, 0x01]),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _synthetic_kagemusha_record_bundle_archive(hop_count: int = 1) -> bytes:
    step_payload = b"".join(
        kagemusha._kagemusha_field(bytes([0xA0 + index])) for index in range(6)
    )
    steps_payload = _u64_le(hop_count) + b"".join(
        kagemusha._kagemusha_field(step_payload) for _ in range(hop_count)
    )
    bundle_payload = b"".join(
        (
            kagemusha._kagemusha_field(b"\x41"),
            kagemusha._kagemusha_field(b"\x42"),
            kagemusha._kagemusha_field(steps_payload),
        )
    )
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME
        ),
        kagemusha._kagemusha_field(bundle_payload) + kagemusha._kagemusha_field(b""),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _synthetic_pallas_open_envelopes_archive(
    count: int = 1,
    *,
    include_vk_commitment: bool = True,
    include_public_inputs_schema_hash: bool = True,
    include_domain_tag: bool = True,
    params_curve_id: int = 1,
    public_curve_id: int = 1,
    transcript_label: str = "pallas-open",
    vk_commitment_payload: bytes | None = None,
    public_inputs_schema_hash_payload: bytes | None = None,
    domain_tag_payload: bytes | None = None,
    vk_commitment_option_payload: bytes | None = None,
    public_inputs_schema_hash_option_payload: bytes | None = None,
    domain_tag_option_payload: bytes | None = None,
) -> bytes:
    envelope = _synthetic_pallas_open_envelope_payload(
        include_vk_commitment=include_vk_commitment,
        include_public_inputs_schema_hash=include_public_inputs_schema_hash,
        include_domain_tag=include_domain_tag,
        params_curve_id=params_curve_id,
        public_curve_id=public_curve_id,
        transcript_label=transcript_label,
        vk_commitment_payload=vk_commitment_payload,
        public_inputs_schema_hash_payload=public_inputs_schema_hash_payload,
        domain_tag_payload=domain_tag_payload,
        vk_commitment_option_payload=vk_commitment_option_payload,
        public_inputs_schema_hash_option_payload=public_inputs_schema_hash_option_payload,
        domain_tag_option_payload=domain_tag_option_payload,
    )
    payload = _u64_le(count) + b"".join(
        kagemusha._kagemusha_field(envelope) for _ in range(count)
    )
    return _kagemusha_norito_frame_from_schema_hash(
        _PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH,
        payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _synthetic_pallas_open_envelope_payload(
    *,
    include_vk_commitment: bool,
    include_public_inputs_schema_hash: bool,
    include_domain_tag: bool,
    params_curve_id: int,
    public_curve_id: int,
    transcript_label: str,
    vk_commitment_payload: bytes | None,
    public_inputs_schema_hash_payload: bytes | None,
    domain_tag_payload: bytes | None,
    vk_commitment_option_payload: bytes | None,
    public_inputs_schema_hash_option_payload: bytes | None,
    domain_tag_option_payload: bytes | None,
) -> bytes:
    n = 4
    params = b"".join(
        (
            kagemusha._kagemusha_field(_u16_le(1)),
            kagemusha._kagemusha_field(_u16_le(params_curve_id)),
            kagemusha._kagemusha_field((n).to_bytes(4, "little")),
            kagemusha._kagemusha_field(_fixed32_sequence(n, 0x10)),
            kagemusha._kagemusha_field(_fixed32_sequence(n, 0x20)),
            kagemusha._kagemusha_field(_fixed32(0x30)),
        )
    )
    public_value = b"".join(
        (
            kagemusha._kagemusha_field(_u16_le(1)),
            kagemusha._kagemusha_field(_u16_le(public_curve_id)),
            kagemusha._kagemusha_field((n).to_bytes(4, "little")),
            kagemusha._kagemusha_field(_fixed32(0x31)),
            kagemusha._kagemusha_field(_fixed32(0x32)),
            kagemusha._kagemusha_field(_fixed32(0x33)),
        )
    )
    proof = b"".join(
        (
            kagemusha._kagemusha_field(_u16_le(1)),
            kagemusha._kagemusha_field(_fixed32_sequence(2, 0x40)),
            kagemusha._kagemusha_field(_fixed32_sequence(2, 0x50)),
            kagemusha._kagemusha_field(_fixed32(0x60)),
            kagemusha._kagemusha_field(_fixed32(0x61)),
        )
    )
    vk_payload = (
        None
        if not include_vk_commitment
        else vk_commitment_payload if vk_commitment_payload is not None else _fixed32(0x70)
    )
    public_inputs_schema_payload = (
        None
        if not include_public_inputs_schema_hash
        else public_inputs_schema_hash_payload
        if public_inputs_schema_hash_payload is not None
        else _fixed32(0x71)
    )
    domain_payload = (
        None
        if not include_domain_tag
        else domain_tag_payload if domain_tag_payload is not None else _fixed32(0x72)
    )
    vk_option_payload = (
        vk_commitment_option_payload
        if vk_commitment_option_payload is not None
        else _option_raw(vk_payload)
    )
    public_inputs_schema_option_payload = (
        public_inputs_schema_hash_option_payload
        if public_inputs_schema_hash_option_payload is not None
        else _option_raw(public_inputs_schema_payload)
    )
    domain_option_payload = (
        domain_tag_option_payload
        if domain_tag_option_payload is not None
        else _option_raw(domain_payload)
    )
    return b"".join(
        (
            kagemusha._kagemusha_field(params),
            kagemusha._kagemusha_field(public_value),
            kagemusha._kagemusha_field(proof),
            kagemusha._kagemusha_field(kagemusha._kagemusha_string(transcript_label)),
            kagemusha._kagemusha_field(vk_option_payload),
            kagemusha._kagemusha_field(public_inputs_schema_option_payload),
            kagemusha._kagemusha_field(domain_option_payload),
        )
    )


def _u64_le(value: int) -> bytes:
    return value.to_bytes(8, "little")


def _u16_le(value: int) -> bytes:
    return value.to_bytes(2, "little")


def _fixed32(seed: int) -> bytes:
    return bytes((seed + index) & 0xFF for index in range(32))


def _fixed32_sequence(count: int, seed: int) -> bytes:
    return _u64_le(count) + b"".join(
        kagemusha._kagemusha_field(_fixed32(seed + index)) for index in range(count)
    )


def _option_raw(payload: bytes | None) -> bytes:
    if payload is None:
        return b"\x00"
    return (
        b"\x01"
        + _kagemusha_norito_length(
            len(payload),
            _TEST_NORITO_COMPACT_LEN_FLAG,
        )
        + payload
    )


def _option_raw_with_trailing_byte(payload: bytes) -> bytes:
    return _option_raw(payload) + b"\x7f"


def _option_raw_with_unknown_tag() -> bytes:
    return b"\x02"


def _option_raw_with_declared_length_too_long(payload: bytes) -> bytes:
    return (
        b"\x01"
        + _kagemusha_norito_length(
            len(payload) + 1,
            _TEST_NORITO_COMPACT_LEN_FLAG,
        )
        + payload
    )


def _assert_kagemusha_archive_schema(archive: bytes, schema: str) -> None:
    assert archive[:4] == b"NRT0"
    assert archive[6:22] == kagemusha._norito_schema_hash(schema)
    assert archive[22] == 0
    assert archive[39] == _TEST_NORITO_COMPACT_LEN_FLAG


def _kagemusha_archive_payload(archive: bytes, schema: str) -> bytes:
    _assert_kagemusha_archive_schema(archive, schema)
    payload_len = int.from_bytes(archive[23:31], "little")
    payload = archive[40:]
    assert len(payload) == payload_len
    return payload


def _read_compact_length(payload: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    cursor = offset
    for _ in range(10):
        assert cursor < len(payload)
        byte = payload[cursor]
        cursor += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            return value, cursor
        shift += 7
    raise AssertionError("compact length is too long")


def _read_field(payload: bytes, offset: int) -> tuple[bytes, int]:
    length, cursor = _read_compact_length(payload, offset)
    end = cursor + length
    assert end <= len(payload)
    return payload[cursor:end], end


def _read_all_fields(payload: bytes) -> list[bytes]:
    fields: list[bytes] = []
    offset = 0
    while offset < len(payload):
        field, offset = _read_field(payload, offset)
        fields.append(field)
    assert offset == len(payload)
    return fields


def _read_sequence_fields(payload: bytes) -> list[bytes]:
    assert len(payload) >= 8
    count = int.from_bytes(payload[:8], "little")
    fields: list[bytes] = []
    offset = 8
    for _ in range(count):
        field, offset = _read_field(payload, offset)
        fields.append(field)
    assert offset == len(payload)
    return fields


def _encode_sequence_fields(fields: list[bytes]) -> bytes:
    return _u64_le(len(fields)) + _encode_test_fields(fields)


def _recursive_spend_bundle_with_accumulator_field(
    field_index: int,
    replacement: bytes,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    accumulator_fields = _read_all_fields(bundle_fields[0])
    accumulator_fields[field_index] = replacement
    bundle_fields[0] = _encode_test_fields(accumulator_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_topup_anchor_nullifiers(
    nullifiers: list[bytes],
) -> bytes:
    return _recursive_spend_bundle_with_accumulator_field(
        5,
        _encode_sequence_fields([bytes(nullifier) for nullifier in nullifiers]),
    )


def _recursive_spend_bundle_with_trailing_bundle_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    bundle_fields.append(kagemusha._kagemusha_string("ignored-extra-bundle-field"))
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_verify_result_with_trailing_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_abi7_archive("verify_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    fields.append(b"\x01")
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_trailing_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    fields.append(kagemusha._kagemusha_string("ignored-extra-lineage-witness-field"))
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_trailing_previous_proofs_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    fields[3] += kagemusha._kagemusha_field(
        kagemusha._kagemusha_string("ignored-extra-previous-proofs-field")
    )
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_trailing_previous_proof_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    previous_proof_fields.append(
        kagemusha._kagemusha_string("ignored-extra-previous-proof-field")
    )
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_trailing_previous_verifier_key_id_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    verifier_key_id_fields = _read_all_fields(previous_proof_fields[0])
    verifier_key_id_fields.append(
        kagemusha._kagemusha_string("ignored-extra-previous-verifier-key-field")
    )
    previous_proof_fields[0] = _encode_test_fields(verifier_key_id_fields)
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_previous_proof_field(
    field_index: int,
    replacement: bytes,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    previous_proof_fields[field_index] = replacement
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_previous_proof_box_backend(
    proof_backend: str,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    proof_box_fields = _read_all_fields(previous_proof_fields[3])
    proof_box_fields[0] = kagemusha._kagemusha_string(proof_backend)
    previous_proof_fields[3] = _encode_test_fields(proof_box_fields)
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_previous_proof_box_backend_and_empty_proof_bytes(
    proof_backend: str,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    proof_box_fields = _read_all_fields(previous_proof_fields[3])
    proof_box_fields[0] = kagemusha._kagemusha_string(proof_backend)
    proof_box_fields[1] = (0).to_bytes(8, "little")
    previous_proof_fields[3] = _encode_test_fields(proof_box_fields)
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_lineage_witness_with_empty_previous_proof_bytes() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("lineage_witness_append_result"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
    )
    fields = _read_all_fields(payload)
    previous_proofs = _read_sequence_fields(fields[3])
    assert previous_proofs
    previous_proof_fields = _read_all_fields(previous_proofs[0])
    proof_box_fields = _read_all_fields(previous_proof_fields[3])
    proof_box_fields[1] = (0).to_bytes(8, "little")
    previous_proof_fields[3] = _encode_test_fields(proof_box_fields)
    previous_proofs[0] = _encode_test_fields(previous_proof_fields)
    fields[3] = _encode_sequence_fields(previous_proofs)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME
        ),
        _encode_test_fields(fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_trailing_accumulator_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    accumulator_fields = _read_all_fields(bundle_fields[0])
    accumulator_fields.append(kagemusha._kagemusha_string("ignored-extra-accumulator-field"))
    bundle_fields[0] = _encode_test_fields(accumulator_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_proof_circuit_id(proof_circuit_id: str) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    expected = (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
            "utf-8"
        )
    )
    replacement = proof_circuit_id.encode("utf-8")
    assert len(replacement) == len(expected)
    assert payload.count(expected) == 2
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        payload.replace(expected, replacement),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_proof_backend(proof_backend: str) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    expected = kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND.encode(
        "utf-8"
    )
    replacement = proof_backend.encode("utf-8")
    assert len(replacement) == len(expected)
    assert payload.count(expected) == 2
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        payload.replace(expected, replacement),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_proof_box_backend(proof_backend: str) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_box_fields = _read_all_fields(proof_fields[3])
    proof_box_fields[0] = kagemusha._kagemusha_string(proof_backend)
    proof_fields[3] = _encode_test_fields(proof_box_fields)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_proof_box_backend_and_empty_proof_bytes(
    proof_backend: str,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_box_fields = _read_all_fields(proof_fields[3])
    proof_box_fields[0] = kagemusha._kagemusha_string(proof_backend)
    proof_box_fields[1] = (0).to_bytes(8, "little")
    proof_fields[3] = _encode_test_fields(proof_box_fields)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_trailing_recursive_proof_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_fields.append(kagemusha._kagemusha_string("ignored-extra-recursive-proof-field"))
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_trailing_verifier_key_id_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    verifier_key_id_fields = _read_all_fields(proof_fields[0])
    verifier_key_id_fields.append(
        kagemusha._kagemusha_string("ignored-extra-verifier-key-field")
    )
    proof_fields[0] = _encode_test_fields(verifier_key_id_fields)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_trailing_proof_box_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_box_fields = _read_all_fields(proof_fields[3])
    proof_box_fields.append(kagemusha._kagemusha_string("ignored-extra-proof-box-field"))
    proof_fields[3] = _encode_test_fields(proof_box_fields)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_empty_proof_bytes() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_box_fields = _read_all_fields(proof_fields[3])
    proof_box_fields[1] = (0).to_bytes(8, "little")
    proof_fields[3] = _encode_test_fields(proof_box_fields)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_empty_proof_public_inputs() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_fields[1] = b""
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_zero_proof_public_inputs_hash() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    proof_fields[2] = bytes(32)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_mismatched_proof_public_inputs_hash() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    proof_fields = _read_all_fields(bundle_fields[1])
    mismatched_hash = bytearray(proof_fields[2])
    mismatched_hash[0] ^= 0x01
    proof_fields[2] = bytes(mismatched_hash)
    bundle_fields[1] = _encode_test_fields(proof_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_current_note_field(
    field_index: int,
    replacement: bytes,
) -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    accumulator_fields = _read_all_fields(bundle_fields[0])
    current_note_fields = _read_all_fields(accumulator_fields[22])
    current_note_fields[field_index] = bytes(replacement)
    accumulator_fields[22] = _encode_test_fields(current_note_fields)
    bundle_fields[0] = _encode_test_fields(accumulator_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_trailing_current_note_field() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    accumulator_fields = _read_all_fields(bundle_fields[0])
    current_note_fields = _read_all_fields(accumulator_fields[22])
    current_note_fields.append(kagemusha._kagemusha_string("ignored-extra-current-note-field"))
    accumulator_fields[22] = _encode_test_fields(current_note_fields)
    bundle_fields[0] = _encode_test_fields(accumulator_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _recursive_spend_bundle_with_equal_current_note_nullifier() -> bytes:
    payload = _kagemusha_archive_payload(
        _shared_recursive_spend_archive("init_bundle"),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
    )
    bundle_fields = _read_all_fields(payload)
    accumulator_fields = _read_all_fields(bundle_fields[0])
    current_note_fields = _read_all_fields(accumulator_fields[22])
    current_note_fields[1] = current_note_fields[0]
    accumulator_fields[22] = _encode_test_fields(current_note_fields)
    bundle_fields[0] = _encode_test_fields(accumulator_fields)
    return _kagemusha_norito_frame_from_schema_hash(
        kagemusha._norito_schema_hash(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME
        ),
        _encode_test_fields(bundle_fields),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )


def _fixed_array_payload(value: int, count: int) -> bytes:
    return _encode_test_fields([bytes((value,)) for _ in range(count)])


def _count_prefixed_fixed_array_payload(value: int, count: int) -> bytes:
    return count.to_bytes(8, "little") + _fixed_array_payload(value, count)


def _numeric_payload(mantissa: bytes, scale: int = 0) -> bytes:
    return _encode_test_fields(
        [
            len(mantissa).to_bytes(4, "little") + mantissa,
            scale.to_bytes(4, "little"),
        ]
    )


def _numeric_payload_with_mantissa_payload(mantissa_payload: bytes) -> bytes:
    return _encode_test_fields(
        [
            mantissa_payload,
            (0).to_bytes(4, "little"),
        ]
    )


def _numeric_payload_with_scale_payload(scale_payload: bytes) -> bytes:
    return _encode_test_fields(
        [
            (1).to_bytes(4, "little") + b"\x01",
            scale_payload,
        ]
    )


def _numeric_payload_with_trailing_field() -> bytes:
    return _numeric_payload(b"\x01") + kagemusha._kagemusha_field(
        (0x42).to_bytes(4, "little")
    )


def _zero_numeric_payload() -> bytes:
    return _numeric_payload(b"")


def _read_option_some(payload: bytes) -> bytes:
    assert payload[0] == 1
    field, offset = _read_field(payload, 1)
    assert offset == len(payload)
    return field


def _assert_option_none(payload: bytes) -> None:
    assert payload == b"\x00"


def _read_fixed_bytes_payload(payload: bytes, expected_len: int) -> bytes:
    out = bytearray()
    offset = 0
    while offset < len(payload):
        field, offset = _read_field(payload, offset)
        assert len(field) == 1
        out.extend(field)
    assert len(out) == expected_len
    return bytes(out)


def _encode_test_fields(fields: list[bytes]) -> bytes:
    return b"".join(kagemusha._kagemusha_field(field) for field in fields)


def _legacy_no_length_const_vec_u8_payload(value: bytes) -> bytes:
    return b"".join(kagemusha._kagemusha_field(bytes((byte,))) for byte in value)


def _recursive_spend_note(
    commitment_seed: int = 0x44,
    nullifier_seed: int = 0x55,
    amount: str = "7",
) -> kagemusha.KagemushaRecursiveSpendableNoteDescriptor:
    return kagemusha.KagemushaRecursiveSpendableNoteDescriptor(
        note_commitment=bytes([commitment_seed]) * 32,
        spend_nullifier=bytes([nullifier_seed]) * 32,
        amount=amount,
    )


def _recursive_spend_verifier_record() -> kagemusha.KagemushaRecursiveSpendVerifierRecordRef:
    return kagemusha.KagemushaRecursiveSpendVerifierRecordRef(
        verifier_key_id="offline_kagemusha/test/lineage",
        record_bytes=_synthetic_kagemusha_archive(
            kagemusha.KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            0x52,
        ),
    )


def _recursive_spend_recipient() -> str:
    from iroha_python.address import AccountAddress

    return AccountAddress.from_account(
        domain="wonderland",
        public_key=bytes([0x24]) * 32,
    ).to_i105()


def _instruction_archive_bytes(instruction: object) -> bytes:
    to_json = getattr(instruction, "to_json")
    encoded = json.loads(to_json())
    assert isinstance(encoded, str)
    archive = base64.b64decode(encoded)
    assert archive.startswith(b"NRT0")
    return archive


def _is_malformed_probe_archive(value: bytes) -> bool:
    return bytes(value) == MALFORMED_PROBE_ARCHIVE


def _kagemusha_norito_frame(schema_byte: int) -> bytes:
    frame = bytearray(40)
    frame[:4] = b"NRT0"
    frame[6:22] = bytes([schema_byte]) * 16
    return bytes(frame)


def _kagemusha_norito_frame_with_payload(schema_byte: int) -> bytes:
    frame = bytearray(_kagemusha_norito_frame(schema_byte) + b"\x00\x00\xa5\x5a\x11")
    frame[23:31] = (3).to_bytes(8, "little")
    frame[31:39] = bytes([0xB9, 0xD3, 0xA8, 0x0C, 0xCD, 0x5D, 0x13, 0x24])
    return bytes(frame)


def _kagemusha_norito_frame_with_header_padding(
    archive: bytes, padding: bytes
) -> bytes:
    return bytes(archive[:40] + padding + archive[40:])


_TEST_CRC64_MASK = 0xFFFF_FFFF_FFFF_FFFF
_TEST_CRC64_REFLECTED_POLY = 0xC96C_5795_D787_0F42


def _build_test_crc64_table() -> tuple[int, ...]:
    table: list[int] = []
    for index in range(256):
        crc = index
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ _TEST_CRC64_REFLECTED_POLY
            else:
                crc >>= 1
        table.append(crc)
    return tuple(table)


_TEST_CRC64_TABLE = _build_test_crc64_table()
_TEST_NORITO_COMPACT_LEN_FLAG = 0x02
_TEST_NORITO_PACKED_STRUCT_FLAG = 0x04
_TEST_NORITO_FIELD_BITSET_FLAG = 0x20
_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = bytes.fromhex(
    "c88489618a012c283ff3bb2ebabc7775"
)
_OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = bytes.fromhex(
    "119f4df38a98ef5848ad0aadb9715779"
)
_PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH = bytes.fromhex(
    "fe3826328f081771750f24fe110260ca"
)


def _test_crc64(payload: bytes) -> int:
    crc = _TEST_CRC64_MASK
    for byte in payload:
        index = (crc ^ byte) & 0xFF
        crc = _TEST_CRC64_TABLE[index] ^ (crc >> 8)
    return (crc ^ _TEST_CRC64_MASK) & _TEST_CRC64_MASK


def _kagemusha_norito_frame_from_payload(schema_byte: int, payload: bytes) -> bytes:
    frame = bytearray(_kagemusha_norito_frame(schema_byte) + bytes(payload))
    frame[23:31] = len(payload).to_bytes(8, "little")
    frame[31:39] = _test_crc64(payload).to_bytes(8, "little")
    return bytes(frame)


def _kagemusha_norito_frame_from_schema_hash(
    schema_hash: bytes,
    payload: bytes,
    flags: int = 0,
) -> bytes:
    frame = bytearray(40 + len(payload))
    frame[0:4] = b"NRT0"
    frame[6:22] = schema_hash
    frame[23:31] = len(payload).to_bytes(8, "little")
    frame[31:39] = _test_crc64(payload).to_bytes(8, "little")
    frame[39] = flags
    frame[40:] = payload
    return bytes(frame)


def _kagemusha_norito_length(value: int, flags: int = 0) -> bytes:
    if not flags & _TEST_NORITO_COMPACT_LEN_FLAG:
        return value.to_bytes(8, "little")
    remaining = value
    output = bytearray()
    while remaining >= 0x80:
        output.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    output.append(remaining)
    return bytes(output)


def _kagemusha_overlong_compact_length(value: int) -> bytes:
    if value < 0 or value >= 0x80:
        raise ValueError("test helper only encodes small overlong lengths")
    return bytes([value | 0x80, 0x00])


def _kagemusha_oversized_terminal_compact_length() -> bytes:
    return (b"\x80" * 9) + b"\x02"


def _kagemusha_huge_canonical_compact_length() -> bytes:
    return (b"\x80" * 9) + b"\x01"


def _kagemusha_norito_field(
    payload: bytes,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
) -> bytes:
    return _kagemusha_norito_length(len(payload), flags) + payload


def _kagemusha_norito_string(
    value: str,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
) -> bytes:
    payload = value.encode("utf-8")
    return _kagemusha_norito_length(len(payload), flags) + payload


def _kagemusha_norito_byte_vec(value: bytes) -> bytes:
    return len(value).to_bytes(8, "little") + value


def _kagemusha_zk1_tlv(tag: bytes, payload: bytes) -> bytes:
    return tag + len(payload).to_bytes(4, "little") + bytes(payload)


def _kagemusha_lineage_verifier_key(circuit_id: str, seed: int) -> bytes:
    return (
        b"ZK1\x00"
        + _kagemusha_zk1_tlv(b"IPAK", bytes([8, 0, 0, 0]))
        + _kagemusha_zk1_tlv(b"CID1", circuit_id.encode("utf-8"))
        + _kagemusha_zk1_tlv(b"H2VK", bytes([seed]) * 32)
    )


def _kagemusha_verifier_key_commitment(verifier_key: bytes) -> bytes:
    backend = kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND.encode("utf-8")
    digest = hashlib.sha256()
    digest.update(b"iroha:zk:v1:vk")
    digest.update(len(backend).to_bytes(8, "big"))
    digest.update(backend)
    digest.update(len(verifier_key).to_bytes(8, "big"))
    digest.update(verifier_key)
    return digest.digest()


def _kagemusha_lineage_proving_key_archive(
    circuit_id: str,
    verifier_key: bytes,
    seed: int,
) -> bytes:
    return _kagemusha_lineage_proving_key_archive_raw(
        1,
        circuit_id,
        _kagemusha_verifier_key_commitment(verifier_key),
        bytes([seed]) * 64,
    )


def _recursive_spend_lineage_artifacts_for_init(
    seed: int = 0x91,
) -> kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts:
    verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        seed,
    )
    proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        verifier_key,
        seed + 1,
    )
    return kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        verifier_key,
        proving_key_archive,
    )


def _recursive_spend_lineage_artifacts_for_append(
    seed: int = 0x93,
) -> kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts:
    verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        seed,
    )
    proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        verifier_key,
        seed + 1,
    )
    return kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_append(
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        verifier_key,
        proving_key_archive,
    )


def _kagemusha_lineage_proving_key_archive_raw(
    version: int,
    circuit_id: str,
    verifier_key_commitment: bytes,
    proving_key: bytes,
    flags: int = _TEST_NORITO_COMPACT_LEN_FLAG,
    schema_hash: bytes = _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    trailing_payload: bytes = b"",
) -> bytes:
    payload = (
        _kagemusha_norito_field(version.to_bytes(2, "little"), flags)
        + _kagemusha_norito_field(_kagemusha_norito_string(circuit_id, flags), flags)
        + _kagemusha_norito_field(verifier_key_commitment, flags)
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(proving_key), flags)
        + trailing_payload
    )
    return _kagemusha_norito_frame_from_schema_hash(
        schema_hash,
        payload,
        flags,
    )


def _kagemusha_input_archive(schema_byte: int = 0x50) -> bytes:
    return _kagemusha_norito_frame_with_payload(schema_byte)


RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE = _kagemusha_input_archive(0xE1)
RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE = _kagemusha_input_archive(0xE2)


def _kagemusha_test_keypair() -> iroha_python.Ed25519KeyPair:
    return iroha_python.Ed25519KeyPair.from_private_key(bytes([0x42] * 32))


def test_kagemusha_instruction_archive_transaction_helpers_wrap_redeem_archive() -> None:
    archive = _shared_recursive_spend_abi7_archive("redeem_instruction")
    instruction = kagemusha.kagemusha_instruction_archive_instruction(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
    )
    canonical_archive = _instruction_archive_bytes(instruction)
    assert canonical_archive.startswith(b"NRT0")
    assert len(canonical_archive) > 0

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    envelope = kagemusha.build_kagemusha_instruction_transaction(
        "chain",
        authority,
        keypair.private_key,
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
        creation_time_ms=1,
        ttl_ms=10_000,
        nonce=1,
        metadata={"kagemusha": "redeem"},
    )
    assert envelope.chain_id == "chain"
    assert envelope.authority == authority
    assert bytes(envelope.signed_transaction)
    assert bytes(envelope.signed_transaction_versioned)
    assert envelope.hash_hex()

    draft = iroha_python.TransactionDraft(
        iroha_python.TransactionConfig(chain_id="chain", authority=authority)
    )
    draft.kagemusha_instruction_archive(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        archive,
    )
    assert len(draft) == 1


def test_kagemusha_recursive_redeem_transaction_helper_derives_instruction_before_signing() -> None:
    request_archive = _shared_recursive_spend_abi7_archive("redeem_request")
    redeem_instruction_archive = _shared_recursive_spend_abi7_archive("redeem_instruction")
    instruction = kagemusha.kagemusha_recursive_redeem_instruction(request_archive)
    committed_instruction = kagemusha.kagemusha_instruction_archive_instruction(
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
        redeem_instruction_archive,
    )
    assert _instruction_archive_bytes(instruction) == _instruction_archive_bytes(
        committed_instruction
    )

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    envelope = kagemusha.build_kagemusha_recursive_redeem_transaction(
        "chain",
        authority,
        keypair.private_key,
        request_archive,
        creation_time_ms=2,
        ttl_ms=10_000,
        nonce=2,
        metadata={"kagemusha": "recursive-redeem"},
    )
    assert envelope.chain_id == "chain"
    assert envelope.authority == authority
    assert bytes(envelope.signed_transaction)
    assert envelope.hash_hex()

    draft = iroha_python.TransactionDraft(
        iroha_python.TransactionConfig(chain_id="chain", authority=authority)
    )
    draft.kagemusha_recursive_redeem(request_archive)
    assert len(draft) == 1


def test_kagemusha_instruction_archive_transaction_helpers_reject_adversarial_inputs() -> None:
    archive = _shared_recursive_spend_abi7_archive("redeem_instruction")

    assert (
        kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES[
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE
        ]
        == "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
    )
    assert (
        iroha_python.KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME
        == "iroha_data_model::isi::offline::KagemushaTransfer"
    )

    with pytest.raises(ValueError, match="instruction_type must be KagemushaTransfer"):
        kagemusha.kagemusha_instruction_archive_instruction("RedeemRecursive", archive)

    whitespace_instruction_type = (
        f" {kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE} "
    )
    with pytest.raises(ValueError, match="instruction_type must be KagemushaTransfer"):
        kagemusha.kagemusha_instruction_archive_instruction(
            whitespace_instruction_type,
            archive,
        )

    with pytest.raises(ValueError, match="instruction_archive must not be empty"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            b"",
        )

    with pytest.raises(ValueError, match="schema must match RedeemKagemushaRecursive"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            _shared_recursive_spend_abi7_archive("redeem_request"),
        )

    tampered = bytearray(archive)
    tampered[-1] ^= 0x01
    with pytest.raises(ValueError, match="instruction_archive must be a valid Norito archive"):
        kagemusha.kagemusha_instruction_archive_instruction(
            kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
            tampered,
        )

    def assert_rejects_instruction_archive(mutated: bytearray) -> None:
        with pytest.raises(
            ValueError,
            match="instruction_archive must be a valid Norito archive",
        ):
            kagemusha.kagemusha_instruction_archive_instruction(
                kagemusha.KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
                mutated,
            )

    compressed = bytearray(archive)
    compressed[22] = 1
    assert_rejects_instruction_archive(compressed)

    unsupported_flags = bytearray(archive)
    unsupported_flags[39] = 0x08
    assert_rejects_instruction_archive(unsupported_flags)

    invalid_field_bitset = bytearray(archive)
    invalid_field_bitset[39] = 0x20
    assert_rejects_instruction_archive(invalid_field_bitset)

    non_zero_padding = bytearray(archive)
    non_zero_padding.insert(40, 0x7F)
    assert_rejects_instruction_archive(non_zero_padding)

    excessive_padding = bytearray(archive)
    excessive_padding[40:40] = b"\x00" * 65
    assert_rejects_instruction_archive(excessive_padding)

    keypair = _kagemusha_test_keypair()
    authority = keypair.default_account_id("wonderland")
    with pytest.raises(ValueError, match="instruction_type must be KagemushaTransfer"):
        kagemusha.build_kagemusha_instruction_transaction(
            "chain",
            authority,
            keypair.private_key,
            whitespace_instruction_type,
            archive,
        )

    with pytest.raises(ValueError, match="redeem_request_archive must be a valid Norito archive"):
        kagemusha.build_kagemusha_recursive_redeem_transaction(
            "chain",
            authority,
            keypair.private_key,
            b"\x00",
        )

    bad_request_flags = bytearray(_shared_recursive_spend_abi7_archive("redeem_request"))
    bad_request_flags[39] = 0x20
    with pytest.raises(ValueError, match="redeem_request_archive must be a valid Norito archive"):
        kagemusha.build_kagemusha_recursive_redeem_transaction(
            "chain",
            authority,
            keypair.private_key,
            bad_request_flags,
        )


class _Native:
    def __init__(self) -> None:
        self.calls: list[tuple[str, bytes]] = []
        setattr(self, RECURSIVE_AGGREGATION_METHOD, self._recursive_aggregation)

    def _reject_probe(self, context: str, *archives: bytes) -> None:
        if archives and all(_is_malformed_probe_archive(archive) for archive in archives):
            raise ValueError(f"invalid Kagemusha {context} probe archive")

    def kagemusha_recursive_spend_native_bridge_abi_version(self) -> int:
        return kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION + 1

    def kagemusha_prove_verified_compact_payment_token_with_records(
        self,
        record_bundle: bytes,
    ) -> bytes:
        self._reject_probe("compact", record_bundle)
        self.calls.append(("compact", record_bundle))
        return _kagemusha_norito_frame_with_payload(0x31)

    def _recursive_aggregation(
        self,
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
    ) -> bytes:
        self._reject_probe("recursive aggregation", record_bundle, pallas_open_envelopes)
        self.calls.append(
            ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes)
        )
        return _kagemusha_norito_frame_with_payload(0x32)

    def kagemusha_build_pallas_open_envelopes_archive(
        self,
        record_bundle: bytes,
    ) -> bytes:
        self._reject_probe("Pallas open-envelope builder", record_bundle)
        self.calls.append(("pallas_open_envelope_builder", record_bundle))
        return _kagemusha_norito_frame_with_payload(0x3C)

    def kagemusha_build_previous_proof_open_envelopes_archive(
        self,
        previous_bundle: bytes,
    ) -> bytes:
        self._reject_probe("previous proof open-envelope builder", previous_bundle)
        self.calls.append(("previous_proof_open_envelope_builder", previous_bundle))
        return _kagemusha_norito_frame_with_payload(0x3D)

    def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
        self._reject_probe("init", request)
        self.calls.append(("init", request))
        return _kagemusha_norito_frame_with_payload(0x33)

    def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
        self._reject_probe("append", request)
        self.calls.append(("append", request))
        return _kagemusha_norito_frame_with_payload(0x34)

    def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
        self._reject_probe("transition profile init", request)
        self.calls.append(("transition-profile-init", request))
        return _kagemusha_norito_frame_with_payload(0x35)

    def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
        self._reject_probe("transition profile append", request)
        self.calls.append(("transition-profile-append", request))
        return _kagemusha_norito_frame_with_payload(0x36)

    def kagemusha_recursive_spend_lineage_append_boundary(self, profile: bytes) -> bytes:
        self._reject_probe("lineage append boundary", profile)
        self.calls.append(("lineage-append-boundary", profile))
        return _kagemusha_norito_frame_with_payload(0x37)

    def kagemusha_recursive_spend_lineage_witness_from_init_result(
        self,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage init", request, bundle)
        self.calls.append(("lineage-init", request + b"|" + bundle))
        return _kagemusha_norito_frame_with_payload(0x38)

    def kagemusha_recursive_spend_lineage_witness_append_result(
        self,
        previous_witness: bytes,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        self._reject_probe("lineage append", previous_witness, request, bundle)
        self.calls.append(("lineage-append", previous_witness + b"|" + request + b"|" + bundle))
        return _kagemusha_norito_frame_with_payload(0x39)

    def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
        self._reject_probe("verify", request)
        self.calls.append(("verify", request))
        return _kagemusha_norito_frame_with_payload(0x3A)

    def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
        self._reject_probe("redeem", request)
        self.calls.append(("redeem", request))
        return _kagemusha_norito_frame_with_payload(0x3B)


def test_recursive_kagemusha_helpers_reject_empty_requests(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(ValueError, match="request_archive must not be empty"):
            helper(b"")
    with pytest.raises(ValueError, match="profile_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"")
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            b"",
            _kagemusha_input_archive(0x51),
        )
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x52),
            b"",
        )
    with pytest.raises(ValueError, match="previous_witness_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            b"",
            _kagemusha_input_archive(0x53),
            _kagemusha_input_archive(0x54),
        )
    with pytest.raises(ValueError, match="request_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x55),
            b"",
            _kagemusha_input_archive(0x56),
        )
    with pytest.raises(ValueError, match="bundle_archive must not be empty"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x57),
            _kagemusha_input_archive(0x58),
            b"",
        )

    assert native.calls == []


def test_recursive_kagemusha_helpers_reject_malformed_norito_requests() -> None:
    with pytest.raises(ValueError, match="request_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_init(b"\x01")
    with pytest.raises(ValueError, match="profile_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(b"\x01")
    with pytest.raises(ValueError, match="bundle_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x59),
            b"\x01",
        )
    with pytest.raises(ValueError, match="request_archive must be a valid Norito archive"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x5A),
            b"\x01",
            _kagemusha_input_archive(0x5B),
        )


def test_recursive_kagemusha_helpers_reject_empty_payload_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="request_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_recursive_spend_verify(_kagemusha_norito_frame(0x5C))
    with pytest.raises(
        ValueError,
        match="previous_witness_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_norito_frame(0x5D),
            _kagemusha_input_archive(0x5E),
            _kagemusha_input_archive(0x5F),
        )


def test_kagemusha_native_prover_helpers_reject_empty_requests(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"")

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(b"")

    with pytest.raises(ValueError, match="previous_bundle_archive must not be empty"):
        getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(b"")

    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"",
            b"pallas",
        )

    with pytest.raises(ValueError, match="pallas_open_envelopes_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xA1),
            b"",
        )
    with pytest.raises(ValueError, match="record_bundle_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            b"",
            b"pallas",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(ValueError, match="pallas_open_envelopes_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xA2),
            b"",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must not be empty",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xA3),
            _kagemusha_input_archive(0xA4),
            b"",
        )
    with pytest.raises(ValueError, match="compact_token_archive must not be empty"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            b"",
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must not be empty",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            b"",
        )
    with pytest.raises(ValueError, match="compact_token_archive must be a valid Norito archive"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            b"\x01",
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="compact_token_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    assert native.calls == []


def test_kagemusha_native_prover_helpers_reject_malformed_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must be a valid Norito archive",
    ):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(b"\x01")
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(b"\x01")
    with pytest.raises(
        ValueError,
        match="previous_bundle_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(b"\x01")
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            b"\x01",
            _kagemusha_input_archive(0xB1),
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xB2),
            b"\x01",
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB3),
            b"\x01",
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB3),
            _kagemusha_input_archive(0xB4),
            b"\x01",
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must be a valid Norito archive",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            b"\x01",
        )


def test_kagemusha_native_prover_helpers_reject_empty_payload_norito_requests() -> None:
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must contain a non-empty Norito payload",
    ):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_norito_frame(0xB4)
        )
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(
            _kagemusha_norito_frame(0xB5)
        )
    with pytest.raises(
        ValueError,
        match="previous_bundle_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(
            _kagemusha_norito_frame(0xB6)
        )
    with pytest.raises(
        ValueError,
        match="record_bundle_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_norito_frame(0xB5),
            _kagemusha_input_archive(0xB6),
        )
    with pytest.raises(
        ValueError,
        match="pallas_open_envelopes_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB7),
            _kagemusha_norito_frame(0xB8),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_key_artifacts_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xB7),
            _kagemusha_input_archive(0xB8),
            _kagemusha_norito_frame(0xB9),
        )
    with pytest.raises(
        ValueError,
        match="recursive_compact_verifier_keys_archive must contain a non-empty Norito payload",
    ):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_input_archive(0x4B),
            _kagemusha_norito_frame(0x4C),
        )


def test_recursive_compact_unavailable_classifier_matches_reserved_fragments() -> None:
    payment_token_message = (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT
    )
    multi_hop_message = (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT
    )

    assert kagemusha.is_kagemusha_recursive_compact_unavailable(
        RuntimeError(payment_token_message)
    )
    assert kagemusha.is_kagemusha_recursive_compact_unavailable(
        f"bridge: {multi_hop_message}"
    )
    assert not kagemusha.is_kagemusha_recursive_compact_unavailable(
        RuntimeError("recursive compact proof composition unavailable")
    )
    assert not kagemusha.is_kagemusha_recursive_compact_unavailable(None)
    assert (
        iroha_python.is_kagemusha_recursive_compact_unavailable(multi_hop_message)
        is True
    )


def test_recursive_kagemusha_helpers_probe_and_delegate(monkeypatch: pytest.MonkeyPatch) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    record_bundle = _kagemusha_input_archive(0xB9)
    pallas_open_envelopes = _kagemusha_input_archive(0xBA)
    previous_bundle = _kagemusha_input_archive(0xBB)
    init_request = _kagemusha_input_archive(0x61)
    append_request = _kagemusha_input_archive(0x62)
    transition_init_request = _kagemusha_input_archive(0x63)
    transition_append_request = _kagemusha_input_archive(0x64)
    boundary_profile = _kagemusha_input_archive(0x65)
    lineage_init_request = _kagemusha_input_archive(0x66)
    lineage_init_bundle = _kagemusha_input_archive(0x67)
    lineage_append_previous_witness = _kagemusha_input_archive(0x68)
    lineage_append_request = _kagemusha_input_archive(0x69)
    lineage_append_bundle = _kagemusha_input_archive(0x6A)
    verify_request = _kagemusha_input_archive(0x6B)
    redeem_request = _kagemusha_input_archive(0x6C)

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is True
    assert kagemusha.is_kagemusha_pallas_open_envelope_builder_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode_for_capabilities(True, True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode_for_capabilities(False, True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(True)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode(False)
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    assert (
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            record_bundle
        )
        == _kagemusha_norito_frame_with_payload(0x31)
    )
    recursive_aggregation = getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)
    assert (
        recursive_aggregation(record_bundle, pallas_open_envelopes)
        == _kagemusha_norito_frame_with_payload(0x32)
    )
    pallas_open_envelope_builder = getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)
    previous_proof_open_envelope_builder = getattr(
        kagemusha,
        PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD,
    )
    assert (
        pallas_open_envelope_builder(record_bundle)
        == _kagemusha_norito_frame_with_payload(0x3C)
    )
    assert (
        previous_proof_open_envelope_builder(memoryview(previous_bundle))
        == _kagemusha_norito_frame_with_payload(0x3D)
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    def permissive_recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native.calls.append(
            (
                "permissive_recursive_compact",
                record_bundle + b"|" + pallas_open_envelopes + b"|" + key_artifacts,
            )
        )
        return b"permissive_recursive_compact"

    setattr(native, RECURSIVE_COMPACT_METHOD, permissive_recursive_compact)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
    assert native.calls[-1] == (
        "permissive_recursive_compact",
        MALFORMED_PROBE_ARCHIVE
        + b"|"
        + MALFORMED_PROBE_ARCHIVE
        + b"|"
        + MALFORMED_PROBE_ARCHIVE,
    )

    def recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        native.calls.append(
            ("recursive_compact", record_bundle + b"|" + pallas_open_envelopes + b"|" + key_artifacts)
        )
        return _kagemusha_norito_frame_with_payload(0x4D)

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, lambda compact_token, verifier_keys: True)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )

    def recursive_compact_verify(compact_token: bytes, verifier_keys: bytes) -> bool:
        native._reject_probe("recursive compact verify", compact_token, verifier_keys)
        native.calls.append(("recursive_compact_verify", compact_token + b"|" + verifier_keys))
        return compact_token[6] == 0x4B

    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, recursive_compact_verify)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is True
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1
    )

    def unavailable_recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        raise RuntimeError("recursive compact proof composition unavailable")

    setattr(native, RECURSIVE_COMPACT_METHOD, unavailable_recursive_compact)
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    with pytest.raises(RuntimeError, match="proof composition unavailable"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            record_bundle,
            pallas_open_envelopes,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )
        == _kagemusha_norito_frame_with_payload(0x4D)
    )
    valid_recursive_compact_token = _kagemusha_norito_frame_with_payload(0x4B)
    forged_recursive_compact_token = _kagemusha_norito_frame_with_payload(0x4C)
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            valid_recursive_compact_token,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
        is True
    )
    assert (
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            forged_recursive_compact_token,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )
        is False
    )
    assert (
        kagemusha.kagemusha_recursive_spend_init(init_request)
        == _kagemusha_norito_frame_with_payload(0x33)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_append(bytearray(append_request))
        == _kagemusha_norito_frame_with_payload(0x34)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_init(transition_init_request)
        == _kagemusha_norito_frame_with_payload(0x35)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_transition_profile_append(
            transition_append_request
        )
        == _kagemusha_norito_frame_with_payload(0x36)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary(boundary_profile)
        == _kagemusha_norito_frame_with_payload(0x37)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            lineage_init_request,
            lineage_init_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x38)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            lineage_append_previous_witness,
            lineage_append_request,
            lineage_append_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x39)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_verify(memoryview(verify_request))
        == _kagemusha_norito_frame_with_payload(0x3A)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_redeem(redeem_request)
        == _kagemusha_norito_frame_with_payload(0x3B)
    )
    assert native.calls == [
        ("compact", record_bundle),
        ("recursive_aggregation", record_bundle + b"|" + pallas_open_envelopes),
        ("pallas_open_envelope_builder", record_bundle),
        ("previous_proof_open_envelope_builder", previous_bundle),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        ("permissive_recursive_compact", b"\x00|\x00|\x00"),
        (
            "recursive_compact",
            record_bundle
            + b"|"
            + pallas_open_envelopes
            + b"|"
            + RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        (
            "recursive_compact_verify",
            valid_recursive_compact_token
            + b"|"
            + RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        (
            "recursive_compact_verify",
            forged_recursive_compact_token
            + b"|"
            + RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        ("init", init_request),
        ("append", append_request),
        ("transition-profile-init", transition_init_request),
        ("transition-profile-append", transition_append_request),
        ("lineage-append-boundary", boundary_profile),
        ("lineage-init", lineage_init_request + b"|" + lineage_init_bundle),
        (
            "lineage-append",
            lineage_append_previous_witness
            + b"|"
            + lineage_append_request
            + b"|"
            + lineage_append_bundle,
        ),
        ("verify", verify_request),
        ("redeem", redeem_request),
    ]


def test_recursive_spend_compact_projection_probes_and_delegates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    bundle_archive = _kagemusha_input_archive(0xE1)
    projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD)

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is False
    )
    with pytest.raises(
        RuntimeError,
        match=(
            "recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7"
            ".*compact projection symbol"
        ),
    ):
        projection(bundle_archive)

    def project_bundle(bundle: bytes) -> bytes:
        native._reject_probe("recursive spend compact projection", bundle)
        native.calls.append(("recursive_spend_compact_projection", bundle))
        return _kagemusha_norito_frame_with_payload(0x4F)

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, project_bundle)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is True
    )
    assert projection(bundle_archive) == _kagemusha_norito_frame_with_payload(0x4F)
    assert native.calls[-1] == ("recursive_spend_compact_projection", bundle_archive)

    oversized_bundle_archive = memoryview(bundle_archive + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(bundle_archive),
    )
    invalid_archives = (
        (b"", "must not be empty"),
        (b"\x01", "must be a valid Norito archive"),
        (_kagemusha_norito_frame(0x4C), "must contain a non-empty Norito payload"),
        (oversized_bundle_archive, "must not exceed"),
    )
    calls_before_invalid_archives = list(native.calls)
    for invalid_archive, expected_message in invalid_archives:
        with pytest.raises(ValueError, match=f"bundle_archive {expected_message}"):
            projection(invalid_archive)
        assert native.calls == calls_before_invalid_archives

    def invalid_projection(bundle: bytes) -> bytes:
        native._reject_probe("recursive spend compact projection", bundle)
        return b"\x01"

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, invalid_projection)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is True
    )
    with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
        projection(bundle_archive)


def test_recursive_spend_compact_projection_rejects_permissive_native_probes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    bundle_archive = _kagemusha_input_archive(0xE4)
    projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD)
    probe_calls: list[bytes] = []

    def permissive_project_bundle(bundle: bytes) -> bytes:
        probe_calls.append(bundle)
        return _kagemusha_norito_frame_with_payload(0x50)

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, permissive_project_bundle)

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is False
    )
    with pytest.raises(RuntimeError, match="compact projection symbol"):
        projection(bundle_archive)
    assert probe_calls == [MALFORMED_PROBE_ARCHIVE, MALFORMED_PROBE_ARCHIVE]


@pytest.mark.parametrize(
    ("native_output", "message"),
    (
        (None, "returned no output"),
        (b"", "returned empty output"),
        ("not-norito", "returned text instead of Norito bytes"),
        (_kagemusha_norito_frame(0x51), "returned empty Norito payload"),
    ),
)
def test_recursive_spend_compact_projection_rejects_unsafe_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
    native_output: object,
    message: str,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    bundle_archive = _kagemusha_input_archive(0xE5)
    projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD)

    def unsafe_projection(bundle: bytes) -> object:
        native._reject_probe("recursive spend compact projection", bundle)
        return native_output

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, unsafe_projection)

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is True
    )
    with pytest.raises(RuntimeError, match=message):
        projection(bundle_archive)


def test_recursive_spend_compact_projection_verifier_probes_and_delegates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)
    compact_token = _kagemusha_input_archive(0xE2)
    verifier_record = _kagemusha_input_archive(0xE3)
    verify_projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD)
    verify_projection_at_height = getattr(
        kagemusha,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
    )

    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is False
    )
    with pytest.raises(
        RuntimeError,
        match=(
            "recursive spend compact Kagemusha payment-token projection verifier "
            "requires native bridge ABI 7.*compact projection verifier symbols"
        ),
    ):
        verify_projection(compact_token, verifier_record)

    def verify_without_height(token: bytes, record: bytes) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        native.calls.append(("recursive_spend_compact_projection_verify", token + b"|" + record))
        return False

    def verify_at_height(token: bytes, record: bytes, block_height: int) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        native.calls.append(
            (
                "recursive_spend_compact_projection_verify_at_height",
                token + b"|" + record + b"|" + str(block_height).encode("ascii"),
            )
        )
        return True

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, verify_without_height)
    setattr(
        native,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
        verify_at_height,
    )
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is True
    )
    assert verify_projection(compact_token, verifier_record) is False
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify",
        compact_token + b"|" + verifier_record,
    )
    assert verify_projection(compact_token, verifier_record, block_height=2) is True
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify_at_height",
        compact_token + b"|" + verifier_record + b"|2",
    )
    assert verify_projection_at_height(compact_token, verifier_record, 3) is True
    assert native.calls[-1] == (
        "recursive_spend_compact_projection_verify_at_height",
        compact_token + b"|" + verifier_record + b"|3",
    )

    oversized_verifier_archive = memoryview(compact_token + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(compact_token),
    )
    invalid_archives = (
        (b"", "must not be empty"),
        (b"\x01", "must be a valid Norito archive"),
        (_kagemusha_norito_frame(0x4C), "must contain a non-empty Norito payload"),
        (oversized_verifier_archive, "must not exceed"),
    )
    calls_before_invalid_archives = list(native.calls)
    for invalid_archive, expected_message in invalid_archives:
        with pytest.raises(ValueError, match=f"compact_token_archive {expected_message}"):
            verify_projection(invalid_archive, verifier_record)
        with pytest.raises(ValueError, match=f"verifier_record_archive {expected_message}"):
            verify_projection(compact_token, invalid_archive)
        with pytest.raises(ValueError, match=f"compact_token_archive {expected_message}"):
            verify_projection_at_height(invalid_archive, verifier_record, 2)
        with pytest.raises(ValueError, match=f"verifier_record_archive {expected_message}"):
            verify_projection_at_height(compact_token, invalid_archive, 2)
        assert native.calls == calls_before_invalid_archives

    with pytest.raises(ValueError, match="block_height must be non-negative"):
        verify_projection(compact_token, verifier_record, block_height=-1)
    with pytest.raises(ValueError, match="block_height must be non-negative"):
        verify_projection_at_height(compact_token, verifier_record, -1)
    for bad_height in (True, False, 1.5, "1"):
        with pytest.raises(TypeError, match="block_height must be an integer"):
            verify_projection(
                compact_token,
                verifier_record,
                block_height=bad_height,  # type: ignore[arg-type]
            )
        with pytest.raises(TypeError, match="block_height must be an integer"):
            verify_projection_at_height(
                compact_token,
                verifier_record,
                bad_height,  # type: ignore[arg-type]
            )
    with pytest.raises(ValueError, match="block_height must fit in u64"):
        verify_projection(compact_token, verifier_record, block_height=1 << 64)
    with pytest.raises(ValueError, match="block_height must fit in u64"):
        verify_projection_at_height(compact_token, verifier_record, 1 << 64)

    def invalid_boolean(token: bytes, record: bytes) -> str:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        return "false"

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, invalid_boolean)
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is True
    )
    with pytest.raises(RuntimeError, match="returned non-boolean result"):
        verify_projection(compact_token, verifier_record)

    def invalid_boolean_at_height(
        token: bytes,
        record: bytes,
        block_height: int,
    ) -> bytes:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        return b"not-a-boolean"

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, verify_without_height)
    setattr(
        native,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
        invalid_boolean_at_height,
    )
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
        is True
    )
    with pytest.raises(RuntimeError, match="returned non-boolean result"):
        verify_projection_at_height(compact_token, verifier_record, 4)


def test_recursive_spend_compact_projection_verifier_rejects_permissive_native_probes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    compact_token = _kagemusha_input_archive(0xE6)
    verifier_record = _kagemusha_input_archive(0xE7)
    verify_projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD)

    def rejecting_without_height(token: bytes, record: bytes) -> bool:
        active_native._reject_probe("recursive spend compact projection verifier", token, record)
        return True

    def rejecting_at_height(token: bytes, record: bytes, block_height: int) -> bool:
        active_native._reject_probe("recursive spend compact projection verifier", token, record)
        return True

    def permissive_without_height(token: bytes, record: bytes) -> bool:
        active_native.calls.append(("permissive-verify", token + b"|" + record))
        return True

    def permissive_at_height(token: bytes, record: bytes, block_height: int) -> bool:
        active_native.calls.append(
            (
                "permissive-verify-at-height",
                token + b"|" + record + b"|" + str(block_height).encode("ascii"),
            )
        )
        return True

    for mode in ("without-height", "at-height"):
        active_native = _Native()
        setattr(
            active_native,
            RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD,
            permissive_without_height if mode == "without-height" else rejecting_without_height,
        )
        setattr(
            active_native,
            RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
            permissive_at_height if mode == "at-height" else rejecting_at_height,
        )
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: active_native)

        assert (
            kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available()
            is False
        )
        with pytest.raises(RuntimeError, match="compact projection verifier symbols"):
            verify_projection(compact_token, verifier_record)


def test_recursive_spend_compact_projection_copies_mutable_archives_before_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[object, ...]] = []

    def project_bundle(bundle: bytes) -> bytes:
        native._reject_probe("recursive spend compact projection", bundle)
        calls.append(("projection", bundle))
        return _kagemusha_norito_frame_with_payload(0x52)

    def verify_without_height(token: bytes, record: bytes) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        calls.append(("verify", token, record))
        return True

    def verify_at_height(token: bytes, record: bytes, block_height: int) -> bool:
        native._reject_probe("recursive spend compact projection verifier", token, record)
        calls.append(("verify-at-height", token, record, block_height))
        return True

    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD, project_bundle)
    setattr(native, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD, verify_without_height)
    setattr(
        native,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
        verify_at_height,
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    bundle = bytearray(_kagemusha_input_archive(0xE8))
    compact_token = bytearray(_kagemusha_input_archive(0xE9))
    verifier_record = bytearray(_kagemusha_input_archive(0xEA))
    expected_bundle = bytes(bundle)
    expected_compact_token = bytes(compact_token)
    expected_verifier_record = bytes(verifier_record)
    projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_METHOD)
    verify_projection = getattr(kagemusha, RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_METHOD)
    verify_projection_at_height = getattr(
        kagemusha,
        RECURSIVE_SPEND_COMPACT_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
    )

    assert projection(memoryview(bundle)) == _kagemusha_norito_frame_with_payload(0x52)
    assert verify_projection(compact_token, memoryview(verifier_record)) is True
    assert verify_projection_at_height(memoryview(compact_token), verifier_record, 7) is True

    bundle[6] = 0x7F
    compact_token[6] = 0x7F
    verifier_record[6] = 0x7F

    assert calls == [
        ("projection", expected_bundle),
        ("verify", expected_compact_token, expected_verifier_record),
        ("verify-at-height", expected_compact_token, expected_verifier_record, 7),
    ]


def test_recursive_kagemusha_lineage_helpers_copy_mutable_archives_before_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[str, bytes, ...]] = []

    def lineage_init(request: bytes, bundle: bytes) -> bytes:
        native._reject_probe("lineage init", request, bundle)
        calls.append(("lineage-init", request, bundle))
        return _kagemusha_norito_frame_with_payload(0x58)

    def lineage_append(
        previous_witness: bytes,
        request: bytes,
        bundle: bytes,
    ) -> bytes:
        native._reject_probe("lineage append", previous_witness, request, bundle)
        calls.append(("lineage-append", previous_witness, request, bundle))
        return _kagemusha_norito_frame_with_payload(0x59)

    native.kagemusha_recursive_spend_lineage_witness_from_init_result = lineage_init
    native.kagemusha_recursive_spend_lineage_witness_append_result = lineage_append
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    init_request = bytearray(_kagemusha_input_archive(0xA1))
    init_bundle = bytearray(_kagemusha_input_archive(0xA2))
    previous_witness_storage = bytearray(_kagemusha_input_archive(0xA3))
    append_request = bytearray(_kagemusha_input_archive(0xA4))
    append_bundle = bytearray(_kagemusha_input_archive(0xA5))
    expected_init_request = bytes(init_request)
    expected_init_bundle = bytes(init_bundle)
    expected_previous_witness = bytes(previous_witness_storage)
    expected_append_request = bytes(append_request)
    expected_append_bundle = bytes(append_bundle)

    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            init_request,
            init_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x58)
    )
    assert (
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            memoryview(previous_witness_storage),
            append_request,
            append_bundle,
        )
        == _kagemusha_norito_frame_with_payload(0x59)
    )

    init_request[6] = 0x7F
    init_bundle[6] = 0x7F
    previous_witness_storage[6] = 0x7F
    append_request[6] = 0x7F
    append_bundle[6] = 0x7F

    assert calls == [
        ("lineage-init", expected_init_request, expected_init_bundle),
        (
            "lineage-append",
            expected_previous_witness,
            expected_append_request,
            expected_append_bundle,
        ),
    ]


def test_recursive_compact_payment_token_verifier_rejects_non_boolean_native_result(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def recursive_compact(
        record_bundle: bytes,
        pallas_open_envelopes: bytes,
        key_artifacts: bytes,
    ) -> bytes:
        native._reject_probe("recursive compact", record_bundle, pallas_open_envelopes, key_artifacts)
        return b"recursive_compact"

    def non_boolean_verify(compact_token: bytes, verifier_keys: bytes) -> bytes:
        native._reject_probe("recursive compact verify", compact_token, verifier_keys)
        return b"not-a-boolean"

    setattr(native, RECURSIVE_COMPACT_METHOD, recursive_compact)
    setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, non_boolean_verify)
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is True
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is True
    with pytest.raises(RuntimeError, match="returned non-boolean result"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )


def test_recursive_kagemusha_shared_abi6_fixture_matches_sdk_surface() -> None:
    manifest = _shared_recursive_spend_manifest()
    assert manifest["schema"] == "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1"
    assert (
        kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1
        == "recursive_compact_v1"
    )
    assert kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 7
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1
        == "kagemusha-recursive-compact-v1"
    )
    assert (
        manifest["native_bridge_abi_version"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
    )
    assert manifest["operation_count"] == 9

    operations = manifest["operations"]
    assert isinstance(operations, list)
    assert len(operations) == manifest["operation_count"]
    assert {operation["symbol"] for operation in operations} == {
        "connect_norito_kagemusha_recursive_spend_init",
        "connect_norito_kagemusha_recursive_spend_append",
        "connect_norito_kagemusha_recursive_spend_transition_profile_init",
        "connect_norito_kagemusha_recursive_spend_transition_profile_append",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
        "connect_norito_kagemusha_recursive_spend_verify",
        "connect_norito_kagemusha_recursive_spend_redeem",
    }
    append_witness = next(
        operation
        for operation in operations
        if operation["name"] == "lineage_witness_append_result"
    )
    assert append_witness["input_archives"] == [
        "KagemushaRecursiveSpendLineageWitnessV1",
        "KagemushaRecursiveSpendAppendRequestV1",
        "KagemushaRecursiveSpendBundleV1",
    ]
    assert append_witness["output_archive"] == "KagemushaRecursiveSpendLineageWitnessV1"

    circuit_ids = manifest["proof_circuit_ids"]
    assert isinstance(circuit_ids, dict)
    assert (
        circuit_ids["recursive_aggregation"]
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage_one_hop"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    )
    assert (
        circuit_ids["reserved_lineage_append"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )

    limits = manifest["limits"]
    assert isinstance(limits, dict)
    assert limits["compact_token_max_hops"] == kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
    assert (
        limits["reserved_lineage_witnessless_max_hops"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
    )
    assert (
        limits["previous_proof_open_envelopes_required_count"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
    )
    assert (
        limits["previous_proof_open_envelopes_max_bytes"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
    )
    assert (
        limits["pallas_open_envelope_max_transcript_label_bytes"]
        == kagemusha.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
    )
    assert limits["native_archive_max_bytes"] == kagemusha.KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES

    domains = manifest["domains"]
    assert isinstance(domains, dict)
    assert (
        domains["transition_profile"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
    )
    assert (
        domains["lineage_append_boundary_final_note_binding"]
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
    )

    benchmarks = manifest["payload_benchmarks"]
    assert isinstance(benchmarks, dict)
    assert benchmarks["semantic_payload_bytes"] == 1751
    assert benchmarks["reserved_lineage_payload_bytes"] == 3847
    assert benchmarks["reserved_lineage_transition_profile_bytes"] == 2817

    archive_fixture = _shared_recursive_spend_archives()
    assert (
        archive_fixture["schema"]
        == "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1"
    )
    archives = archive_fixture["archives"]
    assert isinstance(archives, list)
    assert {archive["name"] for archive in archives} == {
        "init_request",
        "init_bundle",
        "transition_profile_init",
        "append_request",
        "append_bundle",
        "transition_profile_append",
        "lineage_append_boundary",
        "lineage_witness_from_init_result",
        "lineage_witness_append_result",
        "verify_request",
        "verify_result",
        "redeem_request",
        "redeem_instruction",
    }
    request_archive_fields = archive_fixture["request_archive_fields"]
    assert isinstance(request_archive_fields, list)
    request_fields_by_type = {
        entry["norito_type"]: entry["fields"] for entry in request_archive_fields
    }
    expected_request_fields = {
        "KagemushaRecursiveSpendInitRequestV1": [
            "record_bundle",
            "pallas_open_envelopes_archive",
            "current_note",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "block_height",
        ],
        "KagemushaRecursiveSpendAppendRequestV1": [
            "previous_bundle",
            "record_bundle",
            "pallas_open_envelopes_archive",
            "current_note",
            "output_proof_circuit_id",
            "previous_lineage_verifier_record",
            "previous_recursive_proof_open_envelopes_archive",
            "lineage_verifier_key",
            "lineage_proving_key_archive",
            "block_height",
        ],
        "KagemushaRecursiveSpendVerifyRequestV1": [
            "bundle",
            "lineage_verifier_record",
            "block_height",
        ],
        "KagemushaRecursiveSpendRedeemRequestV1": [
            "bundle",
            "recipient",
            "public_amount",
            "redeem_proof",
            "lineage_witness",
            "change_output",
            "lineage_verifier_record",
            "block_height",
        ],
    }
    assert set(request_fields_by_type) == set(expected_request_fields)
    for request_type, expected_fields in expected_request_fields.items():
        fields = request_fields_by_type[request_type]
        assert [field["name"] for field in fields] == expected_fields
        block_height = next(field for field in fields if field["name"] == "block_height")
        assert block_height["type"] == "Option<u64>"
        assert block_height["norito_default"] is True
        assert block_height["semantics"] == "verifier_record_activation_height"

    redeem_archive = next(
        archive for archive in archives if archive["name"] == "redeem_request"
    )
    assert redeem_archive["operation"] == "redeem"
    assert redeem_archive["norito_type"] == "KagemushaRecursiveSpendRedeemRequestV1"
    assert (
        redeem_archive["sha256_hex"]
        == "4fbfbe8b05b86c430a3743b0da68b819afca8c666357ef7b2e171b837f97f415"
    )
    assert redeem_archive["byte_len"] > 0
    assert len(base64.b64decode(redeem_archive["bytes_base64"])) > 0
    redeem_instruction_archive = next(
        archive for archive in archives if archive["name"] == "redeem_instruction"
    )
    assert redeem_instruction_archive["norito_type"] == "RedeemKagemushaRecursive"
    assert (
        redeem_instruction_archive["sha256_hex"]
        == "31cd92a5a2f8894634c531830621604937d4631f5f08b58cba01a45dc26e9eba"
    )

    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(1)
        == circuit_ids["reserved_lineage_append"]
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(63)
        == circuit_ids["reserved_lineage_append"]
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(64)
        == circuit_ids["recursive_aggregation"]
    )
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(0)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(63)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(64)
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
    )
    assert not kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        65,
    )


def test_recursive_kagemusha_shared_abi7_fixture_manifest_matches_archives_and_generator() -> None:
    manifest = _shared_recursive_spend_abi7_manifest()
    assert set(manifest) == {
        "schema",
        "fixture_kind",
        "archive_fixture",
        "native_bridge_abi_version",
        "operation_count",
        "generator",
        "domains",
        "operations",
    }
    assert manifest["schema"] == "iroha.kagemusha.recursive_spend.abi7.fixture_manifest.v1"
    assert manifest["fixture_kind"] == "native_bridge_norito_archives"
    assert (
        manifest["native_bridge_abi_version"]
        == kagemusha.KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
    )

    archive_fixture = manifest["archive_fixture"]
    assert isinstance(archive_fixture, dict)
    assert set(archive_fixture) == {"path", "schema"}
    assert (
        archive_fixture["path"]
        == "fixtures/kagemusha_recursive_spend_abi7/archives.json"
    )
    assert (
        archive_fixture["schema"]
        == "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1"
    )

    generator = manifest["generator"]
    assert isinstance(generator, dict)
    assert set(generator) == {"crate", "test", "print_env"}
    assert generator["crate"] == "iroha_python_rs"
    assert (
        generator["test"]
        == "kagemusha_recursive_spend_abi7_archive_fixture_matches_python_native_bridge"
    )
    assert generator["print_env"] == "KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI7_ARCHIVES"

    domains = manifest["domains"]
    assert isinstance(domains, dict)
    assert set(domains) == {"lineage_accumulator", "fixture_label"}
    assert (
        domains["lineage_accumulator"]
        == "iroha:kagemusha:v1:recursive-spend-accumulator"
    )
    assert domains["fixture_label"] == "kagemusha-recursive-spend-python-real"

    expected_operations = {
        "append_bundle": ("append", "KagemushaRecursiveSpendBundleV1", "bundle"),
        "verify_request": (
            "verify",
            "KagemushaRecursiveSpendVerifyRequestV1",
            "request",
        ),
        "verify_result": (
            "verify",
            "KagemushaRecursiveSpendVerifyResultV1",
            "result",
        ),
        "redeem_request": (
            "redeem",
            "KagemushaRecursiveSpendRedeemRequestV1",
            "request",
        ),
        "redeem_instruction": (
            "redeem",
            "RedeemKagemushaRecursive",
            "instruction",
        ),
    }
    operations = manifest["operations"]
    assert isinstance(operations, list)
    assert manifest["operation_count"] == len(expected_operations)
    assert len(operations) == manifest["operation_count"]
    operations_by_name = {
        operation["name"]: operation
        for operation in operations
        if isinstance(operation, dict)
    }
    assert set(operations_by_name) == set(expected_operations)
    for name, (operation, norito_type, archive_kind) in expected_operations.items():
        entry = operations_by_name[name]
        assert set(entry) == {"name", "operation", "norito_type", "archive_kind"}
        assert entry["operation"] == operation
        assert entry["norito_type"] == norito_type
        assert entry["archive_kind"] == archive_kind

    archives = _shared_recursive_spend_abi7_fixture("archives.json")
    assert set(archives) == {
        "schema",
        "fixture_kind",
        "native_bridge_abi_version",
        "archives",
    }
    assert archives["schema"] == archive_fixture["schema"]
    assert archives["fixture_kind"] == "native_bridge_norito_archives"
    assert archives["native_bridge_abi_version"] == manifest["native_bridge_abi_version"]
    archive_entries = archives["archives"]
    assert isinstance(archive_entries, list)
    assert len(archive_entries) == len(expected_operations)
    assert {archive["name"] for archive in archive_entries} == set(expected_operations)
    for archive in archive_entries:
        assert set(archive) == {
            "name",
            "operation",
            "norito_type",
            "byte_len",
            "sha256_hex",
            "bytes_base64",
        }
        operation, norito_type, _archive_kind = expected_operations[archive["name"]]
        assert archive["operation"] == operation
        assert archive["norito_type"] == norito_type
        archive_bytes = base64.b64decode(archive["bytes_base64"])
        assert len(archive_bytes) == archive["byte_len"]
        assert hashlib.sha256(archive_bytes).hexdigest() == archive["sha256_hex"]


def test_recursive_kagemusha_typed_request_codecs_round_trip_shared_fixtures() -> None:
    abi6_result = kagemusha.decode_kagemusha_recursive_spend_verify_result(
        _shared_recursive_spend_archive("verify_result")
    )
    assert abi6_result.valid is False
    assert abi6_result.hop_count == 2
    assert abi6_result.encoded_bytes == 4011
    assert abi6_result.chain_admissible is False
    assert abi6_result.lineage_witness_required is True

    abi7_result = kagemusha.decode_kagemusha_recursive_spend_verify_result(
        _shared_recursive_spend_abi7_archive("verify_result")
    )
    assert abi7_result.valid is True
    assert abi7_result.hop_count == 1
    assert abi7_result.encoded_bytes == 13622
    assert abi7_result.lineage_witness_required is True
    with pytest.raises(ValueError, match=r"Trailing bytes after verify_result"):
        kagemusha.decode_kagemusha_recursive_spend_verify_result(
            _recursive_spend_verify_result_with_trailing_field()
        )

    init_summary = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _shared_recursive_spend_archive("init_bundle")
    )
    assert init_summary.hop_count == 1
    assert (
        init_summary.proof_circuit_id
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    )
    assert init_summary.chain_id == "kagemusha-recursive-spend-abi-chain"
    assert init_summary.asset == "686w6ABhTWPaCrWNjjXs7X1SW6w9"
    fallback_asset_summary = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _recursive_spend_bundle_with_accumulator_field(2, _fixed_array_payload(0x01, 16))
    )
    assert fallback_asset_summary.asset == "hex:01010101010101010101010101010101"
    assert init_summary.current_note.amount == "7"
    assert any(init_summary.initial_root)
    assert any(init_summary.final_root)
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-accumulator"
    )
    assert len(init_summary.topup_anchor_nullifiers) >= 2
    malformed_topup_anchor_sets = (
        [],
        [bytes(32)],
        [
            init_summary.topup_anchor_nullifiers[0],
            init_summary.topup_anchor_nullifiers[1],
            bytes([0x34]) * 32,
        ],
        [init_summary.topup_anchor_nullifiers[0], init_summary.topup_anchor_nullifiers[0]],
        [init_summary.topup_anchor_nullifiers[1], init_summary.topup_anchor_nullifiers[0]],
        [init_summary.current_note.note_commitment],
        [init_summary.current_note.spend_nullifier],
    )
    for nullifiers in malformed_topup_anchor_sets:
        with pytest.raises(ValueError, match=r"bundle\.accumulator\.topup_anchor_nullifiers"):
            kagemusha.decode_kagemusha_recursive_spend_bundle(
                _recursive_spend_bundle_with_topup_anchor_nullifiers(list(nullifiers))
            )

    append_summary = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _shared_recursive_spend_archive("append_bundle")
    )
    assert append_summary.hop_count == 2
    assert (
        append_summary.proof_circuit_id
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )

    abi7_append_summary = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _shared_recursive_spend_abi7_archive("append_bundle")
    )
    assert abi7_append_summary.hop_count == 1
    assert (
        abi7_append_summary.proof_circuit_id
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert abi7_append_summary.asset == "7Y5nGzchCJcxcv98NUoBfwBR1nTk"
    with pytest.raises(ValueError, match=r"bundle\.proof_circuit_id"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_proof_circuit_id(
                UNSUPPORTED_RECURSIVE_SPEND_PROOF_CIRCUIT_ID
            )
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_backend"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_proof_backend(
                UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND
            )
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_backend"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_proof_box_backend(
                UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND
            )
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_backend"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_proof_box_backend_and_empty_proof_bytes(
                UNSUPPORTED_RECURSIVE_SPEND_PROOF_BACKEND
            )
        )
    with pytest.raises(ValueError, match=r"Trailing bytes after recursive_proof"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_recursive_proof_field()
        )
    with pytest.raises(ValueError, match=r"Trailing bytes after verifier_key_id"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_verifier_key_id_field()
        )
    with pytest.raises(ValueError, match=r"Trailing bytes after proof"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_proof_box_field()
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_bytes"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_empty_proof_bytes()
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_public_inputs"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_empty_proof_public_inputs()
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_public_inputs_hash"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_zero_proof_public_inputs_hash()
        )
    with pytest.raises(ValueError, match=r"bundle\.proof_public_inputs_hash"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_mismatched_proof_public_inputs_hash()
        )

    per_element_note_commitment = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _recursive_spend_bundle_with_current_note_field(
            0,
            _fixed_array_payload(0x24, 32),
        )
    )
    assert per_element_note_commitment.current_note.note_commitment == bytes([0x24]) * 32
    per_element_spend_nullifier = kagemusha.decode_kagemusha_recursive_spend_bundle(
        _recursive_spend_bundle_with_current_note_field(
            1,
            _fixed_array_payload(0x25, 32),
        )
    )
    assert per_element_spend_nullifier.current_note.spend_nullifier == bytes([0x25]) * 32

    malformed_current_notes = (
        (
            _recursive_spend_bundle_with_current_note_field(0, bytes(32)),
            "note_commitment",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(1, bytes(32)),
            "spend_nullifier",
        ),
        (
            _recursive_spend_bundle_with_equal_current_note_nullifier(),
            "spend_nullifier",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(2, _zero_numeric_payload()),
            "amount",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                0,
                _fixed_array_payload(0x04, 31),
            ),
            "note_commitment",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                0,
                _fixed_array_payload(0x04, 33),
            ),
            "note_commitment",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                0,
                _count_prefixed_fixed_array_payload(0x04, 32),
            ),
            "note_commitment",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                1,
                _fixed_array_payload(0x05, 31),
            ),
            "spend_nullifier",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                1,
                _fixed_array_payload(0x05, 33),
            ),
            "spend_nullifier",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                1,
                _count_prefixed_fixed_array_payload(0x05, 32),
            ),
            "spend_nullifier",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload(b"\x01", scale=1),
            ),
            "numeric scale",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload_with_scale_payload(
                    _count_prefixed_fixed_array_payload(0x16, 4)
                ),
            ),
            "numeric scale",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload_with_mantissa_payload(
                    (2).to_bytes(4, "little") + b"\x01"
                ),
            ),
            "numeric mantissa length",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload(b"\xff"),
            ),
            "numeric amount",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload(bytes(16) + b"\x01"),
            ),
            "amount",
        ),
        (
            _recursive_spend_bundle_with_current_note_field(
                2,
                _numeric_payload_with_trailing_field(),
            ),
            "amount",
        ),
    )
    for archive, expected_field in malformed_current_notes:
        with pytest.raises(ValueError, match=expected_field):
            kagemusha.decode_kagemusha_recursive_spend_bundle(archive)
    with pytest.raises(ValueError, match=r"Trailing bytes after bundle"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_bundle_field()
        )
    with pytest.raises(ValueError, match=r"Trailing bytes after current_note"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_current_note_field()
        )

    malformed_accumulator_fields = (
        (
            0,
            kagemusha._kagemusha_string(
                "iroha:kagemusha:v1:recursive-spend-accumulator-digest"
            ),
            r"bundle\.accumulator\.domain",
        ),
        (
            0,
            kagemusha._kagemusha_string(
                " iroha:kagemusha:v1:recursive-spend-accumulator"
            ),
            r"bundle\.accumulator\.domain",
        ),
        (
            0,
            kagemusha._kagemusha_string(
                "iroha:Kagemusha:v1:recursive-spend-accumulator"
            ),
            r"bundle\.accumulator\.domain",
        ),
        (
            1,
            kagemusha._kagemusha_string("kagemusha-recursive-spend-abi-chain"),
            r"bundle\.accumulator\.chain_id",
        ),
        (
            1,
            kagemusha._kagemusha_field(kagemusha._kagemusha_string("")),
            r"bundle\.accumulator\.chain_id",
        ),
        (
            1,
            kagemusha._kagemusha_field(
                kagemusha._kagemusha_string(" kagemusha-recursive-spend-abi-chain")
            ),
            r"bundle\.accumulator\.chain_id",
        ),
        (
            1,
            kagemusha._kagemusha_field(
                kagemusha._kagemusha_string("kagemusha-recursive-spend-abi-chain ")
            ),
            r"bundle\.accumulator\.chain_id",
        ),
        (
            1,
            kagemusha._kagemusha_field(
                kagemusha._kagemusha_string("kagemusha recursive-spend-abi-chain")
            ),
            r"bundle\.accumulator\.chain_id",
        ),
        (3, bytes(32), r"bundle\.accumulator\.initial_root"),
        (4, bytes(32), r"bundle\.accumulator\.final_root"),
        (4, init_summary.initial_root, r"bundle\.accumulator\.final_root"),
        (2, _fixed_array_payload(0x01, 15), r"bundle\.accumulator\.asset"),
        (2, _count_prefixed_fixed_array_payload(0x01, 16), r"bundle\.accumulator\.asset"),
        (2, _fixed_array_payload(0x01, 17), r"bundle\.accumulator\.asset"),
        (3, _fixed_array_payload(0x02, 31), r"bundle\.accumulator\.initial_root"),
        (
            3,
            _count_prefixed_fixed_array_payload(0x02, 32),
            r"bundle\.accumulator\.initial_root",
        ),
        (3, _fixed_array_payload(0x02, 33), r"bundle\.accumulator\.initial_root"),
        (4, _fixed_array_payload(0x03, 31), r"bundle\.accumulator\.final_root"),
        (
            4,
            _count_prefixed_fixed_array_payload(0x03, 32),
            r"bundle\.accumulator\.final_root",
        ),
        (4, _fixed_array_payload(0x03, 33), r"bundle\.accumulator\.final_root"),
        (6, (0).to_bytes(4, "little"), r"bundle\.accumulator\.hop_count"),
        (
            6,
            _count_prefixed_fixed_array_payload(0x06, 4),
            r"bundle\.accumulator\.hop_count",
        ),
        (
            6,
            (
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
                + 1
            ).to_bytes(4, "little"),
            r"bundle\.accumulator\.hop_count",
        ),
        (7, bytes(32), r"bundle\.accumulator\.lineage_digest"),
        (7, _fixed_array_payload(0x07, 31), r"bundle\.accumulator\.lineage_digest"),
        (
            7,
            _count_prefixed_fixed_array_payload(0x07, 32),
            r"bundle\.accumulator\.lineage_digest",
        ),
        (7, _fixed_array_payload(0x07, 33), r"bundle\.accumulator\.lineage_digest"),
        (
            8,
            bytes([0x7D]) * 32,
            r"bundle\.accumulator\.aggregation_transcript_digest",
        ),
        (8, bytes(32), r"bundle\.accumulator\.aggregation_transcript_digest"),
        (9, bytes(32), r"bundle\.accumulator\.nullifier_digest"),
        (10, bytes(32), r"bundle\.accumulator\.output_commitment_digest"),
        (11, bytes(32), r"bundle\.accumulator\.fold_digest"),
        (12, bytes(32), r"bundle\.accumulator\.recursive_proof_chain_digest"),
        (13, bytes(32), r"bundle\.accumulator\.transition_profile_binding_digest"),
        (
            14,
            bytes([0x7E]) * 32,
            r"bundle\.accumulator\.append_opening_preflight_digest",
        ),
        (
            14,
            _fixed_array_payload(0x0E, 31),
            r"bundle\.accumulator\.append_opening_preflight_digest",
        ),
        (
            14,
            _count_prefixed_fixed_array_payload(0x0E, 32),
            r"bundle\.accumulator\.append_opening_preflight_digest",
        ),
        (
            14,
            _fixed_array_payload(0x0E, 33),
            r"bundle\.accumulator\.append_opening_preflight_digest",
        ),
        (
            15,
            bytes([0x7F]) * 32,
            r"bundle\.accumulator\.append_boundary_digest",
        ),
        (16, bytes(32), r"bundle\.accumulator\.verifier_params_fingerprint"),
        (17, bytes(32), r"bundle\.accumulator\.fixed_window_table_schedule_digest"),
        (
            18,
            bytes(32),
            r"bundle\.accumulator\.fixed_window_shared_table_manifest_digest",
        ),
        (19, bytes(32), r"bundle\.accumulator\.fixed_window_table_base_digest"),
        (20, bytes(32), r"bundle\.accumulator\.verifier_witness_batch_digest"),
        (
            20,
            _fixed_array_payload(0x14, 31),
            r"bundle\.accumulator\.verifier_witness_batch_digest",
        ),
        (
            20,
            _count_prefixed_fixed_array_payload(0x14, 32),
            r"bundle\.accumulator\.verifier_witness_batch_digest",
        ),
        (
            20,
            _fixed_array_payload(0x14, 33),
            r"bundle\.accumulator\.verifier_witness_batch_digest",
        ),
        (21, (3).to_bytes(4, "little"), r"bundle\.accumulator\.verifier_opening_len"),
        (
            21,
            _count_prefixed_fixed_array_payload(0x15, 4),
            r"bundle\.accumulator\.verifier_opening_len",
        ),
    )
    for field_index, replacement, expected in malformed_accumulator_fields:
        with pytest.raises(ValueError, match=expected):
            kagemusha.decode_kagemusha_recursive_spend_bundle(
                _recursive_spend_bundle_with_accumulator_field(
                    field_index,
                    replacement,
                )
            )
    with pytest.raises(ValueError, match=r"Trailing bytes after accumulator"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(
            _recursive_spend_bundle_with_trailing_accumulator_field()
        )

    record_bundle = _synthetic_kagemusha_record_bundle_archive()
    pallas = _synthetic_pallas_open_envelopes_archive()
    verifier_record = _recursive_spend_verifier_record()
    note = _recursive_spend_note()
    init_artifacts = _recursive_spend_lineage_artifacts_for_init()
    append_artifacts = _recursive_spend_lineage_artifacts_for_append()

    init_request = kagemusha.KagemushaRecursiveSpendInitRequest(
        record_bundle=record_bundle,
        pallas_open_envelopes=pallas,
        current_note=note,
        lineage_key_artifacts=init_artifacts,
        block_height=7,
    )
    _assert_kagemusha_archive_schema(
        kagemusha.encode_kagemusha_recursive_spend_init_request(init_request),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
    )

    append_request = kagemusha.KagemushaRecursiveSpendAppendRequest(
        previous_bundle=_shared_recursive_spend_archive("init_bundle"),
        record_bundle=record_bundle,
        pallas_open_envelopes=pallas,
        current_note=note,
        previous_lineage_verifier_record=verifier_record,
        block_height=8,
    )
    _assert_kagemusha_archive_schema(
        kagemusha.encode_kagemusha_recursive_spend_append_request(append_request),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
    )

    lineage_append_request = kagemusha.KagemushaRecursiveSpendAppendRequest(
        previous_bundle=_shared_recursive_spend_archive("init_bundle"),
        record_bundle=record_bundle,
        pallas_open_envelopes=pallas,
        current_note=note,
        output_proof_circuit_id=(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        ),
        previous_lineage_verifier_record=verifier_record,
        previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(),
        lineage_key_artifacts=append_artifacts,
        block_height=8,
    )
    _assert_kagemusha_archive_schema(
        kagemusha.encode_kagemusha_recursive_spend_append_request(lineage_append_request),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
    )

    verify_request = kagemusha.KagemushaRecursiveSpendVerifyRequest(
        bundle=_shared_recursive_spend_archive("init_bundle"),
        lineage_verifier_record=verifier_record,
        block_height=9,
    )
    _assert_kagemusha_archive_schema(
        kagemusha.encode_kagemusha_recursive_spend_verify_request(verify_request),
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
    )

    redeem_request = kagemusha.KagemushaRecursiveSpendRedeemRequest(
        bundle=_shared_recursive_spend_archive("init_bundle"),
        recipient=_recursive_spend_recipient(),
        public_amount="6",
        redeem_proof=_synthetic_kagemusha_archive(
            kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
            0x64,
        ),
        lineage_witness=_shared_recursive_spend_archive("lineage_witness_append_result"),
        change_output=bytes(range(0x80, 0xA0)),
        lineage_verifier_record=verifier_record,
        block_height=10,
    )
    redeem_payload = _kagemusha_archive_payload(
        kagemusha.encode_kagemusha_recursive_spend_redeem_request(redeem_request),
        kagemusha.KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    )
    redeem_fields = _read_all_fields(redeem_payload)
    assert len(redeem_fields) == 8
    recipient_payload = redeem_fields[1]
    assert recipient_payload[:4] == (0).to_bytes(4, "little")
    recipient_key_payload, recipient_offset = _read_field(recipient_payload, 4)
    assert recipient_offset == len(recipient_payload)
    assert recipient_key_payload[:8] == (33).to_bytes(8, "little")
    assert _read_fixed_bytes_payload(recipient_key_payload[8:], 33) == (
        b"\x00" + bytes([0x24]) * 32
    )
    assert redeem_fields[2] == bytes((6,)) + b"\x00" * 15
    assert redeem_fields[4][0] == 1
    change_payload = _read_option_some(redeem_fields[5])
    assert change_payload[:8] != (32).to_bytes(8, "little")
    assert _read_fixed_bytes_payload(change_payload, 32) == bytes(range(0x80, 0xA0))
    assert redeem_fields[6][0] == 1
    assert redeem_fields[7][0] == 1

    exact_redeem_payload = _kagemusha_archive_payload(
        kagemusha.encode_kagemusha_recursive_spend_redeem_request(
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient=_recursive_spend_recipient(),
                public_amount="7",
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x66,
                ),
                lineage_verifier_record=verifier_record,
            )
        ),
        kagemusha.KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    )
    exact_redeem_fields = _read_all_fields(exact_redeem_payload)
    assert len(exact_redeem_fields) == 8
    _assert_option_none(exact_redeem_fields[4])
    _assert_option_none(exact_redeem_fields[5])
    assert exact_redeem_fields[6][0] == 1
    _assert_option_none(exact_redeem_fields[7])


def test_recursive_kagemusha_redeem_request_rejects_legacy_public_key_layout() -> None:
    redeem_request = kagemusha.KagemushaRecursiveSpendRedeemRequest(
        bundle=_shared_recursive_spend_archive("init_bundle"),
        recipient=_recursive_spend_recipient(),
        public_amount="7",
        redeem_proof=_synthetic_kagemusha_archive(
            kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
            0x68,
        ),
        lineage_verifier_record=_recursive_spend_verifier_record(),
    )
    valid_archive = kagemusha.encode_kagemusha_recursive_spend_redeem_request(
        redeem_request
    )
    valid_payload = _kagemusha_archive_payload(
        valid_archive,
        kagemusha.KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    )
    fields = _read_all_fields(valid_payload)
    old_public_key_payload = _legacy_no_length_const_vec_u8_payload(
        b"\x00" + bytes([0x24]) * 32
    )
    assert old_public_key_payload[:8] != (33).to_bytes(8, "little")
    fields[1] = (0).to_bytes(4, "little") + kagemusha._kagemusha_field(
        old_public_key_payload
    )
    malformed_archive = kagemusha._kagemusha_norito_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
        _encode_test_fields(fields),
    )

    try:
        kagemusha.load_crypto_extension()
    except RuntimeError as exc:
        pytest.skip(f"native extension unavailable: {exc}")

    with pytest.raises(ValueError, match="invalid Kagemusha recursive spend redeem archive"):
        kagemusha.kagemusha_recursive_spend_redeem(malformed_archive)


def test_recursive_kagemusha_typed_request_codecs_reject_malformed_inputs() -> None:
    invalid_amounts = (
        "",
        "0",
        "00",
        "01",
        "0007",
        "-1",
        "+1",
        "1.0",
        "1e3",
        "7 ",
        " 7",
        "\t7",
        "7\n",
        str(1 << 128),
        "9" * 40,
    )
    for amount in invalid_amounts:
        with pytest.raises(ValueError):
            _recursive_spend_note(amount=amount)

    with pytest.raises(ValueError, match="note_commitment"):
        _recursive_spend_note(commitment_seed=0)
    with pytest.raises(ValueError, match="spend_nullifier"):
        _recursive_spend_note(commitment_seed=0x22, nullifier_seed=0x22)
    with pytest.raises(ValueError, match="exactly 32 bytes"):
        kagemusha.KagemushaRecursiveSpendableNoteDescriptor(b"\x01", bytes([2]) * 32, "1")

    record_bundle = _synthetic_kagemusha_record_bundle_archive()
    pallas = _synthetic_pallas_open_envelopes_archive()
    note = _recursive_spend_note()
    verifier_record = _recursive_spend_verifier_record()
    init_artifacts = _recursive_spend_lineage_artifacts_for_init(0x95)
    append_artifacts = _recursive_spend_lineage_artifacts_for_append(0x97)
    invalid_block_heights = (
        True,
        False,
        1.5,
        "1",
        "00",
        "01",
        "0007",
        "-0",
        "+7",
        "7 ",
        " 7",
        "18446744073709551616",
        -1,
        1 << 64,
    )
    block_height_request_builders = (
        lambda block_height: kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_key_artifacts=init_artifacts,
            block_height=block_height,
        ),
        lambda block_height: kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            block_height=block_height,
        ),
        lambda block_height: kagemusha.KagemushaRecursiveSpendVerifyRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            block_height=block_height,
        ),
        lambda block_height: kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="1",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x7A,
            ),
            block_height=block_height,
        ),
    )
    for build_request in block_height_request_builders:
        for block_height in invalid_block_heights:
            with pytest.raises((TypeError, ValueError), match="block_height"):
                build_request(block_height)
    invalid_public_amounts = (
        "",
        "0",
        "00",
        "01",
        "0007",
        "-1",
        "+1",
        "1.0",
        "1e3",
        "7 ",
        " 7",
        "\t7",
        "7\n",
        str(1 << 128),
        "9" * 40,
    )
    for public_amount in invalid_public_amounts:
        with pytest.raises(ValueError, match="public_amount"):
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient=_recursive_spend_recipient(),
                public_amount=public_amount,
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x7B,
                ),
            )
    for change_output, error_match in (
        (b"\x01" * 31, "change_output must be exactly 32 bytes"),
        (b"\x00" * 32, "change_output must be non-zero"),
    ):
        with pytest.raises(ValueError, match=error_match):
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient=_recursive_spend_recipient(),
                public_amount="7",
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x7C,
                ),
                change_output=change_output,
            )
    partial_bundle = _shared_recursive_spend_abi7_archive("append_bundle")
    partial_summary = kagemusha.decode_kagemusha_recursive_spend_bundle(partial_bundle)
    assert partial_summary.topup_anchor_nullifiers
    for change_output in (
        partial_summary.current_note.note_commitment,
        partial_summary.current_note.spend_nullifier,
        partial_summary.topup_anchor_nullifiers[0],
    ):
        with pytest.raises(ValueError, match="change_output must not reuse"):
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=partial_bundle,
                recipient=_recursive_spend_recipient(),
                public_amount="6",
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x7D,
                ),
                change_output=change_output,
            )
    with pytest.raises(ValueError, match="change_output is required"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="6",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x7D,
            ),
        )
    with pytest.raises(ValueError, match="public_amount must not exceed"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="8",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x7F,
            ),
        )
    for public_amount in ("7", "8"):
        with pytest.raises(ValueError, match="public_amount must be less"):
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient=_recursive_spend_recipient(),
                public_amount=public_amount,
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x7E,
                ),
                change_output=bytes([0x42]) * 32,
            )
    with pytest.raises(ValueError, match="lineage_witness is required"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x80,
            ),
        )
    semantic_missing_witness_redeem_proof = b""
    with pytest.raises(ValueError, match="lineage_witness is required"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=semantic_missing_witness_redeem_proof,
        )
    with pytest.raises(ValueError, match="lineage_verifier_record is required"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x81,
            ),
        )
    reserved_missing_record_redeem_proof = b""
    with pytest.raises(ValueError, match="lineage_verifier_record is required"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=reserved_missing_record_redeem_proof,
        )
    with pytest.raises(ValueError, match="lineage_witness"):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x82,
            ),
            lineage_witness=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
                0x7C,
            ),
            lineage_verifier_record=verifier_record,
        )
    malformed_lineage_witnesses = (
        (_recursive_spend_lineage_witness_with_trailing_field(), r"lineage_witness"),
        (
            _recursive_spend_lineage_witness_with_trailing_previous_proofs_field(),
            r"lineage_witness\.previous_recursive_proofs",
        ),
        (
            _recursive_spend_lineage_witness_with_trailing_previous_proof_field(),
            r"lineage_witness\.previous_recursive_proofs",
        ),
        (
            _recursive_spend_lineage_witness_with_trailing_previous_verifier_key_id_field(),
            r"lineage_witness\.previous_recursive_proofs\.verifier_key_id",
        ),
        (
            _recursive_spend_lineage_witness_with_previous_proof_field(1, b""),
            r"lineage_witness\.previous_recursive_proofs\.proof_public_inputs",
        ),
        (
            _recursive_spend_lineage_witness_with_previous_proof_field(2, bytes(32)),
            r"lineage_witness\.previous_recursive_proofs\.proof_public_inputs_hash",
        ),
        (
            _recursive_spend_lineage_witness_with_previous_proof_field(
                2,
                b"\x44" * 32,
            ),
            r"lineage_witness\.previous_recursive_proofs\.proof_public_inputs_hash",
        ),
        (
            _recursive_spend_lineage_witness_with_previous_proof_box_backend("halo2/kzg"),
            r"lineage_witness\.previous_recursive_proofs\.proof_backend",
        ),
        (
            _recursive_spend_lineage_witness_with_previous_proof_box_backend_and_empty_proof_bytes(
                "halo2/kzg"
            ),
            r"lineage_witness\.previous_recursive_proofs\.proof_backend",
        ),
        (
            _recursive_spend_lineage_witness_with_empty_previous_proof_bytes(),
            r"lineage_witness\.previous_recursive_proofs\.proof_bytes",
        ),
    )
    for lineage_witness_archive, expected_error in malformed_lineage_witnesses:
        with pytest.raises(ValueError, match=expected_error):
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient=_recursive_spend_recipient(),
                public_amount="7",
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x83,
                ),
                lineage_witness=lineage_witness_archive,
                lineage_verifier_record=verifier_record,
            )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_proving_key_archive=_synthetic_kagemusha_archive("test::Key", 0x73),
        )
    with pytest.raises(ValueError, match="pallas_open_envelopes"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=_synthetic_kagemusha_archive(
                "test::PallasOpenEnvelopes",
                0x72,
            ),
            current_note=note,
            lineage_key_artifacts=init_artifacts,
        )
    with pytest.raises(ValueError, match="pallas_open_envelopes"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=_synthetic_pallas_open_envelopes_archive(2),
            current_note=note,
            lineage_key_artifacts=init_artifacts,
        )
    with pytest.raises(ValueError, match="pallas_open_envelopes"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=_synthetic_pallas_open_envelopes_archive(
                include_domain_tag=False,
            ),
            current_note=note,
            lineage_key_artifacts=init_artifacts,
        )
    for malformed_transcript_label in ("", "\u00e9" * 65):
        with pytest.raises(
            ValueError,
            match=r"pallas_open_envelopes\[0\]\.transcript_label is invalid",
        ):
            kagemusha.KagemushaRecursiveSpendInitRequest(
                record_bundle=record_bundle,
                pallas_open_envelopes=_synthetic_pallas_open_envelopes_archive(
                    transcript_label=malformed_transcript_label,
                ),
                current_note=note,
                lineage_key_artifacts=init_artifacts,
            )
    malformed_pallas_metadata_payloads = (
        (
            "vk_commitment",
            {"vk_commitment_payload": _fixed_array_payload(0x70, 32)},
            r"pallas_open_envelopes\[0\]\.vk_commitment must be exactly 32 bytes",
        ),
        (
            "vk_commitment",
            {"vk_commitment_option_payload": _option_raw_with_trailing_byte(_fixed32(0x70))},
            r"pallas_open_envelopes\[0\]\.vk_commitment payload length mismatch",
        ),
        (
            "vk_commitment",
            {"vk_commitment_option_payload": _option_raw_with_unknown_tag()},
            r"pallas_open_envelopes\[0\]\.vk_commitment option tag must be 0 or 1",
        ),
        (
            "vk_commitment",
            {
                "vk_commitment_option_payload": _option_raw_with_declared_length_too_long(
                    _fixed32(0x70)
                )
            },
            r"pallas_open_envelopes\[0\]\.vk_commitment payload length mismatch",
        ),
        (
            "public_inputs_schema_hash",
            {"public_inputs_schema_hash_payload": _fixed_array_payload(0x71, 32)},
            r"pallas_open_envelopes\[0\]\.public_inputs_schema_hash must be exactly 32 bytes",
        ),
        (
            "public_inputs_schema_hash",
            {
                "public_inputs_schema_hash_option_payload": _option_raw_with_trailing_byte(
                    _fixed32(0x71)
                )
            },
            r"pallas_open_envelopes\[0\]\.public_inputs_schema_hash payload length mismatch",
        ),
        (
            "public_inputs_schema_hash",
            {"public_inputs_schema_hash_option_payload": _option_raw_with_unknown_tag()},
            r"pallas_open_envelopes\[0\]\.public_inputs_schema_hash option tag must be 0 or 1",
        ),
        (
            "public_inputs_schema_hash",
            {
                "public_inputs_schema_hash_option_payload": _option_raw_with_declared_length_too_long(
                    _fixed32(0x71)
                )
            },
            r"pallas_open_envelopes\[0\]\.public_inputs_schema_hash payload length mismatch",
        ),
        (
            "domain_tag",
            {"domain_tag_payload": _fixed_array_payload(0x72, 32)},
            r"pallas_open_envelopes\[0\]\.domain_tag must be exactly 32 bytes",
        ),
        (
            "domain_tag",
            {"domain_tag_option_payload": _option_raw_with_trailing_byte(_fixed32(0x72))},
            r"pallas_open_envelopes\[0\]\.domain_tag payload length mismatch",
        ),
        (
            "domain_tag",
            {"domain_tag_option_payload": _option_raw_with_unknown_tag()},
            r"pallas_open_envelopes\[0\]\.domain_tag option tag must be 0 or 1",
        ),
        (
            "domain_tag",
            {
                "domain_tag_option_payload": _option_raw_with_declared_length_too_long(
                    _fixed32(0x72)
                )
            },
            r"pallas_open_envelopes\[0\]\.domain_tag payload length mismatch",
        ),
    )
    for _metadata_field, metadata_kwargs, expected in malformed_pallas_metadata_payloads:
        with pytest.raises(ValueError, match=expected):
            kagemusha.KagemushaRecursiveSpendInitRequest(
                record_bundle=record_bundle,
                pallas_open_envelopes=_synthetic_pallas_open_envelopes_archive(
                    **metadata_kwargs,
                ),
                current_note=note,
                lineage_key_artifacts=init_artifacts,
            )
    with pytest.raises(ValueError, match="block_height"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_verifier_key=b"vk",
            lineage_proving_key_archive=_synthetic_kagemusha_archive("test::Key", 0x74),
            block_height=-1,
        )
    with pytest.raises(ValueError, match="lineage_key_artifacts"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_key_artifacts=append_artifacts,
        )
    with pytest.raises(ValueError, match="lineage_key_artifacts"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_verifier_key=init_artifacts.lineage_verifier_key,
            lineage_proving_key_archive=init_artifacts.lineage_proving_key_archive,
            lineage_key_artifacts=init_artifacts,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_verifier_key=init_artifacts.lineage_verifier_key,
            lineage_proving_key_archive=append_artifacts.lineage_proving_key_archive,
        )

    with pytest.raises(ValueError, match="previous_lineage_verifier_record"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
        )
    with pytest.raises(
        ValueError,
        match="output_proof_circuit_id is not valid for the previous bundle",
    ):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            output_proof_circuit_id="kagemusha-recursive-spend-invalid-output-v1",
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            lineage_key_artifacts=append_artifacts,
        )
    previous_openings_without_lineage_record = _synthetic_pallas_open_envelopes_archive()
    with pytest.raises(ValueError, match="previous_lineage_verifier_record"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            output_proof_circuit_id=(
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            ),
            previous_proof_open_envelopes=previous_openings_without_lineage_record,
        )
    with pytest.raises(
        ValueError,
        match="previous_lineage_verifier_record is only valid for lineage previous bundles",
    ):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
        )
    with pytest.raises(
        ValueError,
        match="previous_proof_open_envelopes are only valid for lineage append output",
    ):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(),
        )
    with pytest.raises(ValueError, match="previous_lineage_verifier_record"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record={
                "verifier_key_id": "malformedPreviousLineageRecordBeforeOpenings",
                "record_bytes": b"\x00",
            },
            previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(),
        )
    with pytest.raises(ValueError, match="previous_proof_open_envelopes"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            output_proof_circuit_id=(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            ),
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            lineage_verifier_key=b"vk",
            lineage_proving_key_archive=_synthetic_kagemusha_archive("test::Key", 0x75),
        )
    with pytest.raises(ValueError, match="lineage_key_artifacts"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            output_proof_circuit_id=(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            ),
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(),
            lineage_key_artifacts=init_artifacts,
        )
    with pytest.raises(ValueError, match="previous_proof_open_envelopes"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            output_proof_circuit_id=(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            ),
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(2),
            lineage_key_artifacts=append_artifacts,
        )
    for malformed_transcript_label in ("", "\u00e9" * 65):
        with pytest.raises(
            ValueError,
            match=r"previous_proof_open_envelopes\[0\]\.transcript_label is invalid",
        ):
            kagemusha.KagemushaRecursiveSpendAppendRequest(
                previous_bundle=_shared_recursive_spend_archive("init_bundle"),
                record_bundle=record_bundle,
                pallas_open_envelopes=pallas,
                current_note=note,
                output_proof_circuit_id=(
                    kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                ),
                previous_lineage_verifier_record=_recursive_spend_verifier_record(),
                previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(
                    transcript_label=malformed_transcript_label,
                ),
                lineage_key_artifacts=append_artifacts,
            )
    for _metadata_field, metadata_kwargs, expected in malformed_pallas_metadata_payloads:
        with pytest.raises(
            ValueError,
            match=expected.replace("pallas_open_envelopes", "previous_proof_open_envelopes"),
        ):
            kagemusha.KagemushaRecursiveSpendAppendRequest(
                previous_bundle=_shared_recursive_spend_archive("init_bundle"),
                record_bundle=record_bundle,
                pallas_open_envelopes=pallas,
                current_note=note,
                output_proof_circuit_id=(
                    kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                ),
                previous_lineage_verifier_record=_recursive_spend_verifier_record(),
                previous_proof_open_envelopes=_synthetic_pallas_open_envelopes_archive(
                    **metadata_kwargs,
                ),
                lineage_key_artifacts=append_artifacts,
            )
    with pytest.raises(ValueError, match="lineage_key_artifacts"):
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record=_recursive_spend_verifier_record(),
            lineage_key_artifacts=append_artifacts,
        )

    wrong_bundle_schema = _synthetic_kagemusha_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
        0x76,
    )
    with pytest.raises(ValueError, match="bundle"):
        kagemusha.encode_kagemusha_recursive_spend_verify_request(
            kagemusha.KagemushaRecursiveSpendVerifyRequest(bundle=wrong_bundle_schema)
        )
    with pytest.raises(
        ValueError,
        match="lineage_verifier_record is required for reserved-lineage bundles",
    ):
        kagemusha.KagemushaRecursiveSpendVerifyRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
        )
    with pytest.raises(ValueError, match="lineage_verifier_record is only valid"):
        kagemusha.KagemushaRecursiveSpendVerifyRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            lineage_verifier_record=verifier_record,
        )
    with pytest.raises(ValueError, match="lineage_verifier_record is only valid"):
        kagemusha.KagemushaRecursiveSpendVerifyRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            lineage_verifier_record={
                "verifier_key_id": "danglingVerifyLineageRecord",
                "record_bytes": b"\x00",
            },
        )
    with pytest.raises(
        ValueError,
        match="lineage_verifier_record is only valid for reserved-lineage bundles or lineage witnesses",
    ):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x77,
            ),
            lineage_verifier_record=verifier_record,
        )
    with pytest.raises(
        ValueError,
        match="lineage_verifier_record is only valid for reserved-lineage bundles or lineage witnesses",
    ):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x77,
            ),
            lineage_verifier_record={
                "verifier_key_id": "danglingRedeemLineageRecord",
                "record_bytes": b"\x00",
            },
        )
    with pytest.raises(
        ValueError,
        match="lineage_verifier_record is required for lineage witnesses with reserved-lineage previous proofs",
    ):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x78,
            ),
            lineage_witness=_shared_recursive_spend_archive("lineage_witness_append_result"),
        )
    with pytest.raises(
        ValueError,
        match="lineage_verifier_record is only valid for reserved-lineage bundles or lineage witnesses",
    ):
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x79,
            ),
            lineage_witness=_shared_recursive_spend_archive("lineage_witness_from_init_result"),
            lineage_verifier_record=verifier_record,
        )
    kagemusha.KagemushaRecursiveSpendRedeemRequest(
        bundle=_shared_recursive_spend_abi7_archive("append_bundle"),
        recipient=_recursive_spend_recipient(),
        public_amount="7",
        redeem_proof=_synthetic_kagemusha_archive(
            kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
            0x7A,
        ),
        lineage_witness=_shared_recursive_spend_archive("lineage_witness_append_result"),
        lineage_verifier_record=verifier_record,
    )
    with pytest.raises(ValueError, match="recipient"):
        kagemusha.encode_kagemusha_recursive_spend_redeem_request(
            kagemusha.KagemushaRecursiveSpendRedeemRequest(
                bundle=_shared_recursive_spend_archive("init_bundle"),
                recipient="alice@wonderland",
                public_amount="7",
                redeem_proof=_synthetic_kagemusha_archive(
                    kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    0x77,
                ),
                lineage_verifier_record=_recursive_spend_verifier_record(),
            )
        )
    with pytest.raises(ValueError, match="unsupported recipient curve"):
        kagemusha._kagemusha_public_key_payload(0xFF, bytes([0x01]) * 32)
    tampered = bytearray(_shared_recursive_spend_archive("init_bundle"))
    tampered[6] ^= 0x7F
    with pytest.raises(ValueError, match="bundle"):
        kagemusha.decode_kagemusha_recursive_spend_bundle(bytes(tampered))


def test_recursive_kagemusha_typed_helpers_delegate_encoded_requests(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def verifying_verify(request: bytes) -> bytes:
        native._reject_probe("verify", request)
        native.calls.append(("verify", request))
        return _shared_recursive_spend_archive("verify_result")

    native.kagemusha_recursive_spend_verify = verifying_verify
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    record_bundle = _synthetic_kagemusha_record_bundle_archive()
    pallas = _synthetic_pallas_open_envelopes_archive()
    verifier_record = _recursive_spend_verifier_record()
    note = _recursive_spend_note()
    init_artifacts = _recursive_spend_lineage_artifacts_for_init(0x99)

    init_output = kagemusha.kagemusha_recursive_spend_init_typed(
        kagemusha.KagemushaRecursiveSpendInitRequest(
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            lineage_key_artifacts=init_artifacts,
        )
    )
    assert init_output.startswith(b"NRT0")
    assert native.calls[-1][0] == "init"
    _assert_kagemusha_archive_schema(
        native.calls[-1][1],
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
    )

    append_output = kagemusha.kagemusha_recursive_spend_append_typed(
        kagemusha.KagemushaRecursiveSpendAppendRequest(
            previous_bundle=_shared_recursive_spend_archive("init_bundle"),
            record_bundle=record_bundle,
            pallas_open_envelopes=pallas,
            current_note=note,
            previous_lineage_verifier_record=verifier_record,
        )
    )
    assert append_output.startswith(b"NRT0")
    assert native.calls[-1][0] == "append"
    _assert_kagemusha_archive_schema(
        native.calls[-1][1],
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
    )

    verify_result = kagemusha.kagemusha_recursive_spend_verify_typed(
        kagemusha.KagemushaRecursiveSpendVerifyRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            lineage_verifier_record=verifier_record,
        )
    )
    assert verify_result.hop_count == 2
    assert native.calls[-1][0] == "verify"
    _assert_kagemusha_archive_schema(
        native.calls[-1][1],
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
    )

    redeem_output = kagemusha.kagemusha_recursive_spend_redeem_typed(
        kagemusha.KagemushaRecursiveSpendRedeemRequest(
            bundle=_shared_recursive_spend_archive("init_bundle"),
            recipient=_recursive_spend_recipient(),
            public_amount="7",
            redeem_proof=_synthetic_kagemusha_archive(
                kagemusha.KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                0x84,
            ),
            lineage_witness=_shared_recursive_spend_archive("lineage_witness_append_result"),
            lineage_verifier_record=verifier_record,
        )
    )
    assert redeem_output.startswith(b"NRT0")
    assert native.calls[-1][0] == "redeem"
    _assert_kagemusha_archive_schema(
        native.calls[-1][1],
        kagemusha.KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
    )


def test_recursive_kagemusha_availability_rejects_permissive_native_probes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for method_name in (
        "kagemusha_prove_verified_compact_payment_token_with_records",
        RECURSIVE_AGGREGATION_METHOD,
        RECURSIVE_COMPACT_METHOD,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        *RECURSIVE_SPEND_METHODS,
    ):
        native = _Native()
        if method_name == RECURSIVE_AGGREGATION_METHOD:
            setattr(native, method_name, lambda record, pallas: b"accepted")
        elif method_name == RECURSIVE_COMPACT_METHOD:
            setattr(native, method_name, lambda record, pallas, key_artifacts: b"accepted")

            def rejecting_verify(archive: bytes, verifier_keys: bytes) -> bool:
                native._reject_probe("recursive compact verify", archive, verifier_keys)
                return False

            setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, rejecting_verify)
        elif method_name == RECURSIVE_COMPACT_VERIFY_METHOD:
            def rejecting_recursive_compact(
                record: bytes,
                pallas: bytes,
                key_artifacts: bytes,
            ) -> bytes:
                native._reject_probe("recursive compact", record, pallas, key_artifacts)
                return b"accepted"

            setattr(native, RECURSIVE_COMPACT_METHOD, rejecting_recursive_compact)
            setattr(native, method_name, lambda archive, verifier_keys: True)
        elif method_name == "kagemusha_recursive_spend_lineage_witness_from_init_result":
            setattr(native, method_name, lambda request, bundle: b"accepted")
        elif method_name == "kagemusha_recursive_spend_lineage_witness_append_result":
            setattr(native, method_name, lambda witness, request, bundle: b"accepted")
        else:
            setattr(native, method_name, lambda archive: b"accepted")
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda native=native: native)

        if method_name == "kagemusha_prove_verified_compact_payment_token_with_records":
            assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
        elif method_name == RECURSIVE_AGGREGATION_METHOD:
            assert (
                kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available()
                is False
            )
        elif method_name in (RECURSIVE_COMPACT_METHOD, RECURSIVE_COMPACT_VERIFY_METHOD):
            assert (
                kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
                is False
            )
            if method_name == RECURSIVE_COMPACT_METHOD:
                assert (
                    kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
                    is True
                )
                with pytest.raises(
                    RuntimeError,
                    match="recursive compact Kagemusha payment-token prover",
                ):
                    getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
                        _kagemusha_input_archive(0xBB),
                        _kagemusha_input_archive(0xBC),
                        RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
                    )
            else:
                assert (
                    kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
                    is False
                )
                with pytest.raises(
                    RuntimeError,
                    match="recursive compact Kagemusha payment-token verifier",
                ):
                    getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
                        _kagemusha_norito_frame_with_payload(0x4B),
                        RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
                    )
        else:
            assert kagemusha.is_kagemusha_recursive_spend_available() is False
            with pytest.raises(RuntimeError, match="reject malformed probe archives"):
                kagemusha.kagemusha_recursive_spend_verify(_kagemusha_input_archive(0x74))

    vague_prover_native = _Native()

    def vague_recursive_compact_prover(record: bytes, pallas: bytes, key_artifacts: bytes) -> bytes:
        raise RuntimeError("Kagemusha recursive compact proof unavailable")

    def rejecting_recursive_compact_verify(archive: bytes, verifier_keys: bytes) -> bool:
        vague_prover_native._reject_probe("recursive compact verify", archive, verifier_keys)
        return True

    setattr(
        vague_prover_native,
        RECURSIVE_COMPACT_METHOD,
        vague_recursive_compact_prover,
    )
    setattr(
        vague_prover_native,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        rejecting_recursive_compact_verify,
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: vague_prover_native)
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
        is False
    )
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
        is True
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token prover"):
        getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            _kagemusha_input_archive(0xBD),
            _kagemusha_input_archive(0xBE),
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        )

    vague_verifier_native = _Native()

    def rejecting_recursive_compact_prover(record: bytes, pallas: bytes, key_artifacts: bytes) -> bytes:
        vague_verifier_native._reject_probe("recursive compact", record, pallas, key_artifacts)
        return _kagemusha_input_archive(0xBF)

    def vague_recursive_compact_verify(archive: bytes, verifier_keys: bytes) -> bool:
        raise RuntimeError("Kagemusha recursive compact verifier unavailable")

    setattr(
        vague_verifier_native,
        RECURSIVE_COMPACT_METHOD,
        rejecting_recursive_compact_prover,
    )
    setattr(
        vague_verifier_native,
        RECURSIVE_COMPACT_VERIFY_METHOD,
        vague_recursive_compact_verify,
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: vague_verifier_native)
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
        is False
    )
    assert (
        kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
        is False
    )
    with pytest.raises(RuntimeError, match="recursive compact Kagemusha payment-token verifier"):
        getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            _kagemusha_norito_frame_with_payload(0x4B),
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        )


def test_recursive_kagemusha_key_artifact_helpers_are_package_root_exports() -> None:
    import iroha_python

    if "is_kagemusha_recursive_spend_available" not in iroha_python.__all__:
        pytest.skip("package root crypto exports are unavailable")

    from iroha_python import (
        KagemushaRecursiveSpendLineageKeyArtifacts
        as RootLineageKeyArtifacts,
        kagemusha_recursive_spend_lineage_key_artifacts_for_append
        as root_lineage_key_artifacts_for_append,
        kagemusha_recursive_spend_lineage_key_artifacts_for_init
        as root_lineage_key_artifacts_for_init,
        kagemusha_recursive_spend_compact_payment_token_from_bundle
        as root_recursive_spend_compact_projection,
        kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes
        as root_recursive_compact_prover,
        kagemusha_verify_recursive_compact_payment_token
        as root_recursive_compact_verify,
        kagemusha_verify_recursive_spend_compact_payment_token_projection
        as root_recursive_spend_compact_projection_verify,
        kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height
        as root_recursive_spend_compact_projection_verify_at_height,
        is_kagemusha_recursive_compact_payment_token_prover_available
        as root_is_recursive_compact_prover_available,
        is_kagemusha_recursive_compact_payment_token_verifier_available
        as root_is_recursive_compact_verifier_available,
        is_kagemusha_recursive_spend_compact_payment_token_projection_available
        as root_is_recursive_spend_compact_projection_available,
        is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available
        as root_is_recursive_spend_compact_projection_verifier_available,
        is_kagemusha_pallas_open_envelope_builder_available
        as root_is_pallas_open_envelope_builder_available,
        kagemusha_build_pallas_open_envelopes_archive
        as root_pallas_open_envelope_builder,
        kagemusha_build_previous_proof_open_envelopes_archive
        as root_previous_proof_open_envelope_builder,
        requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output
        as root_requires_key_artifacts_for_append_output,
        requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init
        as root_requires_key_artifacts_for_init,
    )

    assert (
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init"
        in iroha_python.__all__
    )
    assert "KagemushaRecursiveSpendLineageKeyArtifacts" in iroha_python.__all__
    assert (
        "kagemusha_recursive_spend_lineage_key_artifacts_for_init"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_recursive_spend_lineage_key_artifacts_for_append"
        in iroha_python.__all__
    )
    assert (
        "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output"
        in iroha_python.__all__
    )
    assert (
        "is_kagemusha_recursive_compact_payment_token_verifier_available"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
        in iroha_python.__all__
    )
    assert "kagemusha_verify_recursive_compact_payment_token" in iroha_python.__all__
    assert (
        "is_kagemusha_recursive_spend_compact_payment_token_projection_available"
        in iroha_python.__all__
    )
    assert (
        "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available"
        in iroha_python.__all__
    )
    assert "is_kagemusha_pallas_open_envelope_builder_available" in iroha_python.__all__
    assert "kagemusha_build_pallas_open_envelopes_archive" in iroha_python.__all__
    assert (
        "kagemusha_build_previous_proof_open_envelopes_archive"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_recursive_spend_compact_payment_token_from_bundle"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_verify_recursive_spend_compact_payment_token_projection"
        in iroha_python.__all__
    )
    assert (
        "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
        in iroha_python.__all__
    )
    assert (
        root_requires_key_artifacts_for_init
        is kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init
    )
    assert (
        root_requires_key_artifacts_for_append_output
        is kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output
    )
    assert (
        RootLineageKeyArtifacts
        is kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts
    )
    assert (
        root_lineage_key_artifacts_for_init
        is kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init
    )
    assert (
        root_lineage_key_artifacts_for_append
        is kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_append
    )
    assert (
        root_is_recursive_compact_prover_available
        is kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available
    )
    assert (
        root_is_recursive_compact_verifier_available
        is kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available
    )
    assert (
        root_recursive_compact_prover
        is getattr(kagemusha, RECURSIVE_COMPACT_METHOD)
    )
    assert (
        root_recursive_compact_verify
        is getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)
    )
    assert (
        root_is_recursive_spend_compact_projection_available
        is kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available
    )
    assert (
        root_is_recursive_spend_compact_projection_verifier_available
        is kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available
    )
    assert (
        root_is_pallas_open_envelope_builder_available
        is kagemusha.is_kagemusha_pallas_open_envelope_builder_available
    )
    assert (
        root_pallas_open_envelope_builder
        is getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)
    )
    assert (
        root_previous_proof_open_envelope_builder
        is getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)
    )
    assert (
        root_recursive_spend_compact_projection
        is kagemusha.kagemusha_recursive_spend_compact_payment_token_from_bundle
    )
    assert (
        root_recursive_spend_compact_projection_verify
        is kagemusha.kagemusha_verify_recursive_spend_compact_payment_token_projection
    )
    assert (
        root_recursive_spend_compact_projection_verify_at_height
        is kagemusha.kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height
    )
    prover_signature = inspect.signature(root_recursive_compact_prover)
    assert list(prover_signature.parameters) == [
        "record_bundle_archive",
        "pallas_open_envelopes_archive",
        "recursive_compact_key_artifacts_archive",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in prover_signature.parameters.values()
    )
    pallas_builder_signature = inspect.signature(root_pallas_open_envelope_builder)
    assert list(pallas_builder_signature.parameters) == ["record_bundle_archive"]
    previous_builder_signature = inspect.signature(root_previous_proof_open_envelope_builder)
    assert list(previous_builder_signature.parameters) == ["previous_bundle_archive"]
    verifier_signature = inspect.signature(root_recursive_compact_verify)
    assert list(verifier_signature.parameters) == [
        "compact_token_archive",
        "recursive_compact_verifier_keys_archive",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in verifier_signature.parameters.values()
    )
    projection_at_height_signature = inspect.signature(
        root_recursive_spend_compact_projection_verify_at_height
    )
    assert list(projection_at_height_signature.parameters) == [
        "compact_token_archive",
        "verifier_record_archive",
        "block_height",
    ]
    assert all(
        parameter.default is inspect.Parameter.empty
        for parameter in projection_at_height_signature.parameters.values()
    )


def test_recursive_kagemusha_lineage_key_artifacts_validate_inputs() -> None:
    assert kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND == "halo2/ipa"
    for opening_len in (2, 4, 8, 16, 32, 64, 128):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
                opening_len,
            )
            is True
        )
    for opening_len in (0, 1, 3, 65, 129, -2, 2.5, "2", True):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
                opening_len,  # type: ignore[arg-type]
            )
            is False
        )

    init_verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        0xA1,
    )
    init_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        init_verifier_key,
        0xA2,
    )
    append_verifier_key = _kagemusha_lineage_verifier_key(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        0xA3,
    )
    append_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        append_verifier_key,
        0xA4,
    )

    verifier_key = bytearray(init_verifier_key)
    proving_key = bytearray(init_proving_key_archive)
    init_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
        128,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        verifier_key,
        memoryview(proving_key),
    )
    verifier_key[:] = b"\x00" * len(verifier_key)
    proving_key[:] = b"\x00" * len(proving_key)
    assert init_artifacts.proof_circuit_id == (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    )
    assert init_artifacts.verifier_opening_len == 128
    assert init_artifacts.lineage_verifier_key_backend == "halo2/ipa"
    assert init_artifacts.lineage_verifier_key == init_verifier_key
    assert init_artifacts.lineage_proving_key_archive == init_proving_key_archive
    assert init_artifacts.is_init_artifact is True
    assert init_artifacts.is_append_artifact is False
    with pytest.raises(FrozenInstanceError):
        init_artifacts.lineage_proving_key_archive = b"mutated"  # type: ignore[misc]

    append_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_append(
        64,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        append_verifier_key,
        append_proving_key_archive,
    )
    assert append_artifacts.proof_circuit_id == (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert append_artifacts.is_init_artifact is False
    assert append_artifacts.is_append_artifact is True

    generic_artifacts = kagemusha.kagemusha_recursive_spend_lineage_key_artifacts(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        append_verifier_key,
        append_proving_key_archive,
    )
    assert (
        kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
            generic_artifacts,
        )
        == generic_artifacts
    )

    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            append_verifier_key,
            append_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            append_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            b"not-zk1",
            init_proving_key_archive,
        )
    duplicate_cid_verifier_key = (
        init_verifier_key
        + _kagemusha_zk1_tlv(
            b"CID1",
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
                "utf-8",
            ),
        )
    )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            duplicate_cid_verifier_key,
            init_proving_key_archive,
        )
    whitespace_cid_verifier_key = _kagemusha_lineage_verifier_key(
        f" {kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1} ",
        0xA5,
    )
    whitespace_cid_proving_key_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        whitespace_cid_verifier_key,
        0xA6,
    )
    with pytest.raises(ValueError, match="lineage_verifier_key"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            whitespace_cid_verifier_key,
            whitespace_cid_proving_key_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            b"not-norito",
        )
    missing_circuit_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA5]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            missing_circuit_archive,
        )
    smuggled_circuit_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
                "utf-8",
            )
            + bytes([0xA6]) * 64
        ),
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            smuggled_circuit_archive,
        )
    wrong_commitment_archive = _kagemusha_lineage_proving_key_archive(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        append_verifier_key,
        0xA6,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            wrong_commitment_archive,
        )
    smuggled_commitment_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(append_verifier_key),
        _kagemusha_verifier_key_commitment(init_verifier_key) + bytes([0xA7]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            smuggled_commitment_archive,
        )
    wrong_version_archive = _kagemusha_lineage_proving_key_archive_raw(
        2,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA8]) * 64,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            wrong_version_archive,
        )
    empty_proving_key_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        b"",
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            empty_proving_key_archive,
        )
    trailing_payload_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xA9]) * 64,
        trailing_payload=b"\x7f",
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            trailing_payload_archive,
        )
    old_schema_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAA]) * 64,
        schema_hash=_OLD_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            old_schema_archive,
        )
    packed_struct_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAB]) * 64,
        flags=_TEST_NORITO_COMPACT_LEN_FLAG | _TEST_NORITO_PACKED_STRUCT_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            packed_struct_archive,
        )
    field_bitset_archive = _kagemusha_lineage_proving_key_archive_raw(
        1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        _kagemusha_verifier_key_commitment(init_verifier_key),
        bytes([0xAC]) * 64,
        flags=_TEST_NORITO_COMPACT_LEN_FLAG | _TEST_NORITO_FIELD_BITSET_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            field_bitset_archive,
        )
    overlong_version_length_payload = (
        _kagemusha_overlong_compact_length(2)
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xAD]) * 64))
    )
    overlong_version_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        overlong_version_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            overlong_version_length_archive,
        )
    oversized_terminal_compact_length_payload = (
        _kagemusha_oversized_terminal_compact_length()
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xB0]) * 64))
    )
    oversized_terminal_compact_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        oversized_terminal_compact_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            oversized_terminal_compact_length_archive,
        )
    huge_canonical_compact_length_payload = (
        _kagemusha_huge_canonical_compact_length()
        + (1).to_bytes(2, "little")
        + _kagemusha_norito_field(
            _kagemusha_norito_string(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xB1]) * 64))
    )
    huge_canonical_compact_length_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        huge_canonical_compact_length_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            huge_canonical_compact_length_archive,
        )
    circuit_id_bytes = (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.encode(
            "utf-8",
        )
    )
    overlong_circuit_string_payload = (
        _kagemusha_norito_field((1).to_bytes(2, "little"))
        + _kagemusha_norito_field(
            _kagemusha_overlong_compact_length(len(circuit_id_bytes)) + circuit_id_bytes,
        )
        + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
        + _kagemusha_norito_field(_kagemusha_norito_byte_vec(bytes([0xAE]) * 64))
    )
    overlong_circuit_string_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        overlong_circuit_string_payload,
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            overlong_circuit_string_archive,
        )
    invalid_utf8_circuit_archive = _kagemusha_norito_frame_from_schema_hash(
        _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        (
            _kagemusha_norito_field((1).to_bytes(2, "little"))
            + _kagemusha_norito_field(_kagemusha_norito_length(1) + b"\xff")
            + _kagemusha_norito_field(_kagemusha_verifier_key_commitment(init_verifier_key))
            + _kagemusha_norito_field(
                _kagemusha_norito_byte_vec(circuit_id_bytes + bytes([0xAF]) * 64),
            )
        ),
        _TEST_NORITO_COMPACT_LEN_FLAG,
    )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            invalid_utf8_circuit_archive,
        )
    with pytest.raises(ValueError, match="lineage_proving_key_archive"):
        kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
            128,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
            init_verifier_key,
            _kagemusha_norito_frame(0x9A),
        )

    invalid_dataclasses = [
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "proof_circuit_id",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                3,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "verifier_opening_len",
        ),
        *(
            (
                kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                    kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                    128,
                    backend,
                    b"vk",
                    b"pk",
                ),
                "lineage_verifier_key",
            )
            for backend in ("halo2/kzg", " halo2/ipa", "halo2/ipa ", "HALO2/IPA")
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                "not-bytes",  # type: ignore[arg-type]
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            kagemusha.KagemushaRecursiveSpendLineageKeyArtifacts(
                kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"",
            ),
            "lineage_proving_key_archive",
        ),
    ]
    for artifacts, message in invalid_dataclasses:
        with pytest.raises(ValueError, match=message):
            kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
                artifacts,
            )
    for malformed, message in (
        (None, "lineage_key_artifacts"),
        ("not-artifacts", "lineage_key_artifacts"),
    ):
        with pytest.raises(ValueError, match=message):
            kagemusha.validate_kagemusha_recursive_spend_lineage_key_artifacts(
                malformed,
            )
    for builder_args, message in (
        (
            (
                3,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"pk",
            ),
            "verifier_opening_len",
        ),
        *(
            ((128, backend, b"vk", b"pk"), "lineage_verifier_key")
            for backend in ("halo2/kzg", " halo2/ipa", "halo2/ipa ", "HALO2/IPA")
        ),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                b"vk",
                b"",
            ),
            "lineage_proving_key_archive",
        ),
        (
            (
                128,
                kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                "not-bytes",
                b"pk",
            ),
            "lineage_verifier_key",
        ),
    ):
        with pytest.raises(ValueError, match=message):
            kagemusha.kagemusha_recursive_spend_lineage_key_artifacts_for_init(
                *builder_args,  # type: ignore[arg-type]
            )


def test_recursive_kagemusha_exports_stable_circuit_ids() -> None:
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 6
    assert kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND == "halo2/ipa"
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-aggregation-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-onehop-v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        == "kagemusha-recursive-spend-lineage-append-v1"
    )
    assert kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 == 64
    assert kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
        == 1
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
        == 8 * 1024 * 1024
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
        == 128
    )
    assert kagemusha.KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024
    assert "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES" in kagemusha.__all__
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN
        == "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
    )
    assert (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
        == "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            None,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "",
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
        == "unknown-kagemusha-recursive-spend-circuit"
    )
    whitespace_lineage_output_circuit_id = (
        f" {kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1} "
    )
    assert (
        kagemusha.normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            whitespace_lineage_output_circuit_id,
        )
        == whitespace_lineage_output_circuit_id
    )
    for circuit_id in (
        None,
        "",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert (
            kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
            )
        )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            "unknown-kagemusha-recursive-spend-circuit",
        )
    )
    assert not (
        kagemusha.is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            whitespace_lineage_output_circuit_id,
        )
    )
    for lineage_circuit_id in (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert kagemusha.is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            lineage_circuit_id,
        )
    assert not kagemusha.is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init()
    for output_circuit_id in (
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        assert (
            kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
                output_circuit_id,
            )
        )
    for output_circuit_id in (
        None,
        "",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
        True,
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
                output_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        whitespace_lineage_output_circuit_id,
        None,
        True,
    ):
        assert not (
            kagemusha.is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        "",
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        "unknown-kagemusha-recursive-spend-circuit",
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    )
    assert not kagemusha.is_supported_kagemusha_recursive_spend_append_proof_transition(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
    )
    assert not (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        )
    )
    for previous_circuit_id in (
        "unknown-kagemusha-recursive-spend-circuit",
        None,
        True,
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                previous_circuit_id,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        2,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    assert not kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        64,
    )
    for circuit_id, hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 65),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 2**63),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
        (None, 1),
        ("", 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
    ):
        assert not kagemusha.can_redeem_kagemusha_recursive_spend_witnessless(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
        assert kagemusha.requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
            circuit_id,
            hop_count,  # type: ignore[arg-type]
        )
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(0)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(1)
    assert kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(63)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(64)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(-1)
    assert not kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(2**63)
    for previous_hop_count in (
        1.5,
        float("nan"),
        float("inf"),
        float("-inf"),
        True,
        "1",
    ):
        assert not (
            kagemusha.can_append_kagemusha_recursive_spend_witnessless_lineage(
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            1,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            63,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            64,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    ), "preferred append selector falls back at the witnessless hop cap"
    assert (
        kagemusha.preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
            0,
        )
        == kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        None,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        63,
    )
    for circuit_id, previous_hop_count in (
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
        ),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (whitespace_lineage_output_circuit_id, 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        1,
    )
    assert not (
        kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    ), "semantic previous proofs cannot select Reserved-lineage output"
    for previous_circuit_id, output_circuit_id, previous_hop_count in (
        (
            "unknown-kagemusha-recursive-spend-circuit",
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            "unknown-kagemusha-recursive-spend-circuit",
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            whitespace_lineage_output_circuit_id,
            1,
        ),
        (
            whitespace_lineage_output_circuit_id,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            0,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1.5,
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("nan"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            float("-inf"),
        ),
        (
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            True,
        ),
    ):
        assert not (
            kagemusha.can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                previous_circuit_id,
                output_circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            1,
        )
    )
    assert (
        kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
            kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            64,
        )
    )
    for circuit_id, previous_hop_count in (
        ("", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0),
        (kagemusha.KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1),
        ("unknown-kagemusha-recursive-spend-circuit", 1),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1.5),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("nan")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, float("-inf")),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, True),
        (kagemusha.KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, "1"),
    ):
        assert not (
            kagemusha.requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                circuit_id,
                previous_hop_count,  # type: ignore[arg-type]
            )
        )


def test_recursive_kagemusha_availability_requires_bridge_abi_6(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for abi_version in (
        5,
        True,
        "6",
        -1,
        6.5,
        0x1_0000_0000,
        10**100,
    ):
        native = _Native()
        native.kagemusha_recursive_spend_native_bridge_abi_version = (
            lambda abi_version=abi_version: abi_version
        )
        setattr(native, RECURSIVE_COMPACT_METHOD, lambda record, pallas, key_artifacts: b"compact")
        setattr(native, RECURSIVE_COMPACT_VERIFY_METHOD, lambda token, verifier_keys: True)
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

        assert kagemusha.is_kagemusha_recursive_spend_available() is False
        assert (
            kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available()
            is False
        )
        assert (
            kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available()
            is False
        )
        assert (
            kagemusha.preferred_kagemusha_offline_spend_mode()
            == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
        )
        with pytest.raises(RuntimeError, match="native bridge ABI 6"):
            kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x70))


def test_recursive_kagemusha_availability_rejects_broken_abi_probe(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def broken_abi_probe() -> int:
        raise OSError("bridge denied")

    native.kagemusha_recursive_spend_native_bridge_abi_version = broken_abi_probe
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="native bridge ABI 6"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x71))


def test_recursive_kagemusha_helpers_require_complete_abi_surface(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class PartialNative:
        def kagemusha_recursive_spend_native_bridge_abi_version(self) -> int:
            return 6

        def kagemusha_recursive_spend_init(self, request: bytes) -> bytes:
            return b"init"

        def kagemusha_recursive_spend_append(self, request: bytes) -> bytes:
            return b"append"

        def kagemusha_recursive_spend_transition_profile_init(self, request: bytes) -> bytes:
            return b"transition-profile-init"

        def kagemusha_recursive_spend_transition_profile_append(self, request: bytes) -> bytes:
            return b"transition-profile-append"

        def kagemusha_recursive_spend_lineage_witness_from_init_result(
            self,
            request: bytes,
            bundle: bytes,
        ) -> bytes:
            return b"lineage-init"

        def kagemusha_recursive_spend_verify(self, request: bytes) -> bytes:
            return b"verify"

        def kagemusha_recursive_spend_redeem(self, request: bytes) -> bytes:
            return b"redeem"

    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: PartialNative())

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0x72))


@pytest.mark.parametrize("missing_method", RECURSIVE_SPEND_METHODS)
def test_recursive_kagemusha_helpers_reject_each_missing_abi_method(
    monkeypatch: pytest.MonkeyPatch,
    missing_method: str,
) -> None:
    native = _Native()
    setattr(native, missing_method, None)
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    with pytest.raises(RuntimeError, match="complete native bridge ABI 6 surface"):
        kagemusha.kagemusha_recursive_spend_verify(_kagemusha_input_archive(0x73))


def test_recursive_kagemusha_helpers_reject_empty_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def empty_one(archive: bytes) -> bytes:
        native._reject_probe("empty one", archive)
        return b""

    def empty_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("empty two", first, second)
        return b""

    def empty_three(first: bytes, second: bytes, third: bytes) -> bytes:
        native._reject_probe("empty three", first, second, third)
        return b""

    native.kagemusha_prove_verified_compact_payment_token_with_records = empty_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, empty_two)
    native.kagemusha_recursive_spend_init = empty_one
    native.kagemusha_recursive_spend_append = empty_one
    native.kagemusha_recursive_spend_transition_profile_init = empty_one
    native.kagemusha_recursive_spend_transition_profile_append = empty_one
    native.kagemusha_recursive_spend_lineage_append_boundary = empty_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = empty_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = empty_three
    native.kagemusha_recursive_spend_verify = empty_one
    native.kagemusha_recursive_spend_redeem = empty_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC0)
        )
    with pytest.raises(RuntimeError, match="returned empty output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xC1),
            _kagemusha_input_archive(0xC2),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned empty output"):
            helper(_kagemusha_input_archive(0x80))
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x81),
            _kagemusha_input_archive(0x82),
        )
    with pytest.raises(RuntimeError, match="returned empty output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x83),
            _kagemusha_input_archive(0x84),
            _kagemusha_input_archive(0x85),
        )


def test_recursive_kagemusha_helpers_reject_oversized_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def oversized_one(archive: bytes) -> bytes:
        native._reject_probe("oversized one", archive)
        return b"x" * 49

    def oversized_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("oversized two", first, second)
        return b"x" * 49

    monkeypatch.setattr(kagemusha, "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES", 48)
    native.kagemusha_prove_verified_compact_payment_token_with_records = oversized_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, oversized_two)
    native.kagemusha_recursive_spend_redeem = oversized_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC3)
        )
    with pytest.raises(RuntimeError, match="returned oversized output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xC4),
            _kagemusha_input_archive(0xC5),
        )
    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_recursive_spend_redeem(_kagemusha_input_archive(0x86))


def test_recursive_kagemusha_helpers_reject_oversized_inputs_before_copy_and_native(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    valid_archive = _kagemusha_input_archive(0xB0)
    oversized_archive = memoryview(valid_archive + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(valid_archive),
    )
    monkeypatch.setattr(
        kagemusha,
        "load_crypto_extension",
        lambda: pytest.fail("oversized Kagemusha input reached native loading"),
    )

    def assert_oversized(call, field: str) -> None:
        with pytest.raises(
            ValueError,
            match=rf"{field} must not exceed {len(valid_archive)} bytes",
        ):
            call()

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        assert_oversized(lambda helper=helper: helper(oversized_archive), "request_archive")

    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_append_boundary(
            oversized_archive
        ),
        "profile_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            oversized_archive,
            valid_archive,
        ),
        "request_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            valid_archive,
            oversized_archive,
        ),
        "bundle_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            oversized_archive,
            valid_archive,
            valid_archive,
        ),
        "previous_witness_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            valid_archive,
            oversized_archive,
            valid_archive,
        ),
        "request_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            valid_archive,
            valid_archive,
            oversized_archive,
        ),
        "bundle_archive",
    )
    assert_oversized(
        lambda: kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            oversized_archive
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(
            oversized_archive
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(
            oversized_archive
        ),
        "previous_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            oversized_archive,
            valid_archive,
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            valid_archive,
            oversized_archive,
        ),
        "pallas_open_envelopes_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            oversized_archive,
            valid_archive,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        "record_bundle_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            valid_archive,
            oversized_archive,
            RECURSIVE_COMPACT_KEY_ARTIFACTS_ARCHIVE,
        ),
        "pallas_open_envelopes_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            oversized_archive,
            RECURSIVE_COMPACT_VERIFIER_KEYS_ARCHIVE,
        ),
        "compact_token_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_METHOD)(
            valid_archive,
            valid_archive,
            oversized_archive,
        ),
        "recursive_compact_key_artifacts_archive",
    )
    assert_oversized(
        lambda: getattr(kagemusha, RECURSIVE_COMPACT_VERIFY_METHOD)(
            valid_archive,
            oversized_archive,
        ),
        "recursive_compact_verifier_keys_archive",
    )


def test_recursive_kagemusha_helpers_reject_oversized_memoryview_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    valid_archive = _kagemusha_input_archive(0xB1)
    oversized_archive = memoryview(valid_archive + b"\x00")
    monkeypatch.setattr(
        kagemusha,
        "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
        len(valid_archive),
    )

    def oversized_one(archive: bytes) -> memoryview:
        native._reject_probe("oversized memoryview one", archive)
        return oversized_archive

    native.kagemusha_recursive_spend_redeem = oversized_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned oversized output"):
        kagemusha.kagemusha_recursive_spend_redeem(valid_archive)


def test_recursive_kagemusha_helpers_reject_malformed_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def assert_rejects_malformed_native_outputs(output: bytes) -> None:
        native = _Native()

        def malformed_one(archive: bytes) -> bytes:
            native._reject_probe("malformed one", archive)
            return output

        def malformed_two(first: bytes, second: bytes) -> bytes:
            native._reject_probe("malformed two", first, second)
            return output

        native.kagemusha_prove_verified_compact_payment_token_with_records = malformed_one
        native.kagemusha_build_pallas_open_envelopes_archive = malformed_one
        native.kagemusha_build_previous_proof_open_envelopes_archive = malformed_one
        setattr(native, RECURSIVE_AGGREGATION_METHOD, malformed_two)
        native.kagemusha_recursive_spend_redeem = malformed_one
        monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
                _kagemusha_input_archive(0xC6)
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(
                _kagemusha_input_archive(0xC7)
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(
                _kagemusha_input_archive(0xC8)
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
                _kagemusha_input_archive(0xC9),
                _kagemusha_input_archive(0xCA),
            )
        with pytest.raises(RuntimeError, match="returned invalid Norito archive"):
            kagemusha.kagemusha_recursive_spend_redeem(
                _kagemusha_input_archive(0x87)
            )

    assert_rejects_malformed_native_outputs(b"\x01")

    compressed = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    compressed[22] = 1
    assert_rejects_malformed_native_outputs(bytes(compressed))

    unsupported_flags = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    unsupported_flags[39] = 0x08
    assert_rejects_malformed_native_outputs(bytes(unsupported_flags))

    invalid_field_bitset = bytearray(_kagemusha_norito_frame_with_payload(0x4B))
    invalid_field_bitset[39] = 0x20
    assert_rejects_malformed_native_outputs(bytes(invalid_field_bitset))

    assert_rejects_malformed_native_outputs(
        _kagemusha_norito_frame_with_header_padding(
            _kagemusha_norito_frame_with_payload(0x4B), b"\x7f"
        )
    )
    assert_rejects_malformed_native_outputs(
        _kagemusha_norito_frame_with_header_padding(
            _kagemusha_norito_frame_with_payload(0x4B), b"\x00" * 65
        )
    )


def test_recursive_kagemusha_helpers_reject_empty_payload_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def empty_payload_one(archive: bytes) -> bytes:
        native._reject_probe("empty payload one", archive)
        return _kagemusha_norito_frame(0x4B)

    def empty_payload_two(first: bytes, second: bytes) -> bytes:
        native._reject_probe("empty payload two", first, second)
        return _kagemusha_norito_frame(0x4C)

    native.kagemusha_prove_verified_compact_payment_token_with_records = empty_payload_one
    native.kagemusha_build_pallas_open_envelopes_archive = empty_payload_one
    native.kagemusha_build_previous_proof_open_envelopes_archive = empty_payload_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, empty_payload_two)
    native.kagemusha_recursive_spend_redeem = empty_payload_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xC9)
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        getattr(kagemusha, PALLAS_OPEN_ENVELOPE_BUILDER_METHOD)(
            _kagemusha_input_archive(0xCA)
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        getattr(kagemusha, PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD)(
            _kagemusha_input_archive(0xCB)
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xCC),
            _kagemusha_input_archive(0xCD),
        )
    with pytest.raises(RuntimeError, match="returned empty Norito payload"):
        kagemusha.kagemusha_recursive_spend_redeem(_kagemusha_input_archive(0x88))


def test_recursive_kagemusha_helpers_reject_missing_native_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def missing_one(archive: bytes) -> None:
        native._reject_probe("missing one", archive)
        return None

    def missing_two(first: bytes, second: bytes) -> None:
        native._reject_probe("missing two", first, second)
        return None

    def missing_three(first: bytes, second: bytes, third: bytes) -> None:
        native._reject_probe("missing three", first, second, third)
        return None

    native.kagemusha_prove_verified_compact_payment_token_with_records = missing_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, missing_two)
    native.kagemusha_recursive_spend_init = missing_one
    native.kagemusha_recursive_spend_append = missing_one
    native.kagemusha_recursive_spend_transition_profile_init = missing_one
    native.kagemusha_recursive_spend_transition_profile_append = missing_one
    native.kagemusha_recursive_spend_lineage_append_boundary = missing_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = missing_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = missing_three
    native.kagemusha_recursive_spend_verify = missing_one
    native.kagemusha_recursive_spend_redeem = missing_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xCC)
        )
    with pytest.raises(RuntimeError, match="returned no output"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xCD),
            _kagemusha_input_archive(0xCE),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned no output"):
            helper(_kagemusha_input_archive(0x90))
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0x91),
            _kagemusha_input_archive(0x92),
        )
    with pytest.raises(RuntimeError, match="returned no output"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0x93),
            _kagemusha_input_archive(0x94),
            _kagemusha_input_archive(0x95),
        )


def test_recursive_kagemusha_helpers_reject_native_text_outputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()

    def text_one(archive: bytes) -> str:
        native._reject_probe("text one", archive)
        return "not-norito"

    def text_two(first: bytes, second: bytes) -> str:
        native._reject_probe("text two", first, second)
        return "not-norito"

    def text_three(first: bytes, second: bytes, third: bytes) -> str:
        native._reject_probe("text three", first, second, third)
        return "not-norito"

    native.kagemusha_prove_verified_compact_payment_token_with_records = text_one
    setattr(native, RECURSIVE_AGGREGATION_METHOD, text_two)
    native.kagemusha_recursive_spend_init = text_one
    native.kagemusha_recursive_spend_append = text_one
    native.kagemusha_recursive_spend_transition_profile_init = text_one
    native.kagemusha_recursive_spend_transition_profile_append = text_one
    native.kagemusha_recursive_spend_lineage_append_boundary = text_one
    native.kagemusha_recursive_spend_lineage_witness_from_init_result = text_two
    native.kagemusha_recursive_spend_lineage_witness_append_result = text_three
    native.kagemusha_recursive_spend_verify = text_one
    native.kagemusha_recursive_spend_redeem = text_one
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xCF)
        )
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        getattr(kagemusha, RECURSIVE_AGGREGATION_METHOD)(
            _kagemusha_input_archive(0xD0),
            _kagemusha_input_archive(0xD1),
        )

    for helper in (
        kagemusha.kagemusha_recursive_spend_init,
        kagemusha.kagemusha_recursive_spend_append,
        kagemusha.kagemusha_recursive_spend_transition_profile_init,
        kagemusha.kagemusha_recursive_spend_transition_profile_append,
        kagemusha.kagemusha_recursive_spend_lineage_append_boundary,
        kagemusha.kagemusha_recursive_spend_verify,
        kagemusha.kagemusha_recursive_spend_redeem,
    ):
        with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
            helper(_kagemusha_input_archive(0xA0))
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_from_init_result(
            _kagemusha_input_archive(0xA1),
            _kagemusha_input_archive(0xA2),
        )
    with pytest.raises(RuntimeError, match="returned text instead of Norito bytes"):
        kagemusha.kagemusha_recursive_spend_lineage_witness_append_result(
            _kagemusha_input_archive(0xA3),
            _kagemusha_input_archive(0xA4),
            _kagemusha_input_archive(0xA5),
        )


def test_recursive_kagemusha_redeem_propagates_native_multi_hop_lineage_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []
    request = _kagemusha_input_archive(0xA6)

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: bundle.accumulator.hop_count"
        )

    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"bundle\.accumulator\.hop_count"):
        kagemusha.kagemusha_recursive_spend_redeem(request)
    assert calls == [request]


def test_recursive_kagemusha_helpers_propagate_forged_lineage_record_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[tuple[str, bytes]] = []
    verify_request = _kagemusha_input_archive(0xA7)
    redeem_request = _kagemusha_input_archive(0xA8)

    def rejecting_verify(request: bytes) -> bytes:
        native._reject_probe("verify", request)
        calls.append(("verify", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    def rejecting_redeem(request: bytes) -> bytes:
        native._reject_probe("redeem", request)
        calls.append(("redeem", request))
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: lineage_verifier_record.commitment"
        )

    native.kagemusha_recursive_spend_verify = rejecting_verify
    native.kagemusha_recursive_spend_redeem = rejecting_redeem
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_verify(verify_request)
    with pytest.raises(RuntimeError, match=r"lineage_verifier_record\.commitment"):
        kagemusha.kagemusha_recursive_spend_redeem(redeem_request)
    assert calls == [
        ("verify", verify_request),
        ("redeem", redeem_request),
    ]


def test_recursive_kagemusha_transition_profile_append_propagates_forged_opening_rejection(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    native = _Native()
    calls: list[bytes] = []
    request = _kagemusha_input_archive(0xA9)

    def rejecting_transition_profile_append(request: bytes) -> bytes:
        native._reject_probe("transition profile append", request)
        calls.append(request)
        raise RuntimeError(
            "invalid Kagemusha recursive spend request: hop domain metadata mismatch"
        )

    native.kagemusha_recursive_spend_transition_profile_append = (
        rejecting_transition_profile_append
    )
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: native)

    assert kagemusha.is_kagemusha_recursive_spend_available() is True
    with pytest.raises(RuntimeError, match="hop domain metadata mismatch"):
        kagemusha.kagemusha_recursive_spend_transition_profile_append(request)
    assert calls == [request]


def test_recursive_kagemusha_availability_fails_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(kagemusha, "load_crypto_extension", lambda: object())

    assert kagemusha.is_kagemusha_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_aggregation_proof_bundle_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_prover_available() is False
    assert kagemusha.is_kagemusha_recursive_compact_payment_token_verifier_available() is False
    assert (
        kagemusha.is_kagemusha_recursive_spend_compact_payment_token_projection_available()
        is False
    )
    assert kagemusha.is_kagemusha_recursive_spend_available() is False
    assert (
        kagemusha.preferred_kagemusha_offline_spend_mode()
        == kagemusha.KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    )
    with pytest.raises(RuntimeError, match="Kagemusha support"):
        kagemusha.kagemusha_prove_verified_compact_payment_token_with_records(
            _kagemusha_input_archive(0xD2)
        )
    with pytest.raises(RuntimeError, match="recursive Kagemusha support"):
        kagemusha.kagemusha_recursive_spend_init(_kagemusha_input_archive(0xAA))
