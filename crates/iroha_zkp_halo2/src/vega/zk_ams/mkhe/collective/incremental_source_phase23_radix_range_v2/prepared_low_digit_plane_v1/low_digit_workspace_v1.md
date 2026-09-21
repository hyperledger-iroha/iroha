# Low-digit original workspace admission

`LowDigitWorkspaceV1` reserves exactly two 16,384-element T256 scalar
payloads plus `size_of::<LowDigitWorkspaceV1>()` on the existing original
proof ledger before the actual low-digit cursor is constructed. No new
ledger, RNG, source, point inventory or production authority is created.

The materialized-source entry validates its source/record/snapshot, obtains
this named reservation through the original retained `SourceComplete` phase,
and only then moves its evidence into the canonical read cursor. Capacity
refusal retains that same entire source, phase, inventory and entropy counters
for `retry_v1`; other refusal and every later error destroy the source.

The group vector and one prepared value vector use existing exact-capacity
zeroizing owners. The reservation is the last field in the driver; the prepared
plane is declared before that driver. Both vectors therefore drop before their
credit is released, including error and unwind. Successful plane finish keeps
the reservation for the next plane; final source return requires the complete
existing 11,696-plane/read schedule and destroys the reservation after vectors.

This is named payload admission only. Original source/spool allocations,
provider/ledger control allocation, emitted chunks retained by consumers,
MSM scratch, allocator metadata, stack/RSS and work units remain outside this
slice. Delta and other earlier consumers still need their own coherent joins.
No arithmetic or hardware execution path changes. Multiplicity generation,
the 38-to-40 source migration, qPCS redesign, governed keys, composite admission
and full resource/security qualification remain unfinished.

The `low_digit_workspace_` test filter covers seven controls, including actual
source-phase capacity refusal with production entropy counters, original-ledger
identity, real scalar buffers and erasure/unwind. Its source prefix is an
explicit isolated inventory fixture; it is not an authenticated full-source
positive or release qualification. Run the existing low-digit and retained
source controls against the same compiled source before accepting integration.
