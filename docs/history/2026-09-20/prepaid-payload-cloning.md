# Prepaid payload-cloning boundary

Work location: `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.
This continues the [cursor and retirement work](charged-cursor-retirement.md).
The overall liveness goal and L1–L6 remain open.

## Defect and implementation

Charging the concrete padded node leaves its nested key/value copies outside
admission when the engine invokes ordinary `Clone`. Leaf copies, branch keys,
root separators and rebalancing bridges can each own distinct allocations. A
node charge cannot authorize or bound them.

The existing engine now requires an explicit `NodeCloning<K,V>` implementation
from its original funding provider. It has no blanket funded implementation or
default method. The explicit Untracked mode calls ordinary Clone. A funded
implementation must split the operation's original prepaid owner before each
nested allocation and return a payload retaining that owner until actual free.
Every node-owned payload copy goes through this policy, including separators in
left/right splits, rekey, merge and redistribution. Existing slot moves retain
their original payloads without readmission. Both unused recursive minimum
helpers were removed in favor of the existing shared raw traversal.

A panic after private mutation must abort that cursor. For example, removal can
redistribute children before the parent separator copy panics; individual clone
cleanup is not transactional rollback of that private edit. Only the existing
preflight refusal happens before mutation and preserves the incoming operation
for retry. Published source generations remain retained independently.

Existing node/cursor allocation fixtures explicitly opt into ordinary Clone to
isolate their former scope. Separate concrete nested-payload controls reject
ordinary Clone, observe actual allocator frees before charge refunds, and test
partial construction and original-node preservation. A type-level negative
control rejects a node-only provider inheriting a payload-copy capability.

## Qualification scope

The unchanged code passes the eight-package workspace test build (MV, Core,
Torii, data model, test network, Kagami, daemon and Rust SDK; selected integration
harnesses compile). Its 7,150 captured compiled inputs remain unchanged. Core
passes 203 distinct selected tests on one immutable executable: 171 retained
publication/ownership controls and 32 original review regressions. This is not
the full Core or workspace runtime suite.

Actual MV consumers pass all 165 tests both normally and under native
AddressSanitizer. The actual vendored source passes the following isolated
manifest suites, with zero failures or ignored cases and unchanged source:

| Configuration | Passed |
| --- | ---: |
| Default selected features | 281 |
| Skinny node geometry | 154 |
| Release | 279 |
| Async | 310 |
| Native AddressSanitizer | 281 |

These totals include seven new concrete nested-payload regressions and the
node-only-provider negative capability control. Strict MV library/test Clippy,
seven original writer-input API controls, workspace/scoped formatting, codec,
history and whitespace guards pass. All 216 retained/native/geometry formal
controls pass on unchanged source. The final source-bound receipt at
`dist/sumeragi-main-work/validation121.json` records the full canonical gate and
original ledger check, and joins their complete source inventories with the
build, runtime, vendor and sanitizer captures and current checkout. Its frozen
canonical inventory includes this record; results from earlier source are not
substituted for the current candidate.

A synchronized counterexample replaced the new-root separator policy call with
ordinary Clone. The actual split regression failed with the expected bypass
panic; original source bytes were restored before qualification. The earlier
counterexample replacing the sole value-policy call was rejected at compile time
by the now-unused trait method, before runtime; that log remains separate.
The initial focused compile exposed an unused import and the now-unused minimum
helpers; both were removed. Those intermediate failures are retained alongside
the passing captured runs, not counted as final-candidate evidence.

## Remaining connected work

This boundary does not itself compute the complete operation demand. Close
planning under the original map writer, bind real MV payload types to prepaid
owners, and join node, payload, buffer and shell demand in one admission before
mutation. Initial root/control storage, MV undo ownership and aggregate State
policy also remain unfinished. Public maps still select Untracked and production
Validate remains on its existing scalar path. The final unchanged candidate still
requires real four/seven-validator fault, restart and final-transaction campaigns.
