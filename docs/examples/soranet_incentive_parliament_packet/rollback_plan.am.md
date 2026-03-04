---
lang: am
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 47b6ac4be21202943d4145c604557a2ee50823acc139633dd6cf690a81cbce8e
source_last_modified: "2026-01-22T14:35:37.885394+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የማበረታቻ መልሶ ማሰራጫ እቅድ

የአስተዳደር ጥያቄዎች ሀ
ማቆም ወይም የቴሌሜትሪ መከላከያው ከተቃጠለ.

1. ** አውቶሜሽን ፍሪዝ።** በእያንዳንዱ ኦርኬስትራ አስተናጋጅ ላይ ያለውን ማበረታቻ ያቁሙ
   (`systemctl stop soranet-incentives.service` ወይም ተመጣጣኝ መያዣ
   ማሰማራት) እና ሂደቱ ከአሁን በኋላ እየሰራ አለመሆኑን ያረጋግጡ።
2. ** በመጠባበቅ ላይ ያሉ መመሪያዎችን ማፍሰስ.** ሩጫ
   I18NI0000002X
   ምንም የላቀ የክፍያ መመሪያዎች አለመኖራቸውን ለማረጋገጥ። ውጤቱን በማህደር ያስቀምጡ
   ለኦዲት Norito የሚጫኑ ጭነቶች።
3. **የአስተዳደር ማጽደቅን ይሻሩ።** `reward_config.json` አርትዕ፣ አዘጋጅ
   `"budget_approval_id": null`፣ እና አወቃቀሩን በ
   `iroha app sorafs incentives service init` (ወይም `update-config` ከሮጠ
   ረጅም ዕድሜ ያለው ዴሞን)። የክፍያው ሞተር አሁን በመዝጋት ወድቋል
   `MissingBudgetApprovalId`፣ስለዚህ ዴሞን እስከ አዲስ ድረስ ክፍያዎችን ለመቁረጥ ፈቃደኛ አይሆንም።
   ማጽደቅ ሃሽ ተመልሷል። የጂት ቁርጠኝነትን እና SHA-256ን ይመዝግቡ
   በአደጋ ምዝግብ ማስታወሻ ውስጥ የተሻሻለ ውቅረት።
4. **ለሶራ ፓርላማ አሳውቅ።** የፈሰሰውን የክፍያ ደብተር፣ በጥላ የሚመራውን ያያይዙ
   ሪፖርት, እና አጭር ክስተት ማጠቃለያ. የፓርላማ ቃለ ጉባኤ ሃሽ መያዙን ልብ ሊባል ይገባል።
   የተሻረው ውቅር እና ዴሞን የቆመበት ጊዜ።
5. ** የመልስ ማረጋገጫ።** ዴሞን እስከ፡- ድረስ እንዳይሰራ ያድርጉት።
   - የቴሌሜትሪ ማንቂያዎች (`soranet_incentives_rules.yml`) አረንጓዴ ለ>=24 ሰ
   - የግምጃ ቤት ማስታረቅ ሪፖርት ዜሮ የጎደሉ ዝውውሮችን ያሳያል, እና
   - ፓርላማ አዲስ የበጀት ሃሽ አፀደቀ።

አንዴ አስተዳደር የበጀት ማጽደቂያ ሃሽ እንደገና ካወጣ፣ `reward_config.json` ያዘምኑ
በአዲሱ የቴሌሜትሪ የ `shadow-run` ትዕዛዝን በአዲሱ የቴሌሜትሪ ሂደት እንደገና ያሂዱ።
እና ማበረታቻዎችን ዴሞን እንደገና ያስጀምሩ።