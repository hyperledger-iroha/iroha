# Musubi V1 signed fixtures

The argument-free `iroha_data_model` binary `musubi_fixtures` constructs both
documents from typed Rust values and public synthetic signing seeds. It never
reads the generated JSON. The generator, closed output pair and read-only
two-pass check are registered in `generated-files.toml`.

`instructions_v1.json` covers exact instruction payloads, concrete and aggregate
frames, and signed provider attestations. `sdk_v1.json` covers canonical model
values, request/response pairs and rejected inputs. Rust, Kotlin and Swift
consumer suites read these shared files.

## Purpose-issued replication order correction, 2026-09-07

Commit `f9a236e8692db3a40fbf83cbd904adf3e00aed1b` reserved the high bit of the
first order-ID byte for automatic SoraFS orders. Musubi-purpose issuance must
leave that bit clear. The typed fixture inputs changed from `[0xC2; 32]` to
`[0x42; 32]` in that commit, but the generated documents still contained the
former order and its signatures.

Both producers now share `fixture_replication_order`, which asserts the
purpose-issued namespace. The refreshed pair was emitted twice with identical
bytes, parsed and staged through the existing closed-output writer, and reviewed
before publication. Only instruction cases 14 and 15 and SDK route 2 changed:
the order ID, two provider signatures, their dependent attestation digests and
the affected instruction bytes. All schema names/hashes, field sets and other
cases are unchanged. No production wire layout changed in this correction.

The focused Rust selection passes six tests, including namespace and stale
signature regressions; the two exact-JSON diagnostic helper tests also pass.
The Kotlin instruction/SDK fixture suites pass 33 tests with no failures or
skips. Swift execution is unverified because its required
`NoritoBridge.xcframework` is absent; the native requirement remains enforced.

| Document | Previous SHA-256 | Corrected SHA-256 |
| --- | --- | --- |
| `instructions_v1.json` | `2f39f7d374f05835479a20483c57631d7e9bf45ac7a13165b67ef8194994b461` | `5b361ce3b0105832bbd303692cd98ac0a5448b9f8179a01fe156e38ec8adf6e0` |
| `sdk_v1.json` | `6cc11b47f04951c60c55aead322d2901dc135162b0cbc69cba8ca65f8f5a7cf5` | `c0adca505ef3156a2f700129e46863f2877449c8c301e68748e098ab9888d985` |

The generated-family identity capture under
`crates/iroha_data_model/tests/fixtures/musubi_generated_identity_frames.json`
remains unchanged. It qualifies a separate declaration-only migration and must
not be rewritten to absorb an active-codec identity change.

This refresh is local fixture evidence. The registered external-target checker,
source-bound release provenance and complete release/platform qualification
remain separate obligations.
