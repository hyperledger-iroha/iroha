---
title: Data Availability Ingestion Rollback Plan
summary: Playbook for pausing or reversing the DA ingest surface (`/v1/da/ingest`) when Torii, rent, or replication policy regressions threaten DA-2/DA-7 readiness.
---

# Data Availability Ingestion Rollback Plan

This playbook satisfies the roadmap requirement that every DA ingest launch
(DA-2/DA-7/DA-8 in `roadmap.md`) ships with a rollback and evidence plan. It is
tailored to the ingest surfaces described in
`specs/da/ingest_plan.md`, the rent policy in
`specs/da/rent_policy.md`, and the CLI flows implemented under
`crates/iroha_cli/src/commands/da.rs`. Keep it alongside the DA governance
packet so operators can pause the endpoint without improvising the steps during
an incident.

## 1. Metadata

- **Feature / rollout name:** DA-2/DA-7 — Torii `/v1/da/ingest` GA (rent quote +
  durable manifest and shard spool for native publication).
- **Owner / DRI:** DA program TL (Storage Team) with Torii duty engineer and
  Release/SRE on-call handling Alertmanager and governance comms.
- **Change window:** Planned upgrades piggyback on DA deployments announced in
  the DA status digest; emergency rollback can be triggered at any time but must
  be announced in `#da-ops`.
- **Rollback window:** First 2 hours after a DA configuration or binary change,
  or immediately when governance orders a halt. Any extended pause (>24 h)
  requires approval from Treasury + Governance.
- **Receipt spool version:** Builds that write version-2 durable receipts
  (`StoredDaReceiptV2`, `DaIngestSignedReceiptAssignmentV2`) refuse a retained
  version-1 `da-receipt-*.norito` or `*.receipt.norito` frame with a typed
  "retired version-1" error, and Torii does not start over it. Before rolling
  such a build onto a node, list `torii.da_ingest.manifest_store_dir` and drain
  or archive any version-1 receipt frames. Neither a rollback nor a roll forward
  converts receipts between versions.
- **Dependencies:** Torii build with DA handlers enabled, access to the
  retained tickets under `torii.da_ingest.manifest_store_dir`, rent/replication policy files,
  Alertmanager routes for `torii_da_*` metrics, and CLI/SDK helpers:
  `iroha app da submit`, `iroha app da get-blob`, `iroha app da prove-availability`,
  `iroha app da rent-quote`.
- **Evidence bundle paths:** `specs/da/ingest_plan.md`,
  `specs/da/rent_policy.md`, `dashboards/grafana/sorafs_capacity_health.json`,
  `fixtures/da/ingest/…`, and the ingest smoke harness
  `scripts/taikai_ingest_smoke.sh`.

## 2. Trigger Conditions

| Condition | Detection & Threshold | Escalation |
|-----------|----------------------|------------|
| **Manifest spool corruption / write failures** | `torii_da_spool_batches_total{outcome="partial_error"}` increasing or Torii logging “DA spool action failed”. Alert when 3 errors occur within 5 minutes or when the spool directory free space drops below 15%. | Page Storage TL + Release/SRE; open incident if manifests cannot be written in <10 min. |
| **Rent or incentive mis-pricing** | Sudden deltas between `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, or `torii_da_pdp_bonus_micro_total` vs `specs/da/rent_policy.md` tables, or `iroha app da rent-quote` disagreeing with receipt-embedded `rent_quote`. | Treasury + Economics WG; halt ingest until rent calculator bug is understood. |
| **Replay cache divergence / duplicate rejection** | `iroha app da submit` returns `409 Conflict` for fresh producer sequences. Check the replay cache store under `torii.da_ingest.replay_cache_store_dir`. | DA Foundations TL; coordinate with Torii team to reset caches after snapshots are captured. |
| **Taikai anchor/backfill stuck** | `record_taikai_ingest_metrics` panels (Telem board `sorafs_capacity_health`) show ingest latency >5 min, or `scripts/taikai_ingest_smoke.sh` fails on staging fixtures. | Media Platform WG + Storage TL. |
| **Governance/legal directive** | Governance votes or regulator requests suspend DA ingestion (e.g., contested rent policy or dispute). Detection via `specs/governance_playbook.md` notifications. | Governance contact + Storage TL; rollback immediately. |

## 3. Prerequisites & Safeguards

Before disabling or reverting the endpoint gather the following artefacts:

1. **Snapshot the manifest spool.**
   ```bash
   # Set MANIFEST_ROOT and REPLAY_ROOT to the absolute manifest_store_dir and
   # replay_cache_store_dir from the running node configuration. Set TORII_CONFIG
   # to the deployed configuration containing the DA policy tables.
   : "${MANIFEST_ROOT:?}" "${REPLAY_ROOT:?}" "${TORII_CONFIG:?}"
   ts=$(date -u +%Y%m%dT%H%M%SZ)
   mkdir -p artifacts/da_rollback/$ts
   sudo rsync -a "$MANIFEST_ROOT/" "artifacts/da_rollback/$ts/manifest_spool/"
   ```
   Keep the rsync log; it proves which manifests were pending before rollback.

2. **Capture replay cache + policy snapshot.**
   ```bash
   sudo tar -C "$REPLAY_ROOT" -czf "artifacts/da_rollback/$ts/replay_cache.tgz" .
   yq -p toml '.torii.da_ingest.replication_policy' "$TORII_CONFIG" \
     > "artifacts/da_rollback/$ts/replication_policy.before.yaml"
   yq -p toml '.torii.da_ingest.rent_policy' "$TORII_CONFIG" \
     > "artifacts/da_rollback/$ts/rent_policy.before.yaml"
   ```

3. **Record telemetry baselines.** Export the Grafana panels referenced in
   `dashboards/grafana/sorafs_capacity_health.json` (rent counters, manifest queue
   depth, PDP/PoTR bonuses) and attach them to the incident log. Include Alertmanager
   silence IDs if any alerts were muted.

4. **Preserve recent receipts and proof evidence.** Copy each submitter's
   `da_receipt.norito`, `da_receipt.json` and signed request from its submission directory.
   For a raw blob served by a configured fetch provider, capture a PoR report with
   its gateway provider descriptor and the accepted storage ticket:

   ```bash
   evidence_dir="artifacts/da_rollback/$ts/$ticket"
   iroha app da prove-availability \
     --storage-ticket "$ticket" --gateway-provider "$gateway_provider" \
     --manifest-cache-dir "$evidence_dir/manifest" --artifact-dir "$evidence_dir" \
     --scoreboard-out "$evidence_dir/scoreboard.json" --json-out "$evidence_dir/proof.json"
   ```

   For a ticket published with native `sorafs_cli deploy --da-source`, use
   `sorafs_cli da-recover` with its publication CID, finalized DA manifest
   digest and provider gateway URLs as described in `specs/da/ingest_plan.md`.

5. **Rent quote evidence.** Run `iroha app da rent-quote --gib <size> --months <months> \
   --policy-label "rollback-$ts" --quote-out artifacts/da_rollback/$ts/rent_quote.json`
   so governance can trace why rent contributions were paused.

## 4. Step-by-Step Rollback

| Step | Command / Action | Owner | Notes |
|------|------------------|-------|-------|
| 1 | Announce freeze in `#da-ops` and tag Governance/Treasury. Open incident doc in `specs/ops/archive/<id>/`. | SRE duty | Include reason + ticket ID. |
| 2 | Block `/v1/da/ingest` at the ingress layer. Example Istio patch: <br>`cat <<'YAML' > artifacts/da_rollback/$ts/da_ingest_block.yaml`<br>`apiVersion: networking.istio.io/v1beta1`<br>`kind: VirtualService`<br>`metadata: { name: torii-da-ingest-block, namespace: torii }`<br>`spec:`<br>`  hosts: ["torii.svc.cluster.local"]`<br>`  http:`<br>`    - match:`<br>`        - uri: { prefix: "/v1/da/ingest" }`<br>`      fault: { abort: { httpStatus: 503, percentage: { value: 100 } } }`<br>`      route:`<br>`        - destination: { host: torii.svc.cluster.local }`<br>`YAML`<br>`kubectl apply -f artifacts/da_rollback/$ts/da_ingest_block.yaml` | Networking TL / Platform Ops | Any traffic receives 503 so clients fall back to offline packaging. Remove the resource once rollback completes. |
| 3 | Confirm the ingress block covers both `POST /v1/da/ingest` and `POST /v1/da/ingest/parts`. Let previously accepted spool writes drain. | Release Engineering | Check ingress responses and the Torii spool queue on each ingest node. |
| 4 | Queue new submissions locally with `iroha app da submit --no-submit --payload "$payload" --lane-id "$lane" --epoch "$epoch" --sequence "$sequence" --artifact-dir "$submission_dir"`. Publish the service notice. | SDK Program Lead | Use a separate directory per request and retain the producer-signed request and its input payload. |
| 5 | For a preserved ticket, run `da_reconstruct --manifest "$ticket_dir/manifest.norito" --chunks-dir "$ticket_dir/source" --output "$evidence_dir/reconstructed.bin" --json-out "$evidence_dir/reconstruction.json"`. | Storage TL | Set `ticket_dir` to the snapshot's `artifacts/<first-ticket-byte-hex>/<ticket-hex>` directory and create `evidence_dir` outside the snapshot. |
| 6 | If Taikai input is affected, check offline packaging with `scripts/taikai_ingest_smoke.sh --workspace "$WORKSPACE" --fixtures "$WORKSPACE/fixtures/taikai/segments" --out "artifacts/da_rollback/$ts/smoke"`. | DA Foundations TL | Attach the CAR and metadata evidence. Exercise live ingest separately on staging. |
| 7 | File governance memo summarising pause reason, rent impact, and ETA; attach to `specs/governance_playbook.md` references and update `status.md`. | Governance contact | Required before resuming service. |

## 5. Verification Checklist

- `kubectl get virtualservice torii-da-ingest-block -n torii` shows the abort rule,
  and both ingest POST paths respond with `503 Service Unavailable`.
- `torii_da_rent_*` counters stop increasing while the endpoint is blocked.
- `manifest_store_dir` reports no new files after the patch timestamp.
- Reconstruct a retained ticket from the preserved manifest and shards. Compare
  the recovered payload against the original producer digest.
- Governance memo includes rent quote JSON, Grafana exports, the ingress patch,
  and replay cache tarball checksums.

## 6. Communications

| Audience | Channel | Owner | Message |
|----------|---------|-------|---------|
| Operators / partners | `#da-ops`, status page, email distro | Communications lead | “DA ingest paused at `<timestamp>` UTC due to `<condition>`. Receipts continue to verify; queued payloads must remain local until further notice. Next update in <time>.” |
| Governance | Governance docket (`specs/governance_playbook.md`) | Treasury + Storage TL | Include incident summary, rent impact, and approval reference. |
| Internal engineering | `#storage`, `#sdk-program` | DA TL | Provide instructions for offline packaging and projected restore time. |

## 7. Re-enable Criteria

Only remove the block once **all** of the following hold and are documented in
the incident log:

1. Root cause triaged (e.g., spool corruption identified, rent policy patched).
2. A staging producer can submit a fresh canary using its configured lane and trusted
   operator key. Its signed receipt matches the submitted payload.
3. `torii_da_spool_batches_total{outcome="partial_error"}` stays flat for 30 minutes after test traffic.
4. Governance sign-off recorded with link to evidence bundle.
5. SDK teams confirm they can resume calling `/v1/da/ingest`.

Deploy any corrected configuration through the normal release process, then restore
traffic by removing the incident ingress override:

```bash
kubectl delete -f "artifacts/da_rollback/$ts/da_ingest_block.yaml"
```

Submit a fresh canary on the restored endpoint:

```bash
iroha --config "$client_config" app da submit --payload "$payload" \
  --lane-id "$lane" --epoch "$epoch" --sequence "$sequence" \
  --operator-public-key "$operator_key" --artifact-dir "$submission_dir"
```

Attach the accepted receipt and associated rent counters to the incident ticket.

## 8. Attachments & Archival

Store the following under `specs/ops/archive/<incident-id>/`:

- Manifest spool rsync log + tarball hash.
- Replay cache tarball hash and lint output.
- Rent policy + replication policy snapshots.
- Grafana JSON exports and Alertmanager silence IDs.
- CLI artefacts from `iroha app da prove-availability` / `iroha app da rent-quote`.
- Copy of the ingress block YAML and helm upgrade command output.
- Governance memo + status page screenshot.

Keeping these artefacts complete the DA rollout acceptance criteria and ensures
future audits can reproduce the rollback decision.
