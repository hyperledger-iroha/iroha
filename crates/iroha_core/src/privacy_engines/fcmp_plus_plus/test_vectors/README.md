# FCMP++ interoperability-vector provenance

These two fixtures are deterministic, membership-only interoperability vectors
for Iroha's native-Rust FCMP++ verifier. They contain a real upstream
Spend-Authorization-and-Linkability (SAL) proof and divisor-backed
full-chain-membership proof. They are not complete Iroha transactions: balance
and Iroha's statement-bound strict-positive Bulletproofs+ range proof are
covered by the native end-to-end KATs named below.

## Fixture manifest

| File | Inputs | Tree layers / root curve | `proof` bytes | File bytes | SHA-256 |
| --- | ---: | --- | ---: | ---: | --- |
| `one_input_one_layer.txt` | 1 | 1 / Selene | 3,360 | 7,231 | `a29f634c5cd0d399ff63d6668685929a177d7c96149db22c663c55097cc7853a` |
| `one_input_two_layers.txt` | 1 | 2 / Helios | 3,776 | 8,063 | `63bd4728b66e1499256e7fcaa9252e97dc5994105dd77ddbcfcd427f829ad69f` |

`SHA256SUMS` repeats the machine-checkable digests. From this directory:

```text
shasum -a 256 -c SHA256SUMS
```

Each file is newline-delimited lowercase hexadecimal in this exact field
order:

```text
context=<32-byte SAL transaction context>
root=<32-byte compressed Selene or Helios root>
o_tilde=<32-byte compressed Edwards O~>
i_tilde=<32-byte compressed Edwards I~>
r=<32-byte compressed Edwards R>
c_tilde=<32-byte compressed Edwards C~ pseudo-out>
key_image=<32-byte compressed Edwards L>
proof=<FcmpPlusPlus::write bytes>
```

The `proof` value retains the upstream encoding:
`(O~ || I~ || R || SAL)` per input, followed by
`Fcmp::write` (the two generalized-Bulletproof circuit proofs and the
root-blind proof of knowledge). Upstream supplies `C~`, the key image, root,
layer count, and transaction context out of band.

## Immutable upstream identity

The membership/SAL fixtures were generated from
<https://github.com/kayabaNerve/fcmp-plus-plus>:

- signed commit:
  `15ef71140944b5b5d2feff0e58569b71f34c84a2`
- commit URL:
  <https://github.com/kayabaNerve/fcmp-plus-plus/commit/15ef71140944b5b5d2feff0e58569b71f34c84a2>
- Git tree object:
  `494b0a210b8503bf4065506ac738e6013cb9430a`
- parent object:
  `42f0f67df1e8b3360e8f0a137dbddb74307f4618`
- signature: GitHub reports a valid PGP signature, verified at
  `2025-08-26T19:25:12Z`
- tag: none; this manifest intentionally pins the immutable commit directly

Relevant paths, tests, and Git blob objects at that commit are:

| Path | Test/API | Git blob |
| --- | --- | --- |
| `networks/monero/ringct/fcmp++/src/tests/mod.rs` | `tests::test` | `c774fa7ce73a6ecfa7b7c5012995bdd6ca627303` |
| `networks/monero/ringct/fcmp++/src/lib.rs` | `FcmpPlusPlus::{proof_size,write,read,verify}` | `d636dedd8ebeb35ce959c99cf03ec79d0c84da6c` |
| `networks/monero/ringct/fcmp++/src/sal/mod.rs` | `SpendAuthAndLinkability::{prove,verify,write,read}` | `841ee4d30a7e3af385c1a641be5d5256b3b8efb2` |
| `crypto/fcmps/src/lib.rs` | `Fcmp::{proof_size,prove,verify,write,read}` | `9bd9f2b62400b1f3b26b8284a1bb8aaa906827e4` |
| `crypto/fcmps/src/tests.rs` | `tests::test_single_input` (including one- and two-layer paths) | `443e92fea53092106e086cfd1df66806ff8651c9` |

Iroha's related native Bulletproofs+ compatibility vector is derived from
<https://github.com/serai-dex/serai>:

- signed commit:
  `971951a1a66014fce5a943b4c78fc24c63187dbb`
- commit URL:
  <https://github.com/serai-dex/serai/commit/971951a1a66014fce5a943b4c78fc24c63187dbb>
- Git tree object:
  `be1e5882cf3d93bc253611f8ce21d77a8ef292c2`
- parent object:
  `92d9e908cbb748da8fab3705ace2a224511e9164`
- signature: GitHub reports a valid SSH signature, verified at
  `2025-08-15T19:26:39Z`
- tag: none; this manifest intentionally pins the immutable commit directly
- principal source:
  `networks/monero/ringct/bulletproofs/src/lib.rs`
  (`Bulletproof::{prove_plus,verify,write,read_plus}`), Git blob
  `13a7a14e33a9f6b9ec98cb6e49e7a40dbc7473cc`
- upstream test:
  `networks/monero/ringct/bulletproofs/src/tests/mod.rs::bulletproofs_plus`,
  Git blob `fa4c89396b4c768dd8f0f8232ade2f704017490c`
- Figure-3 implementation:
  `networks/monero/ringct/bulletproofs/src/plus/aggregate_range_proof.rs`,
  Git blob `6468cdf188f0ec234505240715445d4fbcb87edc`

## Deterministic generation and verification

The fixture generator reset `ChaCha20Rng` to seed `[0x5a; 32]` for each case,
used one input and context `[0x42; 32]`, constructed the one- and two-layer
paths with the pinned upstream APIs, and serialized the proof with
`FcmpPlusPlus::write`. It then emitted the external statement fields and proof
as lowercase hexadecimal in the order above.

The pinned upstream tests use `OsRng`, so an unmodified checkout validates the
same construction and encodings but deliberately does not reproduce identical
bytes. The exact upstream validation commands are:

```text
git clone https://github.com/kayabaNerve/fcmp-plus-plus.git
git -C fcmp-plus-plus checkout --detach 15ef71140944b5b5d2feff0e58569b71f34c84a2
cargo test --manifest-path fcmp-plus-plus/Cargo.toml -p monero-fcmp-plus-plus --lib tests::test -- --exact --nocapture
cargo test --manifest-path fcmp-plus-plus/Cargo.toml -p full-chain-membership-proofs --lib tests::test_single_input -- --exact --nocapture
```

The authoritative local byte and native-equation checks are:

- `membership::tests::pinned_upstream_end_to_end_proof_verifies`
- `membership::tests::replay_public_root_and_every_proof_phase_fail_closed`
- `prover::tests::native_one_layer_prover_round_trips_end_to_end`
- `range::tests::pinned_serai_bulletproofs_plus_vector_verifies_natively`
- `range::tests::native_standard_transcript_proof_is_upstream_compatible`

`membership.rs` adds the `IFC1` header `[1, layers, 0, 0]` only inside tests.
`wire.rs::decode_fcmp_membership_fixture_v1` supplies a structurally valid
dummy range suffix only under `cfg(test)`. Production has no membership-only
acceptance path: `balance.rs::verify_fcmp_transaction_v1` requires the complete
canonical `IFC1` proof, commitment balance, and statement-bound range proof.

## License provenance

The relevant upstream crates declare MIT licensing:

| Upstream license file | Copyright | Git blob | File SHA-256 |
| --- | --- | --- | --- |
| `crypto/fcmps/LICENSE` and `networks/monero/ringct/fcmp++/LICENSE` | 2024 Luke Parker | `659881f1accb4bf463b6732c6f96a57e3c8c1a7d` | `90dd5570b3bd5a8aa641b3d434dde4880374892120b14b17889719634c04a9ba` |
| `crypto/helioselene/LICENSE` | 2022-2024 Luke Parker | `91d893c119a4c21bba539faff51c9b2ffa2021e4` | `ff862c96603cdae0ada3fc0b90fa801dc0eb20292570d4f4b53e35ca14407723` |
| Serai `networks/monero/ringct/bulletproofs/LICENSE` | 2022-2024 Luke Parker | `91d893c119a4c21bba539faff51c9b2ffa2021e4` | `ff862c96603cdae0ada3fc0b90fa801dc0eb20292570d4f4b53e35ca14407723` |

No upstream source is vendored in this directory; these files are generated
interoperability data retained with their exact source and license provenance.
