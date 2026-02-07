---
lang: am
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:22:38.873410+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2 የውሂብ ሞዴል እና አይኤስአይ - ትግበራ-የተገኘ ዝርዝር

ይህ ዝርዝር የንድፍ ግምገማን ለማገዝ በ`iroha_data_model` እና `iroha_core` ላይ ካለው ትግበራ አንፃር የተቀየረ ነው። የኋሊት መሄጃ መንገዶች ወደ ስልጣን ኮድ ያመለክታሉ።

## ወሰን
- ቀኖናዊ አካላትን (ጎራዎች፣ መለያዎች፣ ንብረቶች፣ ኤንኤፍቲዎች፣ ሚናዎች፣ ፈቃዶች፣ እኩዮች፣ ቀስቅሴዎች) እና መለያዎቻቸውን ይገልጻል።
- ሁኔታን የሚቀይሩ መመሪያዎችን (አይኤስአይ) ይገልጻል፡ ዓይነቶች፣ መለኪያዎች፣ ቅድመ ሁኔታዎች፣ የግዛት ሽግግሮች፣ የተለቀቁ ክስተቶች እና የስህተት ሁኔታዎች።
- የመለኪያ አስተዳደርን፣ ግብይቶችን እና የማስተማር ተከታታይነትን ያጠቃልላል።

ቆራጥነት፡ ሁሉም የማስተማሪያ ትርጉሞች የሃርድዌር-ጥገኛ ባህሪ የሌላቸው የንፁህ ሁኔታ ሽግግሮች ናቸው። ተከታታይነት Norito ይጠቀማል; ቪኤም ባይትኮድ IVM ይጠቀማል እና በሰንሰለት ላይ ከመፈጸሙ በፊት የተረጋገጠ አስተናጋጅ-ጎን ነው።

---

## አካላት እና መለያዎች
መታወቂያዎች `Display`/`FromStr` የማዞሪያ ጉዞ ያላቸው የተረጋጋ የህብረቁምፊ ቅጾች አሏቸው። የስም ደንቦች ነጭ ቦታን እና የተያዙት `@ # $` ቁምፊዎችን ይከለክላሉ።- `Name` - የተረጋገጠ የጽሑፍ መለያ። ደንቦች: `crates/iroha_data_model/src/name.rs`.
- `DomainId` - `name`. ጎራ፡ `{ id, logo, metadata, owned_by }` ግንበኞች: `NewDomain`. ኮድ: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — ቀኖናዊ አድራሻዎች በ `AccountAddress` (IH58 / `sora…` compressed / hex) እና Torii በ `AccountAddress::parse_any` በኩል ግብዓቶችን መደበኛ ያደርጋል። IH58 ተመራጭ መለያ ቅርጸት ነው; የ `sora…` ቅጽ ለሶራ-ብቻ UX ሁለተኛ-ምርጥ ነው። የሚታወቀው `alias@domain` ሕብረቁምፊ እንደ ማዞሪያ ተለዋጭ ስም ብቻ ነው የሚቆየው። መለያ: `{ id, metadata }`. ኮድ: `crates/iroha_data_model/src/account.rs`.
- የመለያ መግቢያ ፖሊሲ - ጎራዎች Norito-JSON `AccountAdmissionPolicy` በሜታዳታ ቁልፍ `iroha:account_admission_policy` በማከማቸት ስውር መለያ መፍጠርን ይቆጣጠራሉ። ቁልፉ በማይኖርበት ጊዜ የሰንሰለት ደረጃ ብጁ መለኪያ `iroha:default_account_admission_policy` ነባሪውን ያቀርባል; ያ ደግሞ በማይኖርበት ጊዜ፣ ጠንካራው ነባሪ `ImplicitReceive` (የመጀመሪያው ልቀት) ነው። መመሪያው `mode` (`ExplicitOnly` ወይም `ImplicitReceive`) ሲደመር አማራጭ በየግብይት (ነባሪ `16`) እና በየብሎክ መፍጠር ኮፍያዎችን፣ አማራጭ Norito መለያ (burn) `min_initial_amounts` በንብረት ፍቺ፣ እና አማራጭ `default_role_on_create` (ከ`AccountCreated` በኋላ የተሰጠ፣ ከጠፋ `DefaultRoleError` ውድቅ ያደርጋል)። ዘፍጥረት መርጦ መግባት አይችልም; የተሰናከሉ/የተሳሳቱ ፖሊሲዎች ከ`InstructionExecutionError::AccountAdmission` ጋር ላልታወቁ መለያዎች የመቀበያ አይነት መመሪያዎችን አይቀበሉም። ስውር መለያዎች ሜታዳታ `iroha:created_via="implicit"` ከ`AccountCreated` በፊት; ነባሪ ሚናዎች ተከታይን `AccountRoleGranted` ያመነጫሉ፣ እና የፈጻሚው ባለቤት-መሰረታዊ ደንቦች አዲሱ መለያ ያለ ተጨማሪ ሚናዎች የራሱን ንብረቶች/ኤንኤፍቲዎች እንዲያጠፋ ያስችለዋል። ኮድ: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` - `asset#domain`. ፍቺ፡ `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. ኮድ: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId` — `asset#domain#account` ወይም `asset##account` ጎራዎች የሚዛመዱ ከሆነ፣ `account` ቀኖናዊው `AccountId` ሕብረቁምፊ (IH58 ተመራጭ) ነው። ንብረት፡ `{ id, value: Numeric }` ኮድ: `crates/iroha_data_model/src/asset/{id.rs,value.rs}`.
- `NftId` - `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. ኮድ: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` - `name`. ሚና፡ `{ id, permissions: BTreeSet<Permission> }` ከገንቢ `NewRole { inner: Role, grant_to }` ጋር። ኮድ: `crates/iroha_data_model/src/role.rs`.
- `Permission` - `{ name: Ident, payload: Json }`. ኮድ: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — የአቻ ማንነት (የህዝብ ቁልፍ) እና አድራሻ። ኮድ: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` - `name`. ቀስቅሴ: `{ id, action }`. እርምጃ: `{ executable, repeats, authority, filter, metadata }`. ኮድ: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` ምልክት የተደረገበት ማስገባት/ማስወገድ። ኮድ: `crates/iroha_data_model/src/metadata.rs`.
- የደንበኝነት ምዝገባ ስርዓተ-ጥለት (የመተግበሪያ ንብርብር): ዕቅዶች `AssetDefinition` ግቤቶች ከ `subscription_plan` ሜታዳታ ጋር; የደንበኝነት ምዝገባዎች `Nft` መዝገቦች ከ `subscription` ሜታዳታ ጋር; የሂሳብ አከፋፈል የሚከናወነው በጊዜ ቀስቅሴዎች የደንበኝነት ምዝገባ NFTsን ነው። `docs/source/subscriptions_api.md` እና `crates/iroha_data_model/src/subscription.rs` ይመልከቱ።
- ** ክሪፕቶግራፊክ ፕሪሚቲቭስ** (ባህሪ `sm`)- `Sm2PublicKey` / `Sm2Signature` ቀኖናዊውን SEC1 ነጥብ + ቋሚ ስፋት `r∥s` ኢንኮዲንግ ለ SM2። ገንቢዎች የጥምዝ አባልነትን እና የመታወቂያ ፍቺን (`DEFAULT_DISTID`)ን ያስገድዳሉ፣ ማረጋገጫ ግን የተበላሹ ወይም ከፍተኛ ደረጃ ስካላርዎችን ውድቅ ያደርጋል። ኮድ: `crates/iroha_crypto/src/sm.rs` እና `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` የጂኤም/ቲ 0004 መፍጨትን እንደ Norito-ተከታታይ `[u8; 32]` አዲስ ዓይነት ሃሽ በማኒፌክቶች ወይም በቴሌሜትሪ በሚታይበት ቦታ ሁሉ ያጋልጣል። ኮድ: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` ባለ 128-ቢት SM4 ቁልፎችን ይወክላል እና በአስተናጋጅ syscals እና በዳታ-ሞዴል ዕቃዎች መካከል ይጋራል። ኮድ: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  እነዚህ ዓይነቶች ከኤድ25519/BLS/ML-DSA ፕሪሚየቶች ጎን ለጎን ተቀምጠዋል እና አንዴ የ`sm` ባህሪ ከነቃ ለውሂብ-ሞዴል ተጠቃሚዎች (Torii፣ ኤስዲኬዎች፣ ዘፍጥረት መሳርያ) ይገኛሉ።

ጠቃሚ ባህሪያት: `Identifiable`, `Registered`/`Registrable` (የገንቢ ንድፍ), `HasMetadata`, `IntoKeyValue`. ኮድ: `crates/iroha_data_model/src/lib.rs`.

ክስተቶች፡ እያንዳንዱ አካል በሚውቴሽን ላይ የሚለቀቁ ክስተቶች አሉት (ፍጠር/ሰርዝ/ባለቤቱ ተለውጧል/ዲበ ውሂብ ተለውጧል፣ ወዘተ)። ኮድ: `crates/iroha_data_model/src/events/`.

---

## መለኪያዎች (ሰንሰለት ውቅር)
- ቤተሰቦች፡ `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`፣ `BlockParameters { max_transactions }`፣ `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`፣ `SmartContractParameters { fuel, memory, execution_depth }`፣ በተጨማሪም `custom: BTreeMap`።
- ነጠላ ዝርዝሮች ለ diffs: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. ሰብሳቢ: `Parameters`. ኮድ: `crates/iroha_data_model/src/parameter/system.rs`.

መለኪያዎችን ማቀናበር (ISI)፡ `SetParameter(Parameter)` ተዛማጅ መስኩን ያዘምናል እና `ConfigurationEvent::Changed` ያወጣል። ኮድ: `crates/iroha_data_model/src/isi/transparent.rs`, በ `crates/iroha_core/src/smartcontracts/isi/world.rs` ውስጥ አስፈፃሚ.

---

## መመሪያ ተከታታይነት እና መዝገብ ቤት
- ዋና ባህሪ: `Instruction: Send + Sync + 'static` ከ `dyn_encode()` ጋር, `as_any()`, የተረጋጋ `id()` (የኮንክሪት ዓይነት ስም ነባሪዎች).
- `InstructionBox`: `Box<dyn Instruction>` መጠቅለያ. Clone/Eq/Ord በ`(type_id, encoded_bytes)` ይሰራሉ ​​ስለዚህ እኩልነት በዋጋ ነው።
- Norito serde ለ `InstructionBox` ተከታታይነት ያለው `(String wire_id, Vec<u8> payload)` (የሽቦ መታወቂያ ከሌለ ወደ `type_name` ይመለሳል)። ማጥፋት ለግንባታ ሰሪዎች አለምአቀፍ `InstructionRegistry` የካርታ መለያዎችን ይጠቀማል። ነባሪ መዝገብ ሁሉንም አብሮገነብ ISI ያካትታል። ኮድ: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI፡ አይነቶች፣ ትርጓሜዎች፣ ስህተቶች
ማስፈጸሚያ በ `Execute for <Instruction>` በ `iroha_core::smartcontracts::isi` በኩል ተተግብሯል። ከዚህ በታች ይፋዊ ተፅእኖዎችን፣ ቅድመ ሁኔታዎችን፣ የተለቀቁ ክስተቶችን እና ስህተቶችን ይዘረዝራል።

### ይመዝገቡ/ይመዝገቡ
ዓይነቶች፡ `Register<T: Registered>` እና `Unregister<T: Identifiable>`፣ ከድምር አይነቶች `RegisterBox`/`UnregisterBox` የኮንክሪት ዒላማዎችን የሚሸፍኑ።

- አቻ ይመዝገቡ: ወደ ዓለም አቻ ስብስብ ውስጥ ያስገባል.
  - ቅድመ ሁኔታዎች፡ አስቀድሞ መኖር የለበትም።
  - ክስተቶች: `PeerEvent::Added`.
  - ስህተቶች: `Repetition(Register, PeerId)` ከተባዛ; `FindError` ፍለጋዎች ላይ። ኮድ: `core/.../isi/world.rs`.

- ጎራ ይመዝገቡ፡ ከ`NewDomain` በ`owned_by = authority` ይገነባል። የተከለከለ፡ `genesis` ጎራ።
  - ቅድመ ሁኔታዎች: ጎራ አለመኖር; `genesis` አይደለም.
  - ክስተቶች: `DomainEvent::Created`.
  - ስህተቶች: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. ኮድ: `core/.../isi/world.rs`.- መለያ ይመዝገቡ: ከ `NewAccount` ይገነባል, በ `genesis` ጎራ ውስጥ የተከለከለ; `genesis` መለያ መመዝገብ አይቻልም።
  - ቅድመ ሁኔታዎች: ጎራ መኖር አለበት; መለያ አለመኖሩ; በዘፍጥረት ውስጥ አይደለም.
  - ክስተቶች: `DomainEvent::Account(AccountEvent::Created)`.
  - ስህተቶች: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. ኮድ: `core/.../isi/domain.rs`.

ንብረትን ይመዝገቡ ትርጉም: ከገንቢ ይገነባል; ስብስቦች `owned_by = authority`.
  - ቅድመ ሁኔታዎች: ፍቺ ያለመኖር; ጎራ አለ።
  - ክስተቶች: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - ስህተቶች: `Repetition(Register, AssetDefinitionId)`. ኮድ: `core/.../isi/domain.rs`.

- NFT ይመዝገቡ: ከገንቢ ይገነባል; ስብስቦች `owned_by = authority`.
  - ቅድመ ሁኔታዎች፡ NFT አለመኖር; ጎራ አለ።
  - ክስተቶች: `DomainEvent::Nft(NftEvent::Created)`.
  - ስህተቶች: `Repetition(Register, NftId)`. ኮድ: `core/.../isi/nft.rs`.

ሚናን ይመዝገቡ፡ ከ`NewRole { inner, grant_to }` ይገነባል (የመጀመሪያው ባለቤት በአካውንት ሚና ካርታ የተመዘገበ)፣ `inner: Role` ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ሚና ያለመኖር።
  - ክስተቶች: `RoleEvent::Created`.
  - ስህተቶች: `Repetition(Register, RoleId)`. ኮድ: `core/.../isi/world.rs`.

- ቀስቅሴን ይመዝገቡ፡ ቀስቅሴውን በተገቢው ቀስቅሴ በማጣሪያ ዓይነት ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ማጣሪያው የማይሰራ ከሆነ፣ `action.repeats` `Exactly(1)` (አለበለዚያ `MathError::Overflow`) መሆን አለበት። የተባዙ መታወቂያዎች የተከለከሉ ናቸው።
  - ክስተቶች: `TriggerEvent::Created(TriggerId)`.
  - ስህተቶች፡ `Repetition(Register, TriggerId)`፣ `InvalidParameterError::SmartContract(..)` በመቀየር/ማረጋገጫ ውድቀቶች ላይ። ኮድ: `core/.../isi/triggers/mod.rs`.

- አቻ/ጎራ/አካውንት/ንብረት ትርጉም/NFT/ ሚና/ቀስቅሴን መመዝገብ ውጣ፡ ኢላማውን ያስወግዳል፤ የስረዛ ክስተቶችን ያስወጣል። ተጨማሪ የማስወገጃ ማስወገጃዎች፡-
  - Domainን አስወግድ፡ ሁሉንም በጎራ ውስጥ ያሉ መለያዎችን፣ ሚናዎቻቸውን፣ ፈቃዶቻቸውን፣ tx-sequence countersን፣ መለያ መለያዎችን እና የUAID ማሰሪያዎችን ያስወግዳል። ንብረታቸውን ይሰርዛል (እና በንብረት ሜታዳታ); በጎራው ውስጥ ያሉትን ሁሉንም የንብረት መግለጫዎች ያስወግዳል; በጎራው ውስጥ ያሉትን NFTs እና በተወገዱ መለያዎች የተያዙ ማናቸውንም NFTs ይሰርዛል፤ የሥልጣናቸው ጎራ የሚዛመድ ቀስቅሴዎችን ያስወግዳል። ክስተቶች፡- `DomainEvent::Deleted`፣ እና በንጥል የተሰረዙ ክስተቶች። ስህተቶች፡ `FindError::Domain` ከጠፋ። ኮድ: `core/.../isi/world.rs`.
  - መለያን አስወግድ፡ የመለያ ፈቃዶችን፣ ሚናዎችን፣ tx-sequence counter፣ የመለያ መለያ ካርታን እና የ UAID ማሰሪያዎችን ያስወግዳል። በመለያው (እና በንብረት ሜታዳታ) የተያዙ ንብረቶችን ይሰርዛል; በመለያው የተያዙ NFT ዎችን ይሰርዛል; ሥልጣናቸው ያ መለያ የሆነውን ቀስቅሴዎችን ያስወግዳል። ክስተቶች፡ `AccountEvent::Deleted`፣ በተጨማሪም `NftEvent::Deleted` በተወገደው NFT። ስህተቶች፡ `FindError::Account` ከጠፋ። ኮድ: `core/.../isi/domain.rs`.
  - AssetDefinitionን ያስውጡ፡ የዚያን ፍቺ ሁሉንም ንብረቶች እና በንብረት ሜታዳታ ይሰርዛል። ክስተቶች፡ `AssetDefinitionEvent::Deleted` እና `AssetEvent::Deleted` በንብረት። ስህተቶች: `FindError::AssetDefinition`. ኮድ: `core/.../isi/domain.rs`.
  - NFT ን ያስወግዱ: NFT ያስወግዳል. ክስተቶች: `NftEvent::Deleted`. ስህተቶች: `FindError::Nft`. ኮድ: `core/.../isi/nft.rs`.
  - ሚናን አለመመዝገብ-መጀመሪያ ሚናውን ከሁሉም መለያዎች ይሽራል; ከዚያም ሚናውን ያስወግዳል. ክስተቶች: `RoleEvent::Deleted`. ስህተቶች: `FindError::Role`. ኮድ: `core/.../isi/world.rs`.
  - ቀስቅሴን ከመመዝገብ ውጣ: ቀስቅሴን ካለ ያስወግዳል; የተባዛ ያልተመዘገቡ `Repetition(Unregister, TriggerId)` ያስገኛሉ። ክስተቶች: `TriggerEvent::Deleted`. ኮድ: `core/.../isi/triggers/mod.rs`.

### ሚንት / ማቃጠል
አይነቶች፡ `Mint<O, D: Identifiable>` እና `Burn<O, D: Identifiable>`፣በቦክስ `MintBox`/`BurnBox`።ንብረት (ቁጥር) ሚንት/ማቃጠል፡ ሚዛኖችን እና የፍቺን `total_quantity` ያስተካክላል።
  - ቅድመ ሁኔታዎች: `Numeric` እሴት `AssetDefinition.spec()` ማሟላት አለበት; ሚንት በ`mintable` የተፈቀደ፡
    - `Infinitely`: ሁልጊዜ ይፈቀዳል.
    - `Once`: በትክክል አንድ ጊዜ ተፈቅዶለታል; የመጀመሪያው mint `mintable` ወደ `Not` ገልብጦ `AssetDefinitionEvent::MintabilityChanged` ያወጣል፣ በተጨማሪም ለኦዲትነት ዝርዝር `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`።
    - `Limited(n)`: `n` ተጨማሪ ከአዝሙድና ክወናዎችን ይፈቅዳል. እያንዳንዱ የተሳካ ሚንት ቆጣሪውን ይቀንሳል; ዜሮ ሲደርስ ትርጉሙ ወደ `Not` ይገለበጣል እና ከላይ እንደተገለጹት የ`MintabilityChanged` ክስተቶችን ያወጣል።
    - `Not`: ስህተት `MintabilityError::MintUnmintable`.
  - ግዛት ለውጦች: ከአዝሙድና ላይ የጎደለ ከሆነ ንብረት ይፈጥራል; በቃጠሎ ላይ ሚዛኑ ዜሮ ከሆነ የንብረት ግቤት ያስወግዳል።
  - ክስተቶች፡ `AssetEvent::Added`/`AssetEvent::Removed`፣ `AssetDefinitionEvent::MintabilityChanged` (`Once` ወይም `Limited(n)` አበል ሲያልቅ)።
  - ስህተቶች: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. ኮድ: `core/.../isi/asset.rs`.

- ቀስቅሴ ድግግሞሾች ከአዝሙድና / የሚነድ: ለውጦች `action.repeats` አንድ ቀስቅሴ ቆጠራ.
  - ቅድመ-ሁኔታዎች-በአዝሙድ ላይ ፣ ማጣሪያው ሊታከም የሚችል መሆን አለበት ። አርቲሜቲክ ከመጠን በላይ መፍሰስ / መፍሰስ የለበትም።
  - ክስተቶች: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - ስህተቶች: `MathError::Overflow` ልክ ባልሆነ ሚንት ላይ; `FindError::Trigger` ከጠፋ። ኮድ: `core/.../isi/triggers/mod.rs`.

### ማስተላለፍ
አይነቶች: `Transfer<S: Identifiable, O, D: Identifiable>`, እንደ `TransferBox` በቦክስ.

ንብረት (ቁጥር)፡ ከምንጩ `AssetId` ቀንስ፣ ወደ መድረሻው `AssetId` (ተመሳሳይ ፍቺ፣ የተለየ መለያ) ይጨምሩ። የዜሮ ምንጭ ንብረትን ሰርዝ።
  - ቅድመ ሁኔታዎች: የምንጭ ንብረት አለ; ዋጋ `spec` ያሟላል።
  - ክስተቶች: `AssetEvent::Removed` (ምንጭ), `AssetEvent::Added` (መድረሻ).
  - ስህተቶች: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. ኮድ: `core/.../isi/asset.rs`.

የጎራ ባለቤትነት፡ `Domain.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; ጎራ አለ።
  - ክስተቶች: `DomainEvent::OwnerChanged`.
  - ስህተቶች: `FindError::Account/Domain`. ኮድ: `core/.../isi/domain.rs`.

AssetDefinition ባለቤትነት፡ `AssetDefinition.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; ፍቺ አለ; ምንጭ በአሁኑ ጊዜ ባለቤት መሆን አለበት.
  - ክስተቶች: `AssetDefinitionEvent::OwnerChanged`.
  - ስህተቶች: `FindError::Account/AssetDefinition`. ኮድ: `core/.../isi/account.rs`.

- NFT ባለቤትነት፡ `Nft.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; NFT አለ; ምንጭ በአሁኑ ጊዜ ባለቤት መሆን አለበት.
  - ክስተቶች: `NftEvent::OwnerChanged`.
  - ስህተቶች፡- `FindError::Account/Nft`፣ `InvariantViolation` ምንጭ የ NFT ባለቤት ካልሆነ። ኮድ: `core/.../isi/nft.rs`.

### ዲበ ውሂብ፡ ቁልፍ-እሴት አዘጋጅ/አስወግድ
ዓይነቶች: `SetKeyValue<T>` እና `RemoveKeyValue<T>` ከ `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` ጋር. የታሸጉ ቁጥሮች ቀርበዋል።

- አዘጋጅ: `Metadata[key] = Json(value)` ያስገባ ወይም ይተካዋል.
- አስወግድ: ቁልፉን ያስወግዳል; ስህተት ከጠፋ.
- ክስተቶች: `<Target>Event::MetadataInserted` / `MetadataRemoved` ከአሮጌ / አዲስ እሴቶች ጋር.
- ስህተቶች: ዒላማው ከሌለ `FindError::<Target>`; `FindError::MetadataKey` ለማስወገድ ቁልፍ በጠፋ። ኮድ: `crates/iroha_data_model/src/isi/transparent.rs` እና ፈጻሚ impls በአንድ ዒላማ.### ፈቃዶች እና ሚናዎች፡ መስጠት/መሻር
ዓይነቶች፡ `Grant<O, D>` እና `Revoke<O, D>`፣በቦክስ ቁጥሮች ለ`Permission`/`Role` እስከ/ከ `Account`፣ እና Norito፣ እና Norito

- ለመለያ ፍቃድ ስጥ፡- `Permission` ይጨምራል። ክስተቶች: `AccountEvent::PermissionAdded`. ስህተቶች፡ `Repetition(Grant, Permission)` ከተባዛ። ኮድ: `core/.../isi/account.rs`.
- ከመለያ ፈቃዱን ይሰርዙ፡ ካለ ያስወግዳል። ክስተቶች: `AccountEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/account.rs`.
- ሚናን ለመለያ ይስጡ፡ ከሌለ `(account, role)` ካርታ ስራን ያስገባል። ክስተቶች: `AccountEvent::RoleGranted`. ስህተቶች: `Repetition(Grant, RoleId)`. ኮድ: `core/.../isi/account.rs`.
- ሚና ከመለያ መሻር፡ ካለ የካርታ ስራን ያስወግዳል። ክስተቶች: `AccountEvent::RoleRevoked`. ስህተቶች፡ `FindError::Role` ከሌለ። ኮድ: `core/.../isi/account.rs`.
የሚና ፈቃድ ስጥ፡- በፍቃድ ታክሎ ሚናን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionAdded`. ስህተቶች: `Repetition(Grant, Permission)`. ኮድ: `core/.../isi/world.rs`.
- የሚናውን ፈቃድ መሻር፡ ያለዚያ ፍቃድ ሚናውን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/world.rs`.

### ቀስቅሴዎች፡ መፈጸም
ዓይነት: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- ባህሪ፡ ለቀስቅሴ ንኡስ ስርዓት `ExecuteTriggerEvent { trigger_id, authority, args }`ን ያሰማል። በእጅ መፈጸም የሚፈቀደው በጥሪ ቀስቅሴዎች ብቻ ነው (`ExecuteTrigger` ማጣሪያ); ማጣሪያው መመሳሰል አለበት እና ደዋዩ ቀስቅሴ እርምጃ ባለስልጣን መሆን አለበት ወይም ለዛ ባለስልጣን `CanExecuteTrigger`ን ያዝ። በተጠቃሚ የቀረበ ፈፃሚ ንቁ ሲሆን የማስፈንጠሪያ አፈፃፀም በሂደት አስፈፃሚው የተረጋገጠ እና የግብይቱን አስፈፃሚ የነዳጅ በጀት ይበላል (ቤዝ `executor.fuel` እና አማራጭ ሜታዳታ `additional_fuel`)።
- ስህተቶች: `FindError::Trigger` ካልተመዘገበ; `InvariantViolation` ባለስልጣን ከተጠራ። ኮድ: `core/.../isi/triggers/mod.rs` (እና ሙከራዎች `core/.../smartcontracts/isi/mod.rs` ውስጥ).

### ያሻሽሉ እና ይመዝገቡ
- `Upgrade { executor }`: በተሰጠው `Executor` bytecode, updates executor እና የውሂብ ሞዴል, `ExecutorEvent::Upgraded` በመጠቀም ፈፃሚውን ያፈልሳል. ስህተቶች፡ በስደት አለመሳካት ላይ እንደ `InvalidParameterError::SmartContract` ተጠቅልሏል። ኮድ: `core/.../isi/world.rs`.
- `Log { level, msg }`: ከተሰጠው ደረጃ ጋር የመስቀለኛ መዝገብ ያወጣል; ምንም የመንግስት ለውጥ የለም. ኮድ: `core/.../isi/world.rs`.

### የስህተት ሞዴል
የጋራ ኤንቨሎፕ፡ `InstructionExecutionError` ከተለዋዋጮች ጋር ለግምገማ ስህተቶች፣ መጠይቅ ውድቀቶች፣ ልወጣዎች፣ ህጋዊ አካል አልተገኘም፣ መደጋገም፣ ምናምንቴነት፣ ሂሳብ፣ ልክ ያልሆነ መለኪያ እና የማይለዋወጥ ጥሰት። ቆጠራዎች እና ረዳቶች በ `crates/iroha_data_model/src/isi/mod.rs` በ `pub mod error` ውስጥ ይገኛሉ።

---## ግብይቶች እና ተፈፃሚዎች
- `Executable`: ወይ `Instructions(ConstVec<InstructionBox>)` ወይም `Ivm(IvmBytecode)`; ባይትኮድ ተከታታይነት ያለው እንደ ቤዝ64 ነው። ኮድ: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`፡ ይገነባል፣ ይጠቁማል እና ፓኬጆችን በሜታዳታ፣ `chain_id`፣ `authority`፣ `creation_time_ms`፣ አማራጭ፣ እና I1832000 `nonce`. ኮድ: `crates/iroha_data_model/src/transaction/`.
- በሂደት ጊዜ፣ `iroha_core` `InstructionBox` ባች በ`Execute for InstructionBox` ያስፈጽማል፣ ወደ ተገቢው `*Box` ወይም ተጨባጭ መመሪያ። ኮድ: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- የአሂድ ጊዜ ፈፃሚ ማረጋገጫ በጀት (በተጠቃሚ የቀረበ ፈጻሚ)፡ ቤዝ `executor.fuel` ከመለኪያዎች እና አማራጭ የግብይት ሜታዳታ `additional_fuel` (`u64`)፣ በግብይቱ ውስጥ ባሉ መመሪያዎች/ቀስቃሽ ማረጋገጫዎች ተጋርቷል።

---

## ተለዋዋጮች እና ማስታወሻዎች (ከሙከራዎች እና ከጠባቂዎች)
- የዘፍጥረት ጥበቃዎች፡ የ`genesis` ጎራ ወይም መለያዎች በ`genesis` ጎራ መመዝገብ አይችሉም። `genesis` መለያ መመዝገብ አይቻልም። ኮድ/ሙከራዎች፡ `core/.../isi/world.rs`፣ `core/.../smartcontracts/isi/mod.rs`።
- የቁጥር ንብረቶች `NumericSpec` በአዝሙድ / በማስተላለፍ / በማቃጠል ላይ ማሟላት አለባቸው; spec አለመመጣጠን `TypeError::AssetNumericSpec` ያስገኛል.
- Mintability: `Once` አንድ ከአዝሙድና ይፈቅዳል እና `Not` ወደ ይገለብጣል; `Limited(n)` ወደ `Not` ከመገልበጡ በፊት በትክክል `n` ሚንት ይፈቅዳል። በ`Infinitely` ላይ መፈጠርን ለመከልከል የሚደረጉ ሙከራዎች `MintabilityError::ForbidMintOnMintable` እና `Limited(0)`ን ማዋቀር `MintabilityError::InvalidMintabilityTokens` ያስገኛል።
- የዲበ ውሂብ ስራዎች ቁልፍ-ትክክለኛ ናቸው; የሌለ ቁልፍን ማስወገድ ስህተት ነው።
- ቀስቃሽ ማጣሪያዎች የማይቻሉ ሊሆኑ ይችላሉ; ከዚያ `Register<Trigger>` ብቻ `Exactly(1)` መድገም ይፈቅዳል።
- ቀስቅሴ ሜታዳታ ቁልፍ `__enabled` (ቦል) በሮች አፈፃፀም; የነቁ ነባሪዎች ይጎድላሉ፣ እና የተሰናከሉ ቀስቅሴዎች በውሂብ/በጊዜ/በጥሪ ዱካዎች ላይ ይዘለላሉ።
- ቆራጥነት: ሁሉም የሂሳብ ስራዎች የተረጋገጡ ስራዎችን ይጠቀማሉ; የተተየቡ የሒሳብ ስህተቶችን ከስር/ትርፍ ይመልሳል; ዜሮ ሚዛኖች የንብረት ግቤቶችን ይጥላሉ (ምንም የተደበቀ ሁኔታ የለም)።

---## ተግባራዊ ምሳሌዎች
- ማዕድን ማውጣት እና ማስተላለፍ;
  - `Mint::asset_numeric(10, asset_id)` → በስፔክ/mintability ከተፈቀደ 10 ይጨምራል; ክስተቶች: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → ይንቀሳቀሳል 5; ለማስወገድ / ለመደመር ክስተቶች.
- የዲበ ውሂብ ዝመናዎች፡-
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → ወደላይ; ማስወገድ በ `RemoveKeyValue::account(...)`.
የሚና/ፈቃድ አስተዳደር፡-
  - `Grant::account_role(role_id, account)`፣ `Grant::role_permission(perm, role)`፣ እና `Revoke` መሰሎቻቸው።
- የህይወት ዑደት ማነሳሳት;
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` በማጣሪያ በተዘዋዋሪ ከማይታይነት ማረጋገጫ ጋር; `ExecuteTrigger::new(id).with_args(&args)` ከተዋቀረ ባለስልጣን ጋር መዛመድ አለበት።
  - የሜታዳታ ቁልፍ `__enabled` ወደ `false` በማቀናበር ቀስቅሴዎችን ማሰናከል ይቻላል (የነቁ ነባሪዎች ይጎድላሉ)። በ `SetKeyValue::trigger` ወይም በ IVM `set_trigger_enabled` syscall ቀይር።
  - ቀስቅሴ ማከማቻ በጭነት ላይ ተስተካክሏል፡ የተባዙ መታወቂያዎች፣ የማይዛመዱ መታወቂያዎች እና የጎደሉት ባይትኮድ ቀስቅሴዎች ወድቀዋል። የባይቴኮድ ማጣቀሻ ቆጠራዎች እንደገና ይሰላሉ።
  - የማስፈንጠሪያው IVM ባይትኮድ በአፈፃፀም ጊዜ ከጠፋ፣ ማስጀመሪያው ይወገዳል እና አፈፃፀሙ ከውድቀት ጋር እንደ ምንም-op ይቆጠራል።
  - የተዳከሙ ቀስቅሴዎች ወዲያውኑ ይወገዳሉ; በአፈፃፀም ወቅት የተሟጠጠ መግቢያ ካጋጠመው ተቆርጦ እንደጠፋ ይቆጠራል.
- የመለኪያ ማሻሻያ;
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` አዘምኗል እና `ConfigurationEvent::Changed` ያወጣል።

---

## የመከታተያ ችሎታ (የተመረጡ ምንጮች)
 - የውሂብ ሞዴል ኮር: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ትርጓሜዎች እና መዝገብ: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - ISI አፈፃፀም: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - ክስተቶች: `crates/iroha_data_model/src/events/**`.
 - ግብይቶች: `crates/iroha_data_model/src/transaction/**`.

ይህ ዝርዝር ወደ ኤፒአይ/የባህሪ ሠንጠረዥ እንዲሰፋ ወይም ከእያንዳንዱ ተጨባጭ ክስተት/ስህተት ጋር የተገናኘ እንዲሆን ከፈለጉ ቃሉን ይናገሩ እና እሰፋዋለሁ።