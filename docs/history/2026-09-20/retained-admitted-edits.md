# Retained admitted map edits and initial root custody

Checkpoint123 extends the existing closed insertion engine. It does not enable
production Native validation or establish release readiness. L1–L6 remain open.
Work is confined to `/Users/takemiyamakoto/dev/iroha`, `optimizations`.
Checkpoint122's receipts and initial failed attempts remain unchanged.

## Original ownership through multiple edits

`BptreeMap::try_insert_owned_admitted` accepts the original unpublished owner
and one owned entry. It authenticates the same physical root and base generation
under the original nonblocking writer lock before planning from its private tree.
Foreign, changed, busy, poisoned, planning and capacity refusals return the same
successor and input. No cursor, shell, entry or bookkeeping reconstruction occurs.

Each admitted edit reserves its complete checked node, nested-copy and tracking
bound once. The existing cursor, base and publication shell remain intact.
Sufficient tracking buffers remain in place; exhausted buffers grow geometrically
under their exact prepaid array layouts. Their initialized pointer entries are
copied without cloning nodes or payloads, then the old allocation is physically
freed before its charge returns. The old and new buffers are both paid during
construction. Unused reservation returns before handoff. No intermediate map
state is published; one final commit exposes the assembled result.

A panic destroys the entire private successor; it does not return partially
modified work for retry. Destruction does not reserve rollback capacity, including
when the budget is completely full. As before, callback-bearing credit owners
require their original synchronous refund-notification deferral scope around
admission, mutation and lock release.

The planner uses a conservative structural/copy bound. It does not claim that
its bound is the minimum bytes for an insertion into already mutable nodes.

## Initial original root

The synchronous linear cell's permanent writer root now uses the same private
strong-only charged allocation as its reader and cursor. `InitialLayouts` reports
its exact concrete control-block layout; `InitialCharges` supplies the original
root and reader charges before construction. Detached owners retain that same
root and pointer identity. Final physical deallocation and payload destruction
precede refund. Initial reader construction unwind destroys the original data
before the empty shell charges return.

The public map constructor admits its initial node, root and reader allocations.
Platform-native mutex backing and runtime infrastructure remain explicitly
outside this claim. No guessed allowance, alternate mutex path or compatibility
constructor was added.

## Validation scope

The first development build exposed a test-fixture borrow error from moving a
map and its borrowed readers together into a closure. Cleanup now drops the
readers before moving the map. The next run passed 137 MV library tests, 12
admitted-map controls and the then-existing 11 linear-custody controls.
These development runs are not the final unchanged-source receipt.

Final focused runs under `dist/sumeragi-main-work/generation123/` pass:

- `mv-final/`: 181 actual workspace MV controls: 137 library, 13 admitted-map,
  13 linear custody, 5 EBR custody and 13 map-generation tests.
- `asan-final/` and `asan-skinny/`: the same 181 controls pass under native
  AddressSanitizer, both ordinary and reduced node geometry.
- `default/`, `skinny/`, `release/`, `async/`, `asan-current/`: actual vendored
  sources through the isolated test manifest pass 290, 288, 288, 319 and 290
  controls respectively. Each before/after/current inventory matches all 61
  vendor and harness inputs; `vendor-independent-join.json` records that check.
- Strict MV library/test Clippy passes; the actual workspace library build passes
  all 20 public API controls. The funded positive exercises a retained second
  edit and final publication; 17 negative controls reject unrestricted funded
  mutation or missing writer input with the intended compiler diagnostic.

The final MV, sanitizer, Clippy and API builds capture matching 7,165 compiled
inputs before and after. These are scoped library/engine checks, not a new full
workspace test run or real-network qualification. `validation123.json` joins the
final Core check, canonical ledger check, guards and source manifests separately.

Initial root-test failures remain under `mv/`, `mv-verified/` and `asan/`.
The allocation observer needed consistent root/reader allocation order and had
also been armed while allocating its own waiter fixtures. It now prepares those
fixtures before arming the original charge/layout records for construction.
Neither physical-free nor payload-before-refund assertions were weakened.
The vendor test's first compile failure was a missing test-local atomic import;
its failed log and corrected runs remain. Earlier passing vendor matrices are
retained under `*-initial/`, before the explicit buffer-replacement ordering edit.

New retained-edit controls exercise 128 irregular payloads through multiple
splits before one publication, exact input/private-state preservation on capacity
refusal and retry, foreign/busy/changed refusal before admission, allocation-free
abort with a full budget, later published-leaf copy unwind and private-leaf split
unwind. A further control injects a panic in the retired tracking buffer's
charge destructor: replacement bookkeeping is already installed, so abort still
reclaims every original private node and returns to the published baseline.
Vendor controls also inspect the exact original cursor address across
127 further edits/refusals, tree invariants and checked tracking growth arithmetic.
The real MV allocator witness checks every charged physical free before refund
and rejects ordinary payload Clone.

## Remaining production boundary

`mv::Storage` still owns an Untracked current map and a separate
`EbrCell<BTreeMap<K, Option<V>>>` undo journal. Block and transaction edits clone
preimages; transaction Drop currently performs restorative edits. This extension
is a necessary engine operation, not funded Storage integration.

Next, preserve a private transaction-start checkpoint so child abort destroys
only its new allocations and restores the original root without re-admission.
Current edits and first preimages must then be admitted together in explicitly
charged storage, with one original current/undo publication identity. Merely
committing each instruction would violate block atomicity. Arbitrary `get_mut`,
removal/iterator storage, actual model clone policies, native lock/runtime storage
and a configured aggregate State budget remain required. Final unchanged-candidate
four/seven-validator failure, restart and final-transaction qualification is open.
