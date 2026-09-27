"""Parse the Rust-exported proof with exact byte consumption.

Run with:  python3 reference/tests/test_proof_parse.py
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REF = os.path.dirname(HERE)
sys.path.insert(0, REF)

from pyvega.proof import load_proof  # noqa: E402
from pyvega.curve import WirePoint  # noqa: E402


def main():
  path = os.path.join(REF, "fixtures", "cubic", "proof.bin")
  with open(path, "rb") as f:
    data = f.read()

  proof = load_proof(data)  # raises unless the whole buffer is consumed
  print(f"parsed proof.bin: {len(data)} bytes, fully consumed (cursor == len)")

  # Structural summary.
  print(f"  comm_W_shared present : {proof.comm_W_shared is not None}")
  print(f"  step_instances        : {len(proof.step_instances)}")
  print(f"  core public_values    : {len(proof.core_instance.public_values)}")
  print(f"  U_verifier rounds     : {len(proof.U_verifier.comm_w_per_round)}")
  print(f"  ipa.z_vec length      : {len(proof.eval_arg.ipa.z_vec)}")
  print(f"  outer sumcheck polys  : {len(proof.relaxed_snark.sc_proof_outer)}")
  print(f"  inner sumcheck polys  : {len(proof.relaxed_snark.sc_proof_inner)}")

  # The shared witness commitment is hoisted to the top level; every instance's
  # own comm_W_shared is therefore always None on the wire. The cubic
  # (x^3 + x + 5) circuit shares no witness, so the top-level field is None too.
  print(f"  top-level comm_W_shared: {'Some' if proof.comm_W_shared else 'None'}")
  for i, u in enumerate(proof.step_instances):
    assert u.comm_W_shared is None, f"step_instances[{i}].comm_W_shared != None"
  assert proof.core_instance.comm_W_shared is None, "core.comm_W_shared != None"
  print("  hoist check           : OK (all instance comm_W_shared == None)")

  # Every point flag is a defined value; identity encodes 32 zero x-bytes.
  npoints = 0

  def check_point(wp: WirePoint):
    nonlocal npoints
    npoints += 1
    assert (wp.flag & ~0x80) == 0 or wp.is_identity

  for wp in (proof.comm_W_shared or []):
    check_point(wp)
  check_point(proof.eval_arg.ipa.delta)
  check_point(proof.eval_arg.ipa.beta)
  for rnd in proof.U_verifier.comm_w_per_round:
    for wp in rnd:
      check_point(wp)
  for wp in proof.nifs.comm_T:
    check_point(wp)
  print(f"  point flags checked   : {npoints}")

  # Decompress a sample of points to confirm they lie on the curve.
  sample = [proof.eval_arg.ipa.delta, proof.eval_arg.ipa.beta]
  sample += [wp for rnd in proof.U_verifier.comm_w_per_round[:2] for wp in rnd]
  sample += list(proof.nifs.comm_T)
  on_curve = 0
  for wp in sample:
    P = wp.point()  # decompress; raises if not on curve
    if not P.is_zero():
      on_curve += 1
  print(f"  decompressed on-curve : {on_curve}/{len(sample)} (rest identity)")

  print("\nPASS: proof.bin structurally conforms to the spec.")


if __name__ == "__main__":
  main()
