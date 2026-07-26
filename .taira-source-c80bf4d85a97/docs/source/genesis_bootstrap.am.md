---
lang: am
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የዘፍጥረት ቡትስትራፕ ከታመኑ እኩዮች

Iroha እኩዮች ያለአገር ውስጥ `genesis.file` የተፈረመ የዘፍጥረት ብሎክ ከታመኑ እኩዮቻቸው ማምጣት ይችላሉ
የ Norito-encoded bootstrap ፕሮቶኮልን በመጠቀም።

- ** ፕሮቶኮል፡** እኩዮች `GenesisRequest` (`Preflight` ለሜታዳታ፣ `Fetch` ለክፍያ) እና
  `GenesisResponse` ክፈፎች በ`request_id` ተከፍተዋል። ምላሽ ሰጪዎች የሰንሰለት መታወቂያ፣ የፈራሚ ፐብኪ፣
  ሃሽ, እና የአማራጭ መጠን ፍንጭ; የተጫኑ ጭነቶች የሚመለሱት በ`Fetch` እና የተባዙ የጥያቄ መታወቂያዎች ላይ ብቻ ነው።
  `DuplicateRequest` ተቀበል።
- **ጠባቂዎች:** ምላሽ ሰጪዎች የፈቃድ ዝርዝርን (`genesis.bootstrap_allowlist` ወይም የታመኑ እኩዮችን ያስገድዳሉ)
  ስብስብ)፣ የሰንሰለት-መታወቂያ/የፐብኪ/ሃሽ ማዛመድ፣የታሪፍ ገደቦች (`genesis.bootstrap_response_throttle`) እና ሀ
  የመጠን ካፕ (`genesis.bootstrap_max_bytes`)። ከተፈቀደው ዝርዝር ውጭ ያሉ ጥያቄዎች `NotAllowed` ይቀበላሉ እና
  በተሳሳተ ቁልፍ የተፈረሙ የጫኑ ጭነቶች `MismatchedPubkey` ይቀበላሉ።
- ** የጠያቂ ፍሰት፡** ማከማቻ ባዶ ሲሆን እና `genesis.file` ካልተቀናበረ (እና
  `genesis.bootstrap_enabled=true`)፣ መስቀለኛ መንገድ የታመኑ አቻዎችን ከአማራጭ ጋር ቀድሟል።
  `genesis.expected_hash`፣ከዚያም ክፍያውን ያመጣል፣ፊርማዎችን በ`validate_genesis_block` በኩል ያረጋግጣል፣
  እና እገዳውን ከመተግበሩ በፊት `genesis.bootstrap.nrt` ከኩራ ጋር ይቀጥላል። ቡትስትራፕ ይሞክራል።
  ክብር `genesis.bootstrap_request_timeout`፣ `genesis.bootstrap_retry_interval`፣ እና
  `genesis.bootstrap_max_attempts`.
- **የመውደቅ ሁነታዎች፡** ለሚፈቀዱ ዝርዝር ጥፋቶች፣የሰንሰለት/pubkey/hash አለመዛመድ፣መጠን ጥያቄዎች ውድቅ ተደርገዋል።
  የካፕ ጥሰቶች፣ የዋጋ ገደቦች፣ የጎደሉ የአካባቢ ዘፍጥረት ወይም የተባዙ የጥያቄ መታወቂያዎች። የሚጋጩ ሃሽ
  በመላ እኩዮቻቸው መጭውን ማስወረድ; ምንም ምላሽ ሰጪዎች/የጊዜ ማብቂያዎች ወደ አካባቢያዊ ውቅር አይመለሱም።
- ** የኦፕሬተር እርምጃዎች: *** ቢያንስ አንድ የታመነ እኩያ በትክክለኛው ዘፍጥረት ሊደረስበት የሚችል መሆኑን ያረጋግጡ ፣ ያዋቅሩ
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` እና እንደገና ሞክር ቁልፎች፣ እና
  ያልተመጣጠኑ የክፍያ ጭነቶችን ላለመቀበል በአማራጭ `expected_hash` ይሰኩት። ቀጣይነት ያለው ጭነት ሊሆን ይችላል።
  `genesis.file` ወደ `genesis.bootstrap.nrt` በመጠቆም በቀጣይ ቦት ጫማዎች ላይ እንደገና ጥቅም ላይ ይውላል.