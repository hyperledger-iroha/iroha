"""Native recursive Kagemusha offline-cash helpers.

These helpers operate on raw Norito archives so Python applications do not
reimplement recursive proof internals.
"""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Literal, Union

from ._native import load_crypto_extension

BytesLike = Union[bytes, bytearray, memoryview]
KagemushaOfflineSpendMode = Literal[
    "recursive_compact_v1",
    "recursive_spend_v1",
    "checked_prefold_v1",
]

KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1 = "recursive_compact_v1"
KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1 = "recursive_spend_v1"
KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1"
KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION = 6
KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION = 7
KAGEMUSHA_MAX_BRIDGE_ABI_VERSION = 0xFFFF_FFFF
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
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 = True
KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 = 1
KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES = 8 * 1024 * 1024
KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128
KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024
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
    "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION",
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
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1",
    "KagemushaOfflineSpendMode",
    "KagemushaRecursiveSpendLineageKeyArtifacts",
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
    "is_kagemusha_recursive_compact_payment_token_prover_available",
    "is_kagemusha_recursive_compact_payment_token_verifier_available",
    "is_kagemusha_recursive_spend_compact_payment_token_projection_available",
    "is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available",
    "is_kagemusha_recursive_compact_unavailable",
    "is_kagemusha_recursive_spend_available",
    "preferred_kagemusha_offline_spend_mode_for_capabilities",
    "preferred_kagemusha_offline_spend_mode",
    "kagemusha_prove_verified_compact_payment_token_with_records",
    _RECURSIVE_AGGREGATION_METHOD,
    _RECURSIVE_COMPACT_TOKEN_METHOD,
    _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,
    _RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD,
    _RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD,
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_transition_profile_init",
    "kagemusha_recursive_spend_transition_profile_append",
    "kagemusha_recursive_spend_lineage_append_boundary",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
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
_RECURSIVE_SPEND_ABI_VERSION_METHOD = "kagemusha_recursive_spend_bridge_abi_version"
_MALFORMED_NATIVE_PROBE_ARCHIVE = b"\x00"
_KAGEMUSHA_NORITO_HEADER_BYTES = 40
_KAGEMUSHA_NORITO_MAX_HEADER_PADDING_BYTES = 64
_KAGEMUSHA_NORITO_SUPPORTED_FLAGS_MASK = 0x27
_KAGEMUSHA_NORITO_FIELD_BITSET_FLAG = 0x20
_KAGEMUSHA_NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06
_KAGEMUSHA_NORITO_MAGIC = b"NRT0"
_KAGEMUSHA_CRC64_MASK = 0xFFFF_FFFF_FFFF_FFFF
_KAGEMUSHA_CRC64_REFLECTED_POLY = 0xC96C_5795_D787_0F42
_KAGEMUSHA_ZK1_MAGIC = b"ZK1\x00"
_KAGEMUSHA_ZK1_TLV_CID1 = b"CID1"
_KAGEMUSHA_ZK1_TLV_IPAK = b"IPAK"
_KAGEMUSHA_ZK1_TLV_H2VK = b"H2VK"


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
            circuit_id = payload.decode("utf-8").strip()
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
        return _assert_kagemusha_norito_archive(
            lineage_proving_key_archive,
            "lineage_proving_key_archive",
        )
    except ValueError as exc:
        raise ValueError("lineage_proving_key_archive") from exc


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
        or version > KAGEMUSHA_MAX_BRIDGE_ABI_VERSION
    ):
        return None
    return version


def _has_recursive_spend_abi(module: object) -> bool:
    version = _recursive_spend_abi_version(module)
    return (
        version is not None
        and version >= KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION
    )


def _has_recursive_compact_abi(module: object) -> bool:
    version = _recursive_spend_abi_version(module)
    return (
        version is not None
        and version >= KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION
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
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    missing = _missing_recursive_spend_methods(module)
    if missing:
        missing_list = ", ".join(missing)
        raise RuntimeError(
            "recursive Kagemusha support requires the complete native bridge ABI "
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION} surface; "
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
        )
        and _probe_native_archive_method(
            module,
            _RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD,
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
    _ = recursive_compact_available
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


def kagemusha_prove_verified_compact_payment_token_with_records(
    record_bundle_archive: BytesLike,
) -> bytes:
    return _call_native_archive_method(
        _COMPACT_TOKEN_METHOD,
        _norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive"),
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
) -> bytes:
    record_bundle = _norito_archive_bytes_named(record_bundle_archive, "record_bundle_archive")
    pallas_open_envelopes = _norito_archive_bytes_named(pallas_open_envelopes_archive, "pallas_open_envelopes_archive")
    if not is_kagemusha_recursive_compact_payment_token_prover_available():
        raise RuntimeError(
            "recursive compact Kagemusha payment-token prover requires native "
            "bridge ABI 7 with compact prover and verifier symbols"
        )
    return _call_native_archive_method(
        _RECURSIVE_COMPACT_TOKEN_METHOD,
        record_bundle,
        pallas_open_envelopes,
    )


def _verify_recursive_compact_payment_token(compact_token_archive: BytesLike) -> bool:
    compact_token = _archive_bytes_named(compact_token_archive, "compact_token_archive")
    _assert_kagemusha_norito_archive(compact_token, "compact_token_archive")
    if not is_kagemusha_recursive_compact_payment_token_verifier_available():
        raise RuntimeError(
            "recursive compact Kagemusha payment-token verifier requires native "
            "bridge ABI 7 with the compact verifier symbol"
        )
    result = _native_method(_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD)(compact_token)
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
    if block_height is not None and block_height < 0:
        raise ValueError("block_height must be non-negative")
    if not is_kagemusha_recursive_spend_compact_payment_token_projection_verifier_available():
        raise RuntimeError(
            "recursive spend compact Kagemusha payment-token projection verifier "
            "requires native bridge ABI 7 with the compact projection verifier symbols"
        )
    if block_height is None:
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
            block_height,
        )
    if not isinstance(result, bool):
        raise RuntimeError(
            f"{_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD} returned non-boolean result"
        )
    return result


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
globals()[_RECURSIVE_COMPACT_TOKEN_METHOD] = _prove_verified_recursive_compact_payment_token
globals()[_RECURSIVE_COMPACT_TOKEN_VERIFY_METHOD] = _verify_recursive_compact_payment_token
globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_FROM_BUNDLE_METHOD] = (
    _recursive_spend_compact_payment_token_from_bundle
)
globals()[_RECURSIVE_SPEND_COMPACT_TOKEN_PROJECTION_VERIFY_METHOD] = (
    _verify_recursive_spend_compact_payment_token_projection
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
