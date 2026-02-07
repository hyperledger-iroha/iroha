---
lang: az
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-12-29T18:16:35.085419+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet GAR Intake Template

Use this intake form when requesting a GAR action (purge, ttl override, rate
ceiling, moderation directive, geofence, or legal hold). The submitted form
should be pinned alongside the `gar_controller` outputs so audit logs and
receipts cite the same evidence URIs.

| Field | Value | Notes |
|-------|-------|-------|
| Request ID |  | Guardian/ops ticket id. |
| Requested by |  | Account + contact. |
| Date/time (UTC) |  | When the action should start. |
| GAR name |  | e.g., `docs.sora`. |
| Canonical host |  | e.g., `docs.gw.sora.net`. |
| Action |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| TTL override (seconds) |  | Required only for `ttl_override`. |
| Rate ceiling (RPS) |  | Required only for `rate_limit_override`. |
| Allowed regions |  | ISO region list when requesting `geo_fence`. |
| Denied regions |  | ISO region list when requesting `geo_fence`. |
| Moderation slugs |  | Match the GAR moderation directives. |
| Purge tags |  | Tags that must be purged before serving. |
| Labels |  | Machine labels (incident id, drill name, pop scope). |
| Evidence URIs |  | Logs/dashboards/specs backing the request. |
| Audit URI |  | Per-pop audit URI if different from defaults. |
| Requested expiry |  | Unix timestamp or RFC3339; leave blank for default. |
| Reason |  | User-facing explanation; appears in receipts and dashboards. |
| Approver |  | Guardian/committee approver for the request. |

### Submission steps

1. Fill the table and attach it to the governance ticket.
2. Update the GAR controller config (`policies`/`pops`) with matching
   `labels`/`evidence_uris`/`expires_at_unix`.
3. Run `cargo xtask soranet-gar-controller ...` to emit events/receipts.
4. Drop `gar_controller_summary.json`, `gar_reconciliation_report.json`,
   `gar_metrics.prom`, and `gar_audit_log.jsonl` into the same ticket. The
   approver confirms the receipt count matches the PoP list before dispatch.
