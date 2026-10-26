# Deferred Archive receipt custody

`Coordinator` retains exact incoming Credited bytes in the existing wallet archive and
selects their index through the protected Manifest before verification. The index is the
restart inventory; directory enumeration and host-supplied receipt identities grant no
custody authority. This is the current first-release schema, with no earlier-layout decoder.

The public sequence is `retain_deferred_credited`, `next_deferred_credited` on recovery,
and `verify_deferred_credited`. A dedicated completed receipt rejection removes only its
unverified FIFO membership and preserves the original record. Storage, missing source,
resource, installed-verifier and generic proof failures remain retryable. Verification
returns a delivery projection only after the verified membership is durably selected.

The first verified original for an actual Send becomes its permanent Archive anchor.
Native resolves the committed Send independently of incoming receipt claims and preserves
the receiver's open Request identity while authenticating its exact payer completion.
Request-only Archive intent retention preserves the anchor's originals after Send witness
collection. Newer valid evidence uses that anchor and never replaces it or starts another
Archive. Both ready and unverified indexes use immutable intake ordinals; delayed
verification does not reorder distinct ready Sends. Checked counts may exceed sixteen.

Each exact intake records its first optional Native-observed OS wall-clock timestamp once.
Duplicate or uncertain-publication retry never resets or backfills it. A fresh age view
reports due, not due, unavailable or regression, using a checked 24-hour difference. This
clock is local housekeeping data and is never a monetary-time input. Charging and the
sixteen-Send trigger are independent alternatives in the shared admission owner.

`deferred_archive_summary` is advisory data. `reconcile_deferred_archive` reopens the
exact source-selected anchor and removes ready membership only for its exact Archive
Complete outcome. Preparing, Pending, Unknown, Archived and missing output never become
Complete. Reconciliation does not require fresh-work capacity admission; actual storage
and publication failures still propagate. A generic resume result must be followed by
this exact query. Selected Pending recovery precedes Released-only metadata publication;
retention itself never acquires payment priority.

`drain_deferred_archive` consumes a scheduler turn, rereads current custody and reserves a
private permit bound to the selected source and exact Archive request. The shared scheduler
atomically checks exchange inactivity, no fold backlog, charging/count/age eligibility and
one Archive at a time. Native dispatch validates that permit before fresh execution; the
next turn must repeat admission. ABI33 bridge routes retain Core exchange leases and
accepted children, and the maintenance worker discovers, verifies and drains eligible
receipts. Swift and Kotlin expose the same exchange lifetime.

TODO: qualify automatic maintenance through the complete current signed catalog, rebuilt
SDKs and physical-device failure/recovery schedules. Source integration and component
tests do not establish genuine proof progress, platform durability or phone performance.

The authored mock tests cover durable selection/restart, malformed and foreign evidence,
operational failure retry, first-anchor custody, open Request completion, publication cuts,
exact completion reconciliation, fold metadata rebasing, FIFO and schema bounds. They use
explicit mock proof/custody fixtures. Their component execution is recorded in the
[evidence gate](../../../../../specs/kagemusha_evidence_gate.md#captured-component-checks-2026-10-1823);
matching genuine maintenance execution remains outstanding.
