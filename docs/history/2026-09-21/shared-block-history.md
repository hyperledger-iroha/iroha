# Original shared block history

The live `BlockHashes` container now uses the existing B+tree's original immutable
generations. Block and query opening no longer clone the complete chain. Ordinary
append and replacement-tip overwrite edit a private detached cursor; the short
hash writer is released before acquiring World. A transaction borrows its parent's
read-only history. The original private cursor moves through capture, admission
refusal, abort and publication without rebuilding its nodes.

StateView and StateQueryView retain the original committed generation. Readers
remain stable across append and same-height replacement and cannot veto publication.
Indexed/range reads replace contiguous-slice requirements; canonical snapshot JSON
streams the same array encoding. Explicit cold restoration/export can materialize
owned sequences. Emergency Fast startup retains its original read-only mapping and
cannot acquire the mutable tree family or open Native execution.
Torii orderbook and daemon provider-ingest checks borrow the same indexed history;
they retain exact height, tip, historical-hash and timestamp checks. Projection
fixtures seed the committed journal before opening its immutable query view.

Native State ownership and publication seals now retain the actual map root and
original reader generation, not separately allocated identity tokens. Final
preparation reacquires the original writer and checks the precise predecessor,
including equal-content ABA. It admits installation before reacquisition, returns
the original successor on refusal and can abort a prepared publication. Publication
sets the height under the original map locks; released cleanup and notification
custody outlive the aggregate commit fence. All active-reader observations that
can exclude preparation report their actual release, including a Changed refusal
after successful writer acquisition. Busy probes do not manufacture a release.
Status reads distinguish a poisoned publication from temporary contention, so a
permanent journal failure cannot masquerade as a retryable busy result.

Checkpoint131 moves the fixed-width journal to **Prepaid**
allocation custody. Kura owns one finite, file-configured
`block_hash_history_bytes` pool (256 MiB by default). Strict construction and
snapshot restoration use that original pool. Opening an execution scope plans
and creates its entire private successor before acquiring World or running start
effects. Its precreated tip is hidden from the execution read facade until the
final hash is installed without another allocation. Unfinished tips cannot be
published. Replacement admission precedes DA-index rewind. Empty Emergency Fast
history is explicitly read-only and owns no mutable tree.

The original tree reports its current charged footprint together with the
successor demand. An impossible configured bound is a permanent local failure;
credits held by old readers or other private work yield an original release
observation. Validate, proposal construction and autonomous prefix selection
preserve this distinction. A capacity refusal cannot become an invalid-block
marker, an empty-work result or an excuse to repeatedly try smaller prefixes.
Proposal and merge owners retain one registered release future each. World and
runtime guards are destroyed before history refund callbacks; aggregate commit
cleanup uses the original pool's synchronous notification-deferral scope.

Checkpoint131 passes shipping daemon/CLI/Kagami checks and the nine-package
Core/Config/Torii/daemon/CLI/Kagami/integration/xtask/MV test build. All 283 selected
Core, 14 orderbook/daemon, 18 query-consumer, 224 MV and 888 Config controls pass.
The final build re-emits the exact tested Core, consumer, MV and Config executables.
The Core run's broad capture changed only in two Torii test fixtures during execution;
its raw `unchanged=false` result is preserved and joined through explicit source
scope and executable equality, not relabeled as an unchanged broad run. Config's
runtime snapshot fixture is captured separately. The vendor matrix passes
347 default, 345 skinny, 345 release, 376 async and 347 AddressSanitizer tests on
one unchanged 63-input scope. Formal qualification covers 509 Native, 609 related
and 133 Parliament controls; exact unchanged scopes and the narrowly revalidated
checker changes are recorded separately.

The shipping check exposed eight query fixture writers that were accidentally
exported without their test feature. They now stay test-only, publish only their
World overlay and fail on publication errors. They cannot fabricate a finalized
height or transaction membership. Eight helper regressions and actual proof,
governance, evidence and bridge consumers exercise this boundary. Endpoint
fixtures retain the required ledger-read permission, exact lowercase proposal ID
errors and `NotFound` for an absent referendum tally; an existing empty referendum
still returns zero. The wake regression observes the actual service Queue channel
rather than comparing separately constructed waker wrappers. Formal mutation
checks now require the entire history-refusal branch, so another branch's matching
retry call cannot conceal a missing delivery rearm.

The checkpoint receipt `dist/sumeragi-main-work/validation131.json` records final
source joins, artifacts, independent checks and preserved failed attempts under
`generation131-core`, `generation131-formal` and `generation131-vendor-final`.
These checks do not establish bounded RSS, complete execution admission or release
readiness. Native mutex/runtime storage, concrete World payloads, joint Storage
removal and the complete retained Validate-to-Apply production cutover remain
unfinished. L1–L6 and unchanged four/seven-validator qualification remain open.

The following checkpoint130 results are historical and qualify only that earlier
source.

Validation receipts live under `dist/sumeragi-main-work/generation130-core`,
`generation130-formal` and `generation130-vendor-final`. Earlier checkpoint129
receipts remain historical. All 223 selected Core runtime regressions pass on
unchanged captured source, including original generation/ABA/refusal/abort/wake
checks, retained State/query views, the original review regressions, and actual
signed retirement and replacement publication. The Core test artifact SHA-256 is
`8bd36c13b1bd28e3b24156e14c2f0854de8bf4fcccb6577af5da79d9fbfb9279`.
The independently captured vendor matrix passes 339 default, 337 skinny,
337 release, 368 async and 339 AddressSanitizer tests on the same 63 inputs.
The combined `cargo test -p iroha_core -p iroha_torii -p irohad --no-run
--locked --offline` build passes, as do all 14 selected Torii/daemon runtime
controls and 224 MV ownership controls on unchanged final captures. Core's exact
tested immutable executable is also emitted by this final consumer build. The
earlier Core runtime capture and final build capture differ only in Torii's
hash-fixture correction; their broad manifests are not presented as identical.
The checkpoint receipt `dist/sumeragi-main-work/validation130.json` identifies
the formal scopes, exact source joins, artifacts and preserved failures. These
are local scoped checks, not whole-workspace or network release qualification.

Preserved development failures exposed missing parent links in history fixtures,
a default DA policy signed for a two-lane fixture, a foreign State fixture that
reused another network's Kura, and remaining slice/mutable-view consumers. Their
corrections retain authenticated genesis, strict network binding, immutable
views and every original publication assertion. Earlier failed or drifting
builds and runtime runs do not qualify the final candidate.
The Torii finalized-event fixture also now takes its expected hash from the
journal; `Hash::prehashed` sets the canonical marker bit, so raw even-byte test
patterns were never equivalent to the stored hash.
