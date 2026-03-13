---
id: payment-settlement-plan
lang: fr
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plan de paiement et de reglement SNS

> Source canonique: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

La tache du roadmap **SN-5 -- Payment & Settlement Service** introduit une
couche de paiement deterministe pour le Sora Name Service. Chaque enregistrement,
renouvellement ou remboursement doit emettre un payload Norito structure afin que
la tresorerie, les stewards et la gouvernance puissent rejouer les flux financiers
sans feuilles de calcul. Cette page condense la spec pour les publics du portail.

## Modele de revenus

- Le fee de base (`gross_fee`) derive de la matrice de prix du registrar.
- La tresorerie recoit `gross_fee x 0.70`, les stewards recoivent le reste moins
  les bonuses de referral (plafonnes a 10 %).
- Des holdbacks optionnels permettent a la gouvernance de suspendre les paiements
  des stewards pendant les litiges.
- Les bundles de settlement exposent un bloc `ledger_projection` avec des ISI
  `Transfer` concrets afin que l'automatisation puisse publier les mouvements XOR
  directement dans Torii.

## Services et automatisation

| Composant | Objectif | Evidence |
|-----------|----------|----------|
| `sns_settlementd` | Applique la politique, signe les bundles, expose `/v2/sns/settlements`. | Bundle JSON + hash. |
| Settlement queue & writer | File idempotente + soumissionnaire de ledger pilote par `iroha_cli app sns settlement ledger`. | Manifeste bundle hash <-> tx hash. |
| Reconciliation job | Diff quotidien + releve mensuel sous `docs/source/sns/reports/`. | Markdown + digest JSON. |
| Refund desk | Remboursements approuves par la gouvernance via `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

Les helpers CI reproduisent ces flux:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Observabilite et reporting

- Dashboards: `dashboards/grafana/sns_payment_settlement.json` pour les totaux
  tresorerie vs stewards, les paiements de referral, la profondeur de file et la
  latence de remboursement.
- Alertes: `dashboards/alerts/sns_payment_settlement_rules.yml` surveille l'age
  pending, les echecs de reconciliation et la derive du ledger.
- Releves: les digests quotidiens (`settlement_YYYYMMDD.{json,md}`) se reunissent
  en rapports mensuels (`settlement_YYYYMM.md`) qui sont charges sur Git et dans
  le stockage d'objets de gouvernance (`s3://sora-governance/sns/settlements/<period>/`).
- Les packets de gouvernance regroupent dashboards, logs CLI et approbations
  avant le sign-off du council.

## Checklist de rollout

1. Prototyper les helpers de quote + ledger et capturer un bundle de staging.
2. Lancer `sns_settlementd` avec queue + writer, brancher les dashboards et
   executer les tests d'alertes (`promtool test rules ...`).
3. Livrer le helper de remboursement plus le modele de releve mensuel; mirroir
   les artefacts dans `docs/portal/docs/sns/reports/`.
4. Executer une repetition partenaire (un mois complet de settlements) et
   capturer le vote de gouvernance marquant SN-5 comme termine.

Se referer au document source pour les definitions exactes du schema, les
questions ouvertes et les amendements futurs.
