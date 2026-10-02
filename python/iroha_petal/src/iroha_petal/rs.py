# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Reed-Solomon over GF(2^8) with errors-and-erasures decoding.

The field uses the primitive polynomial ``x^8 + x^4 + x^3 + x^2 + 1``
(``0x11D``, the same field as QR Code) with ``alpha = 2``. A codeword is
``data || parity``, systematic, and the generator is
``g(x) = (x - alpha^0)(x - alpha^1)...(x - alpha^(nsym-1))``, so the first
consecutive root is ``alpha^0``. The first byte of a codeword is the
highest-degree coefficient.

Every Petal lane is one codeword (``n <= 255``). Low-confidence cells are
passed to :meth:`ReedSolomon.decode` as erasures, which cost one parity symbol
each instead of two.
"""

from __future__ import annotations

import enum
from typing import Iterable, List, Sequence

__all__ = ["RsErrorKind", "RsError", "ReedSolomon", "gf_mul", "gf_exp"]

#: Primitive polynomial of the field.
_PRIMITIVE = 0x11D


def _make_tables():
    exp = [0] * 512
    log = [0] * 256
    x = 1
    for i in range(255):
        exp[i] = x
        log[x] = i
        x <<= 1
        if x & 0x100:
            x ^= _PRIMITIVE
    for j in range(255, 512):
        exp[j] = exp[j - 255]
    return tuple(exp), tuple(log)


_EXP, _LOG = _make_tables()


def gf_mul(a: int, b: int) -> int:
    """Multiply two field elements."""
    if a == 0 or b == 0:
        return 0
    return _EXP[_LOG[a] + _LOG[b]]


def _gf_div(a: int, b: int) -> int:
    """Divide ``a`` by a non-zero ``b``."""
    if a == 0:
        return 0
    return _EXP[_LOG[a] + 255 - _LOG[b]]


def gf_exp(exponent: int) -> int:
    """Return ``alpha^exponent``."""
    return _EXP[exponent % 255]


def _gf_inv(a: int) -> int:
    """Return the multiplicative inverse of a non-zero element."""
    return _EXP[255 - _LOG[a]]


def _poly_mul(a: Sequence[int], b: Sequence[int]) -> List[int]:
    """Multiply two polynomials stored lowest-degree first."""
    out = [0] * (len(a) + len(b) - 1)
    exp, log = _EXP, _LOG
    for i, x in enumerate(a):
        if x == 0:
            continue
        log_x = log[x]
        for j, y in enumerate(b):
            if y:
                out[i + j] ^= exp[log_x + log[y]]
    return out


def _poly_eval(poly: Sequence[int], x: int) -> int:
    """Evaluate a lowest-degree-first polynomial at ``x`` (Horner)."""
    exp, log = _EXP, _LOG
    acc = 0
    if x == 0:
        return poly[0] if poly else 0
    log_x = log[x]
    for coefficient in reversed(poly):
        acc = (exp[log[acc] + log_x] if acc else 0) ^ coefficient
    return acc


class RsErrorKind(enum.Enum):
    """Reasons a Reed-Solomon decode can fail."""

    #: The codeword length, parity count or erasure list is invalid.
    INVALID_SHAPE = "invalid Reed-Solomon codeword shape"
    #: More errata than the code can correct, or the word is not decodable.
    UNCORRECTABLE = "Reed-Solomon word is uncorrectable"


class RsError(ValueError):
    """A Reed-Solomon decode failed; :attr:`kind` says why."""

    def __init__(self, kind: RsErrorKind) -> None:
        super().__init__(kind.value)
        self.kind = kind


def _syndromes(nsym: int, word: Sequence[int]) -> List[int]:
    exp, log = _EXP, _LOG
    out = []
    for j in range(nsym):
        # alpha^j has logarithm j for every j < 255
        acc = 0
        for byte in word:
            acc = (exp[log[acc] + j] if acc else 0) ^ byte
        out.append(acc)
    return out


def _berlekamp_massey(syndromes: Sequence[int]) -> List[int]:
    """Berlekamp-Massey over GF(256); returns the lowest-degree-first locator."""
    n = len(syndromes)
    c = [0] * (n + 1)
    b = [0] * (n + 1)
    c[0] = 1
    b[0] = 1
    length = 0
    m = 1
    previous_discrepancy = 1
    for i in range(n):
        d = syndromes[i]
        for j in range(1, length + 1):
            d ^= gf_mul(c[j], syndromes[i - j])
        if d == 0:
            m += 1
            continue
        scale = _gf_div(d, previous_discrepancy)
        if 2 * length <= i:
            snapshot = list(c)
            for j in range(max(n + 1 - m, 0)):
                c[j + m] ^= gf_mul(scale, b[j])
            length = i + 1 - length
            b = snapshot
            previous_discrepancy = d
            m = 1
        else:
            for j in range(max(n + 1 - m, 0)):
                c[j + m] ^= gf_mul(scale, b[j])
            m += 1
    return c[: length + 1]


class ReedSolomon:
    """A Reed-Solomon code with a fixed number of parity bytes."""

    __slots__ = ("_nsym", "_generator")

    def __init__(self, nsym: int) -> None:
        if not 1 <= nsym <= 254:
            raise ValueError("parity byte count out of range")
        # Highest-degree-first monic generator.
        generator = [1]
        for i in range(nsym):
            root = gf_exp(i)
            following = [0] * (len(generator) + 1)
            for k, coefficient in enumerate(generator):
                following[k] ^= coefficient
                following[k + 1] ^= gf_mul(coefficient, root)
            generator = following
        self._nsym = nsym
        self._generator = tuple(generator)

    @property
    def parity_len(self) -> int:
        """Number of parity bytes."""
        return self._nsym

    def encode(self, data: bytes) -> bytes:
        """Encode ``data``, returning ``data || parity``."""
        data = bytes(data)
        nsym = self._nsym
        if len(data) + nsym > 255:
            raise ValueError("Reed-Solomon codeword longer than 255 bytes")
        generator = self._generator
        exp, log = _EXP, _LOG
        remainder = [0] * nsym
        for byte in data:
            feedback = byte ^ remainder[0]
            if feedback:
                log_f = log[feedback]
                for j in range(nsym):
                    following = remainder[j + 1] if j + 1 < nsym else 0
                    g = generator[j + 1]
                    remainder[j] = following ^ (exp[log_f + log[g]] if g else 0)
            else:
                for j in range(nsym):
                    remainder[j] = remainder[j + 1] if j + 1 < nsym else 0
        return data + bytes(remainder)

    def syndromes(self, word: bytes) -> List[int]:
        """Syndromes ``S_j = word(alpha^j)`` for ``j < parity_len``."""
        return _syndromes(self._nsym, word)

    def decode(self, word: bytearray, erasures: Iterable[int] = ()) -> int:
        """Correct ``word`` in place, treating ``erasures`` as known-bad positions.

        ``word`` must be a mutable byte buffer (``bytearray``). Succeeds when
        ``2 * errors + erasures <= parity_len``. The corrected word is
        re-checked against zero syndromes before it is written back, so a
        success always yields a valid codeword. Returns the number of rewritten
        positions: every erasure plus the errors found elsewhere (``0`` when the
        word was already a codeword).

        Raises :class:`RsError` with :attr:`RsErrorKind.INVALID_SHAPE` for
        malformed arguments and :attr:`RsErrorKind.UNCORRECTABLE` when the word
        cannot be decoded.
        """
        nsym = self._nsym
        n = len(word)
        erasures = list(erasures)
        if n <= nsym or n > 255 or len(erasures) > nsym:
            raise RsError(RsErrorKind.INVALID_SHAPE)
        seen = set()
        for position in erasures:
            if position < 0 or position >= n or position in seen:
                raise RsError(RsErrorKind.INVALID_SHAPE)
            seen.add(position)
        syndromes = _syndromes(nsym, word)
        if not any(syndromes):
            return 0
        f = len(erasures)
        # Erasure locator Gamma(x) = prod (1 + X_e x), lowest degree first.
        gamma = [1]
        for position in erasures:
            gamma = _poly_mul(gamma, (1, gf_exp(n - 1 - position)))
        # Forney syndromes: the coefficients of S(x)Gamma(x) from index f upward
        # are the syndromes of the error-only word.
        forney = _poly_mul(syndromes, gamma)[:nsym]
        locator = _berlekamp_massey(forney[f:])
        error_count = len(locator) - 1
        if 2 * error_count + f > nsym:
            raise RsError(RsErrorKind.UNCORRECTABLE)
        psi = _poly_mul(locator, gamma)
        degree = len(psi) - 1
        # Chien search over all positions.
        positions = [i for i in range(n) if _poly_eval(psi, gf_exp(255 - ((n - 1 - i) % 255))) == 0]
        if len(positions) != degree:
            raise RsError(RsErrorKind.UNCORRECTABLE)
        # Omega(x) = S(x)Psi(x) mod x^nsym.
        omega = _poly_mul(syndromes, psi)[:nsym]
        # Formal derivative of Psi in characteristic 2 keeps odd-degree terms.
        derivative = [c if k % 2 == 1 else 0 for k, c in enumerate(psi) if k >= 1]
        corrected = bytearray(word)
        for i in positions:
            x = gf_exp(n - 1 - i)
            x_inv = _gf_inv(x)
            numerator = _poly_eval(omega, x_inv)
            denominator = _poly_eval(derivative, x_inv)
            if denominator == 0:
                raise RsError(RsErrorKind.UNCORRECTABLE)
            corrected[i] ^= gf_mul(x, _gf_div(numerator, denominator))
        if any(_syndromes(nsym, corrected)):
            raise RsError(RsErrorKind.UNCORRECTABLE)
        word[:] = corrected
        return len(positions)
