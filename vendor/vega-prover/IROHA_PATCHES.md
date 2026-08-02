# Iroha integration patches

The upstream baseline is Microsoft `vega-prover` commit
`c0ee259053cd12eaf43ed71b5cde375452b3ee4d`, Git tree
`7226b6cbfbfe8613dd2d5ee831096b7578a5c115`. Every vendored upstream-origin
file and its pristine SHA-256 is listed in `UPSTREAM_MANIFEST.sha256`.

Iroha modifies exactly five upstream files:

| File | Pristine SHA-256 | Patched SHA-256 | Purpose |
| --- | --- | --- | --- |
| `src/lib.rs` | `a4f4c282f52d5d2f1d9799dc2bb17b43d07a7ea1ff51b4f4bf585630db65f3ff` | `936bb79781b01f910826eeb708eb7fe01cc02be01c1d815e252cb50e62bdbcb8` | Expose the hidden Iroha RNG integration module. |
| `src/provider/pcs/hyrax_pc.rs` | `65fb6259969172c15863cd35824aa85d24e908fc38b036ab2eb4606fa71adf6a` | `cee54fc22fcb7ec552b7a187a05aafaf1667b5a57e1ca1350c5e07a340d4990c` | Draw Hyrax blindings from the proof-scoped external CSPRNG. |
| `src/provider/pcs/ipa.rs` | `a16f7d0560fe3363113a81768e57a0b06dbcf1996b5bb48cf3dc909d3aa49125` | `61a92fa6f6f833ee10c6ed57da6cabdceb05abe8db9a6985f65bb7afc47ba77e` | Draw IPA hiding randomness from the same scoped CSPRNG. |
| `src/r1cs/mod.rs` | `c152a02ea03e0bd506ce0f60023c291ffaeda4b251998be499d47d999a702aae` | `891cc378df2a338da263a60cfed0a5ec15985dc4ffff0c78cc5cc252f92a62fc` | Draw the relaxed-instance zero-knowledge mask from the scoped CSPRNG. |
| `src/vega_mc_zkp.rs` | `05f2aba7947851447dade720e8501e1c9336ae5d0a29810ecebe6da9e245253c` | `b5f69e7a4c956efc5359f54408530824be0e7232d4eb39ba06eae4f5f4788d75` | Expose a verifier-key-derived proof-dimension view and isolate Microsoft's canonical bincode proof/key representation behind a bounded adapter. |

Iroha adds `src/iroha_rng.rs`. It serializes proof scopes, installs one
externally supplied 256-bit seed, shares that CSPRNG with the prover's Rayon
workers, clears it on every exit path, and converts an internal panic at the
scope boundary into an explicit error. Every patched random draw fails closed
outside that scope; there is no ambient `thread_rng()` fallback.

Iroha also adds the two independently generated Python oracle binaries under
`reference/fixtures/cubic/`, the narrow `.gitignore` override that includes
them in source archives, and the provenance/manifest documents in this
directory.

The runtime bincode dependency is private to the pinned Microsoft crate. Iroha
first performs a verifier-key-derived, non-allocating wire scan, then calls the
bounded adapter; all surrounding Iroha envelopes and APIs use native Iroha
types and Norito. A dev-only bincode dependency remains solely as a
cross-conformance oracle proving that the adapter preserves Microsoft's exact
canonical bytes.

These patches do not change Vega's R1CS relations, Fiat--Shamir transcript,
proof equations, curve/field arithmetic, or canonical bincode representation.
The cross-conformance tests enforce the pristine and patched hashes, reject
undeclared source drift, and require the patched native verifier to accept a
proof produced by the unmodified Python reference implementation.
