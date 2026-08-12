# Falcon-512 table assets

These versioned binary files are exact little-endian serializations of the
fixed Falcon-512 tables previously expressed as Rust array literals. The Rust
consumers use fixed-size `include_bytes!` values and specialized `const fn`
decoders, so the executable receives the same typed arrays without runtime
parsing, allocation, synchronization, or branches.

The implementation and tables are adapted from `rust-fn-dsa` 0.3 at commit
`daf14859b5aa3f8d75c42966ba7de83e6eb59997`. Upstream provenance and the
Unlicense text remain in [`../NOTICE.md`](../NOTICE.md) and
[`../LICENSE`](../LICENSE).

The assets are consumed as follows:

- `kgen_fxp_gm_u64le_v1.bin` reconstructs `kgen::fxp::GM_TAB`, preserving
  each `FXR` raw `u64` bit pattern. `kgen::vect` indexes this table in FFT
  loops.
- `kgen_mp31_tables_le_v1.bin` reconstructs `kgen::mp31::REV10` followed by
  `PRIMES`. Key-generation polynomial, NTRU, and big-integer routines index
  those arrays directly.
- `sign_gm_binary64le_v1.bin` reconstructs `sign::poly::GM` from the exact
  emulated-FLR binary64 bit patterns used by signer FFT loops.
- `comm_ntt_u16le_v1.bin` reconstructs `comm::mq::GM` followed by `iGM`,
  retaining their Montgomery representation and direct NTT indexing.
- `kat512_v1.bin` reconstructs the six test-only `KAT_512` components in
  upstream order.

`manifest.json` pins every byte length, SHA-256 digest, record layout, and the
pre-extraction Rust declaration hashes. The direct non-tautological table and
fixture checks remain `pinned_upstream_keygen_test0_raw_trapdoor_kat`,
`pinned_upstream_signer_target_preimage_equation_and_norm_kat`,
`signer_rejects_noncanonical_target_and_zero_proposal_budget`, and
`signer_self_check_rejects_each_mutated_trapdoor_component`. The independent
`pinned_falcon_chacha20_stream_kat` and
`pinned_falcon_chacha20_two_refill_digest_and_tail_boundary` checks retain the
PRNG behavior used by the signer fixture.
