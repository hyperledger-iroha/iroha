---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-bootstrap-plan
title: Bootstrap et observabilite Sora Nexus
description: Plan operationnel pour mettre en ligne le cluster central de validateurs Nexus avant d'ajouter les services SoraFS et SoraNet.
---

:::note Source canonique
Cette page reflete `docs/source/soranexus_bootstrap_plan.md`. Gardez les deux copies alignees jusqu'a ce que les versions localisees arrivent sur le portal.
:::

# Plan de bootstrap et d'observabilite Sora Nexus

## Objectifs
- Mettre en place le reseau de base validateurs/observateurs Sora Nexus avec cles de gouvernance, APIs Torii et monitoring du consensus.
- Valider les services coeur (Torii, consensus, persistance) avant d'activer les deploiements piggyback SoraFS/SoraNet.
- Etablir des workflows CI/CD et des dashboards/alertes d'observabilite pour assurer la sante du reseau.

## Prerequis
- Materiel de cles de gouvernance (multisig du conseil, cles de comite) disponible dans HSM ou Vault.
- Infrastructure de base (clusters Kubernetes ou noeuds bare-metal) dans les regions primaire/secondaire.
- Configuration bootstrap mise a jour (`configs/nexus/bootstrap/*.toml`) refletant les derniers parametres de consensus.

## Environnements reseau
- Operer deux environnements Nexus avec des prefixes reseau distincts:
- **Sora Nexus (mainnet)** - prefixe reseau de production `nexus`, hebergeant la gouvernance canonique et les services piggyback SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefixe reseau de staging `testus`, miroir de la configuration mainnet pour les tests d'integration et la validation pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Maintenir des fichiers genesis, des cles de gouvernance et des empreintes d'infrastructure separes pour chaque environnement. Testus sert de terrain de preuve pour tous les rollouts SoraFS/SoraNet avant promotion vers Nexus.
- Les pipelines CI/CD doivent deployer d'abord sur Testus, executer des smoke tests automatises, et demander une promotion manuelle vers Nexus une fois les checks passes.
- Les bundles de configuration de reference se trouvent sous `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet), chacun contenant un exemple `config.toml`, `genesis.json` et des repertoires d'admission Torii.

## Etape 1 - Revue de configuration
1. Auditer la documentation existante:
   - `docs/source/nexus/architecture.md` (consensus, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigences infra).
   - `docs/source/nexus/governance_keys.md` (procedures de garde des cles).
2. Valider que les fichiers genesis (`configs/nexus/genesis/*.json`) s'alignent avec le roster actuel des validateurs et les poids de staking.
3. Confirmer les parametres reseau:
   - Taille du comite de consensus et quorum.
   - Intervalle de bloc / seuils de finalite.
   - Ports de service Torii et certificats TLS.

## Etape 2 - Deploiement du cluster bootstrap
1. Provisionner les noeuds validateurs:
   - Deployer des instances `irohad` (validateurs) avec volumes persistants.
   - Verifier que les regles de firewall autorisent le trafic consensus & Torii entre noeuds.
2. Demarrer les services Torii (REST/WebSocket) sur chaque validateur avec TLS.
3. Deployer des noeuds observateurs (lecture seule) pour une resilience additionnelle.
4. Executer les scripts bootstrap (`scripts/nexus_bootstrap.sh`) pour distribuer genesis, demarrer le consensus et enregistrer les noeuds.
5. Executer des smoke tests:
   - Soumettre des transactions de test via Torii (`iroha_cli tx submit`).
   - Verifier la production/finalite des blocs via telemetrie.
   - Verifier la replication du ledger entre validateurs/observateurs.

## Etape 3 - Gouvernance et gestion des cles
1. Charger la configuration multisig du conseil; confirmer que les propositions de gouvernance peuvent etre soumises et ratifiees.
2. Stocker de maniere securisee les cles de consensus/comite; configurer des sauvegardes automatiques avec journalisation d'acces.
3. Mettre en place les procedures de rotation de cles d'urgence (`docs/source/nexus/key_rotation.md`) et verifier le runbook.

## Etape 4 - Integration CI/CD
1. Configurer les pipelines:
   - Build & publication des images validator/Torii (GitHub Actions ou GitLab CI).
   - Validation automatique de configuration (lint genesis, verification des signatures).
   - Pipelines de deploiement (Helm/Kustomize) pour clusters staging et production.
2. Implementer des smoke tests en CI (demarrer un cluster ephemere, executer la suite canonique de transactions).
3. Ajouter des scripts de rollback pour les deploiements rates et documenter les runbooks.

## Etape 5 - Observabilite et alertes
1. Deployer la stack de monitoring (Prometheus + Grafana + Alertmanager) par region.
2. Collecter les metriques coeur:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK pour les services Torii et consensus.
3. Dashboards:
   - Sante du consensus (hauteur de bloc, finalite, statut des peers).
   - Latence/taux d'erreur de l'API Torii.
   - Transactions de gouvernance et statut des propositions.
4. Alertes:
   - Arret de production de blocs (>2 intervalles de bloc).
   - Baisse du nombre de peers sous le quorum.
   - Pics de taux d'erreur Torii.
   - Backlog de la file de propositions de gouvernance.

## Etape 6 - Validation et handoff
1. Executer la validation end-to-end:
   - Soumettre une proposition de gouvernance (p. ex. changement de parametre).
   - La faire passer par l'approbation du conseil pour s'assurer que le pipeline de gouvernance fonctionne.
   - Executer un diff d'etat du ledger pour assurer la coherence.
2. Documenter le runbook pour on-call (reponse incident, failover, scaling).
3. Communiquer la disponibilite aux equipes SoraFS/SoraNet; confirmer que les deploiements piggyback peuvent pointer vers des noeuds Nexus.

## Checklist d'implementation
- [ ] Audit genesis/configuration termine.
- [ ] Noeuds validateurs et observateurs deployes avec un consensus sain.
- [ ] Cles de gouvernance chargees, proposition testee.
- [ ] Pipelines CI/CD en marche (build + deploy + smoke tests).
- [ ] Dashboards d'observabilite actifs avec alerting.
- [ ] Documentation de handoff livree aux equipes downstream.
