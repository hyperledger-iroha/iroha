---
id: nexus-fee-model
lang: am
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
ይህ ገጽ `docs/source/nexus_fee_model.md` ያንጸባርቃል። ጃፓንኛ፣ ዕብራይስጥ፣ ስፓኒሽ፣ ፖርቱጋልኛ፣ ፈረንሳይኛ፣ ራሽያኛ፣ አረብኛ እና የኡርዱ ትርጉሞች በሚሰደዱበት ጊዜ ሁለቱንም ቅጂዎች አንድ ላይ አቆይ።
::

# Nexus ክፍያ ሞዴል ዝማኔዎች

የተዋሃደ የሰፈራ ራውተር አሁን በሌይን የሚወስኑ ደረሰኞችን ይይዛል
ኦፕሬተሮች የጋዝ ዕዳዎችን ከ I18NT0000001X ክፍያ ሞዴል ጋር ማስታረቅ ይችላሉ።

- ለሙሉ ራውተር አርክቴክቸር፣ ቋት ፖሊሲ፣ ቴሌሜትሪ ማትሪክስ እና ልቀት
  ቅደም ተከተል `docs/settlement-router.md` ይመልከቱ። መመሪያው እንዴት እንደሆነ ያብራራል
  እዚህ የተመዘገቡት መለኪያዎች ከNX-3 የመንገድ ካርታ ሊደርስ ከሚችለው እና እንዴት SREዎች ጋር ይያያዛሉ
  በምርት ውስጥ ራውተርን መከታተል አለበት.
- የጋዝ ንብረት ውቅር (`pipeline.gas.units_per_gas`) ያካትታል ሀ
  `twap_local_per_xor` አስርዮሽ፣ አንድ I18NI0000007X (`tier1`፣ `tier2`፣
  ወይም `tier3`)፣ እና `volatility_class` (`stable`፣ `elevated`፣ I18NI0000014X)።
  እነዚህ ባንዲራዎች የሰፈራ ራውተርን ይመገባሉ ስለዚህም የተገኘው XOR
  ጥቅስ ለመስመሩ ቀኖናዊ TWAP እና የፀጉር አቆራረጥ ደረጃን ይዛመዳል።
- ጋዝ የሚከፍል እያንዳንዱ ግብይት `LaneSettlementReceipt` ይመዘግባል።  እያንዳንዱ
  ደረሰኝ በጠሪው የቀረበውን ምንጭ ለዪን፣ በአካባቢው ያለውን አነስተኛ መጠን፣
  የ XOR ክፍያ ወዲያውኑ፣ ከፀጉር አቆራረጥ በኋላ የሚጠበቀው XOR፣ የተረጋገጠው።
  ልዩነት (`xor_variance_micro`)፣ እና የጊዜ ማህተም በሚሊሰከንዶች።
- የአፈፃፀም ድምር ደረሰኞችን በሌይን/በመረጃ ቦታ ያግዱ እና ያትሟቸዋል።
  በ `lane_settlement_commitments` በ `/v2/sumeragi/status` በኩል።  ድምር
  `total_local_micro`፣ I18NI0000020X፣ እና
  `total_xor_after_haircut_micro` የማታ ላይ ማጠቃለያ ላይ
  ማስታረቅ ወደ ውጭ መላክ.
- አዲስ የ `total_xor_variance_micro` ቆጣሪ ምን ያህል የደህንነት ህዳግ እንደነበረ ይከታተላል
  ጥቅም ላይ የዋለ (በትክክለኛው XOR እና ከፀጉር በኋላ ባለው ጥበቃ መካከል ያለው ልዩነት)
  እና `swap_metadata` የመወሰኛ ልወጣ መለኪያዎችን ይመዘግባል
  (TWAP፣ epsilon፣ liquidity profile እና volatility_class) ኦዲተሮች እንዲችሉ
  ከአሂድ ጊዜ ውቅር ነጻ የዋጋ ግብአቶችን ያረጋግጡ።

ሸማቾች I18NI0000024X ከነባሩ መስመር ጋር ማየት ይችላሉ።
እና የዳታ ስፔስ ቁርጠኝነት ቅጽበተ-ፎቶዎች ያንን ክፍያ ማቋረጦችን፣ የፀጉር መቆራረጥን ደረጃዎችን ለማረጋገጥ፣
እና ስዋፕ አፈጻጸም ከተዋቀረው I18NT0000002X ክፍያ ሞዴል ጋር ይዛመዳል።