# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-1d405def3cb83a352a937bf9bc56bb174880bccda61c1085d96d75a1356c4afe"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Autoscale scale-in cleanup now has same-block adversarial coverage proving
  emergency validator overrides, public-lane economic rows, public validator
  terminalization, and AXT replay ledger rows staged earlier in the block for
  the managed elastic lane being retired are cleaned from both the block overlay
  and committed world state, while surviving default-lane rows remain intact.
  That same-block economic cleanup now also covers surviving-key stake-share and
  reward rows whose embedded record lane is the retired autoscale lane, while
  preserving reward-claim cursors as key-owned state because they carry no
  embedded lane.


<a id="record-f2cc68260640836018af3f58d69199d2468133b81d0f965df38e67a65202186b"></a>

- Committed autoscale lifecycle publish now mirrors that persistent reset cleanup
  after the world block overlay has committed, covering AXT replay rows, DA
  pin-intent indexes, public-lane validators/economics, verified relay contract
  state, emergency overrides, and embedded retired-lane stale rows without
  mutating WSV storage while the committing world overlay is still live. The
  focused regression now reaches that cleanup through `StateBlock::commit()` so
  future autoscale commit hooks should keep persistent WSV block mutations after
  `world.commit()` to avoid storage lock inversion.


<a id="record-5f41cd14b1f7869fe1f5b3114a9ce2c827c342382174158d2eb32d95815921a3"></a>

- Commit ordering now keeps fallible autoscale geometry reconciliation before
  transaction commit, but publishes the autoscale catalog/runtime reset and
  staged DA commitment, pin-intent indexes, block-hash log entries,
  latest-header/query-index markers, commit-topology cells, or verified relay
  hydration only after a registered transaction block commits successfully.
  Staged autoscale/DA/verified-relay side effects with `MissingInsertBlock`
  abort before geometry, WSV cleanup, or relay-cache hydration, and WSV-only
  commits without an inserted transaction block no longer advance block metadata
  caches. Future commit hooks should preserve that boundary so aborted
  transaction commits cannot leak lane lifecycle, DA runtime state, verified
  relay authority, or topology-derived lane authority.


<a id="record-2c63ec9a37cb0d846d561bf5158b3b2b144d89118a3101033391a4342c7c698b"></a>

- Operator config-swap cleanup now has matching DA pin-intent world-index
  coverage: `set_nexus` lane retirement must remove reset-lane ticket, alias,
  manifest, and lane-epoch indexes while leaving active-lane indexes intact.


<a id="record-1b26f94ae692f4df86c0a1b1c3dfe9e663d5bb4fb9da0bb1af848deaa201e19a"></a>

- Public-lane Torii reward aggregation now has adversarial coverage for
  key/record mismatched reward rows, proving user-visible pending-reward results
  ignore forged embedded lane or epoch values while valid later epochs remain
  payable up to the requested bound.


<a id="record-472adb9c1a66cc4f466c0c781a82d8a9f8b6bbfb4ca36e130585452c2ce88b96"></a>

- Confidential-v2 SDK note derivation and encrypted note payload handling now
  exist for Kotlin/JVM, Android Java, and Swift, with Rust-vector parity for
  owner tags, note commitments, nullifiers, asset tags, and chain tags,
  Rust-fixture parity for the `ConfidentialEncryptedPayload` wire envelope,
  low-order X25519 public-key rejection parity, canonical ciphertext-length
  rejection parity, a 64 KiB encrypted-note ciphertext cap, and a shared
  deterministic X25519/HKDF-SHA256/XChaCha20-Poly1305 plaintext vector. Swift
  now also exposes the typed confidential transfer witness/request builders
  and verified-fold top-up bundle builders, including the
  confidential-transfer-v2 verifier-record archive with packed direct fixed32
  fields, length-delimited generic fixed32 `Vec`/`Option` payloads, marked Iroha
  schema/envelope hashes, framed delegated `AssetDefinitionId` bytes, omitted
  absent trailing attachment defaults, and a four-byte `u32` `status`
  discriminant despite Rust's `ConfidentialStatus` `repr(u8)`. Default
  decryption binds the plaintext
  owner tag to the supplied spend key, while diversified notes must use the
  explicit expected-owner-tag overload. Keep the higher-level wallet flows
  pinned to this contract when wiring shield-note recovery into production
  clients. The focused JVM and
  Swift SDK runners plus SDK parity guard now execute and pin the
  encrypted-payload model tests, confidential-note contract tests, Merkle-path
  parser tests, witness/request builder tests, and verified-fold top-up tests.


<a id="record-ec09d27b97fbbfbefedd9730ff3ceeb76536e64253671ca2991d1bfb42d79661"></a>

- Confidential-v2 JVM/Android proof assembly now has typed transfer and
  unshield witness/request codecs for the production native bridge. Keep wallet
  integrations on these builders instead of raw witness bytes so canonical
  u128 values, fixed 32-byte fields, bounded commitment trees, duplicate
  input rejection, transfer/unshield shape separation, exact verifier
  references, public-input schema constants, and the native Norito witness
  alignment padding remain pinned by SDK and Rust golden-vector tests.
  The SDK parity guard now pins the Kotlin/JVM and Android Java witness codec
  sources plus the typed native-ready request tests, with exact diagnostics for
  verifier-ref mismatches, empty and oversized verification proofs, missing
  transfer outputs, transfer outputs on unshield witnesses, duplicate input
  leaves, and out-of-range leaves, plus native request-archive errors for empty
  inputs, oversize inputs, and empty privacy request payloads, and privacy
  proof-request component errors for null algorithm ids, empty public inputs,
  and oversized witness bytes. Its
  workflow-routed negative control mutates those diagnostic markers so softened
  witness-builder and proof-request assertions are caught. The mobile harnesses
  also exercise the unshield verify request builder, empty unshield-proof
  rejection, and transfer/unshield verifier-ref separation, and the privacy JVM
  runner compiles these Android privacy harnesses from the Android plus Norito
  sourcepath so codec dependencies cannot fall out of direct `javac`
  verification. C# now emits parameter-specific managed proof-request
  diagnostics before native dispatch, including `proof must not exceed 33554432
  bytes` for confidential transfer/unshield verifier requests. The C# privacy
  proof-request archive tests now also pin exact `requestArchive` diagnostics
  for empty, oversized, malformed, wrong-schema, and empty-payload request
  archives before native dispatch. The SDK parity guard mutates the C# source,
  test, request-archive diagnostics, and component diagnostic markers.
  The Ubuntu/Windows C# SDK matrix must keep the same C# confidential
  transfer/unshield verify request proof-size prechecks.
  verification.
  The 2026-06-29 Windows `.NET 8.0.422` full C# pass certifies matching C#
  confidential transfer/unshield verify request proof-size prechecks for
  `proof must not exceed 33554432 bytes` on `win-x64`.


<a id="record-33c58d1eccf5c9a69a04a1aba526d6d06b18c8d3d4b6e48ef6fd98e11c66cadc"></a>

- Native asset locks are now first-class ISIs for escrow-style conditional
  custody, including optional release authority, expiry, partial drawdown,
  deterministic custody, Python SDK helpers, negative/adversarial unit tests, and
  a 4-peer localnet coverage path. Keep future escrow work on this native
  instruction surface unless it explicitly needs IVM contract semantics.


<a id="record-33eb1698276b1fac63796eab549f1884bde8b4d230514fd9048778799a62cfbd"></a>

- Soracles provider statistics now expose deterministic inlier-share reputation
  scores in basis points plus clamped governance deltas for off-chain
  scheduling/governance consumers. Current oracle aggregation intentionally
  remains equal-weight median/percentile until a governed weighting policy is
  adopted.

