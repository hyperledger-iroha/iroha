"""Stand-alone Python setup: key generation, R1CS shapes, and verifier-key
serialization to the Rust bincode wire format.

This makes the reference implementation fully self-contained: Python generates
its own Hyrax commitment keys, builds the cubic application shape
(:mod:`pyvega.app_circuit`) and the fixed verifier-circuit shapes
(:mod:`pyvega.verifier_circuit`), and serializes a ``VerifierKey`` byte-for-byte
identical to the one the shipped library produces. No Rust runtime is involved.

The verifier key is a root of trust: a verifier must be instantiated with the
canonical key, not an arbitrary prover-supplied one. Its generators are
*nothing-up-my-sleeve* points, derived deterministically by hashing fixed
domain-separated labels to the curve (:mod:`pyvega.hash_to_curve`, reproducing
the library's ``from_label``: SHAKE256 XOF then RFC 9380 SSWU), so no one knows a
discrete-log relation between them. Reproducing that derivation exactly is what
lets an independent implementation arrive at the *same* trusted key. ``ck`` and
``vk_ee`` share generators, as do ``vc_ck`` and ``vc_vk``, matching the library.
"""

from typing import Dict, List

from .params import Q
from .field import scalar_to_repr
from .curve import point_to_wire
from .hash_to_curve import from_label
from . import mathutil
from .vk import SparseMatrixRaw, SplitShape
from . import app_circuit
from .verifier_circuit import VcConfig, zero_values, build


def _u64(n: int) -> bytes:
  return int(n).to_bytes(8, "little")


# hash-to-curve key generation
def keygen(label: bytes, num_cols: int):
  """Generate ``num_cols`` column generators plus a hiding base ``h``.

  The generators reproduce the shipped library's ``from_label`` exactly, so the
  serialized key is byte-identical to the Rust verifier key."""
  pts = from_label(label, num_cols + 1)
  return pts[:num_cols], pts[num_cols]


# sparse-matrix / shape serialization
def _csr(rows: List[Dict[int, int]], num_rows: int, cols: int) -> SparseMatrixRaw:
  """Build a :class:`SparseMatrixRaw` (CSR) from per-row ``{col: coeff}`` dicts,
  padding to ``num_rows`` with empty rows."""
  data = b""
  indices = b""
  indptr = [0]
  count = 0
  for r in range(num_rows):
    row = rows[r] if r < len(rows) else {}
    # bellpepper stores each constraint's terms sorted by column (aux indices
    # first, then inputs at idx + num_vars), so emit entries in that order.
    for col, val in sorted(row.items()):
      data += scalar_to_repr(val % Q)
      indices += _u64(col)
      count += 1
    indptr.append(count)
  return SparseMatrixRaw(
    n_data=count,
    data_blob=data,
    n_indices=count,
    indices_blob=indices,
    n_indptr=len(indptr),
    indptr_blob=b"".join(_u64(o) for o in indptr),
    cols=cols,
  )


def _matrix_bytes(m: SparseMatrixRaw) -> bytes:
  """Bincode wire form of a ``SparseMatrix``: data, indices, indptr vectors then cols."""
  return (
    _u64(m.n_data) + m.data_blob
    + _u64(m.n_indices) + m.indices_blob
    + _u64(m.n_indptr) + m.indptr_blob
    + _u64(m.cols)
  )


def _split_shape_bytes(shape: SplitShape) -> bytes:
  """Serialize a ``SplitR1CSShape``: 10 u64 dims, then A, B, C."""
  out = b"".join(_u64(d) for d in shape.dims)
  return out + _matrix_bytes(shape.A) + _matrix_bytes(shape.B) + _matrix_bytes(shape.C)


# verifier-circuit shapes
def _vc_config(num_steps: int, num_cons_app: int, num_vars_app: int) -> VcConfig:
  num_rounds_b = mathutil.log2(mathutil.next_power_of_two(num_steps))
  num_rounds_x = mathutil.log2(num_cons_app)
  num_rounds_y = mathutil.log2(num_vars_app) + 1
  return VcConfig(num_rounds_b, num_rounds_x, num_rounds_y, 32)


def _challenge_counts(cfg: VcConfig) -> List[int]:
  """Per-round squeezed-challenge counts (mirrors zk.rs process_round)."""
  ncpr = [0] * cfg.num_rounds
  for ri in range(cfg.num_rounds_b):  # NIFS rounds squeeze r_b
    ncpr[ri] = 1
  for ri in range(cfg.idx_outer_start, cfg.idx_outer_final):  # outer rounds -> r_x
    ncpr[ri] = 1
  ncpr[cfg.idx_outer_final] = 1  # outer final squeezes r_batch
  for ri in range(cfg.idx_inner_start, cfg.idx_inner_final):  # inner rounds -> r_y
    ncpr[ri] = 1
  return ncpr


def _vc_matrices(cfg: VcConfig):
  """Emit verifier-circuit A/B/C as padded SparseMatrixRaw + dimensions."""
  A_rows, B_rows, C_rows, W_rounds, _X = build(cfg, zero_values(cfg))
  num_cons_unpadded = len(A_rows)
  num_cons = mathutil.next_power_of_two(num_cons_unpadded)
  cols = cfg.num_vars + 1 + cfg.num_io
  nvpr_unpadded = [len(w) for w in W_rounds]
  A = _csr(A_rows, num_cons, cols)
  B = _csr(B_rows, num_cons, cols)
  C = _csr(C_rows, num_cons, cols)
  return A, B, C, nvpr_unpadded, num_cons_unpadded, num_cons, cols


def _vc_shape_bytes(cfg, A, B, C, nvpr_unpadded, num_cons_unpadded, num_cons) -> bytes:
  """Serialize the multi-round ``SplitMultiRoundR1CSShape`` (vc_shape)."""
  nvpr = [cfg.width] * cfg.num_rounds
  ncpr = _challenge_counts(cfg)
  out = _u64(num_cons) + _u64(num_cons_unpadded) + _u64(cfg.num_rounds)
  out += _u64(len(nvpr_unpadded)) + b"".join(_u64(v) for v in nvpr_unpadded)
  out += _u64(len(nvpr)) + b"".join(_u64(v) for v in nvpr)
  out += _u64(len(ncpr)) + b"".join(_u64(v) for v in ncpr)
  out += _u64(cfg.num_public) + _u64(cfg.width)
  return out + _matrix_bytes(A) + _matrix_bytes(B) + _matrix_bytes(C)


def _r1cs_shape_bytes(A, B, C, num_cons, num_vars, num_io) -> bytes:
  """Serialize the regular ``R1CSShape`` (vc_shape_regular; same A/B/C)."""
  out = _u64(num_cons) + _u64(num_vars) + _u64(num_io)
  return out + _matrix_bytes(A) + _matrix_bytes(B) + _matrix_bytes(C)


# Hyrax key serialization
def _hyrax_key_bytes(pts, h) -> bytes:
  out = _u64(len(pts)) + _u64(len(pts))  # num_cols, then Vec length
  for p in pts:
    out += point_to_wire(p)
  return out + point_to_wire(h)


# top-level
def serialize_vk(num_steps: int = 2) -> bytes:
  """Build and serialize the stand-alone verifier key for the cubic circuit.

  Both commitment keys derive from the single label ``b"ck"`` — the label the
  shipped library uses — so ``vc_ck`` is the length-33 prefix of ``ck`` and the
  whole key is byte-identical to the Rust verifier key."""
  num_vars_app = app_circuit.DEFAULT_NUM_REST
  num_cons_app = 4

  ck_pts, ck_h = keygen(b"ck", num_vars_app)
  vc_ck_pts, vc_ck_h = keygen(b"ck", 32)

  app_shape = app_circuit.cubic_shape(num_vars_app)
  cfg = _vc_config(num_steps, num_cons_app, num_vars_app)
  A, B, C, nvpr_unpadded, num_cons_unpadded, num_cons, _cols = _vc_matrices(cfg)

  out = b""
  out += _hyrax_key_bytes(ck_pts, ck_h)  # ck
  out += _hyrax_key_bytes(ck_pts, ck_h)  # vk_ee (shared generators)
  out += _split_shape_bytes(app_shape)  # S_step
  out += _split_shape_bytes(app_shape)  # S_core
  out += _vc_shape_bytes(cfg, A, B, C, nvpr_unpadded, num_cons_unpadded, num_cons)
  out += _r1cs_shape_bytes(A, B, C, num_cons, cfg.num_vars, cfg.num_io)
  out += _hyrax_key_bytes(vc_ck_pts, vc_ck_h)  # vc_ck
  out += _hyrax_key_bytes(vc_ck_pts, vc_ck_h)  # vc_vk (shared generators)
  out += _u64(num_steps)
  return out
