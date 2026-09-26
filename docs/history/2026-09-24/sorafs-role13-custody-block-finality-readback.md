# Role-13 custody block-finality readback prerequisite

2026-09-24, `optimizations`. The role-13 release-manifest source now has a
purpose-owned `read_current_release_manifest_custody_block_finality_v1` reader
in `crates/iroha_core/src/query/release_manifest_authority.rs`. It selects one
current committed State height, validates the exact deployment-scoped custody
history there, and pairs its block hash with that same State view's durable
Kura block and cryptographically verified revision-4 CommitQC. Stale height,
foreign purpose/deployment substitution, missing finality and forked artifact
fail closed. The returned pair has private fields and no wire representation.

This is deliberately a **raw custody plus block-finality prerequisite**, not a
production role-13 state source. The Core `MutateSorafsReleaseManifestAuthority`
handler still rejects every action. No native role-13 operation journal, exact
Reserve/Complete readback, successful executed Check proof, same-State
operation/custody join, or finalized application-state proof exists. In
particular, block finality cannot establish that a fixture-inserted custody row
was produced by a successful instruction. The focused test explicitly stages
such a row in a test-only State, confirms that an absent or forked CommitQC
cannot authorize even this read, and confirms no operation journal is created.
The signer state-source and production release-manifest dispatch remain closed.

Source formatting and scoped `git diff --check` passed. The focused Core test
`role13_current_raw_custody_requires_exact_block_finality_and_remains_non_authoritative`
passed (1/1) with `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test
-p iroha_core --lib <selector> -- --nocapture`. The final fixture assertions of
the `must_use` finality receipt's exact height and block hash passed in a fresh
merged Core build; the adjacent role-11 selector passed in the same test binary.
Authenticated software custody is sufficient for the eventual role-13 service;
there is no HSM prerequisite or first-release compatibility path.
