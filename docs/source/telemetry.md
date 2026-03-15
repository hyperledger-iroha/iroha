# Telemetry & Metrics Overview

日本語の概要は [`telemetry.ja.md`](./telemetry.ja.md) を参照してください。

Iroha exports Prometheus-format metrics and a JSON status summary. This page lists key metrics and example PromQL queries you can use to build dashboards.

Endpoints
- `/metrics`: Prometheus exposition text. Hidden when telemetry is disabled or the profile does not allow expensive metrics.
- `/status`: JSON status (hidden when telemetry is disabled). Includes top-level gauges (peers, blocks, queue active count), a `crypto { sm_helpers_available, sm_openssl_preview_enabled, halo2: { enabled, curve, backend, max_k, verifier_budget_ms, verifier_max_batch } }` snapshot, the `sumeragi { leader_index, highest_qc_height, locked_qc_height, locked_qc_view, gossip_fallback_total, view_change_proof_accepted_total, view_change_proof_stale_total, view_change_proof_rejected_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, pacemaker_backpressure_deferrals_total, tx_queue_depth, tx_queue_capacity, tx_queue_saturated, epoch_length_blocks, epoch_commit_deadline_offset, epoch_reveal_deadline_offset, prf_epoch_seed (hex), prf_height, prf_view }` view (highest/locked QC heights), a `governance` snapshot, and (when available) `sorafs_micropayments` — the most recent SoraFS micropayment sample per provider including credit counters and ticket totals.
- `/v1/sumeragi/new_view` (JSON): latest NEW_VIEW receipt counts per `(height, view)` (bounded in-memory window; oldest entries evicted).
- `/v1/sumeragi/new_view/sse` (SSE): periodic stream of the same JSON payload for live dashboards.
- `/v1/sumeragi/status` (Norito by default): consensus status snapshot. Set `Accept: application/json` to receive `{ leader_index, view_change_index, highest_qc { height, view, subject_block_hash }, locked_qc { height, view, subject_block_hash }, commit_qc { height, view, epoch, block_hash, validator_set_hash, validator_set_len, signatures_total }, commit_quorum { height, view, block_hash, signatures_present, signatures_counted, signatures_set_b, signatures_required, last_updated_ms }, tx_queue { depth, capacity, saturated }, epoch { length_blocks, commit_deadline_offset, reveal_deadline_offset }, gossip_fallback_total, block_created_dropped_by_lock_total, block_created_hint_mismatch_total, block_created_proposal_mismatch_total, consensus_message_handling { entries: [{ kind, outcome, reason, total }] }, pacemaker_backpressure_deferrals_total, da_reschedule_total, rbc_store { sessions, bytes, pressure_level, backpressure_deferrals_total, persist_drops_total, evictions_total, recent_evictions[...] }, lane_activity: [{ lane_id, tx_vertices, tx_edges, overlay_count, overlay_instr_total, overlay_bytes_total, rbc_chunks, rbc_bytes_total }], dataspace_activity: [{ lane_id, dataspace_id, tx_served }], rbc_lane_backlog: [{ lane_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }], rbc_dataspace_backlog: [{ lane_id, dataspace_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }], lane_commitments: [{ block_height, lane_id, tx_count, total_chunks, rbc_bytes_total, teu_total, block_hash }], dataspace_commitments: [{ block_height, lane_id, dataspace_id, tx_count, total_chunks, rbc_bytes_total, teu_total, block_hash }], lane_governance: [{ lane_id, alias, dataspace_id, visibility, storage_profile, governance, manifest_required, manifest_ready, manifest_path, validator_ids, quorum, protected_namespaces, runtime_upgrade { allow, require_metadata, metadata_key, allowed_ids } }], lane_governance_sealed_total, lane_governance_sealed_aliases, prf { height, view, epoch_seed }, vrf_penalty_epoch, vrf_committed_no_reveal_total, vrf_no_participation_total, vrf_late_reveals_total, collectors_targeted_{current,last_per_block}, redundant_sends_total, worker_loop { stage, stage_started_ms, last_iteration_ms, queue_depths { vote_rx, block_payload_rx, rbc_chunk_rx, block_rx, consensus_rx, lane_relay_rx, background_rx }, queue_diagnostics { blocked_total { vote_rx, block_payload_rx, rbc_chunk_rx, block_rx, consensus_rx, lane_relay_rx, background_rx }, blocked_ms_total { ... }, blocked_max_ms { ... }, dropped_total { ... } } }, commit_inflight { active, id, height, view, block_hash, started_ms, elapsed_ms, timeout_ms, timeout_total, last_timeout_timestamp_ms, last_timeout_elapsed_ms, last_timeout_height, last_timeout_view, last_timeout_block_hash, pause_total, resume_total, paused_since_ms, pause_queue_depths { ... }, resume_queue_depths { ... } }, settlement { dvp { success_total, failure_total, final_state_totals { none|delivery_only|payment_only|both }, failure_reasons, last_event { observed_at_ms, settlement_id, plan { order, atomicity }, outcome, failure_reason, final_state, legs { delivery_committed, payment_committed } } }, pvp { success_total, failure_total, final_state_totals { none|primary_only|counter_only|both }, failure_reasons, last_event { observed_at_ms, settlement_id, plan { order, atomicity }, outcome, failure_reason, final_state, legs { primary_committed, counter_committed }, fx_window_ms } } } }` (highest/locked QC snapshots in `highest_qc`/`locked_qc`).
- `/v1/sumeragi/status/sse` (SSE): periodic stream (≈1s) emitting the same JSON payload as `/v1/sumeragi/status` for dashboards.
- When `nexus.enabled = false` (Iroha 2 mode), lane/dataspace sections in `/status` and `/v1/sumeragi/status` are emptied and Prometheus output omits lane/dataspace labels so single-lane deployments stay lane-free.
- `/v1/sumeragi/rbc` (JSON): RBC session/throughput metrics: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- `/v1/sumeragi/rbc/sessions` (JSON): RBC session snapshot: `{ sessions_active, items: [{ block_hash, height, view, total_chunks, received_chunks, ready_count, delivered, invalid, payload_hash, recovered, lane_backlog: [{ lane_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }], dataspace_backlog: [{ lane_id, dataspace_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }] }] }`.
- `/v1/sumeragi/pacemaker` (JSON): pacemaker timers and config: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- `/v1/sumeragi/qc` (Norito by default): highest/locked QC snapshot; includes `subject_block_hash` for the highest QC when known. Set `Accept: application/json` to receive the JSON view.
- `/v1/sumeragi/commit_qc/{hash}` (Norito by default): full commit QC record for a block hash (if present). Set `Accept: application/json` to receive `{ subject_block_hash, commit_qc }` with `parent_state_root`, `post_state_root`, and aggregate signature data when available.
- `/v1/sumeragi/leader` (JSON): leader index snapshot; includes PRF context `{ height, view, epoch_seed }` in NPoS mode when available.
- `/v1/sumeragi/phases` (JSON): compact per-phase latencies (ms) for operator dashboards; returns the latest observed durations for consensus phases.
- `/v1/soranet/privacy/{event,share}` (Norito): privacy telemetry ingest for relay/collector signals. Requires `torii.soranet_privacy_ingest.enabled = true`, a token header (`X-SoraNet-Privacy-Token` or `X-API-Token`) when `require_token` is set, and a CIDR allow-list entry (empty list denies). Rate limits come from the same config (`rate_per_sec`/`burst`), and rejects surface `401/403/429` plus `soranet_privacy_ingest_reject_total{endpoint,reason}` counters for alerting.
- `/v1/sumeragi/collectors` (JSON): deterministic collector plan snapshot derived from the committed topology and on-chain parameters; exposes `mode`, plan `(height, view)` (where `height` mirrors the current chain height), `collectors_k`, `redundant_send_r`, `proxy_tail_index`, `min_votes_for_commit`, the ordered collector list, and `epoch_seed` (hex) when NPoS is active.
- `/v1/sumeragi/params` (JSON): snapshot of the on-chain Sumeragi parameters `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
- `/v1/sumeragi/new_view/json` (JSON): NEW_VIEW receipt snapshot `{ ts_ms, items: [{height, view, count}] }` (bounded in-memory window; oldest entries evicted).
  - Updated: also returns `locked_qc { height, view }`.

Aggregate governance-seal counters (`lane_governance_sealed_total`,
`lane_governance_sealed_aliases`) ride alongside the lane records. They provide a
quick “are any lanes still sealed?” view in both `/v1/sumeragi/status` and
`iroha_cli --output-format text ops sumeragi status`; the CLI prints the alias list inline so
operators can reconcile outstanding manifests without diffing the full payload.
Use `iroha_cli app nexus lane-report --only-missing --fail-on-sealed` during rollouts
or CI to surface the same data with a non-zero exit when seals remain.

SM helper telemetry (Prometheus metrics)
- `iroha_sm_syscall_total{kind="hash|verify|seal|open",mode}` — cumulative SM helper syscall successes grouped by helper kind and mode (`gcm`/`ccm` for SM4 helpers).
- `iroha_sm_syscall_failures_total{kind,mode,reason}` — cumulative failure counts with reason labels (`permission_denied`, `norito_invalid`, `decode_error`, etc.).

Settlement telemetry
- `iroha_settlement_events_total{kind="dvp|pvp",outcome="success|failure",reason}` — settlement lifecycle counters labelled by instruction kind and failure reason (`insufficient_funds`, `counterparty_mismatch`, `unsupported_policy`, `zero_quantity`, `missing_entity`, `math_error`, `other`; success uses `reason="-"`).
- `iroha_settlement_finality_events_total{kind="dvp|pvp",outcome="success|failure",final_state="none|delivery_only|payment_only|both|primary_only|counter_only"}` — finality counters grouped by settlement kind, execution outcome, and which legs remained committed. DvP reports `delivery_only|payment_only`, PvP reports `primary_only|counter_only`; `none` means both legs rolled back.
- `iroha_settlement_fx_window_ms{kind="pvp",order,atomicity}` — histogram of observed PvP FX windows (milliseconds between committed legs) labelled by execution order (`delivery_then_payment`/`payment_then_delivery`) and atomicity policy (`all_or_nothing|commit_first_leg|commit_second_leg`).

Subscription telemetry
- `iroha_subscription_billing_attempts_total{pricing="fixed|usage"}` — billing trigger invocations grouped by pricing kind.
- `iroha_subscription_billing_outcomes_total{pricing="fixed|usage",result="paid|failed|suspended|skipped"}` — billing outcomes grouped by pricing kind and result label.

Network time telemetry
- `nts_offset_ms` (gauge) — smoothed or raw offset vs local clock.
- `nts_confidence_ms` (gauge) — MAD confidence bound.
- `nts_peers_sampled` (gauge) — peers contributing recent samples.
- `nts_samples_used` (gauge) — samples used after RTT filtering.
- `nts_fallback` (gauge) — 1 when NTS falls back to local time.
- `nts_healthy` (gauge) — 1 when health thresholds pass and no fallback.
- `nts_min_samples_ok` / `nts_offset_ok` / `nts_confidence_ok` (gauges) — per-check health flags.
- `nts_rtt_ms_bucket{le="..."}` / `nts_rtt_ms_sum` / `nts_rtt_ms_count` — RTT histogram buckets (ms) and aggregates.
- `torii_nts_unhealthy_reject_total` (counter) — time-sensitive transactions rejected during admission because NTS is unhealthy.

Runbook guidance
- Alert when `max_over_time(nts_healthy[5m]) == 0` or `max_over_time(nts_fallback[5m]) > 0`; these indicate the time service is unsynchronized or missing samples.
- Use `nts_min_samples_ok`, `nts_offset_ok`, and `nts_confidence_ok` to pinpoint root cause; check `/v1/time/status` for peer sample and RTT diagnostics.
- If `enforcement_mode = "reject"`, admission blocks time-sensitive instructions while unhealthy. Switch to `warn` only for temporary operational relief.

Configuration
- `telemetry_enabled` (default: true): Master kill switch. When set to false, the daemon skips telemetry worker startup, Torii hides `/metrics` and `/status`, and runtime instrumentation is bypassed regardless of profile.
- `telemetry_profile` (default: `operator`): Capability bundle wiring both Torii routing and runtime sinks. Profiles toggle three capability flags — `metrics`, `expensive_metrics`, and `developer_outputs`. When `telemetry_enabled = false`, the effective profile is forced to `disabled`.
- `torii.peer_telemetry_urls` (default: empty): Optional list of Torii base URLs used to fetch peer telemetry metadata. When unset, peer telemetry discovery is disabled to avoid probing P2P ports.
- `torii.peer_geo.enabled` (default: false): Enable peer geo lookups for Torii telemetry (opt-in; requires network access to the configured endpoint).
- `torii.peer_geo.endpoint` (default: unset): Optional ip-api compatible endpoint used for peer geo lookups; when unset and `torii.peer_geo.enabled = true`, Torii uses the built-in ip-api default.
- Build-time ISI instrumentation: `#[metrics]` counters (`isi{kind="total|success"}`) and timing histograms (`isi_times`) require building `irohad` with `--features expensive-telemetry` (or `iroha_core` `expensive-telemetry`). The runtime still respects `telemetry_enabled` and `telemetry_profile` for exposure.

Telemetry redaction and integrity
- Redaction is mandatory for `operator`, `extended`, and `full` profiles; startup rejects configs where `telemetry_redaction.mode` is not `strict` or the build lacks the `log-obfuscation` feature.
- Field-name normalization is case-insensitive and splits punctuation/camelCase/acronyms into snake_case segments for taxonomy and allow-list checks.
- Sensitive taxonomy is defined by explicit prefixes and keywords (kept in sync with guardrails below).
- Redacted values are replaced with `[REDACTED]`; string payloads longer than 2048 bytes are truncated and suffixed with `...(truncated)` for deterministic export.
- `telemetry_redaction.mode` options: `strict` (always redact), `allowlist` (allow-listed entries bypass keyword redaction but explicit prefixes still redact), `disabled` (developer-only). `telemetry_redaction.allowlist` must be a subset of the approved policy below.

Telemetry redaction prefixes:
<!-- TELEMETRY_REDACTION_PREFIXES_START -->
```text
redact
sensitive
secret
pii
```
<!-- TELEMETRY_REDACTION_PREFIXES_END -->

Telemetry redaction keywords:
<!-- TELEMETRY_REDACTION_KEYWORDS_START -->
```text
password
passwd
passphrase
secret
credential
token
access_token
refresh_token
session_token
session
authorization
cookie
jwt
bearer
api_key
api_key_hash
apikey
private_key
privkey
mnemonic
seed
```
<!-- TELEMETRY_REDACTION_KEYWORDS_END -->

Approved telemetry redaction allowlist (normalized field names):
<!-- TELEMETRY_REDACTION_ALLOWLIST_START -->
```text
(none)
```
<!-- TELEMETRY_REDACTION_ALLOWLIST_END -->

- Redaction audit metrics: `telemetry_redaction_total{reason="keyword|explicit"}`, `telemetry_redaction_skipped_total{reason="allowlist|disabled|unsupported"}`, and `telemetry_truncation_total`.
- Tamper-evident exports: when `telemetry_integrity.enabled = true`, websocket telemetry and dev-telemetry JSON lines include a `chain` object (`seq`, `prev_hash`, `hash`, optional `signature`, `key_id`). `hash` is `blake3(prev_hash || seq || payload_json)` where `payload_json` is the Norito JSON serialization of the record. `signature` is a keyed Blake3 hash when `telemetry_integrity.signing_key_hex` (32-byte hex) is set. Without a persisted state file the chain restarts at sequence 1 on startup.
- To persist continuity across restarts, set `telemetry_integrity.state_dir` to a writable directory. Each sink writes its own state file (for example, `telemetry_integrity_ws.json` and `telemetry_integrity_dev.json`).

Build-time instrumentation
- `iroha_core/expensive-telemetry` (enables `iroha_telemetry/metric-instrumentation`) compiles the `#[metrics]` attribute into Prometheus counters and timing histograms.
- Without it, `#[metrics(+"...")]` still parses but timing histograms are a no-op; runtime `telemetry_profile` still controls exposure.

Profile capability matrix

| Profile    | `/status` | `/metrics` | Developer routes (`/v1/sumeragi/*`, SSE) | Intended use |
|------------|-----------|------------|------------------------------------------|--------------|
| `disabled` | no        | no         | no                                       | Telemetry fully off |
| `operator` | yes       | no         | no                                       | Production nodes that only need JSON status |
| `extended` | yes       | yes        | no                                       | Operators scraping Prometheus |
| `developer`| yes       | no         | yes                                      | Local debugging and dashboards |
| `full`     | yes       | yes        | yes                                      | Combine operator + developer tooling |

## Platform-Specific Telemetry Profiles

### Android SDK (AND7)

Roadmap item **NRPC/AND7** requires the Android SDK to follow the same telemetry
and privacy guarantees as the Rust node. Use the artefacts below whenever you
need to brief operators or confirm governance readiness:

| Artefact | Purpose |
|----------|---------|
| [`sdk/android/telemetry_redaction.md`](./sdk/android/telemetry_redaction.md) | Canonical policy covering hashed authorities, device buckets, retention, and override governance. |
| [`android_runbook.md`](./android_runbook.md#2-telemetry--redaction) | Step-by-step operational workflow (config threading, exporter checks, override handling). |
| [`sdk/android/readiness/signal_inventory_worksheet.md`](./sdk/android/readiness/signal_inventory_worksheet.md) | Owner matrix for every Android span/event/metric plus validation evidence. |
| [`sdk/android/telemetry_chaos_checklist.md`](./sdk/android/telemetry_chaos_checklist.md) | Quarterly rehearsal scenarios referenced by SRE governance. |
| [`sdk/android/readiness/and7_operator_enablement.md`](./sdk/android/readiness/and7_operator_enablement.md) | Curriculum outline and knowledge-check plan for support/on-call enablement. |

**Key guardrails**

- Android exports hashed authorities (`android.torii.http.request` /
  `android.torii.http.retry`) using the same Blake2b-256 salt published through
  `iroha_config.telemetry.redaction_salt`; watch
  `android.telemetry.redaction.salt_version` for drift during salt rotations.
- Device metadata is limited to coarse `android.telemetry.device_profile`
  buckets (SDK major version + `emulator|consumer|enterprise`). Alert when
  bucket ratios diverge by more than 10 % from the Rust node baseline.
- Network context drops carrier names entirely; only `network_type` and
  `roaming` are exported. Any request for subscriber data should be rejected and
  handled through the override workflow.
- Overrides are logged via `android.telemetry.redaction.override` and must
  match the manifest + audit procedure documented in the Android Support
  Playbook; on-call engineers should update
  `docs/source/sdk/android/telemetry_override_log.md` immediately after apply
  and revoke operations.

**Operational validation**

- `scripts/telemetry/check_redaction_status.py` produces the status bundle
  attached to chaos drills and incident timelines; run it against staging before
  filing SRE readiness evidence.
- Chaos rehearsals record Grafana snapshots plus the latest
  `android.telemetry.redaction.failure` and `android.telemetry.redaction.override`
  counters; link those artefacts in `docs/source/sdk/android/readiness/labs/`.
- PromQL snippets:
  - `increase(android.telemetry.redaction.failure_total[5m]) > 0` should page
    immediately outside chaos windows.
  - `sum by (device_profile)(android.telemetry.device_profile)` compared against
    the Rust `node.hardware_profile` histogram verifies bucket alignment.
  - `clamp_min(rate(android.telemetry.redaction.override_total[1h]), 0)` feeds
    the monthly override audit.

Refer to [`android_support_playbook.md`](./android_support_playbook.md#8-telemetry-redaction--observability-and7)
for the escalation tree that ties these metrics back to pager rotations.

## Governance telemetry

- `governance_proposals_status{status}` (gauge): current proposal counts grouped by
  status (`proposed`, `approved`, `rejected`, `enacted`). The `/status` JSON exposes
  the same data under `governance.proposals`, and the gauges are seeded from the
  recovered world state on startup so they reflect persisted proposals even
  before new transitions occur.
- `governance_protected_namespace_total{outcome}` (counter): admission enforcement
  for protected namespaces. Increments with `outcome="allowed"` when a deployment
  is backed by an enacted proposal and `outcome="rejected"` when the gate blocks it.
- `governance_manifest_admission_total{result}` (counter): queue admission outcomes
  driven by lane manifests. Each `result` label captures a distinct path:
  `allowed`, `missing_manifest`, `non_validator_authority`, `quorum_rejected`,
  `protected_namespace_rejected`, `runtime_hook_rejected`.
- `governance_manifest_quorum_total{outcome}` (counter): manifest quorum checks on
  the validator set. `outcome="satisfied"` records admits that met the quorum,
  while `outcome="rejected"` flags submissions missing the required validator approvals.
- `governance_manifest_hook_total{hook, outcome}` (counter): governance hook enforcement
  decisions. Currently `hook="runtime_upgrade"` is emitted with outcomes
  `allowed` / `rejected` whenever runtime upgrade submissions pass or fail manifest policy.
- `governance_manifest_activations_total{event}` (counter): manifest lifecycle
  events emitted by `EnactReferendum`. `event="manifest_inserted"` counts new
  manifests keyed by `code_hash`; `event="instance_bound"` counts namespace
  bindings (contract instance activations).
- `/status` now includes a `governance` object with proposal counts, protected
  namespace totals, aggregated manifest admission outcomes, and a
  `recent_manifest_activations` array listing the most
  recent enactments (namespace, contract id, code/ABI hash, block height, and
  activation timestamp in milliseconds).

Example `governance` excerpt from `/status`:

```json
"governance": {
  "proposals": {
    "proposed": 2,
    "approved": 1,
    "rejected": 0,
    "enacted": 1
  },
  "protected_namespace": {
    "total_checks": 5,
    "allowed": 4,
    "rejected": 1
  },
  "manifest_admission": {
    "allowed": 4,
    "missing_manifest": 1,
    "non_validator_authority": 0,
    "quorum_rejected": 1,
    "protected_namespace_rejected": 2,
    "runtime_hook_rejected": 0
  },
  "manifest_quorum": {
    "satisfied": 3,
    "rejected": 1
  },
"recent_manifest_activations": [
    {
      "namespace": "apps",
      "contract_id": "demo.contract",
      "code_hash_hex": "deadbeef",
      "abi_hash_hex": "cafebabe",
      "height": 42,
      "activated_at_ms": 1234567
    }
  ]
}
```

### Governance constraints validation runbook

The governance surface enforces that only enacted manifests may bind contract
instances inside protected namespaces. When rolling out runtime upgrades,
operators should validate that manifests were activated and that subsequent
deploys flowed through the protected gate.

**Key dashboards**
- Import `docs/source/grafana_governance_constraints.json` into Grafana (JSON
  dashboard). The template exposes the following panels:
  - `Proposal Status Counts` — shows the live values of
    `governance_proposals_status{status=…}` so proposal transitions can be
    reconciled with council decisions.
  - `Protected Namespace Enforcement (5m)` — plots
    `increase(governance_protected_namespace_total{outcome=…}[5m])` to detect
    deploy attempts that were allowed or rejected after the upgrade.
  - `Manifest Quorum Checks (5m)` — charts
    `increase(governance_manifest_quorum_total{outcome=…}[5m])` so missing
    quorum approvals surface quickly during incident response.
  - `Manifest Admission Outcomes (5m)` — visualises
    `increase(governance_manifest_admission_total{result=…}[5m])`, breaking down
    successful admits versus each rejection path (missing manifest, validator
    mismatch, quorum, namespace, runtime hook).
  - `Manifest Activations (5m)` — charts
    `increase(governance_manifest_activations_total{event=…}[5m])`, confirming
    that `EnactReferendum` inserted manifests and bound namespaces to the new
    `code_hash`.
  - `Rejected Deploys (24h)` — a daily stat over
    `increase(governance_protected_namespace_total{outcome="rejected"}[24h])`
    highlighting unexpected admission failures.

**Alert thresholds**
- Protected namespaces: alert when
  `increase(governance_protected_namespace_total{outcome="rejected"}[15m]) > 0`
  outside of a planned rollback window.
- Manifest gate regressions: alert when
  `increase(governance_manifest_admission_total{result!="allowed"}[5m]) > 0`
  to catch sustained rejections (missing manifests, quorum failures, namespace
  or runtime-hook policy violations).

### Lane admission latency remediation

- **Metric to watch:** `torii_lane_admission_latency_seconds{lane_id,endpoint}`
  (histogram; 0.75 s P95 budget across transaction admission paths).
- **Alert snippet:** see `dashboards/alerts/soranet_lane_rules.yml` — the
  `SoranetLaneAdmissionLatencyDegraded` rule fires when the rolling 5 minute P95
  exceeds 750 ms.

When the alert triggers:
1. Inspect `histogram_quantile(0.5/0.95/0.99, sum by (lane_id,endpoint,le)(rate(torii_lane_admission_latency_seconds_bucket[5m])))`
   to confirm whether the regression is confined to a single endpoint (e.g.
   `/transaction` vs `/v1/contracts/instance/activate`).
2. Pull `/v1/sumeragi/status` and review `lane_activity` for the affected lane.
   A spike in `tx_vertices`, `overlay_bytes_total`, or `rbc_bytes_total` hints at
   admission pressure rather than infrastructure issues.
3. Check `rbc_lane_backlog` and `rbc_dataspace_backlog` in the same status
   payload. If backlog is accumulating, verify gossip health and DA fetch
   telemetry (`dashboards/grafana/soranet_pq_ratchet.json` covers PQ circuit
   status) before blaming the admission tier.
4. Confirm Torii pacemaker windows by charting
   `sumeragi_pacemaker_backoff_ms` and `sumeragi_pacemaker_rtt_floor_ms`. A
   sudden increase usually indicates view-change churn that will also manifest
   as higher admission latency.
5. If latency remains elevated after clearing backlog, throttle the offending
   lane by raising `iroha_config.torii.transaction_lane.max_inflight` or
   redirecting traffic to a healthy lane using the orchestrator/CLI routing
   helpers. Document changes in the operations notebook and revert once metrics
   stabilise.

Always capture a snapshot of the Grafana panel and `/status` payload before and
after remediation; include them in the incident timeline for audit parity.

### Sumeragi consensus overview dashboard

Operators monitoring proposal health should import
`docs/source/grafana_sumeragi_overview.json` into Grafana. The dashboard tracks:

- `Highest vs Locked QC Height` — gauges `sumeragi_highest_qc_height` and
  `sumeragi_locked_qc_height` so you can spot stalled view changes or peers
  lagging behind the canonical highest/locked certificates.
- `Proposal Drop Rates (5m)` — visualises
  `increase(sumeragi_gossip_fallback_total[5m])` alongside the BlockCreated drop
  counters (`block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`,
  `block_created_proposal_mismatch_total`) to highlight misbehaving leaders and
  collectors.
- `Proposal Drop Totals` — a stat panel over the cumulative counters for quick
  summarisation in NOC dashboards.

Pair the panel with the alert snippets above (hint/proposal mismatch bursts) to
trigger remediation workflows when drops exceed acceptable limits.
- Missing activations: alert when
  `increase(governance_manifest_activations_total{event="instance_bound"}[30m]) == 0`
  during an upgrade rollout window; a namespace binding never landed on-chain.
- Proposal drift: alert when `governance_proposals_status{status="proposed"}`
  remains non-zero for longer than the agreed SLA (e.g., 24 h) while
  `status="approved"` stays flat — council approvals are stuck in enactment.

**Triage checklist**
1. Run `iroha_cli app gov deploy audit --namespace <ns>` (optionally filter by
   `--hash-prefix`) to compare stored manifests against the `code_hash` and
   `abi_hash` recorded in governance proposals. The command flags mismatches or
   missing manifests.
2. Fetch `/status` and inspect `governance.recent_manifest_activations` to
   confirm the latest activation contains the expected `code_hash_hex` and
   `abi_hash_hex` at the upgrade height.
3. Examine `increase(governance_protected_namespace_total{outcome="rejected"}[5m])`.
   If it spiked, grab Torii logs for the failing deploy and ensure the proposal
   was enacted; re-run `iroha_cli app gov protected get` to confirm the namespace
   list.
4. Verify that `governance_proposals_status{status="approved"}` decreased while
   `status="enacted"` increased after the rollout. If counts drift, queue an
   `EnactReferendum` check and confirm the `iroha_cli app gov enact` automation ran.
5. Inspect `increase(governance_manifest_hook_total{hook="runtime_upgrade", outcome="rejected"}[5m])`
   to spot runtime upgrade submissions blocked by manifest policy. Correlate
   spikes with Torii admission logs and confirm the manifest allowlist /
   metadata requirements match the proposal that triggered the hook.

### Torii ZK attachments & prover runbook

Operators monitoring attachment throughput and the background prover should use
`docs/source/zk/prover_runbook.md` as their primary guide. It captures log
sources, alert thresholds, and mitigation steps for queue backlogs and budget
exhaustion.

**Dashboards**
- Import `docs/source/grafana_zk_prover.json` to get queue depth, latency, and
  budget panels. Update the Prometheus data source UID after import if your
  installation uses a custom name.
- Overlay `histogram_quantile(0.95, sum(rate(zk_verify_latency_ms_bucket[5m])) by (le, backend))`
  with `histogram_quantile(0.95, sum(rate(torii_zk_prover_latency_ms_bucket[5m])) by (le))`
  to spot systemic latency spikes.

**Alert hints**
- Backlog: `avg_over_time(torii_zk_prover_pending[10m]) > 0` (page).
- Budget hits: `increase(torii_zk_prover_budget_exhausted_total{reason="bytes"}[30m]) > 0`
  (ticket) and escalate if the rate persists across multiple windows.
- Latency: `histogram_quantile(0.95, sum(rate(torii_zk_prover_latency_ms_bucket[5m])) by (le))`
  exceeding `torii.zk_prover_max_scan_millis` for >15 minutes.
- Sanitizer rejects: `increase(torii_attachment_reject_total[10m]) > 0` to catch
  unsupported types, expansion limits, or malformed payloads; correlate with
  `histogram_quantile(0.95, sum(rate(torii_attachment_sanitize_ms_bucket[5m])) by (le))`.

**Triage outline**
1. Inspect `torii_zk_prover_pending`, `torii_zk_prover_inflight`, and
   `torii_zk_prover_last_scan_ms` to understand queue pressure.
2. Review Torii logs (target `torii::zk_prover`) for scan summaries and budget
   hits. Enable debug logging temporarily when attachment ids are required.
3. Confirm configuration values in `iroha_config` (`[torii] zk_prover_*`).
4. Prune or retry problematic attachments with `iroha_cli app zk attachments delete`
   when backlog cleanup is required.
5. Document any threshold changes in the ops notebook and update Grafana panel
   annotations.

Torii integration

- `Torii::new_with_handle` accepts a `routing::MaybeTelemetry` gate that pairs the runtime `Telemetry` handle with the active `TelemetryProfile`. Use `routing::MaybeTelemetry::from_profile(runtime_handle, profile)` to construct the gate, or `routing::MaybeTelemetry::disabled()` when telemetry is unavailable.
- `Torii::new` (when the `telemetry` feature is enabled) remains as a convenience wrapper; it now forwards to `new_with_handle` with an operator profile by default. Tests can use `routing::MaybeTelemetry::for_tests()` to obtain an in-process telemetry handle.

- `torii_address_invalid_total{endpoint,reason}` increments whenever HTTP routes reject an account identifier (invalid I105 payloads, domain mismatches, etc.). Keep the `<0.1%` SLO by watching the dedicated Grafana board in `dashboards/grafana/address_ingest.json`.
- `torii_address_collision_total{endpoint,kind="local12_digest"}` and `torii_address_collision_domain_total{endpoint,domain}` record Local‑12 selector collisions. Both feed the collision panel/alert in `dashboards/grafana/address_ingest.json` so operators can tie spikes to specific domains. Production should stay flat; any increment blocks manifest promotions until governance signs off on the fix.

Pipeline metrics
- pipeline_stage_ms: Histogram of per-stage durations with label `stage` in {"access","overlays","dag","schedule","apply","layers_prep","layers_exec","layers_merge"}.
- pipeline_dag_vertices, pipeline_dag_edges, pipeline_conflict_rate_bps: Latest validated block DAG shape and conflict rate in basis points.
- pipeline_access_set_source_total{source=manifest_hints|entrypoint_hints|prepass_merge|conservative_fallback}: Cumulative access-set derivation counts by source.
- pipeline_overlay_count, pipeline_overlay_instructions, pipeline_overlay_bytes: Overlay stats for latest block.
- pipeline_peak_layer_width, pipeline_layer_avg_width, pipeline_layer_median_width: Layer width summary.
- pipeline_layer_count: Number of scheduler layers for the latest block.
- pipeline_scheduler_utilization_pct: Average parallelism utilization (0..100) computed as `avg_width / peak_width * 100`.
- pipeline_detached_prepared, pipeline_detached_merged, pipeline_detached_fallback: Detached execution counters per latest block.

IVM cache metrics
- ivm_cache_hits, ivm_cache_misses, ivm_cache_evictions: Global pre-decode cache counters (cumulative).
- ivm_cache_decoded_streams, ivm_cache_decoded_ops_total: Cumulative decode workload counters (number of streams and total ops) reported by the IVM pre-decode cache.
- ivm_cache_decode_failures, ivm_cache_decode_time_ns_total: Cumulative decode failure count and wall-clock nanoseconds spent decoding.

IVM register pressure metrics (new)
- ivm_register_max_index (histogram): highest GPR index touched during a VM execution. Buckets cover 16 → 512 so you can alert when contracts spill beyond the hot register bank.
- ivm_register_unique_count (histogram): number of distinct GPRs accessed per execution. Useful to spot contracts with high register churn before Merkle tagging becomes expensive.
- Example PromQL:
  - `histogram_quantile(0.9, sum(rate(ivm_register_max_index_bucket[5m])) by (le))` — p90 of the highest register index over the last 5 minutes.
  - `histogram_quantile(0.95, sum(rate(ivm_register_unique_count_bucket[5m])) by (le))` — p95 of unique registers touched.
- Alert hint: page when `histogram_quantile(0.95, …)` holds above ~256 for >10m; this signals workloads exceeding the intended fast-register tier and justifies enabling the tiered register RFC.

Block/consensus metrics
- commit_time_ms (histogram), last_commit_time_ms (gauge), block_height, block_height_non_empty, txs{type in [accepted,rejected,total]}.
- Queue gauges: `queue_size` (active queue size, queued + in-flight), `queue_queued` (waiting in the hash queue), `queue_inflight` (selected but not yet committed).

P2P metrics (selected)
- connected_peers, `p2p_peer_churn_total{event="connected|disconnected"}`, p2p_* gauges/counters for queue depth/drops, throttling, DNS, handshake latencies (`p2p_handshake_ms_*`).
- `consensus_ingress_drop_total{topic,reason}` counts consensus ingress drops for payload topics (`topic` in `ConsensusPayload|ConsensusChunk|BlockSync`, `reason` in `rate|bytes|rbc_session_limit|penalty`).

Sumeragi metrics
- Counters: `sumeragi_tail_votes_total`, `sumeragi_widen_before_rotate_total`, `sumeragi_view_change_suggest_total`, `sumeragi_view_change_install_total`; histogram: `sumeragi_cert_size` (signatures per committed block).
- Commit quorum/certificate: `sumeragi_commit_signatures_present`, `sumeragi_commit_signatures_counted`, `sumeragi_commit_signatures_set_b`, `sumeragi_commit_signatures_required` track the last commit tally; `sumeragi_commit_certificate_height`, `sumeragi_commit_certificate_view`, `sumeragi_commit_certificate_epoch`, `sumeragi_commit_certificate_signatures_total`, `sumeragi_commit_certificate_validator_set_len` summarize the latest commit certificate.
- Queue health: `sumeragi_tx_queue_depth`/`sumeragi_tx_queue_capacity` gauge the live mempool size and effective ceiling; `sumeragi_tx_queue_saturated` flips to `1` when Torii reports saturation, signalling that redundant collector fan-out is temporarily suppressed.
- Pending blocks: `sumeragi_pending_blocks_total` counts pending blocks tracked by the local node; `sumeragi_pending_blocks_blocking` isolates those that gate proposal/view-change progress; `sumeragi_commit_inflight_queue_depth` shows whether the commit pipeline is busy (0/1).
- Proposal gaps: `sumeragi_proposal_gap_total` counts view-change rotations triggered because no proposal was observed before the cutoff.
- VRF emission: `sumeragi_vrf_commits_emitted_total`, `sumeragi_vrf_reveals_emitted_total`, and `sumeragi_vrf_reveals_late_total` count how many commit/reveal messages this validator broadcast (including late reveals accepted after the window). Pair with `sumeragi_vrf_non_reveal_*` counters to monitor participation health at epoch boundaries.
- Collector fan-out: `sumeragi_redundant_sends_total` (aggregate), `sumeragi_redundant_sends_by_peer{peer="…"}`, and `sumeragi_redundant_sends_by_collector{idx="…"}` highlight redundant collector sends; investigate sustained spikes to locate congested collectors or unhealthy peers.
- Collector targeting: `sumeragi_collectors_targeted_current` (gauge) tracks the in-flight collector count for the current block; `sumeragi_collectors_targeted_per_block` histogram (`*_bucket`) records how many collectors were targeted per committed block.
- DA availability warnings: `sumeragi_rbc_da_reschedule_total` (and `/v1/sumeragi/status → da_reschedule_total`) is legacy and no longer increments; use `sumeragi_da_gate_block_total{reason="missing_local_data"}` for missing local payloads.
- Channel pressure: `sumeragi_dropped_block_messages_total` and `sumeragi_dropped_control_messages_total` partition channel drops; `dropped_messages` remains the aggregate counter for existing dashboards.

Sumeragi additions (new series)
- `sumeragi_highest_qc_height` (gauge) — current adopted highest QC height.
- `sumeragi_new_view_publish_total` (counter) — NEW_VIEW messages published by this node.
- `sumeragi_new_view_recv_total` (counter) — NEW_VIEW messages received and accepted by this node.
  - See also: `sumeragi_new_view_receipts_by_hv{height="<h>",view="<v>"}` for per-(height,view) receipt counts.
- `sumeragi_post_to_peer_total{peer}` (counter) — post attempts to peers (collector routing and backpressure insight).
- `sumeragi_bg_post_enqueued_total{kind}` (counter) — background-post tasks enqueued by kind in {Post,Broadcast}.
- `sumeragi_bg_post_overflow_total{kind}` (counter) — background-post queue full events; sender blocks until space is available.
- `sumeragi_bg_post_drop_total{kind}` (counter) — background-post drops when the queue is missing or disconnected.
- `sumeragi_bg_post_queue_depth` (gauge) — global background-post queue depth.
- `sumeragi_bg_post_queue_depth_by_peer{peer}` (gauge) — per-collector background-post queue depth.

Sumeragi pacemaker
- Config gauges:
  - `sumeragi_pacemaker_backoff_multiplier` — backoff multiplier applied to each view-change increment.
  - `sumeragi_pacemaker_rtt_floor_multiplier` — multiplier for RTT-based floor.
  - `sumeragi_pacemaker_max_backoff_ms` — maximum backoff cap applied to the pacemaker window.
  - `sumeragi_pacemaker_jitter_frac_permille` — jitter band as permille of the window (0..=1000).
- Runtime gauges:
  - `sumeragi_pacemaker_backoff_ms` — current backoff window (ms) that gates the next view-change suggestion.
  - `sumeragi_pacemaker_rtt_floor_ms` — current RTT-based floor (ms) considered when computing the backoff window; 0 when no RTT samples.
  - `sumeragi_pacemaker_jitter_ms` — jitter magnitude applied (ms; absolute value).

PromQL examples
- Pacemaker backoff trend (avg over 5m):
  - avg_over_time(sumeragi_pacemaker_backoff_ms[5m])
- RTT floor trend (avg over 5m):
  - avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])
- Verify config:
  - max(sumeragi_pacemaker_backoff_multiplier)
  - max(sumeragi_pacemaker_rtt_floor_multiplier)
  - max(sumeragi_pacemaker_max_backoff_ms)
  - max(sumeragi_pacemaker_jitter_frac_permille)

NEW_VIEW receipts
- GaugeVec:
  - `sumeragi_new_view_receipts_by_hv{height="<h>",view="<v>"}` — deduplicated NEW_VIEW sender count for (height, view).
- Counter:
- `sumeragi_new_view_dropped_by_lock_total` — NEW_VIEW frames rejected because the advertised highest certificate is behind the current locked certificate.
- Example queries:
  - Latest counts across recent heights: `sum by (height,view) (sumeragi_new_view_receipts_by_hv)`
  - Filter for current height h: `sumeragi_new_view_receipts_by_hv{height="<h>"}`
 - Operator endpoints:
   - JSON snapshot: `GET /v1/sumeragi/new_view` → `{ ts_ms, items: [{height,view,count}, ...] }`
   - SSE stream: `GET /v1/sumeragi/new_view/sse` (1s interval) emits the same structure per event.
   - Note: counts are kept in a bounded in-memory window; oldest `(height, view)` entries are evicted.

Example PromQL
- P50/P90 stage latency (ms):
  - histogram_quantile(0.5, sum(rate(pipeline_stage_ms_bucket[5m])) by (le,stage))
 - histogram_quantile(0.9, sum(rate(pipeline_stage_ms_bucket[5m])) by (le,stage))
- Commit time P95 (ms):
 - histogram_quantile(0.95, sum(rate(commit_time_ms_bucket[5m])) by (le))
- IVM cache hit rate (%):
  - 100 * (ivm_cache_hits - ivm_cache_hits offset 5m) / clamp_min((ivm_cache_hits - ivm_cache_hits offset 5m) + (ivm_cache_misses - ivm_cache_misses offset 5m), 1)
- Detached merge ratio:
 - pipeline_detached_merged / clamp_min(pipeline_detached_prepared, 1)
 - Sumeragi tail votes rate (s⁻¹):
   - rate(sumeragi_tail_votes_total[5m])
 - Widen-before-rotate rate (s⁻¹):
   - rate(sumeragi_widen_before_rotate_total[5m])
 - View-change suggests vs installs (s⁻¹):
   - rate(sumeragi_view_change_suggest_total[5m])
 - rate(sumeragi_view_change_install_total[5m])
- Certificate size P90 (signatures):
  - histogram_quantile(0.9, sum(rate(sumeragi_cert_size_bucket[5m])) by (le))
- Collector targeting distribution:
  - sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m]))
  - histogram_quantile(0.95, sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m])))
- Redundant send spikes / top offenders:
  - rate(sumeragi_redundant_sends_total[5m])
  - topk(5, sum by (peer) (rate(sumeragi_redundant_sends_by_peer[5m])))
  - sum by (idx) (rate(sumeragi_redundant_sends_by_collector[5m]))
- Channel drop alerts:
  - rate(sumeragi_dropped_block_messages_total[5m])
  - rate(sumeragi_dropped_control_messages_total[5m])
  - rate(dropped_messages[5m])

Sumeragi phases latencies (operator dashboards)
- Endpoint: `GET /v1/sumeragi/phases` (JSON)
- Shape: `{ propose_ms, collect_da_ms, collect_prevote_ms, collect_precommit_ms, collect_aggregator_ms, commit_ms, pipeline_total_ms, ema_ms }` where `ema_ms` mirrors the phase keys (`propose_ms`, …, `collect_aggregator_ms`, `commit_ms`, `pipeline_total_ms`).
- Purpose: quick, compact snapshot of the latest observed durations (milliseconds) for each consensus phase to power lightweight dashboards.
- `collect_aggregator_ms` tracks redundant collector fan-out latency (validator →
  secondary collectors). Pair it with `sumeragi_redundant_sends_*` counters when
  tuning K/r parameters or alert thresholds. Gossip fallback frequency surfaces
  via `sumeragi_gossip_fallback_total`, and proposal drops caused by the locked
  QC gate are exported as `block_created_dropped_by_lock_total`. Header rejections
  are split between `block_created_hint_mismatch_total` (height/view/parent
  mismatches) and `block_created_proposal_mismatch_total` (proposal header/payload
  mismatches that emit InvalidProposal evidence).
- `pipeline_total_ms` sums the pacemaker-controlled phases (propose, collect_da, collect_prevote, collect_precommit, commit) to provide a single end-to-end latency figure; `collect_aggregator_ms` remains a separate fan-out signal and is not included.

Example response
```json
{
  "propose_ms": 11,
  "collect_da_ms": 22,
  "collect_prevote_ms": 33,
  "collect_precommit_ms": 44,
  "collect_aggregator_ms": 50,
  "commit_ms": 77,
  "pipeline_total_ms": 187,
  "collect_aggregator_gossip_total": 3,
  "block_created_dropped_by_lock_total": 1,
  "block_created_hint_mismatch_total": 2,
  "block_created_proposal_mismatch_total": 4,
  "ema_ms": {
    "propose_ms": 15,
    "collect_da_ms": 26,
    "collect_prevote_ms": 37,
    "collect_precommit_ms": 48,
    "collect_aggregator_ms": 57,
    "commit_ms": 81,
    "pipeline_total_ms": 207
  }
}
```

Alert snippets
- Hint mismatch burst: `increase(block_created_hint_mismatch_total[5m]) > 0`
- Proposal mismatch burst: `increase(block_created_proposal_mismatch_total[5m]) > 0`
- Locked QC gate drop spike: `increase(block_created_dropped_by_lock_total[5m]) > 0`
- Pacemaker deferrals under sustained load: `increase(sumeragi_pacemaker_backpressure_deferrals_total[5m]) > 0`
- Pacemaker deferrals by reason: `increase(sumeragi_pacemaker_backpressure_deferrals_by_reason_total[5m]) > 0`

Sumeragi pacemaker (example)
- Endpoint: `GET /v1/sumeragi/pacemaker`
- Shape: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille, round_elapsed_ms, view_timeout_target_ms, view_timeout_remaining_ms }`

Example response
```json
{
  "backoff_ms": 500,
  "rtt_floor_ms": 120,
  "jitter_ms": 15,
  "backoff_multiplier": 2,
  "rtt_floor_multiplier": 2,
  "max_backoff_ms": 60000,
  "jitter_frac_permille": 50,
  "round_elapsed_ms": 340,
  "view_timeout_target_ms": 1000,
  "view_timeout_remaining_ms": 660
}
```

Sumeragi QC snapshot (example)
- Endpoint: `GET /v1/sumeragi/qc`
- Shape: `{ highest_qc: { height, view, subject_block_hash }, locked_qc: { height, view } }` (QC snapshot)

Example response
```json
{
  "highest_qc": {
    "height": 1234,
    "view": 7,
    "subject_block_hash": "9f1b0c7b59f1e2a3d4c5b6a79800112233445566778899aabbccddeeff001122"
  },
  "locked_qc": { "height": 1229, "view": 6 }
}
```

Prometheus exports matching gauges for these snapshots:
- `sumeragi_highest_qc_height`
- `sumeragi_locked_qc_height`
- `sumeragi_locked_qc_view`

## Fraud monitoring metrics

- `fraud_psp_assessments_total{tenant,band,lane,subnet}` — counter incremented whenever Torii admits a PSP assessment. Use it to monitor tenant activity and severity distribution.
- `fraud_psp_attestation_total{tenant,engine,lane,subnet,status}` — attestation verifier outcomes (`status="verified"` on success). Alerts should trigger on non-verified statuses to catch PSP regressions or key rotation issues.
- `fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}` — counter for transactions missing metadata. `cause="missing"` means the host rejected the transaction; `cause="grace"` denotes temporary bypass via `missing_assessment_grace_secs`.
- `fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}` — counter tracking malformed metadata fields (e.g., missing tenant, non-numeric latency).
- `fraud_psp_latency_ms{tenant,lane,subnet}` — histogram of PSP-reported scoring latency in milliseconds; buckets follow an exponential series (5 ms … ~1.3 s).
- `fraud_psp_score_bps{tenant,band,lane,subnet}` — histogram of risk scores (0–10 000 bps) recorded after admission.
- `fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}` — counter capturing outcome drift: `direction="missed_fraud"` when a confirmed fraud cleared with `band∈{low,medium}`, `direction="false_positive"` when a `band∈{high,critical}` decision resolved as non-fraud.

PromQL starters:

- Tenant heartbeat: `sum by (tenant) (rate(fraud_psp_assessments_total[5m]))`
- Latency P95 per tenant: `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant))`
- Grace-window watchdog: `sum(rate(fraud_psp_missing_assessment_total{cause="grace"}[10m]))`
- Mismatch ratio: `sum(rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) / clamp_min(sum(rate(fraud_psp_assessments_total[1h])), 1)`

Telemetry expects the following transaction metadata to be present when fraud monitoring is enabled: `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, and, once PSPs complete post-incident triage, `fraud_assessment_disposition` (values documented in `docs/source/fraud_monitoring_system.md`).

Sumeragi leader (example)
- Endpoint: `GET /v1/sumeragi/leader`
- Shape: `{ leader_index, prf: { height, view, epoch_seed } }`

Example response
```json
{
  "leader_index": 3,
  "prf": {
    "height": 1234,
    "view": 7,
    "epoch_seed": "c0ffee1234567890deadbeef00112233445566778899aabbccddeeff00112233"
  }
}
```

Sumeragi RBC (status example)
- Endpoint: `GET /v1/sumeragi/rbc`
- Shape: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`

Example response
```json
{
  "sessions_active": 2,
  "sessions_pruned_total": 10,
  "ready_broadcasts_total": 8,
  "ready_rebroadcasts_skipped_total": 3,
  "deliver_broadcasts_total": 7,
  "payload_bytes_delivered_total": 1234567,
  "payload_rebroadcasts_skipped_total": 5
}
```

Sumeragi RBC sessions (example)
- Endpoint: `GET /v1/sumeragi/rbc/sessions`
- Shape: `{ sessions_active, items: [{ block_hash, height, view, total_chunks, received_chunks, ready_count, delivered, invalid, payload_hash, recovered, lane_backlog: [{ lane_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }], dataspace_backlog: [{ lane_id, dataspace_id, tx_count, total_chunks, pending_chunks, rbc_bytes_total }] }] }`

Example response
```json
{
  "sessions_active": 1,
  "items": [
    {
      "block_hash": "7a6f2d3c4b5a9e8d7c6b5a4c3d2e1f0a11223344556677889900aabbccddeeff",
      "height": 1234,
      "view": 7,
      "total_chunks": 12,
      "received_chunks": 12,
      "ready_count": 5,
      "delivered": true,
      "invalid": false,
      "payload_hash": "f1e2d3c4b5a697887766554433221100ffeeddccbbaa00998877665544332211",
      "recovered": true,
      "lane_backlog": [
        {
          "lane_id": 0,
          "tx_count": 6,
          "total_chunks": 12,
          "pending_chunks": 0,
          "rbc_bytes_total": 786432
        }
      ],
      "dataspace_backlog": [
        {
          "lane_id": 0,
          "dataspace_id": 0,
          "tx_count": 6,
          "total_chunks": 12,
          "pending_chunks": 0,
          "rbc_bytes_total": 786432
        }
      ]
    }
  ]
}
```

Sumeragi telemetry snapshot
- Endpoint: `GET /v1/sumeragi/telemetry`
- Shape: `{ availability: { total_votes_ingested, collectors: [{ collector_idx, peer_id, votes_ingested }] }, qc_latency_ms: [{ kind, last_ms }], rbc_backlog: { pending_sessions, total_missing_chunks, max_missing_chunks } }`

Example response
```json
{
  "availability": {
    "total_votes_ingested": 12,
    "collectors": [
      {
        "collector_idx": 4,
        "peer_id": "ed0120...",
        "votes_ingested": 5
      }
    ]
  },
  "qc_latency_ms": [
    {
      "kind": "availability",
      "last_ms": 138
    }
  ],
  "rbc_backlog": {
    "pending_sessions": 1,
    "total_missing_chunks": 3,
    "max_missing_chunks": 2
  }
}
```

Layer widths and utilization
- Peak width per block: max_over_time(pipeline_peak_layer_width[5m])
- Average width trend: avg_over_time(pipeline_layer_avg_width[5m])
- Median width trend: avg_over_time(pipeline_layer_median_width[5m])
- Layer count trend: avg_over_time(pipeline_layer_count[5m])
- Scheduler utilization (P50): quantile_over_time(0.5, pipeline_scheduler_utilization_pct[10m])
- Width histogram (e.g., layers <= 8): sum by (le) (increase(pipeline_layer_width_hist_bucket{le="8"}[5m]))
- Overlay volume (instructions/bytes):
  - rate(pipeline_overlay_instructions[5m])
  - rate(pipeline_overlay_bytes[5m])

Configuration
- Telemetry master switch: when disabled in `iroha_config`, Torii hides `/metrics` and `/status`, no telemetry outputs are started, and all observations are skipped. Gauges/counters remain unchanged while disabled.
- Halo2 verifier gauges:
  - `iroha_zk_halo2_enabled`: 0/1 flag indicating whether Halo2 verification is active.
  - `iroha_zk_halo2_curve_id`: numeric identifier for the selected curve (`0=Pallas`, `1=Pasta`, `2=Goldilocks`, `3=Bn254`).
  - `iroha_zk_halo2_backend_id`: numeric identifier for the backend (`0=IPA`, `1=Unsupported`).
  - `iroha_zk_halo2_max_k`: maximum supported circuit exponent (`N = 2^k`).
  - `iroha_zk_halo2_verifier_budget_ms`: soft verifier time budget per proof (milliseconds).
  - `iroha_zk_halo2_verifier_max_batch`: maximum proofs accepted in a batch verification.

DA/RBC (Sumeragi) configuration
- `sumeragi.da.enabled` (bool): enables data availability tracking and Reliable Broadcast (RBC) payload distribution together. Availability evidence (`availability evidence` or an RBC `READY` quorum) is tracked (advisory; does not gate commit); missing local payloads are fetched via RBC or block sync, and RBC remains transport/recovery while its delivery latency is still tracked.
- `sumeragi.advanced.rbc.chunk_max_bytes` (usize): maximum bytes per RBC chunk when broadcasting payloads; must be > 0. Clamped at startup so serialized RBC chunks fit within the consensus payload plaintext cap derived from `network.max_frame_bytes_block_sync`.
- `sumeragi.advanced.rbc.session_ttl_ms` (u64): inactive RBC sessions are pruned after this TTL (milliseconds) to bound memory.
- `sumeragi.advanced.rbc.rebroadcast_sessions_per_tick` (usize): cap on RBC session rebroadcasts per tick to prevent payload storms when backlogs accumulate.

Metrics: RBC exports gauges/counters (`sumeragi_rbc_sessions_active`, `sumeragi_rbc_sessions_pruned_total`, `sumeragi_rbc_ready_broadcasts_total`, `sumeragi_rbc_deliver_broadcasts_total`, `sumeragi_rbc_payload_bytes_delivered_total`, `sumeragi_rbc_rebroadcast_skipped_total{kind="payload|ready"}`, `sumeragi_rbc_mismatch_total{peer,kind}`, `sumeragi_rbc_persist_drops_total`) and per-lane/dataspace backlog gauges (`sumeragi_rbc_lane_{tx_count,total_chunks,pending_chunks,bytes_total}{lane_id}` and `sumeragi_rbc_dataspace_{tx_count,total_chunks,pending_chunks,bytes_total}{lane_id,dataspace_id}`) alongside the Torii JSON endpoints shown above. The rebroadcast-skipped counters increment whenever the core skips payload/READY rebroadcasts.
Additional gauges track backlog pressure: `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`, and `sumeragi_rbc_backlog_sessions_pending`.

### Troubleshooting: RBC & pacemaker backpressure

1. **Capture live snapshots.** Start with `iroha_cli --output-format text ops sumeragi telemetry` (or `GET /v1/sumeragi/telemetry`) to inspect `rbc_backlog` and vote ingestion, then fetch `/v1/sumeragi/rbc` and `/v1/sumeragi/rbc/sessions` to list active payloads, chunk counts, and recovery flags.
2. **Inspect backlog counters.** Watch `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`, and `sumeragi_rbc_backlog_sessions_pending`. Sustained non-zero values over five minutes (e.g., `max_over_time(sumeragi_rbc_backlog_chunks_max[5m]) > 0`) imply slow chunk delivery; correlate with `ready_count` vs `delivered` in the session snapshot.
3. **Check DA availability warnings.** Alert on spikes in `sumeragi_da_gate_block_total{reason="missing_local_data"}`; `sumeragi_rbc_da_reschedule_total` is legacy and should remain zero in current pipelines.
4. **Evaluate pacemaker deferrals and proposal backpressure.** Use `increase(sumeragi_pacemaker_backpressure_deferrals_total[5m])`, `increase(sumeragi_pacemaker_backpressure_deferrals_by_reason_total{reason="..."}[5m])`, `max_over_time(sumeragi_pacemaker_backpressure_deferral_age_ms{reason="..."}[5m])`, `max_over_time(sumeragi_tx_queue_saturated[5m])`, `max_over_time(sumeragi_pending_blocks_blocking[5m])`, `max_over_time(sumeragi_commit_inflight_queue_depth[5m])`, `sumeragi_rbc_backlog_*`, and relay drop/backpressure counters to confirm whether the pacemaker halted due to queue saturation, relay/RBC backlog, or blocking pending blocks. Combine with `increase(gossip_fallback_total[5m])` and `increase(block_created_proposal_mismatch_total[5m])` to surface collectors retrying without progress.
5. **Review logs and network health.** Filter consensus logs for `rbc` and `pacemaker_backpressure_deferral` to spot repeated retries, DA restarts, or queue pressure. Cross-check P2P metrics (`p2p_queue_depth{priority=...}`, `p2p_dropped_posts`, `p2p_dropped_broadcasts`, `p2p_subscriber_queue_full_total`, `p2p_subscriber_queue_full_by_topic_total{topic=...}`, `p2p_subscriber_unrouted_total`, `p2p_subscriber_unrouted_by_topic_total{topic=...}`) and payload ingress drops (`consensus_ingress_drop_total{topic="ConsensusPayload|ConsensusChunk|BlockSync",reason="rate|bytes|rbc_session_limit|penalty"}`) to identify network bottlenecks; adjust collector fan-out, queue capacity, or baseline load accordingly.
6. **Correlate logs automatically.** Run `python3 scripts/sumeragi_backpressure_log_scraper.py <logfile>`
   to list each pacemaker deferral together with nearby missing-availability entries. Adjust
   `--window-before` / `--window-after` to match your alert window and add `--status path/to/status.json`
   when you have a `/v1/sumeragi/status` snapshot handy. The script prints a human-readable summary
   by default and supports `--json` for feeding structured reports into on-call automation.
7. **Escalate persistent issues.** If backlog/deferral metrics stay high beyond two blocks:
   - Freeze new client submissions via admission rate limiting.
   - Manually inspect problematic sessions with `iroha_cli --output-format json ops sumeragi telemetry` to confirm which height/view is stuck.
   - Consider increasing `sumeragi.advanced.rbc.chunk_max_bytes` or provisioning additional bandwidth before re-enabling full load.


Availability collectors expose vote ingestion counters: `sumeragi_da_votes_ingested_total`, `sumeragi_da_votes_ingested_by_collector{collector_idx="..."}`, and `sumeragi_da_votes_ingested_by_peer{peer="..."}`. Availability-evidence assembly latency is recorded via the histogram `sumeragi_qc_assembly_latency_ms{kind="availability"}` with the latest observed latency mirrored in `sumeragi_qc_last_latency_ms{kind="availability"}`.

Pacemaker configuration (Sumeragi)
- `sumeragi.advanced.pacemaker.backoff_multiplier` (u32): scales each timeout backoff step (default 1).
- `sumeragi.advanced.pacemaker.rtt_floor_multiplier` (u32): RTT floor multiplier; floor = avg_rtt * multiplier (default 2).
- `sumeragi.advanced.pacemaker.max_backoff_ms` (u64): backoff cap in milliseconds (default 60000).
- `sumeragi.advanced.pacemaker.jitter_frac_permille` (u32): jitter band in permille of the window (0..=1000, default 0 = off).

Notes
- All metrics have deterministic semantics across hardware. Parallel paths publish counters only after deterministic commit.
- Extend dashboards with Torii endpoint metrics (`torii_*`) once wired; see roadmap for status.
## Alerting — VRF Participation Drift

Prometheus rules surfacing randomness degradation:

```
alert: SumeragiVrfNoParticipation
expr: increase(sumeragi_vrf_no_participation_total[140m]) > 0
for: 1m
labels:
  severity: critical
annotations:
  summary: "Validator skipped VRF commit and reveal windows"
  description: |
    Non-participation penalties incremented (count={{ $value }}). Inspect `iroha_cli ops sumeragi vrf-epoch --epoch <current>`
    to identify the offline signer and stage reconfiguration or slashing if the validator cannot recover.

alert: SumeragiVrfNonReveal
expr: increase(sumeragi_vrf_non_reveal_penalties_total[140m]) > 0
labels:
  severity: warning
annotations:
  summary: "Validator missed VRF reveal deadline"
  description: |
    Non-reveal penalties incremented (count={{ $value }}). Use `sumeragi_vrf_non_reveal_by_signer` labels to page the validator and re-submit the reveal.

alert: SumeragiVrfPayloadReject
expr: increase(sumeragi_vrf_rejects_total_by_reason{reason!="late"}[5m]) > 0
labels:
  severity: warning
annotations:
  summary: "VRF payload rejected"
  description: |
    Instance {{ $labels.instance }} rejected a VRF payload for reason {{ $labels.reason }}. Confirm the validator is on the latest parameters and re-issue the payload.

alert: SumeragiVrfCommitStall
expr: rate(sumeragi_vrf_commits_emitted_total[5m]) == 0 and increase(sumeragi_blocks_committed_total[5m]) > 0
for: 10m
labels:
  severity: warning
annotations:
  summary: "Epoch commitments stalled while blocks continue"
  description: |
    No VRF commitments observed during an active epoch. Verify validators are online and check `/v1/sumeragi/status.vrf_penalty_epoch`.
```

Adjust the 140 minute window to match your deployment’s `sumeragi.npos.vrf.commit_deadline_offset_blocks + sumeragi.npos.vrf.reveal_deadline_offset_blocks` (defaults assume one-second blocks). Link alerts to the response checklist in `docs/source/sumeragi.md#vrf-alert-response-runbook`.
The sample rule group lives at `docs/source/references/prometheus.rules.sumeragi_vrf.yml`; include it from your Prometheus configuration (see `docs/source/references/prometheus.template.yml` for an example `rule_files` entry).
Run `scripts/check_prometheus_rules.sh` to validate the rules locally. The helper invokes `promtool check rules` if Prometheus is installed, or falls back to `docker run --rm prom/prometheus …` when Docker is available.
## Alerting — Consensus Membership Mismatch


### Alerting — Torii Pre-Auth Gating

Pre-auth connection gating exposes two metrics:
- `torii_pre_auth_reject_total{reason}` — counter of rejected connections. Reasons: `global_cap`, `ip_cap`, `rate`, `ban`.
- `torii_operator_auth_total{action,result,reason}` — operator auth events; `action` is `gate|register_options|register_verify|login_options|login_verify`, `result` is `allowed|denied|rate_limited|locked`, and `reason` mirrors the auth error labels.
- `torii_operator_auth_lockout_total{action,reason}` — operator auth lockouts per action and failure reason.
- `torii_contract_throttled_total{endpoint}` — contract API requests rejected by the deploy limiter (`endpoint` = `code`, `deploy`, `activate`).
- `torii_contract_errors_total{endpoint}` — contract API requests that failed for other reasons (missing token, queue error, etc.).
- `torii_active_connections_total{scheme}` — gauge tracking concurrent connections per scheme (`http`, `ws`).

Suggested alert for sustained rejections:
```
alert: ToriiPreAuthRejects
expr: increase(torii_pre_auth_reject_total[5m]) > 10
for: 10m
labels:
  severity: warning
annotations:
  summary: "Torii pre-auth gating is rejecting clients"
  description: |
    Instance {{ $labels.instance }} rejected {{ $value }} connections (reason={{ $labels.reason }}).
    Inspect Torii pre-auth configuration and recent traffic patterns. Check allowlists for trusted operators.
```

Operational guidance:
- Track `torii_active_connections_total` during incidents to confirm capacity pressure.
- Use CIDR allowlists for monitoring systems that should bypass gating.
- When bans trigger repeatedly, inspect the offending IP, update policies, and clear bans by restarting Torii if needed.

Refer to `sorafs/provider_advert_rollout.md` for the R0–R3 enforcement
timeline, Grafana export (`grafana_sorafs_admission.json`), and the canonical
alert wiring that Observability keeps in sync across environments.

### Torii Norito-RPC Observability

Norito-RPC transport telemetry requirements are captured in `docs/source/torii/norito_rpc_telemetry.md`. Key metrics:
- `torii_request_duration_seconds_bucket{scheme}` — scheme-level latency histogram; filter on `scheme="norito_rpc"` for burn-in dashboards.
- `torii_request_failures_total{scheme,code}` — error counter keyed by connection scheme and HTTP status.
- `torii_http_requests_total{content_type,status,method}` — request counter exposing `content_type="application/x-norito"` for Norito calls.
- `torii_http_request_duration_seconds_bucket{content_type,method}` — latency histogram for content-type parity dashboards.
- `torii_http_response_bytes_total{content_type,method,status}` — optional payload-size counter for regression detection.
- `torii_norito_decode_failures_total{payload_kind,reason}` — bucketed Norito RPC decode failures (invalid magic, checksum mismatch, unsupported feature, etc.).
- `torii_address_invalid_total{surface,reason}` — rejects grouped by Torii surface (e.g., `routing.source`, `iso_bridge.source`) and the stable address error code so SDK drift or malformed I105 literals are visible.
- Local‑8 specific counters are retired; rely on `torii_address_invalid_total{surface,reason}` and `torii_address_collision_total{surface,kind}` to monitor address ingestion and Local‑12 safety.
- Existing gauges (`torii_active_connections_total{scheme}`, `torii_pre_auth_reject_total{reason}`) must include `scheme="norito_rpc"` to track transport gating.

Alert rules live in `dashboards/alerts/torii_norito_rpc_rules.yml` (see companion test `dashboards/alerts/tests/torii_norito_rpc_rules.test.yml`). Highlights:
- `ToriiNoritoRpcErrorSpike` warns when Norito 5xx responses exceed allowable thresholds for five minutes.
- `ToriiNoritoRpcLatencyDegraded` fires when Norito P95 latency breaches 750 ms.
- `ToriiNoritoRpcSilentTraffic` detects 30 minutes without Norito requests, signalling misrouted traffic.

Use `scripts/telemetry/test_torii_norito_rpc_alerts.sh` to run `promtool test rules` locally or in CI so alert updates stay in sync with dashboards.

### SoraFS Data Availability Observability Pack (DA-9)

- **Dashboards.**
  - `dashboards/grafana/sorafs_fetch_observability.json` visualises ingestion latency (`histogram_quantile` over `sorafs_orchestrator_fetch_duration_ms` / `sorafs_orchestrator_chunk_latency_ms`), retry/backoff counters, and transport errors so DA fetch regressions are visible during burn-in.
  - `dashboards/grafana/sorafs_gateway_observability.json` focuses on replication/backlog metrics such as `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_deadline_slack_epochs`, PoR ingest queues (`torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total`), and gateway refusal/throttle counters.
  - `dashboards/grafana/sorafs_capacity_penalties.json` tracks retention (`torii_sorafs_storage_bytes_{used,capacity}`), accumulated GiB·hours, replication SLA outcomes (`torii_sorafs_replication_sla_total`), disputes/slash proposals, and fee projections so finance/storage stakeholders review the same evidence bundle.
  - `dashboards/grafana/sorafs_capacity_health.json` graphes declared/effective/utilised GiB, outstanding reservations, backlog depth, and expired-order rates per provider to highlight saturation before SLO drift.
- **Chunking cost.** `torii_da_chunking_seconds` tracks DA chunking + erasure coding CPU time; monitor p95 via `histogram_quantile(0.95, sum(rate(torii_da_chunking_seconds_bucket[5m])) by (le))`.
- **Proof health.** PDP/PoTR pass rates surface through `torii_sorafs_proof_stream_events_total{kind="pdp|potr",result}`; latency distributions are published via `torii_sorafs_proof_stream_latency_ms`. Pair them with `torii_sorafs_storage_por_samples_{success,failed}_total` to prove challenge coverage and link panels directly to provider alerts.
- **Retention digest.** `docs/source/status/sorafs_da_weekly_digest.md` is the weekly template mandated by DA-9. Populate it with snapshots exported from the dashboards above plus the PromQL snippets used to compute p95 ingestion latency and backlog deltas; attach the Markdown to the Friday digest mail that SRE sends to Storage + Product.
- **Alerting.** Import `dashboards/alerts/sorafs_fetch_rules.yml`, `dashboards/alerts/sorafs_gateway_rules.yml`, and `dashboards/alerts/sorafs_capacity_rules.yml` so ingestion, gateway, and capacity saturation signals stay covered. Validate each pack with its fixture (`dashboards/alerts/tests/sorafs_fetch_rules.test.yml`, `dashboards/alerts/tests/sorafs_capacity_rules.test.yml`) via `promtool test rules …` (or `scripts/check_prometheus_rules.sh dashboards/alerts/sorafs_*`) before publishing changes.
- **Runbook integration.** Incident tickets tied to DA need three artefacts: (1) Grafana JSON exports (`sorafs_fetch_observability` and `sorafs_gateway_observability`), (2) the filled digest template, and (3) alert UUIDs from the rule groups above. This satisfies the DA-9 roadmap requirement that SRE/operator reviews include dashboards plus a narrative digest.

### Confidential Tree Telemetry (M2.2)

Roadmap item **M2.2 — Gas & Telemetry** now exposes per-asset gauges and eviction
counters for the confidential commitment tree so operators can prove tree depth,
root-history hygiene, and checkpoint behaviour:

- `iroha_confidential_tree_commitments{asset_id}` — current number of commitments (leaves).
- `iroha_confidential_tree_depth{asset_id}` — Merkle depth in levels (bounded by the verifier profile).
- `iroha_confidential_root_history_entries{asset_id}` — retained historical roots (should match `zk.root_history_cap`).
- `iroha_confidential_frontier_checkpoints{asset_id}` — number of recorded frontier checkpoints.
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}` — height of the last frontier checkpoint (0 when no checkpoint exists).
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}` — commitment count captured alongside the last checkpoint.
- `iroha_confidential_root_evictions_total{asset_id}` — counter of root-history evictions after enforcing the cap.
- `iroha_confidential_frontier_evictions_total{asset_id}` — counter of frontier checkpoint evictions when the interval/depth window trims history.

Grafana panel `dashboards/grafana/confidential_assets.json` graphs the gauges
and pairs the eviction counters with alert rules that fire when roots or
checkpoints churn faster than expected. Capture evidence with a simple scrape:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)){asset_id="xor#wonderland"}'
```

Pair this with `rg 'iroha_confidential_(root|frontier)_evictions_total'` to prove
the eviction counters advanced after a maintenance window. The calibration doc
(`docs/source/confidential_assets_calibration.md`) records the signed baselines
and links to the corresponding governance acknowledgement.

The same dashboard now surfaces the Halo2 verifier cache counters exposed via
`iroha_zk_verifier_cache_events_total{cache,event}` (cache = `vk` |
`builtin`, event = `hit` | `miss`). Use these counters to compute the 5-minute
miss ratio:

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

`dashboards/alerts/confidential_assets_rules.yml` ships the
`ConfidentialVerifierCacheMissSpike` warning whenever misses exceed 40%
of lookups for ten minutes, ensuring cache regressions are caught alongside the
tree-depth guard.

#### Norito RPC degraded runbook

Use this checklist when the Norito transport fails SLOs or generates alerts:

- **Primary signals:** `ToriiNoritoRpcErrorSpike`, `ToriiNoritoRpcLatencyDegraded`, `ToriiNoritoRpcSilentTraffic`, `torii_norito_decode_failures_total`, and `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` panels in `dashboards/grafana/torii_norito_rpc_observability.json`.
- **Client corroboration:** SDK CI and mock-harness jobs export `torii_mock_harness_retry_total`, `torii_mock_harness_duration_ms`, and `torii_mock_harness_fixture_version`; spikes in these client-side metrics usually indicate regressions before production traffic is impacted.
- **Support tooling:** `python/iroha_python/scripts/run_norito_rpc_smoke.sh` validates end-to-end request/response flows, and `scripts/telemetry/test_torii_norito_rpc_alerts.sh` confirms alert expressions continue to match the on-disk rules after remediation.

**Immediate triage**

1. Determine the failure mode:
   - Error spike: `torii_request_failures_total{scheme="norito_rpc"}` or decode counters rise.
   - Latency breach: `histogram_quantile(0.95, torii_request_duration_seconds_bucket{scheme="norito_rpc"})` exceeds 750 ms.
   - Silent traffic: `torii_request_duration_seconds_count{scheme="norito_rpc"}` flat-lines while HTTP/JSON remain healthy.
2. Scope the impact with dashboards, then inspect Torii logs filtered on `ConnScheme::NoritoRpc` for `schema_mismatch`, checksum, or TLS errors. Decode failures emit the exact `reason` tag shown in telemetry.
3. Verify edge ingress still forwards the binary payloads by running `curl -H 'Content-Type: application/x-norito' https://<torii-host>/rpc/ping` (or the mock harness) from multiple regions. A JSON-looking response indicates a proxy stripped the header.
4. Compare server metrics with SDK mock-harness counters. If retries spike in CI but production traffic is normal, halt the client rollout and coordinate with SDK owners instead of throttling Norito globally.

**Mitigation options**

- **Misbehaving clients:** Gate them via `torii.preauth_scheme_limits.norito_rpc` until parity is restored; the config lives under `client_api` in `iroha_config`.
- **Decode or schema mismatches:** Ensure Torii and SDKs run the same fixture bundle by checking `fixtures/norito_rpc/schema_hashes.json` (the DTO→hash table); regenerate fixtures (`cargo xtask norito-rpc-fixtures --all`) if hashes diverge.
- **Ingress/proxy issues:** Fix header forwarding or MTU settings, then rerun `python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **Full brownout / rollback:** If service impact persists, flip `torii.transport.norito_rpc.stage` to `canary` or `disabled` (per `docs/source/torii/norito_rpc_rollout_plan.md`), reload Torii, and ensure `/rpc/capabilities` reports the downgraded stage so SDKs fall back to JSON without guessing. Record the change in the canary runbook (`docs/source/runbooks/torii_norito_rpc_canary.md`).

**Verification**

1. Watch `torii_request_failures_total` and the decode counter return to baseline; clear Alertmanager silences only after the metrics stay flat for one evaluation period.
2. Confirm `torii_active_connections_total{scheme="norito_rpc"}` stabilises and the `ToriiNoritoRpcSilentTraffic` alert stays green.
3. Re-run the Norito RPC smoke test (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) and alert tests (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).
4. Capture evidence (Grafana PNGs, config patches, CLI outputs) and attach it to the NRPC-2 runbook ticket plus `status.md` so the roadmap artifact remains auditable.

A new Prometheus counter `sumeragi_membership_mismatch_total{peer,height,view}` and gauge `sumeragi_membership_mismatch_active{peer}` were introduced to detect validator roster divergence. `/v1/sumeragi/status` now surfaces a `membership_mismatch` block with the active peer list and last mismatch context to speed triage.

The gauges `sumeragi_membership_view_hash`, `sumeragi_membership_height`, `sumeragi_membership_view`, and `sumeragi_membership_epoch` expose the deterministic membership hash together with the `(height, view, epoch)` context. Compare these values across peers to confirm roster alignment without waiting for mismatch alarms.

Recommended alert (recorded in the runbook):

```
alert: SumeragiMembershipMismatch
expr: increase(sumeragi_membership_mismatch_total[5m]) > 0
for: 5m
labels:
  severity: warning
annotations:
  summary: "Consensus membership mismatch detected"
  description: |
    Node {{ $labels.instance }} observed validator membership mismatch for peer {{ $labels.peer }} at height {{ $labels.height }} view {{ $labels.view }}.
    Investigate peer configuration, on-chain `SumeragiParameters`, and recent key rotation events.
```

Operations checklist:
- Verify the mismatch is expected (e.g., pending topology change) via `/v1/sumeragi/status`.
- If unexpected, quarantine the offending peer and confirm configuration files match the on-chain roster.
- After remediation, ensure `sumeragi_membership_mismatch_active{peer}` returns to `0`.

## Nexus scheduler TEU metrics

The Nexus scheduler exports TEU-focused metrics once the lane router lands. All
series are exposed behind the `metrics` capability and default to `0` in
single-lane mode so dashboards can be provisioned early.

Key metrics:
- `nexus_scheduler_lane_teu_capacity{lane}` (gauge) — per-lane TEU cap.
- `nexus_scheduler_lane_teu_slot_committed{lane}` (gauge) — TEU used during
  the most recent slot.
- `nexus_scheduler_lane_teu_slot_breakdown{lane,bucket}` (gauge) — stacked
  view of floor/headroom/must-serve/circuit-breaker consumption.
- `nexus_scheduler_lane_teu_deferral_total{lane,reason}` (counter) — TEU
  deferred because the lane hit a cap, quota, or envelope limit.
- `nexus_scheduler_dataspace_teu_backlog{lane,dataspace}` (gauge) — queued
  TEU demand remaining after scheduling.
- `nexus_scheduler_dataspace_age_slots{lane,dataspace}` (gauge) — slots
  since the dataspace was last served.
- `nexus_scheduler_starvation_bound_slots{lane}` (gauge) — configured
  starvation bound applied to the lane.
- `nexus_scheduler_must_serve_truncations_total{lane}` (counter) — truncated
  must-serve slices.
- `nexus_scheduler_lane_trigger_level{lane}` (gauge) — current circuit-breaker tier.

The `/status` endpoint mirrors these gauges and now ships richer per-lane
pipeline summaries via `nexus_scheduler_lane_teu_status` responses. Each lane
snapshot includes the scheduler graph counters (`tx_vertices`, `tx_edges`,
`overlay_count`, `overlay_instr_total`, `overlay_bytes_total`, `rbc_chunks`,
`rbc_bytes_total`) plus:

- `peak_layer_width`, `layer_count` — outer bounds of the scheduler layering
  that executed for the lane in the latest block.
- `avg_layer_width`, `median_layer_width`,
  `scheduler_utilization_pct` — basic moments for histogram dashboards.
- `layer_width_buckets[le]` — monotonically increasing buckets (`le =
  1,2,4,8,16,32,64,128`) matching the Prometheus histogram shown on Grafana.
- `manifest_required`, `manifest_ready` — whether the lane requires a manifest and if one is loaded.
- `manifest_path` — best-effort path of the active manifest (surfaced for operators).
- `manifest_validators`, `quorum` — validator roster and quorum declared by the manifest.
- `protected_namespaces` — namespaces gated by the lane’s governance policy.
- `runtime_upgrade` — snapshot of the runtime-upgrade hook (`allow`, `require_metadata`, `metadata_key`, `allowed_ids`).
- `dataspace_id`, `dataspace_alias` — identify which dataspace the lane services, matching the scheduler backlog gauges.
- `storage_profile` — lane storage strategy (`full_replica`, `commitment_only`, `split_replica`).
- `detached_prepared`, `detached_merged`, `detached_fallback` — detached overlay
  execution counters sized to the lane.
- `quarantine_executed` — quarantine fallbacks the lane had to drain during the
  block.

The sister collection `nexus_scheduler_dataspace_teu_status` mirrors the
per-dataspace backlog view and reports `tx_served` plus the dataspace
`fault_tolerance` (f) so operators can see the configured lane-relay committee
sizing (`3f+1`) in the same status payload.

The transaction queue now keeps these snapshots warm in between block commits.
Routing rules can specify dataspace aliases, so `ConfigLaneRouter` resolves both
lane and dataspace IDs before the queue updates telemetry.
Every time a transaction is enqueued or drained from consensus, the queue
recomputes pending TEU per lane/dataspace and updates the gauges. Operators can
therefore watch `nexus_scheduler_dataspace_teu_backlog` to understand backlog
pressure even before the next slot envelope is assembled. Lane headroom now
reflects remaining capacity (`capacity - min(pending_teu, capacity)`) instead of
mirroring backlog directly, so `nexus.scheduler.headroom` only fires when a lane
actually consumes most of its configured TEU budget rather than whenever the
queue is non-empty.

#### Nexus configuration drift

- `nexus_config_diff_total{knob,profile}` (counter) — increments whenever the active Nexus configuration diverges from the single-lane baseline. The `knob` label identifies the section that changed (for example `nexus.lane_catalog.count`, `nexus.routing.rules`, `nexus.da`), and `profile` is set to `active`.

Alert snippet:

```
increase(nexus_config_diff_total{profile="active"}[5m]) > 0
```

Page outside planned maintenance windows. Each increment also emits a `telemetry` log entry `nexus.config.diff` with a Norito JSON payload listing `baseline` and `current` values — review it during config rollouts and the `TRACE-TELEMETRY-BRIDGE` dry-run to confirm the expected knobs moved.

- `nexus_lane_configured_total` (gauge) — reports how many Nexus lane catalog entries the node has applied. Compare it against the expected lane count (for example, `1` for single-lane bundles or `3` for Nexus multi-lane deployments) to catch misconfigured peers ahead of routed-trace audits. When `nexus.enabled=false`, lane/dataspace metrics (including this gauge) are reset and filtered out of `/metrics` and `/status` so Iroha 2 deployments stay lane-free.

Alert snippet:

```
nexus_lane_configured_total != EXPECTED_LANE_COUNT
```

Example PromQL snippets:
- Lane headroom: `nexus_scheduler_lane_teu_capacity - nexus_scheduler_lane_teu_slot_committed`.
- Starvation bound: `max_over_time(nexus_scheduler_dataspace_age_slots[5m])` vs `max(nexus_scheduler_starvation_bound_slots)`.
- Deferrals per reason: `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[10m])`.
- Truncated must-serve: `increase(nexus_scheduler_must_serve_truncations_total[30m])`.

Dashboard:
- Import `docs/source/grafana_scheduler_teu.json`. Panels include lane capacity
  stats, stacked bucket timeseries, backlog heatmap, starvation bound trend, and
  trigger status table. Set `$PROM_DS` to your Prometheus data source after import.

Alert thresholds:
- Cap exhaustion: trigger when `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` for `15m`.
- Starvation bound: trigger when
  `max(nexus_scheduler_dataspace_age_slots) >= max(nexus_scheduler_starvation_bound_slots)` for `5m`.
- Must-serve truncation: ticket when
  `increase(nexus_scheduler_must_serve_truncations_total[1h]) > 0`.

Operator triage:
1. Run `iroha_cli app nexus lane-report --lane <id>` to inspect the lane cap,
   configured bound, and backlog snapshot (CLI update tracked under Nexus
   router workstreams).
2. Check `nexus_scheduler_lane_trigger_level` (tier `>0` implies a
   circuit-breaker is reducing caps); reference `docs/source/nexus_transition_notes.md`
   for trigger semantics.
3. Inspect Torii logs (`pipeline::scheduler`) for per-slot summaries including
   bucket breakdowns and queue diffs.
4. If starvation continues, verify `routing_policy` and dataspace quotas in
   configuration; confirm peers share identical catalog hashes.

##### Lane topology events

- Telemetry log `nexus.lane.topology` — emitted whenever a lane is provisioned, retired, or renamed. Each Norito JSON payload includes:
  - `action`: `provisioned`, `retired`, or `alias_migrated`.
  - `lane_id`, `alias`, `slug`, `dataspace_id`, `visibility`, and `storage_profile` for new lanes.
  - For migrations: `alias_before`, `alias_after`, `slug_before`, `slug_after`.

Operator workflow:
1. Tail `journalctl -u irohad -o json | jq 'select(.msg==\"nexus.lane.topology\")'` (or subscribe via the OTLP bridge) whenever the governance catalog changes so you capture an immutable record of the storage changes.
2. Store the JSON payloads with the change request; auditors cross-check them with `nexus_lane_configured_total` and Kura’s directory layout.
3. Add alert rules when unexpected events arrive (for example, `action="retired"` outside a maintenance window). A simple Loki/Grafana expression is `count_over_time({job="irohad"} |= "nexus.lane.topology" | json | action="retired" [5m]) > 0`.
4. Feed the events into automation such as `scripts/nexus_lane_smoke.py --from-telemetry telemetry.ndjson --require-alias-migration alpha:payments` so the smoke tests can assert that relabeling occurred after a rename. The helper rejects telemetry logs that do not contain the expected alias entries, making it safe to gate CI on the recorded `nexus.lane.topology` payloads.

##### Lane headroom telemetry log

- When per-lane headroom drops below 15 % of its configured capacity, hits zero,
  or `nexus_scheduler_lane_trigger_level > 0`, Iroha emits the structured log
  `nexus.scheduler.headroom`. Each Norito payload records `lane_id`,
  `capacity`, `committed`, `headroom_teu`, `headroom_pct`, bucket breakdown,
  `trigger_level`, `starvation_bound_slots`, and cumulative deferrals. Tail the
  log during drills with:

  ```sh
  journalctl -u irohad -o json | jq 'select(.msg=="nexus.scheduler.headroom")'
  ```

- Prometheus exposes `nexus_scheduler_lane_headroom_events_total{lane_id}` so you
  can alert when the log fires more than a handful of times per hour outside
  rehearsals. Combine it with `nexus_scheduler_lane_teu_slot_breakdown` to tune
  TEU alerts.
- After uploading the telemetry pack, run
  `scripts/telemetry/validate_nexus_telemetry_pack.py --pack-dir <dir>
  --workload-seed NEXUS-REH-2026Q1 --slot-range 820-860` to generate
  `telemetry_manifest.json` + `.sha256` for the evidence bucket and tracker.

#### OTLP capture batch sizing

The OTLP exporter used during rehearsals throttled when its batch queue capped
out. Configure your collector (for example `otelcol-contrib`) with a 256-sample
batch to keep `otlp.ndjson` captures within the rehearsal window:

```yaml
exporters:
  otlphttp:
    endpoint: https://telemetry.example.net/otlp
processors:
  batch:
    timeout: 5s
    send_batch_size: 256
    send_batch_max_size: 256
extensions:
  health_check: {}
service:
  extensions: [health_check]
  pipelines:
    metrics:
      receivers: [otlp]
      processors: [batch]
      exporters: [otlphttp]
```

## Nexus finality, DA quorum, and oracle SLO telemetry (NX-18)

NX-18 introduces a dedicated metric set for the 1 s finality gate. The gauges
and histograms ship in every build (single-lane deployments simply emit steady
`0`/baseline values) so operators can configure dashboards and alerts ahead of
the Nexus cut-over. All metrics back the Grafana board stored in
`dashboards/grafana/nexus_lanes.json` and the runbook documented in
`docs/source/runbooks/nexus_lane_finality.md`.

### Metric cheat sheet

| Metric | Description | SLO / Action |
|--------|-------------|--------------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | Slot-duration histogram derived from the end of every Sumeragi slot. | Keep p95 ≤ 1 000 ms (warning at 950 ms). Breaches must trigger the slot runbook and be recorded in the NX-18 drill log. |
| `iroha_slot_duration_ms_latest` | Gauge of the most recent slot duration. | Capture alongside the histogram whenever filing incidents; sustained spikes > 1 100 ms indicate an unhealthy validator even if quantiles remain green. |
| `iroha_da_quorum_ratio` | Rolling fraction of slots that satisfied the DA quorum window. | Target ≥ 0.95; combine with `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` to locate failing attesters or timeouts. |
| `sumeragi_rbc_da_reschedule_total` | Legacy counter for DA-driven slot reschedules (no longer incremented). | Keep at zero; investigate missing-availability counters instead (see `ops/runbooks/da-quorum.md`). |
| `iroha_oracle_price_local_per_xor` | Latest TWAP reported by the lane-specific oracle. | Watch for spikes when swap lines are thin; tie haircuts to treasury reports. |
| `iroha_oracle_staleness_seconds` | Seconds since the last oracle refresh. | Alert at ≥ 75 s; fail the NX-18 gate at ≥ 90 s until the feed is restarted. |
| `iroha_oracle_twap_window_seconds` | Effective TWAP window length. | Should remain at 60 s ± 5 s; deviations mean the oracle config drifted. |
| `iroha_oracle_haircut_basis_points` | Applied haircut per lane/dataspace. | Compare against the liquidity tier table before approving router changes. |
| `iroha_settlement_buffer_xor` / `iroha_settlement_buffer_capacity_xor` | Remaining buffer headroom and configured capacity. | Soft alert at 25 %, hard alert at 10 %; below the hard threshold force XOR-only routing and log the incident. |

### Automation & evidence

- `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` parses Prometheus dumps and enforces the p95/p99 gates. CI wires this via `ci/check_nexus_lane_smoke.sh` so every release candidate ships the JSON summary next to the metrics snapshot (`fixtures/nexus/lanes/metrics_ready.prom` provides the sample pack for local validation).
- `scripts/telemetry/bundle_slot_artifacts.py --metrics <metrics.prom> --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` emits `slot_bundle_manifest.json` and SHA-256 digests for the required artefacts. `scripts/run_release_pipeline.py` invokes it automatically (skip with `--skip-nexus-lane-smoke`) so NX-18 sign-offs include immutable evidence.
- The chaos/acceptance harness now runs `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` inside `ci/check_nexus_lane_smoke.sh` to gate DA quorum, oracle staleness/TWAP/haircuts, settlement buffers, and slot quantiles in one place. Keep the thresholds in the script aligned with the dashboards/alert rules.
- Capture routed-trace telemetry with `scripts/telemetry/check_nexus_audit_outcome.py` and archive the resulting JSON under `docs/examples/nexus_audit_outcomes/`. The tool enforces that every rehearsal produced a `nexus.audit.outcome` event and keeps the `TRACE-TELEMETRY-BRIDGE` checkpoints auditable.
- Record all drills with `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>` so quarter-end reports can enumerate slot, DA, oracle, and buffer rehearsals.

### Operator references

- Dashboards: import `dashboards/grafana/nexus_lanes.json` and keep it in sync with production panels, including quantile/ratio thresholds and Alertmanager annotations.
- Runbooks: use `docs/source/runbooks/nexus_lane_finality.md` for finality/oracle procedures and `ops/runbooks/da-quorum.md` for DA-specific mitigations. Both documents call out the metrics above plus evidence requirements.
- Release gates: ensure `status.md` entries link to the latest NX-18 slot bundle and Grafana captures whenever preparing a release candidate; the roadmap requires signed artefacts before multi-lane code paths are enabled by default.

Environment variables provide the same knobs (`OTEL_BLRP_MAX_EXPORT_BATCH_SIZE`
and `OTEL_METRIC_EXPORT_INTERVAL`). Set both to `256` and `5s` respectively
during Nexus rehearsals, then archive the resulting `otlp.ndjson` inside the
telemetry pack before running the validation script above.

#### Routed-trace audit outcomes

- `telemetry` log `nexus.audit.outcome` — emitted via `Telemetry::record_audit_outcome` whenever a routed-trace checkpoint completes. The Norito payload includes `trace_id`, `slot_height`, `reviewer`, `status` (for example `pass`, `fail`, `mitigated`), and an optional `mitigation_url`.
- Prometheus surfaces `nexus_audit_outcome_total{trace_id,status}` and `nexus_audit_outcome_last_timestamp_seconds{trace_id}` so dashboards and alert rules can track routed-trace health.

Operator workflow:
1. During `TRACE-*` rehearsals, tail the telemetry stream (`journalctl -u irohad -o json` or OTLP bridge) and confirm an event appears for each scheduled audit window.
2. Archive the JSON payload alongside the audit artefacts so reviewers can trace the verdict and mitigation link.
3. Run `scripts/telemetry/check_nexus_audit_outcome.py` against the telemetry log (for example, `--trace-id TRACE-TELEMETRY-BRIDGE --window-start <ISO time> --window-minutes 30`). The helper enforces that a matching payload exists, fails the run if a disallowed status (default `fail`) is observed, and stores the JSON artefact under `docs/examples/nexus_audit_outcomes/` for audit records.
4. Alert when `status="fail"` or if no event is observed within 30 minutes of the expected audit slot; the Prometheus rule `dashboards/alerts/nexus_audit_rules.yml` fires on failing statuses, while CI should integrate the script above to gate the “missing outcome” requirement.


## Norito Streaming Telemetry

Prometheus metrics exposed by the Norito streaming runtime:

- `streaming_encode_latency_ms`: histogram of publisher encode latency.
- `streaming_encode_audio_jitter_ms`: EWMA audio jitter observed at the publisher (ms).
- `streaming_encode_audio_max_jitter_ms`: gauge tracking the peak audio jitter observed (ms).
- `streaming_encode_dropped_layers_total`: counter of rendition layers dropped during encode.
- `streaming_decode_buffer_ms`: viewer buffer depth histogram.
- `streaming_decode_dropped_frames_total`: counter for decoder frame drops.
- `streaming_decode_max_queue_ms`: histogram of max decode queue depth.
- `streaming_decode_av_drift_ms`: histogram of absolute audio/video drift measured at viewers.
- `streaming_decode_max_drift_ms`: gauge for maximum audio/video drift observed (ms).
- `streaming_audio_jitter_ms`, `streaming_audio_max_jitter_ms`: viewer-reported audio jitter histogram + peak gauge derived from `ReceiverReport` diagnostics.
- `streaming_av_drift_ms`, `streaming_av_max_drift_ms`: viewer-reported audio/video drift histogram + peak gauge (absolute milliseconds).
- `streaming_av_drift_ewma_ms`: signed gauge tracking the viewer EWMA drift used for throttling decisions.
- `streaming_av_sync_window_ms`: gauge exposing the active aggregation window (ms) advertised by viewers.
- `streaming_av_sync_violation_total`: counter incremented when viewers flag segments beyond the ±10 ms sync budget.
- `streaming_network_rtt_ms`, `streaming_network_loss_percent_x100`, `streaming_network_fec_{repairs,failures}_total`, `streaming_network_datagram_reinjects_total`: network health metrics.
- `streaming_energy_encoder_mw`, `streaming_energy_decoder_mw`: power usage gauges from publishers/viewers.

Telemetry payloads emitted over Norito (`TelemetryEncodeStats`, `TelemetryDecodeStats`, and the new `SyncDiagnostics` bundle carried inside `ReceiverReport`) now include audio jitter and drift fields so dashboards can surface out-of-sync segments. See `docs/source/project_tracker/nsc28b_av_sync_telemetry.md` for the enforcement rollout plan.
