# FastPQ Poseidon2 fixed assets

`poseidon2_goldilocks_width3_v1.bin` contains the canonical width-three
Goldilocks Poseidon2 constants: 65 rows of three round constants followed by
the three-by-three MDS matrix. Every field element is stored as `u64` little
endian. Fixed-size `const fn` decoding reconstructs the original public array
types, so permutation round indexing and runtime arithmetic are unchanged and
there is no runtime parser, allocation, synchronization, or extra branch.

The values are cross-pinned to `../../../../artifacts/poseidon/constants.ron`
and the provenance comments in `../poseidon.rs` (`poseidon-primitives` 0.2.0,
parameters pinned to `ark-poseidon2` commit `3f2b7fe`). The existing
`poseidon_hash_known_vector` test and the Metal/CUDA manifest parity checks
remain independent consumers. `manifest.json` pins both the asset and source
preimages.
