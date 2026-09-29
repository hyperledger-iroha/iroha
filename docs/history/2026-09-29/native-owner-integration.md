# Native validator integration — September 29

Only `/Users/takemiyamakoto/soramitsudev/iroha`, branch `optimizations`, is used.
No commit, index reset, live deployment or live value transfer was performed.
The active goal remains complete first-release validator staking with canonical
network XOR and no compatibility formats or competing consensus implementations.

## Applied source

Checkpoint 07 (500 paths), delta 01 (57 paths), module ownership repairs, the
18-path State legacy-family retirement, 391-path old Sumeragi graph retirement,
and 15-path native carrier-reader cut are applied. Per-stage baseline hashes,
patch hashes and application receipts are under `target/staking-validation`.
Application rechecks originals, merges independent drift, and verifies that the
Git index is unchanged. These source counts are not test or readiness evidence.

The offchain carrier reader authenticates actual native prefix frames from signed
genesis and State's captured hash cut, charges source work and bytes, and requires
H2 for genesis output. Torii pipeline, service readers, Push and Explorer use that
State authority. Actual signed-genesis/native executor fixtures replace synthetic
finality in the affected Core/Push controls; runtime execution is still pending.

## Verification and open gates

The latest completed command is `scripts/cargo_fast.sh --stable-local-metadata
--incremental -- check -p iroha_core --lib --locked` (log
`target/staking-validation/native-integrated-core-check-05.log`). It reached Core
and failed with 189 errors and 27 warnings. Most errors are unresolved callers of
retired owners; an unconditional native archive test module was subsequently
removed from production compilation while preserving its `cfg(test)` declaration.
Earlier metadata-include, config-derive and SCCP public-lint blockers are repaired.
No current Core pass, focused custody rerun, workspace pass or release is claimed.

The codec-retirement guard passed before these latest integrations. Formatting
checks found existing diffs; the carrier cut's 15 Rust files parsed under rustfmt.
The native evidence Python suite previously passed 153 controls including 65,536
rows. These receipts do not replace a final unchanged-candidate run. SDK native
binding/parity and genuine generated captures remain unqualified.

Remaining work includes Queue/gossip cleanup, Kura physical and finality owner
retirement, native evidence/slashing, deterministic State-owned historical proof,
complete resource funding, S6 cross-dataspace atomic execution and operator/SDK
closure. Real 4→7→4 and noncommittee-pool election, missing target custody,
withheld fresh incumbent keys, Parliament pulses, authenticated loss/replay,
all-seat restart, slashing, reward claims and full withdrawal must run alongside
formal, DA, liveness, workspace and SDK gates on one unchanged candidate.

The [superseded root paragraphs](superseded-native-owner-status.json) preserve
exact original text and SHA-256 digests. Their historical evidence and obsolete
Decision/QueuePlan prescriptions are not current implementation authority.

## Subsequent integrated cut

The later cut applies Queue owner retirement and its eight shared routing helpers,
canonical gossip and proxy/ingress (no QueuePlan certificates), direct queue config,
Python/JS native status, the pure native evidence verifier, and canonical block
output metadata retirement. The old lane settlement field and 95 setter arguments
are gone; actual scoped gas transfers, payer bounds and sponsor debits remain.
Retired `lane_relay_burn` is rejected before native execution. The unused old lane
drain signing owner is removed; actual native key records still own signing safety.

Kura creates one permanent native gate before driver startup. Storage poison closes
it before the poison latch becomes visible. Global and lane startup acquire it
before credential or file mutations; Core steps, worker operations and physical
sends enter it separately. Operations already admitted retain their original owner
until completion, while later operations and buffered sends are refused. Stop
cleanup releases retained ingress under the same admission lock. There is no late
binding that can reopen a poisoned storage owner.

The funded scalar native execution tip and its undo live outside World to avoid a
result self-reference. Original Worker or signed-genesis capabilities create it;
its journal is sealed and published with the same State generation. Deterministic
history follows native parent hash/result and Iroha parent hash from this tip,
charging each actual source frame before read. Snapshot values remain claims until
verified against the native prefix. Genesis-only snapshot replay and configured
network checks before typed restoration remain explicit follow-up work.

Exact patches include gate13 `bfa5a7e9a5b00cd45b8811483ecd4fcf564755c4ae9b9e32cd1055063a374275`,
native tip18 `e49d81219341d2e6e68a329a810a4cb3235e0dcebe8e0fe9eb2b01dd01439b58`,
block metadata84 `a5752aa01d4911b40837f1f9fd9d35db2040510d7295ebc112ff36ac0fd5e74e`,
and Torii ingress21 `c89d60994d3c8ef2fdfad94305d9b024c7091f7aea85ee7ae0c0c6511e8a265f`.
All application receipts preserve the index and exact original source hashes.

New scoped evidence: two actual queue configuration tests pass; 739 configuration
library tests pass. The subsequent integration target passed 265 and failed two
stale snapshot/protocol expectations, corrected before suite03. Core check06 and
config suite03 are running/queued. Four byte-identical standalone atomic gate tests
pass, with actual threaded/World controls still unrun. Native status/release-refusal
Python tests pass 60; full Python SDK collection is blocked by the missing matching
ABI-24 native wheel. Codec guard03 passes after block-wire retirement. The maintained
September 6 historical archive verification passes. No full Core/SDK/network pass
or release qualification is claimed. The original paragraphs replaced by this
update are in `superseded-native-integration-progress.json` with exact hashes.


## Native identity and canonical signed layout integration

Core check06 ended with 141 errors and 35 warnings. Subsequent source fixes retain
charged Cell owner identity through frozen reads and World projection, remove stale
merge cache pruning and the unused partial-context beacon helper, and remove the
retired lane-context diagnostic digest. Actual schedule pulse capture still verifies
complete native instance/epoch/parent context. Snapshot readers now take the expected
configured chain ID and authenticate raw chain/network identity before constructing
State; the daemon no longer overwrites the restored chain ID. Genesis-only snapshots
return a distinct replay-required outcome and use the existing strict complete-history
path to execute the original signed genesis. Positive-height snapshots still hit the
native startup TODO(S7), and the snapshot compatibility probe's scratch Kura needs
migration. Neither issue is a qualified restart.

The signed transaction payload is now exactly nine fields, with no admission-intent
slot, enum, getter, builder or compatibility decoder. Model patch
`eb45f4f370af78c1b8e306a122939c5b5c164acff72e604a8ba13cf05035698f`
and production callers patch
`ad07aef906abf8101f2a43fe73474300af1f64db6f4ad5756cb22e45590873ab`
are integrated. Four remaining model fixture builder invocations were removed without
removing assertions. Genuine generated signatures and hashes remain owned by the
Rust fixture exporter; SDK fixture generation and codec validation are not yet complete.

The obsolete merge-sidecar transfer/lifecycle/signing guard and its CLI input are
removed. Kagami now projects native global/merged output rows and bounded native
result-preimage pulse claims, checks the exact execution identity, and retains its
explicitly unauthenticated report labels. Its new real native pulse and structural
merge tests are unrun. The source/test retirement inventory names 145 obsolete
protocol tests and their exact source digests; retirement is not a native test pass.

Configuration suite03 completed: 739 library, 267 configuration integration, 1 error,
14 liteserver, 5 Taira contract and 1 documentation test pass (1,027 total, none ignored
or failed). The new original/frozen charged-cell owner regression passes (1 selected,
286 filtered). Core check07 and signed-transaction codec tests are running/queued.
All source integration receipts preserve the existing index and branch `optimizations`.
Current Core, complete workspace/SDK, formal, DA and real network gates remain open.


## Connected wire and obsolete authority closure

Core check07 ended with 127 errors and 33 warnings. Native control ownership,
borrowed context encoding, exact native raw-frame parsing, payload imports and
fee-call signatures were repaired afterward. Original-history evidence attribution
and bounded canonical native evidence frames are integrated; the Core admission,
penalty, portable audit and fully funded deep graph work remain in progress.
The actual SDK transaction fixture generator failed in Core (110 errors/36 warnings),
so no new fixture bytes or hashes were published. Signed-model check02 subsequently
failed before tests (26 cascading errors/1 warning) because four Vec fields carried
an Option-only Norito derive attribute. The minimal correction removes those invalid
attributes while retaining the JSON derive's missing-field rejection. Tests remain pending.

Integrated source stages include the Kura pending QueuePlan owner/configuration
retirement (12 paths), current queue sampling and cleanup controls (7), client signed
transaction layout closure (79), native evidence model (3) and derive correction (3),
model fixture repair (4), and a distinct merged-source Kagami fixture (1).
Kotlin main and test Kotlin/Java compilation retains the JDK8 API guard; Swift source
parsing and client script controls passed in the captured SDK stage. Missing ABI24
native account/Swift artifacts and native Sumeragi captures still block runtime parity.

The lane relay and emergency committee-fill instructions, registry/visitor/permission
and fee hooks are removed (19 paths). Exact old captured rows remain only as negative
instruction fixtures; all other captured values are unchanged. This does not resolve
the independent privacy-record identity mismatch. Emergency configuration is removed
through defaults/user/actual/policy fingerprint (10 paths), with retired-key refusal
controls. Its obsolete World field/authority capture/telemetry is removed (10 paths);
snapshot restoration rejects the old field. The prepared World lifecycle prune/undo
regression now uses actual signed DA pin indexes. Remaining old State relay fixture
controls and read-only relay DTO/query surfaces require explicit closure.

Legacy-codec guard04 passes. Configuration suite04 was started against the current
source; its result is not yet qualified. No Core/workspace/network, DA/liveness/formal
or complete SDK gate is claimed. S6 native cross-dataspace atomic execution, S7
positive-height restart, and S8 complete original deep proof funding remain blockers.
Every applied stage retained the existing index and branch `optimizations`.


## Native historical proofs and canonical storage closure

The [applied follow-up stage ledger](native-owner-followup-stages.json) preserves
31 exact patch hashes, per-file originals/results and integration receipts. It covers
native evidence, configuration and snapshot fixtures, portable checkpoint pages,
geometry, Kura ownership retirement, application proof APIs and original write archives.
All apply receipts preserve the existing Git index; only the `optimizations` checkout
is used. Inventorying or retiring old-owner tests grants no native test pass.

Kura's old V2 finality/replica/MergeLedger production graph and its eviction authority
are removed. Canonical export observes the four real block files and retains native
certificate bytes, descriptor identity and bounded failure behavior. Bodies remain
retained; finite disk exhaustion refuses before mutation. Native release/retention
and positive-height startup remain unfinished. Geometry is addition-only, with the
sole complete zero-origin journal and exact immutable markers; old checkpoints,
retirement receipts, GC and in-place replacement paths are removed.

Portable serving uses actual native finality and independently selected complete
checkpoints. Execution validation now checks input/output commitment shape, exact
wire bounds, top-up count/root presence and the canonical combined post-state root.
Ledger state responses expose `witnessed_post_state_root` and `finality_proof` directly;
that root covers the execution write set, not complete World state. New actual-chain
endpoint tests preserve JSON/Norito closure, below-quorum refusal and re-encoded native
height/signature tampering after cache warmup; they have not run yet.

Snapshot publication signs exact bytes captured from one State generation and checks
the original native current/undo tip against durable certified history. This remains
a signed local cache, not a proof that all World fields are committed by R. Native
archive publication now streams the original ordered ordinary writes and captured
Parliament casting leaves together with lane values into its original charged byte
owner. Historical fee, casting and reserve-receipt queries check these values against
the exact native ordinary-write root. Kura mint outbox checks require the same State
store and full native checkpoint. Portable BLS finality grants no offline mint authority:
paired Pasta, release, hardware profile and recursive monetary verification remain
separate requirements. Complete decoded projection/proof/scratch allocation funding
is explicitly still S8 work.

Configuration suite06 passes 1,027 tests with no failures or ignored tests. Transaction
suite07 executes the correct filter: 59 pass, two fail (privacy canonical wire length
and Vega intent KAT), one generator is ignored. Neither expected identity nor expected
hash was weakened; genuine native fixture generation still requires a working Core.
Earlier suite05 selected zero tests and is not a pass. Core08 stops in two shared API
calls; Core09 stops at the proof-error enum size lint. Their repairs are applied;
Core10 and finality model08 were launched after further integrations and are pending.
Current source remains mutable and unqualified. Real four/seven-validator networks,
all-seat restart, formal/DA/liveness and unchanged workspace/SDK gates remain open.


## Checkpoint consumers and replay refusal follow-up

Core10 reaches Core and reports 72 errors and 51 warnings. The follow-up stages
repair native Arc ownership, exact archive slice bounds and network imports;
remaining obsolete Kura telemetry/pruning consumers and recursive mint adapters
are being replaced. The native testnet observation owner now retains complete
checkpoints and rederives journal/ledger identity from authenticated native decisions.
CLI settlement requires a bounded canonical checkpoint and only advances the pin
after native finality and exact application-output verification. Musubi production
archive/pin/outbox readers derive finality from one State view and original native
BLS/paired-attestation history; their old fixture producer is still being migrated.

Positive-height snapshot caches require complete original execution replay before
returning typed State or mutating Kura. Configured network/chain identity remains
checked first. Both full and manifest-only typed constructors refuse granting a
World at a positive height; emergency Fast and hash-only bootstrap modes cannot
silently invoke strict replay. Signed native H1/H2 and account-substitution tests
are added but unrun. This is a fail-closed restoration boundary, not evidence that
accelerated full-World snapshots or all-seat restart are qualified.

The selected native finality model08 suite passes 36 tests, and KAGEMUSHA native
attachment model09 passes six; neither has failures or ignored tests. The latter
covers complete checkpoint bindings, alternate exact-quorum witnesses for the same
decision, unsigned-genesis/corrupt-certificate refusal and monetary membership
substitutions. The canonical checkpoint exporter is queued separately and is not
a qualification gate. The privacy/Vega fixture failures remain unchanged.

Python launcher/resource controls pass 579 focused checks and the repaired metric
projection suite passes 234. The earlier combined run had 807 passes and one obsolete
synthetic subtotal failure; the subtotal was corrected to the actual ten-family
owner inventory. Python 3.9 collection and a broader obsolete `RunReplayInput` import
remain recorded failures. The scaling reader now consumes the actual four-file
source schema; these controls do not run or qualify a validator network.

Every newly applied stage is appended to the exact follow-up stage ledger; all
application receipts preserve the Git index. Source is still changing. Core, daemon,
workspace, formal/DA/liveness, native SDK and real disposable-network gates remain
open. No live network or value-moving transaction was performed.


The Core10 Kura residue follow-up removes remaining old recovery flags, retired
telemetry publication and emergency-validator pruning calls. Original physical
uncommitted-tail trimming and its crash hooks remain local to the canonical block
store. Added poison/body-preservation/geometry tests remain unrun. Core11 is queued.

The recursive mint adapter now takes an independently selected full native anchor,
checks the exact flagged non-genesis decision, original result preimage and every
paired Pasta share, then constructs the existing release-pinned recursive witness.
Its new actual-boundary positive/substitution tests are unrun. The production
funded result producer still has no caller; publication/restart completion is not
claimed. Musubi now retains all 18 tests while replacing its old V2 producer with
actual native registration and high-water execution; runtime validation is pending.

The first canonical H1/H2 checkpoint export completed; the independent second
capture is pending and no expected hash was hand-edited. The codec guard06 and
history archive07 pass. The release-refusal regression passes one test under the
available Python environment; a Python 3.12 attempt could not import pytest and
remains a recorded tool-environment failure. These are scoped development checks.


## Actual checkpoint capture and connected checkpoint consumers

Both independent checkpoint captures completed and all four marker values match
exactly. The actual binary files and capture provenance are retained under
`fixtures/sumeragi/native-finality/`: H1 is 31,152 bytes (SHA-256
`5a1bcfd4d938fc35d6b0fce593c57d808521bd0ec2320a26b1240b46926fd538`), H2 is
34,329 bytes (`7e83081ea44ddc82e9fe350ad4afa00883c456e9df030825861790ed3b709c2a`).
These are genuine canonical native BLS checkpoint exports, with explicitly
synthetic protocol outputs, not World/XOR/network qualification.

The threshold-signed mobile bootstrap now carries the full bounded canonical
checkpoint instead of a scalar context id. Native runtime recovery compares the
complete original pin. Kagami reads the exact checkpoint file before signing.
The Rust client/query inclusion surface uses native execution commitments and
preserves full carrier/input/output binding; raw inclusion checks confer no
finality authority. Deployment verification now checks every contiguous native
successor under bounded observation limits and fresh exact committee attestations.
Its synthetic-output BLS committee-transition fixtures do not establish real
4→7→4 networks. Original in-observation prefix retention is being extended to
preserve live attestation behavior across multiple concurrent commits.

Core11 stops before Core because a newly added IVM destructor applied the Zeroize
bridge to a slice of fixed arrays unsupported by the dependency. The repair wipes
each original array through the existing volatile bridge; the existing allocation
observer's complete-path unwind test is queued. Core12 stops before Core on a
double reference to the borrowed full bootstrap checkpoint; the encoder now
receives that original borrow directly. Core13 is queued. A sequential job runs
the exact IVM disposal regression, deployment finality tests and mobile bootstrap
model tests. None is yet a pass. FFI and Swift/Kotlin checkpoint codec stages are
still held for coherent integration.

The applied-path whitespace check passes after removing one surplus terminal
blank line. Scoped rustfmt checked 165 Rust files successfully before the most
recent consumer stages; those stages have their own parse/apply receipts.


## Core library compilation and checkpoint consumers

Core check13 reached the library with two missing documentation errors and 294
warnings. The two-field documentation correction is recorded in the stage ledger;
Core check14 subsequently passed in 4m23s with the same 294 warnings. This is a
library compilation result, not execution or release qualification. The complete
Core test-compilation census is now queued.

The C/JNI checkpoint boundary, matching Swift/Kotlin API and exact native
admission-frame shape are integrated together. Their retained runtime tests and
actual encoded-frame capture remain pending. The PK2 binary now requires the
independently selected full checkpoint and challenged native proof; its old
standalone and scalar-anchor modes are removed. Its fixture adds signed NPoS
policy without changing the captured Permissioned checkpoints.

Deployment finality suite01 executed 23 tests: 22 passed, one failed, none were
ignored. The rotating 7→10→7 fixture reaches a genuine nested result-decoder
allocation limit (270479 requested against 262144); no expected result or
identity check is weakened. The subsequent observation-prefix stage restores
bounded custody of earlier challenged responses authenticated during the same
contiguous advance, including across an epoch boundary; its three new controls
are not yet executed. This portable fixture suite does not execute World,
staking funds, custody readiness or network DKG.

The separate privacy/Vega known-answer reconciliation now exports exact canonical
preimages through the maintained explicit generator. Two actual captures and
independent digest checks are pending. No captured values have been substituted
into the assertions yet. Every applied stage is on `optimizations`, preserves the
existing Git index, and is recorded with source and application hashes in the
[follow-up ledger](native-owner-followup-stages.json).


## Native seal, storage and proof identity integration

The paired-Pasta seal now binds raw original native instance, epoch context,
block hash and R in the model, signer and both actual circuits. Old HeightContext
seal builders, auxiliary share wire and V2 proof helpers are removed directly.
H1 can only bootstrap an unsigned zero-authority root; normal mint finality starts
after it. The explicit recursive bound remains 31 seats and exact 2f+1 seals.
This changes the circuit/signing shape: release key generation and actual paired
proof qualification are outstanding. The production recursive producer join is
still in progress; stored shape-only checkpoints cannot supply authority.

Kura now has one canonical storage locator and no empty MergeLedger owner. Its
current geometry journal omits merge paths; old namespaces and the retired cache
configuration are rejected. The original immutable instance marker is written
only after its journals, with incomplete creation resumable solely by the original
Intent or H0 owner. Existing finality/custody tests are being ported to native
sources with an explicit retired-test inventory. Snapshot tests use actually
executed H1/H2/H3 and refuse both ordinary and Fast positive caches before World
construction, preserving signature/network/policy checks and no-mutation controls.

BlockProofs previously hashed stored transport bytes, including a node-local QC.
They now carry the certificate-free executed wire hash authenticated by original
R; the raw wire endpoint still returns the original stored frame. Added positive
and substituted-transport hash controls use CertifiedTestChain. Torii's matching
fixture executes successful/rejected inputs and real Pipeline/Time callbacks.
These are integrated source changes; their runtime gates remain pending.

Scoped evidence: bootstrap-model01 passes 25 tests, none failed or ignored.
OpenAPI-controls03 passes 95 tests plus 19 subtests on supported Python3.12,
including exact current source inventory, closed native finality contracts and
actual required signed KAGEMUSHA release/purpose/hardware-policy fields. No line
budget, assertion floor or unknown-property rejection was relaxed. Core-tests01
stopped in obsolete SCCP imports; the native fixture migration is integrated and
Core-tests02 is queued. Deploy02 stopped at a typed test closure; its refusing
outer/inner decode assertion is repaired and deploy03 is queued.

Privacy/Vega exporter A passed, but B did not reach the exporter: the new seal
roundtrip test selected the framed trait API on an Encode/Decode type. No golden
assertions were changed. Swift capture A failed during compilation after model
and Core sources changed across compilation; it also exposed one real stale
attestation call argument. Both fresh captures and whole SDK generation must be
rerun after repairs. These attempts qualify neither immutable source nor release.

The subsequent seal compile repair uses the actual fixed V1 nested Encode/DecodeAll
codec and removes the obsolete attestation argument; fresh privacy captures C/D,
native seal model tests and SDK capture recipe02 are queued. Scoped rustfmt03
passes 74 Rust paths; whitespace05, codec08 and archive09 pass (64,736 records,
67,311 occurrences). These precede the final fixture-only integrations.

Kura generic fixtures now retain complete original CertifiedTestChain carriers.
Their geometry/source network pin is derived from that same signed genesis,
replacing a fabricated label; a foreign rebinding remains refused. Detailed
[shared-owner](native-kura-retired-test-owner-inventory.json) and
[block-fixture](native-kura-block-test-retirement-inventory.json) inventories
record retired tests and specifically unqualified assertion parity. Native
restart/no-mutation tests are source additions, not runtime passes yet.

Torii lifecycle ingress fixtures preserve the original prepared/applied World,
committee and next height. A missing parent is actually unapplied genesis, not
a missing obsolete sidecar. Admission tests retain exact local pending identity,
wire and route checks; they do not claim restart-durable acceptance. The
[lifecycle inventory](native-torii-lifecycle-test-inventory.json) records changed
fixtures/tests. All-seat accepted-work durability remains a release gate.

The mint producer foundation now validates complete original per-height receipts
and top-up roots before selecting any operation. Loaded mint results/checkpoints
require terminal release-pinned recursive verification plus the exact original
native certificate/authority binding; newly generated results use the same checks.
The lifecycle worker is deliberately not activated: the actual backend currently
lacks allocation custody for installed IPA/PK/VK/protocol graphs, recursive
verification/circuit/FFT/MSM scratch and CPU execution slots. Serialized artifact
preflight sizes and a bounded queue cannot replace these concrete owners. The
next implementation step is funded original source and retained job ownership,
followed by backend allocations before production wiring.

The [canonical retry retirement inventory](native-torii-canonical-retry-retirement-inventory.json)
records ten removed QueuePlan registry tests and three helpers. Current ingress
regressions instead verify unchanged local custody under forged same-hash
signatures, foreign networks, expiry and changed signing policy. No durable
registry retry or all-seat restart guarantee is inferred from those source tests.

### Current test graph follow-up

The Core test check 02 stopped on missing retired `v2_apply.rs` and
`queue_plan_priority_tests.rs` includes before producing a complete census.
It also observed library errors whose repairs were already present in the
working source when inspected; this run does not validate those repairs.
The IVM disposal 02 attempt failed compilation on a reused test closure borrow.
The test now uses a function with explicit independent input and budget borrows;
all expansion, unchanged-source, malformed-index and refund assertions remain.
Runtime validation is still pending.

Kura preflight tests now use original native frames and current canonical
storage. Exact inode, symlink, hardlink, catalog and byte-layout controls remain.
The current geometry journal has a direct missing-baseline refusal test with
exact-byte preservation. Retired merge/QueuePlan/replica-owner tests are removed;
`native-kura-preflight-test-retirement-inventory.json` records the removals and
unqualified crash, budget, replay and availability coverage. Their retirement
is not evidence that the native equivalents have passed.

### Original ordinary-write allocation custody

The native receipt consumer now preplans exact ordinary-write vector/key/value
layouts from borrowed canonical archive bytes and reserves them from the same
original pool before materialization. All charges move with the immutable
witness through proof construction, and payload destruction precedes refund.
The existing archive-view decoder still enforces declared layout, schema,
padding, checksum, exact consumption and payload-derived limits; complete
canonical stream comparison and certified source/root checks remain.
Six new tests cover exact demand, source retention, retry, foreign-pool refusal,
malformed input, post-admission cleanup and conservative unwind. They have not
run yet. Lane/casting decoding, proof/tree scratch, installed recursive backend
allocations and CPU admission are still unfunded; no producer worker was
activated and full S8 custody is not claimed.

### Native test ports and current validation queue

State test check02's retired include failures are repaired with genuine native
execution-owner refusal and coherent tip/hash publication controls. The removed
source inspections and QueuePlan priority assertions are recorded in
`native-state-test-retirement-inventory.json`; current behavior is not yet run.
Kura's 27 physical budget, durable-marker, acknowledgement-failure, restart and
accounting controls now use original native frames; exact rejected-append bytes
and retained custody are asserted. Removed association/merge/replica-owner
coverage is explicitly unqualified in `native-kura-budget-test-retirement-inventory.json`.

Torii ordinary history, trigger completions and batch transfer fixtures use
actual H2 execution. The batch uses the canonical network XOR definition and
checks seeded 10 becoming sender 3 and recipient 7. Restricted disclosure and
authorization assertions remain. No network or runtime success is claimed.
A separate staking test now checks that a registered numeric asset called XOR
with a different canonical definition cannot replace committed XOR in either
staking or reward configuration; positive exact-identity controls remain.

Privacy exporter C completed successfully; D and SDK recipe02 remain queued.
No golden has been changed. Core tests03 and IVM diagnostic03 are queued after
their source repairs. Rustfmt05 passes for the nine recent changed Rust paths;
codec guard09 and history verification10 pass. Diff-check07 caught one redundant
EOF blank line in a concurrently changed State file; the isolated formatting
repair retains every implementation byte and requires a fresh check.

### Checkpoint client composition and ABI25

Applied follow-up ledger entries 102–114 carry complete canonical checkpoints
through fee, committed-inclusion and Parliament C/JNI/Swift/Kotlin/JavaScript
boundaries. Native verification returns its complete promoted checkpoint with
its public result; clients retain defensive copies and persist promotions before
reuse. The fixed Parliament 41-byte output is diagnostic metadata only. Seed
access still follows public verification, and dual output allocations publish
together. Python's connected committed-inclusion consumer and Rust Parliament
fixture ports remain in progress.

The bridge ABI is now exactly 25. Loader pins, current SDK documentation,
packaging identities and artifact gates change together; ABI24 is rejected.
The original ABI stage encountered textual conflicts before any MAIN writes.
The reviewed composition preserves the newer checkpoint APIs in all four
conflicted files. Signed/captured artifact manifests were not relabeled and
require genuine regeneration. The Git index was unchanged by every application.
All work remains in the requested checkout and `optimizations` branch.

Scoped agent checks report 15 Kotlin wallet/transport component tests, 70
artifact/probe tests, Kotlin JDK8 API compilation and Swift syntax success.
They use explicitly injected native outputs or packaging fixtures and do not
qualify cryptography, device keystores or a release artifact. The integrated
JavaScript checkpoint contract suite passes six tests, zero failures or skips.
The broader JavaScript fee suite still refuses the current darwin-arm64 native
checksum profile before initialization; no check was bypassed.

### Current compiler and codec gates

Core test check03 fails with 2,895 compiler errors in the remaining unmigrated
unit-test graph; the captured per-file diagnostic census is retained under
`target/staking-validation/native-core-tests-check03-census.json`. The separate
bridge admission capture fails on thirteen Core feature-library references to
absent component execution/publication owners. Those obsolete helper surfaces
are removed, and every original caller is recorded in
`native-feature-owner-retirement-inventory.json`. Coverage remains unqualified.
Six multisig/account admission tests are now ported to original signed
four-validator execution and native certified publication, preserving the
original rejection, successful registration and actual committed rollback
assertions. Neither test graph closure nor runtime success is claimed.

Deployment finality03 passes 26 tests and fails the maximum-committee decode
control. Complete repeated 31-seat credentials exceed the old 8,192 cumulative
element cap. The repair uses Norito's bounded input-derived cumulative element
policy while retaining the 64 KiB wire, per-sequence, field, depth and inherited
caller limits. Its rerun is pending. Mint-finality model selection01 passes all
36 tests, but does not qualify recursive proof keys or the production backend.

Privacy captures C and D each pass their actual exporter. ABI25 changed after
those captures; fresh complete source provenance is required before accepting
current goldens. No privacy identity check or golden was weakened. SDK recipe02
fails compilation against retired proof/checkpoint APIs; the connected source
repairs require a fresh run. Bridge coordinator01 and IVM diagnostic03 remain
queued behind the shared Cargo artifact lock.

Diff-check08, codec guard10 and historical archive verification11 pass.
Workspace format check06 fails in 29 concurrent source files, none among the
applied follow-up ledger's paths; its exact path census is retained. This is
not a formatting pass or an unchanged-candidate qualification. Ordinary-write
and casting allocation custody are applied; borrowed lane projection, proof
scratch, recursive backend and CPU custody still require closure. The mint
producer remains inactive. Real 4→7→4 transitions, authenticated message loss,
all-seat restart, complete monetary lifecycle and formal/DA/liveness/workspace/
SDK acceptance remain required.
