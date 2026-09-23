# Fixed Norito membership records

The located membership boundary now has one strict physical record codec and a
cached typed Norito framing owner. The framing owner resolves the declared schema
once, retains explicit flags, exact payload length and type-derived padding, and
shares the existing header/CRC writer with both bare-frame APIs. Record operations
borrow input or use bounded stack fields without per-record schema strings,
alignment copies or decoder-budget allocations. Construction and the caller's
buffers, writer/error allocations and storage resources remain separate funding
obligations. The obsolete padding helper was removed; no alternate codec or
compatibility reader was introduced.

Membership records are exactly 184 bytes: a standard Norito header and a declared
144-byte raw payload. Leaves carry key/value hashes and the exact value location;
branches carry their canonical split/prefix and both child references; heights
carry a nonzero u64. Generations are nonzero, offsets are record-aligned and their
complete extent is checked. Reads reject foreign generations and incomplete or
out-of-range readable extents. Reserved bytes, unknown versions/kinds, unmarked
hashes and malformed branches are rejected. Bad hashes are never normalized.
The [source-coupled layout](../../../crates/iroha_core/src/state/storage_transactions/block/membership_record.md)
defines all offsets and the independently calculated height-17 golden frame.

Logical authentication still belongs to the same map kernel and original
membership root owner. A correct frame/CRC cannot authenticate a substituted
node or height. Current and rollback readers preserve typed missing, corrupt,
wrong-content and missing-value failures; only proved tree absence returns None.
Codec success grants no segment lease, durability or State publication authority.

On one unchanged captured candidate, the full default Norito suite passes 1,348
runtime tests and ten documentation tests, with one existing ignored streaming
snapshot dumper. Eight new allocation-observed controls cover strict borrowed
validation, schema/flags/padding/length/CRC rejection, unaligned buffers, every
truncation, short/error/zero writes and cached identity. All 88 Core membership
controls pass, retaining all previous 79 and adding nine record controls. The
complete Core library test target compiles without warnings; the shipping library
check retains its existing warning for two unchanged unused State cache helpers.

The first full Norito run and initial Core build/record tests passed but exposed
an unused old padding helper. After removing it, the complete Norito suite, Core
compilation and all membership controls were repeated against final inputs.
Independent source review reproduced the golden frame and found no additional
concrete bounds or authentication defect; this is not an independent runtime run.
Commands, exact source/fixture hashes, executables, initial/final logs and gate
receipts are retained under `dist/sumeragi-main-work/generation165-records/`.

The test store contains encoded records in memory; it is not a durable or funded
production store. Next, retain one fully admitted append batch and original
file-generation leases through partial writes, uncertain sync, exact retry and
root publication. Install both roots inside the original complete State publisher,
authenticate restoration, and replace both Apply sites' full-history checkpoints.
Production retained Validate-to-Apply cutover and unchanged real four/seven-peer
loss/reordering/backpressure/leader/restart/final-transaction qualification remain
open. No L1–L6 completion or release readiness is claimed.
