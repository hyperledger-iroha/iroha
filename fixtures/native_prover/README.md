# Native prover known-answer vectors

`kats_v1.json` pins the behaviour of the vendored halo2 stack that the
Iroha-native crates (`iroha_pasta`, `iroha_plonk`, `iroha_plonk_gadgets`) must
reproduce. These vectors stay after `vendor/halo2-axiom`,
`vendor/halo2curves-axiom`, `vendor/halo2-base`, `halo2-ecc` and
`snark-verifier` are deleted. The migration plan and consumer list are in
`specs/native_prover_migration_inventory.md`.

Field elements are canonical little-endian `to_repr` hex. Points are compressed
`to_bytes` hex.

| Section | Contents |
| --- | --- |
| `oracle_baseline` | Commits, crate versions and toolchain the vectors were recorded from. |
| `golden_proofs` | Proof SHA-256 tables of `vendor/halo2-axiom/tests/golden_proof_bytes.rs` (Blake2b) and `crates/iroha_core_zk/src/prover_golden_tests.rs` (KAGEMUSHA Poseidon). TODO(`iroha_core_zk` owner): `prover_golden_tests.rs` should read its table from `golden_proofs.iroha_core_zk_kagemusha` here instead of the oracle reading that source file. |
| `params_ipa` | `ParamsIPA::new(k).write` length and SHA-256 for k = 6..=16 on Eq and Ep, plus digests of the `g` and `g_lagrange` sections. |
| `generators` | The first 64 `Halo2-Parameters` hash-to-curve generators, and `w` and `u`, per curve. |
| `blake2b_transcript` | `Blake2bWrite` with `Challenge255`: per-operation challenges, absorbed bytes, written stream, decoding rejections. |
| `poseidon_transcript` | snark-verifier `PoseidonTranscript<C, NativeLoader, _, 3, 2, 8, 57>::new::<0>` (the KAGEMUSHA transcript): challenges, absorbed elements, stream, rejections. |
| `poseidon_constants` | Unoptimized round constants and MDS matrix of the width-3, rate-2, 8 + 57 round Poseidon for Fp and Fq. |
| `kagemusha_v1_poseidon` | Vectors of the oracle reproduction of `kagemusha_v1_poseidon::hash` and the empty depth-256 replay root. `production_anchors` records that no `iroha_core_zk` test pins these values yet (TODO for its owner); the shared sponge is anchored through the confidential section. |
| `confidential_v3_poseidon` | Vectors of the oracle reproduction of `confidential_poseidon_hash_v3` for every V3 domain, and the empty subtree roots up to depth 16. `production_anchors` names the three production KATs the reproduction is checked against. |

Consumers: `crates/iroha_plonk_oracle/tests/pasta_parity/` checks
`iroha_pasta` against the `params_ipa` digests, the `poseidon_constants`
tables, the `poseidon_transcript` challenges and the `kagemusha_v1_poseidon`
and `confidential_v3_poseidon` vectors, besides comparing it with the vendored
crates directly.

## Regenerate and check

The generator is `crates/iroha_plonk_oracle/tests/native_prover_kats.rs`. By
default it rebuilds every vector from the vendored stack and requires the file
to match. A changed vector means the vendored behaviour changed. Review that
change before you regenerate.

```sh
cargo test -p iroha_plonk_oracle --test native_prover_kats
cargo test --release -p iroha_plonk_oracle --test native_prover_kats -- --include-ignored  # adds k = 15 and 16
IROHA_UPDATE_NATIVE_PROVER_KATS=1 cargo test --release -p iroha_plonk_oracle --test native_prover_kats native_prover_kats_match_fixture
```

`verify_kats_v1.py` is an independent check that uses only the Python standard
library. It re-derives every Blake2b challenge, every Poseidon hash and
transcript challenge (from the pinned constants), the absorbed encodings,
`ParamsIPA` lengths and point validity, and checks that every recorded
transcript rejection is a distinct malformed input (a scalar at least the
modulus; a point that is the identity, non-canonical or off the curve). It does
not re-derive hash-to-curve.

```sh
python3 fixtures/native_prover/verify_kats_v1.py
```
