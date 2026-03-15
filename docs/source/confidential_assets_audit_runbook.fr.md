---
lang: fr
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Playbook d'audit et d'exploitation des actifs confidentiels référencé par `roadmap.md:M4`.

# Runbook d'audit et d'opérations des actifs confidentiels

Ce guide consolide les éléments de preuve sur lesquels les auditeurs et les opérateurs s'appuient.
lors de la validation des flux d’actifs confidentiels. Il complète le playbook de rotation
(`docs/source/confidential_assets_rotation.md`) et le registre d'étalonnage
(`docs/source/confidential_assets_calibration.md`).

## 1. Divulgation sélective et flux d'événements

- Chaque instruction confidentielle émet une charge utile `ConfidentialEvent` structurée
  (`Shielded`, `Transferred`, `Unshielded`) capturé dans
  `crates/iroha_data_model/src/events/data/events.rs:198` et sérialisé par le
  exécuteurs testamentaires (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  La suite de régression exerce les charges utiles concrètes afin que les auditeurs puissent s'appuyer sur
  Dispositions JSON déterministes (`crates/iroha_core/tests/zk_confidential_events.rs:19` – `299`).
- Torii expose ces événements via le pipeline standard SSE/WebSocket ; auditeurs
  abonnez-vous en utilisant `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  éventuellement s'étendre à une seule définition d'actif. Exemple CLI :

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Les métadonnées de politique et les transitions en attente sont disponibles via
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), reflété par le SDK Swift
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) et documenté dans
  à la fois la conception des actifs confidentiels et les guides SDK
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Télémétrie, tableaux de bord et preuves d'étalonnage

- Mesures d'exécution de la profondeur de l'arborescence de la surface, historique des engagements/frontières, expulsion des racines
  compteurs et taux de réussite du cache du vérificateur
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Tableaux de bord Grafana en
  `dashboards/grafana/confidential_assets.json` expédie les panneaux associés et
  alertes, avec le flux de travail documenté dans `docs/source/confidential_assets.md:401`.
- Exécutions d'étalonnage (NS/op, gas/op, ns/gas) avec journaux signés en direct
  `docs/source/confidential_assets_calibration.md`. Le dernier Apple Silicon
  L'exécution de NEON est archivée sur
  `docs/source/confidential_assets_calibration_neon_20260428.log`, et pareil
  Le grand livre enregistre les dérogations temporaires pour les profils SIMD neutres et AVX2 jusqu'à ce que
  les hôtes x86 sont mis en ligne.

## 3. Réponse aux incidents et tâches des opérateurs

- Les procédures de rotation/mise à niveau résident dans
  `docs/source/confidential_assets_rotation.md`, expliquant comment mettre en scène de nouveaux
  les ensembles de paramètres, planifier les mises à niveau des politiques et informer les portefeuilles/auditeurs. Le
  listes de suivi (`docs/source/project_tracker/confidential_assets_phase_c.md`)
  propriétaires de runbooks et attentes en matière de répétition.
- Pour les répétitions de production ou les fenêtres d'urgence, les opérateurs attachent des preuves à
  `status.md` (par exemple, le journal de répétition multi-pistes) et comprennent :
  Preuve `curl` des transitions de politique, instantanés Grafana et événement correspondant
  digère afin que les auditeurs puissent reconstruire menthe → transfert → révéler les délais.

## 4. Cadence des examens externes

- Périmètre de l'examen de sécurité : circuits confidentiels, registres de paramètres, politique
  transitions et télémétrie. Ce document ainsi que les formulaires du grand livre d'étalonnage
  le dossier de preuves envoyé aux fournisseurs ; la planification des révisions est suivie via
  M4 dans `docs/source/project_tracker/confidential_assets_phase_c.md`.
- Les opérateurs doivent tenir `status.md` à jour avec toutes les conclusions ou suivis des fournisseurs.
  éléments d’action. Jusqu'à la fin de l'examen externe, ce runbook sert de
  les auditeurs de référence opérationnelle peuvent tester.