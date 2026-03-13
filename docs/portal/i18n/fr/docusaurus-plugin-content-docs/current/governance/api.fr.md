---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Statut : brouillon/esquisse pour accompagner les taches d'implémentation de la gouvernance. Les formes peuvent changer pendant la mise en œuvre. Le déterminisme et la politique RBAC sont des contraintes normatives ; Torii peut signer/soumettre des transactions lorsque `authority` et `private_key` sont fournis, sinon les clients construisent et soumettent a `/transaction`.Apercu
- Tous les endpoints renvoient du JSON. Pour les flux qui produisent des transactions, les réponses incluent `tx_instructions` - un tableau d'une ou plusieurs instructions squelette :
  - `wire_id` : identifiant de registre pour le type d'instruction
  - `payload_hex` : octets de charge utile Norito (hex)
- Si `authority` et `private_key` sont fournis (ou `private_key` sur les DTO de ballots), Torii signe et soumet la transaction et renvoie quand meme `tx_instructions`.
- Sinon, les clients assemblent une SignedTransaction avec leur autorité et chain_id, puis signent et POST vers `/transaction`.
- SDK de couverture :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` renvoie `GovernanceProposalResult` (normaliser le statut/genre des champs), `ToriiClient.get_governance_referendum_typed` renvoie `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` renvoie `GovernanceTally`, `ToriiClient.get_governance_locks_typed` renvoie `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` renvoie `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` renvoie `GovernanceInstancesPage`, imposant un accès de type sur toute la surface de gouvernance avec des exemples d'utilisation dans le README.
- Client Python léger (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` renvoient des bundles types `GovernanceInstructionDraft` (qui encapsulent le squelette `tx_instructions` de Torii), évitant le parsing JSON manuel quand les scripts composant des flux Finalize/Enact.- JavaScript (`@iroha/iroha-js`) : `ToriiClient` expose les types d'aides pour les propositions, les référendums, les décomptes, les verrous, les statistiques de déverrouillage, et maintenant `listGovernanceInstances(namespace, options)` plus les endpoints Council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) afin que les clients Node.js puissent paginer `/v2/gov/instances/{ns}` et piloter des workflows VRF en parallèle du listing d'instances de contrat existant.

Points de terminaison

- POSTE `/v2/gov/proposals/deploy-contract`
  - Requête (JSON) :
    {
      "espace de noms": "applications",
      "contract_id": "mon.contrat.v1",
      "code_hash": "blake2b32 :..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "fenêtre": { "inférieur": 12345, "supérieur": 12400 },
      "autorité": "i105…?",
      "private_key": "...?"
    }
  - Réponse (JSON) :
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validation : les noeuds canonisent `abi_hash` pour l'`abi_version` fourni et rejettent les incohérences. Pour `abi_version = "v1"`, la valeur attendue est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.Contrats API (déploiement)
- POSTE `/v2/contracts/deploy`
  - Requête : { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportement : calcule `code_hash` depuis le corps du programme IVM et `abi_hash` depuis l'en-tête `abi_version`, puis soumis `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (bytes `.to` complets) pour `authority`.
  - Réponse : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Mentir :
    - GET `/v2/contracts/code/{code_hash}` -> renvoyer le manifeste stocke
    - OBTENIR `/v2/contracts/code-bytes/{code_hash}` -> renvoyer `{ code_b64 }`
- POSTE `/v2/contracts/instance`
  - Requête : { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportement : déployer le bytecode fourni et activer immédiatement le mapping `(namespace, contract_id)` via `ActivateContractInstance`.
  - Réponse : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Service d'alias
- POSTE `/v2/aliases/voprf/evaluate`
  - Requête : { "blinded_element_hex": "..." }
  - Réponse : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflète l'implémentation de l'évaluateur. Valeur actuelle : `blake2b512-mock`.
  - Notes : évaluateur mock déterministe qui applique Blake2b512 avec séparation de domaine `iroha.alias.voprf.mock.v1`. Prevus pour l'outillage de test jusqu'à ce que le pipeline VOPRF de production soit reposé sur Iroha.
  - Erreurs : HTTP `400` sur input hex mal forme. Torii renvoie une enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec le message d'erreur du décodeur.
- POSTE `/v2/aliases/resolve`
  - Requête : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Réponse : { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notes : nécessite le runtime ISO bridge staging (`[iso_bridge.account_aliases]` dans `iroha_config`). Torii normalise les alias en retirant les espaces et en mettant en majuscules avant le lookup. Retourne 404 si l'alias est absent et 503 si le runtime ISO bridge est désactivé.
- POSTE `/v2/aliases/resolve_index`
  - Requête : { "index": 0 }
  - Réponse : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }- Notes : les index d'alias sont attribués deterministiquement selon l'ordre de configuration (basé sur 0). Les clients peuvent mettre en cache hors ligne pour construire des pistes d'audit pour les événements d'attestation d'alias.

Cap de taille de code
- Paramètre personnalisé : `max_contract_code_bytes` (JSON u64)
  - Contrôler la taille maximale autorisée (en octets) pour le stockage de code de contrat en chaîne.
  - Par défaut : 16 Mio. Les noeuds rejettent `RegisterSmartContractBytes` lorsque la taille de l'image `.to` dépasse le cap avec une erreur d'invariant.
  - Les opérateurs peuvent régler via `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et un payload numérique.- POSTE `/v2/gov/ballots/zk`
  - Requête : { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques :
    - Quand les entrées publiques du circuit incluent `owner`, `amount` et `duration_blocks`, et que la preuve vérifie contre la VK configurée, le noeud crée ou étend un verrou de gouvernance pour `election_id` avec ce `owner`. La direction reste cachée (`unknown`) ; seuls montant/expiration sont mis à jour. Les re-votes sont monotones : montant et expiration ne font qu'augmenter (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Les re-votes ZK qui tentent de réduire le montant ou l'expiration sont rejetées côté serveur avec des diagnostics `BallotRejected`.
    - L'exécution du contrat doit appeler `ZK_VOTE_VERIFY_BALLOT` avant d'enfiler `SubmitBallot` ; les hôtes imposent un verrou une seule fois.- POSTE `/v2/gov/ballots/plain`
  - Requête : { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes : les re-votes sont en extension seule - un nouveau scrutin ne peut pas réduire le montant ou l'expiration du verrou existant. Le `owner` doit égaler l'autorité de la transaction. La durée minimale est `conviction_step_blocks`.- POSTE `/v2/gov/finalize`
  - Requete : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet on-chain (échafaudage actuel) : enacter une proposition de déploiement approuvee insere un `ContractManifest` minimal cle `code_hash` avec l'`abi_hash` attendu et marque la proposition Enacted. Si un manifeste existe déjà pour le `code_hash` avec un `abi_hash` différent, l'enactment est rejetée.
  - Remarques :
    - Pour les élections ZK, les chemins de contrat doivent appeler `ZK_VOTE_VERIFY_TALLY` avant d'exécuter `FinalizeElection` ; les hôtes imposent un latch à usage unique. `FinalizeReferendum` rejette les référendums ZK tant que le décompte n'est pas finalisé.
    - La clôture automatique a `h_end` emet Approved/Rejected uniquement pour les référendums Plain; les référendums ZK restent fermés jusqu'à ce qu'un décompte final soit soumis et que `FinalizeReferendum` soit exécuté.
    - Les vérifications de participation utilisent seulement approuver+rejeter; abstain ne compte pas pour le taux de participation.- POSTE `/v2/gov/enact`
  - Requete : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes : Torii soumet la transaction signée lorsque `authority`/`private_key` sont fournis ; sinon il renvoie un squelette pour signature et soumission client. Le préimage est optionnel et informatif pour l'instant.

- OBTENIR `/v2/gov/proposals/{id}`
  - Chemin `{id}` : id de proposition hex (64 caractères)
  - Réponse : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v2/gov/locks/{rid}`
  - Chemin `{rid}` : identifiant de chaîne du référendum
  - Réponse : { "found": bool, "referendum_id": "rid", "locks": { ... } ? }

- OBTENIR `/v2/gov/council/current`
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes : renvoyer le conseil persiste si présent ; sinon dérivez un déterministe de secours avec l'actif de mise configuré et les seuils (miroir de la spec VRF jusqu'à ce que les preuves VRF en direct soient persistantes en chaîne).- POST `/v2/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Requete : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement : vérifier la preuve VRF de chaque candidat contre l'entrée canonique dérivée de `chain_id`, `epoch` et du beacon du dernier hash de block ; trie par octets de sortie desc avec tiebreakers; renvoient les meilleurs membres `committee_size`. Ne persiste pas.
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "vérified": K }
  - Notes : Normal = pk en G1, preuve en G2 (96 octets). Petit = pk en G2, preuve en G1 (48 octets). Les entrées sont séparées par domaine et incluent `chain_id`.

### Defaults de gouvernance (iroha_config `gov.*`)

Le conseil de secours utilise par Torii lorsqu'aucun roster persiste n'existe est paramètre via `iroha_config` :

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

Équivalents des dérogations d'environnement :

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

`parliament_committee_size` limite le nombre de membres fallback renvoyes quand aucun conseil n'est persiste, `parliament_term_blocks` définit la longueur d'époque utilisée pour la dérivation de seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` impose le minimum de mise (en unités minimales) sur l'actif d'éligibilité, et `parliament_eligibility_asset_id` sélectionne quelle vente d'actif est scannée lors de la construction du set de candidats.La vérification VK de gouvernance n'a pas de bypass : la vérification de vote requiert toujours une clé `Active` avec bytes inline, et les environnements ne doivent pas s'appuyer sur des bascules de test pour sauter la vérification.

RBAC
- L'exécution en chaîne nécessite des autorisations :
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (future) : `CanManageParliament`Espaces de noms protégés
- Paramètre personnalisé `gov_protected_namespaces` (JSON array de strings) actif le gate d'admission pour les déploiements dans les listes d'espaces de noms.
- Les clients doivent inclure des clés de métadonnées de transaction pour les déploiements vers des espaces de noms protégés :
  - `gov_namespace` : l'espace de noms cible (ex., "apps")
  - `gov_contract_id` : l'id de contrat logique dans le namespace
- `gov_manifest_approvers` : Tableau JSON optionnel d'ID de compte de validateurs. Lorsqu'un manifeste de voie déclare un quorum > 1, l'admission requiert l'autorité de la transaction plus les comptes listes pour satisfaire le quorum du manifeste.
- La télémétrie expose des compteurs d'admission via `governance_manifest_admission_total{result}` afin que les opérateurs distinguent les admissions reussis des chemins `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- La télémétrie expose le chemin d'application via `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour auditer les approbations manquantes.
- Les voies appliquent l'allowlist de namespaces publiée dans leurs manifestes. Toute transaction qui fixe `gov_namespace` doit fournir `gov_contract_id`, et le namespace doit apparaître dans l'ensemble `protected_namespaces` du manifeste. Les soumissions `RegisterSmartContractCode` sans cette métadonnée sont rejetées lorsque la protection est active.- L'admission impose qu'une proposition de gouvernance Enacted existe pour le tuple `(namespace, contract_id, code_hash, abi_hash)` ; sinon la validation échoue avec une erreur NotPerMIT.

Hooks de mise à niveau du runtime
- Les manifestes de la voie peuvent déclarer `hooks.runtime_upgrade` pour gater les instructions de mise à niveau du runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Champs du crochet :
  - `allow` (bool, par défaut `true`) : quand `false`, toutes les instructions de mise à niveau du runtime sont rejetées.
  - `require_metadata` (bool, par défaut `false`) : exige l'entrée des métadonnées spécifiées par `metadata_key`.
  - `metadata_key` (string) : nom de métadonnées appliqué par le hook. Par défaut `gov_upgrade_id` lorsque les métadonnées sont requises ou qu'une liste d'autorisation est présente.
  - `allowed_ids` (array de strings) : liste autorisée optionnelle de métadonnées de valeurs (après trim). Rejette quand la valeur fournie n'est pas répertoriée.
- Quand le crochet est présent, l'admission du fichier applique la politique métadonnées avant l'entrée de la transaction dans le fichier. Metadata manquante, valeurs vides ou hors liste autorisée produisent une erreur NotPermitte déterministe.
- La télémétrie trace les résultats via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les transactions satisfaisant le hook doivent inclure la métadonnée `gov_upgrade_id=<value>` (ou la clé définie par le manifeste) en plus des approbations de validateurs requises par le quorum du manifeste.Endpoint de marchandise
- POST `/v2/gov/protected-namespaces` - applique `gov_protected_namespaces` directement sur le noeud.
  - Requête : { "espaces de noms": ["apps", "system"] }
  - Réponse : { "ok" : vrai, "appliqué" : 1 }
  - Notes : destiné à l'admin/testing ; nécessite une API de jeton à configurer. Pour la production, préférez une transaction signée avec `SetParameter(Custom)`.CLI d'assistance
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Récupérez les instances de contrat pour le namespace et vérifiez que :
    - Torii stocke le bytecode pour chaque `code_hash`, et son digest Blake2b-32 correspond au `code_hash`.
    - Le manifeste stocke sous `/v2/contracts/code/{code_hash}` rapport des valeurs `code_hash` et `abi_hash` correspondantes.
    - Une proposition de gouvernance édictée existe pour `(namespace, contract_id, code_hash, abi_hash)` dérivée par le même hachage de proposition-id que le noeud utilise.
  - Trier un rapport JSON avec `results[]` par contrat (issues, CV de manifest/code/proposal) plus un CV en une ligne sauf suppression (`--no-summary`).
  - Utile pour auditer les espaces de noms protégés ou vérifier les workflows de déploiement de contrôles par gouvernance.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emet le squelette JSON de métadonnées utilisé lors des déploiements dans des espaces de noms protégés, incluant les options `gov_manifest_approvers` pour satisfaire les règles de quorum du manifeste.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — les astuces de verrouillage sont requises lorsque `min_bond_amount > 0`, et tout ensemble de astuces fournies doit inclure `owner`, `amount` et `duration_blocks`.
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).- L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.
  - Le CV en une ligne expose maintenant un `fingerprint=<hex>` déterministe dérive du `CastZkBallot` encode ainsi que les astuces decodes (`owner`, `amount`, `duration_blocks`, `direction` si fournis).
  - Les réponses CLI annotent `tx_instructions[]` avec `payload_fingerprint_hex` plus des champs décodés pour que les outils en aval vérifient le squelette sans réimplémenter le décodage Norito.
  - Fournir les indices de verrouillage permet au noeud d'émettre des événements `LockCreated`/`LockExtended` pour les bulletins de vote ZK une fois que le circuit expose les mèmes valeurs.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les alias `--lock-amount`/`--lock-duration-blocks` reflètent les noms de flags ZK pour la parité de script.
  - La sortie curriculum vitae reflète l'empreinte `vote --mode zk` en incluant l'empreinte digitale de l'instruction encodee et des champs de vote lisibles (`owner`, `amount`, `duration_blocks`, `direction`), pour une confirmation rapide avant la signature du squelette.Listing d'instances
- GET `/v2/gov/instances/{ns}` - liste les instances de contrat actives pour un espace de noms.
  - Paramètres de requête :
    - `contains` : filtre par sous-chaine de `contract_id` (sensible à la casse)
    - `hash_prefix` : filtre par préfixe hex de `code_hash_hex` (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : un des `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`
  - Réponse : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK Helper : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Balayage d'unlocks (Opérateur/Audit)
- OBTENIR `/v2/gov/unlocks/stats`
  - Réponse : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes : `last_sweep_height` reflète la hauteur de bloc la plus récente ou les serrures expirent sur été balayées et persistent. `expired_locks_now` est calculé en scannant les enregistrements de verrouillage avec `expiry_height <= height_current`.
- POSTE `/v2/gov/ballots/zk-v1`
  - Requête (style DTO v1) :
    {
      "autorité": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "enveloppe_b64": "AAECAwQ=",
      "root_hint": "0x...64hex ?",
      "propriétaire": "i105…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }- POST `/v2/gov/ballots/zk-v1/ballot-proof` (fonctionnalité : `zk-ballot`)
  - Acceptez un JSON `BallotProof` direct et renvoyez un squelette `CastZkBallot`.
  - Demander :
    {
      "autorité": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 du conteneur ZK1 ou H2*
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // AccountId optionnel si le circuit commit propriétaire
        "nullifier": null // chaîne hexadécimale facultative de 32 octets (indice d'annulation)
      }
    }
  - Réponse :
    {
      "ok" : vrai,
      "accepté": vrai,
      "reason": "construire le squelette de la transaction",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Remarques :
    - Le serveur mappe `root_hint`/`owner`/`nullifier` optionnels du vote vers `public_inputs_json` pour `CastZkBallot`.
    - Les octets de l'enveloppe sont ré-encodés en base64 pour le payload d'instruction.
    - La réponse `reason` passe à `submitted transaction` lorsque Torii soumet le bulletin de vote.
    - Ce point de terminaison est disponible uniquement si la fonctionnalité `zk-ballot` est active.Parcours de vérification CastZkBallot
- `CastZkBallot` décode la preuve base64 fournie et rejette les payloads vides ou mal formes (`BallotRejected` avec `invalid or empty proof`).
- Le host résout la clé de vérification du scrutin depuis le référendum (`vk_ballot`) ou les défauts de gouvernance et exige que l'enregistrement existe, soit `Active` et transporte des octets en ligne.
- Les octets de clé de vérification stockes sont re-hashes avec `hash_vk` ; tout mismatch de engagement arrête l'exécution avant vérification pour se protéger des entrées de registre corrompues (`BallotRejected` avec `verifying key commitment mismatch`).
- Les octets de preuve sont envoyés au backend enregistré via `zk::verify_backend` ; les transcriptions invalides remontent en `BallotRejected` avec `invalid proof` et l'instruction échoue déterministiquement.
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- Les preuves reussies emettent `BallotAccepted`; annulateurs dupliques, racines d'éligibilité perimes ou régressions de verrouillage continuent de produire les raisons de rejet existantes décrites plus haut dans ce document.

## Mauvaise conduite des validateurs et consensus conjoints

### Workflow de slashing et jailingLe consensus emet `Evidence` encode en Norito lorsqu'un validateur viole le protocole. Chaque charge utile arrive dans `EvidenceStore` en mémoire et, si elle n'est pas modifiée, est matérialisée dans la carte `consensus_evidence` adossee au WSV. Les enregistrements plus anciens que `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocs `7200` par défaut) sont rejetés pour garder l'archive bornee, mais le rejet est loggé pour les opérateurs. Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

Les infractions reconnues se mappent un-à-un sur `EvidenceKind`; les discriminants sont stables et imposent par le modèle de données :

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - le validateur a signe des hashes en conflit pour le meme tuple `(phase,height,view,epoch)`.
- **InvalidQc** - un agrégateur a gossip un commit certificate dont la forme échoue aux vérifications déterministes (ex., bitmap de signataires vide).
- **InvalidProposal** - un leader propose un bloc qui fait écho à la validation structurelle (ex., violer la règle de lock-chain).
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

Les opérateurs et l'outillage peuvent inspecter et rediffuser les charges utiles via :- Torii : `GET /v2/sumeragi/evidence` et `GET /v2/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count`, et `... submit --evidence-hex <payload>`.

La gouvernance doit traiter les octets de preuve comme preuve canonique :

1. **Collecter le payload** avant qu'il expire. Archiver les octets Norito bruts avec la hauteur/vue des métadonnées.
2. **Préparer la pénalité** en embarquant le payload dans un référendum ou une instruction sudo (ex., `Unregister::peer`). L'exécution revalide le payload ; preuve mal forme ou stale est rejetée déterministiquement.
3. **Planifier la topologie de suivi** afin que le validateur fautif ne puisse pas revenir immédiatement. Les flux typiques enfilent `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` avec le roster mis à jour.
4. **Auditer les résultats** via `/v2/sumeragi/evidence` et `/v2/sumeragi/status` pour confirmer que le compteur d'évidence a avancé et que la gouvernance a appliqué le retrait.

### Séquence du consensus conjoint

Le consensus conjoint garantit que l'ensemble des validateurs sortant finalise le bloc de frontière avant que le nouvel ensemble commence à proposer. Le runtime impose la règle via des paramètres apparents :- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` doivent être commits dans le **meme bloc**. `mode_activation_height` doit être strictement supérieur à la hauteur du bloc qui a porte la mise à jour, donnant au moins un bloc de décalage.
- `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) est la garde de configuration qui empêche les transferts avec un décalage zéro :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Le runtime et la CLI exposent les paramètres mis en scène via `/v2/sumeragi/params` et `iroha --output-format text ops sumeragi params`, afin que les opérateurs confirment les hauteurs d'activation et les rosters de validateurs.
- L'automatisation de gouvernance doit toujours :
  1. Finaliser la décision de retrait (ou réintégration) appuyée par preuve.
  2. Enfiler une reconfiguration de suivi avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveiller `/v2/sumeragi/status` jusqu'à ce que `effective_consensus_mode` bascule à la hauteur attendue.

Tout script qui fait tourner les validateurs ou applique un slashing **ne doit pas** tenter une activation à un lag zéro ou omettre les paramètres de hand-off; ces transactions sont rejetées et laissent le réseau dans le mode précédent.

## Surfaces de télémétrie- Les métriques Prometheus exportent l'activité de gouvernance :
  - `governance_proposals_status{status}` (jauge) convient aux compteurs de propositions par statut.
  - `governance_protected_namespace_total{outcome}` (counter) incrémente lorsque l'admission des espaces de noms protégés accepte ou rejette un déploiement.
  - `governance_manifest_activations_total{event}` (compteur) enregistre les insertions de manifest (`event="manifest_inserted"`) et les liaisons de namespace (`event="instance_bound"`).
- `/status` inclut un objet `governance` qui reflète les compteurs de propositions, rapporte les totaux d'espaces de noms protégés et liste les activations récentes de manifeste (espace de noms, identifiant de contrat, hachage code/ABI, hauteur de bloc, horodatage d'activation). Les opérateurs peuvent sonder ce champ pour confirmer que les textes ont mis à jour les manifestes et que les portes de namespaces protégés sont imposées.
- Un template Grafana (`docs/source/grafana_governance_constraints.json`) et le runbook de télémétrie dans `telemetry.md` montrent comment câbler des alertes pour des propositions bloquées, des activations de manifeste manquantes, ou des rejets inattendus de namespaces protégés pendant les mises à niveau runtime.