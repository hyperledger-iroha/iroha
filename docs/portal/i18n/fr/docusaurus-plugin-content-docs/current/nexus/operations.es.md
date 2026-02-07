---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : Runbook des opérations de Nexus
description : Résumé de la liste du champ du flux de travail de l'opérateur de Nexus, qui reflète `docs/source/nexus_operations.md`.
---

Utilisez cette page comme l’homme de référence rapide de `docs/source/nexus_operations.md`. Reprenez la liste des opérateurs, les ganchos de gestion des changements et les exigences de couverture de télémétrie que les opérateurs de Nexus doivent suivre.

## Liste du cycle de vie| Étapa | Accions | Preuve |
|-------|--------|--------------|
| Pré-vuelo | Vérifiez les hachages/entreprises de lancement, confirmez `profile = "iroha3"` et préparez les plantes de configuration. | Salida de `scripts/select_release_profile.py`, registre de somme de contrôle, bundle de manifestes confirmés. |
| Alineación del catalogo | Actualisez le catalogue `[nexus]`, la politique de recrutement et les parapluies de DA en suivant le manifeste émis par le consommateur, et ensuite capturez `--trace-config`. | Sortie de `irohad --sora --config ... --trace-config` enregistrée avec le ticket d'embarquement. |
| Pruebas de humo y corte | Exécutez `irohad --sora --config ... --trace-config`, exécutez la vérification de l'humidité du CLI (`FindNetworkStatus`), validez les exportations de télémétrie et sollicitez l'admission. | Journal de test de fumée + confirmation d'Alertmanager. |
| État établi | Surveillez les tableaux de bord/alertes, les touches rotatives en fonction de la cadence de gestion et la synchronisation des configurations/runbooks lorsque vous modifiez les manifestes. | Minutes de révision trimestrielle, captures de tableaux de bord, identifiants de tickets de rotation. |

Le détail de l'intégration (remplacement des clés, des plantules de recrutement, des étapes du profil de lancement) est permanent sur `docs/source/sora_nexus_operator_onboarding.md`.

## Gestion des changements1. **Actualisations de lancement** - voir les annonces en `status.md`/`roadmap.md` ; ajoute la liste de contrôle d'intégration à chaque PR de sortie.
2. **Changements de manifestes de voie** - vérifier les bundles des entreprises du Space Directory et les archives sous `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuration** - chaque changement en `config/config.toml` nécessite un ticket faisant référence à la voie/espace de données. Gardez une copie rédigée de la configuration efficace lorsque les nœuds sont unan ou mis à jour.
4. **Simulacros de rollback** - suivre les procédures trimestrielles d'arrêt/restauration/fumée ; enregistré les résultats en `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Agréments de conformité** - les voies privées/CBDC doivent assurer la bonne visibilité avant de modifier la politique de DA ou les boutons de rédaction de télémétrie (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLO- Tableaux de bord : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, mais vues spécifiques du SDK (par exemple, `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et réglementation du transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métriques d'un vigile :
  - `nexus_lane_height{lane_id}` - alerte si aucun progrès n'est effectué pendant trois créneaux horaires.
  - `nexus_da_backlog_chunks{lane_id}` - alerte par encima de umbrales por lane (par défaut 64 publiques / 8 privées).
  - `nexus_settlement_latency_seconds{lane_id}` - alerte lorsque le P99 dépasse 900 ms (public) ou 1200 ms (privé).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerte si la tâche d'erreur 5 minutes dépasse 2%.
  - `telemetry_redaction_override_total` - Septembre 2 immédiat ; assurez-vous que les anulaciones tengan tickets de conformité.
- Exécutez la liste de contrôle de remédiation de la télémétrie dans le [plan de remédiation de la télémétrie de Nexus](./nexus-telemetry-remediation) au moins trimestriel et ajoutez le formulaire complété aux notes de révision des opérations.

## Matrice des incidents| Sévérité | Définition | Réponse |
|--------------|------------|--------------|
| 1 septembre | Brecha de aislamiento de data-space, paro de règlement >15 min o corruption de voto de gobernanza. | Pagear a Nexus Primary + Release Engineering + Compliance, admission congelée, recolectar artefactos, publicar comunicados  30 min, déploiement des manifestes échoué. | Pagear a Nexus Primary + SRE, mitigar <=4 h, suivis registrar en 2 jours habiles. |
| 3 septembre | Deriva no bloqueante (docs, alertas). | Enregistrer le tracker et planifier l'arreglo à l'intérieur du sprint. |

Les tickets d'incidents doivent être enregistrés comme identifiants de voie/espace de données affectés, hachages de manifeste, ligne de temps, mesures/journaux de support et comptes/propriétaires de suivi.

## Archivo de preuves

- Garder les bundles/manifestes/exportations de télémétrie sous `artifacts/nexus/<lane>/<date>/`.
- Conserver les configurations rédigées + la sortie `--trace-config` pour chaque version.
- Quelques minutes supplémentaires du conseil + décisions fermes lorsque vous changez de configuration ou de manifeste.
- Conserver les instantanés semanales de Prometheus pertinents pour les métriques de Nexus pendant 12 mois.
- Registre des éditions du runbook en `docs/source/project_tracker/nexus_config_deltas/README.md` pour que les auditeurs se chargent de modifier leurs responsabilités.

## Matériel lié- Résumé : [Aperçu Nexus](./nexus-overview)
- Spécification : [Spécification Nexus] (./nexus-spec)
- Géométrie des voies : [Modèle de voie Nexus](./nexus-lane-model)
- Transition et cales de routage : [Nexus notes de transition](./nexus-transition-notes)
- Intégration des opérateurs : [intégration de l'opérateur Sora Nexus](./nexus-operator-onboarding)
- Remédiation de télémétrie : [Nexus plan de remédiation de télémétrie](./nexus-telemetry-remediation)