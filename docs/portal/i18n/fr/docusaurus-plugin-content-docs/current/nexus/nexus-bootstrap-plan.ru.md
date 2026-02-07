---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : Bootstrap et démarrez Sora Nexus
description : Le plan d'exploitation contient les validateurs de classe de base Nexus avant la mise à disposition des services SoraFS et SoraNet.
---

:::note Канонический источник
Cette page correspond à `docs/source/soranexus_bootstrap_plan.md`. Vous pouvez télécharger des copies de synchronisation si les versions localisées ne sont pas publiées sur le portail.
:::

# Plan Bootstrap et Observabilité Sora Nexus

## Celi
- Choisissez parmi les validateurs/utilisateurs Sora Nexus avec les clés de gouvernance, l'API Torii et la surveillance du consensus.
- Vérifier les services de clé (Torii, consensus, persistance) en activant les déploiements SoraFS/SoraNet.
- Gérer les workflows CI/CD et les tableaux de bord/alertes d'observabilité pour chaque ensemble.

## Préparatifs
- Clés de gouvernance matérielles (système multisig, clés de comité) téléchargées dans HSM ou Vault.
- Infrastructure de base (claviers Kubernetes ou installations bare-metal) dans les régions primaires/secondaires.
- Configuration d'amorçage avancée (`configs/nexus/bootstrap/*.toml`), permettant de définir les paramètres réels.## Сетевые окружения
- Utilisez la commande Nexus avec les paramètres suivants :
- **Sora Nexus (réseau principal)** - Les préfixes produits sont `nexus`, permettant l'installation canonique et les services de ferroutage SoraFS/SoraNet (ID de chaîne `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - Paramètres de mise en scène définis `testus`, configuration du réseau principal pour les tests d'intégration et les validations préalables à la version (chaîne UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Découvrez les principaux événements de la genèse, les clés de gouvernance et les empreintes d'infrastructure pour la réalisation du calendrier. Testus est un polygone pour tous les déploiements SoraFS/SoraNet avant la production dans Nexus.
- Les pipelines CI/CD doivent être déployés dans Testus, effectuer des tests de fumée automatiques et effectuer une promotion rapide dans Nexus après la procédure. проверок.
- Les bundles de configuration de référence sont disponibles dans `configs/soranexus/nexus/` (réseau principal) et `configs/soranexus/testus/` (testnet), pour que vous puissiez utiliser `config.toml`, `genesis.json` et admission des catalogues Torii.## Partie 1 - Révision de la configuration
1. Аудировать существующую документацию:
   - `docs/source/nexus/architecture.md` (consensus, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigences infrarouges).
   - `docs/source/nexus/governance_keys.md` (clé de commande).
2. Vérifiez que les fichiers Genesis (`configs/nexus/genesis/*.json`) contiennent des validateurs de liste et des poids de jalonnement.
3. Modifier les paramètres :
   - Déterminer le comité de consensus et le quorum.
   - Интервал блоков / пороги finalité.
   - Services de ports Torii et certificats TLS.

## Partie 2 - Déployer le module bootstrap
1. Подготовить валидаторские узлы:
   - Развернуть `irohad` инстансы (validateurs) avec les volumes persistants.
   - En outre, les règles de pare-feu établissent un consensus et le trafic Torii peut être utilisé.
2. Connectez le service Torii (REST/WebSocket) au validateur TLS.
3. Развернуть observer узлы (lecture seule) для дополнительной устойчивости.
4. Téléchargez les scripts bootstrap (`scripts/nexus_bootstrap.sh`) pour développer Genesis, démarrez le consensus et l'enregistrement des utilisateurs.
5. Effectuez les tests de fumée :
   - Exécutez les tests de transition à partir de Torii (`iroha_cli tx submit`).
   - Проверить production/finalité блоков через телеметрию.
   - Vérifier la réplication du grand livre avec les validateurs/observateurs.## Partie 3 - Gouvernance et clés de mise en œuvre
1. Enregistrez la configuration multisig; Il est clair que la gouvernance peut être améliorée et ratifiée.
2. Безопасно хранить les clés du consensus/comité ; настроить автоматические бэкапы с accès à la journalisation.
3. Enregistrez la procédure de rotation différente du bouton (`docs/source/nexus/key_rotation.md`) et validez le runbook.

## Partie 4 - Intégration CI/CD
1. Настроить pipelines:
   - Créer et publier des images validator/Torii (GitHub Actions ou GitLab CI).
   - Автоматическая валидация конфигурации (genèse des peluches, vérification des signatures).
   - Pipelines de déploiement (Helm/Kustomize) pour les phases de préparation et de production.
2. Effectuez des tests de fumée dans CI (ajoutez un cluster éphémère et installez la suite de transition canonique).
3. Créez des scripts de restauration pour les nouveaux déploiements et téléchargez les runbooks.## Partie 5 - Observabilité et alertes
1. Развернуть мониторинговый стек (Prometheus + Grafana + Alertmanager) по регионам.
2. Notez les mesures clés :
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Логи через Loki/ELK для Torii и consensus сервисов.
3. Daschbords :
   - Здоровье консенсуса (высота блоков, finality, статус peers).
   - Латентность/ошибки Torii API.
   - Governance транзакции и статусы предложений.
4. Alertes :
   - Остановка производства блоков (>2 intervalles de bloc).
   - Падение peer count ниже quorum.
   - Spayki ошибок Torii.
   - Backlog очереди gouvernance предложений.

## Partie 6 - Validation et transfert
1. Effectuez la validation de bout en bout :
   - Отправить proposition de gouvernance (например изменение параметра).
   - Proposons que la gouvernance des pipelines fonctionne.
   - Запустить ledger state diff для проверки консистентности.
2. Documenter le runbook pour les astreintes (réponse aux incidents, basculement, mise à l'échelle).
3. Сообщить о готовности командам SoraFS/SoraNet; подтвердить, что piggyback деплои могут указывать на Nexus узлы.## Réalisation des commandes
- [ ] Аудит genesis/configuration завершен.
- [ ] Валидаторские и observer узлы развернуты с здоровым консенсусом.
- [ ] Governance keys загружены, proposal протестирован.
- [ ] CI/CD pipelines работают (build + deploy + smoke tests).
- [ ] Observability dashboards активны с алертами.
- [ ] Документация handoff передана downstream командам.