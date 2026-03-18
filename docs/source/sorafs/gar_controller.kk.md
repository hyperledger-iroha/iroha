---
lang: kk
direction: ltr
source: docs/source/sorafs/gar_controller.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 67e8e60ef4944c5198ea0c2de9009cc39e78f9c1306e0e94c8daf13ad90fb36a
source_last_modified: "2025-12-29T18:16:36.116817+00:00"
translation_last_reviewed: 2026-02-07
---

# GAR Controller Bundle (SNNet-15G)

The GAR controller bundles policy dispatch events, per-PoP enforcement receipts,
a reconciliation/metrics snapshot, and Markdown/JSON summaries so compliance
packs can be shipped without hand-written checklists. Use the sample config under
`configs/soranet/gateway_m0/gar_controller.sample.json` as a starting point.

## Running the generator

- `cargo xtask soranet-gar-controller --config <path> [--output-dir <dir>] [--markdown-out <path>] [--now <unix>]`
- Defaults:
  - Config: `configs/soranet/gateway_m0/gar_controller.sample.json`
  - Output root: `artifacts/soranet/gateway/gar_controller`
  - Includes a Markdown report by default (`gar_controller_report.md`); override with `--markdown-out`.
- The config schema:
  - `base_subject`: NATS subject prefix used when a pop omits `nats_subject`.
  - `operator`: Account ID recorded in every `GarEnforcementReceiptV1`.
  - `default_expires_at_unix`: Optional fallback expiry used when a policy omits `expires_at_unix`.
  - `pops`: List of `{label, nats_subject?, audit_uri?}` entries. Each pop receives every policy event.
  - `policies`: List of `{gar_name, canonical_host, policy (GarPolicyPayloadV1), policy_version?, cache_version?, evidence_uris?, labels?, expires_at_unix?}`.

## Outputs

- `gar_events.jsonl` — NDJSON spool of NATS events (`{subject, payload}`) ready for `nats pub --stdin`.
- `gar_receipts/*.json` + `.to` — per-PoP `GarEnforcementReceiptV1` in JSON and Norito.
- `gar_controller_summary.json` — structured summary of subjects, policies, digests, and output paths.
- `gar_controller_report.md` — operator-facing report for transparency packets and SLA evidence (includes reconciliation + warning table).
- `gar_reconciliation_report.json` — coverage per GAR/pop with warning list (expired policies, missing pops).
- `gar_metrics.prom` — Prometheus-style snapshot (dispatch, expiry, warning counters) for dashboards/runbooks.
- `gar_audit_log.jsonl` — NDJSON audit feed per pop (action/reason/receipt paths + evidence).

## Intake and transparency

- File a GAR moderation/legal intake using the template at
  `docs/examples/soranet_gar_intake_form.md`; keep the `labels`/`evidence_uris`
  in sync so receipts and audit logs cite the incoming request.
- The reconciliation report warns on expired GARs and missing PoP coverage; add
  the report + metrics snapshot to governance packets so dashboards can read the
  same evidence without re-running the generator.
- The audit log mirrors the NATS spool and receipt set; feed it to the ops
  console or NATS fan-out so per-pop ACKs can be correlated with receipts.

## Dispatch semantics

- Primary enforcement action derives from the CDN policy:
  - `legal_hold` → `legal_hold`
  - `allow_regions`/`deny_regions` → `geo_fence`
  - `rate_ceiling_rps` → `rate_limit_override`
  - `ttl_override_secs` → `ttl_override`
  - `purge_tags` → `purge_static_zone`
  - `moderation_slugs` → `moderation`
  - otherwise `audit_notice`
- Receipt IDs and policy digests are deterministic (BLAKE3 over policy bytes + pop + timestamp) to keep governance exports reproducible.
- Evidence URIs combine policy/policy-level entries with optional `audit_uri` hints from each pop so drill logs and dashboards match receipts.
