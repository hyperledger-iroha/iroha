# Storage transaction checkpoints

Checkpoint126 removes inverse-edit rollback from live MV Storage transactions.
Each transaction holds borrowed checkpoints of the original current and
block-undo trees; abort restores those roots and frees only child allocations.
It advances the active liveness goal without closing L1–L6. Work remains in
`/Users/takemiyamakoto/dev/iroha` on `optimizations`, preserving the external index
and unrelated changes.

## Original parent ownership through apply and abort

Previously a transaction cloned its first preimages into a separate standard
map and Drop replayed insertion/removal to undo the transaction. Rollback could
therefore require allocations or payload clones while already unwinding or
under memory pressure. Transactions now retain their original parent trees.
Transaction-start values are borrowed from the saved current root. First block
preimages are staged in the undo checkpoint, preserving earlier siblings and
explicit absent-to-absent touches. An ordered key set records transaction touches
without another value-preimage map. Missing mutable lookup remains untouched.

Abort restores both roots without allocation, cloning or inverse edits. The
parent dirty flag stays unchanged until apply. Apply checks both cursor failure
flags and destroys touch keys while both rollback guards remain armed, then
resolves both Untracked checkpoints and transfers the dirty flag. The transaction
arms its own failure flag before touch/preimage cloning and retains it through
owned removal-query destruction. A caught panic cannot publish partial work.
The transaction API now has the single lifetime of its exclusive block borrow;
Core World and trigger-set consumers use that canonical API without aliases.

The same native checkpoint implementation supports both sealed map modes.
Prepaid mode restores the exact original fixed buffers; Untracked mode can retain
grown vector capacity, but rollback itself allocates nothing. `get_before` borrows
the saved root directly, and ordinary cursor mutation now arms the logical failure
flag just as admitted mutation does. Prepaid mode still exposes only admitted
insertion. Rust borrows prevent mutation or resolution while saved values are in
use and prevent references from escaping a checkpoint.

## Evidence and remaining work

The final focused MV selection contains 197 tests: 139 library, 18 admitted-map,
13 linear, 5 EBR and 22 map-generation cases. The recorded final checks require
all 197 in ordinary and both native AddressSanitizer geometries, plus strict MV
library/test Clippy.
New tests observe exact payload IDs and deallocation, allocation-free acquisition,
abort and apply, later-sibling abort, and refusal after caught preimage-clone and
owned query-key destructor panic. Existing history, no-op, absent-key, replacement,
snapshot, JSON and publication assertions remain intact.

The first full Core build passed, but its expanded runtime selection passed only
142/153: eight cases overflowed their original worker stacks, and three fixtures
failed earlier assertions. All eight stack cases passed against the retained
checkpoint125 executable. The generic checkpoint had carried two unused optional
vector descriptors for Untracked mode, repeated across every World field. Sealed
mode hooks now make that impossible retained-buffer state zero-sized, and the
non-null saved root provides the resolved-state niche. On the actual rebuilt
workspace artifacts, Untracked checkpoints shrink from 104 to 56 bytes and MV
transactions from 248 to 152 bytes; prepaid checkpoints shrink from 112 to 104.
Prepaid mode retains its exact original buffers and transfer-before-drop order.
A structural layout control accompanies the change; the same eight runtime cases
remain mandatory.

All three assertion failures also reproduced on checkpoint125. The fee fixture
now checks the canonical output, typed business rejection and unchanged business
balances with the charged fee, instead of counters for a retired detached path.
The trigger fixture dispatches a real signed ExecuteTrigger through the executor,
which establishes the pre-body callback owner. It requires the exact intended
missing-domain rejection and checks rollback in the still-live block, before
block discard can hide a leaked business effect. The DA rewind fixture seeds the
authoritative World pin indexes before hydration instead of expecting a header
sidecar alone to publish pin authority. Existing rollback assertions are retained.

The compact representation passes 151/153 Core cases on checkpoint126b. The two
remaining failures are the Native single/atomic publication cases with their
original 2 MiB workers. Debugger traces locate the overflow in real economic
execution, with completed block-start transactions still reserving stack space
in `block_with_owned_start_stages`. That constructor's frame grew from 412,624
to 618,816 bytes; `execute_network_source` grew from 284,272 to 608,288 bytes.
The correction moves completed private-settlement expiry and Parliament
transactions into borrowing phase helpers that return before native execution,
as the neighboring World/oracle/confidential phases already do. Execution order,
original owners, atomic rollback before failure recording and errors are retained;
no stack limit is raised and no owner is boxed. The rebuilt constructor frame is 247,392 bytes, below the original checkpoint125
frame. All 156 selected Core runtime cases pass on the rebuilt executable,
including both original 2 MiB Native workers, actual block-start expiry, automatic
enactment and failed-effect rollback.
The Parliament source contract checks the constructor's helper order and the
extracted phase's due-index and failure-recording obligations.
The complete 97-control Parliament module also binds the actual event return,
prepared State publication/replay guards, and shared indexed beacon requirement
through deferred activation. These checks replace retired source-token
expectations with the current owners; their negative mutations remain enforced.

A standalone reproduction against the checkpoint126c MV artifact exposed an
additional direct-Block failure: after an earlier successful edit made the block
dirty, a caught first-preimage clone panic during insertion/removal could still
publish the changed current tree without its undo value. The resulting state
could not be reverted. Direct block edits now retain aggregate failure through
preimage cloning and owned-query destruction. All value reads, new edits,
transaction entry, capture and publication preflight both original tree cursors
as well as that failure state. Checking both cursors before either publication
also rejects an undo-only failure retained by an aborted child checkpoint. Five
new regressions cover insert/remove/mutable access, query destruction, the
undo-only child failure, and both commit/detach refusal before admission, with
unchanged published values. Metadata-only observations grant no publication
authority. The correction introduces no allocation, compatibility adapter or
additional transaction owner.

`dist/sumeragi-main-work/generation126/` retains the initial source captures,
failed runtime, diagnostic baseline comparisons and development logs.
`generation126b/` retains the compact representation's five-mode vendor matrix,
public API controls and the 151/153 runtime result. `generation126c/` preserves the phase correction, its passing 156-case runtime,
the direct-Block counterexample and formal corrections. `generation126d/` captures
the final aggregate-failure correction. `validation126.json` is produced only
when its terminal checks match current inputs and the actual Cargo executable.
The unchanged vendor implementation is joined explicitly to its checkpoint126b
captures. Earlier receipts retain their source scope and are not relabeled.
These are component checks, not production async activation or complete network
qualification.

Both production maps remain Untracked. Ordered touch storage, current edits and
first block preimages still require one complete admission before mutation.
Checkpoint-generation refusal must propagate through State before prepaid
activation; the current infallible transaction API explicitly records this TODO.
Closed removal/update, concrete model payload policies, iterator/native-runtime
storage, aggregate State budget configuration and the complete Validate-to-Apply/
retirement cutover remain open. No full-workspace, release or real four/seven-
validator qualification is claimed by these owner and component checks.
