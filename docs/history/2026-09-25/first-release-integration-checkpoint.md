# First-release integration checkpoint — 2026-09-25

This checkpoint covers only `/Users/takemiyamakoto/devstuff/iroha` on the
existing `optimizations` branch. The source is an uncommitted integration in
progress, not a frozen or signed release candidate. At this checkpoint,
`Cargo.lock` has
SHA-256 `62ee339c8f51fd9cb01145cb1b95225c08624bb8bd82a6b3403f6bce64bc0728`.
The pre-integration source was backed up under this checkout's ignored
`target/` directory before reviewed subsystem patches were applied. No branch,
worktree or sibling checkout was used for this integration.

The current source combines the SoraFS software-custody and signer-operation
slices, Native/shared-execution and privacy/SDK slices, and the first-release
native bridge ABI-24 cut that removes the retired third `RegisterZkAsset`
`vk_shield` field. The C bridge export inventory was aligned with this source.
The canonical JSON/wire codec rejects the retired field; there is no
compatibility decoder. The SoraFS signer journal now requires an explicitly
shared, finite inventory pool for receipt purposes and pending Reserve,
configured through default, user and actual configuration. Its path and scan
admission are a prerequisite only: the daemon has no production local signer
service to receive that pool, and the pool does not retain a whole-operation
lease through key use, receipt staging and completion.

Six instruction-record fixture rows were recaptured from typed Rust values:
`RegisterZkAsset`, `RedeemKagemushaV1`, `TopUpKagemushaV1`,
`RegisterIdentifierPolicy`, `ClaimIdentifier` and `FinalizeElection`. The
two Claim cases were matched by their unchanged encoded account field. The
fixture retains 321 type rows and 357 cases, with unchanged nominal and
directional type identities. Its current SHA-256 is
`59a6162b6c1b3ab384b3e06dd61cf8326eade4333beae4a07b782813ae88f97a`.
The typed capture tests were temporary and removed after the reviewed bytes
were applied. [Fixture provenance](../../../crates/iroha_data_model/tests/fixtures/instruction_record_generated_identity_frames.md)
records the per-subsystem recaptures.

Focused validation on this integrated source:

- `cargo test --locked --offline -p iroha_config sorafs_signer_journal_inventory`:
  2 passed.
- `cargo test --locked --offline -p iroha_data_model --lib generated_record`:
  323 passed, 2 ignored.
- `cargo test --locked --offline -p iroha_data_model --lib`:
  4,082 passed, 15 ignored. The typed Native AMX fixture now carries the two
  explicit absent transaction commitments, the contract-state proof test uses
  the canonical Norito frame, the registry and direct-conviction goldens match
  current typed source, and the BFV full-bootstrap public schema advertises
  the implemented 38-column trace. Its schema hash and the two affected
  provenance frames were recaptured from current source. This is a DataModel
  suite pass, not a BFV production-qualification result.
- After the later arithmetic and documentation edits, 10 focused Native lane
  consensus DataModel tests passed. The extracted public
  `conviction_weight_from_units_v1` boundary/parity test passed; it does not
  admit a private ballot or tally relation. After the later V1 wire-preserving
  lint refactors and inline stream-token outcome evidence, a repeat full
  DataModel library suite passed 4,084 tests with 15 ignored.
- `cargo test --locked --offline -p iroha_data_model --lib register_zk_asset_rejects_retired_third_wire_field`:
  1 passed.
- `cargo test --locked --offline -p irohad --lib signer_operation::journal`:
  18 passed, including exact-capacity, shared-pool, path, restart and
  receipt-byte controls.
- `cargo test --locked --offline -p irohad --lib signer_operation::tests`:
  114 passed after the role-11 reviewed Reserve boundary. The stream-token
  producer now passes the exact borrowed canonical body, time window and
  request digest to a purpose-specific reservation method; generic role-11
  Sign refuses. This test source is not a production finalized-state source.
  Release-manifest restart repeats are refused by the advanced
  authoritative audit before provider use; the source-rollback test retains
  the separate journal fence.
- `cargo check --locked --offline -p iroha_core --lib` passed after limiting
  two test-only views to tests. The remaining warning identifies unconnected
  production StreamToken and Topology custody variants; it is not suppressed.
- `cargo check --locked --offline -p iroha_core --tests` passed without warnings
  after retaining the final-promotion test record and checking four durable
  Kura finality receipts against their source height or block hash.
- The focused Core finality-metadata tests passed 5/5, and the completed
  final-promotion observation test passed 1/1 on the same test build.
- The C/Rust bridge header self-test, retired-codec guard and JavaScript
  native-build policy suite passed; the latter passed 108 tests.
- The Android exporter subset now includes 78 registered V1 instruction types after
  removing 42 unregistered direct grouped variants. All 13 exporter tests,
  11 Python parity tests and the 78-entry manifest parity check pass. Two
  exporter/documentation/hash-tree generations produced byte-identical tracked
  outputs. `RegisterZkAsset` exports two fields and `FinalizeElection.tally`
  exports `Vec<u128>`; the SoraFS manifest fixture replay and rebuilt Android
  consumers remain outstanding.
- Strict Clippy with `--no-deps` passed for the exporter. The full dependency
  traversal remains open: it reached `iroha_data_model` and reported 105
  diagnostics, including 70 documentation findings and six intentional
  cross-layer field comparisons that require semantic review rather than
  Clippy's suggested substitutions. Earlier traversal findings in the
  vendored `concread` feature name, native-digit allocation casts, and crypto
  resource helpers were repaired with focused tests; this is not a workspace
  strict-lint pass. A subsequent strict `iroha_data_model --lib` run after
  targeted documentation and lane-consensus cleanup still reported 50
  diagnostics. Subsequent wire-preserving refactors, documentation fixes, and
  an evidenced inline stream-token outcome reduced the current DataModel
  library's owned strict-Clippy remainder to four large role-11/13/14/15 enum
  findings. These four now carry narrow, reasoned lint expectations rather than
  boxing their canonical Norito payloads; four new focused size/schema/Copy
  tests passed. A fresh strict `cargo clippy --no-deps --locked --offline -p
  iroha_data_model --lib -- -D warnings` run passed; this does not qualify the
  all-targets or workspace lint gates. An all-targets
  diagnostic run with the enum-size lint excluded reported 164 located
  test-target findings. A test-only patch removed 33 of those in the next
  inventory; a separate grouped-fixture unused import was then removed and
  needs a repeat lint run. The unsuppressed dependency traversal also fails in
  vendored `concread`. No workspace strict-lint pass is claimed.
- Python 3.12 from an in-repo `target/` virtual environment passed 1,565
  SoraFS release/topology/package tests, 225 promotion/receipt/Android parity
  tests, and 9 targeted readiness negatives. The older default Python 3.9
  readiness run is diagnostic because scripts require Python 3.10 or newer.
- Focused Python fixture checks passed 7 Native AMX script tests, 67 installed
  Python SDK Native AMX tests, 12 direct-conviction tests, and the BFV static
  asset/schema mutation guard. The installed SDK uses a prebuilt native wheel,
  so same-source native qualification remains open. The broader static-contract
  suite initially found three unrelated OpenAPI/source-line/test-inventory
  ledgers; the exact owner and inventory pins were updated without relaxing the
  2,000-line reduction gate. Its direct rerun passed 4 tests and 15 hostile
  asset-mutation subtests.
- The new Norito exact-canonical-frame helper passed its focused unit test.
  The stream-token parser's 3 focused canonical-payload tests passed after
  replacing one redundant full-payload re-encode with that streaming check.
  Full pre-Reserve resident-memory admission remains open.
- Core's governed privacy public-reserve application now rejects a forged
  prepared delta unless its exact verified pool bridge, account/definition
  identities, authorized amount, live source and destination balances, and
  equal checked debit and credit all agree before either balance changes.
  Its arithmetic relation uses ten fixed stack limbs. Three focused primitives
  tests passed, including wide-value and post-normalization boundaries; two
  independent source reviews found no arithmetic mismatch or hidden iterator
  allocation. All 8 focused `public_reserve` Core tests passed, including
  forged-credit, forged-debit, forged-amount and stale-destination cases. This
  is the transparent payout leg; private-proof conservation and whole-transfer
  preparation/write/resource admission remain open.
- The Native transport test rejected source substitution and actor
  substitution while accepting the unchanged canonical packet, and the
  direct-driver silent-initial-author test reached a real decision. The
  four-validator process outage/restart was still open at that point, and the
  soak requirement remains open. A
  same-source `iroha3d` prebuild succeeded, but the real four-validator
  silent-initial-author test then failed after 228.70 seconds: at height 3 one
  peer converted local `History(Busy(ReleaseWait))` during Native candidate
  assembly into restart-required; the finite input exceeded the bounded outage
  interval. The first attempt had stopped before network launch because the
  daemon binary was absent. This exposed the production retry/fail-stop
  boundary defect corrected and retested below.
- The candidate assembler now distinguishes local State admission before signing
  from the identical refusal after signing, and the Native runner defers a
  release-waitable inner candidate outcome only after retaining its original
  source. Post-sign refusal remains fail-stop. Two focused Core tests passed:
  a physical membership owner caused a pre-sign Busy refusal and released for
  a clean retry, and a phase test rejected post-sign deferral. The real
  four-validator process test then passed in 113.40 seconds with the initial
  Native author stopped before the sole input, later-view finality by the
  three survivors, and authenticated restart checks. The same-source daemon
  SHA-256 was `2f984792729056eff30f1833c79f60cedad49f8a7ae00761de50ccdc0687bd70`.
  A further final-header pre-sign work probe and a bounded Queue snapshot
  scratch allocation were applied. Seven proposal-work tests, the phase test,
  and the Queue `usize::MAX` empty-snapshot regression passed. A rebuilt daemon
  with SHA-256 `96559a688253caeed8e0eacfe4004ae61da7ecebf6e736e6347d787bcd41396a`
  passed the same four-validator outage/restart test in 115.64 seconds. These
  are focused source and network checks, not the ten-seed/two-hour soak or
  complete same-candidate release matrix.
- The FASTPQ diagnostic geometry screen had stale DEEP DTO arithmetic after
  fixed FRI fibers. Its current test-only projection is 500,783 bytes, or
  502,831 with the hypothetical row mask; the corrected screen and its 9
  Python tests pass. This is not a produced compact proof or production
  qualification: emitted ordinary/AXT segments remain about 4 MiB and the
  512 KiB target remains open.
- The move-only MKHE qPCS source preflight now charges the canonical initial,
  quotient and 18 correlated-FRI Merkle-opening hashes plus the repeated
  initial query-binding leaf hashes on the original proof-session ledger before
  public read. Independent arithmetic review bounds the partial charge at
  57,475,588,392 tracked work units. Four opening-work tests, the current
  source-owner order test, and the 117.88-second refusal/owner-retention test
  passed. Other transcript and source arithmetic, memory, spool and I/O remain
  uncharged; the current full initial tree exceeds the 128-billion work cap. The
  source-derived commitment/evaluation redesign and production composite
  remain closed.
- A reconstructed SoraFS stream-token client without the original process-local
  ambiguous-sign owner now fails recovery before connecting to a signer. Its
  focused test passed. This fail-closed guard does not supply the missing
  finalized operation source, signed atomic request/body/quota owner, or
  durable pre-key-use attempt fence.
- Three focused executor tests passed for the new SoraFS topology permission
  identities and exact deployment-scoped direct/boxed action dispatch,
  including genesis and self-observer refusals. The focused schema test passed;
  twice-regenerated `specs/references/schema.json` matched SHA-256
  `2d2ca649c8c475f312799866595cd3fb9f9e363dd762c9b5615bc55d0343c724`,
  and the schema consistency and no-legacy-codec guards passed. Core's current
  World permission preflight passed three focused tests covering all seven
  actions, role and direct grants, revocation, separate Check observer/operator,
  and unchanged state. Core topology execution and inner promotion approval
  verification remain closed.
- The Rust-owned Sumeragi wire TSV was regenerated twice from current source;
  both runs matched SHA-256
  `75da547d5a1e455f074614218bbde204e57ef6bdbba4e4c330760e2e8b4f3884`.
  Its sibling grouped Native AMX JSON was already identical. The stale
  block-header golden was replaced with the current 126-byte frame. The
  grouped DataModel target then passed 185 tests with 5 ignored, and the
  fixture generator's tracked `--check` passed. Python wire-invariant tests
  passed 11/11 and JavaScript grouped-fixture tests 65/65. Kotlin's canonical
  V4 wire and strict Torii JSON models now include the mandatory transaction
  input/output commitments; 111 Sumeragi-focused Kotlin and Java-source tests
  passed. JavaScript's strict Torii status parser now requires and validates
  the same two fields, including nonzero counts and output coverage; its
  direct parser tests passed 3/3. The focused diagnostics inventory now
  expects 46 cases. Its staged runner is still blocked before execution by
  the untracked SoraFS source, and native-addon provenance remains stale.
  Swift package parity remains in progress. A second Rust fixture cut now adds
  an exact populated 2-input/3-output transaction-Merkle pair, generated twice
  with identical TSV SHA-256
  `82a6d1904fa556b3775658f6e96d81cae90d3ca675f2a444ee80373a86d35f20`.
  The grouped Native AMX JSON was unchanged. Rust authority tests passed 2/2,
  the full DataModel group passed 185 with 5 ignored, Kotlin and Java-source
  fixture suites passed 37/37, and 11 formal wire-invariant tests passed after
  the source-binding ledger was updated. This fixture uses placeholder QC
  signatures and does not establish executed-block authenticity.
- The SDK production-source closure manifest was reconciled with 23 present
  Swift, Kotlin, JavaScript and Python source files. With a diagnostic copy of
  the Git index marking two still-untracked source files as intent-to-add, both
  suite digests resolve and 15 non-regeneration resolver tests pass after the
  JavaScript model change. The real
  index remains untouched. Eight regeneration tests require output roots
  outside this repository and were not qualified under the checkout-only scope;
  the signed candidate must also track both new source files.
- A read-only role-16 Native topology row decoder was applied with local
  retained/control/operation checks. Review found and repaired false acceptance
  of malformed revision keys beyond the terminal head and rolled-back or
  missing active-operation slots. Eleven focused Core tests passed against the
  repaired logic; a test-only unused-mut warning was removed afterward. The
  decoder has no funded writer, authenticated replay, finality, or production
  authority, and topology instruction execution remains explicitly closed.
- Its same-State current-control reader now bounds and canonically validates the
  embedded role-16 software-custody frame against the borrowed network, chain,
  deployment purpose and enrollment-head presence. The combined focused Core
  topology selector passed 17 tests, including wrong-scope and malformed-frame
  controls; address-discriminator binding, full replay, execution and finality
  remain absent. This reader is internal and cannot authorize promotion.
- An additional read-only topology scan now checks each retained revision
  prefix for gap-free canonical keys and bounded nonempty raw frames. Its
  focused test passed missing, oversized and extra middle/history controls.
  It performs no intermediate decode, digest, reducer replay, execution or
  finality authentication. Full cold replay remains blocked by absent funded
  allocation ownership in State restore; signer and promotion gates stay closed.
- Core standalone-election state now retains an ordered, capped sequence of
  fixed nullifier/commitment pairs in place of the retired split fields. Direct
  JSON and snapshot preflight reject retired names and malformed or oversized
  corpus rows; current and previous restore reject duplicates and over-cap
  states. Fresh Core JSON, snapshot, restore, full-corpus and semantic-guard
  selectors passed, and the grouped Norito order roundtrip passed. The ballot
  and tally production guards still reject. Independent review found no guard
  opening but identified pre-guard allocations and a missing signed snapshot
  lifecycle test. The new height-zero signed snapshot test passed generated
  capture, typed restore of ordered current/previous corpora, and rollback
  readback. It does not qualify finalized-block replay.
- Election Create, Submit and Finalize scheduler declarations now use the same
  whole-record key; retired field-suffixed election hint keys are rejected.
  The whole-record conflict and strict-hint Core tests each passed. The ballot
  clone/publication resource owner, credential-linked ballots and sound tally
  remain open.
- Cast preflights encoded base64 length before proof decoding. Cast, Submit,
  and Finalize now borrow the retained election through cap/VK checks and the
  still-closed semantic guard before cloning the successor. The corrected
  full-corpus test fixture is restart-valid. Focused Core tests passed for
  direct length refusal, full-corpus Submit refusal, large-corpus Finalize
  refusal, canonical-selector priority, and the independent fail-closed
  semantic guard. Independent review found no new acceptance path, but signed
  ballot binding can still clone before preflight and the eventual successor
  clone/publication remains unfunded. The Finalize large-corpus test reaches a
  closed VK role before the semantic guard, so it verifies unchanged state,
  not heap-allocation behavior.
- The broader election-filtered Core run passed 87 tests and found two stale
  IVM host fixtures: invalid elections now fail during State startup before
  host hydration. Those tests were repaired to check atomic host rejection
  through a borrowed hostile World view and fallible State startup refusal,
  while valid cases still hydrate from State. Both repaired tests passed,
  together with the signed snapshot and the focused ballot/corpus tests.
- The non-test `cargo check --locked --offline -p iroha_core --lib` passed on
  the combined source cut after narrow dormant-variant lint expectations were
  added for the still-unconnected StreamToken and Topology native Check callers.
  `cargo fmt --all`, `git diff --check`, and the no-legacy-codec guard passed.
  This is a source-integrity checkpoint, not candidate freeze or strict
  workspace Clippy qualification.
- `scripts/check_source_file_budget.py` checked 13,009 files and failed with
  252 findings on the current combined checkout. The touched Core `world.rs`
  and IVM `host.rs` remain above their existing exact no-growth baselines at
  42,990/33,785 and 27,589/27,377 lines respectively. The findings also
  include many unrelated existing files; no baseline was raised. Source-file
  decomposition is an open release-tooling gate.
- The focused software-signer receipt JSON suite passed 8 tests. The SoraFS
  promotion-bundle and cosign-crypto script suites passed 207 tests with 29
  skipped. These validate exact refusal/receipt handling; they do not replace
  signed inner approvals or open the production promotion checker.
- That script run used the shell's Python 3.9.6, below the repository's stated
  Python 3.10+ test floor. A pinned-requirements Python 3.12.14 environment
  under `target/` passed the isolated receipt/cosign selection (9 passed,
  29 skipped). Its combined three-file selection returned 203 passed,
  29 skipped and 12 failed: the promotion checker deliberately refuses to
  snapshot inner-approval inputs when `TMPDIR` is inside the source tree.
  These are environment-constrained failures, not a Python 3.12 promotion
  pass; the intentional source-isolation guard was not weakened.
- The separate SoraFS production-readiness script suite completed on the
  shell's Python 3.9.6 with 468 passed in 9,371.25 seconds (2:36:11). This is
  diagnostic component coverage because that interpreter is below the stated
  Python 3.10+ floor; it does not supply signed production summaries, soak
  evidence, or a supported-runtime qualification receipt.
- A current-code Native audit found that the process driver, pre-payload
  replacement, Decision relay, original Apply owner and publication settlement
  are connected; several comments calling them inactive are stale. The next
  concrete memory gap is an uncharged nested `SignedBlock` clone retained in
  `AwaitingNativeSource`. Existing allocation APIs cannot exactly reserve its
  derived nested clone. A sound cut needs a reservation-consuming full clone
  planner or movement of the already decoded body through the original owner;
  the latter is the preferred ownership direction to investigate. The
  accepted-work retry and signed RS16 gates remain mandatory.
- A separate reachability audit found no production construction of the old
  lane Prepare/Commit signing adapter. Native ingress uses the process-lived
  reducer/driver, but old lane wire tags and generic output acceptance remain,
  and startup repair still interprets old persisted committed lane sessions.
  Canonical executed-block repair genuinely uses the historical request/chunk
  carrier, so that carrier must be migrated before retiring old tags and
  persisted-state interpretation. A narrow fail-closed generic output boundary
  is the next identified cut; full first-release removal remains open.
- Scratch-only paired Native output patches now restrict the generic service
  to canonical executed-block request/chunk shapes and migrate old
  service-success assertions. Static apply/format checks pass, but an
  independent review found that ordinary running peers still drop those
  requests; only the interrupted-tip startup corridor handles them. An old
  durable-certificate branch also remains compiled, and output preflight can
  encode an old effect before the new service fence. Live responder wiring,
  earlier preflight refusal, old wire/state retirement and four-validator
  recovery tests remain required; the paired patches are unapplied.
- The canonical executed-block responder trace located the live gap more
  precisely: ordinary and terminal ingress retire canonical requests before
  the existing source worker sees them, and the worker's `Busy` result returns
  the original task without a funded retry owner. Canonical completions are
  not polled, and no typed exact-output handoff retains a prepared response
  through `SourceRetained` and height rollover. A safe cut must reserve before
  physical dequeue, retain the exact request on local refusal, poll both
  completion channels, and join canonical deferred output to the drain
  frontier. Routing the request alone could lose accepted recovery work;
  source tracing and a static preflight fence do not qualify live recovery.
- A read-only integration preflight replayed all twelve staged scratch patches
  against copied current source under `target/`: each checked and applied in
  the planned order, including the dependent Native guard, test migration and
  early preflight fence. This establishes patch applicability only; no Cargo
  compilation or tracked Rust integration has occurred during the Apple source
  seal. The Native trio remains deferred because even a safely retained
  ingress request would stall rollover: production does not poll the canonical
  worker completion channel, and polling it without a typed exact-output owner
  would discard the response.
- A further target-only selected-dequeue ticket patch now carries a pre-funded
  move-only reservation with the exact physical ingress owner, without running
  a capacity callback beneath fair-ingress locks. Static patch/format checks
  pass; it has no production caller and no Cargo result. It remains unapplied
  until canonical completion, exact output retry and rollover census form one
  consuming responder cut. A later review found that its `into_parts` method
  disarms the guard before a successor owner is installed, so the ticket patch
  is unsafe to integrate alone even as an interface. The full responder cut
  must preserve one armed owner across that transfer; a full-cut contract is
  recorded under the repository's ignored `target/` scratch directory.
- A second scratch-only Native responder slice adds a checked atomic byte
  lease and move-only exact request, retry and prepared-output owners with
  fail-closed drop tests. Static patch and formatting checks pass, but the
  mechanical test charges are not configured production reservations. No
  ordinary or terminal caller uses it, and the live worker completion,
  exact-output and drain frontier remain unconnected. Its caller must derive
  network and responder identity from authenticated process state.
- A scratch-only move-owner patch was prepared but left unapplied after
  independent review. It removed the AwaitingSource clone, yet retained the
  decoded body through later ready/executed phases without funding nested
  allocations and added uncharged per-retry hash cloning. The revised design
  must release that body as soon as the source phase no longer needs it and
  preserve exact retry identity without reintroducing deep copies.
- The scratch patch was revised and independently re-reviewed: it now retains
  the moved decode only for an unfinished source/capture owner, releases it
  before ready/Executed/Stopped marker I/O, uses full signed-body equality
  while retained, and leaves a bodyless tombstone if producer preparation
  panics. Static apply/format checks pass. It remains unapplied while the
  current Swift native bridge source cut builds; initial decoder allocations,
  later clones and full physical admission remain open even after this cut.
- A separate Native source-to-recorder move is staged under `target/`. It moves
  the authenticated frozen carrier into execution after common preflight,
  removing the second whole-block clone. The real Native preparation test
  checks backing identity of a nonempty
  admission frame. Static patch and scoped format checks pass, but no Cargo
  result exists while the Apple source seal holds. The initial borrowed
  carrier clone, decoder graph and publication overlap remain unfunded.
- An additional unapplied DataModel cut replaces the deep clone inside
  `SignedBlock::canonical_proposal_wire_hash` with the existing borrowed
  encode-only SignedBlock layout. Independent static review found no wire
  mismatch; proposed tests compare complete version/header/payload bytes in
  ordinary and transparent-api builds. A first test assertion using generic
  `canonical_frame_len` was corrected to exact payload length plus the custom
  header and version, since generic frame alignment may vary by target.
  Compilation and runtime equivalence are pending the Swift source seal.
  Exact payload counting before allocation remains only a design note because
  nested counting can fail locally and current Core callers do not yet preserve
  the original retry owner for that error.
- The Apple bridge's first release target completed after 55m26s and proceeded
  to the next target. The second `aarch64-apple-ios` target completed its
  release build after 64m50s, normalized zero duplicate PQClean symbols and
  linked the complete archive into a cross-target C consumer. The first
  target's host C ABI/crypto smoke passed; iOS device execution was not
  available in this local build. The bridge reported unused-code warnings,
  including target-inapplicable helpers; these remain lint work and do not
  alter the KAGEMUSHA monetary-authority policy. The arm64 iOS simulator
  target then completed its release build after 65m05s, normalized zero
  duplicate PQClean symbols and linked its archive into a cross-target C
  consumer. The x86_64 iOS target is now building. Five-target packaging and
  Swift runtime parity have not yet completed.
- A current FASTPQ source audit confirmed the ordinary and AXT offline
  artifacts remain roughly 7.48/7.50 MB and that the live Core verifier still
  uses witness-derived commitment replay. The test-only DEEP candidate has a
  500,783-byte maximum frame, but private production is deliberately refused;
  authenticated composition masking, a genuine bounded prover and reviewed
  soundness/privacy arguments are missing. Its projected masked variant is a
  research geometry, not production admission or evidence of meeting 512 KiB.
- A further FASTPQ geometry screen finds the current 375-query raw opening
  floor alone is 1,050,000 bytes. A conditional 64-query candidate with an
  authenticated 32-byte composition-mask value per queried row projects to
  502,831 bytes per maximal child, leaving 21,457 under 512 KiB. Two such
  children leave only 42,914 bytes in the 1 MiB AXT envelope; embedding the
  permitted 240 KiB statement as well would exceed it by 202,846 bytes before
  carrier overhead. No masked producer, privacy simulator, concrete 64-query
  soundness bound, exact carrier layout or resource owner exists, so the
  test-only geometry and Core production refusal remain unchanged.
- The MKHE native40 qPCS arithmetic audit found that index-bound leaf hashing
  alone costs 255,332,450,304 tracked operations, above the unchanged
  128-billion whole-proof ceiling; the complete initial tree costs
  3,032,072,370,498. Its 57,475,588,392-operation opening preflight does not
  fund the tree or authenticate a live source. A source-bound commitment and
  evaluation redesign with binding, low-degree, privacy, soundness and complete
  resource arguments remains a prerequisite. The privacy outer hash stays
  SHA3-384; the inner qPCS construction keeps its separately specified
  six-lane hash contract.
- A BFV/Soracloud source audit confirmed that its registered centered
  scale-round chain has eight RNS limbs at degree 64; the atomic 40-limb
  materialization obligation belongs to MKHE F06. BFV's eight-party path
  authenticates a bounded fixed roster but still rejects at the unavailable
  private-share relation proof, and its production qualification function
  still unconditionally rejects pending genuine audited parameter, lattice,
  noise and qROM evidence. Exact Ed25519 roster-key governance is a bounded
  next authentication cut, not a share-proof or release-gate completion. The
  present eight-party declaration carries only a signed opaque hash, with no
  private-share relation proof or safe combine protocol; publishing raw
  `c1*s_i` can reveal `s_i` when `c1` is invertible. The current zero-error
  fixture cannot establish bounded distributed key generation or full-circuit
  decryption noise.
- The four SoraFS purpose-specific signer producers have partial daemon code,
  but the generic software signer still rejects their handles and only test
  `SignerOperationStateSourceV1` implementations exist. Role 11 has separate
  exact signed Reserve/Complete history and Check/finality proofs but no
  purpose-owned same-State completed consumer; Torii correctly refuses its
  completed-proof source. Role 13 Core execution is closed, role 14/15 need
  configured observers and state sources, and each inner approval plus the
  overall promotion checker still has a native authority blocker. The next
  bounded role-11 slice is a read-only completed Check/history join with
  aggregate resource admission, not token issuance or promotion activation.
- A generic-service review confirmed its current `SignRequestV1` returns one
  locally journaled signature and cannot express the four ordered protected
  signatures or the native Reserve/Complete and completed-operation readback
  required by roles 11, 13, 14 and 15. Enabling their handles or payload tags
  would bypass that authority. The coherent first service cut is role 11's
  complete purpose-owned request/receipt, finalized source and read-only
  ambiguous-operation recovery; the other three roles remain closed.
- Role-11 replay resource arithmetic remains unqualified: the current shared
  Check walk reads a complete canonical block at every height, giving roughly
  1,158 GiB of named cold maximum file reads over 4,096 heights. Replacing it
  with one Kura metadata read per height still has a 67 GiB sidecar maximum,
  before its exact Check body, decoder graph and proof work. Those limits are
  not owned by the unrelated 64 GiB privacy or SoraFS archive policies. A
  configured whole-attempt permit, explicit bounded sidecar decode, same-State
  target-body join and cold-cache BLS work accounting are prerequisites to a
  purpose-owned completed-operation consumer.
- A standalone-election fault/invariant audit pinned the required finalized
  history `H`, latest credential states `L(H)`, and exact smallest-unit aggregate
  `T(H)`, including immediate post-cast/update dropout, adaptive corruption,
  close/restart and equal-leakage privacy tests. Current `Vec<u128>` tally and
  ordered 1,000-entry pair corpus are structural storage only. No reviewed
  credential, confidential bond/update, late-dropout completion or closed-corpus
  tally relation exists; both production routes and the vote registry remain
  closed. Older split-corpus/`Vec<u64>` historical descriptions are stale.
- The fixed-slot, pre-issued sum-key election hypothesis fails the release
  privacy condition: substituting a setup zero for an accepted cast exposes a
  proper-subset total, and substituting an older conviction ciphertext exposes
  a historical total, even when two election worlds have identical final
  aggregates. A future candidate must cryptographically bind any pre-issued
  capability to the unique authenticated closed-corpus selection; procedural
  one-time evaluation is insufficient. This design screen neither supplies
  such a construction nor changes the production refusal.
- A [bounded primary-paper screen](standalone-election-programmed-opening-screen.md)
  found that noninteractive private-input validation can tolerate clients
  going offline, but the examined construction retains an analyst master
  secret and permits aggregation of arbitrary ciphertext sets. The examined
  decentralized functional-encryption keys likewise do not restrict reuse to
  the unique latest-version closed corpus. This is not an impossibility proof;
  F11 still needs a concrete master-free, committee-free construction and
  independently reviewed equal-final-total privacy argument.

The independent source audit found no active SoraFS HSM prerequisite:
authenticated software custody is admitted alongside optional hardware
providers, while revocation, finality and replay checks remain mandatory.
KAGEMUSHA's separate offline monetary-authority hardware policy is unchanged.
The daemon still has no production local signer service or finalized
`SignerOperationStateSourceV1` to consume the new bounded journal pool. Merely
constructing the pool at startup would not establish Reserve, Complete, or
challenged Check authority. Torii stream-token retries also lack a stable
signed request ID and durable request/quota owner across a process restart.
The proposed strict request-ID cutover remains unapplied: a source audit found
only test implementations of `SignerOperationStateSourceV1`, an unconditional
completed-proof rejection in Torii's current finality source, and no native
atomic owner of the original token body, authenticated request ID, and quota.
Applying the cutover alone would turn successful issuance into an unconditional
refusal.

The release gates remain open. In particular, signer state sources and provider
backends, whole-operation resource admission, inner completed-operation
verification, the wider Native fault/restart and scaling matrix,
qPCS/FASTPQ/BFV/X509 cryptographic redesign and audit, committee-free election
tally construction, Android fixture replay/native artifact rebuild, full
SDK/hardware parity, independent audits, signed promotion, and same-candidate
network/soak evidence are not qualified by these focused passes. The current
SoraFS promotion checker continues to reject missing inner approval
verification.
