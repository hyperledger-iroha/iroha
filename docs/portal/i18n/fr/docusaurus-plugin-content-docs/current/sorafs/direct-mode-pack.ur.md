---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : pack en mode direct
titre : SoraFS pour un pack de secours en mode direct (SNNet-5a)
sidebar_label : pack en mode direct
description : SNNet-5a en mode direct SoraFS et Torii/QUIC en mode direct pour les contrôles de conformité de la configuration et les étapes de déploiement
---

:::note مستند ماخذ
:::

Circuits SoraNet SoraFS pour le transport par défaut et élément de la feuille de route **SNNet-5a** pour le repli réglementé et le déploiement de l'anonymat pour les opérateurs accès en lecture déterministe برقرار رکھ سکیں۔ Pack de boutons CLI/SDK, profils de configuration, tests de conformité, liste de contrôle de déploiement et transport de confidentialité pour SoraFS et mode direct Torii/QUIC میں چلانے کے لیے درکار ہیں۔

Une mise en scène de repli et des environnements de production réglementés pour des portes de préparation telles que SNNet-5 ou SNNet-9. Il existe des artefacts pour les opérateurs de garantie de déploiement SoraFS et des modes directs anonymes pour les opérateurs. سوئچ کر سکیں۔

## 1. Indicateurs CLI et SDK- La planification des relais `sorafs_cli fetch --transport-policy=direct-only ...` est désactivée et les transports Torii/QUIC sont appliqués. Aide CLI pour `direct-only` et valeur acceptée
- Les SDK comme `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` définissent les paramètres d'exposition et la bascule "mode direct" expose les éléments `iroha::ClientOptions` et `iroha_android` incluent les liaisons générées et l'énumération avant.
- Faisceaux de passerelle (`sorafs_fetch`, liaisons Python) partagés Norito Aides JSON et analyse à bascule directe uniquement pour l'automatisation et le comportement ملے۔

Les runbooks destinés aux partenaires incluent le document d'indicateur et la fonctionnalité bascule les variables d'environnement comme `iroha_config` et le fil de connexion.

## 2. Profils de stratégie de passerelle

La configuration de l'orchestrateur déterministe persiste pour Norito JSON. `docs/examples/sorafs_direct_mode_policy.json` exemple de profil et encodage de code :

- `transport_policy: "direct_only"` — Les fournisseurs rejettent les annonces et les transports relais SoraNet annoncent des annonces.
- `max_providers: 2` — homologues directs pour les points de terminaison Torii/QUIC fiables Les allocations de conformité régionales peuvent être ajustées
- `telemetry_region: "regulated-eu"` — métriques émises et étiquette pour les tableaux de bord/audits de secours et les exécutions de secours
- Budgets de tentatives conservateurs (`retry_budget: 2`, `provider_failure_threshold: 3`) et masque de passerelles mal configuréesJSON et `sorafs_cli fetch --config` (automatisation) et liaisons SDK (`config_from_json`) pour charger les opérateurs et les politiques d'exposition Pistes d'audit et sortie du tableau de bord (`persist_path`)

Boutons de mise en application côté passerelle `docs/examples/sorafs_gateway_direct_mode.toml` میں درج ہیں۔ Le modèle `iroha app sorafs gateway direct-mode enable` et la sortie et le miroir et les contrôles d'enveloppe/admission sont désactivés et les paramètres par défaut de limite de débit sont activés par le fil et la table `direct_mode` et les noms d'hôtes dérivés du plan et le manifeste. résumés سے peupler کرتا ہے۔ Gestion de la configuration, comme commit d'extrait de code, comme valeurs d'espace réservé, et comme plan de déploiement, comme remplacement.

## 3. Suite de tests de conformité

Préparation au mode direct pour l'orchestrateur et les caisses CLI pour une couverture complète :- `direct_only_policy_rejects_soranet_only_providers` est en cas d'échec d'une annonce de candidat ou d'un relais SoraNet pris en charge ہو۔【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` pour les transports en commun Torii/QUIC transporte pour les transports en commun SoraNet relaye la session en cours de lecture 【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` analyser les fichiers et les aides de documents alignés رہیں۔【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` pour la passerelle Torii simulée pour les environnements réglementés et les tests de fumée pour les environnements réglementés Broche de transport direct ہیں۔【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` commande et politique JSON et persistance du tableau de bord et emballage et automatisation du déploiement et automatisation du déploiement.

Les mises à jour publient une suite ciblée sur la suite :

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Les modifications en amont de la compilation de l'espace de travail échouent et l'erreur de blocage est `status.md` pour l'enregistrement et le rattrapage des dépendances.

## 4. Fumée automatiséeIl existe des régressions spécifiques à l'environnement de couverture CLI et des régressions (dérive de la politique de passerelle et incompatibilités manifestes) Il s'agit d'un assistant de fumée dédié `scripts/sorafs_direct_mode_smoke.sh` et d'un `sorafs_cli fetch`, d'une stratégie d'orchestration en mode direct, d'une persistance du tableau de bord et d'une capture récapitulative et d'un wrapper.

Exemple d'utilisation :

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- Les indicateurs CLI de script et les fichiers de configuration clé = valeur et respectent les paramètres (`docs/examples/sorafs_direct_mode_smoke.conf`) Il s'agit du résumé du manifeste et des entrées d'annonces du fournisseur et des valeurs de production qui remplissent les formulaires.
- `--policy` par défaut et `docs/examples/sorafs_direct_mode_policy.json` et `sorafs_orchestrator::bindings::config_to_json` sont également disponibles pour l'orchestrateur JSON. ہے۔ La politique CLI est `--orchestrator-config=PATH`, qui permet de créer des indicateurs et des exécutions reproductibles, ainsi que des drapeaux et des tunes. کیے۔
- Le `sorafs_cli` `PATH` est un assistant pour la caisse `sorafs_orchestrator` (profil de version) pour construire des conduites de fumée livrées en mode direct plomberie et exercice کریں۔
- Sorties :
  - Charge utile assemblée (`--output`, `artifacts/sorafs_direct_mode/payload.bin` par défaut).
  - Récupérer le résumé (`--summary`, charge utile par défaut) pour la région de télémétrie et les rapports du fournisseur en clair et en preuves de déploiement
  - Politique d'instantané du tableau de bord JSON pour le chemin d'accès et persistance (`fetch_state/direct_mode_scoreboard.json`) Résumé des changements de tickets et archives- Automatisation du portail d'adoption : récupérer les fichiers de l'assistant `cargo xtask sorafs-adoption-check` pour le tableau de bord persistant et les chemins récapitulatifs des fichiers Quorum par défaut requis pour la ligne de commande et pour les fournisseurs de services et les fournisseurs Exemple d'exemple de remplacement `--min-providers=<n>` Résumé des rapports d'adoption par défaut (emplacement personnalisé `--adoption-report=<path>`) et assistant par défaut par `--require-direct-only` (remplacement de secours) اور `--require-telemetry` جب متعلقہ flag دیا جائے، pass کرتا ہے۔ Xtask args utilise `XTASK_SORAFS_ADOPTION_FLAGS` pour obtenir un déclassement (le déclassement a été approuvé par `--allow-single-source` pour que le repli de la porte soit toléré کرے اور appliquer بھی)۔ Pour les diagnostics locaux `--skip-adoption-check` استعمال کریں؛ feuille de route pour l'exécution en mode direct réglementé et ensemble de rapports d'adoption

## 5. Liste de contrôle de déploiement1. **Gel de la configuration :** Profil JSON en mode direct et dépôt `iroha_config` pour magasin, hachage et changement de ticket pour le client
2. **Audit de passerelle :** Mode direct pour le commutateur et les points de terminaison Torii Capacité TLS TLV et application de la journalisation d'audit pour les points de terminaison Torii Profil de politique de passerelle pour les opérateurs qui publient des messages
3. **Approbation de conformité :** un manuel de jeu mis à jour et les réviseurs de conformité/réglementation partagent et superposent l'anonymat et capturent les approbations.
4. **Exécution à sec :** suite de tests de conformité et récupération de la mise en scène par des fournisseurs Torii de confiance. Résultats du tableau de bord et archives des résumés CLI ici
5. ** Arrêt de la production : ** annonce de la fenêtre de changement pour le `transport_policy` et le `direct_only` et le moniteur de tableaux de bord en mode direct (latence `sorafs_fetch`, compteurs de défaillance du fournisseur) document de plan de restauration pour SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` diplômé de SoraNet-first et de SoraNet-first
6. **Révision post-modification :** instantanés du tableau de bord, récupération des résumés et résultats de surveillance et ticket de modification et joindre un message `status.md` date d'entrée en vigueur et mise à jour des anomaliesListe de contrôle du runbook `sorafs_node_ops` pour les opérateurs de commutation en direct et de répétition du flux de travail Le SNNet-5 GA utilise la télémétrie de production et la confirmation de la parité pour le retrait du repli.

## 6. Preuves et exigences en matière d'adoption

Les captures en mode direct par la porte d'adoption du SF-6c satisfont aux exigences de l'utilisateur Exécuter le tableau de bord, le résumé, l'enveloppe du manifeste et l'ensemble des rapports d'adoption et valider la posture de repli `cargo xtask sorafs-adoption-check`. La porte des champs manquants échoue ou échoue lorsque vous changez de ticket ou l'enregistrement de métadonnées attendu.- **Métadonnées de transport :** `scoreboard.json` et `transport_policy="direct_only"` déclarent le changement (pour forcer le déclassement et le retournement `transport_policy_override=true`) Les champs de politique d'anonymat sont associés à des valeurs par défaut et héritent de plusieurs évaluateurs et d'un plan d'anonymat par étapes et d'un écart par rapport à un plan d'anonymat par étapes.
- **Compteurs de fournisseurs :** Les sessions de passerelle uniquement et `provider_count=0` persistent et les fournisseurs `gateway_provider_count=<n>` et Torii remplissent les comptes. JSON et modifier le fichier : les décomptes CLI/SDK dérivent du portail d'adoption et capturent le rejet et le fractionnement est manquant.
- **Preuve manifeste :** Les passerelles Torii sont signées `--gateway-manifest-envelope <path>` (équivalent SDK) et passent par `gateway_manifest_provided`. `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json` enregistrement en ligne یقینی بنائیں کہ `summary.json` et `manifest_id`/`manifest_cid` ہوں؛ Il y a une paire de paires pour l'échec du contrôle d'adoption.
- **Attentes de télémétrie :** La capture de télémétrie est effectuée par la porte d'entrée `--require-telemetry` et les métriques du rapport d'adoption émettent également. Répétitions en espace aérien میں drapeau omis کیا جا سکتا ہے، مگر CI اور changement de billets کو document d'absence کرنا چاہیے۔

Exemple :

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json` Tableau de bord, résumé, enveloppe du manifeste et paquet de bûches de fumée, à joindre Travail d'adoption de CI d'artefacts (`ci/check_sorafs_orchestrator_adoption.sh`) et mise en application et mise en miroir des mises à niveau et des rétrogradations en mode direct ainsi que des mises à niveau vérifiables.