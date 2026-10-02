# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Rateless fountain code over GF(2).

A payload is cut into ``k`` source atoms of :data:`~iroha_petal.lanes.ATOM_LEN`
bytes (the last one zero-padded). Encoded atom ``id`` is source atom ``id`` for
``id < k`` (systematic), and otherwise the XOR of a pseudo-random half of the
source atoms chosen by :func:`mask_words`. A receiver that holds any ``k + 2``
or so independent atoms, in any order, recovers the payload by Gaussian
elimination; lost frames cost nothing but time.
"""

from __future__ import annotations

from typing import List, Optional, Sequence

from .lanes import ATOM_LEN

__all__ = [
    "split_payload",
    "mask_len",
    "mix32",
    "mask_words",
    "encode_atom",
    "FountainDecoder",
]

_U32 = 0xFFFFFFFF


def split_payload(payload: bytes) -> List[bytes]:
    """Split a payload into zero-padded source atoms."""
    payload = bytes(payload)
    atoms = []
    for start in range(0, len(payload), ATOM_LEN):
        chunk = payload[start : start + ATOM_LEN]
        atoms.append(chunk + bytes(ATOM_LEN - len(chunk)))
    return atoms


def mask_len(k: int) -> int:
    """Number of 32-bit words needed for a mask over ``k`` source atoms."""
    return (k + 31) // 32


def mix32(x: int) -> int:
    """The 32-bit finalizer of MurmurHash3 (``fmix32``).

    Masks must not come from a GF(2)-linear generator such as xorshift: every
    mask would then lie in a subspace of dimension at most 32 and repair atoms
    could never raise the decoder rank past 32. The multiplications make this
    mixer nonlinear over GF(2).
    """
    x &= _U32
    x ^= x >> 16
    x = (x * 0x85EBCA6B) & _U32
    x ^= x >> 13
    x = (x * 0xC2B2AE35) & _U32
    x ^= x >> 16
    return x


def mask_words(k: int, crc: int, atom_id: int) -> List[int]:
    """The combination mask of encoded atom ``atom_id``, as little-endian bit words.

    ``crc`` is the payload CRC-32C and only diversifies masks between streams.
    ``k`` must be at least one. Atoms with ``atom_id < k`` are systematic (a
    unit vector); every other atom combines a pseudo-random half of the
    sources::

        seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
        word[w] = mix32(seed + (w + 1) * 0x9E3779B9)      (all arithmetic mod 2^32)

    Bits at or above ``k`` are cleared, and an all-zero mask is replaced by the
    single bit ``id mod k``.
    """
    if k < 1:
        raise ValueError("a fountain mask covers at least one source atom")
    mask = [0] * mask_len(k)
    if atom_id < k:
        mask[atom_id // 32] = 1 << (atom_id % 32)
        return mask
    seed = mix32(((atom_id * 0x9E3779B1) & _U32) ^ (crc & _U32) ^ 0xA5A5A5A5)
    for w in range(len(mask)):
        mask[w] = mix32((seed + (w + 1) * 0x9E3779B9) & _U32)
    tail = k % 32
    if tail:
        mask[-1] &= (1 << tail) - 1
    if not any(mask):
        bit = atom_id % k
        mask[bit // 32] |= 1 << (bit % 32)
    return mask


def _mask_int(words: Sequence[int]) -> int:
    value = 0
    for index, word in enumerate(words):
        value |= (word & _U32) << (32 * index)
    return value


def encode_atom(source: Sequence[bytes], crc: int, atom_id: int) -> bytes:
    """Encode atom ``atom_id`` from the source atoms."""
    bits = _mask_int(mask_words(len(source), crc, atom_id))
    out = 0
    while bits:
        low = bits & -bits
        out ^= int.from_bytes(source[low.bit_length() - 1], "big")
        bits ^= low
    return out.to_bytes(ATOM_LEN, "big")


class FountainDecoder:
    """Incremental Gaussian-elimination decoder.

    Each received row is kept as one Python integer: the combination mask in
    the low bits (bit ``i`` is source atom ``i``) and the atom data above it,
    so eliminating a pivot is a single XOR of two big integers.
    """

    __slots__ = ("_k", "_shift", "_pivot", "_rows")

    def __init__(self, k: int) -> None:
        if k <= 0:
            raise ValueError("a stream has at least one source atom")
        self._k = k
        self._shift = mask_len(k) * 32
        self._pivot: List[Optional[int]] = [None] * k
        self._rows: List[int] = []

    @property
    def source_atoms(self) -> int:
        """Number of source atoms."""
        return self._k

    @property
    def rank(self) -> int:
        """Number of linearly independent atoms received so far."""
        return len(self._rows)

    @property
    def is_complete(self) -> bool:
        """Whether enough independent atoms arrived to recover the payload."""
        return len(self._rows) == self._k

    def add_encoded(self, crc: int, atom_id: int, data: bytes) -> bool:
        """Add encoded atom ``atom_id``; return whether it increased the rank."""
        return self.add(mask_words(self._k, crc, atom_id), data)

    def add(self, mask: Sequence[int], data: bytes) -> bool:
        """Add a received combination; return whether it increased the rank.

        A mask of the wrong length, or one naming columns at or above ``k``,
        never raises the rank.
        """
        if len(data) != ATOM_LEN:
            raise ValueError("a fountain atom is 16 bytes")
        if len(mask) != mask_len(self._k):
            return False
        k = self._k
        combination = _mask_int(mask)
        if combination >> k:
            return False
        row = combination | (int.from_bytes(data, "big") << self._shift)
        pivot = self._pivot
        rows = self._rows
        while row:
            column = (row & -row).bit_length() - 1
            if column >= k:
                # the mask is empty: the atom depends on rows already held
                return False
            existing = pivot[column]
            if existing is None:
                pivot[column] = len(rows)
                rows.append(row)
                return True
            row ^= rows[existing]
        return False

    def solve(self) -> Optional[List[bytes]]:
        """Return the source atoms once the decoder is complete."""
        if not self.is_complete:
            return None
        k = self._k
        shift = self._shift
        mask_bits = (1 << shift) - 1
        solution = [0] * k
        rows = self._rows
        pivot = self._pivot
        for column in range(k - 1, -1, -1):
            row = rows[pivot[column]]
            value = row >> shift
            # keep only columns strictly above the pivot
            bits = (row & mask_bits) >> (column + 1)
            base = column + 1
            while bits:
                low = bits & -bits
                value ^= solution[base + low.bit_length() - 1]
                bits ^= low
            solution[column] = value
        return [value.to_bytes(ATOM_LEN, "big") for value in solution]
