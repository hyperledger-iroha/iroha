---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Déployer testnet SoraNet (SNNet-10)
sidebar_label : Lancez testnet (SNNet-10)
la description : Plan d'action commercial, kit d'intégration et portes de télémétrie pour le testnet SoraNet.
---

:::note Канонический источник
Cette page prévoit le déploiement du plan SNNet-10 dans `docs/source/soranet/testnet_rollout_plan.md`. Vous pouvez obtenir des copies synchronisées si vous souhaitez télécharger des documents, mais vous ne pouvez pas les utiliser.
:::

SNNet-10 permet d'activer la superposition d'anonymat SoraNet sur tous les paramètres. Utilisez ce plan pour préparer la feuille de route dans les livrables concrets, les runbooks et les portes de télémétrie, pour que l'opérateur s'occupe de tout, comme le prévoit SoraNet. transport à destination.

## Фазы запуска

| Faza | Таймлайн (цель) | Objet | Objets d'art |
|-------|---------|-------|----------|
| **T0 – Réseau de test fermé** | T4 2026 | 20 à 50 relais pour >=3 ASN, contributeurs principaux. | Kit d'intégration Testnet, suite de fumée d'épinglage de garde, latence de base + métriques PoW, journal d'exercice de baisse de tension. |
| **T1 – Bêta publique** | T1 2027 | >=100 relais, rotation de garde activée, liaison de sortie activée, versions bêta du SDK pour l'utilisation de SoraNet avec `anon-guard-pq`. | Kit d'intégration complet, liste de contrôle de vérification des opérateurs, SOP de publication d'annuaire, pack de tableaux de bord de télémétrie, rapports de répétition d'incidents. |
| **T2 - Réseau principal par défaut** | T2 2027 (selon l'état de la mise à jour SNNet-6/7/9) | Production сеть по умолчанию SoraNet ; включены obfs/MASQUE transports et application du cliquet PQ. | Gouvernance d'approbation des protocoles, procédure de restauration directe uniquement, alarmes de déclassement, rapport de mesures de réussite avancé. |

**Пути пропуска нет** - каждая фаза обязана доставить la télémétrie et les artefacts de gouvernance avant la mise en place des stades.

## Kit d'intégration Testnet

L'opérateur de relais fournit un paquet de mesures pour les familles :

| Artefact | Description |
|--------------|-------------|
| `01-readme.md` | Обзор, контакты и таймлайн. |
| `02-checklist.md` | Liste de contrôle avant le vol (matériel, accessibilité du réseau, vérification de la politique de garde). |
| `03-config-example.toml` | La configuration minimale du relais + orchestrateur SoraNet est associée aux blocs de conformité SNNet-9, notamment le bloc `guard_directory`, avec la broche de hachage après l'instantané de garde. |
| `04-telemetry.md` | Instructions pour la fourniture des tableaux de bord des métriques de confidentialité SoraNet et des seuils d'alerte. |
| `05-incident-playbook.md` | La procédure de réaction en cas de baisse de tension/déclassement avec une augmentation matérielle. |
| `06-verification-report.md` | Shablon, l'opérateur s'occupe et effectue des tests de fumée après la procédure. |

La copie est disponible dans `docs/examples/soranet_testnet_operator_kit/`. Каждое повышение обновляет kit ; Le numéro de la version correspond à la phase suivante (par exemple, `testnet-kit-vT0.1`).

Pour les opérateurs de la version bêta publique (T1), le brief d'intégration du `docs/source/soranet/snnet10_beta_onboarding.md` résume les prérequis, les livrables de télémétrie et les fonctionnalités de flux de travail, en utilisant un kit de détermination et des aides de validation.`cargo xtask soranet-testnet-feed` génère un flux JSON, une fenêtre de promotion, une liste de relais, un rapport de métriques, des preuves d'exploration et des hachages de pièces jointes, sur votre modèle de porte d'étape créé. Pour obtenir des journaux de forage et des accessoires à partir de `cargo xtask soranet-testnet-drill-bundle`, vous devez alimenter le `drill_log.signed = true`.

## Метрики успеха

Avant d'avoir des connaissances sur la télémétrie secondaire, il y a peu de choses à savoir :

- `soranet_privacy_circuit_events_total` : 95 % des circuits sont protégés contre les baisses de tension ou les déclassements ; оставшиеся 5% ограничены approvisionnement PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : <1 % de sessions de récupération lors du déclenchement d'une baisse de tension lors des exercices planifiés.
- `soranet_privacy_gar_reports_total` : variation de +/-10 % de la catégorie mixte GAR ; Les membres de votre famille doivent recevoir des mises à jour de politique.
- Успешность PoW tickets : >=99 % en 3 s ; rapport ci-dessous `soranet_privacy_throttles_total{scope="congestion"}`.
- Latence (95e centile) pour la région : <200 ms après la mise en service des circuits, comme `soranet_privacy_rtt_millis{percentile="p95"}`.

Modèles de tableau de bord et d'alerte disponibles dans `dashboard_templates/` et `alert_templates/` ; Ouvrez votre référentiel de télémétrie et effectuez les contrôles de charpie CI. Utilisez `cargo xtask soranet-testnet-metrics` pour générer des informations sur la gouvernance avant la mise en œuvre du projet.

Les soumissions Stage-gate doivent être soumises à `docs/source/soranet/snnet10_stage_gate_template.md`, en utilisant le formulaire Markdown dans `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Liste de contrôle de vérification

Les opérateurs devraient vous aider à le faire avant votre arrivée dans cette zone :

- ✅ Annonce relais подписан текущим enveloppe d'admission.
- ✅ Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) проходит.
- ✅ `guard_directory` est téléchargé sur l'artefact `GuardDirectorySnapshotV2` et `expected_directory_hash_hex` ajouté au résumé du comité (avant de démarrer le journal de relais, le hachage vérifié).
- ✅ Les mesures à cliquet PQ (`sorafs_orchestrator_pq_ratio`) sont utiles pour vos stades de pointe.
- ✅ La configuration de conformité GAR est compatible avec la balise suivante (dans le catalogue SNNet-9).
- ✅ Simulation d'alarme de déclassement (ouvrir les collecteurs, désactiver l'alerte en 5 minutes).
- ✅ L'exercice PoW/DoS permet de documenter l'atténuation des risques.

Pré-installé le kit d'intégration. Les opérateurs se tournent vers le service d'assistance en matière de gouvernance avant d'obtenir les informations d'identification de production.

## Gouvernance et отчетность

- **Contrôle des changements :** les promotions nécessitent l'approbation du Conseil de gouvernance, зафиксированный в procès-verbal du conseil et приложенный к page d'état.
- **Résumé de l'état :** publier des informations détaillées sur les relais de charge, le rapport PQ, les baisses de tension et les éléments d'action (dans `docs/source/status/soranet_testnet_digest.md` après le démarrage) cadences).
- **Rollbacks :** vous pouvez ajouter un plan de restauration, définir un délai de 30 minutes avant l'invalidation du cache DNS/guard et les modèles de communication client.

## Actifs pris en charge- Le kit d'intégration `cargo xtask soranet-testnet-kit [--out <dir>]` est disponible dans le catalogue `xtask/templates/soranet_testnet/` (en utilisant `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` évalue les mesures de réussite de SNNet-10 et vérifie la réussite/l'échec des examens de gouvernance. Le premier instantané apparaît dans `docs/examples/soranet_testnet_metrics_sample.json`.
- Les modèles Grafana et Alertmanager sont disponibles dans `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` ; Enregistrez le référentiel de télémétrie ou effectuez les contrôles de charpie CI.
- La communication de mise à niveau inférieure pour la messagerie SDK/portail est disponible dans `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les résumés de statut doivent utiliser `docs/source/status/soranet_testnet_weekly_digest.md` sous la forme canonique.

Les requêtes d'extraction permettent de traiter cette partie des artefacts ou de la télémétrie, pour lesquels le plan de déploiement est établi de manière canonique.