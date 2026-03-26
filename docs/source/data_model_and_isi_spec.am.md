---
lang: am
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 የውሂብ ሞዴል እና አይኤስአይ - ትግበራ-የተገኘ ዝርዝር መግለጫ

ይህ ዝርዝር የንድፍ ግምገማን ለማገዝ በ`iroha_data_model` እና `iroha_core` ላይ ካለው ትግበራ አንፃር የተቀየረ ነው። የኋሊት መሄጃ መንገዶች ወደ ስልጣን ኮድ ያመለክታሉ።

## ወሰን
- ቀኖናዊ አካላትን (ጎራዎች፣ መለያዎች፣ ንብረቶች፣ ኤንኤፍቲዎች፣ ሚናዎች፣ ፈቃዶች፣ እኩዮች፣ ቀስቅሴዎች) እና መለያዎቻቸውን ይገልጻል።
- ሁኔታን የሚቀይሩ መመሪያዎችን (አይኤስአይ) ይገልጻል፡ ዓይነቶች፣ መለኪያዎች፣ ቅድመ ሁኔታዎች፣ የግዛት ሽግግሮች፣ የተለቀቁ ክስተቶች እና የስህተት ሁኔታዎች።
- የመለኪያ አስተዳደርን፣ ግብይቶችን እና የማስተማር ተከታታይነትን ያጠቃልላል።

ቆራጥነት፡ ሁሉም የማስተማሪያ ትርጉሞች የሃርድዌር-ጥገኛ ባህሪ የሌላቸው የንፁህ ሁኔታ ሽግግሮች ናቸው። ተከታታይነት Norito ይጠቀማል; ቪኤም ባይትኮድ IVM ይጠቀማል እና በሰንሰለት ላይ ከመፈጸሙ በፊት የተረጋገጠ አስተናጋጅ-ጎን ነው።

---

## አካላት እና መለያዎች
መታወቂያዎች `Display`/`FromStr` የማዞሪያ ጉዞ ያላቸው የተረጋጋ የገመድ ቅጾች አሏቸው። የስም ደንቦች ነጭ ቦታን እና የተያዙት `@ # $` ቁምፊዎችን ይከለክላሉ።- `Name` - የተረጋገጠ የጽሑፍ መለያ። ደንቦች: `crates/iroha_data_model/src/name.rs`.
- `DomainId` - `name`. ጎራ፡ `{ id, logo, metadata, owned_by }` ግንበኞች: `NewDomain`. ኮድ: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — ቀኖናዊ አድራሻዎች የሚመረቱት በ`AccountAddress` (I105 / hex) እና Torii በ `AccountAddress::parse_encoded` በኩል ግብአቶችን መደበኛ ያደርገዋል። I105 ተመራጭ መለያ ቅርጸት ነው; የI105 ቅጽ ለሶራ-ብቻ UX ነው። የሚታወቀው `alias` (ውድቅ የሆነ የቆየ ቅርስ) ሕብረቁምፊ እንደ ማዞሪያ ተለዋጭ ስም ብቻ ነው የሚቆየው። መለያ: `{ id, metadata }`. ኮድ: `crates/iroha_data_model/src/account.rs`.- የመለያ መግቢያ ፖሊሲ - ጎራዎች Norito-JSON `AccountAdmissionPolicy` በሜታዳታ ቁልፍ `iroha:account_admission_policy` በማከማቸት ስውር መለያ መፍጠርን ይቆጣጠራሉ። ቁልፉ በማይኖርበት ጊዜ, የሰንሰለት ደረጃ ብጁ መለኪያ `iroha:default_account_admission_policy` ነባሪው ያቀርባል; ያ ደግሞ በማይኖርበት ጊዜ፣ ጠንካራው ነባሪ `ImplicitReceive` (የመጀመሪያው ልቀት) ነው። መመሪያው `mode` (`ExplicitOnly` ወይም `ImplicitReceive`) ሲደመር አማራጭ በየግብይት (ነባሪ `16`) እና በየብሎክ መፍጠር ካፕ፣ አማራጭ SoraFS `min_initial_amounts` በንብረት ትርጉም፣ እና አማራጭ `default_role_on_create` (ከ`AccountCreated` በኋላ የተሰጠ፣ ከጠፋ `DefaultRoleError` ውድቅ ያደርጋል)። ዘፍጥረት መርጦ መግባት አይችልም; የተሰናከሉ/ያልሆኑ ፖሊሲዎች ከ`InstructionExecutionError::AccountAdmission` ጋር ላልታወቁ መለያዎች የመቀበያ አይነት መመሪያዎችን አይቀበሉም። ስውር መለያዎች ሜታዳታ `iroha:created_via="implicit"` ከ`AccountCreated` በፊት; ነባሪ ሚናዎች ተከታይን `AccountRoleGranted` ያወጣሉ፣ እና የፈጻሚው ባለቤት-መሰረታዊ ደንቦች አዲሱ መለያ ያለ ተጨማሪ ሚናዎች የራሱን ንብረቶች/ኤንኤፍቲዎች እንዲያጠፋ ያስችለዋል። ኮድ: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` - ቀኖናዊ `unprefixed Base58 address with versioning and checksum` (UUID-v4 ባይት)። ፍቺ፡ `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. `alias` ቀጥተኛ ቃላት `<name>#<domain>.<dataspace>` ወይም `<name>#<dataspace>` መሆን አለባቸው፣ከ `<name>` የንብረት ትርጉም ስም ጋር እኩል ነው። ኮድ: `crates/iroha_data_model/src/asset/definition.rs`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`, where `status` is `permanent`, `leased_active`, `leased_grace`, or `expired_pending_cleanup`. Alias selectors resolve against the latest committed block creation time and stop resolving after grace even before sweep removes stale bindings.
- `AssetId`: ቀኖናዊ ኮድ በጥሬው `<asset-definition-id>#<i105-account-id>` (የቆዩ የጽሑፍ ቅጾች በመጀመሪያው መለቀቅ አይደገፉም)።- `NftId` - `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. ኮድ: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` - `name`. ሚና፡ `{ id, permissions: BTreeSet<Permission> }` ከገንቢ `NewRole { inner: Role, grant_to }` ጋር። ኮድ: `crates/iroha_data_model/src/role.rs`.
- `Permission` - `{ name: Ident, payload: Json }`. ኮድ: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — የአቻ ማንነት (የህዝብ ቁልፍ) እና አድራሻ። ኮድ: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` - `name`. ቀስቅሴ: `{ id, action }`. እርምጃ: `{ executable, repeats, authority, filter, metadata }`. ኮድ: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` ምልክት የተደረገበት ማስገባት/ማስወገድ። ኮድ: `crates/iroha_data_model/src/metadata.rs`.
- የደንበኝነት ምዝገባ ንድፍ (የመተግበሪያ ንብርብር): ዕቅዶች `AssetDefinition` ግቤቶች ከ `subscription_plan` ሜታዳታ ጋር; የደንበኝነት ምዝገባዎች `Nft` መዝገቦች ከ `subscription` ሜታዳታ ጋር; የሂሳብ አከፋፈል የሚከናወነው በጊዜ ቀስቅሴዎች የደንበኝነት ምዝገባ NFTsን ነው። `docs/source/subscriptions_api.md` እና `crates/iroha_data_model/src/subscription.rs` ይመልከቱ።
- ** ክሪፕቶግራፊክ ፕሪሚቲቭስ** (ባህሪ `sm`)፡
  - `Sm2PublicKey` / `Sm2Signature` ቀኖናዊውን SEC1 ነጥብ + ቋሚ ስፋት `r∥s` ኢንኮዲንግ ለ SM2። ገንቢዎች የጥምዝ አባልነትን እና የመታወቂያ ፍቺን (`DEFAULT_DISTID`)ን ያስገድዳሉ፣ ማረጋገጫ ግን የተበላሹ ወይም ከፍተኛ ደረጃ ስካላርዎችን ውድቅ ያደርጋል። ኮድ: `crates/iroha_crypto/src/sm.rs` እና `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` የጂ ኤም/ቲ 0004 መፍጨትን እንደ Norito-ተከታታይ `[u8; 32]` አዲስ ዓይነት ሃሽ በማኒፌክቶች ወይም በቴሌሜትሪ ውስጥ በሚታይበት ቦታ ሁሉ ያጋልጣል። ኮድ: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` ባለ 128-ቢት SM4 ቁልፎችን ይወክላል እና በአስተናጋጅ syscals እና በዳታ-ሞዴል ዕቃዎች መካከል ይጋራል። ኮድ: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  እነዚህ ዓይነቶች ከነባር የ Ed25519/BLS/ML-DSA ፕሪሚየቶች ጋር ተቀምጠዋል እና አንዴ የ`sm` ባህሪ ከነቃ ለውሂብ-ሞዴል ሸማቾች (Torii፣ ኤስዲኬዎች፣ ዘፍጥረት መሳርያ) ይገኛሉ።
- ከዳታስፔስ የመጡ የግንኙነት ማከማቻዎች (`space_directory_manifests`፣ `uaid_dataspaces`፣ `axt_policies`፣ `axt_replay_ledger`፣ የሌይን ማስተላለፊያ የአደጋ ጊዜ መሻር መዝገብ) እና የውሂብ ቦታ-ዒላማ ማከማቻ ፈቃዶች (Norito) ፈቃዶች ናቸው። `State::set_nexus(...)` የውሂብ ቦታዎች ከገባሪው `dataspace_catalog` ሲጠፉ፣ ይህም ከአሂድ ጊዜ ካታሎግ ዝመናዎች በኋላ የቆየ የውሂብ ቦታ ማጣቀሻዎችን ይከላከላል። በሌይን ስፋት ያለው DA/relay caches (`lane_relays`፣ `da_commitments`፣ `da_confidential_compute`፣ `da_pin_intents`) እንዲሁም ሌይን ጡረታ ከወጣ ወይም ወደ ሌላ የውሂብ ቦታ ሲመደብ ይቆረጣል። Space Directory ISIs (`PublishSpaceDirectoryManifest`፣ `RevokeSpaceDirectoryManifest`፣ `ExpireSpaceDirectoryManifest`) እንዲሁም `dataspace`ን ከገባሪው ካታሎግ ጋር ያፀድቃል እና በ`InvalidParameter` ያልታወቁ መታወቂያዎችን ውድቅ ያደርጋል።

ጠቃሚ ባህሪያት: `Identifiable`, `Registered`/`Registrable` (የገንቢ ንድፍ), `HasMetadata`, `IntoKeyValue`. ኮድ: `crates/iroha_data_model/src/lib.rs`.

ክስተቶች፡ እያንዳንዱ አካል በሚውቴሽን ላይ የሚለቀቁ ክስተቶች አሉት (ፍጠር/ሰርዝ/ባለቤቱ ተለውጧል/ዲበ ውሂብ ተለውጧል፣ ወዘተ)። ኮድ: `crates/iroha_data_model/src/events/`.

---## መለኪያዎች (ሰንሰለት ውቅር)
- ቤተሰቦች፡ `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`፣ `BlockParameters { max_transactions }`፣ `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`፣ `SmartContractParameters { fuel, memory, execution_depth }`፣ በተጨማሪም `custom: BTreeMap`።
- ነጠላ ቁጥሮች ለዳይፍ፡ `SumeragiParameter`፣ `BlockParameter`፣ `TransactionParameter`፣ `SmartContractParameter`። ሰብሳቢ: `Parameters`. ኮድ: `crates/iroha_data_model/src/parameter/system.rs`.

መለኪያዎችን ማቀናበር (ISI)፡- `SetParameter(Parameter)` ተዛማጅ መስኩን ያዘምናል እና `ConfigurationEvent::Changed` ያወጣል። ኮድ: `crates/iroha_data_model/src/isi/transparent.rs`, በ `crates/iroha_core/src/smartcontracts/isi/world.rs` ውስጥ አስፈፃሚ.

---

## መመሪያ ተከታታይነት እና መዝገብ ቤት
- ዋና ባህሪ: `Instruction: Send + Sync + 'static` ከ `dyn_encode()`, `as_any()`, የተረጋጋ `id()` (የኮንክሪት ዓይነት ስም ነባሪዎች).
- `InstructionBox`: `Box<dyn Instruction>` መጠቅለያ. Clone/Eq/Ord በ`(type_id, encoded_bytes)` ላይ ይሰራሉ ​​ስለዚህ እኩልነት በዋጋ ነው።
- Norito serde ለ `InstructionBox` ተከታታይነት ያለው `(String wire_id, Vec<u8> payload)` (የሽቦ መታወቂያ ከሌለ ወደ `type_name` ይመለሳል)። ማጥፋት ለግንባታ ሰሪዎች ዓለም አቀፍ `InstructionRegistry` የካርታ መለያዎችን ይጠቀማል። ነባሪ መዝገብ ሁሉንም አብሮገነብ ISI ያካትታል። ኮድ: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI፡ አይነቶች፣ ትርጓሜዎች፣ ስህተቶች
ማስፈጸሚያ በ `Execute for <Instruction>` በ `iroha_core::smartcontracts::isi` ተተግብሯል። ከዚህ በታች ይፋዊ ተፅእኖዎችን፣ ቅድመ ሁኔታዎችን፣ የተለቀቁ ክስተቶችን እና ስህተቶችን ይዘረዝራል።

### ይመዝገቡ/ይመዝገቡ
ዓይነቶች፡- `Register<T: Registered>` እና `Unregister<T: Identifiable>`፣ በድምር ዓይነቶች `RegisterBox`/`UnregisterBox` የኮንክሪት ኢላማዎችን የሚሸፍኑ።- አቻ ይመዝገቡ: ወደ ዓለም አቻ ስብስብ ውስጥ ያስገባል.
  - ቅድመ ሁኔታዎች፡ አስቀድሞ መኖር የለበትም።
  - ክስተቶች: `PeerEvent::Added`.
  - ስህተቶች: `Repetition(Register, PeerId)` ከተባዛ; `FindError` ፍለጋዎች ላይ። ኮድ: `core/.../isi/world.rs`.

- ጎራ ይመዝገቡ፡ ከ`NewDomain` በ`owned_by = authority` ይገነባል። የተከለከለ፡ `genesis` ጎራ።
  - ቅድመ ሁኔታዎች: ጎራ አለመኖር; `genesis` አይደለም.
  - ክስተቶች: `DomainEvent::Created`.
  - ስህተቶች: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. ኮድ: `core/.../isi/world.rs`.

- መለያ ይመዝገቡ: ከ `NewAccount` ይገነባል, በ `genesis` ጎራ ውስጥ የተከለከለ; `genesis` መለያ መመዝገብ አይቻልም።
  - ቅድመ ሁኔታዎች: ጎራ መኖር አለበት; መለያ አለመኖሩ; በዘፍጥረት ውስጥ አይደለም.
  - ክስተቶች: `DomainEvent::Account(AccountEvent::Created)`.
  - ስህተቶች: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. ኮድ: `core/.../isi/domain.rs`.

ንብረትን ይመዝገቡ ትርጉም: ከገንቢ ይገነባል; ስብስቦች `owned_by = authority`.
  - ቅድመ ሁኔታዎች: ፍቺ ያለመኖር; ጎራ አለ; `name` ያስፈልጋል፣ ከተስተካከለ በኋላ ባዶ ያልሆነ መሆን አለበት፣ እና `#`/`@` መያዝ የለበትም።
  - ክስተቶች: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - ስህተቶች: `Repetition(Register, AssetDefinitionId)`. ኮድ: `core/.../isi/domain.rs`.

- NFT ይመዝገቡ: ከገንቢ ይገነባል; ስብስቦች `owned_by = authority`.
  - ቅድመ ሁኔታዎች፡ NFT አለመኖር; ጎራ አለ።
  - ክስተቶች: `DomainEvent::Nft(NftEvent::Created)`.
  - ስህተቶች: `Repetition(Register, NftId)`. ኮድ: `core/.../isi/nft.rs`.ሚናን ይመዝገቡ፡ ከ`NewRole { inner, grant_to }` ይገነባል (በመለያ-ሚና ካርታ ስራ የተመዘገበ የመጀመሪያ ባለቤት)፣ `inner: Role` ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ሚና ያለመኖር።
  - ክስተቶች: `RoleEvent::Created`.
  - ስህተቶች: `Repetition(Register, RoleId)`. ኮድ: `core/.../isi/world.rs`.

- ቀስቅሴን ይመዝገቡ፡ ቀስቅሴውን በተገቢው ቀስቅሴ በማጣሪያ ዓይነት ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ማጣሪያው ሊታሰብ የማይችል ከሆነ፣ `action.repeats` `Exactly(1)` (አለበለዚያ `MathError::Overflow`) መሆን አለበት። የተባዙ መታወቂያዎች የተከለከሉ ናቸው።
  - ክስተቶች: `TriggerEvent::Created(TriggerId)`.
  - ስህተቶች፡- `Repetition(Register, TriggerId)`፣ `InvalidParameterError::SmartContract(..)` በመቀየር/ማረጋገጫ ውድቀቶች ላይ። ኮድ: `core/.../isi/triggers/mod.rs`.- አቻ/ጎራ/አካውንት/ንብረት ትርጉም/NFT/ ሚና/ቀስቅሴን መመዝገብ ውጣ፡ ኢላማውን ያስወግዳል፤ የስረዛ ክስተቶችን ያወጣል። ተጨማሪ የማስወገጃ ማስወገጃዎች፡-- ጎራውን አስወግድ፡ የጎራውን ህጋዊ አካል እና መራጩ/የማፅደቅ-የፖሊሲ ሁኔታን ያስወግዳል። በጎራው ውስጥ ያለውን የንብረት ፍቺ ይሰርዛል (እና ምስጢራዊው `zk_assets` የጎን ሁኔታ በእነዚያ ፍቺዎች የተቆለፈ)፣ የነዚያ ትርጓሜዎች ንብረቶች (እና በንብረት ሜታዳታ)፣ በጎራው ውስጥ ያሉ NFTs፣ እና በጎራ ስፋት ያለው መለያ መለያ-ስያሜ/ተለዋጭ ስም ትንበያ። እንዲሁም የተወገዱ ሂሳቦችን ከተወገደው ጎራ ያቋርጣል እና የተወገደውን ጎራ ወይም ከእሱ ጋር የተሰረዙ ንብረቶችን (የጎራ ፈቃዶች፣ የንብረት-ጥራት/የንብረት ፈቃዶች እና ለተወገዱ NFT መታወቂያዎች) የNFT ፈቃዶችን የሚያመለክቱ የመለያ-/ሚና-ወሰን የፈቃድ ግቤቶችን ያቋርጣል። የጎራ መወገድ አለማቀፉን `AccountId`፣ tx-sequence/UAID ሁኔታ፣የውጭ ንብረቱ ወይም የኤንኤፍቲ ባለቤትነት፣አስጀማሪ ባለስልጣን ወይም ሌላ የተረፈውን መለያ የሚያመለክቱ የውጭ ኦዲት/ውቅር ማጣቀሻዎችን አይሰርዝም። የጥበቃ ሀዲዶች፡ በጎራው ውስጥ ያለ ማንኛውም የንብረት ፍቺ አሁንም በድጋሚ ስምምነት፣ የመቋቋሚያ መመሪያ፣ የህዝብ መስመር ሽልማት/የይገባኛል ጥያቄ፣ ከመስመር ውጭ አበል/ማስተላለፊያ፣ የሰፈራ ሪፖ ነባሪዎች (`settlement.repo.eligible_collateral`፣ `settlement.repo.collateral_substitution_matrix`)፣ በአስተዳደር የተዋቀረ የምርጫ/የዜግነት-የመንግስት ባለቤትነት/የማስተካከያ/የማስተካከያ ስምምነቶች ሲጠቀሱ ውድቅ ያደርጋል። ማጣቀሻዎች፣ ኦራክል-ኢኮኖሚክስ የተዋቀሩ ሽልማቶች/slash/ሙግት-ቦንድ ንብረት-ፍቺ ማጣቀሻዎች፣ ወይም Nexus ክፍያ/የእሴት-ጥራት ማጣቀሻዎች (`nexus.fees.fee_asset_id`፣ `nexus.staking.stake_asset_id`)። ክስተቶች፡ `DomainEvent::Deleted`፣በእያንዳንዱ ንጥል ነገር ሰርዝለተወገዱ በጎራ-ወሰን ሀብቶች በክስተቶች ላይ። ስህተቶች: `FindError::Domain` ከጠፋ; `InvariantViolation` በተያዙ የንብረት-ፍቺ ማጣቀሻ ግጭቶች ላይ። ኮድ: `core/.../isi/world.rs`.- መለያን አስወግድ፡ የመለያ ፈቃዶችን፣ ሚናዎችን፣ tx-sequence counter፣ የመለያ መለያ ካርታን እና የ UAID ማሰሪያዎችን ያስወግዳል። በመለያው (እና በንብረት ሜታዳታ) የተያዙ ንብረቶችን ይሰርዛል; በመለያው የተያዙ ኤንኤፍቲዎችን ይሰርዛል; ስልጣኑ መለያ የሆነውን ቀስቅሴዎችን ያስወግዳል; prunes መለያ-/የሚና-ወሰን የፈቃድ ግቤቶች የተወገደውን መለያ የሚጠቅሱ፣ የመለያ-/ሚና-ወሰን NFT-ዒላማ ፈቃዶች ለተወገዱ በባለቤትነት የያዙ NFT መታወቂያዎች፣ እና ለተወገዱ ቀስቅሴዎች የመለያ-/ሚና-ወሰን የቀስቀስ-ዒላማ ፍቃዶች። የጥበቃ ሀዲዶች፡ መለያው አሁንም ጎራ ካለው ውድቅ ያደርጋል፣ የንብረት ትርጉም፣ የSoraFS አቅራቢ ማስያዣ፣ የነቃ የዜግነት መዝገብ፣ የህዝብ መስመር መሸጫ/ሽልማት ሁኔታ (የሽልማት ጥያቄ ቁልፎችን ጨምሮ መለያው እንደ ጠያቂ ወይም የሽልማት-ንብረት ባለቤት ሆኖ የሚታይበት)፣ ንቁ የኦራክል ግዛት (የቃል ምግብ-ታሪክ አቅራቢ ወይም የትዊተር አቅራቢዎችን ጨምሮ) የተዋቀረ ሽልማት/የማስቀየስ መለያ ማመሳከሪያዎች)፣ ንቁ የNexus ክፍያ/የቁጠባ ሂሳብ ማመሳከሪያዎች (`nexus.fees.fee_sink_account_id`፣ `nexus.staking.stake_escrow_account_id`፣ `nexus.staking.slash_sink_account_id`፣ ቀኖናዊ domainless ገቢር መለያ ለዪዎች ተብሎ የተተነተነ እና ውድቅ የተደረገ የገቢ ማስገኛ መለያዎች) ሁኔታ፣ ገቢር ከመስመር ውጭ አበል/ማስተላለፊያ ወይም ከመስመር ውጭ የፍርድ መሻር ሁኔታ፣ ገባሪ ከመስመር ውጭ ኤስክሮ-መለያ ውቅረት ማጣቀሻዎች ለንቁ ንብረት ፍቺዎች (`settlement.offline.escrow_accounts`)፣ ንቁ የአስተዳደር ሁኔታ (ፕሮፖዛል/ደረጃ ማጽደቅ)als/locks/slash/slashes/የምክር ቤት/የፓርላማ ዝርዝሮች፣ የፕሮፖዛል ፓርላማ ቅጽበተ-ፎቶዎች፣ የአሂድ ጊዜ ማሻሻያ ፕሮፖሰር መዝገቦች፣ በአስተዳደር የተዋቀሩ escrow/slash-receiver/viral-pool መለያ ማጣቀሻዎች፣ አስተዳደር SoraFS ቴሌሜትሪ አስረክብ በNorito ወይም በNorito በአስተዳደር የተዋቀረ የSoraFS አቅራቢ-የባለቤት ማጣቀሻዎች በ`gov.sorafs_provider_owners`)፣ የተዋቀረ ይዘት የፍቃድ ዝርዝር መለያ ማጣቀሻዎችን ያትማል (`content.publish_allow_accounts`)፣ ገቢር የማህበራዊ escrow ላኪ ሁኔታ፣ ንቁ የይዘት-ጥቅል ፈጣሪ ሁኔታ፣ ገቢር የ DA ፒን-ሐሳብ ባለቤት ሁኔታ፣ የገባሪ የስቴት ተደራቢ ሁኔታ SoraFS ፒን-መዝገብ ሰጭ/ማሳያ መዝገቦች (ሚስማር መገለጫዎች፣ የገለጻ ተለዋጭ ስሞች፣ የማባዛት ትዕዛዞች)። ክስተቶች፡ `AccountEvent::Deleted`፣ በተጨማሪም `NftEvent::Deleted` በተወገደው NFT። ስህተቶች: `FindError::Account` ከጠፋ; `InvariantViolation` በባለቤትነት ወላጅ አልባ ልጆች ላይ። ኮድ: `core/.../isi/domain.rs`.የንብረት መግለጫን አስወግድ፡ የዚያን ፍቺ ሁሉንም ንብረቶች እና በንብረት ሜታዳታ ይሰርዛል፣ እና በዛ ፍቺ የተያዘውን ሚስጥራዊ `zk_assets` የጎን ሁኔታ ያስወግዳል። እንዲሁም የተወገደውን የንብረት ፍቺ ወይም የንብረቱን ሁኔታ የሚያጣቅሱ ተዛማጅ `settlement.offline.escrow_accounts` ግቤት እና የመለያ-/ሚና-ወሰን የፈቃድ ግቤቶችን ይቆርጣል። የጥበቃ ሀዲዶች፡ ትርጉሙ አሁንም በድጋሚ ስምምነት ሲጠቀስ ውድቅ ያደርጋል፣ የመቋቋሚያ መመሪያ፣ የህዝብ መስመር ሽልማት/የይገባኛል ጥያቄ፣ ከመስመር ውጭ አበል/ማስተላለፊያ ሁኔታ፣ የሰፈራ ሪፖ ነባሪዎች (`settlement.repo.eligible_collateral`፣ `settlement.repo.collateral_substitution_matrix`)፣ በአስተዳደር የተዋቀረ ድምጽ አሰጣጥ/ዜጋ/ዋርድ-የፓርላማ-መደራጀት ማጣቀሻ ኦራክል-ኢኮኖሚክስ የተዋቀረ ሽልማት/ማጭበርበር/ሙግት-ቦንድ የንብረት-ፍቺ ማጣቀሻዎች፣ ወይም Nexus ክፍያ/የእሴት-ጥራት ማጣቀሻዎች (`nexus.fees.fee_asset_id`፣ `nexus.staking.stake_asset_id`)። ክስተቶች፡ `AssetDefinitionEvent::Deleted` እና `AssetEvent::Deleted` በንብረት። ስህተቶች: `FindError::AssetDefinition`, `InvariantViolation` በማጣቀሻ ግጭቶች ላይ. ኮድ: `core/.../isi/domain.rs`.
  - NFT ን አስወግድ፡ NFT ያስወግዳል እና የተወገደውን NFT የሚያጣቅሱ መለያ-/ሚና-ወሰን ያላቸው የፍቃድ ግቤቶችን ያስወግዳል። ክስተቶች: `NftEvent::Deleted`. ስህተቶች: `FindError::Nft`. ኮድ: `core/.../isi/nft.rs`.
  - ሚናን አለመመዝገብ-መጀመሪያ ሚናውን ከሁሉም መለያዎች ይሽራል; ከዚያም ሚናውን ያስወግዳል. ክስተቶች: `RoleEvent::Deleted`. ስህተቶች: `FindError::Role`. ኮድ: `core/.../isi/world.rs`.ቀስቅሴን አለመመዝገብ፡ ካለ ቀስቅሴን ያስወግዳል እና የተወገደውን ቀስቅሴ የሚጠቅሱ መለያ-/ሚና-ወሰን ያላቸው የፍቃድ ግቤቶችን ያስወግዳል። የተባዛ ያልተመዘገቡ `Repetition(Unregister, TriggerId)`. ክስተቶች: `TriggerEvent::Deleted`. ኮድ: `core/.../isi/triggers/mod.rs`.

### ሚንት / ማቃጠል
ዓይነቶች፡ `Mint<O, D: Identifiable>` እና `Burn<O, D: Identifiable>`፣በቦክስ `MintBox`/`BurnBox`።

ንብረት (ቁጥር) ሚንት/ማቃጠል፡ ሚዛኖችን እና የፍቺን `total_quantity` ያስተካክላል።
  - ቅድመ ሁኔታዎች: `Numeric` እሴት `AssetDefinition.spec()` ማሟላት አለበት; ሚንት በ`mintable` የተፈቀደ፡
    - `Infinitely`: ሁልጊዜ ይፈቀዳል.
    - `Once`: በትክክል አንድ ጊዜ ተፈቅዶለታል; የመጀመሪያው mint `mintable` ወደ `Not` ገልብጦ `AssetDefinitionEvent::MintabilityChanged` ያወጣል፣ በተጨማሪም ለኦዲትነት ዝርዝር `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`።
    - `Limited(n)`: `n` ተጨማሪ ከአዝሙድና ክወናዎችን ይፈቅዳል. እያንዳንዱ የተሳካ ሚንት ቆጣሪውን ይቀንሳል; ዜሮ ሲደርስ ትርጉሙ ወደ `Not` ይገለበጥና ከላይ እንደተገለፀው ተመሳሳይ የ`MintabilityChanged` ክስተቶችን ያወጣል።
    - `Not`: ስህተት `MintabilityError::MintUnmintable`.
  - ግዛት ለውጦች: ከአዝሙድና ላይ የጎደለ ከሆነ ንብረት ይፈጥራል; በቃጠሎ ላይ ሚዛኑ ዜሮ ከሆነ የንብረት ግቤት ያስወግዳል።
  - ክስተቶች፡ `AssetEvent::Added`/`AssetEvent::Removed`፣ `AssetDefinitionEvent::MintabilityChanged` (`Once` ወይም `Limited(n)` አበል ሲያልቅ)።
  - ስህተቶች: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. ኮድ: `core/.../isi/asset.rs`.- ቀስቅሴ ድግግሞሾች ከአዝሙድና / ማቃጠል: ለውጦች `action.repeats` አንድ ቀስቅሴ ቆጠራ.
  - ቅድመ-ሁኔታዎች-በአዝሙድ ላይ ፣ ማጣሪያው ሊታከም የሚችል መሆን አለበት ። አርቲሜቲክ ከመጠን በላይ መፍሰስ / መፍሰስ የለበትም።
  - ክስተቶች: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - ስህተቶች: `MathError::Overflow` ልክ ባልሆነ ሚንት ላይ; `FindError::Trigger` ከጠፋ። ኮድ: `core/.../isi/triggers/mod.rs`.

### ማስተላለፍ
ዓይነቶች: `Transfer<S: Identifiable, O, D: Identifiable>`, በቦክስ `TransferBox`.

ንብረት (ቁጥር)፡ ከምንጩ `AssetId` ቀንስ፣ ወደ መድረሻው `AssetId` (ተመሳሳይ ፍቺ፣ የተለየ መለያ) ይጨምሩ። የዜሮ ምንጭ ንብረትን ሰርዝ።
  - ቅድመ ሁኔታዎች: የምንጭ ንብረት አለ; ዋጋ `spec` ያሟላል።
  - ክስተቶች: `AssetEvent::Removed` (ምንጭ), `AssetEvent::Added` (መድረሻ).
  - ስህተቶች፡ `FindError::Asset`፣ `TypeError::AssetNumericSpec`፣ `MathError::NotEnoughQuantity/Overflow`። ኮድ: `core/.../isi/asset.rs`.

የጎራ ባለቤትነት፡ `Domain.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; ጎራ አለ።
  - ክስተቶች: `DomainEvent::OwnerChanged`.
  - ስህተቶች: `FindError::Account/Domain`. ኮድ: `core/.../isi/domain.rs`.

AssetDefinition ባለቤትነት፡ `AssetDefinition.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; ፍቺ አለ; ምንጭ በአሁኑ ጊዜ ባለቤት መሆን አለበት; ባለስልጣን የምንጭ መለያ፣ የምንጭ-ጎራ ባለቤት ወይም የንብረት-ፍቺ-ጎራ ባለቤት መሆን አለበት።
  - ክስተቶች: `AssetDefinitionEvent::OwnerChanged`.
  - ስህተቶች: `FindError::Account/AssetDefinition`. ኮድ: `core/.../isi/account.rs`.- NFT ባለቤትነት፡ `Nft.owned_by` ወደ መድረሻ መለያ ይለውጣል።
  - ቅድመ ሁኔታዎች: ሁለቱም መለያዎች አሉ; NFT አለ; ምንጭ በአሁኑ ጊዜ ባለቤት መሆን አለበት; ባለስልጣን የምንጭ መለያ፣ የምንጭ-ጎራ ባለቤት፣ የNFT-ጎራ ባለቤት ወይም ለ NFT `CanTransferNft` መያዝ አለበት።
  - ክስተቶች: `NftEvent::OwnerChanged`.
  - ስህተቶች፡- `FindError::Account/Nft`፣ `InvariantViolation` ምንጭ የ NFT ባለቤት ካልሆነ። ኮድ: `core/.../isi/nft.rs`.

### ዲበ ውሂብ፡ ቁልፍ-እሴት አዘጋጅ/አስወግድ
ዓይነቶች: `SetKeyValue<T>` እና `RemoveKeyValue<T>` ከ `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }` ጋር. የታሸጉ ቁጥሮች ቀርበዋል።

- አዘጋጅ: `Metadata[key] = Json(value)` ያስገባ ወይም ይተካዋል.
- አስወግድ: ቁልፉን ያስወግዳል; ስህተት ከጠፋ.
- ክስተቶች: `<Target>Event::MetadataInserted` / `MetadataRemoved` ከአሮጌ / አዲስ እሴቶች ጋር.
- ስህተቶች: ዒላማው ከሌለ `FindError::<Target>`; `FindError::MetadataKey` ለማስወገድ ቁልፍ በጠፋ። ኮድ: `crates/iroha_data_model/src/isi/transparent.rs` እና ፈጻሚ impls በአንድ ዒላማ.

### ፈቃዶች እና ሚናዎች፡ መስጠት/መሻር
ዓይነቶች፡ `Grant<O, D>` እና `Revoke<O, D>`፣በቦክስ ቁጥሮች ለ`Permission`/`Role` እስከ/ከ `Account`፣ እና `gov.sorafs_telemetry.per_provider_submitters`፣ እና Norito- ለመለያ ፍቃድ ስጥ፡- `Permission` ይጨምራል። ክስተቶች: `AccountEvent::PermissionAdded`. ስህተቶች፡ `Repetition(Grant, Permission)` ከተባዛ። ኮድ: `core/.../isi/account.rs`.
- ከመለያ ፈቃዱን ይሰርዙ፡ ካለ ያስወግዳል። ክስተቶች: `AccountEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/account.rs`.
- ሚናን ለመለያ ይስጡ፡ ከሌለ `(account, role)` ካርታ ስራን ያስገባል። ክስተቶች: `AccountEvent::RoleGranted`. ስህተቶች: `Repetition(Grant, RoleId)`. ኮድ: `core/.../isi/account.rs`.
- ሚና ከመለያ መሻር፡ ካለ የካርታ ስራን ያስወግዳል። ክስተቶች: `AccountEvent::RoleRevoked`. ስህተቶች፡ `FindError::Role` ከሌለ። ኮድ: `core/.../isi/account.rs`.
የሚና ፈቃድ ስጥ፡- በፍቃድ ታክሎ ሚናን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionAdded`. ስህተቶች: `Repetition(Grant, Permission)`. ኮድ: `core/.../isi/world.rs`.
- የሚናውን ፈቃድ መሻር፡ ያለዚያ ፍቃድ ሚናውን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/world.rs`.### ቀስቅሴዎች፡ መፈጸም
ዓይነት: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- ባህሪ፡ `ExecuteTriggerEvent { trigger_id, authority, args }` ለመቀስቀስ ንኡስ ስርዓት ያሰማል። በእጅ መፈጸም የሚፈቀደው በጥሪ ቀስቅሴዎች ብቻ ነው (`ExecuteTrigger` ማጣሪያ); ማጣሪያው መመሳሰል አለበት እና ደዋዩ ቀስቅሴ እርምጃ ባለስልጣን መሆን አለበት ወይም ለዛ ባለስልጣን `CanExecuteTrigger` ን ይያዙ። በተጠቃሚ የቀረበ ፈፃሚ ንቁ ሲሆን የማስፈንጠሪያ ማስፈጸሚያ በሂደት አስፈፃሚው የተረጋገጠ እና የግብይቱን አስፈፃሚ የነዳጅ በጀት ይበላል (ቤዝ `executor.fuel` እና አማራጭ ሜታዳታ `additional_fuel`)።
- ስህተቶች: `FindError::Trigger` ካልተመዘገበ; `InvariantViolation` ባለስልጣን ከተጠራ። ኮድ: `core/.../isi/triggers/mod.rs` (እና ሙከራዎች `core/.../smartcontracts/isi/mod.rs` ውስጥ).

### ያሻሽሉ እና ይመዝገቡ
- `Upgrade { executor }`: በተሰጠው `Executor` ባይትኮድ፣አስፈፃሚውን አዘምን እና የመረጃ ሞዴሉን በመጠቀም ፈጻሚውን ያፈልሳል፣`ExecutorEvent::Upgraded` ያወጣል። ስህተቶች፡ በስደት አለመሳካት ላይ እንደ `InvalidParameterError::SmartContract` ተጠቅልሏል። ኮድ: `core/.../isi/world.rs`.
- `Log { level, msg }`: ከተሰጠው ደረጃ ጋር የመስቀለኛ መዝገብ ያወጣል; ምንም የመንግስት ለውጥ የለም. ኮድ: `core/.../isi/world.rs`.

### የስህተት ሞዴል
የጋራ ኤንቨሎፕ፡ `InstructionExecutionError` ከተለዋዋጮች ጋር ለግምገማ ስህተቶች፣ የመጠይቅ ውድቀቶች፣ ልወጣዎች፣ ህጋዊ አካል አልተገኘም፣ መደጋገም፣ ምናምንቴነት፣ ሂሳብ፣ ልክ ያልሆነ መለኪያ እና የማይለዋወጥ ጥሰት። ቆጠራዎች እና ረዳቶች በ `crates/iroha_data_model/src/isi/mod.rs` በ `pub mod error` ውስጥ ይገኛሉ።

---## ግብይቶች እና ተፈፃሚዎች
- `Executable`: ወይ `Instructions(ConstVec<InstructionBox>)` ወይም `Ivm(IvmBytecode)`; ባይትኮድ ተከታታይነት ያለው እንደ ቤዝ64 ነው። ኮድ: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`፡ ይገነባል፣ ምልክቶች እና ፓኬጆች በሜታዳታ፣ `chain_id`፣ `authority`፣ `creation_time_ms`፣ አማራጭ፣ እና I1837000 `nonce`. ኮድ: `crates/iroha_data_model/src/transaction/`.
- በሂደት ጊዜ፣ `iroha_core` `InstructionBox` ባች በ`Execute for InstructionBox` ያስፈጽማል፣ ወደ ተገቢው `*Box` ወይም ተጨባጭ መመሪያ። ኮድ: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- የአሂድ ጊዜ ፈፃሚ ማረጋገጫ በጀት (በተጠቃሚ የቀረበ ፈጻሚ)፡ ቤዝ `executor.fuel` ከመለኪያዎች እና አማራጭ የግብይት ሜታዳታ `additional_fuel` (`u64`)፣ በግብይቱ ውስጥ ባሉ መመሪያዎች/ቀስቃሽ ማረጋገጫዎች ተጋርቷል።

---## ተለዋዋጮች እና ማስታወሻዎች (ከሙከራዎች እና ከጠባቂዎች)
- የዘፍጥረት ጥበቃዎች፡ የ`genesis` ጎራ ወይም መለያዎች በ`genesis` ጎራ መመዝገብ አይችሉም። `genesis` መለያ መመዝገብ አይቻልም። ኮድ/ሙከራዎች፡ `core/.../isi/world.rs`፣ `core/.../smartcontracts/isi/mod.rs`።
- የቁጥር ንብረቶች `NumericSpec` በአዝሙድ / በማስተላለፍ / በማቃጠል ላይ ማሟላት አለባቸው; spec አለመመጣጠን `TypeError::AssetNumericSpec` ያስገኛል.
- Mintability: `Once` አንድ ከአዝሙድና ይፈቅዳል እና `Not` ወደ ይገለብጣል; `Limited(n)` ወደ `Not` ከመገልበጡ በፊት በትክክል `n` ሚንት ይፈቅዳል። በ `Infinitely` ምክንያት `MintabilityError::ForbidMintOnMintable` ላይ መፈጠርን ለመከልከል የተደረጉ ሙከራዎች እና `Limited(0)` ማዋቀር `MintabilityError::InvalidMintabilityTokens` ያስገኛል ።
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
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` በማጣሪያ በተዘዋዋሪ ከሚታሰበው ቼክ ጋር; `ExecuteTrigger::new(id).with_args(&args)` ከተዋቀረ ባለስልጣን ጋር መዛመድ አለበት።
  - የሜታዳታ ቁልፍ `__enabled` ወደ `false` በማቀናበር ቀስቅሴዎችን ማሰናከል ይቻላል (የነቁ ነባሪዎች ይጎድላሉ)። በ `SetKeyValue::trigger` ወይም በ IVM `set_trigger_enabled` syscall ቀይር።
  - ቀስቅሴ ማከማቻ በጭነት ላይ ተስተካክሏል፡ የተባዙ መታወቂያዎች፣ የማይዛመዱ መታወቂያዎች እና የጎደሉት ባይትኮድ ቀስቅሴዎች ወድቀዋል። የባይቴኮድ ማጣቀሻ ቆጠራዎች እንደገና ይሰላሉ።
  - የማስፈንጠሪያው IVM ባይትኮድ በአፈፃፀም ጊዜ ከጠፋ፣ ማስጀመሪያው ይወገዳል እና አፈፃፀሙ ከውድቀት ጋር እንደ ምንም-op ይቆጠራል።
  - የተዳከሙ ቀስቅሴዎች ወዲያውኑ ይወገዳሉ; በአፈፃፀም ወቅት የተሟጠጠ መግቢያ ካጋጠመው ተቆርጦ እንደጠፋ ይቆጠራል.
- የመለኪያ ማሻሻያ;
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` አዘምኗል እና `ConfigurationEvent::Changed` ያወጣል።CLI / Torii asset-definition id + ተለዋጭ ምሳሌዎች፡-
- በቀኖናዊ እርዳታ + ግልጽ ስም + ረጅም ቅጽል ይመዝገቡ:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- በቀኖናዊ እርዳታ + ግልጽ ስም + አጭር ቅጽል ይመዝገቡ:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- ሚንት በቅፅል ስም + መለያ ክፍሎች
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- ቀኖናዊ እርዳታ ቅጽል መፍታት፡-
  - `POST /v1/assets/aliases/resolve` ከJSON `{ "alias": "pkr#ubl.sbp" }` ጋር

የስደት ማስታወሻ፡-
- `name#domain` የጽሑፍ ንብረት-መግለጫ መታወቂያዎች በመጀመሪያ ሲለቀቁ ሆን ተብሎ የማይደገፉ ናቸው።
- በአዝሙድ/በማቃጠል/በማስተላለፊያ ድንበሮች ላይ የንብረት መታወቂያዎች ቀኖናዊ `<asset-definition-id>#<i105-account-id>` ይቆያሉ; `iroha tools encode asset-id` በ `--definition <base58-asset-definition-id>` ወይም `--alias ...` እና `--account` ይጠቀሙ።

---

## የመከታተያ ችሎታ (የተመረጡ ምንጮች)
 - የውሂብ ሞዴል ኮር: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ትርጓሜዎች እና መዝገብ: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - የ ISI አፈፃፀም: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - ክስተቶች: `crates/iroha_data_model/src/events/**`.
 - ግብይቶች: `crates/iroha_data_model/src/transaction/**`.

ይህ ዝርዝር ወደ ኤፒአይ/የባህሪ ሠንጠረዥ እንዲሰፋ ወይም ከእያንዳንዱ ተጨባጭ ክስተት/ስህተት ጋር የተገናኘ እንዲሆን ከፈለጉ ቃሉን ይናገሩ እና አራዝመዋለሁ።