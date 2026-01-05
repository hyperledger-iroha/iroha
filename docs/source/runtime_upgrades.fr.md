---
lang: fr
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee7142a82f21e646d4d71844adbf779d180e5647
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# Mises a jour runtime (IVM + Host) - Sans downtime, sans hardfork

Ce document specifie un mecanisme deterministe, controle par la gouvernance, pour deployer des
mises a jour runtime sans arreter le reseau ni hardforker les noeuds. Les noeuds deploient les
binaires a l avance; l activation est coordonnee on-chain dans une fenetre de hauteur bornee.
Les anciens contrats continuent a s executer sans changement; la surface ABI du host reste fixe en v1.

Note (premiere version): ABI v1 est fixe et aucun bump de version ABI n est prevu. Les manifests de runtime upgrade doivent definir `abi_version = 1`, et `added_syscalls`/`added_pointer_types` doivent etre vides.

Objectifs
- Activation deterministe dans une fenetre de hauteur planifiee avec application idempotente.
- Preserver la stabilite ABI v1; les mises a jour runtime ne changent pas la surface ABI du host.
- Garde-fous d admission et d execution pour que les payloads pre-activation ne puissent pas
  activer un nouveau comportement.
- Deploiement convivial pour les operateurs avec visibilite des capacites et modes de panne clairs.

Non-objectifs
- Introduire de nouvelles versions ABI ou etendre les surfaces de syscalls/types de pointeur (hors scope pour cette version).
- Modifier les numeros de syscalls existants ou les IDs de types de pointeur (interdit).
- Patcher les noeuds en direct sans deployer des binaires a jour.

Definitions
- Version ABI: petit entier declare dans `ProgramMetadata.abi_version` qui selectionne une
  `SyscallPolicy` et une allowlist de types de pointeur. Dans la premiere version, c est fixe a `1`.
- Hash ABI: digest deterministe de la surface ABI pour une version donnee: liste de syscalls
  (numeros+formes), IDs/allowlist de types de pointeur, et flags de politique; calcule par
  `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: mapping host qui decide si un numero de syscall est autorise pour une version ABI
  donnee et une politique host.
- Activation Window: intervalle semi-ouvert de hauteur de bloc `[start, end)` dans lequel
  l activation est valide exactement une fois a `start`.

Objets d etat (Modele de donnees)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 des bytes Norito canoniques d un manifest.
- Champs de `RuntimeUpgradeManifest`:
  - `name: String` - libelle lisible.
  - `description: String` - description courte pour les operateurs.
  - `abi_version: u16` - version ABI cible a activer (doit etre 1 en premiere version).
  - `abi_hash: [u8; 32]` - hash ABI canonique pour la politique cible.
  - `added_syscalls: Vec<u16>` - numeros de syscalls qui deviennent valides avec cette version.
  - `added_pointer_types: Vec<u16>` - identifiants de types de pointeur ajoutes par la mise a jour.
  - `start_height: u64` - premiere hauteur de bloc ou l activation est permise.
  - `end_height: u64` - borne superieure exclusive de la fenetre d activation.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` - digests SBOM pour les artefacts de mise a jour.
  - `slsa_attestation: Vec<u8>` - bytes bruts d attestation SLSA (base64 en JSON).
  - `provenance: Vec<ManifestProvenance>` - signatures sur le payload canonique.
- Champs de `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` - payload canonique de proposition.
  - `status: RuntimeUpgradeStatus` - etat du cycle de vie de la proposition.
  - `proposer: AccountId` - autorite qui a soumis la proposition.
  - `created_height: u64` - hauteur de bloc ou la proposition entre dans le ledger.
- Champs de `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` - identifiant d algorithme de digest.
  - `digest: Vec<u8>` - bytes bruts du digest (base64 en JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - Invariants: `end_height > start_height`; `abi_version` doit etre `1`; `abi_hash` doit egaler
    `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`; `added_*` doit etre vide; les numeros/IDs
    existants NE DOIVENT PAS etre supprimes ou renumerotes.

Disposition de stockage
- `world.runtime_upgrades`: map MVCC clee par `RuntimeUpgradeId.0` (hash brut 32 bytes) avec des valeurs
  encodees comme payloads Norito canoniques `RuntimeUpgradeRecord`. Les entrees persistent entre blocs;
  les commits sont idempotents et safe face aux replays.

Instructions (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Effets: insere `RuntimeUpgradeRecord { status: Proposed }` cle `RuntimeUpgradeId` si absent.
  - Rejeter si la fenetre chevauche un autre record Proposed/Activated ou si les invariants echouent.
  - Idempotent: re-soumettre les memes bytes canoniques du manifest ne fait rien.
  - Encodage canonique: les bytes du manifest doivent correspondre a `RuntimeUpgradeManifest::canonical_bytes()`;
    les encodages non canoniques sont rejetes.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: un record Proposed correspondant existe; `current_height` doit egaler `manifest.start_height`;
    `current_height < manifest.end_height`.
  - Effets: bascule le record en `ActivatedAt(current_height)`; l ensemble ABI actif reste `{1}` en premiere version.
  - Idempotent: replays a la meme hauteur sont des no-ops; autres hauteurs rejetees deterministiquement.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: le statut est Proposed et `current_height < manifest.start_height`.
  - Effets: bascule en `Canceled`.

Evenements (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Regles d admission
- Admission des contrats: pour la premiere version, seul `ProgramMetadata.abi_version = 1` est accepte; les autres valeurs sont rejetees avec `IvmAdmissionError::UnsupportedAbiVersion`.
  - Pour ABI v1, recomputer `abi_hash(1)` et exiger l egalite avec le payload/manifest lorsque fourni; sinon rejeter avec
    `IvmAdmissionError::ManifestAbiHashMismatch`.
- Admission des transactions: les instructions `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade`
  requierent les permissions appropriees (root/sudo); doivent respecter les contraintes de chevauchement de fenetres.

Application de provenance
- Les manifests de runtime upgrade peuvent porter des digests SBOM (`sbom_digests`), des bytes d attestation SLSA
  (`slsa_attestation`), et des metadonnees de signataires (signatures `provenance`). Les signatures couvrent le
  `RuntimeUpgradeManifestSignaturePayload` canonique (tous les champs du manifest sauf la liste de signatures `provenance`).
- La configuration de gouvernance controle l application sous `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (accepte une provenance manquante, verifie si presente) ou `required` (rejette si absente).
  - `require_sbom`: quand `true`, au moins un digest SBOM est requis.
  - `require_slsa`: quand `true`, une attestation SLSA non vide est requise.
  - `trusted_signers`: liste de cles publiques de signataires approuves.
  - `signature_threshold`: nombre minimum de signatures de confiance requises.
- Les rejets de provenance exposent des codes d erreur stables dans les echec d instructions (prefixe `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetrie: `runtime_upgrade_provenance_rejections_total{reason}` compte les raisons de rejet de provenance.

Regles d execution
- Politique host VM: pendant l execution du programme, derivez `SyscallPolicy` depuis `ProgramMetadata.abi_version`.
  Les syscalls inconnus pour cette version mappent vers `VMError::UnknownSyscall`.
- Pointer-ABI: allowlist derivee de `ProgramMetadata.abi_version`; les types hors allowlist pour cette version
  sont rejetes pendant decode/validation.
- Changement de host: chaque bloc recompute l ensemble ABI actif; en premiere version il reste `{1}`, mais
  l activation est enregistree et idempotente (valide par `runtime_upgrade_admission::activation_allows_v1_in_same_block`).
  - Liaison de politique de syscalls: `CoreHost` lit la version ABI declaree par la transaction et applique
    `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` contre le `SyscallPolicy` par bloc. Le host
    reutilise l instance VM scopee par transaction, donc les activations en milieu de bloc sont sures - les
    transactions ulterieures observent la politique mise a jour tandis que les precedentes continuent avec leur version d origine.

Invariants de determinisme et de securite
- L activation se produit uniquement a `start_height` et est idempotente; les reorgs en dessous de `start_height`
  reappliquent deterministiquement une fois que le bloc retombe.
- L ensemble ABI actif est fixe a `{1}` en premiere version.
- Aucune negociation dynamique n influence le consensus ou l ordre d execution; le gossip de capacites est
  uniquement informatif.

Deploiement operateur (sans downtime)
1) Deployer un binaire de noeud qui inclut le nouvel artefact runtime tout en gardant ABI v1.
2) Observer la disponibilite de la flotte via la telemetrie.
3) Soumettre `ProposeRuntimeUpgrade` avec une fenetre suffisamment en avance (par ex. `H+N`).
4) A `start_height`, `ActivateRuntimeUpgrade` s execute dans le bloc inclus et enregistre l activation; l ABI reste v1.

Torii et CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (implante)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implante)
  - `GET /v1/runtime/upgrades` -> liste des records (implante).
  - `POST /v1/runtime/upgrades/propose` -> encapsule `ProposeRuntimeUpgrade` (retourne un squelette d instruction; implante).
  - `POST /v1/runtime/upgrades/activate/:id` -> encapsule `ActivateRuntimeUpgrade` (retourne un squelette d instruction; implante).
  - `POST /v1/runtime/upgrades/cancel/:id` -> encapsule `CancelRuntimeUpgrade` (retourne un squelette d instruction; implante).
- CLI
  - `iroha runtime abi active` (implante)
  - `iroha runtime abi hash` (implante)
  - `iroha runtime upgrade list` (implante)
  - `iroha runtime upgrade propose --file <manifest.json>` (implante)
  - `iroha runtime upgrade activate --id <id>` (implante)
  - `iroha runtime upgrade cancel --id <id>` (implante)

API de requete core
- Requete Norito singuliere (signee):
  - `FindActiveAbiVersions` renvoie une struct Norito `{ active_versions: [u16], default_compile_target: u16 }`.
  - Voir exemple: `docs/source/samples/find_active_abi_versions.md` (type/champs et exemple JSON).

Notes d implementation (v1 seulement)
- iroha_data_model
  - Ajouter `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, enums d instructions, evenements, et codecs JSON/Norito avec tests de roundtrip.
- iroha_core
  - WSV: ajouter le registre `runtime_upgrades` avec checks de chevauchement et getters.
  - Executors: implementer les handlers ISI; emettre des evenements; appliquer les regles d admission.
  - Admission: gate des manifests de programme par activite de `abi_version` et egalite de `abi_hash`.
  - Mapping de politique de syscalls: passer l ensemble ABI actif au constructeur du host VM; garantir le determinisme
    en utilisant la hauteur de bloc au debut de l execution.
  - Tests: idempotence de fenetre d activation, rejets de chevauchement, comportement d admission pre/post.
- ivm
  - La surface ABI est fixe en v1; les listes de syscalls et hashes ABI sont figees par les tests golden.
- iroha_cli / iroha_torii
  - Ajouter les endpoints et commandes listes ci-dessus; helpers Norito JSON pour manifests; tests d integration basiques.
- Kotodama compiler
  - Emet `abi_version = 1` et insere le `abi_hash` canonique v1 dans les manifests `.to`.

Telemetrie
- Ajouter la gauge `runtime.active_abi_versions` et le counter `runtime.upgrade_events_total{kind}`.

Considerations de securite
- Seul root/sudo peut proposer/activer/annuler; les manifests doivent etre correctement signes.
- Les fenetres d activation previennent le front-running et assurent une application deterministe.
- `abi_hash` fixe la surface d interface pour eviter une derive silencieuse entre binaires.

Criteres d acceptation (Conformance)
- Les noeuds rejettent deterministiquement le code avec `abi_version != 1` en tout temps.
- Les mises a jour runtime ne changent pas la politique ABI; les programmes existants continuent a s executer sans changement en v1.
- Les tests golden pour les hashes ABI et listes de syscalls passent sur x86-64/ARM64.
- L activation est idempotente et sure sous reorgs.
