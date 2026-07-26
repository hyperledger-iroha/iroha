---
lang: fr
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

# Analyse economique - Shadow Run 2025-10 -> 2025-11

Artefact source: `docs/examples/soranet_incentive_shadow_run.json` (signature + cle publique dans le meme repertoire). La simulation a rejoue 60 epochs par relay avec le moteur de recompenses fixe sur `RewardConfig` enregistre dans `reward_config.json`.

## Resume de distribution

- **Payouts totaux:** 5,160 XOR sur 360 epochs recompensees.
- **Enveloppe de fairness:** coefficient Gini 0.121; part du top relay 23.26%
  (bien en dessous du guardrail governance 30%).
- **Disponibilite:** moyenne de la flotte 96.97%, tous les relays sont restes au-dessus de 94%.
- **Bande passante:** moyenne de la flotte 91.20%, avec le plus faible a 87.23%
  pendant une maintenance planifiee; penalites appliquees automatiquement.
- **Bruit de compliance:** 9 epochs de warning et 3 suspensions ont ete observes et
  transformes en reductions de payout; aucun relay n'a depasse le cap de 12 warnings.
- **Hygiene operationnelle:** aucun snapshot de metriques n'a ete saute pour cause de
  config, bonds ou doublons manquants; aucune erreur de calculateur n'a ete emise.

## Observations

- Les suspensions correspondent aux epochs ou les relays sont passes en maintenance. Le
  moteur de payout a emis des zero payouts pour ces epochs tout en preservant la trace
  d'audit dans le JSON shadow-run.
- Les penalites de warning ont coupe 2% des payouts affectes; la distribution
  reste convergente grace aux poids uptime/bandwidth (650/350 per mille).
- La variance de bandwidth suit la heatmap guard anonymisee. Le plus faible
  (`6666...6666`) a conserve 620 XOR sur la fenetre, au-dessus du plancher 0.6x.
- Les alertes sensibles a la latence (`SoranetRelayLatencySpike`) sont restees sous les
  seuils de warning tout au long de la fenetre; les dashboards correles sont captures dans
  `dashboards/grafana/soranet_incentives.json`.

## Actions recommandees avant GA

1. Continuer les replays shadow mensuels et mettre a jour le set d'artefacts et cette
   analyse si la composition de la flotte change.
2. Bloquer les payouts automatiques sur la suite d'alertes Grafana referencee dans la roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); copier des screenshots dans les
   minutes governance lors d'une demande de renouvellement.
3. Re-executer le stress test economique si la base reward, les poids uptime/bandwidth ou
   la penalite de compliance change de >=10%.
