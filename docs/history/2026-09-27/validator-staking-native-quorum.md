# Validator staking: native quorum checkpoint

This checkpoint uses only `/Users/takemiyamakoto/soramitsudev/iroha` on
`optimizations`, based on `3ec3f6ae968b15ae391bc77f08416243687304f3` with
concurrent uncommitted integration changes. It is not a frozen release candidate.
No other checkout's evidence qualifies this source.

## Changes

The global native scheduler, signed-genesis committee constructor and historical
committee candidate reader require exactly `3f + 1` validators for `1 <= f <= 10`.
Restored schedule windows validate every retained entry and reject a noncanonical
signer order. Generic simulator committee sizes remain separate from this global
production boundary.

Prepare, Commit and Timeout certificates now require exactly one equal-vote
quorum. QC formation refuses an oversized input set; TC formation selects one
deterministic exact quorum from its input pool. The independent simulator oracle
enforces the same certificate cardinality. Positive restart and far-behind joiner
fixtures now sign exact quorums, while genuine over-aggregated certificates remain
negative controls. Distinct valid exact-quorum subsets still certify the same value.

## Validation

`scripts/cargo_fast.sh --stable-local-metadata --incremental --zero-debug --jobs 4
--target-slot staking -- test -p iroha_sumeragi --features sim --lib` passes
329 tests, with zero failures and two explicit opt-in tests subsequently run
separately (the 100,000-height idle scenario and performance report). The earlier run exposed
two positive fixtures using oversized certificates; neither verification nor
scenario acceptance was weakened to fix them. Full logs are in
`target/staking-validation/sumeragi-exact-quorum-tests{,2}.log`.

The maintained strict mutation selection `MS14,MS39,MS40,MS41,MA11` passes its
eight-test baseline and kills all five mutations through their named deterministic
tests, with no survivors or errors. It ran with `--fast`, so randomized mutation
scenarios were not part of this selection. The report and source receipts are in
`target/staking-validation/exact-quorum-mutation/`. Its captured code, manifests,
lockfile and gate were unchanged; only the protocol specification changed during
that run. The subsequent fixture repair required and received the complete
library rerun above. The non-destructive gate wrapper sent no process signals.

The allocation integration passes 34 MV buffer tests, seven MV shared-owner
tests, 69 crypto Merkle-map tests and eight isolated crypto allocator-custody
tests, all without failures or skips. Logs are
`target/staking-validation/mv-allocation-custody.log`,
`crypto-merkle-map2.log` and `crypto-allocation-custody.log`. The first crypto
library compilation exposed one proof fixture still calling the retired
unfunded constructor; it now supplies an explicit finite original pool. The
existing proof/vector assertions remain unchanged. The codec retirement check
also passes (`target/staking-validation/no-legacy-codec.log`).

Native publication now retains the original overlay, captured result allocation,
exact certificate and staged block across reversible authorization refusal.
Failure after one-shot application begins latches recovery rather than executing
the source again. Five new four-validator tests use actual signed genesis and
BLS certificates; their Core run remains pending. Typed terminal failure now
propagates through the driver into the core halt/status state and suppresses
pending signing while preserving safety-record persistence and serving. The later
library run passes 330 tests with zero failures and two opt-in tests excluded
(`sumeragi-native-recovery-tests.log`); those opt-ins have not been rerun on this
later binary. The strict `MS42` gate passes its one-test baseline and kills the
recovery-halt mutation through `det_s42_original_publication_recovery_halts`.
Its 390 captured source inputs remained unchanged
(`native-recovery-mutation/report.json`). Complete physical funding and
owner-returning State installation remain open.
The contract-state map wrapper and its private local snapshot consumer now
require an explicit original allocation pool. The focused model run compiled and
passed 17 tests, but its map-funding test expected temporary capacity refusal
after reducing the whole policy limit to zero. The corrected test separately
checks temporary exhaustion and an impossible demand, preserving exact roots,
entry counts and original charges. Its rerun and Core validation remain pending.
The governance build-closure module uses the `parliament_types` test path; the
earlier `governance::types` filter did not exercise its three tests.

The explicitly selected 100,000-height idle scenario also passes on the same
consensus test binary (SHA and invocation recorded in
`target/staking-validation/sumeragi-idle-100k-receipt.json`). Minimum honest
committed height was 104,360; the run took 319.18 seconds. This is simulator
liveness evidence, not a multi-process network run.

The opt-in performance report also passes on that same test binary, in 37.85
seconds, covering four and 22 nodes with normal, silent-tail, crashed-set and
ten-percent-loss scenarios (`sumeragi-performance-report.log`). All 331 tests
therefore passed across the ordinary and two explicit opt-in invocations; this
does not qualify later source changes.

The historical result format now binds the complete ordered committee and BLS
possession proofs. The reader verifies a contiguous certificate prefix from signed
genesis with a bounded authority ring and reuses its cursor for sequential reads.
The local `CommittedLocally` fallback and digest-only format are removed; malformed
or unavailable authority is rejected. Genesis signatures authenticate its body;
its locally derived execution result needs deterministic replay or a certified
successor to establish independent execution authority. Core validation of these
changes remains pending. The constructor also checks that the signed genesis
header commits the actual payload before reading authority. Installing a full
attestation verifier invalidates the commit-only prefix cache so historical
flagged certificates are checked again.

## Epoch and retained-publication follow-up

The retained State publisher passes the focused Core library check
(`core-retained-publication-check.log`). It keeps the original overlay, prepared
World effects, captures, certificate frame and source-bound charges across
reversible publication refusal. Physical writer release across every deferred
attempt and complete funding of all publication allocations remain unfinished.
The executor now classifies World admission, penalty preparation and execution
capacity refusal as local failure rather than an invalid block; its regression
cases are awaiting the next successful Core test build.

The canonical neutral epoch/context model and removal of unsupported election
knobs are integrated. The focused model run passes 109 tests and fails two
(`model-epoch-policy-tests2.log`); both failures decode the obsolete governance
release-install fixture. A separate real fixture-construction diagnostic also
rejects its mismatched manifest network. Fixture regeneration now derives the
proposal identity from the authenticated manifest and regenerates the actual
threshold fixture signatures; no identity check is weakened. Regeneration and
its subsequent assertions remain pending.

Profile tests pass 23/23 (`config-profile-retirement-tests2.log`), and the grouped
consensus codec selection passes 15 tests with one opt-in unrun
(`model-consensus-roundtrip-policy2.log`). The two real Pasta public-key tests
pass (`pasta-public-key-tests.log`). The latest codec retirement check passes
(`no-legacy-codec-epoch-policy.log`). Core unit builds still expose test-only
migration errors; repairs to witness imports and the included snapshot test
fragment are not a passing test result.

The exact original-pool canonical payload/key allocation bridge is integrated
with source and result hashes in `native-allocation-bridge-stage/applied.json`.
Its five ownership tests and three actual allocator tests are pending. The
mandatory native epoch wire, applied-boundary driver, historical schedule graph,
frozen eligibility policy, preparation consumers, SDKs and signed metadata
version cut remain staged pending connected integration. Staged code is not
production activation evidence.

## Remaining work

The production Core check above passes; focused real-BLS scheduler/history unit
tests remain pending a successful current test build.
Initial Core checks exposed missing `mv` dependencies in the integrated IVM ABI
and crypto sources, then mismatched allocation APIs. Later checks exposed a
retired evidence-module path and one queue-authority caller of the removed
mutable-roster selector. The current queue projection now validates and reads
the exact retained proposal-height schedule, including its original proofs;
missing or malformed entries refuse admission. These are build diagnostics and
source repairs, not successful Core qualification. The updated codec retirement
check passes (`no-legacy-codec-native-recovery.log`).

The native scheduler now captures the signed-genesis roster and original verified
proofs once, retaining both in its canonical schedule. Candidate registration,
mutable expiry and withheld fresh proofs do not change membership. Schedule tests
cover genuine BLS pools of 5, 8 and 11 and malformed restoration; their Core run
is pending. This is a retention-only safety step: connecting authenticated E+2
elections, complete preparation, atomic activation and certified cancellation
remains open. There is no existing completed E+2 producer in
the old context path to reuse. The selected native design blocks entry into the
next epoch until the current boundary's certified result has been applied; the
implementation must replace the lag-2 membership assumption coherently.
Historical authority verification qualification, original funded
execution publication, epoch-bound signing, fresh schedule randomness and signed
RS16 integration also remain open. Real-XOR monetary, disposable 4→7→4, fault,
restart, formal, workspace and SDK gates must pass on one unchanged candidate.
These local tests do not establish staking or release readiness.

## Original allocation and fixture follow-up

All work remains in the `optimizations` checkout. The lower canonical payload bridge
passes **5 MV tests** and **3 crypto allocator tests**, including exact original pool
and physical-free-before-credit behavior (`mv-retained-payload-tests.log`,
`crypto-public-key-allocation-tests.log`). A subsequent read-only source check and
detached Cell/Storage/trigger read slice are integrated. The follow-up passes
12 MV unit tests and two actual-allocation integration tests, plus three Core
trigger read/retry tests (`mv-frozen-read-source-tests2.log`,
`core-frozen-trigger-geometry-tests.log`). The prepaid Cell successor then passes
all four actual-allocation tests (`mv-cell-successor-custody.log`). These APIs
preserve original ownership but do not finish State writer detachment. The next
typed original-field freeze/read prerequisite is integrated with tests pending.

The next native Core selection compiles and executes: **115 passed, 1 failed,
0 ignored** (`core-native-authority-tests4.log`). Retained publication, history
Busy retry, original Musubi scratch custody and generation-refusal cases pass.
The one failure was the historical committee geometry fixture passing an all-zero
seed to genuine BLS key generation. The fixture now generates 35 nonzero seeds;
its full zero-through-35 geometry assertions remain unchanged. Its focused rerun
passes alongside the three trigger tests (four total, zero failed or ignored).
This result does not close the broader custody/admission/snapshot/fee gate.

The release-install generator now uses its authenticated manifest's network ID.
It regenerates the canonical install/activation bytes and JSON from actual signed
evidence without relaxing identity checks. The generator passes, followed by
**5 normal governance release assertions passing, 0 failed, 0 ignored**
(`governance-release-fixture-regeneration.log`,
`governance-release-fixture-assertions.log`). This resolves the two release fixture
failures in the earlier 109/2 selection; it is a separate focused rerun, not a new
combined 111-test claim. The separate privacy-record mismatch remains outside it.

The native epoch/certification integration is still staged. It retains original
canonical result allocations and stages a distinct result-encoding retry owner so
capacity refusal does not discard the completed execution. Immutable certificate
allocation custody, the complete frozen State inventory, authenticated control
witness transport through forced EMPTY, and production beacon ownership remain
connected implementation work. Staged parse checks are not runtime or network
qualification. Real 4→7→4, full formal/DA/liveness, workspace and SDK gates on one
unchanged candidate remain open.

## Focused staking fixture follow-up

The broad 775-test Core selection aborted with a stack overflow in
`completed_tail_retains_prefix_under_whole_candidate_admission_and_static_handoff`.
It also exposed failures in multiple suites; no complete pass count is claimed
(`core-staking-custody-fee-regressions.log`). Staking component fixtures had opened
raw transactions without retaining an execution-source owner. They now use the
existing bounded ordinary invocation fixture before borrowing the transaction;
production source validation and finality authority are unchanged. Explicit
fixed-source hashes remain exact, and rejection tests retain actual transaction
rollback. The first focused rerun passes five and fails one fixture that removed
NPoS parameters before provisioning its XOR accounts. Provisioning now precedes
removal, while all operations under test still run without NPoS parameters. The
full staking rerun is pending (`core-staking-source-fixture-suite.log`). The
original focused regression gate remains open.


## Connected native epoch compile cut

The connected epoch producer, immutable schedule restoration, exact generic
boundary barrier, historical verifier, charged certificate custody, executor retry
phases and strict preparation consumers are now integrated (133 changed files,
`native-connected-integration/applied.json`, patch SHA256
`c2d634a535d7f6a921f45b68c129f4168f0e28f69735eddc5010223bc9b1fed4`).
This cut retains the sole current native wire revision 6. The dependent signed
control-witness change will replace it directly with revision 7; there is no
accepted previous-version path. Initial compiler failures are being repaired;
this is implementation progress, not a build or release pass. The frozen original
snapshot matrix passes one MV test. The next Core freeze gate attempted during
integration did not compile and provides no runtime qualification.

The old-source full staking suite completed with **145 passed, 5 failed,
0 ignored** (`core-staking-source-fixture-suite.log`). Diagnostics found five
fixture contracts to repair: missing committed XOR must reject actual genesis
registration; scope validation alone is independent of currency registration;
manual slash monetary plans must bind the actual offence height; a participant
may schedule its remaining stake while principal remains reserved through its
liability horizon; claim-plan construction must select at most 64 records and
carry later dust through subsequent signed claims. Source checks are unchanged.
All five fixture repairs are applied, with the current-source rerun pending.

The World capture overflow was measured in the actual debug binary's prologues:
simultaneously nested frames exceeded 2.2 MiB before other callees on a 2 MiB
thread. The repair moves individual original fields from the existing heap shell
into their original capture slots, preserving cleanup and ownership. It does not
raise stacks or modify the failing carrier regression. The five-file patch is
recorded in `native-world-capture-stack-stage/applied.json`; its original failing
regression and new extraction tests still require a rebuilt run. Complete native
writer detachment and original-pool World allocation remain open.

## Native integration compile and immutable certificate/SDK checks

The 133-file connected native epoch cut plus its compiler repairs passed
`check -p iroha_core --lib` (`core-native-connected-check4.log`). This is a
library compile, not qualification of the unfinished production native owner.
The rebuilt DataModel `commit_certificate` selection passed **18/18**, including
actual original charged-pointer retention, foreign-pool refusal, retry without
refund and one canonical wire layout (`model-immutable-certificate-tests2.log`).
The current Rust canonical staking fixture comparison passed **1/1** after the
seven committed rows were regenerated by the real Rust records; the corresponding
Kotlin `ValidatorStakingNoritoV1FixtureTest` passed **7/7**, with zero skips, using
installed JDK 21. Logs are in `target/staking-validation/`. The first Rust retry
saw a concurrent crypto manifest/source mismatch; the subsequent comparison
passed without changing or bypassing the fixture check.

Swift did not start its tests: the supplied `dist/NoritoBridge.xcframework` has
native ABI 21 while the package requires ABI 24. The installed active developer
directory is CommandLineTools; `xcodebuild -version` confirms full Xcode is absent
from that active toolchain. No artifact identity check was disabled and no SDK
pass is claimed. A correctly rebuilt ABI-24 Apple artifact and current-source
Swift test run remain required.

The next Core staking rebuild exposed additional test-only import and epoch
argument migrations from the connected native cut. Those compiler errors are
being repaired without changing assertions. The **145 pass / 5 fail** staking run
remains the last completed full staking selection until that rebuild runs. The
five diagnosed fixture repairs and the original broader focused failure gate
remain pending validation.

The pending native control cut adds a sole mandatory signed-header witness and
all-validator partial transport. The audit also found and directly replaces the
old incomplete beacon signature context with mandatory instance, scheduling epoch,
complete epoch context ID, parent native hash and parent result bindings. This is
source-stage work; its compile, signature replay, saturation, actual four-to-seven-
to-four networks, Pasta attestation and complete resource qualification are not
claimed here.

The original World control integration now passes the Core library check
(`core-original-world-controls-check2.log`). Three restore callers use the
original execution budget for predecessor World acquisition. Musubi restoration
preserves typed local admission refusals, and its funding fixtures include the
actual World control demand. Field transactions split one concrete mutable
WorldBlockFields borrow; no validation guard was relaxed.

The subsequent staking selection rebuilt successfully but **did not complete**:
183 tests were selected and the custody-preserving asset-definition test aborted
with stack overflow (`core-staking-fixture-repairs5.log`). The isolated original
execution-prefix regression also aborted (`core-original-prefix-stack-regression.log`).
An unchanged-stack LLDB run identifies the actual chain as World acquisition's
aggregate construction, OriginalWorldFields::initialize, ChargedBuffer::push_reserved
and Vec::push_mut. The final push frame alone probes approximately 210 KiB; repeated
by-value World field moves remain on the caller stack. The pending fix must place
fields into the original admitted backing and preserve complete writer cleanup on
partial initialization. Neither an increased test stack nor a skipped test is a
passing result. The earlier145/5 selection remains historical, not current evidence.

The pending native executor patch now owns local Pasta witness progress through
original backing, immutable shared control, actual signing and receipt publication.
It separately verifies full native QCs during preparation and startup replay.
The complete protocol8 cut, genuine boundary refusal tests, old-owner retirement,
operator native journals and full resource/lane qualification are still pending.
Its source parsing and patch applicability do not establish runtime qualification.

The direct World placement repair is now applied after source review and exact
baseline checks (11 files, patch 7015c92e96e6faa507d5f1dad55fef3358ba1f76a7e3a4181adad8b6dcc7553e).
It fills the original prepaid backing field by field; its partial frontier guard
releases both remaining and transferred writers before destruction. The lower MV
placement/replacement filter passes 28 tests and the separate borrowed-original
Cell/Storage transfer regression passes 1 (`mv-original-placement-tests.log`,
`mv-original-borrowed-transfer-tests.log`). Core cleanup and the three original
stack-overflow regressions still require the rebuilt runtime result.

The isolated privacy admission failure is a separate stale test expectation:
its portable near-miss omitted the required full Halo2 circuit prefix. The fixture
now uses canonical full identifiers and explicitly rejects the retired shorthand
aliases. All reserved-namespace, malformed-shape, closed-registry and VerifyProof
rejection assertions remain. Production admission is unchanged. Its rerun is
pending; this is separate from the earlier typed privacy-record recapture.


## Rebuilt original World and canonical privacy regressions

The rebuilt exact World placement passes all five Core original-control tests
(`core-world-placement-controls.log`). Both original stack regressions pass:
`core-original-prefix-stack-regression2.log` and
`core-staking-custody-stack-regression.log`. The isolated allocator integration
also passes, in both ordinary and replacement modes
(`core-world-original-shell-allocator2.log`). These retain the original thread
stack and production allocator; no test skip or stack increase was used.

Both canonical privacy admission fixtures pass on the rebuilt binary
(`core-privacy-admission-fixture-isolation2.log` and
`core-privacy-canonical-circuit-identity.log`). The correction is test-only and
preserves full circuit identity checks.

The broader staking selection now runs through the previous stack failure. Its
183-test run has exposed three stale fixture expectations so far: two demand the
retired incumbent error text instead of the exact missing committed-height error;
one wrongly expects new stake to be accepted after exit. The fixtures now inspect
the canonical refusal and use an actual dropped transaction before checking the
next overlay. Existing principal and pending withdrawal liabilities remain
asserted. Rebuild and final complete selection results are pending; no full
staking pass is claimed yet.

The staged native executor now additionally rejects decoded or foreign-pool QC
witnesses before queued or Worker retention. Kura admits loaded witnesses through
the same canonical Qc API before production handoff. Genuine boundary tests assert
that refusal preserves the original execution and that stored certificates carry
original-pool witness ownership. This source is uncompiled pending the full
protocol8 cut. Remote admission still needs to retain its copied buffer across
shared-control refusal; bounded decode alone does not complete funded recovery.


## September 28 continuation: exact original publication and fixture repair

The broad 183-test run ended at 174 passes and seven failures, then aborted in
`snapshot_owner_policy_survives_startup_with_live_nondefault_staking`. The
snapshot decoder now separates field decoding from its validation tail without
changing the thread stack, allocations or checks. Its rebuilt regression reaches
the intended strict XOR check; the old positive fixture used a synthetic asset.

The Deferred publication slice passes the production Core check
(`core-native-deferred-publication-check.log`). Six original World control tests,
the original prefix and custody regressions pass on the rebuilt unit binary.
All three new exact original Deferred tests pass. Two older consuming-commit
controls incorrectly assumed an abandoned World shell retained all budget; their
repair asserts the exact original shell/Cell refund and the corresponding actual
release observation. A lexical macro-scope compile repair is pending in the next
rebuild. No runtime pass is claimed for those two repairs yet.

The full MV suite passes **284/284** after the actual publication identity-release
control is funded from its original pool
(`mv-publication-controls-all-tests.log`). This includes five new ownership,
Busy, waiter and refund controls. EBR storage, ordinary World payloads, waiter
registration and other remaining original resource demands are not thereby funded.

Three earlier authenticated admission/lifecycle fixture corrections pass exact
reruns. The remaining positive State/snapshot fixtures now use canonical network
XOR and exact additive principal plus outstanding rewards. The roundtrip fixture
no longer overwrites its deliberate unbonding/slashing parameters with an obsolete
second setup. The missing-custody penalty test now executes genuine signed native
NPoS genesis first. Opaque reward/dust operations require the exact deferred-
operation error; ordinary opaque transfers require their exact transfer error.
These latest fixture repairs still require the current-source build/runtime result.

The mandatory protocol8, source-complete native Pasta attestations, beacon/context
controls, complete committee journals and native readiness changes are composed
in a target-only review aggregate. No isolated protocol7 format is installed.
The old-owner/native evidence retirement and SDK status replacement remain under
implementation. Staged source and formatting are not production qualification.
