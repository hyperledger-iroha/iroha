# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The one pseudo-random generator shared by whitening and the fountain code."""

from __future__ import annotations

__all__ = ["Xorshift32"]

_U32 = 0xFFFFFFFF


class Xorshift32:
    """Marsaglia xorshift32 with shifts 13, 17, 5.

    The generator is part of the wire format: whitening sequences and fountain
    masks are derived from it, so every implementation must match bit for bit.
    A zero seed is replaced by ``0xDEADBEEF`` because xorshift cannot leave the
    all-zero state.
    """

    __slots__ = ("_state",)

    def __init__(self, seed: int) -> None:
        if not 0 <= seed <= _U32:
            raise ValueError("xorshift32 seed must be a 32-bit unsigned integer")
        self._state = seed if seed else 0xDEADBEEF

    @property
    def state(self) -> int:
        """The current 32-bit state."""
        return self._state

    def next_u32(self) -> int:
        """Advance the generator and return the next 32-bit word."""
        x = self._state
        x ^= (x << 13) & _U32
        x ^= x >> 17
        x ^= (x << 5) & _U32
        self._state = x
        return x

    def next_byte(self) -> int:
        """Return the top byte of the next word."""
        return self.next_u32() >> 24

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Xorshift32):
            return NotImplemented
        return self._state == other._state

    def __hash__(self) -> int:
        return hash(("Xorshift32", self._state))

    def __repr__(self) -> str:
        return f"Xorshift32(state=0x{self._state:08X})"
