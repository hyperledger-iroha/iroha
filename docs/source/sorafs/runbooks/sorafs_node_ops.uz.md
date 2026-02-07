---
lang: uz
direction: ltr
source: docs/source/sorafs/runbooks/sorafs_node_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ea59a96be5215efa5796a5f1e0d20fa8949c76c4cb30980ea34216c0545fe745
source_last_modified: "2026-01-22T14:35:37.738882+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraFS Node Operations Runbook

This runbook walks operators through validating an embedded `sorafs-node`
deployment inside Torii. Each section maps directly to the SF-3 deliverables:
pin/fetch round trips, restart recovery, quota rejection, and PoR sampling.

> **Portal:** Mirrored in `docs/portal/docs/sorafs/node-operations.md`. Update
> both copies to keep operational guidance consistent across doc sets.

## 1. Prerequisites

- Enable the storage worker in `torii.sorafs.storage`:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Ensure the Torii process has read/write access to `data_dir`.
- Confirm the node advertises the expected capacity via
  `GET /v1/sorafs/capacity/state` once a declaration is recorded.
- When smoothing is enabled, dashboards expose both the raw and smoothed
  GiB·hour/PoR counters to highlight jitter-free trends alongside spot values.

### CLI Dry Run (Optional)

Before exposing HTTP endpoints you can sanity-check the storage backend with
the bundled CLI.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```

The commands print Norito JSON summaries and refuse chunk-profile or digest
mismatches, making them useful for CI smoke checks ahead of Torii wiring.【crates/sorafs_node/tests/cli.rs:1】

### PoR Proof Rehearsal

Operators can now replay governance-issued PoR artefacts before uploading them
to Torii. The same CLI wires into `NodeHandle::record_por_{challenge,proof,
verdict}`, so local rehearsals surface the exact validation errors Torii would
return.【crates/sorafs_node/src/bin/sorafs-node.rs:197】【crates/sorafs_node/tests/cli.rs:62】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

The command emits a JSON summary containing the manifest digest, provider id,
proof digest, sample count, and (optionally) verdict outcome. Pass
`--manifest-id=<hex>` when you want the CLI to double-check that the stored
manifest matches the challenge digest, and `--json-out=<path>` to archive the
resulting summary alongside the original artefacts for audit evidence. Keep the
`--verdict` flag handy when rehearsing end-to-end repairs—missing verdicts still
exercise challenge + proof ingestion, but providing one proves that the verdict
logic, failure reasons, and telemetry counters line up before you call the HTTP
API.

Once Torii is live you can retrieve the same artefacts via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Both endpoints are served by the embedded storage worker, so CLI smoke tests and
gateway probes stay in sync.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Capacity telemetry + penalty expectations

- Cooldowns are measured in **settlement windows** (`cooldown_windows *
  settlement_window_secs`). With a strike threshold of `1`, back-to-back
  under-delivery windows only slash once until the cooldown interval elapses;
  a second slash is applied on the first window after cooldown.
- The 30-day soak regression locks the current accrual math. Running
  `cargo test -p iroha_core smartcontracts::isi::sorafs::sorafs_tests::capacity_fee_ledger_30_day_soak_deterministic -- --nocapture`
  should print `capacity_soak_digest=71db9e1a17f66920cd4fe6d2bb6a1b008f9cfe1acbb3149d727fa9c80eee80d1`.
  If pricing or accrual logic changes, refresh the digest and rerun the test.

## 2. Pin → Fetch Round Trip

1. Produce a manifest + payload bundle (for example with
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Submit the manifest with base64 encoding:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   The request JSON must contain `manifest_b64` and `payload_b64`. A successful
   response returns `manifest_id_hex` and the payload digest.
3. Fetch the pinned data:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-decode the `data_b64` field and verify it matches the original bytes.

## 3. Restart Recovery Drill

1. Pin at least one manifest as above.
2. Restart the Torii process (or the entire node).
3. Re-submit the fetch request. The payload must still be retrievable and the
   returned digest must match the pre-restart value.
4. Inspect `GET /v1/sorafs/storage/state` to confirm `bytes_used` reflects the
   persisted manifests after the reboot.

## 4. Quota Rejection Test

1. Temporarily lower `torii.sorafs.storage.max_capacity_bytes` to a small value
   (for example the size of a single manifest).
2. Pin one manifest; the request should succeed.
3. Attempt to pin a second manifest of similar size. Torii must reject the
   request with HTTP `400` and an error message containing
   `storage capacity exceeded`.
4. Restore the normal capacity limit when finished.

## 5. Retention / GC Inspection (Read-only)

1. Run a local retention scan against the storage directory:

   ```bash
   iroha app sorafs gc inspect --data-dir ./storage/sorafs
   ```

2. Inspect only expired manifests (dry-run only, no deletions):

   ```bash
   iroha app sorafs gc dry-run --data-dir ./storage/sorafs
   ```

3. Use `--now` or `--grace-secs` to pin the evaluation window when comparing
   reports across hosts or incidents.

The GC CLI is intentionally read-only. Use it to capture retention deadlines
and expired-manifest inventory for audit trails; do not remove data manually in
production.

## 6. PoR Sampling Probe

1. Pin a manifest.
2. Request a PoR sample:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```

3. Verify the response contains `samples` with the requested count and that each
   proof validates against the stored manifest root.

## 7. PoR Proof Replay Helper

Operators can now validate challenge/proof bundles locally before submitting
them to Torii. Use the new CLI helper from the same host that stores the
manifest:

```
sorafs-node ingest por \
  --data-dir=/var/lib/sorafs \
  --manifest-id=<manifest_cid_hex> \
  --challenge=fixtures/por/challenge_v1.to \
  --proof=fixtures/por/proof_v1.to \
  --json-out=por_replay.json
```

The command decodes the Norito payloads, checks that the challenge/proof
content matches the stored manifest, and feeds both objects through the same
`NodeHandle` tracker Torii uses for live ingestion. The JSON summary contains the
manifest digest, provider identifier, proof digest, and (optionally) the verdict
metadata when `--verdict=<path>` is provided. Include the summary artefact in
incident tickets so SRE/governance reviewers can confirm which proofs were
replayed prior to submission.

## 8. Automation Hooks

- CI / smoke tests can reuse the targeted checks added in
  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```
  which covers `pin_fetch_roundtrip`, `pin_survives_restart`,
  `pin_quota_rejection`, and `por_sampling_returns_verified_proofs`.
- Dashboards should track:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` and `torii_sorafs_storage_fetch_inflight`
  - PoR success/failure counters surfaced via `/v1/sorafs/capacity/state`

Following these drills ensures the embedded storage worker can ingest data,
survive restarts, respect configured quotas, and generate deterministic PoR
proofs before the node advertises capacity to the wider network.
