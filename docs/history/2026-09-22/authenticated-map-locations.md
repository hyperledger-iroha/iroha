# Explicit locations for authenticated membership

The existing external map kernel now carries explicit node and value locations.
Root envelopes and branch children retain the exact physical reference returned
by their original store. Leaves retain the canonical value hash and its location.
Locations never enter logical commitments: the original root, branch, leaf,
transaction-key and height vectors are unchanged. Every load still authenticates
its content; missing records and malformed paths remain local failures.

This removes the hash-only interface's requirement for a separate content index.
There is one generic traversal/update implementation, with no compatibility alias,
implicit last-read cache or second decoder. Child-before-parent cold export
returns actual references through bounded traversal frames, without a growing
address map. Incremental updates retain the original divergent terminal and
untouched sibling references. Equal logical values return the exact original
root, even when the proposed value has another location, and write no nodes.

Core's original membership capabilities use the same located kernel for current
and rollback cuts. The height reader follows the authenticated leaf location,
then checks its exact nonzero canonical preimage and frontier. A new value written
elsewhere cannot repair a retained root's missing old location. Exact-slot repair
requires the physical owner's separate authority. Repeated successful writes may
return different locations while keeping all earlier references readable.

All 26 Crypto map and 79 current Core membership controls pass on the same 7,556
captured Rust inputs. They retain the previous 20 map and 77 membership selectors,
adding eight location/export controls; 17 Core controls exercise the root owner.
Both complete library test targets compile without warnings. The non-test Core
library check passes with the existing warning for two unchanged State cache
helpers. Tests cover different placements with identical commitments, missing or
wrong root/child/value locations, duplicate child hashes at different locations,
exact subtree retention, fresh-slot repair refusal, export failures and deepest
paths on the default thread stack. Test stores are fixtures, not disk admission
or durability evidence.

The first combined test build exhausted local disk space. Its unchanged-input
failure log is retained. Completed task binaries were losslessly archived with
original and decompressed hashes verified before removing the archived copies;
no source, live build or unrelated artifact was removed. Separate package builds
and their runtime captures then succeeded. Commands, source hashes, executables,
reviews and structural/hygiene receipts live under
`dist/sumeragi-main-work/generation164-located/`. Earlier counts remain attached
to their original candidates. Read-only cross-boundary review found no further
concrete defect; it is separate from the captured runtime execution.

This completes the located-kernel prerequisite for the
[authenticated height reader](authenticated-membership-values.md). Copyable
locations alone provide no storage lease, allocation credit or durable root
publication authority. The next physical owner must retain exact segment
generations, fund fixed Norito records and complete write batches, preserve the
same provisional batch through short writes/retry, and release refunds outside
outer publication fences. A codec audit found schema-name, alignment and decode
budget allocations in generic typed framing/decoding; decode limits do not fund
them. A fixed framed record must reuse Norito's declared schema/header kernel and
verify borrowed fixed fields with bounded errors, not introduce a second codec.

Complete State integration must install both roots within the original publisher
before writer release, authenticate restoration and replace both production Apply
checkpoint sites' full-history materialization. Retained Validate-to-Apply cutover
and unchanged four/seven-validator loss, reordering, backpressure, leader failure,
restart and final-transaction qualification remain open. No L1–L6 completion or
release readiness is claimed.
