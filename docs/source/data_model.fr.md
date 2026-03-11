---
lang: fr
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c110536e456d6582c2dd2bd72a71fef25e3f43f7f369b3f1c0ce802564f0dbd
source_last_modified: "2026-01-28T18:33:51.649272+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Modèle de données # Iroha v2 – Analyse approfondie

Ce document explique les structures, les identifiants, les caractéristiques et les protocoles qui forment le modèle de données Iroha v2, tel qu'implémenté dans la caisse `iroha_data_model` et utilisé dans l'espace de travail. Il s’agit d’une référence précise que vous pouvez consulter et proposer des mises à jour.

## Portée et fondements

- Objectif : fournir des types canoniques pour les objets de domaine (domaines, comptes, actifs, NFT, rôles, autorisations, pairs), les instructions de changement d'état (ISI), les requêtes, les déclencheurs, les transactions, les blocs et les paramètres.
- Sérialisation : tous les types publics dérivent les codecs Norito (`norito::codec::{Encode, Decode}`) et le schéma (`iroha_schema::IntoSchema`). JSON est utilisé de manière sélective (par exemple, pour les charges utiles HTTP et `Json`) derrière les indicateurs de fonctionnalité.
- Remarque IVM : certaines validations au moment de la désérialisation sont désactivées lors du ciblage de la machine virtuelle Iroha (IVM), car l'hôte effectue une validation avant d'invoquer des contrats (voir la documentation de la caisse dans `src/lib.rs`).
- Portes FFI : certains types sont annotés conditionnellement pour FFI via `iroha_ffi` derrière `ffi_export`/`ffi_import` pour éviter une surcharge lorsque FFI n'est pas nécessaire.

## Traits de base et aides

- `Identifiable` : Les entités ont un `Id` et un `fn id(&self) -> &Self::Id` stables. Doit être dérivé avec `IdEqOrdHash` pour la convivialité des cartes/ensembles.
- `Registrable`/`Registered` : de nombreuses entités (par exemple, `Domain`, `AssetDefinition`, `Role`) utilisent un modèle de générateur. `Registered` lie le type d'exécution à un type de générateur léger (`With`) adapté aux transactions d'enregistrement.
- `HasMetadata` : Accès unifié à une map clé/valeur `Metadata`.
- `IntoKeyValue` : Assistant de partage de stockage pour stocker séparément `Key` (ID) et `Value` (données) afin de réduire la duplication.
- `Owned<T>`/`Ref<'world, K, V>` : wrappers légers utilisés dans les stockages et filtres de requêtes pour éviter les copies inutiles.

## Noms et identifiants

- `Name` : Identifiant textuel valide. Interdit les espaces et les caractères réservés `@`, `#`, `$` (utilisés dans les ID composites). Constructible via `FromStr` avec validation. Les noms sont normalisés en Unicode NFC lors de l'analyse (les orthographes canoniquement équivalentes sont traitées comme identiques et stockées composées). Le nom spécial `genesis` est réservé (coché sans tenir compte de la casse).
- `IdBox` : Une enveloppe de type somme pour tout identifiant pris en charge (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, `PeerId`, `TriggerId`, `RoleId`, `Permission`, `CustomParameterId`). Utile pour les flux génériques et l’encodage Norito en tant que type unique.
- `ChainId` : Identifiant de chaîne opaque utilisé pour la protection contre la relecture dans les transactions.Formes de chaîne des identifiants (autorisées avec `Display`/`FromStr`) :
- `DomainId` : `name` (par exemple, `wonderland`).
- `AccountId` : identifiant canonique codé via `AccountAddress`, qui expose I105, Sora compressé (`i105`) et les codecs hexadécimaux canoniques (`AccountAddress::to_i105`, `to_i105`, `canonical_hex`, `parse_encoded`). I105 est le format de compte préféré ; le formulaire `i105` est le deuxième meilleur pour l'UX Sora uniquement. L'alias de routage convivial `alias` (rejected legacy form) est conservé pour l'UX mais n'est plus traité comme l'identifiant faisant autorité. Torii normalise les chaînes entrantes via `AccountAddress::parse_encoded`. Les identifiants de compte prennent en charge les contrôleurs à clé unique et multisig.
- `AssetDefinitionId` : `asset#domain` (par exemple, `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` : `nft$domain` (par exemple, `rose$garden`).
- `PeerId` : `public_key` (l'égalité des pairs se fait par clé publique).

## Entités

### Domaine
- `DomainId { name: Name }` – nom unique.
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- Constructeur : `NewDomain` avec `with_logo`, `with_metadata`, puis `Registrable::build(authority)` définit `owned_by`.

### Compte
- `AccountId { domain: DomainId, controller: AccountController }` (contrôleur = clé unique ou politique multisig).
- `Account { id, metadata, label?, uaid? }` — `label` est un alias stable facultatif utilisé par les enregistrements de retouche, `uaid` porte l'[ID de compte universel] facultatif à l'échelle Nexus (./universal_accounts_guide.md).
- Constructeur : `NewAccount` via `Account::new(id)` ; `HasMetadata` pour le constructeur et l'entité.

### Définitions et actifs des actifs
-`AssetDefinitionId { domain: DomainId, name: Name }`.
-`AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  -`Mintable` : `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - Constructeurs : `AssetDefinition::new(id, spec)` ou commodité `numeric(id)` ; régleurs pour `metadata`, `mintable`, `owned_by`.
-`AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` avec `AssetEntry`/`AssetValue` convivial pour le stockage.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` exposé pour les API récapitulatives.

### NFT
-`NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (le contenu est constitué de métadonnées clé/valeur arbitraires).
- Constructeur : `NewNft` via `Nft::new(id, content)`.

### Rôles et autorisations
-`RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` avec le constructeur `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – le `name` et le schéma de charge utile doivent s'aligner sur le `ExecutorDataModel` actif (voir ci-dessous).

### Pairs
-`PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` et forme de chaîne analysable `public_key@address`.### Primitives cryptographiques (fonctionnalité `sm`)
- `Sm2PublicKey` et `Sm2Signature` : points conformes SEC1 et signatures `r∥s` à largeur fixe pour SM2. Les constructeurs valident l'appartenance à la courbe et les identifiants distinctifs ; Le codage Norito reflète la représentation canonique utilisée par `iroha_crypto`.
- `Sm3Hash` : nouveau type `[u8; 32]` représentant le résumé GM/T 0004, utilisé dans les manifestes, la télémétrie et les réponses d'appel système.
- `Sm4Key` : wrapper de clé symétrique de 128 bits partagé entre les appels système de l'hôte et les appareils de modèle de données.
Ces types s'assoient aux côtés des primitives Ed25519/BLS/ML-DSA existantes et font partie du schéma public une fois que l'espace de travail est construit avec `--features sm`.

### Déclencheurs et événements
- `TriggerId { name: Name }` et `Trigger { id, action: action::Action }`.
-`action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats` : `Indefinitely` ou `Exactly(u32)` ; utilitaires de commande et d’épuisement inclus.
  - Sécurité : `TriggerCompleted` ne peut pas être utilisé comme filtre d'action (validé lors de la (dé)sérialisation).
- `EventBox` : type de somme pour les événements de pipeline, de lot de pipeline, de données, d'heure, de déclenchement d'exécution et de déclenchement terminé ; `EventFilterBox` reflète cela pour les abonnements et les filtres de déclenchement.

## Paramètres et configuration

- Familles de paramètres système (tous `Default`ed, portent des getters et sont convertis en énumérations individuelles) :
-`SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  -`BlockParameters { max_transactions: NonZeroU64 }`.
  -`TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  -`SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` regroupe toutes les familles et un `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- Énumérations à paramètre unique : `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter` pour les mises à jour et les itérations de type diff.
- Paramètres personnalisés : définis par l'exécuteur, portés comme `Json`, identifiés par `CustomParameterId` (un `Name`).

## ISI (Instructions spéciales Iroha)

- Trait de base : `Instruction` avec `dyn_encode`, `as_any` et un identifiant stable par type `id()` (par défaut, le nom du type concret). Toutes les instructions sont `Send + Sync + 'static`.
- `InstructionBox` : wrapper `Box<dyn Instruction>` détenu avec clone/eq/ord implémenté via l'ID de type + octets codés.
- Les familles d'instructions intégrées sont organisées sous :
  - `mint_burn`, `transfer`, `register` et un ensemble d'assistants `transparent`.
  - Tapez des énumérations pour les méta-flux : `InstructionType`, des sommes encadrées comme `SetKeyValueBox` (domain/account/asset_def/nft/trigger).
- Erreurs : modèle d'erreur riche sous `isi::error` (erreurs de type évaluation, erreurs de recherche, mintabilité, mathématiques, paramètres invalides, répétition, invariants).
- Registre d'instructions : la macro `instruction_registry!{ ... }` crée un registre de décodage d'exécution classé par nom de type. Utilisé par le clone `InstructionBox` et le serde Norito pour réaliser une (dé)sérialisation dynamique. Si aucun registre n'a été explicitement défini via `set_instruction_registry(...)`, un registre par défaut intégré avec tous les principaux ISI est installé paresseusement lors de la première utilisation pour maintenir la robustesse des binaires.

## Transactions- `Executable` : soit `Instructions(ConstVec<InstructionBox>)` ou `Ivm(IvmBytecode)`. `IvmBytecode` est sérialisé en base64 (nouveau type transparent sur `Vec<u8>`).
- `TransactionBuilder` : construit une charge utile de transaction avec `chain`, `authority`, `creation_time_ms`, `time_to_live_ms` et `nonce` en option, `metadata` et un `Executable`.
  - Aides : `with_instructions`, `with_bytecode`, `with_executable`, `with_metadata`, `set_nonce`, `set_ttl`, `set_creation_time`, `sign`.
- `SignedTransaction` (version `iroha_version`) : contient `TransactionSignature` et la charge utile ; fournit le hachage et la vérification de la signature.
- Points d'entrée et résultats :
  -`TransactionEntrypoint` : `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` avec assistants de hachage.
  - `ExecutionStep(ConstVec<InstructionBox>)` : un seul lot ordonné d'instructions dans une transaction.

## Blocs

- `SignedBlock` (versionné) encapsule :
  - `signatures: BTreeSet<BlockSignature>` (des validateurs),
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (état d'exécution secondaire) contenant `time_triggers`, les arbres Merkle d'entrée/résultat, `transaction_results` et `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
- Utilitaires : `presigned`, `set_transaction_results(...)`, `set_transaction_results_with_transcripts(...)`, `header()`, `signatures()`, `hash()`, `add_signature`, `replace_signatures`.
- Racines Merkle : les points d'entrée et les résultats des transactions sont validés via les arbres Merkle ; résultat La racine Merkle est placée dans l’en-tête du bloc.
- Les preuves d'inclusion de blocs (`BlockProofs`) exposent à la fois les preuves Merkle d'entrée/résultat et la carte `fastpq_transcripts` afin que les prouveurs hors chaîne puissent récupérer les deltas de transfert associés à un hachage de transaction.
- Les messages `ExecWitness` (diffusés via Torii et basés sur les potins de consensus) incluent désormais à la fois `fastpq_transcripts` et `fastpq_batches: Vec<FastpqTransitionBatch>` prêts à être prouvés avec `public_inputs` intégré (dsid, slot, racines, perm_root, tx_set_hash), afin que les prouveurs externes puissent ingérer des lignes FASTPQ canoniques sans réencoder les transcriptions.

## Requêtes

- Deux saveurs :
  - Singulier : implémentez `SingularQuery<Output>` (par exemple, `FindParameters`, `FindExecutorDataModel`).
  - Itérable : implémentez `Query<Item>` (par exemple, `FindAccounts`, `FindAssets`, `FindDomains`, etc.).
- Formulaires dactylographiés :
  - `QueryBox<T>` est un `Query<Item = T>` en boîte et effacé avec le serde Norito soutenu par un registre mondial.
  - `QueryWithFilter<T> { query, predicate, selector }` associe une requête à un prédicat/sélecteur DSL ; se convertit en une requête itérable effacée via `From`.
- Registre et codecs :
  - `query_registry!{ ... }` crée un registre global mappant les types de requêtes concrètes aux constructeurs par nom de type pour le décodage dynamique.
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` et `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` est un type somme sur des vecteurs homogènes (par exemple, `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, `Vec<BlockHeader>`), ainsi que des tuples et des assistants d'extension pour une pagination efficace.
- DSL : implémenté dans `query::dsl` avec des traits de projection (`HasProjection<PredicateMarker>` / `SelectorMarker`) pour les prédicats et les sélecteurs vérifiés au moment de la compilation. Une fonctionnalité `fast_dsl` expose une variante plus légère si nécessaire.

## Exécuteur et extensibilité- `Executor { bytecode: IvmBytecode }` : le bundle de codes exécutés par le validateur.
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` déclare le domaine défini par l'exécuteur :
  - Paramètres de configuration personnalisés,
  - Identifiants d'instructions personnalisés,
  - Identifiants de jeton d'autorisation,
  - Un schéma JSON décrivant les types personnalisés pour les outils clients.
- Des exemples de personnalisation existent sous `data_model/samples/executor_custom_data_model` démontrant :
  - Jeton d'autorisation personnalisé via la dérive `iroha_executor_data_model::permission::Permission`,
  - Paramètre personnalisé défini comme un type convertible en `CustomParameter`,
  - Instructions personnalisées sérialisées dans `CustomInstruction` pour exécution.

### CustomInstruction (ISI défini par l'exécuteur)

- Type : `isi::CustomInstruction { payload: Json }` avec identifiant de fil stable `"iroha.custom"`.
- Objectif : enveloppe pour les instructions spécifiques à l'exécuteur dans les réseaux privés/consortium ou pour le prototypage, sans bifurquer du modèle de données public.
- Comportement de l'exécuteur par défaut : l'exécuteur intégré dans `iroha_core` n'exécute pas `CustomInstruction` et paniquera s'il est rencontré. Un exécuteur personnalisé doit convertir `InstructionBox` en `CustomInstruction` et interpréter de manière déterministe la charge utile sur tous les validateurs.
- Norito : encode/décode via `norito::codec::{Encode, Decode}` avec schéma inclus ; la charge utile `Json` est sérialisée de manière déterministe. Les allers-retours sont stables tant que le registre d'instructions inclut `CustomInstruction` (il fait partie du registre par défaut).
- IVM : Kotodama se compile en bytecode IVM (`.to`) et constitue le chemin recommandé pour la logique d'application. Utilisez uniquement `CustomInstruction` pour les extensions de niveau exécuteur qui ne peuvent pas encore être exprimées dans Kotodama. Garantissez le déterminisme et les binaires d’exécuteur identiques entre les pairs.
- Pas pour les réseaux publics : ne pas utiliser pour les chaînes publiques où des exécuteurs hétérogènes risquent de bifurquer vers le consensus. Préférez proposer de nouveaux ISI intégrés en amont lorsque vous avez besoin de fonctionnalités de plateforme.

## Métadonnées

- `Metadata(BTreeMap<Name, Json>)` : magasin de clés/valeurs attaché à plusieurs entités (`Domain`, `Account`, `AssetDefinition`, `Nft`, déclencheurs et transactions).
- API : `contains`, `iter`, `get`, `insert` et (avec `transparent_api`) `remove`.

## Caractéristiques et déterminisme

- Les fonctionnalités contrôlent les API facultatives (`std`, `json`, `transparent_api`, `ffi_export`, `ffi_import`, `fast_dsl`, `http`, `fault_injection`).
- Déterminisme : toute sérialisation utilise le codage Norito pour être portable sur tout le matériel. Le bytecode IVM est un blob d'octets opaque ; l’exécution ne doit pas introduire de réductions non déterministes. L'hôte valide les transactions et fournit des entrées à IVM de manière déterministe.

### API transparente (`transparent_api`)- Objectif : expose un accès complet et modifiable aux structures/énumérations `#[model]` pour les composants internes tels que Torii, les exécuteurs et les tests d'intégration. Sans cela, ces éléments sont intentionnellement opaques, de sorte que les SDK externes ne voient que les constructeurs sécurisés et les charges utiles codées.
- Mécanique : la macro `iroha_data_model_derive::model` réécrit chaque champ public avec `#[cfg(feature = "transparent_api")] pub` et conserve une copie privée pour le build par défaut. L'activation de la fonctionnalité inverse ces cfg, donc la déstructuration de `Account`, `Domain`, `Asset`, etc. devient légale en dehors de leurs modules de définition.
- Détection de surface : la caisse exporte une constante `TRANSPARENT_API: bool` (générée soit en `transparent_api.rs`, soit en `non_transparent_api.rs`). Le code en aval peut vérifier cet indicateur et cette branche lorsqu'il doit recourir à des assistants opaques.
- Activation : ajoutez `features = ["transparent_api"]` à la dépendance dans `Cargo.toml`. Les caisses d'espace de travail qui nécessitent la projection JSON (par exemple, `iroha_torii`) transmettent automatiquement l'indicateur, mais les consommateurs tiers doivent le conserver à moins qu'ils ne contrôlent le déploiement et n'acceptent la surface d'API plus large.

## Exemples rapides

Créez un domaine et un compte, définissez un actif et créez une transaction avec des instructions :

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(domain_id.clone(), kp.public_key().clone());
let new_account = Account::new(account_id.clone()).with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "xor#wonderland".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

Interrogez les comptes et les actifs avec le DSL :

```rust
use iroha_data_model::prelude::*;

let predicate = query::dsl::CompoundPredicate::build(|p| {
    p.equals("metadata.tier", 1_u32)
        .exists("metadata.display_name")
});
let selector = query::dsl::SelectorTuple::default();
let q: QueryBox<QueryOutputBatchBox> =
    QueryWithFilter::new(
        Box::new(query::account::FindAccounts),
        predicate,
        selector,
    ).into();
// Encode and send via Torii; decode on server using the query registry
```

Utilisez le bytecode du contrat intelligent IVM :

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## Gestion des versions

- `SignedTransaction`, `SignedBlock` et `SignedQuery` sont des structures canoniques codées en Norito. Chacun implémente `iroha_version::Version` pour préfixer sa charge utile avec la version ABI actuelle (actuellement `1`) lorsqu'elle est codée via `EncodeVersioned`.

## Notes de révision/mises à jour potentielles

- Requête DSL : envisagez de documenter un sous-ensemble stable destiné aux utilisateurs et des exemples de filtres/sélecteurs courants.
- Familles d'instructions : développez les documents publics répertoriant les variantes ISI intégrées exposées par `mint_burn`, `register`, `transfer`.

---
Si une partie nécessite plus de profondeur (par exemple, un catalogue ISI complet, une liste complète du registre des requêtes ou des champs d'en-tête de bloc), faites-le-moi savoir et j'étendrai ces sections en conséquence.