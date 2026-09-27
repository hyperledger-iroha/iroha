"""Validate the stand-alone prover core: ``W_verifier`` satisfies the circuit.

Runs ``pyvega.prover.prove_core`` against the parsed cubic verifier key and
checks that the assembled in-circuit verifier witness satisfies every constraint
of the emitted R1CS (which is byte-identical to ``vc_shape_regular``).  This is a
strong local oracle: any error in the NIFS fold, the outer/inner sum-check round
polynomials, the Fiat--Shamir interleave, or the circuit witness fill makes at
least one constraint fail.

Run:  ``python3 reference/tests/test_prover_core.py``
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from pyvega.params import Q  # noqa: E402
from pyvega.vk import load_vk  # noqa: E402
from pyvega.prover import prove_core  # noqa: E402

FIX = os.path.join(HERE, "..", "fixtures", "cubic")


def _lc_eval(row, z):
  return sum((coeff * z[col]) % Q for col, coeff in row.items()) % Q


def _check_sat(A, B, C, z):
  for i in range(len(A)):
    az = _lc_eval(A[i], z)
    bz = _lc_eval(B[i], z)
    cz = _lc_eval(C[i], z)
    if (az * bz - cz) % Q != 0:
      return i
  return None


def main():
  vkpath = os.path.join(FIX, "vk.bin")
  if not os.path.exists(vkpath):
    print("test_prover_core: SKIPPED (fixture vk.bin absent)")
    return
  vk = load_vk(open(vkpath, "rb").read())

  res = prove_core(vk, num_steps=vk.num_steps)
  c = res.circuit

  # z = [ W_verifier | ONE | IO ]
  z = list(res.W_verifier) + [1] + list(c.io_vals)
  expected_len = res.cfg.num_vars + 1 + res.cfg.num_io
  assert len(z) == expected_len, f"z length {len(z)} != {expected_len}"

  bad = _check_sat(c.A, c.B, c.C, z)
  assert bad is None, f"constraint {bad} unsatisfied"

  # sanity on the assembled instance
  assert len(res.U_verifier.comm_w_per_round) == res.cfg.num_rounds
  assert len(res.U_verifier.public_values) == 6
  flat_chals = [x for rnd in res.U_verifier.challenges_per_round for x in rnd]
  assert flat_chals == res.challenges, "challenge grouping mismatch"

  print("test_prover_core:")
  print(f"  rounds={res.cfg.num_rounds} constraints={len(c.A)} "
        f"num_vars={res.cfg.num_vars} io={res.cfg.num_io}")
  print(f"  W_verifier satisfies all {len(c.A)} constraints of vc_shape_regular")
  print("  public_values =", res.U_verifier.public_values)
  print("PASS: prover core is correct (is_sat holds)")


if __name__ == "__main__":
  main()
