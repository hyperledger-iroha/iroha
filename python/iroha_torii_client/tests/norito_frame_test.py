"""Header-flag admission tests for the strict Norito v1 frame helpers."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
if str(PACKAGE_ROOT) not in sys.path:
    sys.path.insert(0, str(PACKAGE_ROOT))

from iroha_torii_client.norito_frame import (  # noqa: E402
    encode_norito_frame,
    validate_opaque_norito_frame,
)

_ACCEPTED_FLAGS = (0x00, 0x02)


@pytest.mark.parametrize("flags", range(256))
def test_encode_accepts_only_fixed_width_and_compact_length_layouts(flags: int) -> None:
    if flags in _ACCEPTED_FLAGS:
        frame = encode_norito_frame(b"\x01", type_name="t", flags=flags)
        assert frame[39] == flags
    else:
        with pytest.raises(ValueError, match="unsupported Norito header flags"):
            encode_norito_frame(b"\x01", type_name="t", flags=flags)


@pytest.mark.parametrize("flags", range(256))
def test_validate_accepts_only_fixed_width_and_compact_length_layouts(flags: int) -> None:
    frame = bytearray(encode_norito_frame(b"\x01", type_name="t"))
    frame[39] = flags
    if flags in _ACCEPTED_FLAGS:
        validate_opaque_norito_frame(bytes(frame), context="frame")
    else:
        with pytest.raises(ValueError, match="unsupported Norito header flags"):
            validate_opaque_norito_frame(bytes(frame), context="frame")
