---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : opérations de connexion
titre : Runbook pour l'utilisation Nexus
description : Практичный полевой обзор рабочего процесса operatorora Nexus, отражающий `docs/source/nexus_operations.md`.
---

Utilisez cette section pour désactiver le bouton `docs/source/nexus_operations.md`. Dans le cadre de la liste de contrôle d'exploitation du centre, qui contrôle les installations et les travaux de télémétrie, ceux-ci doivent être occupés par les opérateurs. Nexus.

## Liste de contrôle du cycle de vie

| ÉTAPE | Fête | Доказательства |
|-------|--------|--------------|
| Avant-démarrage | Vérifiez votre réponse, mettez à jour `profile = "iroha3"` et configurez les paramètres de votre ordinateur. | Vous obtenez `scripts/select_release_profile.py`, la somme de contrôle du journal et les manifestes du bundle. |
| Liste des catalogues | Consultez le catalogue `[nexus]`, qui concerne le marché politique et le projet de loi sur la manifestation, pour obtenir `--trace-config`. | Vous êtes `irohad --sora --config ... --trace-config`, actuellement en contact avec l'intégration du billet. |
| Fumée et basculement | Utilisez `irohad --sora --config ... --trace-config`, activez la fumée CLI (`FindNetworkStatus`), vérifiez les téléphones d'exportation et activez l'admission. | Лог smoke-test + подтверждение Alertmanager. |
| Régime d'État | Accédez aux tableaux de bord/alertes, faites pivoter les boutons de gouvernance graphique et synchronisez les configurations/runbooks pour la création de gestionnaires. | Les protocoles d'observation, les certificats de bord, les billets d'identité en rotation. |L'intégration avancée (clé de contact, étapes de marketing et profil de profil) se déroule dans `docs/source/sora_nexus_operator_onboarding.md`.

## Mise à jour des modifications

1. **Обновления реLISа** - следите за объявлениями в `status.md`/`roadmap.md` ; Прикладывайте чек-list onboarding к каждому PR реLISа.
2. **Изменения манифестов lane** - vérifiez les bundles à partir du Space Directory et archivez-les dans `docs/source/project_tracker/nexus_config_deltas/`.
3. **Дельты конфигурации** - каждое изменение `config/config.toml` требует тикет ссылкой на voie/données-espace. Essayez de modifier la copie efficace des configurations avant de rejoindre/mettre à niveau vos utilisateurs.
4. **Тренировки rollback** - ежеквартально репетируйте arrêter/restaurer/fumer ; Fixez les résultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Conformité de conformité** - les voies privées/CBDC doivent être mises en conformité avant la modification de la politique ou des boutons de révision télémétrique (voir `docs/source/cbdc_lane_playbook.md`).

## Télémétrie et SLO- Cartes : `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, ainsi que des vidéos spécifiques au SDK (par exemple, `android_operator_console.json`).
- Alertes : `dashboards/alerts/nexus_audit_rules.yml` et transport Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Paramètres de surveillance :
  - `nexus_lane_height{lane_id}` - alerte lors de l'arrêt de la progression de trois emplacements.
  - `nexus_da_backlog_chunks{lane_id}` - vous alerte sur la voie (pour 64 publiques / 8 privées).
  - `nexus_settlement_latency_seconds{lane_id}` - L'alerte du code P99 est de 900 ms (publique) ou 1 200 ms (privée).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerte si 5 minutes pour un ordinateur >2%.
  - `telemetry_redaction_override_total` - Sev 2 немедленно ; убедитесь, что annule la conformité des billets.
- Vérifiez la liste télémétrique du [plan de remédiation de télémétrie Nexus] (./nexus-telemetry-remediation) au minimum et indiquez le formulaire prévu à cet effet. заметкам операционного обзора.

## Matrices d'incidents| Gravité | Présentation | Ответ |
|--------------|------------|--------------|
| 1 septembre | Нарушение изоляции data-space, остановка règlement >15 minutes ou pour la gouvernance. | Пейджинг Nexus Primary + Release Engineering + Compliance, заморозить l'admission, собрать артефакты, выпустить коммуникации  30 minutes, manifeste de déploiement éprouvé. | Selon Nexus Primaire + SRE, смягчение <=4 heures, former des suivis en течение 2 рабочих дней. |
| 3 septembre | Не блокирующий дрейф (documents, alertes). | Connectez-vous au tracker et planifiez l'installation dans le sprint. |

Les incidents permettant de définir la voie d'identification/l'espace de données, les gestionnaires, les tâches, les mesures/logines et le suivi задачи/ответственных.

## Архив доказательств

- Envoyer des bundles/manifestes/télémétries de sport dans `artifacts/nexus/<lane>/<date>/`.
- Supprimez les configurations + passez `--trace-config` pour chaque version.
- Appliquez le protocole correspondant + les solutions nécessaires à la configuration ou au manifeste.
- Enregistrez les instantanés habituels Prometheus selon la mesure Nexus en 12 mois.
- Fixez les paramètres du runbook dans `docs/source/project_tracker/nexus_config_deltas/README.md`, pour que les auditeurs soient disponibles, afin de faciliter l'exploitation.

## Matériaux suisses- Objet : [Présentation Nexus](./nexus-overview)
- Spécifications : [Spécification Nexus] (./nexus-spec)
- Voie géométrique : [Modèle de voie Nexus](./nexus-lane-model)
- Précédents et cales de routage : [Notes de transition Nexus](./nexus-transition-notes)
- Intégration des opérateurs : [intégration de l'opérateur Sora Nexus] (./nexus-operator-onboarding)
- Correction télémétrique : [Plan de correction de la télémétrie Nexus](./nexus-telemetry-remediation)