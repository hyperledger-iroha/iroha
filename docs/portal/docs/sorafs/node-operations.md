---
id: node-operations
title: Node Operations Runbook
sidebar_label: Node Operations Runbook
description: Validate the embedded `sorafs-node` deployment inside Torii.
---

:::note Canonical Source
Mirrors `docs/source/sorafs/runbooks/sorafs_node_ops.md`. Keep both versions in sync until the Sphinx set is retired.
:::

## Overview

This runbook walks operators through validating an embedded `sorafs-node` deployment inside Torii. Each section maps directly to the SF-3 deliverables: pin/fetch round trips, restart recovery, quota rejection, and PoR sampling.

## 1. Prerequisites

- Enable the storage worker in `sorafs.storage`:

  ```toml
  [sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- Configure distinct proof-outcome, repair, reserve, and orderbook entries under `sorafs.storage.native_transaction_signers`, and inject all four matching live providers. Storage startup requires this durable-drain bundle even when the repair/reserve/orderbook new-work generation flags are disabled. A reserve or orderbook role with both storage and its generation flag disabled is paused before task creation and makes zero external progress. Opening the local `NodeHandle` may still durably normalize an interrupted signer-only `Signing` claim back to `Ready` without refunding its attempt.
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

### PoR Proof Rehearsal

Operators can now replay governance-issued PoR artefacts locally before uploading them to Torii. The CLI reuses the same `sorafs-node` ingestion path, so local runs surface the exact validation errors that the HTTP API would return.

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

The command emits a JSON summary (manifest digest, provider id, proof digest, sample count, optional verdict outcome). Provide `--manifest-id=<hex>` to ensure the stored manifest matches the challenge digest, and `--json-out=<path>` when you want to archive the summary with the original artefacts for audit evidence. Including `--verdict` lets you rehearse the entire challenge → proof → verdict loop offline before calling the HTTP API.

Once Torii is live you can retrieve the same artefacts via HTTP:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Add `?limit=N` to metadata-only manifest probes when a bounded `files` preview
is enough; omit it when another gateway needs the complete directory layout.
Storage-plan probes default to bounded `files`, `chunk_digests_blake3`, and
`chunks` arrays (`limit` default 50, max 500) while keeping full count and
truncation fields.

Both endpoints are served by the embedded storage worker, so CLI smoke tests and gateway probes stay in sync.【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

### Evidence-viewer checkpoint authority

Before enabling `[sorafs.storage.evidence_viewer]`, bind
`checkpoint_store_handle`, non-zero `checkpoint_store_revision`, and canonical
non-zero `checkpoint_store_policy_digest_hex` to the deployment's exact
linearizable sealed-CAS provider, alongside the required WebAuthn, grant,
receipt-signer, and erasure bindings. These are public identity and policy pins
only; CAS credentials, private keys, and vendor diagnostics remain inside the
runtime provider. The deployment registry must return all five dependencies
when the viewer is enabled and none when it is disabled. Missing, unrequested,
substituted, stale, unavailable, or test-marked providers stop startup.

Treat the external signed head as authoritative. `checkpoint_path` names only a
private revalidated cache: it may be absent, match the authoritative record, or
be exactly one predecessor behind it, but it must never seed or replace the
external head. Operate at least two Torii replicas against the same authority
and rehearse concurrent CAS rejection, ambiguous-result readback, provider
rotation, rollback, restart, and cache-loss recovery before promotion. The
source tree supplies the injection contract, not a production CAS/HSM
implementation or deployment evidence.

## 2. Finalized Registration → Provider Ingest → Fetch

1. Produce a manifest + payload bundle (for example with `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json`).
2. Submit the canonical caller-signed registration transaction through `POST /v1/sorafs/pin/register`, then record its finalized height and block hash. V1 has no public payload-upload route.
3. Confirm the supervised provider worker admitted the exact finalized manifest and provider replication assignment into its durable outbox. The outbox identity binds the finalized cursor, manifest digest, provider id, and replication-order id.
4. Fetch the provider-ingested data:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<canonical manifest id>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   Base64-decode the `data_b64` field and verify it matches the original bytes.

## 3. Restart Recovery Drill

1. Complete one provider-outbox ingest as above.
2. Restart the Torii process (or the entire node).
3. Re-submit the fetch request. The payload must still be retrievable and the returned digest must match the pre-restart value.
4. Inspect `GET /v1/sorafs/storage/state` to confirm `bytes_used` reflects the persisted manifests after the reboot.

## 4. Capacity Rejection Test

1. Temporarily lower `sorafs.storage.max_capacity_bytes` to a small value (for example the size of a single manifest).
2. Let one finalized replication assignment complete through the provider outbox.
3. Reconcile a second assignment of similar size. The provider worker must reject it before storage mutation with `storage capacity exceeded`; no HTTP request can reserve capacity or change this result.
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

3. Use `--now` or `--grace-secs` to pin the evaluation window when comparing reports across hosts or incidents.

The GC CLI is intentionally read-only. Use it to capture retention deadlines and expired-manifest inventory for audit trails; do not remove data manually in production.

## 6. Authenticated PoR Sampling Probe

1. Pin and approve the canonical manifest, then obtain the provider identifier
   and runtime-only proof-stream credential for the deployment.
2. Request bounded PoR witnesses through the authenticated proof stream:

   ```bash
   sorafs_cli proof stream \
     --manifest=/path/to/manifest.to \
     --torii-url="https://$TORII/" \
     --provider-id-hex="$PROVIDER_ID_HEX" \
     --proof-kind=por \
     --samples=4 \
     --sample-seed=12345 \
     --stream-token="$SORAFS_STREAM_TOKEN" \
     --emit-events=false \
     --summary-out=por_probe_summary.json
   ```

   The unauthenticated local `/v1/sorafs/storage/por-sample` route is retired
   and is not mounted. Proof-stream requests must set `sample_count` between 1
   and 500. The CLI must resolve the exact native pin record first and carry its
   non-zero finalized height/hash pair in the PoR request; Torii rejects stale
   cursors and verifies every generated witness against that record's committed
   manifest root.

3. Archive the payload-free summary only after the CLI has authenticated every
   returned witness against the exact canonical manifest.

## 7. Automation Hooks

- CI / smoke tests can reuse the targeted checks added in:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  which covers `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, and `por_sampling_returns_verified_proofs`.
- Dashboards should track:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `sorafs_provider_ingest_inflight` and `torii_sorafs_storage_fetch_inflight`
  - PoR success/failure counters surfaced via `/v1/sorafs/capacity/state`
  - Settlement publish attempts via `sorafs_node_deal_publish_total{result=success|failure}`

Following these drills ensures the embedded storage worker can ingest data, survive restarts, respect configured quotas, and generate deterministic PoR proofs before the node advertises capacity to the wider network.
