# Closed admitted B+tree insertion

Work remains in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
The overall Sumeragi liveness goal and L1–L6 remain open.

## Original operation and complete demand

Node charging and payload-policy hooks alone left no public operation that could
plan and reserve a complete edit before constructing its writer. The existing
map/read/snapshot/writer/detached-owner wrappers now carry their sealed allocation
mode through the same cursor and node engine. Untracked retains ordinary mutation;
Prepaid<P> permits a consuming closed insertion and immutable successor handling.
No unrestricted prepaid write, Extend, mutable reference or mutable range is
available. Reattachment accepts the exact original map and predecessor, without
planning, reconstructing or funding another successor.

The new operation acquires the original writer without waiting, borrows the
locked root and plans without allocation. It sums exact padded node layouts,
both fixed bookkeeping allocations, both actual cursor/reader shell layouts and
explicit payload-copy demand. The checked structural bound admits one leaf clone
and singleton sibling, a branch clone and possible sibling at each ancestor, and
one new root. Every existing key/value copied on the path is included. Separator
copies are bounded using actual candidate payload layouts, including every child
minimum at each ancestor: a split can promote a child outside the insertion path.
The key-copy bound also covers copies of freshly cloned keys. Payload planning,
ordering and diagnostic formatting must not allocate unadmitted storage.

One original provider consumes that full reservation. Contention, poison,
planning overflow/unsupported payload or capacity refusal returns the same input
before successor allocation. After the edit, unused prepaid remainder is returned
before detachment; original node/payload/buffer/shell charges remain retained until
physical deallocation. The successor can be inspected, reattached, published,
detached again or aborted, but cannot perform another edit. A panic after mutation
aborts private work rather than returning a partly changed cursor for retry.

The MV budget accepts a checked layout byte sum directly, without inventing an
aggregate Layout that might exceed one allocation's maximum. Actual allocation
charges still split their own exact layouts. The entire synchronous operation,
including admission and destruction, belongs inside that budget's original
refund-notification deferral scope. No physical guard escapes that scope.

## Current validation

Generation 122 executes the actual workspace and vendored implementation. Its
captured compilation inputs contain 7,165 files. The completed runtime evidence is:

| Scope | Passing checks |
| --- | --- |
| MV library and original allocation consumers | 172, including six new public admitted-map tests |
| The same MV selection under native AddressSanitizer | 172 |
| New public admitted-map tests with smaller nodes and AddressSanitizer | 6 |
| Original World/publication and admission/gossip/repair Core regressions | 203 on one captured binary |
| Torii deadline, capacity, permit and partial-claim runtime regressions | 21 on one captured binary |
| Vendor default / smaller-node / release / async / sanitizer modes | 287 / 285 / 285 / 316 / 287 |
| Original charged-input / closed public-map API capability controls | 7 / 13 |
| Retained-carrier and geometry source-contract controls | 216 |
| Admission capacity and canonical retry source-contract controls | 239 |

The six public-map tests observe actual allocator calls and deallocation before
refund. They cover complete-capacity refusal with original-input retry, nonuniform
nested payloads and multi-level splits, old-reader retention, foreign/busy
reattachment, replacement with the original returned value, map destruction while
a successor remains, partial clone unwind and a wake callback that reenters the
original writer. The deallocation witness claims the exact live pointer and layout
before freeing, then marks it freed only after the allocator returns; pointer reuse
cannot grant a newer allocation the older allocation's free witness. Every actual
allocation in the insertion window has an original charge, and occupied credits
match actual outstanding allocations after handoff.

Temporarily disabling unused-provider release makes the concrete split/growth
test fail with `unused operation credit escaped the handoff` (exit 101). Exact
original source was restored before generation 122. The public capability controls
also require twelve specific compiler refusals, alongside an executing positive
that uses the real MV budget. The earlier diagnostic-wrapper mismatch and initial
test lint failure are retained separately from the passing final runs.

The complete eight-package build, strict MV library/test Clippy, canonical
source-binding gate, formatting/codec/history guards and independent final source
join are recorded in `dist/sumeragi-main-work/validation122.json`; that receipt is
emitted only after every required bounded check succeeds. Component captures and
commands live under `dist/sumeragi-main-work/generation122/`, with the Core runs in
`publication122/` and `review-current122/`. Previous checkpoints do not qualify
changed source. These are scoped storage and execution tests, not a live-network
liveness claim.

## Shared deadline binding correction

The first canonical run reported four failures against an obsolete relative
clock expression. The retry and capacity gates also declared incompatible token
lists for the same Torii owner. Their checker and ledger now use one complete
shared binding and one validation helper. Both enforce the observation before
absolute-budget validation and request preprocessing, response-egress reserve,
nonzero execution budget, the bounded execution cap and the original outer/inner
deadline. The helper receives original parsed source so typed and mutable binding
shadows remain visible; it permits independent setup statements to change order.

The merged implementation also rechecks canonical admission after capacity wait.
The retry contract now requires the original deadline in both response call sites;
mutating only the pre-wait or post-wait handoff is rejected. Tests move complete
Rust statements/blocks within their original owner instead of constructing invalid
function fragments. The final affected selection has 90 capacity and 149 retry
checks. This changes source-contract validation and its ledger, not Torii runtime.

Initial canonical/independent failures, the normalized-input preflight failure and
obsolete single-call fixtures remain in the local generation-122 evidence. Final
source captures replace neither those records nor historical results. Compiled
Rust inputs remain unchanged across this formal correction.

## Remaining production boundaries

The explicit initial node/reader constructor still excludes the permanent root
Arc and native mutex storage. It contains a TODO for that real owner; it does not
claim complete map construction admission. Actual model payload policies, MV undo
storage, iterator/removal admission and configured aggregate State policy remain
unfinished. Production Storage still uses Untracked maps and Validate remains on
its existing scalar path. No liveness milestone or network qualification is
closed: the final unchanged candidate still requires the full four/seven-validator
fault, restart, backpressure and final-transaction campaigns.

The next integration step must also preserve multi-edit atomicity: MV Storage
already holds its original current/undo writers, so calling the map-level insertion
would contend with its own lock. Publishing one entry at a time would expose partial
block state. Existing-cursor closed edits, prepaid undo/preimages and allocation-free
abort are required before replacing that production owner. A proposed inline lock
replacement must account for the real Core feature graph: current parking_lot
`deadlock_detection` can allocate on successful acquisition, and thread parking has
separate process-lived storage. No such lock replacement is part of this checkpoint.
