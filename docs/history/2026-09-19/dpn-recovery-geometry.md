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
