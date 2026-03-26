---
lang: zh-hant
direction: ltr
source: docs/source/sns/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 629bfac8c15830944ea267addc8eb45ccf6bac42be7545003749fedb4e91d975
source_last_modified: "2026-01-28T17:11:30.740548+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Sora Name Service Governance Playbook
summary: Runbook for council, guardian, steward, and registrar workflows referenced by SN-1/SN-6.
---

# Sora Name Service Governance Playbook (SN-6)

**Status:** Drafted 2026-03-24 — living reference for SN-1/SN-6 readiness  
**Roadmap links:** SN-6 “Compliance & Dispute Resolution”, SN-7 “Resolver & Gateway Sync”, ADDR-1/ADDR-5 address policy  
**Prerequisites:** Registry schema in [`registry_schema.md`](./registry_schema.md), registrar API contract in [`registrar_api.md`](./registrar_api.md), address UX guidance in [`address_display_guidelines.md`](./address_display_guidelines.md), account structure rules in [`../account_structure.md`](../../account_structure.md), and the collision/telemetry audit in [`address_security_review.md`](./address_security_review.md).

This playbook describes how the Sora Name Service (SNS) governance bodies adopt
charters, approve registrations, escalate disputes, and prove that resolver and
gateway states remain in sync. It fulfils the roadmap requirement that the
`sns governance ...` CLI, Norito manifests, and audit artefacts share a single
operator-facing reference before N1 (public launch).

## 1. Scope & Audience

The document targets:

- Governance Council members who vote on charters, suffix policies, and dispute
  outcomes.
- Guardian board members who issue emergency freezes and review reversals.
- Suffix stewards who run the registrar queues, approve auctions, and manage
  revenue splits.
- Resolver/gateway operators responsible for SoraDNS propagation, GAR updates,
  and telemetry guardrails.
- Compliance, treasury, and support teams who must demonstrate that every
  governance action left auditable Norito artefacts.

It covers the closed-beta (N0), public launch (N1), and expansion (N2) phases
enumerated in `roadmap.md` by linking each workflow to required evidence,
dashboards, and escalation paths.

## 2. Roles & Contact Map

| Role | Core responsibilities | Primary artefacts & telemetry | Escalation |
|------|----------------------|-------------------------------|------------|
| Governance Council | Draft and ratify charters, suffix policies, dispute verdicts, and steward rotations. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, council ballots stored via `sns governance charter submit`. | Council chair + governance docket tracker. |
| Guardian Board | Issue soft/hard freezes, emergency canons, and 72 h reviews. | Guardian tickets emitted by `sns governance freeze`, override manifests logged under `artifacts/sns/guardian/*`. | Guardian on-call rotation (≤15 min ACK). |
| Suffix Stewards | Run registrar queues, auctions, pricing tiers, and customer comms; acknowledge compliances. | Steward policies in `SuffixPolicyV1`, pricing reference sheets, steward acknowledgements stored beside regulatory memos. | Steward program lead + suffix-specific PagerDuty. |
| Registrar & Billing Ops | Operate `/v1/sns/*` endpoints, reconcile payments, emit telemetry, and maintain CLI snapshots. | Registrar API (`registrar_api.md`), `sns_registrar_status_total` metrics, payment proofs archived under `artifacts/sns/payments/*`. | Registrar duty manager and treasury liaison. |
| Resolver & Gateway Operators | Keep SoraDNS, GAR, and gateway state aligned with registrar events; stream transparency metrics. | [`../soradns/deterministic_hosts.md`](../soradns/deterministic_hosts.md), [`../reports/soradns_transparency.md`](../reports/soradns_transparency.md), [`../soradns_ir_playbook.md`](../soradns_ir_playbook.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE on-call + gateway ops bridge. |
| Treasury & Finance | Apply 70/30 revenue split, referral carve-outs, tax/treasury filings, and SLA attestations. | Revenue accrual manifests, Stripe/treasury exports, quarterly KPI appendices under `docs/source/sns/regulatory/`, and the settlement workflows defined in [`payment_settlement_plan.md`](./payment_settlement_plan.md). | Finance controller + compliance officer. |
| Compliance & Regulatory Liaison | Track global obligations (EU DSA, etc.), update KPI covenants, and file disclosures. | Regulatory memos in `docs/source/sns/regulatory/`, reference decks, `ops/drill-log.md` entries for tabletop rehearsals. | Compliance program lead. |
| Support / SRE On-call | Handle incidents (collisions, billing drift, resolver outages), coordinate customer messaging, and own runbooks. | Incident templates, `ops/drill-log.md`, staged lab evidence, Slack/war-room transcripts archived under `incident/`. | SNS on-call rotation + SRE management. |

## 3. Canonical Artefacts & Data Sources

| Artefact | Location | Purpose |
|----------|----------|---------|
| Charter + KPI addenda | `docs/source/sns/governance_addenda/` | Version-controlled signed charters, KPI covenants, and governance decisions referenced by CLI votes. |
| Registry schema | [`registry_schema.md`](./registry_schema.md) | Canonical Norito structures (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Registrar contract | [`registrar_api.md`](./registrar_api.md) | REST/gRPC payloads, `sns_registrar_status_total` metrics, and governance hook expectations. |
| Address UX guide | [`address_display_guidelines.md`](./address_display_guidelines.md) | Canonical Katakana i105 renderings mirrored by wallets/explorers. |
| Address security review | [`address_security_review.md`](./address_security_review.md) | Collision math, Local-8 telemetry references, checksum guidance, and manifest immutability evidence required for ADDR-7. |
| SoraDNS / GAR docs | [`../soradns/deterministic_hosts.md`](../soradns/deterministic_hosts.md), [`../reports/soradns_transparency.md`](../reports/soradns_transparency.md) | Deterministic host derivation, transparency tailer workflow, and alert rules. |
| Regulatory memos | `docs/source/sns/regulatory/` | Jurisdictional intake notes (e.g., EU DSA), steward acknowledgements, template annexes. |
| Drill log | `ops/drill-log.md` | Record of chaos and IR rehearsals required before phase exits. |
| Artefact storage | `artifacts/sns/` | Payment proofs, guardian tickets, resolver diffs, KPI exports, and signed CLI output produced by `sns governance ...`. |

All governance actions must reference at least one artefact in the table above
so auditors can reconstruct the decision trail within 24 hours.

## 4. Lifecycle Playbooks

### 4.1 Charter & Steward Motions

| Step | Owner | CLI / Evidence | Notes |
|------|-------|----------------|-------|
| Draft addendum & KPI deltas | Council rapporteur + steward lead | Markdown template stored under `docs/source/sns/governance_addenda/YY/` | Include KPI covenant IDs, telemetry hooks, and activation conditions. |
| Submit proposal | Council chair | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produces `CharterMotionV1`) | CLI emits Norito manifest stored under `artifacts/sns/governance/<id>/charter_motion.json`. |
| Vote & guardian acknowledgement | Council + guardians | `sns governance ballot cast --proposal <id>` and `sns governance guardian-ack --proposal <id>` | Attach hashed minutes and quorum proofs. |
| Steward acceptance | Steward program | `sns governance steward-ack --proposal <id> --signature <file>` | Required before suffix policies change; record envelope under `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activation | Registrar ops | Update `SuffixPolicyV1`, refresh registrar caches, publish note in `status.md`. | Activation timestamp logged to `sns_governance_activation_total`. |
| Audit log | Compliance | Append entry to `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` and drill log if tabletop performed. | Include references to telemetry dashboards and policy diffs. |

### 4.2 Registration, Auction & Pricing Approvals

1. **Preflight:** Registrar queries `SuffixPolicyV1` to confirm pricing tier,
   available terms, and grace/redemption windows. Keep pricing sheets synced to
   the 3/4/5/6–9/10+ tier table (base tier + suffix coefficients) documented in
   the roadmap.
2. **Sealed-bid auctions:** For premium pools, run the 72 h commit / 24 h reveal
   cycle via `sns governance auction commit` / `... reveal`. Publish the commit
   list (hashes only) under `artifacts/sns/auctions/<name>/commit.json` so
   auditors can verify randomness.
3. **Payment verification:** Registrars validate `PaymentProofV1` against
   treasury splits (70% treasury / 30% steward with ≤10% referral carve-out).
   Store the Norito JSON under `artifacts/sns/payments/<tx>.json` and link it in
   the registrar response (`RevenueAccrualEventV1`).
4. **Governance hook:** Attach `GovernanceHookV1` for premium/guarded names
   referencing council proposal ids and steward signatures. Missing hooks result
   in `sns_err_governance_missing`.
5. **Activation + resolver sync:** Once Torii emits the registry event, trigger
   the resolver transparency tailer to confirm the new GAR/zone state propagated
   (see §4.5).
6. **Customer disclosure:** Update the customer-facing ledger (wallet/explorer)
   via the shared fixtures in `address_display_guidelines.md`, ensuring i105 and
   canonical Katakana i105 renderings match copy/QR guidance.

### 4.3 Renewals, Billing & Treasury Reconciliation

- **Renewal workflow:** Registrars enforce the 30 day grace + 60 day redemption
  windows specified in `SuffixPolicyV1`. After 60 days the Dutch reopen sequence
  (7 days, 10× fee decaying 15%/day) triggers automatically via `sns governance
  reopen`.
- **Revenue split:** Each renewal or transfer creates a
  `RevenueAccrualEventV1`. Treasury exports (CSV/Parquet) must reconcile to
  these events daily; attach proofs to `artifacts/sns/treasury/<date>.json`.
- **Referral carve-outs:** Optional referral percentages are tracked per suffix
  by adding `referral_share` to the steward policy. Registrars emit the final
  split and store referral manifests beside the payment proof.
- **Reporting cadence:** Finance posts monthly KPI annexes (registrations,
  renewals, ARPU, dispute/bond utilisation) under
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Dashboards should pull from
  the same exported tables so Grafana numbers match ledger evidence.
- **Monthly KPI review:** On the first Tuesday of every month, the finance lead,
  program PM, and the steward on duty open
  [`dashboards/grafana/sns_suffix_analytics.json`](../../../dashboards/grafana/sns_suffix_analytics.json)
  (surfaced in the portal analytics page) and capture a PDF/CSV export. The
  review compares `sns_registrar_status_total` trends, freeze backlogs, and
  bulk-release revenue panels, logs action items in the KPI annex markdown, and
  files follow-up incidents if SLA breaches are detected. Attach the exported
  Grafana snapshot plus any promtool output to the annex entry so the memo can
  be audited without re-running the dashboard.
- **Annex automation:** Keep `docs/source/sns/regulatory/annex_jobs.json` up to
  date with the suffix/cycle pairs under review and run
  `python3 scripts/run_sns_annex_jobs.py --verbose` (add `--dry-run` for a quick
  preview). The helper invokes `cargo xtask sns-annex` for each entry, copies
  the dashboard export into `artifacts/sns/regulatory/<suffix>/<cycle>/`, and
  updates the matching regulatory memo with a `KPI Dashboard Annex` block so
  evidence stays consistent across cycles.

### 4.4 Freezes, Disputes & Appeals

| Phase | Owner | Action & Evidence | SLA |
|-------|-------|-------------------|-----|
| Soft freeze request | Steward / support | File ticket `SNS-DF-<id>` with payment proofs, dispute bond reference, and affected selector(s). | ≤4 h from intake. |
| Guardian ticket | Guardian board | `sns governance freeze --selector <i105> --reason <text> --until <ts>` produces signed `GuardianFreezeTicketV1`. Store ticket JSON under `artifacts/sns/guardian/<id>.json`. | ≤30 min ACK, ≤2 h execution. |
| Council ratification | Governance council | Approve or reject freezes, document decision link to guardian ticket and dispute bond digest. | Next council session or asynchronous vote. |
| Arbitration panel | Compliance + steward | Convene 7-juror panel (per roadmap) with hashed ballots submitted via `sns governance dispute ballot`. Attach anonymised vote receipts to incident packet. | Verdict ≤7 days after bond deposit. |
| Appeal | Guardian + council | Appeals double the bond and repeat the juror process; record Norito manifest `DisputeAppealV1` and reference primary ticket. | ≤10 days. |
| Unfreeze & remediation | Registrar + resolver ops | Execute `sns governance unfreeze --selector <i105> --ticket <id>`, update registrar status, and propagate GAR/resolver diffs. | Immediately after verdict. |

Emergency canons (guardian-triggered freezes ≤72 h) follow the same flow but
require retroactive council review and a transparency note under
`docs/source/sns/regulatory/`.

### 4.5 Resolver & Gateway Propagation

1. **Event hook:** Every registry event emits to the resolver event stream
   (`tools/soradns-resolver` SSE). Resolver ops subscribe and record diffs via
   the transparency tailer (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **GAR template update:** Gateways must update GAR templates referenced by
   `canonical_gateway_suffix()` and re-sign the `host_pattern` list. Store diffs
   in `artifacts/sns/gar/<date>.patch`.
3. **Zonefile publication:** Use the zonefile skeleton described in
   `roadmap.md` (name, ttl, cid, proof) and push it to Torii/SoraFS. Archive the
   Norito JSON under `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Transparency check:** Run `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   to ensure alerts remain green. Attach the Prometheus text output to the
   weekly transparency report.
5. **Gateway audit:** Record `Sora-*` header samples (cache policy, CSP, GAR
   digest) and attach them to the governance log so operators can prove that the
   gateway served the new name with the intended guardrails.

## 5. Telemetry & Reporting

| Signal | Source | Description / Action |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrar handlers | Success/error counter for registrations, renewals, freezes, transfers; alerts when `result="error"` spikes per suffix. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Torii metrics | Latency SLOs for API handlers; feed dashboards built from `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` & `soradns_bundle_cid_drift_total` | Resolver transparency tailer | Detect stale proofs or GAR drift; guardrails defined in `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Governance CLI | Counter incremented whenever a charter/addendum activates; used to reconcile council decisions vs. published addenda. |
| `guardian_freeze_active` gauge | Guardian CLI | Tracks soft/hard freeze windows per selector; page SRE if value stays `1` beyond declared SLA. |
| KPI annex dashboards | Finance / Docs | Monthly rollups published alongside regulatory memos. The Grafana definition lives in [`dashboards/grafana/sns_suffix_analytics.json`](../../../dashboards/grafana/sns_suffix_analytics.json) and the same panels are embedded in the portal analytics page for stewards and regulators. |

## 6. Evidence & Audit Requirements

| Action | Evidence to archive | Storage |
|--------|--------------------|---------|
| Charter / policy change | Signed Norito manifest, CLI transcript, KPI diff, steward acknowledgement. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registration / renewal | `RegisterNameRequestV1` payload, `RevenueAccrualEventV1`, payment proof. | `artifacts/sns/payments/<tx>.json`, registrar API logs. |
| Auction | Commit/reveal manifests, randomness seed, winner calculation spreadsheet. | `artifacts/sns/auctions/<name>/`. |
| Freeze / unfreeze | Guardian ticket, council vote hash, incident log URL, customer comms template. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolver propagation | Zonefile/GAR diff, tailer JSONL excerpt, Prometheus snapshot. | `artifacts/sns/resolver/<date>/` + transparency reports. |
| Regulatory intake | Intake memo, deadline tracker, steward acknowledgement, KPI change summary. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Phase Gate Checklist

| Phase | Exit criteria | Evidence bundle |
|-------|---------------|-----------------|
| N0 — Closed beta | SN-1/SN-2 registry schema, manual registrar CLI, guardianship drill complete. | Charter motion + steward ACK, registrar dry-run logs, resolver transparency report, drill entry in `ops/drill-log.md`. |
| N1 — Public launch | Auctions + fixed-price tiers live for `.sora`/`.nexus`, self-service registrar, resolver auto-sync, billing dashboards. | Pricing sheet diff, registrar CI results, payment/KPI annex, transparency tailer output, incident rehearsal notes. |
| N2 — Expansion | `.dao`, reseller APIs, dispute portal, steward scorecards, analytics dashboards. | Portal screenshots, dispute SLA metrics, steward scorecard exports, updated governance charter referencing reseller policies. |

Phase exits require recorded tabletop drills (registration happy path, freeze,
resolver outage) with artefacts attached to `ops/drill-log.md`.

## 8. Incident Response & Escalation

| Trigger | Severity | Immediate owner | Mandatory actions |
|---------|----------|-----------------|-------------------|
| Resolver/GAR drift or stale proofs | Sev 1 | Resolver SRE + guardian board | Page resolver on-call, capture tailer output, decide whether to freeze affected names, post status update every 30 min. |
| Registrar outage, billing failure, or widespread API errors | Sev 1 | Registrar duty manager | Halt new auctions, switch to manual CLI, notify stewards/treasury, attach Torii logs to incident doc. |
| Single-name dispute, payment mismatch, or customer escalation | Sev 2 | Steward + support lead | Collect payment proofs, determine if soft freeze needed, respond to requester within SLA, log outcome in dispute tracker. |
| Compliance audit finding | Sev 2 | Compliance liaison | Draft remediation plan, file memo under `docs/source/sns/regulatory/`, schedule follow-up council session. |
| Drill or rehearsal | Sev 3 | Program PM | Execute scripted scenario from `ops/drill-log.md`, archive artefacts, label gaps as roadmap tasks. |

All incidents must create `incident/YYYY-MM-DD-sns-<slug>.md` with ownership
tables, command logs, and references to the evidence produced throughout this
playbook.

## 9. Steward Scorecards & Replacement (SN-9)

- **Automation:** run `cargo xtask sns-scorecard --input <metrics.json> --output-json docs/examples/sns/steward_scorecard_<Q>.json --output-markdown docs/source/sns/reports/steward_scorecard_<Q>.md --handoff-json docs/examples/sns/steward_handoff_<Q>.json --handoff-markdown docs/source/sns/reports/steward_handoff_<Q>.md --handoff-dir docs/examples/sns/handoffs/<Q>` to generate the quarterly bundle. The generator applies the default KPI gates (renewal ≥55 %, support SLA ≥95 %, dispute turnaround ≤168 h) and emits `rotation.level` as `none`, `monitor`, or `replace` based on breaches + guardian freezes.【fixtures/sns/steward_metrics_2026q1.json:1】【docs/examples/sns/steward_scorecard_2026q1.json:1】
- **Artefacts:** publish the JSON in `docs/examples/sns/` (auditors consume the structured data) and the Markdown report in `docs/source/sns/reports/` (council-ready summary). The hand-off outputs add an aggregate packet plus per-suffix files under `docs/examples/sns/handoffs/<Q>/` with deadline stamps and action owners for DAO/council motions. Reference the report in the weekly `status.md` update and attach it to the governance tracker entry for the quarter.【docs/source/sns/reports/steward_scorecard_2026q1.md:1】【docs/examples/sns/handoffs/2026q1/handoff_index.json:1】
- **Hand-offs:** include `--handoff-json docs/examples/sns/steward_handoff_<Q>.json --handoff-markdown docs/source/sns/reports/steward_handoff_<Q>.md --handoff-dir docs/examples/sns/handoffs/<Q>` when running the generator so DAO/council motions share the same evidence, deadlines, and recommended actions recorded in the scorecard; the per-suffix Markdown files (e.g., `docs/examples/sns/handoffs/2026q1/steward_handoff_dao.md`) travel with tickets and DAO briefs.【docs/examples/sns/steward_handoff_2026q1.json:1】【docs/source/sns/reports/steward_handoff_2026q1.md:1】【docs/examples/sns/handoffs/2026q1/steward_handoff_dao.md:1】
- **Escalation:** use the rotation workflow in [`steward_replacement_playbook.md`](./steward_replacement_playbook.md) to manage advisory → escalation → replacement over the 14-day window defined by SN-9. Stage transitions are keyed to the scorecard output (`rotation.level`) and guardian freeze status.
- **Notifications:** Populate the advisory, escalation, and replacement templates from the playbook and archive signed copies under `artifacts/sns/stewards/<suffix>/`. Every notice must cite the scorecard hash so guardians and compliance can replay the exact KPI evidence.
- **Table stakes:** When `rotation.level=replace`, guardians freeze the registrar queue, the council schedules the interim steward vote, and ops posts a public FAQ within 60 minutes. Warnings (`rotation.level=monitor`) require a remediation plan within 5 business days even if no rotation occurs.

## 10. References

- [`registry_schema.md`](./registry_schema.md)
- [`registrar_api.md`](./registrar_api.md)
- [`address_display_guidelines.md`](./address_display_guidelines.md)
- [`../account_structure.md`](../../account_structure.md)
- [`../soradns/deterministic_hosts.md`](../soradns/deterministic_hosts.md)
- [`../reports/soradns_transparency.md`](../reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR sections)

Keep this playbook updated whenever charter wording, CLI surfaces, or telemetry
contracts change; roadmap entries referencing `docs/source/sns/governance_playbook.md`
should always match the latest revision.
