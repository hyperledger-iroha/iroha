"""Native recursive Kagemusha offline-cash helpers.

These helpers operate on raw Norito archives so Python applications do not
reimplement recursive proof internals.
"""

from __future__ import annotations

from typing import Literal, Union

from ._native import load_crypto_extension

BytesLike = Union[bytes, bytearray, memoryview]
KagemushaOfflineSpendMode = Literal["recursive_spend_v1", "checked_prefold_v1"]

KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1 = "recursive_spend_v1"
KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1 = "checked_prefold_v1"
KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION = 6
KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-aggregation-v1"
)
KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 = (
    "kagemusha-recursive-spend-lineage-v1"
)

_COMPACT_TOKEN_METHOD = "kagemusha_prove_verified_compact_payment_token_with_records"
_RECURSIVE_AGGREGATION_METHOD = (
    "kagemusha_prove_verified_recursive_aggregation_proof_bundle"
    "_with_records_and_pallas_open_envelopes"
)

__all__ = [
    "KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1",
    "KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION",
    "KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1",
    "KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1",
    "KagemushaOfflineSpendMode",
    "is_kagemusha_compact_payment_token_prover_available",
    "is_kagemusha_recursive_aggregation_proof_bundle_prover_available",
    "is_kagemusha_recursive_spend_available",
    "preferred_kagemusha_offline_spend_mode",
    "kagemusha_prove_verified_compact_payment_token_with_records",
    _RECURSIVE_AGGREGATION_METHOD,
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
]

_NATIVE_METHODS = (
    "kagemusha_recursive_spend_init",
    "kagemusha_recursive_spend_append",
    "kagemusha_recursive_spend_lineage_witness_from_init_result",
    "kagemusha_recursive_spend_lineage_witness_append_result",
    "kagemusha_recursive_spend_verify",
    "kagemusha_recursive_spend_redeem",
)
_RECURSIVE_SPEND_ABI_VERSION_METHOD = "kagemusha_recursive_spend_bridge_abi_version"


def _archive_bytes(request_archive: BytesLike) -> bytes:
    data = bytes(request_archive)
    if not data:
        raise ValueError("request_archive must not be empty")
    return data


def _archive_bytes_named(archive: BytesLike, name: str) -> bytes:
    data = bytes(archive)
    if not data:
        raise ValueError(f"{name} must not be empty")
    return data


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
    return callable(getattr(module, name, None))


def _recursive_spend_abi_version(module: object) -> int | None:
    method = getattr(module, _RECURSIVE_SPEND_ABI_VERSION_METHOD, None)
    if not callable(method):
        return None
    try:
        version = int(method())
    except (TypeError, ValueError, RuntimeError):
        return None
    return version


def _has_recursive_spend_abi(module: object) -> bool:
    version = _recursive_spend_abi_version(module)
    return (
        version is not None
        and version >= KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION
    )


def is_kagemusha_compact_payment_token_prover_available() -> bool:
    return _is_native_method_available(_COMPACT_TOKEN_METHOD)


def is_kagemusha_recursive_aggregation_proof_bundle_prover_available() -> bool:
    return _is_native_method_available(_RECURSIVE_AGGREGATION_METHOD)


def is_kagemusha_recursive_spend_available() -> bool:
    try:
        module = load_crypto_extension()
    except RuntimeError:
        return False
    return _has_recursive_spend_abi(module) and all(
        callable(getattr(module, name, None)) for name in _NATIVE_METHODS
    )


def preferred_kagemusha_offline_spend_mode(
    recursive_spend_available: bool | None = None,
) -> KagemushaOfflineSpendMode:
    if recursive_spend_available is None:
        recursive_spend_available = is_kagemusha_recursive_spend_available()
    if recursive_spend_available:
        return KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    return KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1


def kagemusha_prove_verified_compact_payment_token_with_records(
    record_bundle_archive: BytesLike,
) -> bytes:
    return _call_native_archive_method(
        _COMPACT_TOKEN_METHOD,
        _archive_bytes_named(record_bundle_archive, "record_bundle_archive"),
    )


def _prove_verified_recursive_aggregation_proof_bundle(
    record_bundle_archive: BytesLike,
    pallas_open_envelopes_archive: BytesLike,
) -> bytes:
    return _call_native_archive_method(
        _RECURSIVE_AGGREGATION_METHOD,
        _archive_bytes_named(record_bundle_archive, "record_bundle_archive"),
        _archive_bytes_named(
            pallas_open_envelopes_archive,
            "pallas_open_envelopes_archive",
        ),
    )


def _call_native_archive_method(name: str, *archives: bytes) -> bytes:
    result = _native_method(name)(*archives)
    if result is None:
        raise RuntimeError(f"{name} returned no output")
    output = bytes(result)
    if not output:
        raise RuntimeError(f"{name} returned empty output")
    return output


globals()[_RECURSIVE_AGGREGATION_METHOD] = _prove_verified_recursive_aggregation_proof_bundle


def kagemusha_recursive_spend_init(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_init", request_archive)


def kagemusha_recursive_spend_append(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_append", request_archive)


def kagemusha_recursive_spend_lineage_witness_from_init_result(
    request_archive: BytesLike,
    bundle_archive: BytesLike,
) -> bytes:
    return _call_recursive_spend_multi_archive_method(
        "kagemusha_recursive_spend_lineage_witness_from_init_result",
        _archive_bytes_named(request_archive, "request_archive"),
        _archive_bytes_named(bundle_archive, "bundle_archive"),
    )


def kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness_archive: BytesLike,
    request_archive: BytesLike,
    bundle_archive: BytesLike,
) -> bytes:
    return _call_recursive_spend_multi_archive_method(
        "kagemusha_recursive_spend_lineage_witness_append_result",
        _archive_bytes_named(previous_witness_archive, "previous_witness_archive"),
        _archive_bytes_named(request_archive, "request_archive"),
        _archive_bytes_named(bundle_archive, "bundle_archive"),
    )


def kagemusha_recursive_spend_verify(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_verify", request_archive)


def kagemusha_recursive_spend_redeem(request_archive: BytesLike) -> bytes:
    return _call_recursive_spend_method("kagemusha_recursive_spend_redeem", request_archive)


def _call_recursive_spend_method(name: str, request_archive: BytesLike) -> bytes:
    request = _archive_bytes(request_archive)
    module = load_crypto_extension()
    if not _has_recursive_spend_abi(module):
        raise RuntimeError(
            "recursive Kagemusha support requires native bridge ABI "
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    method = getattr(module, name, None)
    if method is None:
        raise RuntimeError(
            f"{name} requires a compiled iroha_python._crypto extension "
            "with recursive Kagemusha support"
        )
    result = method(request)
    if result is None:
        raise RuntimeError(f"{name} returned no output")
    output = bytes(result)
    if not output:
        raise RuntimeError(f"{name} returned empty output")
    return output


def _call_recursive_spend_multi_archive_method(name: str, *archives: bytes) -> bytes:
    module = load_crypto_extension()
    if not _has_recursive_spend_abi(module):
        raise RuntimeError(
            "recursive Kagemusha support requires native bridge ABI "
            f"{KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION}"
        )
    method = getattr(module, name, None)
    if method is None:
        raise RuntimeError(
            f"{name} requires a compiled iroha_python._crypto extension "
            "with recursive Kagemusha support"
        )
    result = method(*archives)
    if result is None:
        raise RuntimeError(f"{name} returned no output")
    output = bytes(result)
    if not output:
        raise RuntimeError(f"{name} returned empty output")
    return output
