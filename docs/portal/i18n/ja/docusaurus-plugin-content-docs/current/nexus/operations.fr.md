---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operations
title: Runbook operations Nexus
description: Résumé opérationnel prêt pour le terrain du workflow opérateur Nexus, reflétant `docs/source/nexus_operations.md`.
---

Utilisez cette page comme compagnon de référence rapide de `docs/source/nexus_operations.md`. Elle condense la checklist opérationnelle, les points de contrôle de gestion du changement et les exigences de couverture télémétrie que les opérateurs Nexus doivent suivre.

## Checklist de cycle de vie

| Étape | Actions | Preuves |
|-------|--------|----------|
| Pré-vol | Vérifier les hash/signatures de release, confirmer `profile = "iroha3"` et préparer les modèles de configuration. | Sortie de `scripts/select_release_profile.py`, journal de checksum, bundle de manifestes signé. |
| Alignement du catalogue | Mettre à jour le catalogue `[nexus]`, la politique de routage et les seuils DA selon le manifeste émis par le conseil, puis capturer `--trace-config`. | Sortie `irohad --sora --config ... --trace-config` stockée avec le ticket d'onboarding. |
| Smoke & cutover | Lancer `irohad --sora --config ... --trace-config`, exécuter le smoke du CLI (`FindNetworkStatus`), valider les exports de télémétrie et demander l'admission. | Log de smoke-test + confirmation Alertmanager. |
| Régime stable | Surveiller dashboards/alertes, faire tourner les clés selon la cadence de gouvernance et synchroniser configs/runbooks lorsque les manifestes changent. | Minutes de revue trimestrielle, captures des dashboards, IDs de tickets de rotation. |

L'onboarding détaillé (remplacement de clés, modèles de routage, étapes de profil de release) reste dans `docs/source/sora_nexus_operator_onboarding.md`.

## Gestion du changement

1. **Mises à jour de release** - suivre les annonces dans `status.md`/`roadmap.md` ; joindre la checklist d'onboarding à chaque PR de release.
2. **Changements de manifestes de lane** - vérifier les bundles signés du Space Directory et les archiver sous `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuration** - tout changement de `config/config.toml` nécessite un ticket référencant la lane/data-space. Conserver une copie expurgée de la config effective lors des joins/upgrade de noeuds.
4. **Exercices de rollback** - répéter trimestriellement les procédures stop/restore/smoke ; consigner les résultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Approbations conformité** - les lanes privées/CBDC doivent obtenir un feu vert conformité avant de modifier la politique DA ou les knobs de redaction de télémétrie (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLOs

- Dashboards : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, plus des vues SDK spécifiques (par ex. `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et règles de transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métriques à surveiller :
  - `nexus_lane_height{lane_id}` - alerter en cas d'absence de progression pendant trois slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerter au-dessus des seuils par lane (par défaut 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - alerter quand le P99 dépasse 900 ms (public) ou 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerter si le ratio d'erreur à 5 minutes dépasse 2%.
  - `telemetry_redaction_override_total` - Sev 2 immédiat ; assurer des tickets conformité pour les overrides.
- Exécuter la checklist de remédiation télémétrie dans le [plan de remédiation télémétrie Nexus](./nexus-telemetry-remediation) au moins trimestriellement et joindre le formulaire rempli aux notes de revue opérations.

## Matrice d'incident

| Gravité | Définition | Réponse |
|----------|------------|----------|
| Sev 1 | Brèche d'isolation data-space, arrêt de settlement >15 min, ou corruption de vote de gouvernance. | Alerter Nexus Primary + Release Engineering + Compliance, geler l'admission, collecter les artefacts, publier comms <=60 min, RCA <=5 jours ouvrés. |
| Sev 2 | Violation SLA de backlog de lane, angle mort télémétrie >30 min, rollout de manifeste échoué. | Alerter Nexus Primary + SRE, atténuer <=4 h, déposer des suivis sous 2 jours ouvrés. |
| Sev 3 | Dérive non bloquante (docs, alertes). | Enregistrer dans le tracker, planifier le correctif dans le sprint. |

Les tickets d'incident doivent enregistrer les IDs de lane/data-space affectés, les hashes de manifeste, la chronologie, les métriques/logs de support et les tâches/propriétaires de suivi.

## Archive de preuves

- Stocker bundles/manifestes/exports de télémétrie sous `artifacts/nexus/<lane>/<date>/`.
- Conserver configs expurgées + sortie `--trace-config` pour chaque release.
- Joindre minutes du conseil + décisions signées lorsque des changements de config ou manifeste sont appliqués.
- Conserver les snapshots Prometheus hebdomadaires des métriques Nexus pendant 12 mois.
- Enregistrer les modifications du runbook dans `docs/source/project_tracker/nexus_config_deltas/README.md` pour que les auditeurs sachent quand les responsabilités ont changé.

## Matériel lié

- Vue d'ensemble : [Nexus overview](./nexus-overview)
- Spécification : [Nexus spec](./nexus-spec)
- Géométrie des lanes : [Nexus lane model](./nexus-lane-model)
- Transition et shims de routage : [Nexus transition notes](./nexus-transition-notes)
- Onboarding opérateur : [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Remédiation télémétrie : [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
