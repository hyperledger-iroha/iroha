---
lang: fr
direction: ltr
source: docs/source/iroha_2_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4e8824c128b9f2a34262a5c9bc09f6b2cd790a0561aa083fa18a987accd7004
source_last_modified: "2026-01-22T15:59:09.647697+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2.0

Hyperledger Iroha v2 est un grand livre distribué déterministe et byzantin tolérant aux pannes qui met l'accent sur un
architecture modulaire, valeurs par défaut solides et API accessibles. La plate-forme est livrée sous forme d'un ensemble de caisses Rust
qui peuvent être intégrés dans des déploiements sur mesure ou utilisés ensemble pour exploiter un réseau blockchain de production.

---

## 1. Aperçu

Iroha 2 poursuit la philosophie de conception introduite avec Iroha 1 : fournir une collection organisée de
capacités prêtes à l'emploi afin que les opérateurs puissent mettre en place un réseau sans écrire de grandes quantités de données personnalisées
code. La version v2 consolide l'environnement d'exécution, le pipeline de consensus et le modèle de données dans un
espace de travail unique et cohérent.

La gamme v2 est conçue pour les organisations qui souhaitent exploiter leur propre consortium ou autorisation.
blockchains. Chaque déploiement gère son propre réseau de consensus, maintient une gouvernance indépendante et peut adapter
configuration, données de genèse et cadence de mise à niveau sans dépendre de tiers. L'espace de travail partagé
permet à plusieurs réseaux indépendants de s'appuyer sur exactement la même base de code tout en choisissant les fonctionnalités et
des politiques qui correspondent à leurs cas d’utilisation.

Iroha 2 et SORA Nexus (Iroha 3) exécutent la même machine virtuelle Iroha (IVM). Les développeurs peuvent créer Kotodama
contrats une seule fois et déployez-les sur des réseaux auto-hébergés ou sur le grand livre mondial Nexus sans recompiler ni
forger l'environnement d'exécution.
### 1.1 Relation avec l'écosystème Hyperledger

Les composants Iroha sont conçus pour interopérer avec d'autres projets Hyperledger. Consensus, modèle de données et
Les caisses de sérialisation peuvent être réutilisées dans des piles composites ou parallèlement aux déploiements Fabric, Sawtooth et Besu.
Les outils communs, tels que les codecs Norito et les manifestes de gouvernance, permettent de maintenir la cohérence des interfaces à travers le monde.
tout en permettant à Iroha de fournir une implémentation par défaut avisée.

### 1.2 Bibliothèques clientes et SDK

Pour garantir des expériences mobiles et Web de première classe, le projet publie des SDK maintenus :

- `IrohaSwift` pour les clients iOS et macOS, intégrant l'accélération Metal/NEON derrière des replis déterministes.
- `iroha_js` pour les applications JavaScript et TypeScript, y compris les constructeurs Kaigi et les assistants Norito.
- `iroha_python` pour les intégrations Python, avec prise en charge HTTP, WebSocket et télémétrie.
- `iroha_cli` pour l'administration et les scripts pilotés par terminal.

langages et plateformes.

### 1.3 Principes de conception- **Le déterminisme d'abord :** Chaque nœud exécute les mêmes chemins de code et produit les mêmes résultats avec les mêmes
  entrées. Les chemins SIMD/CUDA/NEON sont fonction des fonctionnalités et reviennent à des implémentations scalaires déterministes.
- **Modules composables :** Mise en réseau, consensus, exécution, télémétrie et stockage, chacun vivant dans un espace dédié
  des caisses afin que les intégrateurs puissent adopter des sous-ensembles sans transporter la pile entière.
- **Configuration explicite :** Les boutons comportementaux apparaissent via `iroha_config` ; les bascules d'environnement sont
  limité aux commodités du développeur.
- **Paramètres sécurisés :** Les codecs canoniques, l'application stricte de l'ABI de pointeur et les manifestes versionnés font
  mises à niveau inter-réseaux prévisibles.

## 2. Architecture de la plateforme

### 2.1 Composition des nœuds

Un nœud Iroha exécute plusieurs services coopérants :

- **Torii (`iroha_torii`)** expose les API HTTP/WebSocket pour les transactions, les requêtes, les événements de streaming et
  télémétrie (points de terminaison `/v2/...`).
- **Core (`iroha_core`)** coordonne la validation, le consensus, l'exécution, la gouvernance et la gestion de l'état.
- **Sumeragi (`iroha_core::sumeragi`)** implémente le pipeline de consensus prêt pour NPoS avec les modifications de vue,
  disponibilité fiable des données de diffusion et certificats de validation. Voir le
  [Guide de consensus Sumeragi](./sumeragi.md) pour plus de détails.
- **Kura (`iroha_core::kura`)** conserve les blocs canoniques, les side-cars de récupération et les métadonnées des témoins sur le disque.
- **World State View (`iroha_core::state`)** stocke l'instantané en mémoire faisant autorité utilisé pour la validation.
  et des requêtes.
- **La machine virtuelle Iroha (`ivm`)** exécute le bytecode Kotodama (`.to`) et applique la stratégie ABI du pointeur.
- **Norito (`crates/norito`)** fournit une sérialisation binaire et JSON déterministe pour chaque type filaire.
- **Télémétrie (`iroha_telemetry`)** exporte les métriques Prometheus, la journalisation structurée et les événements de streaming.
- **P2P (`iroha_p2p`)** gère les potins, la topologie et les connexions sécurisées entre pairs.

### 2.2 Mise en réseau et topologie

Les homologues Iroha maintiennent une topologie ordonnée dérivée de l'état validé. Chaque cycle de consensus sélectionne un leader,
ensemble de validation, queue de proxy et validateurs de l'ensemble B. Les transactions sont bavardes à l'aide de messages codés Norito
avant que le leader ne les regroupe dans une proposition. Une diffusion fiable garantit que les blocages et le support
les preuves parviennent à tous les pairs honnêtes, garantissant la disponibilité des données même en cas de désabonnement du réseau. Afficher les modifications en rotation
leadership lorsque les délais ne sont pas respectés, et les certificats d'engagement garantissent que chaque bloc engagé porte le
ensemble de signatures canoniques utilisé par tous les pairs.

### 2.3 Cryptographie

La caisse `iroha_crypto` permet la gestion des clés, le hachage et la vérification des signatures :- Ed25519 est le schéma de clé du validateur par défaut.
- Les backends facultatifs incluent Secp256k1, TC26 GOST, BLS (pour les attestations globales) et les assistants ML-DSA.
- Les canaux de streaming associent les identités Ed25519 à HPKE basé sur Kyber pour sécuriser les sessions de streaming Norito.
- Toutes les routines de hachage utilisent des implémentations déterministes (SHA-2, SHA-3, Blake2, Poseidon2) avec espace de travail
  audits documentés dans `docs/source/crypto/dependency_audits.md`.

### 2.4 Ponts de streaming et d'applications

- **Le streaming Norito (`iroha_core::streaming`, `norito::streaming`)** fournit des médias déterministes et cryptés
  et canaux de données avec instantanés de session, rotation des clés HPKE et hooks de télémétrie. Conférence Kaigi et
  les transferts de preuves confidentielles utilisent cette voie.
- **Connect Bridge (`connect_norito_bridge`)** expose une surface C ABI qui alimente les SDK de la plateforme
  (Swift, Kotlin/Android) tout en réutilisant les clients Rust sous le capot.
- **Pont ISO 20022 (`iroha_torii::iso20022_bridge`)** convertit les messages de paiement réglementés en Norito
  transactions, permettant l’interopérabilité avec les flux de travail financiers sans contourner le consensus ou la validation.
- Tous les ponts préservent les charges utiles déterministes Norito afin que les systèmes en aval puissent vérifier les transitions d'état.

## 3. Modèle de données

La caisse `iroha_data_model` définit tous les objets, instructions, requêtes et événements du grand livre. Points forts :

- **Les domaines, comptes et actifs** utilisent les identifiants de compte canoniques I105 (de préférence) ; `name@domain` reste un routage
  alias lorsqu'il est explicitement fourni. Les métadonnées sont déterministes (carte `Metadata`). Les actifs numériques prennent en charge la virgule fixe
  opérations ; Les NFT contiennent des métadonnées structurées arbitraires.
- **Les rôles et autorisations** utilisent des jetons énumérés Norito qui correspondent directement aux contrôles de l'exécuteur.
- Les **déclencheurs** (basés sur le temps, basés sur des blocs ou basés sur des prédicats) émettent des transactions déterministes via la chaîne.
  exécuteur testamentaire.
- Flux **Événements** via Torii et mise en miroir des transitions d'état validées, y compris les flux confidentiels et
  actions de gouvernance.
- **Les transactions, blocs et manifestes** sont codés en Norito (`SignedTransaction`, `SignedBlockWire`) avec
  en-têtes de version explicites, garantissant un décodage extensible vers l'avant.
- La **personnalisation** s'effectue via le modèle de données de l'exécuteur : les opérateurs peuvent enregistrer des instructions personnalisées,
  autorisations et paramètres tout en préservant le déterminisme.
- **Les référentiels (`RepoInstruction`)** permettent de regrouper des plans de mise à niveau déterministes (exécuteurs, manifestes et
  actifs) afin que les déploiements en plusieurs étapes puissent être gérés en chaîne avec l'approbation de la gouvernance.
- Les **artefacts de consensus**, tels que les certificats de validation et les listes de témoins, résident dans le modèle de données et
  aller-retour à travers des tests en or pour garantir la compatibilité entre `iroha_core`, Torii et les SDK.
- **Les registres et événements confidentiels** capturent les descripteurs d'actifs protégés, les clés de vérification, les engagements,
  annuleurs et charges utiles d'événements (`ConfidentialEvent::{Shielded,Transferred,Unshielded}`) afin que les flux soient confidentiels
  restent auditables sans fuite de données en clair.

## 4. Cycle de vie des transactions1. **Admission :** Torii décode la charge utile Norito, vérifie les signatures, la durée de vie et les limites de taille, puis met en file d'attente le
   transaction localement.
2. **Gossip :** La transaction se propage à travers la topologie ; les pairs déduisent par hachage et répètent l'admission
   chèques.
3. **Sélection :** Le leader actuel extrait les transactions de l'ensemble en attente et effectue une validation sans état.
4. **Simulation avec état :** Les transactions candidates s'exécutent dans un `StateBlock` transitoire, en appelant IVM ou
   instructions intégrées. Les conflits ou les violations de règles sont supprimés de manière déterministe.
5. **Matérialisation des déclencheurs :** Les déclencheurs programmés dus dans le tour sont convertis en transactions internes
   et validé en utilisant le même pipeline.
6. **Scellement de la proposition :** Lorsque les limites de bloc sont atteintes ou que les délais d'attente expirent, le leader émet un message codé Norito.
   Message `BlockCreated`.
7. **Validation :** Les pairs de l'ensemble de validation réexécutent les vérifications sans état/avec état. Signe des pairs qui réussissent
   `BlockSigned` et transmettez-les à l'ensemble de collecteurs déterministes.
8. **Commit :** Un collecteur assemble un certificat de validation une fois qu'il a collecté l'ensemble de signatures canoniques,
   diffuse `BlockCommitted`, et finalise le bloc localement.
9. **Application :** Tous les pairs enregistrent le bloc dans Kura, appliquent les mises à jour d'état, émettent des télémétries/événements, purgent
   transactions validées à partir du pool de mémoire et rotation des rôles de topologie.

Les chemins de récupération utilisent la diffusion déterministe pour retransmettre les blocs manquants et visualiser les changements en faisant tourner la direction.
lorsque les délais sont expirés. Les side-cars et la télémétrie fournissent des informations de diagnostic sans muter les résultats consensuels.

## 5. Contrats intelligents et exécution

Les contrats intelligents s'exécutent sur la machine virtuelle Iroha (IVM) :

- **Kotodama** compile les sources `.ko` de haut niveau en bytecode déterministe `.to`.
- **L'application du pointeur ABI** garantit que les contrats interagissent avec la mémoire hôte via des types de pointeurs validés.
  Les surfaces Syscall sont décrites dans `ivm/docs/syscalls.md` ; la liste ABI est hachée et versionnée.
- **Les appels système et les hôtes** couvrent l'accès à l'état du grand livre, la planification des déclencheurs, les primitives confidentielles et les médias Kaigi.
  flux et caractère aléatoire déterministe.
- **L'exécuteur intégré** continue de prendre en charge les instructions spéciales (ISI) Iroha pour les actifs, les comptes, les autorisations,
  et les opérations de gouvernance. Les exécuteurs personnalisés peuvent étendre le jeu d’instructions tout en respectant les schémas Norito.
- **Les fonctionnalités confidentielles**, y compris les transferts protégés et les registres de vérificateurs, sont exposées via l'exécuteur testamentaire.
  instructions et validées par des hôtes avec engagements Poséidon.

## 6. Stockage et persistance- **Kura block store** écrit chaque bloc finalisé sous la forme d'une charge utile `SignedBlockWire` avec un en-tête Norito, en conservant
  en-têtes canoniques, transactions, certificats de validation et données témoins ensemble.
- **World State View** conserve l'état faisant autorité en mémoire pour des requêtes rapides. Instantanés déterministes et
  Les side-cars de pipeline (`pipeline/sidecars.norito` + `pipeline/sidecars.index`) prennent en charge la récupération et les audits.
- **La hiérarchisation par état** permet un partitionnement chaud/froid pour les déploiements à grande échelle tout en préservant le déterminisme
  validation.
- **Sync et relecture** chargent les blocs validés dans leur état en utilisant les mêmes règles de validation. Déterministe
  la diffusion garantit que les pairs peuvent récupérer les données manquantes des voisins sans compter sur un stockage fiable.

## 7. Gouvernance et économie

- Les paramètres en chaîne (`SetParameter`) contrôlent les minuteries de consensus, les limites de mémoire, les boutons de télémétrie, les tranches de frais,
  et des drapeaux de fonctionnalités. Les manifestes Genesis générés par `kagami` installent la configuration initiale.
- Les instructions **Kaigi** gèrent les sessions collaboratives (créer/rejoindre/quitter/fin) et alimentent le streaming Norito
  télémétrie pour les cas d'utilisation de conférence.
- **Hijiri** fournit une réputation déterministe des pairs et des comptes, intégrant le consensus et l'admission
  politiques et multiplicateurs de frais (mathématiques à virgule fixe Q16). Manifestes de preuves, points de contrôle et réputation
  les registres sont engagés en chaîne et les profils d'observateurs régissent la provenance des reçus.
- **Le mode NPoS** (lorsqu'il est activé) utilise des fenêtres électorales soutenues par VRF et des comités pondérés en fonction des enjeux tout en préservant
  paramètres par défaut de configuration déterministe.
- Les **registres confidentiels** régissent les clés de vérification sans connaissance, les cycles de vie des preuves et les engagements pour
  flux protégés.

## 8. Expérience client et outils

- **L'API Torii** offre des interfaces REST et WebSocket pour les transactions, les requêtes, les flux d'événements, la télémétrie et
  critères de gouvernance. Les projections JSON sont dérivées des schémas Norito.
- **Outils CLI** (`iroha_cli`, `iroha_monitor`) couvrent l'administration, les tableaux de bord des pairs en direct et le pipeline
  inspection.
- **Les outils Genesis** (`kagami`) génèrent des manifestes codés en Norito, du matériel de clé de validation et une configuration
  modèles.
- Les **SDK** (Swift, JS/TS, Python) fournissent un accès idiomatique aux instructions, requêtes, déclencheurs et télémétrie.
- **Les scripts et les hooks CI** dans `scripts/` automatisent la validation du tableau de bord, la régénération des codecs et la fumée
  essais.

## 9. Performance, résilience et feuille de route- Le pipeline actuel vise **20 000 tps** avec des temps de bloc de **2 à 3 secondes** dans un réseau favorable.
  conditions, appuyées par une vérification des signatures par lots et une planification déterministe.
- **Télémétrie** expose les métriques Prometheus pour les minuteries de consensus, l'occupation du pool de mémoire, l'état de propagation des blocs,
  Utilisation de Kaigi et mises à jour de la réputation Hijiri.
- **Les fonctionnalités de résilience** incluent la disponibilité déterministe des données, les side-cars de récupération, la rotation de la topologie et
  seuils d'affichage/modification configurables.
- Les prochaines étapes de la feuille de route (voir `roadmap.md`) poursuivent les travaux sur les espaces de données Nexus, confidentialité améliorée
  des outils et une accélération matérielle plus large tout en préservant les sorties déterministes.

## 10. Opérations et déploiement

- **Artefacts :** Les workflows Dockerfiles, Nix flake et `cargo` prennent en charge les versions reproductibles. `kagami` émet
  manifestes Genesis, clés de validation et exemples de configurations pour les déploiements autorisés et NPoS.
- **Réseaux auto-hébergés :** Les opérateurs gèrent leurs propres ensembles de pairs, règles d'admission et cadence de mise à niveau. Le
  L'espace de travail prend en charge de nombreux réseaux Iroha 2 indépendants coexistant sans coordination, partageant uniquement le
  code en amont.
- **Cycle de vie de la configuration :** `iroha_config` résout les couches utilisateur → réelles → valeurs par défaut, garantissant que chaque bouton est
  explicite et contrôlé en version. Les modifications d’exécution transitent par les instructions `SetParameter`.
- **Observabilité :** `iroha_telemetry` exporte les métriques Prometheus, les journaux structurés et les données du tableau de bord vérifiées
  par les scripts CI (`ci/check_swift_dashboards.sh`, `scripts/render_swift_dashboards.sh`,
  `scripts/check_swift_dashboard_data.py`). Les événements en streaming, consensus et Hijiri sont disponibles sur
  WebSocket et `scripts/sumeragi_backpressure_log_scraper.py` corrèle la contre-pression du stimulateur cardiaque avec
  télémétrie pour le dépannage.
- **Tests :** `cargo test --workspace`, tests d'intégration (`integration_tests/`), suites de SDK de langage et
  Les luminaires dorés Norito protègent le déterminisme. L'ABI du pointeur, les listes d'appels système et les manifestes de gouvernance ont
  tests d'or dédiés.
- **Récupération :** Les side-cars Kura, la relecture déterministe et la synchronisation de diffusion permettent aux nœuds de récupérer l'état du disque
  ou des pairs. Les points de contrôle Hijiri et les manifestes de gouvernance fournissent des instantanés vérifiables pour la conformité.

# Glossaire

Pour la terminologie référencée dans ce document, consultez le glossaire à l'échelle du projet à l'adresse
.