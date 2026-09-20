# DPN recovery and retained geometry, September 19, 2026

This record covers development-only validation in the canonical `iroha`
checkout on `optimizations`, HEAD
`c1bca08ef8c4f819b8a5e411bfb0cf33d25c481b`.
It is not shipping qualification or live deployment evidence.

## Current implementation

State retains the original move-only raw and tiered geometry operations across
local storage refusals. Journal, namespace and catalog retries retain their
original owners; partial fresh provisioning preserves its first recovery cause.
Strict replay retains its original State bundle and receipt cursor through a
local geometry refusal. No empty production blocks or compatibility branch was
introduced.

Typed storage/drain-observation failures propagate through autoscale, certified
merge staging, candidate validation and BodyStore without becoming durable
rejection evidence. All nine rejection-event sites use the conditional emitter.
The regression uses identical diagnostic text for local and deterministic errors
to prove that classification follows provenance, rather than message matching.
An autoscale retry retains its original sample/count and cannot claim completion
before its fallible lifecycle evaluation succeeds. Actual Queue release retains
the existing Validate dispatch, row and acknowledgement.

## Recorded outcomes

| Development build | Compilation | Focused runtime | Geometry runtime |
| --- | --- | --- | --- |
| 16 | Pass | 39/53; 14 failures | 168/169; one in-memory fixture failure |
| 17 | Pass | 43/46; three failures | 170/170 |
| 18 | Fail: new local-recovery variant missing from rejection-event match | Not run | Not run |
| 19 | Pass | 76/76 | Not repeated; geometry source unchanged from 17 |

Build16's eight strict replay failures remain unresolved. They fail while
constructing their fixture at `CommittedStatePublication(ExecutionOutputCapacity)`;
the complete consuming publisher is required. Build17 and build19 exclude those
eight known failures from their explicitly scoped selections and claim no pass
for them. The other failures led to original error-source propagation,
configured-lane anchor ordering, in-memory geometry and autoscale observation
corrections; relevant controls pass in build19.

Build19 source remained unchanged through compilation and all 76 tests. Its
tracked-diff SHA-256 is
`43e49de69bcbb42e6bb6199c61e3e04c1cae15e05c47cff0d46b6a1d4e3aa6eb`;
the test executable SHA-256 is
`478b3f1e260490867354748cb8b96e48616a4225272323e88d0f9246370cd9b3`.
Source manifests also bind untracked Rust inputs. Local evidence is under
`target/dpn-devex/core-build19-*` and `target/dpn-devex/focused19.json`.
Build17 geometry evidence is separately bound to its source and executable in
`target/dpn-devex/geometry17.json`.

## Remaining production work

The actual validator drops its prepared carrier and returns only a commitment;
cached/reproposal markers can bypass preparation, and Apply reexecutes into the
raw State publication guard. Keep that guard until complete original
Validate-to-Apply custody, pre-vote resources, source/Native authority, geometry,
archives and the consuming publisher are connected. Component checks cannot
replace the positive pending-Kura, persisted-block recovery, unchanged
four-validator Linux supervisor restart and clean-client qualification gates.

At 2026-09-19 07:59:52 UTC the public faucet policy, `/status` and
`/v1/sumeragi/status` all returned nginx HTTP 502. No live write, validator update,
ledger reset or DPN deployment was performed. Retained height-3598 beacon recovery
remains distinct from fresh bootstrap; shared-ledger replacement is unapproved.

At 08:37 UTC a new check again received HTTP 502 from the policy and consensus
status endpoints. The first `/status` probe had a local name-resolution error;
one retry at 08:37:32 UTC returned HTTP 502. Exact local responses are retained
in `target/dpn-devex/public-health-after-build19.json`.

## Exact superseded status paragraph

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native inspection of seven retained lifecycle frames and the exact height-7 Decision WAL confirms the original Fetch→Store→Validate→Apply owner. The candidate authenticates that owner through pending-Kura admission, ready census and settlement; changed-parent, changed-Decision and foreign-owner controls pass. DPN development build14 compiles and passes all 196 focused archive, checkpoint, carrier, runtime and World controls with unchanged source and executable; the preceding build12 passed all 158 geometry transition/recovery tests. Archive authentication now uses one guarded Kura oracle for the exact receipt, verified finality and signed result-bearing body. Joint acquisition checks both original archive captures before State fences. Held-lease persistence probes the archive index without blocking, preserving exact bytes/reservations on refusal and returning the actual reader/writer release event; poison remains a storage failure. Real reader→Kura contention, partial insertion retry and no-missed-wake controls pass. Original dirty Parliament/citizen telemetry, Musubi shortfall and consuming DA-pin effects are retained. Prepared journal writes retain the original Kura, parent, predecessor and active temporary; same-bytes replacements, parent substitution and occupied absence are rejected. File-sync and post-rename directory-sync retries keep the exact object, and a reconstructed current phase reattests that file without creating a competing temporary. The complete replacement lane map is built before journal effects. This is the existing locked per-phase path: its outer caller still drops the attempt on I/O error, and pre-vote physical reservations, instance pins and the complete State publisher remain unfinished under roadmap N0. Earlier build09 passed 45 of 46 focused checks, with the linked Apply regression returning the precise production failure: sealed execution outputs still reach raw StateBlock publication. Retirement observation performs no storage maintenance; explicit maintenance retains terminal receipt sync and failed merge-tail repair. Canonical journal reopen cannot recreate missing files. First fatal causes survive shutdown/completion races. Thirteen unchanged journal/snapshot controls passed build08, including cold capture after admission and exact payload/reservation lifetime. No full restart or deployment is claimed. Public funding policy and `/status` returned HTTP 502 on September 19 at approximately 05:11 UTC. All 167 release-runner Python tests passed after moving the five pending-Kura checks first after configuration, before other startup checks, shipping builds or network execution. No shared ledger replacement is authorized.

## Exact superseded roadmap paragraph

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 103 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Consume the now-retained ordinary source-prefix owner and exact lease-bound checkpoint receipt with the actual journals. Join the actual canonical validator and original Validate-to-Apply owner to the now-qualified recorded Native replay, retaining the original source groups, one witness and distinct output/metadata cuts. The actual pristine QueuePlan/NPoS control owner, original authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks now pass scoped recorded-execution qualification. Join that retained authority to the actual global validator and validated-prefix owner, preserving the original whole carrier, source groups, State generation and journals. Complete Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease relay coverage. Preserve the qualified recorder order and genuine single/atomic PipelineGas effects; this Native boundary is not a universal audit of State APIs. Preserve bounded resources and complete publication authority before opening the live Native header gate. Separate retirement observation from maintenance only while preserving original route-directory custody and admitting aggregate evidence/writer resources. Finish bounded capture, file-handle and installation resources before exposing the sole State publication operation. Retained per-phase journal writes now bind the original parent/predecessor and preserve rename/sync retry state, with the replacement lane map built before effects; all 158 geometry tests pass. Next retain the actual move-only geometry attempt across the outer caller's typed storage refusal and consume it under the existing Kura publication lease, then connect complete physical reservations, journal ownership and instance pins before voting. Consume admitted archives under the original Kura lease, preserving partial insertion progress and the exact logical reservation across local I/O or physical index contention; release every State/Kura owner before waiting on the refusing index. Carry original captured telemetry and DA-pin effects through that same consumer. The passing source-custody, archive, exact retry, joint Kura/State and decision-binding controls establish component joins; they do not authorize publication or replace production handoff. Qualify native Windows namespace durability before claiming that storage target. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.

## Exact superseded build19 status paragraph

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Development build19 compiles and passes all 76 selected local-refusal, Queue-release, autoscale, carrier and recovery-fixture controls with unchanged source and executable. Build17 separately passed all 170 geometry controls; build14 passed 196 archive/checkpoint/carrier/runtime/World controls on its recorded source. State retains the original raw and tiered geometry attempts, replay bundle and cursor across local failures; partial provisioning preserves its recovery-required cause. Local storage and drain-observation errors remain typed through candidate validation, create no durable rejection and emit no rejection event. Autoscale retry preserves the original sample/count and marks evaluation complete only after lifecycle work succeeds. Queue release retries the original Validate dispatch and acknowledgement; malformed evidence remains a deterministic rejection. These are component results. Eight strict replay controls still fail at fixture publication because sealed outputs require the missing complete consuming State publisher; they were not rerun or counted as passes in build19. Validation still drops its executed carrier, cached/reproposal markers can bypass preparation, and Apply reexecutes. Original Validate-to-Apply custody, complete pre-vote resources, Native/geometry/source authority and the consuming publisher remain required under N0. The positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. Public funding policy, `/status` and `/v1/sumeragi/status` returned HTTP 502 on September 19 at 08:37 UTC. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Archive custody, held-lease geometry and lock ordering (builds20–22)

Build20 failed compilation in four new tests (two pointer casts and two partial
matches retaining a State borrow). Build21 compiled and passed 244/254 selected
Core controls. Its ten failures comprised five obsolete panic expectations,
one stale cache-authority expectation, two lock-order controls and two Native
fixture stack overflows. Those failures were retained, not silently deselected.

Candidate archives now reserve original keys and reputation predecessors before
execution. Original-State capture acquires no archive index. Insertion refusal
returns the same detached carrier with original archive material and capacity;
a completed provider plan survives reputation refusal. Initial admission refusal
returns the same borrowed carrier and predecessors for synchronous handling.
Capacity outlives retained payloads on early errors and unwind.

The private carrier geometry seam now resumes its original raw/tiered attempts
under the existing original Kura lease, without State publication. The shared
State/snapshot-worker backend uses a typed publication mutex whose actual unlock
notifies waiters. Backend contention returns the same decided carrier after all
physical writers release. Root/config/foreign-lease and partial-sync controls pass.

Build22 fixed the confirmed lifecycle deadlock by acquiring the canonical runtime
current/undo owner before State publication fences and consuming it before World
cleanup. MV replacement preserves exact undo, old readers and publication identity
on abandonment. A deterministic acquisition handshake verifies no partial
catalog/manifests publication and leaves block-commit fences free while waiting.
Native fixture construction is separated from the assertion phase, preserving the
default test stack and every original control. SCCP tests now check the exact
typed validation error, empty events and unchanged staged/committed State hashes.
The route-mismatch fixture uses the exact lane destination, not an opaque label.

Build22 source and test executable stayed unchanged: 105/105 MV tests and 264/265
Core controls passed. The one failure is the duplicate-SCCP fixture: a header
setter after output attachment clears those results, so it reached
MissingTransactionResults instead of DuplicateOutboundMessage. The fixture now
sets its root before attaching results; that correction awaits the next build.
Build22 tracked-diff SHA-256:
`22814b1c34b7c8ebb325ab7c491067b4c9b62b1c30b2aa69e024102e6c002614`.
Core executable SHA-256:
`bb1727f03a4e39ca95878ade259be7837361307dd60fb846b27254a103d3172b`.
MV executable SHA-256:
`94a448520a7de367e7e8ca6037c588fb78d4d75824845a90a5d2b388427e684b`.
Evidence: `target/dpn-devex/core-build22-*`, `focused22.json` and `mv22.json`.

At 09:08:42 UTC all three public health URLs again returned HTTP 502; exact
responses are retained in `target/dpn-devex/public-health-build22.json`. No live
mutation or deployment occurred. Complete resource admission, source/Native
publication authority and the production consumer remain unfinished.

## Original source authentication (build23)

Build23 compiled and passed 285/286 selected Core tests with unchanged source and
executable. The corrected duplicate-SCCP control passed. Strict MV library/test
Clippy also passed. The remaining new Kura control exposed an existing transitive
cache mutation in a nominally read-only retained-finality validation path.

A private complete decision-plus-lease owner now authenticates the original
checkpoint, sealed result-bearing wire and retained execution commitment before
State acquisition. Native authentication retains original verified groups,
Decisions, contexts, witness and inventory; it joins their exact first-carrier
finality and admission bytes at the original position. Authenticated original
body absence (eviction or imported prefix) uses only the original privately
verified source. Missing/corrupt proof or occupied corruption refuses. The
current result-bearing carrier must remain available. Abort drops source
authentication with the lease and returns the complete original decision.

The shared output-seal wire check is factored without moving the original
World-delta/policy/inventory validation after the deterministic tail. Ordinary
substitution and wire-mutation controls prove refusal before State fences and
unchanged original allocations on restored retries. Real single/atomic Native
controls pass missing/corrupt source proof, restoration and authenticated absence
checks. This grants no source release or State publication authority.

The failed cold-cache control found retained record validation calling the normal
block loader, which promotes body and derived indexes. The correction uses its
existing nonpromoting reader and removes accidental index fills from that path,
while retaining exact body/wire/merge checks and the subsequent fallible disk
read. The strengthened test starts with deferred hashes and empty reverse and
query indexes as well as a cold body. The correction awaits build24.

Build23 tracked-diff SHA-256: `cfc2b44bfed6239f4d9390347e063fc3eeeedfa463ba3946d3e11ebe3cca050a`.
Core executable SHA-256: `c219dc8890da92139cc79773dc3e0f0de11896f820f3165e342dd7f7bf354901`.
Evidence: `target/dpn-devex/core-build23-*`, `focused23.json` and
`mv-clippy23.log`. The original eight strict replay failures and complete
production consumer/qualification blockers remain open.

## Exact superseded build22 status paragraph

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Development build22 compiles with unchanged source through testing: all 105 MV tests pass, and Core passes 264 of 265 selected archive, carrier, geometry, canonical-runtime, lock-order and Native controls. The remaining duplicate-SCCP fixture attached results before a header setter invalidated them; construction order is corrected but not yet rebuilt. Build21 passed 244/254 and exposed the fixed runtime-writer/State-fence deadlock, two default-stack fixture overflows and stale SCCP/catalog expectations. Candidate archives now reserve original predecessors before execution, capture once without an index lock and retain detached journals across insertion refusal. Carrier geometry resumes the original raw/tiered attempts under the original held Kura lease; typed backend contention releases every physical writer and returns the actual release observation. These are component results. Eight strict replay controls still fail at fixture publication because sealed outputs require the missing complete consuming State publisher; later selections do not count them as passes. Validation still drops its executed carrier, cached/reproposal markers can bypass preparation, and Apply reexecutes. Original Validate-to-Apply custody, complete pre-vote resources, Native/source authority and the consuming publisher remain required under N0. Positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. Public funding policy, `/status` and `/v1/sumeragi/status` returned HTTP 502 on September 19 at 09:08 UTC. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Verified cache correction and combined controls (build24)

Build24 compiled and passed all 289 selected Core controls with source and
executable unchanged through testing. This retains every build23 selector and
adds retained wire/bodyless/restart tamper, retained SCCP inventory tamper and
header-matching canonical-wire substitution controls. The cold-source read now
leaves body cache, deferred hash slots, reverse-height map and query indexes
unchanged while retaining all four Kura fences. Historical observation preserves
its existing chain-length/prune refusal and exact body/wire/merge comparisons;
the following fallible source-body read remains mandatory.

The earlier ten build21 failures, duplicate-SCCP ordering failure from build22
and cache side effect from build23 are covered by passing controls. MV source
has not changed since build22's 105/105 unit pass; strict MV library/test Clippy
passed during build23. The codec and history guards passed (64,736 historical
records; 67,311 occurrences). The eight strict replay fixture-publication
failures are still unresolved and excluded from this explicitly scoped result.

Build24 tracked-diff SHA-256: `d7542aed6eece6cd88a02a01096b513b9e8352079d32c02b11181cb561f1e620`.
Core executable SHA-256: `f1fe01d417fa285c8f4c8cdbaee024a1d74cbd4fe82979cda87ecf3082a9d393`.
Source manifests bind tracked and untracked Rust inputs; evidence is retained in
`target/dpn-devex/core-build24-*`, `focused24.json` and
`focused24-source-after.json`. Documentation was updated after this verified run.

At 09:33:52 UTC funding policy, consensus status and `/status` again returned
HTTP 502; see `target/dpn-devex/public-health-build24.json`. There was no live
mutation, validator update, ledger replacement or DPN deployment. Original
Validate-to-Apply custody, concrete aggregate pre-vote resource accounting,
source-release/Native durability, complete consuming publication and unchanged
network/client qualification remain required before shipping.

## Exact superseded build24 status paragraph

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Development build24 compiles and passes all 289 selected Core controls with unchanged source and executable. All 105 MV tests and strict MV library/test Clippy also pass. This closes the selected failures from builds21–23: runtime-writer/State-fence deadlock, Native fixture stack overlap, stale SCCP/catalog expectations and transitive cache promotion in a read-only retained-evidence path. Original archive predecessors and detached journals survive insertion refusal; carrier geometry resumes under its original held Kura lease and returns actual backend release observations after releasing physical writers. A private complete decision-plus-lease owner joins exact checkpoint, sealed wire, original witness commitment and original Native admission evidence before State acquisition. Missing/corrupt source evidence refuses without replacing the owner; historical body absence requires original privately verified evidence, while the current carrier body remains mandatory. These are component results. Eight strict replay controls still fail at fixture publication because sealed outputs require the missing complete consuming State publisher; later selections do not count them as passes. Validation still drops its executed carrier, cached/reproposal markers can bypass preparation, and Apply reexecutes. Original Validate-to-Apply custody, complete pre-vote resources, source-release/Native durability and the consuming publisher remain required under N0. Positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. Public funding policy, `/status` and `/v1/sumeragi/status` returned HTTP 502 on September 19 at 09:33 UTC. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Retained catalog completion and runtime effect projections (builds25–28)

The original raw/tiered geometry owner now completes CatalogPublished under its
held Kura lease. It reauthenticates the original terminal journal descriptor,
bytes and installed identities before returning a mapping borrowed from the
same owner and lease. Preparation is explicit; a retry cannot allocate a
replacement operation. Pure tiered completion authentication preserves progress
across identity refusal and exact object restoration. Four new carrier controls
cover missing preparation, pending catalog directory-sync retry, same-byte
journal substitution and no-change completion; one tiered control covers retained
terminal progress. This does not authorize State publication or source release.

Prepared DA effects retain the original bundle allocation, canonical lineage
visibility, confidential receipt positions/policy and lane mapping. Disposable
reset journals cannot suppress candidate visibility. Five controls cover drop,
ahead caches, recreation boundaries, retired identity retention, and captured
confidential receipt/shard inputs. Prepared lifecycle effects retain manifest and
privacy owners, reset sets and configuration; three controls preserve exact Arc
identity, one generation, reset-before-DA telemetry, deferred disk work and replay
suppression. Cursor persistence captures final same-carrier cursors after the
State generation under the original commit/lifecycle fences. Actual index writes
and remaining post-work still require complete resource admission.

Workspace Concread is pinned to the already locked 0.5.10; MV inherits that pin
with its unchanged ebr/maps/foldhash feature set and disabled defaults. Locked,
offline Cargo metadata confirms the sole consumer and unchanged dependency graph.
The allocation ownership note identifies actual EBR/B+tree reclamation paths;
no complete heap pool or production resource policy is claimed.

Build25 stopped at a strict trivial-cast lint in a new pointer-identity test.
Build26 compiled and passed 285 of 289 selected controls. Its four failures were
old DA fixtures: two invoked Network execution without exact genesis admission,
one replaced a configured dataspace baseline through the ordinary setter, and
one changed only the non-authoritative Nexus cache. The two apply-only controls
now explicitly use nonempty result-bearing structural fixtures; production
validation guards and all original materialization assertions remain intact.
The filtered unknown-lane control was renamed to state its actual behavior.
The revert fixture creates its configured catalog up front. Build27 verified
three corrections but correctly refused the confidential fixture's default Kura
anchor; build28 initializes that fixture with its exact authenticated catalog
and privacy evidence. No production source changed after build26.

Build28 compiled and passed all 43 selected controls, including all four corrected
fixtures, the new component controls and lock/retirement regressions. Both build26
and build28 source manifests remained unchanged through their respective builds
and runtime checks; executable hashes also remained stable through those checks.
These selections do not run or repair the eight known strict replay publication
failures. No validator build, shipping qualification or live write was performed.

Build26 tracked-diff SHA-256: `12110328a20f31be7c93442951d4f7b61a03df42b229e120bc8d4dec1f6ff3a3`.
Core executable SHA-256: `65ebe38b42b14842971e449155164f6f62f4446f4dfae126f68a562ef6c9017c`.
Build28 tracked-diff SHA-256: `8eaa04035fb6dd9214249efdb69badf15dace78599a45a072726cb0ea560ab8d`.
Core executable SHA-256: `23e67aafcea430974a2a1fd7448b3e412405afccad0fd1ecd7d0b65911f29925`.

Evidence: `target/dpn-devex/core-build25-*` through `core-build28-*`,
`focused26.json`, `fixture27.json`, `focused28.json`, their source-after manifests,
`metadata25.json` and `codec26.log`. The codec guard passes. Documentation was
updated after testing. At 09:59:27 UTC all three public funding/status routes
returned HTTP 502 (`public-health-build25.json`). Original Validate-to-Apply
custody, aggregate resource ownership, Native/source-release authority, complete
consuming publication and unchanged network/client qualification remain open.


## Retained validation markers and actual EBR allocation custody, builds29–30

The canonical `optimizations` checkout now vendors the exact locked Concread
0.5.10 source and retained MPL-2.0 license. Only its EBR implementation differs
from the upstream package. Cargo resolves the local patch with locked offline
metadata; the MV integration test adds only the existing crossbeam-epoch 0.9.20
dev-dependency. No registry source was edited.

Typed, non-Clone charges are admitted before payload cloning and retained in the
original allocated generation. Commit transfers the already allocated writer.
Abort and epoch reclamation destroy/deallocate the exact payload allocation before
charge release. Clone/destructor panic conservatively retains the charge. Four
black-box tests inspect actual allocator free, the original pointer and layout,
commit transfer, and delayed collection under an unrelated epoch pin. All four
and the 105 existing MV tests pass. Both unaccounted charged-write methods fail
compilation as expected; strict MV library/integration-test Clippy passes.

The new private validation service owns bounded candidate/marker descriptor tables
and a typed associated journal owner. It installs original custody before marker
fsync, preserves confirmed receipts across later reproposal write failure, restores
selected owners on abort/refusal, and keeps consumed-subject tombstones until the
height store retires. Review caught and corrected a v5-consume to delayed-v4
reexecution gap before testing. Tests cover marker file/directory sync refusal,
exact store identity, cached-owner absence and capacity refusal before execution.
The actual journal matcher additionally checks full context and signed resultless
proposal identity against a genuinely executed fixture.

Build29 compiles and passes 75/76 selected Core controls. The one failure was an
invalid fixture reopening the same exclusive directory while its original store
was alive (OS35 WouldBlock). Build30 changes only that test, closing its store
while preserving the old service identity, and passes all six final Core custody
controls. No production source changed between these builds. Source manifests
and executable hashes remained unchanged during their build/test checks.

These are component results. The adapter does not replace live scalar validation.
Production MV charge transfer, nested allocation coverage, complete resident
resource policy, source-release/Native durability and the consuming publisher
remain unfinished. The known eight strict replay publication failures and the
required unchanged four-validator Linux/client/deployment qualification remain
open. All three public funding/status routes returned 502 at 10:29:47 UTC. No
validator update, live write, reset or shared-ledger replacement was performed.

Build29 tracked-diff SHA-256: `2215e5702d58c1f043d8cde780cade9cc9b8758add208c21949a32b53bc4cb0c`.
Core executable SHA-256: `7102d0d94b3fc244fb4a39773b71d68b6857e005f34a004914340992754e911d`.
Build30 tracked-diff SHA-256: `2215e5702d58c1f043d8cde780cade9cc9b8758add208c21949a32b53bc4cb0c`; its separate untracked
source manifest records the sole test correction. Core executable SHA-256:
`1a002c39640a82ae0e6ac771db2bb40952e5af02a4f1ebcb336330be34d26731`.

Evidence: `target/dpn-devex/core-build29-*`, `core-build30-*`, `focused29.json`,
`focused30.json`, source-after manifests, `mv29.log`, `mv30-clippy.log`,
`charged-api29.json`, `metadata29-resolved.json`, `codec29.log`,
`public-health-build29.json` and `carrier-resource-policy29.md`. Documentation
was corrected only after verification, including the allocation timing note.

### Superseded current-status paragraph, preserved verbatim

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Build28 compiles and passes all 43 final focused geometry, lifecycle, DA and lock-order controls with unchanged source and executable. Build26 passed 285/289 in the broader component selection; its four DA fixture failures are corrected and covered by build28. Earlier build24 passed 289 other selected Core controls; 105 MV tests and strict MV library/test Clippy passed on unchanged MV source. Retained geometry now completes CatalogPublished and reauthenticates the original journal and installed mapping under its original Kura lease. Candidate DA visibility, confidential policies and lifecycle projections are captured before publication; cursor persistence runs after the State generation with the original lane mapping. Concread now uses one workspace pin matching the existing lockfile. These remain component results. Eight strict replay controls still fail at fixture publication because sealed outputs require the missing complete consuming State publisher; later selections do not count them as passes. Validation still drops its executed carrier, cached/reproposal markers can bypass preparation, and Apply reexecutes. Original Validate-to-Apply custody, concrete aggregate pre-vote resources, source-release/Native durability and the consuming publisher remain required under N0. Positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. Public funding policy, `/status` and `/v1/sumeragi/status` returned HTTP 502 on September 19 at 09:59 UTC. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Original Cell allocation custody (build31)

The original MV Cell current and undo successors now survive detachment,
contention, abort, worker transfer and publication. Installation reacquires
only the exact target writers and authenticates the original predecessor;
it neither clones successor payloads nor allocates their next publication
identity. Charged Cells require prepaid current/undo owners before initial
writer cloning, and retain them until their actual EBR allocations are freed.

A finite requested-layout byte pool atomically reserves complete layout sums,
refuses impossible demand without a wait, and wakes temporary-capacity retries
only after original allocations return credits. This is a resource primitive,
not an installed State-wide budget. Callers must reserve the complete operation
before allocation; they must not wait while retaining a partial reservation
whose own release is required to satisfy the remainder. Nested payloads,
B+tree nodes, cursor vectors and collector/control allocations remain separate
production work. All existing State field instantiations remain explicitly
untracked until exhaustive integration is complete.

A raw writer could panic while cloning its first undo generation, before the
physical guard reached its release-notification wrapper. An already registered
retry then slept indefinitely. Cell and every Storage acquisition entrypoint
now signal after that raw lock unwinds. Deterministic regressions register a
Busy waiter before triggering the panic and verify its wake followed by typed
Poisoned refusal.

Validation: MV31 passed 120 library and five allocator/epoch integration tests
on unchanged source. Core31 compiled and all 75 selected World/runtime/carrier publication, lock-order and real-State controls passed with unchanged source and executable. Strict MV library/integration Clippy, scoped formatting and the charged-API compile-fail doc test passed.
These results do not establish full resource admission, the live retained
Validate-to-Apply consumer, retained-ledger recovery or completed deployment.

## Exact superseded build30 status paragraph

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Build29 passes 105 MV tests and four new actual-allocation custody controls; two compile-fail controls reject unaccounted writes on charged cells. Its broader Core selection passes 75/76; build30 corrects the remaining test fixture by closing the original exclusive BodyStore before reopening and passes all six final custody regressions with unchanged production source. Strict MV library/integration-test Clippy passes. The private retained-validation adapter installs the original candidate before marker fsync, preserves earlier confirmed receipts across later write refusal, restores aborted selections and retains consumed-subject tombstones until height retirement. Its existing descriptor bounds refuse before execution. The exact locked Concread source is now vendored with typed charges retained through actual EBR allocation destruction and deallocation, including unrelated epoch pins and conservative unwind. MV production storage still needs charge integration, typed nested allocation coverage and an aggregate resident-resource policy. These are component results: the live scalar validation path still drops its executed carrier and Apply reexecutes. Original production Validate-to-Apply custody, complete pre-vote resources, source-release/Native durability and the consuming publisher remain required under N0. Earlier retained geometry, DA/lifecycle and lock-order qualifications remain scoped evidence. Eight strict replay publication controls, positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. Public funding policy, `/status` and `/v1/sumeragi/status` returned HTTP 502 on September 19 at 10:29 UTC. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Exact superseded build30 roadmap paragraph

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 105 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Consume the now-retained ordinary source-prefix owner and exact lease-bound checkpoint receipt with the actual journals. Join the actual canonical validator and original Validate-to-Apply owner to the now-qualified recorded Native replay, retaining the original source groups, one witness and distinct output/metadata cuts. The actual pristine QueuePlan/NPoS control owner, original authenticated applying context through suffix finalization and common AXT/DA/SCCP postchecks now pass scoped recorded-execution qualification. Join that retained authority to the actual global validator and validated-prefix owner, preserving the original whole carrier, source groups, State generation and journals. Complete Native DA/pin/SCCP owners, broader NPoS evidence/penalty and sponsor/lease relay coverage. Preserve the qualified recorder order and genuine single/atomic PipelineGas effects; this Native boundary is not a universal audit of State APIs. Preserve bounded resources and complete publication authority before opening the live Native header gate. Separate retirement observation from maintenance only while preserving original route-directory custody and admitting aggregate evidence/writer resources. Finish bounded capture, file-handle and installation resources before exposing the sole State publication operation. State now retains the actual raw/tiered geometry attempts and replay receipt cursor across typed local storage refusal; all 170 geometry controls passed development build17. Build19 separately passes all 76 selected local-refusal and recovery-fixture controls. The private carrier seam now resumes its original geometry under the already-held original Kura lease, releasing all physical writers on typed backend contention. Exact retained source authentication now joins the complete carrier to its held Kura lease before State acquisition; build24 passes 289 selected Core controls. Retained geometry now completes and reauthenticates the original CatalogPublished boundary; original DA/lifecycle projections and post-generation cursor work pass build28’s 43 focused controls. Connect complete physical reservations, source-release/Native durability and instance pins before voting. Preserve typed local recovery through validation without rejection events, and retain the original autoscale sample until its fallible lifecycle evaluation completes. Eight strict replay controls remain blocked at the missing complete publisher; no release or restart pass follows from these component results. Consume admitted archives under the original Kura lease, preserving partial insertion progress and the exact logical reservation across local I/O or physical index contention; release every State/Kura owner before waiting on the refusing index. Carry original captured telemetry and DA-pin effects through that same consumer. The passing source-custody, archive, exact retry, joint Kura/State and decision-binding controls establish component joins; they do not authorize publication or replace production handoff. Qualify native Windows namespace durability before claiming that storage target. The private marker-custody adapter and actual EBR charge hook now pass build29/30 controls, including fsync refusal, consumed-subject tombstones and epoch-delayed free. Integrate real current/undo MV allocation charges, exhaustive typed nested accounting and an explicit aggregate resident policy; the hot-tier weight limit is not that policy. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.


## Original map successors and publication controls (builds32–36)

Storage now retains its original B+tree cursor and original undo allocation
through detachment, physical contention, abort and publication. It borrows touched
values from those exact owners instead of cloning an after-value vector or
replaying mutations during installation. Replacement undo is staged once by the
original block. The original next MV identity and reader publication shell are
also retained across retries.

A detached B+tree cursor shares untouched nodes. Its owner therefore retains both
the exact base reader generation and actual shared root until unpublished work is
destroyed. Reattachment checks the physical root and exact reader generation under
the original writer lock, rejecting stale, foreign and equal-content ABA owners.
This check remains necessary even if an outer identity is accidentally unchanged.

Build32 passed 122 MV and five EBR controls, but seven of eight map controls: its
strict scalar allocation control exposed two 64-byte calls during commit. Exact
phase diagnostics identified macOS lazy pthread mutex allocation for the reader
link and retired-node list. Those fields now use their actual write-once semantics
and preserve the original retirement vector. The permanent reader mutex is
initialized during map construction, before any candidate; a ninth control covers
a fresh map's first commit with no prior read. Build33 passes all 122 MV, five EBR
and nine map controls on unchanged source, including both strict zero-allocation
paths. Core33 compiled and all 75 selected Core publication/custody/lock-order tests passed on unchanged source and executable. Strict MV Clippy then refused a retained retry error of at least 154 bytes. The next candidate retains the original cursor in a Box allocated during initial writer acquisition, keeping retry descriptors small without allocating during detach/adopt/abort/commit. Build34 then passed all 122 MV library, five EBR and nine map controls (136 total) plus strict MV Clippy for the library and both integration targets, on unchanged source. Build35 also passed the charged-API compile-fail documentation control.

The independent controls cover original key/value/boxed payload addresses,
clone-free retry and publication, both writer contention points, raw-base changes,
foreign/stale/ABA refusals, sibling invalidation, map destruction, worker transfer,
old-reader chains, clear, splits/removals and actual payload destruction. This is
original successor ownership, not complete node/cursor/nested resource charging
or an installed aggregate State policy. The live consuming publisher and deployment
remain unfinished. No production empty block, ledger replacement or live mutation
was performed.

The maintained native release checker now requires 56 exact MV ownership controls
before executing Core startup/recovery checks, shipping validation or network
qualification. The library, EBR and map harnesses each have independently bound
copied artifacts and exact test censuses. Missing or changed artifacts and failed
ownership controls cannot reuse a successful checkpoint. Full qualification still
compiles one consistent feature graph before these early execution checks; the
focused diagnostic mode is explicitly unqualified. The selected 42+5+9 controls
were verified against the actual build34 executables in both basic/full scopes.

Build34 first exposed Python bytecode's static nested-context limit in updated
fixtures; consolidating mock contexts fixed that failure. Build35 exposed stale
fixture counts and an intentionally narrowed CLI-copy fixture that had not disabled
the newly required MV stages. Those fixtures were corrected without adding a
missing-harness fallback. Build36 passed all 172 maintained release-check tests.
Core36 compiled successfully, but its post-build source boundary changed before
focused tests ran. No Core36 focused test success is claimed.

The user's concurrent merge task committed the exact frozen build36 tree as
7d2ff5b9fc286823471bf166391bf06cd96899e8 and then merged additional Core/MV changes.
The commit equivalence is proven by the identical tracked-diff SHA256 and all 102
previously untracked SHA256 values (source36-commit-equivalence.json). Those prior
passes remain scoped to the frozen tree. They do not qualify the subsequently
merging checkout. Current-source qualification must follow merge completion and
fresh source capture. This task did not overwrite files owned by the merge task,
change Git state, restart a validator, or deploy anything during this boundary.

This record is now integrated with the subsequent scoped native results.
The concurrent task's Git state was preserved; deployment remains incomplete.


## Native typed deployment and authenticated genesis (DPN development 37–46)

The native dataspace initializer now derives its inline manifest from independently
validated signed genesis and the selected four-peer deployment profile. It no
longer accepts a handwritten `--lane-manifest` file. Four distinct accounts must
be explicitly registered, uniquely bound to the selected peers, and activated in
lane zero; account keys are not inferred from peer keys. Endpoints come from the
selected public profile. Missing, duplicate, changed or unactivated bindings fail
before publication. Loaded intent must retain the same committee and endpoints.

The closed native JSON descriptor and nested binding/privacy types now live in
the shared data model. Core installation and CLI preflight share one bounded
semantic preparation helper for exact lane/dataspace/alias identity, governance,
privacy and 3f+1/2f+1 checks. Runtime installation retains the cumulative source
budget across additions. This check does not replace live account/key/PoP
eligibility, immutable baseline ownership or catalog activation authority. The
producer now emits canonical lexical JSON through Norito's semantic Value writer;
both typed and canonical encodings retain the 256 KiB native source bound.

Current canonical output APIs replace retired per-entry result access in CLI
finality verification and fixtures. The authenticated anchor and BlockProofs join
the exact requested input to its complete Network output and result. A valid proof
for another input, internal output, rejected result or modified wire cannot become
a successful settlement. The full genuine native proof test passes explicitly;
it is not counted through the harness's default ignored-test behavior. Header and
transaction-query fixtures retain their assertions under the current canonical
output schema, and raw geometry capacities retain their typed Bytes boundary.

Native execution uncovered a real staging defect: the default Kagami path and
CLI genesis fixture configured Nexus on blank Kura before authenticating network
geometry. Both now use temporary configured Kura, install validated policy,
prepare the primary geometry anchor, restore authenticated storage and install
Nexus through the configuration boundary. Configured signing keeps the same
sequence. Default public-profile staging also used generic stake/fee assets after
bootstrap had selected canonical XOR. It now uses the same validated signed alias
binding as bootstrap; generic defaults and public alias rejection remain intact.
No compatibility branch or storage guard bypass was added.

The private config fixtures now retain their temporary directory, use the native
private-directory/file owners, and bind complete fixture account identities to the
manifest's chain and discriminant. The intended configured-XOR mismatch assertion
now executes after real config admission. Production foreign-prefix and filesystem
custody rejection are unchanged. Native Taira localnet/devnet generation already
supplies the four explicit bindings required by the new producer.

The maintained clean-client network caller no longer writes lane.json or passes
the retired flag. It independently compares the generated typed committee and
selected endpoints with signed-genesis bindings, compares the saved deployment
intent with CLI output, and compares planned manifest additions with that source.
The existing signed-genesis mapping test retains its selected name and now checks
substituted accounts, peers, endpoints, missing members and wrong quorum as well.

Validation and failures are retained rather than overwritten:

- The model's four schema/roundtrip controls pass in model-manifest37.log.
- Compile37 exposed merged canonical-output/header API fallout; compile40 caught
  one redundant generic string conversion. Compile41 then succeeded in 39.38s.
- Run41 passed 53 manifest, 13 geometry, one contract and 13 ordinary finality
  controls. Dataspace 33/79 and public inputs 1/11 passed; their shared Kura setup
  correctly failed the new network-binding guard. One expensive proof was ignored.
- The unified Core/CLI/Kagami run42 compiled in 8m41s. All 66 Core manifest/geometry,
  11 public-input, one contract and 14 finality tests pass. The latter includes the
  genuine proof and completed in 186.216s. Dataspace 64/79 exposed noncanonical
  generated JSON; Kagami 4/5 exposed the public default asset mismatch.
- Run43 compiled in 16.18s and passes all 79 dataspace tests and seven selected
  Kagami controls. One additional negative config fixture failed private-file
  admission before its intended assertion. Runs44/45 exposed its directory-mode
  and account-prefix omissions; run46 passes all three affected fixture consumers.
  Ten distinct selected Kagami controls now pass, preserving configured and default
  signing, final-identity context parity and intended public/generic asset guards.
- All eight historical strict-replay failures now pass on the merged Core42
  executable in 46.252s. That Core artifact is unchanged in builds43/44; those
  earlier fixture-publication failures no longer describe current replay behavior.
- The maintained release runner passes 172 Python tests. Both scopes require the
  four new typed producer controls and two default/public genesis controls. Exact
  selection counts are macOS 1015/1193 and Linux 1020/1198. Missing named tests fail.
- Scoped formatting, diff checks and the retired-codec guard pass. The updated
  network consumer compiles with Core/CLI/Kagami and the complete Taira consensus
  harness in consumer47 (3m55s), on unchanged source. Its new mapping assertions
  are compile-checked but not yet executed in that harness. No full four-validator
  run or shipping qualification is inferred from these component checks.

Evidence is retained under target/dpn-devex: model-manifest37.log,
manifest40–46 build/source/result records, core42-results.json,
cli42-results.json, replay42-results.json, taira-release-tests43.log and codec44.log.
Compilation reuses the canonical checkout's dpn-devex warm target with six jobs;
no Cargo/rustc process was interrupted and no fresh build lane was created.

At 13:27:56 UTC on September 19, primary status/faucet-policy routes and ports
8444–8446 returned HTTP502. Port8443 served both routes but reported source
`d085418a382e831875731144436c174d8621cdd6`, height1969, zero peers and one queued
transaction. This public observation is not authenticated as the historical
height3598 ledger and does not authorize an alternate deployment target. The
approved MacStadium login target remains requested; no credentials were guessed.
No validator restart, live write, empty production block or ledger replacement
was performed. Actual original Validate-to-Apply custody, complete pre-vote
resource admission, required Linux four-validator recovery and clean-client
qualification, and public DPN deployment remain open under the active goal.


## Superseded current DPN status, preserved verbatim

Current DPN recovery work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Build31 passes 120 MV library tests, five actual-allocation/epoch controls and 75 focused Core publication, custody and lock-order controls on unchanged source and executable. Strict MV Clippy, formatting and the charged-API compile-fail control pass. Original Cell current/undo allocations now survive detach, contention, abort and publication without successor reconstruction; their typed charges stay until actual EBR reclamation. A finite requested-layout credit pool reserves complete demands and returns capacity only after actual release. Cell and Storage also wake already-registered retries when raw writer acquisition panics before returning its guard. State fields still use untracked mode: complete B+tree/cursor/nested allocation custody and a configured aggregate policy remain required. The private retained-validation service retains original candidates across marker refusal, retries and selection abort, but the live scalar validator still drops its carrier and Apply reexecutes. Complete original Validate-to-Apply resources, source-release/Native durability and the consuming publisher remain required under N0. Eight strict replay publication controls, positive pending-Kura and unchanged four-validator Linux restart gates, clean-client qualification and DPN deployment remain incomplete. At 11:09 UTC on September 19 the primary ingress and validator roots 8444–8446 returned HTTP 502; validator root 8443 served funding policy but reported source42, height 1969 and zero peers. That observed state is not authenticated as the historical height-3598 ledger. No live mutation or shared-ledger replacement was performed. Detailed outcomes and superseded status prose are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Superseded current P9 row, preserved verbatim

| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Preserve release94's completed public reset, same-source Linux preparation, convergence/canaries/restarts and public Host routing; finish test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Qualify the maintained native public-input/profile export and `dataspace-deploy init/plan/apply/status` path for physical DPN and paid `dpn`/`admin@dpn`, including bounded fees, once-only submission, independently anchored inclusion and all-four StateApplied. Require selector-bound typed absence and preserved alias diagnostics through Torii, native SDK/MCP readers and OpenAPI, plus one inherited wait deadline with no zero-budget dispatch or late finality and exact typed terminal-failure evidence. Run configuration and selected-harness metadata/codegen before focused acceptance, then the unchanged full release gate; retain composed FD, inventory, shared-deadline transport and stage-consumer checks and submitted ambiguity in one native apply recovery journal. Retain the qualified core onboarding/faucet/final-canary transport lifecycle in future rollouts. Recover the source42 first-epoch beacon incident without weakening finalized history or pulse validation; supply a production native key-session and per-seat signer bootstrap, prove its committed activation before the first required pulse, and make missing beacon readiness fail deployment qualification early. Retain source46’s passing four-peer recovery fixture; qualify bounded finality-read HTTP 429 retries, production beacon custody/bootstrap/readiness, authenticated height observation and maintained controller integration before DPN activation. Resolve the exact retained linked pending-Kura Apply owner and qualify its original-body four-stage restart through complete State publication. Run the mandatory pending-Kura recovery group first and stop before other startup/release work on failure; complete N0’s consuming publisher before claiming ordinary fresh/recovery Apply works. Resolve retained-height-3598 recovery separately; no ledger replacement is authorized. Public faucet policy discovery is deployed, but ingress and chain health must be freshly verified; keep funding authority independently pinned. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |

## Superseded handoff paragraphs, preserved verbatim

The DPN development continuation now retains the original raw and tiered geometry
attempts in State across local refusal, including replay State/receipt custody.
Build17 passes all 170 geometry controls. Build19 compiles and passes all 76
selected local-refusal, Queue-release, autoscale, carrier and recovery-fixture
controls on unchanged source and executable. Typed drain/storage failures remain
local through candidate validation and emit no rejection event; deterministic
errors still reject. Autoscale evaluation preserves its original sample/count
across refusal and marks lifecycle completion only after success. Eight strict
replay controls remain failed at fixture publication because the complete
consuming publisher is missing; they are not included as passes in these later
selections. See the [dated DPN record](../docs/history/2026-09-19/dpn-recovery-geometry.md)
for the preceding failed builds and exact validation scope. The carrier now
resumes its original geometry under the retained Kura lease; local backend
contention releases all physical ownership before returning its actual release
observation. Complete pre-vote resource admission and source/Native authority
remain required before exposing the production consumer. Build28 separately passes
43 focused controls for original CatalogPublished completion, terminal journal
identity, retained DA/lifecycle projections, cursor persistence after generation
close, corrected DA fixtures and lock ordering. Actual index allocation and
complete resource/custody handoff remain unfinished. Build24 passes all 289
selected Core controls; all 105 MV tests and strict MV library/test Clippy also
pass. Original archive captures and their detached carrier survive insertion
refusal. The canonical runtime writer is acquired before State fences, preserving
undo and avoiding the prebuilt-block lifecycle deadlock. A private complete
decision-plus-lease owner now joins sealed wire, original witness commitment and
exact original Native admission evidence before State acquisition. Its pure
retained-evidence reads preserve body/hash/query caches. This immutable source
join grants no source release or State publication authority.

The exact locked Concread source now has an EBR allocation-custody hook: admission
precedes cloning, the writer owns its allocated generation, commit moves that
allocation, and actual reclamation destroys/deallocates it before returning its
charge. Unrelated epoch pins delay both; clone/destructor unwind conservatively
retains the charge. Build29 passes 105 MV tests, four allocator/epoch controls and
two negative API checks. It passes 75/76 selected Core controls; build30 closes the
original store before the reopened-incarnation fixture and passes all six final
custody controls on unchanged production source. Strict MV Clippy passes. Build31 adds original current/undo MV Cell ownership through detach, retry, abort
and publication, a finite requested-layout credit pool, and raw-acquisition panic
wakes for Cell and Storage. All 120 MV library tests, five allocator/epoch tests,
75 focused Core controls, strict MV Clippy and the charged-API compile-fail control
pass on unchanged source. Existing State fields remain untracked. Original B+tree
successors, complete node/cursor/nested payload custody and an explicit configured
aggregate policy remain required before activating the live handoff.


## Concurrent merge boundary and final consumer check

The separate merge task committed its merge as `c8eae6b521cea6035fc34be10b403303ef771cb1` before DPN development run43. Runs43–46 and consumer47 capture that HEAD on `optimizations`; their indicated build boundaries are unchanged. Core42 remains byte-identical in the subsequent native build records, and the CLI43 artifact is retained through the later Kagami-only corrections. Consumer47 successfully checks the selected Core/CLI/Kagami leaves, test-network library and complete Taira consensus harness on unchanged source in 3m55s. The expanded clean-client mapping control is compile-checked, not yet executed in that harness. This task did not commit or modify the concurrent task's Git state. Scoped formatting, diff and codec checks pass. Historical verification reports 64,736 records and 67,311 occurrences; current status/roadmap remain 299/137 lines.


## Native consumer and current recovery qualification, runs48–49

Consumer48 builds the selected Core, CLI, Kagami, test-network library and complete Taira consensus harness in the existing warm `dpn-devex` lane in 8m28s. Cargo reports success with zero compiler errors; before/after source observations match at `c8eae6b521cea6035fc34be10b403303ef771cb1` on the canonical `optimizations` checkout. The emitted test-network artifact executes exactly `dataspace_deploy_cli::signed_genesis_validator_mapping_preserves_runtime_accounts`: one pass, zero ignored, 0.099 seconds, and unchanged executable SHA256. This closes the preceding compile-only consumer qualification; it does not execute a four-validator deployment.

Pending48 first passes all six `pending_kura_` controls on the retained Core42 executable. The combined consumer48 graph emits a different Core artifact, so run49 repeats the relevant current recovery checks on that exact artifact: all six pending-Kura controls pass in 9.951 seconds and all eight strict-replay controls pass in 44.452 seconds, with unchanged executable SHA256. The sixth control verifies foreign Decision authority and exhausted reducer fences reject without mutating the original marker, reducer, registry, progress or WAL. The maintained early recovery gate now requires it in both scopes/platforms; 172 Python tests and 1,594 subtests pass. Exact release census totals are macOS 1016/1194 and Linux 1021/1199 for basic/full. The current retained scoped union has 200 distinct native controls; repeated recovery execution is not double-counted or relabelled full release qualification.

A production ownership audit confirms `V2ApplyService::validate_candidate` still drops its `PreparedCarrier` and returns a scalar commitment. Worker validation, cached/reproposal receipts and recovered startup therefore do not retain production journals; unfinished WSV Apply reexecutes. The private consuming State publisher already exists. Its remaining lifecycle/geometry closure must retain the original Queue identity, recheck late authenticated relay and certified-block evidence under held owners, complete original raw/tiered geometry across retry, publish lifecycle effects before same-carrier DA effects, and persist their cursor continuations before releasing lifecycle custody. Participant durability separately needs evidence authentication under the final aggregate Kura lease. Removing either refusal without those owners, or changing only fresh validation while leaving cache/recovery scalar, would not close the root cause. Aggregate production resource admission remains required. The current raw State commit does support authenticated finalized outputs; the earlier blanket-refusal diagnosis is superseded.

The fresh read-only public observation remains degraded: primary status/funding return502; validator root8443 returns200 but reports source42, height1969, zero peers and queue1. No transaction, validator restart or ledger replacement is attempted. The approved local `iroha-linux-validation` Colima guest initially lacked Rust and mounted the canonical source read-only. Its exact source mount has now been made writable while preserving other settings, with no source clone; pinned Linux toolchain/cache preparation is separate local setup, not public validator recovery or completed Linux qualification.

Evidence remains under `target/dpn-devex`: `consumer48-build-result.json`, paired `consumer48-source-*.json`, `consumer48-test-artifact.json`, `consumer48-mapping-result.json`, `pending48-results.json`, `recovery49-artifact.json`, `recovery49-results.json`, and `public-health48.json`.

## Superseded DPN status before runs48–49, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native dataspace initialization now generates a closed typed manifest from the selected signed genesis and exact four-peer profile; handwritten manifest input is removed. Core and CLI share bounded semantic validation, and generated/loaded intent retains the exact account, peer and endpoint bindings. Default genesis signing now authenticates temporary Kura geometry before Nexus installation and uses the signed public XOR binding for staking and fees. Finality verification consumes the canonical authenticated input/output proof. Focused validation passes four model, 74 Core, 105 CLI and ten Kagami controls, including all 79 deployment tests and the explicitly executed genuine finality proof; all 172 maintained runner tests pass. The eight historical strict-replay publication failures now pass on the merged Core artifact. The maintained clean-client caller uses and independently checks generated input; its current compile result is recorded in the dated evidence. These are scoped results: original live Validate-to-Apply custody, complete pre-vote resource admission, pending-Kura/four-validator Linux recovery, clean-client completion and actual DPN deployment remain unfinished. Earlier original B+tree/MV ownership passes qualify their frozen parent source, not the complete merged release. At 13:27 UTC on September 19, the primary funding/status routes and validator roots 8444–8446 returned HTTP 502; root 8443 served policy but reported source42, height 1969 and zero peers. This does not authenticate the historical height-3598 ledger or establish healthy finality. No live write, validator restart, empty production block or shared-ledger replacement was performed. Detailed source-scoped outcomes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Superseded P9 before runs48–49, preserved verbatim

| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Preserve release94's completed public reset, same-source Linux preparation, convergence/canaries/restarts and public Host routing; finish test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Qualify the maintained native public-input/profile export, typed genesis-derived manifest generation and `dataspace-deploy init/plan/apply/status` path for physical DPN and paid `dpn`/`admin@dpn`, including bounded fees, once-only submission, independently anchored inclusion and all-four StateApplied. Require selector-bound typed absence and preserved alias diagnostics through Torii, native SDK/MCP readers and OpenAPI, plus one inherited wait deadline with no zero-budget dispatch or late finality and exact typed terminal-failure evidence. Run configuration and selected-harness metadata/codegen before focused acceptance, then the unchanged full release gate; retain composed FD, inventory, shared-deadline transport and stage-consumer checks and submitted ambiguity in one native apply recovery journal. Retain the qualified core onboarding/faucet/final-canary transport lifecycle in future rollouts. Recover the source42 first-epoch beacon incident without weakening finalized history or pulse validation; supply a production native key-session and per-seat signer bootstrap, prove its committed activation before the first required pulse, and make missing beacon readiness fail deployment qualification early. Retain source46’s passing four-peer recovery fixture; qualify bounded finality-read HTTP 429 retries, production beacon custody/bootstrap/readiness, authenticated height observation and maintained controller integration before DPN activation. Resolve the exact retained linked pending-Kura Apply owner and qualify its original-body four-stage restart through complete State publication. Run the mandatory pending-Kura recovery group first and stop before other startup/release work on failure; complete N0’s consuming publisher before claiming ordinary fresh/recovery Apply works. Resolve retained-height-3598 recovery separately; no ledger replacement is authorized. Public faucet policy discovery is deployed, but ingress and chain health must be freshly verified; keep funding authority independently pinned. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |


## Linux setup and maintained gate repair, runs50–56

The existing local `iroha-linux-validation` guest uses the same canonical source mount, Rust 1.93.1, and one fixed Linux build lane. No source clone, live validator restart or shared-ledger replacement occurred. Local permission and fixture-custody prerequisites pass. Native compilation is pending after metadata; this is development evidence, not an immutable signed release.

Linux50 stopped before Cargo on a stale source-asset inventory pin. Reviewed asset changes preserve all 55 case identities; additional adverse mutation controls preserve the preceding controls. The separate merge task resolved the subsequent asset-pin conflict and the user committed `482a02fa708fb83f00b3cadd4067ad8f0b641b49`. Linux52 passes the pure FSM (224), source inventory and six lifecycle source assertions, then fails dependency metadata in ICU. Investigation authenticates nine archives and 485 extracted files, finds all original dependency metadata intact, passes ten metadata-import probes and directly recompiles the ordinary ICU library against those same artifacts. A separate upstream lib-test check is not treated as a reproduction or a pass. Linux54 repeats the exact selected configuration/Core/network metadata graph successfully in 292.494 seconds with zero errors and unchanged source at the new merge. The original dependency failure remains unreproduced; no unproven filesystem or corruption diagnosis is claimed.

The general native release inventory now selects the renamed local-capacity refusal control without changing its 531 rows. Reviewed preemption seals match the retained-owner projection implementation; five adverse guard mutations remain rejected, and both focused regression checks pass. Taira basic/full selections now use all five canonical output inclusion tests and the renamed full-only frontier preservation control. No aliases or coverage deletions were added. All 172 maintained runner tests and 1,594 subtests pass in 1.85 seconds; the source audit finds no additional definite stale names, while compiled harness enumeration is still required. Evidence is retained in `target/dpn-devex/linux54-result.json`, `linux52-icu-diagnostic/diagnosis-summary.json`, `native-capacity-selector53-inventory.log`, `preemption-seal55-focused-tests.log` and `canonical-selector56-tests.log`.

## Superseded DPN status before runs50–56, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native dataspace initialization generates a closed typed manifest from the selected signed genesis and exact four-peer profile; handwritten manifest input is removed. Core and CLI share bounded semantic validation, and generated/loaded intent retains exact account, peer and endpoint bindings. Default genesis signing authenticates temporary Kura geometry before Nexus installation and uses the signed public XOR binding for staking and fees. Finality verification consumes the canonical authenticated input/output proof. Retained scoped evidence covers four model, 80 Core, 105 CLI, ten Kagami and one test-network control, including all 79 deployment tests and the explicitly executed genuine finality proof. The combined native consumer48 build passes on unchanged source; its generated-manifest mapping regression, all six pending-Kura recovery controls and eight strict-replay controls pass. The sixth recovery control is now mandatory before startup/release work; all 172 runner tests and 1,594 subtests pass. These results do not qualify the complete release: original live Validate-to-Apply custody, aggregate pre-vote resource admission, four-validator Linux recovery, clean-client completion and actual DPN deployment remain unfinished. The existing consuming publisher still needs lifecycle/geometry and participant durability ownership joined to the production path; ordinary raw State publication is not a blanket refusal. Earlier original B+tree/MV ownership passes qualify their frozen parent source. At 13:53 UTC on September 19, primary funding/status routes returned HTTP 502; validator root 8443 served policy but reported source42, height 1969, zero peers and one queued transaction. This does not authenticate the historical height-3598 ledger or establish healthy finality. No live write, validator restart, empty production block or shared-ledger replacement was performed. Detailed source-scoped outcomes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Superseded P9 before runs50–56, preserved verbatim

| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Preserve release94's completed public reset, same-source Linux preparation, convergence/canaries/restarts and public Host routing; finish test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Qualify the maintained native public-input/profile export, typed genesis-derived manifest generation and `dataspace-deploy init/plan/apply/status` path for physical DPN and paid `dpn`/`admin@dpn`, including bounded fees, once-only submission, independently anchored inclusion and all-four StateApplied. Require selector-bound typed absence and preserved alias diagnostics through Torii, native SDK/MCP readers and OpenAPI, plus one inherited wait deadline with no zero-budget dispatch or late finality and exact typed terminal-failure evidence. Run configuration and selected-harness metadata/codegen before focused acceptance, then the unchanged full release gate; retain composed FD, inventory, shared-deadline transport and stage-consumer checks and submitted ambiguity in one native apply recovery journal. Retain the qualified core onboarding/faucet/final-canary transport lifecycle in future rollouts. Recover the source42 first-epoch beacon incident without weakening finalized history or pulse validation; supply a production native key-session and per-seat signer bootstrap, prove its committed activation before the first required pulse, and make missing beacon readiness fail deployment qualification early. Retain source46’s passing four-peer recovery fixture; qualify bounded finality-read HTTP 429 retries, production beacon custody/bootstrap/readiness, authenticated height observation and maintained controller integration before DPN activation. Retain the six passing pending-Kura recovery controls and qualify their original-body four-stage restart in the unchanged four-validator Linux corridor. Run all six mandatory recovery controls first and stop before other startup/release work on failure. Complete N0’s original Validate-to-Apply custody, aggregate resource admission and consuming publisher lifecycle/participant ownership before claiming one original execution through publication. Resolve retained-height-3598 recovery separately; no ledger replacement is authorized. Public faucet policy discovery is deployed, but ingress and chain health must be freshly verified; keep funding authority independently pinned. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |


## Superseded DPN status and P9 before runs57–60, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native dataspace initialization generates a closed typed manifest from the selected signed genesis and exact four-peer profile; handwritten manifest input is removed. Core and CLI share bounded semantic validation, and generated/loaded intent retains exact account, peer and endpoint bindings. Default genesis signing authenticates temporary Kura geometry before Nexus installation and uses the signed public XOR binding for staking and fees. Finality verification consumes the canonical authenticated input/output proof. Retained native evidence at the preceding `c8eae6b521` source boundary covers four model, 80 Core, 105 CLI, ten Kagami and one test-network control, including all 79 deployment tests, the genuine finality proof, six pending-Kura recovery controls and eight strict-replay controls. Those 200 scoped passes do not qualify the later merged runtime. At current `482a02fa70`, Linux metadata validation passes all selected configuration, Core and test-network targets on unchanged source. Reviewed source inventory/seal updates and six stale selector repairs preserve the required test census; their focused checks pass. The first Linux dependency metadata failure is unreproduced against intact original artifacts, so no corruption cause is asserted. The sixth recovery control is now mandatory before startup/release work; all 172 runner tests and 1,594 subtests pass. These results do not qualify the complete release: original live Validate-to-Apply custody, aggregate pre-vote resource admission, four-validator Linux recovery, clean-client completion and actual DPN deployment remain unfinished. The existing consuming publisher still needs lifecycle/geometry and participant durability ownership joined to the production path; ordinary raw State publication is not a blanket refusal. Earlier original B+tree/MV ownership passes qualify their frozen parent source. At 13:53 UTC on September 19, primary funding/status routes returned HTTP 502; validator root 8443 served policy but reported source42, height 1969, zero peers and one queued transaction. This does not authenticate the historical height-3598 ledger or establish healthy finality. No live write, validator restart, empty production block or shared-ledger replacement was performed. Detailed source-scoped outcomes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Preserve release94's completed public reset, same-source Linux preparation, convergence/canaries/restarts and public Host routing; finish test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Qualify the maintained native public-input/profile export, typed genesis-derived manifest generation and `dataspace-deploy init/plan/apply/status` path for physical DPN and paid `dpn`/`admin@dpn`, including bounded fees, once-only submission, independently anchored inclusion and all-four StateApplied. Require selector-bound typed absence and preserved alias diagnostics through Torii, native SDK/MCP readers and OpenAPI, plus one inherited wait deadline with no zero-budget dispatch or late finality and exact typed terminal-failure evidence. Retain the passing merged-source Linux configuration/Core/network metadata preflight; complete native codegen, all six pending-Kura controls, generated-manifest mapping and the production four-validator supervisor restart before the unchanged full release gate; retain composed FD, inventory, shared-deadline transport and stage-consumer checks and submitted ambiguity in one native apply recovery journal. Retain the qualified core onboarding/faucet/final-canary transport lifecycle in future rollouts. Recover the source42 first-epoch beacon incident without weakening finalized history or pulse validation; supply a production native key-session and per-seat signer bootstrap, prove its committed activation before the first required pulse, and make missing beacon readiness fail deployment qualification early. Retain source46’s passing four-peer recovery fixture; qualify bounded finality-read HTTP 429 retries, production beacon custody/bootstrap/readiness, authenticated height observation and maintained controller integration before DPN activation. Retain the six passing pending-Kura recovery controls and qualify their original-body four-stage restart in the unchanged four-validator Linux corridor. Run all six mandatory recovery controls first and stop before other startup/release work on failure. Complete N0’s original Validate-to-Apply custody, aggregate resource admission and consuming publisher lifecycle/participant ownership before claiming one original execution through publication. Resolve retained-height-3598 recovery separately; no ledger replacement is authorized. Public faucet policy discovery is deployed, but ingress and chain health must be freshly verified; keep funding authority independently pinned. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |

## Completion retry and runner corrections, runs57–60

The finality prefix is retained only within one locked invocation and remains bound to its plan, authenticated genesis authority and original deadline. Every retry still checks the canonical disk bytes and obtains fresh challenge-bound peer/state/carrier observations. Only exact previously admitted proofs skip repeated cryptographic verification; new proofs advance the retained verifier only after validation, durable publication and the deadline check. Six new mandatory controls cover bounded batches and pending retries, disk mutation/deletion and alternate witness replacement, fresh-owner reauthentication, malformed successors, publication/deadline failure and lower-tip conflicts. The CLI release census increases by six; no historical selector or assertion is removed.

CLI58 test-configuration metadata passed in 266 seconds. CLI59 built in 391 seconds with unchanged source, then reported 79 passes and six failures at strict genesis execution-result validation. The new tests had reused a raw signed proposal fixture. The correction reuses the existing executed genesis with its exact manifest/key/hash binding and asserts the four actual signing keys; production genesis validation is unchanged. CLI60 rebuilt only the CLI in 18.32 seconds and passed all 85 deployment tests in 6.681 seconds, zero failures/ignored. Its before/after source observations match at `482a02fa708fb83f00b3cadd4067ad8f0b641b49`, working-diff SHA256 `2c0cc6ea80bd36e678e54f373c99d77966061b3ecf120557d4b032c716841265`. The test executable SHA256 remains `91ab30e03effd3e883dadac0f736d1d1c149b1b960897c18b65c142208d7d90e` across execution. These are focused development results, not full release or live-network qualification. Evidence: `target/dpn-devex/cli59-native.json`, `cli60-native.json`, their native logs and source observations.

Both maintained development check entrypoints now reject missing/nonexecutable fixed-path lsof before compilation. The local Linux guest prerequisite was installed and its closed/open/closed probes pass. The explicit Linux-only `--native-linker llvm` option authenticates the installed clang18 and ld.lld18 paths and custody, reports their identities, and preserves the default system linker and authenticated preparation environment. A linker selection change invalidates Cargo fingerprints; no zero-rebuild or measured performance improvement is claimed. The actual combined Python suites pass 268 tests and 1,731 subtests. Focused formatting, codec policy and diff checks pass. See `combined58-python.log`, `format58.log`, `codec58.log` and the maintained release documentation.

Linux57 continues using its already imported runner and GNU linker while subsequent CLI/runner changes are recorded explicitly. It is a mixed-source diagnostic and cannot provide an unchanged-whole-source release seal. Core/config/network Rust inputs remain frozen. The linker is CPU-active, not waiting on Cargo's build lock. The request to stop only this run and switch linkers is unanswered; no Cargo/rustc process was interrupted. Its six recovery tests, shipping binary build, mapping test and real four-validator supervisor/paid DPN corridor have not yet executed. Primary status and faucet-policy GETs returned 502 at 2026-09-19T15:22:48Z (`public-health59.json`). No approved operator route was found in maintained repository inputs, and the pending access question remains unresolved. No live mutation or ledger replacement occurred.


## Superseded DPN status before the bounded linker diagnostic, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native initialization derives the closed typed manifest and exact four-validator bindings from authenticated signed genesis; Core and CLI share bounded manifest semantics, and canonical finality input/output proofs remain mandatory. At merged `482a02fa70` with the reviewed working changes, all 85 native deployment tests pass, including six new proof-prefix controls. One locked completion invocation now retains authenticated prefix state across pending retries while rereading canonical disk proofs and obtaining fresh peer evidence; new invocations reauthenticate disk. CLI59 exposed an invalid resultless genesis fixture; CLI60 reuses the existing genuinely executed fixture and passes all 85 tests with zero ignored, unchanged build source and unchanged executable identity. Its warm rebuild took 18.32 seconds. The maintained runner now checks fixed-path `lsof` before compilation and offers explicit verified LLVM 18 linking only for Linux development checks; system linking, macOS Apple ld and authenticated release preparation remain unchanged. All 268 runner tests and 1,731 subtests pass. LLVM performance has not been measured. Linux57 remains in CPU-active GNU linking before native recovery/network execution; subsequent CLI/runner edits make it a mixed-source development diagnostic, not unchanged-source release qualification. Earlier 200 native passes qualify their preceding `c8eae6b521` source only; selected Linux metadata passes qualify merged `482a02fa70`. Complete four-validator Linux recovery, clean-client completion and actual DPN deployment remain unfinished. N0 original Validate-to-Apply custody, aggregate resource admission and consuming-publisher lifecycle/participant ownership remain separate unfinished outcomes; ordinary raw State publication is not a blanket refusal. Primary public status and funding-policy routes still return HTTP 502 at 15:22 UTC on September 19. The approved operator SSH route is not available; no public write, validator restart, empty production block or ledger replacement was performed. Detailed source-scoped outcomes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Bounded same-input LLVM link diagnostic

The first separate LLVM18 invocation reached output allocation in 11.78 seconds but failed its 1 GiB file-size cap. Its incomplete output was not retained, and no successful benchmark was claimed. A fresh immediate physical-storage check then admitted one separate 2 GiB-capped attempt, requiring at least 12 GiB free to preserve an 8 GiB reserve plus twice the output cap. That attempt linked a 1,229,502,504-byte AArch64 ELF in 12.133 seconds, exit zero, peak child RSS 6,726,688 KiB. All 403 input identities were unchanged. Inputs were ordinary ELF; GCC LTO plugin/resolution arguments were removed, the tiny ephemeral symbol object was copied privately, six LLVM threads were selected and the original Cargo output was excluded. Original GNU ld remained active after more than an hour; no Cargo/rustc/original-linker process was signaled. This is a separate-output development timing diagnostic, not the maintained Cargo LLVM workflow, immutable release qualification, or test execution. The diagnostic executable was not run. Evidence: `target/dpn-devex/llvm-link-benchmark57/result.json` retains the cap failure; `llvm-link-benchmark60/{plan.json,started.json,result.json}` retains the successful bounded attempt. The pending stop/switch question remains unanswered.


## Superseded Linux linker default and runner counts, preserved verbatim

The maintained runner now checks fixed-path `lsof` before compilation and offers explicit verified LLVM 18 linking only for Linux development checks; system linking, macOS Apple ld and authenticated release preparation remain unchanged. All 268 runner tests and 1,731 subtests pass.

Benchmark the explicit Linux development LLVM option before claiming a build-time improvement; preserve default Apple ld, fixed lsof preflight and authenticated release boundaries.

## Linux development default, run61

Both maintained check entrypoints and direct development calls now default to verified LLVM18 on Linux. Missing or nonexecutable clang18/lld18 stops before native gate execution and reports the exact prerequisite; there is no automatic GNU fallback. Explicit system selection remains available for diagnosis. macOS Apple ld and authenticated preparation/native-check environment implementations are unchanged. Independent review found no actionable concern. The reviewed five-file proposal added platform/default, missing-tool, explicit-system and preparation-boundary coverage; all 270 actual macOS tests and 1,755 subtests pass in 7.35 seconds. The Linux guest executes all 270 cases from the same actual two source files via unittest: 268 passes, two expected macOS-only fclonefileat skips, no failures/errors, 2.20 seconds. Source hashes are unchanged during that run. The actual installed-tool preflight confirms that the Linux default selects the verified compiler/linker without invoking Cargo. This does not change the already imported GNU Linux57 controller, qualify a Cargo rebuild under LLVM, or restart any validator. Rust sources are unchanged from the CLI60 85-test pass. Evidence: `target/dpn-devex/linux-default61-python-macos.log`, `linux-default61-python-linux.{log,json}`, `linux-default61-installed-preflight.log` and the before/after source observations.


## Superseded Linux execution status before runs62–63, preserved verbatim

Linux57 remains in CPU-active GNU linking before native recovery/network execution; subsequent CLI/runner edits make it a mixed-source development diagnostic, not unchanged-source release qualification.

## Scoped Linux runtime diagnostics, runs62–63

A new development-only step executes the separately linked LLVM Core ELF as a test harness. The earlier linker diagnostic had not executed it. The six exact maintained pending-Kura recovery selectors now all pass with zero ignored in 7.762 seconds total native test time. Tests use temporary Kura/WAL/body/ledger storage, in-process state and bounded in-memory actor queues; crash injection returns an error and sends no OS signal. Each selected process has a separate 120-second diagnostic bound without changing the internal recovery deadline. The executable is made owner-read/execute-only and remains SHA256 `6f837b8f5916797fd148550ab597b1a8c2f25d1feed2e74c4ffebe5b32fbe8cc` across execution. The whole source observation also remains unchanged at `482a02fa708fb83f00b3cadd4067ad8f0b641b49`, working-diff SHA256 `5f3145815f9eac7ca3ac0b9a46d189931c9d0744171044767101b2f9797e6da0`. Core source has no working diff from that HEAD. Evidence: `target/dpn-devex/recovery62-result.json`, six individual logs, and source/artifact observations. This supplies focused Linux runtime evidence, not a maintained Cargo/release checkpoint or original Validate-to-Apply custody proof.

The already emitted Linux configuration and consensus contract executables are independently copied into an owner-private diagnostic directory. Their completed Cargo fingerprints/timestamps align with Linux57 observations, but its batch remains incomplete. The maintained `run_stages` and exact-pass checks execute four `CONFIG_STAGES` selectors and `dataspace_deploy_cli::signed_genesis_validator_mapping_preserves_runtime_accounts`. All five pass with zero ignored; no peer, CLI, daemon or live network is started. Source and copy hashes remain unchanged. Config SHA256 is `61247354782a9397fb34650b3917e3cfbd33dca90ea190e87bb21818a0164ad4`; network-contract SHA256 is `9c1822c67afa185135a1be81223889c0ba27d1cfd3f171fd0acb229b33eb9c9f`. Evidence: `target/dpn-devex/consumer63/result.json`, `consumer63-run.log` and its source observations. These five plus the six recovery controls are eleven scoped Linux diagnostic passes; they are not added to historical C8 totals or treated as full release qualification. The selected Linux shipping outputs (`iroha3d`, `iroha3d_taira`, `iroha`, `kagami`) are absent, so the production supervisor restart and paid clean-client corridor cannot yet execute. The original GNU process is still active and no Cargo/rustc process was signaled. At 15:54 UTC on September19, primary status and funding-policy reads both still returned502 (`public-health61.json`). Approved operator access and the pending stop/switch answer remain unavailable.

Superseded P9 clause before runs62–63, preserved verbatim:

Retain the passing merged-source Linux configuration/Core/network metadata preflight; complete native codegen, all six pending-Kura controls, generated-manifest mapping and the production four-validator supervisor restart before the unchanged full release gate;


## Superseded status clauses before runs64–65, preserved verbatim

All 270 macOS runner tests and 1,755 subtests pass; Linux passes 268 cases with only two expected macOS-only clone-test skips.

Separate Linux diagnostics pass six pending-Kura recovery, four configuration and one validator/account mapping control with unchanged source and test executables.

Primary public status and funding-policy routes still return HTTP 502 at 15:22 UTC on September 19.

Retain the passing merged-source Linux metadata preflight and eleven separate runtime diagnostics (six recovery, four configuration, one mapping);

## Early network prerequisites and observation-only dispatch, run64

The maintained runner now partitions exact selected test names instead of comparing whole stage tuples. All observation subsets execute together before shipping codegen; an observation failure aggregates selected failures and stops before any node build. Observation-only selections require neither shipping binaries nor the four-peer storage floor. Both gate entrypoints admit selected beacon roots before source audits or compilation, including every direct ancestor's permissions and Git boundary; runtime custody checks remain. Independent review found no missing selector or weakened runtime gate. Ten initial focused Python checks passed. The actual retained Linux consensus harness then passed all 14 observation selectors with zero ignored through `run_network_checks`, in 1.303 seconds, without shipping compilation or validator startup. Executable SHA256 remained `9c1822c67afa185135a1be81223889c0ba27d1cfd3f171fd0acb229b33eb9c9f`; source before/after diff SHA256 remained `051dd31ff8bcd55760f78f27e00870e02fbcbcff057ca31617346237474d68cb`. Evidence: `target/dpn-devex/network64-native.{json,log}`, source observations and `network64-linux-prerequisites.log`. The same actual Linux external root/capacity preflight passes. This is a scoped development diagnostic, not completion of the original Cargo batch or release qualification.

## Same-timestamp artifact content revalidation, run65

The initial actual macOS runner pass was 281 cases plus 1,778 subtests. Linux exposed one existing same-size source-mutation regression: the consumer had already read the original bytes, so its copy hash was correct while post-stream metadata could miss persistent source changes. A direct Linux filesystem probe observed identical mtime/ctime for 118 of 128 changed-content writes (`artifact65-timestamp-probe.json`). The original failing test is unchanged. Shared `stable_open_relative` now rehashes the pinned descriptor through bounded `pread` calls (captured size plus one growth byte), compares the captured digest, preserves stream offsets and retains every metadata/path check. Seven new deterministic controls cover timestamp collisions, partial consumption/offsets, chunk bounds, truncation/growth and late metadata/path changes. Independent consumer review confirms archive builders and native capture remain compatible; this does not claim protection against arbitrary transient modify-and-restore races beyond the existing stream hashes.

After the fix, both actual runner files pass on Linux: 281 cases, 279 passes, two expected macOS-only clone skips, zero failures/errors. A six-file macOS run includes both runners, the shared contract and tar.gz/tar.zst/OCI consumers: 372 cases and 1,778 subtests pass; 26 bundle/OCI cases fail before reaching archive logic because the trusted source checksum is stale. Source before/after remains `482a02fa708fb83f00b3cadd4067ad8f0b641b49`, diff SHA256 `e011678bd6363ce1b3a82b1f1c6d0acf3ca441458daab66baca11d5981e8842a`. Evidence: `artifact65-python-{macos,linux}.log`, `artifact65-python-linux.json` and source observations. The initial Linux run64 failure and its diagnostic JSON-serialization error remain retained; the wrapper now serializes skipped-test identities correctly.

## Preexisting generic release-source seal limitation, review66

The 7,373-file trusted surface contains exactly 12 current dirty participants and no additional hidden, ignored or nontracked drift. Reconstructing HEAD bytes for those 12 files still gives `e9eaab398aebefedffe9ee287de7d65371a8dcb27d5d50fe82ce9ee286899dbe`, differing from embedded `803b6cb43a720c0834a5855df38bcda9b48f0b16df616cdb8ad3747407db3eec`; the working surface is `9fd913fb5cec8e62a88586e6e5363080a0ff3cac9445792af1b038a76d5be79f`. The last embedded-seal change was commit `f24a15ec7d01d0a7860b4e8a2d9e1fba8ec1f6e7` on September10. Since then 1,314 paths in the current surface changed, among 4,417 total changed paths. Refreshing the seal therefore requires broader semantic review than these fixes. It was not changed or bypassed; generic bundle/OCI qualification remains pending. Evidence: `release-seal66-review.json`, `release-seal66-inventory.json` and the independent history census. No Taira controller reference to this generic feature-graph guard was found; this archive-consumer limitation is distinct from the still-running Linux57 Cargo build and unfinished live DPN deployment.

At 16:22 UTC on September19, both primary status and faucet-policy routes still return502 (`public-health64.json`). The approved SSH route and explicit stop/switch answer remain unavailable. Original GNU Linux57 remains CPU-active, shipping binaries remain pending, and no build process or validator was signaled. No live write, empty production block or ledger reset occurred.


## Superseded public observation before run67, preserved verbatim

Primary public status and funding-policy routes still return HTTP 502 at 16:22 UTC on September 19.

Public faucet policy discovery is deployed, but ingress and chain health must be freshly verified; keep funding authority independently pinned.

## Funding-policy doctor coverage, run67

The maintained native doctor had omitted GET `/v1/accounts/faucet/policy` entirely. Both basic and full now issue that unauthenticated read and require the exact six-field canonical V1 enabled response. Validation uses the response's explicit address discriminant and restores the caller's context; it returns no trusted signing policy. Only the exact disabled-faucet 403 envelope is a warning. Missing 404, upstream 502, unrelated 403, malformed 200, unknown fields, noncanonical identifiers, multisignature authorities and zero quantities fail. Public-reset qualification continues requiring 200; its existing host-contract regression now proves otherwise-successful disabled reports fail in both scopes. No server or API contract changed.

Two new mandatory selectors cover twelve HTTP cases in each scope plus canonical-field validation. The updated runner suite passes 188 cases and 1,667 subtests. The first native attempt found a Norito JSON test-fixture expression requiring parentheses; that fixture-only correction rebuilt the current CLI in 27.474 seconds. All 18 native doctor tests and the host-contract test pass, zero ignored, in 3.091 seconds total process time. All 379 required CLI selectors are present among 1,981 compiled tests. The same executable remains SHA256 `fc126fc448e747ab27cacd3a08f717a1618abbf54a0ecd9afb66aa197d1d5bb3` across execution; the before/after source observation is unchanged at `482a02fa708fb83f00b3cadd4067ad8f0b641b49`, diff SHA256 `dd3a8e4de6f6c8f24cd95fd299c2d03a30d47bfe83ab8793b23c54a667aebcc4`. Independent producer/consumer review found no remaining issue. These are focused development tests, not shipping binaries or release qualification. Evidence: `target/dpn-devex/doctor67-{native.json,native.log,build.jsonl,build.stderr.log,source-before.json,source-after.json,python-macos.log}`. Initial failed compilation and census logs remain separately retained. Codec policy passes. Documentation and one preexisting whitespace-only host-test formatting hunk were updated after native execution; final scoped Rust formatting and diff checks pass. Historical archive verification passes 64,736 records and 67,311 occurrences.

At 16:57:40 UTC on September 19, read-only primary status and funding-policy requests still return 502 (`public-health67.json`). Approved MacStadium operator access remains unavailable. The original Linux57 GNU link is still CPU-active; the pending request to stop it is unanswered and no build process was signaled. The four-validator corridor and actual DPN deployment remain unfinished. No live mutation, validator restart, empty production block or shared-ledger replacement occurred.


## Superseded runner and public observations before runs68–70, preserved verbatim

Both runner suites pass all 281 macOS cases and 1,778 subtests; Linux passes 279 cases with two expected macOS-only clone-test skips.

all 379 mandatory CLI selectors are present among 1,981 compiled tests. The updated runner selection passes 188 cases and 1,667 subtests.

Primary public status and funding-policy routes still return HTTP 502 at 16:57 UTC on September 19.

## Complete native CLI census and maintained serial batching, runs68–70

The retained CLI test executable passes all 379 required macOS CLI selectors through the existing per-test runner in 254.091 seconds, with zero ignored tests. Basic and full scopes share this CLI selection. Independent source review found no intentional fresh-process dependency in those tests or in the five additional Linux selectors. Fixtures own their temporary storage, loopback servers, descriptors and subprocesses; cached executed genesis contains immutable cloned data rather than temporary paths. A controlled exact-name serial batch passes the same 379 tests in 77.023 seconds. Both runs preserve source observation `bcdddf165b24227e45c541d278132d88cf76ceb39eecf2112d3abfa474c958f9` at `482a02fa708fb83f00b3cadd4067ad8f0b641b49` and executable SHA256 `fc126fc448e747ab27cacd3a08f717a1618abbf54a0ecd9afb66aa197d1d5bb3`. Evidence: `target/dpn-devex/cli68-native.{json,log}`, `cli69-native.{json,log}` and their source observations.

The maintained full and focused runners now batch only CLI selectors in one serial libtest process with explicit exact names. Configuration/startup checks, other harness isolation, inherited descriptors, environment and failure aggregation remain intact. The closed result parser requires the exact unique selected census, canonical header and final counters, zero ignored or measured tests, and successful exit. Missing, duplicated, unexpected, malformed, partial or contradictory results fail; captured failure text cannot supply result evidence. Failed or aborted batches report unsuccessful and unexecuted names without replaying successes. A 30-second heartbeat leaves captured result streams untouched. Existing local I/O joins retain their previous duration limits; this change adds no global timeout.

The independently reviewed patch passes 84 focused proposal checks. The actual integrated runner suites pass all 287 macOS tests and 1,800 subtests, and 285 Linux tests with two expected macOS-only clone-test skips. The integrated maintained batch then passes all 379 native CLI controls, zero ignored, in 75.611 seconds, approximately 70 percent less time than the recorded 254.091-second baseline. The complete before/after source observation remains `15b25a24d13c83b4b32cab3f8e71663caf82a2a53afe1af80dcff4d817b6069e`; executable identity remains unchanged. Evidence: `target/dpn-devex/cli70-native.{json,log}`, `cli70-batch.{stdout,stderr}.log`, `cli70-python-macos.log`, `cli70-python-linux.{log,json}` and source observations. This measures the CLI regression portion only, not full Cargo, validator, immutable-release or live-deployment qualification. The five Linux-only native selectors still await the Linux executable.

At 17:21:46 UTC on September 19, primary status and funding-policy reads still return HTTP 502 (`public-health70.json`). Linux57 Cargo remains live; GNU ld has consumed more than two hours of CPU and remains active. No Cargo/rustc/linker process was signaled. Approved MacStadium operator access and the stop/switch answer remain pending. Shipping binaries, the four-validator supervisor restart and paid clean-client DPN corridor, and actual public DPN deployment remain unfinished. No public mutation, validator restart, empty production block or ledger replacement occurred.


## Linux Core census and source repairs, runs71–72

The same separately linked Linux Core diagnostic (SHA256 `6f837b8f5916797fd148550ab597b1a8c2f25d1feed2e74c4ffebe5b32fbe8cc`) ran all18 beacon selectors in19.588 seconds and the remaining195 non-pending-Kura selectors in169.855 seconds. Those195 produced182 passes and13 failures; together with six previously passing pending-Kura controls, all219 basic selectors are accounted for, with206 passes. Each selector retained its own process. Both current runs preserved source observation `7c3298768b1570a3c4cca74db88b2afe6875a8f5c55891b0ccf9cea45f71577c` at `482a02fa708fb83f00b3cadd4067ad8f0b641b49` and unchanged executable identity. This is development diagnostic coverage, not a Cargo-emitted complete batch or release qualification. Evidence: `target/dpn-devex/core71-{beacon,remaining}.{json,log}`, exact per-test logs and the independent safety-selection record.

Five stack-overflow controls pass on that unchanged executable with only the test thread minimum set to the production64MiB Sumeragi stack. Source repairs now give those fixtures owned production-budget threads and preserve panic propagation. Four CompleteTip activation controls previously used a synthetic genesis without admitted physical geometry; their shared fixture now executes nonempty genesis through the existing Apply fixture, retaining exact physical catalog, output/finality checks and separate genesis/validator authorities. The drain test now installs initial and adversarial metadata in the actual canonical runtime rather than only its process-local cache. Two account-profile tests now observe both actual static validation workers, verify profile restoration and preserve the exact canonical output, metadata and permission assertions; they no longer assert counters from removed detached execution. The production catalog snapshot now reads runtime and protected parameters under one stable publication generation, preserving committed descriptions; publication-owned lane reads use a private ownership projection to avoid recursive waiting. Existing description/recovery and publication-boundary regressions cover that change. These source repairs are not yet native-pass evidence.

The focused Core test metadata check passes in334.594 seconds with zero errors on unchanged source observation `e42a0e0838ba2769934540d7941a09a184adc1864d403dcb0b34199b1b529507`. It uses the existing `dpn-devex` Mac target, default+iroha-core-tests, test profile, incremental compilation, six jobs and Apple ld. The maintained runner suite passes194 cases and1,689 subtests; codec and diff checks pass. Evidence: `core72-metadata.{json,jsonl}`, paired source observations, `core72-python.log` and `core72-codec.log`. Metadata compilation does not execute the repaired tests.

The renamed genuine-genesis source contract now binds its actual test wrapper to the behavior body and pins the existing stronger build-fingerprint assertion. An independently reviewed single cfg(test) include is added to the closed include mapping, its expected mirror and mapping-only digest. Ten focused parser/order/mutation controls pass. Two existing broader acceptance cases remain red: the ordinary-ingress contract has40 further preexisting declaration mismatches, and an old test mirror expects77 rather than80 owner keys plus16 differing mappings. Those were recorded, not bypassed; no production or broad trusted-source seal was refreshed. Evidence: `core72-geometry-contract-summary.json` and its referenced logs.

The completed manual LLVM diagnostic has been losslessly archived to `target/dpn-devex/llvm-link-benchmark60/iroha_core.llvm18.diagnostic.gz` (304,773,187bytes, archive SHA256 `998964906c8cf0c32e5943883583bf645cb6d5caa4d138228435621553e82895`). Full decompression reproduces the original executable SHA256. Before replacing the uncompressed copy, source ownership/identity and absence of open users were checked twice; all test logs remain. Warm Cargo caches were not deleted. Local guest root free-block trim was also attempted, reporting508.7MiB, without an observed host-space increase. Physical host free space is about8.6GiB, so additional capacity is requested before native rebuilding. Linux57 remains CPU-active; no Cargo/rustc/linker process was signaled. Source edits after its Core codegen make it a mixed-source diagnostic.

At17:32:18UTC on September19, anonymous primary status and faucet-policy reads returned502. Approved MacStadium operator access and permission to stop/switch the original GNU build remain pending. The four-validator corridor, immutable shipping qualification and public DPN deployment remain incomplete. No public mutation, validator restart, empty production block or ledger replacement occurred.

## Superseded DPN status before Core71–72, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native initialization derives the closed typed manifest and exact four-validator bindings from authenticated signed genesis; Core and CLI share bounded manifest semantics, and canonical finality input/output proofs remain mandatory. At merged `482a02fa70` with the reviewed working changes, all 85 native deployment tests pass, including six new proof-prefix controls. One locked completion invocation now retains authenticated prefix state across pending retries while rereading canonical disk proofs and obtaining fresh peer evidence; new invocations reauthenticate disk. CLI59 exposed an invalid resultless genesis fixture; CLI60 reuses the existing genuinely executed fixture and passes all 85 tests with zero ignored, unchanged build source and unchanged executable identity. Its warm rebuild took 18.32 seconds. The maintained runner checks fixed-path `lsof` before compilation and defaults Linux development checks to verified LLVM 18. macOS keeps Apple ld; explicit system linking remains available for diagnostics, and authenticated release preparation is unchanged. Both runner suites pass all 287 macOS cases and 1,800 subtests; Linux passes 285 cases with two expected macOS-only clone-test skips. Network observation subsets now run before shipping codegen and need no node binaries or peer storage reserve; selected beacon workloads validate external-root custody before compilation and again at runtime. All 14 native Linux observations pass in 1.303 seconds through the maintained dispatcher. A separate LLVM 18 diagnostic linked the same Core object inputs in 12.13 seconds; the maintained Cargo LLVM workflow is not yet measured or qualified. Linux57 remains in CPU-active GNU linking before its own test execution; subsequent CLI/runner edits make it a mixed-source development diagnostic. Separate Linux diagnostics cover 24 distinct native controls: six pending-Kura recovery, four configuration and 14 observations, with unchanged Rust source and executable identities at each recorded run. A real Linux same-timestamp content mutation exposed a shared artifact-reader gap; bounded offset-preserving SHA256 rereads now reject it, and seven deterministic contract controls pass. A six-file macOS run passes 372 cases and 1,778 subtests; 26 generic bundle/OCI tests stop at a preexisting trusted-source-seal mismatch. Its committed baseline already differs from the embedded checksum, with 1,314 current-surface paths changed since the seal update; that broader review remains pending and the seal is unchanged. These do not qualify the Cargo batch or a release. Linux shipping binaries are not yet built, so the real four-validator restart and paid DPN corridor remain unexecuted. Earlier 200 native passes qualify their preceding `c8eae6b521` source only; selected Linux metadata passes qualify merged `482a02fa70`. Complete four-validator Linux recovery, clean-client completion and actual DPN deployment remain unfinished. N0 original Validate-to-Apply custody, aggregate resource admission and consuming-publisher lifecycle/participant ownership remain separate unfinished outcomes; ordinary raw State publication is not a blanket refusal. The native doctor now probes public faucet policy in both scopes, validates exact canonical V1 discovery and distinguishes explicitly disabled funding from missing or broken routes. Disabled-faucet warnings cannot satisfy public-reset qualification, and discovery never supplies signing trust. All 19 focused native doctor/consumer controls pass after a 27.47-second warm CLI rebuild; all 379 mandatory CLI selectors are present among 1,981 compiled tests. All 379 subsequently pass through the maintained serial batch in 75.611 seconds, versus 254.091 seconds with one process per test. The CLI gate now reuses immutable genesis fixtures, requires a closed exact-name result census and emits progress every 30 seconds; other harness isolation and prerequisite order are preserved. Primary public status and funding-policy routes still return HTTP 502 at 17:21 UTC on September 19. The approved operator SSH route is not available; no public write, validator restart, empty production block or ledger replacement was performed. Detailed source-scoped outcomes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

## Core73 native census and Core74 fixture completion

The canonical optimizations checkout at `482a02fa708fb83f00b3cadd4067ad8f0b641b49` rebuilt current Core tests in421.725 seconds using the existing Mac target, six jobs, incremental compilation and Apple ld. All222 selected tests executed in337.816 seconds:220 passed, including all13 failures from the earlier retained Linux diagnostic, six pending-Kura controls and three adjacent snapshot/genesis checks. The executable remained SHA256-identical throughout. Python-only contract work changed the whole-checkout observation during build and test; no immutable release or current Linux qualification is claimed. Evidence: `core73-native-build.json`, `core73-tests.json`, `core73-native-summary.json` and paired source observations.

The remaining two tests were `runtime_nexus_setter_requires_exact_protected_dataspace_projection` and `runtime_catalog_final_overlay_rejects_removal_and_unchanged_malformed_state`. Their direct fixtures installed protected World catalog data without its canonical runtime owner. Core74 explicitly proves the partial pair is rejected, completes the setter fixture before negative/positive calls, and retains frozen policy for the pure overlay validator. Exact errors and current/predecessor/World/cache/geometry nonmutation remain covered. Independent review found no issue. Focused test metadata passes in55.919 seconds with zero errors and unchanged source observation `9b8b8d50fc74b9c4b833e4058ce4d99d5e63aeb1bf3f7ecb6fd1946695ed4e80`; native rerun is pending. Evidence: `core74-catalog-fixtures.json`, its exact patch, and `core74-metadata.json`.

Both broader source checks recorded red inCore72 now pass. The80-owner include mirror and source ordering have57 acceptance/mutation passes. Independent owner-projection review has21 existing controls passing. Four composed ordinary-ingress cases pass with32 negative mutations. Reviewed historical bodies justify narrow classifier/driver, Ready Sign fixture and531-test inventory pins; no broad release-source seal changed. The runtime mutation case retains equivalent expanded coverage while reducing its maintained run from151.94 to12.68 seconds. Evidence: `core73-inventory-result.json`, `core73-contract-final-result.json`, `core73-pin-independent-audit.json` and the runtime-contract audit.

Four exact completed obsolete Mac Core executables were compressed only after byte-bound historical evidence, no-open-user checks and stable descriptor identity, then fully decompressed and SHA256-verified before their uncompressed paths were removed. The original bytes/modes/digests are retained in `core73-obsolete-archive.json`, `core73-manifest41-archive.json`, `core73-core42-archive.json` and `core74-recovery49-archive.json`; object/library/incremental caches and current Core/CLI executables remain. Native codegen needs more space. The ordinary metadata check used its measured1,598,070,784-byte replacement metadata cache plus256MiB output allowance and2GiB headroom for the separately running Linux link; this is a development estimate, not a compiler peak guarantee or an authenticated release-floor override. Evidence: `core74-capacity-audit.json`.

Linux57 was still CPU-active in GNU ld after3:46:41 of linking; it was not signaled. Public `/status` and `/v1/accounts/faucet/policy` both returned502 in `public-health73.json`. Approved MacStadium access, live repair, Linux/shipping/production-supervisor qualification and DPN deployment remain unresolved.

### Superseded DPN status and P9 before runs73–74, preserved verbatim

Current DPN DevEx work uses only `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Native initialization derives typed inputs from authenticated genesis and the exact four-peer profile; deployment keeps independently authenticated completion and resumable once-only submission. Earlier focused CLI qualification passed all 85 deployment controls and all 379 macOS CLI controls; maintained serial batching reduced the latter from 254.091 to 75.611 seconds. The doctor now checks canonical public faucet policy in both scopes without adopting signing trust. The retained Linux diagnostic has exercised all 219 basic Core selectors across runs62/71: 206 passed and 13 failed. Core72 repairs production catalog snapshots losing committed descriptions, five test-stack mismatches, four synthetic-genesis recovery fixtures, one obsolete drain-cache fixture and two retired parallel-execution counter assumptions. The replacement profile tests observe actual static validation workers and preserve canonical output/permission checks. Focused current Core test metadata passes in 334.594 seconds with unchanged source; rebuilt native execution is still pending, so the 13 failures are not yet closed. Maintained runner validation passes 194 cases and 1,689 subtests; codec/diff checks and ten focused CompleteTip source-contract cases pass. Two broader source acceptance checks remain red on preexisting ordinary-ingress declarations and inventory mirrors; no production source seal was refreshed. The completed Linux diagnostic is preserved as a SHA256-verified gzip archive. The Mac has about 8.6 GiB free and additional capacity is requested before native rebuilding; warm Cargo lanes are preserved. Linux57 remains active in GNU linking and has not been signaled; the maintained Linux development default is verified LLVM18, while macOS retains Apple ld. Public status and funding policy most recently returned502; approved MacStadium operator access remains unavailable. Shipping binaries, the real four-validator restart/paid clean-client corridor and actual DPN deployment remain unfinished. N0 original Validate-to-Apply custody, aggregate admission and consuming-publisher ownership remain separate open outcomes; raw State publication is not a blanket refusal. Generic bundle/OCI qualification still needs its broader preexisting source-seal review. No live write, validator restart, empty production block or ledger replacement occurred. Detailed evidence and historical scopes are retained in the [dated recovery record](docs/history/2026-09-19/dpn-recovery-geometry.md).

| P9 | Persistent Taira joining and disposable networks | CLI/daemon and authorized network operators | One signed-bundle `iroha taira join` path initializes an owner-private permissionless observer, validates expiry/network/genesis and supports staking activation. Disposable four-validator `taira_devnet.py up` proves typed Applied, advancing convergence, MCP and guest workload; canonical source/artifact identity, no startup-only success or unmanaged cleanup. Preserve release94's completed public reset, same-source Linux preparation, convergence/canaries/restarts and public Host routing; finish test.inori.co.il application verification. Verify stopped-owner cgroup/firewall cleanup on the deployed native dispatcher. Qualify the maintained native public-input/profile export, typed genesis-derived manifest generation and `dataspace-deploy init/plan/apply/status` path for physical DPN and paid `dpn`/`admin@dpn`, including bounded fees, once-only submission, independently anchored inclusion and all-four StateApplied. Require selector-bound typed absence and preserved alias diagnostics through Torii, native SDK/MCP readers and OpenAPI, plus one inherited wait deadline with no zero-budget dispatch or late finality and exact typed terminal-failure evidence. Retain the merged-source Linux metadata and separate runtime diagnostics; rebuild Core72 repairs and close all13 Core71 failures (current219-selector diagnostic has206 passes), then complete the maintained Cargo batch, shipping binaries and production four-validator supervisor restart before the unchanged full release gate; retain composed FD, inventory, shared-deadline transport and stage-consumer checks and submitted ambiguity in one native apply recovery journal. Retain the qualified core onboarding/faucet/final-canary transport lifecycle in future rollouts. Recover the source42 first-epoch beacon incident without weakening finalized history or pulse validation; supply a production native key-session and per-seat signer bootstrap, prove its committed activation before the first required pulse, and make missing beacon readiness fail deployment qualification early. Retain source46’s passing four-peer recovery fixture; qualify bounded finality-read HTTP 429 retries, production beacon custody/bootstrap/readiness, authenticated height observation and maintained controller integration before DPN activation. Retain the six passing pending-Kura recovery controls and qualify their original-body four-stage restart in the unchanged four-validator Linux corridor. Run all six mandatory recovery controls first and stop before other startup/release work on failure. Complete N0’s original Validate-to-Apply custody, aggregate resource admission and consuming publisher lifecycle/participant ownership before claiming one original execution through publication. Resolve retained-height-3598 recovery separately; no ledger replacement is authorized. Public faucet policy discovery is deployed and both doctor scopes now validate its exact response; carry the 19 passing doctor/consumer controls into fresh ingress and chain-health verification, require enabled funding for deployment qualification and keep funding authority independently pinned. Retain the 85 current-source deployment regressions, including signed-prefix retry and disk-tamper controls; qualify them in the real four-peer completion corridor. Retain the exact serial CLI batch and its 379 passing macOS controls; execute the complete Linux selection in the maintained candidate, preserving configuration/startup ordering, closed result counts, failure aggregation and progress output. Qualify the default Linux development LLVM workflow through its first Cargo fingerprint rebuild; treat the 12.13-second separate Core link only as linker timing. Preserve macOS Apple ld, fixed lsof preflight and authenticated release boundaries. Preserve early beacon-root admission, observation-only build deferral and bounded source-content revalidation; resolve the remaining stale source-contract declarations and inventory mirrors without weakening their checks, and complete the broader trusted release-source review before claiming generic bundle/OCI qualification. Ordinary deployment of existing binaries must remain independent of Cargo/release gates. Remove the client build dependency on full validator Core only through shared exact trust/manifest checks and an explicit operator-command boundary, without duplicate or weakened validation. Measure executable-only revision changes in the release build lane without invalidating shared libraries. |
