"""Parser for the Vega-MC verifier key, its digest, and its decoded contents.

The verifier key is bincode-serialized as ``vk.bin``. Because a bincode struct is
just the concatenation of its fields with no framing, we walk the fields in
declaration order and record each field's byte range; individual field bincode
can then be sliced directly out of ``vk.bin``.

Two views are produced simultaneously:

* **Byte ranges** for the 32-byte **digest** ``SHA-256(D)``, where ``D`` streams
  the fields in declaration order but the two circuit shapes use a compact
  ``shape_raw`` / ``matrix_raw`` encoding instead of bincode (see
  ``book/src/spec/verifier-key.md``). ``matrix_raw`` and bincode of a
  ``SparseMatrix`` contain byte-identical data/indices/indptr blobs and differ
  only in header placement, so ``shape_raw`` is reconstructed by regrouping
  headers.
* **Decoded structures** the verifier actually consumes: the commitment keys as
  lists of (lazily decompressed) group elements, and the R1CS shapes as CSR
  matrices with named dimension fields.

Field layout (declaration order):
  ck, vk_ee            HyraxCommitmentKey / HyraxVerifierKey (identical layout)
  S_step, S_core       SplitR1CSShape     = 10 u64 dims, then A,B,C:SparseMatrix
  vc_shape             SplitMultiRoundR1CSShape
  vc_shape_regular     R1CSShape          = num_cons,num_vars,num_io:u64, A,B,C
  vc_ck, vc_vk         HyraxCommitmentKey / HyraxVerifierKey
  num_steps            u64
Points serialize as 33-byte compressed group elements.
"""

import hashlib
from dataclasses import dataclass
from typing import List

from .codec import Reader
from .params import SCALAR_BYTES
from .curve import WirePoint, read_point


def _le64(x: int) -> bytes:
  return x.to_bytes(8, "little")


  # Hyrax commitment / verifier key
@dataclass
class HyraxKey:
  """A HyraxCommitmentKey / HyraxVerifierKey: generators plus a hiding base.

    ``num_cols`` is the row width; ``ck`` is the list of column generators; ``h``
    is the hiding generator. Points stay in :class:`WirePoint` form until a group
    operation forces decompression via :meth:`WirePoint.point`.
    """

  num_cols: int
  ck: List[WirePoint]
  h: WirePoint


def _parse_hyrax_key(r: Reader) -> HyraxKey:
  num_cols = r.u64()
  ck = r.vec(read_point)
  h = read_point(r)
  return HyraxKey(num_cols=num_cols, ck=ck, h=h)


  # Sparse (CSR) matrix
@dataclass
class SparseMatrixRaw:
  """A CSR matrix captured as byte blobs, decoded to ints on demand.

    The blobs are sufficient for the digest; :meth:`data`/:meth:`indices`/
    :meth:`indptr` decode them into the integer arrays used by matrix evaluation.
    """

  n_data: int
  data_blob: bytes  # n_data * 32 bytes, each a scalar in to_repr (LE) form
  n_indices: int
  indices_blob: bytes  # n_indices * 8 bytes, each a u64 column index
  n_indptr: int
  indptr_blob: bytes  # n_indptr * 8 bytes, each a u64 row offset
  cols: int

  def matrix_raw(self) -> bytes:
    """Compact digest encoding: grouped headers, then the three blobs."""
    return (
      _le64(self.n_data)
      + _le64(self.n_indices)
      + _le64(self.n_indptr)
      + _le64(self.cols)
      + self.data_blob
      + self.indices_blob
      + self.indptr_blob
    )

  def data(self) -> List[int]:
    b = self.data_blob
    return [
      int.from_bytes(b[i * SCALAR_BYTES : (i + 1) * SCALAR_BYTES], "little")
      for i in range(self.n_data)
    ]

  def indices(self) -> List[int]:
    b = self.indices_blob
    return [int.from_bytes(b[i * 8 : (i + 1) * 8], "little") for i in range(self.n_indices)]

  def indptr(self) -> List[int]:
    b = self.indptr_blob
    return [int.from_bytes(b[i * 8 : (i + 1) * 8], "little") for i in range(self.n_indptr)]


def parse_sparse_matrix(r: Reader) -> SparseMatrixRaw:
  n_data = r.u64()
  data_blob = r.take(n_data * SCALAR_BYTES)
  n_indices = r.u64()
  indices_blob = r.take(n_indices * 8)
  n_indptr = r.u64()
  indptr_blob = r.take(n_indptr * 8)
  cols = r.u64()
  return SparseMatrixRaw(
    n_data, data_blob, n_indices, indices_blob, n_indptr, indptr_blob, cols
  )


  # Split R1CS shape (S_step / S_core)
  # dims order: num_cons, num_cons_unpadded, num_shared_unpadded,
  # num_precommitted_unpadded, num_rest_unpadded, num_shared, num_precommitted,
  # num_rest, num_public, num_challenges
@dataclass
class SplitShape:
  dims: List[int]  # 10 dimension counts, in declaration order
  A: SparseMatrixRaw
  B: SparseMatrixRaw
  C: SparseMatrixRaw

  def shape_raw(self) -> bytes:
    out = b"".join(_le64(d) for d in self.dims)
    out += self.A.matrix_raw() + self.B.matrix_raw() + self.C.matrix_raw()
    return out

  @property
  def num_cons(self) -> int:
    return self.dims[0]

  @property
  def num_shared(self) -> int:
    return self.dims[5]

  @property
  def num_precommitted(self) -> int:
    return self.dims[6]

  @property
  def num_rest(self) -> int:
    return self.dims[7]

  @property
  def num_public(self) -> int:
    return self.dims[8]

  @property
  def num_challenges(self) -> int:
    return self.dims[9]

  @property
  def num_vars(self) -> int:
    return self.num_shared + self.num_precommitted + self.num_rest


def _parse_split_shape(r: Reader) -> SplitShape:
  dims = [r.u64() for _ in range(10)]
  A = parse_sparse_matrix(r)
  B = parse_sparse_matrix(r)
  C = parse_sparse_matrix(r)
  return SplitShape(dims, A, B, C)


  # Multi-round split shape (vc_shape)
@dataclass
class MultiRoundShape:
  num_cons: int
  num_cons_unpadded: int
  num_rounds: int
  num_vars_per_round_unpadded: List[int]
  num_vars_per_round: List[int]
  num_challenges_per_round: List[int]
  num_public: int
  commitment_width: int
  A: SparseMatrixRaw
  B: SparseMatrixRaw
  C: SparseMatrixRaw


def _parse_multiround_shape(r: Reader) -> MultiRoundShape:
  num_cons = r.u64()
  num_cons_unpadded = r.u64()
  num_rounds = r.u64()
  nvpr_unpadded = r.vec(Reader.u64)
  nvpr = r.vec(Reader.u64)
  ncpr = r.vec(Reader.u64)
  num_public = r.u64()
  commitment_width = r.u64()
  A = parse_sparse_matrix(r)
  B = parse_sparse_matrix(r)
  C = parse_sparse_matrix(r)
  return MultiRoundShape(
    num_cons=num_cons,
    num_cons_unpadded=num_cons_unpadded,
    num_rounds=num_rounds,
    num_vars_per_round_unpadded=nvpr_unpadded,
    num_vars_per_round=nvpr,
    num_challenges_per_round=ncpr,
    num_public=num_public,
    commitment_width=commitment_width,
    A=A,
    B=B,
    C=C,
  )


  # Regular R1CS shape (vc_shape_regular)
@dataclass
class R1CSShape:
  num_cons: int
  num_vars: int
  num_io: int
  A: SparseMatrixRaw
  B: SparseMatrixRaw
  C: SparseMatrixRaw


def _parse_r1cs_shape(r: Reader) -> R1CSShape:
  num_cons = r.u64()
  num_vars = r.u64()
  num_io = r.u64()
  A = parse_sparse_matrix(r)
  B = parse_sparse_matrix(r)
  C = parse_sparse_matrix(r)
  return R1CSShape(num_cons, num_vars, num_io, A, B, C)


  # The verifier key
@dataclass
class VerifierKey:
# Decoded structures consumed by verify()
  ck: HyraxKey
  vk_ee: HyraxKey
  S_step: SplitShape
  S_core: SplitShape
  vc_shape: MultiRoundShape
  vc_shape_regular: R1CSShape
  vc_ck: HyraxKey
  vc_vk: HyraxKey
  num_steps: int
  # Byte ranges for the digest stream (declaration order)
  ck_bincode: bytes = b""
  vk_ee_bincode: bytes = b""
  vc_shape_bincode: bytes = b""
  vc_shape_regular_bincode: bytes = b""
  vc_ck_bincode: bytes = b""
  vc_vk_bincode: bytes = b""
  num_steps_bincode: bytes = b""

  def digest_stream(self) -> bytes:
    return (
      self.ck_bincode
      + self.vk_ee_bincode
      + self.S_step.shape_raw()
      + self.S_core.shape_raw()
      + self.vc_shape_bincode
      + self.vc_shape_regular_bincode
      + self.vc_ck_bincode
      + self.vc_vk_bincode
      + self.num_steps_bincode
    )

  def digest(self) -> bytes:
    return hashlib.sha256(self.digest_stream()).digest()


def parse_vk(data: bytes) -> VerifierKey:
  r = Reader(data)

  o = r.pos
  ck = _parse_hyrax_key(r)
  ck_bincode = data[o : r.pos]

  o = r.pos
  vk_ee = _parse_hyrax_key(r)
  vk_ee_bincode = data[o : r.pos]

  S_step = _parse_split_shape(r)
  S_core = _parse_split_shape(r)

  o = r.pos
  vc_shape = _parse_multiround_shape(r)
  vc_shape_bincode = data[o : r.pos]

  o = r.pos
  vc_shape_regular = _parse_r1cs_shape(r)
  vc_shape_regular_bincode = data[o : r.pos]

  o = r.pos
  vc_ck = _parse_hyrax_key(r)
  vc_ck_bincode = data[o : r.pos]

  o = r.pos
  vc_vk = _parse_hyrax_key(r)
  vc_vk_bincode = data[o : r.pos]

  o = r.pos
  num_steps = r.u64()
  num_steps_bincode = data[o : r.pos]

  r.expect_end()

  return VerifierKey(
    ck=ck,
    vk_ee=vk_ee,
    S_step=S_step,
    S_core=S_core,
    vc_shape=vc_shape,
    vc_shape_regular=vc_shape_regular,
    vc_ck=vc_ck,
    vc_vk=vc_vk,
    num_steps=num_steps,
    ck_bincode=ck_bincode,
    vk_ee_bincode=vk_ee_bincode,
    vc_shape_bincode=vc_shape_bincode,
    vc_shape_regular_bincode=vc_shape_regular_bincode,
    vc_ck_bincode=vc_ck_bincode,
    vc_vk_bincode=vc_vk_bincode,
    num_steps_bincode=num_steps_bincode,
  )


def load_vk(data: bytes) -> VerifierKey:
  return parse_vk(data)
