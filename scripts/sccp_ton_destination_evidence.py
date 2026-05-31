#!/usr/bin/env python3
"""Render SCCP TON destination rollout evidence.

This helper is offline by design. Operators pass the deployed TON verifier
contract address and verifier code hash to check the destination binding.
Production TOML additionally requires pinned source record hashes, the governed
route allowlist hash, and the expected destination binding hash collected from
independent governance or deployment records.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
from pathlib import Path
from typing import Iterable, NamedTuple


SCCP_DOMAIN_SORA = 0
SCCP_DOMAIN_TON = 4
SCCP_STARK_FRI_PROOF_FAMILY = "stark-fri-v1"
SCCP_DESTINATION_BINDING_PREFIX = b"sccp:destination:binding:v1"
SCCP_ROUTE_ALLOWLIST_LABEL = b"sccp:route-allowlist:lane-evidence:v1"
TON_ROUTE_CANARY_EVIDENCE_LABEL = b"iroha:sccp:ton-route-canary-live-account:v1"
TON_VERIFIER_BACKEND = "ton-contract-v1"
TON_VERIFIER_TARGET_CODE = 3
TON_VERIFIER_BACKEND_FAMILY_CODE = 3
TON_DESTINATION_ANCHOR_ID = "sccp:ton:destination-anchor:ton-mainnet:v1"
TON_ROUTE_ALLOWLIST_ID = "sccp:ton:route-allowlist:ton-mainnet:v1"
TON_BOC_MAGIC = bytes.fromhex("b5ee9c72")
TON_MAX_BOC_BYTES = 64 * 1024
TON_MAX_BOC_CELLS = 4096
TON_MAX_REFS = 4
TON_MAX_CELL_SERIALIZED_DATA_BYTES = 128
TON_CRC32C_REFLECTED_POLY = 0x82F63B78


class TonBocCell(NamedTuple):
    """Bounded TON BoC cell parsed from complete cell data."""

    descriptor: int
    data_descriptor: int
    data: bytes
    refs: tuple[int, ...]
    level: int
    exotic: bool


class TonBocPrunedBranch(NamedTuple):
    """Resolved hashes and depths carried by a TON pruned branch cell."""

    mask: int
    hashes: tuple[bytes, ...]
    depths: tuple[int, ...]


class TonBocComputedCell(NamedTuple):
    """Representation hashes and depths resolved at TON cell levels 0..3."""

    mask: int
    hashes: tuple[bytes, bytes, bytes, bytes]
    depths: tuple[int, int, int, int]


def _strip_0x(value: str) -> str:
    return value[2:] if value.lower().startswith("0x") else value


def parse_hex_bytes(
    value: str,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    """Parse a fixed-width hex value."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = _strip_0x(value)
    if len(text) != byte_length * 2:
        raise argparse.ArgumentTypeError(f"{label} must be {byte_length} bytes")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if nonzero and not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be zero")
    return raw


def parse_code_boc_hex(value: str, *, label: str) -> bytes:
    """Parse non-empty TON code BoC bytes from hex text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    text = _strip_0x(text)
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if len(text) % 2 != 0:
        raise argparse.ArgumentTypeError(f"{label} must have an even hex length")
    try:
        raw = bytes.fromhex(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"{label} must be hex") from exc
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    return raw


def parse_code_boc_base64(value: str, *, label: str) -> bytes:
    """Parse non-empty TON code BoC bytes from base64 or base64url text."""

    if value != value.strip() or any(symbol.isspace() for symbol in value):
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    if not text:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    try:
        raw = base64.b64decode(text, validate=True)
        if base64.b64encode(raw).decode("ascii") != text:
            raise argparse.ArgumentTypeError(f"{label} must be canonical base64")
    except binascii.Error:
        if any(symbol not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=" for symbol in text):
            raise argparse.ArgumentTypeError(
                f"{label} must be base64 or base64url"
            ) from None
        padded = text + ("=" * ((4 - len(text) % 4) % 4))
        try:
            raw = base64.urlsafe_b64decode(padded)
        except binascii.Error as exc:
            raise argparse.ArgumentTypeError(
                f"{label} must be base64 or base64url"
            ) from exc
        canonical_url = base64.urlsafe_b64encode(raw).decode("ascii")
        if text not in {canonical_url, canonical_url.rstrip("=")}:
            raise argparse.ArgumentTypeError(f"{label} must be canonical base64url")
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} must not be empty")
    if not any(raw):
        raise argparse.ArgumentTypeError(f"{label} must not be all zero")
    return raw


def parse_code_boc_file(value: str, *, label: str) -> bytes:
    """Parse non-empty TON code BoC bytes from a raw, hex, or base64 file."""

    path = Path(value).expanduser()
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise argparse.ArgumentTypeError(f"{label} file cannot be read") from exc
    if not raw:
        raise argparse.ArgumentTypeError(f"{label} file must not be empty")
    if raw.startswith(TON_BOC_MAGIC):
        return raw
    try:
        text = raw.decode("ascii").strip()
    except UnicodeDecodeError:
        return raw
    if not text:
        raise argparse.ArgumentTypeError(f"{label} file must not be empty")
    if text.lower().startswith("0x") or all(
        symbol in "0123456789abcdefABCDEF \t\r\n" for symbol in text
    ):
        return parse_code_boc_hex(text, label=label)
    return parse_code_boc_base64(text, label=label)


def parse_positive_decimal_text(value: str, *, label: str) -> str:
    """Parse a positive decimal value and preserve canonical text."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must be a positive decimal")
    text = value
    if (
        not text
        or not text.isascii()
        or not text.isdecimal()
        or (len(text) > 1 and text.startswith("0"))
        or int(text, 10) <= 0
    ):
        raise argparse.ArgumentTypeError(f"{label} must be a positive decimal")
    return text


def parse_account_status(value: str, *, label: str) -> str:
    """Parse a live TON verifier account status for production evidence."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    if value != "active":
        raise argparse.ArgumentTypeError(f"{label} must be active")
    return value


def _parse_canonical_i32_decimal(value: str, *, label: str) -> int:
    if not value or value.startswith("+"):
        raise argparse.ArgumentTypeError(f"{label} workchain must be canonical i32")
    digits = value[1:] if value.startswith("-") else value
    if not digits or (value.startswith("-") and digits == "0"):
        raise argparse.ArgumentTypeError(f"{label} workchain must be canonical i32")
    if len(digits) > 1 and digits.startswith("0"):
        raise argparse.ArgumentTypeError(f"{label} workchain must be canonical i32")
    if not digits.isascii() or not digits.isdecimal():
        raise argparse.ArgumentTypeError(f"{label} workchain must be canonical i32")
    try:
        parsed = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            f"{label} workchain must be canonical i32"
        ) from exc
    if parsed < -(2**31) or parsed > 2**31 - 1:
        raise argparse.ArgumentTypeError(f"{label} workchain must be canonical i32")
    return parsed


def normalize_ton_raw_address(value: str, *, label: str) -> str:
    """Validate a TON raw address and return its canonical text unchanged."""

    if value != value.strip():
        raise argparse.ArgumentTypeError(f"{label} must not contain whitespace")
    text = value
    parts = text.split(":")
    if len(parts) != 2:
        raise argparse.ArgumentTypeError(f"{label} must be workchain:account_hex")
    workchain, account_hex = parts
    workchain_id = _parse_canonical_i32_decimal(workchain, label=label)
    if workchain_id != 0:
        raise argparse.ArgumentTypeError(f"{label} workchain must be basechain 0")
    if len(account_hex) != 64:
        raise argparse.ArgumentTypeError(f"{label} account must be 32 bytes")
    if any(symbol not in "0123456789abcdef" for symbol in account_hex):
        raise argparse.ArgumentTypeError(
            f"{label} account must be lowercase canonical hex"
        )
    account = bytes.fromhex(account_hex)
    if not any(account):
        raise argparse.ArgumentTypeError(f"{label} account must not be zero")
    return text


def _require_fixed_bytes(
    value: bytes,
    *,
    label: str,
    byte_length: int,
    nonzero: bool = True,
) -> bytes:
    if not isinstance(value, (bytes, bytearray)):
        raise ValueError(f"{label} must be {byte_length} bytes")
    raw = bytes(value)
    if len(raw) != byte_length:
        raise ValueError(f"{label} must be {byte_length} bytes")
    if nonzero and not any(raw):
        raise ValueError(f"{label} must not be zero")
    return raw


def _require_ton_raw_address(value: str, *, label: str) -> str:
    try:
        return normalize_ton_raw_address(value, label=label)
    except argparse.ArgumentTypeError as exc:
        raise ValueError(str(exc)) from exc


def _require_destination_evidence(args: argparse.Namespace) -> None:
    args.verifier_contract_address = _require_ton_raw_address(
        args.verifier_contract_address,
        label="verifier_contract_address",
    )
    args.verifier_code_hash = _require_fixed_bytes(
        args.verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )


def _hex(value: bytes) -> str:
    return "0x" + value.hex()


def _push_u8(out: bytearray, value: int) -> None:
    out.append(value)


def _push_u32(out: bytearray, value: int) -> None:
    out.extend(value.to_bytes(4, "little", signed=False))


def _push_vec(out: bytearray, value: bytes) -> None:
    _push_u32(out, len(value))
    out.extend(value)


def _prefixed_blake2b(prefix: bytes, payload: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    hasher.update(prefix)
    hasher.update(payload)
    return hasher.digest()


def _read_sized_uint(data: bytes, offset: int, size: int) -> tuple[int, int]:
    if size < 1 or size > 8 or offset + size > len(data):
        raise ValueError("TON BoC sized integer is truncated")
    value = int.from_bytes(data[offset : offset + size], "big")
    return value, offset + size


def _crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ TON_CRC32C_REFLECTED_POLY
            else:
                crc >>= 1
            crc &= 0xFFFFFFFF
    return crc ^ 0xFFFFFFFF


def _cell_data_padding_is_valid(data_descriptor: int, data: bytes) -> bool:
    return (data_descriptor & 1) == 0 or bool(data and data[-1] != 0)


def _cell_serialized_bit_len_is_byte_aligned(
    data_descriptor: int,
    data: bytes,
) -> bool:
    return (data_descriptor & 1) == 0 and data_descriptor // 2 == len(data)


def _parse_boc_complete_ordinary(boc: bytes) -> tuple[list[int], list[TonBocCell]]:
    if (
        len(boc) < len(TON_BOC_MAGIC) + 2
        or len(boc) > TON_MAX_BOC_BYTES
        or not boc.startswith(TON_BOC_MAGIC)
    ):
        raise ValueError("TON code BoC header is invalid")
    offset = len(TON_BOC_MAGIC)
    flags_size = boc[offset]
    offset += 1
    has_index = (flags_size & 0x80) != 0
    has_crc32c = (flags_size & 0x40) != 0
    has_cache_bits = (flags_size & 0x20) != 0
    flags = (flags_size >> 3) & 0x03
    size_bytes = flags_size & 0x07
    offset_bytes = boc[offset]
    offset += 1
    if (
        has_cache_bits
        or flags != 0
        or size_bytes < 1
        or size_bytes > 4
        or offset_bytes < 1
        or offset_bytes > 8
    ):
        raise ValueError("TON code BoC header flags are unsupported")

    cells_count, offset = _read_sized_uint(boc, offset, size_bytes)
    roots_count, offset = _read_sized_uint(boc, offset, size_bytes)
    absent_count, offset = _read_sized_uint(boc, offset, size_bytes)
    total_cells_size, offset = _read_sized_uint(boc, offset, offset_bytes)
    if (
        cells_count <= 0
        or cells_count > TON_MAX_BOC_CELLS
        or roots_count <= 0
        or roots_count > cells_count
        or absent_count != 0
        or roots_count + absent_count > cells_count
    ):
        raise ValueError("TON code BoC counts are invalid")

    roots: list[int] = []
    for _ in range(roots_count):
        root, offset = _read_sized_uint(boc, offset, size_bytes)
        if root >= cells_count:
            raise ValueError("TON code BoC root index is invalid")
        roots.append(root)

    if has_index:
        previous = 0
        for index in range(cells_count):
            cell_offset, offset = _read_sized_uint(boc, offset, offset_bytes)
            if cell_offset < previous or cell_offset > total_cells_size:
                raise ValueError("TON code BoC index is invalid")
            if index + 1 == cells_count and cell_offset != total_cells_size:
                raise ValueError("TON code BoC index is invalid")
            previous = cell_offset

    if total_cells_size > len(boc) - offset:
        raise ValueError("TON code BoC cell data length is invalid")
    cell_data_end = offset + total_cells_size
    expected_end = cell_data_end + (4 if has_crc32c else 0)
    if expected_end != len(boc):
        raise ValueError("TON code BoC cell data length is invalid")
    if has_crc32c:
        expected_crc = _crc32c(boc[:cell_data_end]).to_bytes(4, "little")
        if boc[cell_data_end:expected_end] != expected_crc:
            raise ValueError("TON code BoC CRC32C is invalid")

    cell_data = boc[offset:cell_data_end]
    cell_offset = 0
    cells: list[TonBocCell] = []
    for cell_index in range(cells_count):
        if cell_offset + 2 > len(cell_data):
            raise ValueError("TON code BoC cell is truncated")
        descriptor = cell_data[cell_offset]
        data_descriptor = cell_data[cell_offset + 1]
        cell_offset += 2
        refs_count = descriptor & 0x07
        exotic = (descriptor & 0x08) != 0
        has_hashes = (descriptor & 0x10) != 0
        level = (descriptor >> 5) & 0x07
        data_bytes = (data_descriptor + 1) // 2
        if (
            refs_count > TON_MAX_REFS
            or has_hashes
            or data_bytes > TON_MAX_CELL_SERIALIZED_DATA_BYTES
            or cell_offset + data_bytes > len(cell_data)
        ):
            raise ValueError("TON code BoC cell descriptor is unsupported")
        data = cell_data[cell_offset : cell_offset + data_bytes]
        if not _cell_data_padding_is_valid(data_descriptor, data):
            raise ValueError("TON code BoC cell data padding is invalid")
        cell_offset += data_bytes
        refs: list[int] = []
        for _ in range(refs_count):
            ref_index, cell_offset = _read_sized_uint(
                cell_data,
                cell_offset,
                size_bytes,
            )
            if ref_index >= cells_count or ref_index <= cell_index:
                raise ValueError("TON code BoC cell refs must be forward internal refs")
            refs.append(ref_index)
        cells.append(
            TonBocCell(
                descriptor=descriptor & ~0x10,
                data_descriptor=data_descriptor,
                data=data,
                refs=tuple(refs),
                level=level,
                exotic=exotic,
            )
        )
    if cell_offset != len(cell_data):
        raise ValueError("TON code BoC has trailing cell data")
    return roots, cells


def _level_mask_value(mask: int) -> int:
    return mask & 0x07


def _level_mask_level(mask: int) -> int:
    value = _level_mask_value(mask)
    level = 0
    while value:
        level += 1
        value >>= 1
    return level


def _level_mask_hash_index(mask: int) -> int:
    value = _level_mask_value(mask)
    count = 0
    while value:
        count += value & 1
        value >>= 1
    return count


def _level_mask_apply(mask: int, level: int) -> int:
    if level == 0:
        return 0
    return _level_mask_value(mask) & ((1 << level) - 1)


def _level_mask_is_significant(mask: int, level: int) -> bool:
    return level == 0 or ((_level_mask_value(mask) >> (level - 1)) & 1) != 0


def _boc_cell_kind(cell: TonBocCell) -> str:
    if not cell.exotic:
        return "ordinary"
    kind = cell.data[0] if cell.data else -1
    if kind == 1:
        return "pruned_branch"
    if kind == 3:
        return "merkle_proof"
    if kind == 4:
        return "merkle_update"
    raise ValueError("TON code BoC exotic cell type is unsupported")


def _parse_pruned_branch(cell: TonBocCell) -> TonBocPrunedBranch:
    if (
        not _cell_serialized_bit_len_is_byte_aligned(
            cell.data_descriptor,
            cell.data,
        )
        or cell.refs
        or len(cell.data) < 2
        or cell.data[0] != 1
    ):
        raise ValueError("TON code BoC pruned branch cell is invalid")
    if len(cell.data) == 35:
        return TonBocPrunedBranch(
            mask=1,
            hashes=(cell.data[1:33],),
            depths=(int.from_bytes(cell.data[33:35], "big"),),
        )
    mask = _level_mask_value(cell.data[1])
    level = _level_mask_level(mask)
    if level < 1 or level > 3 or len(cell.data) != 2 + level * 34:
        raise ValueError("TON code BoC pruned branch cell is invalid")
    hashes = tuple(
        cell.data[2 + index * 32 : 2 + (index + 1) * 32]
        for index in range(level)
    )
    depths_start = 2 + level * 32
    depths = tuple(
        int.from_bytes(
            cell.data[depths_start + index * 2 : depths_start + (index + 1) * 2],
            "big",
        )
        for index in range(level)
    )
    return TonBocPrunedBranch(mask=mask, hashes=hashes, depths=depths)


def _child_hash_depth_for_level(
    computed: TonBocComputedCell,
    level: int,
) -> tuple[bytes, int]:
    index = min(level, 3)
    return computed.hashes[index], computed.depths[index]


def _boc_child_for_hash_level(
    kind: str,
    computed: TonBocComputedCell,
    level: int,
) -> tuple[bytes, int]:
    child_level = level + 1 if kind in ("merkle_proof", "merkle_update") else level
    return _child_hash_depth_for_level(computed, child_level)


def _boc_cell_hashes(cells: list[TonBocCell]) -> list[TonBocComputedCell]:
    zero_hashes: tuple[bytes, bytes, bytes, bytes] = (bytes(32),) * 4
    zero_depths = (0, 0, 0, 0)
    computed = [
        TonBocComputedCell(mask=0, hashes=zero_hashes, depths=zero_depths)
        for _ in cells
    ]
    for index in range(len(cells) - 1, -1, -1):
        cell = cells[index]
        kind = _boc_cell_kind(cell)
        pruned = _parse_pruned_branch(cell) if kind == "pruned_branch" else None
        if kind == "ordinary":
            mask = 0
            for ref in cell.refs:
                if ref < 0 or ref >= len(computed):
                    raise ValueError("TON code BoC cell refs are invalid")
                mask |= computed[ref].mask
        elif kind == "pruned_branch":
            assert pruned is not None
            mask = pruned.mask
        elif kind == "merkle_proof":
            if (
                not _cell_serialized_bit_len_is_byte_aligned(
                    cell.data_descriptor,
                    cell.data,
                )
                or len(cell.data) != 35
                or len(cell.refs) != 1
            ):
                raise ValueError("TON code BoC Merkle proof cell is invalid")
            child_hash, child_depth = _child_hash_depth_for_level(
                computed[cell.refs[0]],
                0,
            )
            proof_hash = cell.data[1:33]
            proof_depth = int.from_bytes(cell.data[33:35], "big")
            if proof_hash != child_hash or proof_depth != child_depth:
                raise ValueError("TON code BoC Merkle proof cell is invalid")
            mask = _level_mask_value(computed[cell.refs[0]].mask >> 1)
        elif kind == "merkle_update":
            if (
                not _cell_serialized_bit_len_is_byte_aligned(
                    cell.data_descriptor,
                    cell.data,
                )
                or len(cell.data) != 69
                or len(cell.refs) != 2
            ):
                raise ValueError("TON code BoC Merkle update cell is invalid")
            for ref_pos, hash_offset, depth_offset in (
                (0, 1, 65),
                (1, 33, 67),
            ):
                child_hash, child_depth = _child_hash_depth_for_level(
                    computed[cell.refs[ref_pos]],
                    0,
                )
                proof_hash = cell.data[hash_offset : hash_offset + 32]
                proof_depth = int.from_bytes(
                    cell.data[depth_offset : depth_offset + 2],
                    "big",
                )
                if proof_hash != child_hash or proof_depth != child_depth:
                    raise ValueError("TON code BoC Merkle update cell is invalid")
            mask = _level_mask_value(
                (computed[cell.refs[0]].mask | computed[cell.refs[1]].mask) >> 1
            )
        else:
            raise ValueError("TON code BoC cell kind is unsupported")

        if cell.level != mask:
            raise ValueError("TON code BoC cell level mask is invalid")

        total_hash_count = _level_mask_hash_index(mask) + 1
        hash_count = 1 if kind == "pruned_branch" else total_hash_count
        hash_offset = total_hash_count - hash_count
        computed_hashes: list[bytes] = []
        computed_depths: list[int] = []
        hash_index = 0
        for level_index in range(_level_mask_level(mask) + 1):
            if not _level_mask_is_significant(mask, level_index):
                continue
            if hash_index < hash_offset:
                hash_index += 1
                continue
            if hash_index == hash_offset:
                if level_index != 0 and kind != "pruned_branch":
                    raise ValueError("TON code BoC cell hash level is invalid")
                current_data = cell.data
            else:
                current_data = computed_hashes[hash_index - hash_offset - 1]

            current_depth = 0
            for ref in cell.refs:
                _, child_depth = _boc_child_for_hash_level(
                    kind,
                    computed[ref],
                    level_index,
                )
                current_depth = max(current_depth, child_depth)
            if cell.refs:
                current_depth += 1
            if current_depth > 0xFFFF:
                raise ValueError("TON code BoC cell depth is invalid")

            applied_mask = _level_mask_apply(mask, level_index)
            descriptor = (
                len(cell.refs)
                | (0 if kind == "ordinary" else 0x08)
                | (applied_mask << 5)
            )
            representation = bytearray([descriptor, cell.data_descriptor])
            representation.extend(current_data)
            for ref in cell.refs:
                _, child_depth = _boc_child_for_hash_level(
                    kind,
                    computed[ref],
                    level_index,
                )
                representation.extend(child_depth.to_bytes(2, "big"))
            for ref in cell.refs:
                child_hash, _ = _boc_child_for_hash_level(
                    kind,
                    computed[ref],
                    level_index,
                )
                representation.extend(child_hash)
            computed_hashes.append(hashlib.sha256(representation).digest())
            computed_depths.append(current_depth)
            hash_index += 1

        resolved_hashes: list[bytes] = [bytes(32) for _ in range(4)]
        resolved_depths = [0, 0, 0, 0]
        for resolved_level in range(4):
            resolved_hash_index = _level_mask_hash_index(
                _level_mask_apply(mask, resolved_level)
            )
            if pruned is not None:
                this_hash_index = _level_mask_hash_index(mask)
                if resolved_hash_index != this_hash_index:
                    resolved_hashes[resolved_level] = pruned.hashes[
                        resolved_hash_index
                    ]
                    resolved_depths[resolved_level] = pruned.depths[
                        resolved_hash_index
                    ]
                else:
                    resolved_hashes[resolved_level] = computed_hashes[0]
                    resolved_depths[resolved_level] = computed_depths[0]
            else:
                resolved_hashes[resolved_level] = computed_hashes[
                    resolved_hash_index
                ]
                resolved_depths[resolved_level] = computed_depths[
                    resolved_hash_index
                ]
        computed[index] = TonBocComputedCell(
            mask=mask,
            hashes=(
                resolved_hashes[0],
                resolved_hashes[1],
                resolved_hashes[2],
                resolved_hashes[3],
            ),
            depths=(
                resolved_depths[0],
                resolved_depths[1],
                resolved_depths[2],
                resolved_depths[3],
            ),
        )
    return computed


def ton_boc_root_hashes(boc: bytes) -> list[bytes]:
    """Return bounded complete TON BoC root representation hashes."""

    if not isinstance(boc, (bytes, bytearray)):
        raise ValueError("TON code BoC must be bytes")
    roots, cells = _parse_boc_complete_ordinary(bytes(boc))
    computed = _boc_cell_hashes(cells)
    hashes: list[bytes] = []
    for root in roots:
        if root < 0 or root >= len(computed):
            raise ValueError("TON code BoC root index is invalid")
        hashes.append(computed[root].hashes[3])
    return hashes


def ton_boc_single_root_hash(boc: bytes) -> bytes:
    """Return the single root representation hash for a bounded TON BoC."""

    hashes = ton_boc_root_hashes(boc)
    if len(hashes) != 1:
        raise ValueError("TON code BoC must contain exactly one root")
    return hashes[0]


def apply_verifier_code_boc_hash(args: argparse.Namespace) -> None:
    """Fill or verify the TON verifier code hash from a deployed code BoC."""

    boc_sources = [
        ("--verifier-code-boc-hex", getattr(args, "verifier_code_boc_hex", None)),
        (
            "--verifier-code-boc-base64",
            getattr(args, "verifier_code_boc_base64", None),
        ),
        ("--verifier-code-boc-file", getattr(args, "verifier_code_boc_file", None)),
    ]
    supplied = [(flag, value) for flag, value in boc_sources if value is not None]
    if len(supplied) > 1:
        flags = ", ".join(flag for flag, _ in supplied)
        raise ValueError(f"{flags} cannot be supplied together")
    if not supplied:
        if getattr(args, "verifier_code_hash", None) is None:
            raise ValueError(
                "--verifier-code-hash, --verifier-code-boc-hex, "
                "--verifier-code-boc-base64, or --verifier-code-boc-file is required"
            )
        return

    _, code_boc = supplied[0]
    code_boc = bytes(code_boc)
    derived_hash = ton_boc_single_root_hash(code_boc)
    verifier_code_hash = getattr(args, "verifier_code_hash", None)
    if verifier_code_hash is not None and verifier_code_hash != derived_hash:
        raise ValueError(
            "--verifier-code-hash does not match TON verifier code BoC root hash: "
            f"expected {_hex(verifier_code_hash)}, got {_hex(derived_hash)}"
        )
    code_boc_root_hash = getattr(args, "verifier_code_boc_root_hash", None)
    if code_boc_root_hash is not None and code_boc_root_hash != derived_hash:
        raise ValueError(
            "verifier code BoC root evidence does not match TON verifier code BoC "
            f"root hash: expected {_hex(code_boc_root_hash)}, got {_hex(derived_hash)}"
        )
    args.verifier_code_hash = derived_hash
    args.verifier_code_boc_bytes = code_boc
    args.verifier_code_boc_base64_text = base64.b64encode(code_boc).decode("ascii")
    args.verifier_code_boc_root_hash = derived_hash
    args.verifier_code_boc_hash_matches = True


def ton_destination_binding_key() -> str:
    """Return Rust's canonical SORA -> TON destination binding key."""

    return (
        f"sccp:{SCCP_DOMAIN_SORA}:{SCCP_DOMAIN_TON}:ton:"
        f"{TON_VERIFIER_BACKEND}:{TON_VERIFIER_TARGET_CODE}"
    )


def ton_destination_binding_hash() -> bytes:
    """Compute Rust's canonical SORA -> TON destination binding hash."""

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, SCCP_DOMAIN_TON)
    _push_u8(payload, 1)  # RecursiveZk
    _push_u8(payload, 1)  # CryptographicProof
    _push_u8(payload, TON_VERIFIER_TARGET_CODE)
    _push_u8(payload, TON_VERIFIER_BACKEND_FAMILY_CODE)
    _push_vec(payload, ton_destination_binding_key().encode("utf-8"))
    _push_vec(
        payload,
        b"iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
    )
    _push_vec(payload, SCCP_STARK_FRI_PROOF_FAMILY.encode("utf-8"))
    _push_vec(payload, TON_VERIFIER_BACKEND.encode("utf-8"))
    return _prefixed_blake2b(SCCP_DESTINATION_BINDING_PREFIX, bytes(payload))


def ton_route_allowlist_hash(
    *,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes:
    """Compute Rust's canonical SORA -> TON route allowlist hash."""

    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_TON)
    _push_vec(payload, b"ton")
    _push_vec(payload, b"GovernanceAllowlist")
    _push_vec(payload, TON_ROUTE_ALLOWLIST_ID.encode("utf-8"))
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    payload.extend(destination_binding_hash)
    return _prefixed_blake2b(SCCP_ROUTE_ALLOWLIST_LABEL, payload)


def ton_route_canary_evidence_hash(
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
    source_verifier_material_hash: bytes,
    source_adapter_engine_deployment_hash: bytes,
    verifier_contract_address: str,
    verifier_code_hash: bytes,
    account_status: str,
    account_state_hash: bytes,
    last_transaction_lt: str,
    last_transaction_hash: bytes,
    verifier_code_boc_root_hash: bytes,
) -> bytes:
    """Compute Rust's canonical TON route canary live-account evidence hash."""

    route_allowlist_hash = _require_fixed_bytes(
        route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    destination_binding_hash = _require_fixed_bytes(
        destination_binding_hash,
        label="destination_binding_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        source_verifier_material_hash,
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        source_adapter_engine_deployment_hash,
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    verifier_contract_address = normalize_ton_raw_address(
        verifier_contract_address,
        label="verifier_contract_address",
    )
    verifier_code_hash = _require_fixed_bytes(
        verifier_code_hash,
        label="verifier_code_hash",
        byte_length=32,
    )
    if not isinstance(account_status, str):
        raise ValueError("account_status must be active")
    try:
        account_status = parse_account_status(
            account_status,
            label="account_status",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError("account_status must be active") from exc
    account_state_hash = _require_fixed_bytes(
        account_state_hash,
        label="account_state_hash",
        byte_length=32,
    )
    if not isinstance(last_transaction_lt, str):
        raise ValueError("last_transaction_lt must be a positive decimal")
    try:
        last_transaction_lt = parse_positive_decimal_text(
            last_transaction_lt,
            label="last_transaction_lt",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError("last_transaction_lt must be a positive decimal") from exc
    last_transaction_hash = _require_fixed_bytes(
        last_transaction_hash,
        label="last_transaction_hash",
        byte_length=32,
    )
    if last_transaction_hash == account_state_hash:
        raise ValueError(
            "last_transaction_hash must differ from account_state_hash"
        )
    verifier_code_boc_root_hash = _require_fixed_bytes(
        verifier_code_boc_root_hash,
        label="verifier_code_boc_root_hash",
        byte_length=32,
    )
    if verifier_code_boc_root_hash != verifier_code_hash:
        raise ValueError("verifier_code_boc_root_hash must match verifier_code_hash")

    payload = bytearray()
    _push_u8(payload, 1)
    _push_u32(payload, SCCP_DOMAIN_SORA)
    _push_u32(payload, SCCP_DOMAIN_TON)
    payload.extend(route_allowlist_hash)
    payload.extend(destination_binding_hash)
    payload.extend(source_verifier_material_hash)
    payload.extend(source_adapter_engine_deployment_hash)
    _push_vec(payload, verifier_contract_address.encode("utf-8"))
    payload.extend(verifier_code_hash)
    _push_vec(payload, account_status.encode("ascii"))
    payload.extend(account_state_hash)
    _push_vec(payload, last_transaction_lt.encode("ascii"))
    payload.extend(last_transaction_hash)
    payload.extend(verifier_code_boc_root_hash)
    return _prefixed_blake2b(TON_ROUTE_CANARY_EVIDENCE_LABEL, payload)


def _toml_string(value: str) -> str:
    return json.dumps(value)


def _toml_line(key: str, value: object) -> str:
    if isinstance(value, bool):
        rendered = "true" if value else "false"
    elif isinstance(value, int):
        rendered = str(value)
    elif isinstance(value, str):
        rendered = _toml_string(value)
    elif isinstance(value, list) and all(isinstance(item, str) for item in value):
        rendered = "[" + ", ".join(_toml_string(item) for item in value) + "]"
    else:
        raise TypeError(f"unsupported TOML value for {key}")
    return f"{key} = {rendered}"


def _destination_rollout_lines(args: argparse.Namespace) -> Iterable[str]:
    yield "[[zk.sccp_destination_rollouts]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", SCCP_DOMAIN_TON)
    yield _toml_line("chain", "ton")
    yield _toml_line("verifier_plan", "TonContractNativeRecursive")
    yield _toml_line("immutable_verifier_ready", True)
    yield _toml_line("anchors_ready", True)
    yield _toml_line("verifier_identity", args.verifier_contract_address)
    yield _toml_line("verifier_code_hash", _hex(args.verifier_code_hash))
    yield _toml_line("destination_binding_key", ton_destination_binding_key())
    yield _toml_line("destination_binding_hash", _hex(ton_destination_binding_hash()))
    yield _toml_line("anchor_id", TON_DESTINATION_ANCHOR_ID)
    yield _toml_line("ton_account_status", args.account_status)
    yield _toml_line("ton_account_state_hash", _hex(args.account_state_hash))
    yield _toml_line("ton_last_transaction_lt", str(args.last_transaction_lt))
    yield _toml_line("ton_last_transaction_hash", _hex(args.last_transaction_hash))
    yield _toml_line(
        "ton_verifier_code_boc_root_hash",
        _hex(args.verifier_code_boc_root_hash),
    )
    yield _toml_line(
        "ton_verifier_code_boc",
        "0x" + bytes(args.verifier_code_boc_bytes).hex(),
    )
    yield _toml_line("blockers", [])


def _route_allowlist_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> Iterable[str]:
    supplied_route_allowlist_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    if supplied_route_allowlist_hash != route_allowlist_hash:
        raise ValueError("route_allowlist_hash does not match validated lane evidence")
    yield "[[zk.sccp_route_allowlists]]"
    yield _toml_line("version", 1)
    yield _toml_line("domain", SCCP_DOMAIN_TON)
    yield _toml_line("chain", "ton")
    yield _toml_line("activation_policy", "GovernanceAllowlist")
    yield _toml_line("route_allowlist_id", TON_ROUTE_ALLOWLIST_ID)
    yield _toml_line("route_allowlist_hash", _hex(route_allowlist_hash))
    yield from _route_canary_toml_lines(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    yield _toml_line("routes_allowlisted", True)
    yield _toml_line("blockers", [])


def _route_canary_toml_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        _toml_line("route_canary_status", "passed"),
        _toml_line("route_canary_evidence_hash", _hex(canary_hash)),
        _toml_line("route_canary_route_allowlist_hash", _hex(route_allowlist_hash)),
        _toml_line(
            "route_canary_destination_binding_hash",
            _hex(destination_binding_hash),
        ),
        _toml_line("ton_route_canary_account_state_hash", _hex(args.account_state_hash)),
        _toml_line(
            "ton_route_canary_last_transaction_lt",
            str(args.last_transaction_lt),
        ),
        _toml_line(
            "ton_route_canary_last_transaction_hash",
            _hex(args.last_transaction_hash),
        ),
    ]


def _route_canary_comment_lines(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> list[str]:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return []
    return [
        "# sccp_route_canary_status = " + json.dumps("passed"),
        "# sccp_route_canary_evidence_hash = " + json.dumps(_hex(canary_hash)),
        "# sccp_route_canary_route_allowlist_hash = "
        + json.dumps(_hex(route_allowlist_hash)),
        "# sccp_route_canary_destination_binding_hash = "
        + json.dumps(_hex(destination_binding_hash)),
        "# sccp_ton_route_canary_account_state_hash = "
        + json.dumps(_hex(args.account_state_hash)),
        "# sccp_ton_route_canary_last_transaction_lt = "
        + json.dumps(str(args.last_transaction_lt)),
        "# sccp_ton_route_canary_last_transaction_hash = "
        + json.dumps(_hex(args.last_transaction_hash)),
    ]


def _route_canary_summary(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> dict[str, object] | None:
    canary_hash = _route_canary_evidence_hash(
        args,
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
    )
    if canary_hash is None:
        return None
    return {
        "status": "passed",
        "evidence_hash": _hex(canary_hash),
        "route_allowlist_hash": _hex(route_allowlist_hash),
        "destination_binding_hash": _hex(destination_binding_hash),
        "ton_account_state_hash": _hex(args.account_state_hash),
        "ton_last_transaction_lt": str(args.last_transaction_lt),
        "ton_last_transaction_hash": _hex(args.last_transaction_hash),
        "evidence_bound": True,
    }


def _route_canary_evidence_hash(
    args: argparse.Namespace,
    *,
    route_allowlist_hash: bytes,
    destination_binding_hash: bytes,
) -> bytes | None:
    canary_hash = getattr(args, "route_canary_evidence_hash", None)
    if canary_hash is None:
        return None
    canary_hash = _require_fixed_bytes(
        canary_hash,
        label="route_canary_evidence_hash",
        byte_length=32,
    )
    source_verifier_material_hash = _require_fixed_bytes(
        getattr(args, "source_verifier_material_hash", None),
        label="source_verifier_material_hash",
        byte_length=32,
    )
    source_adapter_engine_deployment_hash = _require_fixed_bytes(
        getattr(args, "source_adapter_engine_deployment_hash", None),
        label="source_adapter_engine_deployment_hash",
        byte_length=32,
    )
    if canary_hash in (
        route_allowlist_hash,
        destination_binding_hash,
        source_verifier_material_hash,
        source_adapter_engine_deployment_hash,
    ):
        raise ValueError(
            "route_canary_evidence_hash must be distinct from route_allowlist_hash, "
            "destination_binding_hash, source_verifier_material_hash, and "
            "source_adapter_engine_deployment_hash"
        )
    derived_canary_hash = ton_route_canary_evidence_hash(
        route_allowlist_hash=route_allowlist_hash,
        destination_binding_hash=destination_binding_hash,
        source_verifier_material_hash=source_verifier_material_hash,
        source_adapter_engine_deployment_hash=source_adapter_engine_deployment_hash,
        verifier_contract_address=args.verifier_contract_address,
        verifier_code_hash=args.verifier_code_hash,
        account_status=getattr(args, "account_status", None),
        account_state_hash=getattr(args, "account_state_hash", None),
        last_transaction_lt=getattr(args, "last_transaction_lt", None),
        last_transaction_hash=getattr(args, "last_transaction_hash", None),
        verifier_code_boc_root_hash=getattr(args, "verifier_code_boc_root_hash", None),
    )
    if canary_hash != derived_canary_hash:
        raise ValueError(
            "route_canary_evidence_hash must match TON live account route canary evidence: "
            f"expected {_hex(derived_canary_hash)}, got {_hex(canary_hash)}"
        )
    return canary_hash


def _route_allowlist_hash_from_args(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    return ton_route_allowlist_hash(
        source_verifier_material_hash=getattr(
            args,
            "source_verifier_material_hash",
            None,
        ),
        source_adapter_engine_deployment_hash=(
            getattr(args, "source_adapter_engine_deployment_hash", None)
        ),
        destination_binding_hash=destination_binding_hash,
    )


def _require_expected_route_allowlist_hash(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
) -> bytes:
    supplied_hash = _require_fixed_bytes(
        args.route_allowlist_hash,
        label="route_allowlist_hash",
        byte_length=32,
    )
    expected_hash = _route_allowlist_hash_from_args(args, destination_binding_hash)
    if supplied_hash != expected_hash:
        raise ValueError(
            "--route-allowlist-hash does not match canonical source, deployment, "
            "and destination evidence: "
            f"expected {_hex(expected_hash)}, got {_hex(supplied_hash)}"
        )
    return expected_hash


def _require_toml_account_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    account_status = getattr(args, "account_status", None)
    if account_status != "active":
        raise ValueError(f"--{output} requires --account-status active")
    account_state_hash = getattr(args, "account_state_hash", None)
    if account_state_hash is None:
        raise ValueError(f"--{output} requires --account-state-hash")
    _require_fixed_bytes(
        account_state_hash,
        label="account_state_hash",
        byte_length=32,
    )
    last_transaction_hash = getattr(args, "last_transaction_hash", None)
    if last_transaction_hash is None:
        raise ValueError(f"--{output} requires --last-transaction-hash")
    _require_fixed_bytes(
        last_transaction_hash,
        label="last_transaction_hash",
        byte_length=32,
    )
    last_transaction_lt = getattr(args, "last_transaction_lt", None)
    if not isinstance(last_transaction_lt, str):
        raise ValueError(f"--{output} requires --last-transaction-lt")
    try:
        canonical_last_transaction_lt = parse_positive_decimal_text(
            last_transaction_lt,
            label="last transaction LT",
        )
    except argparse.ArgumentTypeError as exc:
        raise ValueError(f"--{output} requires --last-transaction-lt") from exc
    if canonical_last_transaction_lt != last_transaction_lt:
        raise ValueError(f"--{output} requires --last-transaction-lt")


def _toml_account_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_toml_account_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _require_code_boc_root_metadata(
    args: argparse.Namespace,
    *,
    output: str,
) -> None:
    code_boc_root_hash = getattr(args, "verifier_code_boc_root_hash", None)
    if code_boc_root_hash is None:
        raise ValueError(
            f"--{output} requires verifier code BoC root evidence "
            "(use --verifier-code-boc-hex, --verifier-code-boc-base64, "
            "or --verifier-code-boc-file)"
        )
    code_boc_root_hash = _require_fixed_bytes(
        code_boc_root_hash,
        label="verifier_code_boc_root_hash",
        byte_length=32,
    )
    if code_boc_root_hash != args.verifier_code_hash:
        raise ValueError(
            "verifier code BoC root hash must match verifier_code_hash: "
            f"expected {_hex(args.verifier_code_hash)}, got {_hex(code_boc_root_hash)}"
        )
    if getattr(args, "verifier_code_boc_hash_matches", None) is not True:
        raise ValueError(f"--{output} requires verifier code BoC hash match evidence")
    code_boc = getattr(args, "verifier_code_boc_bytes", None)
    if not isinstance(code_boc, (bytes, bytearray)):
        code_boc_base64 = getattr(args, "verifier_code_boc_base64_text", None)
        if isinstance(code_boc_base64, str) and code_boc_base64.strip():
            try:
                code_boc = parse_code_boc_base64(
                    code_boc_base64,
                    label="verifier_code_boc_base64",
                )
            except argparse.ArgumentTypeError as exc:
                raise ValueError(
                    f"--{output} has invalid verifier code BoC base64 evidence: {exc}"
                ) from exc
            args.verifier_code_boc_bytes = code_boc
            args.verifier_code_boc_base64_text = base64.b64encode(
                code_boc
            ).decode("ascii")
    if not isinstance(code_boc, (bytes, bytearray)):
        raise ValueError(
            f"--{output} requires verifier code BoC byte evidence "
            "(use --verifier-code-boc-hex, --verifier-code-boc-base64, "
            "or --verifier-code-boc-file)"
        )
    derived_hash = ton_boc_single_root_hash(bytes(code_boc))
    if derived_hash != code_boc_root_hash:
        raise ValueError(
            "verifier code BoC bytes must hash to verifier_code_boc_root_hash: "
            f"expected {_hex(code_boc_root_hash)}, got {_hex(derived_hash)}"
        )


def _code_boc_root_metadata_ready(args: argparse.Namespace) -> bool:
    try:
        _require_code_boc_root_metadata(args, output="toml")
    except ValueError:
        return False
    return True


def _missing_route_allowlist_args(args: argparse.Namespace) -> list[str]:
    return [
        name
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
        )
        if getattr(args, name, None) is None
    ]


def render_toml(
    args: argparse.Namespace,
    destination_binding_hash: bytes | None = None,
) -> str:
    """Render production TON destination rollout and route allowlist TOML."""

    apply_verifier_code_boc_hash(args)
    _require_destination_evidence(args)
    expected_hash = ton_destination_binding_hash()
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is None:
        raise ValueError(
            "--expected-destination-binding-hash is required before rendering production TOML"
        )
    if expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> TON binding: expected {_hex(expected_pin)}, "
            f"got {_hex(expected_hash)}"
        )
    if destination_binding_hash is None:
        destination_binding_hash = expected_pin
    elif destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> TON binding: expected {_hex(expected_hash)}, "
            f"got {_hex(destination_binding_hash)}"
        )
    missing_route_args = _missing_route_allowlist_args(args)
    if missing_route_args:
        formatted = ", ".join(f"--{name.replace('_', '-')}" for name in missing_route_args)
        raise ValueError(f"--toml requires {formatted}")
    route_allowlist_hash = _require_expected_route_allowlist_hash(
        args,
        destination_binding_hash,
    )
    if getattr(args, "route_canary_evidence_hash", None) is None:
        raise ValueError("--toml requires --route-canary-evidence-hash")
    _require_toml_account_metadata(args, output="toml")
    _require_code_boc_root_metadata(args, output="toml")
    return "\n".join(
        [
            "# sccp_ton_account_status = " + json.dumps(args.account_status),
            "# sccp_ton_account_state_hash = "
            + json.dumps(_hex(args.account_state_hash)),
            "# sccp_ton_last_transaction_lt = "
            + json.dumps(str(args.last_transaction_lt)),
            "# sccp_ton_last_transaction_hash = "
            + json.dumps(_hex(args.last_transaction_hash)),
            "# sccp_ton_code_hash = " + json.dumps(_hex(args.verifier_code_hash)),
            "# sccp_ton_code_boc_root_hash = "
            + json.dumps(_hex(args.verifier_code_boc_root_hash)),
            "# sccp_ton_code_boc_base64 = "
            + json.dumps(args.verifier_code_boc_base64_text),
            "# sccp_ton_code_boc_hash_matches = "
            + json.dumps("true"),
            "# sccp_ton_destination_binding_hash = "
            + json.dumps(_hex(destination_binding_hash)),
            "# sccp_ton_route_allowlist_hash = "
            + json.dumps(_hex(route_allowlist_hash)),
            *_destination_rollout_lines(args),
            "",
            *_route_canary_comment_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            *_route_allowlist_lines(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
            ),
            "",
        ]
    )


def _json_summary(
    args: argparse.Namespace,
    destination_binding_hash: bytes,
    expected_matches: bool,
) -> dict[str, object]:
    apply_verifier_code_boc_hash(args)
    _require_destination_evidence(args)
    expected_hash = ton_destination_binding_hash()
    if destination_binding_hash != expected_hash:
        raise ValueError(
            "destination_binding_hash must match the canonical "
            f"SORA -> TON binding: expected {_hex(expected_hash)}, "
            f"got {_hex(destination_binding_hash)}"
        )
    expected_pin = getattr(args, "expected_destination_binding_hash", None)
    if expected_pin is not None and expected_pin != expected_hash:
        raise ValueError(
            "expected destination binding hash does not match the canonical "
            f"SORA -> TON binding: expected {_hex(expected_pin)}, "
            f"got {_hex(expected_hash)}"
        )
    route_requested = any(
        getattr(args, name, None) is not None
        for name in (
            "route_allowlist_hash",
            "source_verifier_material_hash",
            "source_adapter_engine_deployment_hash",
            "route_canary_evidence_hash",
        )
    )
    summary = {
        "source_domain": SCCP_DOMAIN_SORA,
        "domain": SCCP_DOMAIN_TON,
        "chain": "ton",
        "verifier_plan": "TonContractNativeRecursive",
        "verifier_identity": args.verifier_contract_address,
        "verifier_code_hash": _hex(args.verifier_code_hash),
        "anchor_id": TON_DESTINATION_ANCHOR_ID,
        "destination_binding_key": ton_destination_binding_key(),
        "destination_binding_hash": _hex(destination_binding_hash),
        "expected_destination_binding_hash_matches": expected_matches,
        "toml_ready": False,
    }
    code_boc_root_hash = getattr(args, "verifier_code_boc_root_hash", None)
    if code_boc_root_hash is not None:
        summary["code_boc_root_hash"] = _hex(code_boc_root_hash)
        summary["code_boc_hash_matches"] = (
            getattr(args, "verifier_code_boc_hash_matches", None) is True
            and code_boc_root_hash == args.verifier_code_hash
        )
    account_status = getattr(args, "account_status", None)
    if account_status is not None:
        summary["account_status"] = account_status
    code_boc_base64 = getattr(args, "verifier_code_boc_base64_text", None)
    if isinstance(code_boc_base64, str) and code_boc_base64.strip():
        summary["code_boc_base64"] = code_boc_base64
        summary["code_boc_base64_sha256"] = hashlib.sha256(
            code_boc_base64.encode("ascii")
        ).hexdigest()
    if route_requested:
        if expected_pin is None:
            raise ValueError(
                "--route-allowlist-hash requires "
                "--expected-destination-binding-hash"
            )
        missing_route_args = _missing_route_allowlist_args(args)
        if missing_route_args:
            formatted = ", ".join(
                f"--{name.replace('_', '-')}" for name in missing_route_args
            )
            raise ValueError("route allowlist evidence requires " + formatted)
        route_allowlist_hash = _require_fixed_bytes(
            args.route_allowlist_hash,
            label="route_allowlist_hash",
            byte_length=32,
        )
        expected_route_allowlist_hash = _require_expected_route_allowlist_hash(
            args,
            destination_binding_hash,
        )
        summary.update(
            {
                "source_verifier_material_hash": _hex(
                    args.source_verifier_material_hash
                ),
                "source_adapter_engine_deployment_hash": _hex(
                    args.source_adapter_engine_deployment_hash
                ),
                "route_allowlist_id": TON_ROUTE_ALLOWLIST_ID,
                "route_allowlist_hash": _hex(route_allowlist_hash),
                "expected_route_allowlist_hash": _hex(
                    expected_route_allowlist_hash
                ),
                "expected_route_allowlist_hash_matches": True,
                "toml_ready": (
                    expected_matches and _toml_account_metadata_ready(args)
                    and _code_boc_root_metadata_ready(args)
                ),
            }
        )
        route_canary = _route_canary_summary(
            args,
            route_allowlist_hash=route_allowlist_hash,
            destination_binding_hash=destination_binding_hash,
        )
        if route_canary is not None:
            summary["route_canary"] = route_canary
            summary["toml_ready"] = bool(summary["toml_ready"])
        else:
            summary["toml_ready"] = False
    return summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Render SCCP TON destination rollout evidence.",
    )
    parser.add_argument(
        "--verifier-contract-address",
        required=True,
        type=lambda value: normalize_ton_raw_address(
            value,
            label="verifier contract address",
        ),
        help="Deployed TON verifier contract address as workchain:account_hex.",
    )
    parser.add_argument(
        "--verifier-code-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="verifier code hash",
            byte_length=32,
        ),
        help=(
            "Non-zero deployed TON verifier contract code hash. Required unless "
            "a verifier code BoC is supplied."
        ),
    )
    parser.add_argument(
        "--verifier-code-boc-hex",
        type=lambda value: parse_code_boc_hex(
            value,
            label="verifier code BoC",
        ),
        help=(
            "Deployed TON verifier code BoC as hex. When supplied, the helper "
            "derives verifier_code_hash from the single root representation hash."
        ),
    )
    parser.add_argument(
        "--verifier-code-boc-base64",
        type=lambda value: parse_code_boc_base64(
            value,
            label="verifier code BoC",
        ),
        help=(
            "Deployed TON verifier code BoC as base64/base64url. When supplied, "
            "the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--verifier-code-boc-file",
        type=lambda value: parse_code_boc_file(
            value,
            label="verifier code BoC",
        ),
        help=(
            "Path to deployed TON verifier code BoC bytes, hex, or base64 text. "
            "When supplied, the helper derives verifier_code_hash."
        ),
    )
    parser.add_argument(
        "--route-allowlist-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route allowlist hash",
            byte_length=32,
        ),
        help=(
            "Governed TON route allowlist hash. Must match the canonical "
            "source material, source adapter deployment, and destination "
            "binding tuple."
        ),
    )
    parser.add_argument(
        "--source-verifier-material-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source verifier material hash",
            byte_length=32,
        ),
        help="Source verifier material record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--source-adapter-engine-deployment-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="source adapter engine deployment hash",
            byte_length=32,
        ),
        help="Source adapter engine deployment record hash bound into the route allowlist.",
    )
    parser.add_argument(
        "--route-canary-evidence-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="route canary evidence hash",
            byte_length=32,
        ),
        help=(
            "Non-zero post-deploy route canary evidence hash to emit as "
            "all-lanes preflight metadata."
        ),
    )
    parser.add_argument(
        "--expected-destination-binding-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="expected destination binding hash",
            byte_length=32,
        ),
        help="Expected canonical SORA -> TON destination binding hash.",
    )
    parser.add_argument(
        "--account-status",
        type=lambda value: parse_account_status(
            value,
            label="account status",
        ),
        help="Audited live TON verifier account status. Must be active for TOML.",
    )
    parser.add_argument(
        "--account-state-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="account state hash",
            byte_length=32,
        ),
        help="Audited live TON verifier account state hash; required for TOML.",
    )
    parser.add_argument(
        "--last-transaction-lt",
        type=lambda value: parse_positive_decimal_text(
            value,
            label="last transaction LT",
        ),
        help="Audited live TON verifier last transaction logical time; required for TOML.",
    )
    parser.add_argument(
        "--last-transaction-hash",
        type=lambda value: parse_hex_bytes(
            value,
            label="last transaction hash",
            byte_length=32,
        ),
        help="Audited live TON verifier last transaction hash; required for TOML.",
    )
    parser.add_argument(
        "--toml",
        action="store_true",
        help="Render production TOML records instead of a compact JSON summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        apply_verifier_code_boc_hash(args)
        destination_binding_hash = ton_destination_binding_hash()
        expected_matches = False
        if args.expected_destination_binding_hash is not None:
            if args.expected_destination_binding_hash != destination_binding_hash:
                raise ValueError(
                    "expected destination binding hash does not match the canonical "
                    "SORA -> TON binding: "
                    f"expected {_hex(args.expected_destination_binding_hash)}, "
                    f"got {_hex(destination_binding_hash)}"
                )
            expected_matches = True
        if args.toml:
            print(render_toml(args, destination_binding_hash), end="")
        else:
            print(
                json.dumps(
                    _json_summary(args, destination_binding_hash, expected_matches),
                    sort_keys=True,
                    indent=2,
                )
            )
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
