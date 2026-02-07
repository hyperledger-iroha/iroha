---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Statut : responsable/responsable de la prise en charge de la gouvernance. Les formulaires peuvent être utilisés dans vos robots. La détermination et la politique du RBAC sont des normes générales ; Torii peut permettre de transférer/déménager des transactions avec les clients `authority` et `private_key`, pour que les clients s'en occupent et отправляют в `/transaction`.Обзор
- Tous les points de terminaison utilisent JSON. Pour les véhicules qui nécessitent des transferts, les résultats incluent `tx_instructions` - des instructions de montage importantes ou non :
  - `wire_id` : type d'identificateur de restauration d'instructions
  - `payload_hex` : charge utile de la batterie Norito (hex)
- Si `authority` et `private_key` sont pré-établis (ou `private_key` dans les bulletins de vote DTO), Torii permet et accélère la transition. et nous avons effectivement utilisé `tx_instructions`.
- Lorsque les clients acceptent SignedTransaction avec leur autorité et chain_id, ils les envoient et POST dans `/transaction`.
- Télécharger le SDK :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` remplace `GovernanceProposalResult` (état normal/type), `ToriiClient.get_governance_referendum_typed` remplace `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` prend en charge `GovernanceTally`, `ToriiClient.get_governance_locks_typed` prend en charge `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` prend en charge `GovernanceUnlockStats`, et `ToriiClient.list_governance_instances_typed` s'applique à `GovernanceInstancesPage`, en utilisant les modèles de gouvernance les plus avancés dans le fichier README.
- Client Python léger (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` proposent des bundles types `GovernanceInstructionDraft` (disponibles Torii squelette `tx_instructions`), utilisez le fichier JSON pour finaliser/activer les flux.- JavaScript (`@iroha/iroha-js`) : `ToriiClient` fournit des aides types pour les propositions, les référendums, les décomptes, les verrous, le déverrouillage des statistiques et utilise `listGovernanceInstances(namespace, options)` pour les points de terminaison du conseil. (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), les clients Node.js peuvent configurer `/v1/gov/instances/{ns}` et télécharger VRF workflows наряду с существующим списком контрактных инстансов.

Points de terminaison

- POSTE `/v1/gov/proposals/deploy-contract`
  - Requête (JSON) :
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
  - Validation : vous pouvez canoniser `abi_hash` pour le remplacement `abi_version` et éviter les problèmes. Pour `abi_version = "v1"`, la réponse est `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API Contrats (déploiement)
- POSTE `/v1/contracts/deploy`
  - Requête : { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportement : sélectionnez `code_hash` dans le programme IVM et `abi_hash` dans le programme `abi_version`, afin de l'activer. `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (pour `.to`) sur `authority`.
  - Réponse : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Connexes :
    - GET `/v1/contracts/code/{code_hash}` -> возвращает сохраненный manifeste
    - GET `/v1/contracts/code-bytes/{code_hash}` -> télécharger `{ code_b64 }`
- POSTE `/v1/contracts/instance`
  - Requête : { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportement : Déployez le bytecode précédent et activez le mappage `(namespace, contract_id)` par `ActivateContractInstance`.
  - Réponse : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Service d'alias
- POSTE `/v1/aliases/voprf/evaluate`
  - Requête : { "blinded_element_hex": "..." }
  - Réponse : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` permet de réaliser une opération. Numéro de téléphone : `blake2b512-mock`.
  - Notes : Détermination d'une fausse analyse, par Blake2b512 avec séparation de domaine `iroha.alias.voprf.mock.v1`. Le projet d'outillage de test pour la production du pipeline VOPRF ne correspond pas à Iroha.
  - Erreurs : HTTP `400` lors d'une entrée hexadécimale incorrecte. Torii correspond à l'enveloppe Norito `ValidationFail::QueryFailed::Conversion` avec le décodeur.
- POSTE `/v1/aliases/resolve`
  - Demande : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Réponse : { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Remarques : prend en charge la préparation d'exécution du pont ISO (`[iso_bridge.account_aliases]` à `iroha_config`). Torii normalise l'alias, vous devez vérifier et contacter votre registre. Lorsque vous utilisez l'alias 404 et l'alias 503, le temps d'exécution du pont ISO est activé.
- POSTE `/v1/aliases/resolve_index`
  - Requête : { "index": 0 }
  - Réponse : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - Remarques : l'alias de l'index est déterminé par la configuration (basée sur 0). Les clients peuvent recevoir des réponses hors ligne pour établir une piste d'audit en utilisant un alias d'attestation.Code Taille Casquette
- Paramètre personnalisé : `max_contract_code_bytes` (JSON u64)
  - Mettre en place un maximum de contrats de transaction en chaîne (dans les contrats).
  - Par défaut : 16 Mio. Lorsque vous ouvrez `RegisterSmartContractBytes`, vous pouvez définir `.to` en définissant la limite précédente, en cas de violation invariante.
  - Les opérateurs peuvent utiliser la charge utile `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` et leur charge utile.

- POSTE `/v1/gov/ballots/zk`
  - Requête : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques :
    - Lorsque les schémas sont publiés, ils incluent `owner`, `amount` et `duration_blocks`, et la preuve est vérifiée par VK, Maintenant, vous pouvez créer un verrou de gouvernance pour `election_id` avec `owner`. Écran d'accueil (`unknown`) ; обновляются только montant/expiration. Il y a quatre monotones : montant et expiration qui sont également disponibles (il faut indiquer max(amount, prev.amount) et max(expiry, prev.expiry)).
    - ZK re-vote, пытающиеся уменьшить montant ou expiration, отклоняются сервером с диагностикой `BallotRejected`.
    - L'utilisation du contrat doit obligatoirement être effectuée par `ZK_VOTE_VERIFY_BALLOT` vers les postes `SubmitBallot` ; хосты appliquer одноразовый loquet.- POSTE `/v1/gov/ballots/plain`
  - Requête : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques : les nouveaux votes ne concernent que le vote - le nouveau bulletin de vote ne peut pas indiquer le montant ni le verrouillage d'expiration. `owner` должен совпадать с autorité транзакции. Минимальная длительность - `conviction_step_blocks`.- POSTE `/v1/gov/finalize`
  - Requête : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet en chaîne (échafaudage actuel) : promulguer une proposition de déploiement inversée avec un mini-`ContractManifest`, directement avec `code_hash`, avec `abi_hash` et помечает proposition как Adopté. Si le manifeste est proposé pour `code_hash` avec le `abi_hash`, le texte est adopté.
  - Remarques :
    - Pour les élections ZK, les contrats doivent passer `ZK_VOTE_VERIFY_TALLY` à `FinalizeElection` ; хосты appliquer одноразовый loquet. `FinalizeReferendum` ouvre le réducteur ZK, si le compteur n'est pas finalisé.
    - L'automobile de `h_end` émet des demandes approuvées/rejetées uniquement pour les références simples ; Le référendum ZK est fermé, mais il ne faut pas ouvrir le décompte final et utiliser `FinalizeReferendum`.
    - Le taux de participation prouve qu'il suffit d'approuver+rejeter ; s'abstenir n'est pas nécessaire lors de la participation.- POSTE `/v1/gov/enact`
  - Requête : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Remarques : Torii permet la transition vers les valeurs `authority`/`private_key` ; иначе возвращает skeleton для подписи и отправки clientом. Préimage опционален и сейчас носит information характер.

- OBTENIR `/v1/gov/proposals/{id}`
  - Chemin `{id}` : identifiant de proposition hexadécimal (64 caractères)
  - Réponse : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v1/gov/locks/{rid}`
  - Chemin `{rid}` : chaîne d'identification du référendum
  - Réponse : { "found": bool, "referendum_id": "rid", "locks": { ... } ? }

- OBTENIR `/v1/gov/council/current`
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes : возвращает persisté conseil при наличии; Pour déterminer la solution de repli, utilisez les actifs de mise et les seuils les plus élevés (les spécifications du VRF sont pour vous, si les preuves du VRF ne sont pas pertinentes). en chaîne).- POST `/v1/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Requête : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement : vérification du candidat à la preuve VRF sur l'entrée canonique, notamment `chain_id`, `epoch` et la balise après le hachage de bloc ; сортирует по octets de sortie desc avec bris d'égalité ; возвращает top `committee_size` участников. Je ne suis pas sûr.
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notes : Normal = pk в G1, preuve в G2 (96 octets). Petit = pk в G2, preuve в G1 (48 octets). Entrées доменно разделены и включают `chain_id`.

### Paramètres de gouvernance par défaut (iroha_config `gov.*`)

Conseil de secours, utilisant Torii pour la liste persistante, paramétré pour `iroha_config` :

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

Équipements permanents exclusifs :

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

`parliament_committee_size` ограничивает число fallback членов при отсутствии conseil, `parliament_term_blocks` задает дLINу эпохи для dérivation seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` implique une participation minime (dans des limites minimes) sur l'actif éligible, et `parliament_eligibility_asset_id` permet d'analyser le solde des actifs avant de postuler candidats.Vérification de la gouvernance VK sans contournement : la vérification des bulletins de vote permet de vérifier la clé `Active` sur les octets en ligne et ne permet pas de tester les bascules, ce qui permet de vérifier la vérification.

RBAC
- Démarrage en chaîne :
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (future) : `CanManageParliament`Espaces de noms protégés
- Le paramètre personnalisé `gov_protected_namespaces` (tableau de chaînes JSON) permet le contrôle d'admission pour les déploiements dans les espaces de noms principaux.
- Les clients peuvent également transférer les clés de métadonnées pour les déploiements, notamment dans les espaces de noms protégés :
  - `gov_namespace` : cet espace de noms (par exemple, "apps")
  - `gov_contract_id` : identifiant de contrat logique dans l'espace de noms
- `gov_manifest_approvers` : ID de compte de tableau JSON optionnels validés. Le manifeste de voie indique le quorum > 1, l'admission implique la transition des autorités et les comptes préalables pour la manifestation du quorum.
- Les compteurs d'admission télémétriques sont disponibles à partir de `governance_manifest_admission_total{result}`, pour les opérateurs en charge des admissions de `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- Le chemin d'application télémétrique est disponible à partir de `governance_manifest_quorum_total{outcome}` (`satisfied` / `rejected`), permettant à l'opérateur d'auditer les approbations non autorisées.
- Les voies appliquent la liste blanche des espaces de noms, publiées dans les manifestes. La transition qui consiste à utiliser `gov_namespace` doit précéder `gov_contract_id`, un espace de noms défini dans l'ensemble `protected_namespaces` Manifestateur. Les soumissions `RegisterSmartContractCode` sans ces métadonnées sont inversées, mais elles sont également disponibles.
- Admission требует, чтобы существовал Proposition de gouvernance adoptée pour le tuple `(namespace, contract_id, code_hash, abi_hash)` ; иначе валидация завершается ошибкой NotPerowed.Hooks de mise à niveau d’exécution
- Les manifestes de voie peuvent utiliser `hooks.runtime_upgrade` pour l'instruction de mise à niveau de l'exécution de la porte (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Un crochet:
  - `allow` (bool, `true` par défaut) : par rapport à `false`, les instructions de mise à niveau du runtime s'affichent.
  - `require_metadata` (bool, par défaut `false`) : traite les métadonnées d'entrée, en utilisant `metadata_key`.
  - `metadata_key` (string) : métadonnées, hook forcé. Par défaut `gov_upgrade_id`, ces métadonnées sont disponibles ou sont une liste blanche.
  - `allowed_ids` (tableau de chaînes) : liste d'autorisation facultative pour les métadonnées (après trim). Отклоняет, когда предоставленное значение не входит в список.
- Lorsque le crochet est utilisé, l'admission permet d'appliquer les métadonnées en fonction de la transition en cours. Les métadonnées disponibles peuvent être sélectionnées ou ajoutées à une liste verte pour déterminer NotPerowed.
- Les résultats sont obtenus par télémétrie à partir de `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Les transitions, les hooks activés, permettent d'obtenir les métadonnées `gov_upgrade_id=<value>` (ou le bouton du manifeste) pour les approbations des validateurs, требуемыми manifeste quorum.Point de terminaison pratique
- POST `/v1/gov/protected-namespaces` - indiquez `gov_protected_namespaces` sur le nœud.
  - Requête : { "espaces de noms": ["apps", "system"] }
  - Réponse : { "ok": vrai, "appliqué": 1 }
  - Notes : prévu pour l'administrateur/les tests ; Utilisez le jeton API pour la configuration. Lors de la production, le transfert est prévu avec `SetParameter(Custom)`.Aides CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Utilisez les instances de contrat pour l'espace de noms et la preuve, à savoir :
    - Torii fournit le bytecode pour le numéro `code_hash`, et par exemple Blake2b-32 digest correspond à `code_hash`.
    - Le module manifeste `/v1/contracts/code/{code_hash}` correspond à la compatibilité `code_hash` et `abi_hash`.
    - La proposition de gouvernance adoptée pour `(namespace, contract_id, code_hash, abi_hash)`, dérivée du hachage d'identifiant de proposition, est utilisée maintenant.
  - Vous pouvez trouver JSON avec `results[]` sur le contrat (problèmes, résumés de manifeste/code/proposition) et un résumé détaillé, si cela n'est pas possible (`--no-summary`).
  - Permet de contrôler les espaces de noms protégés ou de fournir des workflows de déploiement contrôlés par la gouvernance.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Obtenez les métadonnées du squelette JSON pour le déploiement dans les espaces de noms protégés, en utilisant `gov_manifest_approvers` pour la vérification du quorum dans le manifeste.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — les astuces de verrouillage sont disponibles pour `min_bond_amount > 0`, et les astuces pour verrouiller `owner`, `amount` et `duration_blocks`.
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).
  - L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.- Le résumé des détails concerne le détergent `fingerprint=<hex>` et le code `CastZkBallot` avec les conseils de décodage (`owner`, `amount`, `duration_blocks`, `direction` pour les réglages).
  - La CLI publie l'annonce `tx_instructions[]` sur le pôle `payload_fingerprint_hex` et les composants de décodage, ce qui permet aux outils en aval de vérifier le squelette sans la réalisation officielle Décodage Norito.
  - Les conseils de verrouillage prédéfinis permettent d'émettre le nœud `LockCreated`/`LockExtended` pour les bulletins de vote ZK, afin que le schéma soit défini pour votre vote.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Les noms `--lock-amount`/`--lock-duration-blocks` affichent des drapeaux ZK pour la parité dans les scripts.
  - Résumé вывод отражает `vote --mode zk`, включая empreinte digitale закодированной инструкции и читаемые bulletin de vote (`owner`, `amount`, `duration_blocks`, `direction`), vous devez mettre à jour le squelette avant de l'utiliser.Liste des instances
- GET `/v1/gov/instances/{ns}` - indique l'activation des contrats pour l'espace de noms.
  - Paramètres de requête :
    - `contains` : filtre pour la sous-chaîne `contract_id` (sensible à la casse)
    - `hash_prefix` : filtre pour le préfixe hexadécimal `code_hash_hex` (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`
  - Réponse : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Assistant SDK : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Déverrouiller le balayage (opérateur/audit)
- OBTENIR `/v1/gov/unlocks/stats`
  - Réponse : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes : `last_sweep_height` augmente la hauteur du bloc après que les verrous expirés soient balayés et persistent. `expired_locks_now` permet d'analyser les enregistrements de verrouillage avec `expiry_height <= height_current`.
- POSTE `/v1/gov/ballots/zk-v1`
  - Requête (DTO de style v1) :
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
  - Utilisez `BallotProof` JSON pour créer et utiliser le squelette `CastZkBallot`.
  - Demande :
    {
      "autorité": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // conteneur base64 ZK1 ou H2*
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // facultatif AccountId et le propriétaire du circuit
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
    - Carte du serveur en option `root_hint`/`owner`/`nullifier` pour le scrutin `public_inputs_json` pour `CastZkBallot`.
    - Les octets d'enveloppe sont généralement codés en base64 pour les instructions de charge utile.
    - `reason` correspond à `submitted transaction` et Torii supprime le bulletin de vote.
    - Ce point de terminaison est installé uniquement avec la fonctionnalité `zk-ballot`.Chemin de vérification CastZkBallot
- `CastZkBallot` fournit une preuve base64 avancée et libère des charges utiles puissantes/bites (`BallotRejected` avec `invalid or empty proof`).
- L'hôte a utilisé la clé de vérification des bulletins de vote pour le référendum (`vk_ballot`) ou les valeurs par défaut de gouvernance et a travaillé en utilisant `Active` et les octets en ligne.
- Les octets de clé de vérification повторно хешируются с `hash_vk` ; L'engagement ne nécessite pas de vérification des entrées de registre appropriées (`BallotRejected` et `verifying key commitment mismatch`).
- Les octets de preuve sont supprimés dans le backend enregistré par `zk::verify_backend` ; les relevés de notes invalides correspondent à `BallotRejected` avec `invalid proof` et aux instructions de détermination.
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- Успешные preuves эмитят `BallotAccepted`; Les annulateurs automatiques, l'utilisation des racines d'éligibilité ou le verrouillage de la régression permettent de fournir des options de recherche pertinentes, selon la gamme.

## Ненадлежащее поведение валидаторов и совместный консенсус

### Procès de réduction et d'emprisonnementLe consensus émet le code Norito `Evidence` pour la validation du protocole. Cette charge utile est transférée dans la mémoire `EvidenceStore` et, si elle est nouvelle, matérialisée dans le `consensus_evidence` soutenu par WSV. Les écrans de l'écran `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocs `7200` par défaut) s'ouvrent, les archives sont ouvertes, mais ne peuvent pas être ouvertes pour la journalisation. opérateurs. Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

La procédure à suivre est celle du numéro `EvidenceKind` ; Spécifications stables et spécifications principales du modèle de données :

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

- **DoublePrepare/DoubleCommit** - Le validateur est en mesure de confiner vos données pour l'utilisateur `(phase,height,view,epoch)`.
- **InvalidQc** - l'agrégateur a défini le certificat de validation, ce formulaire ne permet pas de déterminer les preuves (par exemple, le bitmap du signataire autorisé).
- **InvalidProposal** - Le bloc précédent indique que la structure est validée (par exemple, la règle de la chaîne verrouillée).
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

Les opérateurs et les instruments peuvent gérer et libérer les charges utiles ici :

- Torii : `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.La gouvernance doit regrouper les octets de preuves en fonction des documents canoniques :

1. **Assurez-vous de la charge utile** pour l'installation. L'archive Norito octets contient les métadonnées de hauteur/vue.
2. ** Ajouter la charge utile lors d'un référendum ou d'une instruction sudo (par exemple, `Unregister::peer`). Исполнение повторно валидирует la charge utile ; des preuves mal formées ou périmées sont détectées.
3. **Запланировать suivi топологию** чтобы нарушивший валидатор не смог сразу вернуться. Les types de photos sont `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` sur la liste actuelle.
4. **Voir les résultats** concernant `/v1/sumeragi/evidence` et `/v1/sumeragi/status`, ce qui concerne les preuves scientifiques et la gouvernance.

### Последовательность commun-consensus

Le consensus commun garantit que les validateurs finaliseront le bloc de travail pour que le nouveau projet soit effectué. L'application d'exécution est effectuée à partir des paramètres suivants :

- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` doivent être commis dans **votre bloc**. `mode_activation_height` s'occupe du plus grand bloc de votre appareil, celui-ci étant mis à jour, en respectant un minimum de bloc.
- `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) - c'est une garde de configuration qui prévient le transfert avant la fin :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```- Runtime et CLI permettent de configurer les paramètres par étapes à partir de `/v1/sumeragi/params` et `iroha --output-format text ops sumeragi params`, permettant aux opérateurs de modifier les hauteurs d'activation et les validateurs de la liste.
- La gouvernance automatique est complète :
  1. Финализировать решение об исключении (или восстановлении), подддержанное preuve.
  2. Postez la reconfiguration de suivi avec `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveillez `/v1/sumeragi/status` en sélectionnant `effective_consensus_mode` pour votre appareil.

Un script clair, qui permet de faire tourner les validateurs ou de utiliser le slashing, **ne vient pas** d'activer l'activation sans décalage ou d'activer les paramètres de transfert ; Les transitions s'ouvrent et s'effectuent selon le régime précédent.

## Surfaces de télémétrie- Prometheus metrics экспортируют активность gouvernance :
  - `governance_proposals_status{status}` (jauge) отслеживает количество propositions по статусу.
  - `governance_protected_namespace_total{outcome}` (compteur) est utilisé pour autoriser/rejeter l'admission pour les espaces de noms protégés.
  - `governance_manifest_activations_total{event}` (compteur) fixe le manifeste (`event="manifest_inserted"`) et les liaisons d'espace de noms (`event="instance_bound"`).
- `/status` inclut l'objet `governance`, qui permet de sélectionner les propositions, d'obtenir les totaux des espaces de noms protégés et de ne pas activer les manifestes (espace de noms, contrat). identifiant, code/hachage ABI, hauteur de bloc, horodatage d'activation). Les opérateurs peuvent s'occuper de ce domaine, en ce qui concerne les textes publiés dans les manifestes et les portes d'espace de noms protégées.
- Modèle Grafana (`docs/source/grafana_governance_constraints.json`) et runbook de télémétrie dans `telemetry.md` pour découvrir les propositions de propositions, les activations de manifeste proposées ou Il n'est pas possible d'ouvrir les espaces de noms protégés lors des mises à niveau du runtime.