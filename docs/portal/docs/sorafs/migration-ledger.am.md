---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ae38d9ff7f10a14e63f6d47490dbbe56c9d3b207a30a5899e63414cb726a88f7
source_last_modified: "2026-01-05T09:28:11.880522+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Migration Ledger"
description: "Canonical change log tracking every migration milestone, owners, and required follow-ups."
translator: machine-google-reviewed
---

> ከ[`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md) የተወሰደ።

# SoraFS የፍልሰት መዝገብ

ይህ መዝገብ በSoraFS ውስጥ የተያዘውን የፍልሰት ለውጥ መዝገብ ያንጸባርቃል
አርኪቴክቸር RFC. ግቤቶች በወሳኝ ደረጃ የተከፋፈሉ እና ውጤታማውን ይዘረዝራሉ
መስኮት፣ ተጽዕኖ የተደረገባቸው ቡድኖች እና አስፈላጊ እርምጃዎች። የፍልሰት እቅድ ዝማኔዎች
ሁለቱንም ይህንን ገጽ እና RFC (`docs/source/sorafs_architecture_rfc.md`) ማሻሻል አለቦት
የታችኛው ተፋሰስ ተጠቃሚዎች እንዲሰለፉ ለማድረግ።

| ወሳኝ ምዕራፍ | ውጤታማ መስኮት | ማጠቃለያ ለውጥ | ተጽዕኖ ያላቸው ቡድኖች | የተግባር እቃዎች | ሁኔታ |
|--------|-------------|
| M1 | ሳምንታት 7–12 | CI የመወሰኛ ዕቃዎችን ያስፈጽማል; በመድረክ ውስጥ የሚገኙ ተለዋጭ ማረጋገጫዎች; መሳሪያ ማድረግ ግልጽ የሆኑ የሚጠበቁ ባንዲራዎችን ያጋልጣል። | ሰነዶች፣ ማከማቻ፣ አስተዳደር | የቤት ዕቃዎች ፊርማ መያዛቸውን ያረጋግጡ፣ በመመዝገቢያ መዝገብ ውስጥ ተለዋጭ ስም ይመዝገቡ፣ የመልቀቂያ ማረጋገጫ ዝርዝሮችን በI18NI0000006X ማስፈጸሚያ ያዘምኑ። | ⏳ በመጠባበቅ ላይ |

እነዚህን ወሳኝ ክንውኖች የሚያመለክቱ የአስተዳደር ቁጥጥር አውሮፕላን ደቂቃዎች ይኖራሉ
`docs/source/sorafs/`. ቡድኖች ከእያንዳንዱ ረድፍ በታች የነጥብ ነጥቦችን ማከል አለባቸው
ታዋቂ ክስተቶች ሲከሰቱ (ለምሳሌ፣ አዲስ ቅጽል ምዝገባዎች፣ የመመዝገቢያ ክስተት
ወደ ኋላ) ኦዲት ሊደረግ የሚችል የወረቀት መንገድ ለማቅረብ.

## የቅርብ ጊዜ ዝመናዎች

- 2025-11-01 - `migration_roadmap.md` ወደ አስተዳደር ምክር ቤት ተሰራጭቷል እና
  ለግምገማ የኦፕሬተር ዝርዝሮች; በሚቀጥለው የምክር ቤት ስብሰባ ላይ መፈረም በመጠባበቅ ላይ
  (ማጣቀሻ፡ `docs/source/sorafs/council_minutes_2025-10-29.md` ክትትል)።
- 2025-11-02 — የፒን መዝገብ ቤት መመዝገቢያ ISI አሁን የጋራ ቸንከር/መመሪያን ተግባራዊ ያደርጋል
  በ`sorafs_manifest` ረዳቶች በኩል ማረጋገጥ፣ በሰንሰለት ላይ ያሉ መንገዶችን በማቆየት
  በ Torii ቼኮች።
- 2026-02-13 — ታክሏል አቅራቢ የታቀዱ ደረጃዎችን (R0–R3) ወደ ደብተር እና
  ተዛማጅ ዳሽቦርዶችን እና የኦፕሬተር መመሪያን አሳትሟል
  (`provider_advert_rollout.md`፣ `grafana_sorafs_admission.json`)።