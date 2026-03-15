---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6049b1c4fb42bbfbeaa7fa8f3549c5b7beac1a3e8baec45c0c0ce52f0c3baa2e
source_last_modified: "2025-11-14T09:52:13.533271+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Tableaux de bord de monitoring Taikai
description: Synthèse portail des boards Grafana viewer/cache qui soutiennent la preuve SN13-C.
---

La readiness du routing-manifest Taikai (TRM) repose sur deux boards Grafana et
leurs alertes associées. Cette page reprend les points clés de
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json` et
`dashboards/alerts/taikai_viewer_rules.yml` afin que les reviewers puissent suivre
sans cloner le repo.

## Dashboard viewer (`taikai_viewer.json`)

- **Live edge & latence:** Les panels visualisent les histogrammes de latence
  p95/p99 (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`)
  par cluster/stream. Surveillez p99 > 900 ms ou drift > 1.5 s (déclenche l'alerte
  `TaikaiLiveEdgeDrift`).
- **Erreurs de segments:** Décompose `taikai_ingest_segment_errors_total{reason}`
  pour exposer les échecs de decode, tentatives de lineage replay ou mismatches
  de manifest. Joignez des captures aux incidents SN13-C quand ce panel dépasse
  la bande “warning”.
- **Santé viewer & CEK:** Les panels issus des métriques `taikai_viewer_*` suivent
  l'âge de rotation CEK, le mix de PQ guard, les rebuffer counts et les roll-ups
  d'alertes. Le panel CEK enforce le SLA de rotation que la gouvernance valide
  avant d'approuver de nouveaux aliases.
- **Snapshot de télémétrie des aliases:** La table `/status → telemetry.taikai_alias_rotations`
  est directement sur le board pour permettre aux opérateurs de vérifier les digests
  de manifest avant d'attacher les preuves de gouvernance.

## Dashboard cache (`taikai_cache.json`)

- **Pression par tier:** Les panels tracent `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  et `sorafs_taikai_cache_promotions_total`. Utilisez-les pour voir si une rotation
  TRM surcharge des tiers spécifiques.
- **Refus QoS:** `sorafs_taikai_qos_denied_total` remonte quand la pression cache force
  du throttling ; annotez le drill log dès que le taux s'éloigne de zéro.
- **Utilisation egress:** Permet de confirmer que les sorties SoraFS suivent les
  viewers Taikai lorsque les fenêtres CMAF tournent.

## Alertes et capture des preuves

- Les règles de paging vivent dans `dashboards/alerts/taikai_viewer_rules.yml` et
  correspondent une à une aux panels ci-dessus (`TaikaiLiveEdgeDrift`,
  `TaikaiIngestFailure`, `TaikaiCekRotationLag`, proof-health warnings). Assurez-vous
  que chaque cluster de production les connecte à Alertmanager.
- Les snapshots/captures pris pendant les drills doivent être stockés dans
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` avec les spool files et le JSON
  `/status`. Utilisez `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  pour ajouter l'exécution au drill log partagé.
- Lorsque les dashboards changent, incluez le digest SHA-256 du fichier JSON dans
  la description du PR du portail afin que les auditeurs puissent faire correspondre
  le dossier Grafana géré avec la version du repo.

## Checklist du bundle de preuves

Les revues SN13-C attendent que chaque drill ou incident fournisse les mêmes
artefacts listés dans le runbook Taikai anchor. Capturez-les dans l'ordre ci-dessous
pour que le bundle soit prêt pour la revue de gouvernance :

1. Copiez les fichiers les plus récents `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json` et `taikai-lineage-*.json` depuis
   `config.da_ingest.manifest_store_dir/taikai/`. Ces artefacts de spool prouvent
   quel routing manifest (TRM) et quelle fenêtre lineage étaient actifs. Le helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   copiera les spool files, émettra les hashes et signera éventuellement le résumé.
2. Enregistrez la sortie `/v2/status` filtrée sur
   `.telemetry.taikai_alias_rotations[]` et stockez-la à côté des spool files.
   Les reviewers comparent le `manifest_digest_hex` rapporté et les bornes de
   fenêtre avec l'état de spool copié.
3. Exportez des snapshots Prometheus pour les métriques ci-dessus et prenez des
   captures des dashboards viewer/cache avec les filtres cluster/stream pertinents.
   Déposez le JSON/CSV brut et les captures d'écran dans le dossier d'artefacts.
4. Incluez les IDs d'incident Alertmanager (le cas échéant) qui référencent les
   règles de `dashboards/alerts/taikai_viewer_rules.yml` et notez si elles se sont
   auto-fermées une fois la condition résolue.

Stockez tout sous `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` pour que les audits
et les revues de gouvernance SN13-C puissent récupérer une archive unique.

## Cadence des drills et logging

- Exécutez le drill Taikai anchor le premier mardi de chaque mois à 15:00 UTC.
  Le calendrier garde les preuves fraîches avant la sync de gouvernance SN13.
- Après avoir capturé les artefacts ci-dessus, ajoutez l'exécution au ledger
  partagé via `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`.
  Le helper émet l'entrée JSON requise par `docs/source/sorafs/runbooks-index.md`.
- Liez les artefacts archivés dans l'entrée du runbook index et escaladez toute
  alerte échouée ou régression de dashboard dans les 48 heures via le canal
  Media Platform WG/SRE.
- Conservez le set de captures résumé du drill (latence, drift, erreurs, rotation
  CEK, pression cache) à côté du bundle de spool afin que les opérateurs puissent
  montrer exactement le comportement des dashboards pendant la répétition.

Reportez-vous au [Taikai Anchor Runbook](./taikai-anchor-runbook.md) pour la
procédure complète Sev 1 et la checklist de preuves. Cette page ne capture que
les consignes liées aux dashboards exigées par SN13-C avant de quitter 🈺.
