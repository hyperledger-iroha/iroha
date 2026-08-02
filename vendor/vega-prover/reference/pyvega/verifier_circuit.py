"""Reproduction of ``VegaMcVerifierCircuit`` (src/zk.rs) as a small R1CS emitter.

The in-circuit NeutronNova/Spartan verifier is a *fixed* circuit determined by
``(num_rounds_b, num_rounds_x, num_rounds_y, width)``.  This module mirrors
``zk.rs`` ``rounds()`` and its gadgets exactly, emitting -- in lock-step --

  * the R1CS constraint matrices ``A, B, C`` (used by setup to build ``vc_shape``
    / ``vc_shape_regular``), and
  * the witness ``W`` (per round, later zero-padded to ``width``) together with the
    public IO vector ``X = [challenges .. , public_values ..]`` (used by prove).

Both are functions only of the round structure and the supplied *values*; the
column layout is value-independent, so ``build(cfg, zero_values(cfg))`` yields the
shape and ``build(cfg, real_values)`` yields the witness.

Variable/column convention (matches the Rust CSR dump):
  ``z = [ W(0 .. num_vars-1) , ONE(num_vars) , IO(num_vars+1 .. num_vars+num_io) ]``
  and round ``r`` occupies witness columns ``[width*r , width*r + width)`` (its
  first ``nvpr_unpadded[r]`` columns are real vars; the rest are free padding).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, List

from .params import Q


# configuration
@dataclass
class VcConfig:
  num_rounds_b: int
  num_rounds_x: int
  num_rounds_y: int
  width: int

  @property
  def num_rounds(self) -> int:
    return self.num_rounds_b + self.num_rounds_x + self.num_rounds_y + 5

  @property
  def num_vars(self) -> int:
    return self.num_rounds * self.width

  @property
  def num_challenges(self) -> int:
    return self.num_rounds_b + self.num_rounds_x + 1 + self.num_rounds_y

  @property
  def num_public(self) -> int:
    return 6

  @property
  def num_io(self) -> int:
    return self.num_challenges + self.num_public

  # round-index landmarks (mirror zk.rs)
  @property
  def idx_nifs_final(self) -> int:
    return self.num_rounds_b

  @property
  def idx_outer_start(self) -> int:
    return self.idx_nifs_final + 1

  @property
  def idx_outer_final(self) -> int:
    return self.idx_outer_start + self.num_rounds_x

  @property
  def idx_inner_start(self) -> int:
    return self.idx_outer_final + 1

  @property
  def idx_inner_final(self) -> int:
    return self.idx_inner_start + self.num_rounds_y

  @property
  def idx_commit_w_step(self) -> int:
    return self.idx_inner_final + 1

  @property
  def idx_commit_w_core(self) -> int:
    return self.idx_commit_w_step + 1


# values fed to the circuit
@dataclass
class VcValues:
  """All prover-side values the verifier circuit consumes, in native ints mod Q."""

  nifs_polys: List[List[int]]  # num_rounds_b x 4
  eq_rho_at_rb: int
  t_out_step: int
  outer_polys_step: List[List[int]]  # num_rounds_x x 4
  outer_polys_core: List[List[int]]
  claim_Az_step: int
  claim_Bz_step: int
  claim_Cz_step: int
  claim_Az_core: int
  claim_Bz_core: int
  claim_Cz_core: int
  tau_at_rx: int
  inner_polys_step: List[List[int]]  # num_rounds_y x 3
  inner_polys_core: List[List[int]]
  eval_W_step: int
  eval_W_core: int
  eval_X_step: int
  eval_X_core: int
  quotient_step: int
  quotient_core: int
  # challenges, in allocation order: r_b(nb), r_x(nx), r_batch(1), r_y(ny)
  challenges: List[int]


def zero_values(cfg: VcConfig) -> VcValues:
  """Placeholder values (all zero) sufficient to emit the constraint shape."""
  return VcValues(
    nifs_polys=[[0, 0, 0, 0] for _ in range(cfg.num_rounds_b)],
    eq_rho_at_rb=0,
    t_out_step=0,
    outer_polys_step=[[0, 0, 0, 0] for _ in range(cfg.num_rounds_x)],
    outer_polys_core=[[0, 0, 0, 0] for _ in range(cfg.num_rounds_x)],
    claim_Az_step=0,
    claim_Bz_step=0,
    claim_Cz_step=0,
    claim_Az_core=0,
    claim_Bz_core=0,
    claim_Cz_core=0,
    tau_at_rx=0,
    inner_polys_step=[[0, 0, 0] for _ in range(cfg.num_rounds_y)],
    inner_polys_core=[[0, 0, 0] for _ in range(cfg.num_rounds_y)],
    eval_W_step=0,
    eval_W_core=0,
    eval_X_step=0,
    eval_X_core=0,
    quotient_step=0,
    quotient_core=0,
    challenges=[0] * cfg.num_challenges,
  )


# a variable handle
@dataclass
class Var:
  col: int
  val: int


# the emitter
class VerifierCircuit:
  """Mirrors zk.rs allocation/enforcement order to emit (A,B,C, W, X)."""

  def __init__(self, cfg: VcConfig):
    self.cfg = cfg
    self.one_col = cfg.num_vars
    self.io_base = cfg.num_vars + 1
    # constraint rows: each is a dict {col: coeff}
    self.A: List[Dict[int, int]] = []
    self.B: List[Dict[int, int]] = []
    self.C: List[Dict[int, int]] = []
    # witness, grouped per round (each later padded to width)
    self.W_rounds: List[List[int]] = [[] for _ in range(cfg.num_rounds)]
    self.io_vals: List[int] = []
    self._cur_round = 0
    self._chal_ptr = 0
    self._challenges: List[int] = []

  # primitive allocation
  def one(self) -> Var:
    return Var(self.one_col, 1)

  def alloc(self, val: int) -> Var:
    r = self._cur_round
    local = len(self.W_rounds[r])
    col = self.cfg.width * r + local
    self.W_rounds[r].append(val % Q)
    return Var(col, val % Q)

  def alloc_input(self, val: int) -> Var:
    col = self.io_base + len(self.io_vals)
    self.io_vals.append(val % Q)
    return Var(col, val % Q)

  def _lc(self, terms) -> Dict[int, int]:
    d: Dict[int, int] = {}
    for handle, coeff in terms:
      col = handle.col if isinstance(handle, Var) else handle
      d[col] = (d.get(col, 0) + coeff) % Q
    return {c: v for c, v in d.items() if v != 0}

  def enforce(self, a_terms, b_terms, c_terms) -> None:
    self.A.append(self._lc(a_terms))
    self.B.append(self._lc(b_terms))
    self.C.append(self._lc(c_terms))

  def next_challenge(self) -> int:
    v = self._challenges[self._chal_ptr]
    self._chal_ptr += 1
    return v

  # gadgets (mirror zk.rs)
  def g_alloc_zero(self) -> Var:
    z = self.alloc(0)
    self.enforce([(z, 1)], [(self.one(), 1)], [])
    return z

  def g_alloc_coeffs(self, coeffs: List[int]) -> List[Var]:
    return [self.alloc(c) for c in coeffs]

  def g_mul(self, a: Var, b: Var) -> Var:
    p = self.alloc((a.val * b.val) % Q)
    self.enforce([(a, 1)], [(b, 1)], [(p, 1)])
    return p

  def g_eval_poly_horner(self, coeffs: List[Var], x: Var) -> Var:
    acc = coeffs[-1]
    for c_i in reversed(coeffs[:-1]):
      new_acc = self.alloc((acc.val * x.val + c_i.val) % Q)
      # acc * x = new_acc - c_i
      self.enforce([(acc, 1)], [(x, 1)], [(new_acc, 1), (c_i, -1)])
      acc = new_acc
    return acc

  def g_enforce_sc_claim(self, poly: List[Var], claim: Var) -> None:
    a_terms = [(p, 1) for p in poly] + [(poly[0], 1)]
    self.enforce(a_terms, [(self.one(), 1)], [(claim, 1)])

  def g_enforce_outer_final(self, Az, Bz, Cz, tau, prev) -> None:
    prod = self.g_mul(Az, Bz)
    # tau * (prod - Cz) = prev
    self.enforce([(tau, 1)], [(prod, 1), (Cz, -1)], [(prev, 1)])

  def g_compute_joint_claim(self, Az, Bz, Cz, r, r_sq) -> Var:
    r_times_Bz = self.g_mul(r, Bz)
    joint = self.alloc((Az.val + r_times_Bz.val + r_sq.val * Cz.val) % Q)
    # Cz * r_sq = joint - Az - r_times_Bz
    self.enforce([(Cz, 1)], [(r_sq, 1)], [(joint, 1), (Az, -1), (r_times_Bz, -1)])
    return joint

  def g_inputize(self, x: Var) -> Var:
    xi = self.alloc_input(x.val)
    # bellpepper inputize: input * 1 = variable
    self.enforce([(xi, 1)], [(self.one(), 1)], [(x, 1)])
    return xi

  def g_enforce_inner_final(self, r_y0, eval_W, eval_X, prev) -> None:
    one = self.one()
    tmp_w = self.alloc((eval_W.val * ((1 - r_y0.val) % Q)) % Q)
    # eval_W * (1 - r_y0) = tmp_w
    self.enforce([(eval_W, 1)], [(one, 1), (r_y0, -1)], [(tmp_w, 1)])
    sum_z = self.alloc((tmp_w.val + eval_X.val * r_y0.val) % Q)
    # eval_X * r_y0 = sum_z - tmp_w
    self.enforce([(eval_X, 1)], [(r_y0, 1)], [(sum_z, 1), (tmp_w, -1)])
    q_val = 0 if sum_z.val % Q == 0 else (prev.val * pow(sum_z.val, -1, Q)) % Q
    quotient = self.alloc_input(q_val)
    # quotient * sum_z = prev
    self.enforce([(quotient, 1)], [(sum_z, 1)], [(prev, 1)])


def emit_round(c: "VerifierCircuit", cfg: VcConfig, vals: VcValues, ri: int,
               prior: List, prev_chal: List) -> None:
  """Emit round ``ri``'s constraints + witness into ``c`` (mirrors zk.rs).

  ``prior`` / ``prev_chal`` carry the per-round handoff state across rounds and
  are mutated in place.  Reads ``vals`` fields for round ``ri`` and consumes
  challenges via ``c.next_challenge()``; both must be populated up to round
  ``ri`` before calling.  Used by :func:`build` (all-at-once, shape/witness) and
  by the streaming prover (round-by-round, interleaved with commit/squeeze).
  """
  c._cur_round = ri
  if True:
    if ri < cfg.num_rounds_b:
      poly = c.g_alloc_coeffs(vals.nifs_polys[ri])
      if ri == 0:
        claim = c.g_alloc_zero()
      else:
        r = Var(0, 0)
        r = c.alloc_input(c.next_challenge())
        claim = c.g_eval_poly_horner(prior[ri - 1], r)
      c.g_enforce_sc_claim(poly, claim)
      prior[ri] = poly
      prev_chal[ri] = []

    elif ri == cfg.idx_nifs_final:
      r = c.alloc_input(c.next_challenge())
      claim = c.g_eval_poly_horner(prior[ri - 1], r)
      t_out = c.alloc(vals.t_out_step)
      eq_rho = c.alloc(vals.eq_rho_at_rb)
      # eq_rho * t_out = claim
      c.enforce([(eq_rho, 1)], [(t_out, 1)], [(claim, 1)])
      prior[ri] = [eq_rho, t_out]
      prev_chal[ri] = []

    elif cfg.idx_nifs_final < ri < cfg.idx_outer_final:
      i = ri - cfg.idx_outer_start
      poly_step = c.g_alloc_coeffs(vals.outer_polys_step[i])
      poly_core = c.g_alloc_coeffs(vals.outer_polys_core[i])
      if i == 0:
        claim_step = prior[ri - 1][1]  # t_out_step
        claim_core = c.g_alloc_zero()
      else:
        r = c.alloc_input(c.next_challenge())
        claim_step = c.g_eval_poly_horner(prior[ri - 1][0:4], r)
        claim_core = c.g_eval_poly_horner(prior[ri - 1][4:8], r)
      c.g_enforce_sc_claim(poly_step, claim_step)
      c.g_enforce_sc_claim(poly_core, claim_core)
      prior[ri] = poly_step + poly_core
      prev_chal[ri] = []

    elif ri == cfg.idx_outer_final:
      r = c.alloc_input(c.next_challenge())
      claim_step = c.g_eval_poly_horner(prior[ri - 1][0:4], r)
      claim_core = c.g_eval_poly_horner(prior[ri - 1][4:8], r)
      Az_s = c.alloc(vals.claim_Az_step)
      Bz_s = c.alloc(vals.claim_Bz_step)
      Cz_s = c.alloc(vals.claim_Cz_step)
      Az_c = c.alloc(vals.claim_Az_core)
      Bz_c = c.alloc(vals.claim_Bz_core)
      Cz_c = c.alloc(vals.claim_Cz_core)
      tau = c.alloc(vals.tau_at_rx)
      c.g_enforce_outer_final(Az_s, Bz_s, Cz_s, tau, claim_step)
      c.g_enforce_outer_final(Az_c, Bz_c, Cz_c, tau, claim_core)
      prior[ri] = [Az_s, Bz_s, Cz_s, Az_c, Bz_c, Cz_c, tau]
      prev_chal[ri] = []

    elif cfg.idx_inner_start <= ri < cfg.idx_inner_final:
      idx = ri - cfg.idx_inner_start
      poly_step = c.g_alloc_coeffs(vals.inner_polys_step[idx])
      poly_core = c.g_alloc_coeffs(vals.inner_polys_core[idx])
      r = c.alloc_input(c.next_challenge())
      if idx == 0:
        r_sq = c.g_mul(r, r)
        claims = prior[cfg.idx_outer_final]
        claim_step = c.g_compute_joint_claim(claims[0], claims[1], claims[2], r, r_sq)
        claim_core = c.g_compute_joint_claim(claims[3], claims[4], claims[5], r, r_sq)
      else:
        claim_step = c.g_eval_poly_horner(prior[ri - 1][0:3], r)
        claim_core = c.g_eval_poly_horner(prior[ri - 1][3:6], r)
      c.g_enforce_sc_claim(poly_step, claim_step)
      c.g_enforce_sc_claim(poly_core, claim_core)
      prior[ri] = poly_step + poly_core
      prev_chal[ri] = [r]

    elif ri == cfg.idx_inner_final:
      r = c.alloc_input(c.next_challenge())
      claim_step = c.g_eval_poly_horner(prior[ri - 1][0:3], r)
      claim_core = c.g_eval_poly_horner(prior[ri - 1][3:6], r)
      tau = prior[cfg.idx_outer_final][6]
      c.g_inputize(tau)
      eval_X_step = c.alloc_input(vals.eval_X_step)
      eval_X_core = c.alloc_input(vals.eval_X_core)
      eq_rho = prior[cfg.idx_nifs_final][0]
      c.g_inputize(eq_rho)
      eval_W_step = c.alloc(vals.eval_W_step)
      eval_W_core = c.alloc(vals.eval_W_core)
      r_y0 = prev_chal[cfg.idx_inner_start + 1][0]
      c.g_enforce_inner_final(r_y0, eval_W_step, eval_X_step, claim_step)
      c.g_enforce_inner_final(r_y0, eval_W_core, eval_X_core, claim_core)
      prior[ri] = [eval_W_step, eval_W_core]
      prev_chal[ri] = []

    elif ri == cfg.idx_commit_w_step:
      ew = c.alloc(vals.eval_W_step)
      prev = prior[ri - 1][0]
      c.enforce([(ew, 1)], [(c.one(), 1)], [(prev, 1)])
      for _ in range(cfg.width - 1):
        c.g_alloc_zero()
      prior[ri] = []
      prev_chal[ri] = []

    elif ri == cfg.idx_commit_w_core:
      ew = c.alloc(vals.eval_W_core)
      prev = prior[ri - 2][1]
      c.enforce([(ew, 1)], [(c.one(), 1)], [(prev, 1)])
      for _ in range(cfg.width - 1):
        c.g_alloc_zero()
      prior[ri] = []
      prev_chal[ri] = []


def build(cfg: VcConfig, vals: VcValues):
  """Run the full circuit all-at-once; return (A, B, C, W_rounds, X).

  ``build(cfg, zero_values(cfg))`` yields the constraint shape; ``build(cfg,
  real_values)`` yields the witness.  All challenges must be present in
  ``vals.challenges``.
  """
  c = VerifierCircuit(cfg)
  c._challenges = list(vals.challenges)
  prior: List[List[Var]] = [None] * cfg.num_rounds
  prev_chal: List[List[Var]] = [None] * cfg.num_rounds
  for ri in range(cfg.num_rounds):
    emit_round(c, cfg, vals, ri, prior, prev_chal)
  return c.A, c.B, c.C, c.W_rounds, c.io_vals
