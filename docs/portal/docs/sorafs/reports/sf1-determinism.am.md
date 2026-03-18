---
lang: am
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7dd8b29e8eb37c2cd78c5dc91ce363bb546fa7e8768f8a2cc86f8b2d9508674
source_last_modified: "2026-01-04T08:19:26.498928+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS SF1 Determinism Dry-Run
summary: Checklist and expected digests for validating the canonical `sorafs.sf1@1.0.0` chunker profile.
translator: machine-google-reviewed
---

# SoraFS SF1 ቆራጥነት ደረቅ አሂድ

ይህ ሪፖርት ለቀኖናዊነት የመነሻውን ደረቅ ሩጫ ይይዛል
I18NI0000001X chunker መገለጫ። Tooling WG የማረጋገጫ ዝርዝሩን እንደገና ማስኬድ አለበት።
ቋሚ ማደሻዎችን ወይም አዲስ የሸማች ቧንቧዎችን ሲያረጋግጡ ከታች። ይመዝገቡ
በሠንጠረዡ ውስጥ ያለው የእያንዳንዱ ትዕዛዝ ውጤት ኦዲት ሊደረግ የሚችል ዱካ ለማቆየት።

#የማረጋገጫ ዝርዝር

| ደረጃ | ትዕዛዝ | የሚጠበቀው ውጤት | ማስታወሻ |
|-------|--------|
| 1 | I18NI0000002X | ሁሉም ፈተናዎች ያልፋሉ; `vectors` እኩልነት ፈተና ተሳክቷል። | የቀኖናዊ ዕቃዎችን ያጠናቅራል እና የዝገት ትግበራን ያዛምዳል። |
| 2 | `ci/check_sorafs_fixtures.sh` | ስክሪፕት 0 ይወጣል; ከዚህ በታች የተገለጹትን መግለጫዎች ዘግቧል። | የቤት ዕቃዎች በንጽህና እንደገና መወለዳቸውን ያረጋግጣል እና ፊርማዎች እንደተያያዙ ይቆያሉ። |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | ለI18NI0000006X ግጥሚያዎች የመመዝገቢያ ገላጭ (`profile_id=1`)። | የመመዝገቢያ ዲበ ውሂብ በማመሳሰል ውስጥ መቆየቱን ያረጋግጣል። |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | ያለ I18NI0000009X ማደስ ይሳካል; አንጸባራቂ እና ፊርማ ፋይሎች አልተቀየሩም። | ለክንች ድንበሮች የመወሰኛ ማረጋገጫ ያቀርባል እና ይገለጣል። |
| 5 | `node scripts/check_sf1_vectors.mjs` | በTyScript ቋሚዎች እና Rust JSON መካከል ምንም ልዩነት እንደሌለ ሪፖርት አድርጓል። | አማራጭ ረዳት; በሁሉም የሩጫ ጊዜዎች መካከል ያለውን እኩልነት ያረጋግጡ (ስክሪፕት በTooling WG ይጠበቃል)። |

## የሚጠበቁ የምግብ አዘገጃጀቶች

- Chunk diest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`፡ `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`
- `sf1_profile_v1.json`: `d89a4fdc030b0c7c4911719ea133c780d9f4610b08eef1d6d0e0ca443391718e`
- `sf1_profile_v1.ts`: `9a3bb8e4d96518b3a0a1301046b2d86a793991959ebdd8adda1fb2988e4292dc`
- `sf1_profile_v1.go`: `0f0348b8751b0f85fe874afda3371af75b78fac5dad65182204dcb3cf3e4c0a1`
- `sf1_profile_v1.rs`፡ `66b5956826c86589a24b71ca6b400cc1335323c6371f1cec9475f09af8743f61`

## የመለያ አጥፋ ምዝግብ ማስታወሻ

| ቀን | ኢንጅነር | የማረጋገጫ ዝርዝር ውጤት | ማስታወሻ |
|-------------|--------|------|
| 2026-02-12 | መገልገያ (LLM) | ❌ አልተሳካም | ደረጃ 1፡ I18NI0000022X የ I18NI0000023X Suite አልተሳካም ምክንያቱም የቤት ዕቃዎች ጊዜያቸው ያለፈባቸው ናቸው። ደረጃ 2፡ I18NI0000024X ውርጃዎች—`manifest_signatures.json` በሪፖ ግዛት ውስጥ ጠፍቷል (በስራ ዛፍ ላይ ተሰርዟል)። ደረጃ 4፡ I18NI0000026X የሰነድ ፋይሉ በማይኖርበት ጊዜ ፊርማዎችን ማረጋገጥ አይችልም። የተፈረሙትን እቃዎች ወደነበሩበት መመለስ (ወይም የምክር ቤት ቁልፍ ማቅረብ) እና ማሰሪያዎችን እንደገና በማፍለቅ ቀኖናዊ እጀታዎች በፈተናዎቹ በሚፈለጉት መሰረት እንዲካተቱ ይመከራል። |
| 2026-02-12 | መገልገያ (LLM) | ✅ አለፈ | በI18NI0000027X በኩል የታደሱ ዕቃዎች፣ ቀኖናዊ እጀታ-ብቻ ተለዋጭ ስም ዝርዝሮችን እና አዲስ አንጸባራቂ መፍጨት `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4`። በ`cargo test -p sorafs_chunker` እና በንፁህ I18NI0000030X ሩጫ (ለቼኩ የተደረደሩ እቃዎች) የተረጋገጠ። ደረጃ 5 የመስቀለኛ ክፍል ረዳት እስኪያርፍ ድረስ በመጠባበቅ ላይ። |
| 2026-02-20 | ማከማቻ Tooling CI | ✅ አለፈ | የፓርላማ ፖስታ (I18NI0000031X) በ `ci/check_sorafs_fixtures.sh` የተገኘ; ስክሪፕት ድጋሚ የመነጨ የቤት እቃዎች፣ የተረጋገጠ አንጸባራቂ መፍጨት `c8c45c025ecee39b5ac5bf3db3dc1e2f97a7eaf7ea0aac72056eedd85439d4e4` እና የዝገት ማሰሪያውን እንደገና አሂድ (Go/Node ደረጃዎች ሲገኝ ይፈጸማሉ) ያለ ምንም ልዩነት። |

የማረጋገጫ ዝርዝሩን ከጨረሰ በኋላ Tooling WG የቀኑን ረድፍ መያያዝ አለበት። ማንኛውም እርምጃ ከሆነ
አልተሳካም፣ እዚህ ጋር የተያያዘ ችግር አስገባ እና ከዚህ በፊት የማሻሻያ ዝርዝሮችን አካትት።
አዲስ መገልገያዎችን ወይም መገለጫዎችን ማጽደቅ።