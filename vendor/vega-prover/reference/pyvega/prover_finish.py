"""Finishing stages of the stand-alone Python prover.

Given a :class:`~pyvega.prover.ProverCoreResult` (a validated ``W_verifier`` +
``U_verifier`` plus the folded application objects), this module completes the
proof exactly as ``VegaMcZkSNARK::prove`` does after the in-circuit verifier
witness is finalized:

1. **Nova fold** -- sample a fresh random *satisfying* relaxed instance/witness
   over ``vc_shape_regular`` (the per-prove ZK mask) and fold the verifier-circuit
   instance into it via a Nova NIFS (``sample_random_instance_witness`` +
   ``NovaNIFS::prove``/``commit_T``).
2. **Relaxed R1CS Spartan** -- a *non-ZK* direct-opening proof that the folded
   verifier-circuit instance satisfies the relaxed relation ``Az*Bz = u*Cz + E``
   (two plaintext sum-checks + two Hyrax direct openings).  Safe only because it
   runs on the masked fold.
3. **ZK linear IPA** -- fold the two witness-evaluation claims with ``c_eval`` and
   open the folded application witness at ``r_y[1..]`` via the linear inner-product
   argument.

The blinds/masks are drawn from a cryptographically secure :class:`BlindSource`;
the proof is zero-knowledge, so any secure randomness yields a proof the verifier
accepts.  The assembled :class:`VegaMcZkSNARK` is accepted by the Python
verifier and (after serialization) by the Rust verifier.
"""

from dataclasses import dataclass
from typing import List

from .params import Q
from . import commitment, mathutil
from .prover import _matvec, bind_top, eval_uni
from .prover_commit import BlindSource, hyrax_commit
from .prover_polys import unipoly_from_evals
from .polys import eq_evals
from .curve import point_to_transcript
from .proof import (
  VegaMcZkSNARK,
  RelaxedR1CSInstance,
  NovaNIFS,
  RelaxedR1CSSpartanProof,
  HyraxEvaluationArgument,
  InnerProductArgumentLinear,
)

IPA_PROTOCOL_NAME = b"inner product argument (linear)"


# small relaxed instance/witness containers
@dataclass
class _RelaxedInstance:
  comm_W: list  # list of curve points
  comm_E: list
  u: int
  X: List[int]


@dataclass
class _RelaxedWitness:
  W: List[int]
  r_W: List[int]  # per-row blinds
  E: List[int]
  r_E: List[int]


# matrix helpers
def multiply_vec(S, z: List[int]):
  """``(Az, Bz, Cz)`` for an :class:`R1CSShape` (CSR matrices)."""
  return _matvec(S.A, z), _matvec(S.B, z), _matvec(S.C, z)


def _bind_matrix_row_vars(M, rx: List[int], num_cols: int) -> List[int]:
  """``evals[col] = sum_row rx[row] * M[row, col]`` (``bind_matrix_row_vars``)."""
  indptr, indices, data = M.indptr(), M.indices(), M.data()
  evals = [0] * num_cols
  for row in range(len(indptr) - 1):
    w = rx[row]
    if w == 0:
      continue
    for idx in range(indptr[row], indptr[row + 1]):
      col = indices[idx]
      evals[col] = (evals[col] + w * data[idx]) % Q
  return evals


# plaintext sum-check provers
def _compress(coeffs: List[int]) -> List[int]:
  """``UniPoly::compress`` -- drop the linear (index-1) coefficient."""
  return [coeffs[0]] + list(coeffs[2:])


def _sc_cubic_three(tau: List[int], A, B, D, claim: int, tr):
  """``prove_cubic_with_three_inputs``: eq(tau)-weighted ``A*B - D`` (degree 3).

    Returns ``(compressed_polys, r, (A[0], B[0], D[0]))`` after binding.
    """
  eq = eq_evals(tau)
  A, B, D = list(A), list(B), list(D)
  polys: List[List[int]] = []
  r: List[int] = []
  claim_round = claim % Q
  for _ in range(len(tau)):
    m = len(A) // 2
    e0 = e2 = e3 = 0
    for j in range(m):
      deq = (eq[m + j] - eq[j]) % Q
      da = (A[m + j] - A[j]) % Q
      db = (B[m + j] - B[j]) % Q
      dd = (D[m + j] - D[j]) % Q
      eq0, a0, b0, d0 = eq[j], A[j], B[j], D[j]
      e0 = (e0 + eq0 * ((a0 * b0 - d0) % Q)) % Q
      eqt, at, bt, dt = (eq0 + 2 * deq) % Q, (a0 + 2 * da) % Q, (b0 + 2 * db) % Q, (d0 + 2 * dd) % Q
      e2 = (e2 + eqt * ((at * bt - dt) % Q)) % Q
      eqt, at, bt, dt = (eq0 + 3 * deq) % Q, (a0 + 3 * da) % Q, (b0 + 3 * db) % Q, (d0 + 3 * dd) % Q
      e3 = (e3 + eqt * ((at * bt - dt) % Q)) % Q
    e1 = (claim_round - e0) % Q
    coeffs = unipoly_from_evals([e0, e1, e2, e3])
    comp = _compress(coeffs)
    tr.absorb_unipoly(b"p", comp)
    ri = tr.squeeze(b"c")
    r.append(ri)
    polys.append(comp)
    claim_round = eval_uni(coeffs, ri)
    eq, A, B, D = bind_top(eq, ri), bind_top(A, ri), bind_top(B, ri), bind_top(D, ri)
  return polys, r, (A[0], B[0], D[0])


def _sc_quad(claim: int, num_rounds: int, A, B, tr):
  """``prove_quad``: ``sum_y A(y)*B(y)`` (degree 2).  Returns ``(polys, r, (A0,B0))``."""
  A, B = list(A), list(B)
  polys: List[List[int]] = []
  r: List[int] = []
  claim_round = claim % Q
  for _ in range(num_rounds):
    m = len(A) // 2
    e0 = e2 = 0
    for j in range(m):
      da = (A[m + j] - A[j]) % Q
      db = (B[m + j] - B[j]) % Q
      a0, b0 = A[j], B[j]
      e0 = (e0 + a0 * b0) % Q
      at, bt = (a0 + 2 * da) % Q, (b0 + 2 * db) % Q
      e2 = (e2 + at * bt) % Q
    e1 = (claim_round - e0) % Q
    coeffs = unipoly_from_evals([e0, e1, e2])
    comp = _compress(coeffs)
    tr.absorb_unipoly(b"p", comp)
    ri = tr.squeeze(b"c")
    r.append(ri)
    polys.append(comp)
    claim_round = eval_uni(coeffs, ri)
    A, B = bind_top(A, ri), bind_top(B, ri)
  return polys, r, (A[0], B[0])


# Hyrax direct opening
def _prove_direct(vc_ck, poly: List[int], blind_rows: List[int], point: List[int]):
  """``HyraxPCS::prove_direct`` -- RLC of Hyrax rows.  Returns ``(v, combined_blind)``."""
  num_cols = vc_ck.num_cols
  n = 1 << len(point)
  num_rows = mathutil.div_ceil(n, num_cols)
  if num_rows == 1:
    v = list(poly) + [0] * (num_cols - len(poly))
    return v, blind_rows[0] % Q
  num_vars_rows = mathutil.log2(num_rows)
  L = eq_evals(point[:num_vars_rows])  # length num_rows
  padded = list(poly) + [0] * (n - len(poly))
  v = [0] * num_cols
  for row in range(num_rows):
    lr = L[row]
    if lr == 0:
      continue
    base = row * num_cols
    for j in range(num_cols):
      v[j] = (v[j] + lr * padded[base + j]) % Q
  cb = 0
  for row in range(min(len(L), len(blind_rows))):
    cb = (cb + L[row] * blind_rows[row]) % Q
  return v, cb


# Nova fold (fresh random mask)
def sample_random_instance_witness(vk, rng: BlindSource):
  """Deterministically sample a satisfying relaxed instance/witness (ZK mask)."""
  S = vk.vc_shape_regular
  num_vars, num_io, num_cons = S.num_vars, S.num_io, S.num_cons
  width = vk.vc_ck.num_cols
  z_len = num_vars + num_io + 1
  Z = [rng.next() for _ in range(z_len)]
  u = Z[num_vars]
  Az, Bz, Cz = multiply_vec(S, Z)
  E = [(Az[i] * Bz[i] - u * Cz[i]) % Q for i in range(num_cons)]
  r_W = [rng.next() for _ in range(mathutil.div_ceil(num_vars, width))]
  r_E = [rng.next() for _ in range(mathutil.div_ceil(num_cons, width))]
  W = Z[:num_vars]
  comm_W = hyrax_commit(vk.vc_ck.ck, vk.vc_ck.h, W, width, r_W)
  comm_E = hyrax_commit(vk.vc_ck.ck, vk.vc_ck.h, E, width, r_E)
  U = _RelaxedInstance(comm_W=comm_W, comm_E=comm_E, u=u, X=Z[num_vars + 1:])
  Wit = _RelaxedWitness(W=W, r_W=r_W, E=E, r_E=r_E)
  return U, Wit


def nova_nifs_prove(vk, U1, W1, U2_reg, W2, r_W2, tr, rng: BlindSource):
  """``NovaNIFS::prove`` -- fold ``U2`` (verifier circuit) into ``U1`` (random mask).

    Returns ``(NovaNIFS(comm_T), folded_witness, folded_u, folded_X, r)``.
    """
  S = vk.vc_shape_regular
  num_vars, num_io, num_cons = S.num_vars, S.num_io, S.num_cons
  width = vk.vc_ck.num_cols

  tr.absorb_relaxed_instance(
    b"U1",
    commitment.to_points(U1.comm_W),
    commitment.to_points(U1.comm_E),
    U1.u,
    U1.X,
  )
  tr.absorb_r1cs_instance(b"U2", commitment.to_points(U2_reg.comm_W), U2_reg.X)

  # cross-term T (commit_T): Z = (W1+W2, u1+1, X1+X2), u = u1+1
  r_T = [rng.next() for _ in range(mathutil.div_ceil(num_cons, width))]
  Z = (
    [(W1.W[i] + W2[i]) % Q for i in range(num_vars)]
    + [(U1.u + 1) % Q]
    + [(U1.X[j] + U2_reg.X[j]) % Q for j in range(num_io)]
  )
  u_eff = (U1.u + 1) % Q
  Az, Bz, Cz = multiply_vec(S, Z)
  T = [(Az[i] * Bz[i] - u_eff * Cz[i] - W1.E[i]) % Q for i in range(num_cons)]
  comm_T = hyrax_commit(vk.vc_ck.ck, vk.vc_ck.h, T, width, r_T)

  tr.absorb_commitment(b"comm_T", commitment.to_points(comm_T))
  r = tr.squeeze(b"r")

  folded = _RelaxedWitness(
    W=[(W1.W[i] + r * W2[i]) % Q for i in range(num_vars)],
    r_W=[(W1.r_W[k] + r * r_W2[k]) % Q for k in range(len(W1.r_W))],
    E=[(W1.E[i] + r * T[i]) % Q for i in range(num_cons)],
    r_E=[(W1.r_E[k] + r * r_T[k]) % Q for k in range(len(r_T))],
  )
  folded_u = (U1.u + r) % Q
  folded_X = [(U1.X[j] + r * U2_reg.X[j]) % Q for j in range(num_io)]
  return NovaNIFS(comm_T=comm_T), folded, folded_u, folded_X, r


# relaxed R1CS Spartan (non-ZK, direct openings)
def relaxed_spartan_prove(vk, folded_u: int, folded_X: List[int], W, tr):
  """``RelaxedR1CSSpartanProof::prove`` on the folded verifier-circuit instance."""
  S = vk.vc_shape_regular
  num_cons, num_vars, num_io = S.num_cons, S.num_vars, S.num_io
  num_rounds_x = mathutil.log2(num_cons)
  num_vars_padded = mathutil.next_power_of_two(num_vars)
  num_rounds_y = mathutil.log2(num_vars_padded) + 1
  z_len = num_vars_padded * 2

  tr.absorb_scalar(b"u_relaxed", folded_u)
  tr.absorb_scalars(b"X_relaxed", folded_X)

  z_unpadded = list(W.W) + [folded_u] + list(folded_X)
  Az, Bz, Cz = multiply_vec(S, z_unpadded)

  tau = [tr.squeeze(b"t") for _ in range(num_rounds_x)]
  uCzE = [(folded_u * Cz[i] + W.E[i]) % Q for i in range(num_cons)]

  sc_outer, r_x, (claim_Az, claim_Bz, claim_uCzE) = _sc_cubic_three(
    tau, Az, Bz, uCzE, 0, tr
  )
  tr.absorb_scalars(b"claims_outer", [claim_Az, claim_Bz, claim_uCzE])

  r = tr.squeeze(b"r")
  r_sq = (r * r) % Q
  evals_rx = eq_evals(r_x)  # length num_cons
  claim_E = 0
  for i in range(num_cons):
    claim_E = (claim_E + W.E[i] * evals_rx[i]) % Q
  claim_inner_joint = (claim_Az + r * claim_Bz + r_sq * ((claim_uCzE - claim_E) % Q)) % Q

  num_cols = num_vars + 1 + num_io
  evals_A = _bind_matrix_row_vars(S.A, evals_rx, num_cols)
  evals_B = _bind_matrix_row_vars(S.B, evals_rx, num_cols)
  evals_C = _bind_matrix_row_vars(S.C, evals_rx, num_cols)
  poly_ABC = [
    (evals_A[j] + r * evals_B[j] + (r_sq * folded_u % Q) * evals_C[j]) % Q
    for j in range(num_cols)
  ]
  poly_ABC += [0] * (z_len - num_cols)
  poly_z = z_unpadded + [0] * (z_len - len(z_unpadded))

  sc_inner, r_y, _ = _sc_quad(claim_inner_joint, num_rounds_y, poly_ABC, poly_z, tr)

  v_W, blind_W = _prove_direct(vk.vc_ck, W.W, W.r_W, r_y[1:])
  v_E, blind_E = _prove_direct(vk.vc_ck, W.E, W.r_E, r_x)
  tr.absorb_scalars(b"v_W", v_W)
  tr.absorb_scalars(b"v_E", v_E)

  return RelaxedR1CSSpartanProof(
    sc_proof_outer=sc_outer,
    claims_outer=(claim_Az, claim_Bz, claim_uCzE),
    sc_proof_inner=sc_inner,
    v_W=v_W,
    blind_W=blind_W,
    v_E=v_E,
    blind_E=blind_E,
  )


# ZK linear inner-product argument
def _ipa_prove(ck, h, ck_c, h_c, comm_a_vec, b_vec, comm_c, a_vec, r_a, r_c, tr, rng):
  """``InnerProductArgumentLinear::prove`` (linear sigma protocol)."""
  tr.dom_sep(IPA_PROTOCOL_NAME)
  # instance absorb: comm_a_vec || comm_c (b_vec omitted by design)
  tr.absorb_raw(b"U", point_to_transcript(comm_a_vec) + point_to_transcript(comm_c))

  d_vec = [rng.next() for _ in range(len(b_vec))]
  r_delta = rng.next()
  r_beta = rng.next()

  delta = commitment.msm(d_vec, ck[: len(d_vec)]) + (r_delta % Q) * commitment.to_point(h)
  bd = 0
  for bi, di in zip(b_vec, d_vec):
    bd = (bd + bi * di) % Q
  beta = bd * commitment.to_point(ck_c) + (r_beta % Q) * commitment.to_point(h_c)

  tr.absorb_point(b"delta", delta)
  tr.absorb_point(b"beta", beta)
  r = tr.squeeze(b"r")

  z_vec = [(r * a_vec[i] + d_vec[i]) % Q for i in range(len(d_vec))]
  z_delta = (r * r_a + r_delta) % Q
  z_beta = (r * r_c + r_beta) % Q
  return InnerProductArgumentLinear(
    delta=delta, beta=beta, z_vec=z_vec, z_delta=z_delta, z_beta=z_beta
  )


# orchestrator
def prove_finish(vk, core) -> VegaMcZkSNARK:
  """Complete the proof from a :class:`ProverCoreResult`; return a VegaMcZkSNARK."""
  rng = BlindSource()
  tr = core.transcript

  # (1) Nova fold: fresh random mask, then fold in the verifier-circuit instance.
  random_U, random_W = sample_random_instance_witness(vk, rng)
  r_W2 = [b for row in core.r_w_per_round for b in row]  # flattened per-round blinds
  U2_reg = _regular_from_multiround(core.U_verifier)
  nifs, folded_VC, folded_u, folded_X, _r = nova_nifs_prove(
    vk, random_U, random_W, U2_reg, core.W_verifier, r_W2, tr, rng
  )

  # (2) Relaxed R1CS Spartan on the folded verifier-circuit instance.
  relaxed_snark = relaxed_spartan_prove(vk, folded_u, folded_X, folded_VC, tr)

  # (3) Fold the two witness-eval claims with c_eval and open via the ZK IPA.
  idx = core.cfg.idx_commit_w_step
  comm_eval_W_step = core.comm_w_per_round[idx]
  comm_eval_W_core = core.comm_w_per_round[idx + 1]
  blind_eval_W_step = core.r_w_per_round[idx][0]
  blind_eval_W_core = core.r_w_per_round[idx + 1][0]

  c_eval = tr.squeeze(b"c_eval")

  comm = commitment.fold(
    [core.folded_U.comm_W, core.core_instance_regular.comm_W], [1, c_eval]
  )
  blind = (core.folded_W_rW + c_eval * core.core_blind) % Q
  W = [(core.folded_W[i] + c_eval * core.core_W[i]) % Q for i in range(len(core.folded_W))]
  comm_eval = commitment.fold([comm_eval_W_step, comm_eval_W_core], [1, c_eval])
  blind_eval = (blind_eval_W_step + c_eval * blind_eval_W_core) % Q

  point = core.r_y[1:]
  n = 1 << len(point)
  if n != len(W):
    raise ValueError(f"IPA: len(W)={len(W)} != 2^{len(point)}={n}")
  num_cols = vk.ck.num_cols
  if mathutil.div_ceil(n, num_cols) != 1:
    raise NotImplementedError("IPA prover currently supports single-row app commitments")

  tr.absorb_commitment(b"poly_com", commitment.to_points(comm))
  R = eq_evals(point)  # b_vec, length n
  comm_LZ = commitment.to_point(comm[0])
  ipa = _ipa_prove(
    vk.ck.ck,
    vk.ck.h,
    vk.vc_ck.ck[0],
    vk.vc_ck.h,
    comm_LZ,
    R,
    commitment.to_point(comm_eval[0]),
    W,
    blind,
    blind_eval,
    tr,
    rng,
  )
  eval_arg = HyraxEvaluationArgument(ipa=ipa)

  # (4) Assemble the proof; hoist the shared commitment (None for cubic: unshared).
  comm_W_shared = core.step_instances[0].comm_W_shared
  step_instances = [_strip_shared(u) for u in core.step_instances]
  core_instance = _strip_shared(core.core_instance)

  random_U_wire = RelaxedR1CSInstance(
    comm_W=random_U.comm_W, comm_E=random_U.comm_E, X=random_U.X, u=random_U.u
  )

  return VegaMcZkSNARK(
    comm_W_shared=comm_W_shared,
    step_instances=step_instances,
    core_instance=core_instance,
    eval_arg=eval_arg,
    U_verifier=core.U_verifier,
    nifs=nifs,
    random_U=random_U_wire,
    relaxed_snark=relaxed_snark,
  )


def _regular_from_multiround(U):
  """``SplitMultiRoundR1CSInstance::to_regular_instance`` (X = challenges || public)."""
  from .instance import multiround_to_regular

  return multiround_to_regular(U)


def _strip_shared(u):
  from dataclasses import replace

  return replace(u, comm_W_shared=None)
