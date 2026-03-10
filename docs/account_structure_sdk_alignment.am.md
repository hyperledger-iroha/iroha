---
lang: am
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IH58 ልቀት ማስታወሻ ለኤስዲኬ እና ኮዴክ ባለቤቶች

ቡድኖች፡ Rust SDK፣ TypeScript/JavaScript SDK፣ Python SDK፣ Kotlin SDK፣ Codec tooling

አውድ፡ I18NI0000000X አሁን የመላኪያ IH58 መለያ መታወቂያውን ያንጸባርቃል
ትግበራ. እባክዎ የኤስዲኬ ባህሪን እና ሙከራዎችን ከቀኖናዊው ዝርዝር ጋር ያስተካክሉ።

ቁልፍ ማጣቀሻዎች፡-
- የአድራሻ ኮዴክ + ራስጌ አቀማመጥ - I18NI0000001X §2
- ከርቭ መዝገብ - `docs/source/references/address_curve_registry.md`
- መደበኛ v1 ጎራ አያያዝ - `docs/source/references/address_norm_v1.md`
- ቋሚ ቬክተሮች - `fixtures/account/address_vectors.json`

የእርምጃ እቃዎች፡-
1. ** ቀኖናዊ ውፅዓት፡** I18NI0000005X/ማሳያ IH58 ብቻ መልቀቅ አለበት
   (አይ18NI00000006X ቅጥያ የለም)። ቀኖናዊ ሄክስ ለማረም (`0x...`) ነው።
2. **Accepted inputs:** parsers MUST accept only canonical IH58 account literals. Reject compressed `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **IH58 checksum:** Blake2b-512 ከ `IH58PRE || prefix || payload` በላይ ይጠቀሙ፣ ይውሰዱ
   የመጀመሪያዎቹ 2 ባይት. የታመቀ ፊደል መሠረት **105** ነው።
5. ** ከርቭ ጋቲንግ፡** ኤስዲኬዎች ነባሪ ወደ Ed25519-ብቻ። ለ ግልጽ መርጦ መግባትን ያቅርቡ
   ML-DSA/GOST/SM (ፈጣን የግንባታ ባንዲራዎች፣ JS/Android `configureCurveSupport`)። አድርግ
   sep256k1 በነባሪነት ከዝገት ውጭ ነቅቷል ብለው አያስቡ።
6. ** CAIP-10 የለም: ** እስካሁን የተላከ CAIP-10 ካርታ የለም; አለማጋለጥ ወይም
   በ CAIP-10 ልወጣዎች ላይ የተመሠረተ ነው።

እባክዎ አንዴ ኮዴኮች/ሙከራዎች ከተዘመኑ ያረጋግጡ፤ ክፍት ጥያቄዎችን መከታተል ይቻላል
በመለያ-አድራሻ RFC ክር ውስጥ.