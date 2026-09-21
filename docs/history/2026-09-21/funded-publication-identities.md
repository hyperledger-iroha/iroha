# Funded publication identities

Prepaid MV Storage previously allocated its next publication identity after the
execution callback, outside its original finite allocation reservation. Its
owner and initial version were also uncharged. An admitted execution therefore
had a late allocation boundary, and retaining old identity observations did not
retain corresponding credits.

Construction now reserves both maps and their owner/first-version identities in
one checked sum. Writer startup reserves both map shells and the next identity
together. It creates that identity only after acquiring both physical writers,
before reset, replacement copying or the caller's execution. A poisoned or busy
initial acquisition does not create an unused identity. Both ordinary and
replacement blocks carry the original successor through publication; snapshot
restoration carries the successor opened before copying. Untracked Storage also
retains its successor from block opening through commit or detachment.

Identity storage uses Concread's existing strong-only shared allocation owner,
now exposed as `shared::Shared`. Its declared layout is the actual allocation,
including the original charge. There is one implementation of reference counting
and physical reclamation, with no guessed `Arc` layout, weak references, alternate
identity representation or compatibility adapter. Captures clone that original
owner without additional allocation or credits. Pointer equality still detects
foreign owners and equal-value/undo-only publications without ABA reuse.

The old version remains retained until both physical writers and the publication
lock have released. Declaration order also enforces lock-before-credit release
when the release callback unwinds. The enclosing prepaid operation continues to
defer pool wakeups through callback and writer destruction. The shared control
block is physically freed before payload destruction and charge refund; payload
unwind conservatively retains the charge.

Four additional regressions cover publication at a completely exhausted pool,
retained duplicate/distinct identities after Storage destruction, one-byte-below
whole-writer refusal with exact retry, publication-lock release before refund in
normal and unwinding release, and actual shared-allocation deallocation before
refund (including payload unwind). Existing startup, replacement and poisoned
retry controls retain their assertions. Identity-holding fixtures now explicitly
verify the remaining owner/version credits and their final release.

Both complete MV layouts pass 272 runtime tests plus one compile-fail doctest.
The complete Concread library passes 428 tests. The release harness passes all
220 Python tests and 2,654 subtests; its exact selected ownership census is 256.
Each successful Rust run has 7,274 identical before/after inputs on `optimizations`
at `30aa732d64a4a124482fb1b68e77157f24cbb3fd`. Executable inventories and executed
test names are joined to the captured artifacts. Four new MV cases supplement
all previous cases; Concread's shared-allocation test moves with its implementation.
Fresh Core unit-test compilation passes without diagnostics, and all 58 original
review controls pass on its captured binary: sealed inputs, complete carrier
framing, certified gossip and authenticated completed-repair recovery. The
checkpoint136 receipt records the separate shipping Core and formal gate results.

Failed attempts are retained separately: the original identity-lock fixture type,
six uncharged-identity cleanup expectations, the real poisoned-retry allocation
counterexample, and release-census count/order mismatches. Successful checks use
new capture directories; no failed evidence is overwritten.

This closes publication identity funding for prepaid Storage, not aggregate State
admission. Native mutex/release/runtime storage, concrete World payload policies,
closed prepaid detachment, decoding and aggregate execution/restore work still
need admission. The production scalar Validate path remains until retained
Validate-to-Apply and Native producer/consumer retirement are complete. L1–L6,
full workspace and real four/seven-validator qualification remain open.
