# Rust BFV construction boundary

Date: 2026-09-29. The fixed candidate passed 348 native crypto controls and
15 default API compile-fail doctests. The subsequent containment candidate
also passed focused Core/model/Torii qualification, linked below; integration
qualification remains separate. No replacement encryption security is claimed.

Seven generic seeded key-generation/encryption functions are crate-private.
Their former root and `fhe_bfv` module imports are compile-fail boundaries;
the client crypto facade no longer exposes them either. Diagnostic tests use
`iroha_crypto::bfv_test_fixtures` explicitly. Only Core and data-model development
dependencies request `bfv-test-fixtures`; no workspace feature forwards it, and
the daemon's normal/build feature tree excludes it. This is a Cargo development
convention: explicitly requesting the feature, including `--all-features` or builds unifying development test targets, makes
the diagnostic namespace available. It is not an unforgeable test-only sandbox.

All existing arithmetic/vector/adversarial assertions remain. Crypto tests call
the fixture wrappers; Core and data-model tests migrate imports. The initial
inventory missed a split Gossiper test using the removed BFV policy builder.
Normal component compilation found that omission. The repaired fixture uses a
supported HKDF policy, retains its hash and codec assertions, and explicitly
checks that the signed transaction exceeds 64 KiB. Its checked signer control
remains. No syscall, opcode or production refusal is feature-gated.

The obsolete encrypted email load path is removed. Its six former selection
spellings return `ram_lfe_encryption_unavailable` before instruction registry,
private key or network setup. Transfer loads no longer construct BFV material.
Independent account, fee, route-count and receipt-signature fixtures remain;
the receipt fixture uses explicit synthetic typed metadata and is never admitted
or submitted as successful encrypted execution. Retired RAM-only throughput
artifact fields are removed.

This seven-function stage does not remove every BFV arithmetic operation from
shipping code. At this captured stage, private exact arithmetic and public zero-refresh, bootstrap, Galois
and rotation material constructors remain and require their own boundary review.
The public refresh ciphertext fields can still be composed with plaintext
arithmetic; this is a seven-function API cut, not removal of every possible
ciphertext-construction path.
The exact profile's noiseless equation modulo the plaintext modulus is a known
security defect. The rounded replacement remains unqualified. Neither may be
used to infer that production RAM encryption is available.
The subsequent specialized-constructor and Soracloud containment amendment has
its own source and validation record in
[`bfv-soracloud-production-containment.md`](bfv-soracloud-production-containment.md).

Source inventory, before/after amendment, locked Cargo metadata and the daemon
shipping feature tree are retained under ignored
`dist/zk-remediation/2026-09-29/bfv-rust-api-retirement/`. Cargo.lock is unchanged.

The original amendment passed normal locked, offline Cargo compilation and all
275 selected BFV and 73 RAM controls (zero failures or ignored controls), plus
15 independently compiled public-API rejection doctests. Its immutable binary
SHA-256 is `324910c2d4948f7d310f7526c8757ac9d2b2ec0153a9d676333cd6136cd7a6b8`.
All 20 candidate-local compiler artifacts were rebuilt, including local
proc-macros; their actual paths and hashes are retained. An explicit print-only
conformance fixture generator was excluded rather than run as a test side effect.
Evidence: `dist/zk-remediation/2026-09-29/bfv-api-native-20260929T083304Z/`.

A subsequent one-constant `cfg` correction removes an unused fixture-only domain
from shipping compilation. Normal compilation then emitted no warnings; both
directly affected identifier controls and all 15 API doctests passed. This narrow
rerun does not claim that all 348 native controls were repeated. The new immutable
binary SHA-256 is `c23880a13c175987c922917708529651d5f7d8eec65cccb8766d2435d1353125`.
Evidence: `dist/zk-remediation/2026-09-29/bfv-api-clean-20260929T085141Z/` and
`bfv-rust-api-cfg-hygiene/`. Both stages retained unchanged full source manifests
and immutable test binaries. The component candidate also imports the separately
reviewed two-file typed Torii unsupported-backend error amendment; it has its own
manifest and will receive normal component compilation and native controls.

Three component compilation failures are retained: the missing split-test
helper, a missing `Instruction` trait import, and an incorrect attempted `From`
conversion. The final source correction imports the actual trait and uses its
`Box<Self>::into_instruction_box` method. No test assertions were removed.
Each failed candidate had an unchanged source guard and retained the hashes of
successful dependency artifacts; no component test pass is claimed from these
attempts.

The corrected dedicated-target Core/model/Torii build passed in
`bfv-api-components-20260929T093117Z`. Its subsequent native stage retained
777 passes and two failures, with unchanged source and immutable binaries.
Core passed 349 of 350 controls; the shared-lease setup lacked a retained FASTPQ
producer invocation. Torii passed 35 of 36; an HKDF encrypted-route fixture still
expected the BFV-specific error code and retired internal message after the
reviewed public error mapping changed. The model substring invocation passed
393 controls: all 382 mandatory model tests plus 11 instruction tests. A separate
execution-name audit proves all 768 mandatory controls ran exactly once with
zero ignored tests and retains the extra names; it does not overwrite the
original failed result. The integration test binary separately failed to compile
because two smoke-test imports/diagnostics were stale.

The reviewed lease repair uses the existing component transaction owner while
preserving fee, sequence, expiry and asset assertions. The integration repair
uses the actual query import and committed block-sync duration. Those four test
files, plus three test-logging/dependency files, form an explicit seven-file
predecessor overlay in `bfv-api-component-fixture-repairs/`; whole before/after
manifests and old/new hashes are retained. Kagami's independent signed-fixture
diagnostics own that amended candidate next. A separate main-only Torii fixture
amendment now checks the intended 503 status, backend-specific reject header and
canonical Norito error envelope; it was not silently folded into the seven-file
handoff. Normal repaired component/integration retries remain pending.

The later specialized-constructor containment stage supersedes the open focused
component result without changing these historical failures: 349 native crypto
controls and 37 API import rejections passed, followed by 367 Core and 396 model
controls. A separate reviewed one-test Torii correction then passed all 36
controls with exact source, binary and local dependency lineage. See
[the containment record](bfv-soracloud-production-containment.md) for manifests,
retained failed predecessors and the final Torii binary. This does not promote
the diagnostic arithmetic or qualify the separate integration/network work.
