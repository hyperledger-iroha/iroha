---
lang: fr
direction: ltr
source: docs/source/sorafs/dispute_revocation_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e803b9e5722ede5094c70b6491d90ace782c6ad726f351e6be771b31e68125c2
source_last_modified: "2026-01-22T07:33:57.270665+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Dispute & Revocation Runbook
---

# SoraFS Dispute & Revocation Runbook

This runbook guides governance operators through filing SoraFS capacity disputes,
coordinating revocations, and ensuring data evacuation happens deterministically.

## 1. Assess the Incident

- **Trigger conditions:** detection of SLA breach (uptime/PoR failure), replication shortfall, or billing disagreement.
- **Confirm telemetry:** capture `/v1/sorafs/capacity/state` and `/v1/sorafs/capacity/telemetry` snapshots for the provider.
- **Notify stakeholders:** Storage Team (provider operations), Governance Council (decision body), Observability (dashboard updates).

## 2. Prepare Evidence Bundle

1. Collect raw artefacts (telemetry JSON, CLI logs, auditor notes).
2. Normalize into a deterministic archive (e.g. tarball); record:
   - BLAKE3-256 digest (`evidence_digest`)
   - Media type (`application/zip`, `application/jsonl`, etc.)
   - Hosting URI (object storage, SoraFS pin, or Torii-accessible endpoint)
3. Store bundle in the governance evidence collection bucket with write-once access.

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

   ```
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. Review `dispute_summary.json` (confirm kind, evidence digest, timestamps).
4. Submit the request JSON to Torii `/v1/sorafs/capacity/dispute` via the governance transaction queue. Capture the `dispute_id_hex` response value; it anchors follow-up revocation actions and audit reports.

## 4. Evacuation & Revocation

1. **Grace window:** notify provider of impending revocation; allow evacuation of pinned data when policy permits.
2. **Generate `ProviderAdmissionRevocationV1`:**
   - Use `sorafs_manifest_stub provider-admission revoke` with the approved reason.
   - Verify signatures and revocation digest.
3. **Publish revocation:**
   - Submit revocation request to Torii.
   - Ensure provider adverts are blocked (expect `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` to climb).
4. **Update dashboards:** flag provider as revoked, reference dispute ID, and link evidence bundle.

## 5. Post-Mortem & Follow-Up

- Record timeline, root cause, remediation actions in the governance incident tracker.
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
