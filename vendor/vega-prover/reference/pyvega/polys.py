"""Multilinear / univariate polynomial evaluators over the scalar field F_q.

Faithful ports of ``src/polys/{eq,power,multilinear,univariate}.rs``. Every
function is a deterministic scalar computation (the verifier never needs the
prover's fast/threaded paths), so these mirror the *sequential* Rust code.

Scalars are Python ints in ``[0, Q)``; ``Q`` is the scalar-field modulus.
"""

from .params import Q


def _bit_len_pow2(n: int) -> int:
  """log2 of ``next_power_of_two(n)`` (matches ``n.next_power_of_two().log_2()``)."""
  if n <= 1:
    return 0
  return (n - 1).bit_length()


  # EqPolynomial
def eq_evals(r):
  """``EqPolynomial::evals_from_points`` — the 2^|r| eq-table, sequential path.

    evals[0]=1; for r_val in reversed(r): for i in 0..size: y=evals[i]*r_val;
    evals[size+i]=y; evals[i]-=y; size*=2.
    """
  evals = [0] * (1 << len(r))
  evals[0] = 1
  size = 1
  for r_val in reversed(r):
    for i in range(size):
      y = (evals[i] * r_val) % Q
      evals[size + i] = y
      evals[i] = (evals[i] - y) % Q
    size *= 2
  return evals


def eq_evaluate(r, rx):
  """``EqPolynomial::evaluate`` — prod_i (rx_i*r_i + (1-rx_i)(1-r_i))."""
  assert len(r) == len(rx)
  acc = 1
  for ri, xi in zip(r, rx):
    acc = (acc * ((xi * ri + (1 - xi) * (1 - ri)) % Q)) % Q
  return acc


  # PowPolynomial
def pow_evaluate(t, ell, r):
  """``PowPolynomial::new(t, ell).evaluate(r)``.

    t_pow = [t^(2^0), .., t^(2^(ell-1))]; acc = prod_i (1 + (t_pow[i]-1)*r_rev[i]).
    """
  if len(r) != ell:
    raise ValueError(f"pow_evaluate: expected {ell} elements, got {len(r)}")
  t_pow = []
  p = t % Q
  for _ in range(ell):
    t_pow.append(p)
    p = (p * p) % Q
  acc = 1
  for i, r_i in enumerate(reversed(r)):
    acc = (acc * ((1 + (t_pow[i] - 1) * r_i) % Q)) % Q
  return acc


  # SparsePolynomial
def sparse_poly_evaluate(num_vars, Z, r):
  """``SparsePolynomial::new(num_vars, Z).evaluate(r)`` — verbatim port.

    num_vars_z = log2(next_pow2(len(Z)));
    chis = eq_evals(r[num_vars-1-num_vars_z:]);
    eval_partial = sum(Z[i]*chis[i]);  common = prod(1-r[i], i in 0..num_vars-1-num_vars_z);
    return common * eval_partial.
    """
  assert num_vars == len(r)
  num_vars_z = _bit_len_pow2(len(Z))
  chis = eq_evals(r[num_vars - 1 - num_vars_z :])
  eval_partial = 0
  for z, chi in zip(Z, chis):
    eval_partial = (eval_partial + z * chi) % Q
  common = 1
  for i in range(num_vars - 1 - num_vars_z):
    common = (common * ((1 - r[i]) % Q)) % Q
  return (common * eval_partial) % Q


  # CompressedUniPoly / UniPoly
def unipoly_decompress(coeffs_except_linear, hint):
  """``CompressedUniPoly::decompress`` -> full coeff list [c0, linear, c1, c2, ..].

    linear = hint - 2*c[0] - sum(c[1:]);  coeffs = [c[0], linear, *c[1:]].
    """
  c = coeffs_except_linear
  linear = (hint - c[0] - c[0]) % Q
  for i in range(1, len(c)):
    linear = (linear - c[i]) % Q
  return [c[0], linear] + list(c[1:])


def unipoly_evaluate(coeffs, r):
  """``UniPoly::evaluate`` — eval = c0 + sum_{k>=1} c_k * r^k."""
  eval_ = coeffs[0] % Q
  power = r % Q
  for coeff in coeffs[1:]:
    eval_ = (eval_ + power * coeff) % Q
    power = (power * r) % Q
  return eval_
