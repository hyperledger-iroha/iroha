---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Statut : rascunho/esboco para accompagner as tarefas de Implementacao de Governance. Comme forme, nous pouvons changer lors de la mise en œuvre. Déterminisme et politique RBAC avec restrictions normatives ; Torii peut assassinar/submeter transacoes lorsque `authority` et `private_key` sont fournis, au contraire, les clients construisent et submetem pour `/transaction`.Visa général
- Tous les points de terminaison renvoient le nom JSON. Pour les flux qui produisent des transcoes, les réponses incluent `tx_instructions` - un tableau d'un ou plus d'instructions esqueleto :
  - `wire_id` : identifiant d'enregistrement pour le type d'instruction
  - `payload_hex` : octets de charge utile Norito (hex)
- Se `authority` et `private_key` forem fornecidos (ou `private_key` em DTOs de vote), Torii assina e soumette a transacao e ainda retorna `tx_instructions`.
- Cas contraire, les clients montent une SignedTransaction en utilisant leur autorité et leur chain_id, après avoir envoyé et envoyé POST pour `/transaction`.
- Couverture du SDK :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` retour `GovernanceProposalResult` (état/type de champ normal), `ToriiClient.get_governance_referendum_typed` retour `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` retour `GovernanceTally`, `ToriiClient.get_governance_locks_typed` retour `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retour `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` retour `GovernanceInstancesPage`, veuillez accéder à tout type de surface de gouvernance avec des exemples d'utilisation non LISEZMOI.
- Niveau client Python (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` retornam bundles types `GovernanceInstructionDraft` (encapsuler ou écrire `tx_instructions` par Torii), éviter l'analyse manuel de JSON lorsque les scripts composent des flux Finalize/Enact.- JavaScript (`@iroha/iroha-js`) : `ToriiClient` expose les aides utilisées pour les propositions, les référendums, les décomptes, les verrous, les statistiques de déverrouillage, et maintenant `listGovernanceInstances(namespace, options)` plus les points de terminaison du conseil (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) pour que les clients Node.js puissent pager `/v1/gov/instances/{ns}` et réaliser des workflows avec VRF en même temps que la liste des instances de contrat existantes.

Points de terminaison

- POSTE `/v1/gov/proposals/deploy-contract`
  - Requisicao (JSON):
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
  - Validation : nous avons canonisé `abi_hash` pour ou `abi_version` fornecido e rejeitam divergences. Pour `abi_version = "v1"`, la valeur attendue et `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contrats (déployer)
- POSTE `/v1/contracts/deploy`
  - Requisicao : { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportement : calculez `code_hash` à partir du corps du programme IVM et `abi_hash` à partir de l'en-tête `abi_version`, puis soumettez `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (octets). `.to` complet) sous le nom de `authority`.
  - Réponse : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - En relation :
    - GET `/v1/contracts/code/{code_hash}` -> retour ou manifeste armazenado
    - GET `/v1/contracts/code-bytes/{code_hash}` -> retour `{ code_b64 }`
- POSTE `/v1/contracts/instance`
  - Conditions requises : { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Port : déployez le bytecode fourni et activez immédiatement le mappage `(namespace, contract_id)` via `ActivateContractInstance`.
  - Réponse : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Service d'alias
- POSTE `/v1/aliases/voprf/evaluate`
  - Condition requise : { "blinded_element_hex": "..." }
  - Réponse : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflète la mise en œuvre de l'avaliateur. Valeur actuelle : `blake2b512-mock`.
  - Remarques : utiliser un modèle déterministe qui applique Blake2b512 avec séparation du domaine `iroha.alias.voprf.mock.v1`. Destiné à l'outillage de test ou au pipeline VOPRF de production, il est intégré à Iroha.
  - Erreurs : HTTP `400` dans une entrée hexadécimale mal formée. Torii renvoie une enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec un message d'erreur du décodeur.
- POSTE `/v1/aliases/resolve`
  - Requisicao : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Réponse : { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Remarques : demander la mise en scène du pont ISO d'exécution (`[iso_bridge.account_aliases]` et `iroha_config`). Torii normalise les alias en supprimant les espaces et en les convertissant pour les principales avant la recherche. Retour 404 lorsque l'alias est ausente et 503 lorsque le pont ISO d'exécution est désactivé.
- POSTE `/v1/aliases/resolve_index`
  - Requisicao : { "index": 0 }
  - Réponse : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }- Remarques : indices d'alias sao atribuidos de forma déterministica pela ordem de configuracao (basé sur 0). Les clients peuvent mettre en cache leurs réponses hors ligne pour créer des sessions d'auditoire d'événements d'attestation d'alias.

Limite de tamanho de codigo
- Paramètres personnalisés : `max_contract_code_bytes` (JSON u64)
  - Contrôlez le maximum autorisé (en octets) pour l'armement du code de contrat en chaîne.
  - Par défaut : 16 Mio. Nous rejetons `RegisterSmartContractBytes` lorsque l'image `.to` dépasse la limite d'une erreur de violation invariante.
  - Les opérateurs peuvent être ajustés via `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et une charge utile numérique.- POSTE `/v1/gov/ballots/zk`
  - Conditions requises : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques :
    - Lorsque les entrées publiques du circuit incluent `owner`, `amount` et `duration_blocks`, et une vérification contre un VK configuré, ou pas de cri ou d'extension d'un verrou de gouvernance pour `election_id` comme ça `owner`. Une oculta permanente directe (`unknown`); apenas montant/expiration sao atualizados. Re-vote de manière monotone : le montant et l'expiration s'augmentent automatiquement (sans application max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Re-vote ZK qui tente de réduire le montant ou l'expiration de sao rejeitados no servidor com diagnosticos `BallotRejected`.
    - L'exécution du contrat deve chamar `ZK_VOTE_VERIFY_BALLOT` avant d'envoyer `SubmitBallot` ; les hôtes impoem um latch de uma unica vez.- POSTE `/v1/gov/ballots/plain`
  - Requisicao : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: re-vote sao de extensao apenas - un nouveau bulletin de vote ne peut pas réduire le montant ou l'expiration du verrouillage existant. O `owner` deve igualar a authority da transacao. Duraçao minimum et `conviction_step_blocks`.- POSTE `/v1/gov/finalize`
  - Requisicao : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet en chaîne (échafaudage réel) : promulguer une proposition de déploiement approuvée en insérant un `ContractManifest` minimum avec `code_hash` avec ou `abi_hash` attendu et marqué à propos de l'adoption. Il existe déjà un manifeste pour le `code_hash` et le `abi_hash` différent, la promulgation et le rejet.
  - Remarques :
    - Pour les appareils ZK, les caminhos du contrat doivent chamar `ZK_VOTE_VERIFY_TALLY` avant d'exécuter `FinalizeElection` ; Les hôtes impoem un loquet à usage unique. `FinalizeReferendum` a rejeté les références ZK qui ont fait en sorte que le compteur électrique soit finalisé.
    - L'auto-envoi en `h_end` émet des demandes approuvées/rejetées pour les référendums Plain ; les référendums ZK permanents fermés ont fait qu'un décompte finalisé soit envoyé et `FinalizeReferendum` soit exécuté.
    - En tant que checagens de participation usam apenas approuver+rejeter ; s'abstenir nao conta para o participation.- POSTE `/v1/gov/enact`
  - Conditions requises : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes : Torii soumettre une transacao assassinée lorsque `authority`/`private_key` sao fornecidos ; caso contrario retorna um esqueleto para clientses assinarem e submeterem. Une préimage facultative et hoje informativa.

- OBTENIR `/v1/gov/proposals/{id}`
  - Chemin `{id}` : identifiant de la proposition hexadécimale (64 caractères)
  - Réponse : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v1/gov/locks/{rid}`
  - Chemin `{rid}` : chaîne d'identifiant du référendum
  - Réponse : { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTENIR `/v1/gov/council/current`
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: retorna o conseil persistido quando presente; Dans certains cas, un repli déterministe est dérivé de l'utilisation de l'actif de participation configuré et des seuils (en particulier le VRF spécifié qui prouve que le VRF est produit de manière persistante en chaîne).- POST `/v1/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Requisicao : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement : vérifier le VRF de chaque candidat contre l'entrée canonique dérivée de `chain_id`, `epoch` et le dernier hachage de bloc ; ordena por bytes of saida desc com tiebreakers; retorna os top `committee_size` membres. Nao persiste.
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "vérified": K }
  - Notes : Normal = pk em G1, preuve em G2 (96 octets). Petit = pk em G2, preuve em G1 (48 octets). Les entrées sont séparées par le domaine et incluent `chain_id`.

### Paramètres de gouvernance par défaut (iroha_config `gov.*`)

Le conseil de secours utilisé par Torii lorsqu'il n'existe pas de liste persistante et paramétré via `iroha_config` :

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

Remplace les équivalents ambiants :

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

`parliament_committee_size` limite le nombre de membres de secours renvoyés lorsque le conseil persiste, `parliament_term_blocks` définit le compromis de l'époque utilisé pour le dérivé de la graine (`epoch = floor(height / term_blocks)`), `parliament_min_stake` applique le minimum de mise (en unités minimes) sans actif de L'éligibilité, et `parliament_eligibility_asset_id` sélectionne la vente d'actifs et est effectuée pour la construction du groupe de candidats.La vérification VK de gouvernance n'a pas de contournement : la vérification du scrutin demande toujours un code `Active` avec des octets en ligne, et l'environnement dans lequel il se trouve doit dépendre des bascules de test pour pouvoir effectuer la vérification.

RBAC
- Execucao on-chain exige des autorisations :
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (futuro) : `CanManageParliament`Espaces de noms protégés
- Le paramètre personnalisé `gov_protected_namespaces` (tableau de chaînes JSON) permet le contrôle d'admission pour déployer les espaces de noms répertoriés.
- Les clients développent des éléments de métadonnées de transfert pour déployer des espaces de noms protégés :
  - `gov_namespace` : espace de noms alvo (ex., "apps")
  - `gov_contract_id` : identifiant de contrat logique dans l'espace de noms
- `gov_manifest_approvers` : tableau JSON facultatif pour les identifiants de compte des validateurs. Lorsqu'un manifeste déclare qu'il y a quorum majeur, l'admission demande une autorité de traversée mais avec des listes de renseignements pour satisfaire le quorum du manifeste.
- Telemetria expose les contadores d'admission via `governance_manifest_admission_total{result}` pour que les opérateurs distincts admettent les réussis de caminhos `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- Telemetria expose le chemin de mise en application via `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour que les opérateurs vérifient les demandes erronées.
- Les voies appliquent une liste autorisée d'espaces de noms publiés dans leurs manifestes. Qu'importe qui définit `gov_namespace` doit fornecer `gov_contract_id`, et l'espace de noms doit apparaître en même temps que `protected_namespaces` dans le manifeste. Soumis `RegisterSmartContractCode` sans que ces métadonnées soient rejetées lorsque la protection est autorisée.- Admission impoe qu'existea uma proposta de gouvernance promulguée pour le tuple `(namespace, contract_id, code_hash, abi_hash)` ; cas contraire, il y a une erreur de validation avec une erreur NotPermise.

Hooks de mise à niveau du runtime
- Les manifestes de voie peuvent déclarer `hooks.runtime_upgrade` pour accéder aux instructions de mise à niveau du runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos do crochet :
  - `allow` (bool, `true` par défaut) : lorsque `false`, toutes les instructions de mise à niveau du runtime sont rejetées.
  - `require_metadata` (bool, par défaut `false`) : demande une entrée de métadonnées spécifiée par `metadata_key`.
  - `metadata_key` (string) : nom des métadonnées appliquées au hook. Par défaut `gov_upgrade_id` lorsque les métadonnées sont requises ou ont une liste autorisée.
  - `allowed_ids` (tableau de chaînes) : liste d'autorisation facultative des valeurs de métadonnées (après trim). Rejeita quando o valor fornecido nao esta listado.
- Quand le crochet est présent, l'admission de la fila s'applique à la politique des métadonnées avant l'entrée transfrontalière de la fila. Les métadonnées existantes, les valeurs en blanc ou dans la liste autorisée génèrent une erreur déterministe NotPermise.
- Résultats de Telemetria rastreia via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les transactions qui satisfont le crochet doivent inclure des métadonnées `gov_upgrade_id=<value>` (ou une définition du manifeste) avec l'approbation des validateurs requis pour le quorum du manifeste.Point final de commodité
- POST `/v1/gov/protected-namespaces` - appliquer `gov_protected_namespaces` directement non no.
  - Requisicao : { "namespaces": ["apps", "system"] }
  - Réponse : { "ok": true, "appliqué": 1 }
  - Notes : destiné à un administrateur/test ; demander le jeton de l'API configuré. Pour produire, je préfère envoyer une transaction effectuée avec `SetParameter(Custom)`.CLI d'assistance
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Recherchez des instances de contrat pour l'espace de noms et conférez que :
    - Torii utilise le bytecode pour chaque `code_hash`, et votre résumé Blake2b-32 correspond à `code_hash`.
    - Le manifeste armazenado em `/v1/contracts/code/{code_hash}` rapporte `code_hash` et `abi_hash` correspondants.
    - Existe-t-il une proposition de gouvernance promulguée pour `(namespace, contract_id, code_hash, abi_hash)` dérivée du hachage de l'identifiant de proposition que nous n'utilisons pas.
  - Émettez un rapport JSON avec `results[]` par contrat (problèmes, résumés de manifeste/code/proposition) mais un résumé d'une ligne à moins que vous supprimiez (`--no-summary`).
  - Utiliser pour auditer les espaces de noms protégés ou vérifier les flux de déploiement contrôlés par la gouvernance.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Émettez le squelette JSON de métadonnées utilisé pour les déploiements de sous-mètres dans les espaces de noms protégés, y compris l'option `gov_manifest_approvers` pour satisfaire aux exigences du quorum du manifeste.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — les indices de verrouillage sont obligatoires lorsque `min_bond_amount > 0`, et tout ensemble d'indices fournis doivent inclure `owner`, `amount` et `duration_blocks`.
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).- L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.
  - Le résumé d'une ligne avant apparaît `fingerprint=<hex>` déterministe dérivé du `CastZkBallot` codifié avec des indices décodifiés (`owner`, `amount`, `duration_blocks`, `direction` quando fornecidos).
  - En réponse à l'annotation CLI `tx_instructions[]` avec `payload_fingerprint_hex`, plus de champs décodifiés pour que les ferramentas en aval vérifient ou squeleto sem réimplémentent le décodage Norito.
  - Des indications de verrouillage permettent de ne pas émettre d'événements `LockCreated`/`LockExtended` pour les bulletins de vote ZK afin que le circuit expéditeur de mes valeurs.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les alias `--lock-amount`/`--lock-duration-blocks` utilisent les noms de drapeaux ZK pour la parité du script.
  - Le texte du curriculum vitae `vote --mode zk` inclut l'empreinte digitale des instructions codifiées et les champs de vote légal (`owner`, `amount`, `duration_blocks`, `direction`), offrant une confirmation rapide. avant d'assassiner le squelette.Liste des instances
- GET `/v1/gov/instances/{ns}` - liste des instances de contrat actives pour un espace de noms.
  - Paramètres de requête :
    - `contains` : filtre la sous-chaîne de `contract_id` (sensible à la casse)
    - `hash_prefix` : filtre par préfixe hexadécimal de `code_hash_hex` (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : un de `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`
  - Réponse : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK Helper : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Varredura de unlocks (Opérateur/Auditoria)
- OBTENIR `/v1/gov/unlocks/stats`
  - Réponse : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Remarques : `last_sweep_height` reflète la hauteur du bloc la plus récente des verrous expirés pour les variables et les persistants. `expired_locks_now` et calculé pour les différents enregistrements de serrure avec `expiry_height <= height_current`.
- POSTE `/v1/gov/ballots/zk-v1`
  - Requisicao (style DTO v1) :
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
  - Ajoutez un JSON `BallotProof` directement et retournez au modèle `CastZkBallot`.
  - Requisicao :
    {
      "autorité": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 du conteneur ZK1 ou H2*
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // AccountId facultatif lorsque le circuit comprend le propriétaire
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
    - Le serveur mapeia `root_hint`/`owner`/`nullifier` opte pour le scrutin pour `public_inputs_json` et `CastZkBallot`.
    - Les octets de l'enveloppe sont réencodés en base64 pour la charge utile des instructions.
    - La réponse `reason` change pour `submitted transaction` lorsque Torii soumet le bulletin de vote.
    - Ce point de terminaison est donc disponible lorsque la fonctionnalité `zk-ballot` est autorisée.Chemin de vérification CastZkBallot
- `CastZkBallot` décodifie une preuve base64 fournie et rejette des charges utiles vazios ou malformados (`BallotRejected` avec `invalid or empty proof`).
- L'hôte résout le besoin de vérifier le scrutin à partir du référendum (`vk_ballot`) ou les valeurs par défaut de gouvernance et exigent que le registre existe, esteja `Active` et contient des octets en ligne.
- Octets de chave verificadora armazenados sao re-hasheados com `hash_vk` ; tout décalage d'engagement avorta a execucao antes da verificacao para proteger contra entradas de registro adultradas (`BallotRejected` com `verifying key commitment mismatch`).
- Octets de preuve envoyés au backend enregistrés via `zk::verify_backend` ; Les transcriptions invalides apparaissent comme `BallotRejected` avec `invalid proof` et les instructions de forme déterministe.
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- Provas bem-sucedidas émettem `BallotAccepted` ; les annulateurs dupliqués, les racines d'éligibilité obsolètes ou la régression du verrouillage continuent à être produites comme des raisons de refus existantes décrites antérieurement dans ce document.

## Mauvais comportement des validateurs et consensus conjoint

### Flux de coupure et d'emprisonnementLe consensus émet `Evidence` codifié sur Norito lorsqu'un validateur viole le protocole. Chaque charge utile est entrée dans la mémoire `EvidenceStore` et, inédite, et matérialisée sur la carte `consensus_evidence` renvoyée par WSV. Les registres les plus anciens que `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocs `7200` par défaut) sont rejetés pour maintenir ou stocker un volume limité, mais ils sont enregistrés et enregistrés pour les opérateurs. Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind` ; les discriminants sao estaveis et impostos par le modèle de données :

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

- **DoublePrepare/DoubleCommit** - le validateur assène les hachages conflictuels pour le même tuple `(phase,height,view,epoch)`.
- **InvalidQc** - un agrégateur a bavardé sur un certificat de validation qui n'a pas la forme de vérifications déterministes (ex., bitmap des signataires vazio).
- **InvalidProposal** - un leader propose un bloc qui ne valide pas la structure (par exemple, en violant la chaîne verrouillée).
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

Les opérateurs et les outils peuvent inspecter et rediffuser les charges utiles via :

- Torii : `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count` et `... submit --evidence-hex <payload>`.La gouvernance doit traiter les octets de preuve comme preuve canonique :

1. **Coletar ou payload** avant l'expiration. Archivez les octets Norito bruts avec les métadonnées de hauteur/vue.
2. **Préparer une pénalité** en intégrant la charge utile dans un référendum ou en instruisant sudo (ex., `Unregister::peer`). Une exécution de revalidation de la charge utile ; preuves malformées ou périmées et rejetées de manière déterminante.
3. **Agenda de topologie d'accompagnement** pour que le validateur infrateur ne puisse pas revenir immédiatement. Fluxos tipicos enfileiram `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` avec la liste actualisée.
4. **Résultats d'audit** via `/v1/sumeragi/evidence` et `/v1/sumeragi/status` pour garantir que le contrôleur des preuves avancé et que le gouvernement applique à distance.

### Séquence de consensus conjoint

Le consensus garantit que le groupe des validateurs de ladite finalisation du bloc de frontière avant que le nouveau groupe vienne à son terme. L'application d'exécution à récupérer via les paramètres pareados :

- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` ne doivent pas être confirmés **même bloc**. `mode_activation_height` devrait être plus important que la hauteur du bloc qui persiste à l'actualisation, ce qui entraîne au moins un bloc de décalage.
- `sumeragi.npos.reconfig.activation_lag_blocks` (`1` par défaut) et la protection de configuration qui empêche les transferts avec le décalage zéro :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Le runtime et les paramètres d'exposition CLI mis en scène via `/v1/sumeragi/params` et `iroha --output-format text ops sumeragi params`, pour que les opérateurs confirment les valeurs d'activation et les listes de validateurs.
- Un automação de gouverner deve sempre :
  1. Finaliser la décision de retirer (ou réintégrer) la réponse aux preuves.
  2. Effectuez une reconfiguration de l'accompagnement avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorar `/v1/sumeragi/status` a mangé `effective_consensus_mode` trocart na altura esperada.

Quel que soit le script qui valide la rotation ou applique le slashing **nao deve** tente d'activer le décalage zéro ou d'omettre les paramètres de transfert ; tais transacoes sao rejeitadas e deixam a rede no modo anterior.

## Superficies de télémétrie- Metricas Prometheus exporte l'activité de gouvernance :
  - `governance_proposals_status{status}` (jauge) rastreia contagens de propositions por status.
  - `governance_protected_namespace_total{outcome}` (compteur) incrémente lorsque l'admission d'espaces de noms protégés permet ou refuse un déploiement.
  - `governance_manifest_activations_total{event}` (compteur) enregistre les inserts de manifeste (`event="manifest_inserted"`) et les liaisons d'espace de noms (`event="instance_bound"`).
- `/status` inclut un objet `governance` qui apparaît comme contagieux aux propositions, relatif au total des espaces de noms protégés et à la liste active récente du manifeste (espace de noms, identifiant de contrat, code/hachage ABI, hauteur de bloc, horodatage d'activation). Les opérateurs peuvent consulter ce champ pour confirmer que les textes actualisés se manifestent et que les portes des espaces de noms protégés sont envoyées appliquées.
- Un modèle Grafana (`docs/source/grafana_governance_constraints.json`) et un runbook de télémétrie dans `telemetry.md` sont affichés comme des alertes pour les propositions présentées, les activations de manifestes ausentes ou les demandes d'espaces de noms protégés lors des mises à niveau du runtime.