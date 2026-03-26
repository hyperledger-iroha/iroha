---
lang: fr
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:01:14.866000+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Appel système ABI

Ce document définit les numéros d'appel système IVM, les conventions d'appel pointeur-ABI, les plages de numéros réservées et le tableau canonique des appels système orientés contrat utilisés par l'abaissement Kotodama. Il complète `ivm.md` (architecture) et `kotodama_grammar.md` (langage).

Gestion des versions
- L'ensemble des appels système reconnus dépend du champ d'en-tête `abi_version` du bytecode. La première version accepte uniquement `abi_version = 1` ; les autres valeurs sont rejetées à l'admission. Numéros inconnus pour le piège déterministe `abi_version` actif avec `E_SCALL_UNKNOWN`.
- Les mises à niveau d'exécution conservent `abi_version = 1` et n'étendent pas les surfaces d'appel système ou de pointeur-ABI.
- Les coûts de gaz Syscall font partie du calendrier de gaz versionné lié à la version d'en-tête du bytecode. Voir `ivm.md` (Politique du gaz).

Plages de numérotation
- `0x00..=0x1F` : noyau/utilitaire de VM (les assistants de débogage/de sortie sont disponibles sous `CoreHost` ; les assistants de développement restants sont uniquement des hôtes fictifs).
- `0x20..=0x5F` : pont ISI core Iroha (stable dans ABI v1).
- `0x60..=0x7F` : extension ISI contrôlée par les fonctionnalités du protocole (fait toujours partie d'ABI v1 lorsqu'elle est activée).
- `0x80..=0xFF` : assistants hôte/crypto et emplacements réservés ; seuls les numéros présents dans la liste blanche ABI v1 sont acceptés.

Aides durables (ABI v1)
- Les appels système d'assistance d'état durable (0x50–0x5A : STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) font partie de l'ABI V1 et sont inclus dans le calcul `abi_hash`.
- CoreHost connecte STATE_{GET,SET,DEL} à l'état de contrat intelligent durable soutenu par WSV ; Les hôtes de développement/test peuvent persister localement mais doivent conserver une sémantique d’appel système identique.

Convention d'appel Pointer‑ABI (appels système de contrat intelligent)
- Les arguments sont placés dans les registres `r10+` en tant que valeurs brutes `u64` ou en tant que pointeurs dans la région INPUT vers des enveloppes TLV Norito immuables (par exemple, `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`).
- Les valeurs de retour scalaires sont les `u64` renvoyées par l'hôte. Les résultats du pointeur sont écrits par l'hôte dans `r10`.

Table d'appels système canonique (sous-ensemble)| Hex | Nom | Arguments (dans `r10+`) | Retours | Gaz (base + variable) | Remarques |
|------|----------------------------|------------------------------------------------------------------------------|-----------------------|------------------------------|-------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`, `&Name`, `&Json` | `u64=0` | `G_set_detail + bytes(val)` | Écrit un détail pour le compte |
| 0x22 | MINT_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Monnaies `amount` d'actif à compte |
| 0x23 | BURN_ASSET | `&AccountId`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | Brûle `amount` du compte |
| 0x24 | TRANSFERT_ASSET | `&AccountId(from)`, `&AccountId(to)`, `&AssetDefinitionId`, `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | Virements `amount` entre comptes |
| 0x29 | TRANSFER_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | Commencer la portée du lot de transfert FASTPQ |
| 0x2A | TRANSFER_V1_BATCH_END | – | `u64=0` | `G_transfer` | Vider le lot de transfert FASTPQ accumulé |
| 0x2B | TRANSFER_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Appliquer un lot codé Norito dans un seul appel système |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | Enregistre un nouveau NFT |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`, `&NftId`, `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | Transfère la propriété de NFT |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | Met à jour les métadonnées NFT |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | Brûle (détruit) un NFT |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | Les requêtes itérables s'exécutent de manière éphémère ; `QueryRequest::Continue` rejeté |
| 0xA2 | CREATE_NFTS_FOR_ALL_USERS | – | `u64=count` | `G_create_nfts_for_all` | Auxiliaire; limité aux fonctionnalités || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | Administrateur ; limité aux fonctionnalités |
| 0xA4 | GET_AUTHORITÉ | – (l'hôte écrit le résultat) | `&AccountId`| `G_get_auth` | L'hôte écrit le pointeur vers l'autorité actuelle dans `r10` |
| 0xF7 | GET_MERKLE_PATH | `addr:u64`, `out_ptr:u64`, en option `root_out:u64` | `u64=len` | `G_mpath + len` | Écrit le chemin (feuille → racine) et les octets racine facultatifs |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`, `out_ptr:u64`, en option `depth_cap:u64`, en option `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`, `out_ptr:u64`, en option `depth_cap:u64`, en option `root_out:u64` | `u64=depth` | `G_mpath + depth` | Même présentation compacte pour l'engagement du registre |

Application du gaz
- CoreHost facture du gaz supplémentaire pour les appels système ISI en utilisant le calendrier ISI natif ; Les transferts par lots FASTPQ sont facturés par entrée.
- Les appels système ZK_VERIFY réutilisent le programme de gaz de vérification confidentiel (base + taille de preuve).
- SMARTCONTRACT_EXECUTE_QUERY facture la base + par article + par octet ; le tri multiplie le coût par article et les compensations non triées ajoutent une pénalité par article.

Remarques
- Tous les arguments du pointeur font référence aux enveloppes TLV Norito dans la région INPUT et sont validés au premier déréférencement (`E_NORITO_INVALID` en cas d'erreur).
- Toutes les mutations sont appliquées via l'exécuteur standard de Iroha (via `CoreHost`), et non directement par la VM.
- Les constantes exactes des gaz (`G_*`) sont définies par le programme de gaz actif ; voir `ivm.md`.

Erreurs
- `E_SCALL_UNKNOWN` : numéro d'appel système non reconnu pour le `abi_version` actif.
- Les erreurs de validation d'entrée se propagent sous forme d'interruptions de VM (par exemple, `E_NORITO_INVALID` pour les TLV mal formés).

Références croisées
- Architecture et sémantique VM : `ivm.md`
- Langue et mappage intégré : `docs/source/kotodama_grammar.md`

Note de génération
- Une liste complète des constantes d'appel système peut être générée à partir des sources avec :
  - `make docs-syscalls` → écrit `docs/source/ivm_syscalls_generated.md`
  - `make check-docs` → vérifie que la table générée est à jour (utile en CI)
- Le sous-ensemble ci-dessus reste une table organisée et stable pour les appels système liés aux contrats.

## Exemples de TLV d'administrateur/rôle (hôte simulé)

Cette section documente les formes TLV et les charges utiles JSON minimales acceptées par l'hôte WSV fictif pour les appels système de style administrateur utilisés dans les tests. Tous les arguments du pointeur suivent le pointeur‑ABI (enveloppes TLV Norito placées dans INPUT). Les hôtes de production peuvent utiliser des schémas plus riches ; ces exemples visent à clarifier les types et les formes de base.- REGISTER_PEER / UNREGISTER_PEER
  - Args : `r10=&Json`
  - Exemple JSON : `{ "peer": "peer-id-or-info" }`
  - Remarque CoreHost : `REGISTER_PEER` attend un objet JSON `RegisterPeerWithPop` avec des octets `peer` + `pop` (`activation_at`, `expiry_at`, `hsm` en option) ; `UNREGISTER_PEER` accepte une chaîne d'identification d'homologue ou `{ "peer": "..." }`.

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  -CRÉER_TRIGGER :
    - Args : `r10=&Json`
    - JSON minimal : `{ "name": "t1" }` (champs supplémentaires ignorés par le mock)
  - REMOVE_TRIGGER :
    - Args : `r10=&Name` (nom du déclencheur)
  -SET_TRIGGER_ENABLED :
    - Args : `r10=&Name`, `r11=enabled:u64` (0 = désactivé, différent de zéro = activé)
  - Remarque CoreHost : `CREATE_TRIGGER` attend une spécification de déclenchement complète (chaîne base64 Norito `Trigger` ou
    `{ "id": "<trigger_id>", "action": ... }` avec `action` comme chaîne base64 Norito `Action` ou
    un objet JSON), et `SET_TRIGGER_ENABLED` bascule la clé de métadonnées du déclencheur `__enabled` (manquante
    la valeur par défaut est activée).

- Rôles : CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  -CRÉER_ROLE :
    - Args : `r10=&Name` (nom du rôle), `r11=&Json` (ensemble d'autorisations)
    - JSON accepte la clé `"perms"` ou `"permissions"`, chacune étant un tableau de chaînes de noms d'autorisation.
    - Exemples :
      -`{ "perms": [ "mint_asset:rose#wonder" ] }`
      -`{ "permissions": [ "read_assets:soraカタカナ...", "transfer_asset:rose#wonder" ] }`
    - Préfixes de nom d'autorisation pris en charge dans la simulation :
      -`register_domain`, `register_account`, `register_asset_definition`
      -`read_assets:<account_id>`
      -`mint_asset:<asset_definition_id>`
      -`burn_asset:<asset_definition_id>`
      -`transfer_asset:<asset_definition_id>`
  - DELETE_ROLE :
    - Args : `r10=&Name`
    - Échoue si un compte se voit toujours attribuer ce rôle.
  - GRANT_ROLE / REVOKE_ROLE :
    - Args : `r10=&AccountId` (sujet), `r11=&Name` (nom du rôle)
  - Remarque CoreHost : l'autorisation JSON peut être un objet `Permission` complet (`{ "name": "...", "payload": ... }`) ou une chaîne (la charge utile par défaut est `null`) ; `GRANT_PERMISSION`/`REVOKE_PERMISSION` accepte `&Name` ou `&Json(Permission)`.

- Opérations de désenregistrement (domaine/compte/actif) : invariants (simulé)
  - UNREGISTER_DOMAIN (`r10=&DomainId`) échoue si des comptes ou des définitions d'actifs existent dans le domaine.
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) échoue si le compte a des soldes non nuls ou possède des NFT.
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) échoue s'il existe des soldes pour l'actif.

Remarques
- Ces exemples reflètent l'hôte WSV fictif utilisé dans les tests ; les hôtes de nœuds réels peuvent exposer des schémas d'administration plus riches ou nécessiter une validation supplémentaire. Les règles du pointeur‑ABI s'appliquent toujours : les TLV doivent être dans INPUT, version=1, les ID de type doivent correspondre et les hachages de charge utile doivent être validés.