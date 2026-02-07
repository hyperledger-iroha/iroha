---
lang: ba
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/node-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a37b7ca6ae1aa64e6289ecc44b48ef29c1c884abc039123c1a03b9c35b2e7120
source_last_modified: "2026-01-22T14:35:36.900283+00:00"
translation_last_reviewed: 2026-02-07
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
---

:::note Canonical Source
Mirrors `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Keep both copies aligned across releases.
:::

## Overview

This runbook walks operators through validating an embedded `sorafs-node` deployment inside Torii. Each section maps directly to the SF-3 deliverables: pin/fetch round trips, restart recovery, quota rejection, and PoR sampling.

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
- Confirm the node advertises the expected capacity via `GET /v1/sorafs/capacity/state` once a declaration is recorded.
- When smoothing is enabled, dashboards expose both the raw and smoothed GiB·hour/PoR counters to highlight jitter-free trends alongside spot values.

### CLI Dry Run (Optional)

Before exposing HTTP endpoints you can sanity-check the storage backend with the bundled CLI.【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

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

The commands print Norito JSON summaries and refuse chunk-profile or digest mismatches, making them useful for CI smoke checks ahead of Torii wiring.【crates/sorafs_node/tests/cli.rs#L1】

Once Torii is live you can retrieve the same artefacts via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Both endpoints are served by the embedded storage worker, so CLI smoke tests and gateway probes stay in sync.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Fetch Round Trip

1. Produce a manifest + payload bundle (for example with `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Submit the manifest with base64 encoding:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   The request JSON must contain `manifest_b64` and `payload_b64`. A successful response returns `manifest_id_hex` and the payload digest.
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
3. Re-submit the fetch request. The payload must still be retrievable and the returned digest must match the pre-restart value.
4. Inspect `GET /v1/sorafs/storage/state` to confirm `bytes_used` reflects the persisted manifests after the reboot.

## 4. Quota Rejection Test

1. Temporarily lower `torii.sorafs.storage.max_capacity_bytes` to a small value (for example the size of a single manifest).
2. Pin one manifest; the request should succeed.
3. Attempt to pin a second manifest of similar size. Torii must reject the request with HTTP `400` and an error message containing `storage capacity exceeded`.
4. Restore the normal capacity limit when finished.

## 5. PoR Sampling Probe

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

3. Verify the response contains `samples` with the requested count and that each proof validates against the stored manifest root.

## 6. Automation Hooks

- CI / smoke tests can reuse the targeted checks added in:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  which covers `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, and `por_sampling_returns_verified_proofs`.
- Dashboards should track:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` and `torii_sorafs_storage_fetch_inflight`
  - PoR success/failure counters surfaced via `/v1/sorafs/capacity/state`
  - Settlement publish attempts via `sorafs_node_deal_publish_total{result=success|failure}`

Following these drills ensures the embedded storage worker can ingest data, survive restarts, respect configured quotas, and generate deterministic PoR proofs before the node advertises capacity to the wider network.
