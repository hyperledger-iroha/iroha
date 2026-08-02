"""Field-element byte encodings for the canonical engine.

Two prime fields appear on the wire:

* **Scalar field** ``F_q`` (witnesses, challenges, matrix coefficients). Its
  canonical serialization ``to_repr`` is **little-endian**, 32 bytes. This is the
  form used everywhere a bare field element is serialized in the proof and in the
  verifier-key digest.

* **Base field** ``F_p`` (point coordinates). Its ``to_repr`` is also
  little-endian, but on the wire a base element appears only as the ``x``
  coordinate inside a *compressed point*, where it is stored **big-endian**
  (see :mod:`pyvega.curve`). No bare base element appears in the proof.

There is a third, transcript-only encoding (the byte-reverse of ``to_repr``);
it lives in :mod:`pyvega.transcript`, not here.

All values are represented as plain Python integers in ``[0, modulus)``.
"""

from .params import Q, P, SCALAR_BYTES


def scalar_from_repr(b: bytes) -> int:
  """Decode a scalar from its 32-byte little-endian ``to_repr`` form."""
  if len(b) != SCALAR_BYTES:
    raise ValueError(f"scalar needs {SCALAR_BYTES} bytes, got {len(b)}")
  v = int.from_bytes(b, "little")
  if v >= Q:
    raise ValueError("non-canonical scalar (>= q)")
  return v


def scalar_to_repr(v: int) -> bytes:
  """Encode a scalar as 32 bytes little-endian (``to_repr``)."""
  return (v % Q).to_bytes(SCALAR_BYTES, "little")


def read_scalar(reader) -> int:
  """Consume one scalar (32-byte LE) from a :class:`pyvega.codec.Reader`."""
  return scalar_from_repr(reader.take(SCALAR_BYTES))


def base_from_repr(b: bytes) -> int:
  """Decode a base-field element from its 32-byte little-endian ``to_repr``."""
  if len(b) != SCALAR_BYTES:
    raise ValueError(f"base element needs {SCALAR_BYTES} bytes, got {len(b)}")
  v = int.from_bytes(b, "little")
  if v >= P:
    raise ValueError("non-canonical base element (>= p)")
  return v


def base_to_repr(v: int) -> bytes:
  """Encode a base-field element as 32 bytes little-endian (``to_repr``)."""
  return (v % P).to_bytes(SCALAR_BYTES, "little")


def from_uniform(b: bytes) -> int:
  """Reduce 64 uniform bytes to a scalar (halo2curves ``from_uniform_bytes``).

    The 64 bytes are interpreted as a **little-endian** 512-bit integer and
    reduced modulo ``q``. This is exactly how the transcript's ``squeeze`` maps
    its 64-byte Keccak output to a field challenge.
    """
  if len(b) != 64:
    raise ValueError(f"from_uniform needs 64 bytes, got {len(b)}")
  return int.from_bytes(b, "little") % Q


def scalar_to_transcript(v: int) -> bytes:
  """Transcript encoding of a scalar: ``to_repr`` (LE) reversed = 32-byte BE."""
  return scalar_to_repr(v)[::-1]
