---
lang: am
direction: ltr
source: docs/source/examples/sorafs_manifest/cli_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a8209e602132efb6c29962bf09aea8cd74f972fa956ea8a7a1dbac08a7f6f00f
source_last_modified: "2026-01-05T09:28:12.006380+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Manifest CLI End-to-End Example"
translator: machine-google-reviewed
---

# SoraFS አንጸባራቂ CLI ከመጨረሻ እስከ መጨረሻ ምሳሌ

ይህ ምሳሌ የሰነድ ግንባታን ወደ SoraFS በማተም ያልፋል
`sorafs_manifest_stub` CLI ከወሳኝ መቆራረጥ ዕቃዎች ጋር
በ SoraFS Architecture RFC ውስጥ ተገልጿል. ፍሰቱ አንጸባራቂ ትውልድን ይሸፍናል,
የሚጠበቁ ፍተሻዎች፣ የማምጣት-ዕቅድ ማረጋገጫ፣ እና የማገገም ማረጋገጫ ልምምዶች እንዲሁ
ቡድኖች በ CI ውስጥ ተመሳሳይ ደረጃዎችን መክተት ይችላሉ.

## ቅድመ ሁኔታዎች

- የስራ ቦታ ክሎድ እና የመሳሪያ ሰንሰለት ዝግጁ (`cargo`፣ `rustc`)።
- ከ `fixtures/sorafs_chunker` የሚጠበቁ እሴቶች ሊኖሩ የሚችሉ ዕቃዎች ይገኛሉ
  የተገኘ (ለምርት ስራዎች እሴቶቹን ከስደት ደብተር ግቤት ይጎትቱ
  ከቅርስ ጋር የተያያዘ).
- ለማተም የናሙና የመጫኛ ማውጫ (ይህ ምሳሌ `docs/book` ይጠቀማል)።

## ደረጃ 1 — አንጸባራቂ፣ CAR፣ ፊርማዎችን እና እቅድ ማውጣት

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-out target/sorafs/docs.manifest_signatures.json \
  --car-out target/sorafs/docs.car \
  --chunk-fetch-plan-out target/sorafs/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101d0cfa9be459f4a4ba4da51990b75aef262ef546270db0e42d37728755d \
  --dag-codec=0x71 \
  --chunker-profile=sorafs.sf1@1.0.0
```

ትዕዛዙ፡-

- ክፍያውን በ `ChunkProfile::DEFAULT` በኩል ያሰራጫል።
- CARv2 ማህደር እና ቸንክ-ማምጣት እቅድ ያወጣል።
- የ`ManifestV1` መዝገብ ይገነባል፣ አንጸባራቂ ፊርማዎችን ያረጋግጣል (ከቀረበ) እና
  ፖስታውን ይጽፋል.
- ባይት ከተንሳፈፈ ሩጫው እንዳይሳካ የሚጠበቁ ባንዲራዎችን ያስገድዳል።

## ደረጃ 2 — ውጽዓቶችን በ chunk store + PoR ልምምድ ያረጋግጡ

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  target/sorafs/docs.car \
  --manifest target/sorafs/docs.manifest \
  --report-out target/sorafs/docs.manifest_report.json \
  --por-json-out target/sorafs/docs.por.json
```

ይህ CAR ን በ deterministic chunk ማከማቻ በኩል እንደገና ያሰራጫል።
መልሶ ማግኘት የሚቻልበት የናሙና ዛፍ፣ እና ተስማሚ የሆነ አንጸባራቂ ሪፖርት ያወጣል።
የአስተዳደር ግምገማ.

## ደረጃ 3 - የባለብዙ አቅራቢ መልሶ ማግኛን አስመስለው

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=target/sorafs/docs.fetch_plan.json \
  --provider=primary=target/sorafs/docs.car \
  --chunk-receipts-out=target/sorafs/docs.chunk_receipts.json \
  --json-out=target/sorafs/docs.fetch_report.json
```

ለCI አካባቢዎች፣ በየአቅራቢው የተለየ የመክፈያ መንገዶችን ያቅርቡ (ለምሳሌ፣ የተገጠመ
መጫዎቻዎች) የክልሎችን መርሐግብር እና ውድቀት አያያዝን ለመለማመድ።

## ደረጃ 4 - የመመዝገቢያ ደብተርን ይመዝግቡ

በመያዝ ህትመቱን በ`docs/source/sorafs/migration_ledger.md` ውስጥ ያስገቡ

- CID፣ CAR መፈጨት እና የምክር ቤት ፊርማ ሃሽ አሳይ።
- ሁኔታ (`Draft`፣ `Staging`፣ `Pinned`)።
- ወደ CI ሩጫዎች ወይም የአስተዳደር ትኬቶች አገናኞች።

## ደረጃ 5 - በአስተዳደር መሣሪያ በኩል ይሰኩ (መዝገቡ በቀጥታ ሲሰራ)

አንዴ የፒን መዝገብ ቤት ከተሰማራ (በፍልሰት ፍኖተ ካርታ ውስጥ ወሳኝ ምዕራፍ 2)
አንጸባራቂውን በCLI በኩል ያስገቡ፡-

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --plan=target/sorafs/docs.fetch_plan.json \
  --manifest-out target/sorafs/docs.manifest \
  --manifest-signatures-in target/sorafs/docs.manifest_signatures.json \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --council-signature-file <signer_hex>:path/to/signature.bin

cargo run -p sorafs_cli --bin sorafs_pin -- propose \
  --manifest target/sorafs/docs.manifest \
  --manifest-signatures target/sorafs/docs.manifest_signatures.json
```

የፕሮፖዛል ለዪው እና ተከታዩ የተፈቀደ የግብይት ሃሽ መሆን አለባቸው
ለኦዲትነት በፍልሰት ደብተር ግቤት ተይዟል።

## ማፅዳት

በ`target/sorafs/` ስር ያሉ ቅርሶች በማህደር ሊቀመጡ ወይም ወደ የዝግጅት ኖዶች ሊሰቀሉ ይችላሉ።
አንጸባራቂውን፣ ፊርማዎችን፣ CARን እና እቅድን አንድ ላይ በማምጣት ወደ ታች ያኑሩ
ኦፕሬተሮች እና የኤስዲኬ ቡድኖች ስምምነቱን በቁርጠኝነት ማረጋገጥ ይችላሉ።