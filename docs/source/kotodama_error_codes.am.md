---
lang: am
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2025-12-29T18:16:35.974178+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama የማጠናከሪያ ስህተት ኮዶች

የ Kotodama ማጠናከሪያ መሳሪያ እና የ CLI ተጠቃሚዎች የተረጋጋ የስህተት ኮዶችን ያወጣል
የውድቀት መንስኤን በፍጥነት ይረዱ። `koto_compile --explain <code>` ይጠቀሙ
ተጓዳኝ ፍንጭ ለማተም.

| ኮድ | መግለጫ | የተለመደ ማስተካከያ |
|-------|-------------|------------|
| `E0001` | የቅርንጫፍ ኢላማ ለIVM ዝላይ ኢንኮዲንግ ከክልል ውጭ ነው። | በጣም ትልቅ ተግባራትን ይከፋፍሉ ወይም ውስጠ-ግንቡ ይቀንሱ ስለዚህ መሰረታዊ የማገጃ ርቀቶች በ± 1ሚቢ ውስጥ ይቆያሉ። |
| `E0002` | የጥሪ ጣቢያዎች ፈጽሞ ያልተገለጸ ተግባርን ይጠቅሳሉ። | ጠሪውን ያስወገዱ የትየባ፣ የታይነት ማስተካከያዎች ወይም የባህሪ ባንዲራዎች ካሉ ያረጋግጡ። |
| `E0003` | ABI v1 ሳይነቃ የሚበረክት የስቴት ሲካሎች ተለቀቁ። | `CompilerOptions::abi_version = 1` ያቀናብሩ ወይም `meta { abi_version: 1 }` በ `seiyaku` ውል ውስጥ ይጨምሩ። |
| `E0004` | ከንብረት ጋር የተገናኙ ሲሳይሎች ቀጥተኛ ያልሆኑ ጠቋሚዎችን ተቀብለዋል። | `account_id(...)`፣ `asset_definition(...)` ወዘተ ይጠቀሙ ወይም 0 sentinels ለአስተናጋጅ ነባሪዎች ይለፉ። |
| `E0005` | `for`-loop ማስጀመሪያ ዛሬ ከሚደገፈው የበለጠ ውስብስብ ነው። | ከሉፕ በፊት ውስብስብ ማዋቀርን ያንቀሳቅሱ; ቀላል `let`/መግለጫ ማስጀመሪያዎች ብቻ በአሁኑ ጊዜ ተቀባይነት አላቸው። |
| `E0006` | `for`-loop የእርምጃ አንቀጽ ዛሬ ከሚደገፈው የበለጠ ውስብስብ ነው። | የሉፕ ቆጣሪውን በቀላል አገላለጽ ያዘምኑ (ለምሳሌ `i = i + 1`)። |