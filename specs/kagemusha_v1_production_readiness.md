# KAGEMUSHA V1 production readiness

Source/security assessment started 2026-09-05. KAGEMUSHA is **not production
qualified**. This record tracks implementation and validation work; it is not an
independent cryptographic audit, hardware certification, or authorization to
enable an offline monetary profile.

## Release goals

1. Close monetary proof authority: constrain original recipient credentials,
   plaintext openings, finalized reserve credits, normalized hardware guards,
   replay insertion, and successor state inside the actual recursive relation.
   Generate and verify real proofs using the authenticated final artifacts.
2. Finish the durable product coordinator: connect private state/proof snapshots,
   authenticated history, sealed preparation, hardware commit, inbox, outbox,
   finality, and recovery. Every caller retry must recover the original operation
   and every exposed byte; absence of qualified hardware must remain unavailable.
3. Complete SDK interoperability: caller-persisted request/payment/mint/redemption
   identities; exact canonical reservation bytes; identical response binding;
   current-source native artifacts; Swift, Kotlin, Java consumers and remaining
   SDK conformance. A syntax check or mocked provider does not qualify native use.
4. Close security and performance evidence: 1,024 real recursive handoffs,
   1,000 independently funded balances through aggregate spend and redemption,
   four-validator settlement/restart/replay, complete adversarial crash matrix,
   fuzzing, workspace tests/Clippy, independent review, and measured device limits.
5. Qualify and enable each exact device profile only after all preceding goals
   pass for the immutable release candidate. Do not infer support from brand,
   operating system, successful signing, or application installation.

Proof ceilings remain 6,528 paired-proof bytes and 9,211/12,288 complete raw/text
exchange bytes. Device gates retain 128 MiB process RSS, 10 s proving p95,
1 s verification p95, and 30 s handoff p95. These are required acceptance limits,
not current achieved measurements or a claim of optimality.

## Active implementation — 2026-09-12

Work remains confined to `/Users/takemiyamakoto/dev/iroha`, branch
`optimizations`. The encrypted polynomial-store foundation, bounded
write-once advice assignment and single-column basis conversions are now applied. The store authenticates the proof
context, field, basis, column/phase, ordinal and chunk geometry. Its 512-handle
limit counts live owners; dropping a predecessor releases capacity without
reusing its ordinal. Assignment buffers one 256-row numerator/denominator chunk
per column, preserves rational zero-denominator behavior and ordered tail
values, and destroys the active writer after an operational error or unwind.

The existing prover still owns its full scalar banks. These new components are
not yet a stored consuming prover, and their logical buffer bounds do not
establish full-process RSS, latency or production qualification. The actual vendor suite passes 25 tests in each of default and no-multicore
configurations (50 executions), covering metadata, both-field assignment
comparison, all basis-conversion pairs and failure/unwind handling. The new
basis conversion retains one guarded field column and uses the existing baseline
FFT arithmetic, preserving current prover dispatch. The encrypted Core adapter
passes all 16 tests for authentication, owner lifetime, assignment and basis
conversion on the freshly rebuilt development harness. All 1,827 captured
Core/Crypto/Axiom/curve and build/lock inputs remained unchanged throughout
compilation and execution. These results do not qualify the
existing MSM scratch lifecycle or any complete proof.

A later 20-path implementation batch is applied after the validator source
capture. It adds ordered IPA phase commitments, bounded expression and
retained-graph tiles, MSM scalar-encoding cleanup, Base assignments returning
only actual cells, and monotone native Poseidon BUS emission. The graph helper
caches advice tiles and retains one row of graph intermediates. The BUS retains
one block of guarded endpoints and replays its original copy order.

The batch also includes a strict internal single-phase Assignment bridge and
consuming completion into globally ordered receipts. Unknown witnesses,
reference-return requests, backward writes, ignored instance errors and premature
phase changes invalidate the whole assignment owner. Selector/fixed/copy calls
retain the ordinary prover's PK-frozen semantics, including direct selector
conversion that removes the original selector count. Completion preserves the
original snapshots and guarded blinds; a failed or unwound bounded read destroys
all receipts. These primitives do not yet enforce concrete producer/key admission
or final-producer destruction before commitment. The consuming caller must also
destroy the owner on an external synthesis unwind.

The required full-proof and device limits remain unchanged. The complete stored
prover, original instance-prefix integration and producer memory work remain
incomplete. All 77 new regression tests are prepared, but compilation and
execution await the shared build resource window. The earlier
50 vendor and 16 Core executions above precede these changes and do not validate
them. Formatting, source review and patch checks are not proof equivalence.
Application and static receipts are under
`target/kagemusha-validation/stored-prover-next-window-20260912`.

Before this batch, the native enrollment library and test sources passed a
separate `cargo check`. That check does not validate these later Core/vendor
changes; linked test execution and native artifacts remain pending. The six package
surface tests pass after correcting two stale JavaScript public-subpath
expectations, preserving all coordinator enum and fixture checks. These scoped
receipts are copied under `target/kagemusha-validation/mobile-taira-reconnect-20260912`
and `target/kagemusha-validation/package-public-path-20260912`.

The explicit Apple local-integration build path now accepts the current root
lockfile while retaining locked/offline builds and authenticated source/tool
boundaries. Its fixed owner-only output paths remain inside this checkout.
Every resulting artifact is marked `local-integration`; archive and release
publication reject that scope. This enables native integration work without
conferring canonical release provenance. Fourteen local-policy tests, all 29
archive tests, 24 source-seal tests, 12 Swift-pin tests and 19 strict validator
tests pass (98 in total). Native artifact
construction, complete Swift execution and canonical release qualification
remain pending. The archive tests use an isolated fixed source graph and
independently encoded expected ZIP bytes; release path guards remain enforced.

Current application and execution receipts are retained under
`target/kagemusha-validation/20260912-source-window`,
`target/kagemusha-apple-archive-fixture-followup` and the Apple follow-up test
records. Prior source-bound results below remain historical scoped evidence.

## Current validation boundary — 2026-09-08

The canonical checkout is `optimizations`. The completed R5 SDK chain at
`e7a8083753a46bad47535816fff7fe7d29ba8b05` includes freshly invoked host bridge
and fixture-generator builds, 16 direct balance-key fixture cases, 91 focused JVM
tests, 1,490 full JVM tests, 12 managed wallet Android tests and 108 managed client
Android tests. All executed tests passed with no skips; focused and full JVM
counts overlap. Its source and artifact boundaries were retained. The resulting
five repair files were committed in `8c322866d0556060f4794ab5d178af2acaba1a92`.
A separate Android host JNI test also passed actual software-key generation,
reload, signing and verification. These are host/managed SDK results, not device
or monetary qualification.

After the release71 source hold ended at
`f0322420c46aa6cfc35b37b4b0c4abe57457817f`, the Kotlin retry-archive correction
below passed all 13 focused tests. JavaScript Parliament parity passed all 24
tests, including the TypeScript consumer check for `RegisterInitialSortition`.
The corresponding Swift source and tests were restored to the canonical checkout
after the clean native checkpoint. The required canonical ABI-23
`dist/NoritoBridge.xcframework` is absent, so Swift execution remains open.
Neither focused SDK result rebuilds native proof authority for later Core changes.

The reviewed vendor memory candidate is applied. Its actual default and
no-multicore runs each passed 45 regular tests, 12 additional proof-test
executions and 12 lookup-test executions: 138 executions in total. The runs
retain exact source, executable, recipe and index boundaries. Test-execution
counts are not counts of constructed proofs. The exact Core structured-key
overlay passed 92 regular Core tests and two separately executed row-emission
benchmarks from the shared native build. All 54 journal/recovery tests also
passed. The real-proof API check exposed a test-only helper guard; extending
that guard to the existing profiling feature fixed the compile, and the focused
retry passed. The method body and tested behavior are unchanged. This is
focused validation, not a workspace or warning-free Clippy result. The separate
Base lane passed 14 regular tests, including small BN256/KZG key and seeded-proof
equality against the original map, and two separately executed benchmarks.
Its offline lock generation left the then-current root dependency graph
unchanged; one test-only trait import fixed its initial compile failure. The historical 5,302,980-cell map benchmark retained an identical
checksum while process RSS fell from 557,727,744 to 4,145,152 bytes. These are
unoptimized host map measurements. The separate 1,008-source Core row benchmark
retained the exact 131,046-row checksum while peak RSS fell from 189,562,880
to 28,262,400 bytes; both runs took about 52.6 seconds. Neither benchmark
measures full-proof or device resources.
Full-process memory/latency and the required genuine aggregate proofs remain
open. Local receipts are retained in
`target/kagemusha-main-native-jvm-validation-r5`,
`target/kagemusha-sdk-security-parity-validation-r1` and
`target/kagemusha-validation/kagemusha-combined-memory-r2-actual-20260908` and
`target/kagemusha-proof-work-inventory/base-test-lane/actual-r2`,
`target/kagemusha-validation/kagemusha-core-structured-shared-r2-20260908`,
`target/kagemusha-validation/kagemusha-wal-shared-r1-20260908` and
`target/kagemusha-validation/kagemusha-api-cfg-retry-r1`.

## Requested device scope

The priority families are iPhone, Samsung, Huawei, Google, and Meizu. Other major
brands follow the same exact-profile qualification path. No model/OS/firmware/
provider tuple has been qualified by this assessment.

A read-only host inventory on 2026-09-07 found one Android emulator and no
physical Android target. All remembered Apple targets were unavailable. This
check establishes local device availability only; it performs no attestation or
hardware qualification.

| Family | Integration work and evidence required |
| --- | --- |
| iPhone | Swift/native integration, authorized provisioned secure-element service, exact supported model/OS/territory, and physical qualification. |
| Samsung | Kotlin/Android native integration and an authorized service implementing the complete non-forking device contract; qualify each model/firmware. |
| Huawei | Establish the exact Android or HarmonyOS application/runtime and authorized service, then build and qualify its native integration. Android coverage does not establish HarmonyOS coverage. |
| Google | Kotlin/Android native integration and an authorized non-forking service; qualify exact Pixel/model/firmware profiles. |
| Meizu | Establish model/OS/native runtime and authorized service availability, then run the same full qualification. |

Apple's NFC & SE platform requires an agreement and entitlement; access alone
does not establish KAGEMUSHA contract compliance. See Apple's
[platform requirements](https://developer.apple.com/support/nfc-se-platform/).
Android's rollback-resistant **key deletion** semantics do not specify an
application's aggregate-balance compare-and-swap journal. The latter remains a
separate KAGEMUSHA requirement; see the Android
[KeyProtection contract](https://android.googlesource.com/platform/frameworks/base/+/master/keystore/java/android/security/keystore/KeyProtection.java)
and [device bridge contract](kagemusha_device_bridge_v1.md).

## Security findings and implementation work

- **KGM-22 — Corrected; focused retry-archive validation passes.** The Kotlin
  intent and historical qualification decoders previously entered generic
  decompression before rejecting noncanonical compressed archives. Both now
  bound and snapshot input, inspect the header without decompression, require
  uncompressed zero-layout archives, and retain schema/checksum/complete-decode
  and exact re-encoding checks against that same snapshot. All nine existing
  intent tests and four added regression tests pass. Small malformed-header
  cases verify early rejection; enclosing decode-state and defensive-copy
  behavior are preserved. Historical qualification decoding still confers no
  hardware authentication. Source:
  [retry archive codec](../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaOperationIntentV1.kt).
- **KGM-23 — Fixed; 54 Core recovery tests passed.** Outgoing-index
  reconciliation alone did not pair a separately supplied coordinator WAL with
  the checkpoint-selected journal when only Reserve/BeginIntent records existed.
  Core now requires the selected frame count, hash and byte boundary to occur in
  the held, fully replayed WAL before serving it. Ownership/generation checks
  remain mandatory even for a cached match; valid appended suffixes remain
  recoverable. Creation rejects before touching a new path once selected history
  has advanced beyond the initializer. Three new regressions and 51 retained
  tests passed on the fresh shared Core harness. This source finding does not establish an
  exported-ABI or monetary exploit, and the fix does not supply hardware
  freshness or qualified speculative-suffix authentication. Sources:
  [coordinator pairing](../crates/iroha_core/src/zk/kagemusha_v1_state/coordinator_operation_store.rs)
  and [owned journal ancestry](../crates/iroha_core/src/zk/kagemusha_v1_state/private_journal.rs).
- **KGM-01 — High, monetary relation incomplete.** The original MintFold private
  recipient credential and credit opening were retained by Core but not fully
  constrained to the receiving lane and verified authorization in the composite
  relation. Host validation cannot replace these circuit constraints. The
  correction now constrains the recipient and opening bytes and routes State SHA
  messages through the mandatory authenticated ordered claim fold. Focused Rust
  checks and actual artifact/resource gates are still required before closure.
  Sources: [recipient/opening constraint](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L2612)
  and [claim consumer](../crates/iroha_core/src/zk/kagemusha_v1_recursion/composite.rs#L1705).
- **KGM-02 — High, operation recovery integration incomplete.** Swift exposed
  operations still allocated retry identities internally while Core had moved to
  caller-owned IDs; some SDK provider calls still used the retired allocator
  signature and untagged sender reservation bytes. A lost native return must not
  allocate a second monetary operation on retry. The caller-ID and canonical
  reservation changes now have focused Kotlin/Java/C# coverage, and 22 Swift
  coordinator tests pass against the pinned native host library. C# also now rejects missing-state re-bootstrap, journal rollback
  and recovery equivocation. Current-source native execution is still required.
  Sources: [Core reservation](../crates/iroha_core/src/zk/kagemusha_v1_state/coordinator_operation_store.rs#L274)
  and [C# ID admission/recovery](../csharp/src/Hyperledger.Iroha.Sdk/Kagemusha/KagemushaWalletV1.cs#L1014).
- **KGM-03 — Medium, bridge response substitution.** Outbox release admitted a
  structurally valid response for a different canonical installed envelope.
  Match the exact request envelope before returning backend results and preserve
  cleared C outputs on rejection. The fix and regressions are implemented; Rust
  passed in the initial 20-test native coordinator suite. The expanded typed
  archive/input/receipt boundary subsequently passed the pinned 32-test native
  coordinator suite, with all recorded sources unchanged.
  Source: [native response validation](../crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs#L552).
- **KGM-04 — Medium, physical clock rollback evidence missing.** The physical
  transcript could pass without exercising a host clock rollback and rejection
  of an expired request. The verifier now requires four explicit boundaries and
  the release report requires `clock_rollback`, including for signed reports.
  Focused mutation tests pass. Source:
  [clock boundary verification](../scripts/verify_kagemusha_v1_physical_device.py#L774).
- **KGM-05 — Release blocker, physical provenance closure.** The release manifest
  now requires the full raw transcript, OEM attestation, governed trust roots,
  independently pinned observer policy and native OEM verification report. It
  authenticates and reruns the fixed physical checker, binds the exact candidate
  and OEM challenge, and rejects changed sources before publication. Focused
  substitution tests and the isolated projector test pass; an earlier stable-source
  five-file run after mandatory governed provider-issuer authorization
  passed 216 tests and 50 subtests. The latest native-fixture closure passes 240
  tests and 138 subtests with all 19 recorded inputs unchanged.
  Actual admitted OEM verifiers, roots and
  physical runs remain required for every enabled profile. See the
  [exact closure contract](kagemusha_v1_physical_evidence.md) and
  [release verifier](../scripts/verify_kagemusha_v1_release_evidence.py#L2109).
- **KGM-06 — Medium, JavaScript model mutation.** A public internal-value getter
  exposed mutable WeakMap backing data behind frozen canonical models. The
  getter is removed and only internal encoders access backing values. Public
  projections are defensive, with nested mutation/canonical byte regressions
  passing. Source: [model backing boundary](../javascript/iroha_js/src/kagemusha.js#L195).
- **KGM-07 — High, sender-context and response-evidence integration.** Core and
  the bridge used the same sender-context schema name for different field sets,
  so full public-input digests disagreed; the coordinator also discarded the
  device response's original signature. Core now owns the shared context,
  including its authenticated Core key reference, and method 3 carries the
  original low-S signature as its fifth field. The new ten-field contract rejects
  the retired projection. Rust canonical fixtures pass Kotlin/Java parity and
  full provider tests retain the original authenticator. Native session admission
  and fresh Rust Core regression execution remain required before closure.
- **KGM-08 — Medium, canonical SDK and retained-operation boundaries.** Existing
  mobile context/receipt codecs differed from Rust's declared alias-field layout.
  Actual Rust archive fixtures exposed and corrected the mismatch. The native
  boundary now rejects opaque/wrong-schema archives, changed nested preparation
  or recovery IDs, mismatched input digests, and invalid complete signed release
  commands. Android terminal-envelope bounds and historical policy/key rotation
  cleanup passed focused Kotlin/Java tests. Native recovery now requires the exact
  installed bytes and terminal identity selected by its canonical device reply;
  those checks passed the pinned native coordinator suite, and six sender tests
  passed context/preimage parity, signed zero-amount rejection and receipt retry
  checks. Public archive validity
  never grants a durable operation, admitted signing key or finality capability.
- **KGM-09 — Release blocker, incompatible profile identities.** Native loader
  admission compared its digest of the exact circuit layout to a release digest
  of a different evidence report. An authenticated release therefore could not
  satisfy normal native loading. The signed validation receipt now carries a
  separate native-layout digest, independently derived by the report verifier
  and checked before native artifact reads. Current Python release/provenance
  tests pass. The pinned focused Rust build now passes 31 release tests, including
  the governed provider registry, canonical issuer signing bytes, signed approval
  chain and native-layout substitution. Actual generated-artifact loading and
  the final combined release still need execution. See
  [native profile binding](kagemusha_v1_native_profile_binding.md).
- **KGM-10 — High, incomplete Guard credential history.** The Guard consumer
  expected a two-cell credential projection after the credential circuit had
  moved to a complete 40-cell statement with SHA history. The consumer now binds
  all columns, folds both current credential proofs and their carried histories,
  and shares the exact audits across both proof parities. A native verifier now
  verifies complete monetary Guard proofs against immutable release-authenticated
  protocols and Core's caller-derived statement. The loader also rejects an
  empty-effect sentinel belonging to another release. Eleven Guard archive tests
  and the native Guard protocol-binding regression pass in the frozen lane;
  genuine proof acceptance remains pending. These monetary proofs cannot certify independent hardware journal
  transactions.
- **KGM-11 — Critical, unanchored credential policy authority.** The credential
  relation proves membership under its supplied hardware-policy Merkle root,
  but native release admission authenticates a different digest of the enabled
  profile list. The new Guard verifier must not treat proof validity under a
  caller-selected root as provider authorization. Native monetary Guard, State,
  mint authorization and terminal entry points now fail closed until the exact policy root is
  bound into the actual proof relation and authenticated governance evidence.
  Merely checking local Core inputs would not protect direct terminal admission.
  An enabled profile name, a nonzero policy digest, or
  self-consistent credential proofs do not supply that authority.
  The receipt now authenticates a bounded exact provider inventory and derives
  its Merkle root independently. Both Rust and Python require the exact hardware
  profile issuer's low-S P-256 signature over each public provider commitment and
  registry position. Python verifies the same root against the raw physical
  transcript and OEM report, and OpenSSL interoperability plus refreshed-approval
  substitution regressions pass. The fixed-root circuit and mint-authorization
  integration pass three provider-root and five mint generation/column/transport
  tests in the frozen dependency lane. Genuine-proof execution remains required.
  See [provider policy binding](kagemusha_v1_provider_policy_binding.md).
- **KGM-12 — High, detached sender credential validity.** Terminal authorization
  reconstructed private credential statements but compared only the normalized
  Guard digest, which omits credential issuance. Sender expiry therefore lacked
  an authenticated proof opening. Guard now exposes both complete credential
  digests at public cells `6..10`, with its history at `10..44`.
  Terminal and aggregate consumers bind those exact verified cells; terminal
  constraints hash the canonical compact credential and governed profile and
  require the original trusted commit, or entire half-open lease, inside both
  validity intervals. The issuance field is the exact compact credential ID.
  Native decoders reject the old Guard shape, and all descendant keys require
  regeneration. All six Eq/Ep canonical opening, substitution and complete-window
  regressions pass in the frozen dependency lane; genuine terminal proof
  generation remains unverified.
  New top-ups also enforce validity at the certified reserve commit time;
  exact retries authenticate the stored receipt at its original time and do not
  mint again after expiry. The 22 focused DataModel instruction tests pass,
  including both new boundary tests. All three new Core lifetime/retry regressions
  pass. All four corrected finality retry/cache/anchor/conflict fixture tests now
  pass in the second frozen Core run; actual qualified-device timing remains open.
- **KGM-13 — High, detached recursive assignment context.** The aggregate
  allocated Guard history cells through the builder after its virtual context
  had moved into the recursive loader. Finalization replaces that detached pool.
  Guard history and credential limbs now allocate in the loader's active context.
  A regression exercises the actual context transfer and rejects credential and
  history substitutions in both fields. That regression passes in the frozen
  dependency lane; real aggregate proofs remain required before closure.
- **KGM-14 — Release blocker, sender lifetime admission before hardware commit.**
  The new terminal validity checks also require the provider to reject an expired
  sender credential/profile or straddling lease before irreversibly advancing
  hardware. The current public op7 codec authenticates preparation identity but
  cannot supply that qualified admission. Otherwise hardware could commit a
  transition whose terminal proof is impossible. The device contract now requires
  atomic lifetime checks before mutation and historical recovery after expiry.
  The actual native structural exporter and five normal parser tests now pass,
  including replay of 18 canonical send/redemption commands across three fixture
  contexts and re-signed candidate substitution rejection. All source, binary and
  fixture hashes remained unchanged. The final five Python evidence suites pass
  240 tests and 138 subtests with all 19 recorded inputs unchanged. Both CLI
  boundary tests and actual binary replay of all 18 complete projections pass
  with unchanged source/binary/fixture hashes. These synthetic fixtures do not qualify
  service behavior or physical evidence. Stock dispatch still rejects, so this
  review did not demonstrate an enabled monetary bypass. See
  [the precommit contract](kagemusha_device_bridge_v1.md) and
  [physical qualification requirements](kagemusha_v1_physical_evidence.md).
- **KGM-15 — Release blocker, canonical prepared-transfer hash mismatch.**
  The terminal circuit placed the amount after both sender-state digests, while
  the canonical model places it before those digests. The valid prepared-transfer
  constraint regression failed. The correction must preserve the model's exact
  bytes and all component substitutions; a changed expected digest alone would
  hide the incompatibility. The corrected both-field regression now passes with
  exact model bytes, all component substitutions and rejection of the old byte
  order. This fixed snapshot took 624.28 seconds for the full MockProver test;
  genuine terminal proof generation and refreshed keys remain pending.
- **KGM-16 — Release blocker, missing claim-fold challenge bus witnesses.**
  Mint claim RLC start rows installed challenge state but omitted the same values
  from their permutation bus cells. Nonzero distinct challenges therefore violated
  existing gates and copy constraints. The correction must populate the exact
  original bus witnesses and retain both-field substitution rejection, with no
  constraint relaxation. All three corrected RLC tests now pass in the frozen
  validation lane, including both start challenges, results and padding in both
  fields. The exact claim geometry and processed small-key size tests also pass;
  genuine claim-proof execution remains pending.
- **KGM-17 — Release blocker, dependency advisory policy does not pass.**
  The current Core/mobile dependency graphs include unmaintained `smallstr
  0.3.1` ([RUSTSEC-2026-0215](https://rustsec.org/advisories/RUSTSEC-2026-0215.html))
  and `lru 0.16.4`, which has a conditional panic-safety defect
  ([RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html)).
  The affected LRU threadcache module is disabled in the inspected mobile graphs;
  Core uses `concread`'s B-tree and epoch-cell paths. That reachability result does
  not resolve the dependency policy failure. The audit policy now explicitly
  includes transitive unsoundness instead of inheriting workspace-only coverage.
  Yanked `chacha20 0.10.0` and `spin 0.9.8` also require review; a yank alone is
  not evidence of an exploit. The vulnerable optional `rkyv 0.7.46` lockfile entry
  is absent from those actual mobile graphs. Dependency remediation and any
  necessary lockfile-policy exception remain outstanding; no audit pass or
  bundled SDK binary coverage is claimed.
- **KGM-18 — Corrected; focused durable-finality validation passes.** A valid staged reserve
  receipt could fail promotion after finality framing changed nested alignment.
  The four-times-wire allocation estimate missed 17,496 bytes of additional
  alignment-copy charges in the reproduced receipt. The helper now uses Norito's
  owned-graph allocation envelope, capped at the unchanged 4 MiB, with independent
  sequence, element, field and depth limits. Exact staged/final decoding, repeated
  promotion, restart, unchanged persisted bytes and malformed-graph rejection
  regressions pass in the third retained Core diagnostic run. The maximum
  casting-corpus capacity fixture was separately corrected to use a real canonical
  registration. The isolated follow-up still rejects the maximum 1,000-binding
  corpus at the unchanged 4 MiB allocation limit (4,194,322 attempted bytes).
  A bounded allocation trace reaches only binding 507 of 1,000; the total deficit
  is not merely 18 bytes. The subsequent fixed-V1 borrowed decoder avoids those
  enclosing-field copies while retaining canonical decoding and every existing
  budget. A fresh isolated build passes wire equivalence across all casting phases,
  hostile field/length/variant/trailing-byte checks, outer-budget enforcement,
  finality receipt restart, membership/path substitution, and the complete
  1,000-binding staging/promotion/retry/restart case in 5.04 seconds. The 4 MiB
  ceiling and required binding count are unchanged. This closes the reproduced
  defect in that source-recorded snapshot; full current-source release validation
  remains separate.
  Sources: [sidecar decode limits](../crates/iroha_core/src/kura.rs) and
  [borrowed finality decoder](../crates/iroha_core/src/kura/kagemusha_finality_decode.rs).
- **KGM-19 — Release blocker, complete Terminal key/resource budget.** The earlier
  Terminal configuration used five Table8 SHA lanes and four dense accumulator
  lanes. Its 110 original selector bitmaps require 901,120 bytes at k=16, already
  exceeding the unchanged 65,536-byte verification-key limit. Even before Base
  and materialized selector columns, the fixed/permutation polynomial inventory
  gives a conservative 225,298,502-byte proving-key floor, above 64 MiB. These are
  configuration/serialization bounds, not measured generated keys. Selector
  compression retains the original bitmaps; disabling it increases the proving-key
  requirement. An early configure-only rejection now precedes parameter and scalar
  graph construction. Both configure-only rejection tests and the real small-k6
  serialized-key agreement test pass in the isolated build. The next source snapshot
  replaces all inline Terminal SHA jobs with a required authenticated complete claim,
  retaining candidate/Guard history, merging the full claim ancestry and binding all
  14 carrier values across the reciprocal audits. Its typed producer uses all 26
  original messages and authenticates four release-pinned claim/shard protocols.
  The pinned follow-up build now passes ten focused tests: all four helper pins,
  both-field history/shape rejection, dense geometry and early preflight, complete
  Send/Redeem queues, independence from later proof outputs, and changed semantic
  intent. The new reciprocal claim-tail tests and actual complete proofs remain
  pending. The dense-only auxiliary floor is now 27,263,214 PK bytes, but the complete Base
  graph, generated keys and proofs remain unqualified. Four dense lanes still use
  148 advice columns, or 296 MiB for one k16 polynomial vector, before other memory.
  The original key and mobile limits remain; SHA removal alone does not close this gate.
  Sources: [resource inventory](../crates/iroha_core/src/zk/kagemusha_v1_recursion/artifact_resource_preflight.rs)
  and [generation preflight](../crates/iroha_core/src/zk/kagemusha_v1_recursion/generation.rs).
- **KGM-20 — Corrected; focused reciprocal-audit validation passes.** Terminal
  reused a State-specific helper that selected audit positions 48/50, which are
  history cells in Terminal's public column. Terminal's Eq circuit must instead
  bind the Ep audit at 41/42, and its Ep circuit the Eq audit at 39/40. The
  correction selects those exact Terminal cells and retains the existing audit
  transcript and dense curve equations. Both-curve tests cover changed audit
  limbs, relocation to the old positions, malformed columns and a recomputed
  digest containing a false curve equation. Static review passes. The first
  isolated regression aborts on the default test thread stack. The unchanged
  harness then passes all reciprocal cases with a recorded 32 MiB test stack in
  869.07 seconds. The fresh-source normal regression then passes in 1,025.14
  seconds using a test-local named 32 MiB thread and no environment override.
  Source/binary hashes remain unchanged. This closes the reproduced positional
  defect; final genuine Terminal proofs and release qualification remain blocked
  separately by resource/authority gates.
  Source: [Terminal reciprocal audits](../crates/iroha_core/src/zk/kagemusha_v1_recursion/terminal_authorization.rs).
- **KGM-21 — Release blocker, typed-SHA claim key convergence.** The actual
  supervised State diagnostic fails before State proving while generating its
  reusable typed-SHA claim artifacts. The convergence graph has 264 advice
  columns, 145 selectors (142 materialized), three instance columns and 163
  permutation columns. Its predicted PK is 1,316,114,662 bytes, above even the
  1,073,741,824-byte host diagnostic allowance. The 1,197,834-byte predicted VK
  is below that diagnostic allowance but does not satisfy mobile helper limits.
  The source-stable run exits after 1,123.99 seconds with peak RSS 1,950,744,576
  bytes; it creates no qualifying State proof. A sound claim representation and
  converged key graph are required, not an increase to the diagnostic budget.
  A fresh both-parity configure-only regression confirms that even one legal
  Base gate plus one lookup request yields 109 advice columns, seven configured
  fixed columns, two selectors and eight permutation columns: its predicted
  77,611,726-byte PK exceeds the 64 MiB release helper limit. An independent
  arithmetic floor excluding every selector representation still reaches
  69,206,654 bytes. Neither bound is a generated artifact measurement.
  A subsequent reviewed source change shares the identical 15-bit Base/RLC
  table and retains independent ownership when Base has no matching table.
  Its expected minimum is six configured fixed columns and a 73,417,382-byte
  PK, still above 64 MiB. The old selector-free impossibility bound no longer
  applies after that saving. The sharing change now compiles and passes its
  ownership, both-consumer overflow and actual serialized-key regressions. Compact
  Eq/Ep PKs shrink from 44,056,974 to 39,862,630 bytes, and VKs from 16,682 to
  16,650 bytes. The minimum production configuration bound remains 73,417,382;
  these compact fixture keys do not prove full claim-key convergence.
  The next genuine State profile includes shared range ownership and exact Base
  packing. It exits at the same claim prerequisite after 1,221.907 seconds, with
  1,928,134,656 bytes of peak enforced memory. Its actual graph has 7,687,503
  Base cells, 118 Base advice columns, eight lookup columns, 234 total advice
  columns, six configured fixed columns, 118 materialized selectors and 133
  permutation columns. The predicted PK falls to 1,085,204,558 bytes but still
  exceeds the unchanged 1 GiB diagnostic limit by 11,462,734 bytes; its predicted
  VK is 974,890 bytes. The production limits remain 64 MiB PK and 64 KiB VK.
  Exact packing succeeds; no State proof is produced. The observed reciprocal
  job contains 749 sources, so both dense lanes are necessary: one lane admits
  at most 504 sources at the existing k16 domain. Neither removing a required
  lane nor increasing diagnostic limits closes the production resource gate.
  The next applied change chooses compressed or direct selector encoding from
  exact synthesized inventories while requiring a single candidate to satisfy
  both key limits. A degree-threshold lower bound replaces an unsound inactive
  greedy estimate that could reject feasible keys before synthesis. On the
  measured graph, direct encoding removes only 966,656 bitmap bytes: the VK
  becomes 8,234 bytes and the PK remains 1,084,237,902 bytes, still over budget.
  A reviewed GLV change also removes one redundant modular carry per source,
  preserving the final canonical scalar equality and all curve/segment bindings.
  The isolated build passes twelve focused tests, including actual small proofs
  in both fields and encodings, checked key round trips, the greedy counterexample,
  rational normalization and valid/invalid carry witnesses. Eleven cases per field
  confirm 260 fewer Base cells and 60 fewer lookup entries per source. The full
  749-source extrapolation is 194,740 cells and 44,940 lookups; guarded full-graph
  execution remains pending. The first prior reciprocal matrix has passed and
  its full claim-tail matrix continues. These results do not establish production
  key or mobile closure.
  A subsequent isolated native Poseidon Claim integration passes 44 focused tests.
  It preserves the exact raw generator and all 65 rounds, domain/padding/order,
  source and public bindings, and six Base copies per permutation. Four genuine
  k12 proof cases pass with checked reloaded keys and changed witnesses under the
  same key; wrong public inputs, digests and corrupted proofs reject. Actual Claim
  configuration and maximum-source queue checks pass alongside all 22 resource
  regressions and nine packing/transport tests. This reviewed batch is applied
  after exact preimage checks and compiler-hold release. Its auxiliary PK floor is
  85,984,030 bytes and minimum legal Base profile is 119,555,166 bytes, both above the unchanged 64 MiB
  release cap. The explicit test-only diagnostic envelope now reaches the same
  early configuration guard as consuming keygen. A guarded run measured successive
  373-, 710- and 858-source convergence graphs at 2,360,000, 5,094,505 and 6,322,041
  Base cells before its memory-accounting guard stopped during a companion process's
  natural exit. No final converged key or State proof completed. The companion's full
  reciprocal-tail mutation matrix passed both tests with frozen inputs. Genuine
  State/Terminal/CommitWrapper closure remains outstanding.
  A subsequent isolated candidate passes 57 focused tests, including four actual
  k12 proof cases with the shared native Poseidon equality column, both-field
  protocol identity mutation tests, and exact GLV integer/remainder regressions.
  The latter remove 110 Base cells and 21 lookup entries per source. All twelve
  loaded artifact types keep authenticated fields private to the recursion module;
  all 24 full-library doctests pass, and an external compilation against the actual
  library rejects access to all 162 fields while accepting opaque storage/passing.
  The tested minimum legal
  Claim key estimate is now 98,583,446 bytes, which still exceeds the 64 MiB release
  cap. Its minimum legal 116 advice columns also require 232 MiB of scalar payload
  in one k16 advice vector. Both Claim key-consuming paths use the consuming
  prover, which already transfers owned values and removes eager duplicate
  placeholders but retains the final scalar advice vector. This is a source
  allocation count, not measured device RSS; the
  complete circuit/prover working set must be reduced and measured against the
  unchanged 128 MiB whole-process gate independently of key serialization.
  The reviewed seven-file batch is applied to the shared checkout after exact
  preimage and timestamp checks; its shared build remains separate from the isolated
  evidence. The subsequent current native bridge build passes 137 tests with all
  seven source hashes and its retained executable unchanged; its synthetic catalog
  does not qualify hardware or monetary admission. A corrected complete-fold native
  transcript follow-up passes full Core metadata compilation and 45 unique focused
  tests (28 actual Core and 17 exact native-queue tests), preserving the shared BUS,
  existing geometry, every original fold challenge/byte/equation and rejection
  behavior. Eight small real IPA proof cases pass. It remains isolated pending
  full proof measurements and shared-source application. A subsequent actual
  Core run passes all 64 focused tests using an optimized field dependency with
  debug and overflow checks retained. The subsequent optimized Core executable
  also passes all 64 focused tests with all bound inputs unchanged. Its fixed
  full State diagnostic fails after 319 seconds because a test-only emitted-row
  assertion omits the two new native folds (20,394 actual rows versus 15,444
  expected). The first Eq graph has 387,738 fewer Base cells before reciprocal
  auditing. Inputs remain unchanged and the child exits cleanly; the narrow count
  correction passes two extracted actual geometry tests and is installed in the
  next isolated snapshot. The corrected full Core build passes all 64 focused
  checks; its fixed State diagnostic passes the original failing assertion in
  both fields but times out after 2,700.103 seconds. Both fields complete the
  858-source graph at 5,253,683 Base cells and 81 Base advice columns. Peak owned
  RSS is 1,425,096,704 bytes; inputs remain unchanged and the reviewed guard
  reaps its child correctly. No final key or proof completes. A short CPU sample
  identifies repeated dense-MSM witness inversions, and the linked `pasta_curves`
  artifact remains at O0. The private inversion-reuse candidate and parser
  correction pass five extracted-source tests, including actual BGH19 reads and
  both-field counter equivalence. Complete row/terminal-point comparisons then
  pass in both curves, including accumulator handoff across split lanes; seven
  distinct follow-up checks now pass. Both candidates are installed in the
  phase19 private snapshot of 5,244 sources and five build inputs. Its full Core
  build and all 71 focused checks now pass, including eight small real IPA
  cases and the seven parser/counter checks. Actual Core, Pasta and wrapper
  artifacts are O3 with debug and overflow checks retained; all bound sources,
  installation and the retained executable remain unchanged. The fixed guarded
  State preflight succeeds, but the diagnostic times out at 2,700.076 seconds
  under its unchanged 24 GiB/2,700-second envelope. Peak owned RSS is
  6,654,443,520 bytes; all 5,249 inputs, executable, guard and supervisor remain
  unchanged, and the reviewed guard reaps its child correctly. Reusable typed-SHA generation
  completes with each Claim PK/VK at 799,022,510/6,058 bytes and each shard PK/VK
  at 8,533,246/13,290 bytes. These are actual serialized diagnostic artifacts;
  the Claim PK fails the unchanged 64 MiB release limit. The run reaches
  credential typed-SHA proving but provides no full State pass. Its cached-key
  path uses the borrowed prover, whose eager advice buffers are separate from
  the already improved consuming-key path. Reducing either allocation alone
  cannot establish the unchanged 128 MiB device gate.
  The next private candidate moves selected complete ordinary transcripts into
  the existing two native Poseidon lanes after reserving mandatory work. Static
  comparison preserves the original complete ordinary/hybrid verifier bodies;
  compilation, challenge/stream/equation equivalence, constraint mutation tests,
  genuine proof verification and key convergence remain outstanding. The
  nonzero-view restart fixture now restores real WAL registry and body custody.
  The retained Broadcast and recovered-Apply carriers use heap storage, with live
  Broadcast allocation reserved before publication. Core56 passes the genuine
  nonzero-view production-services restart and inline-carrier size regressions on
  the default stack. This closes the observed registry-insertion stack failure.
  Broader startup/recovery and four-validator settlement qualification remain
  open; these focused passes do not establish a monetary or production release.
  All 19 current cases
  mapped from the original 18 failures pass on the preceding executable; 17 are
  additional unique tests, bringing its focused Core total to 81. The full proof
  remains unverified and historical failed runs retain their original results.
  Optimized diagnostic compilation changes no production configuration or
  resource limit.
  The preceding State diagnostic on the phase16 validation copy times out after
  2,700 seconds under its unchanged exclusive guard. Both fields complete the
  866-source graph at 5,693,567 Base cells and 87 Base advice columns, with peak
  owned RSS of 1,465,925,632 bytes. Inputs and executable remain unchanged; the
  cleanup authentication error and absent child exit code are preserved, and
  subsequent process checks find both owned processes absent. No final key or
  State proof is produced.
  Source: [typed-SHA key generation](../crates/iroha_core/src/zk/kagemusha_v1_recursion/mint_hash_generation.rs).

The platform credential circuit already constrains its positive hardware epoch
inside the proof. The shared credential-assignment helper now enforces the same
local invariant for Guard and MintAuthorization; direct both-field zero/positive
regressions pass in the retained diagnostic run. This strengthens a local invariant: the review did not
find an accepting full zero-generation proof through the existing authenticated
PlatformCredential relation. Guard and MintAuthorization keys must be regenerated
and their final geometry measured. Bootstrap's inactive predecessor generation
and encoding-only zero templates remain valid construction inputs; they do not
authorize a live credential.

The diagnostic State milestone now retains the original sender openings and
extends through actual paired TerminalAuthorization artifact/proof generation.
Its persisted candidate must match the State transport protocol and complete
public column; the inner State protocol, proof and history are independently
verified and never relabeled. The diagnostic compiles and its cheap boundary
tests pass; actual proof execution remains pending. It uses
test-provider secrets and structural commit evidence, so even a successful run
would not qualify an OEM commit. The full State/TerminalAuthorization/CommitWrapper
key graph must still close before payment, sender installation and ReceiveFold
qualification.

The frozen validation checkouts shared Cargo artifacts with the current checkout,
and a concurrent mobile build observed stale local-crypto metadata. Retained test
results and unchanged source hashes are diagnostic observations; they do not
establish isolated dependency provenance. Further frozen builds must use an
independent persistent target and revalidate their exact dependency graph before
serving as production evidence. The first corrected isolated snapshot now records
5,211 compiler/source/fixture files, five build inputs and 59 local artifacts. Its
compile-start test and 14 follow-up tests pass; the reciprocal test stack abort and
maximum casting-corpus allocation rejection remain failed results. A subsequent
5,212-source snapshot compiles the borrowed-decoder correction and passes its
equivalence test plus ten focused regressions, including the complete maximum
casting corpus and corrected reciprocal audit. These results validate their recorded pinned snapshots, not the
entire current checkout or physical hardware. The following 5,217-source snapshot
passes ten tests and retains three failed Terminal fixture filters. Carrier
encoding tests in both Pasta fields measure advice cells of 76/586/154 for
scalar/point/source-commitment encoding, down from 376/1,222/790; bounds,
coordinated witness mutations and cell-cache identity also pass. This is local
constraint evidence, not complete proof performance. The next frozen snapshot
contains corrected Terminal packing/reservation fixtures, active SendSplit queue
coverage and the shared range table; all nine tests pass, including all three
previously failed filters. Every recorded input and binary hash remains unchanged.

The private recursive State checkpoint codec is implemented, with proof decisions
and exact expected-state binding. Nine structural tests pass, but actual
capture/restore reaches the mandatory native provider-policy authority gate
before proof verification. A positive genuine-proof round trip and same-length
proof-substitution checks therefore remain unverified; structural acceptance
does not satisfy that gate. The remaining native coordinator dependencies are concrete:
a qualified service that seals one atomic latest checkpoint across
state/proofs/WAL/accepted replies, native response admission and challenge
consumption, integration of credential/Guard proof production, and finality
resolution. No production backend currently implements the four distinct mint
reservation, mint staging, peer staging and recovery-anchor certificates. Public
device replies do not contain those native-private records, so a response
  signature cannot supply them. Startup also needs a nonauthorizing challenge
journal before the authenticated lane's operation WAL can exist; qualification
currently reserves an operation before Core bootstrap.
The current stock backend intentionally reports unavailable. These are outstanding
implementation and qualification goals, not deployment switches.

Concrete source locations and completed test results are recorded with the
implementation in `status.md`. No active exploitation or qualified production
deployment was established by this source review. Rust/Swift/Kotlin circuit and
device findings come from direct code inspection; the generic security skill
does not provide language-specific audit coverage for those components.

## Physical clock-rollback transcript contract

After byte-identical outbox recovery and before the backup/restore cycle, the
observer must record `clock_rollback_begin`, `clock_rollback_applied`,
`expired_request_rejected`, and `clock_rollback_end` in that order. All four
bind one unique control and the active hardware boot. Begin/rejection bind the
same request digest, current aggregate state, logical counter and epoch.

The host clock must start strictly after request expiry, move strictly before
it, remain before it during the failed sender attempt, and be restored to at
least its initial value. Hardware trusted time must remain strictly past expiry
and nondecreasing throughout. The attempt must return
`expired_request_rejected` without advancing monetary state; its operation ID
cannot duplicate another operation. Observer event time remains monotonic and
is distinct from the intentionally changed host clock. Missing boundaries,
reused controls, substituted request/state/epoch/counter, accepted attempts, or
rollback of trusted time fail validation even with fresh observer signatures.

This negative sender exercise does not change delayed receiver admission:
payments already committed within their original request window remain
receivable after expiry.
