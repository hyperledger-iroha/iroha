# SCCP fixed numeric assets

`go_rng_cooked_i64le_v1.bin` is the 607-word seed table used by Go 1's
`math/rand.NewSource`. Each word is stored as its exact two's-complement `i64`
bit pattern in little-endian order and reconstructed by a fixed-size `const fn`.
The resulting `GO_RNG_COOKED` array, generator state, and shuffle indexing are
unchanged; no runtime parsing, allocation, synchronization, or extra branch is
introduced.

This table is consensus-sensitive through `GoMathRand`,
`go_math_rand_shuffle`, and `parlia_backoff_ms`, which feeds governed Parlia
header validation. The existing `go_math_rand_port_matches_go_1_regression_vectors`
test remains the non-tautological compatibility check. `manifest.json` pins the
asset and its pre-extraction Rust declaration.
