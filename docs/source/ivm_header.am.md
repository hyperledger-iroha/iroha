---
lang: am
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T19:17:13.237630+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ባይትኮድ ራስጌ


አስማት
- 4 ባይት፡ ASCII `IVM\0` በ0 ማካካሻ።

አቀማመጥ (የአሁኑ)
- ማካካሻዎች እና መጠኖች (ጠቅላላ 17 ባይት)
  - 0..4: አስማት `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8` (የባህሪ ቢት፤ ከታች ይመልከቱ)
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64` (ትንሽ ኢንዲያን)
  - 16: `abi_version: u8`

ሁነታ ቢት
- `ZK = 0x01`፣ `VECTOR = 0x02`፣ `HTM = 0x04` (የተያዘ/የተያዘ)።

መስኮች (ትርጉም)
- `abi_version`: syscall ሠንጠረዥ እና የጠቋሚ-ABI ንድፍ ስሪት።
- `mode`፡ ለZK መፈለጊያ/VECTOR/HTM የባህሪ ቢት።
- `vector_length`: ምክንያታዊ የቬክተር ርዝመት ለ vector ops (0 → ያልተቀናበረ)።
- `max_cycles`: ማስፈጸሚያ ንጣፍ የታሰረ በ ZK ሁነታ እና መግቢያ ላይ ጥቅም ላይ ይውላል.

ማስታወሻዎች
- ውስጣዊነት እና አቀማመጥ በአተገባበሩ የተገለጹ እና ከ `version` ጋር የተሳሰሩ ናቸው. ከላይ ያለው በገመድ ላይ ያለው አቀማመጥ በ`crates/ivm_abi/src/metadata.rs` ያለውን ትግበራ ያንፀባርቃል።
- አነስተኛ አንባቢ ለአሁኑ ቅርሶች በዚህ አቀማመጥ ላይ ሊተማመን ይችላል እና የወደፊት ለውጦችን በ `version` gating በኩል ማስተናገድ አለበት።
- የሃርድዌር ማጣደፍ (SIMD/Metal/CUDA) በአንድ አስተናጋጅ መርጦ መግባት ነው። የሩጫ ጊዜው `AccelerationConfig` እሴቶችን ከ `iroha_config` ያነባል፡ `enable_simd` የውሸት ውድቀትን ያስገድዳል፣ `enable_metal` እና `enable_metal` እና `enable_cuda` በር የየራሳቸው ጥቅማጥቅሞች ሲቀናጁም ይተገበራሉ። VM ከመፈጠሩ በፊት `ivm::set_acceleration_config`።
- የሞባይል ኤስዲኬዎች (አንድሮይድ/ስዊፍት) ተመሳሳይ ቁልፎችን ይለጥፋሉ; `IrohaSwift.AccelerationSettings`
  `connect_norito_set_acceleration_config` ይደውላል ስለዚህ macOS/iOS ግንቦች ወደ ብረት መርጠው እንዲገቡ /
  NEON ቆራጥ ውድቀቶችን በሚጠብቅበት ጊዜ።
- ኦፕሬተሮች `IVM_DISABLE_METAL=1` ወይም `IVM_DISABLE_CUDA=1` ወደ ውጭ በመላክ ለምርመራ ልዩ የጀርባ ማከሚያዎችን ማስገደድ ይችላሉ። እነዚህ አካባቢ መሻሮች ከማዋቀር በፊት ይቀድማሉ እና ቪኤምን በሚወስነው የሲፒዩ መንገድ ላይ ያቆዩታል።

የሚበረክት ግዛት ረዳቶች እና ABI ወለል
- ዘላቂው የስቴት አጋዥ ሲስክሎች (0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* እና JSON/SCHEMA ኢንኮድ/ዲኮድ) የV1 ABI አካል ናቸው እና በIVM.37X ስሌት ውስጥ ተካትተዋል።
- የCoreHost ሽቦዎች STATE_{GET,SET,DEL} ወደ WSV የሚደገፍ የሚበረክት ዘመናዊ ኮንትራት ሁኔታ; የዴቭ/የሙከራ አስተናጋጆች ተደራቢዎችን ወይም የአካባቢ ጽናት ሊጠቀሙ ይችላሉ ነገር ግን ተመሳሳይ የሚታይ ባህሪን መጠበቅ አለባቸው።

ማረጋገጫ
- የመስቀለኛ መንገድ መግቢያ `version_major = 1` እና `version_minor = 0` ራስጌዎችን ብቻ ይቀበላል።
- `mode` የታወቁ ቢትስ ብቻ መያዝ አለበት፡ `ZK`፣ `VECTOR`፣ `HTM` (ያልታወቁ ቢትስ ውድቅ ናቸው)።
- `vector_length` ምክር ነው እና `VECTOR` ቢት ካልተዋቀረ ዜሮ ያልሆነ ሊሆን ይችላል። መግባቱ ከፍተኛ ወሰን ብቻ ነው የሚያስፈጽመው።
- የሚደገፉ `abi_version` እሴቶች: መጀመሪያ መለቀቅ `1` (V1) ብቻ ይቀበላል; ሌሎች እሴቶች በመግቢያው ላይ ውድቅ ይደረጋሉ።

### ፖሊሲ (የተፈጠረ)
የሚከተለው የመመሪያ ማጠቃለያ ከአተገባበሩ የመነጨ ነው እና በእጅ መታረም የለበትም።<!-- BEGIN GENERATED HEADER POLICY -->
| መስክ | ፖሊሲ |
|---|---|
| ስሪት_ዋና | 1 |
| ስሪት_አነስተኛ | 0 |
| ሁነታ (የታወቁ ቢት) | 0x07 (ZK=0x01፣ VECTOR=0x02፣ HTM=0x04) |
| አቢ_ስሪት | 1 |
| የቬክተር_ርዝመት | 0 ወይም 1..=64 (ምክር፤ ከ VECTOR ቢት ነጻ) |
<!-- END GENERATED HEADER POLICY -->

### ABI Hashes (የተፈጠረ)
የሚከተለው ሰንጠረዥ ከአፈፃፀሙ የመነጨ ሲሆን ለሚደገፉ ፖሊሲዎች ቀኖናዊ `abi_hash` እሴቶችን ይዘረዝራል።

<!-- BEGIN GENERATED ABI HASHES -->
| ፖሊሲ | አቢ_ሀሽ (ሄክስ) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- ጥቃቅን ዝመናዎች ከ `feature_bits` እና የተያዘ የኦፕኮድ ቦታ መመሪያዎችን ሊጨምሩ ይችላሉ; ዋና ዋና ዝመናዎች ከፕሮቶኮል ማሻሻያ ጋር ብቻ ኢንኮዲንግ ሊለውጡ ወይም ሊያስወግዱ/እንደገና ሊጠቀሙበት ይችላሉ።
- Syscall ክልሎች የተረጋጋ ናቸው; ለገቢር `abi_version` ያልታወቀ `E_SCALL_UNKNOWN`.
- የጋዝ መርሃ ግብሮች ከ `version` ጋር የተሳሰሩ እና በለውጥ ላይ ወርቃማ ቬክተሮችን ይፈልጋሉ.

ቅርሶችን መመርመር
- ለራስጌ መስኮች የተረጋጋ እይታ `ivm_tool inspect <file.to>` ይጠቀሙ።
- ለልማት፣ ምሳሌዎች/ የተገነቡ ቅርሶችን የሚፈትሽ ትንሽ የMakefile ዒላማ `examples-inspect` ያካትታሉ።

ምሳሌ (ዝገት)፡ አነስተኛ አስማት + መጠን ማረጋገጥ

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

ማሳሰቢያ፡ ከአስማት ያለፈ ትክክለኛ የራስጌ አቀማመጥ ተዘጋጅቷል እና ተፈጻሚነት-የተገለፀ ነው፤ ለተረጋጉ የመስክ ስሞች እና እሴቶች `ivm_tool inspect` ይመርጣሉ።