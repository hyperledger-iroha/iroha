---
lang: am
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! የክፍያ v1 ልቀትን ማጽደቅ (ኤስዲኬ ምክር ቤት፣ 2026-04-28)።
//!
//! በ`roadmap.md:M1` የሚፈለገውን የኤስዲኬ ምክር ቤት ውሳኔ ማስታወሻ ይይዛል
//! ኢንክሪፕትድ የተደረገ የክፍያ ጭነት v1 መልቀቅ ኦዲት የሚችል መዝገብ አለው (ሊደርስ የሚችል M1.4)።

# የመጫኛ v1 ልቀት ውሳኔ (2026-04-28)

- ** ሊቀመንበር፡** የኤስዲኬ ምክር ቤት መሪ (ኤም. ታኬሚያ)
- ** ድምጽ መስጠት አባላት፡** ስዊፍት መሪ፣ CLI ማቆያ፣ ሚስጥራዊ ንብረቶች TL፣ DevRel WG
- ** ታዛቢዎች: ** ፕሮግራም Mgmt, ቴሌሜትሪ ኦፕስ

## ግብዓቶች ተገምግመዋል

1. ** ፈጣን ማያያዣዎች እና አስገቢዎች** — `ShieldRequest`/`UnshieldRequest`፣ async submitters እና Tx ገንቢ ረዳቶች በተመጣጣኝ ሙከራዎች እና docs.【IrohaSwift/ምንጮች/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/ምንጮች/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ergonomics** — `iroha app zk envelope` አጋዥ የስራ ፍሰቶችን ኮድ/መመርመሪያ እና የውድቀት ምርመራዎችን ይሸፍናል፣ ከሮድ ካርታው ergonomics መስፈርት ጋር የተጣጣመ።【crates/iroha_cli/src/zk.rs:1256】
3. ** ቆራጥ ዕቃዎች እና ተመሳሳይ ስብስቦች *** - የጋራ መገልገያ + ዝገት/ፈጣን ማረጋገጫ Norito ባይት/ስህተት ንጣፎችን ለማቆየት aligned crypted_payload_vectors.rs:1】【IrohaSwift/ፈተናዎች/IrohaSwiftTests/ምስጢራዊ ኢንክሪፕትድ ፔይሎድTests.swift:73】

#ውሳኔ

- **የክፍያ ጭነት v1 መልቀቅን አጽድቅ** ለኤስዲኬዎች እና ለ CLI፣ ይህም ስዊፍት የኪስ ቦርሳዎች ያለ ምንም የቧንቧ መስመር ሚስጥራዊ ኤንቨሎፕ እንዲመጡ ያስችላቸዋል።
- ** ሁኔታዎች: *** 
  - በCI ተንሳፋፊ ማንቂያዎች (ከ`scripts/check_norito_bindings_sync.py` ጋር የተሳሰረ) የተመጣጣኝ መገልገያዎችን ያቆዩ።
  - በ`docs/source/confidential_assets.md` (ቀድሞውንም በስዊፍት ኤስዲኬ PR በኩል የዘመነ) ውስጥ ያለውን የመጫወቻ መጽሐፍ ይመዝግቡ።
  - ማንኛውንም የምርት ባንዲራ ከመገልበጥዎ በፊት የካሊብሬሽን + የቴሌሜትሪ ማስረጃን ይቅረጹ (በM2 ስር ይከተላሉ)።

## የተግባር እቃዎች

| ባለቤት | ንጥል | የሚከፈልበት |
|-------|---------|
| ፈጣን አመራር | የGA መገኘቱን + README ቅንጥቦችን ያሳውቁ | 2026-05-01 |
| CLI ማቆያ | `iroha app zk envelope --from-fixture` አጋዥ አክል (አማራጭ) | የኋላ መዝገብ (አይከለከልም) |
| DevRel WG | ከክፍያ v1 መመሪያዎች ጋር የኪስ ቦርሳ ፈጣን ጅምርን ያዘምኑ | 2026-05-05 |

> **ማስታወሻ፡** ይህ ማስታወሻ በ`roadmap.md:2426` ያለውን ጊዜያዊ "በመጠባበቅ ላይ ያለ የምክር ቤት ማፅደቂያ" ጥሪን ይተካ እና የመከታተያ ንጥል M1.4 ን ያረካል። የክትትል እርምጃ ንጥሎች ሲዘጉ `status.md` ያዘምኑ።