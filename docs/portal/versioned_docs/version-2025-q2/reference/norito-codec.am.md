---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 38c0cedd4858656db8562c6612f9981df11a1b2292c05908c3671402ee96be9d
source_last_modified: "2026-01-16T16:25:53.031576+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito ኮዴክ ማጣቀሻ

Norito የ Iroha ቀኖናዊ ተከታታይ ንብርብር ነው። እያንዳንዱ ሽቦ ላይ መልእክት፣ በዲስክ ላይ
የመጫኛ ጭነት፣ እና ተሻጋሪ አካል ኤፒአይ Norito ይጠቀማል ስለዚህ አንጓዎች በተመሳሳይ ባይት ይስማማሉ።
በተለያዩ ሃርድዌር ላይ ሲሰሩ እንኳን. ይህ ገጽ ተንቀሳቃሽ ክፍሎችን ያጠቃልላል
እና በ `norito.md` ውስጥ ወደ ሙሉ ዝርዝር መግለጫው ይጠቁማል.

## ዋና አቀማመጥ

| አካል | ዓላማ | ምንጭ |
| --- | --- | --- |
| **ራስጌ** | የመደራደር ባህሪያት (የታሸጉ መዋቅሮች/ተከታታዮች፣ የታመቁ ርዝመቶች፣የመጭመቂያ ባንዲራዎች) እና CRC64 ቼክ ድምርን ስለሚጨምር የመጫኛ ትክክለኛነት ከኮድ መፍታት በፊት ይፈተሻል። | `norito::header` - ይመልከቱ `norito.md` ("ራስጌ እና ባንዲራዎች", የማከማቻ ስር) |
| ** ባዶ ጭነት** | ለሃሺንግ/ለማነፃፀር ጥቅም ላይ የሚውል ቆራጥ እሴት ኢንኮዲንግ። ለመጓጓዣ ተመሳሳይ አቀማመጥ በርዕሱ ተጠቅልሏል. | `norito::codec::{Encode, Decode}` |
| **መጭመቅ** | አማራጭ Zstd (እና የሙከራ ጂፒዩ ማጣደፍ) በ`compression` ባንዲራ ባይት በኩል ገብሯል። | `norito.md`, "የመጭመቂያ ድርድር" |

ሙሉው የባንዲራ መዝገብ (የታሸገ - የተዋቀረ፣ የታሸገ - ሴክ፣ የታመቀ ርዝመቶች፣ መጭመቂያ)
በ `norito::header::flags` ይኖራል። `norito::header::Flags` ምቾትን ያጋልጣል
ለአሂድ ጊዜ ፍተሻ ቼኮች; የተያዙ የአቀማመጥ ቢትስ በዲኮደሮች ውድቅ ይደረጋሉ።

## ድጋፍ ያግኙ

`norito_derive` መርከቦች `Encode`, `Decode`, `IntoSchema` እና JSON አጋዥ ያስገኛል.
ቁልፍ ስምምነቶች፡-

- የ `packed-struct` ባህሪ ሲሆን የታሸጉ አቀማመጦችን ያዘጋጃል/ኢነም
  ነቅቷል (ነባሪ)። ትግበራ በ`crates/norito_derive/src/derive_struct.rs` ውስጥ ይኖራል
  እና ባህሪው በ `norito.md` ("የታሸጉ አቀማመጦች") ውስጥ ተመዝግቧል.
- የታሸጉ ክምችቶች በ v1 ውስጥ ቋሚ-ስፋት ተከታታይ ራስጌዎችን እና ማካካሻዎችን ይጠቀማሉ; ብቻ
  የየእሴት ርዝመት ቅድመ ቅጥያዎች በ`COMPACT_LEN` ተጎድተዋል።
- JSON አጋዥዎች (`norito::json`) የሚወስነው Norito የሚደገፍ JSON ለ
  ኤፒአይዎችን ይክፈቱ። `norito::json::{to_json_pretty, from_json}` ይጠቀሙ — በጭራሽ `serde_json`።

## መልቲኮዴክ እና መለያ ሰንጠረዦች

Norito መልቲኮዴክ ስራዎቹን በ`norito::multicodec` ውስጥ ያቆያል። ማጣቀሻው
ሠንጠረዥ (ሃሽ፣ ቁልፍ ዓይነቶች፣ የክፍያ ጭነት ገላጭዎች) በ`multicodec.md` ውስጥ ተጠብቀዋል።
በማጠራቀሚያ ስር. አዲስ መለያ ሲታከል፡-

1. `norito::multicodec::registry` አዘምን.
2. ጠረጴዛውን በ `multicodec.md` ውስጥ ያራዝሙ.
3. ካርታውን ከበሉ የታችኛው ተፋሰስ ማሰሪያዎችን (Python/Java) ያድሱ።

## ሰነዶችን እና ዕቃዎችን እንደገና በማመንጨት ላይ

ፖርታሉ በአሁኑ ጊዜ የስድ ማጠቃለያን እያስተናገደ፣ የላይ ያለውን ማርክዳውን ተጠቀም
ምንጮች እንደ እውነት ምንጭ:

- ** ዝርዝር ***: `norito.md`
- ** መልቲኮዴክ ጠረጴዛ ***: `multicodec.md`
- ** ካስማዎች ***: `crates/norito/benches/`
- ** ወርቃማ ሙከራዎች ***: `crates/norito/tests/`

የDocusaurus አውቶሜሽን በቀጥታ ሲሰራ ፖርታሉ በኤ.
መረጃውን ከእነዚህ የሚጎትት የማመሳሰል ስክሪፕት (በ`docs/portal/scripts/` ውስጥ ተከታትሏል)
ፋይሎች. እስከዚያ ድረስ ዝርዝሩ በተቀየረ ቁጥር ይህን ገጽ እራስዎ እንዲሰምር ያድርጉት።