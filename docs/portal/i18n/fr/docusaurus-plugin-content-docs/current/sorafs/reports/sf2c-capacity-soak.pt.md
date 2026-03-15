---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rapport de trempage d'accumulation de capacité SF-2c

Données : 2026-03-21

## Escopo

Ce rapport enregistre les testicules déterminants pour le trempage de l'accumulation et le paiement de la capacité SoraFS
sollicités pour le projet SF-2c sur la feuille de route.

- **Soak multi-provider de 30 jours :** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O exploiter l'instance de cinq fournisseurs, cobre 30 janvier de règlement e
  valida que les totaux du grand livre correspondent à un projet de référence
  calculé de forme indépendante. Le teste émet un condensé Blake3
  (`capacity_soak_digest=...`) pour que CI puisse capturer et comparer un instantané
  canonique.
- **Pénalités pour subentrega :** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mesmo archivage). Le test confirme les limites des frappes, des temps de recharge et des barres obliques.
  de collatéral et de contadores du grand livre de manière permanente.

## Exécution

Exécuter en tant que validateurs de trempage localement avec :

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les testicules sont complets au moins une seconde fois sur un ordinateur portable et n'en ont pas besoin
luminaires externes.

## Observabilité

Torii permet d'afficher des instantanés de crédit des fournisseurs avec les registres de frais pour les tableaux de bord
possam fazer gate em saldos baixos et penalty strikes :

- REST : `GET /v2/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  reflètent les champs du grand livre vérifiés lors du test de trempage. Véja
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importation Grafana : `dashboards/grafana/sorafs_capacity_penalties.json` trace os
  contadores de strikes exportados, totais de penalidades e collatéral preso para que o
  le temps de garde peut comparer les lignes de base de trempage avec les ambiances de production.

## Suivi

- Programmer les exécutions sémanales de la porte en CI pour réexécuter le test de trempage (niveau de fumée).
- Estender o paintel Grafana com alvos de scrape de Torii assim que as exportacoes de telemetry de producao entrarem em operacao.