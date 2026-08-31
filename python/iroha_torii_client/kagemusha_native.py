"""Fail-closed native validation for Kagemusha operation-status JSON."""

from __future__ import annotations

import ctypes
import ctypes.util
from functools import lru_cache
from typing import Callable

_REQUIRED_BRIDGE_ABI_VERSION = 23
_MAX_OPERATION_STATUS_JSON_BYTES = 16 * 1024 * 1024
_ABI_VERSION_SYMBOL = "connect_norito_bridge_abi_version"
_STATUS_VALIDATOR_SYMBOL = "connect_norito_kagemusha_offline_operation_status_json_validate_v1"


@lru_cache(maxsize=1)
def _native_status_validator() -> Callable[..., int]:
    discovered = ctypes.util.find_library("connect_norito_bridge")
    candidates = tuple(
        dict.fromkeys(
            candidate
            for candidate in (
                discovered,
                "connect_norito_bridge",
                "libconnect_norito_bridge.dylib",
                "libconnect_norito_bridge.so",
                "connect_norito_bridge.dll",
            )
            if candidate
        )
    )
    for candidate in candidates:
        try:
            library = ctypes.CDLL(candidate)
            abi_version = getattr(library, _ABI_VERSION_SYMBOL)
            abi_version.argtypes = ()
            abi_version.restype = ctypes.c_uint32
            validator = getattr(library, _STATUS_VALIDATOR_SYMBOL)
            validator.argtypes = (
                ctypes.POINTER(ctypes.c_ubyte),
                ctypes.c_ulong,
            )
            validator.restype = ctypes.c_int32
            if abi_version() != _REQUIRED_BRIDGE_ABI_VERSION:
                continue
        except (AttributeError, OSError, TypeError):
            continue

        # The bound function retains its owning CDLL for the process lifetime.
        return validator

    raise RuntimeError(
        "ABI-23 connect_norito_bridge with the Kagemusha operation-status "
        "JSON validator is required"
    )


def validate_offline_operation_status_json_v1(status_json: bytes) -> None:
    """Require Rust's exact structural and anchor-digest validation."""

    if type(status_json) is not bytes:
        raise TypeError("status_json must be immutable bytes")
    if not 1 <= len(status_json) <= _MAX_OPERATION_STATUS_JSON_BYTES:
        raise ValueError(
            f"status_json must contain between 1 and {_MAX_OPERATION_STATUS_JSON_BYTES} bytes"
        )
    buffer = (ctypes.c_ubyte * len(status_json)).from_buffer_copy(status_json)
    status = _native_status_validator()(buffer, len(status_json))
    if status != 0:
        raise RuntimeError(
            f"native Kagemusha operation-status JSON validator failed closed (status {status})"
        )


__all__ = ["validate_offline_operation_status_json_v1"]
