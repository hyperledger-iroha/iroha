---
lang: fr
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-04T13:46:50.705991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Journal des modifications

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

Tous les changements notables apportés à ce projet seront documentés dans ce dossier.

## [Inédit]- Lâchez la cale SCALE ; `norito::codec` est désormais implémenté avec la sérialisation native Norito.
- Remplacez les utilisations `parity_scale_codec` par `norito::codec` dans toutes les caisses.
- Commencer la migration des outils vers la sérialisation native Norito.
- Supprimez la dépendance `parity-scale-codec` restante de l'espace de travail en faveur de la sérialisation Norito native.
- Remplacez les dérivations de traits SCALE résiduelles par des implémentations natives Norito et renommez le module de codec versionné.
- Fusionnez `iroha_config_base_derive` et `iroha_futures_derive` dans `iroha_derive` avec des macros contrôlées par les fonctionnalités.
- *(multisig)* Rejetez les signatures directes des autorités multisig avec un code/raison d'erreur stable, appliquez les plafonds TTL multisig sur les relais imbriqués et faites apparaître les plafonds TTL dans la CLI avant la soumission (parité SDK en attente).
- Déplacez les macros procédurales FFI dans `iroha_ffi` et supprimez la caisse `iroha_ffi_derive`.
- *(schema_gen)* Supprimez la fonctionnalité `transparent_api` inutile de la dépendance `iroha_data_model`.
- *(data_model)* Mettez en cache le normalisateur ICU NFC pour l'analyse `Name` afin de réduire la surcharge d'initialisation répétée.
- 📚 Démarrage rapide du document JS, résolveur de configuration, workflow de publication et recette prenant en compte la configuration pour le client Torii.
- *(IrohaSwift)* Augmentez les objectifs de déploiement minimum vers iOS 15 / macOS 12, adoptez la concurrence Swift sur les API client Torii et marquez les modèles publics comme `Sendable`.
- *(IrohaSwift)* Ajout de `ToriiDaProofSummaryArtifact` et `DaProofSummaryArtifactEmitter.emit` pour que les applications Swift puissent créer/émettre des bundles de preuve DA compatibles CLI sans avoir recours à la CLI, avec des documents et des tests de régression couvrant à la fois la mémoire et le disque. workflows.【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Tests/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* Correction de la sérialisation de l'option Kaigi en supprimant l'indicateur de réutilisation archivée de `KaigiParticipantCommitment`, en ajoutant des tests aller-retour natifs et en supprimant la solution de repli du décodage JS afin que les instructions Kaigi soient désormais Norito aller-retour avant soumission.【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* Autoriser les appelants `ToriiClient` à supprimer les en-têtes par défaut (en passant `null`) afin que `getMetrics` bascule proprement entre le texte JSON et Prometheus Accepter en-têtes.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* Ajout d'assistants itérables pour les NFT, les soldes d'actifs par compte et les détenteurs de définition d'actifs (avec définitions, documents et tests TypeScript) afin que la pagination Torii couvre désormais l'application restante points de terminaison.【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* Ajout de générateurs d'instructions/de transactions de gouvernance ainsi que d'une recette de gouvernance afin que les clients JS puissent déployer par étapes les propositions, les scrutins, la promulgation et la persistance du conseil jusqu'à la fin. fin.【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* Ajout d'assistants de soumission/statut ISO 20022 pacs.008 et d'une recette correspondante, permettant aux appelants JS d'exercer le pont ISO Torii sans HTTP sur mesure plomberie.【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】- *(javascript)* Ajout d'assistants de création pacs.008/pacs.009 ainsi que d'une recette basée sur la configuration afin que les appelants JS puissent synthétiser les charges utiles ISO 20022 avec des métadonnées BIC/IBAN validées avant d'accéder au pont.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* Terminer la boucle d'ingestion/récupération/prove DA : `ToriiClient.fetchDaPayloadViaGateway` dérive désormais automatiquement les handles de chunker (via la nouvelle liaison `deriveDaChunkerHandle`), les résumés de preuves facultatifs réutilisent le `generateDaProofSummary` natif et les README/typings/tests ont été actualisés afin que les appelants du SDK puissent refléter `iroha da get-blob/prove-availability` sans sur mesure plomberie.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascrip t/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* Les métadonnées du tableau de bord `sorafsGatewayFetch` enregistrent désormais l'ID/CID du manifeste de la passerelle chaque fois que des fournisseurs de passerelle sont utilisés afin que les artefacts d'adoption s'alignent sur les captures CLI.
- *(torii/cli)* Appliquer les passages croisés ISO : Torii rejette désormais les soumissions `pacs.008` avec des BIC d'agent inconnus et l'aperçu CLI DvP valide `--delivery-instrument-id` via `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* Ajout de l'ingestion d'argent PvP via `POST /v1/iso20022/pacs009`, en appliquant les vérifications des données de référence `Purp=SECU` et BIC avant de créer des transferts.
- *(outillage)* Ajout de `cargo xtask iso-bridge-lint` (plus `ci/check_iso_reference_data.sh`) pour valider les instantanés ISIN/CUSIP, BIC↔LEI et MIC aux côtés des appareils du référentiel.【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* Publication NPM renforcée en déclarant les métadonnées du référentiel, une liste d'autorisation de fichiers explicite, un `publishConfig` activé pour la provenance, un journal des modifications/garde de test `prepublishOnly` et un workflow d'actions GitHub qui exerce le nœud 18/20 dans CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* Le champ add/sub/mul BN254 s'exécute désormais sur les nouveaux noyaux CUDA avec le traitement par lots côté hôte via `bn254_launch_kernel`, permettant l'accélération matérielle pour les gadgets Poséidon et ZK tout en préservant le déterminisme. solutions de secours.【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 08/05/2025

### 🚀 Caractéristiques

- *(cli)* Ajouter `iroha transaction get` et d'autres commandes importantes (#5289)
- [**breaking**] Séparer les actifs fongibles et non fongibles (#5308)
- [**breaking**] Finaliser les blocs non vides en autorisant les blocs vides après eux (#5320)
- Exposer les types de télémétrie dans le schéma et le client (#5387)
- *(iroha_torii)* Stubs pour les points de terminaison contrôlés par les fonctionnalités (#5385)
- Ajouter des métriques de temps de validation (#5380)

### 🐛 Corrections de bugs

- Réviser les non-zéros (#5278)
- Fautes de frappe dans les fichiers de documentation (#5309)
- *(crypto)* Exposer le getter `Signature::payload` (#5302) (#5310)
- *(core)* Ajouter des vérifications de présence du rôle avant de l'accorder (#5300)
- *(core)* Reconnecter le homologue déconnecté (#5325)
- Correction des pytests liés aux actifs du magasin et au NFT (#5341)
- *(CI)* Correction du workflow d'analyse statique Python pour la poésie v2 (#5374)
- L'événement de transaction expirée apparaît après la validation (#5396)

### 💼 Autre- Inclure `rust-toolchain.toml` (#5376)
- Avertir sur `unused`, pas `deny` (#5377)

### 🚜 Refactoriser

- Parapluie Iroha CLI (#5282)
- *(iroha_test_network)* Utiliser un joli format pour les journaux (#5331)
- [**breaking**] Simplifier la sérialisation de `NumericSpec` dans `genesis.json` (#5340)
- Améliorer la journalisation des échecs de connexion p2p (#5379)
- Rétablir `logger.level`, ajouter `logger.filter`, étendre les routes de configuration (#5384)

### 📚Documentation

- Ajouter `network.public_address` à `peer.template.toml` (#5321)

### ⚡Performances

- *(kura)* Empêcher les écritures de blocs redondantes sur le disque (#5373)
- Implémentation d'un stockage personnalisé pour les hachages de transactions (#5405)

### ⚙️ Tâches diverses

- Correction de l'utilisation de la poésie (#5285)
- Supprimer les consts redondants de `iroha_torii_const` (#5322)
- Supprimer le `AssetEvent::Metadata*` inutilisé (#5339)
- Version Bump Sonarqube Action (#5337)
- Supprimer les autorisations inutilisées (#5346)
- Ajouter le package de décompression à ci-image (#5347)
- Correction de quelques commentaires (#5397)
- Déplacer les tests d'intégration de la caisse `iroha` (#5393)
- Désactiver le travail defectdojo (#5406)
- Ajouter l'approbation du DCO pour les commits manquants
- Réorganiser les workflows (deuxième essai) (#5399)
- Ne pas exécuter Pull Request CI lors du push to main (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 07/03/2025

### Ajouté

- finaliser les blocs non vides en autorisant les blocs vides après eux (#5320)

## [2.0.0-rc.1.2] - 2025-02-25

### Corrigé

- les pairs réenregistrés sont désormais correctement reflétés dans la liste des pairs (#5327)

## [2.0.0-rc.1.1] - 2025-02-12

### Ajouté

- ajouter `iroha transaction get` et d'autres commandes importantes (#5289)

## [2.0.0-rc.1.0] - 06/12/2024

### Ajouté

- implémenter des projections de requêtes (#5242)
- utiliser un exécuteur persistant (#5082)
- ajouter des délais d'écoute à iroha cli (#5241)
- ajouter le point de terminaison de l'API /peers à torii (#5235)
- adresse p2p agnostique (#5176)
- améliorer l'utilitaire et la convivialité multisig (#5027)
- protéger `BasicAuth::password` contre l'impression (#5195)
- tri décroissant dans la requête `FindTransactions` (#5190)
- introduire l'en-tête de bloc dans chaque contexte d'exécution de contrat intelligent (#5151)
- temps de validation dynamique basé sur l'index de changement de vue (#4957)
- définir l'ensemble d'autorisations par défaut (#5075)
- ajouter l'implémentation de Niche pour `Option<Box<R>>` (#5094)
- prédicats de transaction et de bloc (#5025)
- signaler le montant des éléments restants dans la requête (# 5016)
- temps discret borné (#4928)
- ajouter les opérations mathématiques manquantes à `Numeric` (#4976)
- valider les messages de synchronisation de bloc (#4965)
- filtres de requêtes (#4833)

### Modifié

- simplifier l'analyse de l'identifiant des pairs (#5228)
- déplacer l'erreur de transaction hors de la charge utile du bloc (#5118)
- renommer JsonString en Json (#5154)
- ajouter une entité client aux contrats intelligents (#5073)
- leader en tant que service de commande de transactions (#4967)
- faire en sorte que Kura supprime les anciens blocs de la mémoire (#5103)
- utilisez `ConstVec` pour les instructions dans `Executable` (#5096)
- des potins au plus une fois (# 5079)
- réduire l'utilisation de la mémoire de `CommittedTransaction` (#5089)
- rendre les erreurs de curseur de requête plus spécifiques (#5086)
- réorganiser les caisses (#4970)
- introduire la requête `FindTriggers`, supprimer `FindTriggerById` (#5040)
- ne dépendez pas des signatures pour la mise à jour (#5039)
- changer le format des paramètres dans Genesis.json (#5020)
- envoyer uniquement la preuve de changement de vue actuelle et précédente (#4929)
- désactiver l'envoi de message lorsque vous n'êtes pas prêt pour éviter une boucle occupée (#5032)
- déplacer la quantité totale d'actif vers la définition d'actif (#5029)
- signer uniquement l'en-tête du bloc, pas la totalité de la charge utile (#5000)
- utiliser `HashOf<BlockHeader>` comme type de hachage de bloc (#4998)
- simplifier `/health` et `/api_version` (#4960)
- renommer `configs` en `defaults`, supprimer `swarm` (#4862)

### Corrigé- aplatir le rôle interne dans json (#5198)
- correction des avertissements `cargo audit` (#5183)
- ajouter une vérification de plage à l'index de signature (#5157)
- Correction d'un exemple de macro de modèle dans la documentation (#5149)
- fermez ws correctement dans le flux de blocs/événements (#5101)
- vérification des pairs de confiance interrompue (#5121)
- vérifier que le bloc suivant a une hauteur +1 (#5111)
- correction de l'horodatage du bloc Genesis (#5098)
- correction de la compilation `iroha_genesis` sans la fonctionnalité `transparent_api` (#5056)
- gérer correctement `replace_top_block` (#4870)
- correction du clonage de l'exécuteur (#4955)
- afficher plus de détails sur l'erreur (#4973)
- utiliser `GET` pour le flux de blocs (#4990)
- améliorer la gestion des transactions en file d'attente (#4947)
- empêcher les messages de bloc blocksync redondants (#4909)
- éviter les blocages lors de l'envoi simultané de messages volumineux (#4948)
- supprimer la transaction expirée du cache (#4922)
- correction de l'URL du torii avec le chemin (#4903)

### Supprimé

- supprimer l'API basée sur un module du client (#5184)
- supprimer `riffle_iter` (#5181)
- supprimer les dépendances inutilisées (#5173)
- supprimer le préfixe `max` de `blocks_in_memory` (#5145)
- supprimer l'estimation consensuelle (#5116)
- supprimer `event_recommendations` du bloc (#4932)

### Sécurité

## [2.0.0-pre-rc.22.1] - 2024-07-30

### Corrigé

- ajout de `jq` à l'image Docker

## [2.0.0-pre-rc.22.0] - 2024-07-25

### Ajouté

- spécifier explicitement les paramètres en chaîne dans Genesis (#4812)
- autoriser le turbofish avec plusieurs `Instruction` (#4805)
- réimplémenter les transactions multisignatures (#4788)
- implémenter des paramètres intégrés ou personnalisés en chaîne (# 4731)
- améliorer l'utilisation des instructions personnalisées (#4778)
- rendre les métadonnées dynamiques via l'implémentation de JsonString (#4732)
- permettre à plusieurs pairs de soumettre un bloc Genesis (#4775)
- fournir `SignedBlock` au lieu de `SignedTransaction` au peer (#4739)
- instructions personnalisées dans l'exécuteur (#4645)
- étendre le client cli pour demander des requêtes json (#4684)
 - ajout du support de détection pour `norito_decoder` (#4680)
- généraliser le schéma d'autorisations au modèle de données de l'exécuteur (#4658)
- ajout des autorisations de déclenchement de registre dans l'exécuteur par défaut (#4616)
 - prise en charge de JSON dans `norito_cli`
- introduire le délai d'inactivité p2p

### Modifié

- remplacer `lol_alloc` par `dlmalloc` (#4857)
- renommer `type_` en `type` dans le schéma (#4855)
- remplacer `Duration` par `u64` dans le schéma (#4841)
- utilisez EnvFilter de type `RUST_LOG` pour la journalisation (#4837)
- garder le bloc de vote lorsque cela est possible (#4828)
- migrer du warp vers axum (#4718)
- modèle de données d'exécuteur divisé (#4791)
- modèle de données superficiel (#4734) (#4792)
- ne pas envoyer de clé publique avec signature (#4518)
- renommer `--outfile` en `--out-file` (#4679)
- renommer le serveur et le client iroha (#4662)
- renommer `PermissionToken` en `Permission` (#4635)
- rejeter `BlockMessages` avec empressement (#4606)
- rendre `SignedBlock` immuable (#4620)
- renommer TransactionValue en CommitteTransaction (#4610)
- authentifier les comptes personnels par ID (#4411)
- utiliser le format multihash pour les clés privées (#4541)
 - renommer `parity_scale_decoder` en `norito_cli`
- envoyer des blocs aux validateurs de l'ensemble B
- rendre `Role` transparent (#4886)
- dériver le hachage de bloc de l'en-tête (#4890)

### Corrigé- vérifier que l'autorité possède le domaine à transférer (#4807)
- supprimer la double initialisation du logger (#4800)
- correction de la convention de dénomination pour les actifs et les autorisations (#4741)
- mise à niveau de l'exécuteur dans une transaction séparée dans le bloc Genesis (#4757)
- valeur par défaut correcte pour `JsonString` (#4692)
- améliorer le message d'erreur de désérialisation (#4659)
- ne paniquez pas si la clé publique Ed25519Sha512 transmise est d'une longueur invalide (#4650)
- utiliser l'index de changement de vue approprié lors du chargement du bloc d'initialisation (#4612)
- ne pas exécuter prématurément les déclencheurs temporels avant leur horodatage `start` (#4333)
- prend en charge `https` pour `torii_url` (#4601) (#4617)
- supprimer serde(aplatir) de SetKeyValue/RemoveKeyValue (#4547)
- le jeu de déclencheurs est correctement sérialisé
- révoquer les `PermissionToken` supprimés sur `Upgrade<Executor>` (#4503)
- signaler l'indice de changement de vue correct pour le tour en cours
- supprimer les déclencheurs correspondants sur `Unregister<Domain>` (#4461)
- vérifier la clé de pub Genesis dans Genesis Round
- empêcher l'enregistrement du domaine ou du compte Genesis
- supprimer les autorisations des rôles lors de la désinscription de l'entité
- les métadonnées du déclencheur sont accessibles dans les contrats intelligents
- utiliser le verrou rw pour éviter une vue d'état incohérente (#4867)
- gérer le soft fork dans l'instantané (#4868)
- correction de MinSize pour ChaCha20Poly1305
- ajouter des limites à LiveQueryStore pour éviter une utilisation élevée de la mémoire (#4893)

### Supprimé

- supprimer la clé publique de la clé privée ed25519 (#4856)
- supprimer kura.lock (#4849)
- rétablir les suffixes `_ms` et `_bytes` dans la configuration (#4667)
- supprimer les suffixes `_id` et `_file` des champs Genesis (#4724)
- supprimer les actifs d'index dans AssetsMap par AssetDefinitionId (#4701)
- supprimer le domaine de l'identité du déclencheur (#4640)
- supprimer la signature Genesis de Iroha (#4673)
- supprimer `Visit` lié à `Validate` (#4642)
- supprimer `TriggeringEventFilterBox` (#4866)
- supprimer `garbage` dans la poignée de main p2p (#4889)
- supprimer `committed_topology` du bloc (#4880)

### Sécurité

- se prémunir contre les fuites de secrets

## [2.0.0-pre-rc.21] - 2024-04-19

### Ajouté

- inclure l'identifiant du déclencheur dans le point d'entrée du déclencheur (#4391)
- exposer l'événement défini sous forme de champs de bits dans le schéma (#4381)
- introduire le nouveau `wsv` avec accès granulaire (#2664)
- ajouter des filtres d'événements pour les événements `PermissionTokenSchemaUpdate`, `Configuration` et `Executor`
- introduire le "mode" instantané (#4365)
- autoriser l'octroi/la révocation des autorisations du rôle (#4244)
- introduire un type numérique de précision arbitraire pour les actifs (supprimer tous les autres types numériques) (#3660)
- limite de carburant différente pour Executor (#3354)
- intégrer le profileur pprof (#4250)
- ajouter une sous-commande d'actif dans la CLI client (#4200)
- Autorisations `Register<AssetDefinition>` (#4049)
- ajouter `chain_id` pour empêcher les attaques par rejeu (#4185)
- ajouter des sous-commandes pour modifier les métadonnées du domaine dans la CLI client (#4175)
- implémenter l'ensemble de magasins, supprimer, obtenir des opérations dans la CLI client (#4163)
- compter les contrats intelligents identiques pour les déclencheurs (#4133)
- ajouter une sous-commande dans la CLI client pour transférer des domaines (#3974)
- prise en charge des tranches en boîte dans FFI (#4062)
- git commit SHA sur la CLI client (#4042)
- macro proc pour le passe-partout du validateur par défaut (#3856)
- introduction du générateur de requêtes de requête dans l'API client (#3124)
- requêtes paresseuses dans les contrats intelligents (#3929)
- Paramètre de requête `fetch_size` (#3900)
- instruction de transfert de magasin d'actifs (#4258)
- se prémunir contre les fuites de secrets (#3240)
- dédoublonner les triggers avec le même code source (#4419)

### Modifié- faire tomber la chaîne d'outils antirouille la nuit-2024-04-18
- envoyer des blocs aux validateurs du Set B (#4387)
- diviser les événements du pipeline en événements de bloc et de transaction (#4366)
- renommer la section de configuration `[telemetry.dev]` en `[dev_telemetry]` (#4377)
- créer des types `Action` et `Filter` non génériques (#4375)
- améliorer l'API de filtrage des événements avec le modèle de générateur (#3068)
- unifier diverses API de filtrage d'événements, introduire une API de création fluide
- renommer `FilterBox` en `EventFilterBox`
- renommer `TriggeringFilterBox` en `TriggeringEventFilterBox`
- améliorer la dénomination des filtres, par ex. `AccountFilter` -> `AccountEventFilter`
- réécrire la config selon la configuration RFC (#4239)
- masquer la structure interne des structures versionnées de l'API publique (#3887)
- introduire temporairement un ordre prévisible après trop de changements de vue ayant échoué (#4263)
- utiliser des types de clés concrètes dans `iroha_crypto` (#4181)
- Modifications de la vue partagée par rapport aux messages normaux (#4115)
- rendre `SignedTransaction` immuable (#4162)
- exporter `iroha_config` via `iroha_client` (#4147)
- exporter `iroha_crypto` via `iroha_client` (#4149)
- exporter `data_model` via `iroha_client` (#4081)
- supprimer la dépendance `openssl-sys` de `iroha_crypto` et introduire des backends tls configurables dans `iroha_client` (#3422)
- remplacer l'EOF `hyperledger/ursa` non maintenu par la solution interne `iroha_crypto` (#3422)
- optimiser les performances de l'exécuteur (#4013)
- Mise à jour de la topologie par les pairs (#3995)

### Corrigé

- supprimer les déclencheurs correspondants sur `Unregister<Domain>` (#4461)
- supprimer les autorisations des rôles lors de la désinscription de l'entité (#4242)
- affirmer que la transmission Genesis est signée par la clé Genesis Pub (# 4253)
- introduire un délai d'attente pour les pairs qui ne répondent pas dans le p2p (#4267)
- empêcher l'enregistrement du domaine ou du compte Genesis (# 4226)
- `MinSize` pour `ChaCha20Poly1305` (#4395)
- démarrer la console lorsque `tokio-console` est activé (#4377)
- séparez chaque élément avec `\n` et créez de manière récursive des répertoires parents pour les journaux de fichiers `dev-telemetry`
- empêcher l'enregistrement d'un compte sans signature (#4212)
- la génération de bi-clés est désormais infaillible (#4283)
- arrêter d'encoder les clés `X25519` en `Ed25519` (#4174)
- faire la validation de signature dans `no_std` (#4270)
- appeler des méthodes de blocage dans un contexte asynchrone (#4211)
- révoquer les jetons associés lors de la désinscription de l'entité (#3962)
- bug de blocage asynchrone au démarrage de Sumeragi
- Correction de `(get|set)_config` 401 HTTP (#4177)
- Nom de l'archiveur `musl` dans Docker (#4193)
- impression de débogage du contrat intelligent (#4178)
- mise à jour de la topologie au redémarrage (#4164)
- enregistrement d'un nouveau pair (#4142)
- ordre d'itération prévisible sur la chaîne (#4130)
- réarchitecture de l'enregistreur et de la configuration dynamique (#4100)
- déclencher l'atomicité (#4106)
- problème d'ordre des messages du magasin de requêtes (#4057)
- définissez `Content-Type: application/x-norito` pour les points de terminaison qui répondent en utilisant Norito

### Supprimé

- Paramètre de configuration `logger.tokio_console_address` (#4377)
-`NotificationEvent` (#4377)
- Énumération `Value` (#4305)
- Agrégation MST d'iroha (#4229)
- clonage pour ISI et exécution de requêtes dans les contrats intelligents (#4182)
- Fonctionnalités `bridge` et `dex` (#4152)
- événements aplatis (#3068)
-expressions (#4089)
- référence de configuration générée automatiquement
- Bruit `warp` dans les logs (#4097)

### Sécurité

- empêcher l'usurpation de clé de pub en p2p (#4065)
- s'assurer que les signatures `secp256k1` sortant d'OpenSSL sont normalisées (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### Ajouté- Transférer la propriété `Domain`
- Autorisations du propriétaire `Domain`
- Ajouter le champ `owned_by` à `Domain`
- analyser le filtre en JSON5 dans `iroha_client_cli` (#3923)
- Ajout de la prise en charge de l'utilisation du type Self dans les énumérations serde partiellement balisées
- Standardiser l'API de bloc (#3884)
- Implémenter le mode d'initialisation kura `Fast`
- Ajouter l'en-tête de clause de non-responsabilité iroha_swarm
- prise en charge initiale des instantanés WSV

### Corrigé

- Correction du téléchargement de l'exécuteur dans update_configs.sh (#3990)
- rustc approprié dans devShell
- Correction des répétitions de gravure `Trigger`
- Correction du transfert `AssetDefinition`
- Correction de `RemoveKeyValue` pour `Domain`
- Correction de l'utilisation de `Span::join`
- Correction d'un bug d'incompatibilité de topologie (#3903)
- Correction des benchmarks `apply_blocks` et `validate_blocks`
- `mkdir -r` avec chemin de stockage, pas de chemin de verrouillage (#3908)
- N'échouez pas si le répertoire existe dans test_env.py
- Correction de la docstring d'authentification/autorisation (#3876)
- Meilleur message d'erreur pour l'erreur de recherche de requête
- Ajouter la clé publique du compte Genesis à Dev Docker Compose
- Comparez la charge utile du jeton d'autorisation en tant que JSON (#3855)
- Correction de `irrefutable_let_patterns` dans la macro `#[model]`
- Autoriser Genesis à exécuter n'importe quel ISI (#3850)
- Correction de la validation de la genèse (#3844)
- Correction de la topologie pour 3 pairs ou moins
- Corrigez la façon dont l'histogramme tx_amounts est calculé.
- Test de desquamation `genesis_transactions_are_validated()`
- Génération de validateur par défaut
- Correction de l'arrêt progressif d'Iroha

### Refactoriser- supprimer les dépendances inutilisées (#3992)
- Dépendances de bosse (#3981)
- Renommer le validateur en exécuteur (#3976)
- Supprimer `IsAssetDefinitionOwner` (#3979)
- Inclure le code du contrat intelligent dans l'espace de travail (#3944)
- Fusionner les points de terminaison API et télémétrie en un seul serveur
- déplacer l'expression len de l'API publique vers le noyau (#3949)
- Évitez de cloner dans la recherche de rôles
- Requêtes de plage pour les rôles
- Déplacer les rôles de compte vers `WSV`
- Renommez ISI de *Box en *Expr (#3930)
- Supprimer le préfixe 'Versioned' des conteneurs versionnés (#3913)
- déplacer `commit_topology` dans la charge utile du bloc (#3916)
- Migrer la macro `telemetry_future` vers syn 2.0
- Enregistré auprès d'Identifiable dans les limites de l'ISI (#3925)
- Ajouter la prise en charge des génériques de base à `derive(HasOrigin)`
- Nettoyer la documentation des API Emitter pour rendre Clippy heureux
- Ajouter des tests pour la macro derive (HasOrigin), réduire les répétitions dans derive (IdEqOrdHash), corriger le rapport d'erreurs sur stable
- Améliorez la dénomination, simplifiez les .filter_maps répétés et débarrassez-vous des .sauf inutiles dans derive (Filter)
- Faire en sorte que PartiallyTaggedSerialize/Deserialize utilise chéri
- Faire dériver (IdEqOrdHash) utiliser chéri, ajouter des tests
- Faire dériver (Filtre) utiliser chérie
- Mettre à jour iroha_data_model_derive pour utiliser syn 2.0
- Ajouter des tests unitaires de condition de vérification de signature
- Autoriser uniquement un ensemble fixe de conditions de vérification de signature
- Généraliser ConstBytes dans un ConstVec contenant n'importe quelle séquence const
- Utiliser une représentation plus efficace pour les valeurs d'octets qui ne changent pas
- Stocker le fichier wsv finalisé dans un instantané
- Ajouter l'acteur `SnapshotMaker`
- limitation du document sur l'analyse des dérivés dans les macros proc
- nettoyer les commentaires
- extraire un utilitaire de test commun pour analyser les attributs de lib.rs
- utilisez parse_display et mettez à jour Attr -> Nom Attrs
- autoriser l'utilisation de la correspondance de modèles dans les arguments de la fonction ffi
- réduire la répétition dans l'analyse des attributs getset
- renommer Emitter::into_token_stream en Emitter::finish_token_stream
- Utilisez parse_display pour analyser les jetons getset
- Corriger les fautes de frappe et améliorer les messages d'erreur
- iroha_ffi_derive : utilisez Darling pour analyser les attributs et utilisez syn 2.0
- iroha_ffi_derive : remplacez proc-macro-error par manyhow
- Simplifiez le code du fichier de verrouillage Kura
- sérialiser toutes les valeurs numériques sous forme de chaînes littérales
- Séparation Kagami (#3841)
- Réécrire `scripts/test-env.sh`
- Différencier les contrats intelligents et les points d'entrée déclencheurs
-Élide `.cloned()` dans `data_model/src/block.rs`
- mettre à jour `iroha_schema_derive` pour utiliser syn 2.0

## [2.0.0-pre-rc.19] - 2023-08-14

### Ajouté- Hyperledger#3309 Bump IVM runtime pour une amélioration
- hyperledger#3383 Implémenter une macro pour analyser les adresses d'un socket au moment de la compilation
- hyperledger#2398 Ajouter des tests d'intégration pour les filtres de requêtes
- Inclure le message d'erreur réel dans `InternalError`
- Utilisation de `nightly-2023-06-25` comme chaîne d'outils par défaut
- Migration du validateur hyperledger#3692
- [Stage DSL] hyperledger#3688 : Implémenter l'arithmétique de base en tant que macro proc
- hyperledger#3371 Split validateur `entrypoint` pour garantir que les validateurs ne sont plus considérés comme des contrats intelligents
- les instantanés WSV hyperledger#3651, qui permettent d'afficher rapidement un nœud Iroha après un crash
- hyperledger#3752 Remplacez `MockValidator` par un validateur `Initial` qui accepte toutes les transactions
- hyperledger#3276 Ajout d'une instruction temporaire appelée `Log` qui enregistre une chaîne spécifiée dans le journal principal du nœud Iroha
- hyperledger#3641 Rendre la charge utile du jeton d'autorisation lisible par l'homme
- hyperledger#3324 Ajout de vérifications et de refactorisation `iroha_client_cli` liées à `burn`
- hyperledger#3781 Valider les transactions de genèse
- hyperledger#2885 Différencier les événements qui peuvent et ne peuvent pas être utilisés pour les déclencheurs
- Construction basée sur hyperledger#2245 `Nix` du binaire du nœud iroha sous le nom `AppImage`

### Corrigé

- Régression hyperledger#3613 qui pourrait permettre d'accepter des transactions mal signées
- Rejeter rapidement la topologie de configuration incorrecte
- hyperledger#3445 Corrige la régression et fait fonctionner à nouveau `POST` sur le point de terminaison `/configuration`
- hyperledger#3654 Correction de `iroha2` `glibc` basée sur `Dockerfiles` à déployer
- hyperledger#3451 Fix `docker` construit sur les Mac Apple Silicon
- hyperledger#3741 Correction de l'erreur `tempfile` dans `kagami validator`
- hyperledger#3758 Correction de la régression où les caisses individuelles ne pouvaient pas être construites, mais pouvaient être construites dans le cadre de l'espace de travail
- Hyperledger#3777 La faille du correctif dans l'enregistrement des rôles n'est pas validée
- hyperledger#3805 Correction de Iroha qui ne s'arrête pas après avoir reçu `SIGTERM`

### Autre

- hyperledger#3648 Inclut la vérification `docker-compose.*.yml` dans les processus CI
- Déplacer l'instruction `len()` de `iroha_data_model` vers `iroha_core`
- hyperledger#3672 Remplacez `HashMap` par `FxHashMap` dans les macros dérivées
- Commentaires sur la documentation de l'erreur Hyperledger#3374 Unify et implémentation `fmt::Display`
- hyperledger#3289 Utiliser l'héritage de l'espace de travail Rust 1.70 tout au long du projet
- hyperledger#3654 Ajoutez `Dockerfiles` pour construire iroha2 sur `GNU libc <https://www.gnu.org/software/libc/>`_
- Présentation de `syn` 2.0, `manyhow` et `darling` pour les macros proc
- graine hyperledger#3802 Unicode `kagami crypto`

## [2.0.0-pré-rc.18]

### Ajouté

- hyperledger#3468 : curseur côté serveur, qui permet une pagination réentrante évaluée paresseusement, ce qui devrait avoir des implications positives majeures en termes de performances pour la latence des requêtes
- hyperledger#3624 : jetons d'autorisation à usage général ; spécifiquement
  - Les jetons d'autorisation peuvent avoir n'importe quelle structure
  - La structure du jeton est auto-décrite dans le `iroha_schema` et sérialisée sous forme de chaîne JSON
  - La valeur du jeton est codée en `Norito`.
  - En conséquence de ce changement, la convention de dénomination des jetons d'autorisation a été déplacée de `snake_case` à `UpeerCamelCase`.
- hyperledger#3615 Préserver wsv après validation

### Corrigé- hyperledger#3627 Atomicité des transactions désormais appliquée via le clonage du `WorlStateView`
- hyperledger#3195 Extension du comportement de panique lors de la réception d'une transaction Genesis rejetée
- hyperledger#3042 Correction d'un message de demande incorrecte
- hyperledger#3352 Divisez le flux de contrôle et les messages de données en canaux séparés
- hyperledger#3543 Améliorer la précision des métriques

## 2.0.0-pré-rc.17

### Ajouté

- hyperledger#3330 Extend désérialisation `NumericValue`
- prise en charge de l'hyperledger#2622 `u128`/`i128` dans FFI
- hyperledger#3088 Introduire la limitation de file d'attente, pour empêcher le DoS
- variantes de commandes hyperledger#2373 `kagami swarm file` et `kagami swarm dir` pour générer des fichiers `docker-compose`
- Analyse du jeton d'autorisation hyperledger#3597 (côté Iroha)
- hyperledger#3353 Supprimez `eyre` de `block.rs` en énumérant les conditions d'erreur et en utilisant des erreurs fortement typées
- hyperledger#3318 Entrelacer les transactions rejetées et acceptées en blocs pour préserver l'ordre de traitement des transactions

### Corrigé

- hyperledger#3075 Panique en cas de transaction non valide dans le `genesis.json` pour empêcher le traitement des transactions non valides
- hyperledger#3461 Gestion correcte des valeurs par défaut dans la configuration par défaut
- hyperledger#3548 Correction de l'attribut transparent `IntoSchema`
- hyperledger#3552 Correction de la représentation du schéma du chemin du validateur
- hyperledger#3546 Correction du blocage des déclencheurs temporels
- hyperledger#3162 Interdire une hauteur de 0 dans les requêtes de streaming en bloc
- Test initial des macros de configuration
- Hyperledger#3592 Correctif pour les fichiers de configuration en cours de mise à jour sur `release`
- hyperledger#3246 N'impliquez pas `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_ sans `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_
- hyperledger#3570 Afficher correctement les erreurs de requête de chaîne côté client
- l'hyperledger#3596 `iroha_client_cli` affiche les blocs/événements
- hyperledger#3473 Faire fonctionner `kagami validator` en dehors du répertoire racine du référentiel iroha

### Autre

- hyperledger#3063 Mapper la transaction `hash` pour bloquer la hauteur dans `wsv`
- `HashOf<T>` fortement typé dans `Value`

## [2.0.0-pré-rc.16]

### Ajouté

- sous-commande hyperledger#2373 `kagami swarm` pour générer `docker-compose.yml`
- hyperledger#3525 Standardiser l'API des transactions
- hyperledger#3376 Ajouter le cadre d'automatisation Iroha Client CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_
- hyperledger#3516 Conserver le hachage blob d'origine dans `LoadedExecutable`

### Corrigé

- hyperledger#3462 Ajouter la commande d'actif `burn` à `client_cli`
- Types d'erreurs de refactorisation hyperledger#3233
- hyperledger#3330 Correction de la régression, en implémentant manuellement `serde::de::Deserialize` pour `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums`
- hyperledger#3487 Renvoie les types manquants dans le schéma
- hyperledger#3444 Renvoyer le discriminant dans le schéma
- hyperledger#3496 Correction de l'analyse des champs `SocketAddr`
- hyperledger#3498 Correction de la détection de soft-fork
- hyperledger#3396 Stocker le bloc dans `kura` avant d'émettre un événement de validation de bloc

### Autre

- hyperledger#2817 Supprime la mutabilité intérieure de `WorldStateView`
- Refactorisation de l'API Genesis hyperledger#3363
- Refactoriser l'existant et compléter avec de nouveaux tests de topologie
- Passer de `Codecov <https://about.codecov.io/>`_ à `Coveralls <https://coveralls.io/>`_ pour la couverture des tests
- hyperledger#3533 Renommer `Bool` en `bool` dans le schéma

## [2.0.0-pré-rc.15]

### Ajouté- hyperledger#3231 Validateur monolithique
- hyperledger#3015 Prise en charge de l'optimisation de niche dans FFI
- hyperledger#2547 Ajouter un logo à `AssetDefinition`
- hyperledger#3274 Ajouter à `kagami` une sous-commande qui génère des exemples (rétroportés dans LTS)
- hyperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ flocon
- hyperledger#3412 Déplacer les potins sur les transactions vers un acteur distinct
- hyperledger#3435 Présenter le visiteur `Expression`
- hyperledger#3168 Fournir le validateur Genesis dans un fichier séparé
- hyperledger#3454 Faire de LTS la valeur par défaut pour la plupart des opérations et de la documentation Docker
- hyperledger#3090 Propager les paramètres en chaîne de la blockchain vers `sumeragi`

### Corrigé

- hyperledger#3330 Correction de la désérialisation d'énumération non balisée avec les feuilles `u128` (rétroportées dans RC14)
- hyperledger#2581 réduit le bruit dans les journaux
- hyperledger#3360 Correction du benchmark `tx/s`
- hyperledger#3393 Rompre la boucle de blocage de communication dans `actors`
- hyperledger#3402 Correction de la version `nightly`
- hyperledger#3411 Gérer correctement la connexion simultanée des pairs
- hyperledger#3440 Déprécier les conversions d'actifs pendant le transfert, à la place gérées par des contrats intelligents
- hyperledger#3408 : Correction du test `public_keys_cannot_be_burned_to_nothing`

### Autre

- hyperledger#3362 Migrer vers les acteurs `tokio`
- hyperledger#3349 Supprimer `EvaluateOnHost` des contrats intelligents
- hyperledger#1786 Ajout de types natifs `iroha` pour les adresses de socket
- Désactiver le cache IVM
- Réactiver le cache IVM
- Renommez le validateur d'autorisation en validateur
- hyperledger#3388 Faire de `model!` une macro d'attribut au niveau du module
- hyperledger#3370 Sérialiser `hash` sous forme de chaîne hexadécimale
- Déplacer `maximum_transactions_in_block` de la configuration `queue` vers `sumeragi`
- Déprécier et supprimer le type `AssetDefinitionEntry`
- Renommer `configs/client_cli` en `configs/client`
- Mise à jour `MAINTAINERS.md`

## [2.0.0-pré-rc.14]

### Ajouté

- modèle de données hyperledger#3127 `structs` opaque par défaut
- hyperledger#3122 utilise `Algorithm` pour stocker la fonction digest (contributeur de la communauté)
- La sortie hyperledger#3153 `iroha_client_cli` est lisible par machine
- hyperledger#3105 Implémenter `Transfer` pour `AssetDefinition`
- Hyperledger#3010 `Transaction` événement de pipeline d'expiration ajouté

### Corrigé

- révision hyperledger#3113 des tests réseau instables
- hyperledger#3129 Fix `Parameter` dé/sérialisation
- hyperledger#3141 Implémenter manuellement `IntoSchema` pour `Hash`
- hyperledger#3155 Correction d'un crochet de panique dans les tests, évitant ainsi les blocages
- hyperledger#3166 Ne visualise pas les modifications au repos, améliorant ainsi les performances
- hyperledger#2123 Retour à la dé/sérialisation de PublicKey à partir de multihash
- hyperledger#3132 Ajouter un validateur NewParameter
- hyperledger#3249 Diviser les hachages de blocs en versions partielles et complètes
- hyperledger#3031 Correction de l'UI/UX des paramètres de configuration manquants
- hyperledger#3247 Suppression de l'injection de fautes de `sumeragi`.

### Autre

- Ajout du `#[cfg(debug_assertions)]` manquant pour corriger les pannes parasites
- hyperledger#2133 Réécrire la topologie pour être plus proche du livre blanc
- Supprimer la dépendance `iroha_client` sur `iroha_core`
- hyperledger#2943 Dériver `HasOrigin`
- hyperledger#3232 Partager les métadonnées de l'espace de travail
- hyperledger#3254 Refactor `commit_block()` et `replace_top_block()`
- Utiliser un gestionnaire d'allocation par défaut stable
- hyperledger#3183 Renommez les fichiers `docker-compose.yml`
- Amélioration du format d'affichage `Multihash`
- hyperledger#3268 Identificateurs d'articles uniques au monde
- Nouveau modèle de relations publiques

## [2.0.0-pré-rc.13]

### Ajouté- hyperledger#2399 Paramètres de configuration en tant qu'ISI.
- hyperledger#3119 Ajouter la métrique `dropped_messages`.
- hyperledger#3094 Générer un réseau avec les pairs `n`.
- hyperledger#3082 Fournissez des données complètes dans l'événement `Created`.
- hyperledger#3021 Import de pointeur opaque.
- hyperledger#2794 Rejeter les énumérations sans champ avec des discriminants explicites dans FFI.
- hyperledger#2922 Ajouter `Grant<Role>` à la genèse par défaut.
- hyperledger#2922 Omettre le champ `inner` dans la désérialisation `NewRole` json.
- hyperledger#2922 Omettre `object(_id)` dans la désérialisation json.
- hyperledger#2922 Omettre `Id` dans la désérialisation json.
- hyperledger#2922 Omettre `Identifiable` dans la désérialisation json.
- hyperledger#2963 Ajoutez `queue_size` aux métriques.
- hyperledger#3027 implémente le fichier de verrouillage pour Kura.
- hyperledger#2813 Kagami génère une configuration homologue par défaut.
- hyperledger#3019 Prise en charge de JSON5.
- hyperledger#2231 Générer l'API du wrapper FFI.
- hyperledger#2999 Accumuler les signatures de bloc.
- hyperledger#2995 Détection de soft fork.
- hyperledger#2905 Étendre les opérations arithmétiques pour prendre en charge `NumericValue`
- hyperledger#2868 Émet la version iroha et valide le hachage dans les journaux.
- hyperledger#2096 Requête pour le montant total de l'actif.
- hyperledger#2899 Ajouter une sous-commande multi-instructions dans 'client_cli'
- hyperledger#2247 Supprime le bruit de communication du websocket.
- hyperledger#2889 Ajout de la prise en charge du streaming en bloc dans `iroha_client`
- hyperledger#2280 Produire des événements d'autorisation lorsque le rôle est accordé/révoqué.
- hyperledger#2797 Enrichir les événements.
- hyperledger#2725 Réintroduire le délai d'attente dans `submit_transaction_blocking`
- Hyperledger#2712 Demandes de configuration.
- Prise en charge de l'hyperledger#2491 Enum dans FFi.
- hyperledger#2775 Générer différentes clés en genèse synthétique.
- hyperledger#2627 Finalisation de la config, point d'entrée proxy, kagami docgen.
- hyperledger#2765 Générer une genèse synthétique dans `kagami`
- hyperledger#2698 Correction d'un message d'erreur peu clair dans `iroha_client`
- hyperledger#2689 Ajout de paramètres de définition de jeton d'autorisation.
- hyperledger#2502 Stocke le hachage GIT de la build.
- hyperledger#2672 Ajouter la variante et les prédicats `ipv4Addr`, `ipv6Addr`.
- hyperledger#2626 Implémenter la dérive `Combine`, diviser les macros `config`.
- hyperledger#2586 `Builder` et `LoadFromEnv` pour les structures proxy.
- hyperledger#2611 Dérivez `TryFromReprC` et `IntoFfi` pour les structures opaques génériques.
- hyperledger#2587 Divisez `Configurable` en deux traits. #2587 : Diviser `Configurable` en deux traits
- hyperledger#2488 Ajout de la prise en charge des impls de traits dans `ffi_export`
- hyperledger#2553 Ajout d'un tri aux requêtes d'actifs.
- hyperledger#2407 Déclencheurs de paramétrage.
- hyperledger#2536 Présentez `ffi_import` pour les clients FFI.
- hyperledger#2338 Ajouter l'instrumentation `cargo-all-features`.
- Options d'algorithme de l'outil hyperledger#2564 Kagami.
- hyperledger#2490 Implémentez ffi_export pour les fonctions autonomes.
- hyperledger#1891 Valider l'exécution du déclencheur.
- hyperledger#1988 Dérive des macros pour Identifiable, Eq, Hash, Ord.
- Bibliothèque bindgen hyperledger#2434 FFI.
- hyperledger#2073 Préférez ConstString à String pour les types dans la blockchain.
- hyperledger#1889 Ajouter des déclencheurs à l'échelle du domaine.
- hyperledger#2098 Bloquer les requêtes d'en-tête. #2098 : ajouter des requêtes d'en-tête de bloc
- hyperledger#2467 Ajoutez la sous-commande account grant dans iroha_client_cli.
- hyperledger#2301 Ajoute le hachage de bloc de la transaction lors de son interrogation.
 - hyperledger#2454 Ajout d'un script de build à l'outil de décodage Norito.
- hyperledger#2061 Dériver une macro pour les filtres.- hyperledger#2228 Ajouter une variante non autorisée à l'erreur de requête des contrats intelligents.
- hyperledger#2395 Ajoute une panique si Genesis ne peut pas être appliqué.
- hyperledger#2000 Interdire les noms vides. #2000 : Interdire les noms vides
 - hyperledger#2127 Ajout d'un contrôle d'intégrité pour garantir que toutes les données décodées par le codec Norito sont consommées.
- hyperledger#2360 Rendre `genesis.json` à nouveau facultatif.
- hyperledger#2053 Ajoutez des tests à toutes les requêtes restantes dans la blockchain privée.
- hyperledger#2381 Unifier l'enregistrement `Role`.
- hyperledger#2053 Ajout de tests aux requêtes liées aux actifs dans la blockchain privée.
- hyperledger#2053 Ajouter des tests à 'private_blockchain'
- hyperledger#2302 Ajout d'une requête stub 'FindTriggersByDomainId'.
- hyperledger#1998 Ajouter des filtres aux requêtes.
- hyperledger#2276 Inclut le hachage de bloc actuel dans BlockHeaderValue.
- hyperledger#2161 Identifiant du handle et fns FFI partagés.
- ajouter un identifiant de handle et implémenter des équivalents FFI de traits partagés (Clone, Eq, Ord)
- hyperledger#1638 `configuration` renvoie la sous-arborescence de la documentation.
- hyperledger#2132 Ajout d'une macro de procédure `endpointN`.
- hyperledger#2257 Revoke<Role> émet l'événement RoleRevoked.
- hyperledger#2125 Ajouter une requête FindAssetDefinitionById.
- hyperledger#1926 Ajout d'une gestion du signal et d'un arrêt progressif.
- l'hyperledger#2161 génère des fonctions FFI pour `data_model`
- hyperledger#1149 Le nombre de fichiers bloqués ne dépasse pas 1 000 000 par répertoire.
- hyperledger#1413 Ajouter un point de terminaison de version API.
- Hyperledger#2103 prend en charge les requêtes de blocs et de transactions. Ajouter une requête `FindAllTransactions`
- hyperledger#2186 Ajout du transfert ISI pour `BigQuantity` et `Fixed`.
- hyperledger#2056 Ajout d'une caisse de macro de procédure dérivée pour `AssetValueType` `enum`.
- hyperledger#2100 Ajouter une requête pour trouver tous les comptes avec des actifs.
- hyperledger#2179 Optimiser l'exécution du déclencheur.
- hyperledger#1883 Supprime les fichiers de configuration intégrés.
- hyperledger#2105 gère les erreurs de requête dans le client.
- hyperledger#2050 Ajouter des requêtes liées au rôle.
- hyperledger#1572 Jetons d'autorisation spécialisés.
- hyperledger#2121 Vérifiez que la paire de clés est valide une fois construite.
 - hyperledger#2003 Présentation de l'outil de décodage Norito.
- hyperledger#1952 Ajout d'un benchmark TPS comme standard pour les optimisations.
- hyperledger#2040 Ajout d'un test d'intégration avec limite d'exécution des transactions.
- hyperledger#1890 Introduire des tests d'intégration basés sur les cas d'utilisation d'Orillion.
- hyperledger#2048 Ajouter un fichier de chaîne d'outils.
- hyperledger#2100 Ajouter une requête pour trouver tous les comptes avec des actifs.
- hyperledger#2179 Optimiser l'exécution du déclencheur.
- hyperledger#1883 Supprime les fichiers de configuration intégrés.
- hyperledger#2004 Interdire à `isize` et `usize` de devenir `IntoSchema`.
- hyperledger#2105 gère les erreurs de requête dans le client.
- hyperledger#2050 Ajouter des requêtes liées au rôle.
- hyperledger#1572 Jetons d'autorisation spécialisés.
- hyperledger#2121 Vérifiez que la paire de clés est valide une fois construite.
 - hyperledger#2003 Présentation de l'outil de décodage Norito.
- hyperledger#1952 Ajout d'un benchmark TPS comme standard pour les optimisations.
- hyperledger#2040 Ajout d'un test d'intégration avec limite d'exécution des transactions.
- hyperledger#1890 Introduire des tests d'intégration basés sur les cas d'utilisation d'Orillion.
- hyperledger#2048 Ajouter un fichier de chaîne d'outils.
- hyperledger#2037 Introduction des déclencheurs de pré-validation.
- hyperledger#1621 Présentation par déclencheurs d'appel.
- hyperledger#1970 Ajouter un point de terminaison de schéma facultatif.
- hyperledger#1620 Introduire des déclencheurs basés sur le temps.
- hyperledger#1918 Implémenter l'authentification de base pour `client`
- hyperledger#1726 Implémenter un workflow de relations publiques de publication.
- hyperledger#1815 Rendre les réponses aux requêtes plus structurées.- hyperledger#1928 implémente la génération du journal des modifications en utilisant `gitchangelog`
- Hyperledger#1902 Script de configuration Bare Metal à 4 pairs.

  Ajout d'une version de setup_test_env.sh qui ne nécessite pas docker-compose et utilise la version de débogage de Iroha.
- hyperledger#1619 Introduire des déclencheurs basés sur des événements.
- hyperledger#1195 Fermez proprement une connexion websocket.
- hyperledger#1606 Ajouter un lien ipfs au logo du domaine dans la structure du domaine.
- hyperledger#1754 Ajout de la CLI de l'inspecteur Kura.
- hyperledger#1790 Améliorez les performances en utilisant des vecteurs basés sur la pile.
- hyperledger#1805 Couleurs de terminal facultatives pour les erreurs de panique.
- hyperledger#1749 `no_std` dans `data_model`
- hyperledger#1179 Ajout d'une instruction de révocation d'autorisation ou de rôle.
- hyperledger#1782 rend iroha_crypto no_std compatible.
- hyperledger#1172 Implémenter les événements d'instruction.
- hyperledger#1734 Validez `Name` pour exclure les espaces.
- hyperledger#1144 Ajouter une imbrication de métadonnées.
- #1210 Bloquer le streaming (côté serveur).
- hyperledger#1331 Implémentez davantage de métriques `Prometheus`.
- hyperledger#1689 Correction des dépendances des fonctionnalités. #1261 : Ajouter un gonflement de la cargaison.
- hyperledger#1675 utilise le type au lieu de la structure wrapper pour les éléments versionnés.
- hyperledger#1643 Attendez que les pairs valident la genèse dans les tests.
- hyperledger#1678 `try_allocate`
- hyperledger#1216 Ajouter le point de terminaison Prometheus. #1216 : implémentation initiale du point de terminaison des métriques.
- hyperledger#1238 Mises à jour au niveau du journal d'exécution. Création du rechargement de base basé sur le point d'entrée `connection`.
- Formatage du titre hyperledger#1652 PR.
- Ajoutez le nombre de pairs connectés à `Status`

  - Revenir "Supprimer les éléments liés au nombre de pairs connectés"

  Cela annule le commit b228b41dab3c035ce9973b6aa3b35d443c082544.
  - Clarifier que `Peer` a une véritable clé publique uniquement après une poignée de main
  - `DisconnectPeer` sans tests
  - Implémenter l'exécution par les pairs de désenregistrement
  - Ajouter (dé)enregistrer la sous-commande peer à `client_cli`
  - Refuser les reconnexions d'un homologue non enregistré par son adresse

  Une fois que votre homologue se désinscrit et déconnecte un autre homologue,
  votre réseau entendra les demandes de reconnexion du homologue.
  Tout ce que vous pouvez savoir au début, c'est l'adresse dont le numéro de port est arbitraire.
  Alors rappelez-vous le homologue non enregistré par la partie autre que le numéro de port
  et refuser la reconnexion à partir de là
- Ajoutez le point de terminaison `/status` à un port spécifique.

### Correctifs- hyperledger#3129 Correction de la dé/sérialisation `Parameter`.
- hyperledger#3109 Empêche la mise en veille `sumeragi` après un message indépendant du rôle.
- hyperledger#3046 Assurez-vous que Iroha peut démarrer correctement à vide
  `./storage`
- hyperledger#2599 Supprimez les peluches de la pépinière.
- hyperledger#3087 Collectez les votes des validateurs de l'ensemble B après le changement de vue.
- hyperledger#3056 Correction du blocage du benchmark `tps-dev`.
- hyperledger#1170 Implémenter la gestion des soft-fork de style clonage-wsv.
- hyperledger#2456 Rendre le bloc Genesis illimité.
- hyperledger#3038 Réactivez les multisigs.
- hyperledger#2894 Correction de la désérialisation des variables d'environnement `LOG_FILE_PATH`.
- hyperledger#2803 Renvoie le code d'état correct pour les erreurs de signature.
- hyperledger#2963 `Queue` supprime correctement les transactions.
- hyperledger#0000 Vergen cassant le CI.
- hyperledger#2165 Supprimer le fidget de la chaîne d'outils.
- hyperledger#2506 Correction de la validation du bloc.
- hyperledger#3013 Chaîner correctement les validateurs de gravure.
- hyperledger#2998 Supprimer le code de chaîne inutilisé.
- hyperledger#2816 Déplacer la responsabilité de l'accès aux blocs vers kura.
- hyperledger#2384 Remplacez decode par decode_all.
- hyperledger#1967 Remplacez ValueName par Name.
- hyperledger#2980 Correction du type ffi de la valeur du bloc.
- hyperledger#2858 Introduisez parking_lot::Mutex au lieu de std.
- hyperledger#2850 Correction de la désérialisation/décodage de `Fixed`
- hyperledger#2923 Renvoie `FindError` lorsque `AssetDefinition` ne le fait pas
  exister.
- hyperledger#0000 Correction `panic_on_invalid_genesis.sh`
- hyperledger#2880 Fermez correctement la connexion Websocket.
- hyperledger#2880 Correction du blocage du streaming.
- hyperledger#2804 `iroha_client_cli` soumet le blocage des transactions.
- hyperledger#2819 Déplacez les membres non essentiels hors de WSV.
- Correction d'un bug de récursion de sérialisation d'expression.
- hyperledger#2834 Améliore la syntaxe abrégée.
- hyperledger#2379 Ajout de la possibilité de vider de nouveaux blocs Kura dans Blocks.txt.
- hyperledger#2758 Ajouter une structure de tri au schéma.
-CI.
- hyperledger#2548 Avertir en cas de fichier Genesis volumineux.
- hyperledger#2638 Mettre à jour `whitepaper` et propager les modifications.
- hyperledger#2678 Correction des tests sur la branche intermédiaire.
- hyperledger#2678 Correction des tests abandonnés lors de l'arrêt forcé de Kura.
- hyperledger#2607 Refactor du code sumeragi pour plus de simplicité et
  correctifs de robustesse.
- hyperledger#2561 Réintroduire les modifications de vue au consensus.
- hyperledger#2560 Rajoutez block_sync et déconnexion des pairs.
- hyperledger#2559 Ajout de l'arrêt du thread sumeragi.
- hyperledger#2558 Validez Genesis avant de mettre à jour le wsv depuis kura.
- hyperledger#2465 Réimplémenter le nœud sumeragi en tant qu'état monothread
  machine.
- hyperledger#2449 Implémentation initiale de la restructuration Sumeragi.
- hyperledger#2802 Correction du chargement de l'environnement pour la configuration.
- hyperledger#2787 Avertit chaque auditeur de s'arrêter en cas de panique.
- hyperledger#2764 Supprime la limite sur la taille maximale des messages.
- #2571 : Meilleure expérience utilisateur de Kura Inspector.
- hyperledger#2703 Correction des bugs de l'environnement de développement d'Orillion.
- Correction d'une faute de frappe dans un commentaire de document dans schema/src.
- hyperledger#2716 Rendre publique la durée dans Uptime.
- hyperledger#2700 Exporter `KURA_BLOCK_STORE_PATH` dans les images Docker.
- hyperledger#0 Supprimer `/iroha/rust-toolchain.toml` du constructeur
  image.
- Hyperledger#0 Correction `docker-compose-single.yml`
- hyperledger#2554 Erreur d'augmentation si la graine `secp256k1` est inférieure à 32
  octets.
- hyperledger#0 Modifiez `test_env.sh` pour allouer du stockage à chaque homologue.
- hyperledger#2457 Arrêt forcé de kura lors des tests.
- hyperledger#2623 Correction de doctest pour VariantCount.
- Mettre à jour une erreur attendue dans les tests ui_fail.
- Correction d'un commentaire de document incorrect dans les validateurs d'autorisations.- hyperledger#2422 Masquer les clés privées dans la réponse du point de terminaison de configuration.
- hyperledger#2492 : Correction de tous les déclencheurs exécutés qui correspondent à un événement.
- hyperledger#2504 Correction d'un benchmark tps défaillant.
- hyperledger#2477 Correction d'un bug lorsque les autorisations des rôles n'étaient pas comptées.
- hyperledger#2416 Correction des peluches sur le bras macOS.
- hyperledger#2457 Correction des problèmes de test liés à l'arrêt en cas de panique.
  #2457 : Ajout d'un arrêt en cas de configuration de panique
- hyperledger#2473 analyse rustc --version au lieu de RUSTUP_TOOLCHAIN.
- hyperledger#1480 Arrêté en cas de panique. #1480 : Ajouter un crochet de panique pour quitter le programme en cas de panique
- hyperledger#2376 Kura simplifié, pas d'async, deux fichiers.
- Échec de la construction de l'hyperledger#0000 Docker.
- hyperledger#1649 supprime `spawn` de `do_send`
- hyperledger#2128 Correction de la construction et de l'itération `MerkleTree`.
- hyperledger#2137 Préparer des tests pour un contexte multiprocessus.
- hyperledger#2227 Implémenter l'enregistrement et la désinscription des actifs.
- hyperledger#2081 Correction d'un bug d'attribution de rôle.
- hyperledger#2358 Ajout d'une version avec profil de débogage.
- hyperledger#2294 Ajout de la génération de flamegraph à oneshot.rs.
- hyperledger#2202 Correction du champ total dans la réponse à la requête.
- hyperledger#2081 Correction du scénario de test pour accorder le rôle.
- hyperledger#2017 Correction de la désinscription du rôle.
- hyperledger#2303 Correction des pairs de docker-compose qui ne s'arrêtent pas correctement.
- hyperledger#2295 Correction d'un bug de déclenchement de désinscription.
- Hyperledger#2282 améliore les dérivés FFI de l'implémentation de getset.
- hyperledger#1149 Supprime le code nocheckin.
- hyperledger#2232 Faire en sorte que Iroha affiche un message significatif lorsque Genesis a trop d'isi.
- hyperledger#2170 Correction de la construction du conteneur Docker sur les machines M1.
- hyperledger#2215 Rendre le 20/04/2022 facultatif pour `cargo build`
- hyperledger#1990 Activer le démarrage homologue via les variables d'environnement en l'absence de config.json.
- hyperledger#2081 Correction de l'enregistrement des rôles.
- hyperledger#1640 Générez config.json et genesis.json.
- hyperledger#1716 Correction d'un échec de consensus avec f=0 cas.
- hyperledger#1845 Les actifs non monnayables ne peuvent être créés qu'une seule fois.
- hyperledger#2005 Correction `Client::listen_for_events()` ne fermant pas le flux WebSocket.
- hyperledger#1623 Créez un RawGenesisBlockBuilder.
- hyperledger#1917 Ajouter la macro easy_from_str_impl.
- hyperledger#1990 Activer le démarrage homologue via les variables d'environnement en l'absence de config.json.
- hyperledger#2081 Correction de l'enregistrement des rôles.
- hyperledger#1640 Générez config.json et genesis.json.
- hyperledger#1716 Correction d'un échec de consensus avec f=0 cas.
- hyperledger#1845 Les actifs non monnayables ne peuvent être créés qu'une seule fois.
- hyperledger#2005 Correction de `Client::listen_for_events()` ne fermant pas le flux WebSocket.
- hyperledger#1623 Créez un RawGenesisBlockBuilder.
- hyperledger#1917 Ajouter la macro easy_from_str_impl.
- hyperledger#1922 Déplacez crypto_cli dans les outils.
- hyperledger#1969 Intégrez la fonctionnalité `roles` à l'ensemble de fonctionnalités par défaut.
- Arguments CLI du correctif hyperledger#2013.
- hyperledger#1897 Supprime usize/isize de la sérialisation.
- hyperledger#1955 Correction de la possibilité de passer `:` à l'intérieur de `web_login`
- hyperledger#1943 Ajoute des erreurs de requête au schéma.
- hyperledger#1939 Fonctionnalités appropriées pour `iroha_config_derive`.
- hyperledger#1908 corrige la gestion de la valeur zéro pour le script d'analyse de télémétrie.
- hyperledger#0000 Rendre le doc-test implicitement ignoré et explicitement ignoré.
- hyperledger#1848 Empêche la gravure des clés publiques.
- Hyperledger#1811 a ajouté des tests et des contrôles pour déduire les clés des pairs de confiance.
- hyperledger#1821 ajoute IntoSchema pour MerkleTree et VersionedValidBlock, corrige les schémas HashOf et SignatureOf.- hyperledger#1819 Supprime le traçage du rapport d'erreur lors de la validation.
- Hyperledger#1774 enregistre la raison exacte des échecs de validation.
- hyperledger#1714 Comparez PeerId uniquement par clé.
- hyperledger#1788 Réduit l'empreinte mémoire de `Value`.
- hyperledger#1804 corrige la génération de schéma pour HashOf, SignatureOf, ajoute un test pour garantir qu'aucun schéma ne manque.
- Hyperledger#1802 Améliorations de la lisibilité de la journalisation.
  - le journal des événements a été déplacé au niveau de trace
  - ctx supprimé de la capture du journal
  - les couleurs du terminal sont rendues facultatives (pour une meilleure sortie des journaux dans les fichiers)
- hyperledger#1783 Correction du repère torii.
- Correction de l'hyperledger#1772 après #1764.
- hyperledger#1755 Corrections mineures pour #1743, #1725.
  - Correction des JSON selon le changement de structure #1743 `Domain`
- Corrections du consensus hyperledger#1751. #1715 : Correctifs consensuels pour gérer une charge élevée (#1746)
  - Afficher les correctifs de gestion des modifications
  - Afficher les preuves de modifications effectuées indépendamment de hachages de transactions particuliers
  - Passage de messages réduit
  - Collectez les votes de changement de vue au lieu d'envoyer des messages immédiatement (améliore la résilience du réseau)
  - Utiliser pleinement le framework Actor dans Sumeragi (planifier des messages pour soi-même au lieu d'apparitions de tâches)
  - Améliore l'injection de fautes pour les tests avec Sumeragi
  - Rapproche le code de test du code de production
  - Supprime les wrappers trop compliqués
  - Permet à Sumeragi d'utiliser le contexte de l'acteur dans le code de test
- hyperledger#1734 Mettre à jour Genesis pour s'adapter à la nouvelle validation de domaine.
- hyperledger#1742 Erreurs concrètes renvoyées dans les instructions `core`.
- hyperledger#1404 Vérifier corrigé.
- hyperledger#1636 Supprimer `trusted_peers.json` et `structopt`
  #1636 : Supprimez `trusted_peers.json`.
- Mise à jour hyperledger#1706 `max_faults` avec mise à jour de topologie.
- hyperledger#1698 Correction des clés publiques, de la documentation et des messages d'erreur.
- Emissions frappées (1593 et 1405) émission 1405

### Refactoriser- Extraire les fonctions de la boucle principale sumeragi.
- Refactoriser `ProofChain` vers un nouveau type.
- Supprimer `Mutex` de `Metrics`
- Supprimez la fonctionnalité nocturne adt_const_generics.
- hyperledger#3039 Introduire un tampon d'attente pour les multisigs.
- Simplifiez les sumeragi.
- hyperledger#3053 Correction des peluches clippy.
- hyperledger#2506 Ajout de plus de tests sur la validation des blocs.
- Supprimez `BlockStoreTrait` dans Kura.
- Mise à jour des peluches pour `nightly-2022-12-22`
- hyperledger#3022 Supprimer `Option` dans `transaction_cache`
- hyperledger#3008 Ajouter une valeur de niche dans `Hash`
- Mettre à jour les peluches vers 1.65.
- Ajoutez de petits tests pour augmenter la couverture.
- Supprimer le code mort de `FaultInjection`
- Appelez p2p moins souvent depuis sumeragi.
- hyperledger#2675 Valider les noms/identifiants d'éléments sans allouer de Vec.
- hyperledger#2974 Empêche l'usurpation d'identité de bloc sans revalidation complète.
- `NonEmpty` plus efficace dans les combinateurs.
- hyperledger#2955 Supprimer le bloc du message BlockSigned.
- hyperledger#1868 Empêcher l'envoi de transactions validées
  entre pairs.
- hyperledger#2458 Implémenter l'API du combinateur générique.
- Ajoutez un dossier de stockage dans gitignore.
- Ports hyperledger#2909 Hardcode pour le prochain.
- hyperledger#2747 Modification de l'API `LoadFromEnv`.
- Améliorer les messages d'erreur en cas d'échec de configuration.
- Ajouter des exemples supplémentaires à `genesis.json`
- Supprimez les dépendances inutilisées avant la sortie `rc9`.
- Finaliser le peluchage sur le nouveau Sumeragi.
- Extraire les sous-procédures dans la boucle principale.
- hyperledger#2774 Changer le mode de génération de genèse `kagami` de flag à
  sous-commande.
- hyperledger#2478 Ajouter `SignedTransaction`
- hyperledger#2649 Supprimer la caisse `byteorder` de `Kura`
- Renommer `DEFAULT_BLOCK_STORE_PATH` de `./blocks` en `./storage`
- hyperledger#2650 Ajoutez `ThreadHandler` pour arrêter les sous-modules iroha.
- hyperledger#2482 Stocker les jetons d'autorisation `Account` dans `Wsv`
- Ajouter de nouvelles peluches à 1.62.
- Améliorer les messages d'erreur `p2p`.
- vérification de type statique hyperledger#2001 `EvaluatesTo`.
- hyperledger#2052 Rendre les jetons d'autorisation enregistrables avec définition.
  #2052 : implémenter PermissionTokenDefinition
- Assurez-vous que toutes les combinaisons de fonctionnalités fonctionnent.
- hyperledger#2468 Supprime le supertrait de débogage des validateurs d'autorisations.
- hyperledger#2419 Supprime les `drop` explicites.
- hyperledger#2253 Ajouter le trait `Registrable` à `data_model`
- Implémentez `Origin` au lieu de `Identifiable` pour les événements de données.
- Validateurs d'autorisations hyperledger#2369 Refactor.
- hyperledger#2307 Rendre `events_sender` dans `WorldStateView` non facultatif.
- hyperledger#1985 Réduit la taille de la structure `Name`.
- Ajoutez plus de `const fn`.
- Faire des tests d'intégration utiliser `default_permissions()`
- ajoutez des wrappers de jetons d'autorisation dans private_blockchain.
- hyperledger#2292 Supprimer `WorldTrait`, supprimer les génériques de `IsAllowedBoxed`
- hyperledger#2204 Rendre génériques les opérations liées aux actifs.
- hyperledger#2233 Remplacez `impl` par `derive` pour `Display` et `Debug`.
- Améliorations de la structure identifiables.
- Hyperledger#2323 Améliorer le message d'erreur d'initialisation de kura.
- hyperledger#2238 Ajout d'un générateur homologue pour les tests.
- hyperledger#2011 Paramètres de configuration plus descriptifs.
- hyperledger#1896 Simplifie l'implémentation de `produce_event`.
- Refactoriser autour de `QueryError`.
- Déplacez `TriggerSet` vers `data_model`.
- Côté `WebSocket` du client de refactorisation hyperledger#2145, extraire la logique de données pure.
- supprimer le trait `ValueMarker`.
- hyperledger#2149 Expose `Mintable` et `MintabilityError` dans `prelude`
- Hyperledger#2144 refonte du workflow http du client, expose l'API interne.- Déplacez-vous vers `clap`.
- Créer le binaire `iroha_gen`, consolidant les documents, schema_bin.
- hyperledger#2109 Rendre le test `integration::events::pipeline` stable.
- hyperledger#1982 encapsule l'accès aux structures `iroha_crypto`.
- Ajouter le constructeur `AssetDefinition`.
- Supprimez les `&mut` inutiles de l'API.
- encapsuler l'accès aux structures du modèle de données.
- Hyperledger#2144 refonte du workflow http du client, expose l'API interne.
- Déplacez-vous vers `clap`.
- Créer le binaire `iroha_gen`, consolidant les documents, schema_bin.
- hyperledger#2109 Rendre le test `integration::events::pipeline` stable.
- hyperledger#1982 encapsule l'accès aux structures `iroha_crypto`.
- Ajouter le constructeur `AssetDefinition`.
- Supprimez les `&mut` inutiles de l'API.
- encapsuler l'accès aux structures du modèle de données.
- Noyau, `sumeragi`, fonctions d'instance, `torii`
- hyperledger#1903 déplace l'émission d'événements vers les méthodes `modify_*`.
- Diviser le fichier `data_model` lib.rs.
- Ajouter une référence wsv à la file d'attente.
- hyperledger#1210 Flux d'événements divisé.
  - Déplacer les fonctionnalités liées aux transactions vers le module data_model/transaction
- hyperledger#1725 Supprime l'état global dans Torii.
  - Implémentez `add_state macro_rules` et supprimez `ToriiState`
- Correction d'une erreur de linter.
- nettoyage de l'hyperledger#1661 `Cargo.toml`.
  - Trier les dépendances en matière de fret
- hyperledger#1650 ranger `data_model`
  - Déplacer le monde vers wsv, corriger la fonctionnalité de rôles, dériver IntoSchema pour CommitteBlock
- Organisation des fichiers `json` et readme. Mettez à jour le fichier Readme pour vous conformer au modèle.
- 1529 : journalisation structurée.
  - Refactoriser les messages du journal
-`iroha_p2p`
  - Ajouter la privatisation p2p.

###Documentations

- Mettez à jour le fichier Lisez-moi de la CLI client Iroha.
- Mettre à jour les extraits du didacticiel.
- Ajoutez 'sort_by_metadata_key' dans les spécifications de l'API.
- Mettre à jour les liens vers la documentation.
- Étendre le didacticiel avec des documents liés aux actifs.
- Supprimez les fichiers doc obsolètes.
- Réviser la ponctuation.
- Déplacez certains documents vers le référentiel du didacticiel.
- Rapport de friabilité pour la branche de préparation.
- Générer un journal des modifications pour la pré-rc.7.
- Rapport de floconnement du 30 juillet.
- Versions bosses.
- Mettre à jour la squamité du test.
- hyperledger#2499 Correction des messages d'erreur client_cli.
- hyperledger#2344 Générer CHANGELOG pour 2.0.0-pre-rc.5-lts.
- Ajouter des liens vers le tutoriel.
- Mettre à jour les informations sur les hooks git.
- rédaction du test de desquamation.
- hyperledger#2193 Mise à jour de la documentation client Iroha.
- hyperledger#2193 Mise à jour de la documentation CLI Iroha.
- hyperledger#2193 Mise à jour du fichier README pour la caisse de macros.
 - hyperledger#2193 Mise à jour de la documentation de l'outil de décodage Norito.
- hyperledger#2193 Mise à jour de la documentation Kagami.
- hyperledger#2193 Mettre à jour la documentation des benchmarks.
- hyperledger#2192 Consultez les directives de contribution.
- Correction des références cassées dans le code.
- Hyperledger#1280 Document Iroha métriques.
- hyperledger#2119 Ajout de conseils sur la façon de recharger à chaud Iroha dans un conteneur Docker.
- hyperledger#2181 Révisez le fichier README.
- hyperledger#2113 Caractéristiques du document dans les fichiers Cargo.toml.
- hyperledger#2177 Nettoyer la sortie de gitchangelog.
- hyperledger#1991 Ajoutez le fichier readme à l'inspecteur Kura.
- hyperledger#2119 Ajout de conseils sur la façon de recharger à chaud Iroha dans un conteneur Docker.
- hyperledger#2181 Révisez le fichier README.
- hyperledger#2113 Caractéristiques du document dans les fichiers Cargo.toml.
- hyperledger#2177 Nettoyer la sortie de gitchangelog.
- hyperledger#1991 Ajoutez le fichier readme à l'inspecteur Kura.
- générer le dernier journal des modifications.
- Générer un journal des modifications.
- Mettre à jour les fichiers README obsolètes.
- Ajout des documents manquants à `api_spec.md`.

### Modifications CI/CD- Ajoutez cinq autres coureurs auto-hébergés.
- Ajoutez une balise d'image régulière pour le registre Soramitsu.
- Solution de contournement pour libgit2-sys 0.5.0. Revenir à 0.4.4.
- Essayez d'utiliser une image basée sur Arch.
- Mettre à jour les flux de travail pour travailler sur un nouveau conteneur uniquement nocturne.
- Supprimez les points d'entrée binaires de la couverture.
- Basculez les tests de développement vers les coureurs auto-hébergés d'Equinix.
- hyperledger#2865 Suppression de l'utilisation du fichier tmp de `scripts/check.sh`
- hyperledger#2781 Ajouter des décalages de couverture.
- Désactivez les tests d'intégration lents.
- Remplacez l'image de base par le cache Docker.
- hyperledger#2781 Ajout de la fonctionnalité parent de validation codecov.
- Déplacez les tâches vers les coureurs github.
- hyperledger#2778 Vérification de la configuration du client.
- hyperledger#2732 Ajouter des conditions pour mettre à jour les images de base iroha2 et ajouter
  Étiquettes RP.
- Correction de la création d'images nocturnes.
- Correction de l'erreur `buildx` avec `docker/build-push-action`
- Premiers secours en cas de non-fonctionnement `tj-actions/changed-files`
- Activer la publication séquentielle des images, après #2662.
- Ajouter un registre portuaire.
-Étiquetage automatique `api-changes` et `config-changes`
- Valider le hachage dans l'image, à nouveau le fichier de la chaîne d'outils, l'isolation de l'interface utilisateur,
  suivi du schéma.
- Rendre les workflows de publication séquentiels et complémentaires au #2427.
- hyperledger#2309 : réactivez les tests de documentation dans CI.
- hyperledger#2165 Supprimer l'installation de codecov.
- Déplacer vers un nouveau conteneur pour éviter les conflits avec les utilisateurs actuels.
 - hyperledger#2158 Mise à niveau `parity_scale_codec` et autres dépendances. (codec Norito)
- Correction de la construction.
- hyperledger#2461 Améliorer iroha2 CI.
-Mise à jour `syn`.
- déplacer la couverture vers un nouveau flux de travail.
- connexion Docker inversée ver.
- Supprimer la spécification de version de `archlinux:base-devel`
- Mettre à jour la réutilisation et la concurrence des rapports Dockerfiles et Codecov.
- Générer un journal des modifications.
- Ajouter le fichier `cargo deny`.
- Ajouter une branche `iroha2-lts` avec un workflow copié de `iroha2`
- hyperledger#2393 Augmente la version de l'image de base Docker.
- hyperledger#1658 Ajout d'une vérification de la documentation.
- Modification de la version des caisses et suppression des dépendances inutilisées.
- Supprimez les rapports de couverture inutiles.
- hyperledger#2222 Divisez les tests selon qu'ils impliquent ou non une couverture.
- Hyperledger#2153 Correctif #2154.
- Version bump toutes les caisses.
- Correction du pipeline de déploiement.
- hyperledger#2153 Correction de la couverture.
- Ajouter une vérification de genèse et mettre à jour la documentation.
- Augmentez la rouille, la moisissure et la nuit à 1,60, 1,2,0 et 1,62 respectivement.
- déclencheurs de chargement.
- Hyperledger#2153 Correctif #2154.
- Version bump toutes les caisses.
- Correction du pipeline de déploiement.
- hyperledger#2153 Correction de la couverture.
- Ajouter une vérification de genèse et mettre à jour la documentation.
- Bosse la rouille, la moisissure et la nuit à 1,60, 1,2,0 et 1,62 respectivement.
- déclencheurs de chargement.
-load-rs:release déclencheurs de flux de travail.
- Correction du flux de travail push.
- Ajoutez la télémétrie aux fonctionnalités par défaut.
- ajoutez une balise appropriée pour pousser le flux de travail sur le principal.
- Corriger les tests ayant échoué.
- hyperledger#1657 Mettre à jour l'image vers rust 1.57. #1630 : Revenez aux coureurs auto-hébergés.
- Améliorations de l'IC.
- Couverture commutée pour utiliser `lld`.
- Correction de la dépendance CI.
- Améliorations de la segmentation CI.
- Utilise une version fixe de Rust dans CI.
- Correction de la publication Docker et du push CI iroha2-dev. Déplacez la couverture et le banc vers les relations publiques
- Supprimez la version complète inutile de Iroha dans le test Docker CI.

  La version Iroha est devenue inutile car elle est désormais effectuée dans l'image Docker elle-même. Ainsi, le CI construit uniquement le client cli qui est utilisé dans les tests.
- Ajout de la prise en charge de la branche iroha2 dans le pipeline CI.
  - de longs tests n'ont été exécutés que sur PR dans iroha2
  - publier des images Docker uniquement à partir d'iroha2
- Caches CI supplémentaires.

### Assemblage Web


### Modifications de version- Version pré-rc.13.
- Version pré-rc.11.
- Version à RC.9.
- Version à RC.8.
- Mettre à jour les versions vers RC7.
- Préparatifs avant la sortie.
- Mettre à jour le moule 1.0.
- Dépendances de bosse.
- Mise à jour api_spec.md : correction des corps de requête/réponse.
- Mettre à jour la version Rust vers 1.56.0.
- Mettre à jour le guide de contribution.
- Mettez à jour README.md et `iroha/config.json` pour qu'ils correspondent au nouveau format d'API et d'URL.
- Mettre à jour la cible de publication du docker vers hyperledger/iroha2 #1453.
- Met à jour le flux de travail afin qu'il corresponde au principal.
- Mettre à jour les spécifications de l'API et corriger le point de terminaison de santé.
- Mise à jour de Rust vers 1.54.
- Docs(iroha_crypto) : mettre à jour la documentation `Signature` et aligner les arguments de `verify`
- La version Ursa passe de 0.3.5 à 0.3.6.
- Mettre à jour les flux de travail pour les nouveaux coureurs.
- Mettre à jour le fichier docker pour la mise en cache et des builds ci plus rapides.
- Mettre à jour la version libssl.
- Mettre à jour les fichiers docker et async-std.
- Correction de Clippy mis à jour.
- Met à jour la structure des actifs.
  - Prise en charge des instructions clé-valeur dans l'actif
  - Types d'actifs sous forme d'énumération
  - Vulnérabilité de débordement dans le correctif ISI de l'actif
- Mises à jour du guide de contribution.
- Mettre à jour la bibliothèque obsolète.
- Mettre à jour le livre blanc et résoudre les problèmes de peluchage.
- Mettez à jour la bibliothèque concombre_rust.
- Mises à jour README pour la génération de clés.
- Mettre à jour les workflows Github Actions.
- Mettre à jour les workflows Github Actions.
- Mettre à jour le fichier exigences.txt.
- Mettre à jour common.yaml.
- Mises à jour des documents de Sara.
- Mettre à jour la logique des instructions.
- Mettre à jour le livre blanc.
- Met à jour la description des fonctions réseau.
- Mettre à jour le livre blanc en fonction des commentaires.
- Séparation de la mise à jour WSV et de la migration vers Scale.
- Mettre à jour gitignore.
- Mettre à jour légèrement la description du kura dans WP.
- Mettre à jour la description du kura dans le livre blanc.

### Schéma

- hyperledger#2114 Prise en charge des collections triées dans les schémas.
- hyperledger#2108 Ajouter une pagination.
- hyperledger#2114 Prise en charge des collections triées dans les schémas.
- hyperledger#2108 Ajouter une pagination.
- Rendre le schéma, la version et la macro compatibles no_std.
- Correction des signatures dans le schéma.
- Représentation modifiée de `FixedPoint` dans le schéma.
- Ajout de `RawGenesisBlock` à l'introspection de schéma.
- Modification des modèles d'objet pour créer le schéma IR-115.

### Tests

- Doctests du didacticiel hyperledger#2544.
- hyperledger#2272 Ajout de tests pour la requête 'FindAssetDefinitionById'.
- Ajout des tests d'intégration `roles`.
- Standardiser le format des tests d'interface utilisateur, déplacer les tests d'interface utilisateur dérivés pour dériver les caisses.
- Correction de tests simulés (bug futur non ordonné).
- Suppression de la caisse DSL et déplacement des tests vers `data_model`
- Assurez-vous que les tests de réseau instable réussissent pour un code valide.
- Ajout de tests à iroha_p2p.
- Capture les journaux dans les tests sauf si le test échoue.
- Ajoutez des sondages pour les tests et corrigez les tests rarement interrompus.
- Teste la configuration parallèle.
- Supprimez la racine des tests iroha init et iroha_client.
- Correction des avertissements clippy des tests et ajout de vérifications à ci.
- Correction des erreurs de validation `tx` lors des tests de référence.
- hyperledger#860 : Iroha Requêtes et tests.
- Guide ISI personnalisé Iroha et tests Concombre.
- Ajouter des tests pour les clients non standard.
- Modifications et tests d'inscription au pont.
- Tests de consensus avec maquette réseau.
- Utilisation du répertoire temporaire pour l'exécution des tests.
- Des bancs testent les cas positifs.
- Fonctionnalité initiale de Merkle Tree avec tests.
- Correction des tests et de l'initialisation de World State View.

### Autre- Déplacez la paramétrisation dans les traits et supprimez les types FFI IR.
- Ajoutez la prise en charge des unions, introduisez `non_robust_ref_mut` * implémentez la conversion conststring FFI.
- Améliorer IdOrdEqHash.
- Supprimer FilterOpt::BySome de la (dé-)sérialisation.
- Rendre non transparent.
- Rendre ContextValue transparent.
- Rendre la balise Expression::Raw facultative.
- Ajoutez de la transparence pour certaines instructions.
- Améliorer la (dés)sérialisation de RoleId.
- Améliorer la (dés)sérialisation de validateur :: Id.
- Améliorer la (dés)sérialisation de PermissionTokenId.
- Améliorer la (dés)sérialisation de TriggerId.
- Améliorer la (dé-)sérialisation des identifiants d'actifs (-définition).
- Améliorer la (dés)sérialisation de AccountId.
- Améliorer la (dés)sérialisation des Ipfs et DomainId.
- Supprimez la configuration de l'enregistreur de la configuration du client.
- Ajout de la prise en charge des structures transparentes dans FFI.
- Refactoriser &Option<T> en Option<&T>
- Correction des avertissements clippy.
- Ajoutez plus de détails dans la description de l'erreur `Find`.
- Correction des implémentations `PartialOrd` et `Ord`.
- Utilisez `rustfmt` au lieu de `cargo fmt`
- Supprimez la fonctionnalité `roles`.
- Utilisez `rustfmt` au lieu de `cargo fmt`
- Partagez le répertoire de travail en tant que volume avec les instances de développement Docker.
- Supprimer le type associé Diff dans Exécuter.
- Utilisez un encodage personnalisé au lieu d'un retour multival.
- Supprimez serde_json en tant que dépendance iroha_crypto.
- Autoriser uniquement les champs connus dans l'attribut de version.
- Clarifier les différents ports pour les points finaux.
- Supprimer le dérivé `Io`.
- Documentation initiale des key_pairs.
- Revenez aux coureurs auto-hébergés.
- Correction de nouvelles peluches clippy dans le code.
- Supprimez i1i1 des responsables.
- Ajouter une documentation sur l'acteur et des correctifs mineurs.
- Sondez au lieu de pousser les derniers blocs.
- Événements d'état de transaction testés pour chacun des 7 pairs.
- `FuturesUnordered` au lieu de `join_all`
- Passez à GitHub Runners.
- Utilisez VersionedQueryResult vs QueryResult pour le point de terminaison /query.
- Reconnectez la télémétrie.
- Correction de la configuration du robot dépendant.
- Ajoutez le hook git commit-msg pour inclure la signature.
- Réparez le pipeline push.
- Mettre à niveau le robot dépendant.
- Détecter l'horodatage futur lors du push de la file d'attente.
- hyperledger#1197 : Kura gère les erreurs.
- Ajouter une instruction de désinscription des pairs.
- Ajoutez un nom occasionnel facultatif pour distinguer les transactions. Fermez #1493.
- Suppression du `sudo` inutile.
- Métadonnées pour les domaines.
- Correction des rebonds aléatoires dans le workflow `create-docker`.
- Ajout de `buildx` comme suggéré par le pipeline défaillant.
- hyperledger#1454 : Correction de la réponse d'erreur de requête avec un code d'état et des astuces spécifiques.
- hyperledger#1533 : Rechercher une transaction par hachage.
- Correction du point de terminaison `configure`.
- Ajouter une vérification de mintabilité des actifs basée sur des booléens.
- Ajout de primitives crypto typées et migration vers une cryptographie type-safe.
- Améliorations de la journalisation.
- hyperledger#1458 : Ajoutez la taille du canal d'acteur à configurer comme `mailbox`.
- hyperledger#1451 : Ajout d'un avertissement concernant une mauvaise configuration si `faulty_peers = 0` et `trusted peers count > 1`
- Ajouter un gestionnaire pour obtenir un hachage de bloc spécifique.
- Ajout d'une nouvelle requête FindTransactionByHash.
- hyperledger#1185 : modifiez le nom et le chemin des caisses.
- Correction des journaux et améliorations générales.
- hyperledger#1150 : regroupez 1000 blocs dans chaque fichier
- Test de résistance de file d'attente.
- Correction du niveau de journalisation.
- Ajouter une spécification d'en-tête à la bibliothèque client.
- Correction d'un échec de panique dans la file d'attente.
- File d'attente de correction.
- Correction de la version de dockerfile.
- Correction du client HTTPS.
- Accélération ci.
- 1. Suppression de toutes les dépendances ursa, à l'exception de iroha_crypto.
- Correction d'un débordement lors de la soustraction de durées.
- Rendre les champs publics dans le client.
- Poussez Iroha2 vers Dockerhub tous les soirs.
- Correction des codes d'état http.
- Remplacez iroha_error par thiserror, eyre et color-eyre.
- Remplacez la file d'attente par une traverse.- Supprimez certaines allocations de peluches inutiles.
- Introduit des métadonnées pour les définitions d'actifs.
- Suppression des arguments de la caisse test_network.
- Supprimez les dépendances inutiles.
- Correction de iroha_client_cli::events.
- hyperledger#1382 : Supprimez l'ancienne implémentation réseau.
- hyperledger#1169 : Ajout de précision pour les actifs.
- Améliorations du démarrage par les pairs :
  - Permet de charger la clé publique Genesis uniquement à partir de l'environnement
  - Les chemins de configuration, Genesis et Trusted_peers peuvent désormais être spécifiés dans les paramètres cli
- hyperledger#1134 : Intégration du Iroha P2P.
- Remplacez le point de terminaison de la requête par POST au lieu de GET.
- Exécutez on_start dans l'acteur de manière synchrone.
- Migrer vers Warp.
- Retravailler le commit avec des corrections de bugs du courtier.
- Annuler le commit "Introduit plusieurs correctifs de courtier" (9c148c33826067585b5868d297dcdd17c0efe246)
- Introduit plusieurs correctifs de courtier :
  - Se désabonner du courtier à l'arrêt de l'acteur
  - Prise en charge de plusieurs abonnements du même type d'acteur (auparavant un TODO)
  - Correction d'un bug où le courtier se mettait toujours comme identifiant d'acteur.
- Bug du courtier (vitrine de test).
- Ajouter des dérivés pour le modèle de données.
- Supprimez le rwlock du torii.
- Vérifications des autorisations de requête OOB.
- hyperledger#1272 : Mise en place du comptage de pairs,
- Vérification récursive des autorisations de requête à l'intérieur des instructions.
- Programmer l'arrêt des acteurs.
- hyperledger#1165 : Implémentation du comptage des pairs.
- Vérifiez les autorisations de requête par compte dans le point de terminaison torii.
- Suppression de l'exposition de l'utilisation du processeur et de la mémoire dans les métriques du système.
 - Remplacez JSON par Norito pour les messages WS.
- Stocker la preuve des modifications de vue.
- hyperledger#1168 : Ajout d'une journalisation si la transaction ne satisfait pas à la condition de vérification de signature.
- Correction de petits problèmes, ajout du code d'écoute de connexion.
- Présenter le générateur de topologie de réseau.
- Implémenter un réseau P2P pour Iroha.
- Ajoute une métrique de taille de bloc.
- Le trait PermissionValidator est renommé IsAllowed. et autres changements de nom correspondants
- Corrections du socket Web des spécifications de l'API.
- Supprime les dépendances inutiles de l'image Docker.
- Fmt utilise Crate import_granularity.
- Présente le validateur d'autorisation générique.
- Migrer vers le framework acteur.
- Modifier la conception du courtier et ajouter des fonctionnalités aux acteurs.
- Configure les vérifications de l'état du codecov.
- Utilise une couverture basée sur la source avec grcov.
- Correction du format multiple build-args et redéclaration ARG pour les conteneurs de build intermédiaires.
- Présente le message SubscriptionAccepted.
- Supprimez les actifs de valeur nulle des comptes après l'opération.
- Correction du format des arguments de construction du Docker.
- Correction d'un message d'erreur si le bloc enfant n'est pas trouvé.
- Ajout d'OpenSSL du fournisseur à construire, corrige la dépendance pkg-config.
- Correction du nom du référentiel pour dockerhub et des différences de couverture.
- Ajout d'un texte d'erreur clair et d'un nom de fichier si TrustedPeers ne pouvait pas être chargé.
- Modification des entités de texte en liens dans les documents.
- Correction d'un mauvais secret de nom d'utilisateur dans la publication Docker.
- Correction d'une petite faute de frappe dans le livre blanc.
- Permet l'utilisation de mod.rs pour une meilleure structure de fichiers.
- Déplacez main.rs dans une caisse séparée et accordez des autorisations pour la blockchain publique.
- Ajoutez des requêtes dans le client cli.
- Migrer de clap vers structopts pour cli.
- Limiter la télémétrie au test de réseau instable.
- Déplacer les traits vers le module smartcontracts.
-Sed -i "s/world_state_view/wsv/g"
- Déplacez les contrats intelligents dans un module séparé.
- Correction d'un bug de longueur du contenu réseau Iroha.
- Ajoute un stockage local de tâche pour l'identifiant de l'acteur. Utile pour la détection des blocages.
- Ajouter un test de détection de blocage à CI
- Ajouter une macro Introspection.
- Supprime l'ambiguïté des noms de flux de travail ainsi que les corrections de formatage
- Changement d'API de requête.
- Migration d'async-std vers tokio.
- Ajouter une analyse de télémétrie à ci.- Ajouter la télémétrie future pour iroha.
- Ajoutez des contrats à terme iroha à chaque fonction asynchrone.
- Ajouter les contrats à terme iroha pour l'observabilité du nombre de sondages.
- Déploiement manuel et configuration ajoutés au README.
- Réparation du journaliste.
- Ajouter une macro de message dérivé.
- Ajouter un cadre d'acteur simple.
- Ajouter la configuration du robot dépendant.
- Ajoutez de jolis rapporteurs de panique et d'erreurs.
- Migration de la version Rust vers 1.52.1 et correctifs correspondants.
- Spawn bloquant les tâches gourmandes en CPU dans des threads séparés.
- Utilisez unique_port et cargo-lints de crates.io.
- Correction du WSV sans verrouillage :
  - supprime les Dashmaps inutiles et verrouille l'API
  - corrige un bug avec un nombre excessif de blocs créés (les transactions rejetées n'étaient pas enregistrées)
  - Affiche la cause complète des erreurs
- Ajouter un abonné télémétrie.
- Requêtes de rôles et d'autorisations.
- Déplacez les blocs de kura vers wsv.
- Modification des structures de données sans verrouillage dans wsv.
- Correction du délai d'expiration du réseau.
- Correction du point de terminaison de santé.
- Présente les rôles.
- Ajoutez des images push docker à partir de la branche dev.
- Ajoutez un peluchage plus agressif et supprimez les paniques du code.
- Refonte du trait Exécuter pour les instructions.
- Supprimez l'ancien code de iroha_config.
- IR-1060 ajoute des contrôles d'octroi pour toutes les autorisations existantes.
- Correction de ulimit et timeout pour iroha_network.
- Correction du test de délai d'attente Ci.
- Supprimez tous les actifs lorsque leur définition a été supprimée.
- Correction de la panique wsv lors de l'ajout d'un actif.
- Supprimez Arc et Rwlock pour les chaînes.
- Réparation du réseau Iroha.
- Les validateurs d'autorisations utilisent des références dans les contrôles.
- Accorder des instructions.
- Ajout de la configuration pour les limites de longueur de chaîne et la validation des identifiants pour NewAccount, Domain et AssetDefinition IR-1036.
- Remplacez le journal par la bibliothèque de traçage.
- Ajoutez une vérification ci pour les documents et refusez la macro dbg.
- Introduit les autorisations accordables.
- Ajouter la caisse iroha_config.
- Ajoutez @alerdenisov en tant que propriétaire du code pour approuver toutes les demandes de fusion entrantes.
- Correction du contrôle de la taille des transactions lors du consensus.
- Annuler la mise à niveau d'async-std.
- Remplacez certaines consts par une puissance de 2 IR-1035.
- Ajouter une requête pour récupérer l'historique des transactions IR-1024.
- Ajouter la validation des autorisations pour le stockage et la restructuration des validateurs d'autorisations.
- Ajoutez NewAccount pour l'enregistrement du compte.
- Ajoutez des types pour la définition des actifs.
- Introduit des limites de métadonnées configurables.
- Introduit les métadonnées de transaction.
- Ajoutez des expressions dans les requêtes.
- Ajoutez lints.toml et corrigez les avertissements.
- Séparez les pairs_de confiance de config.json.
- Correction d'une faute de frappe dans l'URL de la communauté Iroha 2 dans Telegram.
- Correction des avertissements clippy.
- Introduit la prise en charge des métadonnées clé-valeur pour le compte.
- Ajouter la gestion des versions des blocs.
- Correction des répétitions de peluchage.
- Ajoutez des expressions mul,div,mod,raise_to.
- Ajoutez into_v* pour le versioning.
- Remplacez Error::msg par la macro d'erreur.
- Réécrivez iroha_http_server et retravaillez les erreurs torii.
 - Met à niveau la version Norito vers 2.
- Description du versionnement du livre blanc.
- Pagination infaillible. Corrigez les cas où la pagination peut être inutile en raison d'erreurs et ne renvoie pas de collections vides à la place.
- Ajouter dérive (Erreur) pour les énumérations.
- Correction de la version nocturne.
- Ajouter la caisse iroha_error.
- Messages versionnés.
- Introduit les primitives de gestion des versions des conteneurs.
- Fixer des repères.
- Ajouter une pagination.
-Ajouter le décodage d'encodage Varint.
- Remplacez l'horodatage de la requête par u128.
- Ajoutez l'énumération RejectionReason pour les événements de pipeline.
- Supprime les lignes obsolètes des fichiers Genesis. La destination a été supprimée du registre ISI lors des commits précédents.
- Simplifie l'enregistrement et le désenregistrement des ISI.
- Correction du délai d'attente de validation qui n'était pas envoyé dans un réseau à 4 pairs.
- Mélange de topologie lors du changement de vue.- Ajoutez d'autres conteneurs pour la macro dérivée FromVariant.
- Ajout du support MST pour le client cli.
- Ajoutez la macro FromVariant et la base de code de nettoyage.
- Ajoutez i1i1 aux propriétaires de code.
- Transactions de potins.
- Ajoutez de la longueur pour les instructions et les expressions.
- Ajoutez des documents pour bloquer les paramètres de temps et de temps de validation.
- Remplacement des traits Vérifier et Accepter par TryFrom.
- Introduire l'attente uniquement pour le nombre minimum de pairs.
- Ajoutez une action github pour tester l'API avec iroha2-java.
- Ajouter Genesis pour docker-compose-single.yml.
- Condition de vérification de signature par défaut pour le compte.
- Ajout d'un test pour compte avec plusieurs signataires.
- Ajouter la prise en charge de l'API client pour MST.
- Intégrer Docker.
- Ajouter Genesis à Docker Compose.
- Introduire le MST conditionnel.
- Ajouter wait_for_active_peers impl.
- Ajout d'un test pour le client isahc dans iroha_http_server.
- Spécification de l'API client.
- Exécution de requêtes dans les expressions.
- Intègre les expressions et les ISI.
- Expressions pour ISI.
- Correction des benchmarks de configuration de compte.
- Ajouter la configuration du compte pour le client.
- Correction de `submit_blocking`.
- Les événements du pipeline sont envoyés.
- Connexion de socket Web client Iroha.
- Séparation des événements pour les événements de pipeline et de données.
- Test d'intégration des autorisations.
- Ajoutez des contrôles d'autorisation pour la gravure et la menthe.
- Annuler l'enregistrement de l'autorisation ISI.
- Correction des benchmarks pour World struct PR.
- Présenter la structure World.
- Implémenter le composant de chargement du bloc Genesis.
- Introduire le compte Genesis.
- Introduire le générateur de validateur d'autorisations.
- Ajoutez des étiquettes aux PR Iroha2 avec Github Actions.
- Introduire le cadre d'autorisations.
- Limite du nombre d'émissions de file d'attente et correctifs d'initialisation Iroha.
- Enveloppez Hash dans une structure.
- Améliorer le niveau de journalisation :
  - Ajouter des journaux de niveau d'information au consensus.
  - Marquez les journaux de communication réseau comme niveau de trace.
  - Supprimez le vecteur de bloc de WSV car il s'agit d'une duplication et il affiche toute la blockchain dans les journaux.
  - Définir le niveau de journalisation des informations par défaut.
- Supprimez les références WSV mutables pour validation.
- Incrément de version Heim.
- Ajoutez des pairs de confiance par défaut à la configuration.
- Migration de l'API client vers http.
-Ajouter le transfert isi à la CLI.
- Configuration des instructions liées aux pairs Iroha.
- Implémentation des méthodes d'exécution et de test ISI manquantes.
- Analyse des paramètres de requête d'URL
- Ajouter `HttpResponse::ok()`, `HttpResponse::upgrade_required(..)`
- Remplacement des anciens modèles d'instructions et de requêtes par l'approche DSL Iroha.
- Ajouter la prise en charge des signatures BLS.
- Introduire la caisse du serveur http.
- Patché libssl.so.1.0.0 avec lien symbolique.
- Vérifie la signature du compte pour la transaction.
- Refactoriser les étapes de la transaction.
- Améliorations initiales des domaines.
- Implémenter le prototype DSL.
- Améliorer les benchmarks Torii : désactiver la connexion aux benchmarks, ajouter l'affirmation du taux de réussite.
- Améliorer le pipeline de couverture des tests : remplace `tarpaulin` par `grcov`, publication du rapport de couverture des tests sur `codecov.io`.
- Correction du thème RTD.
- Artefacts de livraison pour les sous-projets iroha.
- Présentez `SignedQueryRequest`.
- Correction d'un bug avec la vérification de signature.
- Prise en charge des transactions de restauration.
- Imprimer la paire de clés générée au format json.
- Prise en charge de la paire de clés `Secp256k1`.
- Prise en charge initiale de différents algorithmes de cryptographie.
- Fonctionnalités DEX.
- Remplacez le chemin de configuration codé en dur par le paramètre cli.
- Correction du flux de travail principal du banc.
- Test de connexion événement Docker.
- Guide du moniteur Iroha et CLI.
- Améliorations de la CLI des événements.
- Filtre d'événements.
- Connexions d'événements.
- Correction dans le workflow principal.
- Rtd pour iroha2.
- Hachage racine de l'arbre Merkle pour les transactions en bloc.
- Publication sur Docker Hub.
- Fonctionnalité CLI pour Maintenance Connect.
- Fonctionnalité CLI pour Maintenance Connect.
- Eprintln pour enregistrer la macro.- Améliorations du journal.
- IR-802 Abonnement aux changements d'état des blocs.
- Envoi d'événements de transactions et de blocs.
- Déplace la gestion des messages Sumeragi vers l'implément de message.
- Mécanisme de connexion général.
- Extrayez les entités de domaine Iroha pour le client non standard.
- Transactions TTL.
- Transactions maximales par configuration de bloc.
- Stocker les hachages de blocs invalidés.
- Synchronisez les blocs par lots.
- Configuration de la fonctionnalité de connexion.
- Connectez-vous à la fonctionnalité Iroha.
- Corrections de validation de bloc.
- Synchronisation des blocs : diagrammes.
- Connectez-vous à la fonctionnalité Iroha.
- Bridge : supprimer des clients.
- Synchronisation des blocs.
- AjouterPeer ISI.
- Renommer les commandes en instructions.
- Point final de métriques simples.
- Bridge : obtenez des ponts enregistrés et des actifs externes.
- Docker compose le test en pipeline.
- Pas assez de votes pour le test Sumeragi.
- Chaînage de blocs.
- Bridge : gestion manuelle des transferts externes.
- Point final de maintenance simple.
- Migration vers serde-json.
- Déminer ISI.
- Ajoutez des clients de pont, l'autorisation AddSignatory ISI et CanAddSignatory.
- Sumeragi : pairs de l'ensemble b correctifs TODO liés.
- Valide le bloc avant de se connecter à Sumeragi.
- Relier les actifs externes.
- Validation des signatures dans les messages Sumeragi.
- Magasin d'actifs binaires.
- Remplacez l'alias PublicKey par type.
- Préparer les caisses pour la publication.
- Logique de votes minimum dans NetworkTopology.
- Refactorisation de la validation TransactionReceipt.
- Changement de déclencheur OnWorldStateViewChange : IrohaQuery au lieu d'Instruction.
- Construction séparée de l'initialisation dans NetworkTopology.
- Ajout d'instructions spéciales Iroha liées aux événements Iroha.
- Gestion du délai d'attente de création de bloc.
- Glossaire et comment ajouter des documents du module Iroha.
- Remplacez le modèle de pont codé en dur par le modèle d'origine Iroha.
- Présenter la structure NetworkTopology.
- Ajouter une entité d'autorisation avec transformation à partir des instructions.
- Sumeragi Messages dans le module de messages.
- Fonctionnalité Genesis Block pour Kura.
- Ajoutez des fichiers README pour les caisses Iroha.
-Bridge et RegisterBridge ISI.
- Le travail initial avec Iroha modifie les auditeurs.
- Injection de contrôles d'autorisation dans OOB ISI.
- Correction de plusieurs pairs Docker.
- Exemple de docker peer to peer.
- Traitement des reçus de transaction.
-Autorisations Iroha.
- Module pour Dex et caisses pour Bridges.
- Correction du test d'intégration avec création d'actifs avec plusieurs pairs.
- Réimplémentation du modèle Asset dans EC-S-.
- Gestion du délai d'attente de validation.
- En-tête de bloc.
- Méthodes liées à ISI pour les entités de domaine.
- Énumération du mode Kura et configuration des pairs de confiance.
- Règle de peluchage de la documentation.
- Ajouter CommitBlock.
- Découplage kura de `sumeragi`.
- Vérifiez que les transactions ne sont pas vides avant la création du bloc.
- Réimplémenter les instructions spéciales Iroha.
- Benchmarks de transactions et transitions de blocs.
- Cycle de vie et états des transactions retravaillés.
- Bloque le cycle de vie et les états.
- Correction d'un bug de validation, cycle de boucle `sumeragi` synchronisé avec le paramètre de configuration block_build_time_ms.
- Encapsulation de l'algorithme Sumeragi dans le module `sumeragi`.
- Module moqueur pour la caisse réseau Iroha implémentée via des canaux.
- Migration vers l'API async-std.
- Fonctionnalité de simulation de réseau.
- Nettoyage du code associé asynchrone.
- Optimisations des performances dans la boucle de traitement des transactions.
- La génération des bi-clés a été extraite du démarrage Iroha.
- Packaging Docker de l'exécutable Iroha.- Présenter le scénario de base Sumeragi.
-Client CLI Iroha.
- Chute d'Iroha après l'exécution du groupe de banc.
- Intégrer `sumeragi`.
- Modifiez l'implémentation `sort_peers` en Rand Shuffle avec le hachage de bloc précédent.
- Supprimez le wrapper de message dans le module homologue.
- Encapsulez les informations relatives au réseau dans `torii::uri` et `iroha_network`.
- Ajout d'une instruction Peer implémentée à la place de la gestion du code en dur.
- Communication entre pairs via une liste de pairs de confiance.
- Encapsulation du traitement des requêtes réseau dans Torii.
- Encapsulation de la logique cryptographique dans le module cryptographique.
- Signe de bloc avec horodatage et hachage du bloc précédent comme charge utile.
- Fonctions crypto placées au-dessus du module et fonctionnant avec Ursa Signer encapsulées dans Signature.
- Sumeragi initiale.
- Validation des instructions de transaction sur le clone de la vue de l'état du monde avant la validation dans le magasin.
- Vérifier les signatures lors de l'acceptation de la transaction.
- Correction d'un bug dans la demande de désérialisation.
- Implémentation de la signature Iroha.
- L'entité Blockchain a été supprimée pour nettoyer la base de code.
- Modifications de l'API Transactions : meilleure création et meilleure gestion des requêtes.
- Correction du bug qui créait des blocs avec un vecteur de transaction vide
- Transférer les transactions en attente.
 - Correction d'un bug avec un octet manquant dans le paquet TCP codé u128 Norito.
- Macros d'attributs pour le traçage des méthodes.
-Module P2p.
- Utilisation de iroha_network dans torii et client.
- Ajouter de nouvelles informations ISI.
- Alias ​​de type spécifique pour l'état du réseau.
- Box<dyn Error> remplacé par String.
- Écoute du réseau avec état.
- Logique de validation initiale des transactions.
- Caisse Iroha_network.
- Dériver une macro pour les traits Io, IntoContract et IntoQuery.
- Implémentation de requêtes pour le client Iroha.
- Transformation des Commandes en contrats ISI.
- Ajouter la conception proposée pour le multisig conditionnel.
- Migration vers les espaces de travail Cargo.
- Migration des modules.
- Configuration externe via variables d'environnement.
- Gestion des requêtes Get et Put pour Torii.
- Correction de Github ci.
- Cargo-make nettoie les blocs après le test.
- Introduire le module `test_helper_fns` avec une fonction pour nettoyer le répertoire avec des blocs.
- Implémenter la validation via l'arbre Merkle.
- Supprimer le dérivé inutilisé.
- Propager async/wait et corriger `wsv::put` non attendu.
- Utilisez la jointure depuis la caisse `futures`.
- Implémenter l'exécution d'un magasin parallèle : l'écriture sur le disque et la mise à jour de WSV se déroulent en parallèle.
- Utilisez des références au lieu de la propriété pour la (dé)sérialisation.
- Éjection de code des fichiers.
- Utilisez ursa::blake2.
- Règle sur les mod.rs dans le guide de contribution.
- Hachez 32 octets.
- Hachage Blake2.
- Le disque accepte les références à bloquer.
- Refactoring du module de commandes et de l'Arbre Merkle Initial.
- Structure des modules refactorisée.
- Mise en forme correcte.
- Ajoutez des commentaires de document à read_all.
- Implémentez `read_all`, réorganisez les tests de stockage et transformez les tests avec des fonctions asynchrones en tests asynchrones.
- Supprimez la capture mutable inutile.
- Examinez le problème, corrigez Clippy.
- Supprimez le tiret.
- Ajouter une vérification de format.
- Ajouter un jeton.
- Créez rust.yml pour les actions github.
- Présenter le prototype de stockage sur disque.
- Transférer les tests et fonctionnalités des actifs.
- Ajouter un initialiseur par défaut aux structures.
- Changer le nom de la structure MSTCache.
- Ajouter un emprunt oublié.
- Aperçu initial du code iroha2.
- API Kura initiale.
- Ajoutez quelques fichiers de base et publiez également la première version du livre blanc décrivant la vision d'iroha v2.
- Branche de base d'iroha v2.

## [1.5.0] - 08/04/2022

### Modifications CI/CD
- Supprimez Jenkinsfile et JenkinsCI.

### Ajouté- Ajout de l'implémentation du stockage RocksDB pour Burrow.
- Introduire l'optimisation du trafic avec Bloom-filter
- Mettre à jour le réseau du module `MST` pour le localiser dans le module `OS` dans `batches_cache`.
- Proposer une optimisation du trafic.

###Documentations

- Correction de la construction. Ajoutez les différences de base de données, les pratiques de migration, le point de terminaison du contrôle de santé, les informations sur l'outil iroha-swarm.

### Autre

- Correction des exigences pour la construction de la documentation.
- Couper la documentation de version pour mettre en évidence l'élément de suivi critique restant.
- Correction de « vérifier si l'image Docker existe » /build all skip_testing.
- /build all skip_testing.
- /build skip_testing; Et plus de documents.
- Ajoutez `.github/_README.md`.
- Supprimez `.packer`.
- Supprimer les modifications sur le paramètre de test.
- Utilisez un nouveau paramètre pour ignorer l'étape de test.
- Ajouter au flux de travail.
- Supprimer la répartition du référentiel.
- Ajouter une répartition du référentiel.
- Ajouter un paramètre pour les testeurs.
- Supprimer le délai d'attente `proposal_delay`.

## [1.4.0] - 31/01/2022

### Ajouté

- Ajouter l'état du nœud de synchronisation
- Ajoute des métriques pour RocksDB
- Ajoutez des interfaces de contrôle de santé via http et des métriques.

### Correctifs

- Correction des familles de colonnes dans Iroha v1.4-rc.2
- Ajout d'un filtre bloom 10 bits dans Iroha v1.4-rc.1

###Documentations

- Ajoutez zip et pkg-config à la liste des dépôts de build.
- Mettre à jour le fichier Lisez-moi : corrigez les liens rompus vers l'état de la construction, le guide de construction, etc.
- Correction de la configuration et des métriques Docker.

### Autre

- Mettre à jour la balise docker GHA.
- Correction des erreurs de compilation Iroha 1 lors de la compilation avec g++11.
- Remplacez `max_rounds_delay` par `proposal_creation_timeout`.
- Mettre à jour l'exemple de fichier de configuration pour supprimer les anciens paramètres de connexion à la base de données.