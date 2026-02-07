---
lang: kk
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-12-29T18:16:35.076964+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
# SNS Arbitration Transparency Report — <Month YYYY>

- **Suffix:** `<.sora / .nexus / .dao>`
- **Reporting window:** `<ISO start>` → `<ISO end>`
- **Prepared by:** `<Council liaison>`
- **Source artefacts:** `cases.ndjson` SHA256 `<hash>`, dashboard export `<filename>.json`

## 1. Executive Summary

- Total new cases: `<count>`
- Cases closed this period: `<count>`
- SLA compliance: `<ack %>` acknowledge / `<resolution %>` decision
- Guardian overrides issued: `<count>`
- Transfers/refunds executed: `<count>`

## 2. Case Mix

| Dispute type | New cases | Closed cases | Median resolution (days) |
|--------------|-----------|--------------|--------------------------|
| Ownership | 0 | 0 | 0 |
| Policy violation | 0 | 0 | 0 |
| Abuse | 0 | 0 | 0 |
| Billing | 0 | 0 | 0 |
| Other | 0 | 0 | 0 |

## 3. SLA Performance

| Priority | Acknowledge SLA | Achieved | Resolution SLA | Achieved | Breaches |
|----------|-----------------|----------|----------------|----------|----------|
| Urgent | ≤ 2 h | 0% | ≤ 72 h | 0% | 0 |
| High | ≤ 8 h | 0% | ≤ 10 d | 0% | 0 |
| Standard | ≤ 24 h | 0% | ≤ 21 d | 0% | 0 |
| Info | ≤ 3 d | 0% | ≤ 30 d | 0% | 0 |

Describe root causes for any breaches and link to remediation tickets.

## 4. Case Register

| Case ID | Selector | Priority | Status | Outcome | Notes |
|---------|----------|----------|--------|---------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standard | Closed | Upheld | `<summary>` |

Provide one-line notes referencing anonymised facts or public vote links. Seal
where required and mention redactions applied.

## 5. Actions & Remedies

- **Freezes / releases:** `<counts + case ids>`
- **Transfers:** `<counts + assets moved>`
- **Billing adjustments:** `<credits/debits>`
- **Policy follow-ups:** `<tickets or RFCs opened>`

## 6. Appeals & Guardian Overrides

Summarise any appeals escalated to the guardian board, including timestamps and
decisions (approve/deny). Link to `sns governance appeal` records or council
votes.

## 7. Outstanding Items

- `<Action item>` — Owner `<name>`, ETA `<date>`
- `<Action item>` — Owner `<name>`, ETA `<date>`

Attach NDJSON, Grafana exports, and CLI logs referenced in this report.
