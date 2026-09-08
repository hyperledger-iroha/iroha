# FASTPQ Poseidon fixed assets

`poseidon_goldilocks_width3_v1.bin` contains the canonical
width-three Goldilocks Poseidon constants: 65 rows of three round constants
followed by the three-by-three MDS matrix. The construction is the original
dense-MDS Poseidon permutation, not Poseidon2, and uses the bijective `x^7`
Goldilocks S-box pinned in `../poseidon.rs`. Every field element is stored as
`u64` little endian. Fixed-size `const fn` decoding reconstructs the public
array types without runtime parsing or allocation.

The values are cross-pinned to `../../../../artifacts/poseidon/constants.ron`
and the permutation tests in `../poseidon.rs`. The
`poseidon_hash_known_vector` and the insecure-`x^5` collision regression pin the CPU
semantics, while the Metal/CUDA manifest and source checks pin accelerator
parity. `manifest.json` records the fixed asset and canonical RON lengths and hashes;
the executable profile digest additionally binds the construction identifier,
S-box exponent, and constants-manifest digest.

`digest384_reference_v1.tsv` pins 31 independently generated typed Digest384
vectors, including empty and split fields, seven-byte packing boundaries,
domain separation, and maximal `u64` coordinates. The Python oracle uses
standard-library `hashlib` SHAKE256 and arbitrary-precision modular arithmetic;
it does not invoke Rust or reuse the optimized Goldilocks reducer. Rust tests
check the one-shot and streaming implementations against these same vectors.
Run `python3 scripts/fastpq/reference_digest384.py` from the repository root to
check them, and pass `--write` only for an intentional fixture regeneration.
These vectors provide implementation conformance evidence, not independent
cryptographic review or production qualification.
