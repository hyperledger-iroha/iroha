---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Informe de trempage d’accumulation de capacité SF-2c

Date : 2026-03-21

## Alcance

Cette information enregistre les essais déterministes de trempage d'accumulation de capacité SoraFS et les pages
sollicitées sous la route SF-2c.

- **Soak multi-provider de 30 jours :** Exécuté par
  `capacity_fee_ledger_30_day_soak_deterministic` fr
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Le harnais instancia 5 fournisseurs, abarca 30 ventanas de règlement y
  valida que les totaux du grand livre coïncident avec une projection de référence
  calculé de forme indépendante. La teste émet un résumé Blake3
  (`capacity_soak_digest=...`) pour que CI puisse capturer et comparer l'instantané
  canonique.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mismo archivo). La vérification confirme que les ombrelles de frappes, les temps de recharge,
  barres obliques de garantie et contadores du grand livre déterminants permanents.

## Éjection

Exécutez les validations de trempage localement avec :

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les essais sont terminés au moins une seconde sur un portable standard et ne sont pas requis
luminaires externes.

## Observabilité

Torii expose maintenant les instantanés de crédit des fournisseurs avec les registres de frais pour les tableaux de bord
Vous pouvez accéder aux coups bas et aux tirs au but :

- REST : `GET /v2/sorafs/capacity/state` devuelve entradas `credit_ledger[*]` que
  reflejan los campos del ledger verificados en la prueba de trempage. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importation de Grafana : `dashboards/grafana/sorafs_capacity_penalties.json` graphique
  contadores de strikes exportados, totals de penalizaciones y collatéraux en garantie pour que le
  equipo on-call peut comparer les lignes de base de trempage avec les événements en vivo.

## Suite

- Programmer les émissions des portes sémanales en CI pour relancer la vérification du trempage (niveau de fumée).
- Extension du tableau de Grafana avec objets de grattage de Torii lors des exportations de
  la télémétrie de production est disponible.