# Native prover migration inventory

Status: M0 contract capture, recorded 2026-10-04. This table lists every
workspace crate, SDK and tool that uses the vendored halo2 stack or another
Pasta/halo2 implementation. The Iroha-native crates (`iroha_pasta`,
`iroha_plonk`, `iroha_plonk_gadgets`, `iroha_plonk_recursion`) replace the
vendored stack. Update a row in the same change that migrates its consumer.

## Oracle baseline

| Item | Value |
| --- | --- |
| Repository HEAD (`git rev-parse HEAD`) | `1de7210a74d62ae5232c67b910dbf4b6b1bcf757` |
| `git log -1 -- vendor/halo2-axiom` | `8f41274044c93ad7e363fdc8be30a671efb436d4` (2026-10-04) |
| `git log -1 -- vendor/halo2curves-axiom`, `vendor/halo2-base` | `48dcbb6683c0763f3f4f47915f969a0b96d484da` |
| `snark-verifier`, `halo2-ecc`/`halo2-base` git | rev `bbfcc721d714bea0d44a27c8fc6c4736e73ca853`, tag `v0.5.3` |
| Toolchain, host | Rust 1.93.1, aarch64-apple-darwin; x86_64 not yet re-run |

Contract artifacts:

- `crates/iroha_plonk_oracle/tests/vendored_goldens.rs` runs
  `vendor/halo2-axiom/tests/golden_proof_bytes.rs` unchanged through `#[path]`.
  It covers 20 cases, each in Rayon pools of 1, 2, 4 and 7 threads; k = 11 is
  ignored and run in release.
- `fixtures/native_prover/kats_v1.json` holds the golden tables, `ParamsIPA`
  digests for k6-k16, generators, `w`/`u`, Blake2b and Poseidon transcript
  vectors, the RP57 constants, and the KAGEMUSHA and confidential hash vectors.
  `crates/iroha_plonk_oracle/tests/native_prover_kats.rs` generates and checks
  it; `fixtures/native_prover/verify_kats_v1.py` re-derives it independently.

## Method

The inventory was built with these commands; it is not regenerated automatically.

- Imports: `rg -l '\b<ident>\b' --type rust -g '!vendor/**'` for each of
  `halo2_proofs`, `halo2_base`, `halo2curves(_axiom)`, `snark_verifier`,
  `halo2_ecc`, `pasta_curves` and `orchard`; then `rg -l` over `IrohaSwift/`,
  `scripts/` and `python/` for `Halo2`, `Pasta` and `orchard`.
- Graph: `cargo tree --frozen --workspace -i <pkg> -e normal,build,dev
  --depth 1`, plus the manifests of `python/iroha_python/iroha_python_rs` (not a
  workspace member) and `ci/dependency_budget.json` package contracts.
- `vendor/` is excluded from the import search; vendored Pasta providers other
  than the halo2 stack are listed by hand (`vendor/vega-prover`).
- Exposure: `configs/soranexus/` and the `skills/sora-*` docs.

The Rust crates below import the vendored stack as `halo2_proofs` (the renamed
`halo2-axiom` package), except where the row names Zcash and except
`iroha_plonk_oracle`, which imports it under its own crate name
`halo2_axiom`.

Use: **P** prove, **V** verify, **T** types/encodings, **H** native hashing.
Milestones follow the judge plan in its corrected form (see the notes at the
end); the labels are defined in the legend below.

## Milestone legend

| Label | Meaning |
| --- | --- |
| M0 | Contract capture: vendored goldens runnable from the workspace, per-family goldens, fixtures and tamper corpora, native-hash, hash-to-curve, transcript and snark-verifier KATs; `vendor/` bug-fix-only |
| M1 | First native σ: `iroha_pasta`, `iroha_plonk`, `iroha_plonk_gadgets` v0, `iroha_plonk_oracle`, σ_send/σ_recv. Exit criteria groups: |
| M1.a (M1a) | Engine parity: native proofs reproduce the vendored golden SHA-256 at 1/2/4/7 threads on aarch64 and x86_64; `ParamsIPA` bytes for k6-k16 and VK bytes identical; verifier verdicts match on the tamper corpora. The `iroha_pasta` parity suites and the `halo2curves` 0.9 encoding KATs belong here |
| M1.b (M1b) | Kernel timings on the M1 Ultra (MSM 2^16, fold, k11 σ-shaped prove) |
| M1.c (M1c) | Native σ: the 24 M7 relation cases, in-circuit digests equal `kagemusha_v1_poseidon::hash`, proof size, prove/verify time and RSS budgets |
| M1.d | Hygiene: no `vendor/` change; fmt, clippy and tests pass for the new crates |
| M2 | Ledger relations and the consensus verifier on the native stack, one family per change: (a) SoraFS PoP, (b) Kaigi, (c) confidential (re-keyed), (d) vote tally, `halo2_backend` and the `lib.rs`/`verification.rs` dispatch |
| M3 | Gadgets II: native ECC, P-256 ECDSA, SHA-256 lanes, SMT depth 256/128, App Attest DER |
| M4 | Recursion: succinct verifier (native and circuit), BGH19 accumulation, single-parity Ω, format decision memo |
| M5 | KAGEMUSHA on the native stack (σ, Λ, Ω, CreditStatus, artifact manifests, bridge handles); the old halo2 relations were deleted on 2026-10-05 |
| M6 | Engine tail and phones: aarch64 `asm`, deferred folds, GLV MSM, streaming and PolyStore spill, device qualification |
| M7 | Deletion: vendored stack, git dependencies, oracle crate and feature removed; CI guard added; audit closed |
| Step 1-8 | Migration steps of the plan: 1 M0 capture; 2 M1 crates beside the vendored stack; 3 native state hashes first (`kagemusha_v1_poseidon` and the confidential native Poseidon move to `iroha_pasta::poseidon`, Pasta imports switch to `iroha_pasta`; gate: KAT equality on every M0 vector); 4 M2 families; 5 M3-M5 KAGEMUSHA; 6 harnesses (`ram_lfe_*`; the KAGEMUSHA harnesses `g3_proof_scaling_measurement_tests.rs` and `prover_golden_tests.rs` were deleted with the old relations); 7 peripheral owners (fuzz manifests, scripts, pytests); 8 M7 deletion |
| T1-T21 | First-milestone tasks. Cited here: T7 `iroha_pasta::poseidon` with a KAT test against the M0 `kagemusha_v1_poseidon` vectors; T15 `iroha_plonk_oracle`; T16 `iroha_core_zk/src/prover_golden_parity_tests.rs` re-proving the σ golden natively (no longer possible: the old circuits were deleted on 2026-10-05; the goldens stay pinned in `kats_v1.json`) |

Outstanding outcomes and owners for the programme belong in `roadmap.md`
(TODO: add a native PLONK/IPA row with owners and the M0-M7 completion
criteria).

## Direct users of the vendored stack

| Consumer | Stack (direct) | Use | Live exposure | Milestone |
| --- | --- | --- | --- | --- |
| `iroha_core_zk` verify dispatch: `lib.rs` (`verify_halo2_ipa`, `zkparse`), `verification.rs`, `halo2_backend.rs` (+`_01/_02/_03` tests), `zk1_test_helpers.rs`, `ivm_proof_identity` | halo2-axiom | V T | Taira `[zk.halo2] enabled = true` (`configs/soranexus/taira/config.toml:314`). Stored proofs and registered VKs: unknown offline | M2(d) |
| `iroha_core_zk::confidential_v2` (+tests): transfer v2, unshield v2/v3 circuits | halo2-axiom, halo2-base, snark-verifier | P V H | Taira `[confidential] enabled = true` (`config.toml:288`). Shielded notes and registered `CONFIDENTIAL_*_VK_DIGEST_V1`: unknown offline | Note hash: Step 3 (KAT gate). Circuits: M2(c), re-key |
| `iroha_core_zk::kagemusha_v1_poseidon` | halo2-base spec, snark-verifier `Poseidon` | H | Only the consensus mint-finality roots of `mint_finality.rs` (below) | Removed with that consensus residue; the T7 KAT vectors stay in `kats_v1.json` |
| Old KAGEMUSHA owners: `iroha_core_zk::kagemusha_v1_state`, `kagemusha_v1_recursion` except `mint_finality.rs`, the `pasta_*`, `kagemusha_p256_curve_gadget.rs` and `app_attest_der_gadget.rs` gadgets, `kagemusha_polynomial_store_v1`, `prover_golden_tests.rs`, `g3_proof_scaling_measurement_tests.rs`, and `iroha_core` `isi/kagemusha.rs` and `kagemusha_v1_reserve.rs` | — | — | None in source | Deleted 2026-10-05 (owner decision, first release). M5 rebuilds KAGEMUSHA natively in `iroha_kagemusha_proof`; the golden table stays pinned in `kats_v1.json` |
| `kagemusha_v1_recursion/mint_finality.rs` | halo2-axiom Pasta, `kagemusha_v1_poseidon` | T H | Consensus: `irohad` (`main.rs`, `taira_runtime_signer.rs`, `consensus_threshold`), `iroha_kagami` genesis, `iroha_test_network` | Consensus residue. Removed by the consensus mint-finality cut (owner decision 2026-10-04), not ported |
| `ram_lfe_*` (`#[cfg(test)]`) | halo2-axiom | P | None | Owner: re-express or delete |
| `kaigi_*_v1_tests.rs` (in `iroha_core_zk`) | halo2-axiom | P V | Test | M2(b) |
| `iroha_core` Kaigi: `isi/kaigi/privacy{,/authorization_v1,/proof_fixture_v1}.rs`, `privacy_release_evidence/kaigi.rs`, `tests/kaigi_privacy.rs` | halo2-axiom | V | Kaigi VKs are config refs (`kaigi_authorization_vk`, `kaigi_usage_vk`), unset in the Taira template. Live: unknown | M2(b) |
| `iroha_core` vote tally and tooling: `tests/zk_vote_tally_audit.rs`, `tests/zk_testkit.rs`, `benches/zk_poseidon.rs`; feature `circuit-params = ["halo2_proofs/circuit-params"]` | halo2-axiom | P V | Test | M2(d). Remove the feature forward |
| `kaigi_zk` (7 files; Poseidon from `poseidon-primitives` `Spec` over Pasta `Fp`) | halo2-axiom | P V H | Through `iroha_core` `zk-halo2` | M2(b). Keep VK bytes, bump IDs (critique). TODO: RP56 KATs |
| `sorafs_manifest::pop_credentials::zk` (own `poseidon-primitives` Poseidon, `zk.rs:100-115`) | halo2-axiom | P V H | SoraFS PoP credentials (irohad runtime provider). Live: unknown | M2(a). TODO: PoP Poseidon KATs |
| `iroha_js_host` (`kaigi_proof_v1.rs`, OsRng; `confidential_wallet.rs` through `iroha_core_zk`) | halo2-axiom | P | npm package | M2(b)(c), caller RNG |
| `xtask` `vote_tally.rs` (feature `dev-vote-fixture`) | halo2-axiom | P | Fixture only | M2(d) |
| `iroha_plonk_oracle` (imports `halo2_axiom`, not `halo2_proofs`) | halo2-axiom, halo2-base, snark-verifier (halo2-ecc transitively) | P V T H | None (test only, `publish = false`). Reads no other crate's sources: the `iroha_core_zk` KAGEMUSHA golden table is pinned in `kats_v1.json` (`golden_proofs.iroha_core_zk_kagemusha`) and carried over unchanged | Deleted M7 |

## Other Pasta and halo2 implementations (not the vendored stack)

| Consumer | Stack | Use | Live exposure | Milestone |
| --- | --- | --- | --- | --- |
| `iroha_data_model::sumeragi::epoch` (`epoch.rs:124`) through `iroha_zkp_poseidon::pasta_keys` | halo2curves 0.9 Pasta | T | Consensus paired-Pasta key check on native epochs | Retained. Owner decides on convergence; cross-implementation encoding KATs in M1a |
| `iroha_zkp_poseidon` (`pasta_keys.rs`, `pasta.rs`, `poseidon.rs`; dev `halo2curves-axiom` cross-check in `pasta/tests.rs`) | halo2curves 0.9; dev halo2curves-axiom | T H | `pasta_keys` is consensus (also `mint_finality.rs:345`) | Dev oracle moves to `iroha_pasta` at Step 3 |
| `iroha_zkp_halo2` (native Pallas/BN254 IPA, SHA3 transcript; used by `ivm`, `iroha_core` `zk-ipa-native`, `iroha_cli`, `iroha_torii`, `iroha_core_privacy`, `fastpq_prover`, `iroha_core_zk`, `iroha_python_rs`) | halo2curves 0.9 (optional) | P V T | Non-optional dependency of `iroha_core`, so every node | Owner: converge or keep separate |
| `iroha_core_privacy` `privacy_engines/orchard.rs`; `iroha_core` feature `privacy-release-evidence` | orchard 0.15.4 (git rev `9d07047d`, a non-optional dependency of `iroha_core_privacy`), Zcash `halo2_proofs` 0.3.4, `pasta_curves` 0.5.2 | V | Taira privacy catalog `orchard-halo2-actions-v1`, `activation_state: not-executed` (`privacy_bootstrap_plan.json`) | Carve-out: stays after M7; allowlist in `check_no_vendored_halo2.sh` (TODO: the script does not exist yet; M7 adds it) |
| Orchard linkers (through `iroha_core_privacy`): `connect_norito_bridge` (iOS/Android), `irohad`, `iroha_torii`, `iroha_cli`, `iroha_kagami`, `iroha_js_host`, `zk_ace_prover`, `integration_tests`, and `python/iroha_python/iroha_python_rs` (`privacy_wallet_bundle.rs`, `privacy_native_actions.rs` build Orchard spend/change prover inputs) | orchard, Zcash `halo2_proofs` 0.3.4, `pasta_curves` 0.5.2 (linked, not imported) | P V T | Every node binary, the mobile bridge, npm and PyPI wheels | Carve-out with Orchard; binary-size and seal reports count it |
| `vendor/vega-prover` (Microsoft Vega; `src/provider/pasta.rs`) | Own `ff` 0.13 Pasta provider; not a workspace member | T | Reference for the pinned Pasta points in `iroha_data_model` `sumeragi/epoch/tests.rs` and for the `iroha_zkp_halo2` Vega fixtures (`vega/canonical_mc_exact.rs`, `vega/microsoft_mc/*`) | Out of the halo2 M7 scope; owner decides whether those KATs move to `iroha_pasta` |
| `ivm` (BN254 `poseidon.rs`, `bn254_vec.rs`); ZK verify syscalls reach `iroha_core_zk` through the `iroha_core` host | halo2curves 0.9 BN254 | H | All nodes | No direct change; follows M2(d) dispatch |
| `fastpq_prover` (BN254 Poseidon, Metal, CUDA bench) | halo2curves 0.9 BN254 | H | FASTPQ | Out of scope |
| `iroha_sccp` (optional, dev); `iroha_deploy` dev (`verify/finality/tests.rs`) | halo2curves 0.9 | T | Tests, SCCP | Out of scope |
| `iroha_pasta` | dev `pasta_curves` 0.5.2 (oracle) | T | None | Keep while orchard keeps `pasta_curves` |

## Transitive consumers (link `iroha_core_zk`)

| Consumer | What it reaches | Live exposure | Milestone |
| --- | --- | --- | --- |
| `irohad`, `iroha_torii` (`zk_prover.rs`, routing), `iroha_cli` (`zk.rs`), `iroha_kagami` (genesis), `iroha_test_network` | Verify dispatch, mint finality (until the consensus cut), confidential | All nodes and operators | M2 (dispatch) |
| `iroha_deploy` (`localnet.rs` registers confidential VK records; `genesis/staging.rs`) | Confidential VKs | Localnet, staging | M2(c) fixtures |
| `connect_norito_bridge` (`confidential_prover_ffi`, `confidential_note_ffi`) | Confidential prover and note hash | iOS and Android apps (staticlib/cdylib) | Step 3, M2(c); reseal and size report |
| `iroha_python_rs` (`confidential_wallet.rs`), `iroha_js_host` | Confidential wallet | PyPI wheels, npm | M2(c) |
| `integration_tests` (dev) | Proof fixtures, `queries/proof.rs` | Test | M2 (4-peer cutover tests) |

## SDKs and tools

| Surface | Halo2 dependence | Milestone |
| --- | --- | --- |
| Swift `Halo2Pasta.swift`, `Halo2Vesta.swift`, `Halo2IPA.swift` | Independent Swift Pasta/Vesta, `ParamsIPA` read/generate/serialize, IPA commit and opening prove/verify. Must match `params_ipa` and `generators` | Cross-check against `kats_v1.json` (M1a); keep the layout |
| Swift `Halo2Transcript.swift`, `Halo2EvaluationDomain.swift`, `Halo2VestaHashToCurve.swift` | Independent Blake2b `Halo2-Transcript` `Challenge255` transcript, FFT domain (`omega`), and Vesta hash-to-curve. Must match `blake2b_transcript`, the FFT omega and `generators` | Cross-check against `kats_v1.json` (M1a) |
| Swift `ConfidentialProver`, `ConfidentialNote`, `VerifyingKeyBackendTag` | Through the bridge; backend label `halo2/ipa` | M2(c) fixtures |
| Kotlin `core-jvm`/`client-android` (privacy native bridge, VK registry); Java `iroha_android` (retiring) | Through the bridge; labels | M2(c) fixtures |
| C# (`Zk/VerifyingKeyBackendTag.cs`, `Privacy/ConfidentialProver.cs`, `Kaigi`) | Through the bridge; labels | M2(b)(c) fixtures |
| JavaScript `iroha_js` (`kaigiScalarV1.js` Pasta Fp checks) | `iroha_js_host` napi | M2(b)(c) |
| Python `iroha_python` | `iroha_python_rs` | M2(c) |
| `fuzz/Cargo.toml`, `crates/fastpq_prover/fuzz/Cargo.toml` | Vendored path dependencies and the halo2-lib patch | Step 7 |
| `scripts/cargo_fuzz_locked_cargo.sh`, `scripts/norito_bridge_source_seal.py`, `scripts/check_ivm_only.py`, `pytests/scripts/norito_bridge_source_seal_reviewed_vendor_test.py`, `pytests/scripts/workspace_release_gate_test.py` | Name vendored paths or packages | Step 7, M7 |
| `ci/dependency_budget.json`, `scripts/check_release_feature_graph.py` (`proofs-halo2`, `zk-halo2`, `zk-halo2-ipa`), source-token guards in `pr.yml` | Pin the current graph and features. TODO: the baseline does not yet count the `iroha_pasta` and `iroha_plonk_oracle` members (`--write-baseline` after review); `iroha_plonk_oracle` should be a `forbidden_packages` entry of the shipping configurations | Update in each migrating change |
| CI for the release-only oracle suites and `fixtures/native_prover/verify_kats_v1.py` | None yet. TODO: a nightly workflow running `cargo test --locked --release -p iroha_pasta -p iroha_plonk_oracle -- --include-ignored` on x86_64 and aarch64, and a `pytests/scripts` wrapper for the verifier | M0 exit (x86_64 run) |

## Live networks

- **Taira:**
  - The checked-in config enables `[zk.halo2]` and `[confidential]`.
  - The privacy rollout lists Orchard as not executed.
  - The deployed build may still serve the old KAGEMUSHA V1 Torii commands, which
    the source no longer has; the Taira reset is their cutover. The Digital Shekel
    asset stays in the genesis template.
  - The genesis templates register no halo2 VKs.
  - Unknown offline: registered VKs by backend label, stored proofs, shielded commitment and nullifier counts, Kaigi sessions, PoP credentials, installed KAGEMUSHA releases and mint-finality epochs.
- **Minamoto:** no checked-in config, and the skill lists no ZK surface. Everything is unknown offline; stay read-only.

A read-only MCP inventory of both networks must gate the M2 cutover and the M7
deletion (spec §9: old-format value keeps its verifier until holders exit).

## Corrections applied to the judge plan

- Mint finality is consensus code with its own removal (owner decision
  2026-10-04), not part of the M5 `mint_*` deletion or a port.
- Orchard and Zcash `halo2_proofs` 0.3.4 are a production dependency of
  `iroha_core_privacy` and stay after M7.
- The consensus Pasta key check (`iroha_data_model` epoch, through
  `iroha_zkp_poseidon::pasta_keys`, halo2curves 0.9) is a fourth Pasta
  implementation. It is out of M7 scope unless the owner decides to converge.
- The KAGEMUSHA and confidential native hashes are byte-identical
  constructions: the same RP57 spec and the same `[domain, len, inputs]`
  preimage, with different domains. One `iroha_pasta::poseidon` sponge serves
  both.
- Still to capture (TODO): Kaigi and SoraFS PoP `poseidon-primitives` Poseidon
  KATs, snark-verifier succinct challenges and BGH19 accumulators, per-family
  goldens and tamper corpora, x86_64 runs.
