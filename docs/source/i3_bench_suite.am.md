---
lang: am
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 Bench Suite

የ Iroha 3 አግዳሚ ወንበሮች በእቃ መያዢያ ጊዜ የምንታመንባቸውን ሞቃት መንገዶችን ያሳልፋሉ ፣ ክፍያ
ኃይል መሙላት፣ የማረጋገጫ ማረጋገጫ፣ መርሐግብር ማውጣት እና የማረጋገጫ የመጨረሻ ነጥቦች። እንደ አንድ ይሰራል
`xtask` ትእዛዝ ከሚወስኑ መገልገያዎች (ቋሚ ዘሮች ፣ ቋሚ ቁልፍ ቁሳቁስ ፣
እና የተረጋጋ የጥያቄ ጭነት) ስለዚህ ውጤቶቹ በአስተናጋጆች ውስጥ ሊባዙ ይችላሉ።

## ክፍሉን በማስኬድ ላይ

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

ባንዲራዎች፡

- `--iterations` ድግግሞሾችን በአንድ ሁኔታ ናሙና ይቆጣጠራል (ነባሪ፡ 64)።
- `--sample-count` ሚዲያን ለማስላት እያንዳንዱን ሁኔታ ይደግማል (ነባሪ፡ 5)።
- `--json-out|--csv-out|--markdown-out` የውጤት ቅርሶችን ይምረጡ (ሁሉም አማራጭ)።
- `--threshold` ሚዲያን ከመነሻ ወሰኖች ጋር ያወዳድራል (`--no-threshold` አዘጋጅ
  መዝለል)።
- `--flamegraph-hint` የማርክዳውን ዘገባ በ`cargo flamegraph` ያብራራል
  አንድ ሁኔታን ለመግለጽ ትእዛዝ።

CI ሙጫ በ `ci/i3_bench_suite.sh` ውስጥ ይኖራል እና ከላይ ባሉት መንገዶች ላይ ነባሪዎች; አዘጋጅ
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` በምሽት ምሽቶች ውስጥ የሩጫ ጊዜን ለማስተካከል።

## ሁኔታዎች

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — ከፋይ vs ስፖንሰር ዴቢት
  እና ጉድለት አለመቀበል.
- `staking_bond` / `staking_slash` — ማስያዣ/የማያያዝ ወረፋ ከ ጋር እና ያለ
  መጨፍጨፍ.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` -
  የምስክር ወረቀቶች፣ የጄዲጂ ማረጋገጫዎች እና ድልድይ ላይ የፊርማ ማረጋገጫ
  የማረጋገጫ ጭነቶች.
- `commit_cert_assembly` - ለቁርጠኝነት የምስክር ወረቀቶች ስብስብ።
- `access_scheduler` - ግጭትን የሚያውቅ የመዳረሻ ስብስብ መርሐግብር።
- `torii_proof_endpoint` — የአክሱም ማረጋገጫ የመጨረሻ ነጥብ ትንተና + የማረጋገጫ ዙር ጉዞ።

እያንዳንዱ ሁኔታ መካከለኛ ናኖሴኮንዶችን በየድግግሞሹ ይመዘግባል፣ ውጤቱን እና ሀ
ለፈጣን መመለሻዎች ቆራጥ ምደባ ቆጣሪ። ገደቦች ይኖራሉ
`benchmarks/i3/thresholds.json`; ሃርድዌር በሚቀየርበት ጊዜ እዛው ይገድባል እና
አዲሱን ቅርስ ከሪፖርቱ ጋር ያከናውኑ።

## መላ መፈለግ

- ጩኸት የሚያስከትሉ ለውጦችን ለማስወገድ ማስረጃ በሚሰበስብበት ጊዜ የሲፒዩ ድግግሞሽ/ገዥን ይሰኩ።
- ለዳሰሳ ሩጫዎች `--no-threshold` ይጠቀሙ፣ ከዚያ የመነሻ መስመሩ ከተጠናቀቀ በኋላ እንደገና አንቃ
  ታደሰ
- አንድ ነጠላ ሁኔታን ለመዘርዘር `--iterations 1` ያቀናብሩ እና በስር እንደገና ያሂዱ
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.