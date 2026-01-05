---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77a3a111d5dc132351c92b586389766a2d183c8bb60aed68b18032e10421c92
source_last_modified: "2025-12-04T07:55:53.675646+00:00"
translation_last_reviewed: 2026-01-01
---

Statut: brouillon/esquisse pour accompagner les taches d'implementation de la gouvernance. Les formes peuvent changer pendant l'implementation. Le determinisme et la politique RBAC sont des contraintes normatives; Torii peut signer/soumettre des transactions quand `authority` et `private_key` sont fournis, sinon les clients construisent et soumettent a `/transaction`.

Apercu
- Tous les endpoints renvoient du JSON. Pour les flux qui produisent des transactions, les reponses incluent `tx_instructions` - un tableau d'une ou plusieurs instructions squelette:
  - `wire_id`: identifiant de registre pour le type d'instruction
  - `payload_hex`: bytes de payload Norito (hex)
- Si `authority` et `private_key` sont fournis (ou `private_key` sur les DTO de ballots), Torii signe et soumet la transaction et renvoie quand meme `tx_instructions`.
- Sinon, les clients assemblent une SignedTransaction avec leur authority et chain_id, puis signent et POST vers `/transaction`.
- Couverture SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` renvoie `GovernanceProposalResult` (normalise les champs status/kind), `ToriiClient.get_governance_referendum_typed` renvoie `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` renvoie `GovernanceTally`, `ToriiClient.get_governance_locks_typed` renvoie `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` renvoie `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` renvoie `GovernanceInstancesPage`, imposant un acces type sur toute la surface de gouvernance avec des exemples d'usage dans le README.
- Client Python leger (`iroha_torii_client`): `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` renvoient des bundles types `GovernanceInstructionDraft` (qui encapsulent le squelette `tx_instructions` de Torii), evitant le parsing JSON manuel quand les scripts composent des flux Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expose des helpers types pour proposals, referenda, tallies, locks, unlock stats, et maintenant `listGovernanceInstances(namespace, options)` plus les endpoints council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) afin que les clients Node.js puissent paginer `/v1/gov/instances/{ns}` et piloter des workflows VRF en parallele du listing d'instances de contrats existant.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Requete (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "alice@wonderland?",
      "private_key": "...?"
    }
  - Reponse (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validation: les noeuds canonisent `abi_hash` pour l'`abi_version` fourni et rejettent les incoherences. Pour `abi_version = "v1"`, la valeur attendue est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API contrats (deploy)
- POST `/v1/contracts/deploy`
  - Requete: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - Comportement: calcule `code_hash` depuis le corps du programme IVM et `abi_hash` depuis l'en-tete `abi_version`, puis soumet `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (bytes `.to` complets) pour `authority`.
  - Reponse: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Lie:
    - GET `/v1/contracts/code/{code_hash}` -> renvoie le manifeste stocke
    - GET `/v1/contracts/code-bytes/{code_hash}` -> renvoie `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Requete: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportement: deploie le bytecode fourni et active immediatement le mapping `(namespace, contract_id)` via `ActivateContractInstance`.
  - Reponse: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Service d'alias
- POST `/v1/aliases/voprf/evaluate`
  - Requete: { "blinded_element_hex": "..." }
  - Reponse: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete l'implementation de l'evaluateur. Valeur actuelle: `blake2b512-mock`.
  - Notes: evaluateur mock deterministe qui applique Blake2b512 avec separation de domaine `iroha.alias.voprf.mock.v1`. Prevus pour l'outillage de test jusqu'a ce que le pipeline VOPRF de production soit relie a Iroha.
  - Erreurs: HTTP `400` sur input hex mal forme. Torii renvoie une enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec le message d'erreur du decoder.
- POST `/v1/aliases/resolve`
  - Requete: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Reponse: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - Notes: requiert le runtime ISO bridge staging (`[iso_bridge.account_aliases]` dans `iroha_config`). Torii normalise les alias en retirant les espaces et en mettant en majuscules avant le lookup. Retourne 404 si l'alias est absent et 503 si le runtime ISO bridge est desactive.
- POST `/v1/aliases/resolve_index`
  - Requete: { "index": 0 }
  - Reponse: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - Notes: les index d'alias sont assignes deterministiquement selon l'ordre de configuration (0-based). Les clients peuvent mettre en cache hors ligne pour construire des pistes d'audit pour les evenements d'attestation d'alias.

Cap de taille de code
- Parametre custom: `max_contract_code_bytes` (JSON u64)
  - Controle la taille maximale autorisee (en bytes) pour le stockage de code de contrat on-chain.
  - Default: 16 MiB. Les noeuds rejettent `RegisterSmartContractBytes` lorsque la taille de l'image `.to` depasse le cap avec une erreur d'invariant.
  - Les operateurs peuvent ajuster via `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et un payload numerique.

- POST `/v1/gov/ballots/zk`
  - Requete: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Reponse: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes:
    - Quand les inputs publics du circuit incluent `owner`, `amount` et `duration_blocks`, et que la preuve verifie contre la VK configuree, le noeud cree ou etend un verrou de gouvernance pour `election_id` avec ce `owner`. La direction reste cachee (`unknown`); seuls amount/expiry sont mis a jour. Les re-votes sont monotones: amount et expiry ne font qu'augmenter (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Les re-votes ZK qui tentent de reduire amount ou expiry sont rejetes cote serveur avec des diagnostics `BallotRejected`.
    - L'execution du contrat doit appeler `ZK_VOTE_VERIFY_BALLOT` avant d'enfiler `SubmitBallot`; les hosts imposent un latch a une seule fois.

- POST `/v1/gov/ballots/plain`
  - Requete: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Reponse: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes: les re-votes sont en extension seule - un nouveau ballot ne peut pas reduire l'amount ou l'expiry du verrou existant. Le `owner` doit egaler l'authority de la transaction. La duree minimale est `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Requete: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "alice@wonderland?", "private_key": "...?" }
  - Reponse: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet on-chain (scaffold actuel): enacter une proposition de deploy approuvee insere un `ContractManifest` minimal cle `code_hash` avec l'`abi_hash` attendu et marque la proposition Enacted. Si un manifeste existe deja pour le `code_hash` avec un `abi_hash` different, l'enactment est rejete.
  - Notes: pour les elections ZK, les chemins de contrat doivent appeler `ZK_VOTE_VERIFY_TALLY` avant d'executer `FinalizeElection`; les hosts imposent un latch a une seule fois.

- POST `/v1/gov/enact`
  - Requete: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "...?" }
  - Reponse: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes: Torii soumet la transaction signee quand `authority`/`private_key` sont fournis; sinon il renvoie un squelette pour signature et soumission client. Le preimage est optionnel et informatif pour l'instant.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: id de proposition hex (64 chars)
  - Reponse: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: string id de referendum
  - Reponse: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - Reponse: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes: renvoie le council persiste si present; sinon derive un fallback deterministe avec l'asset de stake configure et les seuils (miroir de la spec VRF jusqu'a ce que des preuves VRF en direct soient persistees on-chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Requete: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement: verifie la preuve VRF de chaque candidat contre l'input canonique derive de `chain_id`, `epoch` et du beacon du dernier hash de block; trie par bytes de sortie desc avec tiebreakers; renvoie les top `committee_size` membres. Ne persiste pas.
  - Reponse: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notes: Normal = pk en G1, proof en G2 (96 bytes). Small = pk en G2, proof en G1 (48 bytes). Les inputs sont separes par domaine et incluent `chain_id`.

### Defaults de gouvernance (iroha_config `gov.*`)

Le council fallback utilise par Torii quand aucun roster persiste n'existe est parametre via `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Overrides d'environnement equivalents:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` limite le nombre de membres fallback renvoyes quand aucun council n'est persiste, `parliament_term_blocks` definit la longueur d'epoque utilisee pour la derivation de seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` impose le minimum de stake (en unites minimales) sur l'asset d'eligibilite, et `parliament_eligibility_asset_id` selectionne quel solde d'asset est scanne lors de la construction du set de candidats.

La verification VK de gouvernance n'a pas de bypass: la verification de ballot requiert toujours une cle `Active` avec bytes inline, et les environnements ne doivent pas s'appuyer sur des toggles de test pour sauter la verification.

RBAC
- L'execution on-chain requiert des permissions:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (futur): `CanManageParliament`

Namespaces proteges
- Parametre custom `gov_protected_namespaces` (JSON array de strings) active le gating d'admission pour les deploys dans les namespaces listes.
- Les clients doivent inclure des cles metadata de transaction pour les deploys vers des namespaces proteges:
  - `gov_namespace`: le namespace cible (ex., "apps")
  - `gov_contract_id`: l'id de contrat logique dans le namespace
- `gov_manifest_approvers`: JSON array optionnel d'account IDs de validateurs. Quand un manifeste de lane declare un quorum > 1, l'admission requiert l'authority de la transaction plus les comptes listes pour satisfaire le quorum du manifeste.
- La telemetrie expose des compteurs d'admission via `governance_manifest_admission_total{result}` afin que les operateurs distinguent les admits reussis des chemins `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- La telemetrie expose le chemin d'enforcement via `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour auditer les approbations manquantes.
- Les lanes appliquent l'allowlist de namespaces publiee dans leurs manifestes. Toute transaction qui fixe `gov_namespace` doit fournir `gov_contract_id`, et le namespace doit apparaitre dans le set `protected_namespaces` du manifeste. Les soumissions `RegisterSmartContractCode` sans cette metadata sont rejetees lorsque la protection est active.
- L'admission impose qu'une proposition de gouvernance Enacted existe pour le tuple `(namespace, contract_id, code_hash, abi_hash)`; sinon la validation echoue avec une erreur NotPermitted.

Hooks de runtime upgrade
- Les manifestes de lane peuvent declarer `hooks.runtime_upgrade` pour gater les instructions de runtime upgrade (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Champs du hook:
  - `allow` (bool, default `true`): quand `false`, toutes les instructions de runtime upgrade sont rejetees.
  - `require_metadata` (bool, default `false`): exige l'entree metadata specifiee par `metadata_key`.
  - `metadata_key` (string): nom metadata applique par le hook. Default `gov_upgrade_id` quand la metadata est requise ou qu'une allowlist est presente.
  - `allowed_ids` (array de strings): allowlist optionnelle de valeurs metadata (apres trim). Rejette quand la valeur fournie n'est pas listee.
- Quand le hook est present, l'admission de la file applique la politique metadata avant l'entree de la transaction dans la file. Metadata manquante, valeurs vides ou hors allowlist produisent une erreur NotPermitted deterministe.
- La telemetrie trace les outcomes via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les transactions satisfaisant le hook doivent inclure la metadata `gov_upgrade_id=<value>` (ou la cle definie par le manifeste) en plus des approbations de validateurs requises par le quorum du manifeste.

Endpoint de commodite
- POST `/v1/gov/protected-namespaces` - applique `gov_protected_namespaces` directement sur le noeud.
  - Requete: { "namespaces": ["apps", "system"] }
  - Reponse: { "ok": true, "applied": 1 }
  - Notes: destine a l'admin/testing; requiert un token API si configure. Pour la production, preferer une transaction signee avec `SetParameter(Custom)`.

Helpers CLI
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - Recupere les instances de contrat pour le namespace et verifie que:
    - Torii stocke le bytecode pour chaque `code_hash`, et son digest Blake2b-32 correspond au `code_hash`.
    - Le manifeste stocke sous `/v1/contracts/code/{code_hash}` rapporte des valeurs `code_hash` et `abi_hash` correspondantes.
    - Une proposition de gouvernance enacted existe pour `(namespace, contract_id, code_hash, abi_hash)` derivee par le meme hashing de proposal-id que le noeud utilise.
  - Sort un rapport JSON avec `results[]` par contrat (issues, resumes de manifest/code/proposal) plus un resume en une ligne sauf suppression (`--no-summary`).
  - Utile pour auditer les namespaces proteges ou verifier les workflows de deploy controles par gouvernance.
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - Emet le squelette JSON de metadata utilise lors des deployments dans des namespaces proteges, incluant `gov_manifest_approvers` optionnels pour satisfaire les regles de quorum du manifeste.
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --salt-hex <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Valide les account ids canoniques, impose des salts non-zero de 32 bytes, et merge les hints dans `public_inputs_json` (avec `--public <path>` pour des overrides supplementaires).
  - Le resume en une ligne expose maintenant un `fingerprint=<hex>` deterministe derive du `CastZkBallot` encode ainsi que les hints decodes (`owner`, `amount`, `duration_blocks`, `direction` si fournis).
  - Les reponses CLI annotent `tx_instructions[]` avec `payload_fingerprint_hex` plus des champs decodes pour que les outils downstream verifient le squelette sans reimplementer le decodage Norito.
  - Fournir les hints de lock permet au noeud d'emettre des events `LockCreated`/`LockExtended` pour les ballots ZK une fois que le circuit expose les memes valeurs.
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les alias `--lock-amount`/`--lock-duration-blocks` refletent les noms de flags ZK pour la parite de scripting.
  - La sortie resume refl ete `vote-zk` en incluant le fingerprint de l'instruction encodee et des champs ballot lisibles (`owner`, `amount`, `duration_blocks`, `direction`, `salt_hex` quand present), pour une confirmation rapide avant signature du squelette.

Listing d'instances
- GET `/v1/gov/instances/{ns}` - liste les instances de contrat actives pour un namespace.
  - Query params:
    - `contains`: filtre par sous-chaine de `contract_id` (case-sensitive)
    - `hash_prefix`: filtre par prefixe hex de `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: un des `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Reponse: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Balayage d'unlocks (Operateur/Audit)
- GET `/v1/gov/unlocks/stats`
  - Reponse: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes: `last_sweep_height` reflete la hauteur de bloc la plus recente ou les locks expires ont ete balayes et persistes. `expired_locks_now` est calcule en scannant les enregistrements de lock avec `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - Requete (DTO style v1):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint_hex": "...64hex?",
      "owner": "alice@wonderland?",
      "salt_hex": "...64hex?"
    }
  - Reponse: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - Accepte un JSON `BallotProof` direct et renvoie un squelette `CastZkBallot`.
  - Requete:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 du conteneur ZK1 ou H2*
        "root_hint": null,                // hex optionnel 32 bytes (root eligibility)
        "owner": null,                    // AccountId optionnel si le circuit commit owner
        "salt": null                      // hex optionnel 32 bytes pour derivation de nullifier
      }
    }
  - Reponse:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notes:
    - Le serveur mappe `root_hint`/`owner`/`salt` optionnels du ballot vers `public_inputs_json` pour `CastZkBallot`.
    - Les bytes de l'envelope sont re-encodes en base64 pour le payload d'instruction.
    - La reponse `reason` passe a `submitted transaction` quand Torii soumet le ballot.
    - Cet endpoint est disponible uniquement si le feature `zk-ballot` est active.

Parcours de verification CastZkBallot
- `CastZkBallot` decode la preuve base64 fournie et rejette les payloads vides ou mal formes (`BallotRejected` avec `invalid or empty proof`).
- Le host resout la cle de verification du ballot depuis le referendum (`vk_ballot`) ou les defaults de gouvernance et exige que l'enregistrement existe, soit `Active` et transporte des bytes inline.
- Les bytes de cle de verification stockes sont re-hashes avec `hash_vk`; tout mismatch de commitment arrete l'execution avant verification pour se proteger des entrees registry corrompues (`BallotRejected` avec `verifying key commitment mismatch`).
- Les bytes de preuve sont dispatches au backend enregistre via `zk::verify_backend`; les transcriptions invalides remontent en `BallotRejected` avec `invalid proof` et l'instruction echoue deterministiquement.
- Les preuves reussies emettent `BallotAccepted`; nullifiers dupliques, roots d'eligibility perimes ou regressions de lock continuent de produire les raisons de rejet existantes decrites plus haut dans ce document.

## Mauvaise conduite des validateurs et consensus joint

### Workflow de slashing et jailing

Le consensus emet `Evidence` encode en Norito lorsqu'un validateur viole le protocole. Chaque payload arrive dans `EvidenceStore` en memoire et, si inedit, est materialise dans la map `consensus_evidence` adossee au WSV. Les enregistrements plus anciens que `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocs) sont rejetes pour garder l'archive bornee, mais le rejet est logge pour les operateurs.

Les offences reconnues se mappent un-a-un sur `EvidenceKind`; les discriminants sont stables et imposes par le data model:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidCommitCertificate,
    EvidenceKind::InvalidProposal,
    EvidenceKind::DoubleExecVote,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - le validateur a signe des hashes en conflit pour le meme tuple `(phase,height,view,epoch)`.
- **DoubleExecVote** - des votes d'execution en conflit annoncent des roots d'etat post differents.
- **InvalidCommitCertificate** - un agregateur a gossip un commit certificate dont la forme echoue aux verifications deterministes (ex., bitmap de signataires vide).
- **InvalidProposal** - un leader a propose un bloc qui echoue la validation structurelle (ex., viole la regle de locked-chain).

Les operateurs et l'outillage peuvent inspecter et re-broadcast les payloads via:

- Torii: `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, et `... submit --evidence-hex <payload>`.

La gouvernance doit traiter les bytes d'evidence comme preuve canonique:

1. **Collecter le payload** avant qu'il expire. Archiver les bytes Norito bruts avec la metadata height/view.
2. **Preparer la penalite** en embarquant le payload dans un referendum ou une instruction sudo (ex., `Unregister::peer`). L'execution re-valide le payload; evidence mal forme ou stale est rejete deterministiquement.
3. **Planifier la topologie de suivi** afin que le validateur fautif ne puisse pas revenir immediatement. Les flux typiques enfilent `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` avec le roster mis a jour.
4. **Auditer les resultats** via `/v1/sumeragi/evidence` et `/v1/sumeragi/status` pour confirmer que le compteur d'evidence a avance et que la gouvernance a applique le retrait.

### Sequencage du consensus joint

Le consensus joint garantit que l'ensemble de validateurs sortant finalise le bloc de frontiere avant que le nouvel ensemble commence a proposer. Le runtime impose la regle via des parametres appaires:

- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` doivent etre commits dans le **meme bloc**. `mode_activation_height` doit etre strictement superieur a la hauteur du bloc qui a porte la mise a jour, donnant au moins un bloc de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) est la garde de configuration qui empeche les hand-offs a lag zero:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Le runtime et la CLI exposent les parametres staged via `/v1/sumeragi/params` et `iroha sumeragi params --summary`, afin que les operateurs confirment les hauteurs d'activation et les rosters de validateurs.
- L'automatisation de gouvernance doit toujours:
  1. Finaliser la decision de retrait (ou reintegration) supportee par evidence.
  2. Enfiler une reconfiguration de suivi avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveiller `/v1/sumeragi/status` jusqu'a ce que `effective_consensus_mode` bascule a la hauteur attendue.

Tout script qui fait tourner les validateurs ou applique un slashing **ne doit pas** tenter une activation a lag zero ou omettre les parametres de hand-off; ces transactions sont rejetees et laissent le reseau dans le mode precedent.

## Surfaces de telemetrie

- Les metriques Prometheus exportent l'activite de gouvernance:
  - `governance_proposals_status{status}` (gauge) suit les compteurs de proposals par status.
  - `governance_protected_namespace_total{outcome}` (counter) incremente quand l'admission des namespaces proteges accepte ou rejette un deploy.
  - `governance_manifest_activations_total{event}` (counter) enregistre les insertions de manifest (`event="manifest_inserted"`) et les bindings de namespace (`event="instance_bound"`).
- `/status` inclut un objet `governance` qui reflete les compteurs de proposals, rapporte les totaux de namespaces proteges et liste les activations recentes de manifest (namespace, contract id, code/ABI hash, block height, activation timestamp). Les operateurs peuvent sonder ce champ pour confirmer que les enactments ont mis a jour les manifests et que les gates de namespaces proteges sont imposes.
- Un template Grafana (`docs/source/grafana_governance_constraints.json`) et le runbook de telemetrie dans `telemetry.md` montrent comment cabler des alertes pour proposals bloquees, activations de manifest manquantes, ou rejets inattendus de namespaces proteges pendant des upgrades runtime.
