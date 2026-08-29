# Autonomous recovery and capacity contract

Status: **Static-only evidence; production bindings are complete.** No TLC or Apalache result is claimed
by this contract. The dedicated checker performs
only deterministic text, JSON, config, and Rust source-anchor validation.

The compact `SumeragiV2AutonomousRecoveryCapacity` kernel records six safety
cuts:

1. An incomplete pointerless carrier at N keeps an exact bounded recovery
   identity when route/snapshot progress reaches N+1, unless exact terminal or
   receipt evidence discharges it.
2. A READY-bearing autonomous successor accepts its predecessor only from the
   exact per-incarnation WSV frontier, a MergeExecution receipt revalidated
   against its merge log and canonical carrier, or a `Current` receipt
   revalidated against the exact canonical block and results. Hash-only
   ownership is not an application proof.
3. Startup performs no capacity-consuming repair before carrier envelopes are
   reconstructed.
4. A certified frontier durably creates both pair and bundle capacity
   obligations; a restart reconstructs both envelopes before reopening.
5. Complete entrypoint claim-set, canonical association-stage, and prune
   transaction peaks are admitted before the first mutation. The prune peak
   includes reservation envelopes, exact intent and marker growth, and the
   remaining canonical pipeline-sidecar rewrite.
6. Debug append follows durable carrier reservation and is included in
   restart disk accounting.

The positive config is
`multilane_autonomous_recovery_capacity_fixed.cfg`. The negative controls are:

- `RouteLatestOnlySkip`
- `HashOnlyAutonomousPredecessor`
- `StartupRepairBeforeEnvelope`
- `FrontierMissingBundleEnvelope`
- `ClaimPeakAfterMutation`
- `AssociationPeakAfterMutation`
- `PrunePeakAfterMutation`
- `PrunePeakDropsReservationEnvelope`
- `DebugAppendBeforeCarrierReservation`
- `DebugRestartDropsAccounting`

The predecessor cut is source-bound to the READY/ordinary role dispatcher,
the ordinary-receipt repair filter that rejects READY-bearing certificates,
the autonomous exact-frontier gate, both current/predecessor Kura
MergeExecution receipt revalidators, and
`Kura::canonical_lane_block_predecessor_receipt_revalidates_without_sidecar_repair`.
The canonical receipt path holds the prune fence, admits only `Current`, and
revalidates the exact canonical block/results bytes; its paired regressions
prove that malformed replicated frontier bytes fail closed. These bindings
also reject use of hash-only snapshot helpers in the autonomous gate.

Exact incomplete-carrier recovery is source-bound through
`MergeLedgerLog::execution_entries_for_bounded_identities` and
`Kura::rebuild_post_wsv_lane_artifact_budget_reservations_on_startup`. The
first method performs one bounded chronological scan keyed by lane,
dataspace, incarnation, lane height, and proposal height. The second excludes
complete terminal outcomes, accepts only an exact canonical-carrier terminal
outcome or durability-attested MergeExecution receipt as alternate evidence,
uses route-latest only when it is the exact member, and sends older identities
through the bounded scan before rebuilding the exact carrier reservation.

Startup reconstruction is source-bound to `Kura::new_inner`: durable carrier
reconciliation plus post-WSV and certified/bundle envelope rebuilding precede
capacity-consuming frontier and bundle repair, configured-capacity validation,
both disk-accounting cache publications, and the constructor's successful
return.

The certified-frontier obligation is source-bound through
`Kura::persist_committed_lane_block_session_inner`, the exact pair planner,
the complete three-component plan, bounded historical preflight, aggregate
capacity admission, durability-gated component consumption, retirement
blocking, and
`Kura::rebuild_certified_bundle_capacity_reservations_on_startup`. The
reservation sum requires a stable and transient entry for every outstanding
component. Admission covers the frontier, certified pair, and autonomous
bundle before the first certified write; restart credits only exact
authenticated crash bytes while retaining the full in-memory envelope until
repair readback consumes it. The startup map is built locally and published
only after every active route and the complete configured-capacity projection
validate.
Its shared transient arithmetic is bound directly to
`CertifiedBundleCapacityReservation::reserved_bytes`.

The complete entrypoint claim-set peak is source-bound through
`Kura::preflight_autonomous_lane_entrypoint_claims_locked` and
`Kura::prepare_autonomous_lane_entrypoint_claims_with_limit_locked`. The
preflight projects the full ordered main/temp namespace, file-count limit, and
physical-byte peak before the caller opens its accounting mutation guard or
creates the first staged claim.

Lane-history compaction is source-bound through
`Kura::compact_lane_histories_through_merge_frontier_locked`. A previously
durable data/index rewrite is authenticated and promoted, and its exact live
accounting delta is published, before optional fresh-compaction capacity is
calculated. Only a new rewrite may return `CapacityBlocked`; malformed crash
evidence remains fail-closed and cannot be mislabeled as capacity pressure.

Prune admission is source-bound from
`KuraPruneCapacityAdmissionV3::transaction_peak_bytes` through
`KuraPruneCapacityAdmissionV3::required_peak_bytes`, live intent publication,
and startup recovery. The admitted absolute peak retains pending canonical,
post-WSV, certified-bundle, and autonomous-terminal reservation envelopes and
adds the exact intent, marker, and sequential pipeline-sidecar peaks without
deletion credit. Recovery rechecks that durable authority before repair, and
`Kura::truncate_pipeline_sidecars_for_prune` accepts only a remaining rewrite
authorized by the sealed V3 projection.

Canonical association-stage capacity is source-bound through the exact
`Kura::prepare_canonical_association_stage` projection, normal and
top-replacement budget calculations, both publication callers, and the
no-clobber publication boundary. The normal path adds stage
bytes to its complete required projection; replacement reserves the stage
above the larger of the retained-current and projected-after states before the
first stage write.

Debug append is source-bound from JSON encoding through the configured
autonomous mutation preflight and carrier-reservation calculation before the
accounting guard and bound append. Restart accounting is bound through the
validated `blocks.jsonl` file length, active/retired enforced and total root
scans, total-cache refresh, and startup cache publication.

Every split implementation file above is also bound to its exact `kura.rs`
`include!` owner. The Queue reconciliation snapshot, certified merge stage,
and lane-session entrypoint preflight remain stable production anchors rather
than invariant-chain evidence. No editor placeholder remains; the checker
rejects any reintroduced placeholder, source-token drift, split-owner drift,
or critical-order drift.

Formal-engine execution and integration with the release runners remain
separate release evidence. The model and mutation configs must not be cited as
passing TLC/Apalache evidence until those runners execute them and archive the
results.
