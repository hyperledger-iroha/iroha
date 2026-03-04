---
lang: fr
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

# Reconciliation de facturation du gateway SoraGlobal

- **Fenetre:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **Version de catalogue:** `<catalog-version>`
- **Snapshot d'usage:** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, seuil d'alerte `<alert-threshold>%`
- **Payeur -> Tresorerie:** `<payer>` -> `<treasury>` dans `<asset-definition>`
- **Total du:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Verifications des line items
- [ ] Les entrees d'usage couvrent uniquement les ids de metre du catalogue et les regions de facturation valides
- [ ] Les unites de quantite correspondent aux definitions du catalogue (requests, GiB, ms, etc.)
- [ ] Multiplicateurs de region et niveaux de remise appliques selon le catalogue
- [ ] Les exports CSV/Parquet correspondent aux line items de la facture JSON

## Evaluation des guardrails
- [ ] Seuil d'alerte soft cap atteint? `<yes/no>` (joindre la preuve d'alerte si yes)
- [ ] Hard cap depasse? `<yes/no>` (si yes, joindre l'approbation override)
- [ ] Plancher minimum de facture satisfait

## Projection du ledger
- [ ] Total du lot de transferts egal a `total_micros` dans la facture
- [ ] La definition d'asset correspond a la devise de facturation
- [ ] Les comptes payeur et tresorerie correspondent au tenant et a l'operateur enregistre
- [ ] Artefacts Norito/JSON attaches pour replay d'audit

## Notes de litige/ajustement
- Variation observee: `<variance detail>`
- Ajustement propose: `<delta and rationale>`
- Preuves a l'appui: `<logs/dashboards/alerts>`

## Approbations
- Analyste de facturation: `<name + signature>`
- Relecteur tresorerie: `<name + signature>`
- Hash du paquet governance: `<hash/reference>`
