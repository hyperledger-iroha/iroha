---
lang: fr
direction: ltr
source: docs/source/runbooks/nexus_multilane_rehearsal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0aa4642cc60f384f6c52aaae2f97a6e4e8f741d6365c483514c2016d1ba10e82
source_last_modified: "2025-12-14T09:53:36.243318+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de rehearsal de lancement multi‑lane Nexus

Ce runbook guide le rehearsal Nexus Phase B4. Il valide que le bundle
`iroha_config` approuvé par la gouvernance et le manifeste genesis multi‑lane se
comportent de manière déterministe à travers la télémétrie, le routage et les
exercices de rollback.

## Périmètre

- Exercer les trois lanes Nexus (`core`, `governance`, `zk`) avec un ingress Torii
  mixte (transactions, déploiements de contrats, actions de gouvernance) en
  utilisant la seed de charge signée `NEXUS-REH-2026Q1`.
- Capturer les artefacts de télémétrie/traces requis pour l’acceptation B4
  (scrape Prometheus, export OTLP, logs structurés, traces d’admission Norito,
  métriques RBC).
- Exécuter le drill de rollback `B4-RB-2026Q1` immédiatement après le dry‑run et
  confirmer que le profil mono‑lane se ré‑applique proprement.

## Prérequis

1. `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` reflète
   l’approbation GOV-2026-03-19 (manifestes signés + initiales des reviewers).
2. `defaults/nexus/config.toml` (sha256
   `4f57655666bb0c83221cd3b56fd37218822e4c63db07e78a6694db51077f7017`, blake2b
   `65827a4b0348a7837f181529f602dc3315eba55d6ca968aaafb85b4ef8cfb2f6759283de77590ec5ec42d67f5717b54a299a733b617a50eb2990d1259c848017`, avec
   `nexus.enabled = true` intégré) et `defaults/nexus/genesis.json` correspondent
   aux hashes approuvés ; `kagami genesis bootstrap --profile nexus` rapporte le
   même digest enregistré dans le tracker.
3. Le catalogue de lanes correspond au layout approuvé à trois lanes ;
   `irohad --sora --config defaults/nexus/config.toml` doit émettre le banner du
   router Nexus.
4. La CI multi‑lane est verte : `ci/check_nexus_multilane_pipeline.sh`
   (exécute `integration_tests/tests/nexus/multilane_pipeline.rs` via
   `.github/workflows/integration_tests_multilane.yml`) et
   `ci/check_nexus_multilane.sh` (couverture router) passent pour que le profil
   Nexus reste prêt multi‑lane (`nexus.enabled = true`, hashes du catalogue Sora
   intacts, stockage lane sous `blocks/lane_{id:03}_{slug}` et merge logs
   provisionnés). Capturer les digests d’artefacts dans le tracker quand le
   bundle defaults change.
5. Dashboards + alertes de télémétrie pour les métriques Nexus importés dans le
   dossier Grafana du rehearsal ; routes d’alerte pointant vers le service
   PagerDuty du rehearsal.
6. Les lanes Torii SDK sont configurés selon la table de politique de routage et
   peuvent rejouer la charge de rehearsal localement.

## Vue d’ensemble du calendrier

| Phase | Fenêtre cible | Owner(s) | Critère de sortie |
|-------|--------------|----------|-------------------|
| Préparation | Apr 1 – 5 2026 | @program-mgmt, @telemetry-ops | Seed publiée, dashboards en place, nœuds de rehearsal provisionnés. |
| Freeze staging | Apr 8 2026 18:00 UTC | @release-eng | Hashes config/genesis re‑vérifiés ; notice de freeze envoyée. |
| Exécution | Apr 9 2026 15:00 UTC | @qa-veracity, @nexus-core, @torii-sdk | Checklist complétée sans incident bloquant ; pack télémétrie archivé. |
| Drill rollback | Immédiatement post‑exécution | @sre-core | Checklist `B4-RB-2026Q1` complétée ; télémétrie rollback capturée. |
| Rétrospective | Due Apr 15 2026 | @program-mgmt, @telemetry-ops, @governance | Doc retro/leçons apprises + tracker des blocages publié. |

## Checklist d’exécution (Apr 9 2026 15:00 UTC)

1. **Attestation de config** — `iroha_cli config show --actual` sur chaque nœud ;
   confirmer que les hashes correspondent à l’entrée tracker.
2. **Warm‑up lanes** — rejouer la charge seed pendant 2 slots, vérifier que
   `nexus_lane_state_total` montre de l’activité sur les trois lanes.
3. **Capture télémétrie** — enregistrer snapshots Prometheus `/metrics`,
   échantillons OTLP, logs structurés Torii (par lane/dataspace), et métriques RBC.
4. **Hooks gouvernance** — exécuter le sous‑ensemble de transactions de gouvernance
   et vérifier le routage par lane + tags de télémétrie.
5. **Drill incident** — simuler une saturation de lane selon le plan ; s’assurer
   que les alertes se déclenchent et que la réponse est consignée.
6. **Drill rollback `B4-RB-2026Q1`** — appliquer le profil mono‑lane, rejouer la
   checklist rollback, collecter les preuves télémétrie et ré‑appliquer le bundle Nexus.
7. **Upload artefacts** — pousser le pack télémétrie, traces Torii et drill log
   vers le bucket d’évidence Nexus ; lier dans `docs/source/nexus_transition_notes.md`.
8. **Manifeste/validation** — exécuter `scripts/telemetry/validate_nexus_telemetry_pack.py \
   --pack-dir <path> --slot-range <start-end> --workload-seed <value> \
   --require-slot-range --require-workload-seed` pour produire `telemetry_manifest.json`
   + `.sha256`, puis joindre le manifeste à l’entrée tracker du rehearsal. Le
   helper normalise les bornes de slots (enregistrées comme entiers dans le
   manifeste) et échoue rapidement si l’un des indices manque afin que les
   artefacts de gouvernance restent déterministes.

## Sorties

- Checklist de rehearsal signée + drill log incident.
- Pack télémétrie (`prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`).
- Manifeste télémétrie + digest généré par le script de validation.
- Doc rétrospective résumant blocages, mitigations et owners.

## Résumé d’exécution — Apr 9 2026

- Rehearsal exécuté 15:00 UTC–16:12 UTC avec seed `NEXUS-REH-2026Q1`; les trois
  lanes ont soutenu ~2,4k TEU par slot et `nexus_lane_state_total` a reporté des
  enveloppes équilibrées.
- Pack télémétrie archivé à `artifacts/nexus/rehearsals/2026q1/` (inclut
  `prometheus.tgz`, `otlp.ndjson`, `torii_structured_logs.jsonl`, incident log et
  preuves rollback). Checksums enregistrés dans
  `docs/source/project_tracker/nexus_rehearsal_2026q1.md`.
- Drill rollback `B4-RB-2026Q1` complété à 16:18 UTC ; profil mono‑lane ré‑appliqué
  en 6m42s sans lanes bloqués, puis bundle Nexus ré‑activé après confirmation
  télémétrie.
- Incident de saturation de lane injecté au slot 842 (clamp de headroom forcé)
  a déclenché les alertes attendues ; le playbook de mitigation a clos la page
  en 11m avec chronologie PagerDuty documentée.
- Aucun blocage n’a empêché la complétion ; les items de suivi (automation du
  logging de headroom TEU, script de validation du pack télémétrie) sont suivis
  dans la rétrospective du Apr 15.

## Escalade

- Incidents bloquants ou régressions télémétrie arrêtent le rehearsal et exigent
  une escalade gouvernance sous 4 heures ouvrées.
- Toute divergence du bundle config/genesis approuvé doit relancer le rehearsal
  après ré‑approbation.

## Validation du pack télémétrie (Complété)

Exécutez `scripts/telemetry/validate_nexus_telemetry_pack.py` après chaque
rehearsal pour prouver que le bundle télémétrie contient les artefacts canoniques
(export Prometheus, OTLP NDJSON, logs structurés Torii, log de rollback) et
capturer leurs digests SHA-256. Le helper écrit `telemetry_manifest.json` et le
fichier `.sha256` correspondant afin que la gouvernance puisse citer les hashes
d’évidence dans le packet de retro.

Pour le rehearsal du Apr 9 2026, le manifeste validé se trouve avec les
artefacts sous `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` et son
digest dans `telemetry_manifest.json.sha256`. Attachez les deux fichiers à
l’entrée tracker lors de la publication de la retro.

```bash
scripts/telemetry/validate_nexus_telemetry_pack.py \
  artifacts/nexus_rehearsal_2026q1 \
  --slot-range 820-860 \
  --workload-seed NEXUS-REH-2026Q1 \
  --metadata rehearsal_id=B4-2026Q1 team=telemetry-ops
```

Passez `--require-slot-range` / `--require-workload-seed` dans CI pour bloquer
les uploads qui oublient ces annotations. Utilisez `--expected <name>` pour
ajouter des artefacts supplémentaires (p. ex. reçus DA) si le plan de rehearsal
le demande.
