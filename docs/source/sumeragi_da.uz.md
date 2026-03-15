---
lang: uz
direction: ltr
source: docs/source/sumeragi_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a2fd90fac997198ed223d027f5226f45ac9561fe31d8dd0e6a47f65fcf3af92
source_last_modified: "2026-01-28T12:38:19.171246+00:00"
translation_last_reviewed: 2026-02-07
---

# Sumeragi data-availability & RBC scenario

The integration tests [`sumeragi_rbc_da_large_payload_four_peers`] and
[`sumeragi_rbc_da_large_payload_six_peers`] (defined in
`integration_tests/tests/sumeragi_da.rs`) spin up four- and six-peer networks
with `sumeragi.da.enabled = true` (DA + RBC). Each
test submits a ≥10&nbsp;MiB log instruction, observes RBC delivery and commit,
verifies that availability quorum can be formed for the payload, and prints a
structured summary that can be ingested by dashboards or regression tooling.

For light-client driven sampling of RBC payloads see
[`light_client_da.md`](light_client_da.md), which documents the authenticated
`/v1/sumeragi/rbc/sample` endpoint and the associated rate limits and budgets.

### DA timeout & advisory warnings

With `sumeragi.da.enabled=true`, the commit pipeline records local payload availability
(`BlockCreated` or RBC delivery) in the DA gate. Availability evidence (availability votes
or an RBC `READY` quorum) is tracked for audit/telemetry and is advisory in v1; commit/finalize
continue even when availability is missing. Missing local payloads are logged for operator
visibility and fetched via RBC or block sync.

The availability deadline is derived from the configured block/commit times and the
DA timeout tuning knobs; it is used to classify missing payloads as "stale" for logging
and rebroadcast heuristics:
- `sumeragi.advanced.da.quorum_timeout_multiplier` scales `block_time + 4 * commit_time`
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

## Metrics captured

- Payload size (bytes) and derived throughput (MiB/s) when RBC marks the
  payload as delivered.
- RBC session snapshot (`total_chunks`, `received_chunks`, `ready_count`,
  `view`, `block_hash`, `recovered`, `lane_backlog`, `dataspace_backlog`) fetched from
  `/v1/sumeragi/rbc/sessions`.
- Prometheus counters per peer: `sumeragi_rbc_payload_bytes_delivered_total`,
  `sumeragi_rbc_deliver_broadcasts_total`, and
  `sumeragi_rbc_ready_broadcasts_total` obtained from `/metrics`.
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

These scenarios enable `sumeragi.debug.rbc.force_deliver_quorum_one = true` so
RBC DELIVER is emitted after the first READY, keeping the throughput checks
focused on payload transport. Leave the knob disabled in production to preserve
the full 2f+1 READY quorum.

## Expected baselines

With the default `sumeragi.advanced.rbc.chunk_max_bytes = 256&nbsp;KiB`, the 10.5&nbsp;MiB
instruction (11 010 048 bytes), and `force_deliver_quorum_one` enabled, the
following invariants hold:

Note: `sumeragi.advanced.rbc.chunk_max_bytes` is clamped at startup so serialized RBC
chunks fit within the consensus frame plaintext cap derived from
`network.max_frame_bytes_block_sync`.

| Scenario | Chunk count | READY threshold | Per-peer counters | Timing budgets |
| --- | --- | --- | --- | --- |
| Four peers | 42 chunks (all required) | READY votes ≥1 (debug override; normal ≥3 for *f* = 1) | `payload_bytes_delivered_total ≥ 11 010 048`, `deliver_broadcasts_total = 1`, `ready_broadcasts_total = 1` | `commit_ms` and `rbc_deliver_ms` should stay within `commit_time_ms` (default `4000`) |
| Six peers | 42 chunks | READY votes ≥1 (debug override; normal ≥4 for *f* = 2) | Same counters as above | Same budgets as above |

Staying within the 4&nbsp;s commit window implies an average throughput of at
least ≈2.7&nbsp;MiB/s for the payload. Operators should alert if delivery latency
approaches the configured `commit_time_ms`, if throughput dips below that
floor, or if per-peer counters diverge (indicating throttled collectors or
missing chunks).

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
| RBC delivery latency (10.5 MiB payload) | ≤ 3.6 s | `sumeragi_rbc_da_large_payload_*` | Alert at ≥ 3.2 s; investigate collector saturation. |
| Commit latency | ≤ 4.0 s | Same as above | Alert at ≥ 3.6 s; check pacemaker deadlines and view changes. |
| Effective throughput | ≥ 2.7 MiB/s | Same as above | Alert when throughput < 3 MiB/s for two consecutive runs. |
| Sumeragi background-post queue depth | ≤ 32 inflight tasks | Same as above | Alert when depth ≥ 24 to catch growing backlog early. |
| P2P queue drops (any priority/kind) | = 0 | Same as above | Alert immediately when non-zero; inspect bounded queue caps. |

Nightly CI consumes the same JSON summaries and renders the Markdown report so
dashboards can track historical compliance with these budgets.

.. mdinclude:: generated/sumeragi_da_report.md

## Adversarial scenarios

The `integration_tests/tests/sumeragi_adversarial.rs` suite exercises the RBC
debug knobs added for chaos testing:

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

All scenarios accept `SUMERAGI_ADVERSARIAL_ARTIFACT_DIR` to persist the emitted
JSON summaries, mirroring the large-payload harness described above.
