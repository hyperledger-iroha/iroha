"""Stand-alone Vega-MC prover core: NeutronNova NIFS + batched Spartan sum-checks.

This is the counterpart to :mod:`pyvega.verify`.  It reproduces -- *without any of
the production optimizations* -- the exact Fiat--Shamir schedule and algebra of
``VegaMcZkSNARK::prove`` (``src/vega_mc_zkp.rs``) up to and including the assembly
of the in-circuit verifier witness ``W_verifier`` and its multi-round instance
``U_verifier``.  The remaining stages (Nova fold of a fresh random instance,
relaxed-R1CS Spartan, and the ZK linear-IPA opening) are layered on top in
:mod:`pyvega.prover_finish`.

Design notes:

* The sum-check round polynomials are the *unique honest* polynomials, so a naive
  full-table computation reproduces the optimized prover's committed coefficients
  byte-for-byte.  We therefore keep whole multilinear tables and bind them
  high-bit-first (``bind_poly_var_top``) each round.
* The verifier circuit is emitted **round by round** (``verifier_circuit.emit_round``)
  because each round's Fiat--Shamir challenge is squeezed from the commitment to
  *that* round's witness slice and feeds the next round.  Value computation,
  circuit emission and commit/absorb/squeeze therefore run in one interleaved loop.

The NeutronNova round-poly identity (``vega_mc_zkp.rs`` ``finish_round!``):
  ``g_t(X) = eq(rho_t, X) * (c + b*X + a*X^2)`` with ``c = e0*acc_eq``,
  ``a = quad_coeff*acc_eq``, ``b = (T_cur - c*(1-rho))/rho - a - c``; where
  ``e0 = sum_x pow(tau,x)*(Az1*Bz1 - Cz1)`` (0 in round 0, strict R1CS) and
  ``quad_coeff = sum_x pow(tau,x)*(Az2-Az1)*(Bz2-Bz1)`` over the even/odd instance
  split, weighted by the not-yet-processed instance bits.
"""

from dataclasses import dataclass
from typing import Dict, List

from .params import Q
from . import commitment, app_circuit, mathutil
from .transcript import Transcript
from .prover_polys import inv, unipoly_from_evals
from .prover_commit import BlindSource, hyrax_commit
from .instance import (
  DEFAULT_COMMITMENT_WIDTH,
  weights_from_r,
  fold_multiple,
  split_to_regular,
)
from .proof import SplitR1CSInstance, SplitMultiRoundR1CSInstance
from .verifier_circuit import (
  VcConfig,
  VcValues,
  VerifierCircuit,
  zero_values,
  emit_round,
)
from .polys import eq_evals, sparse_poly_evaluate

PROVE_LABEL = b"neutronnova_prove"


# small polynomial / table helpers
def eval_uni(coeffs: List[int], x: int) -> int:
  """Evaluate a univariate poly (coeffs ascending: ``[c0, c1, ...]``) at ``x``."""
  acc = 0
  for c in reversed(coeffs):
    acc = (acc * x + c) % Q
  return acc


def bind_top(table: List[int], r: int) -> List[int]:
  """``bind_poly_var_top``: fold the MSB variable, ``Z'[j]=Z[j]+r*(Z[m+j]-Z[j])``."""
  m = len(table) // 2
  return [(table[j] + r * (table[m + j] - table[j])) % Q for j in range(m)]


def _outer_round_poly(pow_t, Az, Bz, Cz, claim: int) -> List[int]:
  """Cubic outer round poly: ``sum_x pow(tau,x)*(Az*Bz - Cz)`` as 4 coeffs."""
  m = len(Az) // 2

  def gt(t: int) -> int:
    s = 0
    for j in range(m):
      p = (pow_t[j] + t * (pow_t[m + j] - pow_t[j])) % Q
      a = (Az[j] + t * (Az[m + j] - Az[j])) % Q
      b = (Bz[j] + t * (Bz[m + j] - Bz[j])) % Q
      c = (Cz[j] + t * (Cz[m + j] - Cz[j])) % Q
      s = (s + p * ((a * b - c) % Q)) % Q
    return s

  g0, g1, g2, g3 = gt(0), gt(1), gt(2), gt(3)
  if (g0 + g1) % Q != claim % Q:
    raise ValueError("outer sum-check invariant g(0)+g(1)=claim violated")
  return unipoly_from_evals([g0, g1, g2, g3])


def _inner_round_poly(ABC, z, claim: int) -> List[int]:
  """Quadratic inner round poly: ``sum_y ABC(y)*z(y)`` as 3 coeffs."""
  m = len(ABC) // 2

  def gt(t: int) -> int:
    s = 0
    for j in range(m):
      a = (ABC[j] + t * (ABC[m + j] - ABC[j])) % Q
      b = (z[j] + t * (z[m + j] - z[j])) % Q
      s = (s + a * b) % Q
    return s

  g0, g1, g2 = gt(0), gt(1), gt(2)
  if (g0 + g1) % Q != claim % Q:
    raise ValueError("inner sum-check invariant g(0)+g(1)=claim violated")
  return unipoly_from_evals([g0, g1, g2])


def _matvec(M, z: List[int]) -> List[int]:
  """``M z`` for a CSR ``SparseMatrixRaw`` (length = number of rows)."""
  indptr, indices, data = M.indptr(), M.indices(), M.data()
  out = [0] * (len(indptr) - 1)
  for row in range(len(indptr) - 1):
    acc = 0
    for idx in range(indptr[row], indptr[row + 1]):
      acc = (acc + data[idx] * z[indices[idx]]) % Q
    out[row] = acc
  return out


def _compute_poly_ABC(S, eq_rx: List[int], r: int, num_vars: int) -> List[int]:
  """``poly_ABC[y] = sum_x eq(r_x,x)*(A + r*B + r^2*C)[x,y]`` (length ``2*num_vars``)."""
  out = [0] * (2 * num_vars)
  r2 = (r * r) % Q
  for M, coeff in ((S.A, 1), (S.B, r), (S.C, r2)):
    indptr, indices, data = M.indptr(), M.indices(), M.data()
    for row in range(len(indptr) - 1):
      e = eq_rx[row]
      if e == 0:
        continue
      ce = (coeff * e) % Q
      for idx in range(indptr[row], indptr[row + 1]):
        out[indices[idx]] = (out[indices[idx]] + ce * data[idx]) % Q
  return out


def _build_z(W: List[int], X: List[int], num_vars: int) -> List[int]:
  """Assemble the inner-sum-check ``z = [W | 1 | X]`` padded to ``2*num_vars``."""
  v = [0] * (2 * num_vars)
  for i, wi in enumerate(W):
    v[i] = wi % Q
  v[len(W)] = 1
  for i, xi in enumerate(X):
    v[len(W) + 1 + i] = xi % Q
  return v


# result bundle
@dataclass
class ProverCoreResult:
  cfg: VcConfig
  vals: VcValues
  circuit: VerifierCircuit
  U_verifier: SplitMultiRoundR1CSInstance
  W_verifier: List[int]
  comm_w_per_round: List[list]
  r_w_per_round: List[List[int]]
  transcript: Transcript
  # app-side objects the finishing stages need
  folded_W: List[int]
  folded_U: object
  folded_W_rW: int
  core_W: List[int]
  core_blind: int
  core_instance_regular: object
  step_instances: List[SplitR1CSInstance]
  core_instance: SplitR1CSInstance
  r_y: List[int]
  challenges: List[int]


# the prover core
def prove_core(vk, num_steps: int = 2) -> ProverCoreResult:
  """Run NIFS + outer + inner sum-checks; assemble and return ``W_verifier``.

  The application statement is the fixed cubic ``x^3 + x + 5 = y`` with ``x = 2``.
  ``num_steps`` identical step instances (distinct blinds) plus one core instance
  are folded.  Blinds come from a cryptographically secure :class:`BlindSource`.
  """
  blinds = BlindSource()
  digest = vk.digest()
  S_step, S_core = vk.S_step, vk.S_core
  num_vars = S_step.num_vars

  # (0) build the app instances (all cubic, distinct blinds)
  cw = app_circuit.cubic_witness()
  w_vec = list(cw.W)  # witness segment, length num_vars
  z_app = app_circuit.z_vector(cw)
  if not app_circuit.is_sat(S_step, z_app):
    raise ValueError("cubic witness does not satisfy S_step")
  ck = vk.ck

  app_blinds: List[int] = []  # one blind per committed app instance (single row)

  def _commit_app() -> SplitR1CSInstance:
    b = blinds.next()
    app_blinds.append(b)
    comm_rest = hyrax_commit(ck.ck, ck.h, w_vec, DEFAULT_COMMITMENT_WIDTH, [b])
    return SplitR1CSInstance(
      comm_W_shared=None,
      comm_W_precommitted=None,
      comm_W_rest=comm_rest,
      public_values=list(cw.public_values),
      challenges=[],
    )

  step_instances = [_commit_app() for _ in range(num_steps)]
  core_instance = _commit_app()
  step_blinds = app_blinds[:num_steps]  # one per step instance
  core_blind = app_blinds[num_steps]  # the core instance blind
  step_reg = [split_to_regular(u) for u in step_instances]
  core_reg = split_to_regular(core_instance)

  # (config)
  n_padded = mathutil.next_power_of_two(num_steps)
  num_rounds_b = mathutil.log2(n_padded)
  num_rounds_x = mathutil.log2(S_step.num_cons)
  num_rounds_y = mathutil.log2(num_vars) + 1
  vcs = vk.vc_shape
  cfg = VcConfig(num_rounds_b, num_rounds_x, num_rounds_y, vcs.commitment_width)
  if num_rounds_b != 1:
    raise NotImplementedError("prover core currently supports a single NIFS round")

  # (Phase B) main transcript
  tr = Transcript(PROVE_LABEL)
  tr.absorb_raw(b"vk", digest)
  tr.absorb_r1cs_instance(
    b"core_instance", commitment.to_points(core_reg.comm_W), core_reg.X
  )
  step_reg_padded = list(step_reg)
  while len(step_reg_padded) < n_padded:
    step_reg_padded.append(step_reg[0])
  step_blinds_padded = list(step_blinds)
  while len(step_blinds_padded) < n_padded:
    step_blinds_padded.append(step_blinds[0])
  for U in step_reg_padded:
    tr.absorb_r1cs_instance(b"U", commitment.to_points(U.comm_W), U.X)
  tr.absorb_scalar(b"T", 0)
  tau = tr.squeeze(b"tau")
  rhos = [tr.squeeze(b"rho") for _ in range(num_rounds_b)]

  # NIFS layers (Az/Bz/Cz per padded step instance)
  step_W = [w_vec for _ in range(n_padded)]  # all identical for cubic
  layers = []  # (Az, Bz, Cz) per instance
  for b in range(n_padded):
    z_b = app_circuit.z_vector(cw)
    layers.append(app_circuit.multiply_vec(S_step, z_b))
  pow_full = [pow(tau, k, Q) for k in range(S_step.num_cons)]

  # (main interleaved loop)
  c = VerifierCircuit(cfg)
  c._challenges = []
  prior: List = [None] * cfg.num_rounds
  prev_chal: List = [None] * cfg.num_rounds
  vals = zero_values(cfg)
  st: Dict = {}
  comm_w_per_round: List[list] = []
  r_w_per_round: List[List[int]] = []
  challenges_per_round: List[List[int]] = []

  def _commit_round(ri: int) -> List[int]:
    padded_len = vcs.num_vars_per_round[ri]
    sl = list(c.W_rounds[ri]) + [0] * (padded_len - len(c.W_rounds[ri]))
    nrows = (padded_len + cfg.width - 1) // cfg.width if padded_len else 0
    row_blinds = [blinds.next() for _ in range(nrows)]
    comm = hyrax_commit(vk.vc_ck.ck, vk.vc_ck.h, sl, cfg.width, row_blinds)
    comm_w_per_round.append(comm)
    r_w_per_round.append(row_blinds)
    tr.absorb_commitment(b"comm_w_round", comm)
    chals = [tr.squeeze(b"challenge") for _ in range(vcs.num_challenges_per_round[ri])]
    challenges_per_round.append(chals)
    return chals

  def _prepare(ri: int) -> None:
    if ri < cfg.num_rounds_b:  # NIFS round (single round for cubic)
      quad = 0
      for k in range(S_step.num_cons):
        da = (layers[1][0][k] - layers[0][0][k]) % Q
        db = (layers[1][1][k] - layers[0][1][k]) % Q
        quad = (quad + pow_full[k] * ((da * db) % Q)) % Q
      rho = rhos[ri]
      omr = (1 - rho) % Q
      trm1 = (rho - omr) % Q
      cc = 0  # e0 * acc_eq, e0 = 0 in round 0
      a = quad % Q  # acc_eq = 1 in round 0
      abc = ((0 - cc * omr) * inv(rho)) % Q  # (T_cur - c*(1-rho))/rho, T_cur=0
      b = (abc - a - cc) % Q
      poly = [
        (cc * omr) % Q,
        (cc * trm1 + b * omr) % Q,
        (b * trm1 + a * omr) % Q,
        (a * trm1) % Q,
      ]
      vals.nifs_polys[ri] = poly
      st["nifs_poly"] = poly
      st["rho"] = rho
    elif ri == cfg.idx_nifs_final:
      vals.t_out_step = st["t_out_step"]
      vals.eq_rho_at_rb = st["eq_rho"]
    elif cfg.idx_nifs_final < ri < cfg.idx_outer_final:
      i = ri - cfg.idx_outer_start
      vals.outer_polys_step[i] = _outer_round_poly(
        st["pow"], st["Az_s"], st["Bz_s"], st["Cz_s"], st["claim_step"]
      )
      vals.outer_polys_core[i] = _outer_round_poly(
        st["pow"], st["Az_c"], st["Bz_c"], st["Cz_c"], st["claim_core"]
      )
    elif ri == cfg.idx_outer_final:
      vals.claim_Az_step, vals.claim_Bz_step, vals.claim_Cz_step = (
        st["Az_s"][0], st["Bz_s"][0], st["Cz_s"][0]
      )
      vals.claim_Az_core, vals.claim_Bz_core, vals.claim_Cz_core = (
        st["Az_c"][0], st["Bz_c"][0], st["Cz_c"][0]
      )
      vals.tau_at_rx = st["pow"][0]
    elif cfg.idx_inner_start <= ri < cfg.idx_inner_final:
      j = ri - cfg.idx_inner_start
      vals.inner_polys_step[j] = _inner_round_poly(
        st["ABC_s"], st["z_s"], st["claim_step"]
      )
      vals.inner_polys_core[j] = _inner_round_poly(
        st["ABC_c"], st["z_c"], st["claim_core"]
      )
    elif ri == cfg.idx_inner_final:
      r_y = st["r_y"]
      eval_Z_s, eval_Z_c = st["z_s"][0], st["z_c"][0]
      nv_log2 = mathutil.ilog2(num_vars)
      eval_X_s = sparse_poly_evaluate(nv_log2, [1] + list(st["folded_U"].X), r_y[1:])
      eval_X_c = sparse_poly_evaluate(nv_log2, [1] + list(core_reg.X), r_y[1:])
      inv1 = inv((1 - r_y[0]) % Q)
      vals.eval_W_step = ((eval_Z_s - r_y[0] * eval_X_s) * inv1) % Q
      vals.eval_W_core = ((eval_Z_c - r_y[0] * eval_X_c) * inv1) % Q
      vals.eval_X_step = eval_X_s % Q
      vals.eval_X_core = eval_X_c % Q
    # commit rounds (idx_commit_w_step/core) read eval_W already set -- no prep

  def _route(ri: int, chals: List[int]) -> None:
    if ri == cfg.num_rounds_b - 1:  # last NIFS round -> r_b, then fold
      r_b0 = chals[0]
      rho = st["rho"]
      T_cur = eval_uni(st["nifs_poly"], r_b0)
      acc_eq = ((1 - r_b0) * (1 - rho) + r_b0 * rho) % Q
      st["t_out_step"] = (T_cur * inv(acc_eq)) % Q
      st["eq_rho"] = acc_eq
      w = weights_from_r([r_b0], n_padded)
      st["Az_s"] = [sum(w[b] * layers[b][0][k] for b in range(n_padded)) % Q
                    for k in range(S_step.num_cons)]
      st["Bz_s"] = [sum(w[b] * layers[b][1][k] for b in range(n_padded)) % Q
                    for k in range(S_step.num_cons)]
      st["Cz_s"] = [sum(w[b] * layers[b][2][k] for b in range(n_padded)) % Q
                    for k in range(S_step.num_cons)]
      folded_W = [sum(w[b] * step_W[b][i] for b in range(n_padded)) % Q
                  for i in range(num_vars)]
      folded_W_rW = sum(w[b] * step_blinds_padded[b] for b in range(n_padded)) % Q
      folded_U = fold_multiple([r_b0], step_reg_padded)
      z_core = app_circuit.z_vector(app_circuit.cubic_witness())
      Az_c, Bz_c, Cz_c = app_circuit.multiply_vec(S_core, z_core)
      st.update(
        pow=list(pow_full), Az_c=Az_c, Bz_c=Bz_c, Cz_c=Cz_c,
        folded_W=folded_W, folded_U=folded_U, folded_W_rW=folded_W_rW,
        claim_step=st["t_out_step"], claim_core=0, r_x=[],
      )
    elif cfg.idx_nifs_final < ri < cfg.idx_outer_final:  # outer round -> r_x
      r = chals[0]
      st["r_x"].append(r)
      i = ri - cfg.idx_outer_start
      st["claim_step"] = eval_uni(vals.outer_polys_step[i], r)
      st["claim_core"] = eval_uni(vals.outer_polys_core[i], r)
      for key in ("pow", "Az_s", "Bz_s", "Cz_s", "Az_c", "Bz_c", "Cz_c"):
        st[key] = bind_top(st[key], r)
    elif ri == cfg.idx_outer_final:  # -> r_batch, set up inner sum-check
      r = chals[0]
      st["r_batch"] = r
      cs = (vals.claim_Az_step + r * vals.claim_Bz_step + r * r * vals.claim_Cz_step) % Q
      cc = (vals.claim_Az_core + r * vals.claim_Bz_core + r * r * vals.claim_Cz_core) % Q
      eq_rx = eq_evals(st["r_x"])
      st["ABC_s"] = _compute_poly_ABC(S_step, eq_rx, r, num_vars)
      st["ABC_c"] = _compute_poly_ABC(S_core, eq_rx, r, num_vars)
      st["z_s"] = _build_z(st["folded_W"], st["folded_U"].X, num_vars)
      st["z_c"] = _build_z(w_vec, core_reg.X, num_vars)
      st.update(claim_step=cs, claim_core=cc, r_y=[])
    elif cfg.idx_inner_start <= ri < cfg.idx_inner_final:  # inner round -> r_y
      r = chals[0]
      st["r_y"].append(r)
      j = ri - cfg.idx_inner_start
      st["claim_step"] = eval_uni(vals.inner_polys_step[j], r)
      st["claim_core"] = eval_uni(vals.inner_polys_core[j], r)
      for key in ("ABC_s", "z_s", "ABC_c", "z_c"):
        st[key] = bind_top(st[key], r)

  for ri in range(cfg.num_rounds):
    _prepare(ri)
    emit_round(c, cfg, vals, ri, prior, prev_chal)
    chals = _commit_round(ri)
    c._challenges.extend(chals)
    _route(ri, chals)

  # (assembly)
  num_challenges = cfg.num_challenges
  challenges = c.io_vals[:num_challenges]
  public_values = c.io_vals[num_challenges:]
  U_verifier = SplitMultiRoundR1CSInstance(
    comm_w_per_round=comm_w_per_round,
    public_values=public_values,
    challenges_per_round=challenges_per_round,
  )
  W_verifier: List[int] = []
  for ri in range(cfg.num_rounds):
    W_verifier.extend(c.W_rounds[ri])
    W_verifier.extend([0] * (cfg.width - len(c.W_rounds[ri])))

  return ProverCoreResult(
    cfg=cfg,
    vals=vals,
    circuit=c,
    U_verifier=U_verifier,
    W_verifier=W_verifier,
    comm_w_per_round=comm_w_per_round,
    r_w_per_round=r_w_per_round,
    transcript=tr,
    folded_W=st["folded_W"],
    folded_U=st["folded_U"],
    folded_W_rW=st["folded_W_rW"],
    core_W=w_vec,
    core_blind=core_blind,
    core_instance_regular=core_reg,
    step_instances=step_instances,
    core_instance=core_instance,
    r_y=st["r_y"],
    challenges=challenges,
  )
