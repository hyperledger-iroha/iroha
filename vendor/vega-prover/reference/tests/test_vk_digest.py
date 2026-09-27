"""Parse vk.bin byte-exactly and recompute its digest.

Run:  python3 reference/tests/test_vk_digest.py

Passes iff (a) the parser consumes vk.bin with no trailing bytes, and (b) the
recomputed SHA-256 digest equals the Rust-exported vk_digest.bin.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "reference"))

from pyvega.vk import parse_vk  # noqa: E402

FIX = os.path.join(ROOT, "reference", "fixtures", "cubic")


def main():
  vk_bytes = open(os.path.join(FIX, "vk.bin"), "rb").read()
  expected = open(os.path.join(FIX, "vk_digest.bin"), "rb").read()

  vk = parse_vk(vk_bytes)  # raises on trailing bytes

  print(f"vk.bin parsed: {len(vk_bytes)} bytes, fully consumed")
  print(f"  ck               {len(vk.ck_bincode):>10} bytes")
  print(f"  vk_ee            {len(vk.vk_ee_bincode):>10} bytes")
  print(f"  S_step.dims      {vk.S_step.dims}")
  print(f"  S_step A/B/C nnz {vk.S_step.A.n_data}/{vk.S_step.B.n_data}/{vk.S_step.C.n_data}")
  print(f"  S_core.dims      {vk.S_core.dims}")
  print(f"  S_core A/B/C nnz {vk.S_core.A.n_data}/{vk.S_core.B.n_data}/{vk.S_core.C.n_data}")
  print(f"  vc_shape         {len(vk.vc_shape_bincode):>10} bytes")
  print(f"  vc_shape_regular {len(vk.vc_shape_regular_bincode):>10} bytes")
  print(f"  vc_ck            {len(vk.vc_ck_bincode):>10} bytes")
  print(f"  vc_vk            {len(vk.vc_vk_bincode):>10} bytes")
  print(f"  num_steps        {vk.num_steps}")

  got = vk.digest()
  print(f"\nexpected digest {expected.hex()}")
  print(f"computed digest {got.hex()}")

  assert len(expected) == 32, "vk_digest.bin must be 32 bytes"
  assert got == expected, "DIGEST MISMATCH"
  print("\nPASS: vk parsed byte-exactly and digest matches")


if __name__ == "__main__":
  main()
