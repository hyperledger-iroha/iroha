---
lang: dz
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 གནད་སྡུད་དཔེ་གཟུགས་དང་ཨའི་ཨེསི་ཨའི་ — བཀོལ་སྤྱོད་པ‑ ཕྱིར་འགྱངས་ཨིསི་པེཊ་

འ་ནི་གསལ་བཀོད་འདི་ ད་རེས་ཀྱི་ `iroha_data_model` དང་ `iroha_core` ནང་ལུ་ གྲོགས་རམ་བཟོ་བཀོད་བསྐྱར་ཞིབ་འབད་ནི་གི་དོན་ལུ་ ཕྱིར་ལོག་འབད་མི་ བསྐྱར་ལོག་འབད་ཡོདཔ་ཨིན། རྒྱབ་བཤུད་ནང་ལམ་ཚུ་ དབང་ཚད་ཅན་གྱི་ཨང་རྟགས་ལུ་སྟོནམ་ཨིན།

## གོ་སྐབས
- ཁྲིམས་ལུགས་ཀྱི་ངོ་བོ་ཚུ་ (མངའ་ཁོངས་དང་ རྩིས་ཁྲ་ རྒྱུ་དངོས་ ཨེན་ཨེཕ་ཊི་ འགན་ཁུར་ མཉམ་རོགས་ མཉམ་རོགས་ ཊི་རི་ཊི་ཚུ་) དང་ ངོས་འཛིན་འབད་མི་ཚུ་ ངེས་འཛིན་འབདཝ་ཨིན།
- གནས་སྟངས་བསྒྱུར་བཅོས་འབད་ནིའི་བཀོད་རྒྱ་ (ISI): དབྱེ་བ་དང་ ཚད་གཞི་ སྔོན་སྒྲིག་གནས་སྟངས་ གནས་སྟངས་འགྱུར་བ་ བཏོན་པའི་བྱུང་ལས་ དེ་ལས་ འཛོལ་བ་གནས་སྟངས་ཚུ་ འགྲེལ་བཤད་རྐྱབ་ཨིན།
- ཚད་བཟུང་འཛིན་སྐྱོང་དང་ ཚོང་འབྲེལ་ དེ་ལས་ བཀོད་རྒྱ་རིམ་སྒྲིག་ཚུ་ བཅུད་བསྡུས་འབདཝ་ཨིན།

གཏན་འབེབས་རིང་ལུགས་: བཀོད་རྒྱ་ཡིག་བརྡའི་ཆ་མཉམ་རང་ མཐུན་རྐྱེན་ལུ་བརྟེན་པའི་སྤྱོད་ལམ་མེད་པར་ གནས་སྟངས་ཀྱི་འགྱུར་བ་ངོ་མ་ཨིན། རིམ་སྒྲིག་གིས་ Norito ལག་ལེན་འཐབ་ཨིན། ཝི་ཨེམ་བཱའིཊི་ཀོཌི་གིས་ IVM ལག་ལེན་འཐབ་ཨིནམ་དང་ རིམ་སྒྲིག་ལག་ལེན་འཐབ་པའི་ཧེ་མ་ ཧོསཊི་ཕྱོགས་འདི་ བདེན་དཔྱད་འབད་ཡོདཔ་ཨིན།

---

## ངོ་བོ་དང་ངོས་འཛིན།
IDs ཚུ་ `Display`/`FromStr` སྐོར་རིམ་དང་གཅིག་ཁར་ བརྟན་ཏོག་ཏོ་ཡོད་པའི་ཡིག་རྒྱུན་འབྲི་ཤོག་ཚུ་ཡོདཔ་ཨིན། མིང་ལམ་ལུགས་ཚུ་གིས་ བར་སྟོང་དཀརཔོ་དང་ གསོག་འཇོག་འབད་ཡོད་པའི་ `@ # $` ཡིག་འབྲུ་ཚུ།- `Name` — བདེན་དཔྱད་འབད་ཡོད་པའི་ཚིག་ཡིག་ངོས་འཛིན་འབད་མི་འདི་ཨིན། ལམ་ལུགས་: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. ཌོ་མེན་: `{ id, logo, metadata, owned_by }`. བཟོ་སྐྲུན་པ་: `NewDomain`. གསང་གྲངས་: `crates/iroha_data_model/src/domain.rs`.
- Norito — ཀེ་ནོ་ནིག་ཁ་བྱང་ཚུ་ `AccountAddress` (I105 / `i105` བསྡམ་བཞག་ཡོད་པའི་ / ཧེགསི་) དང་ `AccountAddress::parse_encoded` སྤྱིར་བཏང་ཨིན་པུཊི་ཚུ་བརྒྱུད་དེ་ བཏོན་ཡོདཔ་ཨིན། I105 འདི་ གདམ་ཁ་ཅན་གྱི་རྩིས་ཐོའི་རྩ་སྒྲིག་ཨིན། `i105` འབྲི་ཤོག་འདི་ སོ་ར་རྐྱངམ་ཅིག་ཡུ་ཨེགསི་གི་དོན་ལུ་ དྲག་ཤོས་གཉིས་པ་ཨིན། གོམས་འདྲིས་ཅན་གྱི་ `alias` (rejected legacy form) ཡིག་རྒྱུན་འདི་ འགྲུལ་ལམ་གཞན་ཅིག་སྦེ་ བཞག་སྟེ་ཡོདཔ་ཨིན། རྩིས་ཁྲ་: `{ id, metadata }`. གསང་གྲངས་: `crates/iroha_data_model/src/account.rs`.
- རྩིས་ཁྲའི་འཛུལ་ཞུགས་སྲིད་བྱུས་ — མངའ་ཁོངས་ཚུ་གིས་ རྩིས་ཁྲའི་གསར་བསྐྲུན་འདི་ Norito-JSON `AccountAdmissionPolicy` གསོག་འཇོག་འབད་དེ་ མེ་ཊ་ཌེ་ཊ་ལྡེ་མིག་ `iroha:account_admission_policy` གསོག་འཇོག་འབད་ཐོག་ལས་ ཚད་འཛིན་འབདཝ་ཨིན། ལྡེ་མིག་འདི་མེད་པའི་སྐབས་ རིམ་སྒྲིག་གནས་རིམ་གྱི་སྲོལ་སྒྲིག་ཚད་གཞི་ `iroha:default_account_admission_policy` གིས་ སྔོན་སྒྲིག་བྱིནམ་ཨིན། དེ་ཡང་མེད་པའི་སྐབས་ ཧརཌི་སྔོན་སྒྲིག་འདི་ `ImplicitReceive` (དང་པོ་གསར་བཏོན་) ཨིན། སྲིད་བྱུས་ངོ་རྟགས་ཚུ་ `mode` (`ExplicitOnly` ཡང་ན་ `ImplicitReceive`) དང་གདམ་ཁ་ཅན་གྱི་བརྒྱུད་ལམ་ (སྔོན་སྒྲིག་ `16`) དང་ བཀག་ཆ་རེ་རེ་གསར་བསྐྲུན་འབད་མི་ ཁབ་ལེན་དང་ གདམ་ཁ་ཅན་གྱི་ `implicit_creation_fee` (burn ཡང་ན་ secn count). `min_initial_amounts` རྒྱུ་དངོས་ངེས་ཚིག་དང་ གདམ་ཁ་ཅན་གྱི་ `default_role_on_create` (Norito གི་ཤུལ་ལས་ གྲོགས་རམ་ཡོདཔ་ཨིན་, Iroha དང་ཅིག་ཁར་ ངོས་ལེན་འབདཝ་ཨིན།) Genesis ནང་ལུ་ གདམ་ཁ་བརྐྱབ་མི་ཚུགས། བཀྲམ་སྟོན་/ནུས་མེད་སྲིད་བྱུས་ཚུ་གིས་ `InstructionExecutionError::AccountAdmission` དང་གཅིག་ཁར་ མ་ཤེས་པའི་རྩིས་ཐོ་ཚུ་གི་དོན་ལུ་ འཐོབ་ཐངས་ལམ་ལུགས་ཀྱི་བཀོད་རྒྱ་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན། `iroha:created_via="implicit"` གི་ཧེ་མ་ `AccountCreated`; སྔོན་སྒྲིག་འགན་ཁུར་ཚུ་གིས་ རྗེས་འཇུག་ `AccountRoleGranted`, དང་ ལག་ལེན་འཐབ་མིའི་ཇོ་བདག་གཞི་རིམ་སྒྲིག་གཞི་གིས་ རྩིས་ཁྲ་གསརཔ་འདི་གིས་ འགན་ཁུར་ཁ་སྐོང་མེད་པར་ རང་སོའི་རྒྱུ་དངོས་/ཨེན་ཨེཕ་ཊི་ཚུ་ ཟད་འགྲོ་བཏང་བཅུགཔ་ཨིན། གསང་གྲངས་: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. ངེས་ཚིག་: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. གསང་གྲངས་: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. གསང་གྲངས་: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. འགན་ཁུར་: `{ id, permissions: BTreeSet<Permission> }` བཟོ་བསྐྲུན་པ་ `NewRole { inner: Role, grant_to }`. གསང་ཨང་: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. གསང་གྲངས་: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — མཉམ་རོགས་ངོ་རྟགས་ (མི་མང་ལྡེ་མིག་) དང་ཁ་བྱང་། གསང་གྲངས་: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. ཊི་རི་གར་: `{ id, action }`. བྱ་བ་: `{ executable, repeats, authority, filter, metadata }`. གསང་གྲངས་: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — ཞིབ་དཔྱད་འབད་ཡོད་པའི་བཙུགས་/བཏོན་གཏང་ཐོག་ལས་ `BTreeMap<Name, Json>` . གསང་གྲངས་: `crates/iroha_data_model/src/metadata.rs`.
- མཐུད་ལམ་གྱི་དཔེ་རིས་ (གློག་རིམ་བང་རིམ་): འཆར་གཞི་ཚུ་ `AssetDefinition` ཐོ་བཀོད་ཚུ་ `subscription_plan` མེ་ཊ་ཌེ་ཊ་དང་གཅིག་ཁར་ཨིན། མཁོ་སྒྲུབ་ཚུ་ `Nft` དྲན་ཐོ་ཚུ་ `subscription` མེ་ཊ་ཌེ་ཊ་དང་གཅིག་ཁར་ཨིན། briling འདི་ དུས་ཚོད་དང་འཁྲིལ་ཏེ་ ལག་ལེན་འཐབ་ཨིན། `docs/source/subscriptions_api.md` དང་ `crates/iroha_data_model/src/subscription.rs` ལུ་བལྟ།
- **ཀིརིཔ་ཊོ་གཱ་ར་ཕིག་གི་ གནའ་དུས་** (ཁྱད་རྣམ་ `sm`):- `Sm2PublicKey` / `Sm2Signature` གིས་ ཀེ་ནོ་ནིག་ཨེསི་ཨི་སི་༡ ས་ཚིགས་ + གཏན་བཟོས་-རྒྱ་ཚད་ `r∥s` ཨིན་ཀོ་ཌིང་འདི་ ཨེསི་ཨེམ་༢ གི་དོན་ལུ་ཨིན། བཟོ་བསྐྲུན་པ་ཚུ་གིས་ འཐུས་མི་དང་ ID ཡིག་བརྡའི་རིག་པ་ (`DEFAULT_DISTID`) དབྱེ་བ་ཕྱེ་ནི་འདི་ བསྟར་སྤྱོད་འབདཝ་ཨིན་རུང་ བདེན་དཔྱད་འདི་གིས་ སྐྱོན་ཆ་ཅན་དང་ ཡང་ན་ ཁྱབ་ཚད་མཐོ་བའི་ འཇལ་ཚད་ཚུ་ ངོས་ལེན་འབདཝ་ཨིན། གསང་གྲངས་: `crates/iroha_crypto/src/sm.rs` དང་ `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` གིས་ GM/T 00004 ཟས་བཅུད་འདི་ Norito-serialiable `[u8; 32]` གིས་ གསལ་སྟོན་དང་ ཡང་ན་ བརྒྱུད་འཕྲིན་ནང་ ག་ཅི་བཟུམ་ཅིག་ཐོན་རུང་ དབྱེ་བ་གསརཔ་སྦེ་ གསལ་སྟོན་འབདཝ་ཨིན། གསང་ཨང་: `crates/iroha_data_model/src/crypto/hash.rs`.
  - IVM གིས་ 128-bit SM4 ལྡེ་མིག་ཚུ་ངོ་བཏོནམ་ཨིནམ་དང་ ཧོསིཊི་སི་ཀཱལ་དང་ གནད་སྡུད་དཔེ་ཚད་སྒྲིག་ཆས་ཚུ་གི་བར་ན་ བརྗེ་སོར་འབད་ཡོདཔ་ཨིན། གསང་གྲངས་: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  འདི་བཟུམ་གྱི་དབྱེ་བ་ཡོད་པའི་ Ed25519/BLS/ML-DSA དང་མཉམ་དུ་སྡོད་དགོཔ་དང་ གནས་སྡུད་-དཔེ་ཚད་ཉོ་སྤྱོད་འབད་མི་ (Torii, SDKs, རིགས་མཚན་ལག་ཆས་) འདི་ `sm` གི་ཁྱད་རྣམ་འདི་ ལྕོགས་ཅན་བཟོ་ཚར་བའི་ཤུལ་ལས་ འཐོབ་ཚུགས།

གལ་ཆེ་བའི་རང་གཤིས་: `Identifiable`, `Registered`/`Registrable` (བཟོ་བསྐྲུན་པའི་དཔེ་རིས་) `HasMetadata`, Torii. གསང་གྲངས་: `crates/iroha_data_model/src/lib.rs`.

བྱུང་ལས་: ངོ་བོ་ག་ར་གིས་ རིགས་འགྱུར་གྱི་བྱུང་རིམ་ཚུ་ བཏོན་ཡོདཔ་ཨིན། གསང་གྲངས་: `crates/iroha_data_model/src/events/`.

---

## ཚད་བཟུང་ཚུ་ (རིམ་སྒྲིག་རིམ་སྒྲིག་)།
- བཟའ་ཚང་: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, Norito, `SmartContractParameters { fuel, memory, execution_depth }`, དང་ `custom: BTreeMap`.
- ཁྱབ་སྤེལ་གྱི་དོན་ལུ་ གྱངས་ཁ་རྐྱངམ་གཅིག: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. བསྡོམས་རྩིས་: `Parameters`. གསང་གྲངས་: `crates/iroha_data_model/src/parameter/system.rs`.

སྒྲིག་སྟངས་ཚད་བཟུང་ཚུ་ (ISI): `SetParameter(Parameter)` གིས་ མཐུན་སྒྲིག་ཡོད་པའི་ས་སྒོ་འདི་དུས་མཐུན་བཟོ་སྟེ་ `ConfigurationEvent::Changed` གིས་ བཏོནམ་ཨིན། གསང་གྲངས་: `crates/iroha_data_model/src/isi/transparent.rs`, བཀོལ་སྤྱོད་པ་ `crates/iroha_core/src/smartcontracts/isi/world.rs` ནང་།

---

## བསླབ་སྟོན་རིམ་པ་དང་ཐོ་འགོད།
- གཞི་རྩ།: `Instruction: Send + Sync + 'static` `dyn_encode()`, `as_any()`, བརྟན་ཏོག་ཏོ་ `id()` (ངེས་མེད་ཀྱི་དབྱེ་བ་མིང་ལུ་རྒྱབ་འགལ་འབདཝ་ཨིན།)།
- `InstructionBox`: `Box<dyn Instruction>` wrapper. རིགས་མཚུངས་/Eq/Ord `(type_id, encoded_bytes)` གུ་ལག་ལེན་འཐབ་ཨིན། དེ་འབདཝ་ལས་ འདྲ་མཉམ་འདི་ གནས་གོང་དང་འཁྲིལ་ཏེ་ཨིན།
- Norito གི་དོན་ལུ་ `InstructionBox` `(String wire_id, Vec<u8> payload)` ལུ་ རིམ་སྒྲིག་འབདཝ་ཨིན་ (གློག་ཐག་ཨའི་ཌི་མེད་པ་ཅིན་ Norito ལུ་ལོག་འགྱོཝ་ཨིན།) རིམ་འབྱུང་བཟོ་མི་འདི་གིས་ བཟོ་བསྐྲུན་པ་ཚུ་ལུ་ འཛམ་གླིང་ཡོངས་ཁྱབ་ཀྱི་ `InstructionRegistry` སབ་ཁྲ་བཟོ་ནི་གི་ངོས་འཛིན་ཚུ་ལག་ལེན་འཐབ་ཨིན། སྔོན་སྒྲིག་ཐོ་བཀོད་ནང་ ISI ནང་བཟོ་བསྐྲུན་ཆ་མཉམ་ཚུདཔ་ཨིན། གསང་ཨང་: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: དབྱེ་བ། ཡིག་བརྡ། འཛོལ་བ།
ལག་ལེན་འཐབ་ནི་འདི་ `Execute for <Instruction>` བརྒྱུད་དེ་ `iroha_core::smartcontracts::isi` ནང་ལུ་ལག་ལེན་འཐབ་ཨིན། འོག་ལུ་ མི་མང་གི་ནུས་པ་ཚུ་དང་ སྔོན་སྒྲིག་གནས་སྟངས་ཚུ་ བཏོན་མི་བྱུང་ལས་དང་འཛོལ་བ་ཚུ་ ཐོ་བཀོད་འབདཝ་ཨིན།

### ཐོ་འགོད་ / ཐོ་འགོད་མེད་པ།
དབྱེ་བ་ཚུ་: `Register<T: Registered>` དང་ `Unregister<T: Identifiable>`, བསྡོམས་རྩིས་དབྱེ་བ་ `RegisterBox`/Iroha གིས་ གསལ་སྟོན་དམིགས་གཏད་ཚུ་ཁྱབ་སྟེ་ཡོདཔ་ཨིན།

- ཐོ་བཀོད་པི་ཡར་: འཛམ་གླིང་མཉམ་རོགས་ཚུ་ནང་བཙུགས་ནི།
  - སྔོན་སྒྲིག་ཚུ་: ཧེ་མ་ལས་གནས་མི་ཆོག།
  - བྱུང་ལས་: `PeerEvent::Added`.
  - འཛོལ་བ།: `Repetition(Register, PeerId)` གལ་ཏེ་འདྲ་བཤུས་; `FindError` བལྟ་སྟངས་ཚུ། གསང་ཨང་: `core/.../isi/world.rs`.

- ཐོ་བཀོད་མངའ་ཁོངས་: `NewDomain` ལས་ `owned_by = authority` དང་བཅས་བཟོ་བསྐྲུན་འབདཝ་ཨིན། བཀག་ཆ་: `genesis` མངའ་ཁོངས།
  - སྔོན་འགྲོའི་ཆ་རྐྱེན་: མངའ་ཁོངས་མེད་པའི་གནས་སྟངས།; not `genesis`.
  - བྱུང་ལས་: `DomainEvent::Created`.
  - འཛོལ་བ་: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. གསང་ཨང་: `core/.../isi/world.rs`.- ཐོ་བཀོད་རྩིས་ཐོ་: `NewAccount` ལས་ བཟོ་བསྐྲུན་འབདཝ་ཨིན། `genesis` རྩིས་ཁྲ་ཐོ་འགོད་མི་ཐུབ།
  - སྔོན་སྒྲིག་ཚུ་: མངའ་ཁོངས་འདི་དགོཔ་ཨིན། རྩིས་ཁྲའི་མེད་པ།; རིགས་མཚན་གྱི་མངའ་ཁོངས་ནང་ལུ་མེན།
  - བྱུང་ལས་: `DomainEvent::Account(AccountEvent::Created)`.
  - འཛོལ་བ་: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. གསང་གྲངས་: `core/.../isi/domain.rs`.

- ཐོ་བཀོད་ཨེ་ཨེསི་ཊི་ ངེས་འཛིན་: བཟོ་བསྐྲུན་པ་ལས་ བཟོ་བསྐྲུན་འབདཝ་ཨིན། sets `owned_by = authority`.
  - སྔོན་འགྲོའི་ཆ་རྐྱེན་: ངེས་ཚིག་མེད་པའི་གནས་སྟངས། མངའ་ཁོངས་ཡོད།
  - བྱུང་ལས་: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - འཛོལ་བ།: `Repetition(Register, AssetDefinitionId)`. གསང་ཨང་: `core/.../isi/domain.rs`.

- ཐོ་བཀོད་ཨེན་ཨེཕ་ཊི་: བཟོ་བསྐྲུན་པ་ལས་བཟོ་བསྐྲུན་འབདཝ་ཨིན། sets `owned_by = authority`.
  - སྔོན་འགྲོའི་ཆ་རྐྱེན་: NFT མིན་པའི་ ཡོད། མངའ་ཁོངས་ཡོད།
  - བྱུང་ལས་: `DomainEvent::Nft(NftEvent::Created)`.
  - འཛོལ་བ།: `Repetition(Register, NftId)`. གསང་ཨང་: `core/.../isi/nft.rs`.

- ཐོ་བཀོད་འགན་ཁུར་: `NewRole { inner, grant_to }` (ཇོ་བདག་དང་པོ་རྩིས་ཁྲའི་སབ་ཁྲ་བརྒྱུད་དེ་ཐོ་བཀོད་འབད་ཡོདཔ་) ལས་བཟོ་བསྐྲུན་འབདཝ་ཨིན།
  - སྔོན་འགྲོའི་ཆ་རྐྱེན།: འགན་ཁུར་མེད་པའི་འགན་ཁུར་མེད།
  - བྱུང་ལས་: `RoleEvent::Created`.
  - འཛོལ་བ།: `Repetition(Register, RoleId)`. གསང་ཨང་: `core/.../isi/world.rs`.

- ཐོ་བཀོད་ ཊི་རི་གར་: ཚགས་མ་རིགས་ཀྱི་སྦེ་ གཞི་སྒྲིག་འབད་ཡོད་པའི་ འོས་འབབ་ཅན་གྱི་ འབྱུང་ཁུངས་ནང་ ཊི་ཊི་འདི་ གསོག་འཇོག་འབདཝ་ཨིན།
  - སྔོན་སྒྲིག་ཚུ་: ཚགས་མ་འདི་ མཚན་རྟགས་མ་བཀོད་པ་ཅིན་ `action.repeats` `Exactly(1)` (དེ་ལས་ `MathError::Overflow`) འོང་དགོཔ་ཨིན། ངོ་རྟགས་འདྲ་བཤུས་བཀག་ཆ་འབད་ཡོདཔ།
  - བྱུང་ལས་: `TriggerEvent::Created(TriggerId)`.
  - འཛོལ་བ་ཚུ་: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` བསྒྱུར་བཅོས་/བདེན་དཔྱད་འཐུས་ཤོར་ཚུ་ལུ། གསང་ཨང་: `core/.../isi/triggers/mod.rs`.

- ཐོ་བཀོད་མ་འབད་བའི་ པི་ཡར་/མངའ་ཁོངས་/རྩིས་ཐོ་/ཨེ་སི་ཊི་ངེས་འཛིན་/ཨེན་ཨེཕ་ཊི་/རོ་ལི་/ཊི་རི་གཱར་: དམིགས་གཏད་འདི་རྩ་བསྐྲད་གཏངམ་ཨིན། བཏོན་གཏང་ནིའི་བྱུང་ལས་ཚུ་བཏོན་གཏངམ་ཨིན། ཁ་སྐོང་ ཀ་སི་ཀ་ཌིང་བཏོན་གཏང་ནི།
  - ཐོ་བཀོད་མ་འབད་བའི་མངའ་ཁོངས་: མངའ་ཁོངས་དང་ཁོང་རའི་འགན་ཁུར་ ཊི་ཨེགསི་རིམ་སྒྲིག་རྩིས་ཐོ་ རྩིས་ཐོ་ཁ་ཡིག་དང་ ཡུ་ཨའི་ཨའི་ཌི་བཱའིན་ཌིང་ཚུ་ནང་ རྩིས་ཐོ་ཚུ་ཆ་མཉམ་རྩ་བསྐྲད་གཏངམ་ཨིན། བཏོན་གཏང་མི་འདི་གིས་ ཁོང་རའི་རྒྱུ་དངོས་ཚུ་ (དང་རྒྱུ་དངོས་རེ་ལུ་མེ་ཊ་ཌེ་ཊ་); མངའ་ཁོངས་ནང་ རྒྱུ་དངོས་ངེས་ཚིག་ཚུ་ཆ་མཉམ་རྩ་བསྐྲད་གཏངམ་ཨིན། བཏོན་གཏང་ཡོད་པའི་རྩིས་ཐོ་ཚུ་གིས་ བདག་དབང་འབད་མི་ མངའ་ཁོངས་ནང་ ཨེན་ཨེཕ་ཊི་ཚུ་ བཏོན་གཏང་། དབང་ཚད་མངའ་ཁོངས་དང་མཐུན་པའི་ གློག་ཐག་ཚུ་རྩ་བསྐྲད་གཏངམ་ཨིན། བྱུང་ལས་ཚུ་: `DomainEvent::Deleted`, རྣམ་གྲངས་རེ་རེ་བཏོན་གཏང་ནིའི་བྱུང་ལས་ཚུ་དང་། འཛོལ་བ།: `FindError::Domain` བརླག་སོང་ན། གསང་ཨང་: `core/.../isi/world.rs`.
  - ཐོ་བཀོད་མ་འབད་བའི་རྩིས་ཐོ་: རྩིས་ཁྲའི་གནང་བ་དང་ འགན་ཁུར་ ཊི་ཨེགསི་རིམ་པ་ གྱངས་ཁ་ རྩིས་ཁྲའི་སབ་ཁྲ་བཟོ་ནི་ དེ་ལས་ ཡུ་ཨེ་ཨའི་ཌི་བཱའིན་ཌིང་ཚུ་ རྩ་བསྐྲད་གཏངམ་ཨིན། རྩིས་ཐོ་གིས་བདག་དབང་འབད་མི་རྒྱུ་དངོས་ཚུ་བཏོན་གཏང་ (དང་རྒྱུ་དངོས་རེ་ལུ་མེ་ཊ་ཌེ་ཊ་); རྩིས་ཐོ་གིས་བདག་དབང་འབད་མི་ ཨེན་ཨེཕ་ཊི་ཚུ་བཏོན་གཏང་། རྩིས་ཐོ་དེ་ ག་གི་དབང་ཚད་ཨིན་མི་ འབྱུང་ཁུངས་ཚུ་རྩ་བསྐྲད་གཏངམ་ཨིན། བྱུང་ལས་ཚུ་: `AccountEvent::Deleted`, དང་ `NftEvent::Deleted` བཏོན་བཏང་ཡོད་པའི་ཨེན་ཨེཕ་ཊི་རེ་ལུ་ `NftEvent::Deleted`. འཛོལ་བ།: `FindError::Account` བརླག་སོང་ན། གསང་ཨང་: `core/.../isi/domain.rs`.
  - ཐོ་བཀོད་མ་འབད་བའི་ Asset Pefient: ངེས་ཚིག་དེ་གི་རྒྱུ་དངོས་ཚུ་ཆ་མཉམ་དང་ ཁོང་གི་རྒྱུ་དངོས་རེ་རེ་གི་མེ་ཊ་ཌེ་ཊ་ བཏོན་གཏང་དོ་ཡོདཔ་ཨིན། བྱུང་ལས་ཚུ་: `AssetDefinitionEvent::Deleted` དང་ `AssetEvent::Deleted` རྒྱུ་དངོས་རེ་ལུ་། འཛོལ་བ།: `FindError::AssetDefinition`. གསང་ཨང་: `core/.../isi/domain.rs`.
  - ཐོ་བཀོད་མེད་པའི་ཨེན་ཨེཕ་ཊི་: ཨེན་ཨེཕ་ཊི་རྩ་བསྐྲད་གཏངམ་ཨིན། བྱུང་ལས་: `NftEvent::Deleted`. འཛོལ་བ།: `FindError::Nft`. གསང་གྲངས་: `core/.../isi/nft.rs`.
  - ཐོ་བཀོད་མེད་པའི་འགན་ཁུར་: རྩིས་ཁྲ་ཆ་མཉམ་ལས་ འགན་ཁུར་འདི་ དང་པ་རང་ ཆ་མེད་གཏངམ་ཨིན། དེ་ལས་ འགན་ཁུར་འདི་རྩ་བསྐྲད་གཏངམ་ཨིན། བྱུང་ལས་: `RoleEvent::Deleted`. འཛོལ་བ།: `FindError::Role`. གསང་གྲངས་: `core/.../isi/world.rs`.
  - ཐོ་བཀོད་མ་འབད་མི་ ཊི་གར་: ཡོད་ཚེ་ འབྱུང་ཁུངས་བཏོན་གཏང་། འདྲ་བཤུས་མེད་པའི་ཐོ་བཀོད་ཐོན་ཤུགས་ `Repetition(Unregister, TriggerId)`. བྱུང་ལས་: `TriggerEvent::Deleted`. གསང་གྲངས་: `core/.../isi/triggers/mod.rs`.

###མའིནཊ / མེ་བཏེན།
དབྱེ་བ་ཚུ་: `Mint<O, D: Identifiable>` དང་ `Burn<O, D: Identifiable>`, `MintBox`/`BurnBox` སྦེ་སྒྲོམ་ནང་བཙུགས་ཡོདཔ་ཨིན།- རྒྱུ་དངོས་ (ཨང་གྲངས་) མིན་ཊི་/བཱརན་: འདྲ་མཉམ་དང་ངེས་ཚིག་གི་ `total_quantity` བདེ་སྒྲིག་འབདཝ་ཨིན།
  - སྔོན་འགྲོའི་ཆ་རྐྱེན་: `Numeric` གནས་གོང་འདི་ `AssetDefinition.spec()` འདི་ གྲུབ་དགོཔ་ཨིན། mint གིས་ `mintable` གིས་འབད་ཡོདཔ།
    - `Infinitely`: རྟག་བུ་རང་ཆོགཔ་ཨིན།
    - `Once`: གཅིག་ཏག་ཏག་གཅིག་འབད་བཅུག་ཡོདཔ། མིན་ཊི་ཕིལཔ་དང་པ་ `mintable` ལས་ `Not` ལུ་ `AssetDefinitionEvent::MintabilityChanged` དང་ རྩིས་ཞིབ་འབད་ནི་གི་དོན་ལུ་ ཁ་གསལ་ `AssetDefinitionEvent::MintabilityChanged` བཟོཝ་ཨིན།
    - `Limited(n)`: ཁ་སྐོང་མིང་བཏགས་བཀོལ་སྤྱོད་འབད་བཅུགཔ་ཨིན། མཐར་འཁྱོལ་ཅན་གྱི་mint རེ་རེ་གིས་ གྱངས་ཁ་འདི་མར་ཕབ་འབདཝ་ཨིན། འདི་གིས་ ཀླད་ཀོར་ལུ་ལྷོད་པའི་སྐབས་ ངེས་ཚིག་འདི་ `Not` ལུ་ལྷོདཔ་ད་ གོང་འཁོད་བཟུམ་སྦེ་ `MintabilityChanged` བྱུང་ལས་ཚུ་ བཏོནམ་ཨིན།
    - `Not`: འཛོལ་བ་ `MintabilityError::MintUnmintable`.
  - མངའ་སྡེ་བསྒྱུར་བཅོས་: མིནཊི་གུ་མེད་པ་ཅིན་ རྒྱུ་དངོས་གསར་བསྐྲུན་འབདཝ་ཨིན། ག་དེམ་ཅིག་སྦེ་ ལྷག་ལུས་འདི་ ཀླད་ཀོར་ལུ་འགྱུརཝ་ཨིན་པ་ཅིན་ རྒྱུ་དངོས་ཐོ་བཀོད་རྩ་བསྐྲད་གཏངམ་ཨིན།
  - བྱུང་ལས་ཚུ་: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (Iroha ཡང་ན་ `Limited(n)` གིས་ དེ་གི་འབད་ཆོག་པའི་འཐུས་ཚུ་ བཏོནམ་ཨིན།)
  - འཛོལ་བ་: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. གསང་གྲངས་: `core/.../isi/asset.rs`.

- ཊི་རི་གར་བསྐྱར་ལོག་ཚུ་ མིནཊི་/བཱརན་: བསྒྱུར་བཅོས་ `action.repeats` གིས་ ཊི་རི་ཊི་གི་དོན་ལུ་ གྱངས་ཁ་བརྐྱབ་ཨིན།
  - སྔོན་སྒྲིག་ཚུ་: མིན་ཊི་གུ་ ཚགས་མ་འདི་ མིན་ཊི་འབད་དགོ། ཨང་རྩིས་རིག་པ་འདི་ འཕྱུར་/འོག་མ་/ཕོལོ་མ་དགོ།
  - བྱུང་ལས་: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - འཛོལ་བ་ཚུ་: ནུས་མེད་མིན་ཊི་གུ་ `MathError::Overflow`; `FindError::Trigger` མེད་ན། གསང་གྲངས་: `core/.../isi/triggers/mod.rs`.

### གནས་སོར
དབྱེ་བ་: `Transfer<S: Identifiable, O, D: Identifiable>`, སྒྲོམ་ནང་ `TransferBox`.

- རྒྱུ་དངོས་ (ཨང་གྲངས་): འབྱུང་ཁུངས་ `AssetId` ལས་ཕབ་སྟེ་ འགྲོ་ཡུལ་ `AssetId` (ངེས་ཚིག་གཅིག་པ་, རྩིས་ཐོ་སོ་སོ་) ལུ་ཁ་སྐོང་འབད། ཀླད་ཀོར་གྱི་འབྱུང་ཁུངས་རྒྱུ་དངོས་བཏོན་གཏང་།
  - སྔོན་སྒྲིག་ཚུ་: འབྱུང་ཁུངས་རྒྱུ་དངོས་ཡོདཔ་ཨིན། གནས་གོང་ཚུ་ `spec`.
  - བྱུང་ལས་: `AssetEvent::Removed` (འབྱུང་ཁུངས་), `AssetEvent::Added` (གནས་ཚུལ།)
  - འཛོལ་བ།: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. གསང་གྲངས་: `core/.../isi/asset.rs`.

- ཌོ་མེན་བདག་དབང་: འགྲོ་ཡུལ་རྩིས་ཐོ་ལུ་ `Domain.owned_by` བསྒྱུར་བཅོས་ཚུ།
  - སྔོན་སྒྲིག་ཚུ་: རྩིས་ཐོ་གཉིས་ཆ་ར་ཡོདཔ་ཨིན། མངའ་ཁོངས་ཡོད།
  - བྱུང་ལས་: `DomainEvent::OwnerChanged`.
  - འཛོལ་བ།: `FindError::Account/Domain`. གསང་གྲངས་: `core/.../isi/domain.rs`.

- Aset Definition བདག་དབང་: འགྲོ་ཡུལ་རྩིས་ཐོ་ལུ་ བསྒྱུར་བཅོས་ `AssetDefinition.owned_by` བསྒྱུར་བཅོས་འབདཝ་ཨིན།
  - སྔོན་སྒྲིག་ཚུ་: རྩིས་ཐོ་གཉིས་ཆ་ར་ཡོདཔ་ཨིན། ངེས་ཚིག་ཡོདཔ་ཨིན། འབྱུང་ཁུངས་འདི་གིས་ ད་ལྟོ་འདི་ བདག་དབང་འབད་དགོཔ་ཨིན།
  - བྱུང་ལས་: `AssetDefinitionEvent::OwnerChanged`.
  - འཛོལ་བ།: `FindError::Account/AssetDefinition`. གསང་གྲངས་: `core/.../isi/account.rs`.

- NFT བདག་དབང་: འགྲོ་ཡུལ་རྩིས་ཐོ་ལུ་ བསྒྱུར་བཅོས་ `Nft.owned_by` ཚུ།
  - སྔོན་སྒྲིག་ཚུ་: རྩིས་ཐོ་གཉིས་ཆ་ར་ཡོདཔ་ཨིན། NFT ཡོད། འབྱུང་ཁུངས་འདི་གིས་ ད་ལྟོ་འདི་ བདག་དབང་འབད་དགོཔ་ཨིན།
  - བྱུང་ལས་: `NftEvent::OwnerChanged`.
  - འཛོལ་བ་ཚུ་: `FindError::Account/Nft`, `InvariantViolation` གིས་ འབྱུང་ཁུངས་ཨེན་ཨེཕ་ཊི་འདི་ བདག་དབང་མེད་པ་ཅིན་. གསང་གྲངས་: `core/.../isi/nft.rs`.

### མེཊ་ཌ་: གཞི་སྒྲིག་/རྩ་བསྐྲད་གཏང་ ལྡེ་མིག་བཏོན་གཏང་།
དབྱེ་བ་: `SetKeyValue<T>` དང་ `RemoveKeyValue<T>` དང་ `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. སྒྲོམ་ནང་བཙུགས་ཡོད་པའི་ གྱངས་ཁ་ཚུ།

- གཞི་སྒྲིག་: བཙུགས་མི་ཡང་ན་ ཚབ་བཙུགསཔ་ཨིན།
- བཏོན་གཏང་: ལྡེ་མིག་འདི་རྩ་བསྐྲད་གཏངམ་ཨིན། མེད་ན་འཛོལ་བ།
- བྱུང་ལས་ཚུ་: `<Target>Event::MetadataInserted` / `MetadataRemoved` གནས་གོང་རྙིངམ་/གསརཔ་ཚུ་དང་གཅིག་ཁར་ཨིན།
- འཛོལ་བ།: དམིགས་གཏད་འདི་མེད་པ་ཅིན་ `FindError::<Target>`; `FindError::MetadataKey` བཏོན་གཏང་ནིའི་དོན་ལུ་ ལྡེ་མིག་མེད་མི་ལུ་། གསང་གྲངས་: `crates/iroha_data_model/src/isi/transparent.rs` དང་ བཀོལ་སྤྱོད་པ་དམིགས་གཏད་རེ་ལུ་ འཇལཝ་ཨིན།### གནང་བ་དང་འགན་ཁུར་: གྲོགས་རམ་ / བཀོ་ནི།
དབྱེ་བ་ཚུ་: `Grant<O, D>` དང་ `Revoke<O, D>`, `Permission`/`Role`, དང་ `Account`, དང་ `Permission` ལས་/from `Role`.

- རྩིས་ཐོ་ལུ་ གྲོགས་རམ་གནང་བ་: ཧེ་མ་ལས་ རང་བཞིན་མེད་ཚུན་ཚོད་ `Permission` ཁ་སྐོང་འབདཝ་ཨིན། བྱུང་ལས་: `AccountEvent::PermissionAdded`. འཛོལ་བ།: `Repetition(Grant, Permission)` འདྲ་བཤུས་ན། གསང་གྲངས་: `core/.../isi/account.rs`.
- རྩིས་ཐོ་ལས་གནང་བ་བཀག་ཆ་འབད་ནི་: ཡོད་ཚེ་བཏོན་གཏང་། བྱུང་ལས་: `AccountEvent::PermissionRemoved`. འཛོལ་བ་: `FindError::Permission` མེད་ན། གསང་གྲངས་: `core/.../isi/account.rs`.
- རྩིས་ཐོ་ལུ་ གྲོགས་རམ་གྱི་འགན་ཁུར་: མེད་པ་ཅིན་ `(account, role)` སབ་ཁྲ་བཟོ་ནི་འདི་ མེད་པ་ཅིན་ བཙུགསཔ་ཨིན། བྱུང་ལས་: `AccountEvent::RoleGranted`. འཛོལ་བ།: `Repetition(Grant, RoleId)`. གསང་གྲངས་: `core/.../isi/account.rs`.
- རྩིས་ཐོ་ནང་ལས་ རི་བོཀ་འགན་ཁུར་: ཡོད་ཚེ་ སབ་ཁྲ་བཟོ་ནི་འདི་བཏོན་གཏང་། བྱུང་ལས་: `AccountEvent::RoleRevoked`. འཛོལ་བ་: `FindError::Role` མེད་ན། གསང་གྲངས་: `core/.../isi/account.rs`.
- འགན་ཁུར་ལུ་ གྲོགས་རམ་གནང་བ་: གནང་བ་ཁ་སྐོང་འབད་དེ་ འགན་ཁུར་འདི་ ལོག་བཟོ་བསྐྲུན་འབདཝ་ཨིན། བྱུང་ལས་: `RoleEvent::PermissionAdded`. འཛོལ་བ།: `Repetition(Grant, Permission)`. གསང་གྲངས་: `core/.../isi/world.rs`.
- འགན་ཁུར་ལས་ ཆ་འཇོག་འབད་ནི་: གནང་བ་མེད་པར་ འགན་ཁུར་འདི་ ལོག་བཟོ་བསྐྲུན་འབདཝ་ཨིན། བྱུང་ལས་: `RoleEvent::PermissionRemoved`. འཛོལ་བ།: `FindError::Permission` མེད་ན། གསང་གྲངས་: `core/.../isi/world.rs`.

### ཊི་རི་གར་: ལག་ལེན།
དབྱེ་བ་: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- སྤྱོད་ལམ།: ཊི་གར་ཡན་ལག་ལམ་ལུགས་ཀྱི་དོན་ལུ་ `ExecuteTriggerEvent { trigger_id, authority, args }` ཅིག་བཙུགས་ཡོདཔ་ཨིན། ལག་ཐོག་ལས་ ལག་ལེན་འཐབ་ནི་འདི་ བརྒྱུད་འཕྲིན་ཚུ་ (`ExecuteTrigger` ཚགས་མ་) གི་དོན་ལུ་རྐྱངམ་ཅིག་ འབད་ཆོགཔ་ཨིན། ཚགས་མ་འདི་མཐུན་དགོཔ་ཨིནམ་དང་ འབོད་བརྡ་འདི་ ཊི་གར་བྱ་བ་གི་དབང་ཚད་ཅིག་ཨིན་ ཡང་ན་ དབང་འཛིན་དེ་གི་དོན་ལུ་ `CanExecuteTrigger` འདི་འཛིན་དགོ། ལག་ལེན་པ་གིས་བྱིན་མི་བཀོལ་སྤྱོད་པ་འདི་ ཤུགས་ལྡན་སྦེ་ཡོད་པའི་སྐབས་ འབྱུང་ཁུངས་ལག་ལེན་འཐབ་མི་འདི་གིས་ རན་ཊའིམ་ལག་ལེན་འཐབ་མི་འདི་གིས་ བདེན་དཔྱད་འབད་དེ་ ཚོང་འབྲེལ་གྱི་བཀོལ་སྤྱོད་པ་ ས་སྣུམ་འཆར་དངུལ་ (གཞི་རྟེན་ `executor.fuel` དང་ གདམ་ཁ་མེ་ཊ་ཌེ་ཊ་ `additional_fuel`) འདི་ བཀོལ་སྤྱོད་འབདཝ་ཨིན།
- འཛོལ་བ་ཚུ་: ཐོ་བཀོད་མ་འབད་བ་ཅིན་ `FindError::Trigger`; `InvariantViolation` གལ་ཏེ་‑མིན་པའི་དབང་གིས་འབོད་པ་ཅིན། གསང་གྲངས་: `core/.../isi/triggers/mod.rs` (དང་ `core/.../smartcontracts/isi/mod.rs` ནང་བརྟག་དཔྱད་ཚུ་)།

### ཡར་འཕེལ་དང་ལོག་པ།
- Torii:: གིས་ ལག་ལེན་འཐབ་མི་འདི་ `Executor` བཱའིཊི་ཀོཌི་ལག་ལེན་འཐབ་ཐོག་ལས་ གནས་སྤོ་འབདཝ་ཨིན། འཛོལ་བ་ཚུ་: གནས་སྤོ་འགྱོ་མི་འཐུས་ཤོར་གྱི་ཐོག་ལུ་ `InvalidParameterError::SmartContract` སྦེ་བཤུབ་ཡོདཔ། གསང་གྲངས་: `core/.../isi/world.rs`.
- `Log { level, msg }`: བྱིན་ཡོད་པའི་གནས་རིམ་དང་གཅིག་ཁར་ མཐུད་མཚམས་དྲན་དེབ་ཅིག་བཏོན་གཏངམ་ཨིན། གནས་སྟངས་བསྒྱུར་བཅོས་མེདཔ། གསང་གྲངས་: `core/.../isi/world.rs`.

### འཛོལ་བའི་དཔེ་ཚད།
སྤྱིར་བཏང་ཡིག་ཤུབས་: བརྟག་ཞིབ་ཀྱི་འཛོལ་བ་དང་ འདྲི་དཔྱད་འཐུས་ཤོར་ ངོ་བོ་འཚོལ་མ་ཐོབ། བསྐྱར་ཟློས་འབད་མི་ ཨང་རྩིས་ ཨང་རྩིས་ཆ་མེད་དང་ འགྱུར་ལྡོག་མེད་པའི་འགལ་འཛོལ་གྱི་དོན་ལུ་ `InstructionExecutionError` གིས་ དབྱེ་བ་ཡོདཔ་ཨིན། རྩིས་རྐྱབ་ནི་དང་གྲོགས་རམ་འབད་མི་ཚུ་ `crates/iroha_data_model/src/isi/mod.rs` ནང་ལུ་ `pub mod error` འོག་ལུ་ཡོདཔ་ཨིན།

---## འགྲིག་དང་ལག་ལེན་པ།
- `Executable`: ཡང་ན་ `Instructions(ConstVec<InstructionBox>)` ཡང་ན་ `Ivm(IvmBytecode)`; བཱའིཊི་ཀོཌི་གིས་ གཞི་འགྱམ་༦༤ སྦེ་ རིམ་སྒྲིག་འབདཝ་ཨིན། གསང་གྲངས་: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: བཟོ་བཀོད་དང་བརྡ་མཚོན་ཚུ་ ཐུམ་སྒྲིལ་འབདཝ་ཨིན། དེ་ལས་ མེ་ཊ་ཌེ་ཊ་, Norito, `authority`, `creation_time_ms`, གདམ་ཁའི་ Torii, དང་ དེ་ལས་ དེ་མེན་པ་ `nonce`. གསང་གྲངས་: `crates/iroha_data_model/src/transaction/`.
- རན་ཊའི་ནང་ `iroha_core` `InstructionBox` `Execute for InstructionBox` བརྒྱུད་དེ་ བརྡ་རྟགས་ཚུ་ འོས་འབབ་དང་ལྡནམ་སྦེ་ ལག་ལེན་འཐབ་ཨིན། གསང་གྲངས་: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- རཱན་ཊའིམ་ བཀོལ་སྤྱོད་པ་ བདེན་དཔྱད་འཆར་དངུལ་ (ལག་ལེན་པ་གིས་བྱིན་མི་ བཀོལ་སྤྱོད་པ་): གཞི་རྟེན་ `ImplicitReceive` གདམ་ཁ་ཅན་གྱི་ཚོང་འབྲེལ་མེ་ཊ་ཌེ་ཊ་ `additional_fuel` (`u64`) གིས་ ཚོང་འབྲེལ་ནང་འཁོད་ལུ་ བཀོད་རྒྱ་ཚུ་ བརྒྱུད་དེ་ བརྗེ་སོར་འབད་ཡོདཔ་ཨིན།

---

## འགྱུར་མེད་དང་དྲན་ཐོ་ (བརྟག་དཔྱད་དང་སྲུང་སྐྱོབཔ་ལས་)
- རིགས་མཚན་ཉེན་སྲུང་: `genesis` མངའ་ཁོངས་ཐོ་བཀོད་འབད་མི་ཚུགས། `genesis` རྩིས་ཁྲ་ཐོ་འགོད་མི་ཐུབ། གསང་གྲངས་/བརྟག་དཔྱད་: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- ཨང་གྲངས་ཀྱི་རྒྱུ་དངོས་ཚུ་གིས་ ཁོང་རའི་ `NumericSpec` འདི་ མིན་ཊི་/བརྗེ་སྒྱུར་/བརྐོ་སྟེ་ བསྒྲུབ་དགོཔ་ཨིན། spe མཐུན་པའི་ཐོན་འབབ་ `TypeError::AssetNumericSpec`.
- Mitabilit: `Once` གིས་ mint གཅིག་བྱིན་ཞིནམ་ལས་ དེ་ལས་ `Not` ལུ་ ཕིལཔ་ཚུ་བྱིན་བཅུགཔ་ཨིན། `Limited(n)` གིས་ `Not` ལུ་མ་ལྷོད་པའི་ཧེ་མ་ `n` གི་ མིན་ཊི་ཚུ་ བྱིནམ་ཨིན། `Infinitely` ལུ་ བཀག་ཆ་འབད་ནི་གི་དཔའ་བཅམ་མི་འདི་གིས་ `MintabilityError::ForbidMintOnMintable` དང་ `Limited(0)` ཐོན་འབྲས་ཚུ་ རིམ་སྒྲིག་འབད་ནི།
- མེ་ཊ་ཌེ་ཊ་བཀོལ་སྤྱོད་ཚུ་ ལྡེ་མིག་-ནུས་ཅན་ཨིན། ཡོད་པའི་ལྡེ་མིག་ཅིག་བཏོན་གཏང་ནི་འདི་འཛོལ་བ་ཨིན།
- ཊི་རི་གར་ཚགས་མ་ཚུ་ མིན་པའི་ མིན་འོང་། དེ་ལས་ `Register<Trigger>` གིས་ `Exactly(1)` བསྐྱར་ལོག་འབད་ཆོགཔ་ཨིན།
- ཊི་རི་གར་མེ་ཊ་ཌེ་ཊ་ལྡེ་མིག་ `__enabled` (bool) gates བཀོལ་སྤྱོད་; ལྕོགས་ཅན་བཟོ་ཡོད་མི་ལུ་ བརླག་སྟོར་སྔོན་སྒྲིག་ཚུ་དང་ ལྕོགས་མིན་བཟོ་ཡོད་པའི་ ཊི་རི་ཊི་ཚུ་ གནད་སྡུད་/དུས་ཚོད་/དུས་ཚོད་དང་འཁྲིལ་ཏེ་ འགྲུལ་ལམ་ཚུ་ནང་ མཆོང་ལྡིང་འབདཝ་ཨིན།
- གཏན་འབེབས་བཟོ་ནི: ཨང་རྩིས་རིག་པ་ཆ་མཉམ་གྱིས་ ཞིབ་དཔྱད་འབད་ཡོད་པའི་བཀོལ་སྤྱོད་ཚུ་ལག་ལེན་འཐབ་ཨིན། འོག་/ལྷག་པའི་སླར་ལོག་ཚུ་ཡིག་དཔར་རྐྱབས་ཡོད་པའི་ཨང་རྩིས་འཛོལ་བ་ཚུ། ཀླད་ཀོར་གྱིས་ མར་ཕབ་རྒྱུ་དངོས་ཐོ་བཀོད་ཚུ་ (སྦ་བཞག་པའི་གནས་སྟངས་མེདཔ་ཨིན།)

---## སྦྱོང་ལཱ་གི་དཔེ།
- ས་གཏེར་དང་ སྤོ་བཤུད།
  - `Mint::asset_numeric(10, asset_id)` → spec/mitability གིས་ གནང་བ་བྱིན་པ་ཅིན་ ༡༠ ཁ་སྐོང་འབདཝ་ཨིན། བྱུང་རིམ་: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → འགུལ་སྐྱོད་ ༥; བཏོན་གཏང་ནི་/ཁ་སྐོང་འབད་ནིའི་དོན་ལུ་ བྱུང་ལས་ཚུ།
- མེ་ཊ་ཌེ་ཊ་ དུས་མཐུན་ཚུ།
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → ཡར་འཕར་; བཏོན་གཏང་ཐོག་ལས་ `RemoveKeyValue::account(...)`.
- འགན་ཁུར་/གནང་བ་འཛིན་སྐྱོང་།
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)`, དང་ཁོང་ཚོའི་`Revoke`, བཅས་ཡོད།
- ཊི་རི་གར་གྱི་མི་ཚེ་འཁོར་ཆོས།
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` གིས་ ཚགས་མ་གིས་ བརྡ་སྟོན་མི་ བཅུད་ལྡན་ཞིབ་དཔྱད་དང་གཅིག་ཁར་; `ExecuteTrigger::new(id).with_args(&args)` རིམ་སྒྲིག་འབད་ཡོད་པའི་དབང་ཚད་དང་མཐུན་དགོ།
  - མེ་ཊ་ཌེ་ཊ་ལྡེ་མིག་ `__enabled` འདི་ `false` ལུ་གཞི་སྒྲིག་འབད་དེ་ ཊི་རི་གཱར་ཚུ་ ལྕོགས་མིན་བཟོ་ཚུགས། `SetKeyValue::trigger` ཡང་ན་ IVM `set_trigger_enabled` syscall བརྒྱུད་དེ་ བསྡུ་སྒྲིག་འབད།
  - ཊི་རི་གར་གསོག་འཇོག་འདི་ མངོན་གསལ་གུ་ཉམས་བཅོས་འབདཝ་ཨིན་: འདྲ་མཚུངས་ཨའི་ཌི་དང་ མ་མཐུན་པའི་ཨའི་ཌི་ དེ་ལས་ བཱའིཊི་ཀོཌི་མེད་མི་ཚུ་ གཞི་བསྟུན་འབད་ནི་ འབྱུང་བཅུགཔ་ཨིན། བཱའིཊི་ཀོཌི་གཞི་བསྟུན་གྲངས་ཚུ་ ལོག་རྩིས་སྟོན་འབདཝ་ཨིན།
  - གལ་སྲིད་ ཊི་གར་གྱི་ IVM bytecode འདི་ ལག་ལེན་འཐབ་པའི་དུས་ཚོད་ལུ་ མ་ཐོབ་པ་ཅིན་ ཊི་གར་འདི་བཏོན་བཏང་ཞིནམ་ལས་ ལག་ལེན་འཐབ་ནི་འདི་ འཐུས་ཤོར་གྱི་གྲུབ་འབྲས་དང་གཅིག་ཁར་ མེན་མི་ཅིག་སྦེ་ བརྩི་འཇོག་འབདཝ་ཨིན།
  - མར་ཕབ་འབད་ཡོད་པའི་ ཊི་གར་ཚུ་ དེ་འཕྲོ་ལས་ བཏོན་གཏང་ཡོདཔ་ཨིན། ལག་ལེན་འཐབ་པའི་སྐབས་ མར་ཕབ་ཀྱི་ཐོ་བཀོད་ཅིག་ འཐོན་པ་ཅིན་ གཤག་བཅོས་འབད་དེ་ བརླག་སྟོར་ཞུགས་ཡོདཔ་སྦེ་ བརྩི་འཇོག་འབདཝ་ཨིན།
- ཚད་བཟུང་དུས་མཐུན་བཟོ་ནི།
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` གིས་ དུས་མཐུན་བཟོ་སྟེ་ བཏོནམ་ཨིན།

---

## བགྲོད་ཐུབ་པ་(འདམ་ཁ་ཅན་གྱི་འབྱུང་ཁུངས)།
 - གནས་སྡུད་དཔེ་ཚད་ལྟེ་བ་: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ངེས་ཚིག་དང་ཐོ་བཀོད་: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ཨའི་ཨེསི་ཨའི་ བཀོལ་སྤྱོད་: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - བྱུང་ལས་: `crates/iroha_data_model/src/events/**`.
 - ཚོང་འབྲེལ་: `crates/iroha_data_model/src/transaction/**`.

ཁྱོད་ཀྱིས་ ཁྱད་རྣམ་འདི་ བརྡ་སྟོན་འབད་ཡོད་པའི་ API/སྤྱོད་ལམ་ཐིག་ཁྲམ་ ཡང་ན་ བརྡ་རྟགས་ཅན་གྱི་བྱུང་རིམ་/འཛོལ་བ་ག་ར་ལུ་ འབྲེལ་མཐུད་འབད་ཡོད་པའི་ ཕར་ཚུར་འགྱོ་དགོ་པ་ཅིན་ མིང་ཚིག་འདི་ ང་གིས་ རྒྱ་སྐྱེད་འབད་འོང་།