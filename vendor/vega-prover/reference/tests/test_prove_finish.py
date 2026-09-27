"""End-to-end self-verify: the Python prover's proof is accepted by the Python verifier.

Runs the full stand-alone prover (``prove_core`` + ``prove_finish``) on the fixed
cubic statement and checks that the Python verifier accepts the resulting proof.
This is the deterministic self-check before the Rust cross-conformance gate.

Run:
    python3 reference/tests/test_prove_finish.py
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, ".."))

from pyvega.vk import load_vk
from pyvega.prover import prove_core
from pyvega.prover_finish import prove_finish
from pyvega.prover_serialize import serialize_proof
from pyvega import verify as verify_mod

VK_PATH = os.path.join(HERE, "..", "fixtures", "cubic", "vk.bin")


def main():
  vk = load_vk(open(VK_PATH, "rb").read())
  num_steps = vk.num_steps
  print(f"test_prove_finish: num_steps={num_steps}")

  core = prove_core(vk, num_steps=num_steps)
  print("  prove_core done (W_verifier assembled)")

  proof = prove_finish(vk, core)
  print("  prove_finish done (proof assembled)")
  print(f"    step_instances={len(proof.step_instances)}  "
        f"sc_outer={len(proof.relaxed_snark.sc_proof_outer)}  "
        f"sc_inner={len(proof.relaxed_snark.sc_proof_inner)}  "
        f"z_vec={len(proof.eval_arg.ipa.z_vec)}")

  verify_mod.verify(proof, vk, num_steps)
  print("PASS: Python verifier accepted the Python prover's proof")

  # Emit the serialized proof so the Rust cross-conformance gate
  # (verify_python_proof) has a reproducible input. The .bin is git-ignored.
  pb = serialize_proof(proof)
  out = os.path.join(HERE, "..", "fixtures", "cubic", "python_proof.bin")
  open(out, "wb").write(pb)
  print(f"  wrote reference/fixtures/cubic/python_proof.bin ({len(pb)} bytes)")


if __name__ == "__main__":
  main()
