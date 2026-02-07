---
id: chunker-conformance
lang: am
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Conformance Guide
sidebar_label: Chunker Conformance
description: Requirements and workflows for preserving the deterministic SF1 chunker profile across fixtures and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: ማስታወሻ ቀኖናዊ ምንጭ
::

ይህ መመሪያ እያንዳንዱ ትግበራ ለመቆየት መከተል ያለባቸውን መስፈርቶች ያዘጋጃል።
ከI18NT0000000X መወሰኛ chunker መገለጫ (SF1) ጋር የተስተካከለ። እንዲሁም
የሥራ ሂደትን ፣ የመፈረሚያ ፖሊሲን እና የማረጋገጫ ደረጃዎችን ይመዘግባል
በሁሉም ኤስዲኬዎች ውስጥ ያሉ ቋሚ ሸማቾች እንደተመሳሰሉ ይቆያሉ።

## ቀኖናዊ መገለጫ

- የመገለጫ እጀታ: I18NI0000003X
- የግቤት ዘር (ሄክስ): `0000000000dec0ded`
- የዒላማ መጠን፡ 262144 ባይት (256ኪባ)
ዝቅተኛ መጠን፡ 65536 ባይት (64ኪባ)
ከፍተኛ መጠን፡ 524288 ባይት (512ኪባ)
- ሮሊንግ ፖሊኖሚል፡ `0x3DA3358B4DC173`
- የማርሽ ሰንጠረዥ ዘር: `sorafs-v1-gear`
- መስበር ጭምብል: I18NI0000007X

የማጣቀሻ ትግበራ፡ `sorafs_chunker::chunk_bytes_with_digests_profile`.
ማንኛውም የሲምዲ ማጣደፍ አንድ አይነት ወሰኖች እና መፍጨት አለበት።

## ቋሚ ጥቅል

`cargo run --locked -p sorafs_chunker --bin export_vectors` እንደገና ያድሳል
የሚከተሉትን ፋይሎች በ`fixtures/sorafs_chunker/` ያዘጋጃል እና ያወጣል፡

- `sf1_profile_v1.{json,rs,ts,go}` - ቀኖናዊ ቁርጥራጭ ድንበሮች ለዝገት ፣
  TypeScript እና Go ሸማቾች እያንዳንዱ ፋይል ቀኖናዊ እጀታውን እንደ የ
  በ `profile_aliases` ውስጥ የመጀመሪያ (እና ብቻ) ግቤት። ትዕዛዙ የሚተገበረው በ
  `ensure_charter_compliance` እና መቀየር የለበትም።
- `manifest_blake3.json` — BLAKE3-የተረጋገጠ አንጸባራቂ እያንዳንዱን ቋሚ ፋይል ይሸፍናል።
- `manifest_signatures.json` - የምክር ቤት ፊርማዎች (Ed25519) ከማንፀባረቂያው በላይ
  መፈጨት ።
- `sf1_profile_v1_backpressure.json` እና ጥሬ ኮርፖራ በ `fuzz/` ውስጥ -
  በ chunker የኋላ-ግፊት ሙከራዎች ጥቅም ላይ የሚውሉ ቆራጥ የዥረት ሁኔታዎች።

### የመፈረም ፖሊሲ

ቋሚ ዳግም መወለድ ** አለበት *** ትክክለኛ የምክር ቤት ፊርማ ማካተት አለበት። ጀነሬተር
`--allow-unsigned` በግልጽ ካልተላለፈ በስተቀር ያልተፈረመ ውፅዓት ውድቅ ያደርጋል (የታሰበ)
ለአካባቢያዊ ሙከራዎች ብቻ). የፊርማ ፖስታዎች አባሪ ብቻ እና ናቸው።
በአንድ ፈራሚ የተቀነሰ።

የምክር ቤት ፊርማ ለመጨመር፡-

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## ማረጋገጫ

የ CI አጋዥ I18NI0000019X ጄነሬተርን በድጋሚ ያጫውታል።
`--locked`. የቤት እቃዎች ተንሸራታች ወይም ፊርማዎች ከጠፉ ስራው አይሳካም። ተጠቀም
ይህ ስክሪፕት በምሽት የስራ ፍሰቶች እና የቋሚ ለውጦችን ከማቅረቡ በፊት።

በእጅ የማረጋገጫ ደረጃዎች:

1. `cargo test -p sorafs_chunker` አሂድ.
2. በአገር ውስጥ I18NI0000022X ይደውሉ።
3. I18NI0000023X ንጹህ መሆኑን ያረጋግጡ።

## Playbookን አሻሽል።

አዲስ chunker መገለጫ ሲያቀርቡ ወይም SF1 ሲያዘምኑ፡-

በተጨማሪ ይመልከቱ፡ [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) ለ
የሜታዳታ መስፈርቶች፣ የፕሮፖዛል አብነቶች እና የማረጋገጫ ዝርዝሮች።

1. I18NI0000025X (RFC SF-1 ይመልከቱ) በአዲስ መመዘኛዎች ይቀርጹ።
2. መገልገያዎችን በ `export_vectors` በኩል ያድሱ እና አዲሱን አንጸባራቂ መፍጨት ይቅዱ።
3. መግለጫውን ከሚፈለገው የምክር ቤት ምልአተ ጉባኤ ጋር ይፈርሙ። ሁሉም ፊርማዎች መሆን አለባቸው
   ከ `manifest_signatures.json` ጋር ተያይዟል።
4. የተጎዱ የኤስዲኬ መጫዎቻዎችን (Rust/Go/TS) ያዘምኑ እና የአሂድ ጊዜ ተሻጋሪነትን ያረጋግጡ።
5. መለኪያዎች ከተቀየሩ fuzz corpora ያድሱ።
6. ይህንን መመሪያ በአዲሱ የመገለጫ እጀታ፣ ዘሮች እና መፍጨት ያዘምኑት።
7. ለውጡን ከተዘመኑ ሙከራዎች እና የመንገድ ካርታ ማሻሻያዎች ጋር ያቅርቡ።

ይህን ሂደት ሳይከተሉ የቁርጥ ድንበሮችን ወይም መፈጨትን የሚነኩ ለውጦች
ልክ ያልሆኑ ናቸው እና መዋሃድ የለባቸውም።