# Rust SDK capability checkpoint — 2026-09-29

This checkpoint implements part of the first-release architecture plan. It does
not qualify the complete repository, a native release, or a live network.

## Canonical ownership

Public Nexus preparation and committee observations now use
`Client::nexus().prepare_public_lane_plan(...).await` and
`Client::nexus().validator_committee(...).await`. Both dispatch through the owned
async transport with typed errors, response bounds and context deadlines. Exact
request/network/epoch, monetary-intent and native attachment checks remain;
committee attachments still need independent chain/genesis authentication.
The old synchronous methods are removed. All ten external calls use these
capabilities, preserving their downstream finality assertions.

Consensus diagnostics now belong exclusively to
`OperatorClient::consensus().diagnostics().await`. Requests bind the explicit
operator, network and exact request bytes; public/account contexts have no
capability. CLI and integration callers provide the intended operator explicitly.
The explicit blocking facades share the same implementations and reusable runtime.
There are no compatibility shims, probes or automatic retries on these operations.

I105 encoding accepts the network prefix explicitly. The SDK's ambient-prefix
helper, duplicate helper and global-setting reexports are removed; nine CLI test
calls now supply the prefix. This does not retire the model's remaining ambient
formatting state.

The CLI canonical-read test group now has its own cohesive module, retaining
all original assertions and adding four operator-authority/output regressions.
The original test file falls from 3,480 to 2,851 lines, below the retained test
budget. No source exception or budget was enlarged.

## SDK validation

The final SDK library passes **865 tests**, with zero failures or ignored cases,
and **13 doctests**. Strict library Clippy passes with `--no-deps -- -D warnings`.
All captured Rust/manifest/selected-fixture inputs and Git identities remain
unchanged for each final build/check/runtime interval. The test executable is
`54372a051a54d585553ea25d6b6ccc969bf4b1af6d2398209a8a4fda7b9ae4fd`.
These are scoped functional captures, not authoritative memory or release seals.
All SDK runs unset `RUST_MIN_STACK`; the SDK uses no explicit enlarged stack.

The first full run records 848 passes and ten failures: six stale protocol-version
fixture assertions, three obsolete ambiguous-submission wording assertions, and
one mock responder constructing an entire client inside its request deadline.
The corrections assert current V1 configuration and structured ambiguity identities,
precompute response bytes and synchronize pending transport with a deterministic
Notify barrier. Dedicated real deadline/cancellation tests remain. A second full
run passes 865, then strict Clippy finds twelve issues; the final repair separates
monetary validation and corrects casts, documentation and method references.
The final rebuild and full run above include those repairs. No earlier failure
or stale binary is relabelled a final pass.

The route inventory matches all **670** current Torii descriptors. All **21**
feature-resolved normal/build dependency boundary configurations pass; **183**
CI-routing/dependency/source-budget script tests and **eight** operation-inventory
tests pass. The non-Norito codec guard passes. The manifest dependency ratchet
still fails with 43 overruns and a fingerprint mismatch; the final source-file
check still reports 183 findings and 155 exceptions. Limits were not relaxed.

## Remaining qualification

All four selected integration targets compile with
`atomic-private-settlement-smoke`: `network_functional`, `consensus_and_da`,
`sumeragi_npos_committee_transition` and `nexus_and_streaming`. The first attempt
failed on an unavailable peer-key accessor; the corrected caller uses its peer
client's configured operator key. The complete rerun exits zero with all scoped
inputs and Git identities unchanged. This does not execute network scenarios.
All CLI test targets compile. The 11 canonical-read runtime cases pass, including
explicit operator signing and rejection before I/O. All ten focused address and
consensus smoke cases pass after correcting the retired-command test to the CLI's
canonical input-error exit code (4). The initial JSON fixture compilation error
and the exit-code assertion failure remain in their recorded results. Smoke
captures include the actual child CLI executable and Python mock sources. Its
final runtime interval changed no inputs; the intervening test-network constructor
changes were recorded separately from the CLI build and do not affect that binary.

Strict genesis validation exposed a constructor ordering bug: provisional policy
commitments were validated before binding actual execution. Locally generated
manifests now discover only the exact typed policy mismatch, bind/re-sign once,
and require a second strict execution. Supplied final commitments remain strict;
no compatibility path, validator bypass or retry loop was added. Four constructor
regressions and existing custom, file-backed and immutable-cache cases pass.
Actual native genesis execution then passes on a fixed **2-MiB** stack with exact
signed-wire equality, both policy hashes and runtime-profile restoration. The
obsolete per-call 64-MiB worker is removed; the canonical entry independently
passes that same ordinary-stack regression. Nine enlarged test wrappers are also
removed. The first broader run records six passes and three failures: a stale
capacity diagnostic assertion, a real 31-validator staking-source overflow and
an unsupported private-dataspace genesis input. The capacity case now checks the
exact required/effective counts and rejection before execution. Generated validator
registrations are grouped by the unchanged bootstrap source-count bounds; the
actual 4-, 7- and 31-validator construction test then passes. Native pre-execution
still checks complete source and byte limits. No source ceiling or stack grew.
The custom-lane fixture now installs signed native lane authority; the positive
control passes, and private input before lane activation remains an explicit
passing rejection. This changes the stale positive fixture's scope and does not
qualify later private-lane execution or a live network. All 29 configuration
fixture cases pass after supplying the custom-staking fixture's canonical signed
NPoS/XOR policy. Typed error retention preserves the first actual output index
and cause through poisoning, startup and daemon contexts. The real duplicate
rejection test passes through both strict pre-execution and public startup, with
height zero, no domain publication, no persisted block and exact original bytes.
Core library/all-test and daemon test compilation pass; an unrelated integration
source changed during those checks, so they are not immutable whole-source seals.

The broader 84-case genesis selection is **not passing**: it records three stale
managed-config fixture failures, a custom-staking parameter-order failure, a
retained-manifest fingerprint failure, then aborts in async peer construction.
The same image passed strict 2-MiB execution and the default owned Tokio worker
before exposing this separate stack path. The native macOS crash report identifies
`World::try_block` during actual genesis execution; disassembly measures its
`0xbd350`-byte frame plus saved registers. Expanded per-store layout iterators
retain aggregate temporary space until acquisition finishes. The correction moves
those read-only, checked demand calculations into a per-store frame while keeping
complete admission before allocation/writers. On the same debug target, the actual
frame including saved registers falls from **775,088 to 506,016 bytes (34.715%)**.
The exact previously crashing async peer configuration case passes, together with
shared async preparation, its failure path, fixed 2-MiB execution, the default Tokio
worker, the retained-manifest check and all three managed-config cases: **nine
passes**. No stack ceiling, allocation budget or validation rule was raised.
The build capture is unchanged; unrelated Core private-settlement/test inputs
changed before this runtime, so it is scoped artifact evidence, not a final
whole-source qualification. The parameter-order correction consolidates actual source parameters and builder
overrides into one early carrier, retains a leading executor upgrade and all
ordinary instruction order, and preserves each remaining source payload's
non-instruction fields. Original signatures, authority, transaction domain,
attachments and multisig restrictions are checked before rewriting or removing
changed carriers. The next broad run passes 90 cases and fails two stale
fixtures: an expectation of the old parameter position and a storage-enabled
profile missing mandatory software signer roles. The corrected builder assertion
requires exact ordinary instruction equality; both affected Sora profile tests
retain storage enablement and supply distinct role credentials. The subsequent
selection passes **93 tests**, including all five normalization regressions, real
custom NPoS, both profile controls and the default-stack paths. Runtime inputs
remain unchanged; the build capture includes these test-fixture edits and is not
a whole-source seal. One real-node test is explicitly outside this selection.
The failed broad runs remain recorded.

The first Core runtime records five acquisition-control passes and two fixture
failures: a rejection test assumed its original duplicate input was last, and a
capture test budget counted successors while omitting charged generations. The
repairs locate the exact original input and derive complete control demand from
the authoritative inventory. A separate selection passes **30 canonical genesis,
World journal and publication controls**, including every-field busy retry and
unwind. These use the earlier captured image and do not qualify later source.
The subsequent Core selection passes **47 tests**, including those repaired cases,
the new demand-overflow test, typed actual rejection before schedule finalization,
startup rollback and private-settlement authority controls. Later private routing
height correction in another active workstream still requires a fresh combined
build; these 47 passes do not qualify that later source. The later selection
passes **49 Core controls**, including the corrected private-authority boundary
and key checks, with unchanged runtime inputs. Only test-network fixture files
changed during its Core build. The daemon library selection separately passes
**four offline validation controls** on an unchanged build/runtime capture.
An earlier daemon binary selection ran zero tests and does not count as a pass.

Independent review then found that custom normalization replaced the original
block signature before checking it. Core now exposes validation-only original
intent authentication, including all proposal commitments and no execution
authority. Normalization checks resultless construction sources explicitly and
requires full structural validation for supplied complete sources before any
rewrite. The 11 normalization controls pass, including six new envelope cases;
the updated Core selection passes **50 tests**, including the new intent check.
Both unchanged original wire and valid rebinding are covered. The final slice
passes **99 genesis, 50 Core and four daemon tests**, with zero failures or ignored
cases. Every runtime leaves captured source inputs unchanged. Fresh builds for
all three targets have identical unchanged source inventories, and their emitted
test binaries exactly match the passing runtime hashes. The source inventory
digest is `541a011f7d2b0b73a1bee3f7f11dda855547bfdc56911f994ee4eb63afe74c2c`;
`final-genesis-artifact-cross-check.json` records the joins. Formatting and the
codec guard pass. These are selected functional tests, not full crate/workspace,
real-node, native/device, or authoritative memory qualification. Earlier failures
and source drift remain separately recorded.
SDK private-settlement Prepare also retains per-operation synchronous workers;
its complete async migration remains open.

The remaining model extractions, broad Core/Torii decomposition, source/dependency
budget closure, comparable pinned-runner memory reduction, JVM/native/device
qualification, full workspace checks and unchanged four-validator scenarios are
not completed by this checkpoint. The separate
[Norito performance record](norito-flat-writer-performance.md) preserves actual
codec measurements and their network/finality limitations.

Commands, Cargo artifacts, exact patches/preimages and scoped capture reports
are retained under ignored `dist/architecture-redesign-2026-09-29/`.
