"""The Keccak-256 Fiat--Shamir transcript.

Faithful port of ``src/provider/keccak.rs``. The transcript keeps a 64-byte
``state`` and a running Keccak accumulator ``acc`` of everything absorbed since
the last squeeze. The core primitive is

    updated_state(prefix, input) =
        Keccak256(prefix || input || 0x00) || Keccak256(prefix || input || 0x01)

which produces 64 bytes from a running hasher (``prefix`` = bytes already
absorbed) plus fresh ``input``.

* ``new(label)``   : state = updated_state(b"", "NoTR" || label); acc = b"".
* ``absorb(l, x)`` : acc ||= l || x.to_transcript_bytes().
* ``squeeze(l)``   : output = updated_state(acc, "NoDS" || round_le16 || state || l);
                     round += 1; state = output; acc = b"";
                     return from_uniform(output).

Absorbed encodings (see :mod:`pyvega.field` / :mod:`pyvega.curve`):
  scalar -> 32-byte big-endian; group element -> uncompressed x_LE || y_LE (64 B).
"""

from Crypto.Hash import keccak

from .field import from_uniform, scalar_to_transcript, scalar_to_repr
from .curve import point_to_transcript

PERSONA_TAG = b"NoTR"
DOM_SEP_TAG = b"NoDS"
STATE_SIZE = 64

COMMIT_BEGIN = b"poly_commitment_begin"
COMMIT_END = b"poly_commitment_end"


def commitment_bytes(points) -> bytes:
  """``HyraxCommitment::to_transcript_bytes``: begin-tag, points, end-tag.

    ``points`` are decompressed group elements; each is encoded uncompressed as
    ``x_LE || y_LE`` (64 B). The identity is never a commitment component here.
    """
  body = b"".join(point_to_transcript(p) for p in points)
  return COMMIT_BEGIN + body + COMMIT_END


def keccak256(data: bytes) -> bytes:
  h = keccak.new(digest_bits=256)
  h.update(data)
  return h.digest()


def updated_state(prefix: bytes, input: bytes) -> bytes:
  """``Keccak256(prefix||input||0x00) || Keccak256(prefix||input||0x01)`` (64 B)."""
  base = prefix + input
  return keccak256(base + b"\x00") + keccak256(base + b"\x01")


class Transcript:
  """A Keccak-256 Fiat--Shamir transcript for the canonical engine."""

  def __init__(self, label: bytes):
    self.round = 0
    self.state = updated_state(b"", PERSONA_TAG + label)
    self.acc = b""

    # absorbing
  def absorb_raw(self, label: bytes, repr_bytes: bytes):
    self.acc += label + repr_bytes

  def absorb_scalar(self, label: bytes, v: int):
    self.absorb_raw(label, scalar_to_transcript(v))

  def absorb_scalars(self, label: bytes, vs):
  # A Vec<Scalar> absorbs its label once, then each element's repr bytes.
    buf = b"".join(scalar_to_transcript(v) for v in vs)
    self.absorb_raw(label, buf)

  def absorb_point(self, label: bytes, P):
    """Absorb a curve point (must be non-identity)."""
    self.absorb_raw(label, point_to_transcript(P))

  def absorb_unipoly(self, label: bytes, compressed_coeffs):
    """Absorb a sumcheck round polynomial (``UniPoly::to_transcript_bytes``).

        The transcript image of a ``UniPoly`` is its *compressed* coefficient
        vector (linear term omitted), each coefficient serialized ``to_repr`` =
        **little-endian** 32 bytes. This differs from :meth:`absorb_scalar`
        (big-endian); the round polynomial is the one place a scalar is absorbed
        little-endian.
        """
    buf = b"".join(scalar_to_repr(c) for c in compressed_coeffs)
    self.absorb_raw(label, buf)

  def absorb_commitment(self, label: bytes, points):
    """Absorb a Hyrax commitment (list of decompressed group elements)."""
    self.absorb_raw(label, commitment_bytes(points))

  def absorb_r1cs_instance(self, label: bytes, comm_W_points, X):
    """Absorb an ``R1CSInstance``: ``comm_W`` then ``X`` (scalars, BE)."""
    buf = commitment_bytes(comm_W_points) + b"".join(scalar_to_transcript(x) for x in X)
    self.absorb_raw(label, buf)

  def absorb_relaxed_instance(self, label: bytes, comm_W_points, comm_E_points, u, X):
    """Absorb a ``RelaxedR1CSInstance``: ``comm_W || comm_E || u || X``.

        ``u`` is a scalar absorbed big-endian; ``X`` is a scalar slice (BE).
        """
    buf = (
      commitment_bytes(comm_W_points)
      + commitment_bytes(comm_E_points)
      + scalar_to_transcript(u)
      + b"".join(scalar_to_transcript(x) for x in X)
    )
    self.absorb_raw(label, buf)

  def dom_sep(self, tag: bytes):
    self.acc += DOM_SEP_TAG + tag

    # squeezing
  def squeeze(self, label: bytes) -> int:
    if self.round >= 1 << 16:
      raise OverflowError("transcript round counter overflow (u16)")
    input = DOM_SEP_TAG + self.round.to_bytes(2, "little") + self.state + label
    output = updated_state(self.acc, input)
    self.round += 1
    self.state = output
    self.acc = b""
    return from_uniform(output)
