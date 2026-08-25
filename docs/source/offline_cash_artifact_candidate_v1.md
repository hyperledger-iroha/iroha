# Offline Cash V1 developer artifact candidate

`offline_cash_artifact_candidate_v1` is an opt-in `iroha_core` developer binary
for generating the complete 34-role artifact inventory. Its output contains
transparent IPA parameters and processed Halo2 proving/verifying keys only. It
does not accept, generate, load, or store release-authority signing keys, and
its output is not an authenticated release.

Run it with an explicit, canonical, absolute directory that does not exist:

```sh
cargo run -p iroha_core \
  --bin offline_cash_artifact_candidate_v1 \
  --features dev-tools,zk-halo2-ipa \
  -- --out-dir /absolute/path/offline-cash-v1-candidate
```

The generator holds intermediate payloads in owner-private unlinked files. It
then writes all role files into an owner-private sibling staging directory and
publishes the directory with one final atomic no-replace rename. Existing targets, partial role
sets, over-cap payloads, changed spool bytes, unsafe file names, and ambiguous
or relative paths fail closed. A failure before rename leaves no final output;
a failure syncing the parent after rename is reported as durability-uncertain
and does not claim rollback.

The directory contains one `.bin` file for every
`OfflineCashArtifactRoleV1::ALL` entry, in that canonical metadata order, plus:

- `offline_cash_artifact_manifest_candidate_v1.json`, with the profile digest,
  per-role file/protocol digests, lengths, and artifact-set digest;
- `offline_cash_artifact_manifest_candidate_v1.sha256`, the SHA-256 of the exact
  newline-terminated JSON bytes.

Both records say `unauthenticated_developer_candidate` and
`not_qualified_not_attested_not_promoted`. They intentionally omit a release
ID, release attestation, authority signatures, and promotion receipt.

Before publication, the generator also constructs and verifies the complete
common-k16 recursive graph. It proves all fixed GuardBundle children, folds and
binds both parities, proves both GuardBundle wrappers, folds the State graph,
rechecks both reciprocal audit digests after final binding substitution, and
generates both final State proofs. Each GuardBundle proof must match its pinned
3,264-byte shape and each final State proof must match its pinned 3,072-byte
shape (within the 3,200-byte final-wire hard cap); exact transcript readers
then terminally decide each State outer accumulator and circuit-bound carried
lineage. These randomized, zeroizing qualification proofs are never written as
release artifacts and confer no authentication authority.

Before a separate release corridor may authenticate or promote these bytes, it
still needs reviewed source-tree and `Cargo.lock` identities, finalized hardware
policy identity, four-validator restart/replay/adversarial receipts, required
fuzz and performance evidence, physical-device evidence, and the locally
trusted release-authority policy plus threshold attestation. The developer
binary performs no network or live-Taira mutation.
