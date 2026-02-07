---
id: dispute-revocation-runbook
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Dispute & Revocation Runbook
sidebar_label: Dispute & Revocation Runbook
description: Governance workflow for filing SoraFS capacity disputes, coordinating revocations, and evacuating data deterministically.
---

:::note Canonical Source
:::

## Purpose

This runbook guides governance operators through filing SoraFS capacity disputes, coordinating revocations, and ensuring data evacuation completes deterministically.

## 1. Assess the Incident

- **Trigger conditions:** detection of SLA breach (uptime/PoR failure), replication shortfall, or billing disagreement.
- **Confirm telemetry:** capture `/v1/sorafs/capacity/state` and `/v1/sorafs/capacity/telemetry` snapshots for the provider.
- **Notify stakeholders:** Storage Team (provider operations), Governance Council (decision body), Observability (dashboard updates).

## 2. Prepare Evidence Bundle

1. Collect raw artefacts (telemetry JSON, CLI logs, auditor notes).
2. Normalize into a deterministic archive (for example, a tarball); record:
   - BLAKE3-256 digest (`evidence_digest`)
   - Media type (`application/zip`, `application/jsonl`, and so on)
   - Hosting URI (object storage, SoraFS pin, or Torii-accessible endpoint)
3. Store the bundle in the governance evidence collection bucket with write-once access.

## 3. File the Dispute

1. Create a spec JSON for `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Run the CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. Review `dispute_summary.json` (confirm kind, evidence digest, timestamps).
4. Submit the request JSON to Torii `/v1/sorafs/capacity/dispute` via the governance transaction queue. Capture the `dispute_id_hex` response value; it anchors follow-up revocation actions and audit reports.

## 4. Evacuation & Revocation

1. **Grace window:** notify the provider of impending revocation; allow evacuation of pinned data when policy permits.
2. **Generate `ProviderAdmissionRevocationV1`:**
   - Use `sorafs_manifest_stub provider-admission revoke` with the approved reason.
   - Verify signatures and the revocation digest.
3. **Publish revocation:**
   - Submit the revocation request to Torii.
   - Ensure provider adverts are blocked (expect `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` to climb).
4. **Update dashboards:** flag the provider as revoked, reference the dispute ID, and link the evidence bundle.

## 5. Post-Mortem & Follow-Up

- Record the timeline, root cause, and remediation actions in the governance incident tracker.
- Determine restitution (stake slashing, fee clawbacks, customer refunds).
- Document learnings; update SLA thresholds or monitoring alerts if required.

## 6. Reference Materials

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (dispute section)
- `docs/source/sorafs/provider_admission_policy.md` (revocation workflow)
- Observability dashboard: `SoraFS / Capacity Providers`

## Checklist

- [ ] Evidence bundle captured and hashed.
- [ ] Dispute payload validated locally.
- [ ] Torii dispute transaction accepted.
- [ ] Revocation executed (if approved).
- [ ] Dashboards/runbooks updated.
- [ ] Post-mortem filed with governance council.
