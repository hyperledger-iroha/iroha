---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : Runbook des opéras Nexus
description : CV immédiat pour une utilisation dans le camp du flux de travail de l'opérateur Nexus, en particulier `docs/source/nexus_operations.md`.
---

Utilisez cette page comme référence rapide de `docs/source/nexus_operations.md`. Elle contient la liste de contrôle opérationnel, les guides de gestion de gestion et les exigences de couverture de télémétrie que les opérateurs Nexus doivent suivre.

## Liste du cycle de vie

| Étapa | Acoès | Preuve |
|-------|--------|--------------|
| Pré-voo | Vérifiez les hachages/assinaturas de release, confirmez `profile = "iroha3"` et préparez les modèles de configuration. | Saida de `scripts/select_release_profile.py`, journal de la somme de contrôle, bundle de manifestes assassinés. |
| Ajout du catalogue | Actualisez le catalogue `[nexus]`, la politique de rotation et les limites du DA conformément au manifeste émis par le conseil, et commencez à capturer `--trace-config`. | Saida de `irohad --sora --config ... --trace-config` s'est armazenada avec le ticket d'embarquement. |
| Fumée et basculement | Exécutez `irohad --sora --config ... --trace-config`, montez ou fumez la CLI (`FindNetworkStatus`), validez les exportations de télémétrie et sollicitez l'admission. | Journal du test de fumée + confirmation d'Alertmanager. |
| État d'état | Surveillez les tableaux de bord/alertes, la rotation des touches se conforme à la cadence de gouvernance et synchronise les configurations/runbooks lorsque les manifestes changent. | Minutes de révision trimestrielle, captures de tableaux de bord, identifiants de tickets de rotation. |Les détails d'intégration (remplacement des éléments, modèles de rotation, étapes du profil de publication) sont permanents dans `docs/source/sora_nexus_operator_onboarding.md`.

## Gestao de mudança

1. **Actualisations de la version** - accompagne les annonces dans `status.md`/`roadmap.md` ; annexe ou liste de contrôle d'intégration à chaque PR de sortie.
2. **Mudancas de manifesto de lane** - vérifiez les bundles assassinés du Space Directory et archivez-les sur `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuration** - Aujourd'hui, dans `config/config.toml`, demandez un ticket référençant une voie/un espace de données. Gardez une copie redigida de la configuration efficace lorsque nous y sommes entrés ou que nous l'avons mis à jour.
4. **Treinos de rollback** - suivre les procédures trimestrielles d'arrêt/restauration/fumée ; enregistrer les résultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Avenants de conformité** - les voies privées/CBDC doivent obtenir la conformité avant de modifier la politique de DA ou les boutons de réduction de télémétrie (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLO- Tableaux de bord : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mais des vues spécifiques du SDK (par exemple, `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et règles de transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métriques à observer :
  - `nexus_lane_height{lane_id}` - alerte pour zéro progression par trois emplacements.
  - `nexus_da_backlog_chunks{lane_id}` - alerte acima dos limiares por lane (padrao 64 publics / 8 privés).
  - `nexus_settlement_latency_seconds{lane_id}` - alerte lorsque le P99 dépasse 900 ms (public) ou 1200 ms (privé).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerte sur les taxons d'erreur de 5 minutes pendant >2 %.
  - `telemetry_redaction_override_total` - Septembre 2 immédiat ; garanta que annule les tickets de conformité de Tenham.
- Exécuter la liste de contrôle de correction de la télémétrie n° [Nexus plan de correction de télémétrie] (./nexus-telemetry-remediation) pour le moins de trimestre et l'annexe du formulaire prévu dans les notes de révision opérationnelle.

## Matrice des incidents| Sévérité | Définition | Réponse |
|--------------|------------|--------------|
| 1 septembre | Violation de l'isolement de l'espace de données, parada de règlement >15 min, ou corruption du vote de gouvernance. | Action Nexus Primary + Release Engineering + Compliance, congélation admise, colete artefatos, communication publique 30 min, déploiement du manifeste falho. | Acione Nexus Primaire + SRE, mitigue <=4 h, suivis de registre à 2 jours de l'utérus. |
| 3 septembre | Deriva nao bloqueante (docs, alertas). | Enregistrez-vous sur le tracker et l'agenda de correspondance dans le sprint. |

Les tickets d'incident développent des identifiants de registraire de voie/espace de données confirmés, des hachages de manifeste, une chronologie, des mesures/journaux de support et des tarifs/propriétaires de suivi.

## Archivage des preuves

- Bundles/manifestes/exportations de télémétrie Armazene dans `artifacts/nexus/<lane>/<date>/`.
- Mantenha configs redigidas + Saida de `--trace-config` pour chaque version.
- Anexe minutas do conselho + decisoes assinadas quando mudancas de config ou manifesto ocorrerem.
- Conserver les instantanés semanais de Prometheus pertinents pour les mesures Nexus pendant 12 mois.
- Enregistrez les éditeurs du runbook dans le `docs/source/project_tracker/nexus_config_deltas/README.md` pour que les auditeurs saibam assument leurs responsabilités.

## Matériel lié- Visa général : [Aperçu Nexus](./nexus-overview)
- Spécification : [spécification Nexus] (./nexus-spec)
- Géométrie des voies : [Modèle de voie Nexus](./nexus-lane-model)
- Transmission et cales de rotation : [Notes de transition Nexus](./nexus-transition-notes)
- Intégration des opérateurs : [intégration de l'opérateur Sora Nexus](./nexus-operator-onboarding)
- Correction de télémétrie : [Plan de correction de télémétrie Nexus](./nexus-telemetry-remediation)