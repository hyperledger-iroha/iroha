---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de trempage d'accumulation de capacité SF-2c

Date : 2026-03-21

## Portée

Ce rapport consigne les tests déterministes de trempage d'accumulation et de paiement de capacité SoraFS demandés
dans la feuille de route SF-2c.

- **Soak multi-provider sur 30 jours:** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` dans
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Le harnais instancie cinq fournisseurs, couvre 30 fenêtres de règlement et
  valider que les totaux du grand livre correspondent à une projection de référence
  sensible. Le test émet un digest Blake3 (`capacity_soak_digest=...`)
  afin que CI puisse capturer et comparer le snapshot canonique.
- **Pénalités de sous-livraison:** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (même fichier). Le test confirme que les seuils de frappe, les temps de recharge,
  Les slashes de collatéral et les compteurs du grand livre restent déterministes.

## Exécution

Relancez les validations de trempage localement avec :

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les tests se terminent en moins d'une seconde sur un ordinateur portable standard et ne
nécessaire aucun luminaire externe.

## Observabilité

Torii expose maintenant des instantanés de fournisseurs de crédit aux côtés des ledgers de frais afin que les tableaux de bord
peut gate sur les faibles soldes et penalty strikes:

- REST : `GET /v1/sorafs/capacity/state` renvoyer des entrées `credit_ledger[*]` qui
  faire les champs du grand livre vérifiés dans le test de trempage. Voir
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importer Grafana : fichiers de trace `dashboards/grafana/sorafs_capacity_penalties.json`
  compteurs de grèves exportés, les totaux de pénalités et le collatéral engagé afin que
  l'équipe de garde peut comparer les lignes de base de trempage avec les environnements live.

## Suivi

- Planifier des exécutions hebdomadaires de gate en CI pour rejouer le test de trempage (smoke-tier).
- Étendre le tableau Grafana avec les cibles de scrape Torii une fois que les exports de télémétrie de production
  seront en ligne.