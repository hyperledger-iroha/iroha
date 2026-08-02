"""NeutronNova NIFS verifier (port of ``NovaNIFS::verify`` in ``src/nifs.rs``).

Folds a relaxed instance ``U1`` with a regular instance ``U2`` using a
Fiat--Shamir challenge ``r`` derived after absorbing both instances and the
cross-term commitment ``comm_T``:

    comm_W = U1.comm_W + r * U2.comm_W
    comm_E = U1.comm_E + r * comm_T
    X      = U1.X + r * U2.X
    u      = U1.u + r
"""

from .params import Q
from . import commitment
from .instance import RelaxedInstance


def nifs_verify(nifs, transcript, U1: RelaxedInstance, U2) -> RelaxedInstance:
  """Absorb ``U1``, ``U2``, ``comm_T``, squeeze ``r``, and return the fold."""
  if len(U1.X) != len(U2.X):
    raise ValueError("NIFS::verify: instances have mismatched public IO lengths")

  transcript.absorb_relaxed_instance(
    b"U1",
    commitment.to_points(U1.comm_W),
    commitment.to_points(U1.comm_E),
    U1.u,
    U1.X,
  )
  transcript.absorb_r1cs_instance(b"U2", commitment.to_points(U2.comm_W), U2.X)
  transcript.absorb_commitment(b"comm_T", commitment.to_points(nifs.comm_T))
  r = transcript.squeeze(b"r")

  X = [(a + r * b) % Q for a, b in zip(U1.X, U2.X)]
  comm_W = commitment.fold([U1.comm_W, U2.comm_W], [1, r])
  comm_E = commitment.fold([U1.comm_E, nifs.comm_T], [1, r])
  u = (U1.u + r) % Q
  return RelaxedInstance(comm_W=comm_W, comm_E=comm_E, X=X, u=u)
