---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Déploiement du testnet de SoraNet (SNNet-10)
sidebar_label : Déplier le testnet (SNNet-10)
description : Plan d'activation des phases, kit d'intégration et portes de télémétrie pour les promotions de testnet de SoraNet.
---

:::note Fuente canonica
Cette page reflète le plan de déploiement SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Mantengan ambas copias alineadas hasta que los docs heredados se retraiten.
:::

SNNet-10 coordonne l’activation de la superposition anonymisée de SoraNet en rouge. Utilisez ce plan pour traduire la puce de la feuille de route dans des éléments concrets, des runbooks et des portes de télémétrie pour que chaque opérateur réponde aux attentes avant que SoraNet effectue le transport par défaut.

## Phases de lancement

| Phase | Chronogramme (objetivo) | Alcance | Artefacts requis |
|-------|---------|-------|----------|
| **T0 - Cerrado Testnet** | T4 2026 | 20-50 relais et >=3 ASN opérés par les contributeurs principaux. | Kit d'intégration du testnet, suite de fumée d'épinglage de garde, ligne de base de latence + métriques de PoW, journal de forage de baisse de tension. |
| **T1 - Bêta publique** | T1 2027 | >=100 relais, rotation de garde autorisée, liaison de sortie appliquée, SDK bêta par défaut sur SoraNet avec `anon-guard-pq`. | Kit d'intégration actualisé, liste de contrôle de vérification des opérateurs, SOP de publication d'annuaire, paquet de tableaux de bord de télémétrie, rapports de répétition d'incidents. |
| **T2 - Réseau principal par défaut** | Q2 2027 (conditionné à compléter SNNet-6/7/9) | Le rouge de production par défaut sur SoraNet ; transports obfs/MASQUE et application de PQ cliquet habilités. | Minutes d'approbation de la gouvernance, procédure de restauration directe uniquement, alarmes de déclassement, rapport confirmé des mesures de sortie. |

Pas de **ruta de salto** : chaque étape doit être entregar la télémétrie et les artefacts de gouvernance de l'étape antérieure à la promotion.

## Kit d'intégration de testnet

Chaque opérateur de relais reçoit un paquet déterministe avec les archives suivantes :

| Artefact | Description |
|--------------|-------------|
| `01-readme.md` | CV, points de contact et chronogramme. |
| `02-checklist.md` | Liste de contrôle avant le vol (matériel, accessibilité du rouge, politique de vérification de la garde). |
| `03-config-example.toml` | Configuration minimale du relais + orchestrateur SoraNet alignée avec des blocs de conformité SNNet-9, y compris un bloc `guard_directory` qui fixe le hachage de l'instantané de garde le plus récent. |
| `04-telemetry.md` | Instructions pour câbler les tableaux de bord de métriques de confidentialité de SoraNet et les seuils d'alerte. |
| `05-incident-playbook.md` | Procédure de réponse en cas de baisse de tension/déclassement avec matrice d'escalade. |
| `06-verification-report.md` | Installation pour que les opérateurs effectuent et réalisent une fois les tests de fumée effectués. |

Une copie rendue vive en `docs/examples/soranet_testnet_operator_kit/`. Chaque kit de promotion de rafraîchissement ; les numéros de version suivent la phase (par exemple, `testnet-kit-vT0.1`).Pour les opérateurs de version bêta publique (T1), le bref résumé en `docs/source/soranet/snnet10_beta_onboarding.md` reprend les prérequis, les éléments de télémétrie et le flux d'envoi, en mettant en évidence le kit déterministe et les aides à la validation.

`cargo xtask soranet-testnet-feed` génère le flux JSON qui ajoute la fenêtre de promotion, la liste des relais, les rapports de mesures, les preuves d'exercices et les hachages des accessoires référencés par la plante de la porte de scène. Fermez les journaux de forage et les accessoires avec `cargo xtask soranet-testnet-drill-bundle` en premier pour le registre d'alimentation `drill_log.signed = true`.

## Mesures de sortie

La promotion entre les phases est autorisée avec la télémétrie suivante, enregistrée pendant un minimum de deux semaines :

- `soranet_privacy_circuit_events_total` : 95 % des circuits sont terminés sans cas de baisse de tension ou de déclassement ; Les 5% restants sont limités à la fourniture PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : <1 % des sessions de récupération pour l'activation de la baisse de tension hors des exercices programmés.
- `soranet_privacy_gar_reports_total` : variation à l'intérieur de +/-10 % du mélange prévu pour les catégories GAR ; Les picos doivent être expliqués par les mises à jour de la politique approuvée.
- Tasa de exito de tickets PoW: >=99% dentro de la ventana objetivo de 3 s ; rapporté via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latence (pourcentage 95) par région : <200 ms une fois que les circuits sont complètement armés, capturés via `soranet_privacy_rtt_millis{percentile="p95"}`.

Les modèles de tableaux de bord et d'alertes sont présents en `dashboard_templates/` et `alert_templates/` ; réfléchissez à votre dépôt de télémétrie et agrégez les chèques de charpie de CI. Utilisez `cargo xtask soranet-testnet-metrics` pour générer le rapport de cara sur la gouvernance avant de faire la promotion.

Les présentations de stage-gate doivent suivre `docs/source/soranet/snnet10_stage_gate_template.md`, qui ajoute la liste Markdown du formulaire pour copier sous `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de vérification

Les opérateurs doivent essayer la suivante avant d'entrer dans chaque étape :

- ✅Relais d'annonce confirmé avec l'enveloppe d'admission actuelle.
- ✅ Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) approuvé.
- ✅ `guard_directory` apunta al artefacto `GuardDirectorySnapshotV2` plus récent et `expected_directory_hash_hex` coïncident avec le résumé du comité (l'organisation du relais enregistre le hachage validé).
- ✅ Les mesures du cliquet PQ (`sorafs_orchestrator_pq_ratio`) sont maintenues par l'encima des seuils objetivo pour l'étape sollicitée.
- ✅ La configuration de conformité GAR coïncide avec la balise la plus récente (voir le catalogue SNNet-9).
- ✅ Simulation d'alarme de déclassement (déshabiliter les collecteurs, attendre l'alerte en 5 min).
- ✅ Drill PoW/DoS exécuté avec des étapes d'atténuation documentées.

Une plante prête à être incluse dans le kit d'intégration. Les opérateurs souhaitent que le rapport soit complété sur la table d'aide à la gouvernance avant de recevoir des informations d'identification de production.

## Gouvernance et rapports- **Contrôle des changements :** les promotions nécessitent l'approbation du Conseil de gouvernance enregistrée dans les minutes du conseil et ajoutées à la page de l'état.
- **Resumen de l'état :** publier les mises à jour sémanales du résumé du contenu des relais, le ratio PQ, les incidents de baisse de tension et les éléments d'action pendants (se stockent sur `docs/source/status/soranet_testnet_digest.md` lorsque la cadence est lancée).
- **Rollbacks :** maintenez un plan de restauration ferme qui réinitialise le rouge à la phase précédente dans 30 minutes, y compris l'invalidation du cache DNS/guard et les plantules de communication avec les clients.

## Actifs de support

- `cargo xtask soranet-testnet-kit [--out <dir>]` matérialise le kit d'intégration à partir de `xtask/templates/soranet_testnet/` vers le répertoire de destination (par défaut `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` évalue les mesures de sortie de SNNet-10 et émet un rapport structuré réussite/échec adapté aux révisions de gouvernance. Un instantané de l'exemple vivant en `docs/examples/soranet_testnet_metrics_sample.json`.
- Les plantules de Grafana et Alertmanager vivent en `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` ; copiez votre dépôt de télémétrie ou connectez-vous aux chèques de charpie en CI.
- La plante de communication de rétrogradation pour les messages du SDK/portail réside en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les résumés semanales de l'état doivent utiliser `docs/source/status/soranet_testnet_weekly_digest.md` comme forme canonique.

Les pull request doivent actualiser cette page avec tout changement d'artefacts ou de télémétrie pour que le plan de déploiement soit maintenu canonique.