---
lang: dz
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b6388355a41797eb7d0b7f47cfa8fcac4e136c5a2e5eb0a264384ecdba930b8
source_last_modified: "2026-02-01T13:51:49.945202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 གནད་སྡུད་དཔེ་གཟུགས་ – གཏིང་ཟབ་པའི་མཆོངས།

ཡིག་ཆ་འདི་གིས་ `iroha_data_model` cret ནང་ལུ་ ལག་ལེན་འཐབ་སྟེ་ `iroha_data_model` cret ནང་ལུ་ ལག་ལེན་འཐབ་སྟེ་ཡོད་པའི་ Iroha v2 གནད་སྡུད་དཔེ་ཚད་འདི་བཟོ་མི་ གཞི་བཀོད་དང་ ངོས་འཛིན་འབད་མི་ རང་གཤིས་ དེ་ལས་ མཐུན་སྒྲིག་ཚུ་ འགྲེལ་བཤད་རྐྱབ་ཨིན། འདི་ཡང་ ཁྱོད་ཀྱིས་བསྐྱར་ཞིབ་འབད་ཚུགས་པའི་ ཁུངས་གཏུག་གཏན་གཏན་ཅིག་འོང་དགོཔ་དང་ དུས་མཐུན་བཟོ་ནི་གི་གྲོས་འཆར་བཀོད་དགོཔ་ཨིན།

## ཁྱབ་ཁོངས་དང་གཞི་རྩ།

- དམིགས་ཡུལ་: མངའ་ཁོངས་དངོས་པོ་ (མངའ་ཁོངས་ རྩིས་ཁྲ་ རྒྱུ་དངོས་ ཨེན་ཨེཕ་ཊི་ འགན་ཁུར་ གནང་བ་ མཉམ་རོགས་) ཚུ་གི་དོན་ལུ་ ཁྲིམས་ལུགས་ཀྱི་དབྱེ་བ་ཚུ་བྱིནམ་ཨིན།
- རིམ་སྒྲིག་: མི་མང་དབྱེ་བ་ཆ་མཉམ་ Norito ཀོ་ཌེཀསི་ (`norito::codec::{Encode, Decode}`) དང་ འཆར་གཞི་ (`iroha_schema::IntoSchema`) ལས་ཐོན་ཡོདཔ། JSON འདི་ སེལ་འཐུ་འབད་དེ་ལག་ལེན་འཐབ་ཨིན་ (དཔེར་ན་ HTTP དང་ `Json` pabloads) ཁྱད་རྣམ་དར་ཆ་ཚུ་གི་རྒྱབ་ཁར་ཨིན།
- IVM not: Iroha བར་ཅུ་འཕྲུལ་ཆས་ (IVM) ལུ་དམིགས་གཏད་བསྐྱེད་པའི་སྐབས་ གཏན་འཁེལ་མེད་པའི་དུས་ཚོད་བདེན་དཔྱད་ལ་ལུ་ཅིག་ ལྕོགས་མིན་བཟོཝ་ཨིན།
- FFI gates: དབྱེ་བ་ལ་ལུ་ཅིག་ FFI དགོས་མཁོ་མེད་པའི་སྐབས་ མཐོ་ཚད་ལས་ བཀག་ཐབས་ལུ་ `iroha_ffi` བརྒྱུད་དེ་ Norito བརྒྱུད་དེ་ FFI གི་དོན་ལུ་ ཆ་རྐྱེན་ཅན་གྱི་ཐོག་ལས་ བརྡ་སྟོན་འབདཝ་ཨིན།

## ཁྱད་ཆོས་དང་རོགས་སྐྱོར།

- `Identifiable`: ངོ་བོ་ཚུ་ལུ་ བརྟན་ཏོག་ཏོ་ `Id` དང་ `fn id(&self) -> &Self::Id`. སབ་ཁྲ་/ཆ་ཚན་མཐུན་འབྲེལ་གྱི་དོན་ལུ་ `IdEqOrdHash` ལས་ཐོབ་དགོ།
- `Registrable`/`Registered`: ངོ་བོ་ལེ་ཤ་ཅིག་ (དཔེར་ན་ `Domain`, `AssetDefinition`, `Role`) བཟོ་བསྐྲུན་པ་གི་དཔེ་རིས་ལག་ལེན་འཐབ་ཨིན། `Registered` གིས་ ཐོ་བཀོད་ཚོང་འབྲེལ་གྱི་དོན་ལུ་འོས་འབབ་ཅན་གྱི་ ལྗིད་ཚད་མར་ཕབ་འབད་མི་ བཟོ་བསྐྲུན་པ་དབྱེ་བ་ (`With`) ལུ་ རན་དུས་ཚོད་ཀྱི་དབྱེ་བ་འདི་ འབྲེལ་མཐུད་འབདཝ་ཨིན།
- `HasMetadata`: ལྡེ་མིག་/གནས་གོང་ `Metadata` སབ་ཁྲ་ཅིག་ལུ་ མཉམ་བསྡོམས་འཛུལ་སྤྱོད་འབད་ནི།
- `IntoKeyValue`: འདྲ་བཤུས་མར་ཕབ་འབད་ནིའི་དོན་ལུ་ `Key` (ID) དང་ `Value` (གནས་སྡུད་) གསོག་འཇོག་འབད་ནི་ལུ་ གྲོགས་རམ་གྱི་བགོ་བཤའ་རྐྱབ་མི།
- `Owned<T>`/Norito: དགོས་མཁོ་མེད་པའི་འདྲ་བཤུས་ཚུ་ བཀག་ཐབས་ལུ་ གསོག་འཇོག་དང་ འདྲི་དཔྱད་ཚགས་མ་ཚུ་ནང་ ལག་ལེན་འཐབ་མི་ ལྗིད་ཚད་འཇམ་པའི་ བཀབ་ཆ་ཚུ།

## མིང་དང་ངོས་འཛིན།

- `Name`: ནུས་ཅན་གྱི་ཚིག་ཡིག་ངོས་འཛིན་འབད་མི། བར་སྟོང་དཀརཔོ་དང་ བཀག་བཞག་ཡོད་པའི་ཡིག་འབྲུ་ཚུ་ `@`, `#`, `$` (ཀམ་པོ་སི་ཊི་ཨའི་ཌི་ནང་ལག་ལེན་འཐབ་ཡོདཔ་ཨིན།) བདེན་དཔྱད་དང་གཅིག་ཁར་ `FromStr` བརྒྱུད་དེ་བཟོ་བསྐྲུན་འབད་ཡོདཔ། མིང་ཚུ་ པཱར་སི་གུར་ ཡུ་ནི་ཀོཌ་ཨེན་ཨེཕ་སི་ལུ་ སྤྱིར་བཏང་སྦེ་བཟོཝ་ཨིན། དམིགས་བསལ་གྱི་མིང་ `genesis` བཀག་བཞག་ཡོདཔ་ཨིན།
- `IdBox`: རྒྱབ་སྐྱོར་འབད་ཡོད་པའི་ ID (`DomainId`, `AccountId`, `AssetDefinitionId`, `AssetId`, `NftId`, ID (`AccountId`, `AssetId`, `NftId`, ID (`AssetDefinitionId`, ID 18NI00000067X, ID `PeerId`, `TriggerId`, `RoleId`, `Permission`, `Permission`, Norito) སྤྱིར་བཏང་རྒྱུན་འབབ་དང་ Norito ཨིན་ཀོ་ཌིང་འདི་དབྱེ་བ་གཅིག་སྦེ་ཕན་ཐོགས།
- `ChainId`: ཚོང་འབྲེལ་ནང་ བསྐྱར་རྩེད་སྲུང་སྐྱོབ་ཀྱི་དོན་ལུ་ ལག་ལེན་འཐབ་མི་ ཨོ་པ་ཀི་རིམ་སྒྲིག་ངོས་འཛིན་འབད་མི།IDs གི་ཡིག་རྒྱུན་འབྲི་ཤོག་ (Norito/`FromStr`) དང་བཅས་པའི་སྐོར་རིམ་འབྲི་ཤོག་ཚུ།
- `DomainId`: `name` (དཔེར་ན་ Norito).
- Norito: I105 དང་ Sora བསྡམ་བཞག་མི་ (`i105`) དེ་ལས་ ཀེ་ནོ་ནིག་ཧེགསི་ཀོམ་ཌི་སི (`AccountAddress::to_i105`, `to_i105`, བརྒྱུད་དེ་ ཨིན་ཀོཌི་འབད་ཡོདཔ་ཨིན། `canonical_hex`, `parse_encoded`). I105 འདི་ གདམ་ཁ་ཅན་གྱི་རྩིས་ཐོའི་རྩ་སྒྲིག་ཨིན། `i105` འབྲི་ཤོག་འདི་ Sora-only UX གི་དོན་ལུ་ དྲག་ཤོས་གཉིས་པ་ཨིན། མི་ལུ་མཐུན་སྒྲིག་ཡོད་པའི་ འགྲུལ་ལམ་མིང་གཞན་ `alias` (rejected legacy form) འདི་ UX གི་དོན་ལུ་ ཉམས་སྲུང་འབད་དེ་ཡོདཔ་ཨིན་རུང་ ད་ལས་ཕར་ དབང་ཚད་ཅན་གྱི་ངོས་འཛིན་འབད་མི་ཅིག་སྦེ་ བརྩི་འཇོག་འབདཝ་ཨིན། Torii `AccountAddress::parse_encoded` བརྒྱུད་དེ་ ནང་འོང་ཡིག་རྒྱུན་ཚུ་ སྤྱིར་བཏང་བཟོཝ་ཨིན། རྩིས་ཐོ་ཨའི་ཌི་ཚུ་གིས་ ལྡེ་མིག་གཅིག་དང་ སྣ་མང་སིག་ཚད་འཛིན་གཉིས་ཆ་རང་ལུ་རྒྱབ་སྐྱོར་འབདཝ་ཨིན།
- `AssetDefinitionId`: `asset#domain` (དཔེར་ན་ `xor#soramitsu`).
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId`: `nft$domain` (དཔེར་ན་ `rose$garden`).
- `PeerId`: Kotodama (མཉམ་རོགས་འདྲ་མཉམ་འདི་ མི་མང་ལྡེ་མིག་གིས་)

## ངོ་བོ་ཚུ།

### མངའ༌ཁོངས
- `DomainId { name: Name }` – གཞན་དང་མ་འདྲ་བའི་མིང་།
- `Domain { id, logo: Option<IpfsPath>, metadata: Metadata, owned_by: AccountId }`.
- བཟོ་བསྐྲུན་པ་: `NewDomain`, `with_logo`, `with_metadata`, དེ་ལས་ `Registrable::build(authority)`, དེ་ལས་ `owned_by`.

### རྩིས་ཁྲ
- `AccountId { domain: DomainId, controller: AccountController }` (ཚད་འཛིན་ = ལྡེ་མིག་རྐྱང་པ་ ཡང་ན་ སྣ་མང་རིག་གཞུང་གི་སྲིད་བྱུས་)།
- `Account { id, metadata, label?, uaid? }` — `label` འདི་ བསྐྱར་ལོག་དྲན་ཐོ་ཚུ་གིས་ལག་ལེན་འཐབ་མི་ གདམ་ཁ་ཅན་གྱི་ བརྟན་བཞུགས་ཀྱི་ མིང་ཚིག་ཅིག་ཨིན།
- བཟོ་བསྐྲུན་པ་: `NewAccount` བརྒྱུད་དེ་ `Account::new(id)`; `HasMetadata` བཟོ་བསྐྲུན་པ་དང་ངོ་བོ་གཉིས་ཆ་རའི་དོན་ལུ་ཨིན།

### རྒྱུ་དངོས་ངེས་ཚིག དང་རྒྱུ་ཆ།
- `AssetDefinitionId { domain: DomainId, name: Name }`.
- `AssetDefinition { id, spec: NumericSpec, mintable: Mintable, logo: Option<IpfsPath>, metadata, owned_by: AccountId, total_quantity: Numeric }`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - བཟོ་བསྐྲུན་པ་: `AssetDefinition::new(id, spec)` ཡང་ན་ སྟབས་བདེ་ `numeric(id)`; `metadata`, `mintable`, `owned_by`.
- `AssetId { account: AccountId, definition: AssetDefinitionId }`.
- `Asset { id, value: Numeric }` དང་མཉམ་པའི་ གསོག་འཇོག་ `AssetEntry`/`AssetValue`.
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` བཅུད་དོན་ཨེ་པི་ཨའི་ཚུ་གི་དོན་ལུ་ ཕྱིར་བཏོན་འབད་ཡོདཔ།

### ཨེན་ཨེཕ་ཊི་ཚུ།
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (ནང་དོན་འདི་ གང་བྱུང་ལྡེ་མིག་/གནས་གོང་མེ་ཊ་ཌེ་ཊ་) ཨིན།
- བཟོ་བསྐྲུན་པ།: `NewNft` བརྒྱུད་དེ་ `Nft::new(id, content)`.

### འགན་ཁུར་དང་གནང་བ།
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` དང་ བཟོ་བསྐྲུན་པ་ `NewRole { inner: Role, grant_to: AccountId }`.
- `Permission { name: Ident, payload: Json }` – `name` དང་ པེ་ལོཌི་འཆར་གཞི་འདི་ ཤུགས་ལྡན་ `ExecutorDataModel` (འོག་ལུ་བལྟ།)

### འདྲ་མཉམ་གྱི་མི
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` དང་ ཆ་འཇོག་འབད་བཏུབ་པའི་ `public_key@address` ཡིག་རྒྱུན་འབྲི་མ།### ཀིརིཔ་ཊོ་གཱ་ར་ཕིག་གི་ དང་ཕུག་ (ཁྱད་རྣམ་ `sm`)
- `Sm2PublicKey` དང་ `Sm2Signature`: SEC1-མཐུན་སྒྲིག་ས་ཚིགས་དང་ གཏན་འཇགས་རྒྱ་ཚད་ `r∥s` མིང་རྟགས་ཚུ། བཟོ་བསྐྲུན་པ་ཚུ་གིས་ གུག་གུགཔ་འཐུས་མི་དང་ ངོ་རྟགས་ཚུ་ དབྱེ་བ་ཕྱེ་ཚུགས། Norito ཨིན་ཀོ་ཌིང་གིས་ `iroha_crypto` གིས་ལག་ལེན་འཐབ་མི་ ཁྲིམས་ལུགས་ངོ་ཚབ་འདི་ གསལ་སྟོན་འབདཝ་ཨིན།
- `Sm3Hash`: གསལ་སྟོན་དང་ བརྒྱུད་འཕྲིན་ དེ་ལས་ སི་ཀཱལ་ལན་འདེབས་ཚུ་ནང་ ལག་ལེན་འཐབ་མི་ ཇི་ཨེམ་/ཊི་ ༠༠༠༤ ཌའི་ཇེསཊ་ ངོ་ཚབ་འབད་མི་ `[u8; 32]` དབྱེ་བ་གསརཔ་གི་དབྱེ་བ་གསརཔ་ཨིན།
- `Sm4Key`: ཧོསིཊི་སི་སི་ཀཱལསི་དང་ གནད་སྡུད་དཔེ་ཚད་ཀྱི་སྒྲིག་བཀོད་ཚུ་གི་བར་ན་ བརྗེ་སོར་འབད་ཡོད་པའི་ ༡༢༨-བིཊི་ མཉམ་མཐུན་ལྡེ་མིག་ བཀབ་བཞག་འབད་ཡོདཔ།
འདི་བཟུམ་མའི་རིགས་འདི་ ད་ལྟོ་ཡོད་པའི་ Ed25519/BLS/ML-DSA གི་ གནའ་དུས་དང་ ལཱ་གི་ས་སྒོ་འདི་ `--features sm` དང་ཅིག་ཁར་ བཟོ་བསྐྲུན་འབད་ཚར་བའི་ཤུལ་ལས་ མི་མང་གི་འཆར་གཞི་གི་ཆ་ཤས་ཅིག་ལུ་འགྱུར་དོ་ཡོདཔ་ཨིན།

### ཀྲི་གཱར་དང་ལས་རིམ།
- `TriggerId { name: Name }` དང་ `Trigger { id, action: action::Action }`.
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` ཡང་ན་ `Exactly(u32)`; བཀོད་སྒྲིག་དང་ ཆག་ཆག་གི་མཐུན་རྐྱེན་ཚུ་ཚུདཔ་ཨིན།
  - ཉེན་སྲུང་: `TriggerCompleted` འདི་ བྱ་བའི་ཚགས་མ་སྦེ་ ལག་ལེན་འཐབ་མི་བཏུབ།
- `EventBox`: མདོང་ལམ་དང་ པའིཔ་ལའིན་-བེཆ་ གནད་སྡུད་ དུས་ཚོད་ བཀོལ་སྤྱོད་-trigger དང་ ཊི་གར་-མཇུག་བསྡུ་ཡོད་པའི་བྱུང་ལས་ཚུ་གི་དོན་ལུ་ བསྡོམས་རྩིས་དབྱེ་བ་; `EventFilterBox` གིས་ མཐུན་རྐྱེན་དང་ ཊི་གར་ཚགས་མ་ཚུ་གི་དོན་ལུ་ དེ་ མེ་ལོང་བཟོཝ་ཨིན།

## ཚད་བཟུང་དང་རིམ་སྒྲིག་།

- ལམ་ལུགས་ཚད་གཞི་བཟའ་ཚང་ (ཆ་མཉམ་ `Default`ed དང་ ལེན་ཆས་འབག་ དེ་ལས་ ངོ་རྐྱང་གི་ནམ་ཚུ་ལུ་ གཞི་བསྒྱུར་འབད་ནི།):
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` བཟའ་ཚང་ཆ་མཉམ་དང་ `custom: BTreeMap<CustomParameterId, CustomParameter>` སྡེ་ཚན་བཟོཝ་ཨིན།
- གཅིག་རྐྱང་པའི་ཚད་བཟུང་ རྡུལ་ཕྲན་ཚུ་: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`, ཌིཕ་བཟུམ་གྱི་དུས་མཐུན་དང་བསྐྱར་བརྗོད་ཀྱི་དོན་ལུ་ Norito.
- སྲོལ་སྒྲིག་ཚད་གཞི་ཚུ་: བཀོལ་སྤྱོད་པ་-ངེས་འཛིན་འབད་ཡོད་པའི་ `CustomParameterId` (a `Name`) གིས་ངོས་འཛིན་འབད་མི་ `Json` སྦེ་འབག་ཡོདཔ་ཨིན།

## ISI (Iroha དམིགས་བསལ་བརྡ་སྟོན།)

- རང་གཤིས་གཙོ་བོ་: `Instruction` `dyn_encode`, `as_any`, དང་ དབྱེ་བ་རེ་རེའི་ངོས་འཛིན་པ་ `id()` (བརྟན་པོའི་དབྱེ་བ་མིང་ལུ་རྒྱབ་གཏད)། བཀོད་རྒྱ་ཆ་མཉམ་ `Send + Sync + 'static` ཡིན།
- `InstructionBox`: བདག་དབང་ `Box<dyn Instruction>` གིས་ རིགས་མཚུངས་/eq/ord དང་ དབྱེ་བ་ཨའི་ཌི་ + ཨིན་ཀོཌི་འབད་ཡོད་པའི་བཱའིཊི་ཚུ་བརྒྱུད་དེ་ ལག་ལེན་འཐབ་ཡོདཔ་ཨིན།
- བཟོ་བསྐྲུན་གྱི་སློབ་སྟོན་བཟའ་ཚང་ཚུ་ འོག་ལུ་བཀོད་ཡོདཔ་ཨིན།
  - `mint_burn`, `transfer`, `register`, དང་ `transparent` གྲོགས་རམ་གྱི་བང་རིམ།
  - མེ་ཊ་ཕོལོ་ཚུ་གི་དོན་ལུ་ མདའ་རྟགས་ཚུ་ དབྱེ་བ་ དབྱེ་བ་བརྩམ་སྟེ་: `InstructionType`, `SetKeyValueBox` བཟུམ་གྱི་ སྒྲོམ་ནང་ཡོད་པའི་བསྡོམས་ཚུ་ (མངའ་ཁོངས་/རྩིས་ཐོ་/རྒྱུ་དངོས་_Def/nft/tricger).
- འཛོལ་བ་: `isi::error` གི་འོག་ལུ་ འཛོལ་བ་ཕྱུགཔོ་གི་དཔེ་ཚད་ (བརྟག་ཞིབ་དབྱེ་བ་འཛོལ་བ་, འཛོལ་བ།, དཔེ་སྟོན་, ཨང་རྩིས་, ནུས་མེད་ཚད་གཞི་ ནུས་མེད་ བསྐྱར་ཟློས་, འགྱུར་ལྡོག་ཅན་)།)
- བཀོད་རྒྱ་ཐོ་བཀོད་: `instruction_registry!{ ... }` མེཀ་རོ་གིས་ མིང་གིས་ལྡེ་མིག་བརྐྱབ་ཡོད་པའི་ རན་ཊའིམ་ཌི་ཀོཌི་ཐོ་བཀོད་ཅིག་བཟོ་བསྐྲུན་འབདཝ་ཨིན། `InstructionBox` ཀླད་ཀོར་དང་ Norito serde གིས་ ཌའི་ནམ་(དེ་)རིམ་སྒྲིག་གྲུབ་ཐབས་ལུ་ ལག་ལེན་འཐབ་ཡོདཔ་ཨིན། ཐོ་བཀོད་གང་རུང་ཅིག་ `set_instruction_registry(...)` བརྒྱུད་དེ་ གསལ་ཏོག་ཏོ་སྦེ་ གཞི་སྒྲིག་མ་འབད་བ་ཅིན་ གཉིས་ལྡན་ཚུ་ སྒྲིང་སྒྲིང་སྦེ་བཞག་ནི་གི་དོན་ལུ་ འགོ་དང་པ་ ལག་ལེན་ཐོག་ལུ་ ISI གི་ སྔོན་སྒྲིག་ཐོ་བཀོད་འདི་ གཞི་བཙུགས་འབད་དེ་ཡོདཔ་ཨིན།

## བསྐུར་རྒྱ།- `Executable`: ཡང་ན་ `Instructions(ConstVec<InstructionBox>)` ཡང་ན་ `Ivm(IvmBytecode)`. `IvmBytecode` གིས་ base64 སྦེ་ རིམ་སྒྲིག་འབདཝ་ཨིན། (`Vec<u8>` ལས་ དྭངས་གསལ་གྱི་དབྱེ་བ་གསརཔ་)།
- `TransactionBuilder`: `chain`, `authority`, `creation_time_ms`, གདམ་ཁ་ཅན་གྱི་`time_to_live_ms` དང་ `time_to_live_ms`, Norito, Norito, དང་ an 18NI00000201X, དང་ `creation_time_ms`, དང་བཅས་པའི་ཚོང་འབྲེལ་གྱི་བཀག་ཆ་ཅིག་བཟོ་བསྐྲུན་འབདཝ་ཨིན། `Executable`.
  `with_instructions`, `with_bytecode`, `with_executable`, Kotodama, `set_nonce`, `set_ttl`, IVM, IVM, `set_creation_time`, Norito, Norito, Norito, Norito, Norito, `sign`.
- `SignedTransaction` (`iroha_version` དང་བཅས་ཐོན་རིམ་): `TransactionSignature` དང་ པེ་ལོཌ་ཚུ་ འབག་འོང་། ཧ་ཤིང་དང་མིང་རྟགས་བདེན་དཔྱད་བྱིནམ་ཨིན།
- འཛུལ་ཞུགས་དང་གྲུབ་འབྲས།
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - Norito = `Result<DataTriggerSequence, TransactionRejectionReason>` ཧ་ཤིང་གྲོགས་རམ་པ་ཚུ་དང་གཅིག་ཁར་།
  - `ExecutionStep(ConstVec<InstructionBox>)`: ཚོང་འབྲེལ་ནང་ བཀོད་རྒྱ་གཅིག་ བཀོད་སྒྲིག་འབད་ཡོད་པའི་ བཀོད་རྒྱ་གཅིག་ཨིན།

## སྡེབ་ཚན་ཚུ།

- `SignedBlock` (ཐོན་རིམ་) བསྡུ་སྒྲིག་འབདཝ་ཨིན།
  - `signatures: BTreeSet<BlockSignature>` (བདེན་དཔྱད་འབད་མི་ཚུ་ལས་)།
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`,
  - `result: BlockResult` (མཉམ་པའི་ལག་ལེན་གནས་སྟངས་) དང་ `time_triggers` དང་ འཛུལ་ཞུགས་/གྲུབ་འབྲས་ Markle ཤིང་ཚུ་ `transaction_results`, དང་ `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>`.
`presigned`, Norito, Norito, `header()`, `signatures()`, `signatures()`, `hash()`, `hash()`, Norito. `replace_signatures`.
- Merkle roots: ཚོང་འབྲེལ་གྱི་འཛུལ་སྒོ་དང་གྲུབ་འབྲས་ཚུ་ Merkle ཤིང་ཚུ་བརྒྱུད་དེ་ འབད་ཡོདཔ་ཨིན། resport མེར་ཀལ་རྩ་འདི་ བཀག་ཆ་མགོ་ཡིག་ནང་ལུ་བཙུགས་ཡོདཔ་ཨིན།
- བཀག་ཆ་འབད་ཡོད་པའི་བདེན་ཁུངས་ (`BlockProofs`) གིས་ ཐོ་བཀོད་/གྲུབ་འབྲས་གཉིས་ཆ་ར་ ཕྱིར་བཏོན་འབདཝ་ཨིན་ དེ་ལས་ `fastpq_transcripts` སབ་ཁྲ་འདི་གིས་ རིམ་སྒྲིག་འབད་མི་ བརྡ་རྟགས་ཚུ་གིས་ ཚོང་འབྲེལ་ཧེཤ་དང་འབྲེལ་བའི་ སྤོ་བཤུད་ཀྱི་ཌེལ་ཊ་ཚུ་ ལེན་ཚུགས།
- `ExecWitness` འཕྲིན་དོན་ (Torii དང་ ཕགཔ་གི་རྒྱབ་སྐྱོར་ཐོག་ལས་ མོས་མཐུན་གྱི་ འགོས་) ད་ལྟོ་ `fastpq_transcripts` དང་ དཔེ་སྟོན་གྱི་ `fastpq_batches: Vec<FastpqTransitionBatch>` གཉིས་ཆ་ར་ ཚུད་དེ་ཡོདཔ་ཨིན། tootes, perm_root, tx_set_hash), དེ་འབདཝ་ལས་ ཕྱིའི་མཆོདཔ་ཚུ་གིས་ ལོག་སྟེ་ཨེན་ཀོ་ཌིང་ཡིག་བསྒྱུར་མེད་པར་ ཕེསི་ཊི་པི་ཀིའུ་གྲལ་ཐིག་ཚུ་ བཙུགས་ཚུགས།

## འདྲི་དཔྱད།

- བྲོ་བ་གཉིས།
  - སིང་གལ་: ལག་ལེན་འཐབ་ཨིན། `SingularQuery<Output>` (དཔེར་ན་ `FindParameters`, `FindExecutorDataModel`).
  - བསྐྱར་ལོག་འབད་ཚུགསཔ་: `Query<Item>` (དཔེར་ན་ `FindAccounts`, `FindAssets`, `FindDomains`, ལ་སོགས་པ་ཚུ་)།
- དབྱེ་བ་མེདཔ་བཟོ་ཡོད་པའི་འབྲི་ཤོག་ཚུ།
  - `QueryBox<T>` འདི་ སྒྲོམ་ནང་བཙུགས་ཏེ་ཡོདཔ་ད་ `Query<Item = T>` འདི་ Norito དང་གཅིག་ཁར་ འཛམ་གླིང་ཐོ་བཀོད་ཅིག་གིས་རྒྱབ་སྐྱོར་འབད་ཡོདཔ་ཨིན།
  - `QueryWithFilter<T> { query, predicate, selector }` གིས་ ཌི་ཨེསི་ཨེལ་ སྔོན་སྒྲིག་/སེལ་འཐུའི་/སེལ་འཐུའི་ འདྲི་དཔྱད་ཆ་སྒྲིག་འབདཝ་ཨིན། `From` བརྒྱུད་དེ་ བཏོན་གཏང་ཡོད་པའི་བསྐྱར་ལོག་འདྲི་དཔྱད་ལུ་གཞི་བསྒྱུར་འབདཝ་ཨིན།
- ཐོ་འགོད་དང་གསང་གྲངས།
  - `query_registry!{ ... }` གིས་ ཌའི་ནམ་ཌི་ཀོཌི་གི་དོན་ལུ་ མིང་ཡིག་དཔར་རྐྱབས་ཐོག་ལས་ བཟོ་བསྐྲུན་པ་ཚུ་ལུ་ འཛམ་གླིང་ཐོ་བཀོད་ཀྱི་ སབ་ཁྲ་གི་སབ་ཁྲ་བཟོ་ཐངས་ཀྱི་དབྱེ་བ་ཅིག་ བཟོ་བསྐྲུན་འབདཝ་ཨིན།
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` དང་ `QueryResponse = Singular(..) | Iterable(QueryOutput)`.
  - `QueryOutputBatchBox` འདི་ གཅིག་མཚུངས་སྦེ་ཡོད་མི་ ཝེག་ཊར་ཚུ་གི་ཐོག་ལུ་ བསྡོམས་ཀྱི་དབྱེ་བ་ཅིག་ཨིན། (དཔེར་ན་ `Vec<Account>`, `Vec<Name>`, `Vec<AssetDefinition>`, Norito), དེ་ལས་ ཤོག་ལེབ་དང་རྒྱ་བསྐྱེད་ཀྱི་གྲོགས་རམ་པ་ཚུ་ལུ་ ཚིག་རྒྱན་དང་ རྒྱ་བསྐྱེད་ཀྱི་གྲོགས་རམ་པ་ཨིན།
- DSL: པར་བརྙན་གྱི་རང་གཤིས་ཚུ་དང་གཅིག་ཁར་ `query::dsl` ནང་ལུ་ལག་ལེན་འཐབ་ཡོདཔ་ཨིན། `fast_dsl` གིས་ དགོས་མཁོ་ཡོད་པ་ཅིན་ འགྱུར་ལྡོག་ཅན་གྱི་དབྱེ་བ་ཅིག་ གསལ་སྟོན་འབདཝ་ཨིན།

## བཀོལ་སྤྱོད་པ་དང་རྒྱ་ཆེན།- `Executor { bytecode: IvmBytecode }`: བདེན་དཔྱད་འབད་མི་གིས་ བཀོལ་སྤྱོད་འབད་ཡོད་པའི་ཨང་རྟགས་བཱན་ཌལ་འདི་ཨིན།
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` གིས་ བཀོལ་སྤྱོད་པ་-ངེས་འཛིན་འབད་ཡོད་པའི་མངའ་ཁོངས་གསལ་བསྒྲགས་འབདཝ་ཨིན།
  - སྲོལ་སྒྲིག་རིམ་སྒྲིག་ཚད་བཟུང་ཚུ།
  - སྲོལ་སྒྲིག་བཀོད་རྒྱ་ངོས་འཛིན་ཚུ།
  - འཛུལ་ཞུགས་ཊོ་ཀེན་ངོས་འཛིན་ཚུ།
  - མཁོ་སྤྲོད་འབད་མི་ ལག་ཆས་དོན་ལུ་ སྲོལ་སྒྲིག་དབྱེ་བ་ཚུ་ འགྲེལ་བཤད་རྐྱབ་མི་ JSON ལས་རིམ་ཅིག།
- `data_model/samples/executor_custom_data_model` འོག་ལུ་ རང་མོས་བཟོ་བཅོས་དཔེ་ཚད་ཡོདཔ་ཨིན།
  - `iroha_executor_data_model::permission::Permission` བརྒྱུད་དེ་ སྲོལ་སྒྲིག་གནང་བ་ཊོ་ཀེན་འདི་ བཏོན་གཏང་།
  - སྲོལ་སྒྲིག་ཚད་བཟུང་འདི་ `CustomParameter` ལུ་གཞི་བསྒྱུར་འབད་བཏུབ་སྦེ་གཞི་བསྒྱུར་འབད་བཏུབ་སྦེ་ངེས་འཛིན་འབད་ཡོདཔ།
  - ལག་ལེན་འཐབ་ནིའི་དོན་ལུ་ `CustomInstruction` ནང་ལུ་ རིམ་སྒྲིག་འབད་ཡོད་པའི་སྲོལ་སྒྲིག་བཀོད་རྒྱ་ཚུ།

### སྲོལ་སྒྲིག་བཀོད་རྒྱ་ (བཀོད་ཁྱབ་-ངེས་འཛིན་འབད་ཡོད་པའི་ཨའི་ཨེསི་ཨའི་)

- དབྱེ་བ་: `isi::CustomInstruction { payload: Json }` བརྟན་ཏོག་ཏོ་ཐགཔ་ཨའི་ཌི་ `"iroha.custom"`.
- དམིགས་ཡུལ་: སྒེར་གྱི་/མཉམ་འབྲེལ་ཡོངས་འབྲེལ་ནང་ ལག་ལེན་འཐབ་མི་ དམིགས་བསལ་གྱི་བཀོད་རྒྱ་ཚུ་གི་དོན་ལུ་ ཡང་ན་ མི་མང་གི་གནས་སྡུད་དཔེ་ཚད་འདི་ མ་བཙུགས་པར་ བཟོ་དཔེ་བཟོ་ནི་གི་དོན་ལུ་ ཡིག་ཤུགས།
- སྔོན་སྒྲིག་ལག་ལེན་པའི་སྤྱོད་ལམ་: `iroha_core` ནང་ཡོད་པའི་ ནང་འཁོད་ལག་ལེན་འཐབ་མི་འདི་གིས་ `CustomInstruction` ལག་ལེན་འཐབ་མི་བཏུབ་ནི་དང་ འཕྱད་པ་ཅིན་ འདྲོག་འོང་། སྲོལ་སྒྲིག་བཀོལ་སྤྱོད་པ་གིས་ `InstructionBox` ལུ་ `CustomInstruction` ལུ་མར་ཕབ་འབད་དེ་ བདེན་དཔྱད་འབད་མི་ཆ་མཉམ་གུ་ པེ་ལོཌི་འདི་ གཏན་འབེབས་བཟོ་དགོ།
- Norito: ལས་འཆར་དང་གཅིག་ཁར་ `norito::codec::{Encode, Decode}` བརྒྱུད་དེ་ ཨིན་ཀོ་ཌི/ཌི་ཀོཌ། `Json` པེ་ལོཌི་འདི་ རིམ་སྒྲིག་ཐོག་ལས་ གཏན་འབེབས་བཟོཝ་ཨིན། བཀོད་རྒྱ་ཐོ་བཀོད་ནང་ `CustomInstruction` ཚུད་པའི་ སྐོར་རིམ་ཚུ་ བརྟན་ཏོག་ཏོ་སྦེ་ཡོདཔ་ཨིན། (སྔོན་སྒྲིག་ཐོ་བཀོད་ཀྱི་ཆ་ཤས་ཅིག་ཨིན།)
- IVM: Kotodama བསྡུ་སྒྲིག་འབད་དེ་ IVM བཱའིཊི་ཀོཌི་ (`.to`) ལུ་ གློག་རིམ་གཏན་ཚིགས་ཀྱི་དོན་ལུ་ གྲོས་འཆར་གྱི་འགྲུལ་ལམ་ཨིན། Kotodama ནང་ལུ་ ད་ལྟོ་ཡང་ བརྡ་སྟོན་འབད་མ་ཚུགས་པའི་ ལག་ལེན་པའི་གནས་རིམ་རྒྱ་བསྐྱེད་ཚུ་གི་དོན་ལུ་ `CustomInstruction` རྐྱངམ་ཅིག་ལག་ལེན་འཐབ། མཉམ་རོགས་ཀྱིས་ གཏན་འབེབས་བཟོ་ནི་དང་ ལག་ལེན་འཐབ་མི་ འདྲ་མཚུངས་སྦེ་ཡོད་མི་གཉིས་ལྡན་ཚུ་ ངེས་གཏན་བཟོ།
- མི་མང་ཡོངས་འབྲེལ་གྱི་དོན་ལུ་མེན་པར་ མི་མང་གི་རིམ་སྒྲིག་ཚུ་གི་དོན་ལུ་ ལག་ལེན་མ་འཐབ། ཁྱོད་ལུ་ སྟེགས་བུ་ཁྱད་རྣམ་དགོ་པའི་སྐབས་ ཡར་འཕར་གྱི་ ISI ཡར་རྒྱུན་གསརཔ་གི་གྲོས་འཆར་བཀོད་ནི་ལུ་ གཙོ་བོར་བཏོན།

## མེཊ་ཌི།

- `Metadata(BTreeMap<Name, Json>)`: ལྡེ་མིག་/གནས་གོང་ཚོང་ཁང་འདི་ ངོ་བོ་ལེ་ཤ་ཅིག་ལུ་མཉམ་སྦྲགས་འབད་ཡོདཔ་ཨིན་ (`Domain`, `Account`, `AssetDefinition`, `Nft`, trigtures དང་ ཚོང་འབྲེལ་ཚུ་)
- API: Kotodama, `iter`, `get`, `insert`, དང་ (`transparent_api`, `remove`, དང་མཉམ་དུ།

## ཁྱད་ཆོས་དང་གཏན་འབེབས་རིང་བ།

- ཁྱད་རྣམ་ཚུ་གིས་ གདམ་ཁ་ཅན་གྱི་ཨེ་པི་ཨའི་ (`std`, `json`, Norito, `ffi_export`, `ffi_import`, `fast_dsl`, `fast_dsl`, Norito, `fault_injection`).
- གཏན་འབེབས་བཟོ་ནི: རིམ་སྒྲིག་ཆ་མཉམ་གྱིས་ མཐུན་རྐྱེན་ཚུ་ནང་ འབག་བཏུབ་པའི་ ཨིན་ཀོ་ཌིང་ལག་ལེན་འཐབ་ཨིན། IVM བཱའིཊི་ཀོཌི་འདི་ མ་དངུལཔ་བླུན་པོ་ཅིག་ཨིན། ལག་ལེན་འཐབ་མི་འདི་གིས་ ཐག་བཅད་མེན་པའི་མར་ཕབ་ཚུ་ འགོ་བཙུགས་མི་ཆོག། ཧོསཊི་གིས་ ཚོང་འབྲེལ་ཚུ་ བདེན་དཔྱད་འབད་ནི་དང་ བཀྲམ་སྤེལ་ཚུ་ IVM ལུ་ གཏན་འབེབས་བཟོཝ་ཨིན།

### དྭངས་གསལ་ཨེ་པི་ཨའི་ (`transparent_api`)- དམིགས་ཡུལ་: `#[model]` གི་ ནང་འཁོད་ཆ་ཤས་ཚུ་ དཔེར་ན་ Torii, བཀོལ་སྤྱོད་པ་དང་ མཉམ་བསྡོམས་བརྟག་དཔྱད་ཚུ་ ཆ་ཚང་སྦེ་ འཛུལ་སྤྱོད་འབད་ཚུགསཔ་ཨིན། དེ་མེད་པ་ཅིན་ ཅ་ཆས་ཚུ་ ཤེས་བཞིན་དུ་ གསལ་རི་རི་མེདཔ་ལས་ ཕྱི་ཁའི་ཨེསི་ཌི་ཀེ་ཚུ་གིས་ ཉེན་སྲུང་ཅན་གྱི་བཟོ་བསྐྲུན་འབད་མི་དང་ ཨེན་ཀོཌ་ པེ་ལོཌ་ཚུ་རྐྱངམ་ཅིག་ མཐོངམ་ཨིན།
- འཕྲུལ་རིག་: `iroha_data_model_derive::model` མེཀ་རོ་གིས་ `#[cfg(feature = "transparent_api")] pub` དང་གཅིག་ཁར་ མི་མང་གི་ས་སྒོ་རེ་རེ་བཞིན་ ལོག་བྲིས་ཏེ་ སྔོན་སྒྲིག་བཟོ་བསྐྲུན་གྱི་དོན་ལུ་ སྒེར་གྱི་འདྲ་བཤུས་ཅིག་བཞགཔ་ཨིན། ཁྱད་རྣམ་འདི་ སི་ཨེཕ་ཇི་དེ་ཚུ་ ལྕོགས་ཅན་བཟོ་ དེ་འབདཝ་ལས་ `Account`, `Domain`, ལ་སོགས་པ་ཚུ་ ཁོང་རའི་ངེས་འཛིན་ཚད་གཞི་གི་ཕྱི་ཁར་ ཁྲིམས་མཐུན་ལུ་འགྱུརཝ་ཨིན།
- ཁ་ཐོག་བརྟག་དཔྱད།: ཀེརེཊ་གིས་ `TRANSPARENT_API: bool` རྟག་བརྟན་ཅིག་ (`transparent_api.rs` ཡང་ན་ `non_transparent_api.rs`) ལུ་ ཆ་འཇོག་འབད་ཡོདཔ་ཨིན།) མར་ཁུའི་གསང་ཡིག་འདི་གིས་ དར་ཚིག་འདི་དང་ཡན་ལག་འདི་ དྭངས་གསལ་ཅན་གྱི་གྲོགས་རམ་པ་ཚུ་ལུ་ལོག་དགོཔ་ད་ ཞིབ་དཔྱད་འབད་ཚུགས།
- ལྕོགས་ཅན་: `Cargo.toml` ནང་ བརྟེན་སྡོད་མི་ལུ་ `features = ["transparent_api"]` ཁ་སྐོང་འབད། JSON པར་བརྙན་དགོ་མི་ ལཱ་གི་ས་སྒོ་གི་ཀེརེཊ་ (དཔེར་ན་ `iroha_torii`) འདི་ རང་བཞིན་གྱིས་ གདོང་ཁར་འགྱོ་དོ་ཡོདཔ་ཨིན་རུང་ དེ་ཚུ་གིས་ བཀྲམ་སྤེལ་ཚད་འཛིན་འབད་དེ་ ཨེ་པི་ཨའི་ རྒྱ་ཆེ་བའི་ ཁ་ཐོག་འདི་ ངོས་ལེན་མ་འབད་ཚུན་ཚོད་ ཕྱོགས་གསུམ་པའི་ ཉོ་སྤྱོད་འབད་མི་ཚུ་གིས་ བཀག་བཞག་དགོཔ་ཨིན།

## མགྱོགས་དཔེ།

མངའ་ཁོངས་དང་རྩིས་ཐོ་གསར་བསྐྲུན་འབད་ཞིནམ་ལས་ རྒྱུ་དངོས་ཅིག་ངེས་འཛིན་འབད་ཞིནམ་ལས་ བཀོད་རྒྱ་ཚུ་དང་གཅིག་ཁར་ ཚོང་འབྲེལ་ཅིག་བཟོ་བསྐྲུན་འབད།

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

ཌི་ཨེསི་ཨེལ་དང་གཅིག་ཁར་ འདྲི་དཔྱད་རྩིས་ཐོ་དང་རྒྱུ་དངོས་ཚུ།

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

IVM smart གན་རྒྱ་བཱའིཊི་ཀོཌི་ལག་ལེན་འཐབ།

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

## ཀེར་ཕྲང་འབད་ནི།

- `SignedTransaction`, དང་ `SignedBlock`, དང་ `SignedQuery` འདི་ ཀེ་ནོ་ནིག་ Kotodama འདི་ཨིན། ད་ལྟོའི་ཨེ་བི་ཨའི་ཐོན་རིམ་དང་གཅིག་ཁར་ ཁོང་རའི་པེ་ལོཌི་སྔོན་སྒྲིག་འབད་ནི་ལུ་ `iroha_version::Version` ལག་ལེན་འཐབ་ཨིན།

## བསྐྱར་ཞིབ་དྲན་འཛིན་ / འོས་ལྡན་དུས་མཐུན།

- འདྲི་དཔྱད་ཌི་ཨེསི་ཨེལ་: སྤྱིར་བཏང་ཚགས་མ་/གདམ་ཁ་ཅན་ཚུ་གི་དོན་ལུ་ ལག་ལེན་པའི་གདོང་ཕྱོགས་ཡན་ལག་ཆ་ཚན་དང་དཔེ་ཚུ་ཡིག་ཐོག་ལུ་བཀོད་ནི་ལུ་བརྩི་འཇོག་འབད།
- བཀོད་རྒྱ་བཟའ་ཚང་ཚུ་: ISI གི་དབྱེ་བ་ཚུ་ `mint_burn` གིས་ གསལ་སྟོན་འབད་མི་ ནང་འཁོད་ ISI གི་རིགས་ཚུ་ ཐོ་བཀོད་འབད་དེ་ རྒྱ་སྐྱེད་འབད།

---
གལ་སྲིད་ ཆ་ཤས་གང་རུང་ཅིག་ལུ་ གཏིང་ཚད་མངམ་དགོ་པ་ཅིན་ (དཔེར་ན་ ISI ཐོ་གཞུང་ཆ་ཚང་ འདྲི་དཔྱད་ཐོ་བཀོད་ཐོ་ཡིག་ཆ་ཚང་ ཡང་ན་ མགོ་ཡིག་ས་སྒོ་ཚུ་བཀག་ཆ་འབད་ནི) ང་ལུ་ཤེས་བཅུག་སྟེ་ དེ་དང་འཁྲིལ་ཏེ་ ང་གིས་ དབྱེ་ཁག་དེ་ཚུ་ རྒྱ་སྐྱེད་འབད་འོང་།