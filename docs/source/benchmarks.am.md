---
lang: am
direction: ltr
source: docs/source/benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5a5420a123c456aad264ceb70d744b20b09848f7dca23700b4ee1370144bb57c
source_last_modified: "2025-12-29T18:16:35.920013+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Benchmarking ሪፖርት

ዝርዝር በእያንዳንዱ አሂድ ቅጽበተ-ፎቶዎች እና የ FASTPQ WP5-B ታሪክ ውስጥ ይኖራሉ
[`benchmarks/history.md`] (benchmarks/history.md); በማያያዝ ጊዜ ያንን መረጃ ጠቋሚ ይጠቀሙ
ወደ የመንገድ ካርታ ግምገማዎች ወይም የኤስአርአይ ኦዲት ቅርሶች። ጋር ያድሱት።
አዲስ ጂፒዩ በተቀረጸ ቁጥር `python3 scripts/fastpq/update_benchmark_history.py`
ወይም Poseidon መሬትን ያሳያል።

## የፍጥነት ማስረጃዎች ጥቅል

እያንዳንዱ ጂፒዩ ወይም የተቀላቀለ ሁነታ ቤንችማርክ የተተገበረውን የፍጥነት ቅንብሮችን ማካተት አለበት።
ስለዚህ WP6-B/WP6-C ከግዜው ቅርሶች ጎን ለጎን የውቅር እኩልነትን ማረጋገጥ ይችላል።

- ከእያንዳንዱ ሩጫ በፊት/በኋላ የሩጫ ጊዜውን ፎቶ ያንሱ፡-
  `cargo xtask acceleration-state --format json > artifacts/acceleration_state_<stamp>.json`
  (በሰው ለሚነበቡ ምዝግብ ማስታወሻዎች `--format table` ይጠቀሙ)። ይህ `enable_{metal,cuda}` ይመዘግባል፣
  የመርክል ገደቦች፣ SHA-2 ሲፒዩ አድልዎ ገደቦች፣ የተገኙት የኋላ ጤና ቢት እና ማንኛውም
  የተጣበቁ እኩልነት ስህተቶች ወይም ምክንያቶችን ያሰናክሉ.
- JSON ከተጠቀለለው የቤንችማርክ ውጤት ቀጥሎ ያከማቹ
  (`artifacts/fastpq_benchmarks/*.json`፣ `benchmarks/poseidon/*.json`፣ Merkle ጠራርጎ
  ይቀርጻል፣ ወዘተ) ስለዚህ ገምጋሚዎች ጊዜን እና ውቅርን በአንድ ላይ ሊለያዩ ይችላሉ።
- Knob ትርጓሜዎች እና ነባሪዎች በ `docs/source/config/acceleration.md` ውስጥ ይኖራሉ; መቼ ነው።
  መሻር ተተግብሯል (ለምሳሌ፡ `ACCEL_MERKLE_MIN_LEAVES_GPU`፣ `ACCEL_ENABLE_CUDA`)፣
  በአስተናጋጆች ላይ ተደጋጋሚ ሙከራዎችን ለማድረግ በሩጫ ሜታዳታ ውስጥ አስባቸው።

## Norito ደረጃ-1 መለኪያ (WP5-B/C)

- ትዕዛዝ: `cargo xtask stage1-bench [--size <bytes|Nk|Nm>]... [--iterations <n>]`
  JSON + Markdown በ `benchmarks/norito_stage1/` በየመጠን ጊዜ ያመነጫል
  ለ scalar vs accelerated structural-index ገንቢ።
- የቅርብ ጊዜ ሩጫዎች (macOS aarch64, dev profile) በቀጥታ በ
  `benchmarks/norito_stage1/latest.{json,md}` እና ትኩስ መቁረጫ CSV ከ
  `examples/stage1_cutover` (`benchmarks/norito_stage1/cutover.csv`) ሲምዲ ያሳያል
  ከ~6-8ኪቢ ወደ ፊት ያሸንፋል። ጂፒዩ/ትይዩ ደረጃ-1 አሁን ወደ **192KiB** ነባሪ ሆኗል
  መቆራረጥ (ለመሻር `NORITO_STAGE1_GPU_MIN_BYTES=<n>`) ማስጀመርን ለማስወገድ
  በትናንሽ ሰነዶች ላይ አፋጣኝ ለትልቅ ጭነት ማፋጠን.

## Enum vs ባህሪ ነገር መላኪያ

- የማጠናቀር ጊዜ (የማረሚያ ግንባታ): 16.58s
- የሩጫ ጊዜ (መስፈርት ፣ ዝቅተኛ ይሻላል)
  - `enum`: 386 p (አማካይ)
  - `trait_object`: 1.56 ns (አማካይ)

እነዚህ መለኪያዎች ከማይክሮ ቤንችማርክ የሚመጡት በቁጥር ላይ የተመሰረተ መላክን ከቦክስ ባህሪይ ነገር ትግበራ ጋር በማነፃፀር ነው።

## Poseidon CUDA batching

የፖሲዶን ቤንችማርክ (`crates/ivm/benches/bench_poseidon.rs`) አሁን ሁለቱንም ነጠላ-ሃሽ ፐርሙቴሽን እና አዲሱን የታጠቁ ረዳቶችን የሚለማመዱ የስራ ጫናዎችን ያካትታል። ክፍሉን በ:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

መስፈርት ውጤቱን በ`target/criterion/poseidon*_many` ይመዘግባል። የጂፒዩ ሰራተኛ ሲገኝ የJSON ማጠቃለያዎችን ወደ ውጭ ይላኩ (ለምሳሌ `target/criterion/**/new/benchmark.json` ወደ `benchmarks/poseidon/criterion_poseidon2_many_cuda.json` ይቅዱ) (ለምሳሌ `target/criterion/**/new/benchmark.json` ወደ `target/criterion/**/new/benchmark.json` ቅዳ) ስለዚህ የታችኛው ቡድኖች በእያንዳንዱ ሲፒዩ ከ CUDA ጋር ማወዳደር ይችላሉ። የተወሰነው የጂፒዩ መስመር ቀጥታ ስርጭት እስኪጀምር ድረስ፣ ማመሳከሪያው ወደ ሲምዲ/ሲፒዩ ትግበራ ይመለሳል እና አሁንም ለቡድን አፈጻጸም ጠቃሚ የመልሶ ማቋቋም መረጃ ይሰጣል።

ለተደጋገሙ ቀረጻዎች (እና የተመጣጣኝ ማስረጃዎችን በጊዜ አቆጣጠር ውሂብ ለማቆየት) ያሂዱ

```bash
cargo xtask poseidon-cuda-bench --json-out benchmarks/poseidon/poseidon_cuda_latest.json \
  --markdown-out benchmarks/poseidon/poseidon_cuda_latest.md --allow-overwrite
```የትኞቹ ዘሮች የ Poseidon2/6 ስብስቦችን ይወስናሉ ፣ የCUDA ጤናን ይመዘግባል/ምክንያቶችን ያሰናክላል ፣ ቼኮች
ከስካላር መንገዱ ጋር ያለው እኩልነት፣ እና ከብረት ጎን ለጎን የኦፕ/ሰከንድ + የፍጥነት ማጠቃለያዎችን ያወጣል።
የአሂድ ጊዜ ሁኔታ (የባህሪ ባንዲራ፣ ተገኝነት፣ የመጨረሻ ስህተት)። ሲፒዩ-ብቻ አስተናጋጆች አሁንም scalar ይጽፋሉ
የጠፋውን አፋጣኝ ያጣቅሱ እና ያስተውሉ፣ ስለዚህ CI ያለ ጂፒዩ እንኳን ቅርሶችን ማተም ይችላል።
ሯጭ ።

## FASTPQ የብረት መለኪያ (አፕል ሲሊከን)

የጂፒዩ መስመር የዘመነ የ`fastpq_metal_bench` ከጫፍ እስከ ጫፍ ሩጫ በማክሮስ 14 (arm64) ከሌይን-ሚዛናዊ መለኪያ ስብስብ፣ 20,000 ምክንያታዊ ረድፎች (ወደ 32,768 የታሸገ) እና 16 የአምድ ቡድኖችን ያዘ። የታሸገው ቅርስ በ`artifacts/fastpq_benchmarks/fastpq_metal_bench_20k_refresh.json` ላይ ይኖራል፣የብረት ዱካ ከቀደሙት ቀረጻዎች ጋር በ `traces/fastpq_metal_trace_*_rows20000_iter5.trace` ስር ይከማቻል። አማካኝ ጊዜዎች (ከ`benchmarks.operations[*]`) አሁን ያንብቡ፡-

| ኦፕሬሽን | ሲፒዩ አማካኝ (ms) | ሜታል አማካኝ (ኤምኤስ) | ፍጥነት (x) |
|------------------|-------------|
| FFT (32,768 ግብዓቶች) | 83.29 | 79.95 | 1.04 |
| IFFT (32,768 ግብዓቶች) | 93.90 | 78.61 | 1.20 |
| LDE (262,144 ግብዓቶች) | 669.54 | 657.67 | 1.02 |
| የፖሲዶን ሃሽ አምዶች (524,288 ግብዓቶች) | 29,087.53 | 30,004.90 | 0.97 |

ምልከታዎች፡-

- FFT/IFFT ሁለቱም ከታደሱት BN254 ከርነሎች ይጠቀማሉ (IFFT የቀደመውን ሪግሬሽን በ ~20%) ያጸዳል።
- LDE እኩልነት አጠገብ ይቆያል; ዜሮ ሙሌት አሁን 33,554,432 የታሸገ ባይት በ18.66ሚሴ አማካይ መዝግቧል ስለዚህ የJSON ጥቅል የወረፋውን ተፅእኖ ይይዛል።
- Poseidon hashing አሁንም በዚህ ሃርድዌር ላይ ሲፒዩ-የታሰረ ነው; የብረታ ብረት መንገዱ የቅርብ ጊዜ የወረፋ መቆጣጠሪያዎችን እስኪቀበል ድረስ ከፖሲዶን ማይክሮቤንች መገለጫዎች ጋር ማወዳደርዎን ይቀጥሉ።
- እያንዳንዱ ቀረጻ አሁን `AccelerationSettings.runtimeState().metal.lastError` ይመዘግባል፣ በመፍቀድ
  መሐንዲሶች የሲፒዩ ውድቀትን በተለየ የአካል ጉዳት ምክንያት ያብራራሉ (የመመሪያ ለውጥ፣
  እኩልነት አለመሳካት ፣ ምንም መሳሪያ የለም) በቀጥታ በቤንችማርክ አርቲፊኬት ውስጥ።

ሩጫውን እንደገና ለማራባት የብረታ ብረት ፍሬዎችን ይገንቡ እና ያሂዱ፡-

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 --output fastpq_metal_bench_20k.json
```

የተገኘውን JSON በ `artifacts/fastpq_benchmarks/` ከብረት ፈለግ ጋር በማያያዝ የመወሰን ማስረጃው ሊባዛ የሚችል ሆኖ እንዲቆይ ያድርጉ።

## FASTPQ CUDA አውቶማቲክ

የCUDA አስተናጋጆች የSM80 ቤንችማርክን በአንድ እርምጃ ማሄድ እና መጠቅለል ይችላሉ፡-

```bash
cargo xtask fastpq-cuda-suite \
  --rows 20000 --iterations 5 --columns 16 \
  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \
  --label device_class=xeon-rtx --device rtx-ada
```

ረዳቱ `fastpq_cuda_bench` ይጣራል፣ በመለያዎች/መሳሪያ/ማስታወሻዎች፣ ክብር
`--require-gpu`፣ እና (በነባሪ) መጠቅለያዎች/በ`scripts/fastpq/wrap_benchmark.py`።
ውጤቶቹ ጥሬውን JSONን፣ የተጠቀለለውን ጥቅል በ`artifacts/fastpq_benchmarks/`፣
እና `<name>_plan.json` ከውጤቱ ቀጥሎ ትክክለኛ ትዕዛዞችን ይመዘግባል/env so
ደረጃ 7 በመላ ጂፒዩ ሯጮች መባዛት የሚችል መቆየትን ያሳያል። `--sign-output` እና ይጨምሩ
ፊርማዎች በሚያስፈልግበት ጊዜ `--gpg-key <id>`; ለመልቀቅ `--dry-run` ይጠቀሙ
አግዳሚ ወንበሩን ሳያስፈጽም እቅድ / መንገዶች.

### የ GA ልቀት ቀረጻ (ማክኦኤስ 14 arm64፣ መስመር-ሚዛናዊ)

WP2-Dን ለማርካት በተመሳሳይ አስተናጋጅ ላይ የመልቀቅ ግንባታን በGA-ዝግጁ መዝግበናል።
ወረፋ ሂዩሪስቲክስ እና እንደ አሳተመ
`fastpq_metal_bench_20k_release_macos14_arm64.json`. ጥበቡ ሁለት ይይዛል
የአምድ ስብስቦች (በሌይን-ሚዛናዊ፣ ወደ 32,768 ረድፎች የታሸገ) እና ፖሲዶን ያካትታል
ለዳሽቦርድ ፍጆታ የማይክሮቤንች ናሙናዎች።| ኦፕሬሽን | ሲፒዩ አማካኝ (ms) | ሜታል አማካኝ (ኤምኤስ) | ፍጥነት | ማስታወሻ |
|------------------|--------
| FFT (32,768 ግብዓቶች) | 12.741 | 10.963 | 1.16× | የጂፒዩ ኮርነሎች የታደሰ የወረፋ ጣራዎችን ይከታተላሉ። |
| IFFT (32,768 ግብዓቶች) | 17.499 | 25.688 | 0.68× | ሪግሬሽን ወደ ወግ አጥባቂ ወረፋ አድናቂ-ውጭ; ሂዩሪስቲክስን ማቀናበርዎን ይቀጥሉ። |
| LDE (262,144 ግብዓቶች) | 68.389 | 65.701 | 1.04× | ዜሮ ሙላ ምዝግብ ማስታወሻዎች 33,554,432 ባይት በ9.651ሚሴ ለሁለቱም ባች። |
| የፖሲዶን ሃሽ አምዶች (524,288 ግብዓቶች) | 1,728.835 | 1,447.076 | 1.19× | የPoseidon ወረፋ ማስተካከያ ከተደረገ በኋላ ጂፒዩ በመጨረሻ ሲፒዩን አሸንፏል። |

በJSON ውስጥ የተካተቱ የፖሲዶን ማይክሮ ቤንች እሴቶች 1.10× ፍጥነት (ነባሪው መስመር) ያሳያሉ።
596.229ms vs scalar 656.251ms በአምስት ድግግሞሾች)፣ ስለዚህ ዳሽቦርዶች አሁን ገበታ ማድረግ ይችላሉ።
ከዋናው አግዳሚ ወንበር ጎን ለጎን የሌይን ማሻሻያዎች። ሩጫውን በ:

```bash
FASTPQ_METAL_LIB=target/release/build/fastpq_prover-*/out/fastpq.metallib \
FASTPQ_METAL_TRACE_CHILD=1 \
cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release \
  -- --rows 20000 --iterations 5 \
  --output fastpq_metal_bench_20k_release_macos14_arm64.json
```

የታሸገውን የJSON እና `FASTPQ_METAL_TRACE_CHILD=1` ዱካዎች ከስር ምልክት አድርገው ያስቀምጡ
`artifacts/fastpq_benchmarks/` ስለዚህ ተከታይ WP2-D/WP2-E ግምገማዎች GA ሊለያዩ ይችላሉ
የሥራ ጫናውን እንደገና ሳያካሂዱ ቀደም ባሉት የማደስ ስራዎች ላይ ይያዙ።

እያንዳንዱ ትኩስ `fastpq_metal_bench` ቀረጻ አሁን ደግሞ `bn254_metrics` ብሎክ ይጽፋል፣
ለሲፒዩ የ `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` ግቤቶችን የሚያጋልጥ
መነሻ መስመር እና የትኛውም የጂፒዩ ጀርባ (ሜታል/CUDA) ንቁ ነበር፣ ** እና *** ሀ
`bn254_dispatch` ብሎክ የታዩትን የክር ቡድን ስፋቶችን ፣ ሎጂካዊ ክር ይመዘግባል
ለነጠላ-አምድ BN254 FFT/LDE መላኪያዎች ቆጠራዎች እና የቧንቧ መስመር ገደቦች። የ
የቤንችማርክ መጠቅለያ ሁለቱንም ካርታዎች ወደ `benchmarks.bn254_*` ይገለበጣል፣ ስለዚህ ዳሽቦርዶች እና
Prometheus ላኪዎች እንደገና ሳይተነተኑ ምልክት የተደረገባቸውን መዘግየት እና ጂኦሜትሪ መቧጠጥ ይችላሉ።
ጥሬው ኦፕሬሽኖች ድርድር. የ `FASTPQ_METAL_THREADGROUP` መሻር አሁን ተፈጻሚ ይሆናል።
BN254 እንክብሎችም እንዲሁ፣ የክር ቡድን ጠራጊዎችን ከአንድ ሊባዙ ይችላሉ። knob.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【ስክሪፕቶች/fastpq/wrap.7】【ስክሪፕቶች/fastpq/wrap.

የታችኛው ዳሽቦርዶች ቀላል እንዲሆኑ፣ `python3 scripts/benchmarks/export_csv.py` ን ያሂዱ
ጥቅል ከያዙ በኋላ. ረዳቱ `poseidon_microbench_*.json` ወደ ውስጥ ይዘረጋል።
የ `.csv` ፋይሎችን በማዛመድ አውቶማቲክ ስራዎች ነባሪ እና ባለከፍተኛ መስመሮችን ሊለያዩ ይችላሉ
ብጁ ተንታኞች.

## ፖሲዶን ማይክሮቤንች (ሜታል)

`fastpq_metal_bench` አሁን እራሱን በ `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ስር እንደገና ያስፈጽማል እና ጊዜዎቹን ወደ `benchmarks.poseidon_microbench` ያስተዋውቃል። የቅርብ ጊዜዎቹን የብረታ ብረት ቀረጻዎች በ`python3 scripts/fastpq/export_poseidon_microbench.py --bundle <wrapped_json>` ወደ ውጭ ላክን እና በ`python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json` ጨምረናቸው ነበር። ከታች ያሉት ማጠቃለያዎች በ `benchmarks/poseidon/` ስር ይኖራሉ፡

| ማጠቃለያ | የተጠቀለለ ጥቅል | ነባሪ አማካኝ (ሚሴ) | Scalar አማካኝ (ms) | ስፒድፕ vs scalar | አምዶች x ግዛቶች | መደጋገም |
|--------|--------------|
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 1,990.49 | 1,994.53 | 1.002 | 64 x 262,144 | 5 |
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2,167.66 | 2,152.18 | 0.993 | 64 x 262,144 | 5 |ሁለቱም ቀረጻዎች 262,144 ስቴቶች በሩጫ (trace log2 = 12) ከአንድ የሙቀት ድግግሞሽ ጋር። የ"ነባሪ" መስመር ከተስተካከለው የብዝሃ-ግዛት ከርነል ጋር ይዛመዳል፣ "ስካላር" ግን ከርነሉን በሌይን ወደ አንድ ሁኔታ ለንፅፅር ይዘጋዋል።

## Merkle ደፍ ጠራርጎ

የ`merkle_threshold` ምሳሌ (`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`) Metal-vs-CPU Merkle hashing ዱካዎችን ያጎላል። የቅርብ ጊዜው የAppleSilicon ቀረጻ (ዳርዊን 25.0.0 arm64፣ `ivm::metal_available()=true`) በ`benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` ከሚዛመደው CSV ወደ ውጭ መላክ ይኖራል። ሲፒዩ-ብቻ macOS 14 የመነሻ መስመሮች በ`benchmarks/merkle_threshold/macos14_arm64_{cpu,metal}.json` ስር ይቀራሉ ሜታል ለሌላቸው አስተናጋጆች።

| ቅጠሎች | ሲፒዩ ምርጥ (ms) | ሜታል ምርጥ (ኤምኤስ) | ፍጥነት |
|--------|------------|-------|
| 1,024 | 23.01 | 19፡69 | 1.17× |
| 4,096 | 50.87 | 62.12 | 0.82× |
| 8,192 | 95.77 | 96.57 | 0.99× |
| 16,384 | 64.48 | 58.98 | 1.09× |
| 32,768 | 109.49 | 87.68 | 1.25× |
| 65,536 | 177.72 | 137.93 | 1.29× |

ትላልቅ የቅጠል ቆጠራዎች ከብረት (1.09-1.29×); ትናንሽ ባልዲዎች አሁንም በሲፒዩ ላይ በፍጥነት ይሰራሉ፣ ስለዚህ CSV ሁለቱንም አምዶች ለመተንተን ያቆያል። የCSV አጋዥ የጂፒዩ vs ሲፒዩ መመለሻ ዳሽቦርዶች እንዲሰለፉ ለማድረግ ከእያንዳንዱ መገለጫ ጎን የ`metal_available` ባንዲራ ይጠብቃል።

የመራቢያ ደረጃዎች፡-

```bash
cargo run --release -p ivm --features metal --example merkle_threshold -- --json \
  > benchmarks/merkle_threshold/<hostname>_$(uname -r)_$(uname -m).json
```

አስተናጋጁ ግልጽ የሆነ ብረት ማንቃትን የሚፈልግ ከሆነ `FASTPQ_METAL_LIB`/`FASTPQ_GPU` ያቀናብሩ እና ሁለቱንም ሲፒዩ + ጂፒዩ ቀረጻዎች WP1-F የመመሪያ ገደቦችን እንዲያስቀምጡ ያድርጉ።

ጭንቅላት ከሌለው ሼል በሚሮጡበት ጊዜ `IVM_DEBUG_METAL_ENUM=1` ወደ መሳሪያ ቆጠራ እና `IVM_FORCE_METAL_ENUM=1`ን ለማለፍ `MTLCreateSystemDefaultDevice()` ያዘጋጁ። CLI የCoreGraphics ክፍለ ጊዜን ያሞቀዋል ** በፊት ** ነባሪውን የብረታ ብረት መሳሪያ በመጠየቅ እና `MTLCopyAllDevices()` ዜሮ ሲመለስ ወደ `MTLCreateSystemDefaultDevice()` ይወድቃል። አስተናጋጁ አሁንም ምንም አይነት መሳሪያ እንደሌለ ካላሳወቀ ቀረጻው `metal_available=false`ን አያቆይም (ጠቃሚ የሲፒዩ መነሻ መስመሮች በ`macos14_arm64_*` ስር ይኖራሉ)፣ የጂፒዩ አስተናጋጆች ግን `FASTPQ_GPU=metal` መንቃት አለባቸው ስለዚህ ጥቅሉ የተመረጠውን የኋላ ክፍል ይመዘግባል።

`fastpq_metal_bench` የ `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` ውጤቶች በጂፒዩ ዱካ ላይ መቆየት አለመቻሉን ከመወሰኑ በፊት በ `FASTPQ_DEBUG_METAL_ENUM=1` በኩል ተመሳሳይ ቁልፍ ያጋልጣል። `FASTPQ_GPU=gpu` አሁንም `backend="none"` በተጠቀለለው JSON ውስጥ ባቀረበ ጊዜ አንቃው ስለዚህ የቀረጻው ጥቅል አስተናጋጁ የብረት ሃርድዌርን እንዴት እንደዘረዘረ በትክክል ይመዘግባል፤ `FASTPQ_GPU=gpu` ሲቀናበር ልጥፉ ወዲያው ይቋረጣል ነገር ግን ምንም ማፍጠኛ ሳይገኝ ማረሚያ ቁልፍ ላይ በመጠቆም የተለቀቀው ቅርቅብ ከግዳጅ ጂፒዩ ጀርባ የሲፒዩ ውድቀትን አይደብቅም። ሩጫ።【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】

የCSV አጋዥ በየመገለጫ ሰንጠረዦች (ለምሳሌ `macos14_arm64_*.csv` እና `takemiyacStudio.lan_25.0.0_arm64.csv`)፣ የ`metal_available` ባንዲራ በመጠበቅ የሪግሬሽን ዳሽቦርዶች የሲፒዩ እና የጂፒዩ መለኪያዎችን ያለአንዳች ተንታኞች እንዲገቡ ያደርጋል።