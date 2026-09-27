"""The Python verifier accepts a real Rust-generated proof.

Run:  python3 reference/tests/test_verify.py

Passes iff ``pyvega.verify.verify`` accepts ``proof.bin`` against ``vk.bin``:
every per-instance and per-round challenge re-derivation matches, the 6 pinned
public values equal the native recomputation, relaxed Spartan checks pass, and the
final Hyrax PCS opening verifies. The returned public values must match meta.json.
"""

import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "reference"))

from pyvega.vk import load_vk  # noqa: E402
from pyvega.proof import load_proof  # noqa: E402
from pyvega.verify import verify  # noqa: E402
from pyvega.field import scalar_to_repr  # noqa: E402

FIX = os.path.join(ROOT, "reference", "fixtures", "cubic")


def main():
  vk_bytes = open(os.path.join(FIX, "vk.bin"), "rb").read()
  proof_bytes = open(os.path.join(FIX, "proof.bin"), "rb").read()
  meta = json.load(open(os.path.join(FIX, "meta.json")))

  t0 = time.time()
  vk = load_vk(vk_bytes)
  proof = load_proof(proof_bytes)
  print(f"parsed vk ({len(vk_bytes)} B) + proof ({len(proof_bytes)} B) in {time.time()-t0:.1f}s")

  t1 = time.time()
  pv_step, pv_core = verify(proof, vk, vk.num_steps)
  print(f"verify() accepted in {time.time()-t1:.1f}s")

  # Compare returned public values against meta.json (32-byte LE hex per scalar).
  def hexify(scalars):
    return [scalar_to_repr(s).hex() for s in scalars]

  got_step = [hexify(pv) for pv in pv_step]
  got_core = hexify(pv_core)
  assert got_step == meta["public_values_step"], (got_step, meta["public_values_step"])
  assert got_core == meta["public_values_core"], (got_core, meta["public_values_core"])

  print(f"public_values_step: {got_step}")
  print(f"public_values_core: {got_core}")
  print("\nPASS: Python verifier accepted the Rust proof")


if __name__ == "__main__":
  main()
