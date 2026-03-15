---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-11-10T16:20:18.769519+00:00"
translation_last_reviewed: 2026-01-30
---

# Rapport de soak d'accumulation de capacité SF-2c

Date: 2026-03-21

## Portée

Ce rapport consigne les tests déterministes de soak d'accumulation et de paiement de capacité SoraFS demandés
dans la feuille de route SF-2c.

- **Soak multi-provider sur 30 jours:** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` dans
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Le harness instancie cinq providers, couvre 30 fenêtres de settlement et
  valide que les totaux du ledger correspondent à une projection de référence
  calculée indépendamment. Le test émet un digest Blake3 (`capacity_soak_digest=...`)
  afin que CI puisse capturer et comparer le snapshot canonique.
- **Pénalités de sous-livraison:** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (même fichier). Le test confirme que les seuils de strikes, cooldowns,
  slashes de collateral et compteurs du ledger restent déterministes.

## Exécution

Relancez les validations de soak localement avec:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les tests se terminent en moins d'une seconde sur un laptop standard et ne
nécessitent aucun fixture externe.

## Observabilité

Torii expose maintenant des snapshots de crédit providers aux côtés des fee ledgers afin que les dashboards
puissent gate sur les faibles soldes et penalty strikes:

- REST: `GET /v1/sorafs/capacity/state` renvoie des entrées `credit_ledger[*]` qui
  reflètent les champs du ledger vérifiés dans le test de soak. Voir
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Import Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` trace les
  compteurs de strikes exportés, les totaux de pénalités et le collateral engagé afin que
  l'équipe on-call puisse comparer les baselines de soak avec les environnements live.

## Suivi

- Planifier des exécutions hebdomadaires de gate en CI pour rejouer le test de soak (smoke-tier).
- Étendre le tableau Grafana avec les cibles de scrape Torii une fois que les exports de telemetry de production
  seront en ligne.
