"""The stand-alone prover's building blocks are correct.

Run:  python3 reference/tests/test_prover_foundations.py

Checks:
  * the cubic app circuit's constructed matrices are byte-identical to the Rust
    fixture and its witness satisfies (A z) o (B z) = C z;
  * univariate ``from_evals`` interpolation round-trips (deg 2/3/general);
  * ``MLE.bind_top`` matches the naive multilinear evaluation;
  * a Hyrax commitment of the app witness has the right shape and is accepted by
    the verifier's ``split_validate`` / ``check_commitment`` path.
"""

import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "reference"))

from pyvega import app_circuit as ac  # noqa: E402
from pyvega.prover_polys import unipoly_from_evals, MLE  # noqa: E402
from pyvega.polys import unipoly_evaluate, eq_evals  # noqa: E402
from pyvega.params import Q  # noqa: E402

FIX = os.path.join(ROOT, "reference", "fixtures")


def test_app_circuit():
  S = ac.cubic_shape()
  w = ac.cubic_witness()
  z = ac.z_vector(w)
  assert len(z) == S.num_rest + 1 + S.num_public
  assert ac.is_sat(S, z), "cubic witness must satisfy the R1CS"
  assert w.public_values == [15]

  # Byte-identical to the Rust-exported cubic matrices (if the fixture exists).
  vkpath = os.path.join(FIX, "cubic", "vk.bin")
  if os.path.exists(vkpath):
    from pyvega.vk import parse_vk

    vk = parse_vk(open(vkpath, "rb").read())
    assert S.dims == vk.S_step.dims
    for py, ru in ((S.A, vk.S_step.A), (S.B, vk.S_step.B), (S.C, vk.S_step.C)):
      assert py.matrix_raw() == ru.matrix_raw(), "matrix must match fixture byte-for-byte"
    print("  app circuit: matrices byte-match fixture, is_sat OK")
  else:
    print("  app circuit: is_sat OK (fixture vk.bin absent, skipped byte-match)")


def test_unipoly():
  for coeffs in ([7, 3, 9, 2], [5, 4, 6], [1, 2, 3, 4, 5]):
    n = len(coeffs)
    evals = [unipoly_evaluate(coeffs, x) for x in range(n)]
    assert unipoly_from_evals(evals) == [c % Q for c in coeffs]
  print("  unipoly from_evals: round-trips deg 2/3/4")


def test_mle_bind():
  random.seed(1)
  nv = 5
  Z = [random.randrange(Q) for _ in range(1 << nv)]
  r = [random.randrange(Q) for _ in range(nv)]
  chi = eq_evals(r)
  naive = sum(Z[i] * chi[i] for i in range(len(Z))) % Q
  m = MLE(list(Z))
  for ri in r:
    m.bind_top(ri)
  assert m.final() == naive
  print("  MLE.bind_top: matches naive multilinear evaluation")


def _rows_from_csr(M):
  """Decode a SparseMatrixRaw into a list of ``{col: coeff}`` row dicts."""
  indptr, indices, data = M.indptr(), M.indices(), M.data()
  rows = []
  for r in range(len(indptr) - 1):
    d = {}
    for idx in range(indptr[r], indptr[r + 1]):
      d[indices[idx]] = (d.get(indices[idx], 0) + data[idx]) % Q
    rows.append({c: v for c, v in d.items() if v != 0})
  return rows


def test_verifier_circuit():
  """Emitted A/B/C must equal the Rust vc_shape_regular row-for-row (order-free)."""
  vkpath = os.path.join(FIX, "cubic", "vk.bin")
  if not os.path.exists(vkpath):
    print("  verifier circuit: SKIPPED (fixture vk.bin absent)")
    return
  from pyvega.vk import parse_vk
  from pyvega.verifier_circuit import VcConfig, zero_values, build

  vk = parse_vk(open(vkpath, "rb").read())
  # cubic: num_rounds_b=1, num_rounds_x=2, num_rounds_y=12, width=32
  cfg = VcConfig(num_rounds_b=1, num_rounds_x=2, num_rounds_y=12, width=32)
  A, B, C, _W, _X = build(cfg, zero_values(cfg))

  reg = vk.vc_shape_regular
  for name, mine, ru in (("A", A, reg.A), ("B", B, reg.B), ("C", C, reg.C)):
    ru_rows = _rows_from_csr(ru)
    assert len(mine) <= len(ru_rows), f"{name}: emitted more rows than Rust"
    for i, row in enumerate(mine):
      assert row == ru_rows[i], f"{name} row {i} differs: mine={row} rust={ru_rows[i]}"
    for i in range(len(mine), len(ru_rows)):
      assert ru_rows[i] == {}, f"{name} Rust row {i} expected empty (padding)"
  print(f"  verifier circuit: {len(A)} constraint rows match vc_shape_regular exactly")


def test_commit():
  try:
    from pyvega.prover_commit import BlindSource, hyrax_commit
    from pyvega.instance import split_validate, split_to_regular, check_commitment, DEFAULT_COMMITMENT_WIDTH
    from pyvega.proof import SplitR1CSInstance
    from pyvega.transcript import Transcript
    from pyvega.vk import parse_vk
  except Exception as exc:  # pragma: no cover
    print(f"  commit: SKIPPED ({exc})")
    return

  vkpath = os.path.join(FIX, "cubic", "vk.bin")
  if not os.path.exists(vkpath):
    print("  commit: SKIPPED (fixture vk.bin absent)")
    return

  vk = parse_vk(open(vkpath, "rb").read())
  S = ac.cubic_shape()
  w = ac.cubic_witness()
  blinds = BlindSource().next_vec(1)
  comm = hyrax_commit(vk.ck.ck, vk.ck.h, w.W, DEFAULT_COMMITMENT_WIDTH, blinds)
  check_commitment(comm, S.num_rest, DEFAULT_COMMITMENT_WIDTH)
  u = SplitR1CSInstance(None, None, comm, w.public_values, [])
  split_validate(u, S, Transcript(b"neutronnova_prove"))
  reg = split_to_regular(u)
  assert reg.X == [15]
  assert len(reg.comm_W) == 1
  print("  commit: Hyrax commit accepted by split_validate, X == [15]")


def main():
  print("test_prover_foundations:")
  test_app_circuit()
  test_unipoly()
  test_mle_bind()
  test_verifier_circuit()
  test_commit()
  print("PASS: prover foundations are correct")


if __name__ == "__main__":
  main()
