---
lang: fr
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 Harnais SLO

La ligne de version Iroha 3 comporte des SLO explicites pour les chemins critiques Nexus :

- durée du créneau de finalité (cadence NX‑18)
- vérification des preuves (certificats de commit, attestations JDG, épreuves de pont)
- gestion des points de terminaison de preuve (proxy de chemin Axum via latence de vérification)
- parcours de frais et de jalonnement (flux payeur/sponsor et obligations/slash)

## Budget

Les budgets résident dans `benchmarks/i3/slo_budgets.json` et sont directement mappés sur le banc
scénarios dans la suite I3. Les objectifs sont des cibles p99 par appel :

- Frais/mise en jeu : 50 ms par appel (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Valider le certificat / JDG / vérifier le pont : 80 ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Valider l'assemblage du certificat : 80 ms (`commit_cert_assembly`)
- Planificateur d'accès : 50 ms (`access_scheduler`)
- Proxy de point de terminaison de preuve : 120 ms (`torii_proof_endpoint`)

Les indices de taux de combustion (`burn_rate_fast`/`burn_rate_slow`) codent le 14,4/6,0
ratios multi-fenêtres pour les alertes de radiomessagerie et de ticket.

## Harnais

Exécutez le harnais via `cargo xtask i3-slo-harness` :

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Sorties :

- `bench_report.json|csv|md` — résultats bruts de la suite de bancs I3 (hash git + scénarios)
- `slo_report.json|md` — Évaluation SLO avec rapport réussite/échec/budget par cible

Le harnais consomme le fichier des budgets et applique `benchmarks/i3/slo_thresholds.json`
pendant l'exécution sur banc pour échouer rapidement lorsqu'une cible régresse.

## Télémétrie et tableaux de bord

- Finalité : `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Vérification de la preuve : `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Les panneaux de démarrage Grafana sont installés dans `dashboards/grafana/i3_slo.json`. Prometheus
des alertes de taux de combustion sont fournies dans `dashboards/alerts/i3_slo_burn.yml` avec le
budgets ci-dessus intégrés (finalité 2 s, vérification de la preuve 80 ms, proxy du point de terminaison de preuve
120 ms).

## Notes opérationnelles

- Faites passer le harnais en tenue de nuit ; publier `artifacts/i3_slo/<stamp>/slo_report.md`
  aux côtés des artefacts de banc pour les preuves de gouvernance.
- Si un budget échoue, utilisez la démarque de référence pour identifier le scénario, puis explorez
  dans le panneau/alerte Grafana correspondant pour établir une corrélation avec les métriques en direct.
- Les SLO de point de terminaison de preuve utilisent la latence de vérification comme proxy pour éviter les erreurs par route.
  explosion de cardinalité ; la cible de référence (120 ms) correspond à la rétention/DoS
  garde-fous sur l'API de preuve.