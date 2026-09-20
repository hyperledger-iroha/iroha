# Native Apply completion and original allocation custody

All work uses `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
This is a connected publication prerequisite; the production Native runner
cutover and four/seven-validator qualification remain open.

## Completion authority

The terminal `PublishedCarrier` now exposes a borrowed `PublishedNativeApply`
only after actual global State publication. It retains the original State's
opaque owner identity, result-bearing carrier and Native execution custody.
Candidate selection, execution, durable finality, checkpoint persistence and an
aborted physical installation cannot construct this proof.

The proof authenticates the original local Decision and the published Decision
independently against their exact frozen lane context. Their immutable value and
manifest must agree. Valid quorum signer subsets can differ; requiring identical
certificate bytes would strand a peer whose valid local quorum differs from the
carrier's quorum. The original local certificate and WAL remain unchanged.

Review of the first implementation found that checking a caller-supplied State
alone did not bind a transferred closed lane owner to its actual opening State.
Opening now retains that State's opaque owner identity through physical replay
and adoption. Completion uses this retained identity directly; a caller cannot
substitute another State with equal frozen contexts or a valid publication.

`LaneInstance::settle_published_apply` passes completion to the shared reducer
and consumes only its original held Apply effect. Repeated delivery is explicit;
local readiness and outstanding control completion retain the obligation. The
shared step admits zero new effect slots for application completion and checks
that its actual reducer outcome emits none. A saturated owner can therefore
release its Apply without waiting for new-work headroom. Process and driver
forward to the original owner even after global publication closes that lane
instance. No fresh signing authority or production ingress path is introduced.

The single and atomic publication regressions use actual local WAL/body owners,
different valid three-of-four signer subsets, real global publication and the
existing checkpoint/physical retry fixture. Completion is also exercised with
the fixture's descriptor budget restricted to its actual retained obligations.
They retain negative State, context,
value and manifest checks, and verify that prepublication stages consume nothing.

## Allocation boundary audit

MV `Cell::Block::try_detach` and storage detachment move the original current and
undo successors, including their allocation charges. They do not clone the
payloads. Runtime and World journal comments now reflect this implementation.
The four-cell runtime publication regression records all eight original value
allocation identities before detachment, after detachment, after physical abort,
and after publication. Comparing equal final values alone cannot establish this.

The remaining resource gap is earlier and broader than detachment. Core's
untracked MV blocks can clone current roots during acquisition and capture an
undo preimage on first mutation; nested values can allocate during execution.
World capture additionally allocates its inventory vector and field wrappers;
carrier preparation can construct a complete cold tiered snapshot, geometry and
archive projections. Installation and delayed-reader reclamation retain their
own resources. MV's charged acquisition APIs supply a lower-level owner but do
not provide an aggregate Core policy or recursively charge nested values.

The live `V2ApplyService::validate_candidate` still returns a scalar commitment
and drops preparation, while Apply executes again. The generic retained-validation
service reserves descriptors only. Its concrete producer must acquire actual
payload capacity before execution and carry that same reservation through all
these phases. Encoded transaction size, a unit admission callback or a reservation
created after execution cannot establish this property. Native ingress stays
closed until that producer, consuming publisher and process-lived lane owner are
connected and the old fresh signer is retired together.

## Validation

Build85 failed the test harness's denied `trivial_casts` lint in the new allocation
identity assertions (24 macro expansions of the same reference-to-pointer cast).
The correction uses `std::ptr::from_ref(...).cast::<()>()`, preserving every
identity assertion. The failed build took 193.33 seconds on unchanged inputs and
provides no new Rust runtime evidence. On that frozen source, the canonical
multilane structural checker passed with zero diagnostics and all 35 selected
existing Native source controls passed. Those checks cover their existing bound
owners, not a newly added formal binding for the Apply API.

Build86 passed the seven-package combined build in 445.63 seconds on unchanged
Rust inputs. Its 37-test runtime selection passed 35, including the actual Driver
closed-owner publication and all seven runtime-allocation tests. Both expanded
single/atomic tests failed while restoring the foreign-State fixture: the
original fixture's Parliament attempt lacked its referenced governance proposal.
No publication or snapshot check was weakened. The full canonical structural
gate and 11 directly affected existing Native terminal controls passed on 11,589
unchanged inputs. The runtime binary was captured immutably after Cargo reported
that harness complete; the complete combined build receipt is separate.

Before terminal consumption below, the cutover still needed a consumer for
closed instances without Apply or with returned control completion. Physical
closure drains handles and retains these obligations; the new Apply
proof deliberately does not turn absence into readiness. Driver polling now
receives non-owning progress while the original instance retains every retired
effect, materialized packet and actual returned body result. Retirement remains
charged against that instance's existing descriptor limit until an explicit
one-time handoff through Instance, Process, Driver or the transferred closed
owner. An obsolete or authenticated-closed completed body moves its existing
descriptor to retirement before the fresh-work capacity and control gates; no
extra slot or reducer event is needed. Current productive results retain the
original admission and acknowledgement gates. The exact-capacity Driver case and
actual held-WAL Process case cover these two independent completion obligations.
Unrelated instances keep their independent service turns. Body custody
retired only by closure retains the original output guard after handoff; dropping
that unfinished owner fences output for restart. The reducer's obsolete-tag and
deterministic-rejection paths remain distinct. No terminal cleanup permission,
body readiness or successful Apply acknowledgement is fabricated.

The two Driver regressions hold actual body work after its physical execution
and before completion delivery. They exercise a real timeout-certificate view
change, retained capacity, another lane's WAL progress, authenticated closure,
exact closed-owner transfer and loss of a taken unfinished body. The original
packet, rejection and body-completion assertions remain. This fixes the dropped
retirement handoff. The later terminal consumer below closes the in-memory
cleanup gap; the original Validate-to-Apply reservation remains open.

Build87 passed in 79.52 seconds; its unchanged 56-test selection passed 54. The
beacon fixture now includes its exact typed governance proposal and complete
sortition policy, but its later World-only commit overwrote the actual carrier's
undo. Strict snapshot recovery correctly rejected an H admission in H-1 state.
Beacon setup now occurs in the original admission carrier's World journal before
that carrier's single commit; reading its retained request is pure. Build88
passed in 101.41 seconds and the new complete-snapshot/predecessor regression
passed. Its unchanged 57-test selection passed 55: both original publication tests
then reached a premature fixture Apply assertion. The fixture now services the
Commit's actual Fetch/Store/Validate jobs before requiring Apply, preserving its
original Decision, WAL and earlier body assertions. Both canonical87 and
canonical88 structural captures passed with zero diagnostics and unchanged
11,589-source manifests. These failed runtime receipts remain preserved; they
are not passing publication qualification.

Build89 passed the combined seven-package harness build in 357.84 seconds. All
59 selected runtime controls passed on unchanged source and immutable executable,
including single/atomic publication, original-State and alternate-quorum checks,
all allocation-identity assertions, both Driver retirement cases and complete
snapshot restoration. Its canonical structural gate and 11 existing terminal
source controls passed on 11,589 unchanged inputs. Scoped formatting, codec and
archive checks passed. `validation89.json` binds those results. The subsequent
capacity-neutral retirement change is qualified only by its own captured result;
the 89 receipt does not qualify that later source.

Current build and focused runtime receipts are recorded under
`dist/sumeragi-main-work/` with epoch 85 or later. A receipt qualifies only its
captured source and command; this record grants no full-workspace, model-proof,
network or release qualification. The prior reviewed-regression and batch
qualification remains bound to `validation84.json` and its captured inputs.


## Authenticated closed-instance terminal consumption

The original closed owner now has one consuming terminal operation authorized
by the actual published carrier, original State identity and complete frozen
instance. Both fsynced Decision records and retained unlaunched Decision records
are authenticated against that publication. Different valid exact quorum subsets
remain acceptable for one immutable value. Earlier proposal/lock/timeout intents
need not match the final value. No synthetic Ready, persistence acknowledgement
or Apply event is supplied. An original held Apply must pass its existing genuine
publication settlement first; physical WAL/body drainage must already be complete.

Every fallible check precedes consumption. Refusal returns the same Box, control
completion, held effects and body allocation with their original output fence
armed. A separately transferred closed body has its own consuming proof check;
consuming it cannot disarm the remaining closed instance. Successful terminal
consumption releases only original in-memory custody, preserving durable files.
The new cases exercise the actual publisher, actual physical body execution held
before result delivery, matching/conflicting fsynced and unlaunched Decisions,
foreign State, successor context and exact-capacity closure. They are qualified
only by a subsequent source-bound build and runtime receipt, not by epoch90.

Epoch90 passed its seven-package combined build in 104.35 seconds and all 59
publication/ownership controls. A separate same-source extension passed all 32
original review regressions on the same captured Core executable, giving 91
distinct Rust tests. The exact pending-membership ledger test also passed. Its
canonical structural gate reported zero diagnostics on 11,589 unchanged inputs;
11 terminal mutation controls were last run on epoch89. These results remain
bound to `validation90.json`, `validation90-review-extension.json`, and
`review-ledger90.json`; they do not qualify the later terminal-consumption change.
Production retained execution/resource admission, Native runner activation and
full workspace/four/seven-validator qualification remain open.


Build91 passed in 225.27 seconds, with unchanged source; its 64-test selection
passed 61. The two new durable-Decision cases failed a test-only capacity helper's
explicit refusal to shrink an outstanding completion reservation. They now keep
that real WAL reservation intact while retaining the body independently. The
no-QC case overflowed its ordinary test stack while constructing another genuine
State publication inside the already large process-test frame. That foreign
publication is now constructed first, retaining its heap owner across the original
test. No stack override, authority check or production capacity gate was changed.
The canonical91 structural gate and 11 terminal mutation controls passed on
11,589 unchanged inputs. All failed runtime receipts remain preserved; the
fixture corrections require their own subsequent run.


Build92 passed in 92.14 seconds and all 64 publication/ownership tests passed on
unchanged source and captured executable. Canonical92 reported zero diagnostics
on 11,589 unchanged inputs. Formatting, codec and archive checks passed. The
aggregate `validation92.json` binds those results; the 11 terminal source-mutation
checks retain their preceding91 scope. This completes the tested in-memory
terminal consumer, not production resource admission or Native activation.

## Original context and rollback allocation custody

Accepted Native opening now creates one immutable shared context. The instance,
actual body jobs and transferred retirement owners retain that same allocation.
Handoff no longer deep-copies its frozen committee, policy and reducer context.
The original State and publication authentication remain unchanged. The existing
five terminal-publication cases check the original context through real workers,
closed-owner consumption and separately retained retirement.

World publication now retains the original 278 field-wrapper boxes and original
field-vector allocation across preparation. Each transient prepared wrapper owns
the emptied original box and the actual prepared journal. Refusal or abort moves
that journal back into the same box. The original vector remains available with
its existing capacity, so restoring the failed field, prepared prefix and untouched
suffix requires no replacement wrapper or vector allocation. Inventory order and
the original journals are preserved. Existing tests now check every wrapper
address, vector address and capacity across all 278 held-field refusals, full
abort, admission refusal and a late changed predecessor, alongside the original
writer-release and current/undo assertions.

Review also found a panic-unwind ownership defect: destructured capture admission
could drop before remaining original fields or the restored retry vector. Both
capacity owners now occupy earlier-declared local slots, outliving all payload
locals during preparation, abort and publication. A regression uses all 278 real
captured World fields and injects a panic after an actual prepared field or after
an actual abort restored its original journal. Nonpanicking Drop witnesses
check that both refunds follow destruction of the retained payload; exact
poison/release classification and unchanged current/undo images remain required.
This does not claim that partially published State can be rolled back after an
arbitrary publisher panic.

These changes close specific handoff allocation and unwind-accounting gaps. They
do not fund State acquisition, nested execution payloads, initial journal capture
or installation. The live validator still requires complete pre-execution
resource admission and the original retained Validate-to-Apply owner; Native
activation remains closed.

Build95 failed in 76.80 seconds with unchanged inputs. Five existing journal tests
still accessed the stored journal directly after its private representation
became an Option. All five accesses now borrow the populated journal explicitly;
no assertion was removed. The preflight canonical structural check passed with
zero diagnostics, but no epoch95 runtime executable was run. This failed build does not qualify the corrected source. Its later results
belong to their own captured build and executable.

Build96 passed the seven-package combined build in 237.05 seconds on unchanged
inputs. Its 80-test publication/ownership selection passed 79, including all five
Native context/terminal cases and all 278 original-wrapper refusal positions.
The new unwind case passed its preparation-phase refund-order assertions but
failed its existing blocking writer probe: actual panic unwinding correctly
poisons MV writers, and that probe assumed ordinary unpoisoned acquisition. Its
corrected probe captures the same original cut before the panic, then checks
every actual field nonblockingly. Unwound fields must report `Poisoned`
while normally released or untouched fields must prepare and abort; `Busy` and
`Changed` fail. It neither clears poison nor changes production recovery policy.
All failed receipts remain preserved.
The 11 enclosing terminal source controls and original pending-membership ledger
test passed on their separately captured inputs; neither supplies a passing epoch96
unwind or complete runtime qualification.

Build97 passed the seven-package combined build in 93.27 seconds on unchanged
inputs, and all 80 publication/ownership tests passed on its immutable captured
Core executable. This includes the actual-journal unwind regression with exact
poison classification, every original World wrapper/vector refusal position and
the five Native context/terminal cases. Fresh source captures also passed all
11 enclosing terminal publisher controls and the original pending-membership
ledger test. These source controls cover their bound publisher and ledger
contracts; the new allocation-identity and unwind claims rely on the Rust
regressions, not on a new formal proof.

The same unchanged executable additionally passed all 32 original review
regressions, giving 112 distinct Rust tests. These include the complete-input
sealed forms, exact two-MiB carrier framing, one-copy 160-KiB certified gossip,
and restart after the actual synced repair temporary, with rejection of foreign,
tampered and unowned temporaries. Epoch97 receipts under
`dist/sumeragi-main-work/` retain build/runtime hashes and the separately captured
structural, ledger and guard results. Full workspace and unchanged real
four/seven-validator qualification remain open, as do production resource
admission, retained Validate-to-Apply execution and the atomic Native cutover.


## Retained lifecycle publication

The private terminal publisher now consumes the captured nonretiring lifecycle
instead of refusing every nonidentity geometry. The retained geometry owns the
original State identity as well as its Kura, predecessor, successor and exact
header. A substitute State sharing that same Kura is not the publication target.
Before storage work the publisher checks source identity, lifecycle-effect
presence, retirement ownership and participant durability. Retirement/replacement
still requires the original service Queue cut and is explicitly refused here.

Geometry preparation and completion reuse the original raw/tiered attempts under
the already-held Kura lease. A busy backend returns its actual release observation
with the complete decided carrier after all State/component writers are released.
Storage completes before the State generation opens; the retained DA mapping and
lifecycle resets precede this carrier's DA updates. Both post-publication cursor
continuations finish after generation close and before releasing the original
lifecycle/Kura fences. Identity and metadata-only transitions do not acquire the
backend for geometry. The redundant second pending lifecycle copy was removed.

New coverage uses a signed height-two lifecycle transaction after actual genesis
publication, route planning, candidate validation, exact CommitQC and durable
checkpoint binding. It exercises backend contention and missing lifecycle-effect
refusal followed by publication of the same original carrier. Separate structural
negative controls reject a foreign State, wrong header and retirement/replacement
before storage mutation. These are private publication tests, not live handoff
or network qualification.

Build98 exposed two new fixture imports of private `nexus::LaneId`; both now use
its defining `iroha_model_base::topology` module. That failed run also observed an
external HEAD/index change on `optimizations` and source edits, so it is not an
unchanged-candidate result. Build99 then passed on unchanged Rust inputs. Its original review selection
passed all 32 tests on unchanged inputs and binary. The expanded publication
selection passed 119/121: two new lifecycle fixtures correctly failed lane
planning because their global genesis roster had no declared lane committee.
The fixture now installs an explicit primary-lane manifest bound to those four
signed genesis keys before predecessor execution. Production lane planning and
committee authentication remain intact. The publication run overlapped this test
repair, so its broad source capture changed. Build100 passed compilation and
all 32 original review controls, but the two new fixtures then reached a rejected
lifecycle transaction. Its old assertion did not preserve the rejection reason.
The signed genesis now grants `CanSetParameters` to the submitting account, and
both genesis context-binding passes install the same explicit zero-fee fixture
policy. Production fee and permission enforcement remain unchanged. Build101
passes on unchanged inputs in 127.30 seconds; the actual lane-addition/backend
retry regression passes on the default stack. All 127 expanded publication,
geometry, lifecycle, original custody and candidate-preparation controls pass.
The same immutable Core executable passes all 32 original review regressions,
for 159 distinct Rust tests with unchanged captured sources. Its SHA256 is
`cd23cabc0e639b0cd46e0274d79f2d1e05fc609b360b21ae853be5523bc12e9e`.
The pending-membership ledger passes 1/1 on unchanged inputs. Scoped rustfmt,
codec prohibition, archive verification and diff checks pass. Epoch101 receipts
under `dist/sumeragi-main-work/` retain the exact build/runtime inputs. The final
canonical structural capture initially failed in
`native-apply101/formal/final/`: five one-line source tokens did not match rustfmt's
multiline expressions and trailing commas, producing ten diagnostics across the
two gate layers. This failure is preserved. The corresponding contract and ledger
declarations were aligned together with the actual source. The repeated final
capture is recorded in `native-apply101/formal/final2/`, with fresh focused controls
under `native-geometry101/controls/`. `validation101.json` is produced only after
checking that the gate has zero diagnostics, all captures remain unchanged and
their hashes still match disk.

The parallel source-contract selection passed all 34 controls. Its broad capture
correctly records the separate Rust test fixture edit; the qualified 1,733-input
contract capture includes all 227 copied source inputs and is unchanged through
the later fixture-only edits; this selection precedes the raw-token formatting
correction above. It covers original State/header binding, retirement
refusal, retained raw/tiered custody and completion/publication ordering. The
read-only review also confirmed metadata-only transitions avoid geometry backend
acquisition and that cursor post work does not reacquire Kura/State fences.

Production pre-execution resource admission, the retained Validate-to-Apply
handoff, original service Queue retirement, old participant durability, the atomic
Native runner/signer cutover and unchanged four/seven-validator qualification
remain required. None of the six liveness goals closes at this milestone.
