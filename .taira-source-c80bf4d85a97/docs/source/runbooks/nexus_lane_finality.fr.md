---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: fr
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de finalité des lanes Nexus & oracle

**Statut :** Actif — satisfait le livrable dashboard/runbook NX-18.  
**Audience :** Core Consensus WG, SRE/Telemetry, Release Engineering, leads d'astreinte.  
**Périmètre :** Couvre les SLOs de durée de slot, quorum DA, oracle et buffer de
règlement qui conditionnent la promesse de finalité à 1 s. À utiliser avec
`dashboards/grafana/nexus_lanes.json` et les helpers de télémétrie sous
`scripts/telemetry/`.

## Dashboards

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — publie le tableau
  “Nexus Lane Finality & Oracles”. Les panneaux suivent :
  - `histogram_quantile()` sur `iroha_slot_duration_ms` (p50/p95/p99) ainsi que le
    gauge du dernier échantillon.
  - `iroha_da_quorum_ratio` et `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])`
    pour mettre en évidence le churn DA.
  - Surfaces oracle : `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`,
    `iroha_oracle_twap_window_seconds` et `iroha_oracle_haircut_basis_points`.
  - Panneau buffer de règlement (`iroha_settlement_buffer_xor`) montrant les débits
    par lane en direct issus des reçus `LaneBlockCommitment`.
- **Règles d’alerte** — réutilisent les clauses SLO Slot/DA de `ans3.md`. Alerte lorsque :
  - p95 de durée de slot > 1000 ms sur deux fenêtres consécutives de 5 m,
  - ratio de quorum DA < 0,95 ou `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - staleness oracle > 90 s ou fenêtre TWAP ≠ 60 s configurées,
  - buffer de règlement < 25 % (soft) / 10 % (hard) une fois la métrique active.

## Fiche de métriques

| Métrique | Cible / Alerte | Notes |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | Utilisez le panneau dashboard ou exécutez `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) contre l’export Prometheus collecté pendant les runs de chaos. |
| `iroha_slot_duration_ms_latest` | Reflète le slot le plus récent ; investiguer si > 1100 ms même si les quantiles paraissent OK. | Exporter la valeur pour les incidents. |
| `iroha_da_quorum_ratio` | ≥ 0,95 sur une fenêtre glissante de 30 m. | Dérivé des reschedules DA lors des commits de blocs. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | Doit rester à 0 hors répétitions de chaos. | Traitez toute hausse soutenue comme `missing-availability warning`. |

Chaque reschedule déclenche aussi un warning pipeline Torii avec `kind = "missing-availability warning"`. Capturez ces événements en plus du pic métrique pour identifier l’en‑tête de bloc affecté, la tentative de retry et les compteurs de requeue sans fouiller les logs des validateurs.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 s. Alerte à 75 s. | Indique des feeds TWAP 60 s obsolètes. |
| `iroha_oracle_twap_window_seconds` | Exactement 60 s ± tolérance 5 s. | Une divergence signifie un oracle mal configuré. |
| `iroha_oracle_haircut_basis_points` | Correspond au tier de liquidité du lane (0/25/75 bps). | Escalader si les haircuts grimpent sans raison. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. Forcer le mode XOR‑only sous 10 %. | Le panneau expose les débits micro‑XOR par lane/dataspace ; exporter avant d’ajuster la politique du routeur. |

## Playbook de réponse

### Dépassement de durée de slot
1. Confirmer via dashboard + `promql` (p95/p99).  
2. Capturer la sortie `scripts/telemetry/check_slot_duration.py --json-out <path>`
   (et le snapshot métrique) afin que les revues CXO valident la barrière 1 s.  
3. Inspecter les entrées RCA : profondeur de queue mempool, reschedules DA, traces IVM.  
4. Ouvrir un incident, joindre un screenshot Grafana et planifier un drill chaos si la régression persiste.

### Dégradation de quorum DA
1. Vérifier `iroha_da_quorum_ratio` et le compteur de reschedule ; corréler avec les logs `missing-availability warning`.  
2. Si ratio <0,95, épingler les attesters défaillants, élargir les paramètres d’échantillonnage, ou basculer le profil en mode XOR‑only.  
3. Exécuter `scripts/telemetry/check_nexus_audit_outcome.py` pendant les rehearsals routed‑trace pour prouver que les événements `nexus.audit.outcome` passent encore après remédiation.  
4. Archiver les bundles de reçus DA avec le ticket d’incident.

### Staleness oracle / dérive haircut
1. Utiliser les panneaux 5–8 pour vérifier prix, staleness, fenêtre TWAP et haircut.  
2. Pour staleness >90 s : redémarrer ou faire un failover de l’oracle, puis relancer le harness chaos.  
3. Pour mismatch haircut : inspecter le profil de liquidité et les changements de gouvernance récents ; notifier la trésorerie si les lignes de swap doivent être ajustées.

### Alertes buffer de règlement
1. Utiliser `iroha_settlement_buffer_xor` (et les reçus nocturnes) pour confirmer la marge avant d’ajuster la politique du routeur.  
2. Quand la métrique franchit un seuil, déclencher les procédures :
   - **Soft breach (<25 %)** : engager la trésorerie, considérer l’appel de lignes de swap, enregistrer l’alerte.  
   - **Hard breach (<10 %)** : forcer l’inclusion XOR‑only, refuser les lanes subventionnés et documenter dans `ops/drill-log.md`.  
3. Référencez `docs/source/settlement_router.md` pour les leviers repo/reverse‑repo.

## Preuves & automatisation

- **CI** — câbler `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` et `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` au workflow d’acceptation RC afin que chaque release candidate livre le résumé de durée de slot plus les résultats des gates DA/oracle/buffer avec le snapshot métrique référencé plus haut. Le helper est déjà appelé depuis `ci/check_nexus_lane_smoke.sh`.  
- **Parité dashboard** — exécuter `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` pour s’assurer que le board publié correspond aux exports staging/prod.  
- **Artefacts de trace** — durant les rehearsals TRACE ou drills chaos NX-18, invoquer `scripts/telemetry/check_nexus_audit_outcome.py` pour archiver le payload `nexus.audit.outcome` le plus récent (`docs/examples/nexus_audit_outcomes/`). Joindre l’archive et les captures Grafana au drill log.
- **Bundling des preuves de slot** — après génération du JSON de résumé, exécuter `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` afin que `slot_bundle_manifest.json` capture les digests SHA‑256 des deux artefacts. Téléverser le répertoire tel quel avec le bundle de preuves RC. Le pipeline release l’exécute automatiquement (option `--skip-nexus-lane-smoke`) et copie `artifacts/nx18/` dans la sortie release.

## Checklist de maintenance

- Garder `dashboards/grafana/nexus_lanes.json` synchronisé avec les exports Grafana après chaque changement de schéma ; documenter les éditions dans les messages de commit en référant NX-18.  
- Mettre à jour ce runbook quand de nouvelles métriques (p. ex., gauges de buffer de règlement) ou seuils d’alerte arrivent.  
- Enregistrer chaque rehearsal chaos (latence de slot, jitter DA, stall oracle, depletion buffer) avec `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

Suivre ce runbook fournit les preuves “dashboards/runbooks opérateur” requises par NX-18 et garantit que le SLO de finalité reste applicable avant le GA Nexus.
