"""bincode little-endian / fixint decoder.

The Vega proof and verifier key are serialized with bincode's *free* function
``bincode::serialize`` which is byte-identical to
``DefaultOptions::new().with_little_endian().with_fixint_encoding()``:

  * integers          little-endian, fixed width (u8/u16/u32/u64)
  * ``usize``         encoded as ``u64`` (8 bytes LE)
  * ``bool``          1 byte (0x00 / 0x01)
  * ``Vec<T>``/seq    8-byte LE ``u64`` length prefix, then that many elements
  * ``[T; N]``        no length prefix, exactly N elements
  * ``Option<T>``     1 tag byte (0x00 = None, 0x01 = Some) then T if Some
  * enum variant      4-byte LE ``u32`` variant index (not exercised by the proof)
  * struct / tuple    fields in declaration order, no framing

See ``book/src/spec/serialization.md`` for the full specification.
"""


class Reader:
  """A cursor over a byte string that consumes bincode-encoded values."""

  def __init__(self, data: bytes):
    self.data = data
    self.pos = 0

    # primitives
  def take(self, n: int) -> bytes:
    if self.pos + n > len(self.data):
      raise EOFError(
        f"need {n} bytes at offset {self.pos}, only "
        f"{len(self.data) - self.pos} remain"
      )
    out = self.data[self.pos : self.pos + n]
    self.pos += n
    return out

  def u8(self) -> int:
    return self.take(1)[0]

  def u16(self) -> int:
    return int.from_bytes(self.take(2), "little")

  def u32(self) -> int:
    return int.from_bytes(self.take(4), "little")

  def u64(self) -> int:
    return int.from_bytes(self.take(8), "little")

    # bincode encodes usize as u64.
  usize = u64

  def bool(self) -> bool:
    b = self.u8()
    if b not in (0, 1):
      raise ValueError(f"invalid bool byte 0x{b:02x} at offset {self.pos - 1}")
    return b == 1

    # combinators
  def vec(self, elem):
    """Decode ``Vec<T>``: 8-byte LE length prefix, then ``elem`` that many times."""
    n = self.usize()
    return [elem(self) for _ in range(n)]

  def array(self, n: int, elem):
    """Decode ``[T; N]``: no length prefix, exactly N elements."""
    return [elem(self) for _ in range(n)]

  def option(self, elem):
    """Decode ``Option<T>``: 1 tag byte then ``T`` when Some."""
    tag = self.u8()
    if tag == 0:
      return None
    if tag == 1:
      return elem(self)
    raise ValueError(f"invalid Option tag 0x{tag:02x} at offset {self.pos - 1}")

  def tuple(self, *elems):
    return tuple(elem(self) for elem in elems)

    # position helpers
  @property
  def remaining(self) -> int:
    return len(self.data) - self.pos

  @property
  def at_end(self) -> bool:
    return self.pos == len(self.data)

  def expect_end(self):
    if not self.at_end:
      raise ValueError(
        f"trailing bytes: consumed {self.pos} of {len(self.data)} "
        f"({self.remaining} left over)"
      )
