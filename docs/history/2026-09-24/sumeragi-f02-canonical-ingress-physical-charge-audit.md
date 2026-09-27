# F02 canonical ingress physical-charge audit (2026-09-24)

This is a read-only source audit of the canonical executed-body request binder. No
implementation or test gate is closed by this record. The production ingress
connection remains disabled at
`crates/iroha_core/src/sumeragi/v2_canonical_executed_body_serve.rs:74`.

The exact-message comparison streams Norito output into a borrowed-byte writer,
so it no longer constructs a second message-sized buffer. The canonical-only
request has no certificate or signer PoPs, and its default Norito layout uses
compact lengths rather than packed structs or sequences. Its successful codec
comparison therefore has no identified message-sized heap buffer. This does
not make the surrounding ownership check allocation-free. The binder calls
`FairV2IngressOwnershipEvidence::validate_exact`, then
`matches_semantic_origin`, which calls `validate_exact` again. Each validation
hashes attempt histories. `fair_v2_ingress_attempt_cursor_hash` constructs a
`Vec` with capacity for only 24 bytes per attempt, then appends a domain,
count and 48 bytes per attempt, causing growth for populated histories. The
route-set equality check itself compares retained maps without a projection
allocation. Failure paths also build owned error strings.

No original physical memory lease currently crosses this seam.
`FairV2Ingress::try_push_at` accounts for encoded queue bytes and message count;
its `FairV2IngressResourceSnapshot` is immutable ownership evidence, not a
spendable reservation. `dequeue_selected_locked` subtracts the queued bytes
before returning `InboundBlockMessage`, and neither that envelope nor
`CanonicalExecutedBodyServeTask` carries a physical capacity lease. The
historical worker's `try_reserve_work` charges its rate and outstanding queue
after binding; it cannot fund ingress-side resident allocations. Admission
also materializes `Arc::<[u8]>::from(inbound.message().encode())` before its
queue-capacity cut. Adding a lease only at dequeue would leave that earlier
allocation unfunded.

The next atomic implementation cut must reserve a conservative authenticated
frame/class upper bound before admission encoding, or encode into a bounded,
fallibly reserved buffer. The original fair-ingress owner then needs a
transferable physical lease covering retained bytes and comparison scratch,
carried with the exact inbound and worker task until terminal release. Capacity
refusal must return the unchanged carrier for local retry, never classify it as
a consensus-invalid request. The known attempt-hash allocation can separately
be removed by streaming the same byte sequence through
`Hash::new_from_writer`; that improvement alone does not prove physical
funding for the complete ingress path.

Focused qualification should cover exact-capacity success, one-byte-short
refusal with identical message, route, ordinal and ownership evidence, retry
after release, tampered-message rejection, and cursor-hash parity across empty,
single- and multi-route histories. Existing canonical worker source tests
cover changed-message ownership and worker pressure, but not a physical
memory-lease boundary. No Cargo command was run for this audit.
