# BFV construction and Soracloud production containment

Date: 2026-09-29. Source amendment and genuine Norito schema capture are
complete. Crypto, default API-boundary and focused Core/model/Torii controls
pass on the retained isolated candidate sequence. This record does not qualify replacement
encryption or a complete private execution relation.

The exact BFV profile exposes a noiseless public-key equation modulo the
plaintext modulus. Adding attestation, a complete execution proof, or more audit
evidence cannot repair that encryption defect. The rounded construction remains
unqualified. No recovery experiment was performed.

Ten specialized seeded constructors join the seven generic constructors already
retired from the public production API. Galois, bootstrap/zero-refresh,
sample-extraction switch-key and outer-slot rotation fixtures remain available
only in the explicit `bfv_test_fixtures` development namespace. The authoritative
arithmetic implementations remain crate-private; no opcode or syscall is gated.
Twenty additional independent constructor import rejection doctests cover both
former root and module paths. Two further controls reject imports of the retired
blocker type; the default boundary suite has 37 controls. Existing arithmetic,
vector and adversarial assertions are preserved through explicit fixture calls.

This visibility boundary does not prevent callers from creating public polynomial
structs or composing raw arithmetic. It does not make raw arithmetic secure.
Production policy and state admission therefore refuse FHE independently.

The model's local `SoracloudManifestError::FheUnavailable` has no wire derive.
Plaintext and client ciphertext storage remain supported. FHE policy registration
and rotation, job execution, state upserts and secret envelopes reject. A generic
FHE secret label rejects even when no BFV policy is attached. Deployment, upgrade,
rollback and snapshot restore check declarations; restore rejects retained FHE
policy history, secrets and state rows. Core maps the typed local error through
its existing invalid-parameter family with the fixed `soracloud_fhe_unavailable`
reason. Snapshot restore uses its existing invalid-state error family.

Permission and provenance checks retain their meaning. The new controls exercise
ordinary public refusal before missing/malformed input processing, unchanged
state/audit records, rollback refusal, and authenticated state/secret cleanup.
Policy revocation remains exercised by the original lifecycle test. Diagnostic
metadata tests first assert production refusal, then call only the existing
private validation functions; they perform no alternative job execution or
output writes. Explicit diagnostic rows seed historical metadata, never a
successful public FHE admission. Independent snapshot invariants use supported
client-ciphertext fixtures, with separate unsupported-FHE restore negatives.

The published production blocker changes from an obsolete missing-evidence reason
to `BfvProductionSupportBlockerV1::KnownInsecureExactProfile`, with explicit wire
tag 1. The former type is removed without an alias or fallback. The first native
control correctly failed the attempted variant-only rename: Norito identities
are declared nominally, so its frame and raw tag 0 still admitted the old value.
That failure is retained. The repair changes the nominal identity and numeric
tag, with independent retired-frame, raw-tag-0 and JSON-spelling negatives. The
corrected native control passed and emitted schema identity
`062ac90b1bf1a793353c52dd1fc4715a`. Its immutable binary is
`4e2983ce84c66d688cf1e5e694ecea3a77d15f16161e4f5a59fe81b4d2c3de20`.
The sole affected captured-schema row was regenerated from that output; the
rebuilt codec/native suite passes against that fixture. The failed
variant-only attempt, corrected native output and one-row fixture amendment
are retained in `bfv-fhe-schema-native-20260929T094730Z`,
`bfv-fhe-schema-native-20260929T095531Z` and `bfv-fhe-schema-capture` under the
same ignored evidence root. No hand-selected hash or compatibility reader is
used.

The production-only gate diff was independently source-reviewed. Full amendment
and fixture changes are in ignored
`dist/zk-remediation/2026-09-29/bfv-soracloud-retirement/`. The older
`bfv-api-qualification` candidate remains separate and frozen for its own
component, SDK-fixture and network qualification. No result from that predecessor
qualifies this new amendment.

The separately reviewed release-feature gate rejects `iroha_crypto`'s
`bfv-test-fixtures` feature in shipping dependency graphs. Its three-file checker,
regression and bootstrap amendment passed four feature-gate controls and 22
bootstrap mutation/swap/directory controls. The exact bootstrap checker digest
was updated; the pinned whole-release source seal was not broadly regenerated.
The actual isolated Python 3.12 checker rejects the existing changed whole-source
seal before running Cargo metadata, as required. A prior Python 3.9 environment
failure is retained separately. This qualifies the narrow guard, not the current
shipping graph or a release build. Evidence and independent source review:
`dist/zk-remediation/2026-09-29/bfv-release-feature-isolation/`.

The new candidate's normal default-feature crypto build and controls passed in
`bfv-fhe-native-20260929T095952Z`: 276 BFV and 73 RAM-LFE native controls,
plus 37 independent default-feature API compile-fail doctests, with zero ignored
tests. The retained native binary SHA-256 is
`5d6503f039f3c1ca0baca651b6baee1f9141479d4f94897caf74b30675f391f7`.
The whole 20,739-file source manifest remained unchanged through execution;
its SHA-256 is
`f8a9a4e6c0089774ba7dd6cd5ef98ce44458d97a37362958205352617024474e`.
All 20 compiler-emitted local artifacts have checked lineage: 19 reused path/hash
pairs match the preceding isolated build and the crypto test binary rebuilt.
The unique target began empty at the first schema attempt. No object from the
shared debug cache was imported. The subsequent ordinary doctest command passed;
its log is retained separately from the compiler JSON artifact inventory.
The native command excludes only the explicit print/regeneration entrypoint,
not a correctness test. Total build/control time was 931.839 seconds.
These results qualify the scoped crypto hard cut and fixtures. The separate
component results below cover containment paths; replacement encryption, full
private execution, network behavior and release qualification remain open.

The subsequent component snapshot adds only the independently reviewed lease
transaction-owner fixture and two integration smoke compilation fixes, with no
crypto or production changes. Its manifest is
`2b9035f71e615f5f0db03597b6725b35d90d0ccd39de978a77c9078f471de7c6`
(`bfv-fhe-component-fixture-repairs/source.json`). All five codec guards and
scoped formatting checks pass with unchanged source, recorded in
`bfv-fhe-component-hygiene/result.json`. Ordinary Core, model and Torii test
compilation passed in 1,166.563 seconds in
`bfv-fhe-components-20260929T101705Z`, using the same candidate-specific target.
All 73 local artifacts have verified lineage, including ten reused outputs.
Immutable binaries and their hashes are retained. Native execution finished in
4,464.812 seconds with 798 passes and one failure, zero ignored and unchanged
source/binary guards. The name audit proves all 367 mandatory Core controls
passed exactly once, including all 302 broad Soracloud tests. Model execution
passed 396 controls: all 385 mandatory names plus 11 explicitly retained substring
matches in instruction tests. Torii passed 35 of 36; the sole failure was the
reviewed stale HKDF error-code assertion. The original failed receipt and exact
execution-name audit remain in `native-controls/`; no failure is relabeled.

Only that reviewed test file was then overlaid with current modification time.
Normal Torii compilation and all 36 exact controls passed, zero ignored, in
`bfv-fhe-torii-fixture-retry-20260929T115654Z`. Build time was 894.444 seconds;
total time was 954.988 seconds. All 71 emitted local artifacts have verified
lineage (44 rebuilt, 27 reused exact paths/hashes). The amended source manifest
SHA-256 is `ac7741554a10fb026c4af6f1b107eb618c6f071b68fd6c509f8fcb82eef7e5dd`;
the retained binary is
`1431a6c1f3ae370c41d28de6ca4596ef5845f150b2208556c2e0d0a91d55a8e1`.
Source and immutable binary guards pass. The native assertions distinguish
known-insecure BFV and unsupported HKDF encrypted-route 503 responses, their
stable headers and canonical Norito envelopes, and generic 500 error redaction.
The preceding Core/model binaries retain their exact predecessor scope; their
sources and production behavior did not change in this one-test amendment.
This closes focused containment component qualification, not replacement
cryptography, a complete private relation, network behavior or release readiness.

A later main-only three-comment amendment removes public rustdoc links to retired
constructors and states the known-insecure gate directly. The non-doc source is
unchanged, scoped formatting passes, and the 17-name link inventory has no public
reference to a private constructor. Its evidence is
`bfv-public-rustdoc-retirement/`; running candidates retain their captured
pre-comment bytes. No duplicate arithmetic run is claimed for that cleanup.
