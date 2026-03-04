---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : Bootstrap et observabilité de Sora Nexus
description : Plan opérationnel pour mettre en ligne le cluster central des validateurs Nexus avant d'ajouter des services SoraFS et SoraNet.
---

:::note Fuente canonica
Cette page reflète `docs/source/soranexus_bootstrap_plan.md`. Assurez-vous d'avoir des copies alignées jusqu'à ce que les versions localisées soient placées sur le portail.
:::

# Plan de bootstrap et d'observabilité de Sora Nexus

## Objets
- Utiliser la base rouge des validateurs/observateurs de Sora Nexus avec les clés de gouvernance, les API de Torii et le moniteur de consensus.
- Valider les services centraux (Torii, accord, persistance) avant d'autoriser l'exécution des opérations en s'appuyant sur SoraFS/SoraNet.
- Établir des workflows de CI/CD et des tableaux de bord/alertes d'observabilité pour assurer la santé du rouge.

## Prérequis
- Matériel de clés de gouvernance (multisig del consejo, clés de comité) disponible dans HSM ou Vault.
- Base d'infrastructure (clusters Kubernetes ou nœuds bare-metal) dans les régions primaires/secondaires.
- Configuration du bootstrap actualisée (`configs/nexus/bootstrap/*.toml`) qui reflète les derniers paramètres de consensus.## Entourons de rouge
- Utiliser les entrées Nexus avec les préfixes de distinctions rouges :
- **Sora Nexus (mainnet)** - préfixe rouge de production `nexus`, hébergeant la gouvernance canonique et les services superposés de SoraFS/SoraNet (ID de chaîne `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - préfixe de rouge de staging `testus`, en particulier la configuration du réseau principal pour les essais d'intégration et de validation pré-version (chaîne UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Gestion des archives séparées, des postes de direction et des infrastructures pour chaque entreprise. Nous testons actuellement le banc d'essais de tous les déploiements SoraFS/SoraNet avant de promouvoir le Nexus.
- Les pipelines de CI/CD doivent être téléchargés d'abord sur Testus, effectuer des tests de fumée automatisés et demander le manuel de promotion à Nexus une fois que vous avez effectué les contrôles.
- Les bundles de configuration de référence sont présents en `configs/soranexus/nexus/` (mainnet) et `configs/soranexus/testus/` (testnet), chacun avec `config.toml`, `genesis.json` et les répertoires d'admission Torii par exemple.## Paso 1 - Révision de la configuration
1. Vérifier la documentation existante :
   - `docs/source/nexus/architecture.md` (accord, mise en page de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de llaves).
2. Valider la genèse des archives (`configs/nexus/genesis/*.json`) avec la liste actuelle des validateurs et les pesos de jalonnement.
3. Confirmer les paramètres du rouge :
   - Tamano du comité de consensus et quorum.
   - Intervalle de blocages / parapluies de finalisation.
   - Ports de service Torii et certifiés TLS.

## Paso 2 - Déploiement du bootstrap du cluster
1. Fournir des nœuds validateurs :
   - Supprimer les instances `irohad` (validadores) avec des volumes persistants.
   - Assurer les règles du pare-feu qui permettent le trafic de consentement et Torii entre les nœuds.
2. Lancez les services Torii (REST/WebSocket) avec chaque validateur avec TLS.
3. Desplegar nodos observadores (lecture solo) pour une résilience supplémentaire.
4. Exécuter des scripts de bootstrap (`scripts/nexus_bootstrap.sh`) pour distribuer Genesis, lancer un consensus et enregistrer des nœuds.
5. Effectuer des tests de fumée :
   - Enviar transacciones de prueba via Torii (`iroha_cli tx submit`).
   - Vérifier la production/finalité des blocs au milieu de la télémétrie.
   - Révision de la réplication du grand livre entre validateurs/observateurs.## Paso 3 - Gobernanza et gestion des clés
1. Charger la configuration multisig del consejo ; confirmer que les propositions de gouvernement peuvent être envoyées et ratifiées.
2. Almacenar de forma segura las llaves de consenso/comite ; configurer les sauvegardes automatiquement avec la journalisation des accès.
3. Configurer les procédures de rotation des clés d'urgence (`docs/source/nexus/key_rotation.md`) et vérifier le runbook.

## Étape 4 - Intégration de CI/CD
1. Configurer les pipelines :
   - Création et publication d'images de validateur/Torii (GitHub Actions ou GitLab CI).
   - Validación automatizada de configuración (lint de genesis, verificacion de firmas).
   - Pipelines de déploiement (Helm/Kustomize) pour clusters de staging et de production.
2. Implémenter des tests de fumée en CI (levantar cluster efimero, correr suite canonica de transacciones).
3. Créez des scripts de restauration pour exécuter les erreurs et documenter les runbooks.## Étape 5 - Observabilité et alertes
1. Désinstaller la pile de surveillance (Prometheus + Grafana + Alertmanager) par région.
2. Métriques centrales recopieuses :
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Journaux via Loki/ELK pour les services Torii et accord.
3. Tableaux de bord :
   - Salud de consensus (altura de bloque, finalizacion, estado de peers).
   - Latence/tâche d'erreur de l'API Torii.
   - Transacciones de gobernanza y estado de propuestas.
4. Alertes :
   - Paro de production de bloques (>2 intervalles de blocage).
   - Conteo de peers por debajo del quorum.
   - Picos sur la barre d'erreur de Torii.
   - Backlog de la cola de propuestas de gobernanza.

## Étape 6 - Validation et transfert
1. Exécuter la validation de bout en bout :
   - Envoyer une proposition de gouvernement (par exemple, changement de paramètre).
   - Procesarla via aprobacion del consejo para asegurar que el pipeline de gobernanza funciona.
   - Exécuter les différences de l'état du grand livre pour assurer la consistance.
2. Documenter le runbook pour l'astreinte (réponse aux incidents, basculement, escalade).
3. Communiquer la disponibilité des équipements de SoraFS/SoraNet ; confirmar que los despliegues piggyback peuvent apuntar a nodos Nexus.## Checklist de mise en œuvre
- [ ] Auditorium de genèse/configuration complète.
- [ ] Nodos validadores y observateurs desplégados consenso saludable.
- [ ] Llaves de gobernanza cargadas, propuesta probada.
- [ ] Pipelines CI/CD corriendo (build + déploiement + smoke tests).
- [ ] Tableaux de bord d'observabilité activés avec alertes.
- [ ] Documentation de transfert entre les équipes en aval.