"""Strict Mach-O decoder controls shared by the SoraFS runtime-input relation."""

from __future__ import annotations

from pathlib import Path
import struct
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import macho_decoder  # noqa: E402


def thin_macho(cpu_type: int) -> bytes:
    """Return a minimal little-endian 64-bit image with one dylib load."""

    name = b"@rpath/Fixture\0"
    command_size = (24 + len(name) + 7) & ~7
    command = (
        struct.pack("<6I", 0x0C, command_size, 24, 0, 0, 0)
        + name
        + bytes(command_size - 24 - len(name))
    )
    return struct.pack(
        "<8I", 0xFEEDFACF, cpu_type, 0, 2, 1, len(command), 0, 0,
    ) + command


def test_strict_macho_parser_accepts_thin_and_nonoverlapping_fat_images() -> None:
    arm = thin_macho(0x0100000C)
    x86 = thin_macho(0x01000007)
    table_size = 8 + 2 * 20
    arm_offset = table_size
    x86_offset = arm_offset + len(arm)
    assert arm_offset % 8 == 0 and x86_offset % 8 == 0
    fat = (
        struct.pack(">II", 0xCAFEBABE, 2)
        + struct.pack(">5I", 0x0100000C, 0, arm_offset, len(arm), 3)
        + struct.pack(">5I", 0x01000007, 0, x86_offset, len(x86), 3)
        + arm + x86
    )

    thin = macho_decoder.parse_macho(arm, "thin")
    universal = macho_decoder.parse_macho(fat, "fat")
    assert thin is not None and len(thin) == 1
    assert universal is not None
    assert [item["cpu_type"] for item in universal] == [
        0x0100000C, 0x01000007,
    ]
    assert universal[0]["commands"][0]["name"] == "@rpath/Fixture"


def test_strict_macho_parser_rejects_nonzero_fat64_reserved_field() -> None:
    thin = thin_macho(0x0100000C)
    offset = 40
    fat = (
        struct.pack(">II", 0xCAFEBABF, 1)
        + struct.pack(">IIQQII", 0x0100000C, 0, offset, len(thin), 3, 1)
        + thin
    )
    with pytest.raises(macho_decoder.MachOError, match="reserved field"):
        macho_decoder.parse_macho(fat, "fat64")
