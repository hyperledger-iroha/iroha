"""Group-element commitment operations over the T256 curve.

A Hyrax commitment is a list of group elements (one per matrix row). The verifier
needs four operations on these lists:

* :func:`to_points`   -- decompress a commitment to curve points (transcript/MSM);
* :func:`msm`         -- ``vartime_multiscalar_mul``: ``sum_i s_i * P_i``;
* :func:`combine`     -- ``combine_commitments``: concatenate the point lists;
* :func:`fold`        -- ``fold_commitments``: element-wise weighted sum.

Inputs may mix :class:`~pyvega.curve.WirePoint` (compressed, lazily decompressed)
and already-decompressed :class:`~pyvega.curve.EccPoint`; :func:`_pt` normalizes
either form.
"""

from .params import Q
from .curve import curve, WirePoint


def _pt(x):
  """Return a curve point from either a WirePoint or an already-decompressed point."""
  return x.point() if isinstance(x, WirePoint) else x


def to_points(comm):
  """Decompress a commitment (list of points) to curve points."""
  return [_pt(p) for p in comm]


def to_point(x):
  """Decompress a single point (WirePoint or already-decompressed) to a curve point."""
  return _pt(x)


def msm(scalars, points):
  """``vartime_multiscalar_mul``: ``sum_i scalars[i] * points[i]``.

    Zero scalars are skipped (they contribute the identity). ``scalars`` are
    reduced into ``[0, Q)`` before use.
    """
  E = curve()
  acc = E(0)
  for s, p in zip(scalars, points):
    s = int(s) % Q
    if s != 0:
      acc = acc + s * _pt(p)
  return acc


def combine(comms):
  """``combine_commitments``: concatenate the constituent point lists."""
  out = []
  for c in comms:
    out.extend(c)
  return out


def fold(comms, weights):
  """``fold_commitments``: ``result[j] = sum_i weights[i] * comms[i][j]``.

    All commitments must have the same length; the result is a list of curve
    points of that length.
    """
  if not comms or len(comms) != len(weights):
    raise ValueError("fold: commitments and weights must be non-empty and aligned")
  n = len(comms[0])
  if any(len(c) != n for c in comms):
    raise ValueError("fold: all commitments must have the same length")
  E = curve()
  out = []
  for j in range(n):
    acc = E(0)
    for w, c in zip(weights, comms):
      w = int(w) % Q
      if w != 0:
        acc = acc + w * _pt(c[j])
    out.append(acc)
  return out
