---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Déploiement du testnet vers SoraNet (SNNet-10)
sidebar_label : Déploiement de testnet (SNNet-10)
description : Plan d'activation des phases, kit d'intégration et portes de télémétrie pour la promotion du testnet de SoraNet.
---

:::note Fonte canonica
Cette page affiche le plan de déploiement SNNet-10 dans `docs/source/soranet/testnet_rollout_plan.md`. Mantenha ambas comme copies synchronisées.
:::

SNNet-10 coordonne l'activation des étapes de superposition des anonymats de SoraNet aujourd'hui au réseau. Utilisez ce plan pour traduire la feuille de route dans les livrables concrets, les runbooks et les portes de télémétrie pour que chaque opérateur entende les attentes avant que SoraNet ne vire ou ne transporte le pad.

## Phases de lancement

| Phase | Chronogramme (alvo) | Escopo | Artefatos obrigatorios |
|-------|---------|-------|----------|
| **T0 - Testnet téléchargé** | T4 2026 | 20-50 relais em >=3 ASN opérés par les contributeurs principaux. | Kit d'intégration Testnet, suite de fumée d'épinglage de garde, ligne de base de latence + mesures de PoW, journal de forage de baisse de tension. |
| **T1 - Bêta publique** | T1 2027 | >=100 relais, rotation de garde autorisée, liaison de sortie appliquée, SDK bêta compatible avec SoraNet avec `anon-guard-pq`. | Kit d'intégration actualisé, liste de contrôle de vérification des opérateurs, SOP de publication d'annuaire, paquet de tableaux de bord de télémétrie, rapports de répétition d'incidents. |
| **T2 - Padrao du réseau principal** | Q2 2027 (conditionné à compléter SNNet-6/7/9) | Rede de production padrao em SoraNet; transports obfs/MASQUE et application de PQ cliquet habilités. | Minutes d'approbation de la gouvernance, procédure de restauration directe uniquement, alarmes de déclassement, rapport aux mesures de réussite. |

Nao ha **caminho de salto** - chaque phase doit inclure la télémétrie et les artefatos de gouvernance de l'étape antérieure à la promotion.

## Kit d'intégration pour testnet

Chaque opérateur de relais reçoit un paquet déterministe avec les fichiers suivants :

| Artefato | Description |
|--------------|-------------|
| `01-readme.md` | CV, points de contact et chronogramme. |
| `02-checklist.md` | Liste de contrôle avant le vol (matériel, accessibilité du réseau, politique de vérification de la garde). |
| `03-config-example.toml` | Configuration minimale du relais + orchestrateur SoraNet compatible avec les blocs de conformité SNNet-9, y compris le bloc `guard_directory` qui corrige le hachage du dernier instantané de garde. |
| `04-telemetry.md` | Instructions pour relier les tableaux de bord de métriques de confidentialité de SoraNet et les seuils d'alerte. |
| `05-incident-playbook.md` | Procédure de réponse à une baisse de tension/déclassement avec une matrice d'escalade. |
| `06-verification-report.md` | Modèle que les opérateurs complètent et effectuent lorsque les tests de fumée sont réussis. |

Une copie rendue vive sur `docs/examples/soranet_testnet_operator_kit/`. Chaque promotion actualise le kit ; des numéros de versao accompagnent une phase (par exemple, `testnet-kit-vT0.1`).Pour les opérateurs de version bêta publique (T1), un bref résumé des prérequis de CV `docs/source/soranet/snnet10_beta_onboarding.md`, des informations de télémétrie et de flux d'envoi, fournies pour le kit déterministe et pour les aides à la validation.

`cargo xtask soranet-testnet-feed` génère le flux JSON qui ajoute à la période de promotion, la liste des relais, le rapport des mesures, les preuves d'exercices et les hachages des anexos référencés sur le modèle de stage-gate. Assine les journaux de forage et les anexos avec `cargo xtask soranet-testnet-drill-bundle` premier pour le registre d'alimentation `drill_log.signed = true`.

## Mesures de réussite

Une promotion entre les étapes de la télémétrie suivante, diffusée au minimum deux semaines :

- `soranet_privacy_circuit_events_total` : 95 % des circuits sont complets lors d'événements de baisse de tension ou de déclassement ; os 5% restantes ficam limitados pela oferta de PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : <1 % des sessions de récupération pour la période de coupure de courant du forum des exercices planifiés.
- `soranet_privacy_gar_reports_total` : variation à l'intérieur de +/-10 % du mélange attendu de catégories GAR ; picos doit être expliqué par les mises à jour de la politique approuvée.
- Taxes de réussite des billets PoW : >=99 % à l'intérieur de la semaine pendant 3 s ; signalé via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latence (95e percentile) par région : <200 ms lorsque les circuits sont totalement construits, capturés via `soranet_privacy_rtt_millis{percentile="p95"}`.

Modèles de tableaux de bord et d'alertes en direct sur `dashboard_templates/` et `alert_templates/` ; Nous n'avons pas votre dépôt de télémétrie et nous ajoutons aux contrôles de charpie de CI. Utilisez `cargo xtask soranet-testnet-metrics` pour gérer le rapport sur la gouvernance avant de solliciter une promotion.

Les soumissionnaires de stage-gate doivent suivre `docs/source/soranet/snnet10_stage_gate_template.md`, qui seront déposés pour le formulaire Markdown immédiatement pour copier le `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de vérification

Les opérateurs doivent assassiner la suite avant d'entrer dans chaque phase :

- [x] Relay advert assinado com o enveloppe d'admission actuelle.
- [x] Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) approuvé.
- [x] `guard_directory` posé pour l'artefato `GuardDirectorySnapshotV2` plus récent et `expected_directory_hash_hex` coïncide avec le résumé du comité (le démarrage, l'enregistrement du relais ou le hachage validé).
- [x] Metricas de PQ ratchet (`sorafs_orchestrator_pq_ratio`) permanent acima dos seuils également pour une phase sollicitée.
- [x] La configuration de conformité GAR correspond à la balise la plus récente (voir catalogue SNNet-9).
- [x] Simulacao d'alarme de downgrade (desativar collectors, esperar alerta em 5 min).
- [x] Drill PoW/DoS exécuté avec les étapes d'atténuation documentées.

Un modèle pré-installé est inclus dans le kit d'intégration. Les opérateurs soumettent un rapport complet au helpdesk de gouvernance avant de recevoir des accréditations de production.

## Gouvernance et reporting- **Contrôle des changements :** les promotions exigent l'approbation du Conseil de gouvernance, enregistrées dans les procès-verbaux du conseil et annexées à la page de statut.
- **Résumé de l'état :** publier les mises à jour semanaises, récapitulant les contages des relais, le ratio PQ, les incidents de baisse de tension et les éléments d'action pendants (armazenado em `docs/source/status/soranet_testnet_digest.md` quando a cadencia comecar).
- **Rollbacks :** contrôlez un plan de restauration supprimé qui revient au réseau pour une phase antérieure dans 30 minutes, y compris l'invalidation du DNS/cache de garde et des modèles de communication avec les clients.

## Actifs pris en charge

- `cargo xtask soranet-testnet-kit [--out <dir>]` matérialise le kit d'intégration de `xtask/templates/soranet_testnet/` pour le répertoire d'alvo (par défaut `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` est disponible en tant que métrique de réussite SNNet-10 et émet un rapport réussite/échec établi pour les révisions de gouvernance. Un instantané d'exemple vivant dans `docs/examples/soranet_testnet_metrics_sample.json`.
- Les modèles de Grafana et Alertmanager sont présents dans `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` ; copiez les systèmes dans le référentiel de télémétrie ou connectez-les aux contrôles de charpie du CI.
- Le modèle de communication de rétrogradation pour les messages du SDK/portail réside dans `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les résumés d'état hebdomadaires doivent être utilisés `docs/source/status/soranet_testnet_weekly_digest.md` comme forme canonique.

Les demandes d'extraction doivent mettre à jour cette page avec tout changement d'art ou de télémétrie pour que le plan de déploiement soit maintenu canonique.