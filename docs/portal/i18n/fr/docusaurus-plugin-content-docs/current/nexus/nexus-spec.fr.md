---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : Spécification technique de Sora Nexus
description : Miroir complet de `docs/source/nexus.md`, couvrant l'architecture et les contraintes de conception pour le ledger Iroha 3 (Sora Nexus).
---

:::note Source canonique
Cette page reprend `docs/source/nexus.md`. Gardez les deux copies alignées jusqu'à ce que le backlog de traduction arrive sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger : Spécification technique de conception

Ce document propose l'architecture du Sora Nexus Ledger pour Iroha 3, faisant évoluer Iroha 2 vers un ledger global unique et logiquement unifié organisé autour des Data Spaces (DS). Les Data Spaces fournissent des domaines de confidentialité forts (« espaces de données privés ») et une participation ouverte (« espaces de données publics »). Le design préserve la composabilité à travers le grand livre global tout en garantissant une isolation stricte et la confidentialité des données privées-DS, et l'introduction de l'échelle de disponibilité des données via l'effacement code sur Kura (block storage) et WSV (World State View).Le meme dépôt construit Iroha 2 (réseaux auto-heberges) et Iroha 3 (SORA Nexus). L'exécution est assurée par l'Iroha Virtual Machine (IVM) partagé et la chaîne d'outils Kotodama, afin que les contrats et artefacts de bytecode portables entre les déploiements auto-hébergements et le grand livre global Nexus.

Objectifs
- Un grand livre logique global composé de nombreux validateurs coopérants et Data Spaces.
- Des Data Spaces privés pour une opération autorisée (p. ex., CBDC), avec des données qui ne quittent jamais le DS privé.
- Des Data Spaces publics avec participation ouverte, accès sans autorisation type Ethereum.
- Des contrats intelligents composables entre Data Spaces, soumis à des autorisations explicites pour l'accès aux actifs private-DS.
- Isolation de performance afin que l'activité publique ne dégrade pas les transactions internes private-DS.
- Disponibilité des données à grande échelle : Kura et WSV avec effacement code pour supporter des données effectivement illimitées tout en gardant les données privées-DS privées.Non-objectifs (phase initiale)
- Définir l'économie de token ou les incitations des validateurs ; les politiques de planification et de jalonnement sont enfichables.
- Introduire une nouvelle version ABI; les cibles ABI v1 avec changements des extensions explicites de syscalls et pointeur-ABI selon la politique IVM.Terminologie
- Nexus Ledger : Le grand livre logique global forme en composant des blocs Data Space (DS) en une histoire ordonnée unique et un engagement d'état.
- Data Space (DS) : Domaine d'exécution et de stockage supporté avec ses propres validateurs, gouvernance, classe de confidentialité, politique DA, quotas et politique de frais. Deux classes existent : DS publique et DS privée.
- Espace de données privé : Validateurs permissionnes et contrôle d'accès ; les données de transaction et d'état ne quittent jamais le DS. Seuls des engagements/métadonnées sont globalement ancrés.
- Espace de données publiques : participation sans autorisation ; les données complètes et l'état sont publics.
- Data Space Manifest (DS Manifest) : Manifest code en Norito qui déclare les paramètres DS (validateurs/cles QC, classe de confidentialité, politique ISI, paramètres DA, rétention, quotas, politique ZK, frais). Le hash du manifeste est ancre sur la chaine nexus. Sauf dérogation, les certificats de quorum DS utilisent ML-DSA-87 (classe Dilithium5) comme schéma de signature post-quantique par défaut.
- Space Directory : Contrat de directory global on-chain qui trace les manifestes DS, versions et événements de gouvernance/rotation pour la résolvabilité et les audits.
- DSID : Identifiant globalement unique pour un Data Space. Utiliser pour le namepacing de tous les objets et références.- Ancre : Engagement cryptographique d'un bloc/header DS inclus dans la chaine nexus pour lier l'historique DS au ledger global.
- Kura : Stockage de blocs Iroha. Etendu ici avec stockage de blobs a effacement code et engagements.
- WSV : Iroha Vue de l'état mondial. Etendu ici avec des segments d'état versionnes, avec snapshots, et codes par effacement.
- IVM : Iroha Machine Virtuelle pour l'exécution de contrats intelligents (bytecode Kotodama `.to`).
  - AIR : Représentation Algébrique Intermédiaire. Vue algébrique du calcul pour des preuves type STARK, décrivant l'exécution comme des traces basées sur des champs avec contraintes de transition et de frontière.Modèle de Data Spaces
- Identite : `DataSpaceId (DSID)` identifie un DS et un espace de noms tout. Les DS peuvent être des instances à deux granularités :
  - Domain-DS : `ds::domain::<domain_name>` - exécution et état des scopes a un domaine.
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - exécution et état des portées à une définition d'actif unique.
  Les deux formes coexistent ; les transactions peuvent toucher plusieurs DSID de manière atomique.
- Cycle de vie du manifeste : création DS, mises à jour (rotation de clés, changements de politique) et retrait enregistrés dans Space Directory. Chaque artefact DS par slot référence le hash du manifeste le plus récent.
- Classes : Public DS (participation ouverte, DA publique) et Private DS (permissionne, DA confidentielle). Des politiques hybrides sont possibles via des drapeaux de manifeste.
- Politiques par DS : autorisations ISI, paramètres DA `(k,m)`, chiffrement, rétention, quotas (part min/max de tx par bloc), politique de preuve ZK/optimiste, frais.
- Gouvernance : la gouvernance des membres DS et la rotation des validateurs sont définies par la section gouvernance du manifeste (propositions on-chain, multisig, ou gouvernance externe crée par transactions nexus et attestations).Manifestes de capacités et UAID
- Comptes universels : chaque participant recit un UAID déterministe (`UniversalAccountId` dans `crates/iroha_data_model/src/nexus/manifest.rs`) qui couvre tous les dataspaces. Les manifestes de capacités (`AssetPermissionManifest`) client un UAID à un espace de données spécifique, des époques d'activation/expiration et une liste ordonnée de règles permit/deny `ManifestEntry` qui bornent `dataspace`, `program_id`, `method`, `asset` et des rôles AMX optionnels. Les règles refusent toujours de gagner ; l'évaluateur emet `ManifestVerdict::Denied` avec une raison d'audit ou une subvention `Allowed` avec la métadonnée d'allocation correspondante.
- Allocations : chaque entrée permet le transport des seaux déterministes `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) plus un `max_amount` optionnel. Les hôtes et le SDK consomment le meme payload Norito, donc l'application reste identique entre le matériel et le SDK.
- Télémétrie d'audit : Space Directory diffuse `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) chaque fois qu'un changement d'état manifeste. La nouvelle surface `SpaceDirectoryEventFilter` permet aux abonnes Torii/data-event de surveiller les mises à jour du manifeste UAID, les révocations et les décisions deny-wins sans plomberie sur mesure.Pour des preuves exploitantes de bout en bout, des notes de migration SDK et des checklists de publication de manifest, miroitez cette section avec le Universal Account Guide (`docs/source/universal_accounts_guide.md`). Gardez les deux documents alignés lorsque la politique ou les outils UAID changent.

Architecture de haut niveau
1) Couche de composition globale (Chaîne Nexus)
- Maintient un ordre canonique unique de blocs Nexus de 1 seconde qui finalise des transactions atomiques couvrant un ou plusieurs Data Spaces (DS). Chaque transaction commit met un jour le world state global unifié (vecteur de racines par DS).
- Contient des métadonnées minimales plus des preuves/QC agrégées pour assurer la composabilité, la finalité et la détection de fraude (DSIDs touches, racines d'état par DS avant/après, engagements DA, preuves de validite par DS, et le certificat de quorum DS avec ML-DSA-87). Aucune donnée privée n'est incluse.
- Consensus : comité BFT global pipeline de taille 22 (3f+1 avec f=7), sélection parmi un pool de jusqu'à ~200k validateurs potentiels via un mécanisme VRF/enjeu par époques. Le comité nexus séquence les transactions et finalise le bloc en 1s.2) Espace de données Couche (public/privé)
- Execute des fragments par DS des transactions globales, mets a jour le WSV local du DS et produit des artefacts de validite par bloc (preuves par DS agregees et engagements DA) qui remontent dans le bloc Nexus de 1 seconde.
- Les Private DS chiffrent les données au repos et en transit entre validateurs autorisés; seuls les engagements et preuves de validite PQnt le DS.
- Les Public DS exportent les corps complets de données (via DA) et les preuves de validité PQ.3) Transactions atomiques cross-Data-Space (AMX)
- Modèle : chaque transaction utilisateur peut toucher plusieurs DS (p. ex., domaine DS et un ou plusieurs actifs DS). Elle est commise atomiquement dans un bloc Nexus unique ou elle avorte ; aucun effet partiel.
- Prepare-Commit dans 1s : pour chaque transaction candidate, les DS touches s'exécutent en parallèle contre le meme snapshot (roots DS en début de slot) et produisent des preuves de validité PQ par DS (FASTPQ-ISI) et des engagements DA. Le comité nexus commet la transaction seulement si toutes les preuves DS exigences vérifiables et si les certificats DA arrivent a temps (objectif <=300 ms) ; sinon la transaction est re-planifiée pour le slot suivant.
- Consistance : les ensembles lecture/écriture sont déclarés ; la détection de conflits a lieu au commit contre les racines de début de slot. L'exécution optimisée sans locks par DS évite les décrochages globaux; l'atomicité est imposée par la règle de commit nexus (tout ou rien entre DS).
- Confidentialité : les Private DS exportent uniquement des preuves/engagements lies aux racines DS pré/post. Aucune donnée privée brute ne quitte le DS.4) Disponibilité des données (DA) avec code d'effacement
- Kura stocke les corps de blocs et snapshots WSV comme des blobs un code d'effacement. Les blobs publics sont largement partagés ; les blobs privés sont stockés uniquement chez les validateurs private-DS, avec des chunks chiffres.
- Les engagements DA sont enregistrés à la fois dans les artefacts DS et dans les blocs Nexus, permettant des garanties d'échantillonnage et de récupération sans révéler de contenu privé.

Structure de bloc et de commit
- Artefact de preuve Data Space (par slot de 1s, par DS)
  - Champs : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les Private-DS exportent des artefacts sans corps de données ; les Public DS permettent la récupération via DA.

- Bloc Nexus (cadence 1s)
  - Champs : block_number, parent_hash, slot_time, tx_list (transactions atomiques cross-DS avec DSIDs touches), ds_artifacts[], nexus_qc.
  - Fonction : finaliser toutes les transactions atomiques dont les artefacts DS requis vérifiant ; met a jour le vecteur de racines DS du world state global en une etape.Consensus et programmation
- Consensus Nexus Chain : BFT global pipeline (classe Sumeragi) avec un comité de 22 noeuds (3f+1 avec f=7) visant des blocs de 1s et une finalité 1s. Les membres du comité sont sélectionnés par époques via VRF/enjeu parmi ~200k candidats ; la rotation maintient la décentralisation et la résistance à la censure.
- Consensus Data Space : chaque DS exécute son propre BFT entre ses validateurs pour produire des artefacts par slot (preuves, engagements DA, DS QC). Les comités lane-relay sont dimensionnés à `3f+1` en utilisant le paramètre `fault_tolerance` du dataspace et sont échantillonnés de manière déterministe par époque depuis le pool de validateurs du dataspace en utilisant la graine VRF située à `(dataspace_id, lane_id)`. Les Private DS sont permissionnés; les Public DS permettent la vivacité ouverte sous politiques anti-Sybil. Le Comité Global Nexus reste inchange.
- Planification des transactions : les utilisateurs soumettent des transactions atomiques déclarant les DSIDs touches et les ensembles lecture/écriture. Les DS s'exécutent en parallèle dans le slot; le comité nexus inclut la transaction dans le bloc 1s si tous les artefacts DS vérifient et si les certificats DA sont à l'heure (<=300 ms).- Isolation de performance : chaque DS a des mempools et une exécution indépendante. Les quotas par DS bornent combien de transactions touchant un DS peuvent être commis par bloc pour éviter le blocage de tête de ligne et protéger la latence des Private DS.

Modèle de données et d'espacement de noms
- IDs qualifies par DS : toutes les entités (domaines, comptes, actifs, rôles) sont qualifiées par `dsid`. Exemples : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références globales : une référence globale est un tuple `(dsid, object_id, version_hint)` et peut être placé en chaîne dans la couche nexus ou dans des descripteurs AMX pour un usage cross-DS.
- Sérialisation Norito : tous les messages cross-DS (descripteurs AMX, preuves) utilisent les codecs Norito. Pas de serde en production.Contrats intelligents et extensions IVM
- Contexte d'exécution : ajouter `dsid` au contexte d'exécution IVM. Les contrats Kotodama s'exécutent toujours dans un Data Space spécifique.
- Primitives atomiques cross-DS :
  - `amx_begin()` / `amx_commit()` délimitent une transaction atomique multi-DS dans le hôte IVM.
  - `amx_touch(dsid, key)` déclare une intention read/write pour la détection de conflits contre les racines snapshot du slot.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> résultat (opération permise seulement si la politique l'autorise et si le handle est valide)
- Asset handles et frais :
  - Les opérations d'actifs sont autorisées par les politiques ISI/rôles du DS ; les frais sont payés en token de gas du DS. Des tokens de capacité optionnels et des politiques plus riches (multi-approbateur, rate-limits, geofencing) peuvent être ajoutés plus tard sans changer le modèle atomique.
- Déterminisme : toutes les nouvelles syscalls sont pures et déterministes données les entrées et les ensembles read/write AMX déclare. Pas d'effets caches de temps ou d'environnement.Preuves de validité post-quantiques (ISI généralise)
- FASTPQ-ISI (PQ, sans trust setup) : un argument basé sur le hachage qui généralise le transfert de conception à toutes les familles ISI tout en visant une preuve sous la seconde pour des lots à l'échelle 20k sur du matériel classe GPU.
  - Profil opérationnel :
    - Les nœuds de production construisent le prouveur via `fastpq_prover::Prover::canonical`, qui initialise désormais toujours le backend de production ; le déterministe simulé a été retiré. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent aux opérateurs de figer l'exécution CPU/GPU de manière déterministe tandis que l'observateur hook enregistre les triples demande/résolu/backend pour les audits de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmétisation :
  - KV-Update AIR : traite WSV comme une map key-value type, engagée via Poseidon2-SMT. Chaque ISI s'étend à un petit ensemble de lignes lecture-vérification-écriture sur des clés (comptes, actifs, rôles, domaines, métadonnées, fourniture).
  - Contraintes gatees par opcode : une seule table AIR avec colonnes sélecteurs imposent des règles par ISI (conservation, compteurs monotoniques, permissions, range checks, mises à jour de métadonnées bornees).- Arguments de recherche : tables transparentes engagées par hash pour permissions/rôles, précisions d'actifs et paramètres de politique évitant les contraintes bitwise lourdes.
- Engagements et mises à jour d'état :
  - Preuve SMT agrégée : toutes les clés touchées (pré/post) sont prouvées contre `old_root`/`new_root` en utilisant une frontière compressée avec des dédupes frères et sœurs.
  - Invariants : les invariants globaux (p. ex., supply total par actif) sont imposés via l'égalité de multiensembles entre lignes d'effet et compteurs suivis.
- Système de preuve :
  - Engagements polynomiaux style FRI (DEEP-FRI) avec forte arite (8/16) et Blow-up 8-16 ; hache Poséidon2 ; transcription Fiat-Shamir avec SHA-2/3.
  - Recursion optionnelle : agrégation récursive locale DS pour compresser des micro-batches en une preuve par slot si nécessaire.
- Portee et exemples couverts :
  - Actifs : transférer, créer, graver, enregistrer/désenregistrer les définitions d'actifs, définir la précision (borne), définir les métadonnées.
  - Comptes/Domaines : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (état uniquement ; les contrôles de signatures sont attestés par les validateurs DS, pas prouvés dans l'AIR).
  - Rôles/Permissions (ISI) : accorder/révoquer des rôles et des autorisations ; impose via des tables de recherche et des contrôles de politique monotones.- Contrats/AMX : marqueurs start/commit AMX, capacité mint/revoke si active ; prouve comme transitions d'état et compteurs de politique.
- Vérifie hors AIR pour préserver la latence :
  - Signatures et cryptographie lourde (p. ex., signatures utilisateur ML-DSA) sont vérifiées par les validateurs DS et attestées dans le DS QC ; la preuve de validité couvre seulement la cohérence d’état et la conformité de politique. Cela garde des preuves PQ et rapides.
- Cibles de performance (illustratif, CPU 32 cœurs + un GPU moderne) :
  - 20k ISI mixes avec key-touch petit (<=8 clés/ISI) : ~0,4-0,9 s de preuve, ~150-450 Ko de preuve, ~5-15 ms de vérification.
  - ISI plus lourdes (plus de clés/contraintes riches) : micro-batch (p. ex., 10x2k) + récursion pour garder <1 s par slot.
- Configuration du DS Manifest :
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures vérifiées par DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (par défaut ; les alternatives doivent être déclarées explicitement)
- Solutions de repli :
  - ISI complexes/personnalisés peuvent utiliser un STARK général (`zk.policy = "stark_fri_general"`) avec preuve différente et finale 1 s via attestation QC + slashing sur preuves invalides.
  - Options non PQ (p. ex., Plonk avec KZG) exigeant une configuration fiable et ne sont plus prises en charge dans le build par défaut.Introduction AIR (pour Nexus)
- Trace d'exécution : matrice avec largeur (colonnes de registres) et longueur (étapes). Chaque ligne est une étape logique du traitement ISI; les colonnes contiennent des valeurs pre/post, selecteurs et flags.
- Contraintes :
  - Contraintes de transition : imposent des relations de ligne à ligne (p. ex., post_balance = pre_balance - montant pour une ligne de débit quand `sel_transfer = 1`).
  - Contraintes de frontière : client I/O public (old_root/new_root, compteurs) à la première/dernière ligne.
  - Lookups/permutations : assurer l'appartenance et l'égalité de multiensembles contre des tables engagées (permissions, paramètres d'actifs) sans circuits lourds de bits.
- Engagement et vérification :
  - Le prouver engage les traces via des encodages hash et construit des polynomes de faible degré valides si les contraintes respectent.
  - Le vérificateur vérifie le faible degré via FRI (hash-based, post-quantique) avec quelques ouvertures Merkle; le cout est logarithmique en étapes.
- Exemple (Transfer) : les registres incluent pre_balance, montant, post_balance, nonce et sélecteurs. Les contraintes imposent non-negativite/range, conservation et monotonicite de nonce, tandis qu'une multi-preuve SMT agrégée lie les feuilles pré/post aux racines anciennes/nouvelles.Evolution ABI et appels système (ABI v1)
- Syscalls à ajouter (noms illustratifs) :
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types pointeur-ABI à ajouter :
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises à jour requises :
  - Ajouter un `ivm::syscalls::abi_syscall_list()` (garder l'ordre), portail par politique.
  - Mapper les numéros inconnus à `VMError::UnknownSyscall` dans les hôtes.
  - Mettre à jour les tests : syscall list golden, ABI hash, pointer type ID goldens, Policy tests.
  - Documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modèle de confidentialité
- Contention des données privées : les corps de transaction, diffs d'état et snapshots WSV des privées DS ne quittent jamais le sous-ensemble de validateurs privés.
- Exposition publique : seuls les en-têtes, engagements DA et preuves de validité PQ sont exportés.
- Preuves ZK optionnelles : les Private DS peuvent produire des preuves ZK (p. ex., vendue suffisante, politique satisfaite) permettant des actions cross-DS sans révéler l'état interne.
- Contrôle d'accès : l'autorisation est imposée par les politiques ISI/rôles dans le DS. Les tokens de capacité sont optionnels et peuvent être ajoutés plus tard.Isolation de performance et QoS
- Consensus, mempools et stockage séparés par DS.
- Quotas de scheduling nexus par DS pour supporter le temps d'inclusion des ancres et éviter le blocage de tête de ligne.
- Budgets de ressources de contrats par DS (compute/memory/IO), imposés par l'hôte IVM. La contention publique DS ne peut pas consommer les budgets privés-DS.
- Les appels cross-DS asynchrones évitent les longues attentes synchronisées dans l'exécution private-DS.

Disponibilité des données et conception de stockage
1) Code d'effacement
- Utiliser Reed-Solomon systématique (p. ex., GF(2^16)) pour l'effacement du code au niveau blob des blocs Kura et snapshots WSV : paramètres `(k, m)` avec `n = k + m` shards.
- Paramètres par défaut (propose, public DS) : `k=32, m=16` (n=48), permettant la récupération jusqu'à 16 fragments perdus avec ~1.5x d'expansion. Pour DS privé : `k=16, m=8` (n=24) dans l'ensemble permissionne. Les deux sont configurables par DS Manifest.
- Blobs publics : les fragments sont distribués via de nombreux noeuds DA/validateurs avec contrôles de disponibilité par échantillonnage. Les engagements DA dans les headers permettent la vérification par des clients légers.
- Blobs privés : shards numériques et distribués uniquement parmi les validateurs private-DS (ou custodians designes). La chaîne globale ne porte que des engagements DA (sans emplacement de shard nickel).2) Engagements et échantillonnage
- Pour chaque blob : calculer une racine Merkle sur les fragments et l'inclure dans `*_da_commitment`. Rester PQ en évitant les engagements à courbe elliptique.
- DA Attesters : attesters régionaux échantillonnés par VRF (p. ex., 64 par région) emettent un certificat ML-DSA-87 attestant un échantillonnage réussi des éclats. Objectif de latence d'attestation DA <=300 ms. Le comité Nexus valide les certificats au lieu de tirer les fragments.

3) Intégration Kura
- Les blocs stockent les corps de transaction comme blobs a effacement code avec engagements Merkle.
- Les en-têtes présagent les engagements de blob ; les corps sont récupérables via le réseau DA pour public DS et via des canaux privés pour private DS.

4) Intégration WSV
- Snapshots WSV : périodiquement, checkpoint de l'état DS en snapshots chunkes et codes par effacement avec engagements enregistrés dans les headers. Entre les instantanés, les journaux de modifications sont conservés. Les instantanés publics sont en grande partie des fragments ; les snapshots privés restent dans les validateurs privés.
- Accès porteur de preuves : les contrats peuvent fournir (ou demander) des preuves d'état (Merkle/Verkle) crées par des engagements de snapshot. Les Private DS peuvent fournir des attestations zéro-connaissance au lieu de preuves brutes.5) Rétention et taille
- Pas de pruning pour public DS : conserver tous les corps Kura et snapshots WSV via DA (scalabilite horizontale). Les DS privés peuvent définir une rétention interne, mais les engagements exportés restent immuables. La couche nexus conserve tous les blocs Nexus et les engagements d'artefacts DS.

Réseau et rôles de noeuds
- Validateurs globaux : participent au consensus nexus, valident les blocs Nexus et les artefacts DS, effectuent des contrôles DA pour public DS.
- Validateurs Data Space : exécutent le consensus DS, exécutent les contrats, gerent Kura/WSV local, gerent DA pour leur DS.
- Noeuds DA (optionnel) : stockent/publiquent des blobs publics, facilitent le sampling. Pour private DS, les noeuds DA sont co-localisés avec les validateurs ou des dépositaires de confiance.Améliorations et considérations système
- Découplage séquençage/mempool : adopter un mempool DAG (p. ex., style Narwhal) alimentant un pipeline BFT à la couche nexus pour réduire la latence et améliorer le débit sans changer le modèle logique.
- Quotas DS et fairness : quotas par DS par bloc et caps de poids pour éviter le blocage de tête de ligne et assurer une latence prévisible pour DS privé.
- Attestation DS (PQ) : les certificats de quorum DS utilisent ML-DSA-87 (classe Dilithium5) par défaut. C'est post-quantique et plus important que les signatures EC soient plus acceptables à un QC par slot. Les DS peuvent préciser opter pour ML-DSA-65/44 (plus petits) ou des signatures EC si déclarer dans le DS Manifest ; les Public DS encouragent fortement à garder ML-DSA-87.
- DA attesters : pour public DS, utiliser des attesters régionaux échantillonnés par VRF qui emettent des certificats DA. Le comité nexus valide les certificats au lieu du sampling brut des tessons ; les Private DS gardent les attestations DA internes.
- Récursion et preuves par époque : optionnellement agréger plusieurs micro-batches dans un DS en une preuve récursive par slot/époque pour garder taille de preuve et temps de vérification stables sous charge élevée.- Scaling de voies (si nécessaire) : si un comité global unique devient un goulot, introduire K voies de séquençage parallèles avec une fusion déterministe. Cela préserve un ordre global unique tout en scalant horizontalement.
- Accélération déterministe : fournir des noyaux SIMD/CUDA gates par fonctionnalité pour hachage/FFT avec un repli CPU bit-exact pour préserver le déterminisme cross-hardware.
- Seuils d'activation des voies (proposition) : activer 2-4 voies si (a) la finalite p95 dépasse 1,2 s pendant >3 minutes consécutives, ou (b) l'occupation par bloc dépasse 85% pendant >5 minutes, ou (c) le débit entrant de tx requerrait >1,2x la capacité de bloc à des niveaux soutenus. Les voies bucketisent les transactions de manière déterministe par hash de DSID et se fusionnent dans le bloc nexus.

Frais et économie (valeurs initiales)
- Unite de gas : token de gas par DS avec compteurs de calcul/IO ; les frais sont payés en actif de gaz natif du DS. La conversion entre DS est une préoccupation applicative.
- Priorité d'inclusion : round-robin entre DS avec quotas par DS pour préserver la fairness et les SLOs 1s ; dans un DS, la mise en avant par frais peut partir.
- Futur : une marche globale de taxes ou des politiques minimisant MEV peut etre explore sans changer l'atomicite ni le design de preuves PQ.Workflow cross-Data-Space (exemple)
1) Un utilisateur soumet une transaction AMX touchant un public DS P et un privé DS S : déplacer l'actif X de S vers le bénéficiaire B dont le compte est dans P.
2) Dans le slot, P et S exécutent leur fragment contre le snapshot du slot. S'assure de l'autorisation et de la disponibilité, met a jour son etat interne, et produit une preuve de validite PQ et un engagement DA (aucune donnee privee ne fuite). P préparer la mise à jour d'état correspondant (p. ex., mint/burn/locking dans P selon la politique) et sa preuve.
3) Le comité nexus vérifie les deux preuves DS et les certificats DA ; si les deux vérifient dans le slot, la transaction est commise atomiquement dans le bloc Nexus de 1s, mettant à jour les deux racines DS dans le vecteur world state global.
4) Si une preuve ou un certificat DA est manquant/invalide, la transaction avorte (aucun effet), et le client peut re-soumettre pour le slot suivant. Aucune donnéee privée ne quitte S à aucun moment.- Considérations de sécurité
- Exécution déterministe : les syscalls IVM restent déterministes ; les résultats cross-DS sont dictés par AMX commit et finalite, pas par l'horloge ou le timing reseau.
- Contrôle d'accès : les permissions ISI dans private DS restreignent qui peuvent soumettre des transactions et quelles opérations sont autorisées. Les tokens de capacité encodent des droits fins pour une utilisation cross-DS.
- Confidentialité : chiffrement de bout en bout pour données privées-DS, fragments d'effacement de code stockés uniquement parmi les membres autorisés, preuves ZK optionnelles pour attestations externes.
- Resistance DoS : l'isolation au niveau mempool/consensus/stockage empèche la congestion publique d'impacter la progression des private DS.

Changements des composants Iroha
- iroha_data_model : présenter `DataSpaceId`, IDs qualifie par DS, descripteurs AMX (ensembles lecture/ecriture), types de preuve/engagement DA. Sérialisation Norito uniquement.
- ivm : ajouter des syscalls et des types pointeur-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et preuves DA ; mettre à jour les tests/docs ABI selon la politique v1.