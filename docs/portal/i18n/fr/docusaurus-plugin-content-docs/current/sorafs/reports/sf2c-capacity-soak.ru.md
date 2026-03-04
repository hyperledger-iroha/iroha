---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ouvrir le trempage des composants SF-2c

Données : 2026-03-21

## Область

Cela signifie que les tests de détermination du solvant de trempage des substances SoraFS et votre plaque,
запрошенные в дорожной carte SF-2c.

- **Trempage multi-fournisseurs de 30 jours :** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic`
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Exploitez vos fournisseurs, obtenez un règlement en 30 heures et
  vérifiez que ce grand livre est compatible avec votre intégralité
  projet. Testez Blake3 digest (`capacity_soak_digest=...`), par CI
  Je peux créer et créer un instantané canonique.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (тот же файл). Testez ce que sont les frappes, les temps de recharge, les barres obliques
  les garanties et les registres des comptes sont déterminés.

## Выполнение

Appliquez les ingrédients de trempage localement :

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Les tests s'effectuent pendant une seconde sur le ordinateur de bureau standard et ne génèrent aucun problème
внешних luminaires.

## Наблюдаемость

Torii permet d'obtenir des instantanés des fournisseurs de crédit dans les registres de frais, par exemple
Les tableaux de bord comprennent la porte de l'équilibrage national et les pénalités :

- REST : `GET /v1/sorafs/capacity/state` возвращает записи `credit_ledger[*]`,
  которые отражают поля ledger, проверенные в trempage teste. См.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importer Grafana : `dashboards/grafana/sorafs_capacity_penalties.json` строит
  Grèves de grève des exportations, sommes des travailleurs et des garanties, choses
  Vous devez utiliser la commande pour effectuer le trempage de base avec vos tâches.

## Дальнейшие шаги

- Planifiez les programmes de porte spécifiques au CI pour le test de trempage (niveau de fumée).
- Fixez le panneau Grafana pour gratter Torii après l'installation de la télémétrie des exportations dans le cadre de la production.