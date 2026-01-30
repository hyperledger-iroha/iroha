---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/nexus-spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d19f23013a149d404e1a3a8b0fcadb804829ff9093a5e3306ae2f06c90fcc507
source_last_modified: "2025-11-14T04:43:20.502934+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reprend `docs/source/nexus.md`. Gardez les deux copies alignees jusqu'a ce que le backlog de traduction arrive sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger: Specification technique de conception

Ce document propose l'architecture du Sora Nexus Ledger pour Iroha 3, faisant evoluer Iroha 2 vers un ledger global unique et logiquement unifie organise autour des Data Spaces (DS). Les Data Spaces fournissent des domaines de confidentialite forts ("private data spaces") et une participation ouverte ("public data spaces"). Le design preserve la composabilite a travers le ledger global tout en garantissant une isolation stricte et la confidentialite des donnees private-DS, et introduit l'echelle de disponibilite des donnees via l'effacement code sur Kura (block storage) et WSV (World State View).

Le meme depot construit Iroha 2 (reseaux auto-heberges) et Iroha 3 (SORA Nexus). L'execution est assuree par l'Iroha Virtual Machine (IVM) partagee et la toolchain Kotodama, afin que les contrats et artefacts de bytecode restent portables entre les deployments auto-heberges et le ledger global Nexus.

Objectifs
- Un ledger logique global compose de nombreux validateurs cooperants et Data Spaces.
- Des Data Spaces prives pour une operation permissionnee (p. ex., CBDC), avec des donnees qui ne quittent jamais le DS prive.
- Des Data Spaces publics avec participation ouverte, acces sans permission type Ethereum.
- Des contrats intelligents composables entre Data Spaces, soumis a des permissions explicites pour l'acces aux actifs private-DS.
- Isolation de performance afin que l'activite publique ne degrade pas les transactions internes private-DS.
- Disponibilite des donnees a grande echelle: Kura et WSV avec effacement code pour supporter des donnees effectivements illimitees tout en gardant les donnees private-DS privees.

Non-objectifs (phase initiale)
- Definir l'economie de token ou les incitations des validateurs; les politiques de scheduling et de staking sont pluggables.
- Introduire une nouvelle version ABI; les changements ciblent ABI v1 avec des extensions explicites de syscalls et pointer-ABI selon la politique IVM.

Terminologie
- Nexus Ledger: Le ledger logique global forme en composant des blocs Data Space (DS) en une histoire ordonnee unique et un engagement d'etat.
- Data Space (DS): Domaine d'execution et de stockage borne avec ses propres validateurs, gouvernance, classe de confidentialite, politique DA, quotas et politique de frais. Deux classes existent: public DS et private DS.
- Private Data Space: Validateurs permissionnes et controle d'acces; les donnees de transaction et d'etat ne quittent jamais le DS. Seuls des engagements/metadonnees sont ancres globalement.
- Public Data Space: Participation sans permission; les donnees completes et l'etat sont publics.
- Data Space Manifest (DS Manifest): Manifest code en Norito qui declare les parametres DS (validateurs/cles QC, classe de confidentialite, politique ISI, parametres DA, retention, quotas, politique ZK, frais). Le hash du manifest est ancre sur la chaine nexus. Sauf override, les certificats de quorum DS utilisent ML-DSA-87 (classe Dilithium5) comme schema de signature post-quantique par defaut.
- Space Directory: Contrat de directory global on-chain qui trace les manifests DS, versions et evenements de gouvernance/rotation pour la resolvabilite et les audits.
- DSID: Identifiant globalement unique pour un Data Space. Utilise pour le namespacing de tous les objets et references.
- Anchor: Engagement cryptographique d'un bloc/header DS inclus dans la chaine nexus pour lier l'historique DS au ledger global.
- Kura: Stockage de blocs Iroha. Etendu ici avec stockage de blobs a effacement code et engagements.
- WSV: Iroha World State View. Etendu ici avec des segments d'etat versionnes, avec snapshots, et codes par effacement.
- IVM: Iroha Virtual Machine pour l'execution de contrats intelligents (bytecode Kotodama `.to`).
  - AIR: Algebraic Intermediate Representation. Vue algebrique du calcul pour des preuves type STARK, decrivant l'execution comme des traces basees sur des champs avec contraintes de transition et de frontiere.

Modele de Data Spaces
- Identite: `DataSpaceId (DSID)` identifie un DS et namespace tout. Les DS peuvent etre instancies a deux granularites:
  - Domain-DS: `ds::domain::<domain_name>` - execution et etat scopes a un domaine.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execution et etat scopes a une definition d'actif unique.
  Les deux formes coexistent; les transactions peuvent toucher plusieurs DSID de maniere atomique.
- Cycle de vie du manifest: creation DS, mises a jour (rotation de cles, changements de politique) et retrait enregistres dans Space Directory. Chaque artefact DS par slot reference le hash du manifest le plus recent.
- Classes: Public DS (participation ouverte, DA publique) et Private DS (permissionne, DA confidentielle). Des politiques hybrides sont possibles via des flags de manifest.
- Politiques par DS: permissions ISI, parametres DA `(k,m)`, chiffrement, retention, quotas (part min/max de tx par bloc), politique de preuve ZK/optimiste, frais.
- Gouvernance: la gouvernance de membres DS et la rotation des validateurs sont definies par la section governance du manifest (propositions on-chain, multisig, ou gouvernance externe ancree par transactions nexus et attestations).

Manifests de capacites et UAID
- Comptes universels: chaque participant recoit un UAID deterministe (`UniversalAccountId` dans `crates/iroha_data_model/src/nexus/manifest.rs`) qui couvre tous les dataspaces. Les manifests de capacites (`AssetPermissionManifest`) lient un UAID a un dataspace specifique, des epochs d'activation/expiration et une liste ordonnee de regles allow/deny `ManifestEntry` qui bornent `dataspace`, `program_id`, `method`, `asset` et des roles AMX optionnels. Les regles deny gagnent toujours; l'evaluateur emet `ManifestVerdict::Denied` avec une raison d'audit ou un grant `Allowed` avec la metadata d'allowance correspondante.
- Allowances: chaque entree allow transporte des buckets deterministes `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) plus un `max_amount` optionnel. Les hosts et SDK consomment le meme payload Norito, donc l'application reste identique entre hardware et SDK.
- Telemetrie d'audit: Space Directory diffuse `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) chaque fois qu'un manifest change d'etat. La nouvelle surface `SpaceDirectoryEventFilter` permet aux abonnes Torii/data-event de monitorer les mises a jour de manifest UAID, les revocations et les decisions deny-wins sans plumbing sur mesure.

Pour des preuves operateur end-to-end, des notes de migration SDK et des checklists de publication de manifest, miroitez cette section avec la Universal Account Guide (`docs/source/universal_accounts_guide.md`). Gardez les deux documents alignes quand la politique ou les outils UAID changent.

Architecture de haut niveau
1) Couche de composition globale (Nexus Chain)
- Maintient un ordre canonique unique de blocs Nexus de 1 seconde qui finalisent des transactions atomiques couvrant un ou plusieurs Data Spaces (DS). Chaque transaction commit met a jour le world state global unifie (vecteur de roots par DS).
- Contient des metadonnees minimales plus des preuves/QC agreges pour assurer la composabilite, la finalite et la detection de fraude (DSIDs touches, roots d'etat par DS avant/apres, engagements DA, preuves de validite par DS, et le certificat de quorum DS avec ML-DSA-87). Aucune donnee privee n'est incluse.
- Consensus: comite BFT global pipeline de taille 22 (3f+1 avec f=7), selectionne parmi un pool de jusqu'a ~200k validateurs potentiels via un mecanisme VRF/stake par epochs. Le comite nexus sequence les transactions et finalise le bloc en 1s.

2) Couche Data Space (Public/Private)
- Execute des fragments par DS des transactions globales, met a jour le WSV local du DS et produit des artefacts de validite par bloc (preuves par DS agregees et engagements DA) qui remontent dans le bloc Nexus de 1 seconde.
- Les Private DS chiffrent les donnees au repos et en transit entre validateurs autorises; seuls les engagements et preuves de validite PQ quittent le DS.
- Les Public DS exportent les corps complets de donnees (via DA) et les preuves de validite PQ.

3) Transactions atomiques cross-Data-Space (AMX)
- Modele: chaque transaction utilisateur peut toucher plusieurs DS (p. ex., domain DS et un ou plusieurs asset DS). Elle est commit atomiquement dans un bloc Nexus unique ou elle avorte; aucun effet partiel.
- Prepare-Commit dans 1s: pour chaque transaction candidate, les DS touches executent en parallele contre le meme snapshot (roots DS en debut de slot) et produisent des preuves de validite PQ par DS (FASTPQ-ISI) et des engagements DA. Le comite nexus commit la transaction seulement si toutes les preuves DS requises verifient et si les certificats DA arrivent a temps (objectif <=300 ms); sinon la transaction est re-planifiee pour le slot suivant.
- Consistance: les ensembles lecture/ecriture sont declares; la detection de conflits a lieu au commit contre les roots de debut de slot. L'execution optimiste sans locks par DS evite les stalls globaux; l'atomicite est imposee par la regle de commit nexus (tout ou rien entre DS).
- Confidentialite: les Private DS exportent uniquement des preuves/engagements lies aux roots DS pre/post. Aucune donnee privee brute ne quitte le DS.

4) Disponibilite des donnees (DA) avec effacement code
- Kura stocke les corps de blocs et snapshots WSV comme des blobs a effacement code. Les blobs publics sont largement sharde; les blobs prives sont stockes uniquement chez les validateurs private-DS, avec des chunks chiffres.
- Les engagements DA sont enregistres a la fois dans les artefacts DS et dans les blocs Nexus, permettant des garanties de sampling et de recuperation sans reveler de contenu prive.

Structure de bloc et de commit
- Artefact de preuve Data Space (par slot de 1s, par DS)
  - Champs: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les Private-DS exportent des artefacts sans corps de donnees; les Public DS permettent la recuperation via DA.

- Bloc Nexus (cadence 1s)
  - Champs: block_number, parent_hash, slot_time, tx_list (transactions atomiques cross-DS avec DSIDs touches), ds_artifacts[], nexus_qc.
  - Fonction: finalise toutes les transactions atomiques dont les artefacts DS requis verifient; met a jour le vecteur de roots DS du world state global en une etape.

Consensus et scheduling
- Consensus Nexus Chain: BFT global pipeline (classe Sumeragi) avec un comite de 22 noeuds (3f+1 avec f=7) visant des blocs de 1s et une finalite 1s. Les membres du comite sont selectionnes par epochs via VRF/stake parmi ~200k candidats; la rotation maintient decentralisation et resistance a la censure.
- Consensus Data Space: chaque DS execute son propre BFT entre ses validateurs pour produire des artefacts par slot (preuves, engagements DA, DS QC). Les comites lane-relay sont dimensionnes a `3f+1` en utilisant le parametre `fault_tolerance` du dataspace et sont echantillonnes de maniere deterministe par epoch depuis le pool de validateurs du dataspace en utilisant la seed VRF liee a `(dataspace_id, lane_id)`. Les Private DS sont permissionnes; les Public DS permettent la liveness ouverte sous politiques anti-Sybil. Le comite global nexus reste inchange.
- Scheduling des transactions: les utilisateurs soumettent des transactions atomiques declarant les DSIDs touches et les ensembles lecture/ecriture. Les DS executent en parallele dans le slot; le comite nexus inclut la transaction dans le bloc 1s si tous les artefacts DS verifient et si les certificats DA sont a l'heure (<=300 ms).
- Isolation de performance: chaque DS a des mempools et une execution independants. Les quotas par DS bornent combien de transactions touchant un DS peuvent etre commit par bloc pour eviter le head-of-line blocking et proteger la latence des Private DS.

Modele de donnees et namespacing
- IDs qualifies par DS: toutes les entites (domaines, comptes, actifs, roles) sont qualifies par `dsid`. Exemple: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- References globales: une reference globale est un tuple `(dsid, object_id, version_hint)` et peut etre placee on-chain dans la couche nexus ou dans des descripteurs AMX pour un usage cross-DS.
- Serialisation Norito: tous les messages cross-DS (descripteurs AMX, preuves) utilisent les codecs Norito. Pas de serde en production.

Contrats intelligents et extensions IVM
- Contexte d'execution: ajouter `dsid` au contexte d'execution IVM. Les contrats Kotodama s'executent toujours dans un Data Space specifique.
- Primitives atomiques cross-DS:
  - `amx_begin()` / `amx_commit()` delimitent une transaction atomique multi-DS dans le host IVM.
  - `amx_touch(dsid, key)` declare une intention read/write pour la detection de conflits contre les roots snapshot du slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operation permise seulement si la politique l'autorise et si le handle est valide)
- Asset handles et frais:
  - Les operations d'actifs sont autorisees par les politiques ISI/roles du DS; les frais sont payes en token de gas du DS. Des capability tokens optionnels et des politiques plus riches (multi-approver, rate-limits, geofencing) peuvent etre ajoutes plus tard sans changer le modele atomique.
- Determinisme: toutes les nouvelles syscalls sont pures et deterministes donnees les entrees et les ensembles read/write AMX declares. Pas d'effets caches de temps ou d'environnement.

Preuves de validite post-quantiques (ISI generalises)
- FASTPQ-ISI (PQ, sans trusted setup): un argument hash-based qui generalise le design transfer a toutes les familles ISI tout en visant une preuve sous la seconde pour des batches a l'echelle 20k sur du hardware classe GPU.
  - Profil operationnel:
    - Les noeuds de production construisent le prover via `fastpq_prover::Prover::canonical`, qui initialise desormais toujours le backend de production; le mock deterministe a ete retire. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent aux operateurs de figer l'execution CPU/GPU de maniere deterministe tandis que l'observer hook enregistre les triples demande/resolu/backend pour les audits de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmetisation:
  - KV-Update AIR: traite WSV comme une map key-value typee, engagee via Poseidon2-SMT. Chaque ISI s'etend a un petit ensemble de lignes read-check-write sur des cles (comptes, actifs, roles, domaines, metadata, supply).
  - Contraintes gatees par opcode: une seule table AIR avec colonnes selecteurs impose des regles par ISI (conservation, compteurs monotonic, permissions, range checks, mises a jour de metadata bornees).
  - Arguments de lookup: tables transparentes engagees par hash pour permissions/roles, precisions d'actifs et parametres de politique evitent les contraintes bitwise lourdes.
- Engagements et mises a jour d'etat:
  - Preuve SMT agregee: toutes les cles touchees (pre/post) sont prouvees contre `old_root`/`new_root` en utilisant un frontier compresse avec siblings dedupes.
  - Invariants: invariants globaux (p. ex., supply total par actif) sont imposes via egalite de multiensembles entre lignes d'effet et compteurs suivis.
- Systeme de preuve:
  - Engagements polynomiaux style FRI (DEEP-FRI) avec forte arite (8/16) et blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir avec SHA-2/3.
  - Recursion optionnelle: aggregation recursive locale DS pour compresser des micro-batches en une preuve par slot si necessaire.
- Portee et exemples couverts:
  - Actifs: transfer, mint, burn, register/unregister asset definitions, set precision (borne), set metadata.
  - Comptes/Domains: create/remove, set key/threshold, add/remove signatories (etat uniquement; les checks de signatures sont attestes par les validateurs DS, pas prouves dans l'AIR).
  - Roles/Permissions (ISI): grant/revoke roles et permissions; imposes via tables de lookup et checks de politique monotonic.
  - Contrats/AMX: marqueurs begin/commit AMX, capability mint/revoke si active; prouves comme transitions d'etat et compteurs de politique.
- Checks hors AIR pour preserval la latence:
  - Signatures et cryptographie lourde (p. ex., signatures utilisateur ML-DSA) sont verifiees par les validateurs DS et attestees dans le DS QC; la preuve de validite couvre seulement la coherence d'etat et la conformite de politique. Cela garde des preuves PQ et rapides.
- Cibles de performance (illustratif, CPU 32 cores + une GPU moderne):
  - 20k ISI mixes avec key-touch petit (<=8 cles/ISI): ~0.4-0.9 s de preuve, ~150-450 KB de preuve, ~5-15 ms de verification.
  - ISI plus lourdes (plus de cles/contraintes riches): micro-batch (p. ex., 10x2k) + recursion pour garder <1 s par slot.
- Configuration du DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures verifiees par DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (par defaut; alternatives doivent etre declarees explicitement)
- Fallbacks:
  - ISI complexes/personnalisees peuvent utiliser un STARK general (`zk.policy = "stark_fri_general"`) avec preuve differee et finalite 1 s via attestation QC + slashing sur preuves invalides.
  - Options non PQ (p. ex., Plonk avec KZG) exigent trusted setup et ne sont plus supportees dans le build par defaut.

Introduction AIR (pour Nexus)
- Trace d'execution: matrice avec largeur (colonnes de registres) et longueur (etapes). Chaque ligne est une etape logique du traitement ISI; les colonnes contiennent valeurs pre/post, selecteurs et flags.
- Contraintes:
  - Contraintes de transition: imposent des relations de ligne a ligne (p. ex., post_balance = pre_balance - amount pour une ligne de debit quand `sel_transfer = 1`).
  - Contraintes de frontiere: lient I/O public (old_root/new_root, compteurs) a la premiere/derniere ligne.
  - Lookups/permutations: assurent l'appartenance et l'egalite de multiensembles contre des tables engagees (permissions, parametres d'actifs) sans circuits lourds de bits.
- Engagement et verification:
  - Le prover engage les traces via des encodages hash et construit des polynomes de faible degre valides si les contraintes tiennent.
  - Le verifier verifie le faible degre via FRI (hash-based, post-quantique) avec quelques ouvertures Merkle; le cout est logarithmique en etapes.
- Exemple (Transfer): les registres incluent pre_balance, amount, post_balance, nonce et selecteurs. Les contraintes imposent non-negativite/range, conservation et monotonicite de nonce, tandis qu'une multi-preuve SMT agregee lie les feuilles pre/post aux roots old/new.

Evolution ABI et syscalls (ABI v1)
- Syscalls a ajouter (noms illustratifs):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Types pointer-ABI a ajouter:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises a jour requises:
  - Ajouter a `ivm::syscalls::abi_syscall_list()` (garder l'ordre), gate par politique.
  - Mapper les numeros inconnus a `VMError::UnknownSyscall` dans les hosts.
  - Mettre a jour les tests: syscall list golden, ABI hash, pointer type ID goldens, policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modele de confidentialite
- Contention des donnees privees: les corps de transaction, diffs d'etat et snapshots WSV des private DS ne quittent jamais le sous-ensemble de validateurs prives.
- Exposition publique: seuls les headers, engagements DA et preuves de validite PQ sont exportes.
- Preuves ZK optionnelles: les Private DS peuvent produire des preuves ZK (p. ex., solde suffisant, politique satisfaite) permettant des actions cross-DS sans reveler l'etat interne.
- Controle d'acces: l'autorisation est imposee par les politiques ISI/roles dans le DS. Les capability tokens sont optionnels et peuvent etre ajoutes plus tard.

Isolation de performance et QoS
- Consensus, mempools et stockage separes par DS.
- Quotas de scheduling nexus par DS pour borner le temps d'inclusion des anchors et eviter le head-of-line blocking.
- Budgets de ressources de contrats par DS (compute/memory/IO), imposes par le host IVM. La contention public DS ne peut pas consommer les budgets private-DS.
- Appels cross-DS asynchrones evitent les longues attentes synchrones dans l'execution private-DS.

Disponibilite des donnees et design de stockage
1) Effacement code
- Utiliser Reed-Solomon systematique (p. ex., GF(2^16)) pour l'effacement code au niveau blob des blocs Kura et snapshots WSV: parametres `(k, m)` avec `n = k + m` shards.
- Parametres par defaut (proposes, public DS): `k=32, m=16` (n=48), permettant la recuperation jusqu'a 16 shards perdus avec ~1.5x d'expansion. Pour private DS: `k=16, m=8` (n=24) dans l'ensemble permissionne. Les deux sont configurables par DS Manifest.
- Blobs publics: shards distribues via de nombreux noeuds DA/validateurs avec checks de disponibilite par sampling. Les engagements DA dans les headers permettent la verification par light clients.
- Blobs prives: shards chiffres et distribues uniquement parmi les validateurs private-DS (ou custodians designes). La chaine globale ne porte que des engagements DA (sans emplacement de shard ni cles).

2) Engagements et sampling
- Pour chaque blob: calculer une racine Merkle sur les shards et l'inclure dans `*_da_commitment`. Rester PQ en evitant les engagements a courbe elliptique.
- DA Attesters: attesters regionaux echantillonnes par VRF (p. ex., 64 par region) emettent un certificat ML-DSA-87 attestant un sampling reussi des shards. Objectif de latence d'attestation DA <=300 ms. Le comite nexus valide les certificats au lieu de tirer les shards.

3) Integration Kura
- Les blocs stockent les corps de transaction comme blobs a effacement code avec engagements Merkle.
- Les headers portent les engagements de blob; les corps sont recuperables via le reseau DA pour public DS et via des canaux prives pour private DS.

4) Integration WSV
- Snapshots WSV: periodiquement, checkpoint de l'etat DS en snapshots chunkes et codes par effacement avec engagements enregistres dans les headers. Entre snapshots, des change logs sont maintenus. Les snapshots publics sont largement shards; les snapshots prives restent dans les validateurs prives.
- Acces porteur de preuves: les contrats peuvent fournir (ou demander) des preuves d'etat (Merkle/Verkle) ancrees par des engagements de snapshot. Les Private DS peuvent fournir des attestations zero-knowledge au lieu de preuves brutes.

5) Retention et pruning
- Pas de pruning pour public DS: conserver tous les corps Kura et snapshots WSV via DA (scalabilite horizontale). Les Private DS peuvent definir une retention interne, mais les engagements exportes restent immuables. La couche nexus conserve tous les blocs Nexus et les engagements d'artefacts DS.

Reseau et roles de noeuds
- Validateurs globaux: participent au consensus nexus, valident les blocs Nexus et les artefacts DS, effectuent des checks DA pour public DS.
- Validateurs Data Space: executent le consensus DS, executent les contrats, gerent Kura/WSV local, gerent DA pour leur DS.
- Noeuds DA (optionnel): stockent/publient des blobs publics, facilitent le sampling. Pour private DS, les noeuds DA sont co-localises avec les validateurs ou des custodians de confiance.

Ameliorations et considerations systeme
- Decouplage sequencing/mempool: adopter un mempool DAG (p. ex., style Narwhal) alimentant un BFT pipeline a la couche nexus pour reduire la latence et ameliorer le throughput sans changer le modele logique.
- Quotas DS et fairness: quotas par DS par bloc et caps de poids pour eviter le head-of-line blocking et assurer une latence previsible pour private DS.
- Attestation DS (PQ): les certificats de quorum DS utilisent ML-DSA-87 (classe Dilithium5) par defaut. C'est post-quantique et plus volumineux que les signatures EC mais acceptable a un QC par slot. Les DS peuvent explicitement opter pour ML-DSA-65/44 (plus petits) ou des signatures EC si declare dans le DS Manifest; les Public DS sont fortement encourages a garder ML-DSA-87.
- DA attesters: pour public DS, utiliser des attesters regionaux echantillonnes par VRF qui emettent des certificats DA. Le comite nexus valide les certificats au lieu du sampling brut des shards; les Private DS gardent les attestations DA internes.
- Recursion et preuves par epoch: optionnellement agreger plusieurs micro-batches dans un DS en une preuve recursive par slot/epoch pour garder taille de preuve et temps de verification stables sous charge elevee.
- Scaling de lanes (si necessaire): si un comite global unique devient un goulot, introduire K lanes de sequencing paralleles avec un merge deterministe. Cela preserve un ordre global unique tout en scalant horizontalement.
- Acceleration deterministe: fournir des kernels SIMD/CUDA gates par feature pour hashing/FFT avec un fallback CPU bit-exact pour preserver le determinisme cross-hardware.
- Seuils d'activation des lanes (proposition): activer 2-4 lanes si (a) la finalite p95 depasse 1.2 s pendant >3 minutes consecutives, ou (b) l'occupation par bloc depasse 85% pendant >5 minutes, ou (c) le debit entrant de tx requerrait >1.2x la capacite de bloc a des niveaux soutenus. Les lanes bucketisent les transactions de maniere deterministe par hash de DSID et se merge dans le bloc nexus.

Frais et economie (valeurs initiales)
- Unite de gas: token de gas par DS avec compute/IO metes; les frais sont payes en actif de gas natif du DS. La conversion entre DS est une preoccupation applicative.
- Priorite d'inclusion: round-robin entre DS avec quotas par DS pour preserver la fairness et les SLOs 1s; dans un DS, la mise en avant par fees peut departager.
- Futur: un marche global de fees ou des politiques minimisant MEV peuvent etre explores sans changer l'atomicite ni le design de preuves PQ.

Workflow cross-Data-Space (exemple)
1) Un utilisateur soumet une transaction AMX touchant un public DS P et un private DS S: deplacer l'actif X de S vers le beneficiaire B dont le compte est dans P.
2) Dans le slot, P et S executent leur fragment contre le snapshot du slot. S verifie l'autorisation et la disponibilite, met a jour son etat interne, et produit une preuve de validite PQ et un engagement DA (aucune donnee privee ne fuite). P prepare la mise a jour d'etat correspondante (p. ex., mint/burn/locking dans P selon la politique) et sa preuve.
3) Le comite nexus verifie les deux preuves DS et les certificats DA; si les deux verifient dans le slot, la transaction est commit atomiquement dans le bloc Nexus de 1s, mettant a jour les deux roots DS dans le vecteur world state global.
4) Si une preuve ou un certificat DA est manquant/invalide, la transaction avorte (aucun effet), et le client peut re-soumettre pour le slot suivant. Aucune donnee privee ne quitte S a aucun moment.

- Considerations de securite
- Execution deterministe: les syscalls IVM restent deterministes; les resultats cross-DS sont dictates par AMX commit et finalite, pas par l'horloge ou le timing reseau.
- Controle d'acces: les permissions ISI dans private DS restreignent qui peut soumettre des transactions et quelles operations sont autorisees. Les capability tokens encodent des droits fins pour usage cross-DS.
- Confidentialite: chiffrement end-to-end pour donnees private-DS, shards a effacement code stockes uniquement parmi les membres autorises, preuves ZK optionnelles pour attestations externes.
- Resistance DoS: l'isolation au niveau mempool/consensus/stockage empeche la congestion publique d'impacter la progression des private DS.

Changements des composants Iroha
- iroha_data_model: introduire `DataSpaceId`, IDs qualifies par DS, descripteurs AMX (ensembles lecture/ecriture), types de preuve/engagement DA. Serialisation Norito uniquement.
- ivm: ajouter des syscalls et des types pointer-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et preuves DA; mettre a jour les tests/docs ABI selon la politique v1.
