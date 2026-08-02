"""The application circuit proven by the reference prover: the cubic relation.

The shipped library proves a large SHA-256 circuit; for a small, self-contained
reference we prove the tiny cubic relation ``y = x^3 + x + 5`` (``x = 2``,
``y = 15``) — the same ``CubicCircuit`` the Rust test-suite uses. Its R1CS has
four constraints, yet proving it drives the whole protocol, and it stays easy to
check by hand.

The variable layout matches what the Rust bellpepper synthesizer emits:

  rest witness   z[0..num_rest)      unpadded = [x, x_sq, x_cu, y_computed]
  constant ONE   z[num_rest]
  public input   z[num_rest+1 ..]    = [y]

so ``cols = num_rest + 1 + num_public``.  Constraints ``(A z) o (B z) = C z``:

  row0:  x       * x    = x_sq
  row1:  x_sq    * x    = x_cu
  row2: (5 + x + x_cu)  * 1 = y_computed          (A row order: ONE, x, x_cu)
  row3:  y_public * 1   = y_computed              (the ``inputize`` constraint)

The CSR triples are emitted in the exact order the Rust linear combinations
produce them, so the constructed matrices are byte-identical to the fixture and
the verifier-key digest matches.
"""

from dataclasses import dataclass
from typing import List, Tuple

from .params import Q
from .vk import SparseMatrixRaw, SplitShape

# Column indices within the padded z-vector.
COL_X = 0
COL_XSQ = 1
COL_XCU = 2
COL_YCOMP = 3

DEFAULT_NUM_REST = 2048
NUM_PUBLIC = 1


def _sparse_from_triples(triples: List[Tuple[int, int, int]], num_rows: int, cols: int) -> SparseMatrixRaw:
  """Build a :class:`SparseMatrixRaw` from ``(row, col, value)`` triples.

    Triples are grouped by ``row`` preserving their given order *within* a row
    (matching the Rust CSR emission order), so the resulting data/indices blobs
    are byte-identical to the shipped matrices.
    """
  by_row: List[List[Tuple[int, int]]] = [[] for _ in range(num_rows)]
  for row, col, val in triples:
    by_row[row].append((col, val))

  data: List[int] = []
  indices: List[int] = []
  indptr: List[int] = [0]
  for row in range(num_rows):
    for col, val in by_row[row]:
      indices.append(col)
      data.append(val % Q)
    indptr.append(len(data))

  data_blob = b"".join(int(d).to_bytes(32, "little") for d in data)
  indices_blob = b"".join(int(i).to_bytes(8, "little") for i in indices)
  indptr_blob = b"".join(int(p).to_bytes(8, "little") for p in indptr)
  return SparseMatrixRaw(
    n_data=len(data),
    data_blob=data_blob,
    n_indices=len(indices),
    indices_blob=indices_blob,
    n_indptr=len(indptr),
    indptr_blob=indptr_blob,
    cols=cols,
  )


def cubic_shape(num_rest: int = DEFAULT_NUM_REST) -> SplitShape:
  """Construct the cubic ``SplitR1CSShape`` (identical for step and core)."""
  col_one = num_rest
  col_y = num_rest + 1
  cols = num_rest + 1 + NUM_PUBLIC
  num_cons = 4

  a_triples = [
    (0, COL_X, 1),
    (1, COL_XSQ, 1),
    (2, col_one, 5),
    (2, COL_X, 1),
    (2, COL_XCU, 1),
    (3, col_y, 1),
  ]
  b_triples = [
    (0, COL_X, 1),
    (1, COL_X, 1),
    (2, col_one, 1),
    (3, col_one, 1),
  ]
  c_triples = [
    (0, COL_XSQ, 1),
    (1, COL_XCU, 1),
    (2, COL_YCOMP, 1),
    (3, COL_YCOMP, 1),
  ]

  A = _sparse_from_triples(a_triples, num_cons, cols)
  B = _sparse_from_triples(b_triples, num_cons, cols)
  C = _sparse_from_triples(c_triples, num_cons, cols)

  # dims: num_cons, num_cons_unpadded, num_shared_unpadded,
  # num_precommitted_unpadded, num_rest_unpadded, num_shared,
  # num_precommitted, num_rest, num_public, num_challenges
  dims = [num_cons, num_cons, 0, 0, 4, 0, 0, num_rest, NUM_PUBLIC, 0]
  return SplitShape(dims=dims, A=A, B=B, C=C)


@dataclass
class CubicWitness:
  """A satisfying assignment: the ``rest`` witness plus its public values."""

  W: List[int]  # length num_rest (padded with zeros)
  public_values: List[int]  # length num_public


def cubic_witness(num_rest: int = DEFAULT_NUM_REST) -> CubicWitness:
  """Compute the satisfying assignment for ``x = 2``."""
  x = 2
  x_sq = (x * x) % Q
  x_cu = (x_sq * x) % Q
  y = (x_cu + x + 5) % Q
  rest = [x % Q, x_sq, x_cu, y] + [0] * (num_rest - 4)
  return CubicWitness(W=rest, public_values=[y])


def z_vector(w: CubicWitness, num_rest: int = DEFAULT_NUM_REST) -> List[int]:
  """Assemble ``z = [rest | ONE | public]`` (challenges are empty for cubic)."""
  return list(w.W) + [1] + list(w.public_values)


def multiply_vec(S: SplitShape, z: List[int]) -> Tuple[List[int], List[int], List[int]]:
  """Return ``(A z, B z, C z)`` as length-``num_cons`` vectors."""

  def matvec(M: SparseMatrixRaw) -> List[int]:
    indptr = M.indptr()
    indices = M.indices()
    data = M.data()
    out = [0] * (len(indptr) - 1)
    for row in range(len(indptr) - 1):
      acc = 0
      for idx in range(indptr[row], indptr[row + 1]):
        acc = (acc + data[idx] * z[indices[idx]]) % Q
      out[row] = acc
    return out

  return matvec(S.A), matvec(S.B), matvec(S.C)


def is_sat(S: SplitShape, z: List[int]) -> bool:
  """Check ``(A z) o (B z) == (C z)`` for every constraint row (strict R1CS)."""
  Az, Bz, Cz = multiply_vec(S, z)
  return all((a * b - c) % Q == 0 for a, b, c in zip(Az, Bz, Cz))
