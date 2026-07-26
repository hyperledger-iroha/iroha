---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Déploiement du testnet SoraNet (SNNet-10)
sidebar_label : Déploiement du testnet (SNNet-10)
description : Plan d'activation par phases, kit d'onboarding et gates de télémétrie pour les promotions du testnet SoraNet.
---

:::note Source canonique
Cette page reflète le plan de déploiement SNNet-10 dans `docs/source/soranet/testnet_rollout_plan.md`. Gardez les deux copies alignées jusqu'à la retraite des documents historiques.
:::

SNNet-10 coordonne l'activation par étapes de la superposition d'anonymat SoraNet sur tout le réseau. Utilisez ce plan pour traduire le bullet de roadmap en livrables concrets, runbooks et gates de télémétrie afin que chaque opérateur comprenne les attentes avant que SoraNet devienne le transport par défaut.

## Phases de lancement

| Phases | Calendrier (cible) | Portee | Artefacts requis |
|-------|---------|-------|----------|
| **T0 - Testnet ferme** | T4 2026 | 20-50 relais sur >=3 ASN fonctionnent par des contributeurs principaux. | Kit d'intégration testnet, suite de fumée de guard pinning, base de latence + métriques PoW, journal de baisse de tension. |
| **T1 - Bêta publique** | T1 2027 | >=100 relais, rotation de garde active, imposition de liaison de sortie, SDK bêta par défaut sur SoraNet avec `anon-guard-pq`. | Kit d'onboarding mis à jour, checklist de vérification opérateur, SOP de publication du répertoire, pack de tableaux de bord de télémétrie, rapports de répétition d'incident. |
| **T2 - Réseau principal par défaut** | T2 2027 (conditionné à l'achèvement SNNet-6/7/9) | Le réseau de production passe par défaut sur SoraNet ; transports obfs/MASQUE et application du PQ cliquet actifs. | Procès-verbal d'approbation de gouvernance, procédure de rollback direct uniquement, alarmes de downgrade, rapport signé de métriques de succès. |

Il n'y a **aucune voie de saut** - chaque phase doit livrer la télémétrie et les artefacts de gouvernance de l'étape précédent avant promotion.

## Kit d'intégration du testnet

Chaque opérateur de relais reçoit un package déterministe avec les fichiers suivants :

| Artefact | Descriptif |
|--------------|-------------|
| `01-readme.md` | Vue d'ensemble, points de contact et calendrier. |
| `02-checklist.md` | Liste de contrôle avant le vol (matériel, réseau d'accessibilité, politique de vérification de la garde). |
| `03-config-example.toml` | Configuration minimale relais + orchestrateur SoraNet alignée sur les blocs de conformité SNNet-9, incluant un bloc `guard_directory` qui fixe le hash du dernier snapshot guard. |
| `04-telemetry.md` | Instructions pour câbler les tableaux de bord de métriques de confidentialité SoraNet et les seuils d'alerte. |
| `05-incident-playbook.md` | Procédure de réponse à une baisse de tension/downgrade avec matrice d'escalade. |
| `06-verification-report.md` | Modèle que les opérateurs remplissent et retournent une fois les tests de fumée valides. |

Une copie rendue est disponible dans `docs/examples/soranet_testnet_operator_kit/`. Chaque promotion rafraichit le kit; les numéros de version suivent la phase (par exemple, `testnet-kit-vT0.1`).Pour les opérateurs public-beta (T1), le brief concis dans `docs/source/soranet/snnet10_beta_onboarding.md` reprend les prérequis, livrables de télémétrie et le workflow de soumission tout en renvoyant vers le kit déterministe et les helpers de validation.

`cargo xtask soranet-testnet-feed` génère le flux JSON qui agrége la fenêtre de promotion, le roster de relais, le rapport de métriques, les preuves de forets et les hachages des pièces jointes références par le template stage-gate. Signez d'abord les grumes de forage et les pièces jointes avec `cargo xtask soranet-testnet-drill-bundle` afin que le flux enregistre `drill_log.signed = true`.

## Métriques de succès

La promotion entre phases est conditionnée par la télémétrie suivante, collectée pendant au moins deux semaines :

- `soranet_privacy_circuit_events_total` : 95% des circuits se terminent sans baisse de tension ni downgrade ; les 5% restants sont limites par l'offre PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : <1% des sessions de fetch par jour declenchent un brownout en dehors des drills planifies.
- `soranet_privacy_gar_reports_total` : variance dans +/-10% du mix attendu de catégories GAR ; les photos doivent être expliquées par des mises à jour de politique approuvées.
- Taux de réussite des tickets PoW : >=99% dans la fenêtre cible de 3 s ; rapport via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latence (95e percentile) par région : <200 ms une fois les circuits complétés établis, capturé via `soranet_privacy_rtt_millis{percentile="p95"}`.

Les modèles de tableaux de bord et d'alertes figurent dans `dashboard_templates/` et `alert_templates/` ; recopiez-les dans votre repo de télémétrie et ajoutez-les aux checks de lint CI. Utilisez `cargo xtask soranet-testnet-metrics` pour générer le rapport oriente gouvernance avant de demander la promotion.

Les soumissions stage-gate doivent suivre `docs/source/soranet/snnet10_stage_gate_template.md`, qui renvoie vers le formulaire Markdown prêt à copier sous `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de vérification

Les opérateurs doivent valider les points suivants avant d'entrer dans chaque phase :

- ✅Relais annonce signe avec l'admission enveloppe courant.
- ✅ Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) valide.
- ✅ `guard_directory` pointe vers le dernier artefact `GuardDirectorySnapshotV2` et `expected_directory_hash_hex` correspondent au digest du comité (le démarrage du relais journalise le hash valide).
- ✅ Les métriques de cliquet PQ (`sorafs_orchestrator_pq_ratio`) restent au-dessus des seuils cibles pour l'étape demandée.
- ✅ La config de conformité GAR correspond au dernier tag (voir le catalogue SNNet-9).
- ✅ Simulation d'alarme downgrade (désactiver les collecteurs, attendre une alerte sous 5 min).
- ✅ Drill PoW/DoS exécuté avec des étapes d'atténuation documentées.

Un modèle pré-rempli est inclus dans le kit d'onboarding. Les opérateurs soumettent le rapport complet au helpdesk gouvernance avant de recevoir des informations d'identification de production.

## Gouvernance et reporting- **Change control:** les promotions exigent une approbation du Governance Council enregistrée dans les minutes du conseil et jointe à la page de statut.
- **Status digest:** publier des mises à jour hebdomadaires reprenant le nombre de relais, le ratio PQ, les incidents baisse de tension et les actions items en attente (stocke dans `docs/source/status/soranet_testnet_digest.md` une fois la cadence lancée).
- **Rollbacks :** maintient un plan de rollback signé qui ramène le réseau à la phase précédente en 30 minutes, incluant l'invalidation DNS/guard cache et des templates de communication client.

## Actifs de support

- `cargo xtask soranet-testnet-kit [--out <dir>]` matérialise le kit d'onboarding depuis `xtask/templates/soranet_testnet/` vers le répertoire cible (par défaut `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` évalue les métriques de succès SNNet-10 et emet un rapport structure pass/fail adapté à la gouvernance des revues. Un instantané d'exemple vit dans `docs/examples/soranet_testnet_metrics_sample.json`.
- Les modèles Grafana et Alertmanager vivent sous `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` ; copiez-les dans votre repo de télémétrie ou branchez-les dans les checks de lint CI.
- Le modèle de communication downgrade pour les messages SDK/portal réside dans `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les résumés de statut hebdomadaires doivent utiliser `docs/source/status/soranet_testnet_weekly_digest.md` comme forme canonique.

Les pull request doivent mettre à jour cette page avec tout changement d'artefacts ou de télémétrie afin que le plan de déploiement reste canonique.