---
lang: ur
direction: rtl
source: docs/source/sumeragi.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9caf355c02cf37191beb19cbdc56478a1896367a0017ec7c6a8121886e346fff
source_last_modified: "2026-01-29T20:26:30.094718+00:00"
translation_last_reviewed: 2026-01-30
---

## Sumeragi: Current Implementation (v1)

For a granular view of the remaining migration work, see
[`sumeragi_npos_task_breakdown.md`](sumeragi_npos_task_breakdown.md).

Overview
- Roles and rotation: the ordered topology partitions peers into roles — `Leader`, `ValidatingPeer`, `ProxyTail`, and `SetBValidator`. Before each leader selection, the roster is canonicalized by sorting peer IDs to keep ordering deterministic across nodes. In permissioned mode the roster is deterministically shuffled per block height using PRF(seed, height) (seeded by the epoch PRF/VRF state), and view changes rotate the shuffled topology to advance the leader; previous-block-hash rotation is no longer used for leader selection. In NPoS mode the active validator roster is canonicalized (sorted) before PRF leader selection so signer indices remain consistent across peers, and the PRF-derived leader index rotates the view-aligned topology so signatures and collectors agree on who is index 0. `rotated_for_prev_block_hash(prev_hash)` in `network_topology.rs` remains as a legacy deterministic helper.
- Fault tolerance and quorum: for validator roster size `n`, the runtime derives `f = floor((n-1)/3)` and uses commit quorum `2f+1` for `n >= 4` (see `commit_quorum_from_len` in `network_topology.rs`); for `n <= 3` the quorum is `n` (all validators). Set A is always `min_votes_for_commit()`; to change `f`, adjust the validator roster size via `trusted_peers` or the NPoS stake roster.
- K‑collector mode: per height/view, the topology designates K collectors via PRF selection from the canonical roster, excluding the leader. When a PRF seed is unavailable, the system falls back to the wraparound slice starting at `proxy_tail_index()` (inclusive), skipping the leader and filling at least the commit quorum size (bounded by non-leader peers). Setting K=1 selects a single PRF collector; when PRF is unavailable the fallback still fills at least the commit quorum (starting at the proxy tail). Validators can also send commit votes to a small parallel topology fanout (`collectors.parallel_topology_fanout`) to reduce collector hop latency.
- Commit certificates: validators sign the block header and send a `CommitVote` to the deterministic collector set (PRF-selected, excluding the leader); when collectors are disabled or local-only, votes fall back to the full commit topology. Any collector that reaches quorum (2f+1 in permissioned mode, or ≥2/3 total stake in NPoS) gossips a `CommitCertificate`; peers commit on the certificate + payload. Prepare/NewView certificates and availability evidence remain for pacemaker/telemetry but do not gate commit.

### Safety and Liveness (Proof Sketch)

This is a proof sketch for the current v1 implementation (not a formal proof).
It summarizes the invariants enforced by the code paths in
`crates/iroha_core/src/sumeragi/main_loop/{proposal_handlers,commit,qc,votes}.rs`.

Assumptions (both modes)
- Partial synchrony after GST (eventual bounded message delays).
- Cryptography is secure (BLS signatures unforgeable; hash functions collision resistant).
- Deterministic roster selection and view alignment are consistent across honest peers.
- Payload dissemination eventually succeeds (BlockCreated + block sync and/or RBC).

Safety (permissioned)
- Quorum intersection: for `n = 3f+1`, the commit quorum is `2f+1`; any two quorums intersect
  in at least `f+1` validators, so at least one honest validator is in the intersection.
- Single-vote rule: a validator records at most one commit vote per `(height, view, epoch)` and
  will not sign a conflicting block at the same height/view (see `emit_precommit_vote`).
- Locked-QC rule: a validator only accepts proposals and emits votes that extend the locked
  QC (`qc_extends_locked_with_lookup`), preventing honest validators from signing conflicting
  chains once they are locked.
- Therefore, two conflicting commit certificates at the same height would require an honest
  validator to sign both branches, which is excluded by the single-vote + locked-QC rules.

Safety (NPoS)
- Quorum is defined by stake: a commit QC requires ≥2/3 of total stake in the active roster
  (`stake_quorum_reached_for_peers`).
- If <1/3 of stake is Byzantine, any two ≥2/3 stake quorums intersect in >1/3 stake that must
  include honest validators.
- The same single-vote and locked-QC rules apply, so honest stake will not sign conflicting
  branches at the same height/view, ruling out two conflicting commit certificates.

Liveness (permissioned)
- The pacemaker advances views on timeouts and missing-payload/validation failures; view
  changes are monotonically increasing for a height (see `trigger_view_change_with_cause`).
- Once the network is synchronous and an honest leader is selected for a view, proposals and
  commit votes propagate to collectors/topology; `2f+1` votes produce a commit QC and the
  block is committed when the payload is available.
- If payloads are missing, block sync/RBC recovery retries; persistent failure triggers a view
  change, ensuring the protocol does not stall on a single leader or payload.

Liveness (NPoS)
- The pacemaker logic is the same, but commit QCs require ≥2/3 stake; liveness holds when
  ≥2/3 of stake (by the active roster) is online and able to exchange votes.
- Deterministic PRF leader selection plus view changes ensure that an honest leader is
  eventually chosen after GST, allowing a commit QC to form.

DA/RBC note
- Data availability is tracked and recovered (RBC + block sync), but commit does not wait for
  availability evidence; it only requires the local payload. Missing payloads are handled by
  retries and view changes so the protocol keeps making progress.

### NPoS mode configuration (operators)

2. **Choose the roster source.** `sumeragi.npos.use_stake_snapshot_roster = true` tells the validator to hydrate the epoch roster from the staking snapshot provider (required for production NPoS). Leaving it `false` continues to mirror `trusted_peers` so small devnets can stage upgrades without the staking sidecar. When stake records are missing for some roster peers, NPoS assigns the minimum self-bond (or 1 when unset) to the missing entries; if no public-lane stake records exist, every roster peer receives the same fallback stake for quorum checks. If the commit topology omits active validators, the epoch roster widens to include the active set before filtering so validator elections and quorum checks retain the full active roster.
3. **Define epoch cadence.** `sumeragi.npos.epoch_length_blocks` controls how long a validator set lives. Within each epoch, `sumeragi.npos.vrf.commit_window_blocks` and `.reveal_window_blocks` fence the VRF commit/reveal RPCs (when omitted, VRF windows and deadlines are derived from `epoch_length_blocks`), and the on-chain `SumeragiParameters.block_time_ms` target (scaled by `pacing_factor_bps`) informs telemetry dashboards and the pacemaker expectation. When `sumeragi_npos_parameters` is present on-chain (genesis or governance), its collector fan-out, `epoch_length_blocks`, VRF commit/reveal windows, and election/reconfig knobs are authoritative and override the local config values. Pacemaker timeouts are derived from `SumeragiParameters.block_time_ms` scaled by `pacing_factor_bps` (basis points, >= 10_000), clamped by `min_finality_ms`, plus any `sumeragi.advanced.npos.timeouts` overrides.
4. **Calibrate collector fan-out.**
   - `sumeragi.collectors.k` decides how many collectors assemble votes per slot.
   - `sumeragi.collectors.redundant_send_r` caps how many additional collectors a validator targets when local timeouts expire.
   - `sumeragi.collectors.parallel_topology_fanout` adds a small topology fanout alongside collectors to reduce hop latency (0 disables).
   - `sumeragi.advanced.npos.timeouts.*_ms` provide the per-phase pacemaker budget (proposal, prevote, precommit, commit, DA, aggregator). These values are milliseconds in the user config and are mirrored directly into the runtime; `exec`/`witness` timeouts remain reserved for future witness pacing and do not influence the pacemaker today. Unset fields derive from the on-chain effective block time (`SumeragiParameters.block_time_ms` scaled by `pacing_factor_bps`).
5. **Record election and reconfiguration policy.**
   - `sumeragi.npos.election.*` sets self‑bond minimums and the guardrails for nominator concentration, seat variance, and validator correlation.
   - `sumeragi.npos.reconfig.{evidence_horizon_blocks,activation_lag_blocks,slashing_delay_blocks}` govern how long evidence is retained, how long it takes for a newly scheduled validator set to activate, and how long consensus slashing is delayed before it applies.

Minimal TOML scaffold:

```toml
[sumeragi]
consensus_mode = "npos"

[sumeragi.collectors]
k = 3
redundant_send_r = 3 # example for 4 validators (2f+1)

[sumeragi.npos]
epoch_length_blocks = 7200
use_stake_snapshot_roster = true
# advanced.npos.timeouts.*_ms derive from the effective block time when omitted

## Optional: override derived VRF windows.
#[sumeragi.npos.vrf]
# commit_window_blocks = 120
# reveal_window_blocks = 60

[sumeragi.npos.election]
min_self_bond = 10_000
max_nominator_concentration_pct = 20
seat_band_pct = 10
max_entity_correlation_pct = 25

[sumeragi.npos.reconfig]
evidence_horizon_blocks = 7200
activation_lag_blocks = 1
slashing_delay_blocks = 259200
```


The same knobs exist in `iroha_config::parameters::actual`, so runtime overrides and the `/configuration` surface use the exact field names referenced above.

#### Configuration audit & evidence

- `iroha --output-format text ops sumeragi params` prints the effective `consensus_mode`, stake-roster toggle, epoch length, and collector knobs that the binary loaded. Capture the output (with timestamp + peer id) before enabling traffic so release packets can prove which consensus mode was activated.
- `iroha --output-format json ops sumeragi params > artifacts/sumeragi-params-<height>.json` emits the raw Norito-parsed structure, including nested `sumeragi.npos.*` fields. Store this alongside the `config.toml` that was deployed so audits can diff the intent vs. runtime configuration without scraping logs.
- `curl -s "$TORII/v2/configuration" | jq '.sumeragi'` (or `GET /v2/configuration` via the SDK `ToriiClient.getConfiguration`) returns the live parameters as seen by Torii. Compare the JSON stanza across validators to ensure the same `consensus_mode`, redundant-send policy, and epoch cadence are in effect everywhere before declaring the lane healthy.
- Record the evidence bundle inside your lane hand-off (CLI summary, `/configuration` snapshot, and the signed config file). Governance requires these attachments before approving an NPoS cut-over, and the artefacts double as rollback proof whenever parameters change.

### Operator readiness checklist (NPoS)

1. **Confirm configuration** — run `iroha --output-format text ops sumeragi params` (or inspect `/configuration`) before rolling validators. The summary prints the live `consensus_mode`, stake-roster toggle, epoch length, and collector knobs so you can prove the binary loaded `npos` with the intended redundancy (`redundant_send_r`). Capture the output in the lane hand-off packet.
2. **Check runtime state** — `iroha --output-format text ops sumeragi status` exposes the active leader/view, epoch tuple, RBC backlog, DA retry counters, and pacemaker deferrals. Pair it with `iroha --output-format text ops sumeragi collectors` to show which peers are acting as collectors on the current height, and `iroha --output-format text ops sumeragi rbc status` to confirm RBC sessions and throughput match expectations before enabling external traffic.
3. **Validate randomness & evidence** — follow {doc}`sumeragi_randomness_evidence_runbook` to pull the VRF epoch snapshot (`iroha ops sumeragi vrf-epoch --epoch <height>`), enumerate penalties (`iroha ops sumeragi vrf-penalties --epoch <height>`), and record the telemetry gauges (`sumeragi_vrf_commits_emitted_total`, `sumeragi_vrf_reveals_late_total`, `sumeragi_vrf_non_reveal_penalties_total`, `sumeragi_vrf_no_participation_total`, etc.). The runbook also covers `/v2/sumeragi/evidence{,/count}` and the CLI wrappers so NPoS slashing evidence is mirrored into the GA bundle.
4. **Archive evidence** — every readiness rehearsal must attach the CLI summaries, JSON snapshots, and SSE capture referenced in the runbook plus the generated Markdown reports (`docs/source/generated/sumeragi_*_report.md`). Keep the artefact directory path recorded in `status.md` and `docs/source/project_tracker/npos_sumeragi_phase_a.md` so governance reviewers can re-download the same run.

#### Mode selection validation flow

Operators must prove every validator actually booted into `npos` before the lane
accepts traffic:

1. Run `iroha --output-format json ops sumeragi params > artifacts/<lane>/mode_check.json`
   immediately after restarting each peer. Compare the captured `consensus_mode`,
   `use_stake_snapshot_roster`, `k_aggregators`, and `redundant_send_r` values
   against the hand-off manifest; mismatches require redeploying the config or
   re-running genesis.
2. Capture `iroha --output-format text ops sumeragi status` (and optionally
   `iroha --output-format json ops sumeragi status > artifacts/<lane>/status.json`) to prove the pacemaker view matches
   the intended epoch and that RBC backlog/backpressure counters are zero before
   opening Torii to clients.
3. Query `/v2/status` and `/v2/sumeragi/status` via `curl -s "$TORII/status"`
   and archive the payload in the same artefact directory. The JSON includes the
   VRF commit/reveal schedule and `prf_epoch_seed`; store it alongside the CLI
   output so auditors can cross-check the CLI and HTTP surfaces.
4. Keep the artefacts under version control (or a signed object store) next to
   the chaos/performance captures referenced later in this document. Doing so
   satisfies the roadmap requirement for deterministic evidence of mode
   selection without adding a separate runbook.

### Telemetry, metrics, and evidence checkpoints

- `/v2/sumeragi/status` now reports `commit_quorum`/`commit_qc` summaries alongside `worker_loop.queue_diagnostics` (blocked/dropped enqueues per queue), `dedup_evictions`, `bg_post_drop_{post,broadcast}_total`, and `commit_inflight` (active commit id/height/view, elapsed/timeout, and pause/resume queue depths) so stalled commit jobs and channel backpressure are visible without log scraping.
- `/v2/sumeragi/status` includes the effective timing snapshot (`effective_min_finality_ms`, `effective_block_time_ms`, `effective_commit_time_ms`, `effective_pacing_factor_bps`, and `effective_npos_timeouts` in NPoS) so operators can validate on-chain timing and scaling in one place.
- `/v2/sumeragi/commit_qc/{hash}` returns the full commit QC record (when available), including `parent_state_root`, `post_state_root`, and the aggregate signature payload needed for independent verification.
- Prometheus metrics provide the same signals for fleet monitoring:
  - `sumeragi_epoch_length_blocks`, `sumeragi_epoch_commit_deadline_offset`, `sumeragi_epoch_reveal_deadline_offset` — confirm the node loaded the intended VRF schedule.
  - `sumeragi_prf_epoch_seed_hex` — last seed published by the VRF pipeline (pairs with `/v2/sumeragi/status.prf_epoch_seed`).
  - `sumeragi_npos_collector_selected_total` and `sumeragi_npos_collector_assignments_by_idx{collector_idx}` — sanity check collector rotation.
- `sumeragi_da_gate_block_total{reason}` and `sumeragi_rbc_da_reschedule_total` — monitor DA availability warnings on missing local payloads; reschedule counters are legacy and should remain zero now that DA is advisory.
  - `sumeragi_missing_block_fetch_total{outcome}` plus `sumeragi_missing_block_fetch_target_total{target="signers|topology|none"}` confirm missing-block fetch cadence and whether signer targeting fell back to the full topology.
  - `sumeragi_rbc_store_pressure`, `sumeragi_rbc_backpressure_deferrals_total`, `sumeragi_rbc_persist_drops_total`, `sumeragi_rbc_store_evictions_total`, and the per‑dataspace gauges (`sumeragi_rbc_dataspace_*`) — highlight on‑disk RBC congestion.
  - `mode_tag`, `staged_mode_tag`, `staged_mode_activation_height`, and `mode_activation_lag_blocks` (also exposed via `/v2/sumeragi/status`) surface staged flips; non-zero lag means the activation height passed but the runtime mode hasn’t flipped yet and should trigger an alert.
  - Live cutover guardrails: `mode_flip_kill_switch` mirrors `sumeragi.mode_flip_enabled`; `mode_flip_blocked` plus the `{success,failure,blocked}_total` counters, `last_mode_flip_timestamp_ms`, and `last_mode_flip_error` show whether the node applied or rejected the staged flip. When the kill switch is false the node keeps exporting the staged mode but refuses to flip until the switch is restored.
  - Runtime flips are deferred while a commit is in flight or a pending block is actively processing; `mode_flip_blocked` records `pending_processing`/`commit_inflight` to explain the lag until the pipeline drains.
  - Runtime flips flush mode-sensitive caches (pending blocks/RBC sessions/certificate and proposal caches) and reset pacemaker/view-change trackers to the base pacemaker interval at flip time so post-flip timers/leaders are recomputed deterministically.
  - `sumeragi_rbc_deliver_defer_{ready,chunks}_total` and the `/v2/sumeragi/status.pending_rbc` + `rbc_backlog` snapshots expose READY quorum vs chunk-gate stalls, stash totals by reason (`pending_rbc.stash_*`), stash age/drops, and pending session counts so dashboards can alarm on stuck RBC sessions before DA/commit stalls.
  - `sumeragi_rbc_mismatch_total{peer,kind}` and `/v2/sumeragi/status.rbc_mismatch` track per-peer chunk digest/payload hash/chunk root mismatches; mismatched payloads are dropped (no penalties) and logs are throttled per peer/kind to keep behavior deterministic.
  - `sumeragi_consensus_message_handling_total{kind,outcome,reason}` and `/v2/sumeragi/status.consensus_message_handling` capture drop/deferral reasons for BlockCreated, BlockSyncUpdate, ProposalHint, Proposal, QcVote, Qc, VrfCommit, VrfReveal, ExecWitness, RbcInit, RbcChunk, RbcReady, RbcDeliver, FetchPendingBlock, and Evidence to triage missing-payload warnings and backpressure drops.
- Block-sync roster telemetry: `/v2/sumeragi/status.block_sync_roster` (Norito) and `/v2/sumeragi/status.block_sync.roster` (JSON) expose drop counters (`drop_missing_total`, `drop_unsolicited_share_blocks_total`) plus source gauges (`commit_roster_journal`, `roster_sidecar`, paired `commit+checkpoint` hints, single cert/checkpoint history). Prometheus mirrors the same labels via `sumeragi_block_sync_roster_source_total{source}` and `sumeragi_block_sync_roster_drop_total{reason}`, and counts dropped unsolicited ShareBlocks via `sumeragi_block_sync_share_blocks_unsolicited_total`. Roster selection orders persisted snapshots (journal → sidecar) ahead of hints/history, and rejects `BlockSyncUpdate` payloads unless they carry a certified roster (commit certificate and/or validator checkpoint). Validator checkpoints embed the view index plus parent/post state roots in the signed preimage, so they validate without requiring commit-QC history. In NPoS, commit-certificate validation prefers a matching stake snapshot; when one is missing it derives a deterministic snapshot from local stake records (falling back to the minimum self-bond for missing peers) and only drops hints on roster or snapshot mismatch. Cached precommit signer records carry the stake snapshot so locally derived block-sync QCs still satisfy stake quorum. Block-sync share responses now propagate roster metadata so fresh peers can verify updates without waiting on local snapshots. Missing-block payload hydration uses `BlockCreated` replies instead of block-sync updates, removing the need for uncertified roster sources. Config parsing rejects incomplete PoP maps; if a mismatch still slips through, PoP filtering is skipped and the BLS baseline roster is used to avoid divergent topologies.
- View-change causes: `/v2/sumeragi/status.view_change_causes` reports per-cause counters (commit failure/quorum timeout/stake quorum timeout/censorship evidence/missing payload/missing commit certificate (`missing_qc_total`)/validation reject; DA availability is reserved for compatibility) and the last labeled timestamp to help operators triage view changes; Prometheus mirrors these timestamps in `sumeragi_view_change_cause_last_timestamp_ms{cause}`.
- Pending-block replay: after a view change installs, the leader rebroadcasts the highest pending `BlockCreated` payload on a cadence derived from `block_time` (small floor, 2x base multiplier for >=1s block times, 1x for sub-1s, plus the 2x payload multiplier) so peers missing the payload hydrate without waiting for hints/sidecars.
- View-change pruning: once a higher view is installed for a height, stale pending blocks/RBC sessions/missing-block requests for lower views of that height are dropped to prevent rebroadcast storms; missing payloads are re-requested only for the active view.
- Validation gate rejects: `/v2/sumeragi/status.validation_rejects` surfaces totals by reason plus the last rejected block hash/height/view/reason/timestamp. Prometheus mirrors the rejects via `sumeragi_validation_reject_total{reason}` with gauges `sumeragi_validation_reject_last_reason`, `_last_height`, `_last_view`, and `_last_timestamp_ms` (all zero when unset) so alerts can distinguish stateless vs execution vs prev-hash/height/topology failures before voting.
- Evidence helpers:
  - `/v2/sumeragi/evidence`, `/v2/sumeragi/evidence/count`, and `/v2/sumeragi/evidence/submit` carry the same DTOs surfaced in the JS/Python SDKs, so the CLI and automation can inspect or upload observer/fault evidence without recomputing payload formats.
  - View‑change telemetry is mirrored via `view_change_proof_{accepted,stale,rejected}_total`; `/v2/sumeragi/status` exposes the same counters for ad‑hoc audits.

**Quick reference**

| Surface | Signal | Expectation | Operator action |
|---------|--------|-------------|-----------------|
| CLI `iroha --output-format text ops sumeragi params` | `consensus_mode`, `block_time_ms`, `commit_time_ms`, `min_finality_ms`, `pacing_factor_bps`, `k_aggregators`, `redundant_send_r`, VRF windows | Match governance manifest; only change via tracked config updates | Re-deploy config or regenerate genesis when values drift |
| CLI `iroha --output-format text ops sumeragi collectors` / HTTP `/v2/sumeragi/collectors` | Collector indices, epoch/view tuple, `k_aggregators` slice | Current collectors rotate deterministically; no gaps or duplicate indices | If a peer is missing or duplicated, re-run the staking snapshot sync, verify `epoch_length_blocks`, and restart collectors after confirming randomness evidence |
| CLI `iroha --output-format text ops sumeragi vrf-epoch`, `iroha --output-format text ops sumeragi vrf-penalties` | `finalized=true`, participant count equals roster size, penalty lists reflect drills only | Every epoch emits a seed and penalty set that matches the staking roster/governance intent | When counts drift or unexpected penalties appear, follow {doc}`sumeragi_randomness_evidence_runbook` to resubmit commits/reveals and capture artefacts for governance |
| Prometheus | `sumeragi_phase_latency_ms{phase="commit"}` and phase-specific histograms | P95 < `0.8 * sumeragi.advanced.npos.timeouts.<phase>_ms` | Trigger chaos harness + perf runbook if exceeded; investigate collector fan-out |
| Prometheus | `sumeragi_da_gate_block_total{reason="missing_local_data"}`, `sumeragi_rbc_store_evictions_total`, `sumeragi_rbc_persist_drops_total` | Flat line in steady state; spikes indicate missing local payloads or RBC churn | Verify `BlockCreated` delivery/RBC backlog, then capture artefacts |
| Prometheus | `sumeragi_vrf_no_participation_total`, `sumeragi_vrf_reveals_late_total` | 0 outside planned drills | Follow {doc}`sumeragi_randomness_evidence_runbook`; page affected validators |
| HTTP `/v2/sumeragi/evidence/count` and CLI `iroha ops sumeragi evidence count` | Monotonic growth when faults occur; zero drift between peers | If counts diverge, re-run evidence submit/list checks and inspect Alertmanager snapshots |

### Troubleshooting checklist

- **Epoch/VRF drift:** If collectors or leaders look stuck, compare the operator intent with `sumeragi_epoch_length_blocks` and `sumeragi_prf_epoch_seed_hex`. A zero epoch length indicates the node never switched into NPoS mode.
  1. Capture `iroha --output-format text ops sumeragi params` plus `/v2/status`.
  2. Diff the values against the governance manifest and the most recent `mode_check.json`.
  3. Restart the peer with the corrected config and re-run the validation flow above.
- **RBC or DA bottlenecks:** Spikes in `sumeragi_da_gate_block_total{reason="missing_local_data"}` or `status.rbc_store.pressure_level > 0` mean payload recovery is lagging (consensus continues, but DA payloads are missing locally). Confirm `sumeragi.collectors.redundant_send_r` and the DA timeouts are sized for the hardware, then inspect `/v2/sumeragi/rbc/sessions` for the offending chunk.
  1. Use `iroha --output-format text ops sumeragi rbc status` to capture backlog depth.
  2. Fetch `/v2/sumeragi/rbc/sessions` to identify the stuck height/hash pair.
  3. Increase redundancy (`sumeragi.collectors.redundant_send_r`) temporarily, restart impacted collectors, and attach the run’s `summary.json` artefact to the incident report.
- **View-change storms:** Rising `view_change_proof_rejected_total` or `gossip_fallback_total` usually trace back to mismatched VRF windows or collectors. Cross-check the VRF commit/reveal windows and `epoch_commit_deadline_offset`/`epoch_reveal_deadline_offset`.
  1. Poll `iroha --output-format text ops sumeragi vrf-epoch --epoch <current>` to verify the recorded windows.
  2. Confirm `/v2/sumeragi/collectors` matches the expected roster, then run the VRF portion of the randomness runbook.
- **Roster activation delays:** When slashing or onboarding validators, `view_change_proof_accepted_total` should continue increasing. If not, verify `sumeragi.npos.reconfig.activation_lag_blocks`, confirm the governance evidence is visible via `/v2/sumeragi/evidence`, and account for the `sumeragi.npos.reconfig.slashing_delay_blocks` window.
  1. Use `iroha --output-format text ops sumeragi evidence list` to confirm the slashing payload propagated.
  2. Inspect `/v2/sumeragi/status.mode_activation_lag_blocks` to ensure the window has elapsed.
  3. Confirm `slashing_delay_blocks` in `iroha --output-format text ops sumeragi params` and whether governance cancelled the penalty before the delay expires.
  4. Only re-run admission once both checks succeed; otherwise escalate governance/persistence issues.
- **Evidence ingestion stalls:** Diverging `iroha ops sumeragi evidence count` outputs or a flat `sumeragi_evidence_records_total` time series indicates the HTTP ingest path is failing.
  1. Compare `evidence count` across at least two validators.
  2. Tail the Torii logs for `/v2/sumeragi/evidence/submit` errors and re-submit a known fixture.
  3. If the retry also fails, capture the `count` output and Alertmanager silence IDs, then roll back to the last known-good build.

Node Roles (config)
- `validator` (default): participates in consensus according to its current topology role.
- `observer`: excluded from the consensus topology (role becomes `Undefined`); does not propose, vote, or collect even if its position would otherwise be a collector. It fully syncs via block gossip and can commit using received commit certificates.
  - Observers listed in `trusted_peers` remain in the P2P dial set so they keep receiving gossip and block sync, but they never enter the consensus roster.
  - Note: Set B validators remain in the topology and are full validators; they are not the `observer` role.

Validator Key Requirements
- Validators must use BLS-Normal public keys and present a proof-of-possession (PoP). In NPoS, the consensus roster is filtered to the active public-lane validator set whenever stake records are available (including after commit-topology updates); if the commit topology omits active validators, the roster is widened with the active set before PoP filtering so the full validator set participates. When no active validators are present it falls back to `trusted_peers` after filtering out peers without BLS-Normal keys (the legacy `trusted_peers_bls` mapping no longer exists). `trusted_peers_pop` must cover the full validator roster and is validated at config parse time; incomplete or invalid PoP maps are rejected, and runtime roster derivation falls back to the BLS baseline only as a safety net. If PoP filtering would drop below quorum, the bootstrap roster falls back to the BLS baseline with a warning to avoid deadlocks. Non-BLS peers are always excluded; peers with missing/invalid PoP are excluded only when PoP filtering is enforced. The node’s `public_key`/`private_key` must be BLS-Normal and is used for both transport and consensus.
- Commit certificates carry a mandatory BLS aggregate signature over same-message signatures, along with a compact signer bitmap. Receivers verify the aggregate signature against the signer set and reject certificates with mismatched aggregates or out-of-range bitmap bits. Explicit votes remain the source of truth for quorum accounting and evidence, but aggregates are no longer advisory.
- Transaction admission uses `crypto.allowed_signing`/`allowed_curve_ids`; consensus vote signatures do not. Leaving `allowed_signing` at the Ed25519/secp defaults is fine for BLS validators—add `bls_normal` only if you intend to accept BLS-signed transactions. The BLS feature must be compiled in (`--features bls`) and `signature_batch_max_bls` kept > 0 for validator nodes.

Message Flow (steady state)
- Leader: when a block is expected (transactions queued) and the deadline elapses, the leader sends `BlockCreated` to the commit topology (validators); when there is no queued work the pacemaker stays idle (no heartbeat proposals).
- Proposal assembly defers when consensus block-payload/RBC chunk queues are saturated, when relay backpressure is active, when unresolved RBC sessions exist for the active tip, when the highest QC block is missing locally (it first schedules a missing-block fetch using QC-history rosters if needed), or when pending blocks extend the tip and have not been quorum-rescheduled; if no proposal is observed, the liveness override uses `min(commit_quorum_timeout, 4 * propose_interval)` to allow proposals/view changes, and once a proposal is seen it waits the full commit quorum timeout before overriding backlog. Idle view-change fallbacks pause while a commit is inflight so local execution cannot be preempted by a new view.
- Validators: validate, emit Availability votes, and send `CommitVote` (block header signature) to the deterministic collector set; votes fall back to the full commit topology only when collectors are disabled or local-only. Prepare/NewView certificates remain for pacemaker/telemetry but do not gate commit. On local timeout in view 0, the node may fan out to additional collectors up to `r`, plus any configured parallel topology fanout.
- Collectors: collectors aggregate `CommitVote` signatures and gossip a `CommitCertificate` once quorum is reached; collector ordering is PRF-derived per height/view (leader excluded) with fallback to the full commit topology only when collectors are disabled or local-only. Collectors may still aggregate availability evidence and prepare/new-view certificates for view-change hints.
- Set B validators: vote/sign under the same rules as Set A; routing/collection still prioritizes Set A for throughput, but any quorum of validators is accepted.
- Receivers: verify `CommitCertificate`, update highest/locked QC tracking (`highest_qc`/`locked_qc` fields), and attempt commit whenever the certificate and block payload are present. If a certificate arrives before the payload, peers request the missing block via block sync (signer-first with topology fallback), retry on the rebroadcast cadence (block-time aligned), and force a view change after the missing-payload window (quorum timeout) so consensus does not stall indefinitely. Missing-payload dwell tracking is scoped to the current view, so when the view advances the dwell window resets and a still-missing payload can trigger another view change; locally retained payloads (including aborted pending payloads in DA mode) suppress redundant missing-block fetches to prevent view-change churn. Once a block commits, outstanding missing-block requests at or below that height are cleared as obsolete (including conflicting hashes) so stale retries do not accumulate. Missing-block fetches can also come from RBC reconstruction or missing-parent detection; consensus-priority requests override background windows, but background requests can still trigger a view change when a one-block gap persists.
- Missing-parent recovery: if a block arrives ahead of local height and its parent is unknown, peers request the parent immediately and sweep pending gaps; when the gap is exactly one height the request arms the view-change window, while larger gaps keep retrying without forcing a view change while the node catches up.
- Block-sync updates: peers gossip `BlockSyncUpdate` payloads to a capped fanout (`block_gossip_size`) after commits and for vote/backfill so stragglers and observers can sync without relying on full broadcast. Updates are only broadcast when they carry verifiable roster metadata (commit QC or validator checkpoint; in NPoS the roster evidence must include a matching stake snapshot or be derivable from local stake records); otherwise the sender falls back to `BlockCreated` payload broadcasts to avoid dropped updates. Validator checkpoints include the signed view index plus parent/post state roots so receivers can verify them without cached commit-QC roots. Receivers only apply block-sync payloads when they include commit evidence (commit quorum signatures, a validated commit QC/certificate, or a validator checkpoint), except for explicit permissioned missing-block requests: next-height requests can proceed with any valid signer, while farther-ahead requests still require commit-quorum signatures. To keep the main loop responsive, receivers process commit votes and missing-block bookkeeping immediately but defer the heavy roster/signature/block-validation path while commit or validation work is inflight, replaying the update once the pipeline is idle. Relay ingress enqueues updates with backpressure; when the block-payload queue is full `BlockSyncUpdate` waits for capacity instead of dropping, while other block messages remain best-effort and rely on rebroadcasts for retries. Attached availability evidence, prepare/new-view certificates, and commit certificates are only applied when their height and block hash match the payload; mismatches are ignored. In permissioned mode, commit QCs must also match the block header view before they can be treated as valid evidence. Block-sync share filtering only keeps commit QCs that pass full validation, so invalid QCs cannot bypass signature checks. `ShareBlocks` responses are accepted only after a `GetBlocksAfter` request; unsolicited batches are dropped before they hit Sumeragi. Requests or responses that claim a different `peer_id` than the transport sender are dropped at the relay.
- Commit-conflict handling (strict finality): if a commit QC arrives for a different hash at an already-committed height, the node validates the QC and rejects it as conflicting finality. The local committed block is never rewound or replaced; the conflicting QC is treated as invalid and recorded/gossiped as `InvalidQc` evidence (`commit_conflict_finality`) so operators/governance can slash/evict in a later epoch.
- Availability votes: peers gossip `AvailabilityVote` frames to a capped fanout (`block_gossip_size`) so availability evidence can converge without full broadcast when collector routing is sparse.
- View-change counters: `view_change_suggest_total` increments when the local node triggers a view change (timeouts, missing payloads, validation rejects). `view_change_install_total` increments when the node installs a higher view for a height after observing higher-view traffic or its own triggers; `view_change_index` reports the currently tracked view.

Commit rule (commit certificate)
- Each validator tracks `highest_qc`/`locked_qc` certificate references for view-change safety (highest may be Prepare or Commit), but commits are driven by `CommitCertificate`s rather than child certificates.
- Validators sign the proposed block header and send `CommitVote` to the deterministic collector set; votes fall back to the full commit topology only when collectors are disabled or local-only. Any collector that reaches quorum aggregates `2f+1` signatures (permissioned) or ≥2/3 total stake (NPoS) into a `CommitCertificate` and gossips it.
- A block finalises when a valid `CommitCertificate` for `(height,hash)` is available **and** the block payload is known locally. Peers that receive the certificate first request the payload via block sync and commit once it arrives.
- Commit QCs always bind `parent_state_root` and `post_state_root`; there is no separate execution QC gate. When `sumeragi.da.enabled=true`, availability evidence is tracked but does not gate commit.

Pacemaker (view changes)
- View‑0 voting follows the same rules for Set A and Set B validators. On local timeout in view 0, nodes suggest a view change (no widen‑before‑rotate). Timing is driven by on‑chain parameters: `SumeragiParameters` (`BlockTimeMs`/`CommitTimeMs`/`MinFinalityMs`/`PacingFactorBps`) set the base for both modes; the pacing factor scales the block/commit targets (basis points, >= 10_000), and NPoS derives per‑phase timeouts from the scaled block time with optional `sumeragi.advanced.npos.timeouts` overrides. Leader proposal is roughly at 1/3 and expected commit at 2/3 of the pipeline time.
- View-change proofs advance once `f+1` validators raise suspicion (commit failure or quorum timeout); a full commit quorum is not required for a view change.

Deterministic pacing governor (always enabled)
- Config keys live under `sumeragi.advanced.pacing_governor`.
- Each block evaluates the last `window_blocks` headers and computes view-change pressure
  (average view-change index delta per block, permille) plus commit spacing pressure
  (average inter-block spacing vs target block time, permille).
- If either pressure exceeds its threshold, increase `SumeragiParameters.pacing_factor_bps`
  by `step_up_bps`. If both pressures fall below their clear thresholds, decrease
  by `step_down_bps`.
- Clamp `pacing_factor_bps` to `[min_factor_bps, max_factor_bps]` (min >= 10_000). If a
  block explicitly sets `pacing_factor_bps`, the governor skips that height.
- Updates apply only at block boundaries and emit configuration events; the result is
  deterministic because it is derived solely from committed headers. Monitor
  `effective_pacing_factor_bps` via `/v2/sumeragi/status`.

K / r Parameters
- Config keys: `sumeragi.collectors.k: usize` (collectors per height; default 1 in config) and `sumeragi.collectors.redundant_send_r: u8` (redundant send fanout; tooling may seed on-chain defaults as `2f+1` from the validator count, and the config fallback is 3 (2f+1 for 4 validators) when parameters are absent).
- On-chain: K and r live in `SumeragiParameters` and are authoritative for collector planning and `ConsensusParams` adverts; config values seed the genesis defaults. When peers advertise different K/r, the node logs a mismatch but keeps the on-chain values.
- Fallbacks: if `k` yields no collectors, votes fall back to the full commit topology; `redundant_send_r` is clamped to at least the commit quorum size (subject to the available collectors).
- Determinism: primary/next collector order is a pure function of the canonical roster, PRF seed, and `(height, view)`; when the PRF seed is unavailable the fallback is the wraparound slice starting at `proxy_tail_index()` (skipping the leader) until at least commit quorum size, with no per-node randomness.

NPoS Tunables (`sumeragi.npos.*` + `sumeragi.advanced.npos.timeouts.*`)
- `advanced.npos.timeouts.{propose_ms, prevote_ms, precommit_ms, exec_ms, witness_ms, commit_ms, da_ms, aggregator_ms}` (optional): each value must be > 0 when set. Unset fields derive from the on‑chain effective block time (`SumeragiParameters.block_time_ms` scaled by `pacing_factor_bps`) using the 1s ratios `350/450/550/150/150/850/750/120`. The pacemaker currently consumes propose/collect_da/collect_prevote/collect_precommit/commit; `exec_ms`/`witness_ms` are reserved for future witness pacing and `collect_aggregator` remains observability-only.
- Collector fan-out is configured under `sumeragi.collectors.{k,redundant_send_r}` (shared with permissioned). Both must be > 0; invalid values are rejected during config parsing.
- `vrf.{commit_window_blocks, reveal_window_blocks}` (optional): length of the commit and reveal windows inside an epoch; both must be > 0 when set. Unset values derive from `epoch_length_blocks` using the 3600-block ratios `100/40`. `vrf.commit_deadline_offset_blocks` defaults to `commit_window_blocks` and `vrf.reveal_deadline_offset_blocks` defaults to `commit_window_blocks + reveal_window_blocks` when omitted.
- `election.{min_self_bond, max_nominator_concentration_pct, seat_band_pct, max_entity_correlation_pct}` with defaults `1000`, `25`, `5`, and `25`. Percentages are clamped to the 0–100 range, and `min_self_bond` must be > 0.
- Candidates failing these staking constraints are excluded from the election; the entity correlation cap limits how many winners can share the same validator account.
- `election.finality_margin_blocks` delays activation of the newly elected roster until the chain has advanced by the configured number of blocks after the election snapshot, preventing premature swaps before finality.
- `reconfig.{evidence_horizon_blocks, activation_lag_blocks, slashing_delay_blocks}` default to `7200`, `1`, and `259200`. All three knobs must be > 0 to prevent unusable governance/evidence windows.
- Joint-consensus staging guard: proposals must stage `next_mode` **and** `mode_activation_height` together; manifests and governance packets missing either field are rejected, block application fails if a block sets only one of them, and handshake fingerprints derive from the effective runtime mode (configured mode until the activation height, staged mode after crossing it). Capture both values in cutover runbooks so peers agree on the pre‑activation fingerprint and the post‑activation mode tag. The runtime error surfaces as: `mode_activation_height requires next_mode to be set in the same block`.
- Configuration parsing now validates all tunables and produces a clear `Invalid Sumeragi consensus configuration` error if any constraint is violated (e.g., zero timeout or percentages above 100). Nodes will refuse to start with invalid NPoS settings rather than silently falling back.

Genesis manifests now seed `Sumeragi::NextMode` and the `sumeragi_npos_parameters` custom payload in the `parameters` block; normalization/signing injects the corresponding `SetParameter` instructions so the WSV persists, and Sumeragi loads, the live values during startup. Regenerate manifests via `kagami genesis generate` (add `--consensus-mode npos` for NPoS networks) to pick up the updated parameters and fingerprint.

### Kagami NPoS devnets (local + Docker)

- Sora profile localnets use NPoS for the global merge ledger and disallow staged cutovers; run permissioned Iroha3 localnets without `--sora-profile`.
- Bare-metal (Iroha3 NPoS): `kagami localnet --peers 4 --out-dir ./npos-local --consensus-mode npos --seed demo` writes genesis/configs/start scripts with BLS keys/PoPs and a stable NPoS fingerprint; start with `./npos-local/start.sh`.
- Sora Nexus localnet: `kagami localnet --sora-profile nexus --peers 4 --out-dir ./sora-nexus-local` generates a 4-node NPoS localnet and starts `irohad --sora` for Nexus dataspaces.
- Sora dataspace localnet: `kagami localnet --sora-profile dataspace --peers 4 --out-dir ./sora-ds-local` keeps Sora multi-lane defaults under NPoS; use `--consensus-mode permissioned` only without `--sora-profile`.
- Bare-metal (Iroha2 staged cutover): `kagami localnet --build-line iroha2 --peers 4 --out-dir ./npos-local --consensus-mode permissioned --next-consensus-mode npos --mode-activation-height 5 --seed demo` stages a permissioned→NPoS cutover at height 5 and keeps the advertised fingerprint on the permissioned mode until activation.
- Localnet defaults to a fast 1s pipeline (block/commit split), shortens transaction gossip cadence (`transaction_gossip_period_ms = 100`, `transaction_gossip_resend_ticks = 1`, target reshuffle cadence = 100ms), raises `sumeragi.advanced.rbc.chunk_max_bytes` to 256 KiB, lifts queue capacity to 262,144, sets `nexus.fusion.exit_teu = 1,000,000` and `sumeragi.block.proposal_queue_scan_multiplier = 4` to bound proposal assembly, relaxes Torii tx rate limiting (`torii.tx_rate_per_authority_per_sec = 1,000,000`, `torii.tx_burst_per_authority = 2,000,000`, `torii.api_high_load_tx_threshold = 262,144`), disables Kura fsync (`kura.fsync_mode = "off"`), clamps the pacing governor to 1.0x so effective block/commit timing stays aligned with the pipeline, and bumps redundant-send fanout when DA is enabled; for NPoS localnets it seeds XOR stake, activates validators in genesis, and rewrites `SumeragiParameters` timing fields (`block_time_ms`, `commit_time_ms`) to match the selected pipeline. Override with `--block-time-ms`, `--commit-time-ms`, or `--redundant-send-r` if you need slower timings. When only one of the block/commit values is set, Kagami mirrors it to the other to keep the pipeline balanced; set both to decouple them.
- Localnet soak (permissioned, thousands of blocks/tx): `cargo test -p integration_tests --test sumeragi_localnet_smoke permissioned_localnet_soak_thousands -- --ignored --nocapture` (long-running).
- Docker Compose: point `--config-dir` at the same localnet output and run `kagami swarm --peers 4 --config-dir ./npos-local --image hyperledger/iroha:dev --out-file docker-compose.npos.yml --consensus-mode npos --no-banner --print` to emit a Compose file that re-signs genesis in-container with `GENESIS_CONSENSUS_MODE` overrides (add the `GENESIS_NEXT_CONSENSUS_MODE`/`GENESIS_MODE_ACTIVATION_HEIGHT` pair only on Iroha2 staged networks).
- Rosters/PoPs: re-sign custom topologies with `kagami genesis sign --topology '<peers_json>' --peer-pop <public_key=pop_hex>...` (swarm’s inline signer accepts the same flags). Reuse the same `--seed` when regenerating localnet output so BLS/Ed25519 keys and PoPs stay deterministic for VRF sampling.
- Genesis signing (NPoS): when no public-lane validators are present in the manifest, `kagami genesis sign` injects a bootstrap transaction that registers the default `nexus`/`ivm` domains, mints `xor#nexus`, and stakes/activates each topology peer in the public lane.
- Guardrails: `--mode-activation-height` requires `--next-consensus-mode` (height > 0) and `--consensus-mode` continues to advertise the pre‑activation mode for fingerprints; omit both flags to stay in the configured mode, or pair them to stage a permissioned→NPoS cutover on Iroha2 only.

Deterministic Collector Selection (helpers)
- `collector_indices_k(k) -> Vec<usize>`: contiguous indices from `proxy_tail_index()` up to `k` (no wraparound).
- `collector_indices_k_fallback(k) -> Vec<usize>`: wraparound indices from `proxy_tail_index()` (skipping the leader) until at least commit quorum size, bounded by non-leader peers.
- `collectors_k(k) -> Vec<&PeerId>`: peer IDs for those indices.
- `is_collector(peer, k) -> bool`: membership test for collector duty at a given height.

Commit Certificate Content and Rotation Hint
- Commit certificates include the subject block hash, height/view/epoch, and a signer bitmap plus BLS-normal aggregate. For genesis NEW_VIEW frames, the highest certificate is a deterministic stub (zero bitmap, empty aggregate) accepted only when the hash matches the local genesis block. Receivers rotate their local topology to the certificate’s view before aggregate verification for deterministic checks.

Observer Fast‑Path (pending certificates)
- Observers retain commit certificates (phase `Commit`) keyed by block hash; once the matching block body arrives via gossip/RBC they validate and wait for a `CommitCertificate` before finalizing.

Actor Model
- The Sumeragi main loop owns explicit subcomponents (commit, propose/pacemaker, DA/RBC, VRF, merge/lane relay) with isolated mutable state and focused helpers, while shared consensus state (pending blocks, certificate caches, locks) stays in the core actor.

Backpressure & Telemetry
- Scheduler: with `sumeragi.advanced.worker.parallel_ingress = true`, per-queue worker threads feed a ticketed gate (one actor at a time, FIFO by ticket) instead of the single-thread priority mailbox; the priority mailbox (Votes → RBC chunks → blocks → block payloads → consensus control → lane relay → background) with starvation guards (`non_vote_starve_max`, `block_rx_starve_max`) is used only when `parallel_ingress = false`.
- Pre-tick drain is bounded by the next tick deadline (with a tick-min-gap grace when the deadline is immediate) so pacemaker ticks are not delayed by long queue backlogs; remaining messages drain after the tick within the iteration budget.
- Commit pipeline work runs on the tick path; inbound handlers only request a wakeup so heavy commit processing does not block mailbox drain. The tick path still runs on the max-gap cadence so commits continue without starving message drain.
- Pre-vote block validation is offloaded to background workers; the main loop schedules validation jobs and applies results on tick to keep drain latency bounded under backlog.
- Proposal, `BlockCreated`, `RbcInit`, and `RbcChunk` broadcasts bypass the background post queue and send inline so bulk RBC traffic cannot delay proposal delivery or chunk dissemination.
- Drops: relay ingress uses non-blocking enqueues for block/control messages, so full queues drop and increment `sumeragi_dropped_block_messages_total`/`sumeragi_dropped_control_messages_total`; `BlockSyncUpdate` plus proposal payloads (`Proposal`) bypass the non-blocking path and block until space is available in the block-payload queue to preserve commit/QC evidence and payload delivery, while `BlockCreated`, `FetchPendingBlock`, and `RbcInit` block on the block queue to avoid missing-block stalls. RBC READY/DELIVER also bypass the non-blocking path but are queued with votes to avoid payload backlog; they still travel on the high-priority consensus topic in the relay, while RBC chunks use the dedicated `ConsensusChunk` topic with low-priority P2P routing and best-effort ingress (drops are allowed when the chunk queue is saturated so relays do not stall). Blocking backpressure remains for explicit `incoming_*` calls (internal producers). When relay drops spike (subscriber queues full, network post/broadcast drops, per-peer post overflows, or topic-cap violations), consensus rebroadcasts (votes/block sync/RBC payloads) and idle view-change triggers pause for one rebroadcast cooldown to avoid message storms. Unresolved RBC payloads also pause proposal assembly (subject to the pacemaker RBC backlog soft limits); if no proposal arrives, idle view changes and backpressure overrides are bounded by `min(commit_quorum_timeout, 4 * propose_interval)` so leader loss does not stall liveness. Consensus ingress guardrails still apply: standard caps cover regular consensus/control traffic, critical caps cover recovery/coordination and payload delivery (`BlockCreated`, `FetchPendingBlock`, `ProposalHint`, `Proposal`, `QcVote`, `Qc`, `VrfCommit`, `VrfReveal`, `RbcInit`, `RbcReady`, `RbcDeliver`, plus control-flow evidence) and skip penalty cooldowns, and bulk caps cover payload frames (`BlockSyncUpdate`, `ExecWitness`, `RbcChunk`, `BlockSync`) scaled by block_time vs 2s while tracking RBC session limits. Payload-topic drops are counted in `consensus_ingress_drop_total{topic,reason}` with `topic` in {`ConsensusPayload`, `ConsensusChunk`, `BlockSync`}.
- Dedup caches: vote + block-payload/RBC chunk deduplication uses bounded LRU+TTL caches partitioned by message kind; `/v2/sumeragi/status.dedup_evictions` exposes capacity vs TTL evictions for votes, proposals, block-created, block-sync updates, RBC READY/DELIVER payloads, and RBC chunks.
- Collector metrics: gauges for `collectors_k`/`redundant_send_r`, counters for redundant sends (with per‑collector/per‑peer breakdowns), a gauge for collectors targeted in the current round, and a histogram for collectors targeted per committed block. Certificate size is tracked via `sumeragi_cert_size`.
- Transaction queue backpressure: `sumeragi_tx_queue_depth`/`sumeragi_tx_queue_capacity` gauge the mempool, while `sumeragi_tx_queue_saturated` flips to 1 when Torii reports saturation. When saturated, validators skip redundant collector fan-out (behaving as `redundant_send_r = 1`) to shed load and expedite commit of queued transactions.
- Proposal assembly scans a bounded snapshot of the queue (derived from `max_in_block * sumeragi.block.proposal_queue_scan_multiplier`) so sustained admission traffic cannot stall block creation; newly arrived transactions remain queued for the next proposal.
- Fast-finality cap: when `effective_commit_time_ms <= 1000`, proposal assembly clamps the gas budget to `min(ivm_gas_limit_per_block, sumeragi.block.fast_gas_limit_per_block)` (optional) and selects transactions by gas to keep validation within a 1s finality budget.
- Background post saturation: `sumeragi_bg_post_overflow_total{kind="Post|Broadcast"}` increments when the bounded background-post queue is full; the sender does not block and can fall back to inline network posts. `sumeragi_bg_post_drop_total{kind}` and `/v2/sumeragi/status.bg_post_drop_{post,broadcast}_total` count drops when the queue is missing or disconnected. Sustained overflow suggests a slow background worker or network pressure; drops indicate lost network posts.
- For deterministic testing (e.g., CI or chaos harnesses), set `sumeragi.debug.disable_background_worker = true` to disable the worker and dispatch background posts inline (synchronous, no extra thread, no queue drops).
- READY gossip fallback: `gossip_fallback_total` still records the number of collector plans that exhausted the deterministic fan-out and had to fall back to gossip broadcasts. Correlate with the background metrics above: if both are rising, the node is likely CPU or network bound and will delay RBC convergence.
- See `docs/source/telemetry.md` and the README “Consensus metrics (Sumeragi)” section for metric names and example PromQL.

Consensus Parameter Advert (pinning)
- At startup and when collector plans refresh, nodes broadcast a compact `ConsensusParams` advert carrying `(collectors_k, redundant_send_r)` plus the current membership snapshot `{ height, view, epoch, view_hash }`. The epoch is derived from the advertised height using the finalized VRF epoch schedule so epoch-length changes do not invalidate historical messages and replays across epoch boundaries remain deterministic.
- Receivers verify the advert against their effective parameters and membership view hash. Mismatches increment the Prometheus counter `sumeragi_membership_mismatch_total{peer,height,view}` and mark the offending peer in `sumeragi_membership_mismatch_active{peer}` so operators can wire alerts directly. Any mismatch is logged and flagged locally; consensus semantics are unchanged. Effective values are taken from on‑chain `SumeragiParameters` when present (preferred over local config).

Configuration (example)
```
[sumeragi]
role = "validator"                 # or "observer" (sync-only)
consensus_mode = "permissioned"

[sumeragi.collectors]
k = 3                   # three collectors per height
redundant_send_r = 3    # example for 4 validators (2f+1)

[sumeragi.advanced.queues]
votes = 8192             # vote channel capacity (commit votes + RBC READY/DELIVER)
block_payload = 128      # block payload capacity (Proposal/BlockSyncUpdate)
rbc_chunks = 1024        # RBC chunk capacity
blocks = 256             # block message capacity (BlockCreated/FetchPendingBlock/RbcInit/etc.)
control = 1024           # control/background/lane channel capacity

[sumeragi.advanced.worker]
iteration_budget_cap_ms = 2000       # cap worker loop time budget per iteration
iteration_drain_budget_cap_ms = 2000 # cap mailbox drain per iteration
tick_work_budget_cap_ms = 500        # cap per-tick proposal/commit work (0 disables)
parallel_ingress = true              # run per-queue ingress threads with a ticketed gate
validation_worker_threads = 1        # pre-vote validation worker threads
validation_work_queue_cap = 4        # validation work queue per worker
validation_result_queue_cap = 4      # validation result queue
validation_pending_cap = 8192        # deferred vote-validation backlog cap

[sumeragi.advanced.pacemaker]
pending_stall_grace_ms = 250       # grace before pending progress counts as stalled
active_pending_soft_limit = 1      # allow proposals with <= N blocking pending blocks (0 = strict)
rbc_backlog_session_soft_limit = 8 # allow proposals with <= N backlog sessions (0 = strict)
rbc_backlog_chunk_soft_limit = 256  # allow proposals with <= N missing chunks (0 = strict)

[sumeragi.da]
enabled = true                  # enable DA + RBC; availability evidence tracked (commit does not wait)

[sumeragi.advanced.rbc]
chunk_fanout = null             # RBC chunk fanout cap (null = full roster minus local)

[sumeragi.persistence]
kura_retry_interval_ms = 1000   # retry failed kura persistence with exponential backoff
kura_retry_max_attempts = 5     # abort and requeue the block payload after repeated failures
commit_inflight_timeout_ms = 30000
commit_work_queue_cap = 1       # commit worker queue capacity
commit_result_queue_cap = 1     # commit worker result queue capacity

[sumeragi.recovery]
missing_block_signer_fallback_attempts = 1 # fetch from certificate signers this many times, then try full topology

[sumeragi.gating]
membership_mismatch_alert_threshold = 1   # consecutive mismatches before alert/fail-closed
membership_mismatch_fail_closed = false   # drop consensus messages from mismatched peers
```


Notes
- Consensus thresholds and signature validation are unchanged by K/r; only the set of eligible aggregators expands. With K=1, behavior matches the single‑collector path.
- All paths remain deterministic across hardware. BLS aggregation is over the same message and does not introduce non‑determinism; consensus accepts only certificates whose explicit signatures validate against the current topology.
- RBC READY/DELIVER quorum uses the commit topology’s `min_votes_for_commit()`.
- RBC INIT carries the session roster snapshot and its `roster_hash`; READY/DELIVER include the same `roster_hash` so signatures are bound to a single roster.
- RBC INIT also carries the per‑chunk SHA‑256 digest list; receivers validate each chunk against the digest at its index and drop mismatches without invalidating the session.
- Per‑peer RBC mismatch counters (digest/payload/root) surface in `status.rbc_mismatch` and `sumeragi_rbc_mismatch_total{peer,kind}`; mismatches are drop‑only (no penalties/quarantine) to preserve deterministic behavior, and mismatch logs are throttled per peer/kind.
- RBC INIT that conflicts with an existing session’s payload hash, chunk count, digest list, or chunk root is dropped to avoid poisoning an in‑flight session.
- RBC sessions cache the roster from INIT as unverified until a derived commit-topology roster is available; derived rosters are authoritative for READY/DELIVER validation and local signing, and INIT rosters that conflict with a derived roster are rejected. In permissioned mode, if no derived roster can be computed yet (far-ahead heights), INIT rosters are treated as a provisional authority for READY/DELIVER so availability does not stall; derived rosters still supersede INIT snapshots once available.
- READY/DELIVER received before an authoritative roster is available are stashed, trigger missing `BlockCreated` fetches (falling back to the effective commit topology when needed), and replay once the derived roster is cached; mismatched `roster_hash` values are dropped once an authoritative roster is cached, while derived roster mismatches trigger a fresh derived-roster lookup and are re-stashed until INIT arrives if the roster still mismatches, resetting cached READY/DELIVER state. Local READY/DELIVER emission waits for an authoritative roster and a chunk root unless permissioned fallback to INIT is active; READY/INIT rebroadcasts reuse the cached roster only when the roster hash matches to avoid liveness stalls.
- When READY/DELIVER or chunks arrive before INIT, the node stashes them and immediately requests the missing `BlockCreated` payload (subject to the missing-block backoff). If the session roster is empty, the request falls back to the effective commit topology. Missing-block responses in DA mode include RBC INIT + chunks so the requester can hydrate through the chunk queue; responders with an RBC session will serve INIT/chunks even if the full block is not yet available.
- Empty payloads are represented as a single empty RBC chunk so sessions never advertise a zero chunk count.
- RBC chunk broadcasts pick a deterministic, per-session target set sized to at least the commit quorum; the default (`rbc_chunk_fanout = null`) targets the full roster minus local so READY quorums can form even with f faulty peers. Payload rebroadcasts send cached chunks to the full roster to recover missed fanout.
- RBC payload rebroadcasts and outbound chunk flushes pause while block-payload/RBC chunk queues are backpressured so the worker can drain before sending more chunks.
- When a block payload is known locally and the height remains in the current epoch, RBC may fall back to the active topology even if the height is ahead of the committed tip so READY quorum can form without waiting on roster snapshots.
- Idle view changes are suppressed while unresolved RBC backlog, relay backpressure, or consensus queue backpressure (block-payload/RBC chunk queues saturated) is active; the pacemaker resumes view-change timers once the backlog clears.
- Idle view-change timers reset when the transaction queue transitions from empty to non-empty so fresh work gets a full proposal window before leader rotation.
- RBC payload rebroadcasts always include INIT even when no chunks are cached once a roster snapshot is cached, so peers can learn the roster snapshot and request missing chunks; derived rosters may rebroadcast but remain guarded by roster-hash checks, the background rebroadcast loop does not synthesize roster snapshots on its own, cached chunks continue to rebroadcast while any chunks are still missing even after READY quorum is reached, and only a deterministic f+1 subset of the roster performs payload rebroadcasts per session to limit message storms.
- RBC READY rebroadcasts use the same deterministic f+1 sender subset (leader always included) to cap message volume; skipped rebroadcasts increment `sumeragi_rbc_rebroadcast_skipped_total{kind="payload|ready"}`.
- RBC payloads carry canonical block payload bytes only; if delivery completes before `BlockCreated` arrives, the node requests the signed block header/body from peers so validation and voting can proceed.
- RBC INIT/READY/DELIVER must carry the epoch derived from the advertised height; mismatched epochs are rejected to prevent cross-epoch availability drift.
- RBC READY must reference the expected chunk root for the session; mismatched roots are rejected so quorum only counts votes for the same payload. When the expected root is missing, READY/DELIVER seed it from the message (validated against any computed root) so restored sessions still enforce root consistency.
- RBC DELIVER carries the sender-observed READY signature bundle (sender index + signature). Receivers validate and record those signatures to recover READY quorum when READY messages were missed.
- Persisted RBC sessions include the authoritative roster snapshot so restarts validate READY/DELIVER against the original topology even if the live roster changes; derived fallback rosters are not persisted.
- Vote/certificate signature checks use the roster snapshot tied to the block when available, falling back to the live commit topology so roster changes do not invalidate late votes or certificates; when the parent hash is known, history-derived rosters must match the known chain or the vote is dropped.
- When the commit topology changes at block commit, the node clears pending consensus caches (pending blocks, vote logs, RBC sessions, DA bundles) so stale votes from the old roster cannot stall the pipeline.
- Vote/QC/deferred/roster caches are pruned to a bounded height/view window around the active round to prevent unbounded growth during view-change storms.
- In DA mode, view changes still prune stale pending blocks but retain RBC sessions and missing-block requests while the payload is unavailable so availability can finish after a view change.
- NEW_VIEW tracker entries are retained within the configured future-view window so late quorum receipts can still unblock proposal assembly after a timeout.
- Timing uses on‑chain `SumeragiParameters` (`BlockTimeMs`/`CommitTimeMs`/`MinFinalityMs`/`PacingFactorBps`) for both modes; the pacing factor scales the block/commit targets, and NPoS derives per‑phase timeouts from the scaled block time with optional `sumeragi.advanced.npos.timeouts` overrides. Both affect pacing but not semantics.
- DA-enabled runs derive the quorum timeout as `3 * (block_time + 4 * commit_time)` to leave headroom for RBC/availability-evidence propagation on slower hosts.
- Quorum timeouts are evaluated against the last observed progress for the pending block (RBC chunks/READY/DELIVER or votes); the fast-path view-change window is suppressed while pre-vote validation is inflight, and in DA mode the fast-path shortcut is disabled so timeouts always use the full DA commit window. If progress stalls, the timeout still fires to recover.
- Availability timeouts use `2 * max(quorum_timeout, 2s)` in DA mode to tolerate payload hydration before logging/rebroadcast; when `MissingLocalData` is recorded, quorum reschedules are deferred until the availability timeout so payload recovery can complete.
- Stale-view guards drop old-view consensus traffic; with DA enabled, RBC payload and BlockCreated traffic is still accepted when it matches an active pending/inflight block, and stale-view RBC INIT/READY/DELIVER for uncommitted heights are stashed even before INIT (including when the payload is only present via an aborted pending block) so availability can finish without perfectly synchronized views while stale floods are suppressed.
- Future-window guards drop consensus messages that arrive far ahead of the active round (height/view). View gating applies only to the active round and relaxes after the local commit-quorum timeout so lagging nodes can catch up; `BlockSyncUpdate` respects the same window unless the block is already tracked (pending/explicit missing-block request) or its parent payload is known locally, keeping catch-up traffic bounded without stalling sync.
- Repeated invalid Vote/READY/DELIVER signatures from the same signer trigger a temporary suppression window so verification storms do not saturate the worker.

## Sumeragi (commit-certificate pipeline)

Overview
- Prepare/Commit votes and certificates remain for pacemaker/telemetry and lock tracking; commits are driven by `CommitCertificate`s (proxy‑tail quorum) rather than child certificates. Historical BlockSigned/BlockCommitted frames have been removed.
- NEW_VIEW gating: the leader proposal is gated in views ≥ 1 by commit quorum (≥ `min_votes_for_commit()`, i.e., 2f+1) NEW_VIEW receipts for the tuple (height, view). View 0 remains optimistic‑propose. The actor tracks deduplicated NEW_VIEW counts and adopts the highest certificate (`highest_qc`) monotonically; on view change it sends NEW_VIEW votes to the view-aligned collector set and always includes the leader, falling back to the full commit topology only when collectors are disabled or local-only to keep quorum formation reliable without N^2 broadcast storms.
- NEW_VIEW freshness: highest certificate heights that lag the local locked certificate are still accepted and counted toward view-change quorum; the locked‑certificate rule is enforced when proposing or validating blocks, not when tallying view-change receipts.
- NEW_VIEW stale handling: stale NEW_VIEW frames do not advance the local view, but their highest certificate is still processed to seed missing-block fetch and cache late certificates.
- NEW_VIEW height sanity: frames are accepted when `height == highest_qc.height + 1` for commit or prepare highest QCs so view changes cannot skip heights.
- NEW_VIEW height floor: cached NEW_VIEW entries below the active round height (highest QC height + 1 or committed tip + 1) are discarded so proposals cannot regress after a higher commit certificate is observed.
- NEW_VIEW highest certificate epoch: votes and QCs are rejected when `highest_qc.epoch` does not match the height-derived epoch for `highest_qc.height`.
- NEW_VIEW highest certificate phase: outbound NEW_VIEW frames carry a commit certificate or a prepare certificate for the parent height (`height - 1`); when the available highest QC does not match the target height, peers fall back to the latest committed commit certificate (or the genesis stub) before gossiping.
- NEW_VIEW highest certificate view: when the referenced parent block is known locally, NEW_VIEW votes/QCs are rejected if `highest_qc.height`/`view` do not match the stored header; cached NEW_VIEW entries are reconciled to the local header view when the parent becomes available.
- NEW_VIEW roster selection: NEW_VIEW votes/QCs for the next height use the active commit topology (post-commit state) so roster changes are honored; gaps beyond the next height roll forward from commit-certificate history.
- NEW_VIEW roster history: when rolling forward from commit-certificate history, certificates that contradict a locally known block hash at the same height are ignored to avoid using a forked roster snapshot.
- NEW_VIEW highest certificate roster: highest certificate verification uses the roster snapshot for the referenced block when available so topology changes do not invalidate late highest-certificate payloads.
- Highest certificate: peers update `(height, view, hash)` on incoming NEW_VIEW and on commit-certificate receipts; this informs leader’s proposal header and pacemaker.

Messages (Norito‑encoded)
- Control: `ControlFlow::{NewView, Evidence}` are carried on the control topic and route through the consolidated control flow. `NEW_VIEW` frames include a signature over `(height, view, highest_qc, sender)` and the highest certificate itself; peers verify it against the commit topology and drop unauthenticated frames (the genesis highest certificate is the stub described above).
- Block: `CommitVote` (phase `Prepare`/`Commit`), `AvailabilityVote`, and the corresponding `CommitCertificate` (phase `Prepare`/`Commit`) plus availability evidence.

Commit rules (scaffold wiring)
- DA availability: when `SumeragiParameter::DaEnabled = true`, availability evidence is tracked locally via RBC `READY` quorum (>= `min_votes_for_commit()`) and availability vote aggregation. Commit/finalize does not wait for availability evidence, but still requires the local payload; RBC `DELIVER` or block sync can satisfy it.
- Availability votes: validators emit `AvailabilityVote` after proposal validation whenever DA is enabled and they have not already voted; vote emission does not wait for RBC delivery. This avoids circular waits between payload transport and voting. Collectors may aggregate availability evidence even when the payload is still missing locally; missing-block fetch runs in parallel so availability status updates once evidence is observed. Nodes continue to accept late availability votes for pending blocks even after a view change so DA quorums can still form, and in DA mode stale availability votes are recorded even if the payload has not been hydrated yet. If the collector target set is empty after filtering the local peer, the vote sender falls back to the commit topology to avoid a no-op broadcast.
- Commit votes: nodes accept late commit votes even after a view change so commit certificates can still form; in DA mode stale commit votes are recorded even if the payload is not yet known, and in DA-off runs the same holds when a missing-block fetch is in flight so the certificate can be reconstructed after payload arrival. The local validator gossips a block-sync update (fanout‑capped) after emitting its commit vote to propagate cached votes.
- Payload recovery: nodes that observe availability evidence (RBC `READY` quorum or availability votes) without the payload deterministically fetch it from the certificate signers for `sumeragi.recovery.missing_block_signer_fallback_attempts` attempts, then fall back to the full commit topology (and still fall back immediately when the signer set is empty). Payload hashes are verified before applying the block.
- Availability timeout on idle views (NPoS/DA-only): pending blocks with `MissingLocalData` log and rebroadcast availability evidence after the availability timeout even if no fresh traffic arrives; while `MissingLocalData` is set, quorum reschedules are deferred until the availability timeout, and `ManifestGuard` remains advisory (warnings only). Permissioned/DA-off paths skip DA availability tracking but still benefit from the prevote-only fallback above.
- Prepare-only fallback: if only a prepare certificate reaches quorum and no commit votes arrive by the quorum timeout, the actor requeues the block’s transactions, rebroadcasts the block + block-sync update + prepare certificate, resets a highest certificate reference that pointed at the stalled block, and triggers a view change so the next round can propose a fresh block without stalling (applies to both DA-off permissioned runs and DA-off NPoS smoke tests).
- Commit vote lock: once any commit vote is observed at a height, proposal assembly for that height is deferred until the committed block resolves, and quorum reschedules skip requeueing that block’s transactions to avoid conflicting proposals.
- Aborted pending retention: when a commit inflight timeout or quorum reschedule marks a pending block aborted, the payload is retained for a bounded window (derived from quorum timeout and missing-block fallback attempts) so late commit certificates and missing-block fetches can still finalize; entries are evicted once committed or when the retention window expires without votes or fetches.
- Timer-driven commit path: if any pending block exists, the pacemaker tick runs `process_commit_candidates` even when no new messages arrive, and message handlers request the tick when commit evidence lands. This keeps quorum timeouts, Kura retries, and DA availability tracking advancing on quiet networks (permissioned and NPoS). Operators can confirm this path via `status_snapshot().commit_pipeline_tick_total` and `sumeragi_commit_pipeline_tick_total{mode,outcome}` (`outcome="active"` when pending blocks existed, `outcome="idle"` otherwise).
- Tick cadence: when there is no pending consensus work and the transaction queue is empty, ticks are suppressed and the worker sleeps on the wake channel (with a periodic shutdown poll); scheduled tasks (missing‑block retries, quorum reschedule/aborted‑retention windows, RBC rebroadcast/TTL maintenance, idle view‑change timeouts, and commit‑inflight timeouts) drive a next‑deadline sleep instead of fixed‑interval polling; once work appears, ticks are throttled to a block/commit-derived minimum gap while preserving the maximum-gap liveness guarantees, and queued transactions force immediate tick scheduling even when pending blocks exist to keep commit/DA work moving.
- Worker wake: inbound consensus queues, transaction enqueue paths, and async commit/RBC-persist workers signal a wake channel so the worker blocks until new work or the next tick deadline instead of fixed-interval polling.
- Kura persistence retry/backoff: when `kura.store_block` returns an error the block stays pending and retries with exponential backoff derived from `sumeragi.persistence.kura_retry_interval_ms`. After `sumeragi.persistence.kura_retry_max_attempts` the actor aborts the payload and requeues its transactions so new proposals can make progress. Status counters (`status_snapshot().kura_store.*`) and telemetry (`sumeragi_kura_store_failures_total{outcome="retry|abort"}`) surface failures for alerting, and WSV remains untouched until persistence succeeds.
  - Gauges `sumeragi_kura_store_last_retry_attempt` and `sumeragi_kura_store_last_retry_backoff_ms` export the most recent retry decision so alerting can page on stuck backoffs or exhausted budgets.
  - `/v2/sumeragi/status.kura_store` mirrors the same snapshot (failures/abort totals plus the last failed height/view/hash) in both JSON and Norito responses so operators can audit persistence stalls without shell access.
  - Staging is atomic: pipeline events and WSV deltas are buffered until `kura.store_block` succeeds. The status surface reports `stage_total`/`stage_last_*` for staged blocks, `rollback_total`/`rollback_last_*` with the last reason (`store_failure` or `state_commit_failure`) when a stage is dropped before WSV apply, and `lock_reset_total`/`lock_reset_last_*` when highest/locked certificates are reset to the latest committed tip after a kura abort.
- Availability status is data driven: every availability-evidence update (RBC `READY` quorum or availability votes) or `NEW_VIEW` update re-evaluates tracking. Blocks finalize based on `CommitCertificate`s plus payload validation; when DA is enabled, availability evidence is tracked alongside the commit pipeline (see `process_commit_candidates` in `main_loop.rs`).
- RBC worker recovery: background fan-out uses a bounded channel. If the queue is full the sender blocks and `sumeragi_bg_post_overflow_total{kind}` increments; if the queue is missing or disconnected the message is dropped and `sumeragi_bg_post_drop_total{kind}` increments. RBC sessions themselves are persisted and marked `recovered: true` on restart; the integration scenarios `sumeragi_rbc_recovers_after_peer_restart` and `sumeragi_rbc_session_recovers_after_cold_restart` assert this behaviour end-to-end.
- State roots: commit votes bind `parent_state_root` and `post_state_root` derived from execution witness snapshots. Commit uses the commit certificate plus payload; there is no separate execution QC gate or WSV requirement.
- Commit certificate safety: validators only commit one block per height/epoch (re-votes across views must target the same block). Collectors skip commit-certificate aggregation for blocks that do not extend the locked chain, and receivers drop conflicting commit certificates (emitting invalid-QC evidence) instead of caching them for finalize.

Commit certificate verification (basic)
- Receivers perform bitmap/topology shape checks on the new certificates: signer bitmaps must not reference indices ≥ N and must have at least `min_votes_for_commit()` bits set. The BLS aggregate signature is verified against the signer set; certificates with mismatched aggregates are rejected.

Determinism
- All additional checks are pure functions of local state and message contents. The pipeline preserves deterministic commit semantics while enabling the pacemaker path and certificates.

### Proposal header validation

- Validators cache full proposals (header + payload hash) and may cache proposal hints (height/view/highest certificate metadata). When hints are missing, the cached proposal is used to derive the highest-QC context for `BlockCreated`. On `BlockCreated` ingress they enforce:
  - proposals are dropped if their embedded `highest_qc` does not target `height - 1`, does not match the height-derived epoch, does not match the proposal `parent_hash`, or (when the parent block is known) does not match the parent block view;
  - hint metadata must match the `BlockCreated` header `(height, view, parent)`; when the parent block is known, the hint `highest_qc.view` must match the parent view;
  - `BlockCreated` without cached hints are rejected when locally known ancestry proves they do not extend the locked chain; if ancestry is missing, the payload is kept while parents are fetched;
  - if the locked block itself is missing, the validator retains the lock, requests the payload, and defers accepting non-extending certificates until the lock resolves;
  - the cached proposal header’s `parent_hash` and `tx_root` must equal the block header;
  - the cached proposal payload hash must equal the recomputed canonical payload hash
    (header + transactions + DA bundles, without signatures or execution results).
- Header/payload mismatches immediately drop the block. Proposal mismatches emit `Evidence::InvalidProposal` (broadcast on the control topic) so collectors can quarantine faulty leaders. Hint-only mismatches are dropped without evidence because the full proposal may still arrive later in the view.
- When the proposal has not arrived yet, the node logs a trace entry and accepts the block after standard validation; the sanity checks re-run if the proposal subsequently appears.

### Validation gate telemetry

- The pre-vote validation gate increments `sumeragi_validation_reject_total{reason}` and `/v2/sumeragi/status` surfaces `validation_reject_total`, `validation_reject_reason`, and `validation_rejects.*` (per-reason counters plus last height/view/block/timestamp) whenever validation fails before sending votes. Reasons are bucketed as `stateless`, `execution`, `prev_hash`, `prev_height`, or `topology`; view-change cause telemetry includes a `validation_reject` bucket to show how often rejects force a view bump.
- Blocks rejected by the gate requeue their transactions and emit invalid-proposal evidence when a parent certificate is available so view-change recovery can proceed without advancing highest/locked certificates; highest/locked certificates realign to the last committed chain if they pointed at the rejected block.

### Large payload simulations

- Integration helpers `sumeragi_rbc_da_large_payload_four_peers` and `_six_peers`
  (see `docs/source/sumeragi_da.md`) exercise ≥10 MiB payloads with
  `sumeragi.da.enabled = true` and confirm that availability evidence forms (or an RBC
  `READY` quorum is recorded) and commit progresses without deadlocking. RBC
  delivery typically completes before commit, but commit is not gated on
  availability evidence or local `DELIVER`.
- DA availability timeout: while availability evidence is still missing
  (RBC `READY` quorum not met), the actor logs and rebroadcasts
  availability evidence after the timeout. The reschedule counters
  (`sumeragi_rbc_da_reschedule_total` and `status_snapshot().da_reschedule_total`)
  are legacy and should remain zero now that DA is advisory. Nodes missing payload
  fetch it from certificate signers first, then fall back to the full commit topology after
  the configured retry budget.
- The helpers capture per-peer Prometheus counters and `/v2/sumeragi/rbc/sessions`
  snapshots; automation can watch their `sumeragi_da_summary::*` output.
- Performance budgets are enforced by the helpers: RBC delivery ≤ 3.6 s, commit
  ≤ 4.0 s, throughput ≥ 2.7 MiB/s, background-post queue depth ≤ 32, and P2P queue
  drops = 0. Violations fail the tests and should alert operators in production
  runs.
- Use `cargo run -p build-support --bin sumeragi_da_report` against the latest
  `.summary.json` artifacts to capture measured numbers. The generated Markdown
  now includes `BG queue max` and `P2P drops max` columns mirroring the budgets.
- The generated summary under ``docs/source/generated/sumeragi_da_report.md`` is
  updated by `scripts/run_sumeragi_da.py --report-dest …` and is included here
  to surface the most recent measurements.
- Baseline (1 s block, k=3) metrics rely on a discrete harness run; see
  `docs/source/generated/sumeragi_baseline_report.md` for the latest report.
  The current measurements were captured on an Apple M2 Ultra (24 cores, 192 GB
  RAM, macOS 15.0). Reproduce with:

  ```bash
  SUMERAGI_BASELINE_ARTIFACT_DIR=artifacts/sumeragi-baseline-live \
    python3 scripts/run_sumeragi_baseline.py \
    --fail-on-fixture \
    --report-dest docs/source/generated/sumeragi_baseline_report.md
  ```

  The harness writes Norito summary JSON alongside topology snapshots so the
  Markdown report and fixtures remain in sync.
- For chaos/performance validation, run the targeted stress scenarios via
  `scripts/run_sumeragi_stress.py`. The helper executes the
  `sumeragi_npos_performance.rs` fault-injection tests individually (queue
  backpressure, RBC overflow, redundant fan-out, jitter, chunk loss) and saves
  per-test logs plus a `summary.json` manifest for later analysis. Example:

  ```bash
  python3 scripts/run_sumeragi_stress.py \
    --artifacts artifacts/sumeragi-stress-$(date +%Y%m%d-%H%M)
  ```

  See `docs/source/sumeragi_chaos_performance_runbook.md` for the complete
  Milestone A6 checklist covering baseline captures, soak-matrix runs, telemetry
  validation, and evidence packaging.

For the multi-peer soak matrix (4/6/8 peers) and the corresponding operator
sign-off bundle, run:

```bash
python3 scripts/run_sumeragi_soak_matrix.py \
  --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
  --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
```
See [`sumeragi_soak_matrix.md`](sumeragi_soak_matrix.md) for the default matrix
and sign-off checklist.

Run the suite on dedicated hardware; scenarios intentionally exercise
backpressure saturation and may take several minutes. Inspect the emitted log
files when a test fails and cross-reference the metrics with the VRF and DA
dashboards above. The helper appends results to `summary.json` inside the
artifacts directory so operators can confirm each scenario succeeded without
re-running the full suite; each entry records the exit status plus stdout/
stderr log paths for postmortems. Use `scripts/render_sumeragi_stress_report.py --output-format text <path>` to turn the manifest into a Markdown table with clickable log
links for runbooks or incident retrospectives.

### Collector telemetry runbook

Stalled collectors or slow commit assembly manifest as missing
availability-evidence records, prolonged DA collection, or `commit_ms` /
`pipeline_total_ms` spikes. Operators should wire the following telemetry to
detect and triage issues before commits approach the configured commit-time
budget (permissioned `CommitTimeMs` or NPoS `effective_npos_timeouts.commit_ms`).

**Key dashboards**
- `sum(rate(sumeragi_da_votes_ingested_by_collector[1m])) by (collector_idx)` —
  identify collectors that are not ingesting votes.
- `sumeragi_qc_last_latency_ms{kind="availability"}` and the histogram
  `sumeragi_qc_assembly_latency_ms{kind="availability"}` — last observed and
  recent latency for building availability evidence.
- `sumeragi_phase_latency_ms{phase="collect_da"}` and
  `sumeragi_phase_latency_ms{phase="collect_precommit"}` (P95 over 5 minutes)
  — view-level time spent collecting availability votes and precommit QCs.
- `sumeragi_phase_latency_ms{phase="collect_aggregator"}` — redundant collector
  fan-out latency; correlate spikes with collector backlog and adjust
  `sumeragi.collectors.redundant_send_r`/timeouts as needed. Track fallback frequency with
  `sumeragi_gossip_fallback_total` and monitor rejected proposals via
  `block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`,
  `block_created_proposal_mismatch_total`, and `pacemaker_backpressure_deferrals_total`
  (the latter highlights queues saturated enough to stall proposal assembly).
- `sumeragi_phase_latency_ema_ms{phase="propose|collect_da|collect_prevote|collect_precommit|commit"}`
  — smoothed per-phase latency (EMA). The pacemaker consumes these phases;
  track drift between the EMA and raw histogram to catch sudden latency spikes.
- `/v2/sumeragi/telemetry` (or `iroha_cli --output-format text ops sumeragi status`) — compact
  JSON snapshot with per-collector vote counts, certificate latency, RBC backlog, and the
  latest highest/locked certificate hashes (`highest_qc`/`locked_qc`) (CLI output truncates hashes for readability).
- `/v2/sumeragi/status/sse` — SSE stream mirroring `/v2/sumeragi/status` for live dashboards (≈1s cadence).
- `/v2/sumeragi/phases` (or `iroha_cli --output-format text ops sumeragi phases`) — latest
  `{ propose_ms, collect_da_ms, collect_prevote_ms, collect_precommit_ms, collect_aggregator_ms, commit_ms, pipeline_total_ms }`
  plus an `ema_ms` object mirroring all phases (including `pipeline_total_ms`)
  for dashboards tracking drift between spot and smoothed latencies. The
  pipeline total aggregates the pacemaker-controlled phases (propose →
  commit), providing a single end-to-end latency signal.
- Import `docs/source/grafana_sumeragi_overview.json` for a ready-made Grafana
  dashboard visualising certificate height drift, BlockCreated drop counters, and VRF
  participation/penalty trends.

**Alert thresholds**
- Availability evidence latency: alert when `sumeragi_qc_last_latency_ms` for
  `kind="availability"` exceeds `0.6 * commit_time_ms` for two consecutive
  windows, or when the histogram P95 crosses `0.7 * commit_time_ms`
  (permissioned `CommitTimeMs`, NPoS `effective_npos_timeouts.commit_ms`).
- Vote ingress stagnation: alert when
  `sum(rate(sumeragi_da_votes_ingested_total[2m])) == 0` while
  `sumeragi_rbc_backlog_sessions_pending > 0` (collectors are not making
  progress despite active RBC sessions).
- Commit latency: alert when `commit_ms` or the P95 of
  `sumeragi_phase_latency_ms{phase="commit"}` exceeds `0.75 * commit_time_ms`
  (permissioned `CommitTimeMs`, NPoS `effective_npos_timeouts.commit_ms`), or when it
  remains zero for ≥3 consecutive rounds while new blocks arrive (commit
  pipeline stalled).
- Collector fan-out: alert when `collect_aggregator_ms` exceeds
  `0.5 * sumeragi.advanced.npos.timeouts.aggregator_ms` for three consecutive rounds, or
  when redundant sends occur more than `sumeragi.collectors.redundant_send_r` times in a single view
  (watch `sumeragi_redundant_sends_total`), gossip fallback fires repeatedly
  (monitor `rate(sumeragi_gossip_fallback_total[5m]) > 0`), or proposals are
  dropped at the locked certificate gate (`increase(block_created_dropped_by_lock_total[5m]) > 0`), fail header checks (`increase(block_created_hint_mismatch_total[5m]) > 0`,
  `increase(block_created_proposal_mismatch_total[5m]) > 0`), or stall due to proposal backpressure (queue saturation, relay/RBC backlog, or blocking pending blocks; `increase(pacemaker_backpressure_deferrals_total[5m]) > 0`). Sustained breaches indicate that the
  primary collector is lagging or network backpressure is throttling vote flow.

Redundant fan-out counters tick when the DA retry loop re-broadcasts cached RBC
payloads to additional collectors. The integration harness exercises this path
via `npos_redundant_send_retries_update_metrics` to keep dashboards aligned with
the consensus backoff contract.

**Triage checklist**
1. Confirm collector assignments via `iroha_cli --output-format text ops sumeragi collectors`
   and ensure the stalled peer is still designated.
2. Query `/v2/sumeragi/telemetry` to pinpoint the collector index with a flat
   `votes_ingested` counter. If only one collector is stalled, increase
   `sumeragi.collectors.redundant_send_r` temporarily so validators fan out to
   the next collector while investigating networking issues.
3. Inspect `sumeragi_bg_post_queue_depth` and `p2p_*_throttled_total` metrics to
   determine whether bounded queues or transport caps are delaying vote or
   payload propagation.
4. For FASTPQ/prover stalls, check `/v2/torii/zk/prover/reports` (when the
   prover is enabled) and the Torii logs for failed witness uploads; recover or
   restart the affected prover workers.
5. If both collectors are idle, inspect RBC backlog gauges to confirm payloads
   are still circulating. A growing backlog together with DA stagnation
   indicates collectors are wedged; restart the affected peers and investigate
   for mismatched manifests or disk pressure flagged in `sumeragi_rbc_store_*`
  metrics. `sumeragi_rbc_store_evictions_total` spikes when TTL or capacity
  limits prune sessions, while `sumeragi_rbc_persist_drops_total` climbs when
  the async persist queue saturates; both signal that the persisted backlog is
  shedding load and warrants operator attention. `/v2/sumeragi/status` now exposes the
  most recent `(block_hash, height, view)` tuples in
  `rbc_store.recent_evictions` so operators can correlate evictions with
  specific payloads and epochs. Example excerpt:

  ```json
  {
    "rbc_store": {
      "sessions": 3,
      "bytes": 1048576,
      "pressure_level": 1,
      "backpressure_deferrals_total": 5,
      "persist_drops_total": 2,
      "evictions_total": 12,
      "recent_evictions": [
        { "block_hash": "deadbeefcafebabe", "height": 4201, "view": 7 }
      ]
    }
  }
  ```

  `iroha --output-format text ops sumeragi status` mirrors the same data, including current
  session/byte utilisation, aggregated governance-seal counters
  (`lane_governance_sealed_total`, `lane_governance_sealed_aliases`), and the
  active epoch scheduling tuple (`epoch_len`, `epoch_commit`, `epoch_reveal`), so
  operators can eyeball backlog pressure, spot sealed lanes, and review
  pacemaker timing without parsing JSON. Pair the view with
  `iroha_cli app nexus lane-report --only-missing --fail-on-sealed` during upgrades
  to exit pipelines when governance manifests are still missing.
6. Once service resumes, document the incident and restore redundant-send and
   collector parameters to their baseline values.

### Pacemaker & RBC telemetry surfaces

Operators can pull deterministic telemetry snapshots over Torii or via the CLI.

#### CLI and HTTP reference

- `iroha --output-format text ops sumeragi telemetry` hits `GET /v2/sumeragi/telemetry` and reports availability votes, collector counts, VRF penalties, and RBC backlog figures in a single line suitable for shift notes.
- `iroha --output-format text ops sumeragi pacemaker` (`GET /v2/sumeragi/pacemaker`) exposes the current view timeout, backoff window, jitter configuration, and RTT floor used by the EMA-based pacemaker. Capture this before and after incidents to prove pacing adjustments.
- `iroha --output-format text ops sumeragi phases` (`GET /v2/sumeragi/phases`) prints per-phase latencies together with EMA totals so you can distinguish between DA/collector stalls and proposal throughput regressions.
- `iroha --output-format text ops sumeragi rbc status` (`GET /v2/sumeragi/rbc`) tracks session counts, ready/deliver broadcasts, rebroadcast skips, and payload bytes delivered. Use `iroha --output-format text ops sumeragi rbc sessions` (`GET /v2/sumeragi/rbc/sessions`) when you need the chunk-level breakdown for a specific payload.
- For automation, hit the same endpoints from SDKs (`ToriiClient.getSumeragiTelemetry` et al.) and log the JSON payloads alongside Prometheus scrapes so governance artefacts include both machine-readable metrics and operator-facing evidence.

#### Key metrics

| Metric | Meaning | Operator action |
|--------|---------|-----------------|
| `sumeragi_pacemaker_backpressure_deferrals_total` | Pacemaker skipped a proposal because the transaction queue is saturated, consensus block-payload/RBC chunk queues are saturated, relay/RBC backpressure is active, or a pending block still blocks proposal assembly. | Inspect `iroha --output-format text ops sumeragi status` for `rbc_store.pressure_level`, drain gossip/RBC queues, and consider temporarily raising the per-queue caps (especially `sumeragi.advanced.queues.rbc_chunks`/`sumeragi.advanced.queues.block_payload`) or reducing incoming traffic before resuming. |
| `sumeragi_commit_pipeline_tick_total{mode,outcome}` | Pacemaker tick invoked the commit pipeline (`outcome="active"` when pending blocks existed, `"idle"` when empty). | Pair with `status_snapshot().commit_pipeline_tick_total` to prove timer-driven commits on quiet networks; alert if `idle` climbs while transactions are queued, or if `active` climbs without matching inbound votes. |
| `sumeragi_pacemaker_backoff_ms` / `sumeragi_pacemaker_view_timeout_target_ms` | Current view timeout vs. target window derived from the on-chain effective block time (`block_time_ms` scaled by `pacing_factor_bps`). Sustained values far above the block time indicate repeated view changes or retries. | Run `iroha --output-format text ops sumeragi pacemaker`, compare with the on-chain block time, and audit `p2p_*_throttled_total` plus RBC backlog metrics to find which stage is stretching the view timer. |
| `sumeragi_phase_latency_ema_ms{phase="collect_da_ms",…}` / `sumeragi_phase_total_ema_ms` | EMA latency per phase and across the full pipeline as rendered by `iroha --output-format text ops sumeragi phases`. | Trigger alerts when EMA totals exceed the configured view timeout, then correlate the offending phase with Torii/RBC logs to determine whether DA, vote-collection, or aggregator legs are stalling. |
| `sumeragi_rbc_store_sessions` / `sumeragi_rbc_store_pressure` / `sumeragi_rbc_store_bytes` | Persisted RBC sessions and pressure level. `pressure=2` means the store is shedding sessions to remain within bounds. | Take `iroha --output-format text ops sumeragi telemetry` plus `iroha ops sumeragi rbc status` snapshots, prune stale payloads, and review disk I/O. If pressure stays high, increase the per-session cap or speed up delivery via `sumeragi.collectors.redundant_send_r`. |
| `sumeragi_rbc_store_evictions_total` / `sumeragi_rbc_backpressure_deferrals_total` | Sessions evicted due to TTL/capacity enforcement and proposals deferred because the store refused new payloads. | Use `/v2/sumeragi/status.rbc_store.recent_evictions` and the CLI summary to pinpoint the affected height/view, re-ingest the payload if needed, and adjust `sumeragi.collectors.redundant_send_r` or store caps to avoid repeated evictions. |
| `sumeragi_rbc_persist_drops_total` | Persist requests dropped because the async RBC persist queue is full. | Check disk throughput and queue saturation, then tune RBC store caps or reduce incoming load to avoid prolonged in-memory growth. |
| `sumeragi_rbc_backlog_sessions_pending` / `sumeragi_rbc_backlog_chunks_total` / `sumeragi_rbc_backlog_chunks_max` | Number of payloads still missing chunks and the highest per-session backlog. | When `pending_sessions` or `chunks_total` plateaus, inspect `iroha --output-format text ops sumeragi rbc sessions` to find the stuck block hash, then check network logs for throttling or mismatched manifests. |
| `sumeragi_rbc_rebroadcast_skipped_total{kind="payload|ready"}` | Local peer skipped a payload/READY rebroadcast because it was not in the deterministic f+1 sender subset. | Compare counts across peers to confirm the intended subset is rebroadcasting; if stalls persist and skips climb everywhere, verify roster hashes and the leader index ordering. |
| `sumeragi_rbc_da_reschedule_total` | Legacy counter for DA deadline reschedules (no longer incremented when DA is advisory). | Keep at zero; use `sumeragi_da_gate_block_total{reason="missing_local_data"}` to monitor missing local payloads. |
| `sumeragi_membership_mismatch_total` / `sumeragi_membership_mismatch_active` | Peers disagree on roster membership for a `(height,view)` tuple. | Compare `/v2/sumeragi/status.membership` hashes across peers, run `iroha --output-format text ops sumeragi params` per node, and halt the rollout until every validator reports the same `ordered_peer_ids`. |

#### RBC rebroadcast budget

RBC payload recovery is capped per tick by `sumeragi.advanced.rbc.rebroadcast_sessions_per_tick` to avoid
rebroadcast storms when many sessions are missing chunks. Increase it on well-provisioned
networks to speed recovery, or lower it if P2P queues saturate during soak tests.

#### Pending RBC stash bounds

Before INIT arrives, RBC frames are bounded by per-session caps
(`sumeragi.advanced.rbc.pending_max_chunks`, `sumeragi.advanced.rbc.pending_max_bytes`) and the
session cap `sumeragi.advanced.rbc.pending_session_limit` (default 256). Entries also
expire after `sumeragi.advanced.rbc.pending_ttl_ms` when the session is not yet active;
once a session exists, pending READY/DELIVER frames are retained until the
session is cleared so roster delays do not drop availability evidence.
Session-cap evictions skip active sessions; if the cap is reached by active
sessions, new pending frames are dropped and recorded as `session_cap` drops.
The worst-case buffered payload before INIT is:

When pending frames are dropped (cap, TTL, or session-cap), the node
deterministically requests the missing `BlockCreated` from the cached roster
to preserve liveness; retries respect the standard backoff and roster-hash
checks.

```
sumeragi.advanced.rbc.pending_session_limit
  * min(sumeragi.advanced.rbc.pending_max_bytes, sumeragi.advanced.rbc.chunk_max_bytes * RBC_MAX_TOTAL_CHUNKS)
```

Size `sumeragi.advanced.rbc.pending_max_bytes` to at least one chunk (`sumeragi.advanced.rbc.chunk_max_bytes`), and
set `sumeragi.advanced.rbc.chunk_max_bytes` to a positive value (0 is rejected by config
validation) so payloads can always split into at least one chunk. Then
use the bound above to keep memory budgets deterministic. Drops and evictions
are surfaced via `/v2/sumeragi/telemetry.pending_rbc.{drops_cap_total,drops_cap_bytes_total,drops_ttl_total,drops_ttl_bytes_total,drops_bytes_total,evicted_total,max_*}`
and `iroha --output-format text ops sumeragi status`; sustained movement should trigger alerts
and a manifest/collector audit. Stash counters (`/v2/sumeragi/status.pending_rbc.stash_*`)
track READY/DELIVER/chunk frames parked before INIT or roster verification and
help correlate missing-block fetch retries with roster hydration. Evictions never mark availability on their own—
availability still requires availability evidence (RBC `READY` quorum or availability votes) even when
pending stash frames are discarded.

At runtime, `sumeragi.advanced.rbc.chunk_max_bytes` is clamped so a serialized `RbcChunk` (including
headers and length prefix) fits within the consensus payload plaintext cap derived
from `network.max_frame_bytes_block_sync`.

#### Adaptive observability

Enable `sumeragi.adaptive_observability` to let the actor temporarily widen
collector fan-out and pacemaker intervals when telemetry shows stalls:

- Trigger sources: certificate latency over `qc_latency_alert_ms` or bursts of missing
  availability warnings when `status.da_gate.missing_local_data_total`
  advances by `da_reschedule_burst` (legacy name) between ticks.
- Mitigation: raise redundant collector fan-out to
  `collector_redundant_r.max(baseline)` and add `pacemaker_extra_ms` to the
  proposal interval; the actor logs the decision with the measured metrics.
- Reset: once counters stabilise for `cooldown_ms`, the collector limit and
  pacemaker interval snap back to baseline.
- Defaults: disabled; 400 ms certificate latency threshold, DA burst of 2, +100 ms
  pacemaker interval, redundant fan-out of 3, 5 s cooldown.

### VRF Randomness Pipeline

Sumeragi’s NPoS pacemaker drives leader and collector rotation from a verifiable
random function (VRF). Each epoch (`epoch_length_blocks`) is split into two
windows:

1. **Commit window** (`vrf_commit_deadline_offset` blocks) — validators submit
   `VrfCommit` payloads with a hash of their reveal.
2. **Reveal window** (`vrf_reveal_deadline_offset` blocks) — validators disclose
   the reveal (`VrfReveal`). The actor verifies it against the prior commit and
   records the 32-byte reveal.

The commit/reveal ingestion path is synchronous and deterministic:

- Validators automatically post `VrfCommit` and `VrfReveal` frames to the commit
  topology when the commit/reveal windows open and the node is part of the
  active validator set.
  Manual submissions remain available via Torii for recovery/override flows:
  - `POST /v2/sumeragi/vrf/commit` with `{ "epoch": <u64>, "signer": <u32>,
    "commitment_hex": "<0x…>" }`.
  - `POST /v2/sumeragi/vrf/reveal` with `{ "epoch": <u64>, "signer": <u32>,
    "reveal_hex": "<0x…>" }`.
- Torii rate-limits the requests, validates the 32-byte hex payload, and forwards
  the message to the `SumeragiHandle`.
- The main-loop actor (`handle_vrf_commit`/`handle_vrf_reveal`) checks the local
  epoch, window, and commitment binding, then snapshots the in-progress epoch
  state into WSV (`world.vrf_epochs`) for durability and observability.
- The PRF seed stays fixed for the current epoch; snapshots update participation
  only and do not change leader/collector selection until the next epoch boundary.

When the epoch boundary is reached (block height multiple of
`epoch_length_blocks`), the actor:

1. Computes penalties (`committed_no_reveal`, `no_participation`).
2. Mixes valid reveals into the next epoch seed `S_e`.
3. Persists a finalized `VrfEpochRecord` (with penalties populated).
4. Updates `epoch_report::VrfPenaltiesReport`, status counters, and telemetry.

The refreshed seed flows back into deterministic collector selection through
`deterministic_collectors`, and `/v2/sumeragi/collectors` reports the active
plan alongside the `(height, view)` the pacemaker is evaluating.

If a node restarts after finalizing an epoch record but before persisting the
seed-only snapshot for the next epoch, it recomputes the next-epoch seed from
the finalized record (seed + ordered reveals) when selecting PRF values. This
keeps deterministic rotation consistent across peers even if the seed-only
record is missing at restart.

#### CLI and operator workflow

- `iroha_cli --output-format text ops sumeragi vrf-epoch --epoch <n>` prints the persisted seed,
  participation table, and penalty breakdown together with the recorded
  `commit_deadline_offset` / `reveal_deadline_offset` for that epoch. Omit
  `--output-format text` for the full JSON payload (`found`, `finalized`, counts, offsets).
- `iroha_cli --output-format text ops sumeragi telemetry` shows the latest availability vote totals,
  RBC backlog, and VRF participation summary (`reveals_total`, `late_reveals_total`,
  `committed_no_reveal`, `no_participation`). Drop `--output-format text` to inspect the full JSON payload.
- `iroha_cli --output-format text ops sumeragi params` prints the active consensus parameters pulled from WSV,
  including `evidence_horizon_blocks`, `activation_lag_blocks`, and `slashing_delay_blocks`, so operators can verify staged
  values before and after governance decisions.
- Use the Torii POST endpoints directly when you need to submit commits or reveals
  manually (automation should drive them during normal operations). Example:

  ```bash
  curl -X POST "$TORII/v2/sumeragi/vrf/commit" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"commitment_hex":"0x..."}'

  curl -X POST "$TORII/v2/sumeragi/vrf/reveal" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"reveal_hex":"0x..."}'
  ```
- Automation can poll `/v2/sumeragi/collectors` to confirm the collector set
  matches expectations (`collectors[*].peer_id`), and `/v2/sumeragi/status` for
  `prf_epoch_seed`, `prf_height`, `vrf_late_reveals_total`, and penalty totals.

**Randomness runbook (operator checklist)**
- Monitor `iroha_cli --output-format text ops sumeragi status` after each block; if `vrf_penalty_epoch` jumps or `committed_no_reveal` grows, open the per-epoch snapshot with `iroha_cli --output-format text ops sumeragi vrf-epoch --epoch <n>` to identify missing validators.
- During the commit window validate that each validator submitted a commitment: `iroha_cli --output-format text ops sumeragi telemetry` exposes `commitments_total`, while `/v2/sumeragi/telemetry` lists the per-signer map under `vrf.commitments`.
- Before the reveal deadline, watch `vrf.reveals_total` and `vrf_late_reveals_total`. If a reveal is missing, page the validator and be ready to post it manually via `POST /v2/sumeragi/vrf/reveal` (see curl example above) with a hex-encoded payload captured from the validator.
- Cross-check the epoch’s recorded `commit_deadline_offset` / `reveal_deadline_offset` against your expected schedule; mismatches indicate a configuration drift or stale genesis snapshot and should trigger incident response before the pacemaker advances to the next epoch.
- When a reveal lands late, follow the recovery flow below: capture the current seed, confirm the late reveal is recorded under `vrf.late_reveals`, and verify that `prf.epoch_seed` remains unchanged.
- At epoch rollover confirm penalties cleared: `vrf_committed_no_reveal_total` should drop to zero for the signer that supplied the late reveal, and `/v2/sumeragi/vrf/epoch/{n}` should show `finalized: true`.
- Healthy NPoS runs keep the RBC data plane inside the budgets enforced by CI
  (`integration_tests/tests/sumeragi_npos_happy_path.rs`): `sumeragi_rbc_deliver_broadcasts_total`
  should advance each block, while `sumeragi_bg_post_queue_depth` and the
  per-peer `sumeragi_bg_post_queue_depth_by_peer` gauges remain ≤ 16. Sustained
  breaches point to collectors that are stuck or to transport backpressure
  throttling vote/reveal propagation.


- Integration tests `npos_rbc_store_backpressure_records_metrics` and `npos_rbc_chunk_loss_fault_reports_backlog` capture RBC store pressure and chunk-loss telemetry end-to-end (`integration_tests/tests/sumeragi_npos_performance.rs:633` and `:760`).
- Nodes emit a rate-limited log when RBC store pressure transitions (warn on elevated pressure, info when back to normal). Repeated elevated logs are throttled to one per minute.
#### Metrics and alerts

Telemetry surfaces the VRF lifecycle so operators can wire alerts:

- `sumeragi_vrf_commit_emitted_total` /
  `sumeragi_vrf_reveal_emitted_total` — accepted submissions.
- `sumeragi_vrf_non_reveal_total` / `sumeragi_vrf_no_participation_total` —
  epoch penalties (incremented once per epoch).
- `sumeragi_prf_epoch_seed` (status endpoint) — the current seed; can be
  graphed by decoding the hex string to validate changes.
- `sumeragi_prf_context_height` / `_view` — the `(height, view)` used when
  sampling collectors or leaders.

Combine these with `/v2/sumeragi/vrf/epoch/{epoch}` to audit individual epochs
and `/v2/sumeragi/telemetry`’s `vrf` section for dashboards.

- Torii exposes `/v2/sumeragi/vrf/epoch/{epoch}` for persisted randomness snapshots. The
  response includes the epoch seed (`seed_hex`), commit/reveal participation, and penalty
  tallies. Use `iroha_cli --output-format text ops sumeragi vrf-epoch --epoch <n>` for a human-friendly view;
  omit `--output-format text` to print the full Norito payload.
- `/v2/sumeragi/telemetry` now returns a `vrf` section with the latest epoch summary:
  `{ found, epoch, finalized, seed_hex, roster_len, participants_total, commitments_total,
     reveals_total, late_reveals_total, committed_no_reveal[], no_participation[], late_reveals[] }`.
  The CLI summary mirrors these fields so dashboards can track reveal participation in real time.
- `/v2/sumeragi/status` exposes aggregate penalty counters (`vrf_penalty_epoch`,
  `vrf_committed_no_reveal_total`, `vrf_no_participation_total`,
  `vrf_late_reveals_total`) so operators can monitor participation drift alongside
  highest/locked certificate telemetry while confirming that late reveals do not mutate the
  active PRF seed (`prf.epoch_seed`).
- The status payload now includes a deterministic roster hash under
  `membership { height, view, epoch, view_hash }` and the last mismatch context under
  `membership_mismatch { active_peers, last_peer, last_height, last_view, last_epoch,
  last_local_hash, last_remote_hash, last_timestamp_ms }`. Compare this block across peers—
  either via `/v2/sumeragi/status` or the Norito payload—to confirm validator alignment
  and identify which peer diverged. Hashes are derived from `(chain_id, height, view,
  epoch, ordered_peer_ids)` using Blake2b-256.
- Late reveals: validators may submit reveals after the configured window (`vrf_reveal_deadline_offset`) to clear penalties. The actor verifies the reveal against the stored commitment, records it under `late_reveals`, and increments `sumeragi_vrf_reveals_late_total`. Late submissions never remix the epoch seed—only on-time reveals participate in the Blake2b accumulator—but they do remove the validator from the `committed_no_reveal` set. Late entries are persisted in `world.vrf_epochs[*].late_reveals` so operators can audit the height at which the reveal landed.

#### VRF alert thresholds
- Page when `increase(sumeragi_vrf_no_participation_total[epoch_window]) > 0`. Any increment means at least one validator missed both commit and reveal windows for the tracked epoch. Use a window length matching `vrf_commit_deadline_offset + vrf_reveal_deadline_offset` (for the defaults, `epoch_window = 140m` on a one‑second block cadence).
- Warn on `increase(sumeragi_vrf_non_reveal_penalties_total[epoch_window]) > 0` and include `sumeragi_vrf_non_reveal_by_signer` in the alert labels to pinpoint the validator that skipped the reveal.
- Alert when `increase(sumeragi_vrf_rejects_total_by_reason{reason!="late"}[5m]) > 0`. Non‑late reject reasons (`stale`, `invalid_signature`, `bad_epoch`, etc.) indicate malformed payloads or a validator running stale parameters.
- Notify when `rate(sumeragi_vrf_commits_emitted_total[5m]) == 0` for the entire commit window while the cluster continues finalising blocks (`increase(sumeragi_blocks_committed_total[5m]) > 0`). The combination signals that commitments are not being produced even though the epoch is active.
- Track `increase(sumeragi_vrf_reveals_late_total[epoch_window]) > 0` as a warning; frequent late reveals imply validators are missing the reveal deadline and may slip into non‑reveal penalties.

#### VRF alert response (runbook)
- Capture the current epoch context from `/v2/sumeragi/status` (`prf_height`, `prf_view`, `vrf_penalty_epoch`, `vrf_committed_no_reveal_total`, `vrf_no_participation_total`) to confirm the alerting epoch and whether penalties were already applied.
- Inspect the detailed participation table with `iroha_cli --output-format text ops sumeragi vrf-epoch --epoch <n>`. When multiple validators are missing, rerun without `--output-format text` to list per-signer commitments, reveals, and penalty flags.
- For `no_participation` increments, contact the affected validator and verify that the corresponding Torii ingress logs show VRF submissions. If the validator was offline, stage a joint reconfiguration or jailing proposal before the next epoch to keep quorum guarantees intact.
- For `non_reveal` increments, collect the stored commitment from the epoch snapshot (`commitments[*].commitment_hex`) and have the validator resend the reveal via `POST /v2/sumeragi/vrf/reveal` (see the curl example above). Confirm `sumeragi_vrf_reveals_late_total` increments and `vrf_committed_no_reveal_total` drops back to zero after the late submission.
- When `sumeragi_vrf_rejects_total_by_reason` fires, inspect the `reason` label to determine root cause. `invalid_signature` and `bad_epoch` require the validator to refresh its configuration; `stale` implies the payload was replayed and should be discarded. Record the offending payload hash in the incident notes.
- After remediation, wait for the next block and re-check `/v2/sumeragi/status` plus `/v2/sumeragi/telemetry.vrf` to ensure counters stabilise and the PRF seed (`seed_hex`) matches the value captured before the intervention. If penalties remain non-zero across an epoch boundary, escalate to governance to slash or rotate the validator.

#### Recovery: Late Reveals and Zero-Participation Epochs

1. **Capture the baseline seed.** Before acting on a reveal gap, record the
   current PRF seed from `/v2/sumeragi/status.prf.epoch_seed` (or
   `iroha_cli --output-format text ops sumeragi status`). The seed must remain unchanged if a
   reveal arrives late.
2. **Submit or observe the late reveal.** Once the validator broadcasts the
  payload, `/v2/sumeragi/telemetry.vrf.late_reveals_total` increments and
  `/v2/sumeragi/status.vrf_late_reveals_total` mirrors the count even before
  the epoch finalizes. The per-epoch snapshot `/v2/sumeragi/vrf/epoch/{n}`
   lists the signer, reveal bytes, and the height at which the late message was
   accepted.
3. **Verify the PRF seed is stable.** Compare the cached seed with the current
   `prf.epoch_seed` in `/v2/sumeragi/status`. A mismatch indicates an invalid
   reveal flow and should trigger incident response; late reveals must never
   remix epoch entropy.
4. **Confirm penalties clear at epoch rollover.** After the epoch boundary,
   `/v2/sumeragi/status` reports `vrf_committed_no_reveal_total = 0` for the
   late signer while retaining the `vrf_late_reveals_total` history. Telemetry
   counters (`sumeragi_vrf_committed_no_reveal_total`,
   `sumeragi_vrf_reveals_late_total`) also reset accordingly.
5. **Handle zero-participation epochs.** If no validator submits commitments or
  reveals, expect `/v2/sumeragi/status.vrf_no_participation_total` to equal the
  roster length and `vrf_late_reveals_total = 0`. The epoch snapshot still
  exposes the derived seed so operators can audit deterministic leader
  selection for the next epoch.

### DA availability matrix (`sumeragi.da.enabled`, advisory)

Build-line policy: v3 uses `sumeragi.da.enabled` (on-chain `SumeragiParameters.da_enabled`) as the single DA/RBC switch. In this first release, Iroha v3 deployments must keep DA/RBC enabled; the runtime honors the on-chain value and applies no override, and the node refuses to start if `sumeragi.da.enabled` is false.

- **DA/RBC enabled (`sumeragi.da.enabled=true`)** — availability evidence is tracked (advisory); commits proceed without waiting for evidence:
  - Missing local payload sets `status.da_gate.reason = missing_local_data`; `status.da_gate.missing_local_data_total` increments on every transition into this state. The gate clears once the payload is available locally (via `BlockCreated` or RBC delivery).
  - Manifest guard: when a block carries DA commitments but the corresponding manifest is missing or mismatched, `status.da_gate.reason` becomes one of `manifest_missing` / `manifest_hash_mismatch` / `manifest_read_failed` / `manifest_spool_scan` and `status.da_gate.manifest_guard_total` increments. Audit-only lanes still log the warning; strict lanes report the same reason but do not block commit.
- **DA/RBC disabled (`sumeragi.da.enabled=false`)** — unsupported in v3; the node refuses to start when DA/RBC is disabled.
- `status.da_gate.last_satisfied` records `missing_data_recovered` when the local payload becomes available. Commit does not depend on local RBC delivery; RBC is transport/recovery and is tracked separately via the RBC endpoints and metrics.
- Implementation note: commit certificates are cached by `(phase, hash, height, view, epoch)` so availability evidence cannot overwrite prepare/commit certificates. When certificates arrive before the payload, the node replays cached certificates once the payload is available so message reordering cannot strand availability tracking.

| Build line | Availability status | Status fields to watch | Typical remediation |
|------------|-------------|------------------------|---------------------|
| Iroha v3 (`sumeragi.da.enabled=true`) | Local payload availability (advisory) | `status.da_gate.reason`, `status.da_gate.missing_local_data_total`, `status.da_gate.last_satisfied`, legacy `status.sumeragi.da_reschedule_total` | Verify `BlockCreated` broadcasts and RBC payload recovery, inspect `/v2/sumeragi/status.rbc_store` and RBC backlog for stuck payloads, and restart collectors if quorum cannot form. |

DA availability transitions also emit structured debug logs when the reason changes or when a requirement is satisfied. The logs carry `reason`, `satisfied`, `da_enabled`, and `delivered` fields so operators can align missing-availability evidence with the corresponding telemetry counters without scraping Prometheus.

### Troubleshooting quick reference

| Symptom | Detection | Remediation |
|---------|-----------|-------------|
| Pacemaker keeps extending views beyond the configured block time | `iroha --output-format text ops sumeragi pacemaker` shows `backoff_ms` / `view_timeout_target_ms` climbing, `sumeragi_pacemaker_backpressure_deferrals_total` and `sumeragi_pacemaker_backpressure_deferrals_by_reason_total{reason=...}` increment, and `iroha --output-format text ops sumeragi phases` lists inflated EMA totals. | Inspect `sumeragi_phase_latency_*` to locate the slow phase, check `sumeragi_pacemaker_backpressure_deferral_age_ms{reason=...}` plus `sumeragi_bg_post_queue_depth{,_by_peer}` and `p2p_*_throttled_total`, and clear RBC backlog pressure before restoring the original pacemaker multipliers. |
| DA availability missing with payloads pending | `sumeragi_rbc_backlog_sessions_pending` or `sumeragi_rbc_backlog_chunks_total` plateau, `iroha --output-format text ops sumeragi rbc sessions` shows chunks missing, and DA availability counters (`sumeragi_da_gate_block_total{reason="missing_local_data"}`) increase. | Verify the manifest hash and chunk availability, restart collectors that stopped ingesting votes, temporarily increase `sumeragi.collectors.redundant_send_r`, and document the stalled block hash (`/v2/sumeragi/status.rbc_store.recent_evictions`). |
| Collector stops ingesting votes | `iroha --output-format text ops sumeragi telemetry` reports flat `availability.collectors[*].votes_ingested` for a single index and `sumeragi_bg_post_queue_depth_by_peer` spikes for that collector. | Use `iroha --output-format text ops sumeragi collectors` to confirm the assignments, bump `sumeragi.collectors.redundant_send_r` to fan out to another collector, and debug the peer’s networking (firewall, queue saturation) before restoring the baseline redundancy. |
| Membership mismatch alert | `sumeragi_membership_mismatch_active` gauges flip to `1` and `/v2/sumeragi/status.membership.view_hash` differs between peers. | Compare `/v2/configuration.sumeragi` snapshots, ensure `trusted_peers`/stake snapshots are identical, restart any validator that failed to apply the latest config, and keep the lane quiesced until every peer reports the same roster hash. |
| VRF penalties creeping up | Prometheus alerts on `increase(sumeragi_vrf_no_participation_total)` / `increase(sumeragi_vrf_non_reveal_penalties_total)` or CLI telemetry shows growing penalties. | Follow {doc}`sumeragi_randomness_evidence_runbook` to pull the per-epoch participation table, contact the validator, collect the late reveal via `POST /v2/sumeragi/vrf/reveal`, and confirm the `prf.epoch_seed` remained stable. |
| RBC store evicts sessions faster than expected | `sumeragi_rbc_store_pressure=2`, `sumeragi_rbc_store_evictions_total` increases, and `iroha --output-format text ops sumeragi status` lists recent evictions for nearby heights. | Expand disk allowance, confirm `sumeragi.advanced.rbc.store_max_bytes` and `sumeragi.collectors.redundant_send_r` match the production template, and re-ingest the affected payload once collectors are healthy. |

### Governance Checklist: Reconfiguration & Slashing

1. **Collect evidence** via `/v2/sumeragi/evidence` or `iroha_cli ops sumeragi evidence list` while the
   height remains within `evidence_horizon_blocks`.
2. **Stage the penalty** (e.g., `Unregister::peer`) and let the old validator set commit the block.
3. **Cancel if needed** by submitting `CancelConsensusEvidencePenalty` with the evidence payload
   before `slashing_delay_blocks` elapses; the record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, and no slashing applies.
4. **Schedule activation** by committing `SetParameter::Sumeragi::NextMode` and
   `SetParameter::Sumeragi::ModeActivationHeight` together. The activation height must exceed the
   current height by at least `activation_lag_blocks`.
5. **Verify the schedule** with `iroha_cli --output-format text ops sumeragi params` (or `/v2/sumeragi/params`) to
   confirm the staged `next_mode`, `mode_activation_height`, and `slashing_delay_blocks` values.
6. **Observe the switchover** via `iroha_cli --output-format text ops sumeragi status` once the activation height is
   committed; `next_mode` clears and the new set becomes active one block later.

**Evidence API & CLI quick reference**
- **List** — `iroha_cli --output-format text ops sumeragi evidence list` (JSON via `/v2/sumeragi/evidence`) surfaces the total count and the most recent records; drop `--output-format text` for the full Norito payload.
- **Filter** — refine the snapshot with `--kind DoublePrepare` / `DoubleCommit` / `InvalidQc` / `InvalidProposal` / `Censorship` and paginate via `--limit` / `--offset` when auditing large incident windows.
- **Count** — `iroha_cli ops sumeragi evidence count` (or `GET /v2/sumeragi/evidence/count`) reports the deduplicated total so operators can confirm that rejected payloads did not persist.
- **Submit** — `iroha_cli ops sumeragi evidence submit --evidence-hex <0x…>` (or `--evidence-hex-file forged_evidence.hex`) wraps `POST /v2/sumeragi/evidence` with a hex-encoded Norito payload. Torii validates structure and signatures (vote signatures against the commit topology and chain ID), emits `invalid consensus evidence` on mismatch, and never stores the entry.
- **Horizon audit** — `iroha_cli --output-format text ops sumeragi params` shows the active `evidence_horizon_blocks`; governance updates flow through `SetParameter::Custom(SumeragiNposParameters)` and tests guard short horizons to prevent stale replays from succeeding.

**Evidence runbook (operator checklist)**
- Record the current count via `iroha_cli ops sumeragi evidence count` before submitting slashing material; the value should increase only after valid payloads are accepted.
- When ingesting evidence manually, inspect the payload locally (for example with the Norito tooling or a staging node) to avoid propagating malformed votes before calling `iroha_cli ops sumeragi evidence submit`.
- After submission, poll `iroha_cli --output-format text ops sumeragi evidence list` and confirm the new record’s `recorded_at_height` equals the subject height (or the fallback height if horizon pruning applied).
- Use `iroha_cli ops sumeragi evidence list --kind <Kind>` to isolate double votes versus invalid QC/proposal reports; reconcile the paginated output (`--limit`, `--offset`) across peers to ensure the in-memory snapshot matches before/after governance actions.
- If a payload is rejected with `invalid consensus evidence`, inspect the CLI’s structured error and cross-check the underlying votes or proposal. No state change should occur; the count remains unchanged by design.
- Periodically compare `/v2/sumeragi/evidence/count` across peers. Divergence indicates a horizon mismatch or a node that failed to persist the record and should trigger incident response.
- Submit payloads with `iroha_cli ops sumeragi evidence submit --evidence-hex <0x…>` or
  `--evidence-hex-file forged_evidence.hex`. Use `--output-format text` to surface the
  `{kind, status}` line; omit it to return the full JSON payload.
- For pacemaker queue issues, run `python3 scripts/sumeragi_backpressure_log_scraper.py <logfile>`
  (or pipe `journalctl -f … | python3 scripts/sumeragi_backpressure_log_scraper.py -`) to correlate
  `pacemaker_backpressure_deferrals_total` spikes with "DA availability still missing (advisory)" logs and RBC backlog logs. Add `--status` when you
  have a `/v2/sumeragi/status` snapshot to include counter baselines; see `scripts/sumeragi_backpressure_log_scraper.py --help`
  and the telemetry runbook for details.

CI coverage keeps the evidence pipeline honest:
- `integration_tests/tests/sumeragi_negative_paths.rs` posts forged double-vote payloads with mismatched signer/height/view/epoch/signature metadata, invalid kind/payload pairings, and stale heights; each permutation must yield `invalid consensus evidence` and leave the persisted count untouched.
- `crates/iroha_core/src/sumeragi/evidence.rs` round-trips every negative mutation through the Norito codec before feeding it to the validator so encode/decode cannot “heal” malformed payloads. The fuzz-style loop jitters signatures and block hashes to guard the deduplication keys.
- The helper `set_evidence_horizon` in the integration suite stages short horizons and proves that stale evidence sourced from old heights is ignored even when the network replays it later.
