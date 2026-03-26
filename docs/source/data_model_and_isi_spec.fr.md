---
lang: fr
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 Modèle de données et ISI — Spécification dérivée de l'implémentation

Cette spécification est issue d'une ingénierie inverse à partir de l'implémentation actuelle dans `iroha_data_model` et `iroha_core` pour faciliter la révision de la conception. Les chemins entre guillemets pointent vers le code faisant autorité.

## Portée
- Définit les entités canoniques (domaines, comptes, actifs, NFT, rôles, autorisations, pairs, déclencheurs) et leurs identifiants.
- Décrit les instructions de changement d'état (ISI) : types, paramètres, conditions préalables, transitions d'état, événements émis et conditions d'erreur.
- Résume la gestion des paramètres, les transactions et la sérialisation des instructions.

Déterminisme : toutes les sémantiques d'instructions sont de pures transitions d'état sans comportement dépendant du matériel. La sérialisation utilise Norito ; Le bytecode de la VM utilise le IVM et est validé côté hôte avant l'exécution en chaîne.

---

## Entités et identifiants
Les ID ont des formes de chaîne stables avec un aller-retour `Display`/`FromStr`. Les règles de nom interdisent les espaces et les caractères réservés `@ # $`.- `Name` — identifiant textuel validé. Règles : `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Domaine : `{ id, logo, metadata, owned_by }`. Constructeurs : `NewDomain`. Code : `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — les adresses canoniques sont produites via `AccountAddress` (I105 / hex) et Torii normalise les entrées via `AccountAddress::parse_encoded`. I105 est le format de compte préféré ; le formulaire I105 est destiné à l'UX Sora uniquement. La chaîne familière `alias` (formulaire hérité rejeté) est conservée uniquement comme alias de routage. Compte : `{ id, metadata }`. Code : `crates/iroha_data_model/src/account.rs`.- Politique d'admission de compte : les domaines contrôlent la création de compte implicite en stockant un Norito-JSON `AccountAdmissionPolicy` sous la clé de métadonnées `iroha:account_admission_policy`. Lorsque la clé est absente, le paramètre personnalisé au niveau de la chaîne `iroha:default_account_admission_policy` fournit la valeur par défaut ; lorsque cela est également absent, la valeur par défaut est `ImplicitReceive` (première version). Les balises de stratégie `mode` (`ExplicitOnly` ou `ImplicitReceive`) plus les plafonds facultatifs par transaction (par défaut `16`) et de création par bloc, un `implicit_creation_fee` facultatif (compte de gravure ou de réception), `min_initial_amounts` par définition d'actif et un `default_role_on_create` facultatif (accordé après `AccountCreated`, rejeté avec `DefaultRoleError` s'il est manquant). Genesis ne peut pas s'inscrire ; Les stratégies désactivées/invalides rejettent les instructions de type reçu pour les comptes inconnus avec `InstructionExecutionError::AccountAdmission`. Les comptes implicites tamponnent les métadonnées `iroha:created_via="implicit"` avant `AccountCreated` ; les rôles par défaut émettent un suivi `AccountRoleGranted`, et les règles de base du propriétaire de l'exécuteur permettent au nouveau compte de dépenser ses propres actifs/NFT sans rôles supplémentaires. Code : `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — canonique `unprefixed Base58 address with versioning and checksum` (UUID-v4 octets). Définition : `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Les littéraux `alias` doivent être `<name>#<domain>.<dataspace>` ou `<name>#<dataspace>`, `<name>` étant égal au nom de la définition de l'actif. Code : `crates/iroha_data_model/src/asset/definition.rs`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId` : littéral codé canonique `<asset-definition-id>#<i105-account-id>` (les formes textuelles héritées ne sont pas prises en charge dans la première version).- `NftId` — `nft$domain`. NFT : `{ id, content: Metadata, owned_by }`. Code : `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Rôle : `{ id, permissions: BTreeSet<Permission> }` avec le constructeur `NewRole { inner: Role, grant_to }`. Code : `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Code : `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — identité d'homologue (clé publique) et adresse. Code : `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Déclencheur : `{ id, action }`. Action : `{ executable, repeats, authority, filter, metadata }`. Code : `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` avec insertion/retrait vérifiée. Code : `crates/iroha_data_model/src/metadata.rs`.
- Modèle d'abonnement (couche application) : les plans sont des entrées `AssetDefinition` avec des métadonnées `subscription_plan` ; les abonnements sont des enregistrements `Nft` avec des métadonnées `subscription` ; la facturation est exécutée par des déclencheurs temporels faisant référence aux NFT d'abonnement. Voir `docs/source/subscriptions_api.md` et `crates/iroha_data_model/src/subscription.rs`.
- **Primitives cryptographiques** (fonctionnalité `sm`) :
  - `Sm2PublicKey` / `Sm2Signature` reflètent le point canonique SEC1 + encodage `r∥s` à largeur fixe pour SM2. Les constructeurs appliquent l'appartenance aux courbes et la sémantique d'identification distinctive (`DEFAULT_DISTID`), tandis que la vérification rejette les scalaires mal formés ou de portée élevée. Codes : `crates/iroha_crypto/src/sm.rs` et `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` expose le résumé GM/T 0004 en tant que nouveau type `[u8; 32]` sérialisable Norito utilisé partout où des hachages apparaissent dans les manifestes ou la télémétrie. Code : `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` représente des clés SM4 de 128 bits et est partagé entre les appels système de l'hôte et les appareils de modèle de données. Code : `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Ces types s'ajoutent aux primitives Ed25519/BLS/ML-DSA existantes et sont disponibles pour les consommateurs de modèles de données (Torii, SDK, outils Genesis) une fois la fonctionnalité `sm` activée.
- Les magasins de relations dérivés de l'espace de données (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, registre de remplacement d'urgence à relais de voie) et les autorisations de cible d'espace de données (`CanPublishSpaceDirectoryManifest{dataspace: ...}` dans les magasins d'autorisations de compte/rôle) sont élagués. `State::set_nexus(...)` lorsque les espaces de données disparaissent du `dataspace_catalog` actif, empêchant les références d'espace de données obsolètes après les mises à jour du catalogue d'exécution. Les caches DA/relais au niveau des voies (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) sont également supprimés lorsqu'une voie est retirée ou réaffectée à un espace de données différent afin que l'état local de la voie ne puisse pas fuir lors des migrations d'espace de données. Les ISI de l'annuaire spatial (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) valident également `dataspace` par rapport au catalogue actif et rejettent les ID inconnus avec `InvalidParameter`.

Caractéristiques importantes : `Identifiable`, `Registered`/`Registrable` (modèle de constructeur), `HasMetadata`, `IntoKeyValue`. Code : `crates/iroha_data_model/src/lib.rs`.

Événements : chaque entité a des événements émis lors des mutations (création/suppression/propriétaire modifié/métadonnées modifiées, etc.). Code : `crates/iroha_data_model/src/events/`.

---## Paramètres (Configuration de la chaîne)
- Familles : `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, plus `custom: BTreeMap`.
- Enumérations uniques pour les différences : `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Agrégateur : `Parameters`. Code : `crates/iroha_data_model/src/parameter/system.rs`.

Paramétrage de paramétrage (ISI) : `SetParameter(Parameter)` met à jour le champ correspondant et émet `ConfigurationEvent::Changed`. Code : `crates/iroha_data_model/src/isi/transparent.rs`, exécuteur en `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Sérialisation et registre des instructions
- Trait de base : `Instruction: Send + Sync + 'static` avec `dyn_encode()`, `as_any()`, stable `id()` (par défaut, le nom du type concret).
- `InstructionBox` : enveloppe `Box<dyn Instruction>`. Clone/Eq/Ord fonctionnent sur `(type_id, encoded_bytes)` donc l'égalité se fait par valeur.
- Le serde Norito pour `InstructionBox` est sérialisé comme `(String wire_id, Vec<u8> payload)` (revient à `type_name` s'il n'y a pas d'ID de fil). La désérialisation utilise un mappage global `InstructionRegistry` des identifiants aux constructeurs. Le registre par défaut inclut tous les ISI intégrés. Code : `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI : Types, Sémantique, Erreurs
L'exécution est implémentée via `Execute for <Instruction>` dans `iroha_core::smartcontracts::isi`. Vous trouverez ci-dessous la liste des effets publics, des conditions préalables, des événements émis et des erreurs.

### S'inscrire/Se désinscrire
Types : `Register<T: Registered>` et `Unregister<T: Identifiable>`, avec des types de somme `RegisterBox`/`UnregisterBox` couvrant des cibles concrètes.- Register Peer : s'insère dans l'ensemble des pairs du monde.
  - Conditions préalables : ne doivent pas déjà exister.
  - Événements : `PeerEvent::Added`.
  - Erreurs : `Repetition(Register, PeerId)` si doublon ; `FindError` lors des recherches. Code : `core/.../isi/world.rs`.

- Enregistrer le domaine : construit à partir de `NewDomain` avec `owned_by = authority`. Interdit : domaine `genesis`.
  - Conditions préalables : inexistence du domaine ; pas `genesis`.
  - Événements : `DomainEvent::Created`.
  - Erreurs : `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Code : `core/.../isi/world.rs`.

- Enregistrer un compte : builds à partir de `NewAccount`, interdits dans le domaine `genesis` ; Le compte `genesis` ne peut pas être enregistré.
  - Conditions préalables : le domaine doit exister ; inexistence de compte ; pas dans le domaine de la genèse.
  - Événements : `DomainEvent::Account(AccountEvent::Created)`.
  - Erreurs : `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Code : `core/.../isi/domain.rs`.

- Enregistrez AssetDefinition : construit à partir du constructeur ; définit `owned_by = authority`.
  - Conditions préalables : définition inexistence ; le domaine existe ; `name` est requis, ne doit pas être vide après le découpage et ne doit pas contenir `#`/`@`.
  - Événements : `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Erreurs : `Repetition(Register, AssetDefinitionId)`. Code : `core/.../isi/domain.rs`.

- Enregistrez NFT : construit à partir du constructeur ; définit `owned_by = authority`.
  - Conditions préalables : inexistence du NFT ; le domaine existe.
  - Événements : `DomainEvent::Nft(NftEvent::Created)`.
  - Erreurs : `Repetition(Register, NftId)`. Code : `core/.../isi/nft.rs`.- Enregistrer le rôle : construit à partir de `NewRole { inner, grant_to }` (premier propriétaire enregistré via le mappage compte-rôle), stocke `inner: Role`.
  - Conditions préalables : inexistence du rôle.
  - Événements : `RoleEvent::Created`.
  - Erreurs : `Repetition(Register, RoleId)`. Code : `core/.../isi/world.rs`.

- Register Trigger : stocke le déclencheur dans le déclencheur approprié défini par type de filtre.
  - Conditions préalables : Si le filtre n'est pas montable, `action.repeats` doit être `Exactly(1)` (sinon `MathError::Overflow`). Pièces d'identité en double interdites.
  - Événements : `TriggerEvent::Created(TriggerId)`.
  - Erreurs : `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` sur échecs de conversion/validation. Code : `core/.../isi/triggers/mod.rs`.- Désinscrire Peer/Domain/Account/AssetDefinition/NFT/Role/Trigger : supprime la cible ; émet des événements de suppression. Suppressions en cascade supplémentaires :- Désinscrire le domaine : supprime l'entité de domaine ainsi que son état de politique de sélection/approbation ; supprime les définitions d'actifs dans le domaine (et l'état secondaire confidentiel `zk_assets` saisi par ces définitions), les actifs de ces définitions (et les métadonnées par actif), les NFT dans le domaine et les projections d'étiquettes de compte/alias à l'échelle du domaine. Il dissocie également les comptes survivants du domaine supprimé et supprime les entrées d'autorisation liées au compte/au rôle qui font référence au domaine supprimé ou aux ressources supprimées avec celui-ci (autorisations de domaine, autorisations de définition d'actif/d'actif pour les définitions supprimées et autorisations NFT pour les identifiants NFT supprimés). La suppression du domaine ne supprime pas le `AccountId` global, son état de séquence d'émission/UAID, son actif étranger ou sa propriété NFT, son autorité de déclenchement ou d'autres références d'audit/configuration externes qui pointent vers le compte survivant. Garde-corps : rejets lorsqu'une définition d'actif dans le domaine est toujours référencée par un accord de pension, un grand livre de règlement, une récompense/réclamation sur voie publique, une allocation/transfert hors ligne, des références de définition d'actif de règlement par défaut (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), un vote/citoyenneté/éligibilité au parlement/récompense virale configuré par la gouvernance, une configuration d'économie Oracle. références de définition d'actifs de récompense/slash/dispute-bond, ou références de définition d'actifs de frais/jalonnement Nexus (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Événements : `DomainEvent::Deleted`, plus suppression par élémentsur les événements pour les ressources supprimées au niveau du domaine. Erreurs : `FindError::Domain` si manquant ; `InvariantViolation` sur les conflits de références de définition d'actif conservés. Code : `core/.../isi/world.rs`.- Désinscrire le compte : supprime les autorisations, les rôles, le compteur de séquence d'émission, le mappage des étiquettes de compte et les liaisons UAID du compte ; supprime les actifs appartenant au compte (et les métadonnées par actif) ; supprime les NFT appartenant au compte ; supprime les déclencheurs dont l'autorité est ce compte ; élague les entrées d'autorisation au niveau du compte/du rôle qui font référence au compte supprimé, les autorisations de cible NFT au niveau du compte/du rôle pour les identifiants NFT détenus supprimés et les autorisations de cible de déclenchement au niveau du compte/du rôle pour les déclencheurs supprimés. Garde-corps : rejets si le compte possède toujours un domaine, définition d'actif, liaison du fournisseur SoraFS, enregistrement de citoyenneté actif, état de jalonnement/récompense sur voie publique (y compris les clés de réclamation de récompense où le compte apparaît comme demandeur ou propriétaire d'actif de récompense), état Oracle actif (y compris les entrées du fournisseur d'historique de flux Oracle, les enregistrements de fournisseur de liaison Twitter ou les références de compte de récompense/slash configurées par Oracle-Economics), actif Références de compte de frais/mise en jeu Nexus (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id` ; analysées comme identifiants de compte sans domaine canoniques et rejet de clôture en cas d'échec sur des littéraux invalides), état d'accord de pension actif, état de grand livre de règlement actif, allocation/transfert hors ligne actif ou état de verdict-révocation hors ligne, actif Références de configuration de compte séquestre hors ligne pour les définitions d'actifs actifs (`settlement.offline.escrow_accounts`), état de gouvernance active (proposition/approbation d'étape)listes als/locks/slashes/conseil/parlement, instantanés du parlement des propositions, enregistrements des proposants de mise à niveau d'exécution, références de compte séquestre/slash-receiver/pool viral configurées pour la gouvernance, références de soumission de télémétrie SoraFS de gouvernance via `gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters`, ou configurées pour la gouvernance SoraFS références de propriétaire de fournisseur via `gov.sorafs_provider_owners`), références de compte de liste blanche de publication de contenu configuré (`content.publish_allow_accounts`), état d'expéditeur de dépôt social actif, état de créateur de paquet de contenu actif, état de propriétaire d'intention de code PIN DA actif, état de remplacement du validateur d'urgence de relais de voie actif ou registre de code PIN SoraFS actif. enregistrements d'émetteur/relieur (manifestes d'épingles, alias de manifeste, ordres de réplication). Événements : `AccountEvent::Deleted`, plus `NftEvent::Deleted` par NFT supprimé. Erreurs : `FindError::Account` si manquant ; `InvariantViolation` sur les orphelins de propriété. Code : `core/.../isi/domain.rs`.- Unregister AssetDefinition : supprime tous les actifs de cette définition et leurs métadonnées par actif, et supprime l'état secondaire confidentiel `zk_assets` saisi par cette définition ; supprime également l'entrée `settlement.offline.escrow_accounts` correspondante et les entrées d'autorisation de portée compte/rôle qui font référence à la définition d'actif supprimée ou à ses instances d'actif. Garde-corps : rejets lorsque la définition est toujours référencée par l'accord de pension, le grand livre de règlement, la récompense/réclamation de voie publique, l'état d'allocation/de transfert hors ligne, les références de définition d'actifs de règlement par défaut (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), le vote/citoyenneté/éligibilité au parlement/récompense virale configurés par la gouvernance, l'économie d'Oracle configurée. références de définition d'actifs de récompense/slash/dispute-bond, ou références de définition d'actifs de frais/jalonnement Nexus (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). Événements : `AssetDefinitionEvent::Deleted` et `AssetEvent::Deleted` par actif. Erreurs : `FindError::AssetDefinition`, `InvariantViolation` sur conflits de références. Code : `core/.../isi/domain.rs`.
  - Désinscrire NFT : supprime NFT et élague les entrées d'autorisation liées au compte/au rôle qui font référence au NFT supprimé. Événements : `NftEvent::Deleted`. Erreurs : `FindError::Nft`. Code : `core/.../isi/nft.rs`.
  - Annuler l'enregistrement du rôle : révoque d'abord le rôle de tous les comptes ; puis supprime le rôle. Événements : `RoleEvent::Deleted`. Erreurs : `FindError::Role`. Code : `core/.../isi/world.rs`.- Unregister Trigger : supprime le déclencheur s'il est présent et élague les entrées d'autorisation liées au compte/au rôle qui font référence au déclencheur supprimé ; la désinscription en double donne `Repetition(Unregister, TriggerId)`. Événements : `TriggerEvent::Deleted`. Code : `core/.../isi/triggers/mod.rs`.

### Menthe / Brûlure
Types : `Mint<O, D: Identifiable>` et `Burn<O, D: Identifiable>`, en boîte `MintBox`/`BurnBox`.

- Actif (numérique) mint/burn : ajuste les soldes et la définition `total_quantity`.
  - Conditions préalables : la valeur `Numeric` doit satisfaire à `AssetDefinition.spec()` ; menthe autorisée par `mintable` :
    - `Infinitely` : toujours autorisé.
    - `Once` : autorisé une seule fois ; la première menthe transforme `mintable` en `Not` et émet `AssetDefinitionEvent::MintabilityChanged`, plus un `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` détaillé pour l'auditabilité.
    - `Limited(n)` : permet des opérations de menthe supplémentaires `n`. Chaque frappe réussie décrémente le compteur ; lorsqu'elle atteint zéro, la définition passe à `Not` et émet les mêmes événements `MintabilityChanged` que ci-dessus.
    - `Not` : erreur `MintabilityError::MintUnmintable`.
  - Changements d'état : crée un actif s'il est manquant à l'état neuf ; supprime l'entrée d'actif si le solde devient nul lors de la gravure.
  - Événements : `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (lorsque `Once` ou `Limited(n)` épuise son allocation).
  - Erreurs : `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Code : `core/.../isi/asset.rs`.- Répétitions de déclenchement menthe/brûlure : les modifications comptent pour un déclenchement `action.repeats`.
  - Conditions préalables : à l'état neuf, le filtre doit être monnayable ; l'arithmétique ne doit pas déborder/sous-dépasser.
  - Événements : `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Erreurs : `MathError::Overflow` sur menthe invalide ; `FindError::Trigger` si manquant. Code : `core/.../isi/triggers/mod.rs`.

### Transfert
Types : `Transfer<S: Identifiable, O, D: Identifiable>`, en boîte `TransferBox`.

- Actif (Numérique) : soustraire de la source `AssetId`, ajouter à la destination `AssetId` (même définition, compte différent). Supprimez l'actif source mis à zéro.
  - Conditions préalables : l'actif source existe ; la valeur satisfait à `spec`.
  - Événements : `AssetEvent::Removed` (source), `AssetEvent::Added` (destination).
  - Erreurs : `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Code : `core/.../isi/asset.rs`.

- Propriété du domaine : remplace `Domain.owned_by` par le compte de destination.
  - Conditions préalables : les deux comptes existent ; le domaine existe.
  - Événements : `DomainEvent::OwnerChanged`.
  - Erreurs : `FindError::Account/Domain`. Code : `core/.../isi/domain.rs`.

- Propriété AssetDefinition : remplace `AssetDefinition.owned_by` par le compte de destination.
  - Conditions préalables : les deux comptes existent ; la définition existe; la source doit actuellement en être propriétaire ; l'autorité doit être le compte source, le propriétaire du domaine source ou le propriétaire du domaine de définition d'actif.
  - Événements : `AssetDefinitionEvent::OwnerChanged`.
  - Erreurs : `FindError::Account/AssetDefinition`. Code : `core/.../isi/account.rs`.- Propriété NFT : change `Nft.owned_by` en compte de destination.
  - Conditions préalables : les deux comptes existent ; Le NFT existe ; la source doit actuellement en être propriétaire ; l'autorité doit être le compte source, le propriétaire du domaine source, le propriétaire du domaine NFT ou détenir `CanTransferNft` pour ce NFT.
  - Événements : `NftEvent::OwnerChanged`.
  - Erreurs : `FindError::Account/Nft`, `InvariantViolation` si la source ne possède pas le NFT. Code : `core/.../isi/nft.rs`.

### Métadonnées : définir/supprimer la valeur-clé
Types : `SetKeyValue<T>` et `RemoveKeyValue<T>` avec `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Énumérations encadrées fournies.

- Ensemble : insère ou remplace `Metadata[key] = Json(value)`.
- Supprimer : supprime la clé ; erreur si manquant.
- Evénements : `<Target>Event::MetadataInserted` / `MetadataRemoved` avec les anciennes/nouvelles valeurs.
- Erreurs : `FindError::<Target>` si la cible n'existe pas ; `FindError::MetadataKey` sur clé manquante pour retrait. Code : `crates/iroha_data_model/src/isi/transparent.rs` et exécuteur implicite par cible.

### Autorisations et rôles : accorder/révoquer
Types : `Grant<O, D>` et `Revoke<O, D>`, avec des énumérations encadrées pour `Permission`/`Role` vers/depuis `Account` et `Permission` vers/depuis `Role`.- Accorder l'autorisation au compte : ajoute `Permission` sauf si déjà inhérent. Événements : `AccountEvent::PermissionAdded`. Erreurs : `Repetition(Grant, Permission)` en cas de doublon. Code : `core/.../isi/account.rs`.
- Révoquer l'autorisation du compte : supprime si elle est présente. Événements : `AccountEvent::PermissionRemoved`. Erreurs : `FindError::Permission` si absent. Code : `core/.../isi/account.rs`.
- Accorder un rôle au compte : insère le mappage `(account, role)` s'il est absent. Événements : `AccountEvent::RoleGranted`. Erreurs : `Repetition(Grant, RoleId)`. Code : `core/.../isi/account.rs`.
- Révoquer le rôle du compte : supprime le mappage s'il est présent. Événements : `AccountEvent::RoleRevoked`. Erreurs : `FindError::Role` si absent. Code : `core/.../isi/account.rs`.
- Accorder l'autorisation au rôle : reconstruit le rôle avec l'autorisation ajoutée. Événements : `RoleEvent::PermissionAdded`. Erreurs : `Repetition(Grant, Permission)`. Code : `core/.../isi/world.rs`.
- Révoquer l'autorisation du rôle : reconstruit le rôle sans cette autorisation. Événements : `RoleEvent::PermissionRemoved`. Erreurs : `FindError::Permission` si absent. Code : `core/.../isi/world.rs`.### Déclencheurs : exécuter
Tapez : `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Comportement : met en file d'attente un `ExecuteTriggerEvent { trigger_id, authority, args }` pour le sous-système de déclenchement. L'exécution manuelle est autorisée uniquement pour les déclencheurs d'appel (filtre `ExecuteTrigger`) ; le filtre doit correspondre et l'appelant doit être l'autorité de déclenchement de l'action ou détenir `CanExecuteTrigger` pour cette autorité. Lorsqu'un exécuteur fourni par l'utilisateur est actif, l'exécution du déclencheur est validée par l'exécuteur d'exécution et consomme le budget de carburant de l'exécuteur de la transaction (base `executor.fuel` plus métadonnées facultatives `additional_fuel`).
- Erreurs : `FindError::Trigger` si non enregistré ; `InvariantViolation` si appelé par une non-autorité. Code : `core/.../isi/triggers/mod.rs` (et tests en `core/.../smartcontracts/isi/mod.rs`).

### Mise à niveau et journalisation
- `Upgrade { executor }` : migre l'exécuteur en utilisant le bytecode `Executor` fourni, met à jour l'exécuteur et son modèle de données, émet `ExecutorEvent::Upgraded`. Erreurs : encapsulées sous la forme `InvalidParameterError::SmartContract` en cas d'échec de la migration. Code : `core/.../isi/world.rs`.
- `Log { level, msg }` : émet un log de nœud avec le niveau donné ; aucun changement d'état. Code : `core/.../isi/world.rs`.

### Modèle d'erreur
Enveloppe commune : `InstructionExecutionError` avec des variantes pour les erreurs d'évaluation, les échecs de requête, les conversions, l'entité introuvable, la répétition, la mintabilité, les mathématiques, les paramètres non valides et la violation invariante. Les énumérations et les assistants se trouvent dans `crates/iroha_data_model/src/isi/mod.rs` sous `pub mod error`.

---## Transactions et exécutables
- `Executable` : soit `Instructions(ConstVec<InstructionBox>)` ou `Ivm(IvmBytecode)` ; le bytecode est sérialisé en base64. Code : `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction` : construit, signe et conditionne un exécutable avec des métadonnées, `chain_id`, `authority`, `creation_time_ms`, `ttl_ms` en option et `nonce`. Code : `crates/iroha_data_model/src/transaction/`.
- Au moment de l'exécution, `iroha_core` exécute les lots `InstructionBox` via `Execute for InstructionBox`, en les transcrivant vers le `*Box` approprié ou une instruction concrète. Code : `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Budget de validation de l'exécuteur d'exécution (exécuteur fourni par l'utilisateur) : base `executor.fuel` à partir des paramètres ainsi que des métadonnées de transaction facultatives `additional_fuel` (`u64`), partagées entre les validations d'instructions/déclencheurs au sein de la transaction.

---## Invariants et notes (issus des tests et des gardes)
- Protections Genesis : impossible d'enregistrer le domaine `genesis` ou les comptes dans le domaine `genesis` ; Le compte `genesis` ne peut pas être enregistré. Codes/essais : `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Les actifs numériques doivent satisfaire à leur `NumericSpec` lors de la création/du transfert/de la gravure ; la non-concordance des spécifications donne `TypeError::AssetNumericSpec`.
- Possibilité de menthe : `Once` permet une seule menthe, puis passe à `Not` ; `Limited(n)` autorise exactement les menthes `n` avant de passer à `Not`. Les tentatives d'interdiction de la frappe sur `Infinitely` provoquent `MintabilityError::ForbidMintOnMintable`, et la configuration de `Limited(0)` donne `MintabilityError::InvalidMintabilityTokens`.
- Les opérations sur les métadonnées sont exactes ; supprimer une clé inexistante est une erreur.
- Les filtres de déclenchement peuvent ne pas être modifiables ; alors `Register<Trigger>` autorise uniquement les répétitions `Exactly(1)`.
- Déclencher l'exécution des portes de la clé de métadonnées `__enabled` (bool) ; les valeurs par défaut manquantes sont activées et les déclencheurs désactivés sont ignorés dans les chemins de données/heure/par appel.
- Déterminisme : toute arithmétique utilise des opérations vérifiées ; under/overflow renvoie des erreurs mathématiques tapées ; les soldes nuls suppriment les entrées d’actifs (pas d’état caché).

---## Exemples pratiques
- Frappe et transfert :
  - `Mint::asset_numeric(10, asset_id)` → ajoute 10 si la spécification/la mintabilité le permet ; événements : `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → coups 5 ; événements à supprimer/ajouter.
- Mises à jour des métadonnées :
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → insertion ; suppression via `RemoveKeyValue::account(...)`.
- Gestion des rôles/autorisations :
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` et leurs homologues `Revoke`.
- Cycle de vie du déclencheur :
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` avec contrôle de monnayabilité implicite par filtre ; `ExecuteTrigger::new(id).with_args(&args)` doit correspondre à l’autorité configurée.
  - Les déclencheurs peuvent être désactivés en définissant la clé de métadonnées `__enabled` sur `false` (les valeurs par défaut manquantes sont activées) ; basculer via l'appel système `SetKeyValue::trigger` ou IVM `set_trigger_enabled`.
  - Le stockage des déclencheurs est réparé au chargement : les identifiants en double, les identifiants incompatibles et les déclencheurs faisant référence au bytecode manquant sont supprimés ; Les décomptes de références de bytecode sont recalculés.
  - Si le bytecode IVM d'un déclencheur est manquant au moment de l'exécution, le déclencheur est supprimé et l'exécution est traitée comme une non-opération avec un résultat d'échec.
  - Les déclencheurs épuisés sont immédiatement supprimés ; si une entrée épuisée est rencontrée pendant l'exécution, elle est élaguée et traitée comme manquante.
- Mise à jour des paramètres :
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` met à jour et émet `ConfigurationEvent::Changed`.CLI / Torii asset-definition id + exemples d'alias :
- S'inscrire avec aide canonique + nom explicite + alias long :
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- S'inscrire avec aide canonique + nom explicite + alias court :
  -`iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- Mint par alias + composants du compte :
  -`iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- Résoudre l'alias à l'aide canonique :
  - `POST /v1/assets/aliases/resolve` avec JSON `{ "alias": "pkr#ubl.sbp" }`

Remarque sur la migration :
- Les ID de définition d'actif textuel `name#domain` ne sont intentionnellement pas pris en charge dans la première version.
- Les identifiants d'actifs aux limites de création/gravure/transfert restent canoniques `<asset-definition-id>#<i105-account-id>` ; utilisez `iroha tools encode asset-id` avec `--definition <base58-asset-definition-id>` ou `--alias ...` plus `--account`.

---

## Traçabilité (sources sélectionnées)
 - Noyau du modèle de données : `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Définitions et registre ISI : `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Exécution ISI : `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - Événements : `crates/iroha_data_model/src/events/**`.
 - Opérations : `crates/iroha_data_model/src/transaction/**`.

Si vous souhaitez que cette spécification soit étendue dans une table d'API/de comportement rendue ou réticulée à chaque événement/erreur concret, dites le mot et je l'étendrai.