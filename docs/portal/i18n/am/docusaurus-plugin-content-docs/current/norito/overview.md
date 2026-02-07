---
lang: am
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito አጠቃላይ እይታ

Norito በ Iroha ላይ ጥቅም ላይ የሚውለው ባለ ሁለትዮሽ ተከታታይ ንብርብር ነው፡ እንዴት ውሂብን ይገልጻል
አወቃቀሮች በሽቦው ላይ ተቀምጠዋል, በዲስክ ላይ ይቆያሉ እና በመካከላቸው ይለዋወጣሉ
ኮንትራቶች እና አስተናጋጆች. በስራ ቦታ ላይ ያለ እያንዳንዱ ሳጥን በNorito ፈንታ ይተማመናል።
`serde` ስለዚህ በተለያየ ሃርድዌር ላይ ያሉ እኩዮች ተመሳሳይ ባይት ያመርታሉ።

ይህ አጠቃላይ እይታ ዋናዎቹን ክፍሎች እና ከቀኖናዊ ማጣቀሻዎች ጋር አገናኞችን ያጠቃልላል።

## አርክቴክቸር በጨረፍታ

- ** አርእስት + ክፍያ ** - እያንዳንዱ Norito መልእክት በባህሪ-ድርድር ይጀምራል
  ራስጌ (ባንዲራዎች፣ ቼክተም) የተከተለ ባዶ ጭነት። የታሸጉ አቀማመጦች እና
  መጭመቅ በርዕስ ቢትስ በኩል ይደራደራል።
- ** ቆራጥ ኢንኮዲንግ *** - `norito::codec::{Encode, Decode}` ተግባራዊ ያድርጉ
  ባዶ ኢንኮዲንግ. ተመሳሳዩን አቀማመጥ እንደገና ጥቅም ላይ የሚውለው የተጫኑ ጭነቶችን በራስጌዎች ውስጥ በሚጠቅልበት ጊዜ ነው።
  ማሽኮርመም እና መፈረም ቆራጥነት ይቀራል።
- ** Schema + የሚያመነጨው *** - `norito_derive` `Encode`፣ `Decode`ን፣ እና ያመነጫል።
  `IntoSchema` አተገባበር። የታሸጉ መዋቅሮች/ተከታታይ በነባሪ ነቅተዋል።
  እና በ `norito.md` ውስጥ ተመዝግቧል.
- ** መልቲኮዴክ መዝገብ *** - ለሃሽ ፣ ለቁልፍ ዓይነቶች እና ለክፍያ መለያዎች
  ገላጭዎች በ `norito::multicodec` ይኖራሉ። ስልጣን ያለው ሰንጠረዥ ነው።
  በ `multicodec.md` ውስጥ ተጠብቆ ቆይቷል።

## መሳሪያ

| ተግባር | ትዕዛዝ / API | ማስታወሻ |
| --- | --- | --- |
| ራስጌ/ክፍል መርምር | `ivm_tool inspect <file>.to` | ABI ስሪትን፣ ባንዲራዎችን እና የመግቢያ ነጥቦችን ያሳያል። |
| ዝገት ውስጥ ኢንኮድ/ዲኮድ | `norito::codec::{Encode, Decode}` | ለሁሉም ዋና የውሂብ-ሞዴል ዓይነቶች የተተገበረ። |
| JSON interop | `norito::json::{to_json_pretty, from_json}` | ቆራጥ JSON በNorito እሴቶች የተደገፈ። |
| ሰነዶችን/መግለጫዎችን ፍጠር | `norito.md`, `multicodec.md` | በሪፖ ሥር ውስጥ ያለው የእውነት ምንጭ ሰነድ። |

##የልማት የስራ ሂደት

1. ** ጥቅማጥቅሞችን ያክሉ *** - ለአዲስ መረጃ `#[derive(Encode, Decode, IntoSchema)]` ይምረጡ
   መዋቅሮች. በጣም አስፈላጊ ካልሆነ በስተቀር በእጅ የተጻፉ ተከታታይ ፊልሞችን ያስወግዱ።
2. ** የታሸጉ አቀማመጦችን ያረጋግጡ *** - `cargo test -p norito` (እና የታሸገውን ይጠቀሙ)
   ባህሪ ማትሪክስ በ I18NI0000027X) አዲስ ለማረጋገጥ
   አቀማመጦች ተረጋግተው ይቆያሉ።
3. ** ሰነዶችን ያድሱ *** - ኢንኮዲንግ ሲቀየር፣ `norito.md` ያዘምኑ እና
   የመልቲኮድ ሠንጠረዥ፣ ከዚያ የፖርታል ገጾቹን ያድሱ (`/reference/norito-codec`
   እና ይህ አጠቃላይ እይታ).
4. ** ሙከራዎችን አቆይ Norito-መጀመሪያ ** - የውህደት ፈተናዎች Norito JSON መጠቀም አለባቸው
   ከ `serde_json` ይልቅ ረዳቶች ስለዚህ እንደ ምርት ተመሳሳይ መንገዶችን ይጠቀማሉ።

## ፈጣን አገናኞች

- ዝርዝር፡ [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- መልቲኮድ ስራዎች፡ [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- የባህሪ ማትሪክስ ስክሪፕት: `scripts/run_norito_feature_matrix.sh`
- የታሸጉ-አቀማመጥ ምሳሌዎች: `crates/norito/tests/`

ይህንን አጠቃላይ እይታ ከፈጣን ጅምር መመሪያ (`/norito/getting-started`) ጋር ያጣምሩ
Norito የሚጠቀም ባይትኮድ የማጠናቀር እና የማስኬድ ሂደት
ሸክሞች.