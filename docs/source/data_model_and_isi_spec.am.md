<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8d13f6d206f60d31217ed093a5bbedd7946d27b644f9b3321a577cc6065a901
source_last_modified: "2026-03-30T18:22:55.965549+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Iroha v2 የውሂብ ሞዴል እና አይኤስአይ - ትግበራ-የተገኘ ዝርዝር መግለጫ

ይህ ዝርዝር የንድፍ ግምገማን ለማገዝ በ `iroha_data_model` እና `iroha_core` ላይ ካለው ትግበራ አንፃር የተቀየረ ነው። የኋሊት መሄጃ መንገዶች ወደ ስልጣን ኮድ ያመለክታሉ።

## ወሰን
- ቀኖናዊ አካላትን (ጎራዎች፣ መለያዎች፣ ንብረቶች፣ ኤንኤፍቲዎች፣ ሚናዎች፣ ፈቃዶች፣ እኩዮች፣ ቀስቅሴዎች) እና መለያዎቻቸውን ይገልጻል።
- ሁኔታን የሚቀይሩ መመሪያዎችን (አይኤስአይ) ይገልጻል፡ ዓይነቶች፣ መለኪያዎች፣ ቅድመ ሁኔታዎች፣ የግዛት ሽግግሮች፣ የተለቀቁ ክስተቶች እና የስህተት ሁኔታዎች።
- የመለኪያ አስተዳደርን፣ ግብይቶችን እና የማስተማር ተከታታይነትን ያጠቃልላል።

ቆራጥነት፡ ሁሉም የማስተማሪያ ትርጉሞች የሃርድዌር-ጥገኛ ባህሪ የሌላቸው የንፁህ ሁኔታ ሽግግሮች ናቸው። ተከታታይነት Norito ይጠቀማል; ቪኤም ባይትኮድ IVM ይጠቀማል እና በሰንሰለት ላይ ከመፈጸሙ በፊት የተረጋገጠ አስተናጋጅ-ጎን ነው።

---

## አካላት እና መለያዎች
መታወቂያዎች ከ `Display`/`FromStr` የዙሪያ ጉዞ ጋር የተረጋጋ የገመድ ቅጾች አሏቸው። የስም ደንቦች ነጭ ቦታን እና የተያዙት `@ # $` ቁምፊዎችን ይከለክላሉ።- `Name` - የተረጋገጠ የጽሑፍ መለያ። ደንቦች: `crates/iroha_data_model/src/name.rs`.
- `DomainId` - `name`. ጎራ፡ `{ id, logo, metadata, owned_by }` ግንበኞች: `NewDomain`. ኮድ: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` - ቀኖናዊ አድራሻዎች በ `AccountAddress` በኩል I105 እና Torii ግብዓቶችን በ `AccountAddress::parse_encoded` መደበኛ ያደርጋል። ጥብቅ የአሂድ ጊዜ መተንተን ቀኖናዊ I105ን ብቻ ይቀበላል። በሰንሰለት ሒሳብ ላይ ያሉ ተለዋጭ ስሞች `name@domain.dataspace` ወይም `name@dataspace` ይጠቀማሉ እና ወደ ቀኖናዊ `AccountId` እሴቶች ይፍቱ። በጥብቅ `AccountId` ተንታኞች ተቀባይነት የላቸውም። መለያ: `{ id, metadata }`. ኮድ: `crates/iroha_data_model/src/account.rs`.- የመለያ መግቢያ ፖሊሲ - ጎራዎች Norito-JSON `AccountAdmissionPolicy` በሜታዳታ ቁልፍ `iroha:account_admission_policy` በማከማቸት ስውር መለያ መፍጠርን ይቆጣጠራሉ። ቁልፉ በማይኖርበት ጊዜ, የሰንሰለት ደረጃ ብጁ መለኪያ `iroha:default_account_admission_policy` ነባሪው ያቀርባል; ያ ደግሞ በማይኖርበት ጊዜ፣ ጠንካራው ነባሪ `ImplicitReceive` (የመጀመሪያው ልቀት) ነው። መመሪያው `mode` (`ExplicitOnly` ወይም `ImplicitReceive`) እና አማራጭ በየግብይት (ነባሪ `16`) እና በየብሎክ ፈጠራ ካፕ፣ አማራጭ Norito `min_initial_amounts` በንብረት ትርጉም፣ እና አማራጭ `default_role_on_create` (ከ`AccountCreated` በኋላ የተሰጠ፣ ከጠፋ `DefaultRoleError` ውድቅ ያደርጋል)። ዘፍጥረት መርጦ መግባት አይችልም; የተሰናከሉ/የተሳሳቱ ፖሊሲዎች ከ`InstructionExecutionError::AccountAdmission` ጋር ለማይታወቁ መለያዎች የመቀበያ አይነት መመሪያዎችን አይቀበሉም። ስውር መለያዎች ሜታዳታ `iroha:created_via="implicit"` ከ`AccountCreated` በፊት; ነባሪ ሚናዎች ተከታይን `AccountRoleGranted` ያመነጫሉ፣ እና የፈጻሚው ባለቤት-መሰረታዊ ደንቦች አዲሱ መለያ የራሱን ንብረቶች/ኤንኤፍቲዎች ያለ ተጨማሪ ሚናዎች እንዲያወጣ ያስችለዋል። ኮድ: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.- `AssetDefinitionId` - ቀኖናዊ ቅድመ-ቅጥያ የሌለው Base58 አድራሻ በቀኖናዊ ንብረት-ፍቺ ባይት ላይ። ይህ የህዝብ ንብረት መታወቂያ ነው። ፍቺ፡ `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }` `alias` ቀጥተኛ ቃላት `<name>#<domain>.<dataspace>` ወይም `<name>#<dataspace>` መሆን አለባቸው፣ከ `<name>` የንብረት ትርጉም ስም ጋር እኩል ነው፣እና እነሱ የሚፈቱት በቀኖናዊው Base58 የንብረት መታወቂያ ብቻ ነው። ኮድ: `crates/iroha_data_model/src/asset/definition.rs`.
  - ተለዋጭ ሊዝ ሜታዳታ ከተከማቸ የንብረት-መግለጫ ረድፍ ተለይቶ ይቆያል። Core/Torii ፍቺዎች ሲነበቡ ከማሰሪያው መዝገብ `alias` በቁሳዊነት ተሰራ።
  - የTorii የንብረት-ፍቺ ምላሾች `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }` ያጋልጣሉ፣እዚያም `status` `permanent`፣ `leased_active`፣ `leased_grace`፣040000007NI
  - ተለዋጭ መራጮች ከቅርብ ጊዜ ቁርጠኝነት የፍጥረት ጊዜ ጋር ይቃረናሉ። ከ `grace_until_ms` በኋላ፣ ተለዋጭ ስም መራጮች የበስተጀርባ መጥረግ እስካሁን የቆየውን ማሰሪያ ካላስወገደም መፍታት ያቆማሉ። ቀጥተኛ ፍቺ ማንበብ አሁንም እንደ `expired_pending_cleanup` ሆኖ የቆየውን ማሰሪያ ሪፖርት ሊያደርግ ይችላል።
- `AssetId`፡ የህዝብ ንብረት መለያ በቀኖናዊ ባዶ Base58 ቅጽ። እንደ `name#dataspace` ወይም `name#domain.dataspace` ያሉ የንብረት ተለዋጭ ስሞች ለNorito የውስጥ ደብተር ይዞታዎች በተፈለገ ጊዜ የተከፋፈሉ `asset + account + optional dataspace` መስኮችን ሊያጋልጥ ይችላል፣ነገር ግን ያ የተዋሃደ ቅርጽ የህዝብ `AssetId` አይደለም።
- `NftId` - `nft$domain`. NFT: `{ id, content: Metadata, owned_by }`. ኮድ: `crates/iroha_data_model/src/nft.rs`.- `RoleId` - `name`. ሚና፡ `{ id, permissions: BTreeSet<Permission> }` ከገንቢ `NewRole { inner: Role, grant_to }` ጋር። ኮድ: `crates/iroha_data_model/src/role.rs`.
- `Permission` - `{ name: Ident, payload: Json }`. ኮድ: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — የአቻ ማንነት (የወል ቁልፍ) እና አድራሻ። ኮድ: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` - `name`. ቀስቅሴ: `{ id, action }`. እርምጃ: `{ executable, repeats, authority, filter, metadata }`. ኮድ: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` ከተረጋገጠ ማስገቢያ/ማስወገድ ጋር። ኮድ: `crates/iroha_data_model/src/metadata.rs`.
- የደንበኝነት ምዝገባ ስርዓተ-ጥለት (የመተግበሪያ ንብርብር): ዕቅዶች `AssetDefinition` ግቤቶች ከ `subscription_plan` ሜታዳታ ጋር; የደንበኝነት ምዝገባዎች `Nft` መዝገቦች ከ `subscription` ሜታዳታ ጋር; የሂሳብ አከፋፈል የሚከናወነው በጊዜ ቀስቅሴዎች የደንበኝነት ምዝገባ NFTsን ነው። `docs/source/subscriptions_api.md` እና `crates/iroha_data_model/src/subscription.rs` ይመልከቱ።
- ** ክሪፕቶግራፊክ ፕሪሚቲቭስ** (ባህሪ `sm`)
  - `Sm2PublicKey` / `Sm2Signature` ቀኖናዊውን SEC1 ነጥብ + ቋሚ ስፋት `r∥s` ኢንኮዲንግ ለ SM2። ገንቢዎች የጥምዝ አባልነትን እና የመታወቂያ ፍቺን (`DEFAULT_DISTID`)ን ያስፈጽማሉ፣ ማረጋገጫ ግን የተበላሹ ወይም ከፍተኛ ደረጃ ስካላርዎችን ውድቅ ያደርጋል። ኮድ: `crates/iroha_crypto/src/sm.rs` እና `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` የጂኤም/ቲ 0004 መፍጨትን እንደ Norito-ተከታታይ `[u8; 32]` አዲስ ዓይነት በማኒፌስት ወይም በቴሌሜትሪ ውስጥ በሚታይበት ቦታ ሁሉ ያጋልጣል። ኮድ: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` 128-ቢት SM4 ቁልፎችን ይወክላል እና በአስተናጋጅ syscals እና በዳታ-ሞዴል ዕቃዎች መካከል ይጋራል። ኮድ: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  እነዚህ ዓይነቶች ከኤድ25519/BLS/ML-DSA ፕሪሚየቶች ጎን ለጎን ተቀምጠዋል እና አንዴ የ`sm` ባህሪ ከነቃ ለውሂብ-ሞዴል ሸማቾች (Torii፣ ኤስዲኬዎች፣ ዘፍጥረት መሳርያ) ይገኛሉ።
- ከዳታስፔስ የመጡ የግንኙነት ማከማቻዎች (`space_directory_manifests`፣ `uaid_dataspaces`፣ `axt_policies`፣ `axt_replay_ledger`፣ የሌይን ማስተላለፊያ የአደጋ ጊዜ መሻር መዝገብ) እና የውሂብ ቦታ-ዒላማ ማከማቻ ፈቃዶች (Norito) ናቸው `State::set_nexus(...)` የውሂብ ቦታዎች ከገባሪው `dataspace_catalog` ሲጠፉ ፣ከአሂድ ጊዜ ካታሎግ ዝመናዎች በኋላ የቆየ የውሂብ ቦታ ማጣቀሻዎችን ይከላከላል። በሌይን ስፋት ያለው DA/relay caches (`lane_relays`፣ `da_commitments`፣ `da_confidential_compute`፣ `da_pin_intents`) እንዲሁም ሌይን ጡረታ ከወጣ ወይም ወደ ሌላ የውሂብ ቦታ ሲመደብ ወይም ወደ ሌላ የውሂብ ቦታ ሲመደብ ይቆርጣሉ። Space Directory ISIs (`PublishSpaceDirectoryManifest`፣ `RevokeSpaceDirectoryManifest`፣ `ExpireSpaceDirectoryManifest`) እንዲሁም `dataspace`ን ከገባሪው ካታሎግ ጋር ያፀድቃል እና በ`InvalidParameter` ያልታወቁ መታወቂያዎችን ውድቅ ያደርጋል።

ጠቃሚ ባህሪያት: `Identifiable`, `Registered`/`Registrable` (የገንቢ ንድፍ), `HasMetadata`, `IntoKeyValue`. ኮድ: `crates/iroha_data_model/src/lib.rs`.

ክስተቶች፡ እያንዳንዱ አካል በሚውቴሽን ላይ የሚለቀቁ ክስተቶች አሉት (ፍጠር/ሰርዝ/ባለቤቱ ተለውጧል/ዲበ ውሂብ ተለውጧል፣ ወዘተ)። ኮድ: `crates/iroha_data_model/src/events/`.

---## መለኪያዎች (ሰንሰለት ውቅር)
- ቤተሰቦች፡ `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`፣ `BlockParameters { max_transactions }`፣ `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`፣ `SmartContractParameters { fuel, memory, execution_depth }`፣ በተጨማሪም `custom: BTreeMap`።
- ነጠላ ቁጥሮች ለዳይፍ፡ `SumeragiParameter`፣ `BlockParameter`፣ `TransactionParameter`፣ `SmartContractParameter`። ሰብሳቢ: `Parameters`. ኮድ: `crates/iroha_data_model/src/parameter/system.rs`.

መለኪያዎችን ማቀናበር (ISI)፡ `SetParameter(Parameter)` ተዛማጅ መስኩን ያዘምናል እና `ConfigurationEvent::Changed` ያወጣል። ኮድ: `crates/iroha_data_model/src/isi/transparent.rs`, በ `crates/iroha_core/src/smartcontracts/isi/world.rs` ውስጥ አስፈፃሚ.

---

## መመሪያ ተከታታይነት እና መዝገብ ቤት
- ዋና ባህሪ: `Instruction: Send + Sync + 'static` ከ `dyn_encode()`, `as_any()`, የተረጋጋ `id()` (የኮንክሪት ዓይነት ስም ነባሪዎች).
- `InstructionBox`: `Box<dyn Instruction>` መጠቅለያ. Clone/Eq/Ord በ`(type_id, encoded_bytes)` ይሰራሉ ​​ስለዚህ እኩልነት በዋጋ ነው።
- Norito serde ለ `InstructionBox` ተከታታይ እንደ `(String wire_id, Vec<u8> payload)` (የሽቦ መታወቂያ ከሌለ ወደ `type_name` ይመለሳል)። ማጥፋት ለግንባታ ሰሪዎች ዓለም አቀፍ `InstructionRegistry` የካርታ መለያዎችን ይጠቀማል። ነባሪ መዝገብ ሁሉንም አብሮገነብ ISI ያካትታል። ኮድ: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI፡ አይነቶች፣ ትርጓሜዎች፣ ስህተቶች
ማስፈጸሚያ በ`Execute for <Instruction>` በ `iroha_core::smartcontracts::isi` በኩል ተተግብሯል። ከዚህ በታች ይፋዊ ተፅእኖዎችን፣ ቅድመ ሁኔታዎችን፣ የተለቀቁ ክስተቶችን እና ስህተቶችን ይዘረዝራል።

### ይመዝገቡ/ይመዝገቡ
ዓይነቶች፡ `Register<T: Registered>` እና `Unregister<T: Identifiable>`፣ ከድምር ዓይነቶች `RegisterBox`/`UnregisterBox` የኮንክሪት ኢላማዎችን የሚሸፍኑ።- አቻ ይመዝገቡ: ወደ ዓለም አቻ ስብስብ ውስጥ ያስገባል.
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
  - ቅድመ ሁኔታዎች: ፍቺ ያለመኖር; ጎራ አለ; `name` ያስፈልጋል፣ ከተከረከመ በኋላ ባዶ ያልሆነ መሆን አለበት፣ እና `#`/`@` መያዝ የለበትም።
  - ክስተቶች: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - ስህተቶች: `Repetition(Register, AssetDefinitionId)`. ኮድ: `core/.../isi/domain.rs`.

- NFT ይመዝገቡ: ከገንቢ ይገነባል; ስብስቦች `owned_by = authority`.
  - ቅድመ ሁኔታዎች፡ NFT አለመኖር; ጎራ አለ።
  - ክስተቶች: `DomainEvent::Nft(NftEvent::Created)`.
  - ስህተቶች: `Repetition(Register, NftId)`. ኮድ: `core/.../isi/nft.rs`.ሚናን ይመዝገቡ፡ ከ`NewRole { inner, grant_to }` ይገነባል (የመጀመሪያው ባለቤት በአካውንት ሚና ካርታ የተመዘገበ)፣ `inner: Role` ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ሚና ያለመኖር።
  - ክስተቶች: `RoleEvent::Created`.
  - ስህተቶች: `Repetition(Register, RoleId)`. ኮድ: `core/.../isi/world.rs`.

- ቀስቅሴን ይመዝገቡ፡ ቀስቅሴውን በተገቢው ቀስቅሴ በማጣሪያ ዓይነት ያከማቻል።
  - ቅድመ ሁኔታዎች፡ ማጣሪያው የማይሰራ ከሆነ፣ `action.repeats` `Exactly(1)` (አለበለዚያ `MathError::Overflow`) መሆን አለበት። የተባዙ መታወቂያዎች የተከለከሉ ናቸው።
  - ክስተቶች: `TriggerEvent::Created(TriggerId)`.
  - ስህተቶች፡- `Repetition(Register, TriggerId)`፣ `InvalidParameterError::SmartContract(..)` በመቀየር/ማረጋገጫ ውድቀቶች ላይ። ኮድ: `core/.../isi/triggers/mod.rs`.- አቻ/ጎራ/አካውንት/ንብረት ትርጉም/NFT/ ሚና/ቀስቅሴን መመዝገብ ውጣ፡ ኢላማውን ያስወግዳል፤ የስረዛ ክስተቶችን ያስወጣል። ተጨማሪ የማስወገጃ ማስወገጃዎች፡-- ጎራውን አስወግድ፡ የጎራውን ህጋዊ አካል እና መራጩ/የማፅደቅ-የፖሊሲ ሁኔታን ያስወግዳል። በጎራው ውስጥ ያለውን የንብረት ፍቺ ይሰርዛል (እና ምስጢራዊው `zk_assets` የጎን ሁኔታ በእነዚያ ፍቺዎች የተቆለፈ)፣ የነዚያ ትርጓሜዎች ንብረቶች (እና በንብረት ሜታዳታ)፣ በጎራው ውስጥ ያሉ NFTዎችን፣ እና በተወገደው ጎራ ውስጥ ስር የሰደዱ የመለያ-ቅጽል ትንበያዎች። እንዲሁም የተወገደውን ጎራ ወይም ከእሱ ጋር የተሰረዙ ንብረቶችን (የጎራ ፈቃዶች፣ የተወገዱ ፍቺዎች የንብረት-ፍቺ/ንብረት ፈቃዶች እና ለተወገዱ NFT መታወቂያዎች) የሚጠቅሱ የመለያ-/ሚና-ወሰን የፈቃድ ግቤቶችን ይሰርዛል። የጎራ መወገድ አለማቀፉን `AccountId`፣ tx-sequence/UAID ሁኔታ፣የውጭ ሀብቱ ወይም የNFT ባለቤትነት፣የቀስቃሽ ባለስልጣን ወይም ሌላ የተረፈውን መለያ የሚያመለክቱ የውጭ ኦዲት/ውቅር ማጣቀሻዎችን አይሰርዝም ወይም አይጽፍም። የጥበቃ ሀዲዶች፡ በጎራው ውስጥ ያለ ማንኛውም የንብረት ፍቺ አሁንም በድጋሚ ስምምነት፣ የመቋቋሚያ መመሪያ፣ የህዝብ መስመር ሽልማት/የይገባኛል ጥያቄ፣ ከመስመር ውጭ አበል/ማስተላለፊያ፣ የሰፈራ ሪፖ ነባሪዎች (`settlement.repo.eligible_collateral`፣ `settlement.repo.collateral_substitution_matrix`)፣ በአስተዳደር የተዋቀረ የምርጫ/የዜግነት-የመንግስት ባለቤትነት/የማዋቀር/የማዋቀር ምርጫ/የዜግነት-የማዋቀር/የማስተካከያ/የማስተካከያ/የማስተካከያ ስምምነቶችን ሲያመለክት ውድቅ ያደርጋል። ማጣቀሻዎች፣ ኦራክል-ኢኮኖሚክስ የተዋቀሩ ሽልማቶች/slash/ሙግት-ቦንድ ንብረት-ፍቺ ማጣቀሻዎች፣ ወይም Nexus ክፍያ/የእሴት-ጥራት ማጣቀሻዎች (`nexus.fees.fee_asset_id`፣ `nexus.staking.stake_asset_id`)። ክስተቶች፡ `DomainEvent::Deleted`፣ እና ለተወገዱ የጎራ ሪሶርስ በንጥል ስረዛ ክስተቶችሴሰ ስህተቶች: `FindError::Domain` ከጠፋ; `InvariantViolation` በተያዙ የንብረት-ፍቺ ማጣቀሻ ግጭቶች ላይ። ኮድ: `core/.../isi/world.rs`.- መለያን አስወግድ፡ የመለያ ፈቃዶችን፣ ሚናዎችን፣ tx-sequence counter፣ የመለያ መለያ ካርታን እና የ UAID ማሰሪያዎችን ያስወግዳል። በመለያው (እና በንብረት ሜታዳታ) የተያዙ ንብረቶችን ይሰርዛል; በመለያው የተያዙ ኤንኤፍቲዎችን ይሰርዛል; ስልጣኑ መለያ የሆነውን ቀስቅሴዎችን ያስወግዳል; prunes መለያ-/የሚና-ወሰን የፈቃድ ግቤቶች የተወገደውን መለያ የሚጠቅሱ፣ የመለያ-/ሚና-ወሰን NFT-ዒላማ ፈቃዶች ለተወገዱ በባለቤትነት የያዙ NFT መታወቂያዎች፣ እና ለተወገዱ ቀስቅሴዎች የመለያ-/ሚና-ወሰን የቀስቀስ-ዒላማ ፍቃዶች። የጥበቃ ሀዲዶች፡ መለያው አሁንም ጎራ ካለው ውድቅ ያደርጋል፣ የንብረት ትርጉም፣ የSoraFS አቅራቢ ማስያዣ፣ የነቃ የዜግነት መዝገብ፣ የህዝብ መስመር መሸጫ/ሽልማት ሁኔታ (የሽልማት ጥያቄ ቁልፎችን ጨምሮ መለያው እንደ ጠያቂ ወይም የሽልማት-ንብረት ባለቤት ሆኖ የሚታይበት)፣ ንቁ የኦራክል ግዛት (የቃል ምግብ-ታሪክ አቅራቢ ወይም የትዊተር አቅራቢዎችን ጨምሮ) የተዋቀረ ሽልማት/የማስቀየስ መለያ ማመሳከሪያዎች)፣ ንቁ የNexus ክፍያ/የቁጠባ ሂሳብ ማመሳከሪያዎች (`nexus.fees.fee_sink_account_id`፣ `nexus.staking.stake_escrow_account_id`፣ `nexus.staking.slash_sink_account_id`፣ ቀኖናዊ domainless ንቁ መለያ ለዪዎች ተብሎ የተተነተነ እና ውድቅ የተደረገ በስቴት ዋጋ የተዘጋ ሰነድ) ሁኔታ፣ ገቢር ከመስመር ውጭ አበል/ማስተላለፊያ ወይም ከመስመር ውጭ የፍርድ መሻር ሁኔታ፣ ገባሪ ከመስመር ውጭ ኤስክሮ-መለያ ውቅረት ማጣቀሻዎች ለንቁ ንብረት ፍቺዎች (`settlement.offline.escrow_accounts`)፣ ንቁ የአስተዳደር ሁኔታ (የፕሮፖዛል/የደረጃ ማጽደቅ)als/locks/slashes/የምክር ቤት/የፓርላማ ዝርዝሮች፣ የፕሮፖዛል ፓርላማ ቅጽበተ-ፎቶዎች፣ የአሂድ ጊዜ ማሻሻያ ፕሮፖሰር መዛግብት፣ በአስተዳደር የተዋቀሩ escrow/slash-receiver/viral-pool መለያ ማጣቀሻዎች፣ አስተዳደር SoraFS ቴሌሜትሪ አስረክብ በNorito በአስተዳደር የተዋቀረ የSoraFS አቅራቢ-የባለቤት ማጣቀሻዎች በ`gov.sorafs_provider_owners`)፣ የተዋቀረ ይዘት የፍቃድ ዝርዝር መለያ ማጣቀሻዎችን ያትማል (`content.publish_allow_accounts`)፣ ገቢር የማህበራዊ escrow ላኪ ሁኔታ፣ ንቁ የይዘት-ጥቅል ፈጣሪ ሁኔታ፣ ገቢር የ DA ፒን-ሐሳብ ባለቤት ሁኔታ፣ ገባሪ የግዛት ሽፋን ወይም የአደጋ ጊዜ ገባሪ ሌኔሬሪ SoraFS ፒን-መዝገብ ሰጭ/ማሳያ መዝገቦች (ሚስማር መገለጫዎች፣ የገለጻ ተለዋጭ ስሞች፣ የማባዛት ትዕዛዞች)። ክስተቶች፡ `AccountEvent::Deleted`፣ በተጨማሪም `NftEvent::Deleted` በተወገደው NFT። ስህተቶች: `FindError::Account` ከጠፋ; `InvariantViolation` በባለቤትነት ወላጅ አልባ ህፃናት ላይ። ኮድ: `core/.../isi/domain.rs`.የንብረት መግለጫን አስወግድ፡ የዚያን ፍቺ ሁሉንም ንብረቶች እና በንብረት ሜታዳታ ይሰርዛል፣ እና በዛ ፍቺ የተቆለፈውን ሚስጥራዊ `zk_assets` የጎን ሁኔታ ያስወግዳል። እንዲሁም የተወገደውን የንብረት ፍቺ ወይም የንብረቱን ሁኔታ የሚያጣቅሱ ተዛማጅ `settlement.offline.escrow_accounts` ግቤት እና የመለያ-/ሚና-ወሰን የፈቃድ ግቤቶችን ይቆርጣል። የጥበቃ ሀዲድ፡ ትርጉሙ አሁንም በድጋሚ ስምምነት ሲጣቀስ ውድቅ ያደርጋል፣ የመቋቋሚያ መመሪያ፣ የህዝብ መስመር ሽልማት/የይገባኛል ጥያቄ፣ ከመስመር ውጭ አበል/የማስተላለፊያ ሁኔታ፣ የመቋቋሚያ ሪፖ ነባሪዎች (`settlement.repo.eligible_collateral`፣ `settlement.repo.collateral_substitution_matrix`)፣ በአስተዳደር የተዋቀረ ድምጽ አሰጣጥ/ዜጋ/ዋርድ-የፓርላማ-መደራጀት ማጣቀሻ ኦራክል-ኢኮኖሚክስ የተዋቀረ ሽልማት/ማጭበርበር/ሙግት-ቦንድ የንብረት-ፍቺ ማጣቀሻዎች፣ ወይም Nexus ክፍያ/የእሴት-ጥራት ማጣቀሻዎች (`nexus.fees.fee_asset_id`፣ `nexus.staking.stake_asset_id`)። ክስተቶች፡ `AssetDefinitionEvent::Deleted` እና `AssetEvent::Deleted` በንብረት። ስህተቶች: `FindError::AssetDefinition`, `InvariantViolation` በማጣቀሻ ግጭቶች ላይ. ኮድ: `core/.../isi/domain.rs`.
  - NFT ን አስወግድ፡ NFT ያስወግዳል እና የተወገደውን NFT የሚያጣቅሱ የፈቃድ ግቤቶችን ያስወግዳል። ክስተቶች: `NftEvent::Deleted`. ስህተቶች: `FindError::Nft`. ኮድ: `core/.../isi/nft.rs`.
  - ሚናን አለመመዝገብ-መጀመሪያ ሚናውን ከሁሉም መለያዎች ይሽራል; ከዚያም ሚናውን ያስወግዳል. ክስተቶች: `RoleEvent::Deleted`. ስህተቶች: `FindError::Role`. ኮድ: `core/.../isi/world.rs`.ቀስቅሴን አለመመዝገብ፡ ካለ ቀስቅሴን ያስወግዳል እና የተወገደውን ቀስቅሴ የሚጠቅሱ መለያ-/ሚና-ወሰን ያላቸው የፍቃድ ግቤቶችን ያስወግዳል። የተባዛ ያልተመዘገቡ `Repetition(Unregister, TriggerId)` ያስገኛሉ። ክስተቶች: `TriggerEvent::Deleted`. ኮድ: `core/.../isi/triggers/mod.rs`.

### ሚንት / ማቃጠል
አይነቶች፡ `Mint<O, D: Identifiable>` እና `Burn<O, D: Identifiable>`፣በቦክስ `MintBox`/`BurnBox`።

ንብረት (ቁጥር) ሚንት/ማቃጠል፡ ሚዛኖችን እና የፍቺን `total_quantity` ያስተካክላል።
  - ቅድመ ሁኔታዎች: `Numeric` እሴት `AssetDefinition.spec()` ማሟላት አለበት; ሚንት በ`mintable` የተፈቀደ፡
    - `Infinitely`: ሁልጊዜ ይፈቀዳል.
    - `Once`: በትክክል አንድ ጊዜ ተፈቅዶለታል; የመጀመሪያው mint `mintable` ወደ `Not` ገልብጦ `AssetDefinitionEvent::MintabilityChanged` ያወጣል፣ በተጨማሪም ለኦዲትነት ዝርዝር `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }`።
    - `Limited(n)`: `n` ተጨማሪ ከአዝሙድና ክወናዎችን ይፈቅዳል. እያንዳንዱ የተሳካ ሚንት ቆጣሪውን ይቀንሳል; ዜሮ ሲደርስ ትርጉሙ ወደ `Not` ይገለበጣል እና ከላይ እንደተገለፀው ተመሳሳይ `MintabilityChanged` ክስተቶችን ያወጣል።
    - `Not`: ስህተት `MintabilityError::MintUnmintable`.
  - ግዛት ለውጦች: ከአዝሙድና ላይ የጎደለ ከሆነ ንብረት ይፈጥራል; በቃጠሎ ላይ ሚዛኑ ዜሮ ከሆነ የንብረት ግቤት ያስወግዳል።
  - ክስተቶች፡ `AssetEvent::Added`/`AssetEvent::Removed`፣ `AssetDefinitionEvent::MintabilityChanged` (`Once` ወይም `Limited(n)` አበል ሲያልቅ)።
  - ስህተቶች: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. ኮድ: `core/.../isi/asset.rs`.- ቀስቅሴ ድግግሞሾች ከአዝሙድና / ማቃጠል: ለውጦች `action.repeats` አንድ ቀስቅሴ ቆጠራ.
  - ቅድመ-ሁኔታዎች-በአዝሙድ ላይ ፣ ማጣሪያው ሊታከም የሚችል መሆን አለበት ። አርቲሜቲክ ከመጠን በላይ መፍሰስ / መፍሰስ የለበትም።
  - ክስተቶች: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - ስህተቶች: `MathError::Overflow` ልክ ባልሆነ ሚንት ላይ; `FindError::Trigger` ከጠፋ። ኮድ: `core/.../isi/triggers/mod.rs`.

### ማስተላለፍ
አይነቶች: `Transfer<S: Identifiable, O, D: Identifiable>`, በቦክስ `TransferBox`.

ንብረት (ቁጥር)፡ ከምንጩ `AssetId` ቀንስ፣ ወደ መድረሻው `AssetId` (ተመሳሳይ ፍቺ፣ የተለየ መለያ) ይጨምሩ። የዜሮ ምንጭ ንብረትን ሰርዝ።
  - ቅድመ ሁኔታዎች: የምንጭ ንብረት አለ; ዋጋ `spec` ያሟላል።
  - ክስተቶች: `AssetEvent::Removed` (ምንጭ), `AssetEvent::Added` (መድረሻ).
  - ስህተቶች: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. ኮድ: `core/.../isi/asset.rs`.

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
አይነቶች፡ `Grant<O, D>` እና `Revoke<O, D>`፣በቦክስ ቁጥሮች ለ`Permission`/`Role` እስከ/ከ`Account`፣ እና `Permission` እስከ `Permission`፣ እና `Permission`- የመለያ ፍቃድ ይስጡ፡- ቀድሞውንም ካልተፈጠረ በስተቀር `Permission` ይጨምራል። ክስተቶች: `AccountEvent::PermissionAdded`. ስህተቶች፡ `Repetition(Grant, Permission)` ከተባዛ። ኮድ: `core/.../isi/account.rs`.
- ከመለያ ፈቃዱን ይሰርዙ፡ ካለ ያስወግዳል። ክስተቶች: `AccountEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/account.rs`.
- ሚናን ለመለያ ይስጡ፡ ከሌለ `(account, role)` ካርታ ስራን ያስገባል። ክስተቶች: `AccountEvent::RoleGranted`. ስህተቶች: `Repetition(Grant, RoleId)`. ኮድ: `core/.../isi/account.rs`.
- ሚና ከመለያ መሻር፡ ካለ የካርታ ስራን ያስወግዳል። ክስተቶች: `AccountEvent::RoleRevoked`. ስህተቶች፡ `FindError::Role` ከሌለ። ኮድ: `core/.../isi/account.rs`.
የሚና ፈቃድ ስጥ፡ ከተጨመረ ፍቃድ ጋር ሚናውን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionAdded`. ስህተቶች: `Repetition(Grant, Permission)`. ኮድ: `core/.../isi/world.rs`.
- የሚናውን ፈቃድ መሻር፡ ያለዚያ ፍቃድ ሚናውን እንደገና ይገነባል። ክስተቶች: `RoleEvent::PermissionRemoved`. ስህተቶች፡ `FindError::Permission` ከሌለ። ኮድ: `core/.../isi/world.rs`.### ቀስቅሴዎች፡ መፈጸም
ዓይነት: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- ባህሪ፡ ለቀስቃሽ ንኡስ ስርዓት `ExecuteTriggerEvent { trigger_id, authority, args }`ን ያሰማል። በእጅ መፈጸም የሚፈቀደው በጥሪ ቀስቅሴዎች ብቻ ነው (`ExecuteTrigger` ማጣሪያ); ማጣሪያው መመሳሰል አለበት እና ደዋዩ ቀስቅሴ እርምጃ ባለስልጣን መሆን አለበት ወይም ለዛ ባለስልጣን `CanExecuteTrigger` ን ያዝ። በተጠቃሚ የቀረበ ፈፃሚ ንቁ ሲሆን የማስፈንጠሪያ ማስፈጸሚያ በሂደት አስፈፃሚው የተረጋገጠ እና የግብይቱን አስፈፃሚ የነዳጅ በጀት ይበላል (ቤዝ `executor.fuel` እና አማራጭ ሜታዳታ `additional_fuel`)።
- ስህተቶች: `FindError::Trigger` ካልተመዘገበ; `InvariantViolation` ባለስልጣን ከተጠራ። ኮድ: `core/.../isi/triggers/mod.rs` (እና ሙከራዎች `core/.../smartcontracts/isi/mod.rs` ውስጥ).

### ያሻሽሉ እና ይመዝገቡ
- `Upgrade { executor }`: በተሰጠው `Executor` ባይትኮድ፣አስፈፃሚውን አዘምን እና የውሂብ ሞዴሉን በመጠቀም ፈጻሚውን ያፈልሳል፣`ExecutorEvent::Upgraded` ያወጣል። ስህተቶች፡ በስደት አለመሳካት ላይ እንደ `InvalidParameterError::SmartContract` ተጠቅልሏል። ኮድ: `core/.../isi/world.rs`.
- `Log { level, msg }`: ከተሰጠው ደረጃ ጋር የመስቀለኛ መዝገብ ያወጣል; ምንም የመንግስት ለውጥ የለም. ኮድ: `core/.../isi/world.rs`.

### የስህተት ሞዴል
የጋራ ኤንቨሎፕ፡ `InstructionExecutionError` ከተለዋዋጮች ጋር ለግምገማ ስህተቶች፣ የመጠይቅ ውድቀቶች፣ ልወጣዎች፣ ህጋዊ አካል አልተገኘም፣ ድግግሞሹ፣ ማትባት፣ ሂሳብ፣ ልክ ያልሆነ መለኪያ እና የማይለዋወጥ ጥሰት። ቆጠራዎች እና ረዳቶች በ `crates/iroha_data_model/src/isi/mod.rs` በ `pub mod error` ውስጥ ይገኛሉ።

---## ግብይቶች እና ተፈፃሚዎች
- `Executable`: ወይ `Instructions(ConstVec<InstructionBox>)` ወይም `Ivm(IvmBytecode)`; ባይቴኮድ ተከታታይነት ያለው እንደ ቤዝ64 ነው። ኮድ: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`፡ ይገነባል፣ ምልክቶች እና ፓኬጆች በሜታዳታ፣ `chain_id`፣ `authority`፣ `creation_time_ms`፣ አማራጭ፣ እና I1839001 `nonce`. ኮድ: `crates/iroha_data_model/src/transaction/`.
- በሂደት ጊዜ፣ `iroha_core` `InstructionBox` ባች በ`Execute for InstructionBox` ያስፈጽማል፣ ወደ ተገቢው `*Box` ወይም ተጨባጭ መመሪያ። ኮድ: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- የአሂድ ጊዜ ፈፃሚ ማረጋገጫ በጀት (በተጠቃሚ የቀረበ ፈጻሚ)፡ ቤዝ `executor.fuel` ከመለኪያዎች እና አማራጭ የግብይት ሜታዳታ `additional_fuel` (`u64`)፣ በግብይቱ ውስጥ ባሉ መመሪያዎች/ቀስቃሽ ማረጋገጫዎች ተጋርቷል።

---## ተለዋዋጮች እና ማስታወሻዎች (ከሙከራዎች እና ከጠባቂዎች)
- የዘፍጥረት ጥበቃዎች፡ የ`genesis` ጎራ ወይም መለያዎች በ`genesis` ጎራ መመዝገብ አይችሉም። `genesis` መለያ መመዝገብ አይቻልም። ኮድ/ሙከራዎች፡ `core/.../isi/world.rs`፣ `core/.../smartcontracts/isi/mod.rs`።
- የቁጥር ንብረቶች `NumericSpec` በ mint / በማስተላለፍ / በማቃጠል ላይ ማሟላት አለባቸው; spec አለመዛመድ `TypeError::AssetNumericSpec` ያስገኛል.
- Mintability: `Once` ነጠላ mint ይፈቅዳል እና ከዚያ ወደ `Not` ይገለበጣል; `Limited(n)` ወደ `Not` ከመገልበጡ በፊት በትክክል `n` ሚንት ይፈቅዳል። በ`Infinitely` ምክንያት `MintabilityError::ForbidMintOnMintable` ላይ መፈጠርን ለመከልከል የተደረጉ ሙከራዎች እና `Limited(0)` ማዋቀር `MintabilityError::InvalidMintabilityTokens` ይሰጣል።
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
- የሕይወት ዑደት ቀስቅሴ;
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` በማጣሪያ በተዘዋዋሪ ከሚታሰበው ቼክ ጋር; `ExecuteTrigger::new(id).with_args(&args)` ከተዋቀረ ባለስልጣን ጋር መዛመድ አለበት።
  - የሜታዳታ ቁልፍ `__enabled` ወደ `false` በማቀናበር ቀስቅሴዎችን ማሰናከል ይቻላል (የነቁ ነባሪዎች ይጎድላሉ)። በ `SetKeyValue::trigger` ወይም በ IVM `set_trigger_enabled` syscall ቀይር።
  - ቀስቅሴ ማከማቻ በጭነት ላይ ተስተካክሏል፡ የተባዙ መታወቂያዎች፣ የማይዛመዱ መታወቂያዎች እና የጎደሉት ባይትኮድ ቀስቅሴዎች ወድቀዋል። የባይቴኮድ ማጣቀሻ ቆጠራዎች እንደገና ይሰላሉ።
  - የማስፈንጠሪያው IVM ባይትኮድ በአፈፃፀም ጊዜ ከጠፋ፣ ማስጀመሪያው ይወገዳል እና አፈፃፀሙ ከውድቀት ጋር እንደ ምንም-op ይቆጠራል።
  - የተዳከሙ ቀስቅሴዎች ወዲያውኑ ይወገዳሉ; በአፈፃፀም ወቅት የተሟጠጠ መግቢያ ካጋጠመው ተቆርጦ እንደጠፋ ይቆጠራል.
- የመለኪያ ማሻሻያ;
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` አዘምኗል እና `ConfigurationEvent::Changed` ያወጣል።CLI / Torii የንብረት-ፍቺ መታወቂያ + ተለዋጭ ምሳሌዎች፡-
- በቀኖናዊ Base58 መታወቂያ + ግልጽ ስም + ረጅም ተለዋጭ ስም ይመዝገቡ።
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#ubl.sbp`
- በቀኖናዊ Base58 መታወቂያ + ግልጽ ስም + አጭር ቅጽል ይመዝገቡ:
  - `iroha ledger asset definition register --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa --name pkr --alias pkr#sbp`
- ሚንት በቅፅል ስም + መለያ ክፍሎች
  - `iroha ledger asset mint --definition-alias pkr#ubl.sbp --account <i105> --quantity 500`
- በቀኖናዊ Base58 መታወቂያ ቅጽል መፍታት፡-
  - `POST /v1/assets/aliases/resolve` ከJSON `{ "alias": "pkr#ubl.sbp" }` ጋር

የስደት ማስታወሻ፡-
- `name#domain` የጽሑፍ ንብረት-መግለጫ መታወቂያዎች በመጀመሪያ ልቀት ላይ ሆን ተብሎ ያልተደገፉ ይቆያሉ; ቀኖናዊ Base58 መታወቂያዎችን ይጠቀሙ ወይም ነጥብ ያለበትን ስም ይፍቱ።
- የህዝብ ንብረት መራጮች ቀኖናዊ Base58 የንብረት-ጥራት መታወቂያዎችን እና የተከፈለ የባለቤትነት መስኮችን (`account`፣ አማራጭ `scope`) ይጠቀማሉ። ጥሬ ኢንኮድ የተደረገ `AssetId` ቀጥተኛ ረዳቶች ሆነው ይቀራሉ እና የTorii/CLI መራጭ ወለል አካል አይደሉም።
- የንብረት ፍቺ ዝርዝር/መጠይቅ ያጣሩ እና ዓይነቶች በተጨማሪ `alias_binding.status`፣ `alias_binding.lease_expiry_ms`፣ `alias_binding.grace_until_ms` እና `alias_binding.bound_at_ms` ይቀበላሉ።

---

## የመከታተያ ችሎታ (የተመረጡ ምንጮች)
 - የውሂብ ሞዴል ኮር: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - ISI ትርጓሜዎች እና መዝገብ: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - የ ISI አፈፃፀም: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - ክስተቶች: `crates/iroha_data_model/src/events/**`.
 - ግብይቶች: `crates/iroha_data_model/src/transaction/**`.

ይህ ዝርዝር ወደ ኤፒአይ/የባህሪ ሠንጠረዥ እንዲሰፋ ወይም ከእያንዳንዱ ተጨባጭ ክስተት/ስህተት ጋር የተገናኘ እንዲሆን ከፈለጉ ቃሉን ይናገሩ እና አራዝመዋለሁ።