# Proposed native40 stored-opening context schedule

Status: source-coupled ownership proposal. The bounded source-to-writer and
canonical tail-replay transition is implemented in
[`ordered_storage_handoff_v1.rs`](../../collective/incremental_source_phase23_radix_range_v2/ordered_storage_handoff_v1.rs);
its [scope](../../collective/incremental_source_phase23_radix_range_v2/ordered_storage_handoff_v1.md)
retains the current 38-limb lineage and all native40 proof/qualification gates.
The remaining production joins below are not implemented or qualified. The reviewed reconciliation removes the unused
344-weight canonical-reopen placeholder and its early-context dependency. It
does not activate any production source, materializer, proof or release seal.
See [the dependency audit](canonical_reopen_reconciliation_audit_v1.md) for the
removed symbols, inspected equations and transitive-input limitations.

All paths below are relative to `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe`.

## Retained obligations

`Phase23RadixWitnessMaterializedV2` in
`collective/incremental_source_phase23_radix_range_v2.rs` owns the actual compact
radix snapshot, its validated materialization record/seal, and the original
`Phase23GlobalLookupSourceReplayEvidenceV1`. Prepared comparator and signed
planes consume this same owner while extending its original commitment session.
The two-file writer now attaches to that owner before the first stored plane.
Only its consuming stored-plane route can complete an attached writer; internal
unspooled preparation cannot mint a stored-proof authority.

The private context has three source axes: completed source-replay record,
source-opening record and radix materialization record, in addition to fixed
native40 topology, T256 basis and exact plane mapping. There is no replacement
weight vector, challenge or canonical-reopen authority. The remaining field
named `radix_range_record_digest` must receive the actual materialization record,
not a later Hyrax proof or post-equation envelope.

These records are available before comparator/signed production, but their
presence is not native40 source qualification: the earlier source-algebra
preflight still uses 38 limbs. The future native40 driver must consume the actual
governed source owner with the required 40-limb relation. It cannot adapt a
fixture digest or mark the earlier 38-limb record sufficient.

The existing source/packing same-opening statement remains the sole identified
native40 source-to-packing proof obligation. It reconstructs 344 D commitments
from original D-low/bD points, includes 1,032 signed commitments, derives its
existing `tau`, authentically rereads the canonical vectors, and verifies
`z*H = A + c*sum_o(tau^o*(C[o]-<v[o],G>))`. The original source-order `Csrc`
blindings cannot replace its required 1,376 derived packing-mask owner.

## Proposed original-source and writer lifetime

Keep the sole context encoder in
`global_lookup_statement_v1/vector_arithmetic_plane_openings_v1.rs`. A consuming
transition on the actual materialized owner must validate its radix record,
snapshot/seal and original replay Evidence; revalidate the source receipt and
source-opening record; and check the original source point prefix in that same
commitment session. Only this owner can lend a short-lived validated view to the
existing encoder. Raw digest arrays remain insufficient for production admission.

The resulting local storage context binds:

```text
fixed native40 topology, T256 basis, exact plane mapping,
actual source-replay record, actual source-opening record,
actual radix materialization record, exact existing context language
```

Use that immutable context to reserve and create the same fixed ordered two-file
plan. The driver retains the source, writer, original commitment session and
shared budget together. Later proofs cannot retag, rewrite or substitute either
file. The context is local storage identity, not a new proof transcript frame.

Run original low-digit, bD/bS, delta, beta/m and signed/negative producers in their
existing physical order. For each of the 9,288 stored planes, the driver itself
consumes all 32 value chunks and the original 33rd-slot rho/point tail, writes
every exact global slot successfully, and only then permits producer advancement.
The delta stage contributes original commitments but no stored plane. A callback,
detached receipt, or emitted-but-dropped chunk does not establish a successful
write. A temporary pre-I/O capacity refusal retains the original pair and
unconsumed source/cursor; authenticated corruption remains a separate error.

After cursor 9,288, exact global slot count 306,504 and original inventory
position 27,176, seal/authenticate the pair. Derive the existing plane record's
inventory subset root from the actual original point tickets in logical order:
physical ranges `[12040,12728)` and `[18576,27176)`. Do not rebuild a second
inventory or trust only a point decoded from a stored tail. Delta commitments
remain retained between those ranges. This subset is not the complete pre-z
inventory: Q-mask, multiplicity and inverse-product-mask producers remain needed.

The same owner must then supply actual authenticated value/tail slots to the
existing statement-3/5/8 witness consumers, using their exact one-shot role
permits. Each plane must match its original admitted point/rho and equation
`C_plane = sum_v(value_v*G_v) + blinding*H`. The current ordinal-only replay method
does not perform those reads or checks. Witness access occurs when its actual
prover needs it; it cannot be deferred until that prover has already finished.

`RadixWitnessProofBindingV2::finish_v2` now rejects an attached ordered writer.
Its internal unspooled path still ignores `next_comparator_plane` and does not
construct a stored-proof authority. Before a production transition can be inhabited it
must require the actual completed pair and all successful plane/slot writes
together with its real proof obligations. A cursor value without owned successful
storage is insufficient. Preserve all failed-write/unwind poisoning and scalar
zeroization in the original preparation owners.

## Preserve the existing later proof binding

Continue the same original pre-z inventory through its genuine remaining
producers and the existing native transcript chronology: pre-z commitments, sole
z, post-z inverse commitments, inverse-product rho and subsequent rounds.
Neither local storage completion nor its digest creates a challenge or proof
authority. Per-plane secret rho values, inverse-product batching rho and the
source/packing challenge tau have different roles and remain distinct.

Retain the stored pair/source/session identity through the actual proof owner
chain. The existing `RnsNativeDirectGlobalMembershipReplayV2` grants the purpose-
bound mutable authenticated source borrow used by
`verify_rns_native_source_packing_same_opening_owned_v2`. Its resulting
`RnsNativeSourcePackingSameOpeningPrerequisiteV1` retains the actual predecessor,
point root, source replay receipt and verified equation bindings. Any eventual
join with the stored pair must consume those same original owners and prove
their identity agreement; a caller-provided digest tuple cannot stand in for
the join. Its final composite transition still requires the genuine qPCS owner.

Do not feed local snapshot ciphertext roots, source materialized scalar digests,
stored records, post-equation source anchors, final aggregation schedules or
successor envelopes into earlier Fiat--Shamir challenges. Preserve the existing
successor-independent safe-core ordering in
`rns_native_comparator_range_carry_product.rs`,
`rns_native_direct_global_membership_handoff.rs` and
`rns_native_source_packing_same_opening.rs`. The current zero added wire bytes
and frames remains a requirement. No proof equation or public challenge changes
are proposed by this storage repair.

## Exact ownership boundaries for the next implementation

| Existing owner/file | Required change after review |
| --- | --- |
| `global_lookup_statement_v1/vector_arithmetic_plane_openings_v1.rs` | Derive the existing three-axis context only from a validated original-owner view; create its completed plane record only from the actual retained pair and original point subset. Keep production admission uninhabited until all true prerequisites exist. |
| `global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/ordered_snapshot_v1.rs` | Attach the existing writer to that owner with unchanged geometry and resource custody; keep its exact pair through all consuming stages. |
| `global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/replay_caps_v1.rs` | Advance only after real authenticated required slots and point/rho equations; retain a permit on local pre-I/O capacity refusal. |
| `collective/incremental_source_phase23_radix_range_v2.rs` | Own source plus writer, derive the validated early view, require all successful stored-plane emissions before future proof transition. |
| Its `prepared_comparator_plane_v1.rs`, `prepared_small_signed_plane_v1.rs` and `prepared_plane_opening_v1.rs` children | Route existing consuming emissions through the original driver without a second preparation path or weaker tail check. |
| `collective/incremental_source_phase23_source_algebra/global_lookup_source_replay_v1.rs` and `source_openings_v1.rs` | Validate original early source/opening axes and retain Evidence/session custody; no detached authority records. |
| `source_openings_v1/commitment_session_v1/retained_source_session_v1.rs` and its original signed-session child | Derive the exact comparator/signed subset from the original admitted point tickets without cloning/extracting/replacing inventory, masks or RNG. |
| Existing native source/packing predecessor and proof owner | Preserve actual authenticated source replay and the 1,376-owner equation; implement its separately required derived-mask owner before activating production proving. |

## Acceptance and remaining limits

- Independent byte vectors cover the three context axes, domain/field order,
  mapping and exact ordered plan. Zero/substituted/mixed-session axes reject.
  Compare existing public transcript messages/challenges to detect any accidental
  addition of private/local spool inputs.
- A positive test uses the actual authenticated source and original session,
  performs every required successful write, and reads back every required role
  through the same pair. Digest fixtures alone do not qualify this path.
- Missing/reordered value or tail chunks, a substituted original point/rho,
  second-file creation/seal failure, two mixed valid sessions and repeated
  permits reject at the actual owner/I/O/equation boundary. Preserve the budget
  distinction between temporary capacity refusal and corrupt authenticated data.
- Retain actual original source replay in the same-opening verifier and its
  existing hostile source/inventory/schedule/challenge controls. No new
  344-weight test is required because no surviving proof consumes those weights.
- Charge pair write/seal I/O (10,053,331,200 bytes), the three specified role
  passes (5,212,838,400 bytes), actual native source/packing replay and all other
  source/qPCS work through one eventual session ledger. The deleted unused
  180,707,328-byte placeholder pass is not a bound for the surviving native proof
  replay. Preserve 512 MiB/16 GiB/64 GiB ceilings. Tiny-file tests and arithmetic
  do not establish full-size runtime or RSS qualification.

The bounded removal clears an obsolete dependency only. Native40 source lineage,
actual stored witness custody/replay, remaining original inventory producers,
derived packing masks, qPCS/composite admission and production qualification
remain open and must be completed at their real owners.
