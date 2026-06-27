<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Transition Notes

This log tracks the lingering **Phase B — Nexus Transition Foundations** work
until the multi-lane launch checklist finishes. It supplements the milestone
entries in `roadmap.md` and keeps the evidence referenced by B1–B4 in one place
so governance, SRE, and SDK leads can share the same source of truth.

## Scope & Cadence

- Covers the routed-trace audits and telemetry guardrails (B1/B2), the
  governance-approved configuration delta set (B3), and the multi-lane launch
  rehearsal follow-ups (B4).
- Replaces the temporary cadence note that previously lived here; as of the 2026
  Q1 audit the detailed report resides in
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`, while this page owns
  the running schedule and mitigation register.
- Update the tables after each routed-trace window, governance vote, or launch
  rehearsal. Whenever artefacts move, mirror the new location inside this page
  so downstream docs (status, dashboards, SDK portals) can link to a stable
  anchor.

## Evidence Snapshot (2026 Q1–Q2)

| Workstream | Evidence | Owner(s) | Status | Notes |
|------------|----------|----------|--------|-------|
| **B1 — Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | ✅ Complete (Q1 2026) | Three audit windows recorded; TLS lag from `TRACE-CONFIG-DELTA` closed during the Q2 rerun. |
| **B2 — Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | ✅ Complete | Alert pack, diff bot policy, and OTLP batch sizing (`nexus.scheduler.headroom` log + Grafana headroom panel) shipped; no open waivers. |
| **B3 — Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | ✅ Complete | GOV-2026-03-19 vote captured; signed bundle feeds the telemetry pack noted below. |
| **B4 — Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | ✅ Complete (Q2 2026) | Q2 canary rerun closed the TLS lag mitigation; validator manifest + `.sha256` capture slot range 912–936, workload seed `NEXUS-REH-2026Q2`, and the recorded TLS profile hash from the rerun. |

## Quarterly Routed-Trace Audit Schedule

| Trace ID | Window (UTC) | Outcome | Notes |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ Pass | Queue-admission P95 stayed well below the ≤750 ms target. No action required. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ Pass | OTLP replay hashes attached to `status.md`; SDK diff bot parity confirmed zero drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ Resolved | TLS profile lag closed during Q2 rerun; telemetry pack for `NEXUS-REH-2026Q2` records TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (see `artifacts/nexus/tls_profile_rollout_2026q2/`) and zero stragglers. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ Pass | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest under `artifacts/nexus/rehearsals/2026q1/` (slot range 912–936) with agenda in `artifacts/nexus/rehearsals/2026q2/`. |

Future quarters should add new rows and move the
completed entries to an appendix when the table grows beyond the current
quarter. Reference this section from routed-trace reports or governance minutes
using the `#quarterly-routed-trace-audit-schedule` anchor.

## Mitigation & Backlog Items

| Item | Description | Owner | Target | Status / Notes |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Finish propagating the TLS profile that lagged during `TRACE-CONFIG-DELTA`, capture rerun evidence, and close the mitigation log. | @release-eng, @sre-core | Q2 2026 routed-trace window | ✅ Closed — TLS profile hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` captured in `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; rerun confirmed no stragglers. |
| `TRACE-MULTILANE-CANARY` prep | Schedule the Q2 rehearsal, attach fixtures to the telemetry pack, and ensure SDK harnesses reuse the validated helper. | @telemetry-ops, SDK Program | Planning call 2026-04-30 | ✅ Completed — agenda stored at `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` with slot/workload metadata; harness reuse noted in the tracker. |
| Telemetry pack digest rotation | Run `scripts/telemetry/validate_nexus_telemetry_pack.py` before each rehearsal/release and log digests next to the config delta tracker. | @telemetry-ops | Per release candidate | ✅ Completed — `telemetry_manifest.json` + `.sha256` emitted in `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests copied into the tracker and evidence index. |

## Config Delta Bundle Integration

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` remains the
  canonical diff summary. When new `defaults/nexus/*.toml` or genesis changes
  land, update that tracker first, then mirror the highlights here.
- Signed config bundles feed the rehearsal telemetry pack. The pack, validated
  by `scripts/telemetry/validate_nexus_telemetry_pack.py`, must be published
  alongside the config delta evidence so operators can replay the exact
  artefacts used during B4.
- Iroha 2 bundles remain lane-free: configs with `nexus.enabled = false` now
  reject lane/dataspace/routing overrides unless the Nexus profile is enabled
  (`--sora`), so strip `nexus.*` sections from single-lane templates.
- Universal merge-ledger rollout policy (Nexus mode): release the async merge
  schema and routing contract via genesis reset + full node redeploy. In-place
  migration and backward wire compatibility are intentionally out of scope for
  this rollout.
- Keep the governance vote log (GOV-2026-03-19) linked from both the tracker and
  this note so future votes can copy the format without re-discovering the
  approval ritual.

## Launch Rehearsal Follow-Ups

- `docs/source/runbooks/nexus_multilane_rehearsal.md` captures the canary plan,
  participant roster, and rollback steps; refresh the runbook whenever the lane
  topology or telemetry exporters change.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` lists every artefact
  checked during the April 9 rehearsal and now carries the Q2 prep notes/agenda.
  Append future rehearsals to the same tracker instead of opening one-off
  trackers to keep evidence monotonic.
- Publish OTLP collector snippets and Grafana exports (see `docs/source/telemetry.md`)
  whenever the exporter batching guidance changes; the Q1 update bumped the
  batch size to 256 samples to prevent headroom alerts.
- Multi-lane CI/test evidence now lives in
  `integration_tests/tests/nexus/multilane_pipeline.rs` and runs under the
  `Nexus Multilane Pipeline` workflow
  (`.github/workflows/integration_tests_multilane.yml`), replacing the retired
  `pytests/nexus/test_multilane_pipeline.py` reference; keep the hash for
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `5434666dee1a353467a927189b27422a9c85366a14134ba54b3be83a1beed13d`) in sync
  with the tracker when refreshing rehearsal bundles.

## Runtime Lane Lifecycle

- Runtime lane lifecycle plans and enabled Nexus config swaps now validate
  dataspace bindings and routing-policy targets before mutation. Plans/configs
  that would leave the default route, a rule lane, or a rule dataspace
  unresolved fail atomically. Rule lanes without an explicit rule dataspace are
  validated against `nexus.routing_policy.default_dataspace`, and explicit
  rules cannot target autoscale-owned lanes. Fallible router resolution
  (`try_route*`) rejects the same autoscale-owned explicit-rule target even if a
  corrupted in-memory policy bypasses config validation. Malformed lifecycle
  plans that repeat addition ids, repeat addition aliases, or repeat the same
  retired lane id are rejected before catalog mutation; Kura/tiered storage
  reconciliation failures also leave the catalog unchanged. Alias relabels fail
  before catalog or tiered-state changes when the target Kura segment is already
  occupied, so storage cannot remain under the old slug while the catalog
  advances. The helpers prune cached lane relays for retired lanes so
  merge-ledger synthesis does not reuse stale proofs. Merge-candidate synthesis
  and `State::commit_merge_entry` also revalidate cached relay snapshots against
  the active Nexus lane/dataspace catalog, so stale relays for removed lanes,
  rebound dataspaces, or dataspace entries missing from the active catalog are
  ignored or rejected even if cache state was populated
  outside the normal relay-admission path. State keeps a latest-snapshot index
  per `(lane_id, dataspace_id)` across active-only merge entries. Candidate
  synthesis uses that index to avoid reintroducing unchanged lanes that were
  omitted from the newest entry, and commit validation rejects equal or lower
  lane heights, preventing higher-epoch merge replays from reusing old
  settlement material.
- Apply plans through the Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) to add/retire lanes without restart; routing,
  TEU snapshots, and manifest registries reload automatically after a successful plan.
- External Nexus config swaps and manual lifecycle plans reject
  autoscale-managed ownership claims, and manual lifecycle plans also reject
  attempts to retire valid autoscale-managed lanes. The consensus autoscaler is
  the only path that may mint or retire healthy lanes with autoscale-managed
  ownership metadata; invalid owned lanes may be explicitly retired for repair.
  When autoscale is enabled, manual lifecycle additions and full
  config swaps also reserve the configured
  `autoscale.min_lanes..autoscale.max_lanes` elastic id range for the
  autoscaler, so operator-managed base/governance/zk lanes cannot silently
  consume future scale-out ids. The routing `default_lane` must stay outside
  that range too, so an autoscale-owned elastic lane cannot become the default
  route's stable base anchor. Full config swaps may preserve active
  autoscale-managed lanes unchanged, but swaps that add, mutate, omit, or
  replace one are treated as
  manual autoscale-lane changes and fail atomically. Preserved
  autoscale-managed lanes must also remain inside the configured
  `autoscale.min_lanes..autoscale.max_lanes` id range and stay bound to
  `nexus.routing_policy.default_dataspace`, so config changes cannot strand an
  owned elastic lane outside the range or dataspace the autoscaler can manage.
  A config swap also cannot disable `autoscale.enabled` while autoscale-managed
  lanes are still present; otherwise the only owner allowed to destroy them
  would be disabled. Static TOML parsing rejects
  `nexus.autoscale.enabled = true` unless `nexus.enabled = true`; it also
  rejects the reserved `autoscale.managed` lane metadata key and manual lanes
  in the enabled elastic id range before runtime for the same reason. Runtime
  lifecycle validation checks every lane in the post-plan catalog too, so
  unrelated plans cannot
  preserve a pre-existing manual lane in the elastic range or an
  autoscale-owned lane with malformed metadata, disabled autoscale, an
  out-of-range id, or a non-default dataspace binding. An explicit retire of a
  manual elastic-range lane remains available as the manual-lane repair path.
  Invalid autoscale-owned lanes can also be repaired by an explicit lifecycle
  retire, while valid autoscale-owned lanes remain protected from manual
  retirement and unrelated lifecycle updates cannot carry corrupted owned lanes
  forward.
  Canonical dataspace routing also skips any lane that claims autoscale
  ownership, including malformed claims, so dataspace-targeted writes,
  settlements, and permission-scope routes require a non-autoscale lane for
  their dataspace anchor and fail closed when only autoscale-owned lanes exist.
  The internal autoscale lifecycle path also validates that newly owned lanes
  are deterministic public elastic lanes in the configured default dataspace
  (`autoscale.managed = true`, positive `autoscale.created_height`, and
  `elastic-lane-{id}` alias) and refuses to add or retire unmanaged/manual
  lanes, malformed autoscale-owned lanes, or managed lanes outside the
  configured elastic id range or default dataspace.
  Runtime `State::set_nexus` applies the same disabled-profile guard as the
  parser, so direct actual-config swaps cannot disable Nexus while carrying
  lane, dataspace, or routing overrides, enabling autoscale, enabling
  lane-relay emergency overrides, or enabling the relay worker. The relay
  worker also requires lane-relay-burn settlement with a canonical sponsor
  account at the state boundary, matching the parser guard. Activated
  lane-relay-burn fee receipts
  require a canonical sponsor even when the relay worker is disabled, and
  emergency relay multisig thresholds cannot exceed member count. Per-dataspace
  fee sponsors also require fee sponsorship, enabled Nexus, and a dataspace key
  present in the active dataspace catalog. Runtime config swaps also mirror the
  parser's fee-shape checks: the Nexus fee asset selector must be the canonical
  XOR asset definition id or `xor#universal`/`xor#universal.universal` and is
  trimmed to the parser-normalized selector, the fee sink literal cannot be
  blank, blank canonical sponsors are treated as absent, and each sponsored
  contract allowlist entry must name a contract target plus at least one
  non-empty entrypoint.
- Autoscale scale-out creates elastic lanes in the configured default
  dataspace. Those lanes are admitted into default traffic only when their
  visibility is public, their metadata marks them as managed, their alias
  matches `elastic-lane-{id}`, their creation height is valid, and their
  dataspace still matches the configured default dataspace. Default traffic then
  shards by transaction hash across the default lane plus those valid elastic
  lanes only when live Nexus and autoscale are enabled, the default lane remains
  outside the elastic id range, and the managed lane id remains in
  `autoscale.min_lanes..autoscale.max_lanes`. If autoscale is disabled or no
  eligible elastic lane remains, or if Nexus itself is disabled, live routing
  falls back to the default lane. If the active elastic range contains a manual
  lane, malformed autoscale-managed lane, or managed lane outside the default
  dataspace, live routing also fails closed to the default lane until the
  catalog is repaired; catalog-only routing without a live Nexus
  state view also falls back to the base default lane instead of sharding over
  a stale router snapshot. The integration router harness pins the same disabled
  Nexus/autoscale gates and in-range corruption fallback at the public
  `ConfigLaneRouter::route_with_view` boundary while preserving the
  enabled-autoscale sharding path.
  Block autoscale application also requires both enabled Nexus and enabled
  autoscale, so corrupted actual state with either gate disabled cannot create
  or retire elastic lanes. Autoscale catalog changes are staged inside the
  block scope and published to committed Nexus state and lane storage geometry
  only during `StateBlock::commit()` after transaction-height validation, so a
  height-mismatch validation failure cannot leak a lane addition, retirement,
  runtime reset, or cooldown marker. DA commitment, shard/receipt cursor,
  confidential-compute receipt, and pin-intent indexes prepared during block
  application are staged behind the same commit validation boundary, so a block
  that fails height validation cannot partially publish those runtime or world
  indexes either. The commit path runs the fallible autoscale lifecycle
  preparation before publishing staged DA indexes, so storage errors during
  elastic-lane geometry reconciliation cannot leak DA runtime, query state, or
  block-local WSV cleanup from an uncommitted block. Operator-driven lifecycle
  and config-swap retirements use the same preflight barrier: Kura or tiered
  retire conflicts preserve the committed emergency overrides, AXT replay
  entries, verified relay state, public-lane validator activity, and
  public-lane economic rows that would otherwise be reset. State-level lane
  geometry reconciliation dry-runs
  both Kura block/merge storage and tiered-state snapshot geometry before
  either backend is mutated, then prepares tiered-state storage before Kura
  block/merge provisioning, so a Kura path conflict, tiered path conflict,
  occupied relabel target, retired archive-root conflict, or bad tiered cold
  root fails lane creation/relabel/retirement before the other storage backend
  creates new lane artifacts. Deterministic autoscale scale-out and scale-in
  use that same ordering at commit time, and staged DA indexes remain behind
  the failure boundary when a new or retiring elastic lane hits a Kura or tiered
  conflict.
  State-free router fast paths defer unmatched no-target default traffic to the
  live Nexus route even when unrelated policy rules are present, so unmatched
  rules cannot bypass the autoscale elastic range. Corrupted managed lanes
  outside the configured elastic id range are ignored instead of receiving
  default traffic. Fallible default-route resolution rejects a
  corrupted in-memory policy whose default lane claims autoscale ownership
  before returning a route or routing plan.
- Autoscale scale-in runs through the block-local lifecycle path during block
  application, so retiring a managed elastic lane also prunes lane-scoped relay
  state, verified relay contract-state records, merge-history checkpoints, DA
  commitment, confidential-compute, pin-intent, DA receipt cursor, and DA shard
  cursor indexes owned only by the retired lane, plus emergency-validator state
  in the same committed state transition. Verified relay cleanup is exact-keyed
  to the canonical relay state key and its matching contract-map key for the
  decoded record. Undecodable lowercase exact canonical relay keys are parsed
  by key lane and pruned when they name a reset lane, while arbitrary prefixed
  siblings, uppercase digest variants, and malformed contract-map rows remain
  inert because their lane cannot be safely inferred. Hydration from
  contract-visible state also scans only lowercase exact canonical relay keys
  before decode, so arbitrary prefixed state cannot drive relay-cache admission
  attempts. Block-local resets also drop any verified relay records staged
  earlier in the block for reset lanes before commit-time hydration, treating
  either the public relay reference lane or the embedded envelope lane as reset
  ownership. The same reset prunes AXT replay ledger entries keyed by a retired
  handle target lane while
  preserving cross-lane replay guards whose handles target surviving lanes.
  Public-lane stake-share rows and reward records keyed by or carrying the
  reset lane, plus reward-claim cursors keyed by the reset lane, are removed as
  live economic indices so a recreated lane id cannot inherit stale reward
  epochs or claim cursors. Operator staking status snapshots for reset lanes are
  cleared with the same reset, preventing stale bonded or slash totals from
  surviving lane-id reuse in status surfaces.
  The block-local path refreshes AXT policy caches after the lane catalog
  changes, retargeting Space Directory-derived entries when directory data is
  present and pruning explicit cache entries whose target lane no longer exists.
  Scale-in also requires a
  resolvable default route, default
  route autoscale capacity strictly above `autoscale.min_lanes`, and a complete
  historical sample window before any managed elastic lane can be retired, so
  unrelated manual lanes cannot make scale-in eligible.
- Fresh lane-id additions also reset any rehydrated volatile lane state for
  that id. This closes the restart edge where Kura can reload historical merge
  snapshots or persisted DA shard cursor journals for a lane that was retired
  before the node came back up, then the autoscaler recreates the same lane id
  at a lower local height. Lifecycle plans that retire and add the same lane id
  in one transaction are treated as a fresh lane incarnation too, so merge
  history, lane relays, verified relay contract-state records, DA receipts, pin
  intents, persisted DA shard cursors, and operator staking status are reset
  even when the replacement keeps the same dataspace and runtime geometry.
  Contract-state cleanup only removes the exact verified relay keys owned by
  the old incarnation, so
  noncanonical prefixed state cannot be mistaken for reset-owned evidence.
- Autoscale utilization counts committed fragments, not just external
  transaction envelopes. The active block uses the in-flight execution counter,
  while historical window samples read the persisted committed-fragment total
  from block results and keep external transactions as a legacy floor. Block
  validation rejects forged non-zero committed-fragment totals that do not
  match re-execution. Latency ratios and utilization use widened deterministic
  integer intermediates and saturate only the final permille value, so extreme
  counters or timestamps cannot wrap or deflate a hot sample into a cold one.
  Utilization capacity is measured against the default route's eligible lanes
  only, so unrelated
  governance, zk, manually managed, or out-of-range autoscale-managed lanes do
  not dilute default-lane scale-out decisions. Live routing disables elastic
  default-route sharding when runtime autoscale bounds are invalid or the
  default lane is inside the elastic range, keeping no-target traffic on the
  configured default lane until runtime state is repaired. Scale-out eligibility
  uses that same default-route autoscale
  capacity instead of total catalog length, so unrelated manual lanes and
  corrupted out-of-range managed lanes do not inflate hot default-traffic
  capacity. Runtime autoscale prechecks fail closed before plan construction
  when the active elastic range is occupied by a manual or malformed managed
  lane, or when autoscale-owned corruption exists outside the configured range,
  requiring an explicit repair retire instead of creating or retiring lanes
  around the corrupted entry. Missing Kura blocks, including gaps inside longer
  decision windows, or equal/backward block timestamps make the historical
  sample window incomplete, and incomplete windows fail closed rather than
  extrapolating load from the current block alone or clamping bad timing
  evidence into synthetic hot/cold samples.
- Autoscale config parsing rejects zero lane/window/target values, inverted
  `min_lanes`/`max_lanes`, configured maxima above the compiled safety cap, and
  scale-in thresholds that are not strictly below scale-out thresholds. Ratio
  thresholds must also round to at least one permille, so tiny positive values
  cannot become zero thresholds, and scale-in ratios must still round below
  scale-out ratios at permille precision. The programmatic `set_nexus`,
  lifecycle, and block application paths repeat the lane-bound safety-cap
  checks, and block application also repeats the effective-ratio checks.
  Autoscale transitions are skipped if runtime state contains an excessive
  `max_lanes`, non-finite ratios, zero or sub-permille ratios, or thresholds
  whose effective permille hysteresis has collapsed. A future
  `last_transition_height` is treated as an active cooldown and suppresses
  create/retire transitions without overwriting the marker. If the internal
  lifecycle helper rejects an add/retire plan after a hot/cold decision, the
  catalog remains unchanged and `last_transition_height` is not advanced, so
  failed autoscale attempts cannot pin cooldown. When configured windows
  conflict, a hot longer scale-out window takes precedence over a cold shorter
  scale-in window so capacity is added rather than retired in the same block.
- Native AMX control-plane ingress now rejects votes whose envelope phase does
  not match the signed attestation body, votes transported by a different
  `PeerId` than the signer, non-BLS-normal signer identities, or malformed
  individual signatures before the vote reaches the proposer-side exact-body
  session cache. The existing live-PoP check remains state-dependent and runs
  immediately before cache insertion. The native AMX QC builder also verifies
  each vote signer and individual signature before BLS aggregation, so polluted
  builder inputs cannot produce a receipt that only fails at later block
  validation. Native AMX vote caches now bound exact attestation-body buckets
  within each session and evict the oldest bucket FIFO, preventing a single
  source/plan from retaining unbounded retried or adversarial bodies while
  proposer collection is pending. Operators tune the session and per-session
  body-bucket caps through
  `sumeragi.advanced.native_amx.session_cache_max` and
  `sumeragi.advanced.native_amx.session_body_bucket_max`, which default to
  `1024` sessions and `256` body buckets.
- When a lifecycle update retires a lane, queue reconfiguration reroutes
  pending autoscaled default-route traffic onto the surviving elastic/default
  candidates. If a lifecycle update otherwise removes the only route that a
  pending transaction depended on, queue reconfiguration rejects the transaction
  immediately and clears cached routing decisions, full routing plans, and
  routing-ledger hints plus TEU backlog accounting so stale retired-lane
  metadata cannot leak into proposal, replay, or scheduler-pressure paths.
- State-aware Torii, gossip, admission, consensus requeue, and block-requeue
  routing synchronize the queue router, routing policy, and cached catalogs from
  current Nexus state before resolving plans, then validate against the current
  lane/dataspace catalogs. Caller-provided or cached routing plans must resolve
  every coordinator and participant leg against current catalogs and exactly
  match a freshly recomputed full plan for the same transaction, so stale plans
  are rejected even when the old lane still exists in the active catalog. This
  lets a freshly added lane route before an external cache refresh and prevents
  stale queue-local policy or lane metadata from surviving retirement or policy
  changes.
  Queue plan-journal replay uses the same committed-state synchronization and
  full-plan comparison, tombstoning stale Native AMX participant legs even when
  the old participant lane still resolves against the active catalog.
  Queue reconfiguration after committed Nexus changes also refreshes cached
  full Native AMX routing plans for pending transactions through both state-
  and view-backed reconfiguration entry points, so participant legs cannot
  remain stale behind an unchanged coordinator route.
  Block requeue discards a stale process-global routing-ledger plan after a
  failed ledger-sourced reinsertion, so the next recovery pass recomputes
  Native AMX participant legs from current committed state instead of replaying
  the same stale hint.
  Lane TEU deferral also returns the full routing plan for consensus requeue, so
  deferred Native AMX transactions keep participant legs instead of requeueing
  as coordinator-only work.
  Transaction gossip route hints also resolve against the active dataspace
  catalog before broadcast or reinsertion, so dangling lane bindings left after
  dataspace removal are rejected alongside missing lanes and lane/dataspace
  mismatches. Gossip batch partitioning also falls back to actual Norito length
  for variable-size full routing plans, so Native AMX participant legs are
  preserved across the gossip plane instead of being requeued indefinitely.
  Outgoing gossip batch assembly also refreshes cached full routing plans from
  current Nexus state before emitting route hints, so Native AMX participant
  drift is corrected before serialization.
  Torii submit-transaction proxy receivers now apply the same full-plan
  comparison to ingress hints, so Native AMX participant drift is rejected even
  when the coordinator route is unchanged. They also validate redundant routing
  hint fields before comparison: coordinator and participant roles must be
  canonical, and a Native AMX hint's advertised `plan_digest` must match the
  digest recomputed from its route legs, so forged proxy hints are rejected
  instead of normalized.
- Proposal assembly now uses the same live Nexus snapshot and autoscale elastic
  range when it refreshes routing vectors for proposal sidecars and execution
  context assembly, so autoscaled default-route transactions cannot be
  re-collapsed to the base default lane after queue admission. The refresh
  compares full routing plans, so Native AMX proposal vectors also replace stale
  participant legs even when the coordinator route is unchanged. Proposal
  size-cap trimming preserves full routing plans for removed transactions too,
  so overflow requeue keeps Native AMX participant metadata.
- Block validation and block execution also recompute execution-context routing
  and per-lane transaction summaries with the live Nexus autoscale range.
  Matching elastic execution contexts validate, while stale base-lane contexts
  for live elastic default-route transactions fail closed. When Nexus is
  disabled, or when active elastic-range catalog corruption forces base-lane
  routing, stale elastic execution contexts fail closed too. Native AMX
  execution contexts also compare every committed coordinator and participant leg
  with the recomputed full plan, so stale participant routes fail before receipt
  validation. Scheduler committed TEU telemetry is also attributed from the
  validated block routing vector, so stale process-local routing-ledger hints
  cannot skew lane load metrics after reroutes or autoscale transitions.
- Torii global pipeline-status reads now use cached routing-plan hints only as a
  probe. Hinted `Queued`, `Approved`, `Committed`, or malformed successful
  responses fall through to full fanout, while only terminal hinted statuses can
  short-circuit, preventing stale retired-lane status caches from hiding newer
  terminal results on the active lane.
- Incoming Torii read and verified-query proxy requests now validate the
  ingress-selected lane/dataspace against the receiver's current Nexus catalogs
  before local read execution. Active routes still execute on the receiver to
  avoid multi-hop proxy cascades during brief authority-view skew, but
  retired-lane or lane/dataspace mismatch hints fail as `route_unavailable`
  with `stale_route` diagnostics.
- Stateful transaction validation without an embedded route now uses the active
  Nexus full-plan resolver before applying lane policy checks. This keeps
  direct validation entrypoints aligned with autoscaled default-route lanes
  instead of using a catalog-only coordinator route.
- Startup replay of the pending queue-plan journal runs through the same
  current-state synchronization before comparing persisted plans. Stale
  journaled routes are tombstoned when committed Nexus policy has moved them,
  even if the old lane still exists in the active catalog. Stale elastic
  default-route plans are also tombstoned when active elastic-range corruption
  forces live routing back to the base lane after restart.
- Operator guidance: when a plan or config swap fails, check for missing
  dataspaces, routing-policy lanes that were retired/rebound without a matching
  policy update, or storage roots that cannot be created (tiered cold root/Kura
  lane directories). Fix the catalog/policy/backing paths and retry; successful
  plans re-emit the lane/dataspace telemetry diff so dashboards reflect the new
  topology.

## NPoS Telemetry & Backpressure Evidence

Phase B’s launch-rehearsal retro asked for deterministic telemetry captures that
prove the NPoS pacemaker and gossip layers stay within their backpressure
limits. The integration harness at
`integration_tests/tests/sumeragi_npos_performance.rs` exercises those
scenarios and emits JSON summaries (`sumeragi_baseline_summary::<scenario>::…`)
whenever new metrics land. Run it locally with:

Live NPoS lane-scope inference is intentionally active-record-only:
`PendingActivation`, `Jailed`, `Exiting`, `Exited`, and `Slashed`
public-lane validator records are retained for lifecycle/audit history, but
they must not constrain recovery candidates or active topology selection after
lane retirement, rebinding, or autoscale scale-in. Lane reset paths also mark
revivable `PendingActivation`, `Active`, and `Jailed` records for the reset
lane as `Exited`, treating either the storage key lane or embedded record lane
as reset ownership, so a retired lane cannot promote stale pending validators
or carry stale active validators into a future incarnation of the same lane id.
Live topology, stake snapshot, validator-election profile, due-activation,
released-exit sweeping, penalty-locator, staking-admission, direct staking
mutations, reward bookkeeping, slash handling, peer/account cleanup guards,
multisig account-rekey rewrites, Soracloud runtime-authority, and host-finance
stake-accounting derivations also require each public-lane validator row's
storage key `(lane_id, validator)` to match the embedded
`PublicLaneValidatorRecord`, so a malformed stale row cannot inflate quorum
weight, auto-promote to active, exit-finalize, mutate bonded stake, receive
reward epochs, enter a live roster, reserve validator capacity or peer
bindings, block account cleanup, get force-exited by peer cleanup, grant
runtime authority, get repaired into a live row during account rekey, or map a
consensus offender to a mismatched validator slot.
The Torii public-lane validator app endpoint uses the same key/record filter
before returning staking-backed rows, preserving manifest fallback only when the
requested lane has no valid staking rows. Torii stake-share and reward app
endpoints apply the matching exact-key filter for `(lane_id, validator, staker)`
and `(lane_id, epoch)` economic rows before serializing account-facing state.
Public-lane stake-share rows, reward records, and reward-claim cursors are live
economic indices rather than audit history; the same reset paths delete
reset-owned rows (by storage key or embedded lane where present) and clear
operator staking status for reset lanes while leaving unchanged-lane economic
state and status intact. Failed lane-retirement
preflight leaves those rows and the reset-lane validator/emergency/AXT/relay
state committed until storage geometry can move atomically.
Authoritative lane validator and peer resolution additionally rejects any lane
absent from the active derived lane config, or whose dataspace is absent from
the active dataspace catalog, so stale manifest bindings or active public
validator records cannot revive a removed or rebound lane committee. The global
NPoS epoch stake snapshot applies the same active lane/dataspace guard before
public validator records can affect topology scope, council member mapping, or
stake-ranked candidates. With Nexus enabled, active-topology derivation,
roster-unavailability recovery candidate selection, block-sync sender-lane
roster caching, and block-apply peer reconciliation also intersect
validator-derived lane scopes with the active lane/dataspace catalogs. Commit
stake snapshot construction and roster-validation cache refreshes that run with
state access now also filter stake maps to active Nexus lanes, preventing a
higher stale unknown-lane stake record from overriding a validator's active-lane
weight. State-backed QC and block-sync validation fallback paths now thread the
same active-lane set into missing-snapshot recomputation, so quorum checks cannot
fall back to stale unknown-lane stake when cached stake snapshots are absent.
Live NPoS commit quorum checks, commit-root signer selection, NEW_VIEW
aggregation, and repair fanout/coverage telemetry use the same active-lane set
for world-backed stake quorum and coverage calculations, closing the last
state-backed quorum path where stale unknown-lane stake could skew signed-stake
math.

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Set `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K`, or
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` to explore higher-stress topologies; the
defaults mirror the 1 s/`k=3` collector profile used in B4.

| Scenario / test | Coverage | Key telemetry |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Blocks 12 rounds with the rehearsal block time to record EMA latency envelopes, queue depths, and redundant-send gauges before serialising the evidence bundle. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Floods the transaction queue to ensure admission deferrals kick in deterministically and that the queue exports capacity/saturation counters. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Samples pacemaker jitter and view timeouts until it proves the configured ±125 ‰ band is enforced. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Pushes large RBC payloads to the soft/hard store limits to show sessions and byte counters climb, back off, and settle without overrunning the store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forces retransmits so the redundant-send ratio gauges and collectors-on-target counters advance, proving the telemetry the retro requested is wired end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Drops deterministically spaced chunks to verify backlog monitors raise faults instead of silently draining payloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Attach the JSON lines the harness prints together with the Prometheus scrape
captured during the run whenever governance asks for evidence that backpressure
alarms match the rehearsal topology.

## Update Checklist

1. Append new routed-trace windows and retire old ones as quarters roll over.
2. Update the mitigation table after every Alertmanager follow-up, even if the
   action is to close the ticket.
3. When config deltas change, update the tracker, this note, and the telemetry
   pack digest list in the same pull request.
4. Link any new rehearsal/telemetry artefacts here so future roadmap status
   updates can reference a single document instead of scattered ad-hoc notes.

## Evidence Index

| Asset | Location | Notes |
|-------|----------|-------|
| Routed-trace audit report (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Canonical source for Phase B1 evidence; mirrored for the portal under `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Contains the TRACE-CONFIG-DELTA diff summaries, reviewer initials, and GOV-2026-03-19 vote log. |
| Telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | Documents the alert pack, OTLP batch sizing, and export budget guardrails tied to B2. |
| Multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lists Apr 9 rehearsal artefacts, validator manifest/digest, Q2 prep notes/agenda, and rollback evidence. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Records slot range 912–936, seed `NEXUS-REH-2026Q2`, and artefact hashes for governance bundles. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash of the approved TLS profile captured during the Q2 rerun; cite in routed-trace appendices. |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Planning notes for the Q2 rehearsal (window, slot range, workload seed, action owners). |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Operational checklist for staging → execution → rollback; update when lane topology or exporter guidance changes. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenced by B4 retro; archive digests alongside the tracker whenever the pack changes. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Proves `nexus.enabled = true` for multi-lane configs, preserves the Sora catalog hashes, and provisions lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) via `ConfigLaneRouter` before publishing artefact digests. |
