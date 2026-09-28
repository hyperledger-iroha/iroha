"""Fully stand-alone cross-conformance: Python does setup + prove + verify with
zero Rust runtime dependency, and the Rust verifier accepts the result.

Run: python3 reference/tests/test_standalone.py
Then the Rust gate: cargo test --test reference_conformance verify_python_standalone -- --ignored --nocapture
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from pyvega import setup as setup_mod
from pyvega.vk import load_vk
from pyvega.prover import prove_core
from pyvega.prover_finish import prove_finish
from pyvega.prover_serialize import serialize_proof
from pyvega.proof import load_proof
from pyvega import verify as verify_mod

FIX = os.path.join(os.path.dirname(__file__), "..", "fixtures", "cubic")
NUM_STEPS = 2


def main():
  print("[1/5] Python setup: key-gen + shapes -> vk bytes")
  vk_bytes = setup_mod.serialize_vk(num_steps=NUM_STEPS)
  print(f"      python vk: {len(vk_bytes)} bytes")

  vk = load_vk(vk_bytes)  # asserts exact consumption
  print(f"      parsed OK; digest={vk.digest().hex()[:16]}... num_steps={vk.num_steps}")

  print("[2/5] Python prover core")
  core = prove_core(vk, num_steps=NUM_STEPS)

  print("[3/5] Python prover finish (Nova fold + relaxed Spartan + ZK IPA)")
  proof = prove_finish(vk, core)

  print("[4/5] serialize proof + round-trip")
  proof_bytes = serialize_proof(proof)
  load_proof(proof_bytes)  # asserts exact consumption
  print(f"      python proof: {len(proof_bytes)} bytes")

  print("[5/5] Python verifier self-check")
  reparsed = load_proof(proof_bytes)
  verify_mod.verify(reparsed, vk, NUM_STEPS)
  print("      PASS: Python verifier accepts the stand-alone Python proof")

  open(os.path.join(FIX, "python_vk.bin"), "wb").write(vk_bytes)
  open(os.path.join(FIX, "python_standalone_proof.bin"), "wb").write(proof_bytes)
  print("      wrote python_vk.bin + python_standalone_proof.bin")


if __name__ == "__main__":
  main()
