# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Error hierarchy for the Norito Python codec."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional


class NoritoError(Exception):
    """Base class for all Norito codec errors."""


@dataclass(eq=False)
class IoError(NoritoError):
    """Represents an underlying IO error during streaming operations."""

    message: str

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        return f"IO error: {self.message}"


class InvalidMagicError(NoritoError):
    """Raised when the Norito header magic does not match `NRT0`."""


class UnsupportedVersionError(NoritoError):
    """Raised when encountering an unsupported Norito version tuple."""

    def __init__(self, major: int, minor: int) -> None:
        super().__init__(f"Unsupported Norito version: {major}.{minor}")
        self.major = major
        self.minor = minor


class UnsupportedCompressionError(NoritoError):
    """Raised when a payload specifies a compression mode we cannot handle."""

    def __init__(self, mode: int) -> None:
        super().__init__(f"Unsupported Norito compression byte: {mode}")
        self.mode = mode


class ChecksumMismatchError(NoritoError):
    """Raised when the CRC64 checksum does not match the payload."""

    def __init__(self, expected: int, actual: int) -> None:
        super().__init__(
            f"Checksum mismatch: expected 0x{expected:016x}, got 0x{actual:016x}"
        )
        self.expected = expected
        self.actual = actual


class LengthMismatchError(NoritoError):
    """Raised when the payload length declared in the header does not match."""

    def __init__(self, expected: int, actual: int) -> None:
        super().__init__(f"Length mismatch: expected {expected}, got {actual}")
        self.expected = expected
        self.actual = actual


class SchemaMismatchError(NoritoError):
    """Raised when the schema hash does not match an expected value."""

    def __init__(self, expected: bytes, actual: bytes) -> None:
        super().__init__(
            "Schema mismatch: expected "
            f"{expected.hex()}, got {actual.hex()}"
        )
        self.expected = expected
        self.actual = actual


class UnsupportedFeatureError(NoritoError):
    """Raised when a header flag is set but not supported by this library."""

    def __init__(self, flag: int) -> None:
        super().__init__(f"Unsupported Norito feature flag 0x{flag:02x}")
        self.flag = flag


class DecodeError(NoritoError):
    """Raised when the payload cannot be decoded into the requested shape."""

    def __init__(self, message: str, *, cause: Optional[BaseException] = None) -> None:
        super().__init__(message)
        self.cause = cause
        self.message = message

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        if self.cause is None:
            return self.message
        return f"{self.message} (caused by {self.cause!r})"
