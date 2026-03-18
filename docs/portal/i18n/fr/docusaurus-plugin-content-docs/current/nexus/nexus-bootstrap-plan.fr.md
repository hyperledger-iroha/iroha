---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : Bootstrap et observabilité Sora Nexus
description : Plan opérationnel pour mettre en ligne le cluster central de validateurs Nexus avant d'ajouter les services SoraFS et SoraNet.
---

:::note Source canonique
Cette page reflète `docs/source/soranexus_bootstrap_plan.md`. Gardez les deux copies alignées jusqu'à ce que les versions localisées arrivent sur le portail.
:::

# Plan de bootstrap et d'observabilité Sora Nexus

## Objectifs
- Mettre en place le réseau de base validateurs/observateurs Sora Nexus avec clés de gouvernance, APIs Torii et monitoring du consensus.
- Valider les services coeur (Torii, consensus, persistance) avant d'activer les déploiements piggyback SoraFS/SoraNet.
- Etablir des workflows CI/CD et des tableaux de bord/alertes d'observabilité pour assurer la santé du réseau.

## Prérequis
- Matériel de clés de gouvernance (multisig du conseil, clés de comité) disponible dans HSM ou Vault.
- Infrastructure de base (clusters Kubernetes ou noeuds bare-metal) dans les régions primaire/secondaire.
- Configuration bootstrap mise à jour (`configs/nexus/bootstrap/*.toml`) reflétant les derniers paramètres de consensus.## Environnements réseau
- Opérer deux environnements Nexus avec des préfixes réseau distincts :
- **Sora Nexus (mainnet)** - préfixe réseau de production `nexus`, hébergeant la gouvernance canonique et les services piggyback SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - préfixe réseau de staging `testus`, miroir de la configuration mainnet pour les tests d'intégration et la validation pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Maintenir des fichiers genesis, des clés de gouvernance et des empreintes d'infrastructure séparées pour chaque environnement. Testus sert de terrain de preuve pour tous les déploiements SoraFS/SoraNet avant promotion vers Nexus.
- Les pipelines CI/CD doivent être déployés d'abord sur Testus, exécuter des tests de fumée automatisés, et demander une promotion manuelle vers Nexus une fois les contrôles réussis.
- Les bundles de configuration de référence se trouvent sous `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet), chacun contenant un exemple `config.toml`, `genesis.json` et des répertoires d'admission Torii.## Etape 1 - Revue de configuration
1. Auditer la documentation existante :
   - `docs/source/nexus/architecture.md` (consensus, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigences infra).
   - `docs/source/nexus/governance_keys.md` (procédures de garde des clés).
2. Valider que les fichiers genesis (`configs/nexus/genesis/*.json`) s'alignent avec le roster actuel des validateurs et le poids de staking.
3. Confirmer les paramètres du réseau :
   - Taille du comité de consensus et quorum.
   - Intervalle de bloc / seuils de finalité.
   - Ports de service Torii et certificats TLS.

## Etape 2 - Déploiement du cluster bootstrap
1. Provisionner les noeuds validateurs :
   - Déployeur des instances `irohad` (validateurs) avec volumes persistants.
   - Vérifier que les règles de pare-feu autorisent le trafic consensus & Torii entre noeuds.
2. Démarrer les services Torii (REST/WebSocket) sur chaque validateur avec TLS.
3. Déployer des noeuds observateurs (lecture seule) pour une résilience additionnelle.
4. Exécuter les scripts bootstrap (`scripts/nexus_bootstrap.sh`) pour distribuer Genesis, démarrer le consensus et enregistrer les nœuds.
5. Exécuteur des tests de fumée :
   - Soumettre des transactions de test via Torii (`iroha_cli tx submit`).
   - Vérifier la production/finalité des blocs via télémétrie.
   - Vérifier la réplication du grand livre entre validateurs/observateurs.## Etape 3 - Gouvernance et gestion des clés
1. Charger la configuration multisig du conseil; confirmer que les propositions de gouvernance peuvent être soumises et ratifiées.
2. Stocker de manière sécurisée les clés de consensus/comite ; configurer des sauvegardes automatiques avec journalisation d'accès.
3. Mettre en place les procédures de rotation des clés d'urgence (`docs/source/nexus/key_rotation.md`) et vérifier le runbook.

## Etape 4 - Intégration CI/CD
1. Configurer les pipelines :
   - Build & publication des images validator/Torii (GitHub Actions ou GitLab CI).
   - Validation automatique de configuration (genèse des peluches, vérification des signatures).
   - Pipelines de déploiement (Helm/Kustomize) pour le staging et la production des clusters.
2. Implémenter des smoke tests en CI (démarrer un cluster éphémère, exécuter la suite canonique de transactions).
3. Ajoutez des scripts de rollback pour les taux de déploiement et documentez les runbooks.## Etape 5 - Observabilité et alertes
1. Déployer la stack de surveillance (Prometheus + Grafana + Alertmanager) par région.
2. Collecter les métriques coeur :
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK pour les services Torii et consensus.
3. Tableaux de bord :
   - Santé du consensus (hauteur de bloc, finalité, statut des pairs).
   - Latence/taux d'erreur de l'API Torii.
   - Transactions de gouvernance et statut des propositions.
4. Alertes :
   - Arrêt de production de blocs (>2 intervalles de bloc).
   - Baisse du nombre de pairs sous le quorum.
   - Photos de taux d'erreur Torii.
   - Backlog du dossier de propositions de gouvernance.

## Etape 6 - Validation et transfert
1. Exécuter la validation de bout en bout :
   - Soumettre une proposition de gouvernance (p. ex. changement de paramètre).
   - La faire passer par l'approbation du conseil pour s'assurer que le pipeline de gouvernance fonctionne.
   - Exécuter un diff d'état du grand livre pour assurer la cohérence.
2. Documenter le runbook pour on-call (réponse à incident, basculement, mise à l'échelle).
3. Communiquer la disponibilité aux équipes SoraFS/SoraNet ; confirmer que les déploiements piggyback peuvent pointer vers des noeuds Nexus.## Checklist de mise en œuvre
- [ ] Audit genèse/configuration terminée.
- [ ] Noeuds validateurs et observateurs déployés avec un consensus sain.
- [ ] Clés de gouvernance chargées, proposition testée.
- [ ] Pipelines CI/CD en marche (construction + déploiement + smoke tests).
- [ ] Tableaux de bord d'observabilité des actifs avec alertes.
- [ ] Documentation de handoff livree aux équipes en aval.