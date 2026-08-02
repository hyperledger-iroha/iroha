"""Top-level Vega-MC verifier (port of ``VegaMcZkSNARK::verify``).

This replays the exact Fiat--Shamir schedule and algebraic checks of
``src/vega_mc_zkp.rs`` and returns ``(public_values_step, public_values_core)`` on
acceptance, raising on any mismatch. The flow:

1. Restore the hoisted shared commitment into every step + core instance.
2. Validate each step instance and the core instance on *fresh* per-instance
   transcripts (re-deriving challenges from the transcript).
3. Convert instances to regular form (padding steps to a power of two).
4. On a new transcript, absorb the key + instances, squeeze ``tau`` and ``rho``,
   validate the multi-round verifier instance, and split its public IO into the
   sum-check challenges ``(r_b, r_x, r, r_y)`` and the 6 pinned public values.
5. Fold the step batch, NIFS-fold the verifier instance, run relaxed Spartan,
   recompute the 6 public values natively, and finish with the Hyrax PCS opening.
"""

from dataclasses import replace

from .params import Q
from . import mathutil, commitment
from .transcript import Transcript
from .instance import (
  split_validate,
  split_to_regular,
  multiround_validate,
  multiround_to_regular,
  fold_multiple,
  RelaxedInstance,
)
from .nifs import nifs_verify
from .spartan import spartan_verify, evaluate_matrix
from .hyrax import pcs_verify
from .polys import eq_evals, eq_evaluate, pow_evaluate, sparse_poly_evaluate

PROVE_LABEL = b"neutronnova_prove"


def verify(proof, vk, num_instances):
  """Verify a Vega-MC proof; return ``(public_values_step, public_values_core)``."""
  if num_instances == 0 or num_instances != len(proof.step_instances):
    raise ValueError(
      f"Expected {num_instances} instances (non-zero), got {len(proof.step_instances)}"
    )
  if num_instances != vk.num_steps:
    raise ValueError(
      f"Verifier key is bound to {vk.num_steps} step instances, got {num_instances}"
    )

  digest = vk.digest()

  # (1) restore the hoisted shared commitment into every instance
  step_instances = [replace(u, comm_W_shared=proof.comm_W_shared) for u in proof.step_instances]
  core_instance = replace(proof.core_instance, comm_W_shared=proof.comm_W_shared)

  # (2) validate the step instances, each on a fresh transcript
  for i, u in enumerate(step_instances):
    tr = Transcript(PROVE_LABEL)
    tr.absorb_raw(b"vk", digest)
    tr.absorb_scalar(b"num_circuits", len(step_instances))
    tr.absorb_scalar(b"circuit_index", i)
    tr.absorb_scalars(b"public_values", u.public_values)
    split_validate(u, vk.S_step, tr)

    # validate the core instance
  tr = Transcript(PROVE_LABEL)
  tr.absorb_raw(b"vk", digest)
  tr.absorb_scalars(b"public_values", core_instance.public_values)
  split_validate(core_instance, vk.S_core, tr)

  # shared-commitment consistency (all instances share comm_W_shared by construction)
  def _wire(comm):
    return None if comm is None else [p.to_wire() for p in comm]

  core_shared = _wire(core_instance.comm_W_shared)
  for u in step_instances:
    if _wire(u.comm_W_shared) != core_shared:
      raise ValueError("All instances must have the same shared commitment")

      # (3) pad step instances to a power of two, then regularize
  step_instances_padded = list(step_instances)
  target = mathutil.next_power_of_two(len(step_instances_padded))
  while len(step_instances_padded) < target:
    step_instances_padded.append(step_instances_padded[0])
  step_instances_regular = [split_to_regular(u) for u in step_instances_padded]
  core_instance_regular = split_to_regular(core_instance)

  # (4) new transcript for the NeutronNova NIFS proof
  tr = Transcript(PROVE_LABEL)
  tr.absorb_raw(b"vk", digest)
  tr.absorb_r1cs_instance(
    b"core_instance",
    commitment.to_points(core_instance_regular.comm_W),
    core_instance_regular.X,
  )
  for U in step_instances_regular:
    tr.absorb_r1cs_instance(b"U", commitment.to_points(U.comm_W), U.X)
  tr.absorb_scalar(b"T", 0)  # T = 0 in NeutronNova

  num_rounds_b = mathutil.log2(len(step_instances_regular))
  num_vars = vk.S_step.num_vars
  num_rounds_x = mathutil.log2(vk.S_step.num_cons)
  num_rounds_y = mathutil.log2(num_vars) + 1

  tau = tr.squeeze(b"tau")
  rhos = [tr.squeeze(b"rho") for _ in range(num_rounds_b)]

  multiround_validate(proof.U_verifier, vk.vc_shape, tr)
  U_verifier_regular = multiround_to_regular(proof.U_verifier)

  num_challenges = num_rounds_b + num_rounds_x + 1 + num_rounds_y
  if len(U_verifier_regular.X) != num_challenges + 6:
    raise ValueError(
      f"Verifier instance has incorrect number of public IO: "
      f"expected {num_challenges + 6}, got {len(U_verifier_regular.X)}"
    )

  challenges = U_verifier_regular.X[0:num_challenges]
  public_values = U_verifier_regular.X[num_challenges : num_challenges + 6]

  r_b = challenges[0:num_rounds_b]
  r_x = challenges[num_rounds_b : num_rounds_b + num_rounds_x]
  r = challenges[num_rounds_b + num_rounds_x]
  r_y = challenges[num_rounds_b + num_rounds_x + 1 :]

  # fold the step batch and NIFS-fold the verifier instance
  folded_U = fold_multiple(r_b, step_instances_regular)
  random_U = RelaxedInstance(
    comm_W=proof.random_U.comm_W,
    comm_E=proof.random_U.comm_E,
    X=proof.random_U.X,
    u=proof.random_U.u,
  )
  folded_U_verifier = nifs_verify(proof.nifs, tr, random_U, U_verifier_regular)

  # relaxed Spartan on the folded verifier instance
  spartan_verify(
    proof.relaxed_snark,
    vk.vc_shape_regular,
    vk.vc_vk,
    folded_U_verifier,
    tr,
  )

  # (5) recompute the 6 pinned public values natively
  T_x = eq_evals(r_x)
  T_y = eq_evals(r_y)
  eval_A_step = evaluate_matrix(vk.S_step.A, T_x, T_y)
  eval_B_step = evaluate_matrix(vk.S_step.B, T_x, T_y)
  eval_C_step = evaluate_matrix(vk.S_step.C, T_x, T_y)
  eval_A_core = evaluate_matrix(vk.S_core.A, T_x, T_y)
  eval_B_core = evaluate_matrix(vk.S_core.B, T_x, T_y)
  eval_C_core = evaluate_matrix(vk.S_core.C, T_x, T_y)

  num_vars_log2 = mathutil.ilog2(num_vars)
  eval_X_step = sparse_poly_evaluate(num_vars_log2, [1] + folded_U.X, r_y[1:])
  eval_X_core = sparse_poly_evaluate(
    num_vars_log2, [1] + core_instance_regular.X, r_y[1:]
  )

  quotient_step = (eval_A_step + r * eval_B_step + (r * r % Q) * eval_C_step) % Q
  quotient_core = (eval_A_core + r * eval_B_core + (r * r % Q) * eval_C_core) % Q
  tau_at_rx = pow_evaluate(tau, len(r_x), r_x)
  eq_rho_at_rb = eq_evaluate(r_b, rhos)

  expected = [
    tau_at_rx,
    eval_X_step,
    eval_X_core,
    eq_rho_at_rb,
    quotient_step,
    quotient_core,
  ]
  if public_values != expected:
    raise ValueError(
      "Verifier instance public tau_at_rx/eval_X_step/eval_X_core/"
      "eq_rho_at_rb/quotients do not match recomputation"
    )

    # PCS evaluation opening
  c_eval = tr.squeeze(b"c_eval")
  eval_w_step_commit_round = num_rounds_b + 1 + num_rounds_x + 1 + num_rounds_y + 1
  comm_eval_W_step = proof.U_verifier.comm_w_per_round[eval_w_step_commit_round]
  comm_eval_W_core = proof.U_verifier.comm_w_per_round[eval_w_step_commit_round + 1]

  comm = commitment.fold(
    [folded_U.comm_W, core_instance_regular.comm_W], [1, c_eval]
  )
  comm_eval = commitment.fold([comm_eval_W_step, comm_eval_W_core], [1, c_eval])

  pcs_verify(vk.vk_ee, vk.vc_ck, tr, comm, r_y[1:], comm_eval, proof.eval_arg)

  public_values_step = [u.public_values for u in step_instances[:num_instances]]
  public_values_core = core_instance.public_values
  return public_values_step, public_values_core
