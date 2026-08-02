"""Prover-side polynomial primitives: univariate interpolation + MLE binding.

The verifier only ever *evaluates* polynomials (``polys.py``); the sum-check
prover additionally needs to

* **interpolate** a round polynomial from its evaluations at ``0, 1, 2, ...``
  (``UniPoly::from_evals``), and
* **bind** a multilinear polynomial's top variable to a challenge
  (``MultilinearPolynomial::bind_poly_var_top``).

Both are faithful ports of ``src/polys/{univariate,multilinear}.rs`` (the
sequential path — the shipped code's zero-structure fast paths are internal and
produce identical results).
"""

from typing import List

from .params import Q


def inv(x: int) -> int:
  """Modular inverse in ``F_q`` (Q is prime, so ``x^(Q-2)``)."""
  return pow(x % Q, Q - 2, Q)


TWO_INV = inv(2)
SIX_INV = inv(6)


def unipoly_from_evals(evals: List[int]) -> List[int]:
  """``UniPoly::from_evals`` -> monomial coefficients ``[c0, c1, ...]``.

    Closed-form for degree 2 (3 evals) and degree 3 (4 evals) matching the Rust
    ``from_evals_deg2``/``from_evals_deg3``; general case via Vandermonde solve.
    """
  n = len(evals)
  e = [v % Q for v in evals]
  if n == 3:
    c = e[0]
    a = ((e[0] - 2 * e[1] + e[2]) * TWO_INV) % Q
    b = (e[1] - c - a) % Q
    return [c % Q, b % Q, a % Q]
  if n == 4:
    d = e[0]
    delta3 = (e[3] - 3 * e[2] + 3 * e[1] - e[0]) % Q
    a = (delta3 * SIX_INV) % Q
    delta2 = (e[2] - 2 * e[1] + e[0]) % Q
    b = (delta2 * TWO_INV - 3 * a) % Q
    c = (e[1] - d - b - a) % Q
    return [d % Q, c % Q, b % Q, a % Q]
  # General: solve the Vandermonde system at xs = 0..n-1.
  xs = list(range(n))
  matrix = []
  for i in range(n):
    x = xs[i]
    row = [1 % Q, x % Q]
    for j in range(2, n):
      row.append((row[j - 1] * x) % Q)
    row.append(e[i])
    matrix.append(row)
  return _gaussian_elimination(matrix)


def _gaussian_elimination(matrix: List[List[int]]) -> List[int]:
  """Solve the augmented ``n x (n+1)`` system in ``F_q``; return the solution."""
  n = len(matrix)
  for col in range(n):
    pivot = None
    for row in range(col, n):
      if matrix[row][col] % Q != 0:
        pivot = row
        break
    if pivot is None:
      raise ValueError("gaussian_elimination: singular matrix")
    matrix[col], matrix[pivot] = matrix[pivot], matrix[col]
    inv_p = inv(matrix[col][col])
    for j in range(col, n + 1):
      matrix[col][j] = (matrix[col][j] * inv_p) % Q
    for row in range(n):
      if row != col and matrix[row][col] % Q != 0:
        factor = matrix[row][col]
        for j in range(col, n + 1):
          matrix[row][j] = (matrix[row][j] - factor * matrix[col][j]) % Q
  return [matrix[i][n] % Q for i in range(n)]


class MLE:
  """A dense multilinear polynomial over ``F_q`` (the sum-check prover's table).

    ``Z`` holds the ``2^num_vars`` evaluations over the boolean hypercube.
    :meth:`bind_top` binds the most-significant variable to a challenge, halving
    the table (high-to-low binding order, matching ``bind_poly_var_top``).
    """

  __slots__ = ("Z",)

  def __init__(self, Z: List[int]):
    self.Z = [v % Q for v in Z]

  def __len__(self) -> int:
    return len(self.Z)

  def __getitem__(self, i: int) -> int:
    return self.Z[i]

  def bind_top(self, r: int) -> None:
    n = len(self.Z) // 2
    r = r % Q
    left = self.Z[:n]
    right = self.Z[n:]
    self.Z = [(left[i] + r * (right[i] - left[i])) % Q for i in range(n)]

  def final(self) -> int:
    """After binding every variable, ``Z`` has one element: the evaluation."""
    if len(self.Z) != 1:
      raise ValueError(f"MLE.final: expected 1 element, got {len(self.Z)}")
    return self.Z[0]
