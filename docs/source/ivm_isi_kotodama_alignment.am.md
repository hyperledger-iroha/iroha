---
lang: am
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T19:17:13.238594+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ የውሂብ ሞዴል ⇄ Kotodama — የአሰላለፍ ግምገማ

ይህ ሰነድ የ Iroha ቨርቹዋል ማሽን (IVM) መመሪያ እንዴት እንደተቀመጠ እና syscall የወለል ካርታ ወደ Iroha ልዩ መመሪያዎች (ISI) እና `iroha_data_model` እና እንዴት Kotodama. የወቅቱ ክፍተቶችን በመለየት ተጨባጭ ማሻሻያዎችን ያቀርባል ስለዚህም አራቱ ንብርብሮች በቆራጥነት እና ergonomically አንድ ላይ ይጣጣማሉ።

በባይቴኮድ ኢላማ ላይ ማስታወሻ፡ Kotodama ስማርት ኮንትራቶች ወደ Iroha ቨርቹዋል ማሽን (IVM) ባይትኮድ (`.to`) ያጠናቅራል። “risc5”/RISC-Vን እንደ ራሱን የቻለ አርክቴክቸር አላነጣጠሩም። እዚህ የተጠቀሱ ማንኛውም RISC-V-እንደ ኢንኮዲንግ የIVM የተቀላቀሉ የማስተማሪያ ቅርጸቶች አካል ናቸው እና የአተገባበር ዝርዝር ሆነው ይቆያሉ።

## ወሰን እና ምንጮች
- IVM፡ `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` እና `crates/ivm/docs/*`።
- ISI/የመረጃ ሞዴል፡ `crates/iroha_data_model/src/isi/*`፣ `crates/iroha_core/src/smartcontracts/isi/*`፣ እና ሰነዶች `docs/source/data_model_and_isi_spec.md`።
- Kotodama፡ `crates/kotodama_lang/src/*`፣ ሰነዶች በ`crates/ivm/docs/*`።
- ኮር ውህደት: `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

ቃላቶች
- “ISI” በፈጻሚው (ለምሳሌ፣ RegisterAccount፣ Mint፣ Transfer) በኩል የዓለምን ሁኔታ የሚቀይሩ አብሮገነብ የትምህርት ዓይነቶችን ያመለክታል።
- “Syscall” የሚያመለክተው IVM `SCALL` ባለ 8-ቢት ቁጥር ያለው ለሂሳብ አሠራሩ አስተናጋጁን ነው።

---

## የአሁኑ ካርታ ስራ (እንደተተገበረ)

### IVM መመሪያዎች
- አርቲሜቲክ, ማህደረ ትውስታ, የቁጥጥር ፍሰት, ክሪፕቶ, ቬክተር እና ZK አጋዥዎች በ `instruction.rs` ውስጥ ተገልጸዋል እና በ `ivm.rs` ውስጥ ተተግብረዋል. እነዚህ እራሳቸውን የቻሉ እና የሚወስኑ ናቸው; የፍጥነት መንገዶች (SIMD/Metal/CUDA) የሲፒዩ ውድቀት አላቸው።
- የስርዓት/የአስተናጋጅ ወሰን በ`SCALL` (opcode 0x60) በኩል ነው። ቁጥሮች በ `syscalls.rs` ውስጥ ተዘርዝረዋል እና የአለም ኦፕሬሽኖችን (ይመዝገቡ/ያውጡ ጎራ/አካውንት/ንብረት፣ ሚንት/ማቃጠል/ማስተላለፊያ፣ ሚና/ፈቃድ ኦፕስ፣ ቀስቅሴዎች) እና ረዳቶችን (`GET_PRIVATE_INPUT`፣ `COMMIT_OUTPUT`፣ `COMMIT_OUTPUT`፣ `COMMIT_OUTPUT`፣ `COMMIT_OUTPUT`፣ Kotodama ወዘተ)።

### አስተናጋጅ ንብርብር
- ባህሪ `IVMHost::syscall(number, &mut IVM)` በ `host.rs` ውስጥ ይኖራል።
- DefaultHost የሚተገበረው የመሪ ያልሆኑ ረዳቶችን ብቻ ነው (አሎክ፣ ክምር ዕድገት፣ ግብዓቶች/ውጤቶች፣ የZK ማረጋገጫ አጋዥዎች፣ የባህሪ ግኝት) - የዓለም ግዛት ሚውቴሽን አይሰራም።
- በ`mock_wsv.rs` ውስጥ የንብረት ክዋኔዎችን (ማስተላለፍ/mint/burn) ወደ ትንሽ ማህደረ ትውስታ WSV በማስታወቂያ ውስጥ `AccountId`/`AssetDefinitionId` በማስታወቂያ x10..x13.

### ISI እና የውሂብ ሞዴል
- አብሮገነብ የ ISI ዓይነቶች እና ትርጓሜዎች በ`iroha_core::smartcontracts::isi::*` ውስጥ ተተግብረዋል እና በ `docs/source/data_model_and_isi_spec.md` ውስጥ ተመዝግበዋል ።
- `InstructionBox` የተረጋጋ "የሽቦ መታወቂያዎች" እና Norito ኢንኮዲንግ ያለው መዝገብ ይጠቀማል; ቤተኛ የማስፈጸሚያ መላክ በዋና ውስጥ ያለው የአሁኑ ኮድ መንገድ ነው።### የ IVM ኮር ውህደት
- `State::execute_trigger(..)` የተሸጎጠውን `IVM` ን ይዘጋዋል፣ `CoreHost::with_accounts_and_args` አያይዞ `load_program` + `run` ይደውላል።
- `CoreHost` `IVMHost`ን ይተገብራል፡ ሁኔታዊ syscalls በጠቋሚ-ABI TLV አቀማመጥ ዲኮድ ተደርገዋል፣ አብሮ በተሰራው ISI (`InstructionBox`) ተቀርጿል፣ እና ተሰልፈዋል። ቪኤም አንዴ ከተመለሰ፣ አስተናጋጁ ፍቃዶች፣ ተለዋዋጮች፣ ክስተቶች እና ቴሌሜትሪዎች ከአገሬው ተወላጅ አፈጻጸም ጋር ተመሳሳይ ሆነው እንዲቆዩ አስተናጋጁ እነዚያን ISI ለመደበኛው አስፈፃሚ ይሰጣል። WSVን የማይነኩ አጋዥ ስካሎች አሁንም ለ`DefaultHost` ውክልና ይሰጣሉ።
- `executor.rs` አብሮ የተሰራ ISI ቤተኛ መስራቱን ቀጥሏል; አረጋጋጭ ፈፃሚውን ወደ IVM ማሸጋገር የወደፊት ስራ ነው።

### Kotodama → IVM
- Frontend ቁርጥራጮች አሉ (lexer/parser/ትንሽ የትርጉም/IR/regalloc)።
- Codegen (`kotodama::compiler`) የIVM ኦፕስ ስብስብ ያመነጫል እና ለንብረት ስራዎች `SCALL` ይጠቀማል፡
  - `MintAsset` → አዘጋጅ x10=መለያ፣ x11=ንብረት፣ x12=&NoritoBytes(ቁጥር); `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` ተመሳሳይ (እንደ NoritoBytes(ቁጥር) ጠቋሚ የተላለፈ መጠን)።
- Demos `koto_*_demo.rs` `WsvHost` በመጠቀም የኢንቲጀር ኢንቲጀር ለፈጣን ሙከራ መታወቂያ ላይ ተዘጋጅቷል።

---

## ክፍተቶች እና አለመዛመድ

1) የኮር አስተናጋጅ ሽፋን እና እኩልነት
- ሁኔታ፡ `CoreHost` አሁን በኮር ውስጥ አለ እና ብዙ የመመዝገቢያ ስልቶችን በመደበኛ ዱካ የሚፈፀሙ ወደ ISI ይተረጉማል። ሽፋን አሁንም አልተጠናቀቀም (ለምሳሌ፣ አንዳንድ ሚናዎች/ፍቃዶች/ቀስቃሾች stubs ናቸው) እና የተመሳሳይነት ፈተናዎች የታሰሩ ISI እንደ ሀገርኛ አፈፃፀም ተመሳሳይ ሁኔታ/ክስተቶችን እንደሚያመጣ ዋስትና ያስፈልጋል።

2) Syscall surface vs. ISI/ዳታ ሞዴል ስያሜ እና ሽፋን
- ኤንኤፍቲዎች፡ ሲሲካል አሁን ከ`iroha_data_model::nft` ጋር የተስተካከሉ የ`SYSCALL_NFT_*` ስሞችን ያጋልጣሉ።
- ሚናዎች/ፍቃዶች/ቀስቃሾች፡ syscall ዝርዝር አለ ነገር ግን እያንዳንዱን ጥሪ ከኮንክሪት ISI ጋር የሚያስተሳስር የማጣቀሻ ትግበራ ወይም የካርታ ሠንጠረዥ የለም።
- መለኪያዎች/ትርጓሜዎች፡- አንዳንድ ሲስካሎች የመለኪያ ኢንኮዲንግ (የተተየቡ መታወቂያዎች እና ጠቋሚዎች) ወይም የጋዝ ፍቺን አይገልጹም። የ ISI ትርጓሜዎች በደንብ የተገለጹ ናቸው።

3) የተተየበው ውሂብ በVM/አስተናጋጅ ድንበር ላይ ለማለፍ ABI
- ጠቋሚ-ABI TLVዎች አሁን በ`CoreHost` (`decode_tlv_typed`) ውስጥ ዲኮድ ተደርገዋል፣ ይህም ለመታወቂያዎች፣ ለሜታዳታ እና ለJSON ጭነት ጭነት የሚወስን መንገድ ይሰጣል። እያንዳንዱ syscall ሰነዶች የሚጠበቁትን የጠቋሚ ዓይነቶች እና Kotodama ትክክለኛ TLVዎችን እንደሚያወጣ ለማረጋገጥ ይቀራል (መመሪያው አንድን ዓይነት ውድቅ ሲያደርግ የስህተት አያያዝን ጨምሮ)።

4) የጋዝ እና የስህተት ካርታ ወጥነት
- IVM ኦፕኮዶች በአንድ ኦፕ ጋዝ ያስከፍላሉ; CoreHost አሁን ተጨማሪ ጋዝ ለአይኤስአይ ሲሲካል ይመልሳል የሀገር በቀል የጋዝ መርሃ ግብር (የባች ማስተላለፎችን እና የአቅራቢውን ISI ድልድይ ጨምሮ) እና ZK ሚስጥራዊውን የጋዝ መርሃ ግብር እንደገና መጠቀማቸውን ያረጋግጣል። DefaultHost አሁንም ለሙከራ ሽፋን አነስተኛ ወጪዎችን ይይዛል።
- የስህተት ገጽታዎች ይለያያሉ: IVM ይመልሳል `VMError::{OutOfGas,PermissionDenied,...}`; ISI `InstructionExecutionError` ምድቦችን ይመልሳል (`Find` ፣ `Repetition` ፣ `InvariantViolation` ፣ `Math` ፣ `Type` ፣ Kotodama)።5) በተጣደፉ መንገዶች ላይ ቁርጠኝነት
- IVM ቬክተር/CUDA/ሜታል የሲፒዩ ውድቀት አላቸው፣ነገር ግን አንዳንድ ኦፕስ ተጠብቀው ይቆያሉ (`SETVL`፣ PARBEGIN/PAREND) እና እስካሁን የመወሰኛ አንኳር አካል አይደሉም።
- የመርክል ዛፎች በIVM እና በመስቀለኛ መንገድ (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`) መካከል ይለያያሉ - አንድ የማዋሃድ ንጥል አስቀድሞ በ `roadmap.md` ውስጥ ይታያል።

6) Kotodama የቋንቋ ወለል ከታሰበው የመመዝገቢያ ትርጉም ጋር
- ኮምፕሌተር ትንሽ ንዑስ ክፍልን ያስወጣል; አብዛኛዎቹ የቋንቋ ባህሪያት (ግዛት/መዋቅሮች፣ ቀስቅሴዎች፣ ፈቃዶች፣ የተተየቡ ፓራም/ተመላሾች) እስካሁን ከአስተናጋጁ/አይኤስአይ ሞዴል ጋር አልተገናኙም።
- ሲካሎች ለባለሥልጣኑ ህጋዊ መሆናቸውን ለማረጋገጥ ምንም ችሎታ/ውጤት ትየባ የለም።

---

## ምክሮች (የተጨባጭ ደረጃዎች)

### ሀ. የምርት IVM አስተናጋጅ በኮር ውስጥ ይተግብሩ
- `iroha_core::smartcontracts::ivm::host` ሞጁል `ivm::host::IVMHost` ን ጨምር።
- ለእያንዳንዱ syscall በ `ivm::syscalls`:
  - ክርክሮችን በቀኖናዊ ኤቢአይ (ቢን ይመልከቱ)፣ ተዛማጅ አብሮ የተሰራውን ISI ይገንቡ ወይም ተመሳሳዩን ኮር አመክንዮ በቀጥታ ይደውሉ፣ ከ`StateTransaction` ጋር ያስፈጽሙት፣ እና የካርታ ስህተቶችን ወደ IVM መመለሻ ኮድ ይመለሱ።
  - በዋና ውስጥ የተገለጸውን በእያንዳንዱ syscall ሠንጠረዥ በመጠቀም ጋዝ መሙላት (እና ለወደፊቱ አስፈላጊ ከሆነ ለIVM በ `SYSCALL_GET_PARAMETER` ይጋለጣል)። መጀመሪያ ላይ ለእያንዳንዱ ጥሪ ቋሚ ተጨማሪ ጋዝ ከአስተናጋጁ ይመልሱ።
- ክር `authority: &AccountId` እና `&mut StateTransaction` ወደ አስተናጋጁ ስለዚህ የፈቃድ ፍተሻዎች እና ዝግጅቶች ከአገሬው ISI ጋር ተመሳሳይ ናቸው።
- ይህንን አስተናጋጅ ከ`vm.run()` በፊት ለማያያዝ `State::execute_trigger(ExecutableRef::Ivm)` ያዘምኑ እና እንደ ISI ተመሳሳይ `ExecutionStep` ፍቺ ይመልሱ (ክስተቶች ቀድሞውኑ በዋና ውስጥ ተለቅቀዋል፤ ወጥነት ያለው ባህሪ መረጋገጥ አለበት)።

### B. ለተተየቡ እሴቶች የሚወስን VM/አስተናጋጅ ABI ይግለጹ
- ለተዋቀሩ ክርክሮች በቪኤም በኩል Norito ይጠቀሙ፡
  - ጠቋሚዎችን (በ x10..x13, ወዘተ.) ወደ VM ማህደረ ትውስታ ክልሎች Norito-የተመሰጠሩ እሴቶችን እንደ `AccountId`፣ `AssetDefinitionId`፣ `Numeric`፣ I1012000
  - አስተናጋጁ ባይት ያነባል በ`IVM` የማስታወሻ ረዳቶች እና በ Norito (`iroha_data_model` `Encode/Decode` ቀድሞውንም ያገኛል) መፍታት።
- ትክክለኛ መታወቂያዎችን ወደ ኮድ/ቋሚ ገንዳዎች ለመደርደር ወይም የጥሪ ፍሬሞችን በማህደረ ትውስታ ለማዘጋጀት በKotodama codegen ውስጥ አነስተኛ ረዳቶችን ይጨምሩ።
- መጠኖች `Numeric` እና እንደ NoritoBytes ጠቋሚዎች ያልፋሉ; ሌሎች ውስብስብ ዓይነቶችም በጠቋሚ ያልፋሉ.
- ይህንን በ `crates/ivm/docs/calling_convention.md` ውስጥ ይመዝግቡ እና ምሳሌዎችን ያክሉ።### ሐ. የሲሲካል ስያሜ እና ሽፋንን ከአይኤስአይ/ዳታ ሞዴል ጋር አሰልፍ
- ግልጽ ለማድረግ ከNFT ጋር የሚዛመዱ ሲስኮሎችን እንደገና ይሰይሙ፡ ቀኖናዊ ስሞች አሁን የ`SYSCALL_NFT_*` ስርዓተ ጥለት (`SYSCALL_NFT_MINT_ASSET`፣ `SYSCALL_NFT_SET_METADATA`፣ ወዘተ) ይከተላሉ።
- የካርታ ሠንጠረዥን (የሰነድ + ኮድ አስተያየቶችን) ያትሙ፡-
  - መለኪያዎች (ተመዝጋቢዎች እና ጠቋሚዎች)፣ የሚጠበቁ ቅድመ ሁኔታዎች፣ ክስተቶች እና የስህተት ካርታዎች።
  - ጋዝ ክፍያዎች.
- ለእያንዳንዱ አብሮገነብ ISI ከKotodama (ጎራዎች፣ ሒሳቦች፣ ንብረቶች፣ ሚናዎች/ፍቃዶች፣ ቀስቅሴዎች፣ መለኪያዎች) የማይታለፍ መሆን ያለበት syscall እንዳለ ያረጋግጡ። አንድ ISI ልዩ መብት ያለው ሆኖ መቆየት ካለበት፣ ሰነዱ እና በአስተናጋጁ ውስጥ የፍቃድ ፍተሻዎችን ያስፈጽሙ።

### D. ስህተቶችን እና ጋዝን አንድ ማድረግ
- በአስተናጋጁ ውስጥ የትርጉም ንብርብር ያክሉ፡ ካርታ `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` ወደ ተወሰኑ `VMError` ኮዶች ወይም የተራዘመ የውጤት ስምምነት (ለምሳሌ፡ `x10=0/1` አዘጋጅ እና በደንብ የተገለጸ `VMError::HostRejected { code }` ይጠቀሙ)።
- ለ syscals በዋና ውስጥ የጋዝ ጠረጴዛን ማስተዋወቅ; በ IVM ሰነዶች ያንጸባርቁት; ወጪዎች የግቤት መጠን ሊገመቱ የሚችሉ እና ከመድረክ-ገለልተኛ መሆናቸውን ያረጋግጡ።

### ኢ ቆራጥነት እና የጋራ ፕሪሚቲቭ
- የመርክል ዛፍን ውህደት ያጠናቅቁ (የፍኖተ ካርታ ይመልከቱ) እና `ivm::merkle_tree` ወደ `iroha_crypto` በተመሳሳይ ቅጠሎች እና ማረጋገጫዎች ያስወግዱ።
- `SETVL`/PARBEGIN/PAREND` ከጫፍ እስከ ጫፍ የመወሰኛ ቼኮች እና የመወሰን መርሐግብር አዘጋጅ ስትራቴጂ እስካለ ድረስ እንደተጠበቀ ያቆዩ። IVM ዛሬ እነዚህን ፍንጮች ችላ የሚላቸውን ሰነድ።
- የፍጥነት መንገዶች ባይት ለ-ባይት ተመሳሳይ ውጤቶችን ማፍራታቸውን ያረጋግጡ። በማይቻልበት ጊዜ የሲፒዩ ውድቀትን እኩልነት በሚያረጋግጥ ከባህሪያት ጀርባ ይጠብቁ።

### F. Kotodama የማጠናከሪያ ሽቦ
- ኮድጅን ወደ ቀኖናዊው ABI (B.) መታወቂያዎችን እና ውስብስብ መለኪያዎችን ያራዝሙ; ኢንቲጀር → መታወቂያ ማሳያ ካርታዎችን መጠቀም ያቁሙ።
- ከንብረቶች በላይ (ጎራዎች/መለያዎች/ሚናዎች/ፍቃዶች/ቀስቃሾች) በቀጥታ ወደ ISI syscalls ግልጽ በሆነ ስሞች ላይ የተሰሩ የካርታ ስራዎችን ያክሉ።
- የሰዓት አቅም ፍተሻዎችን እና አማራጭ `permission(...)` ማብራሪያዎችን ይጨምሩ። የማይንቀሳቀስ ማረጋገጫ በማይቻልበት ጊዜ ወደ ሩጫ ጊዜ አስተናጋጅ ስህተቶች መመለስ።
- ትንንሽ ኮንትራቶችን የሚያጠናቅቁ እና የሚያስኬዱ የዩኒት ሙከራዎችን በ`crates/ivm/tests/kotodama.rs` ይጨምሩ Norito ክርክሮችን የሚፈታ እና ጊዜያዊ WSV የሚቀይር የሙከራ አስተናጋጅ በመጠቀም።

### G. Documentation እና ገንቢ ergonomics
- `docs/source/data_model_and_isi_spec.md` በ syscall ካርታ ሠንጠረዥ እና በ ABI ማስታወሻዎች ያዘምኑ።
- በ `crates/ivm/docs/` ውስጥ `IVMHost` በእውነተኛ `StateTransaction` ላይ እንዴት እንደሚተገበር የሚገልጽ አዲስ ሰነድ "IVM አስተናጋጅ ውህደት መመሪያ" ያክሉ።
- በ`README.md` እና Kotodama IVM IVM `.to` ባይትኮድ ላይ ያነጣጠረ መሆኑን እና syscalls ወደ አለም መንግስት የሚገቡበት ድልድይ መሆናቸውን በ`README.md` ያብራሩ።

---

## የተጠቆመ የካርታ ስራ ሰንጠረዥ (የመጀመሪያው ረቂቅ)

ተወካይ ንዑስ ስብስብ - በአስተናጋጅ ትግበራ ጊዜ ማጠናቀቅ እና ማስፋፋት።- SYSCALL_REGISTER_DOMAIN(መታወቂያ፡ ptr DomainId) → ISI ይመዝገቡ
- SYSCALL_REGISTER_ACCOUNT(መታወቂያ፡ ptr AccountId) → ISI ይመዝገቡ
- SYSCALL_REGISTER_ASSET(መታወቂያ፡ ptr AssetDefinitionId፣ mintable፡ u8) → ISI ይመዝገቡ
- SYSCALL_MINT_ASSET (መለያ፡ ptr AccountId፣ ንብረት፡ ptr AssetDefinitionId፣ መጠን፡ ptr NoritoBytes(ቁጥር)) → ISI Mint
- SYSCALL_BURN_ASSET(መለያ፡ ptr AccountId፣ ንብረት፡ ptr AssetDefinitionId፣ መጠን፡ ptr NoritoBytes(ቁጥር)) → ISI Burn
- SYSCALL_TRANSFER_ASSET(ከ፡ ptr AccountId፣ ወደ ptr AccountId፣ asset: ptr AssetDefinitionId፣ መጠን፡ ptr NoritoBytes(ቁጥር)) → ISI ማስተላለፍ
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (ክፍት/ክልሉን ዝጋ፤ የግለሰብ ግቤቶች በ`transfer_asset`) ዝቅ ይላሉ።
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) →ኮንትራቶች ከሰንሰለት ውጪ ያሉትን ቃላቶች በተከታታይ ሲያደርጉ ቅድመ-የተመሰጠረ ባች ያስገቡ።
- SYSCALL_NFT_MINT_ASSET(መታወቂያ፡ ptr NftId፣ ባለቤት፡ ptr AccountId) → ISI ይመዝገቡ
- SYSCALL_NFT_TRANSFER_ASSET(ከ፡ ptr AccountId፣ ወደ፡ ptr AccountId፣ id: ptr NftId) → ISI Transfer
- SYSCALL_NFT_SET_METADATA(መታወቂያ፡ ptr NftId፣ ይዘት፡ ptr ሜታዳታ) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(መታወቂያ፡ ptr NftId) → ISI መመዝገብ
- SYSCALL_CREATE_ROLE(መታወቂያ፡ ptr RoleId፣ ሚና፡ ptr Role) → ISI ይመዝገቡ
- SYSCALL_GRANT_ROLE(መለያ፡ ptr AccountId፣ ሚና፡ ptr RoleId) → ISI ግራንት
- SYSCALL_REVOKE_ROLE(መለያ፡ ptr AccountId፣ ሚና፡ ptr RoleId) → ISI መሻር
- SYSCALL_SET_PARAMETER(param: ptr Parameter) → ISI SetParameter

ማስታወሻዎች
- “ptr T” ማለት በVM ማህደረ ትውስታ ውስጥ የተከማቸ ለ Norito ኢንኮድ ባይት ለቲ መዝገብ ውስጥ ያለ ጠቋሚ። አስተናጋጁ ወደ ተጓዳኝ `iroha_data_model` አይነት ይከፍታል።
- የመመለሻ ስብሰባ: የስኬት ስብስቦች `x10=1`; አለመሳካት `x10=0` ያዘጋጃል እና ለሞት የሚዳርጉ ስህተቶች `VMError::HostRejected` ከፍ ሊያደርግ ይችላል።

---

## አደጋዎች እና ልቀት እቅድ
- ለጠባብ ስብስብ (ንብረቶች + አካውንቶች) አስተናጋጅ በገመድ ይጀምሩ እና ያተኮሩ ሙከራዎችን ይጨምሩ።
- አስተናጋጅ የትርጓሜ ብስለት ሳለ ቤተኛ ISI አፈጻጸም እንደ ስልጣን መንገድ አቆይ; ተመሳሳይ የመጨረሻ ውጤቶችን እና ክስተቶችን ለማረጋገጥ ሁለቱንም መንገዶች በ"ጥላ ሁነታ" በሙከራዎች ያሂዱ።
- እኩልነት ከተረጋገጠ በኋላ፣ በምርት ውስጥ IVM አስተናጋጅ ለ IVM ቀስቅሴዎች ያንቁ። በኋላ መደበኛ ግብይቶችን በ IVM በኩል ማዞር ያስቡበት።

---

#አስደናቂ ስራ
- Norito-encoded ጠቋሚዎችን (`crates/ivm/src/kotodama_std.rs`) የሚያልፉትን የKotodama ረዳቶችን ያጠናቅቁ እና በአቀናባሪው CLI በኩል ያድርጓቸው።
- የሲስካል ጋዝ ጠረጴዚን (የረዳት ሲሲካልን ጨምሮ) ያትሙ እና የCoreHost ማስፈጸሚያ/ሙከራዎች ከእሱ ጋር እንዲጣጣሙ ያድርጉ።
- ✅ የጠቋሚውን ክርክር ABIን የሚሸፍኑ የዙር ጉዞ Norito መገልገያዎች; በCI ውስጥ ለተቀመጠው አንጸባራቂ እና NFT ጠቋሚ ሽፋን `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` ይመልከቱ።