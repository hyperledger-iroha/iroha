---
lang: ba
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraGlobal Gateway Billing Reconciliation

- **Window:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **Catalog Version:** `<catalog-version>`
- **Usage Snapshot:** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, alert threshold `<alert-threshold>%`
- **Payer -> Treasury:** `<payer>` -> `<treasury>` in `<asset-definition>`
- **Total Due:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Line Item Checks
- [ ] Usage entries cover only catalog meter ids and valid billing regions
- [ ] Quantity units match catalog definitions (requests, GiB, ms, etc.)
- [ ] Region multipliers and discount tiers applied as per catalog
- [ ] CSV/Parquet exports match the JSON invoice line items

## Guardrail Evaluation
- [ ] Soft cap alert threshold reached? `<yes/no>` (attach alert evidence if yes)
- [ ] Hard cap exceeded? `<yes/no>` (if yes, attach override approval)
- [ ] Minimum invoice floor satisfied

## Ledger Projection
- [ ] Transfer batch total equals `total_micros` in invoice
- [ ] Asset definition matches billing currency
- [ ] Payer and treasury accounts match tenant and operator of record
- [ ] Norito/JSON artefacts attached for audit replay

## Dispute/Adjustment Notes
- Observed variance: `<variance detail>`
- Proposed adjustment: `<delta and rationale>`
- Supporting evidence: `<logs/dashboards/alerts>`

## Approvals
- Billing analyst: `<name + signature>`
- Treasury reviewer: `<name + signature>`
- Governance packet hash: `<hash/reference>`
