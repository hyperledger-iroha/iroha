"""Sum-check verifier (port of ``SumcheckProof::verify`` in ``src/sumcheck.rs``).

Given the compressed round polynomials, a starting claim, the round count, and the
per-round degree bound, the verifier replays the interaction: each round it
decompresses the polynomial against the running claim ``e``, absorbs the round
polynomial into the transcript, squeezes the next challenge ``r_i``, and updates
``e = poly(r_i)``. It returns the final evaluation and the challenge vector.

The round polynomial is absorbed via :meth:`Transcript.absorb_unipoly`, which uses
the *little-endian* ``to_repr`` encoding of the compressed coefficients -- the one
place a field element is absorbed little-endian rather than big-endian.
"""

from .polys import unipoly_decompress, unipoly_evaluate


def sumcheck_verify(compressed_polys, claim, num_rounds, degree_bound, transcript):
  """Verify a sum-check proof; return ``(e, r)`` (final eval, challenges)."""
  if len(compressed_polys) != num_rounds:
    raise ValueError("InvalidSumcheckProof: wrong number of round polynomials")

  e = claim
  r = []
  for i in range(num_rounds):
    cp = compressed_polys[i]  # coeffs_except_linear_term
    # CompressedUniPoly::degree() == len(coeffs_except_linear_term).
    if len(cp) != degree_bound:
      raise ValueError(
        f"InvalidSumcheckProof: round {i} degree {len(cp)} != {degree_bound}"
      )
    # decompress reads the first stored coefficient; a degree-0 round has none.
    if degree_bound == 0:
      raise ValueError("InvalidSumcheckProof: degree_bound must be >= 1")
    coeffs = unipoly_decompress(cp, e)
    transcript.absorb_unipoly(b"p", cp)
    r_i = transcript.squeeze(b"c")
    r.append(r_i)
    e = unipoly_evaluate(coeffs, r_i)
  return e, r
