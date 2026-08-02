"""Prover-side commitment helpers: Pedersen/Hyrax commit and ``process_round``.

The verifier only ever *decompresses* commitments; the prover must *create*
them. A Hyrax commitment of a length-``n`` vector with row width ``w`` is the list

    [ <chunk_j, ck[0:len_j]> + blind_j * h   for each width-``w`` chunk j ]

one group element per ``ceil(n/w)`` row, each hidden by an independent blind.
``process_round`` is the multi-round-instance mechanism the in-circuit verifier
witness is committed through: commit this round's (padded) witness, absorb the
commitment, and squeeze the round's Fiat-Shamir challenges.

Blinds are drawn from a :class:`BlindSource`, a cryptographically secure scalar
source — a real zero-knowledge prover must sample blinds this way. The blinds
never leave the prover except inside commitments, so any secure randomness yields
a proof the verifier accepts.
"""

import secrets
from typing import List

from .params import Q
from .curve import curve
from .commitment import msm, to_point


class BlindSource:
  """A cryptographically secure scalar stream in ``[0, Q)`` for blinds and masks."""

  def next(self) -> int:
    # Cryptographically secure, unbiased scalar in [0, Q).
    return secrets.randbelow(Q)

  def next_vec(self, n: int) -> List[int]:
    return [self.next() for _ in range(n)]


def pedersen_row(ck, h, chunk: List[int], blind: int):
  """Commit one row: ``<chunk, ck[0:len(chunk)]> + blind * h`` (a curve point)."""
  acc = msm(chunk, ck[: len(chunk)])
  b = int(blind) % Q
  if b != 0:
    acc = acc + b * to_point(h)
  return acc


def hyrax_commit(ck, h, vector: List[int], width: int, blinds: List[int]):
  """Commit a full vector as ``ceil(len/width)`` Pedersen rows.

    Returns the list of row commitments (curve points). ``blinds`` must have one
    entry per row; the caller keeps them to open/fold the commitment later.
    """
  n = len(vector)
  num_rows = (n + width - 1) // width if n > 0 else 0
  if len(blinds) != num_rows:
    raise ValueError(f"hyrax_commit: need {num_rows} blinds, got {len(blinds)}")
  rows = []
  for j in range(num_rows):
    chunk = vector[j * width : (j + 1) * width]
    rows.append(pedersen_row(ck, h, chunk, blinds[j]))
  return rows


def commit_zeros(h, num_rows: int, blinds: List[int]):
  """Commit ``num_rows`` all-zero rows: each row = ``blind_j * h`` (for padding)."""
  E = curve()
  out = []
  for j in range(num_rows):
    b = int(blinds[j]) % Q
    out.append((b * to_point(h)) if b != 0 else E(0))
  return out
