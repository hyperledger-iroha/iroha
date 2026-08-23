# FASTPQ Poseidon fixed assets

`poseidon2_goldilocks_width3_v1.bin` (legacy filename) contains the canonical
width-three Goldilocks Poseidon constants: 65 rows of three round constants
followed by the three-by-three MDS matrix. The construction is the original
dense-MDS Poseidon permutation, not Poseidon2, and uses the bijective `x^7`
Goldilocks S-box pinned in `../poseidon.rs`. Every field element is stored as
`u64` little endian. Fixed-size `const fn` decoding reconstructs the public
array types without runtime parsing or allocation.

The values are cross-pinned to `../../../../artifacts/poseidon/constants.ron`
and the permutation tests in `../poseidon.rs`. The
`poseidon_hash_known_vector` and former-`x^5` collision regressions pin the CPU
semantics, while the Metal/CUDA manifest and source checks pin accelerator
parity. `manifest.json` records the fixed asset and its generation provenance;
the executable profile digest additionally binds the construction identifier,
S-box exponent, and constants-manifest digest.
