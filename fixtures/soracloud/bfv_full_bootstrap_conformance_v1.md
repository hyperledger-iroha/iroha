# BFV arithmetic conformance material

`bfv_full_bootstrap_conformance_v1.norito` is the canonical Norito encoding of
`[BfvFullBootstrapExecutionProverInputMaterialV1; 2]`, for distinct slots 0 and 1.
It contains deterministic arithmetic material for the Core full-bootstrap STARK
roundtrip and adversarial relation tests. Each consumer performs canonical decoding,
strict artifact/witness/trace validation, and exact re-encoding.

- Size: 807,864 bytes.
- SHA-256: `afc3049a757cdb7af8f8733ca69b6d72ee806a6626228badf0f29576135b1106`.
- Generator: `crates/iroha_crypto/src/fhe_bfv/conformance.rs`, compiled only in the
  crypto unit-test crate. No production API or qualification switch is added.

This fixture is arithmetic conformance data, not an independently reviewed release
artifact. It contains no audit package or reviewer signing key. The generator also
asserts that a locally signed test package cannot qualify either the production
execution wrapper or the production noise-bound wrapper. Both remain unavailable
with `MissingRegisteredHeOrgLatticeNoiseAndQromEvidence`.

Regenerate into an absolute temporary path, inspect the size and digest, and then
replace the fixture and this record together:

```sh
IROHA_BFV_CONFORMANCE_OUTPUT=/absolute/temporary/materials-v1.norito \
  scripts/cargo_fast.sh --target-slot privacy-release-v1 --stable-local-metadata -- \
  test -p iroha_crypto --lib fhe_bfv::conformance -- --include-ignored --nocapture
```

The generator validates both complete material statements and their canonical
roundtrip before writing the bounded output. The 2026-09-06 regeneration passed
both tests in 274.60 seconds with an unoptimized test profile.

The final Core native-STARK suite passes 63 tests, including all ten BFV-prefixed
cases, in 849.78 seconds with command-local test opt-level 3 for `fastpq_isi`,
`fastpq_prover`, and `iroha_crypto`. A supplementary native-module probe with the
same dependency selections generated, fully verified, and re-encoded each slot's
737,089-byte proof in two independent processes. Each slot repeats byte-for-byte;
the slot 0 and 1 proof SHA-256 values are respectively
`fc0e9ff79c781cafb77c8291dc2dfa096992748336841666014db0abc2c4c5c2` and
`69a17c3c0fc851d825da35cd0e04b84cf892d05d642f67a80e7616254e376e61`.
These are CPU arithmetic/determinism checks, not production qualification or
device parity. The exact Core executable identity is recorded in
[ZK-AUDIT-26 and ZK-AUDIT-31](../../specs/zk_cryptographic_audit.md).
