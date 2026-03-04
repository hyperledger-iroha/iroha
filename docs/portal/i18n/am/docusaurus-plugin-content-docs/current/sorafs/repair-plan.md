---
id: repair-plan
lang: am
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
መስተዋቶች `docs/source/sorafs_repair_plan.md`. የ Sphinx ስብስብ ጡረታ እስኪወጣ ድረስ ሁለቱንም ስሪቶች በማመሳሰል ያቆዩዋቸው።
::

## የአስተዳደር ውሳኔ የህይወት ዑደት
1. የተንሰራፋው ጥገና የጭረት ፕሮፖዛል ረቂቅ ይፈጥራል እና የክርክር መስኮቱን ይክፈቱ።
2. የአስተዳደር መራጮች በክርክር መስኮቱ ወቅት ድምጾችን አጽድቀው/ ውድቅ ያደርጋሉ።
3. በ`escalated_at_unix + dispute_window_secs` ውሳኔው የሚሰላው በቆራጥነት ነው፡ ትንሹ መራጮች፣ ማፅደቆች ውድቅ ማድረጉን ያልፋሉ፣ እና የማጽደቅ ጥምርታ የምልአተ ጉባኤውን ገደብ ያሟላል።
4. የተፈቀዱ ውሳኔዎች የይግባኝ መስኮት ይከፍታሉ; ከI18NI0000002X በፊት የተመዘገቡ ይግባኞች ውሳኔውን ይግባኝ የሚል ምልክት አድርገውበታል።
5. የቅጣት ክዳኖች በሁሉም ሀሳቦች ላይ ተፈጻሚ ይሆናሉ; ከካፒታው በላይ የሚቀርቡት ነገሮች ውድቅ ናቸው።

## የአስተዳደር ማሳደግ ፖሊሲ
የማሳደጊያ ፖሊሲው የመጣው ከ`governance.sorafs_repair_escalation` በ`iroha_config` ነው እና ለእያንዳንዱ የጥገና slash ፕሮፖዛል ተፈጻሚ ነው።

| ቅንብር | ነባሪ | ትርጉም |
|--------|--------|-----|
| `quorum_bps` | 6667 | ከተቆጠሩት ድምጾች መካከል ዝቅተኛ የማጽደቅ ሬሾ (መሰረታዊ ነጥቦች)። |
| `minimum_voters` | 3 | ውሳኔን ለመፍታት የሚፈለጉት አነስተኛ ቁጥር ያላቸው የተለያዩ መራጮች። |
| `dispute_window_secs` | 86400 | ድምጾች ከመጠናቀቁ በፊት (ሰከንዶች) ከጨመረ በኋላ ያለው ጊዜ። |
| `appeal_window_secs` | 604800 | ከተፈቀደ በኋላ ይግባኝ የሚቀበሉበት ጊዜ (ሰከንዶች)። |
| `max_penalty_nano` | 1,000,000,000 | ለጥገና መጨመር (nano-XOR) የሚፈቀደው ከፍተኛ የቅጣት ቅጣት። |

- በጊዜ መርሐግብር የመነጩ ሀሳቦች በ `max_penalty_nano` ተያይዘዋል; ከዋናው በላይ ኦዲተሮች ያቀረቧቸው ውድቅ ናቸው።
- የድምጽ መዝገቦች በ`repair_state.to` ውስጥ በቆራጥ አደራደር (`voter_id` መደርደር) ተከማችተዋል ስለዚህ ሁሉም አንጓዎች አንድ አይነት የውሳኔ ጊዜ ማህተም እና ውጤት ያገኛሉ።