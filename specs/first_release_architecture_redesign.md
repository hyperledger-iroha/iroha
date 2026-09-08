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

The numeric JSON map-key writer now uses explicit `JsonObjectKey` and
`JsonObjectKeyOwned` contracts. Maps own quoting and escaping; unsupported
composites fail at compile time. Checked key visitors retain conversion errors
through references, enforce output bounds, and stop after the first sink error.
The complete Norito test selection passes 1,258 tests (one fixture generator
ignored), three compile-fail documentation cases pass, and strict library
Clippy passes. Two isolated allocation tests prove oversized string/byte-array
keys reject before allocation and successful hexadecimal keys allocate only
their output. These results retain unchanged selected kernel inputs.

The owner JSON selection passes **506 tests**: model 435, crypto 23 and
primitives 48, with no failures or ignored tests and matching hashes for all
1,041 selected inputs before and after the run. Six dedicated model contracts
cover named/numeric identifiers, compute risk classes, scoped assets, account
decode-budget errors and escaped proof backends. Their allocator probe verifies
that DomainId and StatePath decode charges cover observed owner allocations,
including Punycode validation and Unicode NFC checks. Governance now owns key
contracts for all 16 fixed-size hash identifiers and ten Parliament body labels.

TODO: Qualify the remaining Core, storage and SDK consumers. The post-migration
full model and selected Core results are recorded below. Bare `--no-default-features`
checking reports 16 errors in unconditional JSON helpers; the supported minimum
`--no-default-features --features base-codec` library check passes. The key
migration does not claim workspace/native/release qualification.

Norito now keeps length counting in the destination through fields, collections
and embedded instruction frames. A measuring helper visits each child once;
only that helper can add the measured length without replay. Byte output still
checks lengths, checksums and finalized layout flags. The canonical field writer
takes a destination and value; unused scratch types, generated buffers, wrapper
functions and optional-hint gates have been removed from all callers.
All **1,642 codec, derive and primitives tests pass** after this removal (three
ignored cases), and strict library Clippy passes for those three packages.
The new runtime test proves the original payload slice reaches the destination
directly; generated-code tests reject reintroduced scratch storage.

After scratch removal, the rebuilt model passed **3,464 tests** (six ignored
generators), all six allocation contracts passed, and the rebuilt SDK passed
**706 tests**, with `RUST_MIN_STACK` unset. The corrected roster-error fixture
preserves separate consensus- and mint-roster rejection assertions without
changing production validation. A concurrent merge changed 395 captured inputs
while these pre-merge executables ran; the results are retained as development
evidence and do not qualify the merged checkout.

Before scratch removal, the complete SDK also passed 706 tests with all 1,194
selected inputs unchanged. Its original Musubi boundary case completed in
98.02 seconds, compared with 464.74 seconds at the earlier counting checkpoint.
Those local timings are diagnostic evidence, not pinned-runner performance or
memory qualification. The merged participant settlement now owns seven private,
validated control fields; previous-settlement linkage is a fixed-size typed
hash, so it does not restore recursive receipt ownership or schema expansion.
All **35 merged-model regressions pass** with `RUST_MIN_STACK` unset, including
schema generation, codec/hash/drop for the maximum 4,096-source group, prior-hash
vectors, rejection of the removed recursive layout, instruction framing and
roster validation. All 5,084 selected inputs remained unchanged through this
run. The merged Core/Torii library check also passes with unchanged selected
inputs; it is compilation evidence, not runtime or strict-lint qualification.
The broader merged Core selection passes 239 tests with three failures, while
all seven selected Torii tests pass. Those failures expose an evidence fixture
that conflicts with State-established lane geometry and a stale model identity
in the production transition-witness constructor. The unchanged-input result
is retained before repairs; it is not a passing Core qualification.
The repaired run passes **396 Core and seven Torii tests**, with all 5,090
selected inputs unchanged. Runtime witnesses, the shared checker and the Verus
theorem now consume one source identity checked against the actual TLA bytes;
the fixture preserves State-established incarnation identity. Pinned Verus
verifies the actual witness theorem with no proof escapes (one selected theorem,
not the complete mandatory harness). The trace contracts now authenticate the
strict readers, propagated persistence errors and exact payload/receipt checks;
action dispatch is counted independently of the shared ActivateKura custody
condition. One iterative function-body reader handles unparenthesized Verus
contract branches without changing proof or TLA bytes. The previously
unwitnessed replica Queue observation now uses the canonical node wrapper.
All 10 Core witness, Queue and restart regressions pass after removing a second
reservation of an already-held fixture fee and binding the replay source region
to its function signature. The fee test compares every field of the retained
nonzero hold. All 19 source tests and 35 subtests pass, including a complete
snapshot authenticating 28 actions and 29 runtime bindings; all 5,104 selected
inputs remain unchanged. The mandatory shared reducer harness passes 197 tests
with zero ignored. Its inventory was compared against the compiled historical
187-test source: all 187 remain, with ten additions and none removed. The full
pinned Verus harness then verifies all 221 project obligations and 1,690 vstd
dependency obligations with zero errors, `--rlimit 60` and `--no-cheating`;
all 40 selected harness inputs remain unchanged. This completes the scoped
Verus execution, not whole-candidate release qualification. All eight mandatory
model-replay cases and eleven fast simulations also pass. The explicit
100,000-height simulation completes 50,000 permissioned and 50,000 NPoS heights,
with all 43 selected harness sources and fixtures unchanged. These use
certificate-supplied reducer fixtures and do not qualify production peer
networking. The replay run omitted fixture files from its pre-run source
manifest; its runtime result remains development evidence. Workspace formatting
and all four retired-codec guards pass. The source-size check still reports 237
violations over 10,643 files, with no new violating paths or expanded exceptions.
The receipt-source contract now follows the shared strict structural reader,
propagates corruption/recovery errors, and authenticates lock order, exact
observation revalidation, durable retries and post-write attestation. All six
focused tests and 28 subtests pass with 5,105 selected inputs unchanged. The
broader three-test source selection initially passed two tests and stopped at
an independent retirement-progress pair inventory mismatch. The reviewed
retirement contract now authenticates all seven artifact pairs, including the
constant-named canonical replica, with strict durability and error propagation.
All 15 focused receipt/retirement tests and 63 subtests pass. The shared
retirement fixture now builds an exact four-validator committee, three signed
Prepare/Commit votes and complete roster PoPs. Its lifecycle projection derives
committee identity, producer and READY signers from the actual attempt binding
and certificate. All ten retirement regressions pass, with all 5,609 selected
inputs unchanged and no stack override. A subsequent cleanup removes the now
unused test-only import; this scoped result is not a whole-candidate seal.

The production-inventory parser retains every literal owner, so the six native
AMX tests are included in the unchanged 881-test inventory seal. Exact guard
checks follow the audited current source. Historical prose assertions now use
the structured receipt selector/command contract. The canonical proof-fidelity
selection currently collects 5,877 tests against an exact release pin of 5,507;
that mismatch remains an explicit qualification blocker. The release count is
not refreshed from collection alone. The recovery mutation harness now replaces
complete code-token spans rather than matching Rust line wrapping. It retains
all ten existing rejection checks and adds four cases covering capacity, read
errors and exact repeated discovery. All four selected candidate source tests,
including the complete corridor source test, pass; the applied three-test
selection also passes with the tested afterimages and all 66 other monitored
sources unchanged. This is source-contract evidence, not corridor execution.

Metadata now projects borrowed entry views into the shared sequence writer,
removing its separate measurement/emission implementation. All 1,655 current
codec/derive/primitives tests pass (three ignored cases), and strict library
Clippy passes for those packages with all 5,086 selected inputs unchanged.
The full model suite finishes on the default stack with 3,566 passing, nine
failing and six ignored tests; all 12 allocation/JSON-key integration checks
pass on unchanged selected sources. The failures concern two obsolete invalid
domain constructors, five retired Kaigi scalar captures and two registry
inventories that predate 17 game/NFT-market instructions. Reviewed fixture
repairs retain the exact original capture, reject each retired scalar in all
four containers, pin new canonical private values, and preserve every existing
wire-ID assignment. All **409 focused tests and 3,576 full-library tests pass**, with zero failures
and six full-suite cases ignored; the complete run takes 617.69 seconds on the
default stack. Of 5,093 selected inputs, only the concurrently edited SDK
`client.rs` changed; this is explicitly retained as a whole-candidate
qualification limit. Workspace formatting passes, and 48 architecture tests
plus 31 subtests pass. The current codec
source gate passes nine tests, including 23 diagnostic-specific mutations;
historical helper fingerprints are preserved in the dated history record.
Source budgets remain unchanged, and two companion historical gates still fail
11 of 18 tests on stale manifest/lock and source assumptions.
Five generic query owners, four concrete records and two typed-hash markers
now declare identities from successful pre-declaration captures. Immutable
fixtures preserve all 96 default and 116 ids-projection frames, directional
hashes, marker composition and private decoder budgets. The default query
suite passes 202 tests and ids-projection passes 203, with zero failures or
ignored cases and all 5,109 selected inputs unchanged in each run. The fresh
default model artifact also passes all eight participant-settlement regressions
with `RUST_MIN_STACK` unset, including the maximum 4,096-source schema, codec,
hash and drop case. These checks preserve active codec dispatch; atomic identity
cutover and physical model extraction remain outstanding.

The next ten model identity declarations preserve 68 captured generic/concrete
frames and five FHE signing preimages. InstructionBox keeps its wire-pair root
projection while its generic parents retain the nominal instruction name;
TriggerId retains the captured private model scope. The borrowed FHE helper is
encoding-only. All nine model/query identity tests pass on one rebuilt artifact,
followed by ten participant-settlement tests and the removed recursive-layout
rejection test, all without a stack override. Every one of the 5,613 selected
inputs remains unchanged. An independent source review confirms the finite
record removes the ownership cycle; this does not establish decoder allocation
peaks, other feature coverage or complete release qualification.

The source guard now applies the existing 3,000-line test limit to pytests,
Swift Tests directories and split Rust test directories; all 50 guard tests pass.
The release corridor's unchanged acceptance and inventory tests now live in
their capability components, sized 1,830, 2,102 and 2,938 lines. All five focused
candidate tests pass, including the complete corridor source test; four applied
smoke tests pass with the exact reviewed afterimages. All 5,877 canonical test
IDs are preserved. The current repository budget reports 242 findings and 174
existing exceptions, with no new violating path after the split. The main test
module's complete decomposition and the exact 5,507 release-pin discrepancy remain unresolved.
Its provider checks now belong to the inventory component: main falls to 33,974
lines and inventory remains below its default cap at 2,250. Only the main
exception ratchets down, from 34,006 to 33,974. All 54 applied provider/inventory
and source-budget checks pass, and all 5,877 collected test IDs are preserved.
The resulting repository budget reports 241 findings with the same 174 exceptions.

Seven concrete model owners and two encoding-only adapters now declare their
captured identities. Immutable fixtures retain 52 populated root/container frames,
a block-send projection and two reputation event-ID projections. A normal
consumer exposed DataEvent's implicit GameSession tag shifting from 22 to 21
without governance. Every variant now has its canonical explicit discriminant;
Governance reserves 19 and the following common variants retain 20, 21 and 22.
The complete default/HTTP model-library suite passes **3,593 tests**, with zero
failures and six fixture generators ignored, in 599.93 seconds on the default
stack. All 5,617 selected inputs remain unchanged. This includes all four new
identity/projection/schema tests and the finite 4,096-source participant schema,
codec, hash/drop and removed-recursive-layout regressions.
The same three public tests also pass in a normal-dependency consumer with
governance actually disabled: all 48 common frames, the block-send projection,
22 available schema variants and rejection of reserved Governance frames.
Locked/offline Cargo passes with all 5,624 selected inputs unchanged. The
consumer's unused vendor-patch entries caused nondeterministic lockfile checks;
removing only those unused entries preserves all 317 package records, feature
sets and dependency edges. Root Cargo.toml/Cargo.lock and optimization settings
remain unchanged. Three repeated locked metadata checks also pass.

The two remaining companion source guards now enforce current registrations,
retained codec assertions, emitter ownership and error propagation. All original
historical guard bytes and fingerprints remain in the dated Norito evidence
record; unrelated lockfile/manifest changes and module ordering no longer stand
in for current behavior. Their 14 applied tests pass, including positive-baseline
mutation checks. Combined with current codec and source-budget tests, all 73
tests and 67 subtests pass. This resolves the earlier 11 stale companion failures;
it does not qualify the outstanding source-size or complete release budgets.

Bare serialization now has one object-safe `SerializePayload` owner, separate
from the typed frame contract. Bare `Encode`, containers and borrowed adapters
use that owner; framed writers explicitly require `NoritoSerialize`. The derive
supports payload-only generic fields and rejects frame-schema attributes and
unions. Tests verify actual bytes, checked lengths, writer failures and rejection
of payload-only values at the typed frame boundary. Existing frame identities,
layout flags and checksums remain unchanged by this trait split; the atomic
`NoritoSchema` cutover is still pending.

The frozen candidate passes **1,663 codec/derive/primitives tests** (three ignored)
and strict library Clippy, **3,599 default/HTTP model tests** (six ignored), and
all **nine governance-disabled normal-consumer tests**, with `RUST_MIN_STACK`
unset. All 19,170 recorded inputs remain unchanged during those runs. The model
suite includes the maximum 4,096-source finite participant control and rejection
of the removed recursive layout. DataEventFilter now reserves Governance tag 23
and retains GameSession tag 24 in both feature selections; six tests exercise
its codec, schema and HTTP subscription aggregate. Root Cargo manifests, lockfile
and optimization settings are unchanged.

The exact reviewed integration preserves inherited dirty work and excludes
three unrelated live edits. Subsequent downstream trait-bound/import corrections
and eight whitespace-only lines are recorded separately from those test inputs.
Selected downstream target checks pass; complete workspace, source-size and
same-source release qualification remain open. The model transcript digest is
`0d813689de0b81ac9ab75df05310ea92e3203824d142e35fe607f0fe4aced8f4`;
the codec transcript digest is
`a57030ead9bc3ae55847ca01a24a81e441495049deb88aae9077257e117bb427`.

Codec and authorization-fixture modules retain every production nominal
declaration and codec body in the same semantic owner. The outbox and Governance
DAG parent files now contain 10,255 and 7,401 lines. Their existing exceptions ratchet downward; both remain
above the ultimate 5,000-line production limit. All 77 applied source-budget and
provider-ingest contract tests pass. The repository still reports 240 findings
with the same 174 exceptions; no complete module-budget pass is claimed.

The all-target build also exposed an existing Torii test source hidden by the
global `security_*` scratch ignore. The rule is now rooted at `/security_*`, so
normal source selection includes the original test bytes. The new regression
uses the actual ignore policy and proves an oversized nested security test is
still measured and rejected. All 387 Torii Rust files are now present in both
the working tree and candidate inventories; an independent literal-path audit
finds no remaining source omission. All selected SDK, storage client, Core, Torii
and P2P targets now compile; pre-existing warnings still prevent a strict-lint
qualification claim.

The complete default SDK library passes **716 tests**, with zero failures or
ignored cases, in 270.64 seconds on the default stack. All 19,174 recorded
candidate inputs remain unchanged, including the formerly stalled exact-release
Musubi transaction-boundary case. The SDK transcript digest is
`a14809f71119f49d5f1f538a6512b36cd1378d364626a8a96ffd69f4137da2ac`.
All 25 transaction-forwarder tests also pass. The new framing regression proves
valid baselines before rejecting truncated, trailing, wrong-header and
oversized signed envelopes; every rejection preserves the complete signing
claim, which subsequently accepts the original valid bytes.

The outbox's 74 tests pass after replacing its truncated CID fixture with a
canonical CID constructed from the same digest. A separate network-fixture
review replaces nominally invalid values that `Hash::prehashed` normalized into
valid identities. Raw payload carriers preserve custom-type field classification
and prove exact valid bytes before clearing the marker. Journal tests now prove
the decoder rejects that marker, independently of the retained approval digest.
Clock and outbox tests cover every supported advertised layout. The coordinator
fixture uses a canonical test-owned temporary directory so macOS's `/var` alias
does not prevent secure storage initialization. These test-only followups have
separate source manifests. All 80 selected outbox/network-fixture tests pass
with zero failures or ignored cases and all 19,174 final inputs unchanged.
Full release qualification remains open.

TODO: Complete remaining model feature selections, SDK release and mandatory consensus qualification,
and migrate remaining historical source assertions to current release contracts.
Source/artifact hashes and individual logs remain under the ignored
`target/architecture-redesign/owned-storage-identity/` directory.

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
| Source inventory | Current catalog records 665 canonical Torii routes after the merge; all 8 inventory and 31 dependency-budget tests pass | Request/response and SDK consumer mapping; sealed, comparable build-memory baseline |
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

## Foundation identity follow-up (2026-09-07)

Seven FFI-owned address types and the Hash, BFV Goldilocks digest and SM3
scalar owners now have explicit nominal declarations. Actual compiler names,
both codec-direction hashes, complete frames and bare bytes were captured
before annotation. Address fixtures cover 56 root/container records and 560
advertised-layout records; crypto fixtures cover all seven forms per scalar
under default, minimal, JSON-only and SM-only feature selections. Their
captured files remain immutable. Permanent tests compare declared identity
against those observations, allowing later physical relocation.

The frozen candidate passes 303 default, 265 minimal and 303 FFI primitive
library tests, plus the scalar fixture test in all four crypto selections.
JSON-only address tests now declare their feature requirement. The owned
inline-string rejection path retains the original allocation; its existing
regression passes. A dedicated crypto test target avoids pulling unrelated
PQC/rand/streaming test requirements into minimal-feature qualification.

Cargo formatting and the codec guard pass. Strict Clippy and repository
source-size qualification remain failing: existing crypto/PQ diagnostics,
primitive test-helper diagnostics and 240 source-budget findings remain.
No exceptions or limits were expanded. This is identity coverage evidence,
not the atomic codec cutover, physical model extraction or release approval.

Seventeen unchanged consensus-key, FX settlement and privacy protocol owners
also declare their independently captured identities. All three source-adjacent
owner tests pass on the rebuilt current model artifact. Complete item bodies
and preceding attributes match the controlled compiler capture; no field,
variant tag, production method or signing preimage changes in this batch.
The subsequent private-settlement batch declares another 58 captured owners.
Its bounded confidential staging and validation errors have private module
owners, with one canonical public error path. Governance, AXT and private
settlement retain all 114 existing tests in separate test modules. Their
production roots now contain 3,511, 2,911 and 4,992 lines respectively; every
extracted test file remains below 3,000 lines. No wire declaration moved,
field/tag changed, assertion disappeared or source-size exception expanded.

The fresh AMX-only run passes all 32 tests on the default stack, with all
19,262 recorded candidate inputs unchanged. It includes the maximum 4,096-source
schema/encode/decode/hash/drop regression and rejection of the removed recursive
settlement layout. After the seventeen declarations, those two regressions and
the three identity tests pass again. The complete refreshed default/HTTP model
library passes all 3,644 tests with zero failures and six fixture generators
ignored, on the default stack (591.39 seconds). All 19,265 recorded inputs
remain unchanged. The tested artifact and its hash are retained in the scoped
capture directory. Prior full-library results above belong to their stated
artifacts; other feature graphs, workspace and release gates remain unqualified.

Untracked review/capture/log manifests are retained under
`target/architecture-redesign/owned-storage-identity/model-identity-closure/`.
The 2026-09-08 follow-up replaces the model's test-only IVM dependency with
its owning `ivm_abi` crate. Cargo.lock changes that one edge without changing
package versions. The feature-resolved model-test graph drops from 359 to
325 packages and contains no IVM, node configuration or telemetry runtime.
The new root-dev boundary retains normal/build traversal and all forbidden
layers; all 16 real selections and 45 guard tests pass, including Cargo
fixtures for root-dev isolation and transitive leaks. This test graph still
enables governance and JSON through the ABI owner; it does not qualify a
governance-disabled build.

The exact manifest ratchet now accounts for five added and three removed
edges since its reviewed baseline. All 106 historical manifests reproduce
the original fingerprint and all 60 limits. Direct owning-code uses justify
the committed Core/FASTPQ/Kaigi/Python additions; removed primitive and FEC
dependencies reduce external package counts. The refreshed limits equal the
observations with no spare allowance; ownership denials and source-file
budgets remain unchanged. This graph review does not qualify the recorded
non-edge runtime, feature or test-profile changes.

NetworkId, EscrowId and ProposalId also declare their independently observed
manual identities. Their 21 immutable root/container/hash/signature records
preserve both codec hashes, complete frames, bare bytes and JSON. Integration
testing exposed an AXT handle fixture predating the committed ABI-v1
EXECUTION_SUMMARY signature update. Its old and current hashes match their
respective IVM ABI goldens exactly. Only the handle's 32-byte ABI hash and
eight-byte frame checksum change; the other four AXT fixtures remain exact.
The test now checks ABI binding explicitly before complete byte equality.
The downstream IVM `core_host_enforces_fixture_snapshot_fields` test also
passes on the same frozen source and default stack; its executable and source
hashes are retained separately. No additional consumer expectation changed.

The final default/HTTP selection passes 199 model-library tests, six AXT
integration tests and six pointer/generated-schema integration tests, with
one fixture printer ignored. All 19,273 frozen inputs remain unchanged and
the three executable artifacts are retained. Formatting, the codec guard,
dependency boundaries and all five manifest-budget scopes pass. All changed
source files satisfy their production/test limits; the frozen repository
still has 237 other source-budget findings. Strict Clippy, the atomic codec
cutover, physical extraction and complete workspace/native/release gates
remain open. These focused results do not replace the separately scoped
3,644-test complete-library evidence above.

## Bare decoding ownership (2026-09-08)

Bare `Decode`, canonical field comparison, owned containers, generated fields
and erased query payloads now depend on `SerializePayload`. Actual framed
ABI, network, storage and conversion entry points explicitly require
`NoritoSerialize`. No wire identity, field order or ABI version changes.
Option decoding uses the shared canonical child decoder, preserving declared
layout flags and alignment while enforcing child byte/depth limits. Tuple
and Result slice decoders reject unconsumed child bytes with `LengthMismatch`
in every profile. Payload-only records, enums, containers and registered
queries have runtime tests without a frame serializer. The 13 model consumers
of `ParseError` now use its owning constructor/accessor; strings and validation
behavior remain unchanged.

The frozen default/HTTP model and ABI run passes 4,110 tests in 11 suites,
with zero failures and 13 ignored cases. All 32 Native AMX tests pass on the
default stack. All 19,275 source inputs remain unchanged; eight test binaries
and the exact log are retained with `bare-decode-model-checkpoint.json` in the
scoped capture directory above. The preceding complete codec/derive/primitives
run passes 1,684 tests (three ignored); six refined child-consumption tests and
267 minimal-feature primitive tests pass separately. Norito/derive library
Clippy and four retired-codec guards pass; the codec-contract Python suite
passes nine tests and 31 subtests. Source-file checks still report 237 findings.

The 42-path integration preserves every recorded root preimage, including
13 already-integrated ParseError consumers. These results do not qualify the
remaining Core/Torii callers, the atomic identity transition, physical model
moves, pinned memory reduction or complete workspace/native/release matrices.
The subsequent validation-owner batch is scoped separately below.

## Validated binary owner decoding (2026-09-08)

`#[norito(validate = "path")]` now invokes a fallible constructor on the actual
reconstructed owner exactly once. Multisig policy/member binary decoders use
this contract; their private carriers serve JSON only. Invalid order, duplicate
members and zero weights retain typed rejection and successful retry coverage.
The shared schema parser accepts the attribute without invoking its function
or changing schema identity. Generic slice derives now compose one valid
parameter list and choose a lifetime absent from nested higher-ranked bounds.

Both Result slice branches enforce nesting limits and restore the guard after
success or rejection. Regression tests first reproduced acceptance at a zero
depth limit. Unit payload size calculation now matches the existing packed
zero-field offset table; a separate failing regression preserved the prior
zero-versus-eight-byte mismatch. Canonical frame checks still reject corrupt
unit metadata and trailing bytes. Valid serialized bytes remain unchanged.

The complete Norito/derive/primitive/schema suites pass **1,726 tests**, with
zero failures and three ignored cases; all nine schema UI fixtures pass. The
focused model suite passes **52 tests**, including all 32 Native AMX cases and
the bounded default-stack regression. Those runs have no drift across 19,283
recorded candidate inputs. One subsequent integration-test import formatting
correction has a separate source record; library source is identical.
Strict library Clippy passes for Norito and both changed derive crates, and
all 18 changed Rust files pass formatting. The 27-path integration preserves
all root preimages and retains its patch and checkpoints under the scoped
capture directory's `validated-decode-integration/` record.
The integrated working tree also passes all four retired-codec guards and
28 codec/release-contract tests with 31 subtests.
The candidate's Core/Torii production-library graph subsequently compiles,
including the required IVM and P2P dependencies, with all 19,283 input hashes
unchanged. Its first check exposed a production `Encode` trait import gated
behind tests; the already-correct working-tree import was reconciled into the
candidate and the failed log retained. Seven Core warnings remain, so this is
compilation evidence rather than strict workspace Clippy or runtime evidence.
Kagami also compiles in its own binary feature graph on the same unchanged
inputs, retaining two CLI warnings. Its codec caller and the production node
callers now have compiler evidence; runtime/feature matrices remain separate.

## Mandatory aggregate protocol JSON (2026-09-08)

The earlier normal model check with `--no-default-features` exposed mandatory
JSON consumers behind contradictory model feature gates. Its failed log remains
retained. JSON is required by `CustomParameter.payload`, account admission,
consensus roster parameters and retained Kaigi state. The isolated candidate
now owns that protocol surface unconditionally through six normal dependencies:
`iroha_crypto`, `iroha_primitives`, `iroha_version`, `norito`, `norito_derive`
and `mv`. Norito's existing `base-codec` selection includes JSON. The obsolete
aggregate `json` feature and active manifest, tooling, generator and caller
selections are removed; lower-crate JSON features remain independently owned.
Governance and HTTP selections remain independent. Generated model and builder
JSON implementations resolve their traits from Norito, preserving defaults,
unknown/duplicate-field rejection and bounded output.

Rustfmt after removing conditional attributes exposed oversized implementation
owners. Validation/constructor impls and private helpers now live in cohesive
private modules; named wire records, schema/codec declarations, local hash
preimage types and serializer bodies stay in their original owner namespaces.
The moves preserve exact implementation bodies and all existing tests.

| Parent owner | Parent lines | New implementation modules and lines |
| --- | ---: | --- |
| `musubi.rs` | 4,663 | `archive_validation.rs` 773; `publication_validation.rs` 448 |
| `sorafs/moderation.rs` | 4,657 | `trust_validation.rs` 502 |
| `block/consensus_v2.rs` | 4,860 | `messages.rs` 672 |
| `nexus/private_settlement.rs` | 4,941 | `settlement_validation.rs` 442 |
| `privacy/protocol.rs` | 4,651 | `privacy/policy.rs` 377 |

The Musubi generator input inventory now includes both validation modules;
reviewed consensus source closure includes its compiled `messages.rs` owner.
Cargo.lock and captured/generated fixture bytes are unchanged. No source-size
exception expands; that checkpoint retains 237 other source-size findings.

Before decomposition, the candidate's full model library passes **3,648 tests**
and its existing integration suites pass. The same command exposes two
derive-test visibility expectations inconsistent with `transparent_api`; those
failed logs are retained and the expectations now follow the selected policy.
After decomposition, **419 focused model tests pass**, with three ignored cases.
The subsequent derive/ABI command passes **217 tests**: 29 derive unit tests,
22 integration tests and 166 `ivm_abi` unit tests, with zero failures or ignored
cases. The integration suites include eleven distinct UI fixtures: the positive
protocol-JSON consumer and ten compile-fail cases; six negative fixtures run
again in their focused harnesses. These UI checks are included in the 22-test
integration count. The consumer fixture retains `schema-structural` check-cfg
warnings, so this is not strict-lint qualification.

Both post-decomposition Rust runs use the default stack and preserve all 19,290
recorded candidate inputs. The feature-hygiene, Musubi input and dependency-budget
Python suites pass **89 controls** across two runs: 87 selected cases, then the
two previously deselected Cargo dependency controls. Separately, all **16
feature-resolved dependency boundaries** pass. These candidate observations do
not imply a full post-decomposition model run or whole-workspace qualification.

The dependency-budget refresh is limited to making the existing model→`mv` and
model→`norito_derive` edges required. Reversing only those classifications
reproduces every previous metric; all declared metrics remain unchanged.
Required model/SDK closures each add one local/workspace package (`mv`), one
external package (`concread`), four edges and one external edge. Model required
edges become 198; SDK edges become 272. CLI, daemon and workspace required edge
counts each increase by two, to 650, 671 and 1,542. The refreshed fingerprint
binds the exact 106 current manifests; denials, layers and source-file limits
are unchanged. This graph accounting does not qualify runtime or memory use.

The final `iroha_data_model --lib --no-default-features` check passes with the
unchanged lock; one existing governance `validate_capacity_intent` dead-code
warning remains. All three final source guards pass, including the corrected
canonical include-manifest digest and precise corrupted-pin/missing-member
negatives. These checks retain all 19,290 final candidate inputs unchanged in
their separate terminal checkpoint. The standalone restricted-visibility selection also passes all 29 derive unit
tests, and strict Clippy passes for the derive library without default features.
Both terminal checkpoints preserve the same final source inputs. The 292
integrated source paths exactly match this qualified candidate; unrelated root
edits are preserved. Minimal compilation and these focused checks do not qualify
runtime behavior or the full feature matrix.
Atomic identity cutover, physical model extraction, remaining feature/target
qualification, measured memory reduction, native/device execution and complete
workspace/release qualification remain outstanding.

## Completion gate

TODO: Complete each pending implementation and acceptance item above. In
addition to focused tests, qualify one consistent candidate with workspace
build/tests, the mandatory consensus harnesses, valid release-feature checks,
formatting, strict Clippy, the codec guard, and four-validator integration tests.
Rebuild native artifacts from that candidate; unavailable device qualification
must remain explicitly unverified.

## Streaming wire identity closure (2026-09-08)

The integrated batch adds explicit captured identities for all 84 current
production streaming binary owners: 70 root records and 14 records under
`streaming::codec`. Pre-declaration compiler captures bind every nominal name
and both codec-direction hashes. Immutable fixtures retain 396 complete/bare
frame records for 132 populated values, each in root, Option and two-element
Vec forms, including every root enum variant. The permanent tests compare
declared identities and frame hashes against those captures and preserve exact
header flags, required zero alignment padding, payload length and decoded
re-encoding. No frame or codec implementation is changed by the declarations.
An initial capture-helper assertion incorrectly omitted alignment padding; its
failed checkpoint is retained, and the corrected helper checks the codec-owned
padding calculation before recording the successful pre-declaration bytes.

The existing public `codec` module becomes a canonical file, and its large
private test module becomes `codec/tests.rs`. Both retain their original Rust
namespaces, private access and all existing assertions. All 109 direct streaming
tests remain, including the 69 codec tests and 59 moved `codec::tests` cases.

| Owner | Lines | Unchanged limit |
| --- | ---: | ---: |
| `crates/norito/src/streaming.rs` | 3,757 | 5,000 |
| `crates/norito/src/streaming/codec.rs` | 4,383 | 5,000 |
| `crates/norito/src/streaming/codec/tests.rs` | 1,722 | 3,000 |

The source-budget change removes only the obsolete 10,218-line streaming
exception. Existing exceptions decrease from 174 to 173; the corrected report
retains 236 source-size findings across all measured languages. The global
production/test limits remain 5,000/3,000; no other exception expands. Earlier
237-finding checkpoints remain historical scoped measurements, not Rust-only
counts.

Final default Norito tests pass **1,308 cases**, and the separate
`--no-default-features --features base-codec --lib` run passes **456 tests**;
each selection retains one existing ignored case. Strict
`cargo clippy -p norito --tests --no-deps --locked --offline -- -D warnings`
also passes after two unnecessary Copy clones are removed from test fixtures.
All three terminal checkpoints preserve the same **19,297 final candidate
inputs**, with zero drift and no stack override. The integrated code and budget
paths match the qualified candidate. These are scoped Norito test and lint
results; Clippy excludes dependencies and unselected features.

Retained evidence is under
`target/architecture-redesign/owned-storage-identity/model-identity-closure/streaming-identity/`:
`final-qualified-node-checkpoint.json`, `final-qualified-base-checkpoint.json`
and `final-qualified-tests-clippy-checkpoint.json` bind
`final-candidate-inputs.json`. The separate pre-declaration identity/value
captures, earlier failed helper checks and corrected source-budget report
remain retained. The same source seal also passes a fresh **nine-case settlement
selection**: eight participant cases plus rejection of the removed recursive
layout, with zero failures/ignored tests and no stack override. The populated
4,096-source case exercises schema construction, codec, hash and drop.
`final-qualified-stack-checkpoint.json` records all 19,297 unchanged inputs and
exact agreement of the three live model/hash owner files with the candidate.

### Integrated structural-feature correction

The subsequent structural check exposes an existing enum-direction defect:
derived serialization selects the structural schema hash, while default enum
deserialization retains the nominal hash. The unfixed exact regression
`schema_hash::derived_enum_schema_agrees_across_all_frame_readers` fails at the
first directional equality assertion; its retained negative checkpoint records
exit 101 with no source drift. The integrated fix emits the same existing
hash-selection body for enum decoding as for encoding. Explicit `schema_name`
projections and default nominal identities remain unchanged. The regression
exercises high-level, reader, core archived/slice and ArchiveView decoding, and
rejects altered frame identities through each checked entry point.

The fixture correction retains the immutable 84-identity and 396-frame files
byte-for-byte. The streaming tests compare declared nominal identities against
captures independently of the active hash mode. Default mode retains complete
frame comparisons; the structural branch compares every case, nominal name,
bare payload, advertised flag and alignment, alongside active directional/header
agreement, roundtrips and wrong-schema/truncation/trailing-byte negatives.
At this integrated checkpoint, these guards executed only under default/base;
the retained structural compile failure prevented their structural execution.
The later integrated [reconstruction checkpoint](#payload-reconstruction-checkpoint-2026-09-08)
executes both guards successfully; its remaining structural failure is recorded below.
The rANS checksum literal is the exact captured 32-byte field, with the original
nominal checksum calculation asserted in default mode. These synthetic wire
records do not qualify signed-table authorization.

Isolated derive lint exposed unused private `FieldAttr`/`ContainerAttr` Debug
derives relying on an undeclared Syn feature. Removing those derives eliminates
the incidental requirement without adding dependencies or features. Eleven test
error-extraction sites use explicit `Err` matching; every original diagnostic
assertion and the five-case validation loop remain intact. Parsing, codec
generation and supported attributes are unchanged by this cleanup.

The final combined default run passes **1,380 tests**: **1,309 Norito**, **57
derive-library** and **14 strict JSON** cases, with one existing ignored case.
The separate base-codec library run passes **456 tests**, with one ignored.
Strict Norito `--tests --no-deps` and isolated derive-library `--lib --no-deps`
Clippy both pass with warnings denied. The final structural `schema_hash::`
integration selection passes **six tests**, with zero failures or ignored cases
under `--no-default-features --features base-codec,schema-structural`.

All final results bind **19,297 unchanged inputs** in
`qualified-correction-inputs.json`, without a stack override. The checkpoints
are `qualified-correction-default-checkpoint.json`,
`qualified-correction-base-checkpoint.json`,
`qualified-correction-tests-clippy-checkpoint.json`,
`qualified-correction-derive-clippy-checkpoint.json` and
`qualified-correction-structural-schema-checkpoint.json` in the retained evidence
directory above. All seven correction paths are integrated with exact afterimage
verification in `structural-correction/final-integration/root-integration.json`;
the fourteen total code/budget paths match the qualified candidate. The initial
1,308-test declaration checkpoint remains separate earlier evidence.

The final Native AMX selection passes **32 tests**, with zero failures or
ignored cases, in 5.60 seconds on the default stack. It exercises the populated
maximum 4,096-source settlement through schema, codec, hash and drop, and
rejects the removed recursive layout. `qualified-correction-native-amx-checkpoint.json`
binds all 19,297 unchanged final inputs in `qualified-correction-inputs.json`
and confirms exact live model/hash owner correspondence. This final run is
distinct from the earlier nine-case selection on `final-candidate-inputs.json`;
neither uses a stack-size override.

The retained full structural group diagnostic failed: **109 pass and ten fail**,
comprising eight compression assumptions with compression disabled and two
pre-existing nominal streaming-ticket golden checks. At that checkpoint the
broader structural library selection failed compilation: **24 errors across 15
test owners**. Ordinary framed fixtures need correct schema ownership, while the
deliberately payload-only generic decoder regressions expose coupling that must
be closed by the planned identity separation. Their schema-free leaves and
assertions remain intact; no tests or production bounds are weakened to hide
the failure. Evidence remains in `structural-before-correction.log`,
`structural-corrected-schema-checkpoint.json` and
`structural-enum-negative-runtime-checkpoint.json`. Those six passing structural
integration cases did not qualify the failed selections or the streaming
guards that had not executed at that checkpoint.

The final source-budget check still exits 1 with 236 findings and 173 exceptions;
none of the seven correction paths violates its limit. Atomic identity cutover,
physical crate extraction, the required measured 25% model-memory reduction and
unchanged-unit budgets, native/workspace and other feature qualification, and
the complete first-release goal remain open.

## Payload reconstruction checkpoint (2026-09-08)

All **166 changed source paths (163 Rust and three Python) are integrated**
from the isolated `abi-candidate` after exact preimage checks; every integrated
afterimage matches the qualified candidate. `DeserializePayload` is the single
reconstruction owner. Typed `NoritoDeserialize` inherits it and retains the
existing frame-hash contract; typed derives emit both contracts,
while payload-only derives reject frame schema attributes. Bare `Decode`, field
and container reconstruction use payload bounds, and actual framed consumers
state their typed contracts explicitly. The final `NoritoSchema` supertrait and
removal of independent codec hashes remain the pending atomic identity cutover.

The migration preserves reconstruction bodies, checked-owner validation, layout
and resource enforcement, and all 41 retained manual hash overrides. Deliberately
schema-free test leaves lose their panicking frame implementations; private
query-page wire helpers retain payload contracts, while public `QueryPageV1`
retains its frame contract and checked reconstruction. Opaque authorization
source guards now also reject payload codec traits. No compatibility aliases,
stack increases or decoder fallback paths are introduced.

| Qualified selection | Terminal result |
| --- | --- |
| Default Norito/derive tests | 1,382 passed, one ignored (1,310 Norito, 58 derive, 14 strict JSON) |
| Base-codec library | 457 passed, one ignored |
| Codec documentation tests | 11 passed, one ignored |
| Strict Clippy | Norito tests and isolated derive library pass, dependencies excluded |
| Changed Rust formatting | All 163 changed Rust files pass the final scoped formatting check |
| Primitive library | 305 passed |
| Complete model library | 3,650 passed, one new registration-fixture failure, six ignored |
| Corrected registration selection | All three pass in a subsequent run |
| Native AMX | All 32 pass on the default stack, including maximum 4,096-source schema/codec/hash/drop and removed-layout rejection |
| ABI/artifact admission libraries | 167 ABI and 15 artifact tests pass |
| Production compilation | All 15 selected libraries and the shipping CLI/Kagami binaries pass |
| Consumer library-test compilation | Revised seven-package `--lib --no-run` selection passes; no runtime test count |
| CLI/Kagami test compilation | `--bins --no-run` passes in 874.132 seconds; no runtime test count |
| Nexus/streaming integration harness | Entire harness compiles; both selected `global_commit::` tests pass, zero failed or ignored (301 filtered out) |
| Core payload runtime selection | 28 passed, zero failed or ignored |
| P2P library runtime | 583 passed, zero failed or ignored |
| Config library runtime | 632 passed, zero failed or ignored, with explicit workspace context |
| Config-base library runtime | 34 passed, zero failed or ignored |
| Telemetry library runtime | 93 passed, zero failed or ignored, with explicit workspace context |
| Torii-shared library runtime | 267 passed, zero failed or ignored |
| Standalone status-wire integration | One passed, zero failed or ignored |
| Torii payload runtime selection | Corrected run: 14 passed, zero failed or ignored; earlier nine-pass/five-failure fixture result retained |
| Capture/identity and opaque/provider source controls | 87 Python tests and 91 subtests pass, repeated successfully on the integrated root |
| Structural library | 467 passed, one signed rANS checksum failure, one ignored |

The model failure correctly rejects cache validation on a resultless genesis
proposal. The fixture now attaches one legitimate successful result through the
checked owner, validates both Merkle caches, and preserves its original proposal
bytes, consensus hash, signature and complete frame-family assertions. The
subsequent three-test pass does not change the retained failed full-run result.
Both structural streaming guards now execute and pass; their earlier compile
failures remain retained. The sole current structural failure uses the unchanged
signed rANS fixture and the feature-selected active schema hash. Its bytes are
not refreshed to manufacture a pass. The whole Parliament source checker still
fails at four existing `world.rs` anchors; focused payload-codec controls do not
qualify that broader gate.

The production check passes for all 15 selected libraries, including SDK, Core,
Torii, P2P, IVM and storage consumers. The subsequent production/CLI check also
passes for the shipping `iroha` and `kagami` binaries. Both commands retain
19,300 unchanged inputs in their separate `production-consumer-check-checkpoint.json`
and `production-and-cli-check-checkpoint.json` seals. Seven Core dead-code,
two Kagami and one CLI helper warnings remain. The two additional CLI unused
payload imports reported by the latter command were removed before the later
passing CLI/Kagami test build. That test build also retains two existing unused
`ErrReport` expressions in the CLI's soracloud tests. These compile checks do
not claim strict all-target lint or test execution.

The revised seven-package Core/P2P/Torii/shared/config/telemetry library-test
build passes `--lib --no-run` in 309.531 seconds. Its
`consumer-library-tests-build-revised-checkpoint.json` binds 19,300 unchanged
source inputs, with no `RUST_MIN_STACK` override. This compiles the test owners;
it assigns no runtime passing count. The initial failed build remains retained:
its four errors came from the old `BudgetedField` serializer owner and the
hybrid-recovery fixture's borrow extending past a consuming-key move. Both
corrections already existed in live root and were reconciled byte for byte into
the candidate, preserving the proof assertions without cloning the recovered
key. Two unused decoder-trait test imports were also removed.

The subsequent `cli-kagami-test-build-checkpoint.json` records a passing
`--bins --no-run` selection in 874.132 seconds, retaining 19,300 unchanged inputs
and no stack override. This qualifies compilation of those test binaries only.
The six earlier consumer runtime selections total 1,637 passing cases; the
standalone status-wire integration contributes one separate passing case. Every
listed runtime checkpoint retains 19,300 inputs with no drift or stack override.
The final config and telemetry results use
`iroha_config-library-runtime-explicit-workspace-checkpoint.json` and
`iroha_telemetry-library-runtime-explicit-workspace-checkpoint.json`; their earlier
launcher-context failures remain retained and are not rewritten as passing runs.

Torii's five failures all stop at the shared test commitment constructor with
`InvalidChunkProfile(UnknownProfile { profile_id: 171 })`. Its synthetic inline
profile ID conflicts with the current canonical inline profile; the production
registry correctly rejects it. A fixture-only correction reuses the actual
request-profile constructor while preserving chunk geometry and every existing
assertion. The fixture correction was qualified in C before integration. The
revised selection passes all 14 actual Torii cases, zero failed or ignored, in
140.980 seconds. Its six other library
harnesses select zero tests and contribute no additional runtime passes.
`torii-payload-runtime-revised-checkpoint.json` retains 19,300 unchanged inputs
without a stack override. The original nine-pass/five-failure result in
`torii-payload-runtime-checkpoint.json` is not rewritten.

The first Nexus/streaming `--no-run` build retains exit 101 after 1,122.580
seconds, with one E0599 in `integration_tests/tests/nexus/global_commit.rs`:
`LaneBlockCommitment::try_deserialize` still imported the former typed method
owner. Replacing that import with DeserializePayload restores its existing
payload reconstruction call. The unrelated unused SerializePayload import in
`data_model/samples/mint_rose_trigger_data_model/src/lib.rs` is also removed.
`nexus-commitment-fixture-runtime-revised-checkpoint.json` then records successful
compilation of the entire integration harness and both actual `global_commit::`
tests passing in 31.816 seconds, zero failed or ignored, 301 filtered out. Both
attempts retain their own 19,300 unchanged-input seals without a stack override.
This qualifies the two commitment fixtures, not runtime execution of the full
Nexus/streaming integration suite; the initial failed build remains retained.

The scoped runtime and compilation selections above are terminal. The complete
166-path integration is recorded in `final-source-review-v3-integration.json`,
with every preimage checked and every afterimage verified. These integrated
paths match the qualified candidate; independent live changes remain outside
that runtime qualification.

The separate `unstaged-codec-review/` preserves the 59 unowned live source deltas.
A bounded review of their 36 Rust files and direct resolution context found no
additional codec migration. Those independent live changes were not synchronized
into the candidate; this source review and the candidate's compilation results
do not qualify the full current live root.

Each terminal run retains its own source seal: the early codec/primitive/AMX
runs cover 19,299 unchanged inputs; the model, corrected registration and
ABI/artifact runs cover 19,300. Exact checkpoints and failed logs are under
`target/architecture-redesign/owned-storage-identity/model-identity-closure/payload-reconstruction/`.
`final-source-review-v2/` retains the earlier 164-path snapshot, matching live
preimages, whitespace-strict replay and per-file hashes. Subsequent existing-root
fixture reconciliations are recorded separately in `reconciled-source-updates/`;
the revised test-build seal records the two test-import cleanups. With the Nexus
and sample import corrections, the stage now changes 166 paths (163 Rust and
three Python). `final-source-review-v3/` now retains the exact 166-path delta,
matching live preimages and whitespace-strict replay; its patch SHA-256 is
`79ca3524d977d1c3090a60d5e386da7647b4f3b4087385cac532d5cc960bb69c`.
The earlier frozen review bundles remain historical snapshots. The restored
synthetic compiler probe is excluded. Cargo.lock and both streaming fixture
files remain exact.

The final v3 legacy-codec guard passes. Its authoritative size check still fails
with
**236 source-size findings and 173 unchanged exceptions** over 10,763 measured
files; its findings and exceptions are identical to v2. No file newly fails its
budget, but five existing violations grow:
Core gossiper and privacy state by two lines each, crypto FHE by one, model
KAGEMUSHA by three and genesis by one. No limit or exception expands.

After integration, the merged-root source controls also pass all 87 Python tests
and 91 subtests, and the legacy-codec guard passes. That root's separate size
check still fails with 236 findings and 173 exceptions over 10,765 files: the
two additional files are the previously audited live-only Core modules. Finding
paths match C, while 11 diagnostic messages have different lengths from the
retained concurrent work. `live-integration-checks/` records these root results
separately from the 166-path candidate's source-budget proof and runtime seals.
Neither those static checks nor C's scoped passes qualify physical model
extraction, the required 25% measured memory reduction, native/device execution,
the full workspace or the first release.
