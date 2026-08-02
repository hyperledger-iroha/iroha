"""Serialize a :class:`VegaMcZkSNARK` to the Rust bincode wire format.

Inverts :mod:`pyvega.proof` (which decodes the wire format).  The encoding is
bincode's free ``serialize`` == ``DefaultOptions + little-endian + fixint``:

  * scalar        32 bytes little-endian (``to_repr``)
  * group elem    33 bytes compressed (flag byte || x big-endian)
  * ``Vec<T>``    8-byte LE ``u64`` length prefix, then the elements
  * ``Option<T>`` 1 tag byte (0x00 None / 0x01 Some), then T if Some
  * struct/tuple  fields in declaration order, no framing

See ``book/src/spec/serialization.md`` for the full specification.
"""

from typing import List

from .field import scalar_to_repr
from .curve import point_to_wire, WirePoint


def _u64(n: int) -> bytes:
  return int(n).to_bytes(8, "little")


def _scalar(v: int) -> bytes:
  return scalar_to_repr(v)


def _point(p) -> bytes:
  """A group element as 33 compressed bytes (accepts WirePoint or EccPoint)."""
  if isinstance(p, WirePoint):
    return p.to_wire()
  return point_to_wire(p)


def _commitment(points) -> bytes:
  """``Vec<GE>`` -- length prefix then each compressed point."""
  out = _u64(len(points))
  for p in points:
    out += _point(p)
  return out


def _option_commitment(c) -> bytes:
  if c is None:
    return b"\x00"
  return b"\x01" + _commitment(c)


def _vec_scalar(xs: List[int]) -> bytes:
  out = _u64(len(xs))
  for x in xs:
    out += _scalar(x)
  return out


def _split_r1cs_instance(u) -> bytes:
  return (
    _option_commitment(u.comm_W_shared)
    + _option_commitment(u.comm_W_precommitted)
    + _commitment(u.comm_W_rest)
    + _vec_scalar(u.public_values)
    + _vec_scalar(u.challenges)
  )


def _split_multiround_instance(u) -> bytes:
  out = _u64(len(u.comm_w_per_round))
  for c in u.comm_w_per_round:
    out += _commitment(c)
  out += _vec_scalar(u.public_values)
  out += _u64(len(u.challenges_per_round))
  for chals in u.challenges_per_round:
    out += _vec_scalar(chals)
  return out


def _sumcheck_proof(sc) -> bytes:
  """``SumcheckProof`` == ``Vec<CompressedUniPoly>``; each poly is ``Vec<scalar>``."""
  out = _u64(len(sc))
  for poly in sc:
    out += _vec_scalar(poly)
  return out


def _ipa(a) -> bytes:
  return (
    _point(a.delta)
    + _point(a.beta)
    + _vec_scalar(a.z_vec)
    + _scalar(a.z_delta)
    + _scalar(a.z_beta)
  )


def _relaxed_instance(u) -> bytes:
  return (
    _commitment(u.comm_W)
    + _commitment(u.comm_E)
    + _vec_scalar(u.X)
    + _scalar(u.u)
  )


def _relaxed_spartan_proof(p) -> bytes:
  ca, cb, cc = p.claims_outer
  return (
    _sumcheck_proof(p.sc_proof_outer)
    + _scalar(ca) + _scalar(cb) + _scalar(cc)
    + _sumcheck_proof(p.sc_proof_inner)
    + _vec_scalar(p.v_W)
    + _scalar(p.blind_W)
    + _vec_scalar(p.v_E)
    + _scalar(p.blind_E)
  )


def serialize_proof(proof) -> bytes:
  """Encode a :class:`VegaMcZkSNARK` to bytes accepted by the Rust verifier."""
  out = _option_commitment(proof.comm_W_shared)
  out += _u64(len(proof.step_instances))
  for u in proof.step_instances:
    out += _split_r1cs_instance(u)
  out += _split_r1cs_instance(proof.core_instance)
  out += _ipa(proof.eval_arg.ipa)  # HyraxEvaluationArgument == its ipa (no framing)
  out += _split_multiround_instance(proof.U_verifier)
  out += _commitment(proof.nifs.comm_T)  # NovaNIFS == its comm_T
  out += _relaxed_instance(proof.random_U)
  out += _relaxed_spartan_proof(proof.relaxed_snark)
  return out
