---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : Runbook pour le déploiement multi-modèles et les principaux fournisseurs
sidebar_label : déploiement du Runbook sur plusieurs modèles
description : Liste de contrôle des opérations pour le déploiement de plusieurs systèmes de configuration et les fournisseurs de services spécifiques à l'écran noir.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/runbooks/multi_source_rollout.md`. Il est possible de copier des copies de synchronisation si vous souhaitez créer des documents sans avoir à vous soucier de l'exploitation.
:::

## Назначение

Ce runbook produit SRE et nécessite des immobilisations pour les processus critiques :

1. Sélectionnez les contrôleurs d'opérateurs multi-histoires.
2. Réglez-vous dans votre magasin actuel ou résolvez les problèmes en priorité sans que les problèmes techniques soient résolus.

Avant, pour l'exploitation du secteur, dans les rames SF-6, vous devez effectuer (`sorafs_orchestrator`, passerelle API diapason, exportateurs télémétrie).

> **См. также:** [Runbook по эксплуатации оркестратора](./orchestrator-ops.md) подробно описывает процедуры на прогон (снятие scoreboard, переключатели поэтапного déploiement, restauration). Utilisez les solutions les plus adaptées à votre projet.

## 1. Validation préalable1. **Подтвердить входные данные gouvernance.**
   - Les candidats fournisseurs doivent publier les conversions `ProviderAdvertV1` avec la charge utile, les diapasons et les postes de travail. Vérifiez que vous disposez du `/v2/sorafs/providers` et assurez-vous que les produits sont correctement installés.
   - Les images télémétriques avec les mesures latentes et les problèmes doivent avoir lieu 15 minutes avant le programme Canary.
2. **Подготовить конфигурацию.**
   - Connectez l'opérateur de configuration JSON au site `iroha_config` :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Consultez JSON avec les limites du déploiement du module (`max_providers`, options de récupération). Utilisez tout cela pour la mise en scène/production, afin de pouvoir organiser des mini-productions.
3. **Programmer les appareils canoniques.**
   - Activez le manifeste/jeton d'extraction temporaire et activez la récupération de données :

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Les options d'extraction temporaires permettent de gérer le manifeste de charge utile Digest (hex) et les flux de données en base64 pour chaque fournisseur installé chez Canary.
   - Enregistrez `artifacts/canary.scoreboard.json` avec la réponse appropriée. Votre nouveau fournisseur ou votre fournisseur a un revenu >10 %.
4. **Проверить, что телеметрия подключена.**
   - Ouvrez l'exportation Grafana à `docs/examples/sorafs_fetch_dashboard.json`. Veuillez noter que les mesures `sorafs_orchestrator_*` ont été ajoutées à la mise en scène avant la production.

## 2. Les fournisseurs de services spéciaux dans le magasin noirDans le cadre de cette procédure, votre fournisseur devra effectuer des vérifications supplémentaires, en les utilisant correctement ou en ne procédant pas à des vérifications appropriées.1. **Зафиксировать доказательства.**
   - Exportez ensuite le fichier de récupération (via `--json-out`). Consultez les index des chaînes, des autres fournisseurs et des résumés nécessaires.
   - Enregistrez les logos des fragments pertinents sur les cibles `telemetry::sorafs.fetch.*`.
2. **Применить немедленный override.**
   - Indiquez au fournisseur qui est pénalisé dans le cadre de la télémétrie, avant l'opérateur (en utilisant le `penalty=true` ou le `token_health` du `0`). Le tableau de bord du tableau de bord est automatiquement fourni par le fournisseur.
   - Pour les détecteurs de fumée ad hoc, remplacez le `--deny-provider gw-alpha` par le `sorafs_cli fetch` afin de pouvoir ouvrir la porte sans utiliser la télémétrie.
   - Переразверните обновленный пакет телеметрии/configurations в затронутой среде (staging → canary → production). Prenez des mesures en cas d'incident quotidien.
3. **Provertir le remplacement.**
   - Повторите chercher le luminaire канонического. Assurez-vous que le tableau de bord soit fourni par un fournisseur non valide avec le numéro `policy_denied`.
   - Vérifiez le `sorafs_orchestrator_provider_failures_total` pour vérifier que le support est prêt pour le fournisseur externe.
4. **Эскалировать долгие блокировки.**
   - Si le fournisseur s'occupe du blocage >24 h, placez le document de gouvernance sur la rotation ou sur votre annonce. Lorsque vous utilisez la liste de refus et que vous activez les images télémétriques, vous ne pouvez pas accéder au tableau de bord.
5. **Протокол отката.**- Pour activer le fournisseur, vous devez choisir la liste de refus, activer et afficher le nouveau tableau de bord instantané. Prenez des mesures en cas d'incident post-mortem.

## 3. Planifier le déploiement de la plate-forme

| Faza | Охват | Signalisation | Critères Go/No-Go |
|------|-------|---------|-------------------|
| **Labo** | Claster d'intégration rapide | La CLI simple récupère les charges utiles | Si le processus est à 0, le taux de rendement est inférieur à 5 %. |
| **Mise en scène** | Plan de contrôle de mise en scène polonais | Tableau de bord Grafana inclus ; a émis des alertes dans le cadre d'un avertissement uniquement | `sorafs_orchestrator_active_fetches` ne s'applique pas après le programme de test ; aucune alerte `warn/critical`. |
| **Canari** | ≤10% de production | Téléavertisseur activé, avec surveillance télémétrique dans les environnements réels | Pour un rendement < 10 %, les taux de latence sont calculés selon la ligne de base de classification ±20 %. |
| **Общая доступность** | Déploiement à 100 % | Правила pager активны | Juste pour le `NoHealthyProviders` pendant 24 h, pour un retour stable, les panneaux SLA sur le bord du bord. |

Pour chaque épisode :1. Ouvrez l'opérateur JSON avec les plans `max_providers` et les récupérations de fichiers.
2. Lancez `sorafs_cli fetch` ou testez l'intégration du SDK sur le luminaire canonique et le manifeste représentatif des éléments.
3. Organisez le tableau de bord des artefacts + le résumé et utilisez les instructions ou la réponse.
4. Vérifiez les services téléphoniques du bord avant la période suivante.

## 4. Démarrage et crochets d'incident

- **Éléments :** Vérifiez qu'Alertmanager surveille `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. Il est normal que le fournisseur s'attaque à la dégradation.
- **Logiciel :** Sélectionnez la cible `telemetry::sorafs.fetch.*` dans votre agrégateur de journaux. Prenez les mesures nécessaires pour `event=complete status=failed`, afin de les utiliser trois fois.
- **Tableaux de bord :** Сохраняйте каждый artefact scoreboard в долговременное хранилище. JSON est également un outil de documentation pour le contrôle de la conformité et les fournisseurs de services.
- **Tableaux de bord :** Clonirуйте канонический Grafana-дэшbord (`docs/examples/sorafs_fetch_dashboard.json`) dans le produit avec les alertes `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. Communication et documentation- Enregistrez la modification deny/boost dans le journal des modifications en fonction des paramètres, de l'opérateur, du propriétaire et de l'incident personnel.
- Utilisez les commandes SDK pour la configuration de vos fournisseurs ou de vos retraits de clients, qui synchronisent la gestion du client en magasin.
- Après la mise en service de GA, lancez le déploiement `status.md` et téléchargez-le dans le runbook dans les versions fiables.