"""Compressed group-element (point) encoding and the T256 curve arithmetic.

A group element serializes to **33 bytes**: a 1-byte flag followed by the
32-byte ``x`` coordinate in **big-endian**. The flag uses halo2curves'
``CompressedFlagConfig::Extra`` convention:

* ``0x40`` set  -> point at infinity (identity); the 32 x-bytes are all zero.
* otherwise      -> affine point, with ``0x80`` set iff ``y`` is odd
  (the "sign" bit = least-significant bit of ``y``'s little-endian ``to_repr``).

Decompression solves ``y^2 = x^3 - 3x + b`` for ``y`` and selects the root whose
parity matches the sign bit.

Points are parsed lazily as :class:`WirePoint` (raw bytes, no field work) so that
structural parsing stays cheap; call :meth:`WirePoint.point` to decompress to an
:class:`EccPoint` when the algebra is actually needed.

**Pure Python, no external CAS.** The base field ``F_p`` and the short-Weierstrass
curve are implemented here with plain integer arithmetic (Python's built-in
big-ints). Because ``p ≡ 3 (mod 4)`` the field square root is the closed form
``a^((p+1)/4) mod p``. This module is the only place group algebra lives; the rest
of the reference implementation (field codecs, polynomials, sum-check) is already
integer-only.
"""

from .params import P, A, B, GX, GY, SCALAR_BYTES, FLAG_SIGN, FLAG_IDENTITY

# Precomputed exponents for the base field F_p (p ≡ 3 mod 4).
_SQRT_EXP = (P + 1) // 4  # square root:      a^((p+1)/4)
_LEGENDRE_EXP = (P - 1) // 2  # Legendre symbol:  a^((p-1)/2)


class Fp:
  """An element of the base field ``F_p`` (coordinates live here).

    A thin wrapper over ``int`` supporting exactly the operations the reference
    prover's hash-to-curve and decompression need: ``+ - * **``, negation,
    ``is_square``, ``sqrt``, and ``int()`` for parity / serialization.
    """

  __slots__ = ("v",)

  def __init__(self, v):
    self.v = int(v) % P

  def __int__(self):
    return self.v

  def __index__(self):
    return self.v

  def __eq__(self, other):
    return self.v == (other.v if isinstance(other, Fp) else int(other) % P)

  def __hash__(self):
    return hash(self.v)

  def __add__(self, other):
    return Fp(self.v + int(other))

  __radd__ = __add__

  def __sub__(self, other):
    return Fp(self.v - int(other))

  def __rsub__(self, other):
    return Fp(int(other) - self.v)

  def __mul__(self, other):
    return Fp(self.v * int(other))

  __rmul__ = __mul__

  def __neg__(self):
    return Fp(-self.v)

  def __pow__(self, exp):
    return Fp(pow(self.v, int(exp), P))

  def is_square(self) -> bool:
    """Euler's criterion: ``a`` is a QR iff ``a^((p-1)/2) != -1`` (0 counts)."""
    return self.v == 0 or pow(self.v, _LEGENDRE_EXP, P) == 1

  def sqrt(self) -> "Fp":
    """A square root of ``a`` (p ≡ 3 mod 4 closed form); raises if non-square."""
    r = pow(self.v, _SQRT_EXP, P)
    if (r * r) % P != self.v:
      raise ValueError("element is not a square")
    return Fp(r)

  def __repr__(self):
    return f"Fp(0x{self.v:x})"


class EccPoint:
  """An affine point on ``E : y^2 = x^3 + A x + B`` over ``F_p`` (or the identity).

    Supports the group operations the Hyrax MSM needs: point addition (``+``),
    negation, and scalar multiplication (``k * P`` / ``P * k``) via double-and-add.
    ``EccPoint(inf=True)`` is the point at infinity (the group identity).
    """

  __slots__ = ("x", "y", "inf")

  def __init__(self, x=None, y=None, inf: bool = False):
    if inf:
      self.x = None
      self.y = None
      self.inf = True
    else:
      self.x = int(x) % P
      self.y = int(y) % P
      self.inf = False

  def is_zero(self) -> bool:
    return self.inf

  def __eq__(self, other):
    if not isinstance(other, EccPoint):
      return NotImplemented
    if self.inf or other.inf:
      return self.inf and other.inf
    return self.x == other.x and self.y == other.y

  def __hash__(self):
    return hash((self.x, self.y, self.inf))

  def __getitem__(self, i: int):
    """Coordinate access: ``pt[0]`` -> x, ``pt[1]`` -> y."""
    if self.inf:
      raise ValueError("the identity has no affine coordinates")
    return self.x if i == 0 else self.y

  def __neg__(self) -> "EccPoint":
    if self.inf:
      return self
    return EccPoint(self.x, (-self.y) % P)

  def __add__(self, other: "EccPoint") -> "EccPoint":
    if self.inf:
      return other
    if other.inf:
      return self
    if self.x == other.x:
      if (self.y + other.y) % P == 0:
        return EccPoint(inf=True)  # P + (-P) = O
      # Doubling: lambda = (3 x^2 + A) / (2 y).
      lam = (3 * self.x * self.x + A) * pow(2 * self.y % P, -1, P) % P
    else:
      # Chord: lambda = (y2 - y1) / (x2 - x1).
      lam = (other.y - self.y) * pow((other.x - self.x) % P, -1, P) % P
    xr = (lam * lam - self.x - other.x) % P
    yr = (lam * (self.x - xr) - self.y) % P
    return EccPoint(xr, yr)

  def __mul__(self, k: int) -> "EccPoint":
    k = int(k)
    if k < 0:
      return (-self).__mul__(-k)
    result = EccPoint(inf=True)
    addend = self
    while k:
      if k & 1:
        result = result + addend
      addend = addend + addend
      k >>= 1
    return result

  __rmul__ = __mul__

  def __repr__(self):
    if self.inf:
      return "EccPoint(identity)"
    return f"EccPoint(x=0x{self.x:x}, y=0x{self.y:x})"


class _Curve:
  """Callable curve object exposing an ``E(...)`` factory.

    ``E(0)`` returns the identity and ``E(x, y)`` builds an affine point, matching
    the constructor style the consumer code uses.
    """

  def __call__(self, x, y=None) -> EccPoint:
    if y is None:
      if int(x) != 0:
        raise ValueError("E(k) only accepts k == 0 (the identity)")
      return EccPoint(inf=True)
    return EccPoint(int(x), int(y))


_CURVE = _Curve()
_GENERATOR = EccPoint(GX, GY)


def base_field():
  """The base field ``F_p`` constructor (``Fp(int) -> Fp``)."""
  return Fp


def curve() -> _Curve:
  """The curve ``E`` (callable: ``E(0)`` identity, ``E(x, y)`` affine point)."""
  return _CURVE


def generator() -> EccPoint:
  """The canonical generator ``G``."""
  return _GENERATOR


class WirePoint:
  """A group element in its 33-byte compressed wire form (lazily decoded)."""

  __slots__ = ("flag", "x_bytes", "_point")

  def __init__(self, flag: int, x_bytes: bytes):
    if len(x_bytes) != SCALAR_BYTES:
      raise ValueError(f"x needs {SCALAR_BYTES} bytes, got {len(x_bytes)}")
    self.flag = flag
    self.x_bytes = x_bytes
    self._point = None

  @classmethod
  def parse(cls, reader) -> "WirePoint":
    """Consume one compressed point (33 bytes) from a Reader."""
    flag = reader.u8()
    x_bytes = reader.take(SCALAR_BYTES)
    wp = cls(flag, x_bytes)
    wp._validate()
    return wp

  def _validate(self):
    if self.is_identity and self.x_bytes != b"\x00" * SCALAR_BYTES:
      raise ValueError("identity flag set but x is non-zero")
      # The only defined non-identity flags are 0x00 and 0x80.
    if not self.is_identity and (self.flag & ~FLAG_SIGN) != 0:
      raise ValueError(f"unexpected point flag byte 0x{self.flag:02x}")

  @property
  def is_identity(self) -> bool:
    return bool(self.flag & FLAG_IDENTITY)

  @property
  def sign(self) -> int:
    """The encoded sign bit (1 = y odd), meaningless for the identity."""
    return 1 if (self.flag & FLAG_SIGN) else 0

  def point(self) -> EccPoint:
    """Decompress to an :class:`EccPoint` on the T256 curve (cached)."""
    if self._point is None:
      self._point = self._decompress()
    return self._point

  def _decompress(self) -> EccPoint:
    if self.is_identity:
      return EccPoint(inf=True)
    x = int.from_bytes(self.x_bytes, "big") % P
    rhs = (x * x % P * x + A * x + B) % P
    if rhs != 0 and pow(rhs, _LEGENDRE_EXP, P) != 1:
      raise ValueError("x is not on the curve (rhs is a non-residue)")
    y = pow(rhs, _SQRT_EXP, P)
    if (y * y) % P != rhs:
      raise ValueError("x is not on the curve")
    # Select the root whose parity matches the encoded sign bit.
    if (y & 1) != self.sign:
      y = (-y) % P
    return EccPoint(x, y)

  def to_wire(self) -> bytes:
    return bytes([self.flag]) + self.x_bytes

  def __repr__(self):
    if self.is_identity:
      return "WirePoint(identity)"
    return f"WirePoint(sign={self.sign}, x={self.x_bytes.hex()})"


def read_point(reader) -> WirePoint:
  return WirePoint.parse(reader)


def point_to_wire(pt) -> bytes:
  """Compress a curve point to its 33-byte wire form (prover side)."""
  if pt.is_zero():
    return bytes([FLAG_IDENTITY]) + b"\x00" * SCALAR_BYTES
  x = int(pt[0])
  y = int(pt[1])
  flag = FLAG_SIGN if (y & 1) else 0x00
  return bytes([flag]) + x.to_bytes(SCALAR_BYTES, "big")


def point_to_transcript(pt) -> bytes:
  """Transcript encoding of a group element: uncompressed ``x_LE || y_LE`` (64 B).

    Each coordinate is ``to_bytes`` (big-endian for the base field) reversed, i.e.
    little-endian. The identity has no coordinates; encoding it raises, matching
    the Rust ``coordinates().unwrap()`` panic (the identity is never absorbed).
    """
  if pt.is_zero():
    raise ValueError("identity has no transcript encoding (matches Rust panic)")
  x = int(pt[0])
  y = int(pt[1])
  return x.to_bytes(SCALAR_BYTES, "little") + y.to_bytes(SCALAR_BYTES, "little")
