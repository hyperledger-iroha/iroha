---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

État : entrepreneur/boceto pour accompagner les tâches de mise en œuvre de la gouvernance. Les formes peuvent être modifiées lors de la mise en œuvre. Le déterminisme et la politique RBAC sont des restrictions normatives ; Torii peut confirmer/envoyer des transactions lorsque vous proposez `authority` et `private_key`, de l'autre côté les clients construisent et envoient un `/transaction`.CV
- Tous les points de terminaison ont développé JSON. Pour les transactions qui produisent des transactions, les réponses incluent `tx_instructions` - un arrangement d'une ou de plus d'instructions telles que :
  - `wire_id` : identifiant d'enregistrement pour le type d'instruction
  - `payload_hex` : octets de charge utile Norito (hex)
- Si vous fournissez `authority` et `private_key` (ou `private_key` en DTOs de bulletins de vote), Torii ferme et envoyez la transaction et aun devuelve `tx_instructions`.
- De l'autre côté, les clients utilisent une SignedTransaction en utilisant leur autorité et leur chain_id, puis ils font POST à ​​`/transaction`.
- Couverture du SDK :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` développé `GovernanceProposalResult` (état/type de champ normal), `ToriiClient.get_governance_referendum_typed` développé `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` développé `GovernanceTally`, `ToriiClient.get_governance_locks_typed` devuelve `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` devuelve `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` devuelve `GovernanceInstancesPage`, imponiendo acceso tipado fr toute la surface de gouvernance avec des exemples d'utilisation dans le fichier README.
- Client léger Python (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` ont développé des bundles de type `GovernanceInstructionDraft` (en impliquant le modèle `tx_instructions` de Torii), évitant l'analyse Manuel JSON lorsque les scripts composent les flux Finalize/Enact.- JavaScript (`@iroha/iroha-js`) : `ToriiClient` expose les aides indiquées pour les propositions, les référendums, les décomptes, les verrous, les statistiques de déverrouillage, et maintenant `listGovernanceInstances(namespace, options)` plus les points finaux du conseil (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) pour que les clients Node.js puissent paginer `/v1/gov/instances/{ns}` et effectuer des réponses au VRF conjointement avec la liste des instances de contrat existantes.

Points de terminaison

- POSTE `/v1/gov/proposals/deploy-contract`
  - Sollicitation (JSON) :
    {
      "espace de noms": "applications",
      "contract_id": "mon.contrat.v1",
      "code_hash": "blake2b32 :..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "fenêtre": { "inférieur": 12345, "supérieur": 12400 },
      "autorité": "ih58…?",
      "private_key": "...?"
    }
  - Réponse (JSON) :
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validation : los nodos canonizan `abi_hash` para el `abi_version` provisto and rechazan desajustes. Pour `abi_version = "v1"`, la valeur attendue est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contrats (déployer)
- POSTE `/v1/contracts/deploy`
  - Sollicitude : { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: calcule `code_hash` du corps du programme IVM et `abi_hash` de l'en-tête `abi_version`, puis envoie `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (octets `.to` complets) en numéro de `authority`.
  - Réponse : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - En relation :
    - GET `/v1/contracts/code/{code_hash}` -> créer le manifeste enregistré
    - GET `/v1/contracts/code-bytes/{code_hash}` -> développer `{ code_b64 }`
- POSTE `/v1/contracts/instance`
  - Demande : { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportement : supprimez la réserve du bytecode et activez immédiatement le mapeo `(namespace, contract_id)` via `ActivateContractInstance`.
  - Réponse : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Service d'alias
- POSTE `/v1/aliases/voprf/evaluate`
  - Demande : { "blinded_element_hex": "..." }
  - Réponse : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflète la mise en œuvre de l'évaluateur. Valeur actuelle : `blake2b512-mock`.
  - Remarques : l'évaluateur mock determinista qui applique Blake2b512 avec séparation du domaine `iroha.alias.voprf.mock.v1`. Disenado para outillage de test hasta que el pipeline VOPRF de produccion este cableado en Iroha.
  - Erreurs : HTTP `400` et entrée hexadécimale mal formée. Torii développe une enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec le message d'erreur du décodeur.
- POSTE `/v1/aliases/resolve`
  - Sollicitude : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Réponse : { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Remarques : nécessite le pontage ISO d'exécution (`[iso_bridge.account_aliases]` et `iroha_config`). Torii normalise l'alias en éliminant les espaces et en passant aux mayusculas avant la recherche. Devuelve 404 lorsque l'alias n'existe pas et 503 lorsque le pont ISO d'exécution est désactivé.
- POSTE `/v1/aliases/resolve_index`
  - Sollicitation : { "index": 0 }
  - Réponse : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }- Remarques : les indices d'alias sont attribués à un formulaire déterminé lors de l'ordre de configuration (basé sur 0). Les clients peuvent mettre en cache leurs réponses hors ligne pour créer des traces d'audience d'événements d'attestation d'alias.

Tope de tamano de codigo
- Paramètres personnalisés : `max_contract_code_bytes` (JSON u64)
  - Contrôlez le maximum autorisé (en octets) pour sauvegarder le code de contrat en chaîne.
  - Par défaut : 16 Mio. Les nœuds rechazan `RegisterSmartContractBytes` lorsque l'image `.to` dépasse le haut avec une erreur de violation invariante.
  - Les opérateurs peuvent ajuster l'envoi de `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et une charge utile numérique.- POSTE `/v1/gov/ballots/zk`
  - Sollicitation : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques :
    - Lorsque les entrées publiques du circuit incluent `owner`, `amount` et `duration_blocks`, et la vérification de la configuration VK, le nœud crée ou étend un blocage de sécurité pour `election_id` avec cela `owner`. La direction permanente occulte (`unknown`); il suffit d'actualiser le montant/l'expiration. Les modifications sont monotones : montant et expiration seulement aumentan (le noeud appliqué max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Les revotaciones ZK qui visent à réduire le montant ou l'expiration sont rechazan del lado del servidor con diagnosticos `BallotRejected`.
    - L'exécution du contrat doit être appelée `ZK_VOTE_VERIFY_BALLOT` avant d'encolar `SubmitBallot` ; Les hôtes imposent un loquet d'un seul coup.- POSTE `/v1/gov/ballots/plain`
  - Sollicitude : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: les revotaciones son de solo extension - un nouveau bulletin de vote ne peut pas réduire le montant ou l'expiration du blocage existant. Le `owner` doit être identique à l'autorité de la transaction. La durée minimale est `conviction_step_blocks`.- POSTE `/v1/gov/finalize`
  - Sollicitation : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet en chaîne (et réel) : promulguer une proposition de déploiement approuvée en insérant un `ContractManifest` minimum avec la clé `code_hash` avec le `abi_hash` attendu et marquer la proposition comme promulguée. S'il existe un manifeste pour le `code_hash` avec un `abi_hash` distinct, la promulgation est demandée.
  - Remarques :
    - Pour les élections ZK, les itinéraires du contrat doivent être appelés `ZK_VOTE_VERIFY_TALLY` avant d'exécuter `FinalizeElection` ; Les hôtes imposent un loquet d'un seul coup. `FinalizeReferendum` rechaza referendos ZK jusqu'à ce que le décompte de l'élection soit finalisé.
    - Le cierre automatique en `h_end` émet seulement Approuvé/Rejeté pour les références Plain ; les référendums ZK permanents Fermés jusqu'à ce qu'ils envoient un décompte finalisé et qu'ils soient éjectés `FinalizeReferendum`.
    - Les validations de participation utilisent uniquement l'approbation et le rejet ; s'abstenir sans compte pour le taux de participation.- POSTE `/v1/gov/enact`
  - Demande : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas : Torii envoyer la transaction ferme lorsque vous proposez `authority`/`private_key` ; de lo contrario, développez un squelette pour que les clients se raffermissent et envien. La préimage est facultative et actuellement informative.

- OBTENIR `/v1/gov/proposals/{id}`
  - Chemin `{id}` : identifiant de la propriété hexadécimale (64 caractères)
  - Réponse : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v1/gov/locks/{rid}`
  - Chemin `{rid}` : chaîne d'identifiant du référendum
  - Réponse : { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTENIR `/v1/gov/council/current`
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes : devuelve el conseil persistido cuando existe ; au contraire, une réponse est déterminée en utilisant l'actif de mise configuré et les ombrelles (en réfléchissant à la spécification du VRF jusqu'à ce que le VRF soit testé en vivo et persiste en chaîne).- POST `/v1/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Sollicitude : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement : vérifier l'essai VRF de chaque candidat contre l'entrée canonique dérivée de `chain_id`, `epoch` et la balise du dernier hachage de blocage ; ordena por bytes of salida desc con tiebreakers ; devuelve los top `committee_size` miembros. Pas de persistance.
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "vérified": K }
  - Notas : Normal = pk en G1, preuve en G2 (96 octets). Petit = pk en G2, preuve en G1 (48 octets). Les entrées sont séparées par le domaine et incluent `chain_id`.

### Paramètres de gouvernance par défaut (iroha_config `gov.*`)

Le conseil de réponse utilisé par Torii s'il n'existe pas de liste persistante paramétrée via `iroha_config` :

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

Remplace les équivalents en temps réel :

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

`parliament_committee_size` limite la quantité de miembros de respaldo devueltos lorsque le conseil de foin persiste, `parliament_term_blocks` définit la longueur de l'époque utilisée pour la dérivation des semences (`epoch = floor(height / term_blocks)`), `parliament_min_stake` applique le minimum de mise (en unidades minimas) sur l'actif d'éligibilité, et `parliament_eligibility_asset_id` sélectionne que le solde de l'actif soit analysé lors de la construction du groupe de candidats.La vérification VK de gouvernement n'a pas de contournement : la vérification des bulletins de vote nécessite toujours une clé de vérification `Active` avec des octets en ligne, et les entités ne doivent pas dépendre des bascules de vérification pour omettre la vérification.

RBAC
- L'éjection en chaîne nécessite des autorisations :
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (futuro) : `CanManageParliament`Espaces de noms protégés
- Le paramètre personnalisé `gov_protected_namespaces` (tableau de chaînes JSON) permet le contrôle d'admission pour les déploiements dans la liste des espaces de noms.
- Les clients doivent inclure des clés de métadonnées de transaction pour déployer des directions et des espaces de noms protégés :
  - `gov_namespace` : l'objet de l'espace de noms (par exemple, "apps")
  - `gov_contract_id` : l'identifiant logique du contrat dans l'espace de noms
- `gov_manifest_approvers` : tableau JSON facultatif pour les identifiants de compte des validateurs. Lorsqu'un manifeste de voie déclare un quorum maire à un seul, l'admission requiert l'autorité de la transaction plus les données répertoriées pour satisfaire le quorum du manifeste.
- La télémétrie expose les contadores d'admission via `governance_manifest_admission_total{result}` pour que les opérateurs distinguent les sorties de routes `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- La télémétrie expose l'itinéraire d'application via `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour que les opérateurs vérifient les autorisations erronées.
- Les voies appliquent la liste autorisée des espaces de noms publiée dans vos manifestes. Toute transaction qui fije `gov_namespace` doit fournir `gov_contract_id`, et l'espace de noms doit apparaître dans l'ensemble `protected_namespaces` du manifeste. Les envois `RegisterSmartContractCode` sans ces métadonnées sont demandés lorsque la protection est autorisée.- L'admission impose qu'il existe une proposition de gouvernement promulguée pour le tuple `(namespace, contract_id, code_hash, abi_hash)` ; au contraire, la validation a échoué avec une erreur NotPerowed.

Hooks de mise à niveau du runtime
- Les manifestes de la voie peuvent déclarer `hooks.runtime_upgrade` pour contrôler les instructions de mise à niveau du runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Champs du crochet :
  - `allow` (bool, `true` par défaut) : lorsque vous êtes `false`, suivez toutes les instructions de mise à niveau du runtime.
  - `require_metadata` (bool, par défaut `false`) : exige l'entrée des métadonnées spécifiées par `metadata_key`.
  - `metadata_key` (string) : nombre de métadonnées appliquées au hook. Par défaut, `gov_upgrade_id` lorsque des métadonnées ou une liste d'autorisation de foin sont requises.
  - `allowed_ids` (array de strings) : liste d'autorisation facultative des valeurs de métadonnées (tram trim). Rechaza cuando el valor provisto no esta listado.
- Lorsque le crochet est présent, l'admission du cola applique la politique des métadonnées avant la transaction entre le cola. Les métadonnées erronées, les valeurs vacios ou les valeurs hors de la liste autorisée produisent une erreur NotPerowed déterministe.
- La télémétrie a été obtenue via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les transactions qui complètent le crochet doivent inclure les métadonnées `gov_upgrade_id=<value>` (ou la clé définie par le manifeste) avec toute approbation des validateurs demandée par le quorum du manifeste.Point final de commodité
- POST `/v1/gov/protected-namespaces` - appliquer `gov_protected_namespaces` directement sur le nœud.
  - Demande : { "namespaces": ["apps", "system"] }
  - Réponse : { "ok": true, "applied": 1 }
  - Notes : pensé pour l'administration/les tests ; nécessite une API de jeton si elle est configurée. Pour la production, préférez envoyer une transaction ferme avec `SetParameter(Custom)`.CLI d'assistance
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Obtenez des instances de contrat pour l'espace de noms et vérifiez que :
    - Torii contient le bytecode pour chaque `code_hash`, et son résumé Blake2b-32 coïncide avec le `code_hash`.
    - Le manifeste almacenado bajo `/v1/contracts/code/{code_hash}` rapporte les valeurs `code_hash` et `abi_hash` coïncidentes.
    - Il existe une proposition de gouvernement promulguée pour `(namespace, contract_id, code_hash, abi_hash)` dérivée du même hachage de l'identifiant de proposition qui utilise le nœud.
  - Émettre un rapport JSON avec `results[]` pour le contrat (problèmes, résumés de manifeste/code/proposition) mais un résumé d'une ligne de salve qui est supérieure (`--no-summary`).
  - Utilisé pour auditer les espaces de noms protégés ou vérifier les flux de déploiement contrôlés par l'administration.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Émettez le modèle JSON de métadonnées utilisé pour envoyer des déploiements aux espaces de noms protégés, y compris les options `gov_manifest_approvers` pour satisfaire aux règles de quorum du manifeste.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — les indices de verrouillage sont obligatoires lorsque `min_bond_amount > 0`, et tout ensemble d'indices fournis doivent inclure `owner`, `amount` et `duration_blocks`.
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).- L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.
  - Le résumé d'une ligne maintenant expose un déterministe `fingerprint=<hex>` dérivé du `CastZkBallot` codifié avec des indices décodifiés (`owner`, `amount`, `duration_blocks`, `direction` lorsqu'il est fourni).
  - Les réponses CLI indiquent `tx_instructions[]` avec `payload_fingerprint_hex` mais les champs sont décodifiés pour que les outils en aval vérifient le squelette sans réimplémenter le décodage Norito.
  - Prouvez les conseils de blocage permettant que le nœud émette des événements `LockCreated`/`LockExtended` pour les bulletins de vote ZK une fois que le circuit expose les mêmes valeurs.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les alias `--lock-amount`/`--lock-duration-blocks` reflètent les nombres de drapeaux de ZK pour la parité dans les scripts.
  - La sortie de résumé de réflexion `vote --mode zk` inclut l'empreinte digitale de l'instruction codifiée et des champs de vote lisibles (`owner`, `amount`, `duration_blocks`, `direction`), en offrant une confirmation rapide avant le fermez le squelette.Liste des instances
- GET `/v1/gov/instances/{ns}` - liste des instances de contrat activées pour un espace de noms.
  - Paramètres de requête :
    - `contains` : filtre la sous-chaîne de `contract_id` (sensible à la casse)
    - `hash_prefix` : filtre par préfixe hexadécimal de `code_hash_hex` (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : un de `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`
  - Réponse : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK d'assistance : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Barrido de unlocks (Opérateur/Auditoria)
- OBTENIR `/v1/gov/unlocks/stats`
  - Réponse : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas : `last_sweep_height` reflète la hauteur du bloc mas récente donde los locks expirados fueron barridos y persistidos. `expired_locks_now` permet de calculer les enregistrements de serrure avec `expiry_height <= height_current`.
- POSTE `/v1/gov/ballots/zk-v1`
  - Sollicitude (style DTO v1) :
    {
      "autorité": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "enveloppe_b64": "AAECAwQ=",
      "root_hint": "0x...64hex ?",
      "propriétaire": "ih58…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (fonctionnalité : `zk-ballot`)
  - Acceptez un direct JSON `BallotProof` et développez un squelette `CastZkBallot`.
  - Sollicitation :
    {
      "autorité": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 du conteneur ZK1 ou H2*
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // AccountId facultatif lorsque le circuit est compromis avec le propriétaire
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
    - Le serveur mapea `root_hint`/`owner`/`nullifier` est optionnel pour le bulletin de vote à `public_inputs_json` pour `CastZkBallot`.
    - Les octets de l'enveloppe sont réencodés en base64 pour la charge utile de l'instruction.
    - La réponse `reason` change à `submitted transaction` lorsque Torii envoie le bulletin de vote.
    - Ce point de terminaison est uniquement disponible lorsque la fonctionnalité `zk-ballot` est autorisée.Itinéraire de vérification de CastZkBallot
- `CastZkBallot` décodifie l'essai base64 en fournissant et en rechaza les charges utiles vacios ou mal formatées (`BallotRejected` avec `invalid or empty proof`).
- L'hôte résout la clé vérificateur du bulletin de vote depuis le référendum (`vk_ballot`) ou les valeurs par défaut de gouvernance et exige que le registre existe, par `Active`, et lève des octets en ligne.
- Les octets de la clé de vérification enregistrée sont re-hashean avec `hash_vk` ; cualquier désajuste de compromiso interrompra l'éjection avant de vérifier pour protéger les entrées d'enregistrement des adultes (`BallotRejected` avec `verifying key commitment mismatch`).
- Les octets de l'essai sont envoyés au backend enregistré via `zk::verify_backend` ; les transcriptions invalides apparaissent comme `BallotRejected` avec `invalid proof` et les instructions tombent de manière déterministe.
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- Pruebas exitosas émis `BallotAccepted` ; les annulateurs duplicados, les racines d'éligibilité des viejos ou les régressions de verrouillage siguen produisant les raisons de rechazo existantes décrites avant dans ce document.

## Malade conducta de validadores y consenso conjunto

### Flujo de slashing et d'emprisonnementLe consensus émet `Evidence` codifié en Norito lorsqu'un validateur viole le protocole. Chaque charge utile est chargée en mémoire `EvidenceStore` et, si elle n'est pas arrivée auparavant, elle est matérialisée sur la carte `consensus_evidence` répondue par WSV. Les registres antérieurs à `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocs `7200` par défaut) sont enregistrés pour que le fichier soit enregistré de manière permanente, mais le enregistrement est enregistré pour les opérateurs. Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

Las ofensas reconnus se mapean uno a uno a `EvidenceKind` ; Les discriminants sont établis et sont renforcés par le modèle de données :

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

- **DoublePrepare/DoubleCommit** - le validateur ferme les hachages en conflit pour le même tuple `(phase,height,view,epoch)`.
- **InvalidQc** - un agrégateur de certificats de validation sous forme de chèques déterministes (par exemple, bitmap de sociétés vides).
- **InvalidProposal** - un guide proposant un blocage qui empêche la validation structurelle (par exemple, briser la règle de la chaîne verrouillée).
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

Les opérateurs et outillages peuvent inspecter et rediffuser des charges utiles à travers :- Torii : `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count` et `... submit --evidence-hex <payload>`.

La gouvernance doit traiter les octets de preuve comme une vérification canonique :

1. **Recolectar el payload** avant de caduque. Archivage des octets Norito croisé avec les métadonnées de hauteur/vue.
2. **Préparer la pénalité** en intégrant la charge utile dans un référendum ou une instruction sudo (par exemple, `Unregister::peer`). L'exécution revalide la charge utile ; preuve mal formée ou rancia se rechaza deterministamente.
3. **Programmer la topologie de suivi** pour que le validateur en infraction ne puisse pas réintégrer immédiatement. Flujos tipicos encolan `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` avec la liste actualisée.
4. **Résultats d'audit** via `/v1/sumeragi/evidence` et `/v1/sumeragi/status` pour garantir que le contador de preuves avance et que la gouvernance applique la suppression.

### Séquence de consensus conjointe

Le consensus garantit que le groupe de validateurs saillants finalise le blocage de la frontière avant que le nouveau groupe ne donne le droit au promoteur. Le runtime impose la règle via les paramètres pareados :- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` doivent confirmer le **mismo bloque**. `mode_activation_height` doit être strictement plus élevé que la hauteur du blocage qui charge la mise à jour, en fournissant au moins un blocage de décalage.
- `sumeragi.npos.reconfig.activation_lag_blocks` (`1` par défaut) est le garde de configuration qui permet les transferts avec un décalage nul :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Le runtime et les paramètres CLI exposés via `/v1/sumeragi/params` et `iroha --output-format text ops sumeragi params`, pour que les opérateurs confirment les valeurs d'activation et les listes de validateurs.
- L'automatisation de la gouvernance doit toujours être :
  1. Finalizar la décision de réinstallation (ou de réinstallation) renvoyée par preuve.
  2. Inscrivez une reconfiguration de suite avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorear `/v1/sumeragi/status` hasta que `effective_consensus_mode` change à la hauteur attendue.

Tout script qui valide automatiquement l'application slashing **ne doit pas** tenter d'activer avec un décalage nul ou d'omettre les paramètres de transfert ; ces transactions se rechazan et dejan la red en el modo previo.

## Superficies de télémétrie- Les mesures Prometheus exportent l'activité d'administration :
  - `governance_proposals_status{status}` (jauge) rastrea conteos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (compteur) incrémente lorsque l'admission des espaces de noms protégés permet de refaire un déploiement.
  - `governance_manifest_activations_total{event}` (compteur) enregistre les insertions du manifeste (`event="manifest_inserted"`) et les liaisons de l'espace de noms (`event="instance_bound"`).
- `/status` inclut un objet `governance` qui reflète les contenus de propositions, rapporte le total des espaces de noms protégés et la liste des activations récentes du manifeste (espace de noms, identifiant de contrat, code/hachage ABI, hauteur de bloc, horodatage d'activation). Les opérateurs peuvent consulter ce champ pour confirmer que les promulgations actualisées se manifestent et que les portes des espaces de noms protégés sont appliquées.
- Une plante Grafana (`docs/source/grafana_governance_constraints.json`) et le runbook de télémétrie en `telemetry.md` peuvent être utilisés pour câbler des alertes pour les erreurs, les activations de manifestes incorrectes ou les notifications d'espaces de noms protégés lors des mises à niveau du runtime.