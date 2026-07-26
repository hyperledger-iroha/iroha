---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : Opérations du Runbook Nexus
description : Résumé opérationnel prêt pour le terrain du workflow opérateur Nexus, reflétant `docs/source/nexus_operations.md`.
---

Utilisez cette page comme compagnon de référence rapide de `docs/source/nexus_operations.md`. Elle condense la checklist opérationnelle, les points de contrôle de gestion du changement et les exigences de couverture télémétrie que les opérateurs Nexus doivent suivre.

## Checklist de cycle de vie| Étape | Actions | Préuvés |
|-------|--------|--------------|
| Pré-vol | Vérifiez les hash/signatures de release, confirmez `profile = "iroha3"` et préparez les modèles de configuration. | Sortie de `scripts/select_release_profile.py`, journal de checksum, bundle de manifestes signé. |
| Alignement du catalogue | Mettre à jour le catalogue `[nexus]`, la politique de routage et les seuils DA selon le manifeste émis par le conseil, puis capturer `--trace-config`. | Sortie `irohad --sora --config ... --trace-config` stockée avec le ticket d'embarquement. |
| Fumée et basculement | Lancer `irohad --sora --config ... --trace-config`, exécuter le smoke du CLI (`FindNetworkStatus`), valider les exports de télémétrie et demander l'admission. | Journal de smoke-test + confirmation Alertmanager. |
| Régime stable | Surveiller les tableaux de bord/alertes, faire tourner les clés selon la cadence de gouvernance et synchroniser les configs/runbooks lorsque les manifestes changent. | Minutes de revue trimestrielle, captures des tableaux de bord, IDs de tickets de rotation. |

L'onboarding détaillé (remplacement de clés, modèles de routage, étapes de profil de release) reste dans `docs/source/sora_nexus_operator_onboarding.md`.

##Gestion du changement1. **Mises à jour de release** - suivre les annonces dans `status.md`/`roadmap.md` ; joindre la checklist d'onboarding à chaque PR de release.
2. **Changements de manifestes de lane** - vérifiez les bundles signés du Space Directory et les archiveur sous `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuration** - tout changement de `config/config.toml` nécessite un ticket référencant la lane/data-space. Conserver une copie expurgée de la configuration effective lors des joins/upgrade de noeuds.
4. **Exercices de rollback** - répéter trimestriellement les procédures stop/restore/smoke ; consigner les résultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Approbations conformité** - les voies privées/CBDC doivent obtenir un feu vert conformité avant de modifier la politique DA ou les boutons de rédaction de télémétrie (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLO- Tableaux de bord : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, plus des vues SDK spécifiques (par ex. `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et règles de transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métriques à surveiller :
  - `nexus_lane_height{lane_id}` - alerter en cas d'absence de progression pendant trois slots.
  - `nexus_da_backlog_chunks{lane_id}` - alerter au-dessus des seuils par voie (par défaut 64 publics / 8 privés).
  - `nexus_settlement_latency_seconds{lane_id}` - alerter lorsque le P99 dépasse 900 ms (public) ou 1200 ms (privé).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerter si le ratio d'erreur à 5 minutes dépasse 2%.
  - `telemetry_redaction_override_total` - Septembre 2 immédiat ; assureur des tickets conformité pour les dérogations.
- Exécuter la checklist de remédiation télémétrie dans le [plan de remédiation télémétrie Nexus](./nexus-telemetry-remediation) au moins trimestriellement et joindre le formulaire rempli aux notes de revue opérations.

## Matrice d'incident| Gravité | Définition | Réponse |
|--------------|------------|--------------|
| 1 septembre | Brèche d'isolement data-space, arrêt de règlement >15 min, ou corruption de vote de gouvernance. | Alerter Nexus Primary + Release Engineering + Compliance, geler l'admission, collecter les artefacts, publier comms 30 min, rollout de manifeste échec. | Alerter Nexus Primaire + SRE, atténuation <=4 h, déposer des suivis sous 2 jours ouvrés. |
| 3 septembre | Dérive non bloquante (docs, alertes). | Enregistrer dans le tracker, planifier le correctif dans le sprint. |

Les tickets d'incident doivent enregistrer les ID de lane/data-space affectés, les hashes de manifeste, la chronologie, les métriques/logs de support et les tâches/propriétaires de suivi.

## Archive de preuves

- Stocker bundles/manifestes/exports de télémétrie sous `artifacts/nexus/<lane>/<date>/`.
- Conserver configs expurgées + sortie `--trace-config` pour chaque release.
- Joindre minutes du conseil + décisions signées lorsque des changements de config ou manifestes sont appliqués.
- Conserver les instantanés Prometheus hebdomadaires des métriques Nexus pendant 12 mois.
- Enregistrer les modifications du runbook dans `docs/source/project_tracker/nexus_config_deltas/README.md` pour que les auditeurs sachent lorsque les responsabilités ont changé.

## Matériel lié- Vue d'ensemble : [Aperçu Nexus](./nexus-overview)
- Spécification : [spécification Nexus](./nexus-spec)
- Géométrie des voies : [Modèle de voie Nexus](./nexus-lane-model)
- Transition et shims de routage : [Nexus transition notes](./nexus-transition-notes)
- Onboarding opérateur : [Sora Nexus Operator onboarding](./nexus-operator-onboarding)
- Remédiation télémétrie : [Plan de remédiation télémétrie Nexus](./nexus-telemetry-remediation)