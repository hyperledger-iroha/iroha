---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-2c Capacité Accumulation Trempage

Date: 2026-03-21

## اسکوپ

Le SF-2c fournit des services de paiement et des tests de trempage déterministes SoraFS d'accumulation de capacité et de paiement

- **30 دن کا trempage multi-fournisseurs :**
  `capacity_fee_ledger_30_day_soak_deterministic` en cours
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں چلایا جاتا ہے۔
  exploiter les fournisseurs pour 30 règlements et 30 règlements pour chaque client
  Il s'agit de totaux du grand livre et de projection de référence.
  Il s'agit du résumé Blake3 (`capacity_soak_digest=...`) de l'instantané canonique CI.
  capturer et diff کر سکے۔
- **Pénalités pour sous-livraison :**
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (اسی فائل میں) نافذ کیا جاتا ہے۔ ٹیسٹ تصدیق کرتا ہے کہ atteint les seuils, les temps de recharge et les barres obliques collatérales
  اور compteurs de grand livre déterministes رہتے ہیں۔

## Exécution

validations de trempage لوکل طور پر چلائیں :

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

یہ ٹیسٹ ایک عام لیپ ٹاپ پر ایک سیکنڈ سے کم وقت میں مکمل ہو جاتے ہیں اور بیرونی rencontres کی ضرورت نہیں ہوتی۔

## Observabilité

Torii pour les instantanés de crédit du fournisseur et les grands livres de frais, ainsi que les tableaux de bord, les soldes et les pénalités pour les portes :

- REST : `GET /v1/sorafs/capacity/state` `credit_ledger[*]` entrées pour le test de trempage et la vérification du test
  champs du grand livre دیکھیں
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Import Grafana : compteurs de frappes exportés `dashboards/grafana/sorafs_capacity_penalties.json`, totaux de pénalités,
  Pour les garanties cautionnées et les intrigues, les services de garde et les lignes de base d'immersion, les environnements en direct et les services de garde.

## Suivi

- La porte CI میں ہفتہ وار exécute un test de trempage شیڈول کریں تاکہ دوبارہ چل سکے (niveau de fumée).
- Pour les exportations de télémétrie de production en direct avec la carte Grafana et les cibles de grattage Torii.