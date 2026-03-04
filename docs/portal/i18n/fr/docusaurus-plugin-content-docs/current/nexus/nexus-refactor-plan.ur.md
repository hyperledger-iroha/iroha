---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
titre : Sora Nexus pour le joueur
description: `docs/source/nexus_refactor_plan.md` Pièce de rechange et Iroha 3 pièces de rechange et pièce de rechange دیتا ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_refactor_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Plan de refactorisation du grand livre Sora Nexus

Le registre Sora Nexus ("Iroha 3") est en cours de préparation pour la feuille de route et la feuille de route. Mise en page de la mise en page, comptabilité Genesis/WSV, consensus Sumeragi, déclencheurs de contrats intelligents, requêtes d'instantanés, liaisons d'hôte pointeur-ABI et codecs Norito. گئی régressions کی عکاسی کرتی ہے۔ Plusieurs correctifs sont apportés au patch monolithique et à l'architecture de l'architecture. پہنچا جائے۔## 0. رہنما اصول
- مختلف ہارڈویئر پر déterministe رویہ برقرار رکھیں ; accélération et options de fonctionnalité opt-in flags et options de repli et options de repli
- Couche de sérialisation Norito ہے۔ L'état/schéma est utilisé par Norito encoder/décoder les tests aller-retour et les mises à jour des appareils.
- configuration `iroha_config` (utilisateur -> réel -> valeurs par défaut) Les chemins d'accès et l'environnement ad hoc basculent
- Politique ABI V1 pour plus de détails et de détails hôtes et types de pointeurs/appels système et déterministes et types de pointeurs
- `cargo test --workspace` et tests d'or (`ivm`, `norito`, `integration_tests`) et jalon pour la porte d'entrée## 1. ریپوزٹری ٹاپولوجی اسنیپ شاٹ
- `crates/iroha_core` : acteurs Sumeragi, chargeur WSV Genesis, pipelines (requête, superposition, voies zk) et colle hôte de contrat intelligent
- `crates/iroha_data_model` : requêtes de données en chaîne et schéma faisant autorité
- `crates/iroha` : API client et tests CLI, SDK et support client
- `crates/iroha_cli` : Cliquez sur CLI et sur `iroha` pour les API et les miroirs.
- `crates/ivm` : VM de bytecode Kotodama, points d'entrée d'intégration d'hôte pointeur-ABI۔
- `crates/norito` : codec de sérialisation pour les adaptateurs JSON et les backends AoS/NCB par exemple
- `integration_tests` : assertions inter-composants et genèse/bootstrap, Sumeragi, déclencheurs, pagination et pagination.
- Docs pour Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`) pour la mise en œuvre de la mise en œuvre ٹکڑوں میں ہے اور کوڈ کے مقابلے میں جزوی طور پر پرانی ہے۔

## 2. ری فیکٹر ستون اور jalons### Phase A - Fondements et observabilité
1. **Télémétrie WSV + instantanés**
   - `state` pour l'API d'instantané canonique (trait `WorldStateSnapshot`) pour les requêtes de recherche, Sumeragi et CLI pour les requêtes
   - `scripts/iroha_state_dump.sh` prend en charge les instantanés déterministes `iroha state dump --format norito` pour les instantanés déterministes
2. **Genèse/Déterminisme Bootstrap**
   - l'ingestion de genèse est un pipeline alimenté par Norito (`iroha_core::genesis`) et un pipeline alimenté par Norito (`iroha_core::genesis`)
   - couverture d'intégration/régression pour Genesis et Genesis pour replay et arm64/x86_64 pour les racines WSV (ٹریک: `integration_tests/tests/genesis_replay_determinism.rs`)۔
3. **Tests de fixité entre caisses**
   - `integration_tests/tests/genesis_json.rs` est un pipeline WSV et des invariants ABI et un harnais sont validés pour la validation.
   - L'échafaudage `cargo xtask check-shape` est utilisé pour dérive de schéma et panique (arriéré d'outils DevEx ici ; `scripts/xtask/README.md` est un élément d'action)### Phase B – WSV et surface de requête
1. **Transactions de stockage d'État**
   - `state/storage_transactions.rs` pour un adaptateur transactionnel pour la commande de commit et la détection de conflits
   - les tests unitaires sont basés sur les actifs/monde/déclencheurs et sur le rollback
2. **Refactor du modèle de requête**
   - Logique de pagination/curseur comme `crates/iroha_core/src/query/` pour les composants réutilisables comme composants réutilisables `iroha_data_model` et Norito représentations et aligner les images
   - déclencheurs, actifs et rôles pour l'ordre déterministe et les requêtes d'instantanés (couverture `crates/iroha_core/tests/snapshot_iterable.rs` pour la couverture)
3. **Cohérence des instantanés**
   - Le chemin d'accès de l'instantané `iroha ledger query` CLI et le chemin d'instantané sont connectés à Sumeragi/fetchers.
   - Tests de régression d'instantanés CLI `tests/cli/state_snapshot.rs` en cours (exécutions lentes et dépendantes des fonctionnalités)### Phase C - Gazoduc Sumeragi
1. **Topologie et gestion d'époque**
   - `EpochRosterProvider` pour les traits de caractère et les implémentations d'instantanés d'enjeux WSV pour les utilisateurs
   - Bancs/tests `WsvEpochRosterAdapter::from_peer_iter` pour un constructeur simulé convivial
2. **Simplification du flux de consensus**
   - `crates/iroha_core/src/sumeragi/*` et les numéros de téléphone suivants : `pacemaker`, `aggregation`, `availability`, `witness` Types de modèles `consensus` pour les types de cartes
   - message ad hoc passant des enveloppes Norito tapées et des tests de propriété de changement de vue (arriéré de messagerie Sumeragi en attente)
3. **Intégration de voie/preuve**
   - les preuves de voie et les engagements DA sont alignés sur les engagements RBC et le portail RBC.
   - Test d'intégration de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` sur le chemin activé par RBC et vérification du chemin activé par RBC### Phase D - Contrats intelligents et hôtes Pointer-ABI
1. **Audit des limites de l'hôte**
   - vérifications de type pointeur (`ivm::pointer_abi`) et adaptateurs hôtes (`iroha_core::smartcontracts::ivm::host`) pour consolider les cartes
   - Attentes de la table de pointeurs pour les liaisons du manifeste de l'hôte, comme `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs`, pour les mappages Golden TLV et pour les exercices.
2. **Bac à sable d'exécution de déclenchement**
   - Déclenche le processus de validation du gaz et de la validation du pointeur `TriggerExecutor` pour la journalisation des événements نافذ کرتا ہے۔
   - déclencheurs d'appel/d'heure pour les tests de régression et les chemins d'échec pour les tests de régression (code : `crates/iroha_core/tests/trigger_failure.rs`)
3. **CLI et alignement client**
   - Les opérations CLI des systèmes d'exploitation (`audit`, `gov`, `sumeragi`, `ivm`) dérivent de manière partagée Fonctions client `iroha` pour les utilisateurs
   - Tests d'instantanés CLI JSON `tests/cli/json_snapshot.rs` Il s'agit d'une référence JSON canonique de sortie de commande principale et d'une correspondance avec la version standard.### Phase E - Durcissement du codec Norito
1. **Registre de schémas**
   - `crates/norito/src/schema/` est un registre de schémas Norito pour les types de données de base et les encodages canoniques.
   - exemple de codage de charge utile et vérification des tests de documents et tests (`norito::schema::SamplePayload`)
2. **Actualisation des luminaires dorés**
   - `crates/norito/tests/*` pour les luminaires dorés et pour le refactoring du schéma WSV et pour la correspondance avec le schéma WSV
   - `scripts/norito_regen.sh` helper `norito_regen_goldens` pour régénérer Norito JSON goldens pour déterministe et régénération
3. **Intégration IVM/Norito**
   - Sérialisation du manifeste Kotodama et Norito pour la validation de bout en bout avec les métadonnées ABI du pointeur.
   - `crates/ivm/tests/manifest_roundtrip.rs` manifeste la parité d'encodage/décodage Norito pour la parité de code Norito.## 3. امور transversal
- **Stratégie de test** : tests unitaires de phase -> tests de caisse -> tests d'intégration échecs aux tests et régressions نئے tests انہیں واپس آنے سے روکتے ہیں۔
- **Documentation** : La phase est terminée avec `status.md` et les éléments sont en cours avec `roadmap.md`. جبکہ مکمل شدہ کام prune کریں۔
- **Référentiels de performances** : `iroha_core`, `ivm` et `norito` pour les bancs d'essai. refactoriser les mesures de base et les régressions
- **Feature Flags** : bascule au niveau de la caisse entre les backends et les chaînes d'outils (`cuda`, `zk-verify-batch`) Chemins d'accès SIMD du processeur, build et runtime pour les versions ultérieures matériel non pris en charge ou solutions de secours scalaires déterministes## 4. فوری اگلے اقدامات
- Échafaudage de phase A (trait instantané + câblage de télémétrie) - mises à jour de la feuille de route et tâches exploitables
- `sumeragi`, `state`, et `ivm` pour l'audit des défauts et les points saillants suivants :
  - `sumeragi` : autorisations de code mort, diffusion de preuve de changement de vue, état de relecture VRF, et exportation de télémétrie EMA et garde de sécurité. Phase C : simplification du flux consensuel et livrables d'intégration de voie/preuve.
  - `state` : Nettoyage `Cell` et routage de télémétrie Phase A et piste de télémétrie WSV pour le backlog d'optimisation du pipeline de la phase C. شامل ہوتے ہیں۔
  - `ivm` : exposition à bascule CUDA, validation de l'enveloppe et couverture Halo2/Metal Phase D ainsi que la limite de l'hôte et le thème transversal d'accélération GPU pour le thème de l'accélération GPU. kernels تیار ہونے تک backlog GPU dédié پر رہتے ہیں۔
- changements de code invasifs entre les équipes RFC et la signature du RFC inter-équipes

## 5. کھلے سوالات
- Le RBC et le P1 sont disponibles en option et les voies du grand livre Nexus sont disponibles en option. partie prenante فیصلہ درکار ہے۔
- Pour les groupes de composabilité P1 pour DS, il y a des preuves de voie pour les tests matures et pour désactiver les tests.
- Paramètres ML-DSA-87 et emplacement canonique امیدوار: نیا caisse `crates/fastpq_isi` (en attente de création)۔

---_آخری اپ ڈیٹ: 2025-09-12_