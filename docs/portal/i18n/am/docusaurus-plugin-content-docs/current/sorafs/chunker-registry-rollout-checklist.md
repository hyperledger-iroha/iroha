---
id: chunker-registry-rollout-checklist
lang: am
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS የመመዝገቢያ ልቀት ማረጋገጫ ዝርዝር

ይህ የማረጋገጫ ዝርዝር አዲስ chunker መገለጫ ወይም ለማስተዋወቅ የሚያስፈልጉትን ደረጃዎች ይይዛል
ከአስተዳደሩ በኋላ የአቅራቢ ቅበላ ጥቅል ከግምገማ ወደ ምርት
ቻርተር ጸድቋል።

> ** ወሰን፡** የሚቀየሩትን ልቀቶች ሁሉ ይመለከታል
> `sorafs_manifest::chunker_registry`፣ የአቅራቢ ኤንቨሎፕ፣ ወይም የ
> ቀኖናዊ ቋሚ ቅርቅቦች (`fixtures/sorafs_chunker/*`)።

## 1. የቅድመ በረራ ማረጋገጫ

1. መገልገያዎችን እንደገና ማመንጨት እና ውሳኔን ማረጋገጥ፡-
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. ቆራጥነት ሃሽ ውስጥ ያረጋግጡ
   `docs/source/sorafs/reports/sf1_determinism.md` (ወይም የሚመለከተው መገለጫ
   ሪፖርት) ከታደሱ ቅርሶች ጋር ይዛመዳል።
3. `sorafs_manifest::chunker_registry` ማጠናቀሩን ያረጋግጡ
   `ensure_charter_compliance()` በመሮጥ፡
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. የፕሮፖዛል ዶሴ ያዘምኑ፡-
   - `docs/source/sorafs/proposals/<profile>.json`
   - የምክር ቤት ደቂቃዎች መግቢያ በ I18NI0000015X ስር
   - ቆራጥነት ሪፖርት

## 2. የአስተዳደር መፈረም

1. የTooling Working Group ሪፖርት እና የውሳኔ ሃሳብ ለሶራ ያቅርቡ
   የፓርላማ መሠረተ ልማት ፓነል.
2. የማጽደቅ ዝርዝሮችን በ ውስጥ ይመዝግቡ
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. በፓርላማ የተፈረመውን ፖስታ ከመሳሪያዎቹ ጋር ያትሙ፡-
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. ኤንቨሎፑ በአስተዳደር ፈላጊ ረዳት በኩል ተደራሽ መሆኑን ያረጋግጡ፡-
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. የታቀደ ልቀት

ለ [መግለጫ ገላጭ ደብተር](./staging-manifest-playbook) ይመልከቱ
የእነዚህ እርምጃዎች ዝርዝር ጉዞ።

1. I18NT0000001X በ`torii.sorafs` ግኝት የነቃ እና የመግባት ስራ ያሰማሩ
   ማስፈጸሚያ በርቷል (`enforce_admission = true`)።
2. የተፈቀደውን የአገልግሎት አቅራቢ ኤንቨሎፕ ወደ ዝግጅት መዝገብ ቤት ይግፉ
   ማውጫ በ I18NI0000020X የተጠቀሰ።
3. በግኝት ኤፒአይ በኩል የአቅራቢዎች ማስታወቂያ መስፋፋትን ያረጋግጡ፡-
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. የአስተዳዳሪ አርዕስቶችን በማንፀባረቅ/እቅድ የመጨረሻ ነጥቦችን ይለማመዱ፡-
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. የቴሌሜትሪ ዳሽቦርዶችን ያረጋግጡ (`torii_sorafs_*`) እና የማስጠንቀቂያ ደንቦች
   አዲስ መገለጫ ያለ ስህተቶች።

## 4. የምርት ልቀት

1. የዝግጅት ደረጃዎችን ከምርት I18NT0000002X አንጓዎች ጋር ይድገሙ።
2. የማግበሪያ መስኮቱን (ቀን/ሰዓት፣ የእፎይታ ጊዜ፣ የመመለሻ እቅድ) ያሳውቁ
   ኦፕሬተር እና ኤስዲኬ ቻናሎች።
3. የሚለቀቀውን PR ያዋህዱ፡-
   - የተሻሻሉ ዕቃዎች እና ኤንቨሎፕ
   - የሰነድ ለውጦች (ቻርተር ማጣቀሻዎች ፣ የመወሰን ሪፖርት)
   - የመንገድ ካርታ/ሁኔታ አድስ
4. የተለቀቁትን መለያ ስጥ እና የተፈረሙትን ቅርሶች ለፕሮቬንሽን አስቀምጥ።

## 5. ከታቀደው ልጥፍ ኦዲት

1. የመጨረሻ መለኪያዎችን ይያዙ (የግኝት ቆጠራዎች፣ የስኬት መጠን ማምጣት፣ ስህተት
   ሂስቶግራም) ከተለቀቀ በኋላ 24 ሰአት.
2. `status.md`ን ከአጭር ማጠቃለያ እና ከቆራጥነት ዘገባ ጋር ያዘምኑ።
3. ማንኛውንም የመከታተያ ስራዎችን (ለምሳሌ፡ ተጨማሪ የመገለጫ ደራሲ መመሪያ) ያስገቡ
   `roadmap.md`.