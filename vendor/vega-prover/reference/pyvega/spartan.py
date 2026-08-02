"""Non-ZK relaxed-Spartan verifier (port of ``RelaxedR1CSSpartanProof::verify``).

Proves a relaxed R1CS instance ``(comm_W, comm_E, u, X)`` satisfies
``Az * Bz = u*Cz + E`` with ``z = (W, u, X)`` via two sum-checks (outer and
inner) plus two *direct* polynomial openings (``verify_direct``) for ``E`` at
``r_x`` and ``W`` at ``r_y[1..]``.

Only ``(u, X)`` are absorbed into the transcript here -- never the commitments;
this proof is sound only composed after a step (NIFS) that already bound the
commitments. That composition is exactly how the top-level verifier uses it.
"""

from .params import Q
from . import commitment
from . import mathutil
from .polys import eq_evals, eq_evaluate
from .sumcheck import sumcheck_verify


def verify_direct(vk, comm, v, blind, point):
  """``HyraxPCS::verify_direct`` -- check a direct opening and return the eval.

    ``comm`` is the (row) commitment, ``v`` the opened width-vector, ``blind`` the
    combined blind, ``point`` the evaluation point. Returns ``<v, eq(point_right)>``
    after checking ``comm_LZ == Com(v, blind)``.
    """
  num_cols = vk.num_cols
  if len(v) != num_cols:
    raise ValueError(f"Direct opening: v.len() ({len(v)}) != num_cols ({num_cols})")

  n = 1 << len(point)
  num_rows = mathutil.div_ceil(n, num_cols)
  num_vars_rows = mathutil.log2(num_rows)

  if len(comm) == 0 or len(comm) > num_rows:
    raise ValueError(
      f"Direct opening: commitment row count {len(comm)} is invalid (expected 1..={num_rows})"
    )

  if num_vars_rows == 0:
    comm_LZ = commitment.to_point(comm[0])
  else:
    L = eq_evals(point[:num_vars_rows])
    actual_rows = len(comm)
    comm_LZ = commitment.msm(L[:actual_rows], comm)

  expected = commitment.msm(v, vk.ck[: len(v)]) + (int(blind) % Q) * commitment.to_point(vk.h)
  if comm_LZ != expected:
    raise ValueError("Direct opening: commitment mismatch")

  R = eq_evals(point[num_vars_rows:])
  eval_ = 0
  for vi, ri in zip(v, R):
    eval_ = (eval_ + vi * ri) % Q
  return eval_


def evaluate_matrix(M, T_x, T_y):
  """``evaluate_matrix_with_tables`` -- ``sum_{i,j} M[i,j] * T_x[i] * T_y[j]``."""
  indptr = M.indptr()
  indices = M.indices()
  data = M.data()
  total = 0
  for row in range(len(indptr) - 1):
    tx = T_x[row]
    if tx == 0:
      continue
    row_sum = 0
    for idx in range(indptr[row], indptr[row + 1]):
      row_sum = (row_sum + T_y[indices[idx]] * data[idx]) % Q
    total = (total + tx * row_sum) % Q
  return total


def spartan_verify(proof, S, vk, U, transcript):
  """Verify ``proof`` against shape ``S``, direct-opening key ``vk``, instance ``U``."""
  # Absorb only (u, X) -- not the commitments.
  transcript.absorb_scalar(b"u_relaxed", U.u)
  transcript.absorb_scalars(b"X_relaxed", U.X)

  num_cons = S.num_cons
  num_vars = S.num_vars
  num_rounds_x = mathutil.log2(num_cons)
  num_vars_padded = mathutil.next_power_of_two(num_vars)
  num_rounds_y = mathutil.log2(num_vars_padded) + 1

  # Outer sum-check
  tau = [transcript.squeeze(b"t") for _ in range(num_rounds_x)]
  claim_outer_final, r_x = sumcheck_verify(
    proof.sc_proof_outer, 0, num_rounds_x, 3, transcript
  )

  claim_Az, claim_Bz, claim_uCzE = proof.claims_outer
  taus_bound_rx = eq_evaluate(tau, r_x)
  claim_outer_expected = (taus_bound_rx * ((claim_Az * claim_Bz - claim_uCzE) % Q)) % Q
  if claim_outer_final != claim_outer_expected:
    raise ValueError("InvalidSumcheckProof: outer claim mismatch")

  transcript.absorb_scalars(b"claims_outer", [claim_Az, claim_Bz, claim_uCzE])

  # Inner sum-check
  r = transcript.squeeze(b"r")
  r_sq = (r * r) % Q

  eval_E = verify_direct(vk, U.comm_E, proof.v_E, proof.blind_E, r_x)
  claim_inner_joint = (
    claim_Az + r * claim_Bz + r_sq * ((claim_uCzE - eval_E) % Q)
  ) % Q

  claim_inner_final, r_y = sumcheck_verify(
    proof.sc_proof_inner, claim_inner_joint, num_rounds_y, 2, transcript
  )

  eval_W = verify_direct(vk, U.comm_W, proof.v_W, proof.blind_W, r_y[1:])

  T_x = eq_evals(r_x)
  T_y = eq_evals(r_y)

  # eval_Z = (1-r_y[0])*eval_W + u*T_y[num_vars] + sum_j X[j]*T_y[num_vars+1+j]
  eval_Z = ((1 - r_y[0]) % Q) * eval_W % Q
  eval_Z = (eval_Z + U.u * T_y[num_vars]) % Q
  for j, x_j in enumerate(U.X):
    eval_Z = (eval_Z + x_j * T_y[num_vars + 1 + j]) % Q

  eval_A = evaluate_matrix(S.A, T_x, T_y)
  eval_B = evaluate_matrix(S.B, T_x, T_y)
  eval_C = evaluate_matrix(S.C, T_x, T_y)
  eval_ABC = (eval_A + r * eval_B + (r_sq * U.u % Q) * eval_C) % Q

  claim_inner_expected = (eval_ABC * eval_Z) % Q
  if claim_inner_final != claim_inner_expected:
    raise ValueError("InvalidSumcheckProof: inner claim mismatch")

  transcript.absorb_scalars(b"v_W", proof.v_W)
  transcript.absorb_scalars(b"v_E", proof.v_E)
