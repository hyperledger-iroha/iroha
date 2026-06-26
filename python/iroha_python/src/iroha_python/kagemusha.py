"""Native recursive Kagemusha offline-cash helpers.

These helpers operate on raw Norito archives so Python applications do not
reimplement recursive proof internals.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Any, Literal, Mapping, Optional, Union

from ._native import load_crypto_extension

BytesLike = Union[bytes, bytearray, memoryview]
KagemushaOfflineSpendMode = Literal[
    "recursive_compact_v1",
    "recursive_spend_v1",
    "checked_prefold_v1",
]
KagemushaInstructionArchiveType = Literal[
    "KagemushaTransfer",
    "RedeemKagemushaRecursive",
]

KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1 = "recursive_compact_v1"
KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1 = "recursive_spend_v1"
KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1"
KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER = "KagemushaTransfer"
KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE = "RedeemKagemushaRecursive"
KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME = (
    "iroha_data_model::isi::offline::KagemushaTransfer"
)
KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME = (
    "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
)
KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
)
KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1"
)
KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1"
)
KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyRequestV1"
)
KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1"
)
KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1"
)
KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1"
)
KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_WIRE_NAME = (
    "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs"
)
KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME = "iroha_data_model::proof::ProofAttachment"
KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME = "iroha_data_model::proof::VerifyingKeyRecord"
KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES = (
    KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER,
    KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE,
)
KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES = {
    KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER: KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME,
    KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE: (
        KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME
    ),
}
KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 6
KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 7
KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION = 0xFFFF_FFFF
KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1 = "kagemusha-recursive-compact-v1"
KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT = (
    "recursive compact Kagemusha payment-token multi-hop proving requires the "
    "append verifier batch"
)
KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT = (
    "recursive compact Kagemusha multi-hop payment-token proving requires the "
    "append verifier batch"
)
KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND = "halo2/ipa"
KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-aggregation-v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-spend-lineage-v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-spend-lineage-onehop-v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-spend-lineage-append-v1"
)
KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS = 64
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 = 64
KAGEMUSHA_FOLD_STEP_MAX_INPUTS = 2
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = True
KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1
KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024
KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128
KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024
KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN = (
    "iroha:kagemusha:v1:recursive-spend-accumulator"
)
_KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN = 2
_KAGEMUSHA_U64_MAX = (1 << 64) - 1
_KAGEMUSHA_U128_MAX = (1 << 128) - 1
_KAGEMUSHA_U128_MAX_DECIMAL_DIGITS = len(str(_KAGEMUSHA_U128_MAX))
KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN = (
    "iroha:kagemusha:v1:recursive-spend-transition-profile"
)
KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN = (
    "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
)
KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN = (
    "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1 = (
    "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1 = (
    "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1 = (
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1 = (
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
)

_COMPACT_TOKEN_METHOD = "kagemusha_prove_verified_compact_payment_token_with_records"
_RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)
_RECURSIVE_COMPACT_TOKEN_METHOD = (
    "kagemusha_prove_verified_recursive_compact_payment_token"
    "_with_records_and_pallas_open_envelopes"
)
_PALLAS_OPEN_ENVELOPE_BUILDER_METHOD = "kagemusha_build_pallas_open_envelopes_archive"
_PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD = (
    "kagemusha_build_previous_proof_open_envelopes_archive"
)
_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD = (
    "kagemusha_verify_recursive_compact_payment_token"
)
_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD = (
    "kagemusha_recursive_spend_compact_payment_token_from_bundle"
)
_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection"
)
_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD = (
    "kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
)

__all__ = [
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1",
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
    "KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_TRANSFER",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPE_REDEEM_RECURSIVE",
    "KAGEMUSHA_TRANSFER_INSTRUCTION_WIRE_NAME",
    "KAGEMUSHA_REDEEM_RECURSIVE_INSTRUCTION_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_WIRE_NAME",
    "KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME",
    "KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES",
    "KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES",
    "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1",
    "KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES",
    "KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES",
    "KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    "KagemushaOfflineSpendMode",
    "KagemushaInstructionArchiveType",
    "KagemushaRecursiveSpendLineageKeyArtifacts",
    "KagemushaRecursiveSpendableNoteDescriptor",
    "KagemushaRecursiveSpendVerifierRecordRef",
    "KagemushaRecursiveSpendInitRequest",
    "KagemushaRecursiveSpendAppendRequest",
    "KagemushaRecursiveSpendVerifyRequest",
    "KagemushaRecursiveSpendVerifyResult",
    "KagemushaRecursiveSpendRedeemRequest",
    "KagemushaRecursiveSpendBundleSummary",
    "can_redeem_kagemusha_recursive_spend_witnessless",
    "is_kagemusha_recursive_spend_lineage_proof_circuit_id",
    "is_kagemusha_recursive_spend_lineage_append_output_circuit_id",
    "is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len",
    "kagemusha_recursive_spend_lineage_key_artifacts_for_init",
    "kagemusha_recursive_spend_lineage_key_artifacts_for_append",
    "kagemusha_recursive_spend_lineage_key_artifacts",
    "validate_kagemusha_recursive_spend_lineage_key_artifacts",
    "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init",
    "requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output",
    "requires_kagemusha_recursive_spend_lineage_witness_for_redeem",
    "can_append_kagemusha_recursive_spend_witnessless_lineage",
    "normalize_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "is_supported_kagemusha_recursive_spend_previous_proof_circuit_id",
    "requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append",
    "is_supported_kagemusha_recursive_spend_append_proof_transition",
    "preferred_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "can_select_kagemusha_recursive_spend_append_output_proof_circuit_id",
    "requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append",
    "is_kagemusha_compact_payment_token_prover_available",
    "is_kagemusha_recursive_aggregation_proof_bundle_prover_available",
    "is_kagemusha_pallas_open_envelope_builder_available",
    "is_kagemusha_recursive_compact_payment_token_prover_available",
    "is_kagemusha_recursive_compact_payment_token_verifier_available",
    "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
    "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
    "is_kagemusha_recursive_compact_unavailable",
    "is_kagemusha_recursive_spend_available",
    "preferred_kagemusha_offline_spend_mode_for_capabilities",
    "preferred_kagemusha_offline_spend_mode",
    "kagemusha_prove_verified_compact_payment_token_with_records",
    _PALLAS_OPEN_ENVELOPE_BUILDER_METHOD,
    _PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD,
    _RECURSIVE_AGGREGATION_METHOD,
    _RECURSIVE_COMPACT_TOKEN_METHOD,
    _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,
    _RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD,
    _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD,
    _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "encode_kagemusha_recursive_spend_init_request",
    "encode_kagemusha_recursive_spend_append_request",
    "encode_kagemusha_recursive_spend_verify_request",
    "encode_kagemusha_recursive_spend_redeem_request",
    "decode_kagemusha_recursive_spend_verify_result",
    "decode_kagemusha_recursive_spend_bundle",
    "kagemusha_recursive_spend_init_typed",
    "kagemusha_recursive_spend_append_typed",
    "kagemusha_recursive_spend_verify_typed",
    "kagemusha_recursive_spend_redeem_typed",
    "kagemusha_instruction_archive_instruction",
    "kagemusha_recursive_redeem_instruction",
    "build_kagemusha_instruction_transaction",
    "build_kagemusha_recursive_redeem_transaction",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
]

_NATIVE_METHODS = (
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
_RECURSIVE_SPEND_ABI_VERSION_METHOD = "kagemusha_recursive_spend_native_bridge_abi_version"
_MALFORMED_NATIVE_PROBE_ARCHIVE = b"\x00"
_KAGEMUSHA_NORITO_HEADER_BYTES = 40
_KAGEMUSHA_NORITO_MAX_HEADER_PADDING_BYTES = 64
_KAGEMUSHA_NORITO_SUPPORTED_FLAGS_MASK = 0x27
_KAGEMUSHA_NORITO_COMPACT_LEN_FLAG = 0x02
_KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG = 0x04
_KAGEMUSHA_NORITO_FIELD_BITSET_FLAG = 0x20
_KAGEMUSHA_NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06
_KAGEMUSHA_NORITO_MAGIC = b"NRT0"
_KAGEMUSHA_ASSET_DEFINITION_ADDRESS_VERSION = 1
_KAGEMUSHA_ASSET_DEFINITION_BASE58_ALPHABET = (
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
)
_KAGEMUSHA_CRC64_MASK = 0xFFFF_FFFF_FFFF_FFFF
_KAGEMUSHA_CRC64_REFLECTED_POLY = 0xC96C_5795_D787_0F42
_KAGEMUSHA_ZK1_MAGIC = b"ZK1\x00"
_KAGEMUSHA_ZK1_TLV_CID1 = b"CID1"
_KAGEMUSHA_ZK1_TLV_IPAK = b"IPAK"
_KAGEMUSHA_ZK1_TLV_H2VK = b"H2VK"
_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 = 1
_KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH = bytes.fromhex(
    "c88489618a012c283ff3bb2ebabc7775"
)
_KAGEMUSHA_PALLAS_OPEN_ENVELOPES_SCHEMA_HASH = bytes.fromhex(
    "fe3826328f081771750f24fe110260ca"
)
_KAGEMUSHA_PALLAS_CURVE_ID = 1
_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_K = 24
_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_N = 1 << _KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_K
_KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128


def _build_kagemusha_crc64_table() -> tuple[int, ...]:
    table: list[int] = []
    for index in range(256):
        crc = index
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ _KAGEMUSHA_CRC64_REFLECTED_POLY
            else:
                crc >>= 1
        table.append(crc)
    return tuple(table)


_KAGEMUSHA_CRC64_TABLE = _build_kagemusha_crc64_table()


def _archive_bytes_named(archive: BytesLike, name: str) -> bytes:
    try:
        view = memoryview(archive)
    except TypeError:
        data = bytes(archive)
        if not data:
            raise ValueError(f"{name} must not be empty")
        if len(data) > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES:
            raise ValueError(
                f"{name} must not exceed {KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes"
            )
        return data
    if view.nbytes == 0:
        raise ValueError(f"{name} must not be empty")
    if view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES:
        raise ValueError(
            f"{name} must not exceed {KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes"
        )
    return view.tobytes()


def _norito_archive_bytes_named(archive: BytesLike, name: str) -> bytes:
    data = _archive_bytes_named(archive, name)
    _assert_kagemusha_norito_archive(data, name)
    return data


def _kagemusha_crc64(payload: bytes) -> int:
    crc = _KAGEMUSHA_CRC64_MASK
    for byte in payload:
        index = (crc ^ byte) & 0xFF
        crc = _KAGEMUSHA_CRC64_TABLE[index] ^ (crc >> 8)
    return (crc ^ _KAGEMUSHA_CRC64_MASK) & _KAGEMUSHA_CRC64_MASK


def _assert_kagemusha_norito_archive(data: bytes, name: str) -> bytes:
    def fail() -> None:
        raise ValueError(f"{name} must be a valid Norito archive")

    if len(data) > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES:
        raise ValueError(
            f"{name} must not exceed {KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES} bytes"
        )
    if len(data) < _KAGEMUSHA_NORITO_HEADER_BYTES:
        fail()
    if data[:4] != _KAGEMUSHA_NORITO_MAGIC:
        fail()
    if data[4] != 0 or data[5] != 0 or data[22] != 0:
        fail()
    flags = data[39]
    if (
        flags & ~_KAGEMUSHA_NORITO_SUPPORTED_FLAGS_MASK
        or (
            flags & _KAGEMUSHA_NORITO_FIELD_BITSET_FLAG
            and (
                flags & _KAGEMUSHA_NORITO_FIELD_BITSET_REQUIRED_FLAGS
                != _KAGEMUSHA_NORITO_FIELD_BITSET_REQUIRED_FLAGS
            )
        )
    ):
        fail()
    payload_length = int.from_bytes(data[23:31], "little")
    if payload_length == 0:
        raise ValueError(f"{name} must contain a non-empty Norito payload")
    minimum_length = _KAGEMUSHA_NORITO_HEADER_BYTES + payload_length
    if len(data) < minimum_length:
        fail()
    padding_length = len(data) - minimum_length
    if padding_length > _KAGEMUSHA_NORITO_MAX_HEADER_PADDING_BYTES:
        fail()
    padding_start = _KAGEMUSHA_NORITO_HEADER_BYTES
    padding_end = padding_start + padding_length
    if any(data[padding_start:padding_end]):
        fail()
    payload = data[padding_end:]
    if _kagemusha_crc64(payload) != int.from_bytes(data[31:39], "little"):
        fail()
    return payload


def _norito_schema_hash(type_name: str) -> bytes:
    digest = hashlib.sha256()
    digest.update(b"norito:v1:type-name\x00")
    digest.update(type_name.encode("utf-8"))
    return digest.digest()[:16]


def _assert_kagemusha_instruction_archive_schema(
    data: bytes,
    instruction_type: KagemushaInstructionArchiveType,
    name: str,
) -> None:
    wire_name = KAGEMUSHA_INSTRUCTION_ARCHIVE_WIRE_NAMES[instruction_type]
    expected_schema = _norito_schema_hash(wire_name)
    if data[6:22] != expected_schema:
        raise ValueError(f"{name} schema must match {instruction_type}")


def _validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
    proof_circuit_id: str,
    lineage_verifier_key_backend: str,
    lineage_verifier_key: bytes,
    lineage_proving_key_archive: bytes,
) -> None:
    verifier_circuit_id = _kagemusha_lineage_verifier_key_envelope_circuit_id(
        lineage_verifier_key
    )
    if verifier_circuit_id != proof_circuit_id:
        raise ValueError("lineage_verifier_key")
    archive_payload = _kagemusha_lineage_proving_key_archive_payload(
        lineage_proving_key_archive
    )
    circuit_id_bytes = proof_circuit_id.encode("utf-8")
    verifier_key_commitment = _kagemusha_verifying_key_commitment(
        lineage_verifier_key_backend,
        lineage_verifier_key,
    )
    if (
        archive_payload.find(circuit_id_bytes) < 0
        or archive_payload.find(verifier_key_commitment) < 0
    ):
        raise ValueError("lineage_proving_key_archive")
    version, circuit_family, archive_commitment, proving_key = (
        _kagemusha_decode_lineage_proving_key_archive_payload(
            archive_payload,
            lineage_proving_key_archive[39],
        )
    )
    if (
        version != _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1
        or circuit_family != proof_circuit_id
        or archive_commitment != verifier_key_commitment
        or not proving_key
    ):
        raise ValueError("lineage_proving_key_archive")


def _kagemusha_lineage_verifier_key_envelope_circuit_id(
    lineage_verifier_key: bytes,
) -> str:
    if not lineage_verifier_key.startswith(_KAGEMUSHA_ZK1_MAGIC):
        raise ValueError("lineage_verifier_key")
    offset = len(_KAGEMUSHA_ZK1_MAGIC)
    circuit_id: str | None = None
    saw_ipa_k = False
    saw_h2_vk = False
    while offset < len(lineage_verifier_key):
        if offset + 8 > len(lineage_verifier_key):
            raise ValueError("lineage_verifier_key")
        tag = lineage_verifier_key[offset : offset + 4]
        payload_length = int.from_bytes(
            lineage_verifier_key[offset + 4 : offset + 8],
            "little",
        )
        payload_start = offset + 8
        payload_end = payload_start + payload_length
        if payload_end > len(lineage_verifier_key):
            raise ValueError("lineage_verifier_key")
        payload = lineage_verifier_key[payload_start:payload_end]
        if tag == _KAGEMUSHA_ZK1_TLV_CID1:
            if (
                circuit_id is not None
                or not payload
                or any(byte < 0x20 or byte > 0x7E for byte in payload)
            ):
                raise ValueError("lineage_verifier_key")
            circuit_id = payload.decode("utf-8")
            if not circuit_id:
                raise ValueError("lineage_verifier_key")
        elif tag == _KAGEMUSHA_ZK1_TLV_IPAK:
            if saw_ipa_k or len(payload) != 4:
                raise ValueError("lineage_verifier_key")
            saw_ipa_k = True
        elif tag == _KAGEMUSHA_ZK1_TLV_H2VK:
            if saw_h2_vk or not payload:
                raise ValueError("lineage_verifier_key")
            saw_h2_vk = True
        else:
            raise ValueError("lineage_verifier_key")
        offset = payload_end
    if circuit_id is None or not saw_ipa_k or not saw_h2_vk:
        raise ValueError("lineage_verifier_key")
    return circuit_id


def _kagemusha_lineage_proving_key_archive_payload(
    lineage_proving_key_archive: bytes,
) -> bytes:
    try:
        archive_payload = _assert_kagemusha_norito_archive(
            lineage_proving_key_archive,
            "lineage_proving_key_archive",
        )
        if (
            lineage_proving_key_archive[6:22]
            != _KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH
            or lineage_proving_key_archive[39] & _KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG
            or lineage_proving_key_archive[39] & _KAGEMUSHA_NORITO_FIELD_BITSET_FLAG
        ):
            raise ValueError("lineage_proving_key_archive")
        return archive_payload
    except ValueError as exc:
        raise ValueError("lineage_proving_key_archive") from exc


def _kagemusha_decode_lineage_proving_key_archive_payload(
    payload: bytes,
    flags: int,
) -> tuple[int, str, bytes, bytes]:
    try:
        offset = 0
        version_payload, offset = _kagemusha_read_norito_field(
            payload,
            offset,
            flags,
        )
        if len(version_payload) != 2:
            raise ValueError("lineage_proving_key_archive")
        version = int.from_bytes(version_payload, "little")

        circuit_family_payload, offset = _kagemusha_read_norito_field(
            payload,
            offset,
            flags,
        )
        circuit_family = _kagemusha_decode_norito_string(
            circuit_family_payload,
            flags,
        )

        verifier_key_commitment, offset = _kagemusha_read_norito_field(
            payload,
            offset,
            flags,
        )
        if len(verifier_key_commitment) != 32:
            raise ValueError("lineage_proving_key_archive")

        proving_key_payload, offset = _kagemusha_read_norito_field(
            payload,
            offset,
            flags,
        )
        proving_key = _kagemusha_decode_norito_byte_vec(proving_key_payload)
        if offset != len(payload):
            raise ValueError("lineage_proving_key_archive")
        return version, circuit_family, verifier_key_commitment, proving_key
    except (UnicodeDecodeError, ValueError) as exc:
        raise ValueError("lineage_proving_key_archive") from exc


def _kagemusha_read_norito_field(
    buffer: bytes,
    offset: int,
    flags: int,
    context: str = "lineage_proving_key_archive",
) -> tuple[bytes, int]:
    length, payload_start = _kagemusha_read_norito_length(
        buffer,
        offset,
        flags,
        context,
    )
    payload_end = payload_start + length
    if payload_end > len(buffer):
        raise ValueError(context)
    return buffer[payload_start:payload_end], payload_end


def _kagemusha_read_norito_length(
    buffer: bytes,
    offset: int,
    flags: int,
    context: str = "lineage_proving_key_archive",
) -> tuple[int, int]:
    if not flags & _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG:
        if offset + 8 > len(buffer):
            raise ValueError(context)
        value = int.from_bytes(buffer[offset : offset + 8], "little")
        if value > _KAGEMUSHA_U64_MAX or value > len(buffer):
            raise ValueError(context)
        return value, offset + 8

    value = 0
    shift = 0
    cursor = offset
    for _ in range(10):
        if cursor >= len(buffer):
            raise ValueError(context)
        byte = buffer[cursor]
        cursor += 1
        chunk = byte & 0x7F
        if shift >= 63 and chunk > 1:
            raise ValueError(context)
        value |= chunk << shift
        if not byte & 0x80:
            encoded_len = cursor - offset
            if encoded_len > 1 and value < (1 << (7 * (encoded_len - 1))):
                raise ValueError(context)
            if value > len(buffer):
                raise ValueError(context)
            return value, cursor
        shift += 7
    raise ValueError(context)


def _kagemusha_decode_norito_string(payload: bytes, flags: int) -> str:
    length, start = _kagemusha_read_norito_length(payload, 0, flags)
    end = start + length
    if end != len(payload):
        raise ValueError("lineage_proving_key_archive")
    return payload[start:end].decode("utf-8")


def _kagemusha_decode_norito_byte_vec(payload: bytes) -> bytes:
    if len(payload) < 8:
        raise ValueError("lineage_proving_key_archive")
    length = int.from_bytes(payload[:8], "little")
    end = 8 + length
    if end != len(payload):
        raise ValueError("lineage_proving_key_archive")
    return payload[8:end]


def _kagemusha_verifying_key_commitment(
    lineage_verifier_key_backend: str,
    lineage_verifier_key: bytes,
) -> bytes:
    backend = lineage_verifier_key_backend.encode("utf-8")
    digest = hashlib.sha256()
    digest.update(b"iroha:zk:v1:vk")
    digest.update(len(backend).to_bytes(8, "big"))
    digest.update(backend)
    digest.update(len(lineage_verifier_key).to_bytes(8, "big"))
    digest.update(lineage_verifier_key)
    return digest.digest()


def _native_method(name: str):
    module = load_crypto_extension()
    method = getattr(module, name, None)
    if method is None:
        raise RuntimeError(
            f"{name} requires a compiled iroha_python._crypto extension "
            "with recursive Kagemusha support"
        )
    return method


def _is_native_method_available(name: str) -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return _probe_native_archive_method(
        module,
        name,
        _MALFORMED_NATIVE_PROBE_ARCHIVE,
    )


def _is_expected_kagemusha_probe_rejection(error: BaseException) -> bool:
    message = str(error)
    return "Kagemusha" in message and any(
        marker in message.lower() for marker in ("archive", "norito", "probe")
    )


def _probe_native_archive_method(module: object, name: str, *archives: bytes) -> bool:
    method = getattr(module, name, None)
    if not callable(method):
        return False
    try:
        method(*archives)
    except Exception as error:
        return _is_expected_kagemusha_probe_rejection(error)
    return False


def _recursive_spend_abi_version(module: object) -> int | None:
    method = getattr(module, _RECURSIVE_SPEND_ABI_VERSION_METHOD, None)
    if not callable(method):
        return None
    try:
        version = method()
    except (TypeError, ValueError, RuntimeError, OSError):
        return None
    except Exception:
        return None
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version < 0
        or version > KAGEMUSHA_MAX_NATIVE_BRIDGE_ABI_VERSION
    ):
        return None
    return version


def _has_recursive_spend_abi(module: object) -> bool:
    version = _recursive_spend_abi_version(module)
    return (
        version is not None
        and version >= KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
    )


def _has_recursive_compact_abi(module: object) -> bool:
    version = _recursive_spend_abi_version(module)
    return (
        version is not None
        and version >= KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
    )


def is_kagemusha_recursive_compact_unavailable(error: object) -> bool:
    message = str(error)
    return (
        KAGEMUSHA_RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT in message
        or KAGEMUSHA_RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT in message
    )


def _missing_recursive_spend_methods(module: object) -> tuple[str, ...]:
    return tuple(
        name
        for name in _NATIVE_METHODS
        if not callable(getattr(module, name, None))
    )


def _require_complete_recursive_spend_surface(module: object) -> None:
    if not _has_recursive_spend_abi(module):
        raise RuntimeError(
            "recursive Kagemusha support requires native bridge ABI "
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION}"
        )
    missing = _missing_recursive_spend_methods(module)
    if missing:
        missing_list = ", ".join(missing)
        raise RuntimeError(
            "recursive Kagemusha support requires the complete native bridge ABI "
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_NATIVE_BRIDGE_ABI_VERSION} surface; "
            f"missing: {missing_list}"
        )
    if not _probe_recursive_spend_surface(module):
        raise RuntimeError(
            "recursive Kagemusha support requires native bridge methods to "
            "reject malformed probe archives"
        )


def is_kagemusha_compact_payment_token_prover_available() -> bool:
    return _is_native_method_available(_COMPACT_TOKEN_METHOD)


def is_kagemusha_recursive_aggregation_proof_bundle_prover_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return _probe_native_archive_method(
        module,
        _RECURSIVE_AGGREGATION_METHOD,
        _MALFORMED_NATIVE_PROBE_ARCHIVE,
        _MALFORMED_NATIVE_PROBE_ARCHIVE,
    )


def is_kagemusha_pallas_open_envelope_builder_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_compact_abi(module)
        and _probe_native_archive_method(
            module,
            _PALLAS_OPEN_ENVELOPE_BUILDER_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
        and _probe_native_archive_method(
            module,
            _PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
    )


def is_kagemusha_recursive_compact_payment_token_prover_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_compact_abi(module)
        and _probe_native_archive_method(
            module,
            _RECURSIVE_COMPACT_TOKEN_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
        and _probe_native_archive_method(
            module,
            _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
    )


def is_kagemusha_recursive_compact_payment_token_verifier_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_compact_abi(module)
        and _probe_native_archive_method(
            module,
            _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
    )


def is_kagemusha_recursive_spend_compact_payment_token_projection_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_compact_abi(module)
        and _probe_native_archive_method(
            module,
            _RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
    )


def is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_compact_abi(module)
        and _probe_native_archive_method(
            module,
            _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
        )
        and _probe_native_archive_method(
            module,
            _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            _MALFORMED_NATIVE_PROBE_ARCHIVE,
            1,
        )
    )


def is_kagemusha_recursive_spend_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return (
        _has_recursive_spend_abi(module)
        and not _missing_recursive_spend_methods(module)
        and _probe_recursive_spend_surface(module)
    )


def _probe_recursive_spend_surface(module: object) -> bool:
    probe = _MALFORMED_NATIVE_PROBE_ARCHIVE
    return all(
        (
            _probe_native_archive_method(module, "kagemusha_recursive_spend_init", probe),
            _probe_native_archive_method(module, "kagemusha_recursive_spend_append", probe),
            _probe_native_archive_method(
                module,
                "kagemusha_recursive_spend_transition_profile_init",
                probe,
            ),
            _probe_native_archive_method(
                module,
                "kagemusha_recursive_spend_transition_profile_append",
                probe,
            ),
            _probe_native_archive_method(
                module,
                "kagemusha_recursive_spend_lineage_append_boundary",
                probe,
            ),
            _probe_native_archive_method(module, "kagemusha_recursive_spend_verify", probe),
            _probe_native_archive_method(
                module,
                "kagemusha_recursive_spend_lineage_witness_from_init_result",
                probe,
                probe,
            ),
            _probe_native_archive_method(
                module,
                "kagemusha_recursive_spend_lineage_witness_append_result",
                probe,
                probe,
                probe,
            ),
            _probe_native_archive_method(module, "kagemusha_recursive_spend_redeem", probe),
        )
    )


def preferred_kagemusha_offline_spend_mode(
    recursive_spend_available: bool | None = None,
    recursive_compact_available: bool | None = None,
) -> KagemushaOfflineSpendMode:
    if recursive_compact_available is None:
        recursive_compact_available = (
            is_kagemusha_recursive_compact_payment_token_prover_available()
            if recursive_spend_available is None
            else False
        )
    if recursive_spend_available is None:
        recursive_spend_available = is_kagemusha_recursive_spend_available()
    return preferred_kagemusha_offline_spend_mode_for_capabilities(
        recursive_compact_available,
        recursive_spend_available,
    )


def preferred_kagemusha_offline_spend_mode_for_capabilities(
    recursive_compact_available: bool,
    recursive_spend_available: bool,
) -> KagemushaOfflineSpendMode:
    if recursive_compact_available:
        return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1
    if recursive_spend_available:
        return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1


def can_redeem_kagemusha_recursive_spend_witnessless(
    proof_circuit_id: str,
    hop_count: int,
) -> bool:
    """Return whether a recursive spend can attempt witnessless online redeem."""

    hop_count_supported = (
        isinstance(hop_count, int)
        and not isinstance(hop_count, bool)
        and 1 <= hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
    )
    return (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        and hop_count_supported
        and proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
    ) or (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        and hop_count_supported
        and proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    ) or (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        and hop_count_supported
        and proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )


def is_kagemusha_recursive_spend_lineage_proof_circuit_id(
    proof_circuit_id: str | None,
) -> bool:
    """Return whether a circuit id is any Reserved-lineage spend profile."""

    return proof_circuit_id in (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )


def is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
    output_proof_circuit_id: str | None,
) -> bool:
    """Return whether a circuit id selects Reserved-lineage append output."""

    return output_proof_circuit_id in (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )


@dataclass(frozen=True)
class KagemushaRecursiveSpendLineageKeyArtifacts:
    """Portable Reserved-lineage verifier/proving key artifact package."""

    proof_circuit_id: str
    verifier_opening_len: int
    lineage_verifier_key_backend: str
    lineage_verifier_key: bytes
    lineage_proving_key_archive: bytes

    @property
    def is_init_artifact(self) -> bool:
        return (
            self.proof_circuit_id
            == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
        )

    @property
    def is_append_artifact(self) -> bool:
        return (
            self.proof_circuit_id
            == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        )


def is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
    verifier_opening_len: int,
) -> bool:
    """Return whether a packaged Reserved-lineage key opening length is supported."""

    return (
        type(verifier_opening_len) is int
        and verifier_opening_len in (2, 4, 8, 16, 32, 64, 128)
    )


def kagemusha_recursive_spend_lineage_key_artifacts_for_init(
    verifier_opening_len: int,
    lineage_verifier_key_backend: str,
    lineage_verifier_key: BytesLike,
    lineage_proving_key_archive: BytesLike,
) -> KagemushaRecursiveSpendLineageKeyArtifacts:
    """Build a validated Reserved-lineage one-hop key artifact package."""

    return kagemusha_recursive_spend_lineage_key_artifacts(
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        verifier_opening_len,
        lineage_verifier_key_backend,
        lineage_verifier_key,
        lineage_proving_key_archive,
    )


def kagemusha_recursive_spend_lineage_key_artifacts_for_append(
    verifier_opening_len: int,
    lineage_verifier_key_backend: str,
    lineage_verifier_key: BytesLike,
    lineage_proving_key_archive: BytesLike,
) -> KagemushaRecursiveSpendLineageKeyArtifacts:
    """Build a validated Reserved-lineage append key artifact package."""

    return kagemusha_recursive_spend_lineage_key_artifacts(
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        verifier_opening_len,
        lineage_verifier_key_backend,
        lineage_verifier_key,
        lineage_proving_key_archive,
    )


def kagemusha_recursive_spend_lineage_key_artifacts(
    proof_circuit_id: str,
    verifier_opening_len: int,
    lineage_verifier_key_backend: str,
    lineage_verifier_key: BytesLike,
    lineage_proving_key_archive: BytesLike,
) -> KagemushaRecursiveSpendLineageKeyArtifacts:
    """Build a validated Reserved-lineage key artifact package."""

    return validate_kagemusha_recursive_spend_lineage_key_artifacts(
        KagemushaRecursiveSpendLineageKeyArtifacts(
            proof_circuit_id=proof_circuit_id,
            verifier_opening_len=verifier_opening_len,
            lineage_verifier_key_backend=lineage_verifier_key_backend,
            lineage_verifier_key=_lineage_key_artifact_bytes(
                lineage_verifier_key,
                "lineage_verifier_key",
            ),
            lineage_proving_key_archive=_lineage_key_artifact_bytes(
                lineage_proving_key_archive,
                "lineage_proving_key_archive",
            ),
        )
    )


def validate_kagemusha_recursive_spend_lineage_key_artifacts(
    artifacts: object,
) -> KagemushaRecursiveSpendLineageKeyArtifacts:
    """Validate and return a defensive Reserved-lineage key artifact package."""

    if not isinstance(artifacts, KagemushaRecursiveSpendLineageKeyArtifacts):
        raise ValueError("lineage_key_artifacts")
    if artifacts.proof_circuit_id not in (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    ):
        raise ValueError("proof_circuit_id")
    if not is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
        artifacts.verifier_opening_len,
    ):
        raise ValueError("verifier_opening_len")
    lineage_verifier_key = _lineage_key_artifact_bytes(
        artifacts.lineage_verifier_key,
        "lineage_verifier_key",
    )
    lineage_proving_key_archive = _lineage_key_artifact_bytes(
        artifacts.lineage_proving_key_archive,
        "lineage_proving_key_archive",
    )
    if (
        artifacts.lineage_verifier_key_backend
        != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND
        or not lineage_verifier_key
    ):
        raise ValueError("lineage_verifier_key")
    if not lineage_proving_key_archive:
        raise ValueError("lineage_proving_key_archive")
    _validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
        artifacts.proof_circuit_id,
        artifacts.lineage_verifier_key_backend,
        lineage_verifier_key,
        lineage_proving_key_archive,
    )
    return KagemushaRecursiveSpendLineageKeyArtifacts(
        proof_circuit_id=artifacts.proof_circuit_id,
        verifier_opening_len=artifacts.verifier_opening_len,
        lineage_verifier_key_backend=artifacts.lineage_verifier_key_backend,
        lineage_verifier_key=lineage_verifier_key,
        lineage_proving_key_archive=lineage_proving_key_archive,
    )


def _lineage_key_artifact_bytes(value: object, name: str) -> bytes:
    if value is None:
        return b""
    if isinstance(value, (bytes, bytearray, memoryview)):
        return bytes(value)
    raise ValueError(name)


def requires_kagemusha_recursive_spend_lineage_key_artifacts_for_init() -> bool:
    """Return whether init proof builders need packaged Reserved-lineage keys."""

    return True


def requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
    proof_circuit_id: str,
    hop_count: int,
) -> bool:
    """Return whether online redeem must carry a record-backed lineage witness."""

    return not can_redeem_kagemusha_recursive_spend_witnessless(
        proof_circuit_id,
        hop_count,
    )


def can_append_kagemusha_recursive_spend_witnessless_lineage(
    previous_hop_count: int,
) -> bool:
    """Return whether this release can append another witnessless lineage hop."""

    return (
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        and isinstance(previous_hop_count, int)
        and not isinstance(previous_hop_count, bool)
        and 1 <= previous_hop_count < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
    )


def normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: str | None,
) -> str:
    """Normalize an append output selector to the Norito request default."""

    if output_proof_circuit_id in (None, ""):
        return KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    if output_proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1:
        return KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    return output_proof_circuit_id


def is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: str | None,
) -> bool:
    """Return whether an append output selector is supported by this release."""

    normalized = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    )
    return normalized in (
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
    )


def requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
    output_proof_circuit_id: str | None,
) -> bool:
    """Return whether append output proving needs packaged Reserved-lineage keys."""

    normalized = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    )
    return is_kagemusha_recursive_spend_lineage_append_output_circuit_id(normalized)


def is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
    previous_proof_circuit_id: str | None,
) -> bool:
    """Return whether a previous recursive proof circuit can be appended."""

    return previous_proof_circuit_id in (
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
    ) or is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)


def requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
    previous_proof_circuit_id: str | None,
) -> bool:
    """Return whether append requests need the previous lineage verifier record."""

    return is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)


def is_supported_kagemusha_recursive_spend_append_proof_transition(
    previous_proof_circuit_id: str | None,
    output_proof_circuit_id: str | None,
) -> bool:
    """Return whether the append proof circuit transition is structurally valid."""

    normalized_output = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    )
    return (
        previous_proof_circuit_id
        == KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        and normalized_output == KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    ) or (
        is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)
        and normalized_output
        in (
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        )
    )


def preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
    previous_hop_count: int,
) -> str:
    """Return the preferred append output proof circuit for this release."""

    if can_append_kagemusha_recursive_spend_witnessless_lineage(previous_hop_count):
        return KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    return KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1


def can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: str | None,
    previous_hop_count: int,
) -> bool:
    """Return whether this release can actually prove the selected append output."""

    if (
        type(previous_hop_count) is not int
        or previous_hop_count < 1
    ):
        return False
    normalized = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    )
    if normalized == KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1:
        return previous_hop_count < KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
    if normalized == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1:
        return can_append_kagemusha_recursive_spend_witnessless_lineage(
            previous_hop_count,
        )
    return False


def can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
    previous_proof_circuit_id: str | None,
    output_proof_circuit_id: str | None,
    previous_hop_count: int,
) -> bool:
    """Return whether an append request may select this output circuit."""

    if not can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
        previous_hop_count,
    ):
        return False
    if not is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        previous_proof_circuit_id,
    ):
        return False
    return is_supported_kagemusha_recursive_spend_append_proof_transition(
        previous_proof_circuit_id,
        output_proof_circuit_id,
    )


def requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
    output_proof_circuit_id: str | None,
    previous_hop_count: int,
) -> bool:
    """Return whether append proving needs previous recursive proof openings."""

    normalized = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    )
    return (
        is_kagemusha_recursive_spend_lineage_append_output_circuit_id(normalized)
        and isinstance(previous_hop_count, int)
        and not isinstance(previous_hop_count, bool)
        and previous_hop_count >= 1
    )


@dataclass(frozen=True)
class KagemushaRecursiveSpendableNoteDescriptor:
    """Current spendable note material carried by recursive spend requests."""

    note_commitment: bytes
    spend_nullifier: bytes
    amount: str

    def __post_init__(self) -> None:
        note_commitment = _kagemusha_fixed32(self.note_commitment, "note_commitment")
        spend_nullifier = _kagemusha_fixed32(self.spend_nullifier, "spend_nullifier")
        if _kagemusha_is_zero32(note_commitment):
            raise ValueError("note_commitment must be non-zero")
        if _kagemusha_is_zero32(spend_nullifier):
            raise ValueError("spend_nullifier must be non-zero")
        if note_commitment == spend_nullifier:
            raise ValueError("spend_nullifier must differ from note_commitment")
        object.__setattr__(self, "note_commitment", note_commitment)
        object.__setattr__(self, "spend_nullifier", spend_nullifier)
        object.__setattr__(
            self,
            "amount",
            _kagemusha_canonical_u128_decimal(self.amount, "amount"),
        )


@dataclass(frozen=True)
class KagemushaRecursiveSpendVerifierRecordRef:
    """Verifier-key registry id paired with its active record Norito archive."""

    verifier_key_id: str
    record_bytes: bytes

    def __post_init__(self) -> None:
        _kagemusha_require_portable_id(self.verifier_key_id, "verifier_key_id")
        record = _kagemusha_typed_archive_payload(
            self.record_bytes,
            KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            "record_bytes",
        )[0]
        _ = record
        object.__setattr__(self, "record_bytes", bytes(self.record_bytes))


@dataclass(frozen=True)
class KagemushaRecursiveSpendInitRequest:
    """Typed `KagemushaRecursiveSpendInitRequestV1` encoder input."""

    record_bundle: bytes
    pallas_open_envelopes: bytes
    current_note: KagemushaRecursiveSpendableNoteDescriptor
    lineage_verifier_key: bytes | None = None
    lineage_proving_key_archive: bytes | None = None
    block_height: int | None = None
    lineage_key_artifacts: KagemushaRecursiveSpendLineageKeyArtifacts | None = None

    def __post_init__(self) -> None:
        _kagemusha_validate_block_height(self.block_height)
        if not isinstance(self.current_note, KagemushaRecursiveSpendableNoteDescriptor):
            raise ValueError("current_note")
        record_bundle = _kagemusha_archive_bytes_named(self.record_bundle, "record_bundle")
        record_bundle_payload = _kagemusha_compact_payload_for_request(
            record_bundle,
            KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
            "record_bundle",
        )
        record_bundle_hop_count = _kagemusha_read_verified_fold_record_bundle_hop_count(
            record_bundle_payload,
            _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG,
            "record_bundle",
        )
        pallas = _kagemusha_require_pallas_open_envelopes_archive(
            self.pallas_open_envelopes,
            "pallas_open_envelopes",
            expected_envelope_count=record_bundle_hop_count,
            max_bytes=KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES,
        )
        lineage_verifier_key, lineage_proving_key_archive = (
            _kagemusha_lineage_key_artifacts_for_init_request(
                self.lineage_key_artifacts,
                self.lineage_verifier_key,
                self.lineage_proving_key_archive,
            )
        )
        object.__setattr__(self, "record_bundle", record_bundle)
        object.__setattr__(self, "pallas_open_envelopes", pallas)
        object.__setattr__(self, "lineage_verifier_key", lineage_verifier_key)
        object.__setattr__(
            self,
            "lineage_proving_key_archive",
            lineage_proving_key_archive,
        )


@dataclass(frozen=True)
class KagemushaRecursiveSpendAppendRequest:
    """Typed `KagemushaRecursiveSpendAppendRequestV1` encoder input."""

    previous_bundle: bytes
    record_bundle: bytes
    pallas_open_envelopes: bytes
    current_note: KagemushaRecursiveSpendableNoteDescriptor
    output_proof_circuit_id: str | None = None
    previous_lineage_verifier_record: KagemushaRecursiveSpendVerifierRecordRef | None = None
    previous_proof_open_envelopes: bytes | None = None
    lineage_verifier_key: bytes | None = None
    lineage_proving_key_archive: bytes | None = None
    block_height: int | None = None
    lineage_key_artifacts: KagemushaRecursiveSpendLineageKeyArtifacts | None = None

    def __post_init__(self) -> None:
        _kagemusha_validate_block_height(self.block_height)
        if not isinstance(self.current_note, KagemushaRecursiveSpendableNoteDescriptor):
            raise ValueError("current_note")
        previous_bundle = _kagemusha_archive_bytes_named(
            self.previous_bundle,
            "previous_bundle",
        )
        previous_summary = decode_kagemusha_recursive_spend_bundle(previous_bundle)
        record_bundle = _kagemusha_archive_bytes_named(self.record_bundle, "record_bundle")
        record_bundle_payload = _kagemusha_compact_payload_for_request(
            record_bundle,
            KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
            "record_bundle",
        )
        record_bundle_hop_count = _kagemusha_read_verified_fold_record_bundle_hop_count(
            record_bundle_payload,
            _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG,
            "record_bundle",
        )
        pallas = _kagemusha_require_pallas_open_envelopes_archive(
            self.pallas_open_envelopes,
            "pallas_open_envelopes",
            expected_envelope_count=record_bundle_hop_count,
            max_bytes=KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES,
        )
        normalized_output = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            self.output_proof_circuit_id,
        )
        if not can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
            previous_summary.proof_circuit_id,
            normalized_output,
            previous_summary.hop_count,
        ):
            raise ValueError("output_proof_circuit_id is not valid for the previous bundle")
        append_needs_previous_lineage_record = (
            requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                previous_summary.proof_circuit_id,
            )
        )
        if append_needs_previous_lineage_record and self.previous_lineage_verifier_record is None:
            raise ValueError(
                "previous_lineage_verifier_record is required for lineage previous bundles"
            )
        if (
            not append_needs_previous_lineage_record
            and self.previous_lineage_verifier_record is not None
        ):
            raise ValueError(
                "previous_lineage_verifier_record is only valid for lineage previous bundles"
            )
        if (
            self.previous_lineage_verifier_record is not None
            and not isinstance(
                self.previous_lineage_verifier_record,
                KagemushaRecursiveSpendVerifierRecordRef,
            )
        ):
            raise ValueError("previous_lineage_verifier_record")
        append_needs_previous_openings = (
            requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                normalized_output,
                previous_summary.hop_count,
            )
        )
        if (
            self.previous_proof_open_envelopes is not None
            and not append_needs_previous_openings
        ):
            raise ValueError(
                "previous_proof_open_envelopes are only valid for lineage append output"
            )
        previous_openings = None
        if self.previous_proof_open_envelopes is not None:
            previous_openings = _kagemusha_require_pallas_open_envelopes_archive(
                self.previous_proof_open_envelopes,
                "previous_proof_open_envelopes",
                expected_envelope_count=KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
                max_bytes=KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
            )
        if append_needs_previous_openings and previous_openings is None:
            raise ValueError(
                "previous_proof_open_envelopes is required for lineage append output"
            )
        lineage_verifier_key = None
        lineage_proving_key_archive = None
        append_needs_lineage_key_artifacts = (
            requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
                normalized_output,
            )
        )
        supplied_lineage_key_material = (
            self.lineage_key_artifacts is not None
            or self.lineage_verifier_key is not None
            or self.lineage_proving_key_archive is not None
        )
        if supplied_lineage_key_material and not append_needs_lineage_key_artifacts:
            raise ValueError("lineage_key_artifacts are only valid for lineage append output")
        if append_needs_lineage_key_artifacts:
            lineage_verifier_key, lineage_proving_key_archive = (
                _kagemusha_lineage_key_artifacts_for_append_request(
                    self.lineage_key_artifacts,
                    self.lineage_verifier_key,
                    self.lineage_proving_key_archive,
                )
            )
        if requires_kagemusha_recursive_spend_lineage_key_artifacts_for_append_output(
            normalized_output,
        ):
            if not lineage_verifier_key:
                raise ValueError("lineage_verifier_key is required for lineage append output")
            if not lineage_proving_key_archive:
                raise ValueError(
                    "lineage_proving_key_archive is required for lineage append output"
                )
        object.__setattr__(self, "previous_bundle", previous_bundle)
        object.__setattr__(self, "record_bundle", record_bundle)
        object.__setattr__(self, "pallas_open_envelopes", pallas)
        object.__setattr__(self, "previous_proof_open_envelopes", previous_openings)
        object.__setattr__(self, "lineage_verifier_key", lineage_verifier_key)
        object.__setattr__(
            self,
            "lineage_proving_key_archive",
            lineage_proving_key_archive,
        )


@dataclass(frozen=True)
class KagemushaRecursiveSpendVerifyRequest:
    """Typed `KagemushaRecursiveSpendVerifyRequestV1` encoder input."""

    bundle: bytes
    lineage_verifier_record: KagemushaRecursiveSpendVerifierRecordRef | None = None
    block_height: int | None = None

    def __post_init__(self) -> None:
        _kagemusha_validate_block_height(self.block_height)
        bundle = _kagemusha_archive_bytes_named(self.bundle, "bundle")
        bundle_summary = decode_kagemusha_recursive_spend_bundle(bundle)
        if is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            bundle_summary.proof_circuit_id
        ):
            if self.lineage_verifier_record is None:
                raise ValueError(
                    "lineage_verifier_record is required for reserved-lineage bundles"
                )
        elif self.lineage_verifier_record is not None:
            raise ValueError(
                "lineage_verifier_record is only valid for reserved-lineage bundles"
            )
        if (
            self.lineage_verifier_record is not None
            and not isinstance(
                self.lineage_verifier_record,
                KagemushaRecursiveSpendVerifierRecordRef,
            )
        ):
            raise ValueError("lineage_verifier_record")
        object.__setattr__(self, "bundle", bundle)


@dataclass(frozen=True)
class KagemushaRecursiveSpendVerifyResult:
    """Decoded `KagemushaRecursiveSpendVerifyResultV1`."""

    valid: bool
    hop_count: int
    encoded_bytes: int
    reason: str
    chain_admissible: bool
    chain_admission_reason: str
    witnessless_redeem_supported: bool = False
    lineage_witness_required: bool = False

    @property
    def lineage_witness_required_for_redeem(self) -> bool:
        return self.lineage_witness_required


@dataclass(frozen=True)
class KagemushaRecursiveSpendRedeemRequest:
    """Typed `KagemushaRecursiveSpendRedeemRequestV1` encoder input."""

    bundle: bytes
    recipient: str
    public_amount: str
    redeem_proof: bytes
    lineage_witness: bytes | None = None
    change_output: bytes | None = None
    lineage_verifier_record: KagemushaRecursiveSpendVerifierRecordRef | None = None
    block_height: int | None = None
    lineage_verifier_records: tuple[KagemushaRecursiveSpendVerifierRecordRef, ...] = ()

    def __post_init__(self) -> None:
        _kagemusha_validate_block_height(self.block_height)
        _kagemusha_require_non_blank_unpadded(self.recipient, "recipient")
        bundle = _kagemusha_archive_bytes_named(self.bundle, "bundle")
        change_output = None
        if self.change_output is not None:
            change_output = _kagemusha_fixed32(self.change_output, "change_output")
            if _kagemusha_is_zero32(change_output):
                raise ValueError("change_output must be non-zero")
        public_amount = _kagemusha_canonical_u128_decimal(self.public_amount, "public_amount")
        bundle_summary = decode_kagemusha_recursive_spend_bundle(bundle)
        _kagemusha_require_redeem_change_binding(
            public_amount,
            bundle_summary.current_note.amount,
            change_output is not None,
        )
        if change_output is not None:
            _kagemusha_require_redeem_change_output_not_reserved(
                change_output,
                bundle_summary,
            )
        final_is_lineage = is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            bundle_summary.proof_circuit_id
        )
        lineage_verifier_records = tuple(self.lineage_verifier_records or ())
        lineage_verifier_record_supplied = (
            self.lineage_verifier_record is not None or bool(lineage_verifier_records)
        )

        def validate_lineage_verifier_records() -> None:
            if (
                self.lineage_verifier_record is not None
                and not isinstance(
                    self.lineage_verifier_record,
                    KagemushaRecursiveSpendVerifierRecordRef,
                )
            ):
                raise ValueError("lineage_verifier_record")
            for lineage_verifier_record in lineage_verifier_records:
                if not isinstance(
                    lineage_verifier_record,
                    KagemushaRecursiveSpendVerifierRecordRef,
                ):
                    raise ValueError("lineage_verifier_records")

        if final_is_lineage:
            if not lineage_verifier_record_supplied:
                raise ValueError(
                    "lineage_verifier_record is required for reserved-lineage bundles"
                )
            validate_lineage_verifier_records()
        lineage_witness = None
        if self.lineage_witness is not None:
            lineage_witness = _kagemusha_require_nested_archive(
                self.lineage_witness,
                "lineage_witness",
            )
        witness_has_reserved_previous = False
        if lineage_witness is not None:
            witness_has_reserved_previous = (
                _kagemusha_lineage_witness_has_reserved_previous_proof(lineage_witness)
            )
        if not final_is_lineage:
            if witness_has_reserved_previous and not lineage_verifier_record_supplied:
                raise ValueError(
                    "lineage_verifier_record is required for lineage witnesses with reserved-lineage previous proofs"
                )
            if (
                not witness_has_reserved_previous
                and lineage_verifier_record_supplied
            ):
                raise ValueError(
                    "lineage_verifier_record is only valid for reserved-lineage bundles or lineage witnesses"
                )
            if lineage_verifier_record_supplied:
                validate_lineage_verifier_records()
        if (
            requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
                bundle_summary.proof_circuit_id,
                bundle_summary.hop_count,
            )
            and lineage_witness is None
        ):
            raise ValueError("lineage_witness is required for this bundle")
        redeem_proof = _kagemusha_archive_bytes_named(self.redeem_proof, "redeem_proof")
        object.__setattr__(self, "bundle", bundle)
        object.__setattr__(self, "public_amount", public_amount)
        object.__setattr__(self, "redeem_proof", redeem_proof)
        object.__setattr__(self, "lineage_witness", lineage_witness)
        object.__setattr__(self, "change_output", change_output)
        object.__setattr__(
            self,
            "lineage_verifier_records",
            lineage_verifier_records,
        )


@dataclass(frozen=True)
class KagemushaRecursiveSpendBundleSummary:
    """Read-only summary decoded from a recursive spend bundle."""

    hop_count: int
    proof_circuit_id: str
    asset: str
    chain_id: str
    initial_root: bytes
    final_root: bytes
    topup_anchor_nullifiers: tuple[bytes, ...]
    current_note: KagemushaRecursiveSpendableNoteDescriptor

    def __post_init__(self) -> None:
        _kagemusha_require_portable_id(self.chain_id, "chain_id")
        object.__setattr__(
            self,
            "initial_root",
            _kagemusha_fixed32(self.initial_root, "initial_root"),
        )
        object.__setattr__(
            self,
            "final_root",
            _kagemusha_fixed32(self.final_root, "final_root"),
        )
        object.__setattr__(
            self,
            "topup_anchor_nullifiers",
            tuple(
                _kagemusha_fixed32(nullifier, "topup_anchor_nullifiers")
                for nullifier in self.topup_anchor_nullifiers
            ),
        )
        _kagemusha_require_recursive_spend_topup_anchor_nullifiers(
            self.topup_anchor_nullifiers,
            self.current_note,
        )


def encode_kagemusha_recursive_spend_init_request(
    request: KagemushaRecursiveSpendInitRequest,
) -> bytes:
    if not isinstance(request, KagemushaRecursiveSpendInitRequest):
        raise ValueError("request")
    payload = b"".join(
        (
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.record_bundle,
                    KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
                    "record_bundle",
                )
            ),
            _kagemusha_field(_kagemusha_bytes_vec(request.pallas_open_envelopes)),
            _kagemusha_field(_kagemusha_spendable_note_payload(request.current_note)),
            _kagemusha_field(
                _kagemusha_option_raw(
                    _kagemusha_verifying_key_box_payload(request.lineage_verifier_key)
                )
            ),
            _kagemusha_field(
                _kagemusha_option_bytes_vec(request.lineage_proving_key_archive)
            ),
            _kagemusha_field(_kagemusha_option_u64(request.block_height)),
        )
    )
    return _kagemusha_norito_archive(
        KAGEMUSHA_RECURSIVE_SPEND_INIT_REQUEST_WIRE_NAME,
        payload,
    )


def encode_kagemusha_recursive_spend_append_request(
    request: KagemushaRecursiveSpendAppendRequest,
) -> bytes:
    if not isinstance(request, KagemushaRecursiveSpendAppendRequest):
        raise ValueError("request")
    normalized_output = normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        request.output_proof_circuit_id,
    )
    output_wire = (
        ""
        if normalized_output == KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        else normalized_output
    )
    previous_record_payload = None
    if request.previous_lineage_verifier_record is not None:
        previous_record_payload = _kagemusha_compact_payload_for_request(
            request.previous_lineage_verifier_record.record_bytes,
            KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            "previous_lineage_verifier_record",
        )
    payload = b"".join(
        (
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.previous_bundle,
                    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
                    "previous_bundle",
                )
            ),
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.record_bundle,
                    KAGEMUSHA_RECURSIVE_SPEND_RECORD_BUNDLE_WIRE_NAME,
                    "record_bundle",
                )
            ),
            _kagemusha_field(_kagemusha_bytes_vec(request.pallas_open_envelopes)),
            _kagemusha_field(_kagemusha_spendable_note_payload(request.current_note)),
            _kagemusha_field(_kagemusha_string(output_wire)),
            _kagemusha_field(_kagemusha_option_raw(previous_record_payload)),
            _kagemusha_field(
                _kagemusha_bytes_vec(request.previous_proof_open_envelopes or b"")
            ),
            _kagemusha_field(
                _kagemusha_option_raw(
                    None
                    if request.lineage_verifier_key is None
                    else _kagemusha_verifying_key_box_payload(request.lineage_verifier_key)
                )
            ),
            _kagemusha_field(
                _kagemusha_option_bytes_vec(request.lineage_proving_key_archive)
            ),
            _kagemusha_field(_kagemusha_option_u64(request.block_height)),
        )
    )
    return _kagemusha_norito_archive(
        KAGEMUSHA_RECURSIVE_SPEND_APPEND_REQUEST_WIRE_NAME,
        payload,
    )


def encode_kagemusha_recursive_spend_verify_request(
    request: KagemushaRecursiveSpendVerifyRequest,
) -> bytes:
    if not isinstance(request, KagemushaRecursiveSpendVerifyRequest):
        raise ValueError("request")
    lineage_record_payload = None
    if request.lineage_verifier_record is not None:
        lineage_record_payload = _kagemusha_compact_payload_for_request(
            request.lineage_verifier_record.record_bytes,
            KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            "lineage_verifier_record",
        )
    payload = b"".join(
        (
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.bundle,
                    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
                    "bundle",
                )
            ),
            _kagemusha_field(_kagemusha_option_raw(lineage_record_payload)),
            _kagemusha_field(_kagemusha_option_u64(request.block_height)),
        )
    )
    return _kagemusha_norito_archive(
        KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_WIRE_NAME,
        payload,
    )


def encode_kagemusha_recursive_spend_redeem_request(
    request: KagemushaRecursiveSpendRedeemRequest,
) -> bytes:
    if not isinstance(request, KagemushaRecursiveSpendRedeemRequest):
        raise ValueError("request")
    lineage_witness_payload = None
    if request.lineage_witness is not None:
        lineage_witness_payload = _kagemusha_compact_payload_for_request(
            request.lineage_witness,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
            "lineage_witness",
        )
    lineage_record_payload = None
    if request.lineage_verifier_record is not None:
        lineage_record_payload = _kagemusha_compact_payload_for_request(
            request.lineage_verifier_record.record_bytes,
            KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            "lineage_verifier_record",
        )
    lineage_record_payloads = tuple(
        _kagemusha_compact_payload_for_request(
            lineage_verifier_record.record_bytes,
            KAGEMUSHA_VERIFYING_KEY_RECORD_WIRE_NAME,
            "lineage_verifier_records",
        )
        for lineage_verifier_record in request.lineage_verifier_records
    )
    payload = b"".join(
        (
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.bundle,
                    KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
                    "bundle",
                )
            ),
            _kagemusha_field(_kagemusha_account_id_payload(request.recipient)),
            _kagemusha_field(_kagemusha_u128(request.public_amount)),
            _kagemusha_raw_field(
                _kagemusha_compact_payload_for_request(
                    request.redeem_proof,
                    KAGEMUSHA_PROOF_ATTACHMENT_WIRE_NAME,
                    "redeem_proof",
                )
            ),
            _kagemusha_field(_kagemusha_option_raw(lineage_witness_payload)),
            _kagemusha_field(_kagemusha_option_fixed32(request.change_output)),
            _kagemusha_field(_kagemusha_option_raw(lineage_record_payload)),
            _kagemusha_field(_kagemusha_option_u64(request.block_height)),
            _kagemusha_field(_kagemusha_raw_vec(lineage_record_payloads)),
        )
    )
    return _kagemusha_norito_archive(
        KAGEMUSHA_RECURSIVE_REDEEM_REQUEST_WIRE_NAME,
        payload,
    )


def decode_kagemusha_recursive_spend_verify_result(
    archive: BytesLike,
) -> KagemushaRecursiveSpendVerifyResult:
    payload, flags = _kagemusha_typed_archive_payload(
        archive,
        KAGEMUSHA_RECURSIVE_SPEND_VERIFY_RESULT_WIRE_NAME,
        "verify_result",
    )
    if flags != _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG:
        raise ValueError("verify_result must use compact Norito layout")
    cursor = 0
    valid, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_bool)
    hop_count, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_u32)
    encoded_bytes, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_u32)
    reason, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", lambda data, f: _kagemusha_read_string_payload(data, f, "verify_result"))
    chain_admissible, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_bool)
    chain_admission_reason, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", lambda data, f: _kagemusha_read_string_payload(data, f, "verify_result"))
    witnessless_redeem_supported = False
    lineage_witness_required = False
    if cursor < len(payload):
        witnessless_redeem_supported, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_bool)
    if cursor < len(payload):
        lineage_witness_required, cursor = _kagemusha_read_field_value(payload, cursor, flags, "verify_result", _kagemusha_read_bool)
    if cursor != len(payload):
        raise ValueError("Trailing bytes after verify_result")
    return KagemushaRecursiveSpendVerifyResult(
        valid=valid,
        hop_count=hop_count,
        encoded_bytes=encoded_bytes,
        reason=reason,
        chain_admissible=chain_admissible,
        chain_admission_reason=chain_admission_reason,
        witnessless_redeem_supported=witnessless_redeem_supported,
        lineage_witness_required=lineage_witness_required,
    )


def _kagemusha_lineage_key_artifacts_for_init_request(
    lineage_key_artifacts: KagemushaRecursiveSpendLineageKeyArtifacts | None,
    lineage_verifier_key: bytes | None,
    lineage_proving_key_archive: bytes | None,
) -> tuple[bytes, bytes]:
    artifacts = _kagemusha_lineage_key_artifacts_for_request(
        lineage_key_artifacts,
        lineage_verifier_key,
        lineage_proving_key_archive,
        build_raw=kagemusha_recursive_spend_lineage_key_artifacts_for_init,
    )
    if not artifacts.is_init_artifact:
        raise ValueError("lineage_key_artifacts must be init artifacts")
    return artifacts.lineage_verifier_key, artifacts.lineage_proving_key_archive


def _kagemusha_lineage_key_artifacts_for_append_request(
    lineage_key_artifacts: KagemushaRecursiveSpendLineageKeyArtifacts | None,
    lineage_verifier_key: bytes | None,
    lineage_proving_key_archive: bytes | None,
) -> tuple[bytes, bytes]:
    artifacts = _kagemusha_lineage_key_artifacts_for_request(
        lineage_key_artifacts,
        lineage_verifier_key,
        lineage_proving_key_archive,
        build_raw=kagemusha_recursive_spend_lineage_key_artifacts_for_append,
    )
    if not artifacts.is_append_artifact:
        raise ValueError("lineage_key_artifacts must be append artifacts")
    return artifacts.lineage_verifier_key, artifacts.lineage_proving_key_archive


def _kagemusha_lineage_key_artifacts_for_request(
    lineage_key_artifacts: KagemushaRecursiveSpendLineageKeyArtifacts | None,
    lineage_verifier_key: bytes | None,
    lineage_proving_key_archive: bytes | None,
    *,
    build_raw: Any,
) -> KagemushaRecursiveSpendLineageKeyArtifacts:
    if lineage_key_artifacts is not None:
        if lineage_verifier_key is not None or lineage_proving_key_archive is not None:
            raise ValueError("lineage_key_artifacts must not be combined with raw key fields")
        return validate_kagemusha_recursive_spend_lineage_key_artifacts(lineage_key_artifacts)
    if lineage_verifier_key is None:
        raise ValueError("lineage_verifier_key is required for recursive spend lineage proving")
    if lineage_proving_key_archive is None:
        raise ValueError(
            "lineage_proving_key_archive is required for recursive spend lineage proving"
        )
    return build_raw(
        _KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
        lineage_verifier_key,
        lineage_proving_key_archive,
    )


def decode_kagemusha_recursive_spend_bundle(
    archive: BytesLike,
) -> KagemushaRecursiveSpendBundleSummary:
    payload, flags = _kagemusha_typed_archive_payload(
        archive,
        KAGEMUSHA_RECURSIVE_SPEND_BUNDLE_WIRE_NAME,
        "bundle",
    )
    if flags != _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG:
        raise ValueError("bundle must use compact Norito layout")
    cursor = 0
    accumulator_payload, cursor = _kagemusha_read_norito_field(payload, cursor, flags, "bundle")
    proof_payload, cursor = _kagemusha_read_norito_field(payload, cursor, flags, "bundle")
    if cursor != len(payload):
        raise ValueError("Trailing bytes after bundle")
    (
        chain_id,
        asset,
        initial_root,
        final_root,
        topup_anchor_nullifiers,
        hop_count,
        current_note,
    ) = (
        _kagemusha_read_accumulator_summary(accumulator_payload, flags)
    )
    proof_circuit_id = _kagemusha_read_recursive_proof_circuit_id(proof_payload, flags)
    if not is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
        proof_circuit_id
    ):
        raise ValueError(
            f"bundle.proof_circuit_id unsupported recursive proof circuit id: {proof_circuit_id}"
        )
    return KagemushaRecursiveSpendBundleSummary(
        hop_count=hop_count,
        proof_circuit_id=proof_circuit_id,
        asset=asset,
        chain_id=chain_id,
        initial_root=initial_root,
        final_root=final_root,
        topup_anchor_nullifiers=topup_anchor_nullifiers,
        current_note=current_note,
    )


def kagemusha_recursive_spend_init_typed(
    request: KagemushaRecursiveSpendInitRequest,
) -> bytes:
    return kagemusha_recursive_spend_init(
        encode_kagemusha_recursive_spend_init_request(request)
    )


def kagemusha_recursive_spend_append_typed(
    request: KagemushaRecursiveSpendAppendRequest,
) -> bytes:
    return kagemusha_recursive_spend_append(
        encode_kagemusha_recursive_spend_append_request(request)
    )


def kagemusha_recursive_spend_verify_typed(
    request: KagemushaRecursiveSpendVerifyRequest,
) -> KagemushaRecursiveSpendVerifyResult:
    return decode_kagemusha_recursive_spend_verify_result(
        kagemusha_recursive_spend_verify(
            encode_kagemusha_recursive_spend_verify_request(request)
        )
    )


def kagemusha_recursive_spend_redeem_typed(
    request: KagemushaRecursiveSpendRedeemRequest,
) -> bytes:
    return kagemusha_recursive_spend_redeem(
        encode_kagemusha_recursive_spend_redeem_request(request)
    )


def _kagemusha_typed_archive_payload(
    archive: BytesLike,
    schema: str,
    field: str,
) -> tuple[bytes, int]:
    data = _archive_bytes_named(archive, field)
    payload = _assert_kagemusha_norito_archive(data, field)
    if data[6:22] != _norito_schema_hash(schema):
        raise ValueError(f"{field} must be a valid {schema} Norito archive")
    if data[22] != 0:
        raise ValueError(f"{field} must not be compressed")
    return payload, data[39]


def _kagemusha_compact_payload_for_request(
    archive: BytesLike,
    schema: str,
    field: str,
) -> bytes:
    payload, flags = _kagemusha_typed_archive_payload(archive, schema, field)
    if flags != _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG:
        raise ValueError(f"{field} must use compact Norito layout")
    return payload


def _kagemusha_norito_archive(schema: str, payload: bytes) -> bytes:
    if not payload:
        raise ValueError("payload must not be empty")
    header = bytearray(_KAGEMUSHA_NORITO_HEADER_BYTES)
    header[0:4] = _KAGEMUSHA_NORITO_MAGIC
    header[6:22] = _norito_schema_hash(schema)
    header[23:31] = len(payload).to_bytes(8, "little")
    header[31:39] = _kagemusha_crc64(payload).to_bytes(8, "little")
    header[39] = _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG
    return bytes(header) + payload


def _kagemusha_compact_length(value: int) -> bytes:
    if value < 0 or value > _KAGEMUSHA_U64_MAX:
        raise ValueError("Norito length must fit in u64")
    out = bytearray()
    remaining = value
    while remaining >= 0x80:
        out.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    out.append(remaining)
    return bytes(out)


def _kagemusha_raw_field(payload: bytes) -> bytes:
    return _kagemusha_compact_length(len(payload)) + bytes(payload)


def _kagemusha_field(payload: bytes) -> bytes:
    return _kagemusha_raw_field(payload)


def _kagemusha_string(value: str) -> bytes:
    if not isinstance(value, str):
        raise TypeError("value must be a string")
    encoded = value.encode("utf-8")
    return _kagemusha_compact_length(len(encoded)) + encoded


def _kagemusha_bytes_vec(value: BytesLike | None) -> bytes:
    data = b"" if value is None else bytes(value)
    if len(data) > _KAGEMUSHA_U64_MAX:
        raise ValueError("byte vector is too large")
    return len(data).to_bytes(8, "little") + data


def _kagemusha_raw_vec(payloads: tuple[bytes, ...]) -> bytes:
    if len(payloads) > _KAGEMUSHA_U64_MAX:
        raise ValueError("vector is too large")
    return len(payloads).to_bytes(8, "little") + b"".join(
        _kagemusha_raw_field(payload) for payload in payloads
    )


def _kagemusha_fixed_bytes_payload(value: bytes) -> bytes:
    return b"".join(_kagemusha_field(bytes((byte,))) for byte in value)


def _kagemusha_const_vec_u8(value: bytes) -> bytes:
    data = bytes(value)
    if len(data) > _KAGEMUSHA_U64_MAX:
        raise ValueError("ConstVec<u8> is too large")
    return len(data).to_bytes(8, "little") + _kagemusha_fixed_bytes_payload(data)


def _kagemusha_option_raw(payload: bytes | None) -> bytes:
    if payload is None:
        return b"\x00"
    return b"\x01" + _kagemusha_raw_field(payload)


def _kagemusha_option_bytes_vec(value: bytes | None) -> bytes:
    if value is None:
        return b"\x00"
    return b"\x01" + _kagemusha_field(_kagemusha_bytes_vec(value))


def _kagemusha_option_u64(value: int | None) -> bytes:
    if value is None:
        return b"\x00"
    checked = _kagemusha_validate_block_height(value)
    assert checked is not None
    return b"\x01" + _kagemusha_field(checked.to_bytes(8, "little"))


def _kagemusha_option_fixed32(value: bytes | None) -> bytes:
    if value is None:
        return b"\x00"
    return b"\x01" + _kagemusha_field(_kagemusha_fixed_bytes_payload(value))


def _kagemusha_spendable_note_payload(
    note: KagemushaRecursiveSpendableNoteDescriptor,
) -> bytes:
    return b"".join(
        (
            _kagemusha_field(_kagemusha_fixed_bytes_payload(note.note_commitment)),
            _kagemusha_field(_kagemusha_fixed_bytes_payload(note.spend_nullifier)),
            _kagemusha_field(_kagemusha_numeric(note.amount)),
        )
    )


def _kagemusha_numeric(value: str) -> bytes:
    integer = int(_kagemusha_canonical_u128_decimal(value, "amount"))
    mantissa = _kagemusha_positive_twos_complement_little_endian(integer)
    return b"".join(
        (
            _kagemusha_field(len(mantissa).to_bytes(4, "little") + mantissa),
            _kagemusha_field((0).to_bytes(4, "little")),
        )
    )


def _kagemusha_positive_twos_complement_little_endian(value: int) -> bytes:
    if value == 0:
        return b""
    big = value.to_bytes((value.bit_length() + 7) // 8, "big")
    if big[0] & 0x80:
        big = b"\x00" + big
    return big[::-1]


def _kagemusha_u128(value: str) -> bytes:
    integer = int(_kagemusha_canonical_u128_decimal(value, "public_amount"))
    return integer.to_bytes(16, "little")


def _kagemusha_verifying_key_box_payload(lineage_verifier_key: bytes | None) -> bytes:
    if lineage_verifier_key is None:
        raise ValueError("lineage_verifier_key must not be empty")
    key = bytes(lineage_verifier_key)
    if not key:
        raise ValueError("lineage_verifier_key must not be empty")
    return b"".join(
        (
            _kagemusha_field(_kagemusha_string(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND)),
            _kagemusha_field(_kagemusha_bytes_vec(key)),
        )
    )


def _kagemusha_account_id_payload(recipient: str) -> bytes:
    try:
        from .address import AccountAddress, AccountAddressError

        address = AccountAddress.parse_encoded(recipient)
    except Exception as exc:
        raise ValueError("recipient must use canonical I105 account form") from exc
    controller = address.controller
    if controller.tag != 0:
        raise ValueError("recipient has no supported controller")
    return (0).to_bytes(4, "little") + _kagemusha_field(
        _kagemusha_public_key_payload(int(controller.curve), controller.public_key)
    )


def _kagemusha_public_key_payload(curve_id: int, public_key: bytes) -> bytes:
    curve_tags = {
        0x01: 0,
        0x04: 1,
        0x03: 2,
        0x05: 3,
        0x02: 4,
        0x0A: 5,
        0x0B: 6,
        0x0C: 7,
        0x0D: 8,
        0x0E: 9,
        0x0F: 10,
    }
    try:
        tag = curve_tags[curve_id]
    except KeyError as exc:
        raise ValueError(f"unsupported recipient curve id: {curve_id}") from exc
    return _kagemusha_const_vec_u8(bytes((tag,)) + bytes(public_key))


def _kagemusha_read_field_value(
    buffer: bytes,
    offset: int,
    flags: int,
    context: str,
    decode,
):
    payload, next_offset = _kagemusha_read_norito_field(buffer, offset, flags, context)
    return decode(payload, flags), next_offset


def _kagemusha_read_bool(payload: bytes, _flags: int) -> bool:
    if payload == b"\x00":
        return False
    if payload == b"\x01":
        return True
    raise ValueError("boolean field must be 0 or 1")


def _kagemusha_read_u16(payload: bytes, _flags: int, _context: str | None = None) -> int:
    if len(payload) != 2:
        raise ValueError("u16 field must be 2 bytes")
    return int.from_bytes(payload, "little")


def _kagemusha_read_u32(payload: bytes, _flags: int, _context: str | None = None) -> int:
    if len(payload) != 4:
        raise ValueError("u32 field must be 4 bytes")
    return int.from_bytes(payload, "little")


def _kagemusha_read_fixed_bytes_payload(
    payload: bytes,
    flags: int,
    length: int,
    context: str,
) -> bytes:
    if len(payload) == length:
        return payload
    out = bytearray()
    cursor = 0
    try:
        while cursor < len(payload):
            field, cursor = _kagemusha_read_norito_field(payload, cursor, flags, context)
            if len(field) != 1:
                raise ValueError(context)
            out.extend(field)
    except ValueError as error:
        raise ValueError(f"{context} must be exactly {length} bytes") from error
    if len(out) != length:
        raise ValueError(f"{context} must be exactly {length} bytes")
    return bytes(out)


def _kagemusha_read_string_payload(payload: bytes, flags: int, context: str) -> str:
    length, start = _kagemusha_read_norito_length(payload, 0, flags, context)
    end = start + length
    if end != len(payload):
        raise ValueError(context)
    return payload[start:end].decode("utf-8")


def _kagemusha_read_chain_id_payload(payload: bytes, flags: int) -> str:
    chain_id_payload, cursor = _kagemusha_read_norito_field(
        payload,
        0,
        flags,
        "bundle.accumulator.chain_id",
    )
    if cursor != len(payload):
        raise ValueError("bundle.accumulator.chain_id")
    chain_id = _kagemusha_read_string_payload(
        chain_id_payload,
        flags,
        "bundle.accumulator.chain_id",
    )
    _kagemusha_require_portable_id(
        chain_id,
        "bundle.accumulator.chain_id",
    )
    return chain_id


def _kagemusha_read_accumulator_summary(
    payload: bytes,
    flags: int,
) -> tuple[
    str,
    str,
    bytes,
    bytes,
    tuple[bytes, ...],
    int,
    KagemushaRecursiveSpendableNoteDescriptor,
]:
    cursor = 0
    domain_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.domain",
    )
    domain = _kagemusha_read_string_payload(
        domain_payload,
        flags,
        "bundle.accumulator.domain",
    )
    if domain != KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN:
        raise ValueError(
            "bundle.accumulator.domain must be "
            f"{KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN}"
        )
    chain_id_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.chain_id",
    )
    chain_id = _kagemusha_read_chain_id_payload(chain_id_payload, flags)
    asset_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.asset",
    )
    asset_bytes = _kagemusha_read_fixed_bytes_payload(
        asset_payload,
        flags,
        16,
        "bundle.accumulator.asset",
    )
    asset = _kagemusha_asset_definition_from_bytes(asset_bytes)
    initial_root, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.initial_root",
    )
    initial_root = _kagemusha_read_fixed_bytes_payload(
        initial_root,
        flags,
        32,
        "bundle.accumulator.initial_root",
    )
    final_root, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.final_root",
    )
    final_root = _kagemusha_read_fixed_bytes_payload(
        final_root,
        flags,
        32,
        "bundle.accumulator.final_root",
    )
    _kagemusha_require_recursive_spend_accumulator_roots(initial_root, final_root)
    topup_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.topup_anchor_nullifiers",
    )
    topup_anchor_nullifiers = _kagemusha_read_topup_anchor_nullifiers(
        topup_payload,
        flags,
    )
    hop_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.hop_count",
    )
    try:
        hop_count = _kagemusha_read_u32(hop_payload, flags)
    except ValueError as error:
        raise ValueError("bundle.accumulator.hop_count") from error
    if not (1 <= hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1):
        raise ValueError("bundle.accumulator.hop_count")
    cursor = _kagemusha_require_recursive_spend_accumulator_corridor(
        payload,
        cursor,
        flags,
        hop_count,
    )
    note_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.current_note",
    )
    current_note = _kagemusha_read_spendable_note(note_payload, flags)
    _kagemusha_require_recursive_spend_topup_anchor_nullifiers(
        topup_anchor_nullifiers,
        current_note,
    )
    if cursor != len(payload):
        raise ValueError("Trailing bytes after accumulator")
    return (
        chain_id,
        asset,
        initial_root,
        final_root,
        topup_anchor_nullifiers,
        hop_count,
        current_note,
    )


def _kagemusha_read_recursive_proof_circuit_id(payload: bytes, flags: int) -> str:
    return _kagemusha_read_recursive_proof_circuit_id_with_context(
        payload,
        flags,
        trailing_context="recursive_proof",
        verifier_trailing_context="verifier_key_id",
        verifier_backend_context="verifier_key_id.backend",
        verifier_name_context="verifier_key_id",
        proof_public_inputs_context="bundle.proof_public_inputs",
        proof_public_inputs_hash_context="bundle.proof_public_inputs_hash",
        proof_backend_context="bundle.proof_backend",
        proof_bytes_context="bundle.proof_bytes",
    )


def _kagemusha_read_recursive_proof_circuit_id_with_context(
    payload: bytes,
    flags: int,
    *,
    trailing_context: str,
    verifier_trailing_context: str,
    verifier_backend_context: str,
    verifier_name_context: str,
    proof_public_inputs_context: str,
    proof_public_inputs_hash_context: str,
    proof_backend_context: str,
    proof_bytes_context: str,
) -> str:
    payload_cursor = 0
    verifier_payload, payload_cursor = _kagemusha_read_norito_field(
        payload,
        payload_cursor,
        flags,
        "recursive_proof.verifier_key_id",
    )
    public_inputs_payload, payload_cursor = _kagemusha_read_norito_field(
        payload,
        payload_cursor,
        flags,
        "recursive_proof.public_inputs",
    )
    if not public_inputs_payload:
        raise ValueError(proof_public_inputs_context)
    public_inputs_hash_payload, payload_cursor = _kagemusha_read_norito_field(
        payload,
        payload_cursor,
        flags,
        "recursive_proof.public_inputs_hash",
    )
    public_inputs_hash = _kagemusha_read_fixed_bytes_payload(
        public_inputs_hash_payload,
        flags,
        32,
        proof_public_inputs_hash_context,
    )
    if _kagemusha_is_zero32(public_inputs_hash):
        raise ValueError(proof_public_inputs_hash_context)
    public_inputs_archive = _kagemusha_norito_archive(
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_WIRE_NAME,
        public_inputs_payload,
    )
    if public_inputs_hash != _kagemusha_iroha_hash(public_inputs_archive):
        raise ValueError(proof_public_inputs_hash_context)
    proof_payload, payload_cursor = _kagemusha_read_norito_field(
        payload,
        payload_cursor,
        flags,
        "recursive_proof.proof",
    )
    if payload_cursor != len(payload):
        raise ValueError(f"Trailing bytes after {trailing_context}")
    proof_backend = _kagemusha_read_proof_box_backend(
        proof_payload,
        flags,
        proof_backend_context=proof_backend_context,
        proof_bytes_context=proof_bytes_context,
    )
    cursor = 0
    backend_payload, cursor = _kagemusha_read_norito_field(
        verifier_payload,
        cursor,
        flags,
        "verifier_key_id",
    )
    name_payload, cursor = _kagemusha_read_norito_field(
        verifier_payload,
        cursor,
        flags,
        "verifier_key_id",
    )
    if cursor != len(verifier_payload):
        raise ValueError(f"Trailing bytes after {verifier_trailing_context}")
    backend = _kagemusha_read_string_payload(
        backend_payload,
        flags,
        verifier_backend_context,
    )
    _kagemusha_require_portable_id(backend, verifier_backend_context)
    if backend != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND:
        if proof_backend_context == "bundle.proof_backend":
            raise ValueError(
                f"bundle.proof_backend unsupported recursive proof backend: {backend}"
            )
        raise ValueError(
            f"{proof_backend_context} unsupported recursive proof backend: {backend}"
        )
    if proof_backend != backend:
        raise ValueError(
            f"{proof_backend_context} recursive proof backend mismatch: {proof_backend}"
        )
    name = _kagemusha_read_string_payload(name_payload, flags, verifier_name_context)
    _kagemusha_require_portable_id(name, verifier_name_context)
    return name


def _kagemusha_lineage_witness_has_reserved_previous_proof(archive: bytes) -> bool:
    payload = _kagemusha_compact_payload_for_request(
        archive,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESS_WIRE_NAME,
        "lineage_witness",
    )
    flags = _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG
    cursor = _kagemusha_skip_norito_fields(payload, 0, flags, 3, "lineage_witness")
    previous_proofs_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "lineage_witness.previous_recursive_proofs",
    )
    if cursor != len(payload):
        raise ValueError("lineage_witness")
    if len(previous_proofs_payload) < 8:
        raise ValueError("lineage_witness.previous_recursive_proofs")
    count = int.from_bytes(previous_proofs_payload[:8], "little")
    if count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS:
        raise ValueError(
            "lineage_witness.previous_recursive_proofs count exceeds "
            f"{KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS}"
        )
    proof_cursor = 8
    has_reserved = False
    for index in range(count):
        proof_payload, proof_cursor = _kagemusha_read_norito_field(
            previous_proofs_payload,
            proof_cursor,
            flags,
            f"lineage_witness.previous_recursive_proofs[{index}]",
        )
        circuit_id = _kagemusha_read_previous_recursive_proof_circuit_id(
            proof_payload,
            flags,
        )
        has_reserved = has_reserved or is_kagemusha_recursive_spend_lineage_proof_circuit_id(
            circuit_id
        )
    if proof_cursor != len(previous_proofs_payload):
        raise ValueError("lineage_witness.previous_recursive_proofs")
    return has_reserved


def _kagemusha_read_previous_recursive_proof_circuit_id(payload: bytes, flags: int) -> str:
    name = _kagemusha_read_recursive_proof_circuit_id_with_context(
        payload,
        flags,
        trailing_context="lineage_witness.previous_recursive_proofs",
        verifier_trailing_context="lineage_witness.previous_recursive_proofs.verifier_key_id",
        verifier_backend_context=(
            "lineage_witness.previous_recursive_proofs.verifier_key_id.backend"
        ),
        verifier_name_context="lineage_witness.previous_recursive_proofs.verifier_key_id.name",
        proof_public_inputs_context=(
            "lineage_witness.previous_recursive_proofs.proof_public_inputs"
        ),
        proof_public_inputs_hash_context=(
            "lineage_witness.previous_recursive_proofs.proof_public_inputs_hash"
        ),
        proof_backend_context="lineage_witness.previous_recursive_proofs.proof_backend",
        proof_bytes_context="lineage_witness.previous_recursive_proofs.proof_bytes",
    )
    if not is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(name):
        raise ValueError("lineage_witness.previous_recursive_proofs.verifier_key_id.name")
    return name


def _kagemusha_read_proof_box_backend(
    payload: bytes,
    flags: int,
    *,
    proof_backend_context: str,
    proof_bytes_context: str,
) -> str:
    cursor = 0
    backend_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "proof.backend",
    )
    proof_bytes_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "proof.bytes",
    )
    if cursor != len(payload):
        raise ValueError("Trailing bytes after proof")
    backend = _kagemusha_read_string_payload(backend_payload, flags, "proof.backend")
    _kagemusha_require_portable_id(backend, "proof.backend")
    if backend != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND:
        raise ValueError(
            f"{proof_backend_context} unsupported recursive proof backend: {backend}"
        )
    proof_bytes = _kagemusha_read_bytes_vec_payload(proof_bytes_payload, "proof.bytes")
    if not proof_bytes:
        raise ValueError(proof_bytes_context)
    return backend


def _kagemusha_read_bytes_vec_payload(payload: bytes, context: str) -> bytes:
    if len(payload) < 8:
        raise ValueError(context)
    length = int.from_bytes(payload[:8], "little")
    if length != len(payload) - 8:
        raise ValueError(context)
    return payload[8:]


def _kagemusha_read_spendable_note(
    payload: bytes,
    flags: int,
) -> KagemushaRecursiveSpendableNoteDescriptor:
    cursor = 0
    note_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.current_note.note_commitment",
    )
    nullifier_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "bundle.accumulator.current_note.spend_nullifier",
    )
    amount_payload, cursor = _kagemusha_read_norito_field(
        payload, cursor, flags, "bundle.accumulator.current_note.amount"
    )
    if cursor != len(payload):
        raise ValueError("Trailing bytes after bundle.accumulator.current_note")
    return KagemushaRecursiveSpendableNoteDescriptor(
        note_commitment=_kagemusha_read_fixed_bytes(
            note_payload,
            flags,
            32,
            "bundle.accumulator.current_note.note_commitment",
        ),
        spend_nullifier=_kagemusha_read_fixed_bytes(
            nullifier_payload,
            flags,
            32,
            "bundle.accumulator.current_note.spend_nullifier",
        ),
        amount=_kagemusha_read_numeric(
            amount_payload,
            flags,
            "bundle.accumulator.current_note.amount",
        ),
    )


def _kagemusha_read_fixed_bytes(
    payload: bytes,
    flags: int,
    expected_size: int,
    field: str,
) -> bytes:
    if len(payload) == expected_size:
        return payload
    out = bytearray()
    cursor = 0
    while cursor < len(payload):
        item, cursor = _kagemusha_read_norito_field(payload, cursor, flags, field)
        if len(item) != 1:
            raise ValueError(f"{field} byte field length must be 1")
        out.extend(item)
    if len(out) != expected_size:
        raise ValueError(f"{field} must be exactly {expected_size} bytes")
    return bytes(out)


def _kagemusha_read_numeric(payload: bytes, flags: int, context: str = "amount") -> str:
    cursor = 0
    mantissa_payload, cursor = _kagemusha_read_norito_field(
        payload, cursor, flags, f"{context}.mantissa"
    )
    scale_payload, cursor = _kagemusha_read_norito_field(
        payload, cursor, flags, f"{context}.scale"
    )
    if cursor != len(payload):
        raise ValueError(f"Trailing bytes after {context}")
    if len(mantissa_payload) < 4:
        raise ValueError(f"{context} numeric mantissa length")
    mantissa_length = int.from_bytes(mantissa_payload[:4], "little")
    if 4 + mantissa_length != len(mantissa_payload):
        raise ValueError(f"{context} numeric mantissa length")
    if len(scale_payload) != 4 or int.from_bytes(scale_payload, "little") != 0:
        raise ValueError(f"{context} numeric scale must be zero")
    integer = _kagemusha_bigint_from_little_twos_complement(mantissa_payload[4:])
    if integer <= 0:
        raise ValueError(f"{context} numeric amount must be greater than zero")
    if integer > _KAGEMUSHA_U128_MAX:
        raise ValueError(f"{context} numeric amount must fit in u128")
    return str(integer)


def _kagemusha_bigint_from_little_twos_complement(payload: bytes) -> int:
    if not payload:
        return 0
    return int.from_bytes(payload, "little", signed=bool(payload[-1] & 0x80))


def _kagemusha_skip_norito_fields(
    payload: bytes,
    cursor: int,
    flags: int,
    count: int,
    context: str,
) -> int:
    for _ in range(count):
        _, cursor = _kagemusha_read_norito_field(payload, cursor, flags, context)
    return cursor


def _kagemusha_validate_block_height(block_height: int | None) -> int | None:
    if block_height is None:
        return None
    if isinstance(block_height, bool) or not isinstance(block_height, int):
        raise TypeError("block_height must be an integer")
    if block_height < 0:
        raise ValueError("block_height must be non-negative")
    if block_height > _KAGEMUSHA_U64_MAX:
        raise ValueError("block_height must fit in u64")
    return block_height


def _kagemusha_read_verified_fold_record_bundle_hop_count(
    payload: bytes,
    flags: int,
    field: str,
) -> int:
    cursor = 0
    bundle_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        f"{field}.bundle",
    )
    _, cursor = _kagemusha_read_norito_field(payload, cursor, flags, f"{field}.records")
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")

    bundle_cursor = _kagemusha_skip_norito_fields(
        bundle_payload,
        0,
        flags,
        2,
        f"{field}.bundle",
    )
    steps_payload, bundle_cursor = _kagemusha_read_norito_field(
        bundle_payload,
        bundle_cursor,
        flags,
        f"{field}.steps",
    )
    if bundle_cursor != len(bundle_payload):
        raise ValueError(f"{field}.bundle has trailing bytes")
    hop_count = _kagemusha_read_verified_fold_step_count(
        steps_payload,
        flags,
        f"{field}.steps",
    )
    if hop_count < 1 or hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS:
        raise ValueError(f"{field} fold step count is out of range")
    return hop_count


def _kagemusha_read_verified_fold_step_count(
    payload: bytes,
    flags: int,
    field: str,
) -> int:
    if len(payload) < 8:
        raise ValueError(f"{field} count is truncated")
    count = int.from_bytes(payload[:8], "little")
    if count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS:
        raise ValueError(f"{field} fold step count is out of range")
    cursor = 8
    for index in range(count):
        item, cursor = _kagemusha_read_norito_field(
            payload,
            cursor,
            flags,
            f"{field}[{index}]",
        )
        item_cursor = _kagemusha_skip_norito_fields(
            item,
            0,
            flags,
            6,
            f"{field}[{index}]",
        )
        if item_cursor != len(item):
            raise ValueError(f"{field}[{index}] has trailing bytes")
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return count


def _kagemusha_require_pallas_open_envelopes_archive(
    value: BytesLike,
    field: str,
    *,
    expected_envelope_count: int,
    max_bytes: int,
) -> bytes:
    data = _archive_bytes_named(value, field)
    if len(data) > max_bytes:
        raise ValueError(f"{field} must not exceed {max_bytes} bytes")
    payload = _assert_kagemusha_norito_archive(data, field)
    if (
        data[6:22] != _KAGEMUSHA_PALLAS_OPEN_ENVELOPES_SCHEMA_HASH
        or data[22] != 0
        or data[39] != _KAGEMUSHA_NORITO_COMPACT_LEN_FLAG
    ):
        raise ValueError(
            f"{field} must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive"
        )
    if len(payload) < 8:
        raise ValueError(f"{field} envelope count is truncated")
    count = int.from_bytes(payload[:8], "little")
    if count != expected_envelope_count:
        raise ValueError(f"{field} requires exactly {expected_envelope_count} envelope(s)")
    cursor = 8
    for index in range(count):
        item, cursor = _kagemusha_read_norito_field(
            payload,
            cursor,
            data[39],
            f"{field}[{index}]",
        )
        _kagemusha_validate_pallas_open_envelope_payload(
            item,
            data[39],
            f"{field}[{index}]",
        )
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return data


def _kagemusha_validate_pallas_open_envelope_payload(
    payload: bytes,
    flags: int,
    field: str,
) -> None:
    cursor = 0
    params_n, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.params",
        _kagemusha_read_pallas_ipa_params,
    )
    public_n, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.public",
        _kagemusha_read_pallas_poly_open_public,
    )
    if public_n != params_n:
        raise ValueError(f"{field} public opening length mismatch")
    _, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.proof",
        lambda proof, proof_flags, proof_field: _kagemusha_read_pallas_ipa_proof(
            proof,
            proof_flags,
            params_n,
            proof_field,
        ),
    )
    transcript_label, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.transcript_label",
        lambda data, data_flags, data_field: _kagemusha_read_string_payload(
            data,
            data_flags,
            data_field,
        ),
    )
    if (
        not transcript_label
        or len(transcript_label.encode("utf-8"))
        > _KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
    ):
        raise ValueError(f"{field}.transcript_label is invalid")
    for metadata in ("vk_commitment", "public_inputs_schema_hash", "domain_tag"):
        _, cursor = _kagemusha_read_decoded_field(
            payload,
            cursor,
            flags,
            f"{field}.{metadata}",
            _kagemusha_read_required_metadata_option,
        )
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")


def _kagemusha_read_decoded_field(payload: bytes, cursor: int, flags: int, field: str, decode):
    child, next_cursor = _kagemusha_read_norito_field(payload, cursor, flags, field)
    return decode(child, flags, field), next_cursor


def _kagemusha_read_pallas_ipa_params(payload: bytes, flags: int, field: str) -> int:
    cursor = 0
    version, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.version",
        _kagemusha_read_u16,
    )
    curve_id, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.curve_id",
        _kagemusha_read_u16,
    )
    n, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.n",
        _kagemusha_read_u32,
    )
    if (
        version != 1
        or curve_id != _KAGEMUSHA_PALLAS_CURVE_ID
        or n < 2
        or n & (n - 1)
        or n > _KAGEMUSHA_PALLAS_OPEN_ENVELOPE_MAX_N
    ):
        raise ValueError(f"{field} is invalid")
    g_count, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.g",
        lambda data, data_flags, data_field: _kagemusha_read_fixed32_sequence_count(
            data,
            data_flags,
            data_field,
            expected_count=n,
            mismatch_field=field,
            mismatch_message=f"{field} generator count mismatch",
        ),
    )
    h_count, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.h",
        lambda data, data_flags, data_field: _kagemusha_read_fixed32_sequence_count(
            data,
            data_flags,
            data_field,
            expected_count=n,
            mismatch_field=field,
            mismatch_message=f"{field} generator count mismatch",
        ),
    )
    if g_count != n or h_count != n:
        raise ValueError(f"{field} generator count mismatch")
    _, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.u",
        lambda data, data_flags, data_field: _kagemusha_read_fixed_bytes_payload(
            data,
            data_flags,
            32,
            data_field,
        ),
    )
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return n


def _kagemusha_read_pallas_poly_open_public(payload: bytes, flags: int, field: str) -> int:
    cursor = 0
    version, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.version",
        _kagemusha_read_u16,
    )
    curve_id, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.curve_id",
        _kagemusha_read_u16,
    )
    n, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.n",
        _kagemusha_read_u32,
    )
    if version != 1 or curve_id != _KAGEMUSHA_PALLAS_CURVE_ID:
        raise ValueError(f"{field} is invalid")
    for name in ("z", "t", "p_g"):
        _, cursor = _kagemusha_read_decoded_field(
            payload,
            cursor,
            flags,
            f"{field}.{name}",
            lambda data, data_flags, data_field: _kagemusha_read_fixed_bytes_payload(
                data,
                data_flags,
                32,
                data_field,
            ),
        )
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return n


def _kagemusha_read_pallas_ipa_proof(
    payload: bytes,
    flags: int,
    n: int,
    field: str,
) -> None:
    cursor = 0
    version, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.version",
        _kagemusha_read_u16,
    )
    expected_rounds = n.bit_length() - 1
    l_count, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.l",
        lambda data, data_flags, data_field: _kagemusha_read_fixed32_sequence_count(
            data,
            data_flags,
            data_field,
            expected_count=expected_rounds,
            mismatch_field=field,
            mismatch_message=f"{field} round count mismatch",
        ),
    )
    r_count, cursor = _kagemusha_read_decoded_field(
        payload,
        cursor,
        flags,
        f"{field}.r",
        lambda data, data_flags, data_field: _kagemusha_read_fixed32_sequence_count(
            data,
            data_flags,
            data_field,
            expected_count=expected_rounds,
            mismatch_field=field,
            mismatch_message=f"{field} round count mismatch",
        ),
    )
    if version != 1 or l_count != r_count or l_count != expected_rounds:
        raise ValueError(f"{field} round count mismatch")
    for name in ("a_final", "b_final"):
        _, cursor = _kagemusha_read_decoded_field(
            payload,
            cursor,
            flags,
            f"{field}.{name}",
            lambda data, data_flags, data_field: _kagemusha_read_fixed_bytes_payload(
                data,
                data_flags,
                32,
                data_field,
            ),
        )
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return None


def _kagemusha_read_fixed32_sequence_count(
    payload: bytes,
    flags: int,
    field: str,
    *,
    expected_count: int | None = None,
    mismatch_field: str | None = None,
    mismatch_message: str | None = None,
) -> int:
    if len(payload) < 8:
        raise ValueError(f"{field} count is truncated")
    count = int.from_bytes(payload[:8], "little")
    if expected_count is not None and count != expected_count:
        raise ValueError(mismatch_message or f"{mismatch_field or field} count mismatch")
    return len(_kagemusha_read_fixed32_sequence(payload, flags, field))


def _kagemusha_read_fixed32_sequence(
    payload: bytes,
    flags: int,
    field: str,
) -> list[bytes]:
    if len(payload) < 8:
        raise ValueError(f"{field} count is truncated")
    count = int.from_bytes(payload[:8], "little")
    cursor = 8
    values = []
    for index in range(count):
        item, cursor = _kagemusha_read_norito_field(
            payload,
            cursor,
            flags,
            f"{field}[{index}]",
        )
        values.append(_kagemusha_read_fixed_bytes_payload(item, flags, 32, f"{field}[{index}]"))
    if cursor != len(payload):
        raise ValueError(f"{field} has trailing bytes")
    return values


def _kagemusha_read_topup_anchor_nullifiers(payload: bytes, flags: int) -> tuple[bytes, ...]:
    field = "bundle.accumulator.topup_anchor_nullifiers"
    if len(payload) < 8:
        raise ValueError(f"{field} count is truncated")
    count = int.from_bytes(payload[:8], "little")
    if count == 0 or count > KAGEMUSHA_FOLD_STEP_MAX_INPUTS:
        raise ValueError(f"{field} count is out of range")
    return tuple(_kagemusha_read_fixed32_sequence(payload, flags, field))


def _kagemusha_read_required_metadata_option(payload: bytes, flags: int, field: str) -> bytes:
    if not payload:
        raise ValueError(f"{field} option tag is truncated")
    tag = payload[0]
    if tag == 0:
        raise ValueError(f"{field} is required")
    if tag != 1:
        raise ValueError(f"{field} option tag must be 0 or 1")
    length, start = _kagemusha_read_norito_length(payload, 1, flags, f"{field}.length")
    end = start + length
    if end != len(payload):
        raise ValueError(f"{field} payload length mismatch")
    value = payload[start:end]
    if len(value) != 32:
        raise ValueError(f"{field} must be exactly 32 bytes")
    if not any(value):
        raise ValueError(f"{field} must be non-zero")
    return value


def _kagemusha_require_nested_archive(value: BytesLike, field: str) -> bytes:
    data = _norito_archive_bytes_named(value, field)
    return data


def _kagemusha_archive_bytes_named(value: BytesLike, field: str) -> bytes:
    return _archive_bytes_named(value, field)


def _kagemusha_fixed32(value: BytesLike, field: str) -> bytes:
    data = bytes(value)
    if len(data) != 32:
        raise ValueError(f"{field} must be exactly 32 bytes")
    return data


def _kagemusha_is_zero32(value: bytes) -> bool:
    return all(byte == 0 for byte in value)


def _kagemusha_iroha_hash(value: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(value, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


_KAGEMUSHA_BLAKE3_IV = (
    0x6A09E667,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
)
_KAGEMUSHA_BLAKE3_MSG_PERMUTATION = (2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
_KAGEMUSHA_BLAKE3_CHUNK_START = 1
_KAGEMUSHA_BLAKE3_CHUNK_END = 2
_KAGEMUSHA_BLAKE3_ROOT = 8


def _kagemusha_u32(value: int) -> int:
    return value & 0xFFFF_FFFF


def _kagemusha_rotate_right_u32(value: int, bits: int) -> int:
    value &= 0xFFFF_FFFF
    return ((value >> bits) | (value << (32 - bits))) & 0xFFFF_FFFF


def _kagemusha_blake3_mix(
    state: list[int],
    a: int,
    b: int,
    c: int,
    d: int,
    x: int,
    y: int,
) -> None:
    state[a] = _kagemusha_u32(state[a] + state[b] + x)
    state[d] = _kagemusha_rotate_right_u32(state[d] ^ state[a], 16)
    state[c] = _kagemusha_u32(state[c] + state[d])
    state[b] = _kagemusha_rotate_right_u32(state[b] ^ state[c], 12)
    state[a] = _kagemusha_u32(state[a] + state[b] + y)
    state[d] = _kagemusha_rotate_right_u32(state[d] ^ state[a], 8)
    state[c] = _kagemusha_u32(state[c] + state[d])
    state[b] = _kagemusha_rotate_right_u32(state[b] ^ state[c], 7)


def _kagemusha_blake3_round(state: list[int], message: list[int]) -> None:
    _kagemusha_blake3_mix(state, 0, 4, 8, 12, message[0], message[1])
    _kagemusha_blake3_mix(state, 1, 5, 9, 13, message[2], message[3])
    _kagemusha_blake3_mix(state, 2, 6, 10, 14, message[4], message[5])
    _kagemusha_blake3_mix(state, 3, 7, 11, 15, message[6], message[7])
    _kagemusha_blake3_mix(state, 0, 5, 10, 15, message[8], message[9])
    _kagemusha_blake3_mix(state, 1, 6, 11, 12, message[10], message[11])
    _kagemusha_blake3_mix(state, 2, 7, 8, 13, message[12], message[13])
    _kagemusha_blake3_mix(state, 3, 4, 9, 14, message[14], message[15])


def _kagemusha_blake3_hash_small_input(data: bytes) -> bytes:
    if len(data) > 64:
        raise ValueError("asset definition checksum preimage must fit one BLAKE3 block")
    block = data + bytes(64 - len(data))
    message = [
        int.from_bytes(block[index * 4 : index * 4 + 4], "little")
        for index in range(16)
    ]
    state = [
        *_KAGEMUSHA_BLAKE3_IV,
        *_KAGEMUSHA_BLAKE3_IV[:4],
        0,
        0,
        len(data),
        _KAGEMUSHA_BLAKE3_CHUNK_START
        | _KAGEMUSHA_BLAKE3_CHUNK_END
        | _KAGEMUSHA_BLAKE3_ROOT,
    ]
    for round_index in range(7):
        _kagemusha_blake3_round(state, message)
        if round_index < 6:
            message = [message[index] for index in _KAGEMUSHA_BLAKE3_MSG_PERMUTATION]
    return b"".join(
        _kagemusha_u32(state[index] ^ state[index + 8]).to_bytes(4, "little")
        for index in range(8)
    )


def _kagemusha_is_uuid_v4_bytes(value: bytes) -> bool:
    return (
        len(value) == 16
        and (value[6] & 0xF0) == 0x40
        and (value[8] & 0xC0) == 0x80
    )


def _kagemusha_base58_encode(value: bytes) -> str:
    number = int.from_bytes(value, "big")
    encoded: list[str] = []
    while number:
        number, remainder = divmod(number, 58)
        encoded.append(_KAGEMUSHA_ASSET_DEFINITION_BASE58_ALPHABET[remainder])
    for byte in value:
        if byte != 0:
            break
        encoded.append(_KAGEMUSHA_ASSET_DEFINITION_BASE58_ALPHABET[0])
    return "".join(reversed(encoded)) or _KAGEMUSHA_ASSET_DEFINITION_BASE58_ALPHABET[0]


def _kagemusha_asset_definition_from_bytes(value: bytes) -> str:
    if not _kagemusha_is_uuid_v4_bytes(value):
        return "hex:" + value.hex()
    body = bytes([_KAGEMUSHA_ASSET_DEFINITION_ADDRESS_VERSION]) + value
    checksum = _kagemusha_blake3_hash_small_input(body)[:4]
    return _kagemusha_base58_encode(body + checksum)


def _kagemusha_canonical_u128_decimal(value: str, field: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field} must be a decimal integer")
    if any(ch < "0" or ch > "9" for ch in value):
        raise ValueError(f"{field} must be a decimal integer")
    if len(value) > 1 and value.startswith("0"):
        raise ValueError(f"{field} must be canonical")
    if len(value) > _KAGEMUSHA_U128_MAX_DECIMAL_DIGITS:
        raise ValueError(f"{field} must fit in u128")
    integer = int(value)
    if integer <= 0:
        raise ValueError(f"{field} must be greater than zero")
    if integer > _KAGEMUSHA_U128_MAX:
        raise ValueError(f"{field} must fit in u128")
    return str(integer)


def _kagemusha_require_redeem_change_binding(
    public_amount: str,
    current_amount: str,
    has_change_output: bool,
) -> None:
    comparison = _kagemusha_compare_canonical_decimal(public_amount, current_amount)
    if has_change_output:
        if comparison >= 0:
            raise ValueError(
                "public_amount must be less than current note amount when change_output is present"
            )
    elif comparison < 0:
        raise ValueError("change_output is required when public_amount is less than current note amount")
    elif comparison > 0:
        raise ValueError("public_amount must not exceed current note amount")


def _kagemusha_require_redeem_change_output_not_reserved(
    change_output: bytes,
    bundle_summary: KagemushaRecursiveSpendBundleSummary,
) -> None:
    reserved = (
        bundle_summary.current_note.note_commitment,
        bundle_summary.current_note.spend_nullifier,
        *bundle_summary.topup_anchor_nullifiers,
    )
    if any(change_output == value for value in reserved):
        raise ValueError(
            "change_output must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier"
        )


def _kagemusha_require_recursive_spend_topup_anchor_nullifiers(
    topup_anchor_nullifiers: tuple[bytes, ...],
    current_note: KagemushaRecursiveSpendableNoteDescriptor,
) -> None:
    if not topup_anchor_nullifiers or len(topup_anchor_nullifiers) > KAGEMUSHA_FOLD_STEP_MAX_INPUTS:
        raise ValueError("bundle.accumulator.topup_anchor_nullifiers count is out of range")
    previous: bytes | None = None
    for nullifier in topup_anchor_nullifiers:
        if _kagemusha_is_zero32(nullifier):
            raise ValueError("bundle.accumulator.topup_anchor_nullifiers must not contain zero values")
        if previous is not None and previous >= nullifier:
            raise ValueError("bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique")
        previous = nullifier
    if (
        current_note.note_commitment in topup_anchor_nullifiers
        or current_note.spend_nullifier in topup_anchor_nullifiers
    ):
        raise ValueError("bundle.accumulator.topup_anchor_nullifiers must not reuse current note material")


def _kagemusha_require_recursive_spend_accumulator_roots(
    initial_root: bytes,
    final_root: bytes,
) -> None:
    if _kagemusha_is_zero32(initial_root):
        raise ValueError("bundle.accumulator.initial_root")
    if _kagemusha_is_zero32(final_root) or final_root == initial_root:
        raise ValueError("bundle.accumulator.final_root")


def _kagemusha_require_recursive_spend_accumulator_corridor(
    payload: bytes,
    cursor: int,
    flags: int,
    hop_count: int,
) -> int:
    def read_fixed32(field: str) -> bytes:
        nonlocal cursor
        field_payload, cursor = _kagemusha_read_norito_field(
            payload,
            cursor,
            flags,
            f"accumulator.{field}",
        )
        return _kagemusha_read_fixed_bytes_payload(
            field_payload,
            flags,
            32,
            f"bundle.accumulator.{field}",
        )

    def require_nonzero(field: str) -> bytes:
        value = read_fixed32(field)
        if _kagemusha_is_zero32(value):
            raise ValueError(f"bundle.accumulator.{field}")
        return value

    lineage_digest = require_nonzero("lineage_digest")
    aggregation_transcript_digest = read_fixed32("aggregation_transcript_digest")
    if (
        _kagemusha_is_zero32(aggregation_transcript_digest)
        or aggregation_transcript_digest != lineage_digest
    ):
        raise ValueError("bundle.accumulator.aggregation_transcript_digest")
    for field in (
        "nullifier_digest",
        "output_commitment_digest",
        "fold_digest",
        "recursive_proof_chain_digest",
        "transition_profile_binding_digest",
    ):
        require_nonzero(field)
    append_opening_preflight_digest = read_fixed32("append_opening_preflight_digest")
    if not _kagemusha_is_zero32(append_opening_preflight_digest) and hop_count <= 1:
        raise ValueError("bundle.accumulator.append_opening_preflight_digest")
    append_boundary_digest = read_fixed32("append_boundary_digest")
    if not _kagemusha_is_zero32(append_boundary_digest) and (
        _kagemusha_is_zero32(append_opening_preflight_digest) or hop_count <= 1
    ):
        raise ValueError("bundle.accumulator.append_boundary_digest")
    for field in (
        "verifier_params_fingerprint",
        "fixed_window_table_schedule_digest",
        "fixed_window_shared_table_manifest_digest",
        "fixed_window_table_base_digest",
        "verifier_witness_batch_digest",
    ):
        require_nonzero(field)
    verifier_opening_payload, cursor = _kagemusha_read_norito_field(
        payload,
        cursor,
        flags,
        "accumulator.verifier_opening_len",
    )
    try:
        verifier_opening_len = _kagemusha_read_u32(verifier_opening_payload, flags)
    except ValueError as error:
        raise ValueError("bundle.accumulator.verifier_opening_len") from error
    if not is_supported_kagemusha_recursive_spend_lineage_key_artifact_opening_len(
        verifier_opening_len
    ):
        raise ValueError("bundle.accumulator.verifier_opening_len")
    return cursor


def _kagemusha_compare_canonical_decimal(left: str, right: str) -> int:
    if len(left) != len(right):
        return -1 if len(left) < len(right) else 1
    if left == right:
        return 0
    return -1 if left < right else 1


def _kagemusha_require_portable_id(value: str, field: str) -> None:
    _kagemusha_require_non_blank_unpadded(value, field)
    if len(value) > 256:
        raise ValueError(f"{field} must not exceed 256 characters")
    allowed = set("._-/:@+=")
    if any(not ("A" <= ch <= "Z" or "a" <= ch <= "z" or "0" <= ch <= "9" or ch in allowed) for ch in value):
        raise ValueError(f"{field} must use portable registry syntax")


def _kagemusha_require_non_blank_unpadded(value: str, field: str) -> None:
    if not isinstance(value, str):
        raise TypeError(f"{field} must be a string")
    if not value.strip():
        raise ValueError(f"{field} must not be blank")
    if value.strip() != value:
        raise ValueError(f"{field} must not contain surrounding whitespace")


def kagemusha_prove_verified_compact_payment_token_with_records(
    record_bundle_archive: BytesLike,
) -> bytes:
    return _call_native_archive_method(
        _COMPACT_TOKEN_METHOD,
        _norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive"),
    )


def _kagemusha_build_pallas_open_envelopes_archive(
    record_bundle_archive: BytesLike,
) -> bytes:
    record_bundle = _norito_archive_bytes_named(
        record_bundle_archive,
        "record_bundle_archive",
    )
    if not is_kagemusha_pallas_open_envelope_builder_available():
        raise RuntimeError(
            "Kagemusha Pallas open-envelope builders require native bridge ABI 7 "
            "with Pallas builder symbols"
        )
    return _call_native_archive_method(
        _PALLAS_OPEN_ENVELOPE_BUILDER_METHOD,
        record_bundle,
    )


def _kagemusha_build_previous_proof_open_envelopes_archive(
    previous_bundle_archive: BytesLike,
) -> bytes:
    previous_bundle = _norito_archive_bytes_named(
        previous_bundle_archive,
        "previous_bundle_archive",
    )
    if not is_kagemusha_pallas_open_envelope_builder_available():
        raise RuntimeError(
            "Kagemusha Pallas open-envelope builders require native bridge ABI 7 "
            "with Pallas builder symbols"
        )
    return _call_native_archive_method(
        _PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD,
        previous_bundle,
    )


def _prove_verified_recursive_aggregation_proof_bundle(
    record_bundle_archive: BytesLike,
    pallas_open_envelopes_archive: BytesLike,
) -> bytes:
    return _call_native_archive_method(
        _RECURSIVE_AGGREGATION_METHOD,
        _norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive"),
        _norito_archive_bytes_named(pallas_open_envelopes_archive, "pallas_open_envelopes_archive"),
    )


def _prove_verified_recursive_compact_payment_token(
    record_bundle_archive: BytesLike,
    pallas_open_envelopes_archive: BytesLike,
    recursive_compact_key_artifacts_archive: BytesLike,
) -> bytes:
    record_bundle = _norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")
    pallas_open_envelopes = _norito_archive_bytes_named(pallas_open_envelopes_archive, "pallas_open_envelopes_archive")
    recursive_compact_key_artifacts = _norito_archive_bytes_named(
        recursive_compact_key_artifacts_archive,
        "recursive_compact_key_artifacts_archive",
    )
    if not is_kagemusha_recursive_compact_payment_token_prover_available():
        raise RuntimeError(
            "recursive compact Kagemusha payment-token prover requires native "
            "native bridge ABI 7 with compact prover and verifier symbols"
        )
    return _call_native_archive_method(
        _RECURSIVE_COMPACT_TOKEN_METHOD,
        record_bundle,
        pallas_open_envelopes,
        recursive_compact_key_artifacts,
    )


def _verify_recursive_compact_payment_token(
    compact_token_archive: BytesLike,
    recursive_compact_verifier_keys_archive: BytesLike,
) -> bool:
    compact_token = _archive_bytes_named(compact_token_archive, "compact_token_archive")
    _assert_kagemusha_norito_archive(compact_token, "compact_token_archive")
    recursive_compact_verifier_keys = _archive_bytes_named(
        recursive_compact_verifier_keys_archive,
        "recursive_compact_verifier_keys_archive",
    )
    _assert_kagemusha_norito_archive(
        recursive_compact_verifier_keys,
        "recursive_compact_verifier_keys_archive",
    )
    if not is_kagemusha_recursive_compact_payment_token_verifier_available():
        raise RuntimeError(
            "recursive compact Kagemusha payment-token verifier requires native "
            "native bridge ABI 7 with the compact verifier symbol"
        )
    result = _native_method(_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD)(
        compact_token,
        recursive_compact_verifier_keys,
    )
    if not isinstance(result, bool):
        raise RuntimeError(
            f"{_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD} returned non-boolean result"
        )
    return result


def _recursive_spend_compact_payment_token_from_bundle(
    bundle_archive: BytesLike,
) -> bytes:
    bundle = _norito_archive_bytes_named(bundle_archive, "bundle_archive")
    if not is_kagemusha_recursive_spend_compact_payment_token_projection_available():
        raise RuntimeError(
            "recursive spend compact Kagemusha payment-token projection requires "
            "native bridge ABI 7 with the compact projection symbol"
        )
    return _call_native_archive_method(
        _RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD,
        bundle,
    )


def _verify_recursive_spend_compact_payment_token_projection(
    compact_token_archive: BytesLike,
    verifier_record_archive: BytesLike,
    block_height: int | None = None,
) -> bool:
    compact_token = _archive_bytes_named(compact_token_archive, "compact_token_archive")
    verifier_record = _archive_bytes_named(verifier_record_archive, "verifier_record_archive")
    _assert_kagemusha_norito_archive(compact_token, "compact_token_archive")
    _assert_kagemusha_norito_archive(verifier_record, "verifier_record_archive")
    checked_block_height = _validate_kagemusha_block_height(block_height)
    if not is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available():
        raise RuntimeError(
            "recursive spend compact Kagemusha payment-token projection verifier "
            "requires native bridge ABI 7 with the compact projection verifier symbols"
        )
    if checked_block_height is None:
        result = _native_method(_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD)(
            compact_token,
            verifier_record,
        )
    else:
        result = _native_method(
            _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD
        )(
            compact_token,
            verifier_record,
            checked_block_height,
        )
    if not isinstance(result, bool):
        raise RuntimeError(
            f"{_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD} returned non-boolean result"
        )
    return result


def _validate_kagemusha_block_height(block_height: int | None) -> int | None:
    if block_height is None:
        return None
    if isinstance(block_height, bool) or not isinstance(block_height, int):
        raise TypeError("block_height must be an integer")
    if block_height < 0:
        raise ValueError("block_height must be non-negative")
    if block_height > _KAGEMUSHA_U64_MAX:
        raise ValueError("block_height must fit in u64")
    return block_height


def _verify_recursive_spend_compact_payment_token_projection_at_height(
    compact_token_archive: BytesLike,
    verifier_record_archive: BytesLike,
    block_height: int,
) -> bool:
    return _verify_recursive_spend_compact_payment_token_projection(
        compact_token_archive,
        verifier_record_archive,
        block_height=block_height,
    )


def _call_native_archive_method(name: str, *archives: bytes) -> bytes:
    result = _native_method(name)(*archives)
    return _require_kagemusha_native_output(name, result)


def _require_kagemusha_native_output(name: str, result: object) -> bytes:
    if result is None:
        raise RuntimeError(f"{name} returned no output")
    if isinstance(result, str):
        raise RuntimeError(f"{name} returned text instead of Norito bytes")
    try:
        view = memoryview(result)
    except TypeError:
        output = bytes(result)
    else:
        if view.nbytes == 0:
            raise RuntimeError(f"{name} returned empty output")
        if view.nbytes > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES:
            raise RuntimeError(f"{name} returned oversized output")
        output = view.tobytes()
    if not output:
        raise RuntimeError(f"{name} returned empty output")
    if len(output) > KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES:
        raise RuntimeError(f"{name} returned oversized output")
    try:
        _assert_kagemusha_norito_archive(output, name)
    except ValueError as error:
        if "non-empty Norito payload" in str(error):
            raise RuntimeError(f"{name} returned empty Norito payload") from error
        raise RuntimeError(f"{name} returned invalid Norito archive") from error
    return output


globals()[_RECURSIVE_AGGREGATION_METHOD] = _prove_verified_recursive_aggregation_proof_bundle
globals()[_PALLAS_OPEN_ENVELOPE_BUILDER_METHOD] = (
    _kagemusha_build_pallas_open_envelopes_archive
)
globals()[_PREVIOUS_PROOF_OPEN_ENVELOPE_BUILDER_METHOD] = (
    _kagemusha_build_previous_proof_open_envelopes_archive
)
globals()[_RECURSIVE_COMPACT_TOKEN_METHOD] = _prove_verified_recursive_compact_payment_token
globals()[_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD] = _verify_recursive_compact_payment_token
globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD] = (
    _recursive_spend_compact_payment_token_from_bundle
)
globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD] = (
    _verify_recursive_spend_compact_payment_token_projection
)
globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_AT_HEIGHT_METHOD] = (
    _verify_recursive_spend_compact_payment_token_projection_at_height
)


def _normalize_kagemusha_instruction_archive_type(
    instruction_type: str,
) -> KagemushaInstructionArchiveType:
    if not isinstance(instruction_type, str):
        raise TypeError("instruction_type must be a string")
    if instruction_type not in KAGEMUSHA_INSTRUCTION_ARCHIVE_TYPES:
        raise ValueError(
            "instruction_type must be KagemushaTransfer or RedeemKagemushaRecursive"
        )
    return instruction_type  # type: ignore[return-value]


def kagemusha_instruction_archive_instruction(
    instruction_type: str,
    instruction_archive: BytesLike,
) -> object:
    """Return an Iroha instruction from a typed Kagemusha Norito archive."""

    normalized_type = _normalize_kagemusha_instruction_archive_type(instruction_type)
    archive = _norito_archive_bytes_named(instruction_archive, "instruction_archive")
    _assert_kagemusha_instruction_archive_schema(
        archive,
        normalized_type,
        "instruction_archive",
    )
    from .crypto import Instruction

    builder = getattr(Instruction, "kagemusha_instruction_archive", None)
    if builder is None:
        raise RuntimeError(
            "kagemusha_instruction_archive_instruction requires a compiled "
            "iroha_python._crypto extension with Kagemusha instruction archive support"
        )
    return builder(normalized_type, archive)


def kagemusha_recursive_redeem_instruction(redeem_request_archive: BytesLike) -> object:
    """Derive a recursive redeem instruction from a redeem request archive."""

    request = _norito_archive_bytes_named(redeem_request_archive, "redeem_request_archive")
    from .crypto import Instruction

    builder = getattr(Instruction, "kagemusha_recursive_redeem", None)
    if builder is None:
        raise RuntimeError(
            "kagemusha_recursive_redeem_instruction requires a compiled "
            "iroha_python._crypto extension with recursive Kagemusha support"
        )
    return builder(request)


def build_kagemusha_instruction_transaction(
    chain_id: str,
    authority: str,
    private_key: BytesLike,
    instruction_type: str,
    instruction_archive: BytesLike,
    *,
    creation_time_ms: Optional[int] = None,
    ttl_ms: Optional[int] = None,
    nonce: Optional[int] = None,
    metadata: Optional[Mapping[str, Any]] = None,
) -> object:
    """Sign a single-instruction transaction from a Kagemusha instruction archive."""

    _kagemusha_require_non_blank_unpadded(chain_id, "chain_id")
    _kagemusha_require_non_blank_unpadded(authority, "authority")
    instruction = kagemusha_instruction_archive_instruction(
        instruction_type,
        instruction_archive,
    )
    private_key_bytes = _archive_bytes_named(private_key, "private_key")
    from .crypto import build_signed_transaction

    return build_signed_transaction(
        chain_id,
        authority,
        private_key_bytes,
        instructions=(instruction,),
        creation_time_ms=creation_time_ms,
        ttl_ms=ttl_ms,
        nonce=nonce,
        metadata=metadata,
    )


def build_kagemusha_recursive_redeem_transaction(
    chain_id: str,
    authority: str,
    private_key: BytesLike,
    redeem_request_archive: BytesLike,
    *,
    creation_time_ms: Optional[int] = None,
    ttl_ms: Optional[int] = None,
    nonce: Optional[int] = None,
    metadata: Optional[Mapping[str, Any]] = None,
) -> object:
    """Derive the native recursive redeem instruction, then sign its transaction."""

    _kagemusha_require_non_blank_unpadded(chain_id, "chain_id")
    _kagemusha_require_non_blank_unpadded(authority, "authority")
    instruction = kagemusha_recursive_redeem_instruction(redeem_request_archive)
    private_key_bytes = _archive_bytes_named(private_key, "private_key")
    from .crypto import build_signed_transaction

    return build_signed_transaction(
        chain_id,
        authority,
        private_key_bytes,
        instructions=(instruction,),
        creation_time_ms=creation_time_ms,
        ttl_ms=ttl_ms,
        nonce=nonce,
        metadata=metadata,
    )


def kagemusha_recursive_spend_init(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_init", request_archive)


def kagemusha_recursive_spend_append(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_append", request_archive)


def kagemusha_recursive_spend_transition_profile_init(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method(
        "kagemusha_recursive_spend_transition_profile_init",
        request_archive,
    )


def kagemusha_recursive_spend_transition_profile_append(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method(
        "kagemusha_recursive_spend_transition_profile_append",
        request_archive,
    )


def kagemusha_recursive_spend_lineage_append_boundary(profile_archive: BytesLike) -> bytes:
    return _call_recursive_spend_multi_archive_method(
        "kagemusha_recursive_spend_lineage_append_boundary",
        _norito_archive_bytes_named(profile_archive, "profile_archive"),
    )


def kagemusha_recursive_spend_lineage_witness_from_init_result(
    request_archive: BytesLike,
    bundle_archive: BytesLike,
) -> bytes:
    return _call_recursive_spend_multi_archive_method(
        "kagemusha_recursive_spend_lineage_witness_from_init_result",
        _norito_archive_bytes_named(request_archive, "request_archive"),
        _norito_archive_bytes_named(bundle_archive, "bundle_archive"),
    )


def kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness_archive: BytesLike,
    request_archive: BytesLike,
    bundle_archive: BytesLike,
) -> bytes:
    return _call_recursive_spend_multi_archive_method(
        "kagemusha_recursive_spend_lineage_witness_append_result",
        _norito_archive_bytes_named(previous_witness_archive, "previous_witness_archive"),
        _norito_archive_bytes_named(request_archive, "request_archive"),
        _norito_archive_bytes_named(bundle_archive, "bundle_archive"),
    )


def kagemusha_recursive_spend_verify(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_verify", request_archive)


def kagemusha_recursive_spend_redeem(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_redeem", request_archive)


def _call_recursive_spend_method(name: str, request_archive: BytesLike) -> bytes:
    request = _norito_archive_bytes_named(request_archive, "request_archive")
    module = load_crypto_extension()
    _require_complete_recursive_spend_surface(module)
    method = getattr(module, name, None)
    if method is None:
        raise RuntimeError(
            f"{name} requires a compiled iroha_python._crypto extension "
            "with recursive Kagemusha support"
        )
    return _require_kagemusha_native_output(name, method(request))


def _call_recursive_spend_multi_archive_method(name: str, *archives: bytes) -> bytes:
    module = load_crypto_extension()
    _require_complete_recursive_spend_surface(module)
    method = getattr(module, name, None)
    if method is None:
        raise RuntimeError(
            f"{name} requires a compiled iroha_python._crypto extension "
            "with recursive Kagemusha support"
        )
    return _require_kagemusha_native_output(name, method(*archives))
