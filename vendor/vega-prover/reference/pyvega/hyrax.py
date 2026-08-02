"""Hyrax polynomial-commitment verifier and its linear inner-product argument.

``pcs_verify`` ports ``HyraxPCS::verify`` (``src/provider/pcs/hyrax_pc.rs``): it
reduces a multilinear evaluation claim to an inner-product instance
``(comm_LZ, R, comm_eval)`` and delegates to ``ipa_verify``.

``ipa_verify`` ports ``InnerProductArgumentLinear::verify`` (``src/provider/pcs/
ipa.rs``): a linear-sized Pedersen sigma protocol with two verification equations
under a single challenge ``r``. The instance's ``b_vec`` is *not* absorbed (the
verifier recomputes it as the eq-table ``R`` of the transcript-derived point).
"""

from .params import Q
from . import commitment
from . import mathutil
from .polys import eq_evals
from .curve import point_to_transcript

IPA_PROTOCOL_NAME = b"inner product argument (linear)"


def _inner_product(a, b):
  acc = 0
  for ai, bi in zip(a, b):
    acc = (acc + ai * bi) % Q
  return acc


def ipa_verify(ck, h, ck_c, h_c, n, comm_a_vec, b_vec, comm_c, arg, transcript):
  """Verify a linear inner-product argument (two sigma equations)."""
  transcript.dom_sep(IPA_PROTOCOL_NAME)

  # Absorb the instance: comm_a_vec || comm_c (b_vec is omitted by design).
  transcript.absorb_raw(
    b"U", point_to_transcript(comm_a_vec) + point_to_transcript(comm_c)
  )
  transcript.absorb_point(b"delta", commitment.to_point(arg.delta))
  transcript.absorb_point(b"beta", commitment.to_point(arg.beta))

  r = transcript.squeeze(b"r")

  if len(arg.z_vec) != n or len(ck) < len(arg.z_vec):
    raise ValueError(
      f"IPA verify: expected {n} elements in z_vec, got {len(arg.z_vec)}"
    )

  h_pt = commitment.to_point(h)
  delta_pt = commitment.to_point(arg.delta)
  beta_pt = commitment.to_point(arg.beta)
  ck_c_pt = commitment.to_point(ck_c)
  h_c_pt = commitment.to_point(h_c)

  # Eq1: r*comm_a_vec + delta == <z_vec, ck> + z_delta*h
  lhs1 = (int(r) % Q) * comm_a_vec + delta_pt
  rhs1 = commitment.msm(arg.z_vec, ck[: len(arg.z_vec)]) + (int(arg.z_delta) % Q) * h_pt
  if lhs1 != rhs1:
    raise ValueError("IPA verify: first equation failed")

    # Eq2: r*comm_c + beta == ck_c*<z_vec, b_vec> + z_beta*h_c
  lhs2 = (int(r) % Q) * comm_c + beta_pt
  rhs2 = (_inner_product(arg.z_vec, b_vec)) * ck_c_pt + (int(arg.z_beta) % Q) * h_c_pt
  if lhs2 != rhs2:
    raise ValueError("IPA verify: second equation failed")


def pcs_verify(vk, ck_eval, transcript, comm, point, comm_eval, arg):
  """``HyraxPCS::verify`` -- reduce to an inner-product instance and check it.

    ``comm`` / ``comm_eval`` are commitments (lists of points); ``point`` is the
    evaluation point; ``arg`` carries the IPA (``arg.ipa``).
    """
  transcript.absorb_commitment(b"poly_com", commitment.to_points(comm))

  n = 1 << len(point)
  num_cols = vk.num_cols
  num_rows = mathutil.div_ceil(n, num_cols)
  num_vars_rows = mathutil.log2(num_rows)

  if len(comm) != num_rows:
    raise ValueError(
      f"Hyrax verify: commitment has {len(comm)} rows, expected exactly {num_rows}"
    )
  if len(comm_eval) == 0:
    raise ValueError("Hyrax verify: evaluation commitment is empty")

  if num_vars_rows == 0:
    R = eq_evals(point)
    comm_LZ = commitment.to_point(comm[0])
  else:
    L = eq_evals(point[:num_vars_rows])
    R = eq_evals(point[num_vars_rows:])
    comm_LZ = commitment.msm(L, comm[: len(L)])

  ipa_verify(
    vk.ck,
    vk.h,
    ck_eval.ck[0],
    ck_eval.h,
    len(R),
    comm_LZ,
    R,
    commitment.to_point(comm_eval[0]),
    arg.ipa,
    transcript,
  )
