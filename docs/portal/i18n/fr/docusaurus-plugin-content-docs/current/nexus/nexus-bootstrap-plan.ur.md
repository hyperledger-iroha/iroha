---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan nexus-bootstrap
titre : Sora Nexus pour le lecteur et le lecteur
description : Nexus pour un cluster de validateurs et un cluster de validation SoraFS pour SoraNet. آپریشنل پلان۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/soranexus_bootstrap_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ورژنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Plan d'amorçage et d'observabilité

## اہداف
- Il s'agit des API Torii pour la surveillance du consensus et du validateur/observateur Sora Nexus.
- Mise en œuvre (Torii, consensus, persistance) et déploiements de ferroutage SoraFS/SoraNet pour les déploiements de ferroutage
- Workflows CI/CD et tableaux de bord/alertes d'observabilité

## پیشگی شرائط
- Matériel clé de gouvernance (clés du conseil multisig, comité) HSM et Vault میں دستیاب ہو۔
- بنیادی انفراسٹرکچر (clusters Kubernetes et nœuds nus)
- Une configuration d'amorçage (`configs/nexus/bootstrap/*.toml`) et des paramètres de consensus définis## نیٹ ورک ماحول
- Pour les environnements Nexus, les préfixes et les préfixes suivants :
- **Sora Nexus (réseau principal)** - Le préfixe `nexus` et la gouvernance canonique ainsi que les services de ferroutage SoraFS/SoraNet sont disponibles (ID de chaîne `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - mise en scène du préfixe `testus` et configuration du réseau principal, tests d'intégration et validation préalable à la version miroir (chaîne UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- L'environnement, les fichiers Genesis, les clés de gouvernance et les empreintes d'infrastructure. Testus a déployé les déploiements SoraFS/SoraNet pour un terrain d'essai et Nexus pour promouvoir l'environnement.
- Pipelines CI/CD pour Testus pour déployer des tests de fumée automatisés et des contrôles pour Nexus pour une promotion manuelle
- Bundles de configuration de référence `configs/soranexus/nexus/` (réseau principal) et `configs/soranexus/testus/` (testnet) et `config.toml`, `genesis.json` et Torii Répertoires d'admission شامل ہیں۔## مرحلہ 1 - Révision de la configuration
1. Documentation relative à l'audit et à la vérification :
   - `docs/source/nexus/architecture.md` (consensus, mise en page Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigences infrarouges).
   - `docs/source/nexus/governance_keys.md` (procédures de garde des clés).
2. Fichiers Genesis (`configs/nexus/genesis/*.json`) pour valider la liste des validateurs et les poids de jalonnement et aligner les poids
3. Paramètres de configuration des paramètres :
   - Taille du comité de consensus et quorum.
   - Intervalle de blocage/seuils de finalité.
   - Ports de service Torii et certificats TLS.

## مرحلہ 2 - Déploiement du cluster Bootstrap
1. Fourniture des nœuds de validation :
   - Instances `irohad` (validateurs) et volumes persistants pour le déploiement
   - Il existe des règles de pare-feu et un consensus pour les nœuds de trafic Torii autorisés.
2. Le validateur des services Torii (REST/WebSocket) et le service TLS sont disponibles.
3. La résilience et les nœuds d'observateurs (lecture seule) sont déployés.
4. Scripts d'amorçage (`scripts/nexus_bootstrap.sh`) pour la genèse du consensus et les nœuds de connexion
5. Tests de fumée ici :
   - Torii pour tester les transactions soumises (`iroha_cli tx submit`).
   - Télémétrie pour la production de blocs/vérification de la finalité
   - Validateurs/observateurs pour la réplication du grand livre et pour la réplication du grand livre## مرحلہ 3 - Gouvernance et gestion des clés
1. Configuration multisig du Conseil Les propositions de gouvernance sont soumises et ratifiées.
2. Clés du consensus/comité accès à la journalisation et sauvegardes automatiques configurer
3. Procédures de rotation des clés d'urgence (`docs/source/nexus/key_rotation.md`) pour vérifier la vérification du runbook

## مرحلہ 4 - Intégration CI/CD
1. Configuration des pipelines :
   - Les images Validator/Torii sont créées et publiées (Actions GitHub et GitLab CI).
   - Validation automatisée de la configuration (genèse des peluches, vérification des signatures).
   - Pipelines de déploiement (Helm/Kustomize) pour la mise en scène et les clusters de production.
2. CI میں smoke tests شامل کریں (cluster éphémère اٹھائیں، suite de transactions canoniques چلائیں).
3. Nouveaux déploiements et scripts de restauration, ainsi que les runbooks et les scripts de restauration## مرحلہ 5 - Observabilité et alertes
1. Pile de surveillance (Prometheus + Grafana + Alertmanager) dans la région pour déployer le système
2. Indicateurs de base par exemple :
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii pour les services de consensus pour les journaux Loki/ELK
3. Tableaux de bord :
   - Consensus de santé (hauteur du bloc, finalité, statut des pairs).
   - Taux de latence/erreur API Torii.
   - Transactions de gouvernance et statuts des propositions.
4. Alertes :
   - Bloquer le décrochage de la production (> 2 intervalles de bloc).
   - Quorum par les pairs
   - Pics de taux d'erreur Torii.
   - Arriéré dans la file d'attente des propositions de gouvernance.

## مرحلہ 6 - Validation et transfert
1. Validation de bout en bout :
   - Proposition de gouvernance soumise کریں (changement de paramètre مثلاً).
   - Approbation par le Conseil du processus de gouvernance et du pipeline de gouvernance
   - Différences d'état du grand livre et cohérence et cohérence
2. Runbook d'astreinte (réponse aux incidents, basculement, mise à l'échelle).
3. SoraFS/SoraNet est prêt à être prêt Il s'agit de déploiements de ferroutage sur les nœuds Nexus et de points de connexion.## Liste de contrôle de mise en œuvre
- [ ] Audit de genèse/configuration مکمل۔
- [ ] Le validateur et les nœuds d'observateurs déploient le consensus et le consensus
- [ ] Clés de gouvernance لوڈ ہوئے، test de proposition ہوا۔
- [ ] Pipelines CI/CD en cours (construction + déploiement + tests de fumée).
- [ ] Tableaux de bord d'observabilité en ligne et alertes en ligne
- [ ] Documentation de transfert en aval ٹیموں کو دے دی گئی۔