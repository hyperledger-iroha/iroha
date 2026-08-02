"""Split-instance validation, regularization, and folding.

These port ``SplitR1CSInstance`` / ``SplitMultiRoundR1CSInstance`` validation and
``to_regular_instance`` from ``src/r1cs/mod.rs``, plus ``R1CSInstance::fold_multiple``.

Validation is the heart of the conformance check: it re-derives every per-round
Fiat--Shamir challenge from the transcript and asserts it equals the challenge the
prover stored in the instance. A byte-exact transcript makes these match.

Commitments stay as lists of :class:`~pyvega.curve.WirePoint` (or EccPoints);
``to_regular_instance`` concatenates them (``combine_commitments``), and folding
uses group arithmetic via :mod:`pyvega.commitment`.
"""

from dataclasses import dataclass
from typing import List

from .params import Q
from .mathutil import div_ceil
from . import commitment

DEFAULT_COMMITMENT_WIDTH = 2048


@dataclass
class R1CSInstanceReg:
  """A regular R1CS instance: a witness commitment and its public IO vector."""

  comm_W: list  # list of points (WirePoint or EccPoint)
  X: List[int]


@dataclass
class RelaxedInstance:
  """A relaxed R1CS instance ``(comm_W, comm_E, X, u)``."""

  comm_W: list
  comm_E: list
  X: List[int]
  u: int


def check_commitment(comm, n: int, width: int):
  """``HyraxPCS::check_commitment``: the commitment must have ``ceil(n/width)`` rows."""
  min_rows = div_ceil(n, width)
  if len(comm) != min_rows:
    raise ValueError(
      f"InvalidCommitmentLength: actual {len(comm)}, expected {min_rows}"
    )


def eq01(bit: int, r: int) -> int:
  """``eq01``: returns ``r`` if ``bit == 1`` else ``1 - r`` (mod Q)."""
  return r % Q if bit == 1 else (1 - r) % Q


def weights_from_r(r_bs: List[int], n: int) -> List[int]:
  """``weights_from_r``: barycentric eq-weights over the ``ell``-bit hypercube."""
  ell = len(r_bs)
  w = []
  for i in range(n):
    wi = 1
    k = i
    for t in range(ell):
      wi = (wi * eq01(k & 1, r_bs[t])) % Q
      k >>= 1
    w.append(wi)
  return w


  # SplitR1CSInstance
def split_validate(u, S, transcript):
  """``SplitR1CSInstance::validate`` -- re-derive and check per-circuit challenges."""
  if len(u.public_values) != S.num_public:
    raise ValueError(
      f"public_values length ({len(u.public_values)}) != num_public ({S.num_public})"
    )

  if S.num_shared > 0:
    if u.comm_W_shared is None:
      raise ValueError("comm_W_shared is missing")
    check_commitment(u.comm_W_shared, S.num_shared, DEFAULT_COMMITMENT_WIDTH)
    transcript.absorb_commitment(b"comm_W_shared", commitment.to_points(u.comm_W_shared))
  elif u.comm_W_shared is not None:
    raise ValueError("comm_W_shared present for an empty segment")

  if S.num_precommitted > 0:
    if u.comm_W_precommitted is None:
      raise ValueError("comm_W_precommitted is missing")
    check_commitment(u.comm_W_precommitted, S.num_precommitted, DEFAULT_COMMITMENT_WIDTH)
    transcript.absorb_commitment(
      b"comm_W_precommitted", commitment.to_points(u.comm_W_precommitted)
    )
  elif u.comm_W_precommitted is not None:
    raise ValueError("comm_W_precommitted present for an empty segment")

  challenges = [transcript.squeeze(b"challenge") for _ in range(S.num_challenges)]
  if challenges != u.challenges:
    raise ValueError("Challenges do not match")

  check_commitment(u.comm_W_rest, S.num_rest, DEFAULT_COMMITMENT_WIDTH)
  transcript.absorb_commitment(b"comm_W_rest", commitment.to_points(u.comm_W_rest))


def split_to_regular(u) -> R1CSInstanceReg:
  """``SplitR1CSInstance::to_regular_instance`` -- X = [public_values, challenges]."""
  partial = [
    c
    for c in (u.comm_W_shared, u.comm_W_precommitted, u.comm_W_rest)
    if c is not None
  ]
  comm_W = commitment.combine(partial)
  X = list(u.public_values) + list(u.challenges)
  return R1CSInstanceReg(comm_W=comm_W, X=X)


  # SplitMultiRoundR1CSInstance
def multiround_validate(u, s, transcript):
  """``SplitMultiRoundR1CSInstance::validate`` -- per-round challenge re-derivation."""
  if len(u.public_values) != s.num_public:
    raise ValueError(
      f"public_values length ({len(u.public_values)}) != num_public ({s.num_public})"
    )
  if len(u.comm_w_per_round) != s.num_rounds:
    raise ValueError(
      f"comm_w_per_round length ({len(u.comm_w_per_round)}) != num_rounds ({s.num_rounds})"
    )
  if len(u.challenges_per_round) != s.num_rounds:
    raise ValueError(
      f"challenges_per_round length ({len(u.challenges_per_round)}) != num_rounds ({s.num_rounds})"
    )

  for rnd in range(s.num_rounds):
    check_commitment(
      u.comm_w_per_round[rnd], s.num_vars_per_round[rnd], s.commitment_width
    )
    transcript.absorb_commitment(
      b"comm_w_round", commitment.to_points(u.comm_w_per_round[rnd])
    )
    derived = [
      transcript.squeeze(b"challenge")
      for _ in range(s.num_challenges_per_round[rnd])
    ]
    if u.challenges_per_round[rnd] != derived:
      raise ValueError(f"MultiRoundR1CSInstance:: Challenges do not match for round {rnd}")


def multiround_to_regular(u) -> R1CSInstanceReg:
  """``SplitMultiRoundR1CSInstance::to_regular_instance`` -- X = [challenges, public_values]."""
  comm_W = commitment.combine(u.comm_w_per_round)
  challenges = [c for rnd in u.challenges_per_round for c in rnd]
  X = challenges + list(u.public_values)
  return R1CSInstanceReg(comm_W=comm_W, X=X)


  # Folding a batch of regular instances
def fold_multiple(r_bs: List[int], Us: List[R1CSInstanceReg]) -> R1CSInstanceReg:
  """``R1CSInstance::fold_multiple`` -- eq-weighted sum over the instance batch."""
  n = len(Us)
  if n == 0:
    raise ValueError("fold_multiple: empty instance list")
  w = weights_from_r(r_bs, n)
  d = len(Us[0].X)
  if any(len(U.X) != d for U in Us):
    raise ValueError("fold_multiple: all X vectors must have the same length")

  X_acc = [0] * d
  for i, Ui in enumerate(Us):
    wi = w[i]
    for j, Uij in enumerate(Ui.X):
      X_acc[j] = (X_acc[j] + wi * Uij) % Q

  comm_acc = commitment.fold([U.comm_W for U in Us], w)
  return R1CSInstanceReg(comm_W=comm_acc, X=X_acc)
