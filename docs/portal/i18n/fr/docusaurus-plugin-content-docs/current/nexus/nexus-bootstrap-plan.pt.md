---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : Bootstrap et observabilité de Sora Nexus
description : Plan opérationnel pour localiser le cluster central des validateurs Nexus en ligne avant les services supplémentaires SoraFS et SoraNet.
---

:::note Fonte canonica
Cette page reflète `docs/source/soranexus_bootstrap_plan.md`. Mantenha as duas copias alinhadas ate que as versoes localizadas cheguem ao portal.
:::

# Plan de bootstrap et d'observation de Sora Nexus

## Objets
- Utiliser la base de données des validateurs/observateurs Sora Nexus avec des critères de gouvernance, des API Torii et un suivi de consensus.
- Valider les services centraux (Torii, accord, persistance) avant d'autoriser le déploiement de ferroutage SoraFS/SoraNet.
- Établir des workflows de CI/CD et des tableaux de bord/alertes d'observation pour garantir la santé du réseau.

## Prérequis
- Matériel de chaves de gouvernance (multisig do conselho, chaves de comite) disponible dans HSM ou Vault.
- Base d'infrastructure (clusters Kubernetes ou bare-metal) dans les régions primaires/secondaires.
- La configuration du bootstrap actualisée (`configs/nexus/bootstrap/*.toml`) reflète les paramètres de consensus les plus récents.## Ambiances de rede
- Utiliser deux ambiances Nexus avec des préfixes de réseau distincts :
- **Sora Nexus (réseau principal)** - préfixe de réseau de production `nexus`, hôpital de gouvernance canonique et services piggyback SoraFS/SoraNet (ID de chaîne `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - préfixe de rede de staging `testus`, destiné à la configuration du réseau principal pour les tests d'intégration et de validation pré-version (chaîne UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos genesis separados, chaves de gouvernance et empreintes de l'infrastructure pour chaque environnement. Testus est devenu un camp de tests pour les déploiements SoraFS/SoraNet avant de promouvoir pour Nexus.
- Les pipelines de CI/CD doivent être déployés d'abord dans Testus, exécuter des tests de fumée automatisés et fournir un manuel de promotion pour Nexus lorsque les contrôles sont effectués.
- Les bundles de configuration de référence se trouvent dans `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet), chacun avec un support `config.toml`, `genesis.json` et des répertoires d'admission Torii par exemple.## Étape 1 - Révision de la configuration
1. Auditer la documentation existante :
   - `docs/source/nexus/architecture.md` (accord, mise en page de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestrutura).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custodia de chaves).
2. Valider la genèse des archives (`configs/nexus/genesis/*.json`) avec la liste actuelle des validateurs et des pesos de jalonnement.
3. Confirmer les paramètres de réseau :
   - Tamanho do comité de consensus et quorum.
   - Intervalo de blocos / limites de finalidade.
   - Ports de service Torii et certifiés TLS.

## Etapa 2 - Déployer le bootstrap du cluster
1. Provisionar nos validadores :
   - Déployer les instances `irohad` (validateurs) avec les volumes persistants.
   - Garantir que le pare-feu permet le trafic de consentement et Torii entre nos.
2. Lancez les services Torii (REST/WebSocket) avec chaque validateur avec TLS.
3. Déployez nos observateurs (quelque lecture) pour une résilience supplémentaire.
4. Exécuter les scripts de bootstrap (`scripts/nexus_bootstrap.sh`) pour distribuer Genesis, lancer le consensus et les numéros d'enregistrement.
5. Exécuter des tests de fumée :
   - Envoyer les transacoes de teste via Torii (`iroha_cli tx submit`).
   - Vérifier la production/finalité des blocs par télémétrie.
   - Réplication du grand livre entre validateurs/observateurs.## Étape 3 - Gouvernance et gestion des chaves
1. Effectuer la configuration multisig du conseil ; confirmer que les propositions de gouvernance peuvent être soumises et ratifiées.
2. Armazenar com seguranca chaves de consenso/comite ; configurer les sauvegardes automatiquement avec la journalisation des accès.
3. Configurer les procédures de rotation des touches d'urgence (`docs/source/nexus/key_rotation.md`) et vérifier le runbook.

## Etapa 4 - Intégration CI/CD
1. Configurer les pipelines :
   - Créer la publication du validateur d'images/Torii (GitHub Actions ou GitLab CI).
   - Validacao automatizada de configuracao (lint de genesis, verificacao de assinaturas).
   - Pipelines de déploiement (Helm/Kustomize) pour clusters de staging et de production.
2. Implémentation de tests de fumée sans CI (subir cluster efemero, rodar suite canonica de transacoes).
3. Ajouter des scripts de restauration pour déployer de faux et documenter les runbooks.## Étape 5 - Observabilité et alertes
1. Déployez la pile de surveillance (Prometheus + Grafana + Alertmanager) par région.
2. Coletar metricas centrais:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Journaux via Loki/ELK pour les services Torii et consentement.
3. Tableaux de bord :
   - Saude do consenso (altura de bloco, finalidade, status de peers).
   - Latence et taxons d'erreur de l'API Torii.
   - Transacoes de gouvernance et statut des propositions.
4. Alertes :
   - Parada de producão de blocos (>2 intervalles de bloc).
   - Queda no numero de peers abaixo do quorum.
   - Picos na taxa de erreur de Torii.
   - Backlog da fila de propositionstas de gouvernance.

## Étape 6 - Validation et transfert
1. Validation de bout en bout :
   - Submeter proposeta de gouvernance (ex. mudanca de parametro).
   - Traiter l'approbation du conseil pour garantir que le pipeline de gouvernance fonctionne.
   - Rodar diff de estado do ledger pour garantir la cohérence.
2. Documenter le runbook pour les astreintes (réponse aux incidents, basculement, mise à l'échelle).
3. Communication rapide pour les équipes SoraFS/SoraNet ; confirmer que le déploiement du système de ferroutage est disponible pour notre Nexus.## Checklist de mise en œuvre
- [ ] Auditoria de genesis/configuracao concluida.
- [ ] Nos validateurs et observateurs déployés avec le consensus saudavel.
- [ ] Chaves de gouvernance carregadas, proposta testada.
- [ ] Pipelines CI/CD rodando (construction + déploiement + tests de fumée).
- [ ] Tableaux de bord d'observabilidade ativos com alertas.
- [ ] Documentation du transfert entre plusieurs fois en aval.