# First-release architecture redesign

This record tracks implementation of the approved SDK and repository redesign.
It is not a release qualification claim. The implementation retains one canonical
first-release interface and does not add compatibility adapters.

## Musubi extraction prerequisites

`iroha_primitives::fs` owns named libc open flags for the previously qualified
platforms. Archive fetching, durable clock/journal storage and publisher file
operations import that one implementation; unsupported-platform rejection stays
at the filesystem consumers. Four host tests cover symlinks, directory-only
opens, descriptor identity after replacement and nonblocking FIFO handling.

The reviewed manifest-graph refresh adds exactly three dependency edges:
primitives to Unix libc, primitives to test-only tempfile, and the publisher to
primitives. Cargo.lock changes no package version. The exact source-graph limits
and fingerprint record this ownership change; forbidden layers and edges are
unchanged. The manifest budget and all 31 guard tests pass.

The publication fixture at `crates/iroha/tests/fixtures/musubi_publication_frames.json`
captures all 28 planned named codecs, their enum variants, enclosing generic
frames, bare signed authorization bytes/hashes, deterministic signatures and
durable state digests. Its SHA-256 is
`9f30e0f9179fa6cf6c35ed118bb89372c7e3e100cf1bbcff4bd2fe60c8524f0d`.
The capture writer was removed before the 75-test runtime verification passed.
Historical compiler names are capture metadata; every frame, payload and signed
transcript must remain exact after relocation. Explicit identity preparation
does not activate the pending workspace-wide Norito identity cutover.

`sorafs_car::musubi::plan` now owns CAR plan and commitment validation for seed
ingress and provider fetching. The closed validation contexts retain their
distinct heap and metadata limits. All 20 CAR Musubi tests pass, including six
adversarial plan tests. All 76 SDK Musubi tests pass after consolidation and
explicit identity declarations, with the captured frames and signed transcripts
unchanged.

The broader SDK run initially passed 788 tests and failed 19. DA geometry and
Nexus network fixtures were stale; response-contract fixtures also required
canonical authority and error-cause assertions. Investigation identified an
actual DA pin-scope bug: marked `Hash` storage altered one bit of the raw BLAKE3
authorization digest. The canonical field now uses the existing DA `BlobDigest`,
preserving every bit. The complete repaired SDK run passes **809 tests, zero
failures or ignored tests**, including two new fixture regressions. The owning
model's eight DA tests also pass: raw-bit preservation, binary/JSON roundtrips,
signature tampering, canonical witness sets and governed admission policies.

The [base extraction inventory](model_base_extraction.md) defines the first
dependency-closed identity move and its codec, FFI and consumer obligations.
Physical relocation awaits the atomic identity cutover.

Connect queue and Soracloud witness paths belong to CLI configuration. The SDK
rejects those sections and accepts a validated application-layer TOML table with
its original source path, preserving relative credential-file resolution.
Configured CLI paths resolve from that same directory. Soracloud authority,
witness path and fee intent share one scoped invocation context, restored
together on exit; witness authentication retains all existing bounds and checks.
The SDK config suite passes 15 tests, with 13 CLI loader, filesystem, witness,
authentication and context-restoration tests. The manifest adds only the CLI's
direct `iroha_config_base` edge; the exact budget and all 15 dependency
boundaries pass. Complete client immutability and async migration remain open.

Integration status helpers and all CLI development binaries now use explicit
blocking/client/account contexts after removal of the blocking facade's `Deref`.
The integration library and its test target compile; the three development-bin
suites pass 40 tests. The CLI invocation guard is structurally `!Send`/`!Sync`,
matching its thread-local restoration responsibility, and its restoration test
passes. Existing synchronous reads remain part of the pending SDK capability
migration; these caller checks do not qualify network scenarios.

Local transaction construction now uses `AccountTransactionDraft` and the
immutable `AccountClient::prepare_transaction` and `sign_transaction` operations.
All six superseded generic helpers and the quote-and-sign composite are removed.
The draft has no network or authority; preparation supplies them from its
context and applies configured nonce/TTL defaults once. Direct signing rejects
foreign networks, substituted authorities and multisig-member contexts; those
members may prepare payloads for external threshold signing. Typed preparation
and signing causes now belong to the crate-level `iroha::Error`/`Result` family.
Ten focused runtime tests pass, including exact signed bytes, attachments,
context isolation, fee binding and entropy failure. The compiler-guided integration
migration replaced 730 implicit blocking-client accesses across 88 files;
configuration changes construct fresh validated contexts, with an isolation
test passing. Musubi, Izanami and every integration test target compile.
Native AMX integration evidence recomputes the finite settlement hash, and the
alias-registry replay fixture retains owned copies of every restart layer.
SDK examples, all CLI/Musubi/Izanami/test-network targets and integration tests
compile. Four specialized SoraFS transaction wrappers are also removed; their
callers use the same draft and account operations. Five focused SoraFS runtime
tests preserve exact instructions, the five-minute moderation TTL and invariant
preflight. Other specialized preparation methods, non-transaction error shapes
and synchronous capabilities remain open. Live four-validator execution was not
run; this remains an ownership and compilation checkpoint.

Fee quoting now has one typed asynchronous account operation with explicit
direct-signature or multisignature-witness authorization. Raw response handling
is private, and synchronous callers use the owned blocking runtime. All old
public raw and witness-specific quote methods are removed. Exact payload and
witness binding, signer order/threshold, response bounds and structured fee
error details retain focused passing tests. Every CLI target and integration
test target compiles through the canonical operation. The broader SDK error
family and remaining synchronous capabilities still require consolidation.

The generated event family now has explicit captured identities for all 25 set
types and 12 enclosing data-event enums. All 225 captured set frames and 75 JSON
values remain exact. The full derive suites pass 48 tests after rejecting
duplicate origin/identity metadata and replacing the stale numerical direct-FFI
check with a declaration inventory. The [identity specification](norito_schema_identity.md)
retains the remaining generated/generic and atomic-cutover obligations.

The Musubi digest, bounded-text and page generators now require explicit
identities for all 17 types. Their 49 captured values preserve 196 complete
frames, including generic containers and populated pages. Active codecs remain
unchanged; this qualifies the generated-family preparation only.

All 16 governance hash wrappers now also declare their captured identities.
A separate capture of these handwritten codecs preserves 256 complete frames
and 64 JSON values; their active codec bodies remain unchanged. Both generated
Musubi and governance identity suites pass after these declarations.

All 12 current instruction enums now declare their captured identities, with
57 variants and 228 complete root/container frames preserved. The compiler
identified two mint/burn callers absent from the historical generated-family
review; their original codecs received a separate capture. This closes that
generator's current declaration set, not the remaining instruction families.

The instruction suite also exposed stale fixture assumptions. Governance
negative-layout tests now resolve the current canonical wire identifier before
framing a retired payload. Cross-family tests under removed Rust type names
require lookup rejection; registered canonical cross-family checks remain.
Shared test-helper ownership is checked per consumer instead of by aggregate
call totals. The wire-ID digest refresh was reviewed against commit
`7035517653`: only the already-committed KAGEMUSHA top-up and redemption names
replace the retired offline-cash names; all 349 other assignments are exact.
Both previous golden digests were reproduced from the parent source before
computing the replacement digests. Production registry and codec logic are
unchanged by these fixture repairs.

Native AMX participant evidence uses `NativeAmxParticipantSettlement`, which
contains the exact zero-effect fields and ordered source receipts but cannot
contain Native AMX receipts. This removes the former type cycle from
`LaneBlockCommitment` through `NativeAmxReceipt` and `NativeAmxLegRecordV2` back
to `LaneBlockCommitment`. Its domain-separated typed hash binds that finite
record and its source order; the removed recursive JSON field is rejected.

The complete instruction library selection passes 319 tests after these repairs.
The two finite Native AMX participant-settlement regressions also pass again on
the default stack. This is local mutable-tree development evidence; full model,
workspace, native and source-bound release qualification remain open.

Twelve Nexus instruction records now declare their captured identities and
preserve 120 complete frames across 24 populated values. All nine owner tests
pass, including ordering and invalid-withdrawal checks. The immutable fixtures
exercise each existing JSON surface: instruction carriers for all twelve and
direct withdrawal JSON. This adds no codec, proof-admission or finality claim.

All twelve current `isi_box!` callers now supply captured identities through a
single required generator derive. Exhaustive variant matches preserve 68
populated variants, 340 complete frames and 68 instruction-carrier JSON values.
The complete instruction selection passes 336 tests after these declarations.
The broader compiler capture records 433 actual identities and 413 distinct
payloads across 330 types; remaining instruction-record fixture gaps are tracked
explicitly. All one-time probes were removed, and the production source matches
its captured bytes after removing only the new identity declarations. Active
codec cutover and model crate moves remain pending.

A complete default-stack model-library diagnostic run then finished with 3,081
passes, 29 failures and six ignored tests; it did not overflow the stack.
Five stale fixture assumptions have since been repaired and pass individually:
the multisig preflight uses a valid current header and proves zero allocation
charges; bridge tag checks inspect the fixed-width prefix; every block decoder
rejects removed nested instruction aliases; query scope includes its canonical
null content; and the audit fixture proves valid complete attribution before
removing the required result commitment. All 336 instruction tests and both
finite-settlement regressions pass again after those repairs. Production
validation and decode resource limits are unchanged. That left twenty-four
other failures before the following codec corrections; the full suite remains
unqualified pending a fresh run. Exact failures from that earlier run are retained under
untracked `target/architecture-redesign/model-library-after-stack-and-box-fixes-review.json`.

The signature-layout diagnosis found a handwritten slice decoder interpreting
the tuple wrapper's field framing as a signature sequence count. It is replaced
with the decoder derived from the same declaration; serialization is unchanged.
Six signature tests pass, covering advertised layouts, exact bytes and rejection
of empty, zero, truncated, trailing and unwrapped signatures. Packed named and
tuple decoders now enforce their complete boundary in table and bitset layouts;
empty table layouts consume their existing zero-offset sentinel. Explicit prefix
decoding continues to report the exact bytes consumed.

`ConstVec` now decodes the advertised element layout once, preserves the first
error and checks complete field consumption. The removed recovery paths retried
the same malformed value, copied aligned payloads and accepted re-encoded or
length-mismatched values. The shared sequence planner charges the count once;
tests prove its exact allocation boundary. `Vec<u8>` retains its raw-byte layout
and `ConstVec<u8>` retains its element layout. All 812 Norito grouped tests, 285
primitive library tests and 48 final immutable-vector tests pass. Strict Clippy
for Norito, its derive crate and the primitives library now passes after three
equivalent numeric-range expressions and filesystem Rustdoc formatting are
corrected. The complete JSON group passes 152 tests, including exact-integer,
saturation, NaN, infinity, fraction and negative-zero equality checks.

The RPC peer fixture still contained the `hsm` field removed from
`RegisterPeerWithPop` by `40f44e84cde4ffca106a47db9db8438cd063989f`.
Repeated vector recovery attempts masked its first length mismatch with
cumulative-budget exhaustion. The authoritative Rust renderer now regenerates
the current four-field record with the original public key and proof. Review
confirms that only this entry and its signed dependents change across ten
canonical/mirrored files; all 55 other publication files remain byte-identical.
The one-time writer is removed. Two subsequent public-API publications under
independent absent external roots match all 79 filesystem entries, including
all 65 file bytes and modes, and every tracked fixture matches them. Three
public fixture files had mode 0600; restoring their required 0644 modes closes
the exporter filesystem check. All 44 exporter tests pass. No decode budget or
stack override was increased.

The final dependent build passes. Its complete model-library diagnostic run
finishes on the default stack with 3,092 passes, 21 failures and six ignored
tests; both finite Native AMX regressions, all signature tests and the complete
RPC manifest roundtrip pass. Two failures are subsequently corrected: the
moderation anchor requires an explicit JSON key while permitting null, and the
address test asserts rejection when constructing an unsupported header version.
Those two cases and all ten authorization tests pass on the subsequent build
without warnings. Moving authorization into its own test module reduces the
transaction test file from 3,093 to 2,770 lines and preserves every assertion.
The final compiled model also passes all nine signature, finite-settlement and
RPC-manifest regressions after those corrections and numeric lint cleanup.
Nineteen other model failures remained at that checkpoint; it did not qualify
the complete final suite.
All 47 derive unit tests and 14 strict-JSON tests pass. Kotlin fixture parity
passes 17 tests, the selected Java-source consumers pass 146 and the Python
fixture-validator suites pass 324. The actual Python, Android, Swift and
cross-SDK checks agree on all 27 fixtures. Full JVM testing reports 1,418 tests
with 56 failures involving absent generated/native artifacts and two separate
privacy KAT drifts. Python SDK runtime tests cannot load the unbuilt `_crypto`
extension; Swift cannot build without `NoritoBridge.xcframework`. These are
unverified runtime qualification, not passing results.

The typed Musubi fixture producers and shared signed JSON now agree on the
purpose-issued order namespace introduced by `f9a236e869`. Review of the
deterministic producer output confines the refresh to the order ID and its
signed dependents; schema fields and unaffected cases are exact. Six Rust and
33 Kotlin tests pass, alongside the two fixture-diagnostic tests. Swift remains
unverified because the required native bridge is absent. The
[fixture record](musubi_signed_fixtures.md) retains the exact scope and hashes.

All 292 current `isi!` declarations now supply explicit captured identities.
Their 283 nongeneric records and 39 instantiated generic forms preserve 357
values and 1,428 root/container frames across 322 concrete types. The 51 missing
populated values were captured through their owning typed constructors before
the declarations were applied. All 325 record tests and the twelve-argument
marker identity test pass; active codec identity and field layouts are unchanged.
The [record fixture](../crates/iroha_data_model/tests/fixtures/instruction_record_generated_identity_frames.md)
retains the exact capture scope and digest.

The remaining model failures were traced to their owners. Consensus and bridge
fixtures now account for the already-committed paired Pasta epoch roster and
complete SORA finality anchor. Invalid roster tests retain every earlier
assertion at the constructor that owns validation; forged successor tests still
reach certificate/context binding. Intent and ZK-ACE known answers were captured
from the production typed APIs after the committed six-lane digest and proof
envelope changes. FHE negative cases use a valid current policy before changing
the field under test.

Two independently generated Exact12 publications are byte-identical and match
the adopted TSV and typed bundle. The production fixture checker passes. The
Kotlin implementation now pins the decoded archive digest
`ca479cad31f3d3fb6d834b5d490bb14f0f823ae4b869cf9c7cb5e32c20250035`.
Its six codec tests pass, as do six native-independent Java consumer tests,
42 Python parity tests and 72 standalone Python codec tests. The two required
Java JNI tests fail because the ABI 23 native exports are unavailable. The
standalone Python run loads the repository codec modules directly and does not
qualify normal SDK import or the absent native `_crypto` extension.

The next complete model-library run finishes on the default stack with 3,439
passes, one failure and six ignored tests. All recorded source inputs remain
unchanged during the run. The FHE investigation exposes two stale production
descriptions from `a29120a068`: the composition challenge uses six
64-bit lanes (48 bytes), and removal of `hash_fn` leaves 57 proof-profile fields.
The public schema now describes both owning definitions exactly; its known
answers and contract fixtures are updated coherently, and the complete focused
schema test passes without relaxing its assertions.

An additional Native AMX regression encodes the removed nested
`LaneBlockCommitment` layout beneath the current leg schema identity. Across six
declared layouts, the current finite leg roundtrips and the removed thirteenth
settlement field is rejected. Current QCs and hash bytes are retained in the
negative fixture, isolating the nested layout boundary. This test passes on the
default stack. The schema known-answer pin also now uses the owning Iroha
`Hash` marker, with all ten focused FHE schema tests passing. The complete final
model-library run passes **3,441 tests with zero failures** in 760.71 seconds on
the default stack. Its six ignored entries are explicit fixture-generation
helpers. All 997 recorded source/fixture inputs and the test artifact remain
unchanged throughout the run. These are selected development input hashes,
not a complete workspace release seal.

Core's receipt-append crash test passes. A real reopen test exposed a fixture
that wrote the primary lane-incarnation marker without first recording its
authenticated geometry binding. The fixture now uses the same primary anchor
initialization API as production State before publishing markers or evidence.
The temporary recovery matrix and authenticated primary restore test pass with
all recovery validation unchanged. All **60 Native AMX Kura tests pass** on the
final artifact after related empty-store fixtures establish valid geometry
before writing their invalid sidecars. The symlink case recognizes the owning
recovery error while retaining its link, target and inventory assertions.
Rejection, size/count bounds and no-mutation checks are preserved. The separate
authenticated primary restore test also passes again on that same artifact.
The Core build retains 193 existing warnings; strict Core Clippy, workspace,
native, four-validator and source-bound release qualification remain open.
After the tuple and metadata corrections below, a fresh Core build again
passes all 60 AMX tests and the authenticated primary restore check on one
artifact, with the default stack and all 2,417 recorded inputs unchanged.
The dependency build additionally reports 23 FastPQ warnings.

The next storage-wrapper capture exposed two layout errors in Metadata's
handwritten encoder: nested entry fields promoted explicit fixed-width lengths
to compact lengths, and packed offsets were emitted without being advertised
in the header. Metadata now streams a borrowed entry view through the shared
field writers, counts packed entry lengths, emits checked offsets and verifies
each streamed length. Its duplicate entry buffers and unchecked cumulative
offset addition are removed. Tuple, block-signature, instruction and query
writers now inherit explicit layout flags; defaults apply only without an
active layout. All 1,246 Norito tests pass (one fixture generator ignored),
strict Norito-library Clippy passes, and the three metadata tests plus two
block-signature wire/allocation tests pass. The instruction/query tuple parity
checks include the previously missed packed-sequence-only mode.
Three dedicated metadata integration tests also pass. With preallocated output
and warmed encoder state, both 16-byte and 256-KiB JSON strings require zero
non-packed scratch allocations. Packed metadata allocates only one entry-count
offset-length vector, independent of payload size; empty metadata allocates
nothing. The same fixtures retain exact sequence-of-tuples bytes and framed
roundtrips under fixed, compact and packed layouts.

The immutable [storage identity capture](../crates/iroha_data_model/tests/fixtures/owned_storage_identity_frames.md)
preserves 432 complete frames across nine values and 40 nominal identities.
`Owned<T>` composes the captured nominal constructor and forwards its inner
root projection; account/NFT/RWA storage records have captured declarations.
All seven common-module tests pass, including exact fixtures, truncation,
container-header substitution and marker-only nested wrappers. The capture
writer is removed. These declarations do not change active codec selection.
The revised model-library artifact passes all **3,446 tests** with the default
stack; six manual fixture generators are ignored. Its executable digest and
all 1,000 selected source-input hashes remain unchanged through the 792-second
run. The capture record and its 999 pre-declaration input hashes are retained
beside the fixture. These are scoped development checks, not full release
provenance. The history archive verifies and all 74 history/source-budget guard
tests pass; the source-size audit still reports 231 violations.
Strict model Clippy is not passing: the dependency-inclusive run reports five
crypto errors and one proof-library error. A separate model-target-only run
reports 129 model diagnostics. Their complete logs are retained under
`target/architecture-redesign/owned-storage-identity/`; neither run qualifies
strict model or workspace linting.

TODO: Correct the independently discovered numeric JSON map-key writer
asymmetry. Numeric key decoding exists, but the current BTreeMap writer emits
unquoted key tokens. A reviewed draft remains unapplied: its infallible path
would turn unsupported-key errors into panics. The correction must retain
string-key bytes, bounded writes and typed failures on fallible entry points;
neither the storage fixture's string keys nor the draft qualify that behavior.

## Accepted design

- Split foundational, privacy, and service wire models into independent
  compilation units, with aggregate ledger composition in `iroha_data_model`.
- Keep node execution and product service implementations outside the Rust SDK
  and storage client dependency graphs.
- Replace mutable client configuration and global transports with immutable
  public/account/operator contexts and asynchronous capability interfaces.
- Keep an explicit blocking facade over the same transport implementation.
- Compose Core storage and Torii routes from capability-owned components.
- Enforce dependency boundaries, module sizes, and measured compiler memory
  instead of a repository-wide Rust line-count objective.
- Classify CI consumers before building their required binaries.
- Use Kotlin/JVM as the sole JVM implementation, with Java consumer tests.
- Keep current status and roadmap documents below 300 lines and preserve
  historical evidence in indexed records.

Focused new crates and a corresponding Cargo.lock refresh are authorized for
this redesign. Mandatory consensus behavior, Norito layout guarantees, and the
single IVM ABI remain enforced. Existing uncommitted work must be preserved.

## Implementation state

| Area | State | Remaining acceptance |
| --- | --- | --- |
| Source inventory | Source state and 657 canonical Torii routes recorded; 8 inventory tests pass | Request/response and SDK consumer mapping; sealed, comparable build-memory baseline |
| Relay incentives | Extracted to `soranet_incentives`; consumers use its canonical API | 13 Rust unit tests pass; downstream checks remain |
| SDK dependency boundary | Core, Torii, daemon, IVM, FastPQ prover and forbidden Halo2 execution features absent; direct SDK node-config dependency removed | Remove the remaining node-config path through telemetry, plus storage orchestration and telemetry implementation dependencies |
| Service policy ownership | `iroha_service_model` owns the four canonical SoraNet policy enums and SoraFS alias-cache defaults; duplicate runtime/config definitions and reexports removed; 4 policy and 131 architecture/router tests pass | Full policy consumer checks and remaining service record extraction |
| Configuration wire records | Canonical shared records; 22 wire, 3 node conversion, and 14 Core runtime tests pass; SDK/CLI check passes | Torii and full consumer qualification |
| Status wire records | Canonical shared records preserve all 32 captured named DTO frames/hashes/JSON; 93 telemetry tests pass; Core/Torii, SDK/CLI, test-network, schema generator and Mochi consumers compile; three integration harnesses compile | Runtime consumer qualification; generic-envelope identity contract |
| CLI artifact ownership | Compiler command moved into `iroha app sorafs toolkit compile`; archive packing owns its CLI module; 10 toolkit tests pass; unused CAR node dependencies removed | Remaining standalone CLI migrations and complete client/storage extraction |
| JSON field dispatch | Unified implementation; FNV, portable CRC, ARM acceleration, and parser regressions pass | Full downstream codec qualification |
| CI binary routing | Implemented; 82 router/workflow tests pass | Execute selected jobs on CI infrastructure |
| Foundational/privacy/service models | Service policy compilation boundary implemented; larger record moves pending | Canonical codecs, registry completeness, and compile-memory measurements |
| Client API and transport | Pending | All consumer migrations and authority/async/finality regressions |
| Core state and Torii routes | Pending | Atomic commit/rollback/replay and capability isolation |
| Architecture budgets | Feature-resolved dependency checks (31 tests); file/provenance schema 2 and release routing (128 tests); compiler-unit profiler (148 tests); measured memory comparator and four pinned baseline contracts (43 tests) | Remaining SDK forbidden edges, source findings, failed baseline repair, pinned-runner CI and comparable candidates |
| JVM consolidation | Norito harness migrated into 55 Java-source tests against Kotlin; 34 related Kotlin tests and 14 JVM gate tests pass; duplicate Java harness removed | Pre-CUDA checkpoint: 1,295 JVM tests pass with rebuilt JNI and fixture generator. Canonical CUDA source adds eight passing Java consumers and five compiled CPU-reference hardware tests; rebuilt binding and hardware execution remain unverified. Kotlin attestation tooling passes 25 tests and real launcher fixture checks; complete remaining Java/JNI/publication removal and native packaging |
| Documentation | Root current views remain below 300 lines; exact dirty history is reconstructable from dated subsystem records, with all 66 roadmap areas mapped to owned outcomes. Archive verification and 70 combined archive/release-contract tests pass; PR gate enforces integrity and 300-line limits | Complete SDK operation/consumer mapping and reconcile remaining source-coupled/public documentation |

The initial source inventory and lockfile snapshot are local build artifacts in
`target/architecture-redesign/pre-change/`. The authoritative memory profiler
must produce valid baseline and candidate reports before memory improvements
can be claimed; ordinary Cargo build output is not that evidence.

The model baseline input is frozen in an independent checkout. Its captured
working-file set has digest
`a809f92277286502aaff109a2bf55efaf43d910aaa84aa0926a336a0f4039bdf`;
the profiler's complete Git inventory, including the absent optional docs
gitlink, has source digest
`36524c89573e4c8bfbb162b43d3d195e9bdd21cc335c40529059faf88a68af9d`.
Its exact source snapshot and private Rust 1.93.1/offline caches were verified.
Optional native artifacts remain absent. The profiler preserves bounded dangling
source links and checks its full source seal before and after execution. The
earlier model and SDK runs completed with reconciled artifacts, but their
instrumentation rejected valid build-script source probes. Their `valid=true`
fields attest the then-implemented seal/reconciliation checks, not an unperturbed
build: `num-traits` lacked `has_total_cmp`, `num-bigint` lacked `has_try_from`, and
`proc-macro2`/`rustix` lacked compiler-capability cfgs present in ordinary builds.
Both successful reports are superseded by the corrected runs below.

Preserve these reports as historical measurements, not accepted memory baselines:

| Run | Input digest | Historical measurement and limit |
| --- | --- | --- |
| Model package | `aff1fc3b8888fb6ffc5f2819f53eeb1903a706ac734392d6b53f2cd7436e9341` | 338 reconciled units; model peak 13,270,646,784 bytes. The arithmetic 25% threshold is 9,952,985,088 bytes, subject to baseline requalification. |
| SDK | `7f39d24eaa4040d55eda280b1ac54255a4b968c1e27f514b3dce1c217161406d` | 498 reconciled cold units; SDK peak 1,574,060,032 bytes and selected model peak 13,299,662,848 bytes. |
| Core tests | `00faa0f7e741d04724eff32f75b4347fc33ff6fb515cb0b8fe2d7c6c4cc37535` | Failed in Halo2 0.3.4 because the wrapper rejected indexmap 1.9.3's stdin std probe. The frozen source is unchanged; dependency feature workarounds would hide the instrumentation defect. |

The corrected instrumentation passes 148 tests, including real ordinary/measured
Cargo probe parity, stdin and file input identities, direct RUSTC calls, and
fatal evidence for swallowed instrumentation failures. The new cold model run
qualifies with 338 compiled units, zero fresh/failed units, and 31 probes (12
source probes). All source/toolchain seals remain stable. It restores the
ordinary `has_total_cmp`, `proc_macro_span_location`, `proc_macro_span_file` and
rustix capability cfgs without modifying the frozen source.

| Qualified run | Input identity | Measured limit |
| --- | --- | --- |
| Model package, corrected instrumentation | `1e6781959e8678dbf77291317e675a23a14230c8f1d50f4fac0c28d0a0c93cd4`; compiler-measurement inventory `6085dc6fcb6e7fd8f2b8d7f428e56e9fcfcb6f5675bec4489a4dee1d0438cb3b` | Peak 13,275,365,376 bytes; candidate must be at most 9,956,524,032 bytes for the required 25% reduction. Ordinary dev/lib features and model optimization are unchanged. |
| SDK, corrected instrumentation | `4256b0cd9dcd1b114b9ce5baee7b59f270926f14861783f6d4795bafbb4034db`; compiler-measurement inventory `055b2ff99d93c4edda5f9dfd39675f938089f054cfa36d81ce3364103750a252` | 498 cold units, 46 probes (16 source probes); SDK peak 1,576,681,472 bytes, selected model peak 13,295,861,760 bytes. Source/toolchain seals stable. |

The qualified run uses helper SHA-256
`3a18f171298ca0086d261e2869738e813c27ce7fc74393db2d393350b95deca7`
and profiler SHA-256
`02b61757b82c4c74d783c0308197085076fa8582236821b316d0a443763c6d73`.
The corrected Core test run has stable source/toolchain seals and restores the
indexmap std probe. Its input is
`f2b6684fadcc37476b9d854e236c54f1fdf482a9eadca9d13481f405ca45c0bf`;
533 cold units complete, including Core's 24,238,915,584-byte dev/library peak.
It then fails on eight removed ballot/lock/referendum API references in
`gov_bond_escrow.rs`. The report is correctly invalid (`returncode=101`), so the
incomplete test build cannot qualify a Core budget.

The corrected Torii test profile has input
`d5f7cbb8a29c2694062592302533f100b25e380d671484be505ad813b24effff`
and measurement inventory
`4902037aeb3f6ecf551f7679306e58d64b9a5686fcbf40a905b7a9972445b96e`.
It completes 608 cold units and 50 compiler probes with stable source/toolchain
seals and no instrumentation error. The Torii dev/library peak is
10,369,712,128 bytes; its library-test target then fails with 82 compile errors
from stale fixtures and callers. This report is also invalid (`returncode=101`),
and cannot qualify the complete Torii test budget. Fixture migration, the
remaining release surfaces, a machine-enforced budget inventory, and comparable
candidates remain required. No candidate memory improvement is claimed.

The corrected daemon release profile has input
`5db4a2dca527e312ac62cf811e55cb52dea79fae1b31d97033b1faad270daecd`
and compiler-measurement inventory
`89515c8f27e5d44f5803f85d9875eadf08ea9dd207f323ffebc578bc89c36d5e`.
It finishes 606 cold units and 54 probes, with stable source/toolchain seals and
no instrumentation error, then fails because the frozen daemon manifest lacks
`iroha_torii_shared`. That dependency is present in the working tree. Completed
model/Core/Torii release units peak at 32,699,990,016 / 38,454,771,712 /
18,646,286,336 bytes. The model retains optimization level 1; Core and Torii use
level 3. All exceed the retained 13-GiB ceiling. The report is invalid and cannot
qualify a release budget.

The corrected JNI bridge release profile succeeds with 562 cold units, 52
compiler probes and stable source/toolchain seals. Its input digest is
`7241242d7ea7710a5a566392508713bfb28c0b97ac0e47eae49df5ec72faaa94`;
the compiler-measurement inventory is
`31089c30c88f6f85a9e193b9abe1135d8f4047b79b6f76651741949f336a6c15`.
Bridge, Core and model peaks are 1,195,311,104, 35,761,225,728 and
32,736,935,936 bytes respectively. The model retains release optimization 1.
This is a qualified baseline measurement, with Core/model above the retained
13-GiB ceiling; it is not current-source JNI execution or candidate qualification.
The JavaScript native release baseline also succeeds against the same frozen
source: 595 cold units, 54 probes, zero fresh/failed units and stable seals. Its
input digest is
`26d886cf671006d7d695911f8ac391b0884ccd2449825940585d7ade5e498042`;
the measurement inventory is
`699e55a3a33bf848507a844879793d3db8443771c565f0c6b34aea825e85ddb1`.
JavaScript host, Core and model peaks are 1,412,186,112, 33,764,720,640 and
32,704,380,928 bytes. Core/model exceed the 13-GiB release ceiling. The complete
report SHA-256 is
`5bf51f8b9cd0231b9cdd9407e79f4c804b5739ccba127dc923b3960e348b689e`.
This is host baseline evidence, not current-source native execution.

The wider release checks remain failing: the SoraFS automation suite reports
3 stale mobile-workflow contract failures (572 tests pass), and the refreshed SDK source
closure suite reports 9 failures because current JavaScript/Kotlin source inputs
are untracked. The exact production inventory includes the new files; its Git
tracking requirement remains enforced.
These are outstanding qualification work, not passing results.

## Completion gate

TODO: Complete each pending implementation and acceptance item above. In
addition to focused tests, qualify one consistent candidate with workspace
build/tests, the mandatory consensus harnesses, valid release-feature checks,
formatting, strict Clippy, the codec guard, and four-validator integration tests.
Rebuild native artifacts from that candidate; unavailable device qualification
must remain explicitly unverified.
