---
lang: fr
direction: ltr
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

حالت: گورننس نفاذی کاموں کے ساتھ چلنے والا ڈرافٹ/اسکیچ۔ عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔ Déterminisme اور RBAC پالیسی معیاری پابندیاں ہیں؛ جب `authority` اور `private_key` فراہم ہوں تو Torii ٹرانزیکشن سائن/سبمٹ کر سکتا ہے، ورہ کلائنٹس بنا کر `/transaction` پر سبمٹ کرتے ہیں۔جائزہ
- Les points de terminaison JSON sont également pris en compte. ٹرانزیکشن بنانے والے فلو کے لئے جوابات میں `tx_instructions` ہوتے ہیں - ایک یا زیادہ squelettes d'instruction Un tableau :
  - `wire_id` : instruction pour l'identifiant du registre
  - `payload_hex` : octets de charge utile Norito (hexadécimal)
- `authority` et `private_key` (les DTO de vote sont `private_key`) et Torii sont disponibles اور سبمٹ کرتا ہے اور پھر بھی `tx_instructions` واپس کرتا ہے۔
- L'autorité de certification et chain_id sont connectés à SignedTransaction en utilisant le code POST `/transaction`. ہیں۔
- Couverture du SDK :
- Python (`iroha_python`) : `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` et champ d'état (champs de statut/type et normalisation) et `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے، `ToriiClient.get_governance_tally_typed` `GovernanceTally` واپس کرتا ہے، `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` واپس کرتا ہے، `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` est une surface de gouvernance et `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage` est une surface de gouvernance پر accès typé ملتا ہے اور README میں exemples d'utilisation دیے گئے ہیں۔
- Client léger Python (`iroha_torii_client`) : `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` ont tapé les bundles `GovernanceInstructionDraft` et les paquets (Torii) `tx_instructions` squelette et envelopper les scripts Finaliser/Enacter les flux et l'analyse manuelle JSON pour les scripts- JavaScript (`@iroha/iroha-js`) : propositions `ToriiClient`, référendums, décomptes, verrous, statistiques de déverrouillage et aides saisies pour les points de terminaison du conseil `listGovernanceInstances(namespace, options)` (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) Vous pouvez télécharger les clients Node.js `/v1/gov/instances/{ns}` et paginer ici Voir les workflows soutenus par VRF et la liste des instances de contrat

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
  - Validation : les nœuds utilisent `abi_version` et `abi_hash` pour canoniser les erreurs et les discordances et rejeter les erreurs. `abi_version = "v1"` indique la valeur `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`API Contrats (déploiement)
- POSTE `/v1/contracts/deploy`
  - Requête : { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportement : IVM en-tête `code_hash` en-tête `abi_version` en `abi_hash` en-tête `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (octets `.to`) `authority` est en cours de lecture.
  - Réponse : { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Connexes :
    - GET `/v1/contracts/code/{code_hash}` -> ذخیرہ شدہ manifeste واپس کرتا ہے
    - OBTENEZ `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` et cliquez ici
- POSTE `/v1/contracts/instance`
  - Requête : { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportement : Déploiement du bytecode par `ActivateContractInstance` et mappage `(namespace, contract_id)` par `(namespace, contract_id)`.
  - Réponse : { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Service d'alias
- POSTE `/v1/aliases/voprf/evaluate`
  - Requête : { "blinded_element_hex": "..." }
  - Réponse : { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - Implémentation de l'évaluateur `backend` pour le projet Valeur nominale : `blake2b512-mock`۔
  - Notes : l'évaluateur fictif déterministe et Blake2b512 et la séparation de domaine `iroha.alias.voprf.mock.v1` s'appliquent également. outillage de test pour la production pipeline VOPRF Iroha fil de fer
  - Erreurs : entrée hexadécimale mal formée par HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` enveloppe et message d'erreur du décodeur et message d'erreur
- POSTE `/v1/aliases/resolve`
  - Demande : { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Réponse : { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Remarques : mise en scène du runtime du pont ISO (`[iso_bridge.account_aliases]` dans `iroha_config`) Torii espace en majuscule pour la recherche en majuscule alias 404 et ISO Bridge Runtime 503 et 503
- POSTE `/v1/aliases/resolve_index`
  - Requête : { "index": 0 }
  - Réponse : { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - Notes : ordre de configuration des indices d'alias et ordre déterministe et assigner ہوتے (basé sur 0)۔ Cache hors ligne, événements d'attestation d'alias, pistes d'audit et pistes d'audit.Code Taille Casquette
- Paramètre personnalisé : `max_contract_code_bytes` (JSON u64)
  - Stockage du code de contrat en chaîne
  - Par défaut : 16 Mo۔ L'image `.to` est associée à des nœuds `RegisterSmartContractBytes` et une erreur de violation invariante est rejetée.
  - Opérateurs `SetParameter(Custom)` pour la charge utile numérique `id = "max_contract_code_bytes"` pour la charge utile numérique et les opérateurs

- POSTE `/v1/gov/ballots/zk`
  - Requête : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques :
    - Le circuit des entrées publiques est `owner`, `amount`, `duration_blocks` et la preuve configurée VK est pour vérifier le nœud. `election_id` pour le verrouillage de gouvernance pour le verrouillage de la gouvernance direction چھپی رہتی ہے (`unknown`) ; صرف montant/expiration اپڈیٹ ہوتے ہیں۔ re-vote monotone ہیں: montant et expiration صرف بڑھتے ہیں (nœud max(montant, montant précédent) et max(expiration, expiration précédente) لگاتا ہے)۔
    - ZK revoit le montant et l'expiration du diagnostic `BallotRejected` côté serveur et le rejet du rejet.
    - Exécution du contrat کو `SubmitBallot` mise en file d'attente کرنے سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا لازم ہے؛ les hôtes one-shot loquet sont appliqués- POSTE `/v1/gov/ballots/plain`
  - Requête : { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Oui|Non|S'abstenir" }
  - Réponse : { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Remarques : les nouveaux votes ne sont étendus que - Le bulletin de vote est verrouillé pour le montant et l'expiration pour le vote. `owner` autorité de transaction pour le droit des transactions Il s'agit d'un `conviction_step_blocks` ہے۔- POSTE `/v1/gov/finalize`
  - Requête : { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Effet en chaîne (échafaudage actuel) : il s'agit de déployer une proposition et de promulguer un `code_hash` minimal à clé `ContractManifest`. `abi_hash` et proposition Adoptée en vigueur اگر `code_hash` کے لئے مختلف `abi_hash` والا manifeste پہلے سے ہو تو promulgation rejet ہوتا ہے۔
  - Remarques :
    - Élections ZK pour les chemins de contrat کو `FinalizeElection` et `ZK_VOTE_VERIFY_TALLY` pour le contrat de travail. les hôtes one-shot loquet sont appliqués `FinalizeReferendum` Les référendums ZK rejettent le décompte des élections finalisé نہ ہو جائے۔
    - Fermeture automatique `h_end` pour référendums simples pour émettre approuvé/rejeté Référendums ZK Fermés رہتے ہیں جب تک décompte finalisé soumettre نہ ہو اور `FinalizeReferendum` exécuter نہ ہو۔
    - Chèques de participation صرف approuver+rejeter استعمال کرتی ہیں؛ s'abstenir de participer میں شمار نہیں ہوتا۔- POSTE `/v1/gov/enact`
  - Requête : { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Réponse : { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes : `authority`/`private_key` est une transaction signée et Torii est une transaction signée. ورنہ وہ squelette واپس کرتا ہے جسے کلائنٹ سائن اور سبمٹ کرے۔ preimage اختیاری اور فی الحال معلوماتی ہے۔

- OBTENIR `/v1/gov/proposals/{id}`
  - Chemin `{id}` : identifiant de proposition hexadécimal (64 caractères)
  - Réponse : { "found": bool, "proposal": { ... } ? }

- OBTENIR `/v1/gov/locks/{rid}`
  - Chemin `{rid}` : chaîne d'identification du référendum
  - Réponse : { "found": bool, "referendum_id": "rid", "locks": { ... } ? }

- OBTENIR `/v1/gov/council/current`
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes : Le conseil municipal a des seuils d'actifs de participation configurés et des dérivées de secours déterministes (spécifications VRF sont également disponibles). Cela reflète les preuves que les preuves VRF en direct sur la chaîne persistent (voir)۔- POST `/v1/gov/council/derive-vrf` (fonctionnalité : gov_vrf)
  - Requête : { "committee_size": 21, "epoch": 123 ? , "candidats": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportement : Il s'agit d'une preuve VRF `chain_id`, `epoch` et d'une balise de hachage de bloc, ainsi que d'une entrée canonique et d'une vérification de la balise de hachage de bloc. octets de sortie et desc pour les départages et les tris meilleurs membres de `committee_size` et کرتا ہے۔ Persister نہیں کرتا۔
  - Réponse : { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notes : Normal = pk en G1, preuve en G2 (96 octets). Petit = pk en G2, preuve en G1 (48 octets). Entrées séparées par domaines ہیں اور `chain_id` ہے۔

### Paramètres de gouvernance par défaut (iroha_config `gov.*`)

Torii est une liste persistante et le conseil de secours `iroha_config` permet de paramétrer les paramètres :

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

Remplacements d'environnement équivalent :

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

`parliament_committee_size` membres de repli sont définis comme limite et le conseil persiste et `parliament_term_blocks` longueur d'époque définissent et dérivation des graines استعمال ہوتا ہے (`epoch = floor(height / term_blocks)`) et `parliament_min_stake` actif éligible pour la participation (les plus petites unités) appliquer کرتا ہے، اور `parliament_eligibility_asset_id` منتخب Il y a plusieurs candidats à la recherche d'un solde d'actifs et d'un scan d'un compteVérification VK de gouvernance et contournement ici : vérification des bulletins de vote `Active` vérification de la clé et des octets en ligne pour les environnements et les environnements et les bascules de test uniquement نہیں کرنا چاہئے۔

RBAC
- Exécution en chaîne et autorisations nécessaires :
  - Propositions : `CanProposeContractDeployment{ contract_id }`
  - Bulletins de vote : `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgation : `CanEnactGovernance`
  - Gestion communale (future) : `CanManageParliament`Espaces de noms protégés
- Le paramètre personnalisé `gov_protected_namespaces` (tableau JSON de chaînes) répertorie les espaces de noms pour le déploiement et le contrôle d'admission.
- Les clients et les espaces de noms protégés déploient des clés de métadonnées de transaction pour les utilisateurs :
  - `gov_namespace` : espace de noms cible (pour : "apps")
  - `gov_contract_id` : espace de noms et identifiant de contrat logique
- `gov_manifest_approvers` : tableau JSON facultatif d'ID de compte de validateur۔ جب lane quorum manifeste > 1 déclarer کرے تو admission کے لئے transaction Authority اور comptes répertoriés دونوں درکار ہوتے ہیں تاکہ manifeste quorum پورا ہو سکے۔
- Télémétrie `governance_manifest_admission_total{result}` pour les compteurs d'admission holistiques et les opérateurs de réseau admettent `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, et `runtime_hook_rejected` sont également disponibles
- Télémétrie `governance_manifest_quorum_total{outcome}` (valeurs `satisfied` / `rejected`) pour le chemin d'application des règles et pour les approbations manquantes et l'audit
- Lanes manifeste une liste d'autorisation d'espace de noms et applique la loi. Il s'agit du fichier `gov_namespace` et du manifeste `gov_contract_id` pour l'espace de noms et le manifeste `protected_namespaces`. سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` soumissions pour les métadonnées et la protection activer et rejeter
- Admission pour appliquer la proposition de gouvernance adoptée `(namespace, contract_id, code_hash, abi_hash)` pour la proposition de gouvernance adoptée Erreur de validation NotPerowed et échec de la validationHooks de mise à niveau d’exécution
- Les manifestes de voie `hooks.runtime_upgrade` déclarent les instructions de mise à niveau de l'exécution (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) et la porte d'entrée.
- Champs de crochet :
  - `allow` (bool, `true` par défaut) : par `false` et les instructions de mise à niveau de l'exécution sont rejetées et rejetées.
  - `require_metadata` (bool, `false` par défaut) : `metadata_key` est une entrée de métadonnées supplémentaire.
  - `metadata_key` (chaîne) : crochet et nom des métadonnées appliqué Par défaut `gov_upgrade_id` pour les métadonnées requises et la liste d'autorisation disponible ici
  - `allowed_ids` (tableau de chaînes) : valeurs de métadonnées et liste d'autorisation facultative (trim کے بعد)۔ La valeur de la valeur est rejetée et rejetée
- Hook موجود ہو تو file d'admission ٹرانزیکشن کے file d'attente میں جانے سے پہلے la politique de métadonnées est appliquée کرتا ہے۔ Métadonnées manquantes, valeurs ou liste d'autorisation et valeurs déterministes Erreur NotPerowed
- Télémétrie `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` pour le suivi des résultats de la télémétrie
- Hook prend en charge les métadonnées `gov_upgrade_id=<value>` (clé définie par le manifeste) pour quorum manifeste. مطابق validateurs کی approbations بھی درکار ہیں۔Point de terminaison pratique
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` pour le nœud de connexion pour appliquer le message
  - Requête : { "espaces de noms": ["apps", "system"] }
  - Réponse : { "ok": vrai, "appliqué": 1 }
  - Notes : administration/tests Configurer le jeton API ici production کے لئے `SetParameter(Custom)` کے ساتھ transaction signée ترجیح دیں۔Aides CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - l'espace de noms et les instances de contrat récupèrent les éléments et effectuent des vérifications croisées :
    - Torii et `code_hash` sont le bytecode qui correspond à la correspondance avec Blake2b-32 digest `code_hash`. ہے۔
    - `/v1/contracts/code/{code_hash}` correspond au manifeste correspondant aux valeurs `code_hash` et `abi_hash`.
    - `(namespace, contract_id, code_hash, abi_hash)` pour la proposition de gouvernance adoptée et pour le hachage de l'identifiant de la proposition et pour dériver et pour le nœud.
  - `results[]` pour JSON رپورٹ دیتا ہے (problèmes, résumés de manifeste/code/proposition) et pour la version `--no-summary` نہ ہو)۔
  - Espaces de noms protégés, audit et workflows de déploiement contrôlés par la gouvernance.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Espaces de noms protégés pour le déploiement et le squelette de métadonnées JSON et les règles de quorum manifestes en option `gov_manifest_approvers`.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` Conseils de verrouillage pour les conseils de verrouillage et conseils de verrouillage `owner`, `amount` et `duration_blocks` en anglais
  - Valide les identifiants de compte canoniques, canonise les indices d'annulation de 32 octets et fusionne les indices dans `public_inputs_json` (avec `--public <path>` pour des remplacements supplémentaires).- L'annulateur est dérivé de l'engagement de preuve (entrée publique) plus `domain_tag`, `chain_id` et `election_id` ; `--nullifier` est validé par rapport à l'épreuve à la livraison.
  - Il s'agit d'un code déterministe `fingerprint=<hex>` et d'indices codés `CastZkBallot` qui dérivent des indices décodés. (`owner`, `amount`, `duration_blocks`, `direction` en anglais)۔
  - Réponses CLI `tx_instructions[]` et `payload_fingerprint_hex` pour les champs décodés pour annoter le squelette d'outillage en aval et pour le décodage Norito et vérifier کر سکے۔
  - Conseils de verrouillage pour les bulletins de vote du nœud ZK et les événements `LockCreated`/`LockExtended` émettent un circuit et les valeurs exposent.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` alias drapeaux ZK et miroir et parité de script
  - Sortie récapitulative `vote --mode zk` avec instruction codée d'empreintes digitales et champs de vote lisibles (`owner`, `amount`, `duration_blocks`, `direction`) جس سے signature سے پہلے فوری تصدیق ہو جاتی ہے۔Liste des instances
- GET `/v1/gov/instances/{ns}` - espace de noms pour les instances de contrat actives
  - Paramètres de requête :
    - `contains` : `contract_id` sous-chaîne et filtre de filtre (sensible à la casse)
    - `hash_prefix` : `code_hash_hex` avec préfixe hexadécimal et filtre (minuscule)
    - `offset` (0 par défaut), `limit` (100 par défaut, 10_000 maximum)
    - `order` : `cid_asc` (par défaut), `cid_desc`, `hash_asc`, `hash_desc`
  - Réponse : { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Assistant SDK : `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python) ۔Déverrouiller le balayage (opérateur/audit)
- OBTENIR `/v1/gov/unlocks/stats`
  - Réponse : { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Remarques : `last_sweep_height` indique la hauteur du bloc et les verrous expirés sont balayés et persistants. `expired_locks_now` pour verrouiller les enregistrements et scanner les fichiers `expiry_height <= height_current`
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
  - `BallotProof` JSON pour le squelette `CastZkBallot` et le squelette `CastZkBallot`
  - Demande :
    {
      "autorité": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "vote": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Conteneur ZK1 ou H2* en base64
        "root_hint": null, // chaîne hexadécimale facultative de 32 octets (racine d'éligibilité)
        "owner": null, // facultatif AccountId et le propriétaire du circuit commit ici
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
    - Serveur en option `root_hint`/`owner`/`nullifier` avec bulletin de vote et `CastZkBallot` avec carte `public_inputs_json`
    - Octets d'enveloppe et charge utile d'instruction en base64 pour encoder en mode base64
    - Pour soumettre le bulletin de vote Torii et `reason` pour `submitted transaction` ou `submitted transaction`.
    - Le point de terminaison est activé et la fonctionnalité `zk-ballot` est activée.Chemin de vérification CastZkBallot
- `CastZkBallot` décode la preuve base64 pour décoder les charges utiles et rejeter les charges utiles (`BallotRejected` avec `invalid or empty proof`).
- Organiser un référendum (`vk_ballot`) pour les défauts de gouvernance et le vote vérifiant la résolution des clés et pour l'enregistrement en ligne `Active` octets رکھتا ہو۔
- Octets de clé de vérification stockés comme `hash_vk` pour le hachage de la clé de vérification incompatibilité d'engagement et vérification et exécution et entrées de registre falsifiées (`BallotRejected` avec `verifying key commitment mismatch`).
- Octets de preuve `zk::verify_backend` pour le backend enregistré et l'envoi des fichiers transcriptions invalides `BallotRejected` avec `invalid proof` کے ساتھ ظاہر ہوتے ہیں اور l'instruction échoue de manière déterministe ہوتی ہے۔
- La preuve doit exposer un engagement de vote et une racine d'éligibilité en tant qu'apports publics ; la racine doit correspondre au `eligible_root` de l’élection et l’annulateur dérivé doit correspondre à tout indice fourni.
- Les preuves réussies `BallotAccepted` émettent کرتے ہیں؛ Annulateurs en double, racines d'éligibilité obsolètes, régressions de verrouillage et raisons de rejet.

## Mauvais comportement du validateur et consensus commun

### Workflow de réduction et d'emprisonnementLe validateur de consensus est en mesure d'émettre un code Norito codé en `Evidence`. La charge utile en mémoire `EvidenceStore` est une carte `consensus_evidence` soutenue par WSV qui se matérialise. جاتا ہے۔ `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocs `7200` par défaut) le rejet du rejet et le rejet de l'archive délimitée par les opérateurs et le journal کیا جاتا ہے۔ Les preuves dans l'horizon respectent également `sumeragi.npos.reconfig.activation_lag_blocks` (par défaut `1`) et le délai de barre oblique `sumeragi.npos.reconfig.slashing_delay_blocks` (par défaut `259200`) ; la gouvernance peut annuler les pénalités avec `CancelConsensusEvidencePenalty` avant que la réduction ne s'applique.

Infractions reconnues `EvidenceKind` sur carte individuelle discriminants مستحکم ہیں اور modèle de données کے ذریعے appliquer ہوتے ہیں :

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

- **DoublePrepare/DoubleCommit** - validateur pour `(phase,height,view,epoch)` qui utilise des hachages pour les utilisateurs.
- **InvalidQc** - l'agrégateur a des potins sur les certificats de validation et des contrôles déterministes ont échoué (bitmap du signataire)
- **InvalidProposal** - le leader a proposé de proposer une validation structurelle ou d'échouer (comme la règle de la chaîne verrouillée)
- **Censure** : les reçus de soumission signés montrent une transaction qui n'a jamais été proposée/engagée.

Les opérateurs chargés des charges utiles d'outillage inspectent et rediffusent les informations :- Torii : `GET /v1/sumeragi/evidence` et `GET /v1/sumeragi/evidence/count`.
- CLI : `iroha ops sumeragi evidence list`, `... count`, et `... submit --evidence-hex <payload>`.

Gouvernance, octets de preuves, preuve canonique, et traiter les choses :

1. **Payload جمع کریں** اس کے expire ہونے سے پہلے۔ raw Norito octets et métadonnées de hauteur/vue et d'archives
2. **Étape de pénalité** charge utile et référendum et instruction sudo pour intégrer l'application (مثلا `Unregister::peer`)۔ Charge utile d'exécution et validation de la charge utile mal formé یا preuves périmées rejeter de manière déterministe ہوتی ہے۔
3. **Calendrier de topologie de suivi ** ** Validateur incriminé فوراً واپس نہ آ سکے۔ Les flux sont `SetParameter(Sumeragi::NextMode)` et `SetParameter(Sumeragi::ModeActivationHeight)` liste mise à jour dans la file d'attente et dans la file d'attente
4. **Fixation d'audit** `/v1/sumeragi/evidence` et `/v1/sumeragi/status` pour le compteur de preuves pour la gouvernance et la suppression des preuves

### Séquençage conjoint-consensuel

Consensus commun Il s'agit d'un validateur sortant définissant un bloc de délimitation finalisant un projet de proposition d'un ensemble de propositions. Les paramètres d'exécution et les règles d'application des règles sont :- `SumeragiParameter::NextMode` et `SumeragiParameter::ModeActivationHeight` pour **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` mise à jour pour une hauteur et un décalage strictement limités
- Garde de configuration `sumeragi.npos.reconfig.activation_lag_blocks` (`1` par défaut) et transferts sans décalage et transferts :
- `sumeragi.npos.reconfig.slashing_delay_blocks` (`259200` par défaut) retarde la réduction du consensus afin que la gouvernance puisse annuler les pénalités avant qu'elles ne s'appliquent.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Runtime et paramètres par étapes CLI comme `/v1/sumeragi/params` et `iroha --output-format text ops sumeragi params` pour les hauteurs d'activation des opérateurs et les listes de validateurs. کر سکیں۔
- Automatisation de la gouvernance par exemple :
  1. preuve پر مبنی suppression (یا réintégration) فیصلہ finaliser کرنا چاہیے۔
  2. `mode_activation_height = h_current + activation_lag_blocks` pour la file d'attente de reconfiguration de suivi
  3. `/v1/sumeragi/status` pour interrupteur de hauteur et interrupteur `effective_consensus_mode` pour interrupteur de hauteur

Les validateurs de script tournent et slashing s'appliquent automatiquement ** activation sans décalage ** et les paramètres de transfert ou omettent les différents scripts. Les transactions sont rejetées en mode میں رہتا ہے۔

## Surfaces de télémétrie- Prometheus metrics governance activity export کرتے ہیں:
  - `governance_proposals_status{status}` (gauge) proposals کی گنتی status کے حساب سے track کرتا ہے۔
  - `governance_protected_namespace_total{outcome}` (compteur) pour incrémenter et pour créer des espaces de noms protégés pour l'admission et le déploiement, pour autoriser et pour rejeter
  - `governance_manifest_activations_total{event}` (counter) manifest insertions (`event="manifest_inserted"`) اور namespace bindings (`event="instance_bound"`) record کرتا ہے۔
- `/status` ou `governance` objet et le nombre de propositions reflètent le rapport total des espaces de noms protégés et les activations manifestes récentes (espace de noms, identifiant de contrat, code/hachage ABI, hauteur de bloc, activation timestamp) liste کرتا ہے۔ Opérateurs de champ et de sondage et de textes de loi et de manifestes et de portes d'espace de noms protégées.
- Modèle Grafana (`docs/source/grafana_governance_constraints.json`) et `telemetry.md` avec runbook de télémétrie, propositions bloquées, activations de manifeste manquantes, mises à niveau d'exécution et rejets inattendus d'espaces de noms protégés. لئے alertes کیسے fil کئے جائیں۔