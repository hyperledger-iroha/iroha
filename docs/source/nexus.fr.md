---
lang: fr
direction: ltr
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

#! Iroha 3 - Sora Nexus Ledger: Specification technique de conception

Ce document propose l architecture du Sora Nexus Ledger pour Iroha 3, faisant evoluer Iroha 2 vers un ledger global unique, logiquement unifie, organise autour des Data Spaces (DS). Les Data Spaces fournissent des domaines de confidentialite forts ("private data spaces") et une participation ouverte ("public data spaces"). Le design preserve la composabilite a travers le ledger global tout en assurant une isolation stricte et la confidentialite des donnees des DS prives, et introduit une mise a l echelle de la disponibilite des donnees via l erasure coding sur Kura (stockage des blocs) et WSV (World State View).

Le meme depot construit Iroha 2 (reseaux auto-heberges) et Iroha 3 (SORA Nexus). L execution est assuree par la Iroha Virtual Machine (IVM) et la chaine d outils Kotodama partagees, donc les contrats et artefacts de bytecode restent portables entre deployments auto-heberges et le ledger global Nexus.

Objectifs
- Un ledger logique global compose de nombreux validateurs cooperatifs et Data Spaces.
- Des Data Spaces prives pour operation permissionnee (ex., CBDC), avec des donnees qui ne quittent jamais le DS prive.
- Des Data Spaces publics avec participation ouverte, acces sans permission de type Ethereum.
- Des contrats intelligents composables entre Data Spaces, soumis a des permissions explicites pour acces aux actifs des DS prives.
- Isolation de performance pour que l activite publique ne degrade pas les transactions internes des DS prives.
- Disponibilite des donnees a grande echelle: Kura et WSV avec erasure coding pour supporter des donnees effectivement illimitees tout en gardant privees les donnees des DS prives.

Non-Objectifs (Phase initiale)
- Definir les economies de tokens ou incitations de validateurs; la planification et le staking sont enfichables.
- Introduire une nouvelle version ABI ou etendre les surfaces syscalls/pointer-ABI; l ABI v1 est fixe et les runtime upgrades ne changent pas l ABI du host.

Terminologie
- Nexus Ledger: Le ledger logique global forme par la composition de blocs Data Space (DS) en un historique unique et ordonne et un engagement d etat.
- Data Space (DS): Un domaine borne d execution et de stockage avec ses propres validateurs, gouvernance, classe de confidentialite, politique DA, quotas et politique de fees. Deux classes existent: DS public et DS prive.
- Private Data Space: Validateurs permissionnes et controle d acces; les donnees de transaction et d etat ne quittent jamais le DS. Seuls des engagements/metadata sont ancres globalement.
- Public Data Space: Participation sans permission; donnees completes et etat disponibles publiquement.
- Data Space Manifest (DS Manifest): Un manifeste encode Norito declarant les parametres DS (validateurs/cles QC, classe de confidentialite, politique ISI, parametres DA, retention, quotas, politique ZK, fees). Le hash du manifeste est ancre sur la chaine nexus. Sauf indication contraire, les quorum certificates DS utilisent ML-DSA-87 (classe Dilithium5) comme schema de signature post-quantum par defaut.
- Space Directory: Un contrat de repertoire global on-chain qui suit les manifestes DS, les versions et les evenements de gouvernance/rotation pour resolvabilite et audits.
- DSID: Un identifiant global unique pour un Data Space. Utilise pour namespacing de tous les objets et references.
- Anchor: Un engagement cryptographique d un bloc/header DS inclus dans la chaine nexus pour lier l historique DS au ledger global.
- Kura: Stockage des blocs Iroha. Etendu ici avec un stockage blob en erasure coding et des engagements.
- WSV: World State View Iroha. Etendu ici avec des segments d etat versionnes, capables de snapshots, en erasure coding.
- IVM: Iroha Virtual Machine pour execution de contrats intelligents (bytecode Kotodama `.to`).
 - AIR: Algebraic Intermediate Representation. Une vue algebrique du calcul pour des preuves type STARK, decrivant l execution comme des traces basees sur des champs avec contraintes de transition et de bord.

Modele des Data Spaces
- Identite: `DataSpaceId (DSID)` identifie un DS et namespace tout. Les DS peuvent etre instancies a deux granularites:
  - Domain-DS: `ds::domain::<domain_name>` - execution et etat limites a un domaine.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execution et etat limites a une seule definition d asset.
  Les deux formes coexistent; les transactions peuvent toucher plusieurs DSID de maniere atomique.
- Cycle de vie du manifeste: creation DS, mises a jour (rotation de cles, changements de politique) et retraite sont enregistres dans le Space Directory. Chaque artefact par slot reference le hash du manifeste le plus recent.
- Classes: DS public (participation ouverte, DA publique) et DS prive (permissionne, DA confidentiel). Des politiques hybrides sont possibles via des flags de manifeste.
- Politiques par DS: permissions ISI, parametres DA `(k,m)`, chiffrement, retention, quotas (part min/max de tx par bloc), politique de preuves ZK/optimistes, fees.
- Gouvernance: membres DS et rotation des validateurs definies par la section gouvernance du manifeste (propositions on-chain, multisig, ou gouvernance externe ancree par transactions et attestations nexus).

Gossip conscient des dataspaces
- Les lots de gossip de transactions portent maintenant un tag de plan (public vs restreint) derive du catalogue de lanes; les lots restreints sont unicast vers les pairs en ligne de la topologie de commit actuelle (respectant `transaction_gossip_restricted_target_cap`) tandis que les lots publics utilisent `transaction_gossip_public_target_cap` (mettre `null` pour broadcast). La selection des cibles se remelange a la cadence par plan definie par `transaction_gossip_public_target_reshuffle_ms` et `transaction_gossip_restricted_target_reshuffle_ms` (par defaut: `transaction_gossip_period_ms`). Quand aucun pair de la topologie de commit n est en ligne, les operateurs peuvent choisir de refuser ou de relayer des payloads restreints vers l overlay public via `transaction_gossip_restricted_public_payload` (par defaut `refuse`); la telemetrie expose les tentatives de fallback, les comptages forward/drop et la politique configuree aux cotes des selections de cibles par dataspace.
- Les dataspaces inconnus sont remis en file quand `transaction_gossip_drop_unknown_dataspace` est active; sinon ils tombent sur un ciblage restreint pour eviter les fuites.
- La validation cote reception drop les entrees dont les lanes/dataspaces ne correspondent pas au catalogue local, dont le tag de plan ne correspond pas a la visibilite dataspace derivee, ou dont la route annoncee ne correspond pas a la decision de routage rederivee localement.

Manifeste de capacites et UAID
- Comptes universels: Chaque participant recoit un UAID deterministe (`UniversalAccountId` dans `crates/iroha_data_model/src/nexus/manifest.rs`) qui couvre tous les dataspaces. Les manifestes de capacites (`AssetPermissionManifest`) lient un UAID a un dataspace specifique, des epochs d activation/expiration, et une liste ordonnee de regles allow/deny `ManifestEntry` qui bornent `dataspace`, `program_id`, `method`, `asset` et des roles AMX optionnels. Les regles deny gagnent toujours; l evaluateur emet soit `ManifestVerdict::Denied` avec une raison d audit, soit un grant `Allowed` avec la metadata d allocation correspondante.
- Les snapshots de portefeuille UAID sont exposes via `GET /v1/accounts/{uaid}/portfolio` (voir `docs/source/torii/portfolio_api.md`), appuyes par l agregateur deterministe dans `iroha_core::nexus::portfolio`.
- Allowances: Chaque entree allow porte des buckets `AllowanceWindow` deterministes (`PerSlot`, `PerMinute`, `PerDay`) plus un `max_amount` optionnel. Les hosts et SDKs consomment le meme payload Norito, donc l enforcement reste identique sur tous les hardwares et SDKs.
- Telemetrie d audit: Le Space Directory emet `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) chaque fois qu un manifeste change d etat. La surface `SpaceDirectoryEventFilter` permet aux abonnes Torii/data-event de surveiller les mises a jour UAID, les revocations et les decisions deny-wins sans plomberie specifique.

### Operations de manifeste UAID

Les operations du Space Directory sont livreies sous deux formes pour que les operateurs puissent choisir soit le
CLI integre (pour des deploiements scriptes) soit des soumissions Torii directes (pour un
CI/CD automatise). Les deux chemins appliquent le droit `CanPublishSpaceDirectoryManifest{dataspace}`
 dans l executeur (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`)
et enregistrent des evenements de cycle de vie dans le world state (`iroha_core::state::space_directory_manifests`).

#### Flux de travail CLI (`iroha app space-directory manifest ...`)

1. **Encoder le JSON du manifeste** - convertir des brouillons de politique en octets Norito et emettre un
   hash reproductible avant revue:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   Le helper accepte soit `--json` (manifeste JSON brut) soit `--manifest` (payload `.to` existant)
   et reflete la logique dans
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **Publier/remplacer des manifestes** - mettre en file des instructions `PublishSpaceDirectoryManifest`
   depuis des sources Norito ou JSON:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` remplit `entries[*].notes` pour les enregistrements qui omettaient des notes operateur.

3. **Expirer** des manifestes qui ont atteint leur fin de vie planifiee ou **revoquer**
   des UAID a la demande. Les deux commandes acceptent `--uaid uaid:<hex>` ou un digest hex
   de 64 caracteres (LSB=1) et l identifiant numerique du dataspace:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Produire des bundles d audit** - `manifest audit-bundle` ecrit le JSON du manifeste,
   le payload `.to`, le hash, le profil de dataspace et des metadonnees lisibles par machine dans un
   repertoire de sortie pour que les reviewers de gouvernance telechargent une seule archive:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   Le bundle integre des hooks `SpaceDirectoryEvent` depuis le profil pour prouver que le
   dataspace expose les webhooks d audit obligatoires; voir `docs/space-directory.md`
   pour la structure des champs et les exigences de preuve.

#### APIs Torii

Les operateurs et SDKs peuvent effectuer les memes actions via HTTPS. Torii applique les
meme verifications de permission et signe les transactions au nom de l autorite fournie
(les cles privees ne circulent qu en memoire dans le handler securise de Torii):

- `GET /v1/space-directory/uaids/{uaid}` - resoudre les liaisons de dataspace courantes
  pour un UAID (adresses normalisees, ids de dataspace, liaisons de programme). Ajouter
  `address_format=compressed` pour la sortie Sora Name Service (IH58 prefere; compressed (`sora`) est le second choix Sora-only).
- `GET /v1/space-directory/uaids/{uaid}/portfolio` -
  agregateur adosse a Norito qui reflete `ToriiClient.getUaidPortfolio` pour que les wallets
  rendent les holdings universels sans scruter l etat par dataspace.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` - obtenir le JSON du manifeste
  canonique, les metadonnees de cycle de vie et le hash du manifeste pour audits.
- `POST /v1/space-directory/manifests` - soumettre des manifestes nouveaux ou de remplacement
  depuis JSON (`authority`, `private_key`, `manifest`, `reason` optionnel). Torii
  renvoie `202 Accepted` une fois la transaction en file.
- `POST /v1/space-directory/manifests/revoke` - mettre en file des revocations d urgence
  avec UAID, id de dataspace, epoch effectif et reason optionnel (reflete la disposition
  CLI).

Le SDK JS (`javascript/iroha_js/src/toriiClient.js`) couvre deja ces surfaces de lecture
via `ToriiClient.getUaidPortfolio`, `.getUaidBindings`, et
`.getUaidManifests`; les futures versions Swift/Python reutiliseront les memes payloads REST.
Voir `docs/source/torii/portfolio_api.md` pour les schemas request/response complets et
`docs/space-directory.md` pour le playbook operateur end-to-end.

Mises a jour recentes SDK/AMX
- **NX-11 (verification relay cross-lane):** les helpers SDK valident maintenant les
  envelopes de relay de lane exposes par `/v1/sumeragi/status`. Le client Rust livre
  des helpers `iroha::nexus` pour construire/verifier des preuves relay et rejeter des tuples
  dupliques `(lane_id, dataspace_id, height)`, le binding Python expose
  `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, et le SDK JS expose
  `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` pour que les operateurs puissent valider
  des preuves de transfert cross-lane avec des hashes coherents avant de les relayer.
  (crates/iroha/src/nexus.rs:1, python/iroha_python/iroha_python_rs/src/lib.rs:666, crates/iroha_js_host/src/lib.rs:640, javascript/iroha_js/src/nexus.js:1)
- **NX-17 (guardrails budget AMX):** `ivm::analysis::enforce_amx_budget` estime
  le cout d execution par dataspace et groupe en utilisant le rapport d analyse statique et
  impose les budgets 30 ms / 140 ms captures ici. Le helper expose des violations claires
  pour budgets par DS et groupe et est couvert par des tests unitaires,
  rendant le budget de slot AMX deterministe pour les planificateurs Nexus et les outils
  SDK. (crates/ivm/src/analysis.rs:142, crates/ivm/src/analysis.rs:241)

Architecture de haut niveau
1) Couche de composition globale (Nexus Chain)
- Maintient un ordre canonique unique de blocs Nexus de 1 s qui finalisent des transactions atomiques sur un ou plusieurs Data Spaces (DS). Chaque transaction committe met a jour le world state global unifie (vecteur de racines par DS).
- Contient une metadata minimale plus des preuves/QC agreges pour assurer composabilite, finalite et detection de fraude (DSIDs touches, racines d etat par DS avant/apres, engagements DA, preuves de validite par DS, et le quorum certificate DS utilisant ML-DSA-87). Aucune donnee privee n est incluse.
- Consensus: Comite BFT global, pipeline, de taille 22 (3f+1 avec f=7), selectionne depuis un pool allant jusqu a ~200k validateurs potentiels par un mecanisme VRF/stake par epoch. Le comite nexus sequence les transactions et finalise le bloc en 1 s.

2) Couche Data Space (Public/Prive)
- Execute des fragments par DS des transactions globales, met a jour le WSV local DS et produit des artefacts de validite par bloc (preuves DS agregees et engagements DA) qui se rassemblent dans le bloc Nexus de 1 s.
- Les DS prives chiffrent les donnees au repos et en transit entre validateurs autorises; seuls les engagements et preuves PQ de validite sortent du DS.
- Les DS publics exportent les corps de donnees complets (via DA) et les preuves PQ de validite.

3) Transactions atomiques cross-Data-Space (AMX)
- Modele: Chaque transaction utilisateur peut toucher plusieurs DS (ex., domain DS et un ou plusieurs asset DS). Elle est committee atomiquement dans un bloc Nexus de 1 s ou abandonnee; aucun effet partiel.
- Prepare-commit dans 1 s: Pour chaque transaction candidate, les DS touches executent en parallele contre le meme snapshot (racines DS debut de slot) et produisent des preuves PQ par DS (FASTPQ-ISI) et engagements DA. Le comite nexus commit la transaction seulement si toutes les preuves DS requises verifient et que les certificats DA arrivent (cible <=300 ms); sinon la transaction est replanifiee pour le slot suivant.
- Consistance: Les ensembles lecture-ecriture sont declares; la detection de conflits a lieu au commit contre les racines de debut de slot. L execution optimiste sans verrou par DS evite les stalls globaux; l atomicite est imposee par la regle de commit nexus (tout-ou-rien entre DS).
- Confidentialite: Les DS prives n exportent que des preuves/engagements lies aux racines DS pre/post. Aucune donnee privee brute ne quitte le DS.

4) Disponibilite des donnees (DA) avec erasure coding
- Kura stocke les corps de blocs et snapshots WSV comme blobs en erasure coding. Les blobs publics sont largement shardes; les blobs prives sont stockes uniquement dans les validateurs DS prives, avec des chunks chiffres.
- Les engagements DA sont enregistres dans les artefacts DS et les blocs Nexus, permettant echantillonnage et recuperation sans reveler de contenus prives.

Structure des blocs et des commits
- Artefact de preuve Data Space (par slot 1 s, par DS)
  - Champs: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les DS prives exportent des artefacts sans corps de donnees; les DS publics permettent la recuperation de corps via DA.

- Bloc Nexus (cadence 1 s)
  - Champs: block_number, parent_hash, slot_time, tx_list (transactions atomiques cross-DS avec DSIDs touches), ds_artifacts[], nexus_qc.
  - Fonction: finalise toutes les transactions atomiques dont les artefacts DS requis verifient; met a jour le vecteur global des racines DS en une etape.

Consensus et planification
- Consensus de la Nexus Chain: BFT global pipeline (classe Sumeragi) avec un comite de 22 noeuds (3f+1 avec f=7) visant des blocs de 1 s et une finalite de 1 s. Les membres du comite sont selectionnes par epoch via VRF/stake parmi ~200k candidats; la rotation maintient la decentralisation et la resistance a la censure.
- Consensus Data Space: Chaque DS execute son propre BFT parmi ses validateurs pour produire des artefacts par slot (preuves, engagements DA, DS QC). Les comites lane-relay sont dimensionnes a `3f+1` via le `fault_tolerance` du dataspace et echantillonnes de maniere deterministe par epoch depuis le pool de validateurs du dataspace en utilisant la graine VRF de l epoch liee a `(dataspace_id, lane_id)`. Les DS prives sont permissionnes; les DS publics permettent une liveness ouverte soumise a des politiques anti-Sybil. Le comite global nexus reste inchange.
- Planification des transactions: Les utilisateurs soumettent des transactions atomiques declarant les DSIDs touches et des ensembles lecture-ecriture. Les DS executent en parallele dans le slot; le comite nexus inclut la transaction dans le bloc de 1 s si tous les artefacts DS verifient et que les certificats DA sont a temps (<=300 ms).
- Isolation de performance: Chaque DS a des mempools et une execution independants. Les quotas par DS limitent combien de transactions touchant un DS peuvent etre commises par bloc pour eviter head-of-line blocking et proteger la latence des DS prives.

Modele de donnees et namespacing
- IDs qualifies par DS: Toutes les entites (domaines, comptes, actifs, roles) sont qualifiees par `dsid`. Exemple: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- References globales: Une reference globale est un tuple `(dsid, object_id, version_hint)` et peut etre place on-chain dans la couche nexus ou dans des descripteurs AMX pour usage cross-DS.
- Serialization Norito: Tous les messages cross-DS (descripteurs AMX, preuves) utilisent les codecs Norito. Pas de serde dans les chemins production.

Contrats intelligents et extensions IVM
- Contexte d execution: Ajouter `dsid` au contexte d execution IVM. Les contrats Kotodama s executent toujours dans un Data Space specifique.
- Primitives atomiques cross-DS:
  - `amx_begin()` / `amx_commit()` delimitent une transaction atomique multi-DS dans l host IVM.
  - `amx_touch(dsid, key)` declare une intention lecture/ecriture pour la detection de conflits contre les racines snapshot du slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operation permise seulement si la politique l autorise et si le handle est valide)
- Handles d actifs et fees:
  - Les operations d actifs sont autorisees par les politiques ISI/roles du DS; les fees sont payees dans le token gas du DS. Des tokens de capacite optionnels et des politiques plus riches (multi-approver, rate-limits, geofencing) peuvent etre ajoutes plus tard sans changer le modele atomique.
- Determinisme: Toutes les syscalls sont pures et deterministes compte tenu des entrees et des ensembles lecture/ecriture AMX declares. Aucun effet cache de temps ou d environnement.

Preuves de validite post-quantum (ISIs generalises)
- FASTPQ-ISI (PQ, sans trusted setup): Un argument base sur hash, kernelise, qui generalise le design de transfer a toutes les familles ISI tout en visant des preuves sous-seconde pour des lots a echelle 20k sur du materiel classe GPU.
  - Profil operationnel:
    - Les noeuds de production construisent le prover via `fastpq_prover::Prover::canonical`, qui initialise toujours le backend de production; le mock deterministe a ete retire. (crates/fastpq_prover/src/proof.rs:126)
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` permettent aux operateurs de figer l execution CPU/GPU de maniere deterministe tandis que le hook observer enregistre les triples requested/resolved/backend pour audits de flotte. (crates/iroha_config/src/parameters/user.rs:1357, crates/irohad/src/main.rs:270, crates/irohad/src/main.rs:2192, crates/iroha_telemetry/src/metrics.rs:8887)
- Arithmetisation:
  - AIR de mise a jour KV: Traite WSV comme une map key-value typee engagee via Poseidon2-SMT. Chaque ISI s etend en un petit ensemble de lignes read-check-write sur des cles (comptes, actifs, roles, domaines, metadata, supply).
  - Contraintes gatees par opcode: Une table AIR unique avec colonnes selecteurs impose les regles par ISI (conservation, compteurs monotones, permissions, range checks, mises a jour de metadata bornees).
  - Arguments de lookup: Tables transparentes, hash-committed, pour permissions/roles, precisions d actifs et parametres de politique evitent des contraintes bit-heavy.
- Engagements d etat et mises a jour:
  - Preuve SMT agreggee: Toutes les cles touchees (pre/post) sont prouvees contre `old_root`/`new_root` en utilisant une frontiere compressee avec siblings dedupes.
  - Invariants: Les invariants globaux (ex., supply total par actif) sont imposes via egalite de multisets entre lignes d effets et compteurs suivis.
- Systeme de preuve:
  - Engagements polynomiaux style FRI (DEEP-FRI) avec haute arite (8/16) et blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir avec SHA-2/3.
  - Recursion optionnelle: agregation recursive locale DS pour compresser des micro-batches a une preuve par slot si necessaire.
- Portee et exemples couverts:
  - Actifs: transfer, mint, burn, enregistrer/desenregistrer des definitions d actifs, set precision (bornee), set metadata.
  - Comptes/Domains: creer/supprimer, set key/threshold, ajouter/supprimer signatories (etat uniquement; les checks de signature sont attestes par les validateurs DS, pas prouves dans l AIR).
  - Roles/Permissions (ISI): accorder/revoquer roles et permissions; appliques par tables de lookup et checks de politique monotone.
  - Contrats/AMX: marqueurs AMX begin/commit, mint/revoke de capacites si active; prouves comme transitions d etat et compteurs de politique.
- Checks hors AIR pour preserver la latence:
  - Signatures et cryptographie lourde (ex., signatures utilisateur ML-DSA) sont verifiees par les validateurs DS et attestees dans le DS QC; la preuve de validite couvre uniquement la coherence d etat et la conformite de politique. Cela garde les preuves PQ rapides.
- Cibles de performance (illustratives, CPU 32-core + une GPU moderne):
  - 20k ISIs mixtes avec faible touche de cles (<=8 cles/ISI): ~0.4-0.9 s de preuve, ~150-450 KB de preuve, ~5-15 ms de verification.
  - ISIs plus lourdes (plus de cles/contraintes riches): micro-batch (ex., 10x2k) + recursion pour garder par slot <1 s.
- Configuration DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures verifiees par DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (par defaut; alternatives doivent etre declarees explicitement)
- Fallbacks:
  - Les ISIs complexes/personnalises peuvent utiliser un STARK general (`zk.policy = "stark_fri_general"`) avec preuve differee et finalite 1 s via attestation QC + slashing sur preuves invalides.
  - Les options non-PQ (ex., Plonk avec KZG) demandent un trusted setup et ne sont plus supportees dans le build par defaut.

Introduction a AIR (pour Nexus)
- Trace d execution: Une matrice avec largeur (colonnes de registres) et longueur (pas). Chaque ligne est une etape logique du traitement ISI; les colonnes contiennent valeurs pre/post, selecteurs et flags.
- Contraintes:
  - Contraintes de transition: imposent des relations ligne-a-ligne (ex., post_balance = pre_balance - amount pour une ligne de debit quand `sel_transfer = 1`).
  - Contraintes de bord: lient l I/O publique (old_root/new_root, compteurs) aux premieres/dernieres lignes.
  - Lookups/permutations: garantissent l appartenance et les egalites de multiset contre des tables engagees (permissions, parametres d actifs) sans circuits bits lourds.
- Engagement et verification:
  - Le prover engage les traces via des encodages base sur hash et construit des polynomes de faible degre valides si les contraintes tiennent.
  - Le verificateur verifie le faible degre via FRI (base sur hash, post-quantum) avec quelques ouvertures Merkle; le cout est logarithmique en pas.
- Exemple (Transfer): registres incluant pre_balance, amount, post_balance, nonce et selecteurs. Les contraintes imposent non-negativite/range, conservation et monotonie du nonce, tandis qu une multi-preuve SMT agreggee lie les feuilles pre/post aux racines old/new.

Stabilite ABI (ABI v1)
- La surface ABI v1 est fixe; aucun nouveau syscall ni type pointer-ABI n est introduit dans ce release.
- Les runtime upgrades doivent garder `abi_version = 1` avec `added_syscalls`/`added_pointer_types` vides.
- Les goldens ABI (liste syscalls, hash ABI, IDs pointer type) restent fixes et ne doivent pas changer.

Modele de confidentialite
- Contention des donnees privees: corps de transaction, diffs d etat et snapshots WSV pour DS prives ne quittent jamais le sous-ensemble de validateurs prives.
- Exposition publique: Seuls les headers, engagements DA et preuves PQ de validite sont exportes.
- Preuves ZK optionnelles: Les DS prives peuvent produire des preuves ZK (ex., solde suffisant, politique satisfaite) permettant des actions cross-DS sans reveler l etat interne.
- Controle d acces: L autorisation est appliquee par les politiques ISI/roles dans le DS. Les tokens de capacite sont optionnels et peuvent etre introduits plus tard si necessaire.

Isolation de performance et QoS
- Consensus, mempools et stockage separes par DS.
- Quotas de planification Nexus par DS pour borner le temps d inclusion des anchors et eviter head-of-line blocking.
- Budgets de ressources de contrat par DS (compute/memory/IO), appliques par l host IVM. La contention DS public ne peut pas consommer les budgets DS prives.
- Les appels cross-DS asynchrones evitent de longues attentes synchrones dans l execution DS privee.

Disponibilite des donnees et design de stockage
1) Erasure Coding
- Utiliser Reed-Solomon systematique (ex., GF(2^16)) pour l erasure coding au niveau blob des blocs Kura et snapshots WSV: parametres `(k, m)` avec `n = k + m` shards.
- Parametres par defaut (proposes, DS publics): `k=32, m=16` (n=48), permettant une recuperation jusqu a 16 pertes de shards avec ~1.5x d expansion. Pour DS prives: `k=16, m=8` (n=24) dans l ensemble permissionne. Les deux sont configurables via DS Manifest.
- Blobs publics: Shards distribues sur de nombreux noeuds DA/validateurs avec checks de disponibilite par echantillonnage. Les engagements DA dans les headers permettent la verification light client.
- Blobs prives: Shards chiffres et distribues uniquement chez les validateurs DS prives (ou custodians designes). La chaine globale porte uniquement des engagements DA (pas de localisation de shards ni de cles).

2) Engagements et echantillonnage
- Pour chaque blob: calculer une racine Merkle sur les shards et l inclure dans `*_da_commitment`. Rester PQ en evitant les engagements a courbe elliptique.
- Attesters DA: attesters regionaux echantillonnes par VRF (ex., 64 par region) emettent un certificat ML-DSA-87 attestant un echantillonnage reussi des shards. Cible de latence d attestation DA <=300 ms. Le comite nexus valide les certificats au lieu de recuperer des shards.

3) Integration Kura
- Les blocs stockent les corps de transaction comme blobs en erasure coding avec engagements Merkle.
- Les headers portent des engagements de blobs; les corps sont recuperes via le reseau DA pour DS publics et via canaux prives pour DS prives.

4) Integration WSV
- Snapshotting WSV: Periodiquement, checkpoint de l etat DS en snapshots chunkes et erasure codes avec engagements enregistres dans les headers. Entre snapshots, maintenir des journaux de changements. Les snapshots publics sont largement shards; les snapshots prives restent dans les validateurs prives.
- Acces par preuve: Les contrats peuvent fournir (ou demander) des preuves d etat (Merkle/Verkle) ancrees par des engagements de snapshot. Les DS prives peuvent fournir des attestations zero-knowledge a la place de preuves brutes.

5) Retention et pruning
- Pas de pruning pour DS publics: conserver tous les corps Kura et snapshots WSV via DA (scaling horizontal). Les DS prives peuvent definir une retention interne, mais les engagements exportes restent immuables. La couche nexus conserve tous les blocs Nexus et engagements d artefacts DS.

Reseaux et roles de noeuds
- Validateurs globaux: participent au consensus nexus, valident les blocs Nexus et artefacts DS, effectuent des checks DA pour DS publics.
- Validateurs Data Space: executent le consensus DS, executent les contrats, gerent Kura/WSV local, gerent DA pour leur DS.
- Noeuds DA (optionnels): stockent/publicisent des blobs publics, facilitent l echantillonnage. Pour DS prives, les noeuds DA sont co-localises avec les validateurs ou des custodians de confiance.

Ameliorations et considerations systeme
- Decouplage sequencing/mempool: adopter un mempool DAG (ex., style Narwhal) alimentant un BFT pipeline au niveau nexus pour reduire la latence et ameliorer le throughput sans changer le modele logique.
- Quotas DS et fairness: quotas par DS par bloc et caps de poids pour eviter head-of-line blocking et assurer une latence previsible pour DS prives.
- Attestation DS (PQ): Les quorum certificates DS par defaut utilisent ML-DSA-87 (classe Dilithium5). C est post-quantum et plus volumineux que les signatures EC mais acceptable a un QC par slot. Les DS peuvent explicitement opter pour ML-DSA-65/44 (plus petit) ou signatures EC si declare dans le DS Manifest; les DS publics sont fortement encourages a garder ML-DSA-87.
- Attesters DA: Pour DS publics, utiliser des attesters regionaux echantillonnes par VRF qui emettent des certificats DA. Le comite nexus valide les certificats au lieu d un echantillonnage brut de shards; les DS prives gardent les attestations DA internes.
- Recursion et preuves par epoch: aggreger plusieurs micro-batches au sein d un DS en une preuve recursive par slot/epoch pour garder la taille des preuves et le temps de verification stables sous forte charge.
- Scalabilite par lanes (si besoin): si un comite global unique devient un goulot, introduire K lanes de sequencing paralleles avec un merge deterministe. Cela preserve un ordre global unique tout en scalant horizontalement.
- Acceleration deterministe: fournir des kernels SIMD/CUDA feature-gates pour hashing/FFT avec un fallback CPU bit-exact pour preserver le determinisme cross-hardware.
- Seuils d activation de lanes (proposition): activer 2-4 lanes si (a) la finalite p95 depasse 1.2 s pendant >3 minutes consecutives, ou (b) l occupation par bloc depasse 85% pendant >5 minutes, ou (c) le taux de tx entrant exigerait >1.2x la capacite de bloc a des niveaux soutenus. Les lanes bucketisent les transactions de maniere deterministe par hash DSID et fusionnent dans le bloc nexus.

Fees et economie (defauts initiaux)
- Unite de gas: token gas par DS avec compute/IO mesure; les fees sont payees dans l asset gas natif du DS. La conversion entre DS est une preoccupation applicative.
- Priorite d inclusion: round-robin entre DS avec quotas par DS pour preserver l equite et les SLO 1 s; dans un DS, les encheres de fees peuvent briser les egalites.
- Futur: un marche de fees global ou des politiques minimisant le MEV peuvent etre explorees sans changer l atomicite ou le design des preuves PQ.

Flux cross-Data-Space (Exemple)
1) Un utilisateur soumet une transaction AMX touchant un DS public P et un DS prive S: deplacer l actif X de S vers un beneficiaire B dont le compte est dans P.
2) Dans le slot, P et S executent chacun leur fragment contre le snapshot du slot. S verifie l autorisation et la disponibilite, met a jour son etat interne, et produit une preuve PQ de validite et un engagement DA (aucune donnee privee ne fuit). P prepare la mise a jour d etat correspondante (ex., mint/burn/locking dans P selon la politique) et sa preuve.
3) Le comite nexus verifie les deux preuves DS et certificats DA; si les deux verifient dans le slot, la transaction est committee atomiquement dans le bloc Nexus de 1 s, mettant a jour les racines DS dans le vecteur global de world state.
4) Si une preuve ou un certificat DA manque ou est invalide, la transaction est abandonnee (sans effets), et le client peut resoumettre pour le slot suivant. Aucune donnee privee ne quitte S a aucune etape.

- Considerations de securite
- Execution deterministe: Les syscalls IVM restent deterministes; les resultats cross-DS sont pilotes par le commit AMX et la finalite, pas par l horloge ou le timing reseau.
- Controle d acces: Les permissions ISI dans les DS prives restreignent qui peut soumettre des transactions et quelles operations sont autorisees. Les tokens de capacite encodent des droits fins pour usage cross-DS.
- Confidentialite: Chiffrement end-to-end pour les donnees DS privees, shards erasure-codes stockes uniquement parmi les membres autorises, preuves ZK optionnelles pour attestations externes.
- Resistance DoS: Isolation aux couches mempool/consensus/stockage empeche la congestion publique d impacter la progression des DS prives.

Changements des composants Iroha
- iroha_data_model: Introduire `DataSpaceId`, identifiants qualifies DS, descripteurs AMX (ensembles lecture/ecriture), types de preuve/engagement DA. Serialization Norito uniquement.
- ivm: Maintenir la surface ABI v1 fixe (pas de nouveaux syscalls ni types pointer-ABI); AMX/runtime upgrades utilisent les primitives v1 existantes; garder les goldens ABI.
- iroha_core: Implementer le planificateur nexus, Space Directory, routage/validation AMX, verification d artefacts DS et enforcement de politiques pour echantillonnage DA et quotas.
- Space Directory et loaders de manifestes: faire passer la metadata des endpoints FMS (et autres descripteurs de services common-good) via le parsing de manifestes DS pour que les noeuds auto-decouvrent des endpoints locaux en rejoignant un Data Space.
- kura: Blob store avec erasure coding, engagements, APIs de recuperation respectant les politiques prive/public.
- WSV: Snapshotting, chunking, engagements; APIs de preuve; integration avec detection et verification de conflits AMX.
- irohad: Roles de noeud, reseautage DA, membership/authentification DS prive, configuration via `iroha_config` (pas de toggles env en production).

Configuration et determinisme
- Tout comportement runtime est configure via `iroha_config` et propage via constructeurs/hosts. Pas de toggles env en production.
- Acceleration hardware (SIMD/NEON/METAL/CUDA) est optionnelle et feature-gated; les fallbacks deterministes doivent produire des resultats identiques entre hardware.
- - Post-Quantum par defaut: Tous les DS doivent utiliser des preuves de validite PQ (STARK/FRI) et ML-DSA-87 pour les QC DS par defaut. Les alternatives requierent une declaration explicite dans le DS Manifest et une approbation de politique.

### Controle du cycle de vie des lanes en runtime

- **Endpoint admin:** `POST /v1/nexus/lifecycle` (Torii) accepte un corps Norito/JSON avec `additions` (objets `LaneConfig` complets) et `retire` (ids de lane) pour ajouter ou retirer des lanes sans redemarrage. Les requetes sont gatees par `nexus.enabled=true` et reutilisent la meme vue configuration/etat Nexus que la queue.
- **Comportement:** En succes, le noeud applique le plan de cycle de vie a la metadata WSV/Kura, reconstruit le routage/limites/manifestes de queue, et repond avec `{ ok: true, lane_count: <u32> }`. Les plans qui echouent la validation (ids retire inconnus, aliases/ids dupliques, Nexus desactive) renvoient `400 Bad Request` avec un `lane_lifecycle_error`.
- **Securite:** Le handler utilise le lock de vue d etat partagee pour eviter les courses avec des lecteurs lors des mises a jour de catalogues; les appelants doivent encore serialiser les mises a jour de cycle de vie en externe pour eviter des plans conflictuels.
- **Propagation:** Le routage/limites de la queue et les manifests de lane sont reconstruits depuis le catalogue mis a jour, et les workers consensus/DA/RBC lisent la config de lane via des snapshots d etat; le scheduling et la selection des validateurs basculent sans redemarrage (le travail en vol termine avec l ancienne config).
- **Nettoyage stockage:** Kura et la geometrie WSV tiered sont reconciliees (creer/retirer/relabel), les mappings de curseurs de shards DA sont synchronises/persistes, et les lanes retirees sont purgees des caches de lane relay et des stores de commitments DA/confidential-compute/pin-intent.

Chemin de migration (Iroha 2 -> Iroha 3)
1) Introduire des IDs qualifies par dataspace et la composition de bloc nexus/etat global dans le data model; ajouter des feature flags pour conserver les modes herites d'Iroha 2 pendant la transition.
2) Implementer des backends Kura/WSV en erasure coding derriere des feature flags, en preservant les backends actuels comme defaults pendant les phases initiales.
3) Maintenir la surface ABI v1 fixe; implementer AMX sans nouveaux syscalls/types pointer et mettre a jour tests/docs sans changer ABI.
4) Livrer une chaine nexus minimale avec un seul DS public et des blocs 1 s; puis ajouter un premier pilote DS prive exportant uniquement preuves/engagements.
5) Etendre aux transactions atomiques cross-DS completes (AMX) avec preuves FASTPQ-ISI locales DS et attesters DA; activer ML-DSA-87 QCs sur tous les DS.

Strategie de tests
- Tests unitaires pour types data model, roundtrips Norito, comportements syscalls AMX, encodage/decodage de preuves.
- Tests IVM pour figer les goldens ABI v1 (liste syscalls, hash ABI, IDs pointer type).
- Tests d integration pour transactions atomiques cross-DS (positives/negatives), cibles de latence attester DA (<=300 ms), et isolation de performance sous charge.
- Tests de securite pour verification DS QC (ML-DSA-87), detection de conflits/semantique d abort, et prevention des fuites de shards confidentiels.

### Actifs de telemetrie et runbook NX-18

- **Dashboard Grafana:** `dashboards/grafana/nexus_lanes.json` exporte maintenant le dashboard "Nexus Lane Finality & Oracles" demande par NX-18. Les panels couvrent `histogram_quantile()` sur `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, avertissements de disponibilite DA (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), gauges de prix/staleness/TWAP/haircut d oracles, et le panel live `iroha_settlement_buffer_xor` pour que les operateurs prouvent les SLO 1 s slot, DA et tresorerie sans requetes bespoke.
- **CI gate:** `scripts/telemetry/check_slot_duration.py` parse des snapshots Prometheus, imprime la latence p50/p95/p99 et applique les seuils NX-18 (p95 <= 1000 ms, p99 <= 1100 ms). Le harness compagnon `scripts/telemetry/nx18_acceptance.py` gate le quorum DA, staleness/TWAP/haircuts d oracles, buffers de settlement et quantiles de slot en un passage (`--json-out` persiste les preuves), et les deux tournent dans `ci/check_nexus_lane_smoke.sh` pour les RC.
- **Bundler d evidence:** `scripts/telemetry/bundle_slot_artifacts.py` copie le snapshot de metrics + resume JSON dans `artifacts/nx18/` et emet `slot_bundle_manifest.json` avec des digests SHA-256, garantissant que chaque RC telecharge exactement les artefacts qui ont declenche le gate NX-18.
- **Automation de release:** `scripts/run_release_pipeline.py` invoque maintenant `ci/check_nexus_lane_smoke.sh` (skip avec `--skip-nexus-lane-smoke`) et copie `artifacts/nx18/` dans la sortie release afin que la preuve NX-18 accompagne les artefacts bundle/image sans etape manuelle.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` documente le flux on-call (seuils, etapes d incident, capture d evidence, drills de chaos) qui accompagne le dashboard, remplissant la puce "publish operator dashboards/runbooks" de NX-18.
- **Helpers telemetrie:** reutiliser `scripts/telemetry/compare_dashboards.py` existant pour diff de dashboards exportes (evitant le drift staging/prod) et `scripts/telemetry/check_nexus_audit_outcome.py` pendant les rehearsals routed-trace ou chaos afin que chaque drill NX-18 archive le payload `nexus.audit.outcome` correspondant.

Questions ouvertes (clarification necessaire)
1) Signatures de transaction: Decision - les utilisateurs finaux sont libres de choisir tout algorithme de signature que leur DS cible annonce (Ed25519, secp256k1, ML-DSA, etc.). Les hosts doivent faire respecter les flags de capacite multisig/curve dans les manifestes, fournir des fallbacks deterministes et documenter les implications de latence lors du mixage d algorithmes. A finaliser: le flux de negotiation de capacites a travers Torii/SDKs et la mise a jour des tests d admission.
2) Economie de gas: Chaque DS peut denominer le gas en un token local, tandis que les fees de settlement globales sont payees en SORA XOR. A finaliser: definir la conversion standard (DEX lane publique vs autres sources de liquidite), les hooks comptables du ledger, et les safeguards pour DS qui subventionnent ou fixent des prix a zero.
3) Attesters DA: Nombre cible par region et seuil (ex., 64 echantillonnes, 43-of-64 signatures ML-DSA-87) pour tenir <=300 ms tout en maintenant la durabilite. Des regions a inclure des le jour un?
4) Parametres DA par defaut: Nous proposons DS public `k=32, m=16` et DS prive `k=16, m=8`. Souhaitez-vous un profil de redondance plus eleve (ex., `k=30, m=20`) pour certaines classes DS?
5) Granularite DS: Les domaines et assets peuvent etre DS. Devons-nous supporter des DS hierarchiques (domain DS comme parent d asset DS) avec heritage optionnel de politiques, ou rester plat pour v1?
6) ISIs lourdes: Pour des ISIs complexes qui ne peuvent pas produire des preuves sous-seconde, devons-nous (a) les rejeter, (b) les diviser en etapes atomiques plus petites entre blocs, ou (c) autoriser une inclusion differee avec flags explicites?
7) Conflits cross-DS: Le jeu lecture/ecriture declare par le client est-il suffisant, ou l host doit-il l inferer et l etendre automatiquement pour securite (au cout de plus de conflits)?

Annexe: Conformite aux politiques du depot
- Norito est utilise pour tous les formats on-wire et la serialisation JSON via les helpers Norito.
- ABI v1 seulement; pas de toggles runtime pour les politiques ABI. Les surfaces syscalls et pointer-types sont fixes et figees par les tests golden.
- Determinisme preserve entre hardware; acceleration optionnelle et gatee.
- Pas de serde en chemins production; pas de configuration basee sur environnement en production.
