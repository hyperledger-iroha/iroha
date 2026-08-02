"""Deterministic nothing-up-my-sleeve generator derivation.

Reproduces the shipped library's ``GE::from_label`` exactly, so the Python setup
builds the *same* verifier key as Rust rather than a different valid one. The
verifier key is a root of trust: its Hyrax generators are the canonical points
every party must agree on, derived so that no one knows a discrete-log relation
among them. The derivation is:

  1. ``SHAKE256(label)`` XOF, read as consecutive 32-byte messages;
  2. each message maps to the curve with RFC 9380 ``hash_to_curve``, suite
     ``T256_XMD:SHA-256_SSWU_RO_`` and domain prefix ``from_uniform_bytes``;
  3. ``ck`` is the first ``num_cols`` points; the hiding base ``h`` is the next.

Sources in the shipped library:
  * from_label            src/provider/traits.rs:205-249
  * SSWU suite, Z = -2    halo2curves-0.10.0/src/t256/curve.rs:92-108
  * expand_message_xmd    halo2curves-0.10.0/src/hash_to_curve.rs:33-90
  * sswu_map_to_curve     halo2curves-0.10.0/src/hash_to_curve.rs:197-300
"""

import hashlib
from typing import List, Tuple

from .params import P, A, B
from .curve import Fp, EccPoint

_DST_PREFIX = b"from_uniform_bytes"
_DST_DOMAIN = b"T256_XMD:SHA-256_SSWU_RO_"
_DST_PRIME = _DST_PREFIX + _DST_DOMAIN + bytes([len(_DST_PREFIX) + len(_DST_DOMAIN)])

_SSWU_Z = Fp(-2)  # p - 2, the suite's non-square Z
_L = 48  # uniform bytes per field element (128-bit security margin over 256-bit p)
_SHA256_BLOCK = 64


def _inv0(a: Fp) -> Fp:
  """Field inverse with the RFC 9380 convention ``inv0(0) = 0``."""
  if int(a) == 0:
    return Fp(0)
  return Fp(pow(int(a), P - 2, P))


def expand_message_xmd(msg: bytes, out_len: int) -> bytes:
  """RFC 9380 ``expand_message_xmd`` over SHA-256 for this suite's DST."""
  z_pad = b"\x00" * _SHA256_BLOCK
  l_i_b = bytes([(out_len >> 8) & 0xFF, out_len & 0xFF])
  b0 = hashlib.sha256(z_pad + msg + l_i_b + b"\x00" + _DST_PRIME).digest()
  b1 = hashlib.sha256(b0 + b"\x01" + _DST_PRIME).digest()
  blocks = [b1]
  ell = (out_len + 31) // 32
  for i in range(2, ell + 1):
    xored = bytes(x ^ y for x, y in zip(b0, blocks[-1]))
    blocks.append(hashlib.sha256(xored + bytes([i]) + _DST_PRIME).digest())
  return b"".join(blocks)[:out_len]


def hash_to_field(msg: bytes) -> Tuple[Fp, Fp]:
  """Two field elements ``u0, u1`` in ``F_p`` (RFC 9380 with ``count = 2``).

  ``expand_message_xmd`` emits big-endian bytes; the base field's
  ``from_uniform_bytes`` reads little-endian, so the reduction is over the
  big-endian integer of each 48-byte block.
  """
  wide = expand_message_xmd(msg, 2 * _L)
  u0 = Fp(int.from_bytes(wide[:_L], "big"))
  u1 = Fp(int.from_bytes(wide[_L:], "big"))
  return u0, u1


def _sqrt_ratio(num: Fp, div: Fp) -> Tuple[bool, Fp]:
  """RFC 9380 ``sqrt_ratio`` for a field with non-square ``Z``.

  Returns ``(is_square, y)`` with ``y**2 == num/div`` when ``num/div`` is a
  square, else ``y**2 == Z*num/div``.
  """
  a = _inv0(div) * num
  if a.is_square():
    return (int(num) == 0 or int(div) != 0), a.sqrt()
  return False, (a * _SSWU_Z).sqrt()


def sswu_map_to_curve(u: Fp) -> EccPoint:
  """Simplified SWU map for a curve with ``a, b != 0`` (RFC 9380, section 6.6.2).

  The final sign of ``y`` is forced to match ``sgn0(u)``, so the point does not
  depend on which square root the field returns.
  """
  a, b, z = Fp(A), Fp(B), _SSWU_Z

  tv1 = z * (u * u)
  tv2 = tv1 * tv1
  tv2 = tv2 + tv1
  tv3 = tv2 + Fp(1)
  tv3 = b * tv3
  tv4 = (-tv2) if int(tv2) != 0 else z
  tv4 = a * tv4
  tv2 = tv3 * tv3
  tv6 = tv4 * tv4
  tv5 = a * tv6
  tv2 = tv2 + tv5
  tv2 = tv2 * tv3
  tv6 = tv6 * tv4
  tv5 = b * tv6
  tv2 = tv2 + tv5
  x = tv1 * tv3
  is_gx1_square, y1 = _sqrt_ratio(tv2, tv6)
  y = tv1 * u
  y = y * y1
  if is_gx1_square:
    x = tv3
    y = y1
  if (int(u) & 1) != (int(y) & 1):
    y = -y

  # Homogeneous (x, y*tv4, tv4); the affine point is (x/tv4, y).
  x_affine = x * _inv0(tv4)
  return EccPoint(int(x_affine), int(y))


def hash_to_curve(msg: bytes) -> EccPoint:
  """RFC 9380 ``hash_to_curve`` (random-oracle) for T256; cofactor is 1."""
  u0, u1 = hash_to_field(msg)
  return sswu_map_to_curve(u0) + sswu_map_to_curve(u1)


def from_label(label: bytes, n: int) -> List[EccPoint]:
  """The ``n`` generators derived from ``label`` (mirrors ``GE::from_label``)."""
  xof = hashlib.shake_256(label).digest(32 * n)
  return [hash_to_curve(xof[32 * i:32 * i + 32]) for i in range(n)]
