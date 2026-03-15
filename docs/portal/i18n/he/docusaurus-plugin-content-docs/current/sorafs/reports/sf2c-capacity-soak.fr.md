---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de soak d'accumulation de capacité SF-2c

תאריך: 21-03-2026

## Portée

Ce rapport consigne les tests déterministes de soak d'accumulation et de paiement de capacité SoraFS demandés
dans la feuille de route SF-2c.

- **השרות מרובה ספקים למשך 30 ז':** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` dans
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  ספקי Le Rants instancie cinq, couvre 30 fenêtres de settlement et
  Valide que les Totaux du Ledger correspondent à une Projection de Référence
  calculée indépendamment. Le test émet un digest Blake3 (`capacity_soak_digest=...`)
  אפין que CI puisse capturer and comparer the snapshot canonique.
- **Pénalités de sous-livraison:** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (même fichier). הבדיקה מאשרת את השביתות, ההתקררות,
  חתכים של בטחונות ו-Compteurs du Ledger restent déterministes.

## ביצוע

Relancez les validations de soak localement avec:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les tests se terminent en moins d'une seconde sur un laptop standard et ne
nécessitent aucun מתקן חיצוני.

## יכולת התבוננות

Torii חשוף לתחזוקה של צילומי מצב של ספקי אשראי aux côtés des פנקסי אגרות אפינ que les לוחות מחוונים
שער puissent sur les faibles soldes ועונשין:

- מנוחה: `GET /v2/sorafs/capacity/state` renvoie des entrées `credit_ledger[*]` qui
  Reflètent les champs du Ledger vérifiés dans le test de soak. Voir
  `crates/iroha_torii/src/sorafs/registry.rs`.
- ייבוא Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` עקבות
  compteurs de strikes exportés, les totaux de pénalités et le collateral engagé afin que
  l'équipe ב-call puisse comparer les baselines de soak עם les environnements בשידור חי.

## Suivi

- Planifier des exécutions hebdomadaires de gate en CI pour rejouer le test de soak (עשן-tier).
- Étendre le tableau Grafana avec les cibles de scrape Torii une fois que les exports de telemetry de production
  seront en ligne.