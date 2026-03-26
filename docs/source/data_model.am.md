---
lang: am
direction: ltr
source: docs/source/data_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 683bfb31442f8f4ce7b1bf5038f9dba92fe092545e655f43b51195c21535d3c4
source_last_modified: "2026-03-12T11:24:23.059339+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha v2 የውሂብ ሞዴል - ጥልቅ ዳይቭ

ይህ ሰነድ የIroha v2 መረጃ ሞዴል የሆኑትን አወቃቀሮች፣ መለያዎች፣ ባህሪያት እና ፕሮቶኮሎች ያብራራል፣ በ `iroha_data_model` crate ውስጥ እንደተተገበረ እና በስራ ቦታ ላይ ጥቅም ላይ ይውላል። እርስዎ መገምገም እና ዝማኔዎችን መጠቆም የሚችሉበት ትክክለኛ ማጣቀሻ ነው።

## ወሰን እና መሰረቶች

ዓላማ፡ ቀኖናዊ ዓይነቶችን ለጎራ ነገሮች (ጎራዎች፣ መለያዎች፣ ንብረቶች፣ ኤንኤፍቲዎች፣ ሚናዎች፣ ፈቃዶች፣ እኩዮች)፣ የግዛት ለውጥ መመሪያዎችን (ISI)፣ መጠይቆችን፣ ቀስቅሴዎችን፣ ግብይቶችን፣ ብሎኮችን እና ግቤቶችን ያቅርቡ።
- ተከታታይነት፡ ሁሉም የወል አይነቶች Norito ኮዴኮች (`norito::codec::{Encode, Decode}`) እና ሼማ (`iroha_schema::IntoSchema`) ያመጣሉ። JSON ከባህሪ ባንዲራዎች በስተጀርባ እየተመረጠ (ለምሳሌ ለኤችቲቲፒ እና `Json` ጭነቶች) ጥቅም ላይ ይውላል።
- IVM ማስታወሻ፡ Iroha ቨርቹዋል ማሽን (IVM) ዒላማ በሚያደርግበት ጊዜ አስተናጋጁ ኮንትራቶችን ከመጥራቱ በፊት ማረጋገጫ ስለሚፈጽም የተወሰኑ የዲሴሪያላይዜሽን ጊዜ ማረጋገጫዎች ይሰናከላሉ።
- FFI በሮች፡ አንዳንድ አይነቶች FFI በማይፈለግበት ጊዜ ከአቅም በላይ ለማስቀረት በ`iroha_ffi` በ `ffi_export`/`ffi_import` በኩል በሁኔታዊ ሁኔታ ተብራርተዋል።

## ዋና ባህሪያት እና አጋዦች- `Identifiable`: አካላት የተረጋጋ `Id` እና `fn id(&self) -> &Self::Id` አላቸው. ከ`IdEqOrdHash` ጋር ለካርታ/ለወዳጅነት ማዘጋጀት አለበት።
- `Registrable`/`Registered`: ብዙ አካላት (ለምሳሌ, `Domain`, `AssetDefinition`, `Role`) ግንበኛ ንድፍ ይጠቀማሉ. `Registered` የሩጫ አይነትን ለምዝገባ ግብይቶች ተስማሚ ከሆነው ቀላል ክብደት ገንቢ አይነት (`With`) ጋር ያገናኛል።
- `HasMetadata`፡ የተዋሃደ የአንድ ቁልፍ/ዋጋ `Metadata` ካርታ።
- `IntoKeyValue`፡ ማባዛትን ለመቀነስ `Key` (ID) እና `Value` (ዳታ)ን ለማከማቸት የተከፋፈለ ረዳት።
- `Owned<T>`/`Ref<'world, K, V>`: ቀላል ክብደት ያላቸው መጠቅለያዎች በማከማቻዎች እና በጥያቄ ማጣሪያዎች ውስጥ አላስፈላጊ ቅጂዎችን ለማስወገድ ያገለግላሉ።

## ስሞች እና መለያዎች- `Name`: ትክክለኛ የጽሑፍ መለያ። ነጭ ቦታን እና የተያዙ ቁምፊዎችን አይፈቅድም `@`, `#`, `$` (በተቀናበረ መታወቂያዎች ውስጥ ጥቅም ላይ ይውላል). ከማረጋገጫ ጋር በ `FromStr` በኩል የሚገነባ። ስሞች በዩኒኮድ NFC በመደበኛነት ተስተካክለዋል (ቀኖናዊ አቻ የፊደል አጻጻፍ እንደ አንድ ዓይነት እና የተከማቹ ናቸው)። ልዩ ስሙ `genesis` ተይዟል (በመያዣ የተረጋገጠ)።
- `IdBox`፡ ለማንኛውም የሚደገፍ መታወቂያ (`DomainId`፣ `AccountId`፣ `AssetDefinitionId`፣ `AssetId`፣ `NftId`፣ Norito `TriggerId`፣ `RoleId`፣ `Permission`፣ `CustomParameterId`)። ለአጠቃላይ ፍሰቶች እና Norito ኢንኮዲንግ እንደ ነጠላ አይነት ይጠቅማል።
- `ChainId`: ግልጽ ያልሆነ ሰንሰለት መለያ በግብይቶች ውስጥ መልሶ ለማጫወት ጥበቃ ጥቅም ላይ ይውላል።የመታወቂያዎች ሕብረቁምፊ ቅርጾች (ዙር-trippable ከ `Display`/`FromStr`)፡
- `DomainId`: `name` (ለምሳሌ `wonderland`)።
- `AccountId`፡ ቀኖናዊ ዶሜናዊ መለያ መለያ በ`AccountAddress` እንደ i105 ብቻ የተመዘገበ። የፓርሰር ግብዓቶች ቀኖናዊ i105 መሆን አለባቸው; የጎራ ቅጥያ (`@domain`)፣ ቀኖናዊ i105 ቃላቶች፣ ተለዋጭ ስሞች፣ ቀኖናዊ ሄክስ ተንታኝ ግብዓት፣ የቆየ `norito:` ክፍያ ጭነቶች፣ እና `uaid:`/`uaid:`/`opaque:` የመለያ ቅጾች ውድቅ ናቸው።
- `AssetDefinitionId`፡ ቀኖናዊ `unprefixed Base58 address with versioning and checksum` (UUID-v4 ባይት)።
- `AssetId`: ቀኖናዊ ኮድ በጥሬው `<canonical-base58-asset-definition-id>` (የቆዩ የጽሑፍ ቅጾች በመጀመሪያው መለቀቅ አይደገፉም)።
- `NftId`፡ `nft$domain` (ለምሳሌ `rose$garden`)።
- `PeerId`: `public_key` (የአቻ እኩልነት በወል ቁልፍ ነው)።

# አካላት

### ጎራ
- `DomainId { name: Name }` - ልዩ ስም።
- `Domain { id, logo: Option<SorafsUri>, metadata: Metadata, owned_by: AccountId }`.
- ግንበኛ: `NewDomain` በ `with_logo`, `with_metadata`, ከዚያም `Registrable::build(authority)` ስብስቦች `owned_by`.## መለያ
- `AccountId` በተቆጣጣሪው ቁልፍ የተከፈተ እና እንደ ቀኖናዊ i105 የተቀመጠ ቀኖናዊ ጎራ-አልባ መለያ መለያ ነው።
- `ScopedAccountId { account: AccountId, domain: DomainId }` ሰፋ ያለ እይታ በሚያስፈልግበት ጊዜ ብቻ ግልጽ የጎራ አውድ ይይዛል።
- `Account { id, metadata, label?, uaid? }` — `label` አማራጭ የተረጋጋ ተለዋጭ ስም በሬኪ መዝገቦች ጥቅም ላይ ይውላል፣ `uaid` አማራጭ Nexus-ሰፊ [ዩኒቨርሳል መለያ መታወቂያ](Kotodama) ይይዛል።
- ገንቢ: `NewAccount` በ `Account::new(id)`; ምዝገባ ግልጽ የሆነ የ`ScopedAccountId` ጎራ ይፈልጋል እና ከነባሪዎች አንዱን አይገምትም።

### የንብረት መግለጫዎች እና ንብረቶች
- `AssetDefinitionId { aid_bytes: [u8; 16] }` እንደ `unprefixed Base58 address` በጽሑፍ ተጋልጧል።
- `AssetDefinition { id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo: Option<SorafsUri>, metadata, owned_by: AccountId, total_quantity: Numeric }`.

  - Torii asset-definition responses may include `alias_binding { alias, status, lease_expiry_ms, grace_until_ms, bound_at_ms }`; alias selectors resolve against latest committed block time and stop resolving after grace, while direct reads may still show `expired_pending_cleanup` until sweep.
  - `name` በሰው ፊት ለፊት የሚታይ የማሳያ ጽሑፍ ያስፈልጋል እና `#`/`@` መያዝ የለበትም።
  - `alias` አማራጭ ነው እና አንዱ መሆን አለበት፡-
    - `<name>#<domain>.<dataspace>`
    - `<name>#<dataspace>`
    ከግራ ክፍል ጋር በትክክል `AssetDefinition.name`.
  - `Mintable`: `Infinitely` | `Once` | `Limited(u32)` | `Not`.
  - ግንበኞች: `AssetDefinition::new(id, spec)` ወይም ምቾት `numeric(id)`; `name` ያስፈልጋል እና በ `.with_name(...)` በኩል መዋቀር አለበት።
- `AssetId { account: AccountId, definition: AssetDefinitionId, scope: AssetBalanceScope }`.
- `Asset { id, value: Numeric }` ለማከማቻ ተስማሚ `AssetEntry`/`AssetValue`።
- `AssetBalanceScope`: `Global` ላልተገደቡ ሒሳቦች እና `Dataspace(DataSpaceId)` ለዳታ ቦታ የተገደቡ ሒሳቦች።
- `AssetTotalQuantityMap = BTreeMap<AssetDefinitionId, Numeric>` ለማጠቃለያ ኤፒአይዎች ተጋልጧል።### NFTs
- `NftId { domain: DomainId, name: Name }`.
- `Nft { id, content: Metadata, owned_by: AccountId }` (ይዘቱ የዘፈቀደ ቁልፍ/የዋጋ ዲበ ውሂብ ነው)።
- ገንቢ: `NewNft` በ `Nft::new(id, content)` በኩል።

### ሚናዎች እና ፈቃዶች
- `RoleId { name: Name }`.
- `Role { id, permissions: BTreeSet<Permission> }` ከገንቢ `NewRole { inner: Role, grant_to: AccountId }` ጋር።
- `Permission { name: Ident, payload: Json }` - የ `name` እና የመጫኛ እቅድ ከገባሪው `ExecutorDataModel` ጋር መጣጣም አለባቸው (ከዚህ በታች ይመልከቱ)።

### እኩዮች
- `PeerId { public_key: PublicKey }`.
- `Peer { address: SocketAddr, id: PeerId }` እና ምሳሌ `public_key@address` ሕብረቁምፊ ቅጽ.

### ክሪፕቶግራፊክ ፕሪሚቲቭስ (ባህሪ `sm`)
- `Sm2PublicKey` እና `Sm2Signature`: SEC1 የሚያሟሉ ነጥቦች እና ቋሚ ስፋት `r∥s` ፊርማዎች ለ SM2. ገንቢዎች የጥምዝ አባልነት እና መለያ መታወቂያዎችን ያረጋግጣሉ; Norito ኢንኮዲንግ በ`iroha_crypto` ጥቅም ላይ የዋለውን ቀኖናዊ ውክልና ያሳያል።
- `Sm3Hash`፡ `[u8; 32]` አዲስ ዓይነት የጂኤም/ቲ 0004 መፍጨትን የሚወክል፣ በማኒፌክት፣ በቴሌሜትሪ እና በሳይካል ምላሾች ውስጥ ጥቅም ላይ ይውላል።
- `Sm4Key`፡ ባለ 128-ቢት ሲሜትሪክ ቁልፍ መጠቅለያ በአስተናጋጅ syscals እና በዳታ-ሞዴል መጫዎቻዎች መካከል ተጋርቷል።
እነዚህ ዓይነቶች ከነባር Ed25519/BLS/ML-DSA ፕሪሚቲቭ ጎን ተቀምጠው የስራ ቦታው በ`--features sm` ከተገነባ በኋላ የህዝብ እቅድ አካል ይሆናሉ።### ቀስቅሴዎች እና ክስተቶች
- `TriggerId { name: Name }` እና `Trigger { id, action: action::Action }`።
- `action::Action { executable: Executable, repeats: Repeats, authority: AccountId, filter: EventFilterBox, metadata }`.
  - `Repeats`: `Indefinitely` ወይም `Exactly(u32)`; ማዘዝ እና ማሟያ መገልገያዎች ተካትተዋል.
  - ደህንነት፡- `TriggerCompleted` እንደ የድርጊት ማጣሪያ (በ(de) ተከታታይነት የተረጋገጠ) መጠቀም አይቻልም።
- `EventBox`: የቧንቧ መስመር, የቧንቧ መስመር-ባች, ዳታ, ጊዜ, ማስፈጸሚያ-ቀስቃሽ እና ቀስቅሴ-የተጠናቀቁ ክስተቶች ድምር አይነት; `EventFilterBox` መስተዋቶች ለደንበኝነት ምዝገባዎች እና ማጣሪያዎች።

## መለኪያዎች እና ውቅር

- የስርዓት መለኪያ ቤተሰቦች (ሁሉም `Default`ed፣ ተሸካሚዎች ተሸክመው ወደ ግለሰባዊ ቁጥሮች ይቀየራሉ)
- `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`.
  - `BlockParameters { max_transactions: NonZeroU64 }`.
  - `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`.
  - `SmartContractParameters { fuel, memory, execution_depth }`.
- `Parameters` ቡድኖች ሁሉንም ቤተሰቦች እና አንድ `custom: BTreeMap<CustomParameterId, CustomParameter>`.
- ነጠላ-መለኪያ ቁጥሮች፡- `SumeragiParameter`፣ `BlockParameter`፣ `TransactionParameter`፣ `SmartContractParameter` ለዲፍ መሰል ዝማኔዎች እና ድግግሞሽ።
- ብጁ መለኪያዎች፡ አስፈፃሚ-የተገለጸ፣ እንደ `Json` የተሸከመ፣ በ`CustomParameterId` (a `Name`) ተለይቷል።

## ISI (Iroha ልዩ መመሪያዎች)- ዋና ባህሪ: `Instruction` ከ `dyn_encode`, `as_any`, እና የተረጋጋ በዓይነት መለያ `id()` (ነባሪዎች የኮንክሪት ዓይነት ስም). ሁሉም መመሪያዎች `Send + Sync + 'static` ናቸው።
- `InstructionBox`: በባለቤትነት የ `Box<dyn Instruction>` መጠቅለያ በ clone/eq/ord በአይነት መታወቂያ + የተመሰጠረ ባይት የተተገበረ።
- አብሮገነብ የማስተማሪያ ቤተሰቦች የተደራጁት በ፡-
  - `mint_burn`፣ `transfer`፣ `register`፣ እና `transparent` የረዳቶች ጥቅል።
  - ለሜታ ፍሰቶች ዝርዝር ቁጥሮችን ይተይቡ፡ `InstructionType`፣ እንደ `SetKeyValueBox` (ጎራ/መለያ/asset_def/nft/ቀስቃሽ) ያሉ በቦክስ የተሰበሰቡ ድምሮች።
ስህተቶች፡ የበለፀገ የስህተት ሞዴል በ `isi::error` (የግምገማ አይነት ስህተቶች፣ስህተቶችን ፈልጎ ማግኘት፣mintability፣ ሂሳብ፣ልክ ያልሆኑ መለኪያዎች፣ድግግሞሽ፣ተለዋዋጮች)።
- የመመሪያ መዝገብ፡ `instruction_registry!{ ... }` ማክሮ በአይነት ስም የተከፈተ የሩጫ ጊዜ መፍታት መዝገብ ይገነባል። ተለዋዋጭ (de) ተከታታይነትን ለማግኘት በ`InstructionBox` clone እና Norito serde ጥቅም ላይ ይውላል። በ`set_instruction_registry(...)` በኩል ምንም መዝገብ በግልፅ ካልተዋቀረ፣ አብሮ የተሰራ ነባሪ መዝገብ ከሁሉም ኮር ISI ጋር በመጀመሪያ ጥቅም ላይ የዋለው ሁለትዮሽ ጥንካሬን ለመጠበቅ ነው።

## ግብይቶች- `Executable`፡ ወይ `Instructions(ConstVec<InstructionBox>)` ወይም `Ivm(IvmBytecode)`። `IvmBytecode` እንደ base64 ተከታታይ ያደርገዋል (ግልጽ የሆነ አዲስ ዓይነት ከ `Vec<u8>` በላይ)።
- `TransactionBuilder`፡ የግብይት ጭነትን በ`chain`፣ `authority`፣ `creation_time_ms`፣ አማራጭ `time_to_live_ms` እና `nonce`፣010001010 `Executable`.
  - ረዳቶች፡- `with_instructions`፣ `with_bytecode`፣ `with_executable`፣ `with_metadata`፣ `set_nonce`፣ `set_ttl`፣ Kotodama
- `SignedTransaction` (በ `iroha_version` ስሪት): `TransactionSignature` እና ጭነትን ይይዛል; ሀሺንግ እና ፊርማ ማረጋገጫ ይሰጣል።
- የመግቢያ ነጥቦች እና ውጤቶች;
  - `TransactionEntrypoint`: `External(SignedTransaction)` | `Time(TimeTriggerEntrypoint)`.
  - `TransactionResult` = `Result<DataTriggerSequence, TransactionRejectionReason>` ከሃሽ ረዳቶች ጋር።
  - `ExecutionStep(ConstVec<InstructionBox>)`: በአንድ ግብይት ውስጥ አንድ የታዘዘ መመሪያ ስብስብ።

## ብሎኮች- `SignedBlock` (የተሰራ) ያጠቃልላል፡-
  - `signatures: BTreeSet<BlockSignature>` (ከአረጋጋጮች) ፣
  - `payload: BlockPayload { header: BlockHeader, transactions: Vec<SignedTransaction> }`፣
  - `result: BlockResult` (ሁለተኛ ደረጃ የማስፈጸሚያ ሁኔታ) `time_triggers`፣ የመግቢያ/ውጤት Merkle ዛፎች፣ `transaction_results` እና `fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>` የያዘ።
መገልገያዎች፡- `presigned`፣ `set_transaction_results(...)`፣ `set_transaction_results_with_transcripts(...)`፣ `header()`፣ `signatures()`፣ `hash()`፣ Kotodama
- Merkle ሥሮች: የግብይት መግቢያ ነጥቦች እና ውጤቶች Merkle ዛፎች በኩል ፈጽሟል; ውጤት Merkle root ወደ የማገጃው ራስጌ ውስጥ ተቀምጧል.
- የማካተት ማረጋገጫዎችን አግድ (`BlockProofs`) ሁለቱንም የመግቢያ/ውጤት Merkle ማረጋገጫዎችን እና የ`fastpq_transcripts` ካርታን ያጋልጣል ስለዚህ ከሰንሰለት ውጪ የሆኑ ፕሮቨሮች ከግብይት ሃሽ ጋር የተገናኙትን የዝውውር ዴልታዎችን ማምጣት ይችላሉ።
- `ExecWitness` መልእክቶች (በTorii የሚተላለፉ እና በስምምነት ወሬ ላይ በ piggy የተደገፈ) አሁን ሁለቱንም `fastpq_transcripts` እና prover-ዝግጁ `fastpq_batches: Vec<FastpqTransitionBatch>` (ሥርወ-ስርወ-ሥርወ-ሥር-ሥር-ሥር-ሥር-ሥር)፣ Norito ያካትታሉ። tx_set_hash)፣ ስለዚህ የውጪ አራሚዎች የጽሑፍ ግልባጮችን እንደገና ሳይቀዱ ቀኖናዊ FASTPQ ረድፎችን ማስገባት ይችላሉ።

##ጥያቄዎች- ሁለት ቅመሞች;
  ነጠላ፡- `SingularQuery<Output>` (ለምሳሌ፡ `FindParameters`፣ `FindExecutorDataModel`) ተግብር።
  - ሊደረግ የሚችል፡- `Query<Item>` (ለምሳሌ `FindAccounts`፣ `FindAssets`፣ `FindDomains`፣ ወዘተ) መተግበር።
- የተሰረዙ ቅጾች;
  - `QueryBox<T>` በቦክስ የተሰረዘ `Query<Item = T>` ከ Norito serde ጋር በአለምአቀፍ መዝገብ የተደገፈ ነው።
  - `QueryWithFilter<T> { query, predicate, selector }` ጥያቄን ከዲኤስኤል ተሳቢ/መራጭ ጋር ያጣምራል። በ`From` በኩል ወደ ተሰረዘ የሚደጋገም ጥያቄ ይቀየራል።
- መዝገብ ቤት እና ኮዴክ;
  - `query_registry!{ ... }` ዓለም አቀፋዊ የመመዝገቢያ ካርታ የኮንክሪት መጠይቅ ዓይነቶችን ለገንቢዎች በአይነት ስም ለተለዋዋጭ ዲኮድ ይገነባል።
  - `QueryRequest = Singular(SingularQueryBox) | Start(QueryWithParams) | Continue(ForwardCursor)` እና `QueryResponse = Singular(..) | Iterable(QueryOutput)`።
  - `QueryOutputBatchBox` ተመሳሳይ በሆነ ቬክተር (ለምሳሌ `Vec<Account>`፣ `Vec<Name>`፣ `Vec<AssetDefinition>`፣ `Vec<BlockHeader>`)፣ እንዲሁም ቱፕል እና ማራዘሚያ አጋዥዎች።
- DSL፡ በ`query::dsl` ውስጥ በፕሮጀክሽን ባህሪያት (`HasProjection<PredicateMarker>` / `SelectorMarker`) የተተገበረ በጊዜ የተረጋገጡ ተሳቢዎች እና መራጮች። የ`fast_dsl` ባህሪ ካስፈለገ ቀለል ያለ ልዩነትን ያጋልጣል።

## ፈፃሚ እና ኤክስቴንሽን- `Executor { bytecode: IvmBytecode }`: አረጋጋጭ-ተፈፃሚው የኮድ ጥቅል።
- `ExecutorDataModel { parameters: CustomParameters, instructions: BTreeSet<Ident>, permissions: BTreeSet<Ident>, schema: Json }` በአስፈፃሚው የተገለጸውን ጎራ ያውጃል፡-
  - ብጁ ውቅር መለኪያዎች,
  - ብጁ መመሪያ መለያዎች;
  - የፍቃድ ማስመሰያ መለያዎች ፣
  - ለደንበኛ መገልገያ ብጁ ዓይነቶችን የሚገልጽ የJSON ንድፍ።
የማበጀት ናሙናዎች በ `data_model/samples/executor_custom_data_model` ስር ይገኛሉ፡-
  - ብጁ የፍቃድ ማስመሰያ በ `iroha_executor_data_model::permission::Permission` ውፅዓት ፣
  - ብጁ ግቤት ወደ `CustomParameter` የሚቀየር ዓይነት ሆኖ ተገልጿል፣
  - ብጁ መመሪያዎች ለአፈፃፀም ወደ `CustomInstruction` ተከታታይ።

### ብጁ መመሪያ (በአስፈጻሚው የተገለጸ ISI)- አይነት: `isi::CustomInstruction { payload: Json }` በተረጋጋ የሽቦ መታወቂያ `"iroha.custom"`.
ዓላማው፡-የህዝብ ዳታ ሞዴልን ሳያንኳኳ በግላዊ/በኮንሰርቲየም ኔትወርኮች ወይም በፕሮቶታይፕ ለፈጻሚ-ተኮር መመሪያዎች ፖስታ።
- ነባሪ የአስፈፃሚ ባህሪ፡ በ`iroha_core` ውስጥ አብሮ የተሰራው አስፈፃሚ `CustomInstruction` አይሰራም እና ካጋጠመው ይደነግጣል። አንድ ብጁ አስፈፃሚ `InstructionBox` ወደ `CustomInstruction` ዝቅ ማድረግ እና በሁሉም አረጋጋጮች ላይ ያለውን ጭነት በቆራጥነት መተርጎም አለበት።
- Norito: በ `norito::codec::{Encode, Decode}` በኩል ያዘጋጃል / ከመርሃግብር ጋር; የ `Json` ጭነት በተከታታይ ተወስኗል። የመመሪያው መዝገብ `CustomInstruction` (የነባሪው መዝገብ አካል ነው) እስካካተተ ድረስ ክብ ጉዞዎች የተረጋጋ ናቸው።
- IVM: Kotodama ወደ IVM ባይትኮድ (`.to`) ያጠናቅራል እና ለትግበራ አመክንዮ የሚመከር መንገድ ነው። እስካሁን በKotodama ውስጥ ሊገለጹ ለማይችሉ ፈጻሚ-ደረጃ ማራዘሚያዎች `CustomInstruction` ብቻ ይጠቀሙ። በእኩዮች መካከል ቆራጥነት እና ተመሳሳይ አስፈፃሚ ሁለትዮሾችን ያረጋግጡ።
- ለሕዝብ አውታረ መረቦች አይደለም: የተለያዩ አስፈፃሚዎች የጋራ ስምምነት ሹካዎችን አደጋ ላይ በሚጥሉበት የህዝብ ሰንሰለት አይጠቀሙ። የመድረክ ባህሪያትን በሚፈልጉበት ጊዜ አዲስ አብሮ የተሰራ ISI ዥረት ሃሳብ ማቅረብን ይምረጡ።

## ዲበ ውሂብ- `Metadata(BTreeMap<Name, Json>)`፡ ቁልፍ/ እሴት ማከማቻ ከብዙ አካላት ጋር ተያይዟል (`Domain`፣ `Account`፣ `AssetDefinition`፣ `Nft`፣ ቀስቅሴዎች እና ግብይቶች)።
- API፡ `contains`፣ `iter`፣ `get`፣ `insert`፣ እና (ከ`transparent_api` ጋር) `remove`።

## ባህሪዎች እና ቆራጥነት

- የአማራጭ ኤፒአይዎችን (`std`፣ `json`፣ `transparent_api`፣ `ffi_export`፣ `ffi_import`፣ `fast_dsl`፣ `fast_dsl`፣018NI00000307X፣01000X፣018 `fault_injection`).
- ቆራጥነት፡ ሁሉም ተከታታይነት Norito ኢንኮዲንግ በሃርድዌር ላይ ተንቀሳቃሽ እንዲሆን ይጠቀማል። IVM ባይት ኮድ ግልጽ ያልሆነ ባይት ብሎብ ነው; ማስፈጸሚያ የማይወስኑ ቅነሳዎችን ማስተዋወቅ የለበትም። አስተናጋጁ ግብይቶችን ያረጋግጣል እና ግብዓቶችን ለIVM በቆራጥነት ያቀርባል።

### ግልጽ ኤፒአይ (`transparent_api`)ዓላማው፡ እንደ Torii፣ ፈፃሚዎች እና የውህደት ፈተናዎች ያሉ የ`#[model]` መዋቅሮች/የመረጃ ዝርዝሮችን ሙሉ፣ ተለዋዋጭ መዳረሻን ያጋልጣል። ያለሱ፣ እነዚያ እቃዎች ሆን ብለው ግልጽ ያልሆኑ ናቸው ስለዚህ ውጫዊ ኤስዲኬዎች ደህንነቱ የተጠበቀ ግንበኞችን እና የተመሰጠሩ ጭነቶችን ብቻ ነው የሚያዩት።
- ሜካኒክስ፡ `iroha_data_model_derive::model` ማክሮ እያንዳንዱን የህዝብ መስክ በ`#[cfg(feature = "transparent_api")] pub` እንደገና ይጽፋል እና ለነባሪ ግንባታው የግል ቅጂ ይይዛል። ባህሪውን ማንቃት እነዚያን cfgs ይገለብጣቸዋል፣ ስለዚህ `Account`፣ `Domain`፣ `Asset`፣ ወዘተ ማበላሸት ከነርሱ ሞጁሎች ውጭ ህጋዊ ይሆናል።
- የገጽታ ማወቂያ፡ ሣጥኑ `TRANSPARENT_API: bool` ቋሚ (በ`transparent_api.rs` ወይም `non_transparent_api.rs` የተፈጠረ) ወደ ውጭ ይልካል። የታችኛው ተፋሰስ ኮድ ይህንን ባንዲራ እና ቅርንጫፍ ወደ ግልጽ ባልሆኑ ረዳቶች መመለስ ሲፈልግ ማረጋገጥ ይችላል።
- በማንቃት ላይ፡ `features = ["transparent_api"]` ወደ ጥገኝነት በ`Cargo.toml` ይጨምሩ። የJSON ትንበያ (ለምሳሌ `iroha_torii`) የሚያስፈልጋቸው የመስሪያ ቦታ ሳጥኖች ባንዲራውን በራስ ሰር ያስተላልፋሉ፣ ነገር ግን የሶስተኛ ወገን ሸማቾች ማሰማራቱን ካልተቆጣጠሩ እና ሰፊውን የኤፒአይ ገጽ ካልተቀበሉ በስተቀር እሱን ማጥፋት አለባቸው።

## ፈጣን ምሳሌዎች

ጎራ እና መለያ ይፍጠሩ፣ ንብረትን ይግለጹ እና በመመሪያዎች ግብይት ይገንቡ፡

```rust
use iroha_data_model::prelude::*;
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;

// Domain
let domain_id: DomainId = "wonderland".parse().unwrap();
let new_domain = Domain::new(domain_id.clone()).with_metadata(Metadata::default());

// Account
let kp = KeyPair::random();
let account_id = AccountId::new(kp.public_key().clone());
let new_account = Account::new(account_id.to_account_id(domain_id.clone()))
    .with_metadata(Metadata::default());

// Asset definition and an asset for the account
let asset_def_id: AssetDefinitionId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa".parse().unwrap();
let new_asset_def = AssetDefinition::numeric(asset_def_id.clone())
    .with_name("USD Coin".to_owned())
    .with_metadata(Metadata::default());
let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
let asset = Asset::new(asset_id.clone(), Numeric::from(100));

// Build a transaction with instructions (pseudo-ISI; exact ISI types live under `isi`)
let chain_id: ChainId = "dev-chain".parse().unwrap();
let tx = TransactionBuilder::new(chain_id, account_id.clone())
    .with_instructions(vec![ /* Register/ Mint/ Transfer instructions here */ ])
    .sign(kp.private_key());
```

መለያዎችን እና ንብረቶችን በDSL ይጠይቁ፡-

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

IVM ብልጥ ውል ባይትኮድ ተጠቀም፡-

```rust
use iroha_data_model::prelude::*;

let bytecode = IvmBytecode::from_compiled(include_bytes!("contract.to").to_vec());
let tx = TransactionBuilder::new("dev-chain".parse().unwrap(), account_id.clone())
    .with_bytecode(bytecode)
    .sign(kp.private_key());
```

asset-definition id / ቅጽል ፈጣን ማጣቀሻ (CLI + Torii):

```bash
# Register an asset definition with canonical Base58 id + explicit name + alias
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#ubl.sbp

# Short alias form (no owner segment): <name>#<dataspace>
iroha ledger asset definition register \
  --id 66owaQmAQMuHxPzxUN3bqZ6FJfDa \
  --name pkr \
  --alias pkr#sbp

# Mint using alias + account components (no manual norito hex copy/paste)
iroha ledger asset mint \
  --definition-alias pkr#ubl.sbp \
  --account sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB \
  --quantity 500

# Resolve alias to canonical Base58 id via Torii
curl -sS http://127.0.0.1:8080/v1/assets/aliases/resolve \
  -H 'content-type: application/json' \
  -d '{"alias":"pkr#ubl.sbp"}'
```የስደት ማስታወሻ፡-
- የድሮ `name#domain` የንብረት ትርጉም መታወቂያዎች v1 ውስጥ ተቀባይነት የላቸውም።
- ለአዝሙድና / ለማቃጠል / ለማስተላለፍ የንብረት መታወቂያዎች ቀኖናዊ `<canonical-base58-asset-definition-id>` ይቀራሉ; እነሱን በ:
  - `iroha tools encode asset-id --definition <base58-asset-definition-id> --account <i105>`
  - ወይም `--alias <name>#<domain>.<dataspace>` / `--alias <name>#<dataspace>` + `--account`.

## ስሪት ማውጣት

- `SignedTransaction`፣ `SignedBlock`፣ እና `SignedQuery` ቀኖናዊ Norito የተመሰጠሩ መዋቅሮች ናቸው። እያንዳንዳቸው `iroha_version::Version` በ `EncodeVersioned` ሲመሰጠሩ ክፍያቸውን አሁን ባለው የ ABI ስሪት (በአሁኑ ጊዜ `1`) ቅድመ ቅጥያ ለማድረግ ይተገብራል።

## የግምገማ ማስታወሻዎች / ሊሆኑ የሚችሉ ዝማኔዎች

- መጠይቅ DSL፡ የተረጋጋ ተጠቃሚን የሚመለከት ንዑስ ስብስብ እና ለጋራ ማጣሪያዎች/መራጮች ምሳሌዎችን መመዝገብ ያስቡበት።
- የማስተማር ቤተሰቦች፡ በ`mint_burn`፣ `register`፣ `transfer` የተጋለጡ አብሮገነብ የISI ልዩነቶችን የሚዘረዝር የህዝብ ሰነዶችን ዘርጋ።

---
የትኛውም ክፍል የበለጠ ጥልቀት የሚያስፈልገው ከሆነ (ለምሳሌ፣ ሙሉ የአይኤስአይ ካታሎግ፣ የተሟላ የጥያቄ መዝገብ ዝርዝር፣ ወይም የራስጌ መስኮችን አግድ)፣ አሳውቀኝ እና እነዚያን ክፍሎች በዚሁ መሰረት እሰፋለሁ።