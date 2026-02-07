---
id: chunker-registry-charter
lang: am
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS Chunker Registry Governance Charter

> ** የጸደቀው: *** 2025-10-29 በሶራ ፓርላማ የመሠረተ ልማት ፓነል (ይመልከቱ)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)። ማንኛውም ማሻሻያ ሀ
> መደበኛ አስተዳደር ድምጽ; የማስፈጸሚያ ቡድኖች ይህንን ሰነድ እንደ ማስተናገድ አለባቸው
> የሚተካ ቻርተር እስኪጸድቅ ድረስ መደበኛ።

ይህ ቻርተር SoraFS chunkerን ለማሻሻል ሂደቱን እና ሚናዎችን ይገልጻል
መዝገብ ቤት. ምን ያህል አዲስ እንደሆነ በመግለጽ የChunker መገለጫ ደራሲ መመሪያን (./chunker-profile-authoring.md) ያሟላል።

## ወሰን

ቻርተሩ በ`sorafs_manifest::chunker_registry` እና በእያንዳንዱ ግቤት ላይ ተፈጻሚ ይሆናል።
መዝገቡን ለሚበላው ማንኛውም መሳሪያ (የግልጽ CLI፣ አቅራቢ-ማስታወቂያ CLI፣
ኤስዲኬዎች)። ተለዋጭ ስም እና ተለዋጭ ስሞችን ያስፈጽማል
`chunker_registry::ensure_charter_compliance()`:

- የመገለጫ መታወቂያዎች በነጠላነት የሚጨምሩ አዎንታዊ ኢንቲጀር ናቸው።
- ቀኖናዊው እጀታ I18NI0000009X ** አለበት ** እንደ መጀመሪያው መታየት አለበት
- የአሊያስ ሕብረቁምፊዎች የተስተካከሉ፣ ልዩ ናቸው እና ከቀኖናዊ እጀታዎች ጋር አይጋጩም።
  የሌሎች ግቤቶች.

## ሚናዎች

- **ደራሲ(ዎች)** - ፕሮፖዛሉን አዘጋጁ፣ መጫዎቻዎችን ማደስ እና መሰብሰብ
  የመወሰን ማስረጃ.
- **የመሳሪያ ስራ ቡድን (TWG)** - የታተመውን በመጠቀም ሃሳቡን ያረጋግጣል
  የማረጋገጫ ዝርዝሮች እና የመመዝገቢያ ልዩነቶች መያዛቸውን ያረጋግጣል።
- **የመንግስት ምክር ቤት (ጂሲ)** - የTWG ሪፖርትን ይገመግማል፣ ፕሮፖዛሉን ይፈርማል
  ኤንቨሎፕ፣ እና የሕትመት/የማቋረጫ ጊዜን ያጸድቃል።
- ** የማከማቻ ቡድን *** - የመመዝገቢያውን አተገባበር ይጠብቃል እና ያትማል
  የሰነድ ዝማኔዎች.

## የህይወት ዑደት የስራ ፍሰት

1. **የፕሮፖዛል ማስረከብ**
   - ደራሲው የማረጋገጫ ዝርዝርን ከደራሲው መመሪያ ያካሂዳል እና ይፈጥራል
     አንድ `ChunkerProfileProposalV1` JSON ስር
     `docs/source/sorafs/proposals/`.
   - የ CLI ውፅዓትን ያካትቱ፡
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - መገልገያዎችን፣ ፕሮፖዛል፣ የውሳኔ አሰጣጥ ሪፖርት እና መዝገብ የያዘ PR አስገባ
     ዝማኔዎች.

2. **የመሳሪያ ግምገማ (TWG)**
   - የማረጋገጫ ዝርዝሩን እንደገና ያጫውቱ (ቋሚዎች ፣ ፉዝ ፣ አንጸባራቂ/PoR ቧንቧ)።
   - `cargo test -p sorafs_car --chunker-registry` ን ያሂዱ እና ያረጋግጡ
     `ensure_charter_compliance()` ከአዲሱ ግቤት ጋር ያልፋል።
   - የCLI ባህሪን ያረጋግጡ (`--list-profiles`፣ `--promote-profile`፣ ዥረት መልቀቅ
     `--json-out=-`) የተዘመኑትን ተለዋጭ ስሞች እና መያዣዎች ያንፀባርቃል።
   - ግኝቶችን እና የማለፊያ/ውድቀት ሁኔታን የሚያጠቃልል አጭር ዘገባ አዘጋጅ።

3. **የካውንስል ማጽደቅ (ጂሲ)**
   - የTWG ሪፖርትን እና የፕሮፖዛል ሜታዳታን ይገምግሙ።
   - የፕሮፖዛል መፍጫውን ይፈርሙ (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     እና የምክር ቤቱ ፖስታ ላይ ፊርማዎችን በማያያዝ ከ
     የቤት እቃዎች.
   - የምርጫውን ውጤት በአስተዳደር ቃለ ጉባኤ ውስጥ ይመዝግቡ።

4. ** ሕትመት ***
   - PRን ያዋህዱ፣ በማዘመን፡-
     - `sorafs_manifest::chunker_registry_data`.
     - ሰነድ (I18NI0000019X፣ የደራሲ/የተግባር መመሪያዎች)።
     - ቋሚዎች እና ቆራጥነት ሪፖርቶች.
   - ስለ አዲሱ መገለጫ እና የታቀደ ልቀት ኦፕሬተሮችን እና የኤስዲኬ ቡድኖችን ያሳውቁ።

5. ** መገለጽ / ጀንበር ስትጠልቅ**
   - ነባር መገለጫን የሚተካ ሀሳቦች ባለሁለት ህትመትን ማካተት አለባቸው
     መስኮት (የጸጋ ጊዜ) እና የማሻሻያ እቅድ.
     በመዝገቡ ውስጥ እና የፍልሰት ደብተርን ያዘምኑ።

6. **የአደጋ ጊዜ ለውጦች**
   - ማስወገድ ወይም ሙቅ መጠገኛዎች በአብላጫ ድምጽ የምክር ቤት ድምጽ ያስፈልጋቸዋል።
   - TWG የአደጋ ቅነሳ እርምጃዎችን መመዝገብ እና የክስተቱን ምዝግብ ማዘመን አለበት።

## የመሳሪያዎች ተስፋዎች

- `sorafs_manifest_chunk_store` እና `sorafs_manifest_stub` ያጋልጣሉ፡
  - `--list-profiles` ለምዝገባ ፍተሻ።
  - `--promote-profile=<handle>` ጥቅም ላይ የዋለው ቀኖናዊ ሜታዳታ ብሎክ ለማመንጨት
    መገለጫ ሲያስተዋውቁ።
  - `--json-out=-` ሪፖርቶችን ወደ stdout ለማሰራጨት ፣ ሊባዛ የሚችል ግምገማን ያስችላል
    መዝገቦች.
- `ensure_charter_compliance()` በተዛማጅ ሁለትዮሽ ውስጥ በሚነሳበት ጊዜ ተጠርቷል።
  (`manifest_chunk_store`፣ `provider_advert_stub`)። አዲስ ከሆነ የCI ፈተናዎች መውደቅ አለባቸው
  ግቤቶች ቻርተሩን ይጥሳሉ.

## መዝገብ መያዝ

- ሁሉንም የመወሰን ሪፖርቶችን በ `docs/source/sorafs/reports/` ውስጥ ያከማቹ።
- የምክር ቤት ደቂቃዎች chunker ውሳኔዎችን በመጥቀስ በቀጥታ ስር
  `docs/source/sorafs/migration_ledger.md`.
- ከእያንዳንዱ ዋና የመመዝገቢያ ለውጥ በኋላ `roadmap.md` እና `status.md` ያዘምኑ።

## ዋቢዎች

- የደራሲ መመሪያ፡ [Chunker መገለጫ ደራሲ መመሪያ](./chunker-profile-authoring.md)
- የተግባር ማረጋገጫ ዝርዝር: `docs/source/sorafs/chunker_conformance.md`
- የመመዝገቢያ ማጣቀሻ፡ [Chunker መገለጫ መዝገብ](./chunker-registry.md)