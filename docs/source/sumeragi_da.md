# Sumeragi data-availability & RBC scenario

The integration tests [`sumeragi_rbc_da_large_payload_four_peers`] and
[`sumeragi_rbc_da_large_payload_six_peers`] (defined in
`integration_tests/tests/sumeragi_da.rs`) spin up four- and six-peer networks
with `sumeragi.da.enabled = true` (DA + RBC). Each run uses the integration
harness default `LARGE_PAYLOAD_BYTES = 1024`, observes RBC delivery and
commit, verifies the protocol READY quorum (four peers: ≥3 votes; six peers:
≥5 votes, derived from the commit quorum), and prints a structured summary
that can be ingested by dashboards or regression tooling.

For light-client driven sampling of RBC payloads see
[`light_client_da.md`](light_client_da.md), which documents the authenticated
`/v1/sumeragi/rbc/sample` endpoint and the associated rate limits and budgets.

### DA timeout & availability tracking

With `sumeragi.da.enabled=true`, the commit pipeline records local payload availability
(`BlockCreated` or RBC delivery) in the DA gate. Availability evidence (availability votes
or an RBC `READY` quorum) is tracked for audit/telemetry and deterministic recovery, but
it is not a separate commit quorum. Local finalize waits while `missing_local_data` is
active, then continues once `BlockCreated` or RBC/block-sync recovery makes the payload
available locally. The DA gate status records
`missing_data_recovered` when local payload material arrives and
`manifest_guard_recovered` when required DA manifest material arrives and passes
the manifest guard.

The availability deadline is derived from the configured block/commit times and the
DA timeout tuning knobs; it is used to classify missing payloads as "stale" for logging
and rebroadcast heuristics:
- `sumeragi.advanced.da.quorum_timeout_multiplier` scales `block_time + 3 * commit_time`
  when DA is enabled (default `3`).
- `sumeragi.advanced.da.availability_timeout_multiplier` scales the availability timeout
  window in DA mode (default `2`).
- `sumeragi.advanced.da.availability_timeout_floor_ms` enforces a minimum availability
  window (default `2000`, set to `0` to disable the floor).
Keep these values aligned across validators to avoid divergent view-change
pacing.

Automatic RBC resend/abort driven by availability tracking was removed to avoid
circular waits between delivery and voting. Nodes that observe `availability evidence`
or an RBC `READY` quorum without the payload fetch it deterministically from the
commit-certificate signers for a bounded number of attempts, then fall back to the full commit
topology.

RBC `DELIVER` evidence is only terminal for local availability after the node
also has verified complete payload bytes for the session: all chunks are present
and the reconstructed payload matches the advertised hash. Delivered sessions
that are still missing chunks, or whose counted-complete chunks do not verify,
remain visible in backlog counters, keep chunk repair eligible, and wake the
commit pipeline when repair completes the local payload. Accepted `READY`
evidence against an incomplete delivered session is also treated as repair
progress and may wake the commit pipeline; only verified complete delivered
sessions suppress duplicate late `READY` progress or clear local `DELIVER`
deferral bookkeeping as terminal. Inbound duplicate `DELIVER` handling follows
the same rule: duplicate delivery frames for partial raw-delivered sessions may
still record valid bundled READY signatures, drive READY/repair bookkeeping,
and must not clear deferral state early.
Malformed live sessions with `total_chunks = 0` are rejected as invalid payload
state by the receiver-side `DELIVER` gate, including when DA mode allows
locally-authoritative missing chunk bytes.

Pending-block validation priority uses the same exact-payload boundary. A live
RBC session or retained RBC status summary may elevate validation as
`rbc_deliver` only when it is delivered, non-invalid, complete, and bound to the
pending block's payload hash. Complete retained summaries without a payload hash,
or with a hash for a different payload, remain diagnostic evidence and do not
advance validation scheduling.

When a cached RBC roster is later promoted to an authoritative derived roster,
the retry path also uses verified completion: partial raw-delivered sessions can
retry local `READY`/`DELIVER` repair after promotion, while verified complete
deliveries skip duplicate retries.

Permissioned future-height sessions may retain an INIT-carried roster while the
derived roster is unavailable, but local READY/DELIVER signing and inbound
READY/DELIVER acceptance only use that unverified roster if it exactly matches
the canonical current active topology and the cached roster source is recorded
as non-authoritative INIT evidence. Source-less or already-derived cached
rosters are not eligible for this escape hatch. Tiny self-consistent, foreign,
same-quorum subset, duplicate, or otherwise non-canonical INIT rosters are
stashed for recovery instead of reducing or reshaping the RBC certificate set.
INIT-carried unverified rosters are not cached as vote rosters; only
authoritative derived rosters can seed that cache for later vote validation.

The periodic RBC repair loop follows the same boundary when re-attempting local
`READY`: raw-delivered sessions with incomplete or unverified chunks still emit
local READY after a roster becomes authoritative, instead of waiting for the raw
DELIVER marker to become useful on its own.

Before local `READY`/`DELIVER` signing or stalled-session rebroadcast accounting
treats a malformed live chunk shape as terminal, the node first tries to hydrate
the session from authoritative local payload bytes. Exact local payloads can
rebuild zero-total metadata into the deterministic positive chunk layout and
recount over-counted `received_chunks`; sessions that cannot be repaired remain
deferred and repair-visible instead of signing from malformed counters.

Reschedule availability gates use the same verified complete-delivery boundary,
so a `delivered=true` session with a READY quorum but mismatched complete bytes
remains availability-unresolved until repair verifies the advertised payload or
the configured availability timeout releases the gate. Invalid chunk shapes
(`total_chunks == 0` or `received_chunks > total_chunks`) also remain
availability-unresolved before timeout, even when READY quorum is present.

Committed-block RBC cleanup also waits for verified complete delivery before
draining runtime session state. A raw delivered marker with missing or
mismatched chunks remains retained after commit so local repair and persisted
status can converge; only exact delivered payload evidence is settled cleanup
state. Stale-view pruning uses the same boundary and will not purge
raw-delivered sessions with incomplete chunks even if the block payload is
already locally available. The same rule applies to committed-tip repair
scheduling: retained raw-delivered sessions at the current tip remain
repair-active, while verified complete delivered sessions are idle. Session TTL
cleanup still ages out retained status summaries and persisted snapshots once
they become stale, even when committed cleanup has already removed the live
runtime session. RBC roster refresh also clears stale READY/DELIVER deferral
bookkeeping whenever it resets READY signatures for a changed roster, so old
retry state cannot leak across commit-topology changes. Local DELIVER emission
also rejects complete chunk sets with mismatched chunk roots before arming
missing-payload retry state, and terminal invalidation clears pending
READY/DELIVER deferrals together with pending RBC messages.

Invalid RBC sessions remain available through detailed status surfaces, but they
do not contribute to operator backlog counters or lane/dataspace backlog gauges;
invalid evidence is terminal for repair pressure and should be diagnosed through
the invalid/mismatch counters instead of treated as live missing-chunk work.

## Metrics captured

- Payload size (bytes) and derived throughput (MiB/s) when RBC marks the
  payload as delivered.
- RBC session snapshot (`total_chunks`, `received_chunks`, `ready_count`,
  `view`, `block_hash`, raw `delivered`, derived `complete_delivery`,
  `recovered`, `invalid`, `lane_backlog`, `dataspace_backlog`) fetched from
  `/v1/sumeragi/rbc/sessions`. `complete_delivery` is false for invalid,
  zero-chunk, over-counted, and chunk-incomplete summaries even when raw
  `delivered` is true.
- Per-height/view delivered probe from
  `/v1/sumeragi/rbc/delivered/{height}/{view}`. Its `delivered=true` result
  requires a non-invalid, positive, count-complete chunk summary; invalid,
  zero-chunk, over-counted, or chunk-incomplete summaries remain visible with
  `present=true` and `delivered=false`.
- Receiver-side RBC DELIVER acceptance applies the same fail-closed shape rule
  to live sessions: after READY quorum is satisfied, `total_chunks == 0` or
  `received_chunks > total_chunks` is invalid payload evidence, including when
  local DA policy otherwise allows missing chunks.
- Prometheus counters per peer: `sumeragi_rbc_payload_bytes_delivered_total`,
  `sumeragi_rbc_deliver_broadcasts_total`, and
  `sumeragi_rbc_ready_broadcasts_total` obtained from `/metrics`. Delivered
  payload-byte telemetry may use authoritative local payload bytes for
  incomplete but valid raw-delivered sessions, but malformed zero-total and
  over-counted raw-delivered sessions must first hydrate into a positive,
  count-complete shape before payload bytes or local-DELIVER accounting can be
  recorded from them.
- Per-lane/dataspace backlog gauges scraped from Prometheus:
  `sumeragi_rbc_lane_{tx_count,total_chunks,pending_chunks,bytes_total}` labeled by
  `lane_id` and `sumeragi_rbc_dataspace_{tx_count,total_chunks,pending_chunks,bytes_total}`
  labeled by `lane_id`/`dataspace_id`.

## Running the scenario

Telemetry must be enabled so the helper can query `/metrics` on each peer. Run
`scripts/check_norito_bindings_sync.sh` (or call the Python helper directly via `python3 scripts/check_norito_bindings_sync.py`) beforehand to verify that the Norito
bindings are aligned; if they are out of sync the build script will refuse to
proceed until the bindings are regenerated.

```bash
cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_four_peers -- --nocapture

cargo test -p integration_tests \
  sumeragi_rbc_da_large_payload_six_peers -- --nocapture
```

Each run prints lines prefixed with `sumeragi_da_summary::<scenario>::{...}` so
automation can capture the JSON payload. Optionally set
`SUMERAGI_DA_ARTIFACT_DIR=/path/to/dir` to persist the rendered summary and raw
Prometheus snapshots for every peer. The helper script
`scripts/run_sumeragi_da.py` enables the knob automatically for nightly runs and
now also writes a `sumeragi-da-report.md` by invoking `cargo run -p
build-support --bin sumeragi_da_report` against the collected artifacts. The
scheduled workflow `.github/workflows/sumeragi-da-nightly.yml` uploads the
entire run directory (summaries, metrics, Markdown report) so operators can
inspect results directly from GitHub Actions.

These scenarios leave `sumeragi.debug.rbc.force_deliver_quorum_one = false` and
exercise the protocol READY quorum. The debug knob is reserved for targeted
diagnostics; routine DA/RBC validation must keep it disabled so delivery and
throughput budgets measure production quorum behavior.

### RS16 initial fanout

RS16-encoded RBC sessions can reduce the first chunk-send wave with
`sumeragi.advanced.rbc.rs16_initial_fanout`:

- `full` sends every encoded shard to every selected chunk target. This is the
  default and preserves the existing transport profile.
- `data` sends exactly `data_shards` deterministic shard indices per RS16
  stripe to each selected target.
- `data_plus_one` sends `data_shards + 1` shard indices per stripe, capped at
  the stripe width, giving one extra shard of tolerance while avoiding the full
  parity fanout.

The selection is deterministic from the RBC session key and validator index, so
all nodes derive the same target/shard plan without runtime randomness. The
large-payload RS16 integration scenarios opt into `data_plus_one` for
measurement; keep the production default at `full` until before/after samples
show the reduced fanout improves delivery latency on representative networks.

## Expected baselines

With `LARGE_PAYLOAD_BYTES = 1024` in the integration harness and the
protocol READY quorum enabled, the default developer smoke runs use these
invariants. Larger payloads are tracked as soak/performance work rather than
this default check:

Note: `sumeragi.advanced.rbc.chunk_max_bytes` is clamped at startup so serialized RBC
chunks fit within the consensus frame plaintext cap derived from
`network.max_frame_bytes_block_sync`.

| Scenario | Chunk count | READY threshold | Per-peer counters | Timing budgets |
| --- | --- | --- | --- | --- |
| Four peers | 1 chunk in plain mode, 4 chunks in RS16 mode | READY votes ≥3 (2f+1 for *f* = 1) | `payload_bytes_delivered_total ≥ 1024`, `deliver_broadcasts_total ≥ 1`, and `ready_broadcasts_total ≥ 1` or equivalent persisted READY evidence | Harness delivery/commit budgets |
| Six peers | 1 chunk in plain mode, 6 chunks in RS16 mode | READY votes ≥5 (`Topology::min_votes_for_commit()` / `commit_quorum_from_len(6)`) | Same counters as above | Same budgets as above |

These smoke runs are primarily quorum, transport, and queue-regression checks.
Operators should alert if delivery latency approaches the harness budget, if
throughput dips below the computed floor for the configured payload, or if
per-peer counters diverge (indicating throttled collectors or missing chunks).

The helper `cargo run -p build-support --bin sumeragi_da_report [ARTIFACT_DIR]`
now ingests the `.summary.json` artifacts emitted by these scenarios and
produces a Markdown report containing aggregated latencies, throughput, and
per-run snapshots. Pass the artifact directory as the CLI argument (or set
`SUMERAGI_DA_ARTIFACT_DIR`) to render the report. The embedded report below was
rendered from the 2025-10-05 fixture run and replaces the earlier placeholder,
showing RBC delivery medians between 3.12&nbsp;s and 3.34&nbsp;s, commits within the
4&nbsp;s budget, and effective throughput ≥ 3.1 MiB/s.

Sandbox note: `scripts/run_sumeragi_da.py` now exports
`IROHA_SKIP_BIND_CHECKS=1` before spawning the peers and ships with a recorded
fixture (`integration_tests/fixtures/sumeragi_da/default/`). macOS seatbelt
sandboxes misreport permission errors during the config preflight bind, so the
env var lets the peers attempt the real bind and succeed when the runtime
permits it. If the environment still denies loopback sockets, the script
replays the fixture so dashboards continue to render data. Disable the fixture
fallback with `--disable-fixture-fallback` when running on hosts that can start
the test network.

## Performance budgets

The large-payload integration tests now enforce explicit performance budgets
while DA tracking, RBC, and SBV‑AM gating are enabled. The same values are emitted in the
structured summary and surface as columns inside the generated
`sumeragi-da-report.md` (`BG queue max`, `P2P drops max`). Operators should
alert when real runs drift beyond these ceilings:

| Metric | Budget | Enforcement | Alert guidance |
| --- | --- | --- | --- |
| RBC delivery latency | 35 s effective base budget (30 s base + 5 s delivery grace), plus 60 s per peer beyond four and a 40 s RS16 premium | `sumeragi_rbc_da_large_payload_*` | Alert when routine smoke runs approach the computed budget; investigate collector saturation. |
| Commit latency | RBC delivery budget + 40 s headroom | Same as above | Alert when commit latency approaches the computed budget; check pacemaker deadlines and view changes. |
| Effective throughput | At least min(payload/delivery-budget, 0.1 MiB/s) | Same as above | Alert when throughput falls below the computed floor for two consecutive runs. |
| Sumeragi background-post queue depth | ≤ 32 inflight tasks | Same as above | Alert when depth ≥ 24 to catch growing backlog early. |
| P2P queue drops (any priority/kind) | = 0 | Same as above | Alert immediately when non-zero; inspect bounded queue caps. |

Nightly CI consumes the same JSON summaries and renders the Markdown report so
dashboards can track historical compliance with these budgets.

Malformed live RBC chunk counters remain operator-visible. Sessions with
`total_chunks == 0` or `received_chunks > total_chunks` are not treated as
complete delivery, even when a local block payload is available. Maintenance
paths first try to hydrate the session from canonical local payload bytes.
Zero-total metadata is rebuilt from the deterministic positive chunk layout when
that exact local payload is available; if the counter shape remains malformed,
READY/DELIVER emission stays deferred and generic plus lane/dataspace backlog
snapshots keep trusted missing pressure visible for repair diagnostics.

.. mdinclude:: generated/sumeragi_da_report.md

## Adversarial scenarios

The `integration_tests/tests/sumeragi_adversarial.rs` suite exercises the RBC
debug knobs added for chaos testing across eleven four-peer scenarios:

- `sumeragi_adversarial_chunk_drop` enables
  `sumeragi.debug.rbc.drop_every_nth_chunk = 2` to verify that commits halt when
  the leader withholds every second chunk. The summary line is printed as
  `sumeragi_adversarial::chunk_drop::{...}` and includes the active RBC session
  snapshot.
- `sumeragi_adversarial_chunk_reorder` enables
  `sumeragi.debug.rbc.shuffle_chunks = true` to demonstrate that chunk
  re-ordering does not impact delivery or commit.
- `sumeragi_adversarial_witness_corruption` toggles
  `sumeragi.debug.rbc.corrupt_witness_ack = true` so the test can assert that
  corrupted acks block commit height while the RBC session still completes.
- `sumeragi_adversarial_duplicate_inits` uses
  `sumeragi.debug.rbc.duplicate_inits = true` to verify that duplicate proposal
  payloads in the next view remain deliverable and appear in the operator
  snapshot.
- `sumeragi_adversarial_chunk_drop_recovery` runs a two-phase flow: it first
  enables `drop_every_nth_chunk` to confirm collectors stall, then restarts the
  network without the knob to ensure commits resume once honest behaviour is
  restored.
- `sumeragi_adversarial_validator_selective_drop` enables
  `sumeragi.debug.rbc.drop_validator_mask` to withhold chunks for a selected
  validator and then checks that the cluster either stalls with explicit
  missing/incomplete RBC telemetry or recovers through commit-quorum-visible
  payload progress.
- `sumeragi_adversarial_chunk_equivocation_marks_invalid` combines
  `sumeragi.debug.rbc.equivocate_chunk_mask` with
  `sumeragi.debug.rbc.equivocate_validator_mask` so a targeted validator sees a
  conflicting shard. The test requires invalidation or mismatch telemetry when
  the corruption stalls progress, and bounded commit-quorum convergence when
  honest payload recovery heals it.
- `sumeragi_adversarial_conflicting_ready_marks_invalid` restarts a targeted
  peer with `sumeragi.debug.rbc.conflicting_ready_mask` and requires conflicting
  READY evidence to surface as invalid sessions, invalid READY counters,
  retained non-delivered RBC state, or bounded recovery.
- `sumeragi_adversarial_locked_qc_gate_rejects_conflicting_proposal` uses
  duplicate INIT emission around a locked-QC proposal conflict to verify the
  lock gate rejects conflicting block creation while preserving observable RBC
  duplicate-session or drop evidence.
- `sumeragi_adversarial_partial_chunk_withholding_stalls_delivery` enables
  `sumeragi.debug.rbc.partial_chunk_mask` to send truncated chunk material and
  verifies that delivery stalls with retained/missing RBC telemetry unless
  commit-quorum recovery supplies the exact payload.
- `sumeragi_adversarial_all_chunks_corrupted_abort` applies
  `equivocate_chunk_mask` to all validators through `equivocate_validator_mask`
  and requires corrupted-shard abort or mismatch evidence without allowing
  unbounded height divergence.

All scenarios accept `SUMERAGI_ADVERSARIAL_ARTIFACT_DIR` to persist the emitted
JSON summaries, mirroring the large-payload harness described above.
