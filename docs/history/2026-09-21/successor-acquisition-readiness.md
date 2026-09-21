# Successor admission and exact reader readiness

Scope: native synchronous map acquisition and Core hash successor admission on
`optimizations` in `/Users/takemiyamakoto/dev/iroha`. This extends
[actual retained writer acquisition](actual-writer-acquisition.md).

Hash successor admission previously observed the Core writer release before a
nonblocking reader observation. Contention on the native active-reader mutex
could therefore return a wait that its actual release would never satisfy.
The observation now comes from that exact native reader source. A regression
holds native commit preparation, refuses successor admission, and releases it
without any Core writer notification; the returned reader wait becomes ready.

A fresh native writer now has a move-only acquisition phase before planning,
admission or cursor construction. Contention acquires nothing. Poison remains in
the acquired guard and is rejected before admission. Planning and admission
refusals return that same physical owner and original input; success transfers
it to the same funded cursor. The standalone nonblocking constructor and map
insertion compose these phases and the existing construction/insertion engine.
The original current footprint calculation is shared by both insertion callers.

Core binds its original poisoning release guard before invoking insertion
admission, releases the actual owner on refusal, and signals successful detachment
after physical unlock. It no longer infers a release from an error classification
or fabricates a unit guard. Hash admission remains first in runtime construction,
before World/component acquisition; original budget refund deferral surrounds
the operation. Exact capacity floors, private-tip visibility and predecessor
identity remain mandatory.

Three native and two Core regressions cover original input custody, allocation-free
refusal, contention without a release, poison before callback, admission unwind,
private success, reader-only readiness and reentrant notification after unlock.
The source ledger and negative controls bind those phases. Method extraction
balances nested generic arguments and rejects foreign outer owners, where-bound
decoys and malformed headers. Both release scopes require all five regressions.

Validation artifacts belong to
`dist/sumeragi-main-work/generation149-{core,formal,release-census}`. The final
`dist/sumeragi-main-work/validation149.json` receipt records the actual completed
checks; intermediate failures are not final evidence.

Other fresh-writer/advisory/query/persistence notification boundaries and callee
unwind through aggregate owners remain open. Complete aggregate resource
admission, retained production Validate-to-Apply publication and process-lived
runner cutover still precede the unchanged four/seven-validator fault, restart
and final-transaction qualification. All L1–L6 remain active.
