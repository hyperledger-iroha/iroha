---
lang: fr
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

# Rapport de transparence d'arbitrage SNS - <Mois YYYY>

- **Suffixe:** `<.sora / .nexus / .dao>`
- **Fenetre de reporting:** `<ISO start>` -> `<ISO end>`
- **Prepare par:** `<Council liaison>`
- **Artefacts sources:** `cases.ndjson` SHA256 `<hash>`, export dashboard `<filename>.json`

## 1. Resume executif

- Total de nouveaux cas: `<count>`
- Cas clos sur la periode: `<count>`
- Conformite SLA: `<ack %>` acknowledgement / `<resolution %>` decision
- Overrides du guardian emis: `<count>`
- Transferts/remboursements effectues: `<count>`

## 2. Repartition des cas

| Type de litige | Nouveaux cas | Cas clos | Resolution mediane (jours) |
|---------------|-------------|----------|----------------------------|
| Propriete | 0 | 0 | 0 |
| Violation de politique | 0 | 0 | 0 |
| Abus | 0 | 0 | 0 |
| Facturation | 0 | 0 | 0 |
| Autre | 0 | 0 | 0 |

## 3. Performance SLA

| Priorite | SLA d'accuse | Atteint | SLA de resolution | Atteint | Violations |
|----------|---------------|--------|------------------|--------|------------|
| Urgent | <= 2 h | 0% | <= 72 h | 0% | 0 |
| Haut | <= 8 h | 0% | <= 10 d | 0% | 0 |
| Standard | <= 24 h | 0% | <= 21 d | 0% | 0 |
| Info | <= 3 d | 0% | <= 30 d | 0% | 0 |

Decrivez les causes racines de toute violation et liez les tickets de remediation.

## 4. Registre des cas

| ID de cas | Selecteur | Priorite | Statut | Resultat | Notes |
|-----------|-----------|----------|--------|----------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standard | Cloture | Confirme | `<summary>` |

Fournissez des notes d'une ligne referencant des faits anonymises ou des liens de votes publics.
Scellez si necessaire et mentionnez les redactions appliquees.

## 5. Actions et remedes

- **Gel / liberation:** `<counts + case ids>`
- **Transferts:** `<counts + assets moved>`
- **Ajustements de facturation:** `<credits/debits>`
- **Suivis de politique:** `<tickets or RFCs opened>`

## 6. Appels et overrides du guardian

Resumez les appels escalades au guardian board, y compris timestamps et decisions
(approve/deny). Liez les enregistrements `sns governance appeal` ou les votes du council.

## 7. Points en attente

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

Joindre NDJSON, exports Grafana et logs CLI references dans ce rapport.
