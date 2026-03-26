---
lang: am
direction: ltr
source: docs/source/ivm_syscalls.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bcf280df1e00065199d386e07b9fd67d8f94c4046d73cfa3b63d1eec18228cd8
source_last_modified: "2026-01-22T16:26:46.570453+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Syscall ABI

ይህ ሰነድ የIVM syscall ቁጥሮችን፣ የጠቋሚ-ABI ጥሪ ስምምነቶችን፣ የተጠበቁ የቁጥር ክልሎችን እና Kotodama ዝቅ በማድረግ ጥቅም ላይ የሚውለውን የኮንትራት ትይዩ የሰነድ ሰንጠረዥን ይገልጻል። እሱ `ivm.md` (አርክቴክቸር) እና `kotodama_grammar.md` (ቋንቋ) ያሟላል።

ስሪት ማውጣት
- የታወቁ የሲስክሎች ስብስብ በባይቴኮድ ራስጌ `abi_version` መስክ ላይ ይወሰናል. የመጀመሪያው ልቀት `abi_version = 1` ብቻ ይቀበላል; ሌሎች እሴቶች በመግቢያው ላይ ውድቅ ይደረጋሉ። ለገቢር `abi_version` የማይታወቁ ቁጥሮች ከ`E_SCALL_UNKNOWN` ጋር ወጥመድ።
- የሩጫ ጊዜ ማሻሻያዎች `abi_version = 1`ን ያቆያሉ እና syscall ወይም pointer-ABI ን አይስፉም።
- Syscall ጋዝ ወጪዎች ከባይትኮድ ራስጌ ስሪት ጋር የተቆራኘው የተሻሻለው የጋዝ መርሃ ግብር አካል ነው። `ivm.md` (የጋዝ ፖሊሲ) ይመልከቱ።

የቁጥር ክልሎች
- `0x00..=0x1F`፡ ቪኤም ኮር/መገልገያ (ማረሚያ/መውጣት ረዳቶች በ`CoreHost` ስር ይገኛሉ፤ የተቀሩት የዴቭ ረዳቶች መሳለቂያ አስተናጋጅ ብቻ ናቸው)።
- `0x20..=0x5F`: Iroha ኮር ISI ድልድይ (በ ABI v1 ውስጥ የተረጋጋ).
- `0x60..=0x7F`፡ ቅጥያ አይኤስአይኤስ በፕሮቶኮል ባህሪያት የታሸገ (አሁንም የABI v1 አካል ሲነቃ)።
- `0x80..=0xFF`: አስተናጋጅ/crypto አጋዥ እና የተያዙ ቦታዎች; በ ABI v1 የተፈቀደ ዝርዝር ውስጥ ያሉ ቁጥሮች ብቻ ይቀበላሉ።

ዘላቂ ረዳቶች (ABI v1)
- ዘላቂው የስቴት አጋዥ ሲስክሎች (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_*, JSON/SCHEMA encode/decode) የV1 ABI አካል ናቸው እና በ`abi_hash` ስሌት ውስጥ ተካተዋል።
- የCoreHost ሽቦዎች STATE_{GET,SET,DEL} ወደ WSV የሚደገፍ የሚበረክት ዘመናዊ ኮንትራት ሁኔታ; የዴቭ/የሙከራ አስተናጋጆች በአገር ውስጥ ሊቆዩ ይችላሉ ነገር ግን ተመሳሳይ የሳይካል ትርጉሞችን መጠበቅ አለባቸው።

የጠቋሚ አቢይ ጥሪ ኮንቬንሽን (ብልጥ የውል ስምምነቶች)
- ክርክሮች በ `r10+` እንደ ጥሬ `u64` እሴቶች ወይም ወደ INPUT ክልል ጠቋሚዎች ወደ የማይለወጡ Norito TLV ፖስታዎች (ለምሳሌ `r10+` ተቀምጠዋል) `Name`፣ `Json`፣ `NftId`)።
- Scalar መመለሻ ዋጋዎች `u64` ከአስተናጋጁ የተመለሱ ናቸው. የጠቋሚ ውጤቶች በአስተናጋጁ ወደ `r10` ተጽፈዋል።

ቀኖናዊ syscall ሰንጠረዥ (ንዑስ ስብስብ)| ሄክስ | ስም | ክርክሮች (በ`r10+`) | ይመለሳል | ጋዝ (ቤዝ + ተለዋዋጭ) | ማስታወሻ |
|------------------|-------------------|------------|
| 0x1A | SET_ACCOUNT_DETAIL | `&AccountId`፣ `&Name`፣ `&Json` | `u64=0` | `G_set_detail + bytes(val)` | ለመለያው ዝርዝር ይጽፋል |
| 0x22 | MINT_ASSET | `&AccountId`፣ `&AssetDefinitionId`፣ `&NoritoBytes(Numeric)` | `u64=0` | `G_mint` | Mints `amount` ንብረት ወደ መለያ |
| 0x23 | BURN_ASSET | `&AccountId`፣ `&AssetDefinitionId`፣ `&NoritoBytes(Numeric)` | `u64=0` | `G_burn` | ከመለያው `amount` ያቃጥላል |
| 0x24 | ማስተላለፍ_ASSET | `&AccountId(from)`፣ `&AccountId(to)`፣ `&AssetDefinitionId`፣ `&NoritoBytes(Numeric)` | `u64=0` | `G_transfer` | `amount` በመለያዎች መካከል ያስተላልፋል |
| 0x29 | ማስተላለፍ_V1_BATCH_BEGIN | – | `u64=0` | `G_transfer` | FASTPQ የማስተላለፊያ ባች ስፋትን ጀምር |
| 0x2A | አስተላልፍ_V1_BATCH_END | – | `u64=0` | `G_transfer` | የተከማቸ የFASTPQ ዝውውር ባች |
| 0x2B | አስተላልፍ_V1_BATCH_APPLY | `r10=&NoritoBytes(TransferAssetBatch)` | `u64=0` | `G_transfer` | Norito የተመሰጠረ ባች በነጠላ syscall ያመልክቱ |
| 0x25 | NFT_MINT_ASSET | `&NftId`, `&AccountId(owner)` | `u64=0` | `G_nft_mint_asset` | አዲስ NFT ይመዘግባል |
| 0x26 | NFT_TRANSFER_ASSET | `&AccountId(from)`፣ `&NftId`፣ `&AccountId(to)` | `u64=0` | `G_nft_transfer_asset` | የNFT ባለቤትነትን ያስተላልፋል |
| 0x27 | NFT_SET_METADATA | `&NftId`, `&Json` | `u64=0` | `G_nft_set_metadata` | NFT ሜታዳታ ያዘምናል |
| 0x28 | NFT_BURN_ASSET | `&NftId` | `u64=0` | `G_nft_burn_asset` | NFT ያቃጥላል (ያጠፋል) |
| 0xA1 | SMARTCONTRACT_EXECUTE_QUERY| `r10=&NoritoBytes(QueryRequest)` | `r10=ptr (&NoritoBytes(QueryResponse))` | `G_scq + per_item*items + per_byte*bytes(resp)` | ተደጋጋሚ መጠይቆች በጊዜ ሂደት ይሮጣሉ; `QueryRequest::Continue` ተቀባይነት አላገኘም |
| 0xA2 | ፍጠር_NFTS_FOR_ALL_ተጠቃሚዎች | – | `u64=count` | `G_create_nfts_for_all` | ረዳት; ባህሪ ያለው || 0xA3 | SET_SMARTCONTRACT_EXECUTION_DEPTH | `depth:u64` | `u64=prev` | `G_set_depth` | አስተዳዳሪ; ባህሪ ያለው |
| 0xA4 | GET_AUTHORITY | – (አስተናጋጁ ውጤቱን ይጽፋል) | `&AccountId`| `G_get_auth` | አስተናጋጁ ጠቋሚውን አሁን ላለው ባለስልጣን ወደ `r10` |
| 0xF7 | አግኝ_MERKLE_PATH | `addr:u64`፣ `out_ptr:u64`፣ አማራጭ `root_out:u64` | `u64=len` | `G_mpath + len` | ዱካ (ቅጠል → ስር) እና አማራጭ ስር ባይት ይጽፋል |
| 0xFA | GET_MERKLE_COMPACT | `addr:u64`፣ `out_ptr:u64`፣ አማራጭ `depth_cap:u64`፣ አማራጭ `root_out:u64` | `u64=depth` | `G_mpath + depth` | `[u8 depth][u32 dirs_le][u32 count][count*32 siblings]` |
| 0xFF | GET_REGISTER_MERKLE_COMPACT| `reg_index:u64`፣ `out_ptr:u64`፣ አማራጭ `depth_cap:u64`፣ አማራጭ `root_out:u64` | `u64=depth` | `G_mpath + depth` | ለመመዝገቢያ ቁርጠኝነት ተመሳሳይ የታመቀ አቀማመጥ |

ጋዝ ማስፈጸሚያ
- CoreHost የቤተኛውን የ ISI መርሃ ግብር በመጠቀም ለ ISI syscalls ተጨማሪ ጋዝ ያስከፍላል; FASTPQ ባች ዝውውሮች በአንድ ግቤት ይከፍላሉ።
- ZK_VERIFY syscals ሚስጥራዊውን የማረጋገጫ ጋዝ መርሃ ግብር (ቤዝ + የማረጋገጫ መጠን) እንደገና ይጠቀማሉ።
- SMARTCONTRACT_EXECUTE_QUERY ቤዝ + በንጥል + በባይት; መደርደር በንጥል ዋጋን ያበዛል እና ያልተደረደሩ ማካካሻዎች የንጥል ቅጣት ይጨምራሉ።

ማስታወሻዎች
- ሁሉም የጠቋሚ ክርክሮች የ Norito TLV ፖስታዎችን በ INPUT ክልል ውስጥ ይጠቅሳሉ እና በመጀመሪያ ማጣቀሻ (`E_NORITO_INVALID` በስህተት) የተረጋገጡ ናቸው ።
- ሁሉም ሚውቴሽን የሚተገበረው በIroha መደበኛ ፈጻሚ (በ`CoreHost` በኩል)፣ በቀጥታ በVM አይደለም።
- ትክክለኛ የጋዝ ቋሚዎች (`G_*`) በንቁ የጋዝ መርሃ ግብር ይገለጻል; `ivm.md` ይመልከቱ።

ስህተቶች
- `E_SCALL_UNKNOWN`: syscall ቁጥር ለገባሪው `abi_version` አልታወቀም።
- የግቤት ማረጋገጫ ስህተቶች እንደ VM ወጥመዶች ይሰራጫሉ (ለምሳሌ፣ `E_NORITO_INVALID` ለተበላሹ TLVs)።

ተሻጋሪ ማጣቀሻዎች
- አርክቴክቸር እና ቪኤም ትርጓሜ፡ `ivm.md`
- ቋንቋ እና አብሮ የተሰራ ካርታ: `docs/source/kotodama_grammar.md`

የትውልድ ማስታወሻ
- የተሟላ የሲስካል ቋሚዎች ዝርዝር ከምንጩ ሊመነጭ ይችላል-
  - `make docs-syscalls` → `docs/source/ivm_syscalls_generated.md` ይጽፋል
  - `make check-docs` → የመነጨው ሠንጠረዥ ወቅታዊ መሆኑን ያረጋግጣል (በ CI ውስጥ ጠቃሚ)
- ከላይ ያለው ንኡስ ስብስብ ውልን ለሚመለከቱ ስልቶች የተስተካከለ እና የተረጋጋ ጠረጴዛ ሆኖ ይቆያል።

## አስተዳዳሪ/ሚና TLV ምሳሌዎች (ሞክ አስተናጋጅ)

ይህ ክፍል በፈተናዎች ውስጥ ጥቅም ላይ የሚውሉትን የ TLV ቅርጾችን እና አነስተኛውን የJSON ክፍያዎችን በአስመሳይ WSV አስተናጋጅ የተቀበሏቸውን የአስተዳዳሪ ስታይል ስልቶችን ይመዘግባል። ሁሉም የጠቋሚ ነጋሪ እሴቶች ጠቋሚ-ABI (Norito TLV ፖስታዎች በINPUT ውስጥ ይቀመጣሉ) ይከተላሉ። የምርት አስተናጋጆች የበለጸጉ ንድፎችን ሊጠቀሙ ይችላሉ; እነዚህ ምሳሌዎች ዓይነቶችን እና መሰረታዊ ቅርጾችን ግልጽ ለማድረግ ዓላማ አላቸው.- REGISTER_PEER / UNREGISTER_PEER
  - Args: `r10=&Json`
  - ምሳሌ JSON: `{ "peer": "peer-id-or-info" }`
  - CoreHost note: `REGISTER_PEER` የ `RegisterPeerWithPop` JSON ነገር ከ `peer` + `pop` ባይት (አማራጭ `activation_at`፣ `activation_at`፣ Norito) ይጠብቃል `UNREGISTER_PEER` የአቻ-መታወቂያ ሕብረቁምፊ ወይም `{ "peer": "..." }` ይቀበላል።

- CREATE_TRIGGER / REMOVE_TRIGGER / SET_TRIGGER_ENABLED
  - CREATE_TRIGGER፡-
    - Args: `r10=&Json`
    ዝቅተኛው JSON፡ `{ "name": "t1" }` (በማሾፍ ችላ የተባሉ ተጨማሪ መስኮች)
  - REMOVE_TRIGGER:
    - Args: `r10=&Name` (ቀስቃሽ ስም)
  - SET_TRIGGER_ENABLED፡-
    - Args፡ `r10=&Name`፣ `r11=enabled:u64` (0 = ተሰናክሏል፣ ዜሮ ያልሆነ = ነቅቷል)
  - CoreHost ማስታወሻ: `CREATE_TRIGGER` ሙሉ ቀስቅሴ ዝርዝር ይጠብቃል (base64 Norito `Trigger` ሕብረቁምፊ ወይም
    `{ "id": "<trigger_id>", "action": ... }` ከ `action` ጋር እንደ ቤዝ64 Norito `Action` ሕብረቁምፊ ወይም
    JSON ነገር)፣ እና `SET_TRIGGER_ENABLED` የመቀስቀሻ ሜታዳታ ቁልፍን `__enabled` ይቀየራል (ጎደለ
    ነባሪ ነቅቷል)።

- ሚናዎች፡ CREATE_ROLE / DELETE_ROLE / GRANT_ROLE / REVOKE_ROLE
  - ፍጠር_ROLE፡-
    - አርግስ፡ `r10=&Name` (የሚና ስም)፣ `r11=&Json` (ፈቃዶች ተቀምጠዋል)
    - JSON አንዱን ቁልፍ `"perms"` ወይም `"permissions"` ይቀበላል፣ እያንዳንዱ የፍቃድ ስም ድርድር።
    - ምሳሌዎች፡-
      - `{ "perms": [ "mint_asset:rose#wonder" ] }`
      - `{ "permissions": [ "read_assets:<katakana-i105-account-id>", "transfer_asset:rose#wonder" ] }`
    - የሚደገፉ የፍቃድ ስም ቅድመ ቅጥያዎች በፌዝ፡-
      - `register_domain`፣ `register_account`፣ `register_asset_definition`
      - `read_assets:<account_id>`
      - `mint_asset:<asset_definition_id>`
      - `burn_asset:<asset_definition_id>`
      - `transfer_asset:<asset_definition_id>`
  ሰርዝ_ROLE፡-
    - Args: `r10=&Name`
    - ማንኛውም መለያ አሁንም ይህ ሚና ከተመደበ አይሳካም።
  - ግራንት_ሮል / REVOKE_ROLE፡
    - አርግስ፡ `r10=&AccountId` (ርዕሰ ጉዳይ)፣ `r11=&Name` (የሚና ስም)
  - CoreHost ማስታወሻ፡ ፍቃድ JSON ሙሉ `Permission` ነገር (`{ "name": "...", "payload": ... }`) ወይም ሕብረቁምፊ (የክፍያ ነባሪዎች ለ `null`) ሊሆን ይችላል; `GRANT_PERMISSION`/`REVOKE_PERMISSION` `&Name` ወይም `&Json(Permission)` መቀበል።

- ኦፕስ (ጎራ/መለያ/ንብረት) ከምዝገባ ያውጡ፡ ተለዋዋጮች (ማሾፍ)
  - መለያዎች ወይም የንብረት መግለጫዎች በጎራው ውስጥ ካሉ UNREGISTER_DOMAIN (`r10=&DomainId`) አይሳካም።
  - UNREGISTER_ACCOUNT (`r10=&AccountId`) መለያው ዜሮ ያልሆኑ ቀሪ ሒሳቦች ካሉት ወይም የኤንኤፍቲዎች ባለቤት ከሆነ አይሳካም።
  - UNREGISTER_ASSET (`r10=&AssetDefinitionId`) ለንብረቱ ምንም ቀሪ ሒሳብ ካለ አይሳካም።

ማስታወሻዎች
- እነዚህ ምሳሌዎች በፈተናዎች ውስጥ ጥቅም ላይ የዋለውን የማስመሰል WSV አስተናጋጅ ያንፀባርቃሉ። እውነተኛ መስቀለኛ መንገድ አስተናጋጆች የበለጸጉ የአስተዳዳሪ ንድፎችን ሊያጋልጡ ወይም ተጨማሪ ማረጋገጫ ሊፈልጉ ይችላሉ። የጠቋሚው-ABI ህጎቹ አሁንም ተፈጻሚ ይሆናሉ፡ TLVs በINPUT፣ version=1፣ አይነት መታወቂያዎች መዛመድ አለባቸው እና የመጫኛ ሃሽ መረጋገጥ አለባቸው።