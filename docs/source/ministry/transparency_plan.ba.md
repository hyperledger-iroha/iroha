---
lang: ba
direction: ltr
source: docs/source/ministry/transparency_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2639f4f7692e13ed61cc6246c87b047dc7415a6c9243ca7c046e6ccea8b55e9a
source_last_modified: "2025-12-29T18:16:35.983537+00:00"
translation_last_reviewed: 2026-02-07
title: Ministry Transparency & Audit Plan
summary: Implementation plan for roadmap item MINFO-8 covering quarterly transparency reports, privacy guardrails, dashboards, and automation.
---

# Transparency & Audit Reports (MINFO-8)

Roadmap reference: **MINFO-8 — Transparency & audit reports** and **MINFO-8a — Privacy-preserving release process**

The Ministry of Information must publish deterministic transparency artefacts so the community can audit moderation efficacy, appeal handling, and blacklist churn. This document defines the scope, artefacts, privacy controls, and operational workflow required to close MINFO-8 before the Q3 2026 target.

## Goals & Deliverables

- Produce quarterly transparency packets that summarise AI moderation accuracy, appeal outcomes, denylist churn, volunteer panel activity, and treasury movements tied to MINFO budgets.
- Ship accompanying raw-data bundles (Norito JSON + CSV) plus dashboards so citizens can slice metrics without waiting for static PDFs.
- Enforce privacy guarantees (differential privacy + minimum-count rules) and signed attestations before any dataset is published.
- Record each publication in the governance DAG and SoraFS so historical artefacts remain immutable and independently verifiable.

### Artefact Matrix

| Artefact | Description | Format | Storage |
|----------|-------------|--------|---------|
| Transparency summary | Human-readable report with executive summary, highlights, risk items | Markdown → PDF | `docs/source/ministry/reports/<YYYY-Q>.md` → `artifacts/ministry/transparency/<YYYY-Q>/summary.pdf` |
| Data appendix | Canonical Norito bundle with sanitized tables (`ModerationLedgerBlockV1`, appeals, blacklist deltas) | `.norito` + `.json` | `artifacts/ministry/transparency/<YYYY-Q>/data` (mirrored to SoraFS CID) |
| Metrics CSVs | Convenience CSV exports for dashboards (AI FP/FN, appeal SLA, denylist churn) | `.csv` | Same directory, hashed & signed |
| Dashboard snapshot | Grafana JSON export of `ministry_transparency_overview` panels + alert rules | `.json` | `dashboards/grafana/ministry_transparency_overview.json` / `dashboards/alerts/ministry_transparency_rules.yml` |
| Provenance manifest | Norito manifest tying digests, SoraFS CID, signatures, release timestamp | `.json` + detached signature | `artifacts/ministry/transparency/<YYYY-Q>/manifest.json(.sig)` (also attached to governance vote) |

## Data Sources & Pipeline

| Source | Feed | Notes |
|--------|------|-------|
| Moderation ledger (`docs/source/sorafs_transparency_plan.md`) | Hourly `ModerationLedgerBlockV1` exports stored in CAR files | Already live for SFM-4c; reused for quarterly aggregation. |
| AI calibration + false-positive rates | `docs/source/sorafs_ai_moderation_plan.md` fixtures + calibration manifests (`docs/examples/ai_moderation_calibration_*.json`) | Metrics aggregated per policy, region, and model profile. |
| Appeal register | Norito `AppealCaseV1` events emitted by MINFO-7 treasury tooling | Contains stake transfers, panel roster, SLA timers. |
| Denylist churn | `MinistryDenylistChangeV1` events from the Merkle registry (MINFO-6) | Includes hash families, TTL, emergency canon flags. |
| Treasury flows | `MinistryTreasuryTransferV1` events (appeal deposits, panel rewards) | Balanced against `finance/mminfo_gl.csv`. |

Emergency canon governance, TTL limits, and review requirements now live in
[`docs/source/ministry/emergency_canon_policy.md`](emergency_canon_policy.md), ensuring
that the churn metrics capture the tier (`standard`, `emergency`, `permanent`), canon id,
and review deadlines that Torii enforces at load time.

Processing stages:
1. **Ingest** raw events into `ministry_transparency_ingest` (Rust service mirroring the transparency ledger ingestor). Runs nightly, idempotent.
2. **Aggregate** per quarter with `ministry_transparency_builder`. Outputs the Norito data appendix plus per-metric tables before privacy filters.
3. **Sanitize** metrics via `cargo xtask ministry-transparency sanitize` (or `scripts/ministry/dp_sanitizer.py`) and emit CSV/JSON slices with metadata.
4. **Package** artefacts, sign them with `ministry_release_signer`, and upload to SoraFS + governance DAG.

## 2026-Q3 Reference Release

- The inaugural governance-gated bundle (2026-Q3) was produced on 2026‑10‑07 via `make check-ministry-transparency`. Artefacts live in `artifacts/ministry/transparency/2026-Q3/`—including `sanitized_metrics.json`, `dp_report.json`, `summary.md`, `checksums.sha256`, `transparency_manifest.json`, and `transparency_release_action.json`—and were mirrored to SoraFS CID `7f4c2d81a6b13579ccddeeff00112233`.
- Publication details, metrics tables, and approvals are captured in `docs/source/ministry/reports/2026-Q3.md`, which now serves as the canonical reference for auditors reviewing the Q3 window.
- CI applies `ci/check_ministry_transparency.sh` / `make check-ministry-transparency` before releases leave staging, verifying the artefact digests, Grafana/alert hashes, and manifest metadata so every future quarter follows the same evidence trail.

## Metrics & Dashboards

The Grafana dashboard (`dashboards/grafana/ministry_transparency_overview.json`) exposes the following panels:

- AI moderation accuracy: per-model FP/FN rate, drift vs calibration target, and alert thresholds tied to `docs/source/sorafs/reports/ai_moderation_calibration_*.md`.
- Appeal lifecycle: submissions, SLA compliance, reversals, bond burns, per-tier backlog.
- Denylist churn: additions/removals per hash family, TTL expirations, emergency canon invocations.
- Volunteer briefs & panel diversity: submissions per language, conflict-of-interest disclosures, publication lag. Balanced brief fields are specified in `docs/source/ministry/volunteer_brief_template.md`, ensuring fact tables and moderation tags are machine readable.
- Treasury balances: deposits, payouts, outstanding liability (feeds MINFO-7).

Alert rules (codified in `dashboards/alerts/ministry_transparency_rules.yml`) cover:
- FP/FN deviation >25% versus calibration baseline.
- Appeal SLA miss rate >5% per quarter.
- Emergency canon TTLs older than policy.
- Publication lag >14 days after quarter close.

## Privacy & Release Guardrails (MINFO-8a)

| Metric class | Mechanism | Parameters | Additional guards |
|--------------|-----------|------------|-------------------|
| Counts (appeals, blacklist changes, volunteer briefs) | Laplace noise | ε = 0.75 per quarter, δ = 1e-6 | Suppress buckets with post-noise value <5; clip contributions to 1 per actor per quarter. |
| AI accuracy | Gaussian noise on numerator/denominator | ε = 0.5, δ = 1e-6 | Release only when sanitized sample count ≥ 50 (the `min_accuracy_samples` floor) and publish the confidence interval. |
| Treasury flows | No noise (already public on-chain) | — | Mask account names except treasury IDs; include Merkle proofs. |

Release requirements:
- Differential privacy reports include the epsilon/delta ledger and RNG seed commitment (`blake3(seed)`).
- Sensitive examples (evidence hashes) redacted unless already in public Merkle receipts.
- Redaction log appended to the summary describing all removed fields and justification.

## Publishing Workflow & Timeline

| T‑Window | Task | Owner(s) | Evidence |
|----------|------|----------|----------|
| T + 3 d after quarter close | Freeze raw exports, trigger aggregation job | Ministry Observability TL | `ministry_transparency_ingest.log`, pipeline job ID |
| T + 7 d | Review raw metrics, run DP sanitizer dry run | Data Trust team | Sanitizer report (`artifacts/.../dp_report.json`) |
| T + 10 d | Draft summary + data appendix | Docs/DevRel + Policy analyst | `docs/source/ministry/reports/<YYYY-Q>.md` |
| T + 12 d | Sign artefacts, produce manifest, upload to SoraFS | Ops / Governance Secretariat | `manifest.json(.sig)`, SoraFS CID |
| T + 14 d | Publish dashboards + alerts, post governance announcement | Observability + Comms | Grafana export, alert rule hash, governance vote link |

Each release must be approved by:
1. Ministry Observability TL (data integrity)
2. Governance Council liaison (policy)
3. Docs/Comms lead (public wording)

## Automation & Evidence Storage

- Use `cargo xtask ministry-transparency ingest` to build the quarterly snapshot from the raw feeds (ledger, appeals, denylist, treasury, volunteer). Follow up with `cargo xtask ministry-transparency build` to emit the dashboard metrics JSON plus the signed manifest before publishing.
- Red-team linkage: pass one or more `--red-team-report docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` files to the ingest step so the transparency snapshot and sanitized metrics carry drill IDs, scenario classes, evidence bundle paths, and dashboard SHAs alongside the ledger/appeal/denylist data. This keeps MINFO-9 drill cadence reflected in every transparency packet without manual edits.
- Volunteer submissions must follow `docs/source/ministry/volunteer_brief_template.md` (example: `docs/examples/ministry/volunteer_brief_template.json`). The ingest step expects a JSON array of those objects, automatically filters `moderation.off_topic` entries, enforces disclosure attestations, and records fact-table coverage so dashboards can highlight missing citations.
- Additional automation lives under `scripts/ministry/`. `dp_sanitizer.py` wraps the `cargo xtask ministry-transparency sanitize` command, while `transparency_release.py` (added alongside the provenance tooling) now packages artefacts, derives the SoraFS CID from the `sorafs_cli car pack|proof verify` summary (or an explicit `--sorafs-cid`), and writes both `transparency_manifest.json` and `transparency_release_action.json` (a `TransparencyReleaseV1` governance payload capturing the manifest digest, SoraFS CID, and dashboards git SHA). Pass `--governance-dir <path>` to `transparency_release.py` (or run `cargo xtask ministry-transparency anchor --action artifacts/.../transparency_release_action.json --governance-dir <path>`) to encode the Norito payload and drop it (plus the JSON summary) into the governance DAG directory before publishing. The same flag also emits a `MinistryTransparencyHeadUpdateV1` request under `<governance-dir>/publisher/head_requests/ministry_transparency/`, referencing the quarter, SoraFS CID, manifest paths, and IPNS key alias (overridable via `--ipns-key`). Provide `--auto-head-update` to process that request immediately via `publisher_head_updater.py`, optionally passing `--head-update-ipns-template '/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}'` when IPNS needs to be published at the same time. Otherwise, run `scripts/ministry/publisher_head_updater.py --governance-dir <path>` later (with the same template if needed) to drain the queue, append `publisher/head_updates.log`, update `publisher/ipns_heads/<key>.json`, and archive the processed JSON under `head_requests/ministry_transparency/processed/`.
- Artefacts stored under `artifacts/ministry/transparency/<YYYY-Q>/` with a `checksums.sha256` file signed by the release key. The tree now carries a reference bundle at `artifacts/ministry/transparency/2026-Q3/` (sanitized metrics, DP report, summary, manifest, governance action) so engineers can test the tooling offline, and `scripts/ministry/check_transparency_release.py` verifies the digests/quarter metadata locally while `ci/check_ministry_transparency.sh` runs the same validation in CI before evidence is uploaded. The checker now enforces the documented DP budgets (ε≤0.75 for counts, ε≤0.5 for accuracy, δ≤1e−6) and fails the build whenever `min_accuracy_samples` or the suppression threshold drift, or when a bucket leaks a value below those floors without being suppressed. Treat the script as the contract between the roadmap (MINFO‑8) and CI: adjust both the table above and the checker together if the privacy parameters ever change.
- Governance anchor: create `TransparencyReleaseV1` action referencing the manifest digest, SoraFS CID, and dashboard git SHA (`iroha_data_model::ministry::TransparencyReleaseV1` defines the canonical payload).

## Open Tasks & Next Steps

| Task | Status | Notes |
|------|--------|-------|
| Implement `ministry_transparency_ingest` + builder jobs | 🈺 In Progress | `cargo xtask ministry-transparency ingest|build` now stitches ledger/appeal/denylist/treasury feeds; remaining work wires the DP sanitizer + release script pipeline. |
| Publish Grafana dashboard + alert pack | 🈴 Completed | Dashboard + alert files live under `dashboards/grafana/ministry_transparency_overview.json` and `dashboards/alerts/ministry_transparency_rules.yml`; wire them into PagerDuty `ministry-transparency` during rollout. |
| Automate DP sanitizer + provenance manifest | 🈴 Completed | `cargo xtask ministry-transparency sanitize` (wrapper: `scripts/ministry/dp_sanitizer.py`) emits the sanitized metrics + DP report, and `scripts/ministry/transparency_release.py` now writes `checksums.sha256` plus `transparency_manifest.json` for provenance. |
| Create quarterly report template (`reports/<YYYY-Q>.md`) | 🈴 Completed | Template added at `docs/source/ministry/reports/2026-Q3-template.md`; copy/rename per quarter and replace the `{{...}}` tokens before publishing. |
| Wire governance DAG anchoring | 🈴 Completed | `TransparencyReleaseV1` lives in `iroha_data_model::ministry`, `scripts/ministry/transparency_release.py` emits the JSON payload, and `cargo xtask ministry-transparency anchor` encodes the `.to` artefact into the configured governance DAG directory so the publisher can ingest releases automatically. |

Delivering the document, dashboard spec, and workflow moves MINFO-8 from 🈳 to 🈺. Remaining engineering tasks (jobs, scripts, alert wiring) are tracked in the table above and should close before the first Q3 2026 publication.
