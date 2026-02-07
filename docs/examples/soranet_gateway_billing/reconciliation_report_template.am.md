---
lang: am
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የሶራግሎባል ጌትዌይ የሂሳብ አከፋፈል ማስታረቅ

- ** መስኮት: ** `<from>/<to>`
- ** ተከራይ: *** `<tenant-id>`
- ** ካታሎግ ስሪት: ** `<catalog-version>`
- ** የአጠቃቀም ቅጽበታዊ ገጽ እይታ: ** `<path or hash>`
- **ጠባቂዎች፡** ለስላሳ ኮፍያ `<soft-cap-xor> XOR`፣ ጠንካራ ካፕ `<hard-cap-xor> XOR`፣ የማንቂያ ገደብ `<alert-threshold>%`
- ** ከፋይ -> ግምጃ ቤት፡** `<payer>` -> `<treasury>` በ`<asset-definition>`
- ** ጠቅላላ ክፍያ:** I18NI0000011X (`<total-micros>` ማይክሮ-XOR)

## የመስመር ንጥል ፍተሻዎች
- [ ] የአጠቃቀም ግቤቶች የካታሎግ ሜትር መታወቂያዎችን እና ትክክለኛ የሂሳብ አከፋፈል ክልሎችን ብቻ ይሸፍናሉ።
- [ ] የመጠን አሃዶች ከካታሎግ ፍቺዎች (ጥያቄዎች፣ ጂቢ፣ ኤምኤስ፣ ወዘተ) ጋር ይዛመዳሉ።
- [ ] የክልል ማባዣዎች እና የቅናሽ ደረጃዎች በካታሎግ ተተግብረዋል።
- [ ] CSV/ፓርኬት ወደ ውጭ መላክ ከJSON የክፍያ መጠየቂያ መስመር ዕቃዎች ጋር ይዛመዳል

## Guardrail ግምገማ
- [ ] ለስላሳ ቆብ ማንቂያ ጣራ ላይ ደርሷል? `<yes/no>` (አዎ ከሆነ የማንቂያ ማስረጃን ያያይዙ)
- [ ] ጠንካራ ካፕ ታልፏል? `<yes/no>` (አዎ ከሆነ፣ መሻር ማጽደቅን ያያይዙ)
- [ ] ዝቅተኛው የክፍያ መጠየቂያ ወለል ረክቷል።

## የመመዝገቢያ ትንበያ
- [] የዝውውር ባች ጠቅላላ `total_micros` ደረሰኝ ውስጥ እኩል ነው።
- [ ] የንብረት ትርጉም ከክፍያ ምንዛሬ ጋር ይዛመዳል
- [ ] ከፋይ እና የግምጃ ቤት ሒሳቦች ከተከራይ እና ከመዝገብ ኦፕሬተር ጋር ይጣጣማሉ
- [ ] Norito/JSON ቅርሶች ለኦዲት መልሶ ማጫወት ተያይዘዋል

## ሙግት/ማስተካከያ ማስታወሻዎች
- የታየ ልዩነት: `<variance detail>`
- የታቀደ ማስተካከያ: `<delta and rationale>`
- ደጋፊ ማስረጃ: `<logs/dashboards/alerts>`

## ማጽደቆች
- የሂሳብ አከፋፈል ተንታኝ፡ `<name + signature>`
- የግምጃ ቤት ገምጋሚ፡- `<name + signature>`
- የአስተዳደር ፓኬት ሃሽ፡ `<hash/reference>`