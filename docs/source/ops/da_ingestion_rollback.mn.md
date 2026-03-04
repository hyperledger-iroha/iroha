---
lang: mn
direction: ltr
source: docs/source/ops/da_ingestion_rollback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ea1f6dea710b521964cef15054ff78d99633ce3c7075ee71b6730886881564fa
source_last_modified: "2026-01-22T14:35:37.847519+00:00"
translation_last_reviewed: 2026-02-07
title: Data Availability Ingestion Rollback Plan
summary: Playbook for pausing or reversing the DA ingest surface (`/v1/da/ingest`) when Torii, rent, or replication policy regressions threaten DA-2/DA-7 readiness.
---

# Data Availability Ingestion Rollback Plan

This playbook satisfies the roadmap requirement that every DA ingest launch
(DA-2/DA-7/DA-8 in `roadmap.md`) ships with a rollback and evidence plan. It is
tailored to the ingest surfaces described in
`docs/source/da/ingest_plan.md`, the rent policy in
`docs/source/da/rent_policy.md`, and the CLI flows implemented under
`crates/iroha_cli/src/commands/da.rs`. Keep it alongside the DA governance
packet so operators can pause the endpoint without improvising the steps during
an incident.

## 1. Metadata

- **Feature / rollout name:** DA-2/DA-7 — Torii `/v1/da/ingest` GA (rent quote +
  manifest spooler shipping manifests to SoraFS orchestrators).
- **Owner / DRI:** DA program TL (Storage Team) with Torii duty engineer and
  Release/SRE on-call handling Alertmanager and governance comms.
- **Change window:** Planned upgrades piggyback on DA deployments announced in
  the DA status digest; emergency rollback can be triggered at any time but must
  be announced in `#da-ops`.
- **Rollback window:** First 2 hours after a DA configuration or binary change,
  or immediately when governance orders a halt. Any extended pause (>24 h)
  requires approval from Treasury + Governance.
- **Dependencies:** Torii build with DA handlers enabled, SoraFS manifest workers
  pointed at `torii.da_ingest.manifest_store_dir`, rent/replication policy files,
  Alertmanager routes for `torii_da_*` metrics, and CLI/SDK helpers:
  `iroha app da submit`, `iroha app da get-blob`, `iroha app da prove-availability`,
  `iroha app da rent-quote`.
- **Evidence bundle paths:** `docs/source/da/ingest_plan.md`,
  `docs/source/da/rent_policy.md`, `dashboards/grafana/sorafs_capacity_health.json`,
  `fixtures/da/ingest/…`, and the ingest smoke harness
  `scripts/taikai_ingest_smoke.sh`.

## 2. Trigger Conditions

| Condition | Detection & Threshold | Escalation |
|-----------|----------------------|------------|
| **Manifest spool corruption / enqueue failures** | `torii_da_manifest_enqueue_fail_total` incrementing, or Torii logs showing “failed to enqueue DA manifest for SoraFS orchestration” (`crates/iroha_torii/src/da/ingest.rs:595`). Alert when 3 errors occur within 5 minutes or when the spool directory free space drops below 15%. | Page Storage TL + Release/SRE; open incident if manifests cannot be written in <10 min. |
| **Rent or incentive mis-pricing** | Sudden deltas between `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, or `torii_da_pdp_bonus_micro_total` vs `docs/source/da/rent_policy.md` tables, or `iroha app da rent-quote` disagreeing with receipt-embedded `rent_quote`. | Treasury + Economics WG; halt ingest until rent calculator bug is understood. |
| **Replay cache divergence / duplicate rejection** | `torii_da_replay_reject_total` spikes or `iroha app da submit` returns `409 Conflict` for unique blobs. Check the replay cache store under `torii.da_ingest.replay_cache_store_dir`. | DA Foundations TL; coordinate with Torii team to reset caches after snapshots are captured. |
| **Taikai anchor/backfill stuck** | `record_taikai_ingest_metrics` panels (Telem board `sorafs_capacity_health`) show ingest latency >5 min, or `scripts/taikai_ingest_smoke.sh` fails on staging fixtures. | Media Platform WG + Storage TL. |
| **Governance/legal directive** | Governance votes or regulator requests suspend DA ingestion (e.g., contested rent policy or dispute). Detection via `docs/source/governance_playbook.md` notifications. | Governance contact + Storage TL; rollback immediately. |

## 3. Prerequisites & Safeguards

Before disabling or reverting the endpoint gather the following artefacts:

1. **Snapshot the manifest spool.**
   ```bash
   MANIFEST_ROOT=$(yq '.torii.da_ingest.manifest_store_dir' configs/torii.toml)
   ts=$(date -u +%Y%m%dT%H%M%SZ)
   mkdir -p artifacts/da_rollback/$ts
   sudo rsync -a --info=progress2 "$MANIFEST_ROOT/" "artifacts/da_rollback/$ts/manifest_spool/"
   ```
   Keep the rsync log; it proves which manifests were pending before rollback.

2. **Capture replay cache + policy snapshot.**
   ```bash
   yq '.torii.da_ingest.replay_cache_store_dir' configs/torii.toml \
     | xargs -I{} sudo tar -C "{}" -czf artifacts/da_rollback/$ts/replay_cache.tgz .
   yq '.torii.da_ingest.replication_policy' configs/torii.toml \
     > artifacts/da_rollback/$ts/replication_policy.before.yaml
   yq '.torii.da_ingest.rent_policy' configs/torii.toml \
     > artifacts/da_rollback/$ts/rent_policy.before.yaml
   ```

3. **Record telemetry baselines.** Export the Grafana panels referenced in
   `dashboards/grafana/sorafs_capacity_health.json` (rent counters, manifest queue
   depth, PDP/PoTR bonuses) and attach them to the incident log. Include Alertmanager
   silence IDs if any alerts were muted.

4. **Preserve recent receipts.** For the latest storage tickets, run:
   ```bash
   iroha app da prove-availability \
     --ticket <storage_ticket> \
     --artifact-dir artifacts/da_rollback/$ts/prove_availability_<ticket> \
     --scoreboard-out artifacts/da_rollback/$ts/<ticket>.scoreboard.json
   ```
   Grab the Norito/JSON receipts and scoreboard so auditors can replay the flow later.

5. **Rent quote evidence.** Run `iroha app da rent-quote --gib <size> --months <months> \
   --policy-label "rollback-$ts" --quote-out artifacts/da_rollback/$ts/rent_quote.json`
   so governance can trace why rent contributions were paused.

## 4. Step-by-Step Rollback

| Step | Command / Action | Owner | Notes |
|------|------------------|-------|-------|
| 1 | Announce freeze in `#da-ops` and tag Governance/Treasury. Open incident doc in `docs/source/ops/archive/<id>/`. | SRE duty | Include reason + ticket ID. |
| 2 | Block `/v1/da/ingest` at the ingress layer. Example Istio patch: <br>`cat <<'YAML' > artifacts/da_rollback/$ts/da_ingest_block.yaml`<br>`apiVersion: networking.istio.io/v1beta1`<br>`kind: VirtualService`<br>`metadata: { name: torii-da-ingest-block, namespace: torii }`<br>`spec:`<br>`  hosts: ["torii.svc.cluster.local"]`<br>`  http:`<br>`    - match:`<br>`        - uri: { prefix: "/v1/da/ingest" }`<br>`      fault: { abort: { httpStatus: 503, percentage: { value: 100 } } }`<br>`      route:`<br>`        - destination: { host: torii.svc.cluster.local }`<br>`YAML`<br>`kubectl apply -f artifacts/da_rollback/$ts/da_ingest_block.yaml` | Networking TL / Platform Ops | Any traffic receives 503 so clients fall back to offline packaging. Remove the resource once rollback completes. |
| 3 | Scale DA-specific workers to zero (if deployed separately) or disable the `da_ingest` HTTP handler by redeploying Torii with `DA_INGEST_DISABLED=true` in the Helm values. | Release Engineering | Prevents pods from writing new manifests mid-rollback. |
| 4 | Drain in-flight submissions by notifying SDK teams to run `iroha app da submit --no-submit` (offline packager) and queue manifests locally until the incident closes. Update docs/portal banner via `docs/portal/scripts/publish_banner.sh --message "...DA ingest paused..."`. | SDK Program Lead | Ensures no new payloads reach Torii while policies are inconsistent. |
| 5 | Repoint SoraFS orchestration to the preserved spool (if needed) and verify chunk stores with `sorafs_car da_reconstruct --manifest artifacts/da_rollback/$ts/.../manifest.encoded`. | Storage TL | Guarantees no manifest is lost before resubmission. |
| 6 | Re-run the DA ingest smoke harness against staging to confirm regressions are understood: `scripts/taikai_ingest_smoke.sh --workspace $WORKSPACE --fixtures fixtures/da/ingest/smoke --out artifacts/da_rollback/$ts/smoke`. | DA Foundations TL | Evidence that the bug is isolated. |
| 7 | File governance memo summarising pause reason, rent impact, and ETA; attach to `docs/source/governance_playbook.md` references and update `status.md`. | Governance contact | Required before resuming service. |

## 5. Verification Checklist

- `kubectl get virtualservice torii-da-ingest-block -n torii` shows the abort rule,
  and `/v1/da/ingest` responds with `503 Service Unavailable`.
- `torii_da_rent_*` counters stop increasing while the endpoint is blocked.
- `manifest_store_dir` reports no new files after the patch timestamp.
- At least one `iroha app da prove-availability` run succeeds using the preserved
  manifest spool, proving data integrity.
- Governance memo includes rent quote JSON, Grafana exports, the ingress patch,
  and replay cache tarball checksums.

## 6. Communications

| Audience | Channel | Owner | Message |
|----------|---------|-------|---------|
| Operators / partners | `#da-ops`, status page, email distro | Communications lead | “DA ingest paused at `<timestamp>` UTC due to `<condition>`. Receipts continue to verify; queued payloads must remain local until further notice. Next update in <time>.” |
| Governance | Governance docket (`docs/source/governance_playbook.md`) | Treasury + Storage TL | Include incident summary, rent impact, and approval reference. |
| Internal engineering | `#storage`, `#sdk-program` | DA TL | Provide instructions for offline packaging and projected restore time. |

## 7. Re-enable Criteria

Only remove the block once **all** of the following hold and are documented in
the incident log:

1. Root cause triaged (e.g., spool corruption identified, rent policy patched).
2. `scripts/taikai_ingest_smoke.sh` green on staging.
3. `torii_da_manifest_enqueue_fail_total` flat for 30 minutes after test traffic.
4. Governance sign-off recorded with link to evidence bundle.
5. SDK teams confirm they can resume calling `/v1/da/ingest`.

To restore service, delete the VirtualService override and redeploy Torii with
the corrected configuration:

```bash
kubectl delete -f artifacts/da_rollback/$ts/da_ingest_block.yaml
helm upgrade torii charts/torii --values deploy/env/prod/torii.values.yaml
```

After redeploying, run `iroha app da submit` against a canary payload, verify the
receipt and the associated rent counters, and update the incident ticket with
the new artefacts.

## 8. Attachments & Archival

Store the following under `docs/source/ops/archive/<incident-id>/`:

- Manifest spool rsync log + tarball hash.
- Replay cache tarball hash and lint output.
- Rent policy + replication policy snapshots.
- Grafana JSON exports and Alertmanager silence IDs.
- CLI artefacts from `iroha app da prove-availability` / `iroha app da rent-quote`.
- Copy of the ingress block YAML and helm upgrade command output.
- Governance memo + status page screenshot.

Keeping these artefacts complete the DA rollout acceptance criteria and ensures
future audits can reproduce the rollback decision.
